"""Edge TTS (unofficial Microsoft online voices). Word events give exact timing.

Edge is free but unofficial: personal or non-commercial use, and it may break.
The core says so in every `explain` line and never uses it under `--local-only`.
"""

from __future__ import annotations

import asyncio
import json
import re
from pathlib import Path

import edge_tts

from .rpc import ToolError, tool

PROVIDER_REFUSED = -32010  # Edge refused the voice or region (HTTP 403): no retry
PROVIDER_RETRYABLE = -32011  # network or service hiccup: one retry is safe
# Measured 3 Nov 2026: about 1 call in 12 ends in NoAudioReceived at random, and the same text then works.
RETRY_WAITS = (1.0, 3.0)


# Edge streams at about real time (measured 3 Nov 2026: 960 words, 292 s of audio, in 269 s). A long text goes out in
# sentence chunks, a few at once, and the parts are joined. Edge's MP3 is constant bitrate, so a part's length is its
# byte count: 48 kbit/s.
CHUNK_WORDS = 60
PARALLEL = 4
MP3_BYTES_PER_S = 48_000 / 8


def _chunks(text: str) -> list[str]:
    """Sentences grouped up to about CHUNK_WORDS words; a text under 80 words stays whole."""
    if len(text.split()) < 80:
        return [text]
    # A sentence ends at . ! ? (or the Arabic marks) followed by a space or the end, so "3.5", "e.g." inside a word
    # and "example.com" stay whole, and "?!" stays with its sentence.
    sentences = [p.strip() for p in re.split(r"(?<=[.!?\u061f\u06d4])\s+", text) if p.strip()]
    out: list[str] = []
    for s in sentences:
        if out and len(out[-1].split()) + len(s.split()) <= CHUNK_WORDS:
            out[-1] += " " + s
        else:
            out.append(s)
    return out


async def _one(text: str, voice: str, rate: str) -> tuple[bytes, list[dict]]:
    data, words = bytearray(), []
    async for ch in edge_tts.Communicate(text, voice, rate=rate, boundary="WordBoundary").stream():
        if ch["type"] == "audio":
            data += ch["data"]
        elif ch["type"] == "WordBoundary":
            start = ch["offset"] / 1e7
            words.append({"text": ch["text"], "start": start, "end": start + ch["duration"] / 1e7})
    return bytes(data), words


async def _synth(text: str, voice: str, rate: str, audio: Path) -> list[dict]:
    gate = asyncio.Semaphore(PARALLEL)

    async def part(t: str):
        # Each part retries on its own: Edge drops about 1 call in 12, so with 20 parts a single failure would
        # otherwise fail (and restart) the whole script most of the time. A 403 is final.
        async with gate:
            for wait in (*RETRY_WAITS, None):
                try:
                    data, words = await _one(t, voice, rate)
                    if data and words:
                        return data, words
                    err: Exception = RuntimeError("Edge TTS returned no audio")
                except Exception as e:
                    if _refused(e):
                        raise
                    err = e
                if wait is None:
                    raise err
                await asyncio.sleep(wait)

    parts = await asyncio.gather(*(part(t) for t in _chunks(text)))
    words, t0 = [], 0.0
    with audio.open("wb") as f:
        for data, ws in parts:
            f.write(data)
            words += [{"text": w["text"], "start": round(w["start"] + t0, 3), "end": round(w["end"] + t0, 3)} for w in ws]
            t0 += len(data) / MP3_BYTES_PER_S
    return words


def _refused(e: Exception) -> bool:
    return getattr(e, "status", None) == 403 or "403" in str(e)


@tool("tts.edge", network=True, cost="free", deterministic=False, timing="exact",
      terms="unofficial, personal or non-commercial, may break")
def synthesize(input: dict, ctx) -> dict:
    out = Path(input["out_dir"])
    out.mkdir(parents=True, exist_ok=True)
    audio, words_file = out / "voice.mp3", out / "words.json"
    ctx.event("progress", done=0, total=1, explain=f"Edge TTS {input['voice']}")
    # Retries happen per part inside _synth (1 s, then 3 s); this is the final answer after them.
    tries = len(RETRY_WAITS) + 1
    try:
        words = asyncio.run(_synth(input["text"], input["voice"], input.get("rate", "+0%"), audio))
    except Exception as e:
        if _refused(e):
            raise ToolError(PROVIDER_REFUSED, f"Edge refused voice {input['voice']} (403)") from e
        raise ToolError(PROVIDER_RETRYABLE, f"Edge TTS failed after {tries} tries: {type(e).__name__}: {e}") from e
    if not words or audio.stat().st_size == 0:
        raise ToolError(PROVIDER_RETRYABLE, "Edge TTS returned no audio")
    words_file.write_text(json.dumps(words, ensure_ascii=False), encoding="utf-8")
    ctx.event("progress", done=1, total=1, explain=f"{len(words)} words")
    return {"artifacts": {"audio": str(audio), "words": str(words_file)}, "timing": "exact", "cost_usd": 0}
