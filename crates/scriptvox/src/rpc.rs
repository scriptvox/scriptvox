//! RVX-RPC host (PLAN-R1 §2.4): JSON-RPC 2.0, one JSON object per line over the
//! worker's stdio. A non-JSON line or a closed stdout kills the worker and gives
//! `-32099 worker_crashed`.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::Duration;

pub const PROTO: &str = "1.0";
pub const WORKER_CRASHED: i64 = -32099;
pub const WORKER_TIMEOUT: i64 = -32098;
pub const PROTO_MISMATCH: i64 = -32002;

#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

enum Line {
    Msg(Value),
    Bad(String),
}

pub struct Worker {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Line>,
    next_id: u64,
    /// No answer within this time kills the worker (a hung TTS call, for example).
    pub timeout: Duration,
}

impl Worker {
    pub fn spawn(mut cmd: Command) -> Result<Self, RpcError> {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| RpcError::new(WORKER_CRASHED, format!("cannot start the worker: {e}")))?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let msg = serde_json::from_str::<Value>(&line)
                    .map(Line::Msg)
                    .unwrap_or(Line::Bad(line));
                let bad = matches!(msg, Line::Bad(_));
                if tx.send(msg).is_err() || bad {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            rx,
            next_id: 0,
            timeout: Duration::from_secs(300),
        })
    }

    /// Writes one raw message. Contract tests use it to replay fixtures.
    pub fn send(&mut self, msg: &Value) -> Result<(), RpcError> {
        writeln!(self.stdin, "{msg}")
            .and_then(|_| self.stdin.flush())
            .map_err(|_| self.crash("the worker closed its stdin"))
    }

    /// Reads one raw message.
    pub fn recv(&mut self) -> Result<Value, RpcError> {
        match self.rx.recv_timeout(self.timeout) {
            Ok(Line::Msg(v)) => Ok(v),
            Ok(Line::Bad(line)) => {
                let shown: String = line.chars().take(80).collect();
                Err(self.crash(format!("worker_crashed: non-JSON line on stdout: {shown}")))
            }
            Err(RecvTimeoutError::Disconnected) => {
                Err(self.crash("worker_crashed: the worker exited"))
            }
            Err(RecvTimeoutError::Timeout) => {
                let _ = self.child.kill();
                Err(RpcError::new(WORKER_TIMEOUT, "worker_timeout"))
            }
        }
    }

    fn crash(&mut self, message: impl Into<String>) -> RpcError {
        let _ = self.child.kill();
        RpcError::new(WORKER_CRASHED, message)
    }

    /// One request. Events go to `on_event`; the call returns its result.
    pub fn request(
        &mut self,
        method: &str,
        params: Value,
        on_event: impl FnMut(&Value),
    ) -> Result<Value, RpcError> {
        self.request_with(method, params, on_event, |m, _| {
            Err((-32601, format!("unknown method {m}")))
        })
    }

    /// Like `request`, and `on_request` answers worker-to-core requests
    /// (`ledger.reserve`) with a result or an `(code, message)` error.
    pub fn request_with(
        &mut self,
        method: &str,
        params: Value,
        mut on_event: impl FnMut(&Value),
        mut on_request: impl FnMut(&str, &Value) -> Result<Value, (i64, String)>,
    ) -> Result<Value, RpcError> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        loop {
            let msg = self.recv()?;
            if msg["id"] == json!(id) && msg.get("method").is_none() {
                if let Some(err) = msg.get("error") {
                    let code = err["code"].as_i64().unwrap_or(-32000);
                    return Err(RpcError::new(
                        code,
                        err["message"].as_str().unwrap_or("worker error"),
                    ));
                }
                return Ok(msg["result"].clone());
            }
            match msg["method"].as_str() {
                Some("event") => on_event(&msg["params"]),
                Some(m) if !msg["id"].is_null() => {
                    let reply = match on_request(m, &msg["params"]) {
                        Ok(r) => json!({"jsonrpc": "2.0", "id": msg["id"], "result": r}),
                        Err((code, message)) => {
                            json!({"jsonrpc": "2.0", "id": msg["id"], "error": {"code": code, "message": message}})
                        }
                    };
                    self.send(&reply)?;
                }
                _ => {}
            }
        }
    }

    /// `hello`: majors must be equal. Returns the worker's tool manifests.
    pub fn hello(&mut self, local_only: bool) -> Result<Value, RpcError> {
        let r = self.request(
            "hello",
            json!({"proto": PROTO, "local_only": local_only}),
            |_| {},
        )?;
        let major = |s: &str| s.split('.').next().unwrap_or("").to_string();
        let theirs = r["proto"].as_str().unwrap_or("");
        if major(theirs) != major(PROTO) {
            return Err(RpcError::new(
                PROTO_MISMATCH,
                format!("proto mismatch: core {PROTO}, worker {theirs}"),
            ));
        }
        Ok(r)
    }

    pub fn shutdown(mut self) {
        let _ = self.request("shutdown", json!({}), |_| {});
        let _ = self.child.wait();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
