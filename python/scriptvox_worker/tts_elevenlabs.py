"""ElevenLabs paid TTS (stdlib HTTP). Word times come from the character
alignment of the `with-timestamps` endpoint, so timing is exact.

Money rule: the tool asks the core for `ledger.reserve` before any HTTP call.
A refused reserve means no request is sent. The key arrives only as the
ELEVENLABS_API_KEY environment variable, never in an RPC message.
"""

from __future__ import annotations

import base64
import json
import os
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from .rpc import ToolError, tool

PROVIDER_REFUSED = -32010
PROVIDER_RETRYABLE = -32011


def words_from_alignment(al: dict) -> list[dict]:
    """Groups characters into words at whitespace."""
    chars = al.get("characters") or []
    starts = al.get("character_start_times_seconds") or []
    ends = al.get("character_end_times_seconds") or []
    words, cur, t0, t1 = [], "", None, None
    for c, s, e in zip(chars, starts, ends, strict=False):
        if c.isspace():
            if cur:
                words.append({"text": cur, "start": round(t0, 3), "end": round(t1, 3)})
            cur, t0 = "", None
            continue
        if t0 is None:
            t0 = s
        cur += c
        t1 = e
    if cur:
        words.append({"text": cur, "start": round(t0, 3), "end": round(t1, 3)})
    return words


ELEVENLABS = "https://api.elevenlabs.io"


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    """urllib copies the key header to whatever host a redirect names, so a redirect is an error answer."""

    def redirect_request(self, *args, **kwargs):
        return None


def _open(req, timeout):
    return urllib.request.build_opener(_NoRedirect).open(req, timeout=timeout)


def base_ok(base: str) -> bool:
    """The key goes to ElevenLabs, or to a test server on loopback."""
    if base == ELEVENLABS:
        return True
    parts = urllib.parse.urlsplit(base)
    return parts.scheme == "http" and parts.hostname == "127.0.0.1" and parts.port is not None and not parts.username


@tool("tts.elevenlabs", network=True, cost="paid", deterministic=False, timing="exact",
      terms="commercial use on paid plans, see elevenlabs.io/terms")
def synthesize(input: dict, ctx) -> dict:
    base = os.environ.get("SCRIPTVOX_ELEVENLABS_URL", ELEVENLABS)
    if not base_ok(base):
        raise ToolError(PROVIDER_REFUSED, f"SCRIPTVOX_ELEVENLABS_URL={base} is not allowed: only a test server on http://127.0.0.1:<port>")
    text = input["text"]
    est = round(len(text) / 1000 * float(input.get("usd_per_1k_chars", 0.08)), 4)
    reservation = ctx.request("ledger.reserve", {"provider": "elevenlabs", "est_usd": est})
    key = os.environ.get("ELEVENLABS_API_KEY")
    if not key:
        raise ToolError(PROVIDER_REFUSED, "ELEVENLABS_API_KEY is not set for the worker")
    url = f"{base}/v1/text-to-speech/{input['voice']}/with-timestamps?output_format=mp3_44100_128"
    payload = {"text": text, "model_id": input.get("model") or "eleven_multilingual_v2"}
    if input.get("rate"):
        # The core's rate ("-15%") as the voice's `speed`, which the API takes from 0.7 to 1.2. The other voice settings
        # stay the stored ones.
        speed = 1 + int(input["rate"].rstrip("%")) / 100
        payload["voice_settings"] = {"speed": round(min(1.2, max(0.7, speed)), 2)}
    body = json.dumps(payload).encode()
    req = urllib.request.Request(url, data=body, method="POST",
                                 headers={"xi-api-key": key, "Content-Type": "application/json", "Accept": "application/json"})
    ctx.event("progress", done=0, total=1, explain=f"ElevenLabs {input['voice']} (${est:.2f} reserved)")
    try:
        with _open(req, timeout=120) as resp:
            data = json.load(resp)
    except urllib.error.HTTPError as e:
        if e.code in (401, 403):
            raise ToolError(PROVIDER_REFUSED, f"ElevenLabs refused the request (HTTP {e.code}): check the key") from e
        if e.code in (400, 404, 422):
            raise ToolError(PROVIDER_REFUSED, f"ElevenLabs refused the request (HTTP {e.code}): check the voice and the text") from e
        raise ToolError(PROVIDER_RETRYABLE, f"ElevenLabs answered HTTP {e.code}") from e
    except (urllib.error.URLError, TimeoutError) as e:
        raise ToolError(PROVIDER_RETRYABLE, f"ElevenLabs is not reachable: {e}") from e
    out = Path(input["out_dir"])
    audio, words_file = out / "voice.mp3", out / "words.json"
    audio.write_bytes(base64.b64decode(data["audio_base64"]))
    words = words_from_alignment(data.get("alignment") or {})
    words_file.write_text(json.dumps(words, ensure_ascii=False), encoding="utf-8")
    return {"artifacts": {"audio": str(audio), "words": str(words_file)}, "timing": "exact",
            "cost_usd": est, "reservation": reservation.get("reservation")}
