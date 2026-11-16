"""scriptvox-gen: a small job queue for open video models on one GPU.

It answers the same submit, status and result calls as the fal queue, so `scriptvox generate` and generated scenes in a
brief run it like any hosted row. One worker thread runs one job at a time on the GPU. The models come from engine
modules named in ENGINES (engines_video.py, engines_audio.py, ...), so each image carries only its own libraries.

  POST {prefix}/<model>/<mode>      body: {prompt, duration, aspect_ratio, ...}  -> {request_id, status_url, ...}
  GET  {prefix}/requests/<id>/status                                            -> {status, error?}
  GET  {prefix}/requests/<id>                                                   -> {video: {url}}
  GET  {prefix}/files/<id>.<ext>    (no key: the id is a random 128-bit token)
  GET  {prefix}/healthz

Jobs are kept as JSON next to the clips, so a restart keeps finished results; a job that was running at the restart
is marked failed (the client then says so, and nothing was charged: this queue is free).
Licence: Apache-2.0, part of scriptvox.
"""

import hmac
import json
import os
import queue
import re
import threading
import time
import traceback
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

PREFIX = os.environ.get("PREFIX", "/scriptvox-gen").rstrip("/")
PUBLIC = os.environ.get("PUBLIC_BASE", "http://127.0.0.1:8080/scriptvox-gen").rstrip("/")
KEY = Path(os.environ.get("KEY_FILE", "/key/value")).read_text().strip()
OUT = Path(os.environ.get("OUT_DIR", "/out"))
MODELS = Path(os.environ.get("MODELS_DIR", "/models"))
MAX_BODY = int(os.environ.get("MAX_BODY", str(40_000_000)))
# Result field and content type per output extension.
FIELD = {"mp4": "video", "png": "image", "wav": "audio", "json": "transcript"}
CTYPE = {"mp4": "video/mp4", "png": "image/png", "wav": "audio/wav", "json": "application/json"}


def load_engines() -> dict:
    """{"<model>/<mode>": engine} from the modules in ENGINES. An engine has `run(inp, out_base) -> (path, metrics)`,
    `validate(inp) -> str | None` and `warm()`."""
    import importlib

    found = {}
    for name in os.environ.get("ENGINES", "video").split(","):
        found.update(importlib.import_module(f"engines_{name.strip()}").ENGINES)
    return found


ENGINES = load_engines()
OUT.mkdir(parents=True, exist_ok=True)
jobs: dict[str, dict] = {}
lock = threading.Lock()
work: "queue.Queue[str]" = queue.Queue()


def save(job: dict) -> None:
    """Writes the job record as <id>.job.json (results use <id>.<ext>). Uploaded data URIs are not kept on disk."""
    rec = dict(job, input={k: (f"<data uri, {len(v)} chars>" if isinstance(v, str) and v.startswith("data:") else v)
                          for k, v in job["input"].items()})
    tmp = OUT / f"{job['id']}.job.json.tmp"
    tmp.write_text(json.dumps(rec))
    tmp.replace(OUT / f"{job['id']}.job.json")


def load() -> None:
    for f in OUT.glob("*.job.json"):
        try:
            job = json.loads(f.read_text())
        except ValueError:
            continue
        if job["status"] != "COMPLETED":
            job.update(status="COMPLETED", error="the server restarted while this job ran; submit it again")
            save(job)
        jobs[job["id"]] = job


def run(job: dict) -> None:
    t0 = time.time()
    path, metrics = ENGINES[f"{job['model']}/{job['mode']}"].run(job["input"], OUT / job["id"])
    job["file"] = Path(path).name
    job["metrics"] = {"seconds_gpu": round(time.time() - t0, 1), **metrics}


def worker() -> None:
    while True:
        jid = work.get()
        with lock:
            job = jobs[jid]
            job["status"] = "IN_PROGRESS"
            save(job)
        try:
            run(job)
            job["status"] = "COMPLETED"
        except Exception as e:  # noqa: BLE001 - any model failure becomes the job error the client reads
            traceback.print_exc()
            job.update(status="COMPLETED", error=f"{type(e).__name__}: {e}"[:500])
        with lock:
            save(job)


def validate(model: str, mode: str, inp: dict) -> str | None:
    eng = ENGINES.get(f"{model}/{mode}")
    if eng is None:
        return f"no model {model}/{mode} here; have: {sorted(ENGINES)}"
    return eng.validate(inp)


ID = re.compile(r"^[0-9a-f]{32}$")


class Handler(BaseHTTPRequestHandler):
    server_version = "scriptvox-gen"

    def log_message(self, fmt, *args):  # one line per request, no headers (the key is a header)
        print(f"{self.command} {self.path.split('?')[0]} -> {fmt % args}", flush=True)

    def reply(self, code: int, body: dict) -> None:
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def authorized(self) -> bool:
        got = self.headers.get("Authorization", "")
        return hmac.compare_digest(got.encode(), f"Key {KEY}".encode())

    def rest(self) -> str | None:
        p = self.path.split("?")[0]
        return p[len(PREFIX):] if p.startswith(PREFIX + "/") else None

    def do_GET(self):
        r = self.rest()
        if r == "/healthz":
            return self.reply(200, {"ok": True, "queued": work.qsize(), "models": sorted(ENGINES)})
        fm = re.match(r"^/files/([0-9a-f]{32})\.(mp4|png|wav|json)$", r or "")
        if fm:
            f = OUT / f"{fm.group(1)}.{fm.group(2)}"
            if not f.is_file():
                return self.reply(404, {"detail": "no such file"})
            return self.send_file(f, CTYPE[fm.group(2)])
        if not self.authorized():
            return self.reply(401, {"detail": "bad key"})
        m = re.match(r"^/requests/([0-9a-f]{32})(/status)?$", r or "")
        job = jobs.get(m.group(1)) if m else None
        if not job:
            return self.reply(404, {"detail": "no such request"})
        if m.group(2):
            body = {"status": job["status"]}
            if job.get("error"):
                body["error"] = job["error"]
            if job["status"] == "IN_QUEUE":
                body["queue_position"] = list(work.queue).index(job["id"]) if job["id"] in work.queue else 0
            return self.reply(200, body)
        if job["status"] != "COMPLETED" or job.get("error"):
            return self.reply(400, {"detail": job.get("error") or "not finished"})
        ext = job["file"].rsplit(".", 1)[-1]
        return self.reply(200, {FIELD[ext]: {"url": f"{PUBLIC}/files/{job['file']}"}, "metrics": job.get("metrics", {})})

    def send_file(self, f: Path, ctype: str) -> None:
        size = f.stat().st_size
        start = 0
        rng = re.match(r"bytes=(\d+)-$", self.headers.get("Range", ""))
        if rng and int(rng.group(1)) < size:
            start = int(rng.group(1))
        self.send_response(206 if start else 200)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(size - start))
        if start:
            self.send_header("Content-Range", f"bytes {start}-{size - 1}/{size}")
        self.end_headers()
        with f.open("rb") as fh:
            fh.seek(start)
            while chunk := fh.read(1 << 20):
                self.wfile.write(chunk)

    def do_POST(self):
        if not self.authorized():
            return self.reply(401, {"detail": "bad key"})
        parts = (self.rest() or "").strip("/").split("/")
        if len(parts) != 2:
            return self.reply(404, {"detail": "POST <prefix>/<model>/<mode>"})
        n = int(self.headers.get("Content-Length", "0"))
        if n > MAX_BODY:
            return self.reply(413, {"detail": f"request over {MAX_BODY // 1_000_000} MB"})
        try:
            inp = json.loads(self.rfile.read(n) or b"{}")
        except ValueError:
            return self.reply(422, {"detail": "body is not JSON"})
        bad = validate(parts[0], parts[1], inp) if isinstance(inp, dict) else "body must be an object"
        if bad:
            return self.reply(422, {"detail": bad})
        jid = uuid.uuid4().hex
        job = {"id": jid, "model": parts[0], "mode": parts[1], "input": inp, "status": "IN_QUEUE",
               "created": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
        with lock:
            jobs[jid] = job
            save(job)
        work.put(jid)
        base = f"{PUBLIC}/requests/{jid}"
        self.reply(200, {"request_id": jid, "status_url": f"{base}/status", "response_url": base,
                         "cancel_url": f"{base}/cancel"})


if __name__ == "__main__":
    load()
    threading.Thread(target=worker, daemon=True).start()
    if os.environ.get("PRELOAD"):
        # Warm one engine at start (for example PRELOAD=wan22-ti2v-5b/t2v), so the first job skips the load.
        threading.Thread(target=ENGINES[os.environ["PRELOAD"]].warm, daemon=True).start()
    ThreadingHTTPServer(("0.0.0.0", int(os.environ.get("PORT", "8080"))), Handler).serve_forever()
