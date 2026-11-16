"""Replays fixtures/contract/*.jsonl against the worker in-process.
The Rust side replays the same files over a real pipe (tests/contract.rs)."""

import json
from pathlib import Path

import pytest
import scriptvox_worker.tools  # noqa: F401 - the same tools as the real worker
from scriptvox_worker.rpc import handle

FIXTURES = sorted((Path(__file__).resolve().parents[2] / "fixtures" / "contract").glob("*.jsonl"))


@pytest.mark.parametrize("path", FIXTURES, ids=lambda p: p.stem)
def test_contract(path):
    steps = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    expected = [s["worker"] for s in steps if "worker" in s]
    core = iter([s["core"] for s in steps if "core" in s])
    got = []

    def recv():
        # A tool waiting for the core gets the next core line of the fixture.
        return next(core, None)

    while (msg := recv()) is not None:
        handle(msg, got.append, recv)
    assert got == expected


def test_stdin_is_utf8_whatever_the_locale_says():
    import os
    import subprocess
    import sys

    script = "import scriptvox_worker.rpc as r\nr.tool('echo')(lambda i, c: {'text': i['text']})\nr.serve()\n"
    text = "ف é"
    req = {"jsonrpc": "2.0", "id": 1, "method": "tool.call", "params": {"tool": "echo", "input": {"text": text}}}
    p = subprocess.run([sys.executable, "-c", script], input=(json.dumps(req, ensure_ascii=False) + "\n").encode(),
                       capture_output=True, check=False, cwd=Path(__file__).resolve().parents[1], timeout=30,
                       env={**os.environ, "PYTHONIOENCODING": "cp1252"})
    assert json.loads(p.stdout.decode("utf-8")) == {"jsonrpc": "2.0", "id": 1, "result": {"text": text}}, p.stderr


def test_print_in_a_tool_never_reaches_protocol_stdout():
    import subprocess
    import sys

    script = (
        "import scriptvox_worker.rpc as r\n"
        "r.tool('noisy')(lambda i, c: print('noise from a tool') or {'ok': True})\n"
        "r.serve()\n"
    )
    reqs = [{"jsonrpc": "2.0", "id": 1, "method": "tool.call", "params": {"tool": "noisy"}},
            {"jsonrpc": "2.0", "id": 2, "method": "shutdown"}]
    p = subprocess.run([sys.executable, "-c", script], input="\n".join(json.dumps(r) for r in reqs) + "\n",
                       capture_output=True, text=True, check=False, cwd=Path(__file__).resolve().parents[1], timeout=30)
    lines = [json.loads(line) for line in p.stdout.splitlines()]  # every stdout line is JSON
    assert lines[0] == {"jsonrpc": "2.0", "id": 1, "result": {"ok": True}}
    assert "noise from a tool" in p.stderr
