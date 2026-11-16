"""RVX-RPC: JSON-RPC 2.0, one JSON object per line over stdio.

stdout carries protocol lines only. `serve()` moves `print`, logging and any
C-level writes to stderr before it reads the first request, because one stray
line on stdout makes the core kill the worker.
"""

from __future__ import annotations

import json
import os
import sys
import traceback
from collections.abc import Callable
from typing import Any

from . import PROTO, __version__

TOOLS: dict[str, dict[str, Any]] = {}


class ToolError(Exception):
    """A tool failure with an RPC error code the core maps to an exit code."""

    def __init__(self, code: int, message: str):
        super().__init__(message)
        self.code = code


def tool(name: str, **manifest: Any) -> Callable:
    """Register a tool. The manifest is sent to the core in `hello`."""

    def deco(fn: Callable) -> Callable:
        TOOLS[name] = {"fn": fn, "manifest": {"name": name, **manifest}}
        return fn

    return deco


@tool("debug.echo", network=False, cost="free", deterministic=True)
def _echo(input: dict, ctx: Ctx) -> dict:
    """Test tool: one progress event, then the input back."""
    ctx.event("progress", done=1, total=1, explain="echo")
    return {"artifacts": {}, "echo": input, "cost_usd": 0}


class Ctx:
    def __init__(self, send: Callable[[dict], None], recv: Callable[[], dict | None] | None = None):
        self.send = send
        self.recv = recv
        self._next = 0

    def event(self, type: str, **params: Any) -> None:
        self.send({"jsonrpc": "2.0", "method": "event", "params": {"type": type, **params}})

    def request(self, method: str, params: dict) -> dict:
        """Asks the core (for example `ledger.reserve`) and waits for its answer.
        An error answer raises ToolError with the core's code."""
        self._next += 1
        rid = f"w{self._next}"
        self.send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        while self.recv is not None:
            msg = self.recv()
            if msg is None:
                break
            if msg.get("id") == rid:
                if "error" in msg:
                    raise ToolError(msg["error"].get("code", -32000), msg["error"].get("message", "refused"))
                return msg.get("result") or {}
        raise ToolError(-32000, f"no answer from the core to {method}")


def error(id: Any, code: int, message: str) -> dict:
    return {"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}


def handle(msg: dict, send: Callable[[dict], None], recv: Callable[[], dict | None] | None = None) -> bool:
    """Answer one request through `send`. Returns False after `shutdown`."""
    id_, method, params = msg.get("id"), msg.get("method"), msg.get("params") or {}
    if method == "hello":
        major = str(params.get("proto", "")).split(".")[0]
        if major != PROTO.split(".")[0]:
            send(error(id_, -32002, f"proto mismatch: core {params.get('proto')}, worker {PROTO}"))
            return True
        tools = sorted((t["manifest"] for t in TOOLS.values()), key=lambda m: m["name"])
        send({"jsonrpc": "2.0", "id": id_, "result": {"proto": PROTO, "worker": __version__, "tools": tools}})
    elif method == "tool.call":
        t = TOOLS.get(params.get("tool"))
        if t is None:
            send(error(id_, -32601, f"unknown tool {params.get('tool')}"))
            return True
        try:
            result = t["fn"](params.get("input") or {}, Ctx(send, recv))
        except ToolError as e:
            send(error(id_, e.code, str(e)))
            return True
        except Exception as e:  # noqa: BLE001 - a tool failure is an RPC error, never a dead worker
            traceback.print_exc(file=sys.stderr)
            send(error(id_, -32000, f"{type(e).__name__}: {e}"))
            return True
        send({"jsonrpc": "2.0", "id": id_, "result": result})
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": id_, "result": {}})
        return False
    elif method == "cancel":
        # ponytail: tools are synchronous in v0.1; cancel takes effect between calls.
        send({"jsonrpc": "2.0", "id": id_, "result": {}})
    else:
        send(error(id_, -32601, f"unknown method {method}"))
    return True


def serve() -> None:
    proto_out = os.fdopen(os.dup(1), "w", encoding="utf-8", buffering=1)
    os.dup2(2, 1)  # C extensions writing to fd 1 now reach stderr
    sys.stdout = sys.stderr
    # The core writes UTF-8; piped stdin on Windows otherwise reads in the locale code page.
    sys.stdin.reconfigure(encoding="utf-8")

    def send(obj: dict) -> None:
        proto_out.write(json.dumps(obj, ensure_ascii=False, separators=(",", ":")) + "\n")

    def recv() -> dict | None:
        # One line at a time, so a tool can wait for the core's answer mid-call.
        while True:
            line = sys.stdin.readline()
            if not line:
                return None
            if line.strip():
                try:
                    return json.loads(line)
                except json.JSONDecodeError:
                    send(error(None, -32700, "parse error"))

    while (msg := recv()) is not None:
        if not handle(msg, send, recv):
            break
