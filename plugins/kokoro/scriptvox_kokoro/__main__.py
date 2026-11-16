"""Plugin process: the same RVX-RPC as the worker, with one tool, `tts.kokoro`.

Kokoro gives no word events, so word times are estimated: the audio length is
spread over the words by their character count (`timing: estimated`).
"""

from __future__ import annotations

import json
import os
import wave
from pathlib import Path

from scriptvox_worker.rpc import ToolError, serve, tool


def estimate_words(text: str, seconds: float) -> list[dict]:
    words = text.split()
    total = sum(len(w) for w in words) or 1
    t, out = 0.0, []
    for w in words:
        d = seconds * len(w) / total
        out.append({"text": w, "start": round(t, 3), "end": round(t + d * 0.92, 3)})
        t += d
    return out


@tool("tts.kokoro", network=False, cost="free", deterministic=True, timing="estimated",
      terms="local; Kokoro model Apache-2.0; G2P GPL-3.0 (separate plugin process)")
def synthesize(input: dict, ctx) -> dict:
    import numpy as np
    from kokoro_onnx import Kokoro

    model, voices = os.environ.get("SCRIPTVOX_KOKORO_MODEL"), os.environ.get("SCRIPTVOX_KOKORO_VOICES")
    if not (model and voices and Path(model).is_file() and Path(voices).is_file()):
        raise ToolError(-32010, "the Kokoro model is not installed: run `scriptvox setup voice-local`")
    ctx.event("progress", done=0, total=1, explain=f"Kokoro {input['voice']} (local)")
    samples, rate = Kokoro(model, voices).create(input["text"], voice=input["voice"], speed=1.0, lang="en-us")
    out = Path(input["out_dir"])
    audio, words_file = out / "voice.wav", out / "words.json"
    pcm = (np.clip(samples, -1.0, 1.0) * 32767).astype("<i2").tobytes()
    with wave.open(str(audio), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(pcm)
    words = estimate_words(input["text"], len(samples) / rate)
    words_file.write_text(json.dumps(words, ensure_ascii=False), encoding="utf-8")
    return {"artifacts": {"audio": str(audio), "words": str(words_file)}, "timing": "estimated", "cost_usd": 0}


serve()
