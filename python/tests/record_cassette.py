"""Records a live Edge stream into cassettes/NAME.json (run by hand, needs network):
uv run --project python python python/tests/record_cassette.py edge_en en-US-AndrewNeural "text"
"""

import asyncio
import base64
import json
import sys
from pathlib import Path

import edge_tts


async def main(name: str, voice: str, text: str) -> None:
    chunks = []
    async for ch in edge_tts.Communicate(text, voice, boundary="WordBoundary").stream():
        if ch["type"] == "audio":
            chunks.append({"type": "audio", "data": base64.b64encode(ch["data"]).decode()})
        else:
            chunks.append({k: ch[k] for k in ("type", "offset", "duration", "text")})
    out = Path(__file__).resolve().parents[1] / "cassettes" / f"{name}.json"
    out.write_text(json.dumps({"voice": voice, "text": text, "chunks": chunks}, ensure_ascii=False))
    print(out, len(chunks))


asyncio.run(main(*sys.argv[1:4]))
