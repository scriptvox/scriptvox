"""Edge TTS tool against recorded cassettes (no network)."""

import base64
import json
from pathlib import Path

import pytest
from scriptvox_worker import tts_edge
from scriptvox_worker.rpc import ToolError

CASSETTES = Path(__file__).resolve().parents[1] / "cassettes"


class Ctx:
    def __init__(self):
        self.events = []

    def event(self, type, **params):
        self.events.append((type, params))


def replay(name):
    tape = json.loads((CASSETTES / f"{name}.json").read_text(encoding="utf-8"))

    class FakeCommunicate:
        def __init__(self, text, voice, rate="+0%", boundary="SentenceBoundary"):
            assert boundary == "WordBoundary", "word events need boundary=WordBoundary"
            assert (text, voice) == (tape["text"], tape["voice"])

        async def stream(self):
            for ch in tape["chunks"]:
                yield ch | {"data": base64.b64decode(ch["data"])} if ch["type"] == "audio" else ch

    return tape, FakeCommunicate


@pytest.mark.parametrize("name", ["edge_en", "edge_ar"])
def test_word_events_and_exact_timing(name, tmp_path, monkeypatch):
    tape, fake = replay(name)
    monkeypatch.setattr(tts_edge.edge_tts, "Communicate", fake)
    r = tts_edge.synthesize({"text": tape["text"], "voice": tape["voice"], "out_dir": str(tmp_path)}, Ctx())
    assert r["timing"] == "exact" and r["cost_usd"] == 0
    words = json.loads(Path(r["artifacts"]["words"]).read_text(encoding="utf-8"))
    assert len(words) == sum(1 for c in tape["chunks"] if c["type"] == "WordBoundary")
    assert all(w["end"] > w["start"] >= 0 for w in words)
    assert Path(r["artifacts"]["audio"]).stat().st_size > 0


def test_403_is_provider_refused(tmp_path, monkeypatch):
    class Refused(Exception):
        status = 403

    class FakeCommunicate:
        def __init__(self, *a, **k):
            pass

        async def stream(self):
            raise Refused("Invalid response status 403")
            yield  # pragma: no cover

    monkeypatch.setattr(tts_edge.edge_tts, "Communicate", FakeCommunicate)
    with pytest.raises(ToolError) as e:
        tts_edge.synthesize({"text": "x", "voice": "ar-AE-HamdanNeural", "out_dir": str(tmp_path)}, Ctx())
    assert e.value.code == tts_edge.PROVIDER_REFUSED


def test_a_hiccup_is_retried_and_a_lasting_one_fails(tmp_path, monkeypatch):
    tape, fake = replay("edge_en")
    calls = {"n": 0}

    class Flaky(fake):
        async def stream(self):
            calls["n"] += 1
            if calls["n"] <= fail_first:
                raise RuntimeError("NoAudioReceived")
            async for ch in super().stream():
                yield ch

    monkeypatch.setattr(tts_edge.edge_tts, "Communicate", Flaky)
    monkeypatch.setattr(tts_edge, "RETRY_WAITS", (0, 0))
    args = {"text": tape["text"], "voice": tape["voice"], "out_dir": str(tmp_path)}
    fail_first = 2
    assert tts_edge.synthesize(args, Ctx())["cost_usd"] == 0 and calls["n"] == 3
    calls["n"], fail_first = 0, 3
    with pytest.raises(ToolError) as e:
        tts_edge.synthesize(args, Ctx())
    assert e.value.code == tts_edge.PROVIDER_RETRYABLE and "3 tries" in str(e.value)


def test_long_text_goes_out_in_sentence_chunks_and_word_times_follow_the_audio(tmp_path, monkeypatch):
    assert tts_edge._chunks("Short text. Stays whole.") == ["Short text. Stays whole."]
    sentence = "One two three four five six seven eight nine ten. "
    text = (sentence * 14).strip()  # 140 words, 14 sentences
    chunks = tts_edge._chunks(text)
    assert len(chunks) == 3 and all(c.endswith(".") for c in chunks)
    assert sum(len(c.split()) for c in chunks) == 140

    class Fake:
        # Each part: one second of CBR audio and one word at 0.5 s.
        def __init__(self, text, voice, rate="+0%", boundary="WordBoundary"):
            self.text = text

        async def stream(self):
            yield {"type": "audio", "data": b"\0" * int(tts_edge.MP3_BYTES_PER_S)}
            yield {"type": "WordBoundary", "offset": 5_000_000, "duration": 1_000_000, "text": self.text.split()[0]}

    monkeypatch.setattr(tts_edge.edge_tts, "Communicate", Fake)
    import asyncio

    words = asyncio.run(tts_edge._synth(text, "v", "+0%", tmp_path / "a.mp3"))
    assert [w["start"] for w in words] == [0.5, 1.5, 2.5]
    assert (tmp_path / "a.mp3").stat().st_size == 3 * int(tts_edge.MP3_BYTES_PER_S)


def test_a_failing_part_retries_alone(tmp_path, monkeypatch):
    calls = {}

    class Flaky:
        def __init__(self, text, voice, rate="+0%", boundary="WordBoundary"):
            self.text = text

        async def stream(self):
            n = calls[self.text] = calls.get(self.text, 0) + 1
            if self.text.startswith("Two") and n == 1:
                raise tts_edge.edge_tts.exceptions.NoAudioReceived("no audio")
            yield {"type": "audio", "data": b"\0" * int(tts_edge.MP3_BYTES_PER_S)}
            yield {"type": "WordBoundary", "offset": 0, "duration": 1_000_000, "text": self.text.split()[0]}

    monkeypatch.setattr(tts_edge.edge_tts, "Communicate", Flaky)
    monkeypatch.setattr(tts_edge, "RETRY_WAITS", (0.0, 0.0))
    import asyncio

    text = " ".join(f"{w} a b c d e f g h i j k l m n o p q r s t u v w x y z a b c d e f g h i j k l m n o p q r s t u v w x y z." for w in ("One", "Two"))
    words = asyncio.run(tts_edge._synth(text, "v", "+0%", tmp_path / "a.mp3"))
    assert [w["text"] for w in words] == ["One", "Two"]
    assert [calls[k] for k in sorted(calls)] == [1, 2]


def test_chunks_keep_numbers_and_domains_whole():
    filler = " ".join(["word"] * 50)
    text = f"The plant makes 3.5 million litres a day, see example.com for more. {filler}. Is it enough?! {filler}."
    chunks = tts_edge._chunks(text)
    joined = " ".join(chunks)
    assert "3.5 million" in joined and "example.com" in joined and "enough?!" in joined
    assert all(c.strip(".!? ") for c in chunks)
