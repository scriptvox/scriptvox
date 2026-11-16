"""ElevenLabs tool with a fake HTTP layer (no network, no key in RPC)."""

import base64
import io
import json
import urllib.error

import pytest
from scriptvox_worker import tts_elevenlabs as el
from scriptvox_worker.rpc import ToolError

ALIGN = {
    "characters": list("Hi there"),
    "character_start_times_seconds": [0.0, 0.1, 0.2, 0.3, 0.35, 0.4, 0.45, 0.5],
    "character_end_times_seconds": [0.1, 0.2, 0.3, 0.35, 0.4, 0.45, 0.5, 0.6],
}


class Ctx:
    def __init__(self, refuse=False):
        self.refuse, self.requests, self.events = refuse, [], []

    def event(self, type, **p):
        self.events.append(p)

    def request(self, method, params):
        self.requests.append((method, params))
        if self.refuse:
            raise ToolError(-32020, "spend cap reached")
        return {"reservation": 7}


def test_words_from_alignment():
    assert el.words_from_alignment(ALIGN) == [{"text": "Hi", "start": 0.0, "end": 0.2}, {"text": "there", "start": 0.3, "end": 0.6}]


def test_exact_timing_and_reserve_first(tmp_path, monkeypatch):
    calls = []

    def fake_urlopen(req, timeout):
        calls.append(req)
        assert req.get_header("Xi-api-key") == "k-test", "the key comes from the environment"
        assert b"k-test" not in req.data, "the key is never in the body"
        return io.BytesIO(json.dumps({"audio_base64": base64.b64encode(b"mp3").decode(), "alignment": ALIGN}).encode())

    monkeypatch.setenv("ELEVENLABS_API_KEY", "k-test")
    monkeypatch.setattr(el, "_open", fake_urlopen)
    ctx = Ctx()
    r = el.synthesize({"text": "Hi there", "voice": "v1", "out_dir": str(tmp_path)}, ctx)
    assert ctx.requests[0][0] == "ledger.reserve" and len(calls) == 1
    assert r["timing"] == "exact" and r["reservation"] == 7 and r["cost_usd"] > 0


def test_refused_reserve_sends_no_request(tmp_path, monkeypatch):
    monkeypatch.setenv("ELEVENLABS_API_KEY", "k-test")
    monkeypatch.setattr(el, "_open", lambda *a, **k: pytest.fail("HTTP after a refused reserve"))
    with pytest.raises(ToolError) as e:
        el.synthesize({"text": "Hi", "voice": "v1", "out_dir": str(tmp_path)}, Ctx(refuse=True))
    assert e.value.code == -32020


@pytest.mark.parametrize(("status", "code"), [(400, el.PROVIDER_REFUSED), (401, el.PROVIDER_REFUSED), (404, el.PROVIDER_REFUSED), (422, el.PROVIDER_REFUSED), (429, el.PROVIDER_RETRYABLE), (503, el.PROVIDER_RETRYABLE)])
def test_http_errors_map_to_codes(tmp_path, monkeypatch, status, code):
    def boom(req, timeout):
        raise urllib.error.HTTPError(req.full_url, status, "x", {}, None)

    monkeypatch.setenv("ELEVENLABS_API_KEY", "k-test")
    monkeypatch.setattr(el, "_open", boom)
    with pytest.raises(ToolError) as e:
        el.synthesize({"text": "Hi", "voice": "v1", "out_dir": str(tmp_path)}, Ctx())
    assert e.value.code == code

def test_the_key_goes_only_to_elevenlabs_or_loopback(tmp_path, monkeypatch):
    monkeypatch.setenv("ELEVENLABS_API_KEY", "k-test")
    monkeypatch.setenv("SCRIPTVOX_ELEVENLABS_URL", "http://127.0.0.2:9")
    monkeypatch.setattr(el, "_open", lambda *a, **k: pytest.fail("the key left for another host"))
    ctx = Ctx()
    with pytest.raises(ToolError):
        el.synthesize({"text": "Hi", "voice": "v1", "out_dir": str(tmp_path)}, ctx)
    assert ctx.requests == [], "nothing reserved"


def test_a_redirect_is_not_followed(tmp_path, monkeypatch):
    import http.server
    import threading

    seen = []

    class H(http.server.BaseHTTPRequestHandler):
        def do_POST(self):
            seen.append(self.path)
            self.send_response(302)
            self.send_header("Location", f"http://127.0.0.1:{self.server.server_port}/stolen")
            self.send_header("Content-Length", "0")
            self.end_headers()

        do_GET = do_POST

        def log_message(self, *a):
            pass

    s = http.server.HTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=s.serve_forever, daemon=True).start()
    monkeypatch.setenv("ELEVENLABS_API_KEY", "k-test")
    monkeypatch.setenv("SCRIPTVOX_ELEVENLABS_URL", f"http://127.0.0.1:{s.server_port}")
    with pytest.raises(ToolError) as e:
        el.synthesize({"text": "Hi", "voice": "v1", "out_dir": str(tmp_path)}, Ctx())
    s.shutdown()
    assert e.value.code == el.PROVIDER_RETRYABLE
    assert len(seen) == 1 and "stolen" not in seen[0]


def test_a_rate_goes_as_the_speed_setting_inside_the_api_range(tmp_path, monkeypatch):
    bodies = []

    def fake_urlopen(req, timeout):
        bodies.append(json.loads(req.data))
        return io.BytesIO(json.dumps({"audio_base64": base64.b64encode(b"mp3").decode(), "alignment": ALIGN}).encode())

    monkeypatch.setenv("ELEVENLABS_API_KEY", "k-test")
    monkeypatch.setattr(el, "_open", fake_urlopen)
    for rate in (None, "-15%", "+20%", "-50%", "+50%"):
        el.synthesize({"text": "Hi there", "voice": "v1", "out_dir": str(tmp_path), **({"rate": rate} if rate else {})}, Ctx())
    assert "voice_settings" not in bodies[0], "no rate: the request of every earlier version"
    assert [b["voice_settings"] for b in bodies[1:]] == [{"speed": 0.85}, {"speed": 1.2}, {"speed": 0.7}, {"speed": 1.2}]
