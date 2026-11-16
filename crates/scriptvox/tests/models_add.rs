//! `models add` end to end against a loopback server that stands in for Hugging Face, with a stub `sd-cli`.
#![cfg(unix)]

mod common;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;

/// A safetensors file whose header says SD 1.x.
fn sd1_bytes() -> Vec<u8> {
    let body = br#"{"first_stage_model.a":{},"cond_stage_model.transformer.b":{}}"#;
    let mut v = (body.len() as u64).to_le_bytes().to_vec();
    v.extend_from_slice(body);
    v.extend_from_slice(&[7u8; 64]);
    v
}

/// Answers by path until the test ends. Returns the base URL.
fn serve(answers: Vec<(String, Vec<u8>)>) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            let _ = r.read_line(&mut line);
            let path = line.split(' ').nth(1).unwrap_or("").to_string();
            let body = answers
                .iter()
                .find(|(p, _)| path.starts_with(p))
                .map(|x| x.1.clone());
            let mut s = s;
            let head = match &body {
                Some(b) => format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    b.len()
                ),
                None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_string(),
            };
            let _ = s.write_all(head.as_bytes());
            if let Some(b) = body {
                let _ = s.write_all(&b);
            }
        }
    });
    base
}

#[test]
fn a_model_is_added_pinned_and_runs_through_its_file() {
    let d = tempfile::tempdir().unwrap();
    let (cfg, data) = (d.path().join("cfg"), d.path().join("data"));
    std::env::set_var("SCRIPTVOX_DATA", &data);
    let run = |args: &[&str], extra: &[(&str, &str)]| -> (i32, Value) {
        let mut c = common::bin();
        c.current_dir(d.path())
            .env("SCRIPTVOX_DATA", &data)
            .env("SCRIPTVOX_CONFIG", &cfg)
            .arg("--json")
            .args(args);
        for (k, v) in extra {
            c.env(k, v);
        }
        let o = c.output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice(&o.stdout).unwrap(),
        )
    };

    // The engine, as a stub: it records its arguments in the -o file.
    let pin = scriptvox::provision::pin("sd-cli").unwrap();
    let bin = scriptvox::provision::tool_dir(&pin).join(&pin.bins[0]);
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(&bin, "#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = -o ] && o=$2; a=\"$a $1\"; shift; done\n[ -n \"$o\" ] && echo \"$a\" > \"$o\"\nexit 0\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();

    let bytes = sd1_bytes();
    let sha: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let info = json!({"sha": "abc123", "cardData": {"license": "creativeml-openrail-m"},
        "siblings": [{"rfilename": "m.safetensors", "size": bytes.len(), "lfs": {"sha256": sha, "size": bytes.len()}},
                     {"rfilename": "README.md", "size": 5}]});
    let base = serve(vec![
        ("/api/models/org/m".into(), info.to_string().into_bytes()),
        ("/org/m/resolve/abc123/m.safetensors".into(), bytes.clone()),
    ]);
    let hf = [("SCRIPTVOX_HF_BASE", base.as_str())];

    let (code, v) = run(&["models", "add", "org/m", "--dry-run"], &hf);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["source"]["hash"], "published", "{v}");
    assert!(
        !cfg.join("generators.toml").exists(),
        "a dry run writes nothing"
    );

    let (code, v) = run(&["models", "add", "org/m", "--name", "my-sd"], &hf);
    assert_eq!(code, 0, "{v}");
    let s = &v["source"];
    assert_eq!(s["revision"], "abc123", "{v}");
    assert_eq!(s["sha256"].as_str().unwrap(), sha, "{v}");
    assert_eq!(s["licence"], "creativeml-openrail-m", "{v}");
    assert_eq!(s["family"], "sd1", "{v}");

    let (code, v) = run(
        &[
            "generate", "--model", "my-sd", "--prompt", "x", "--aspect", "1:1",
        ],
        &[],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    let args = std::fs::read_to_string(v["artifacts"]["image"].as_str().unwrap()).unwrap();
    assert!(
        args.contains("m.safetensors")
            && args.contains("-W 512 -H 512")
            && args.contains("--steps 20"),
        "{args}"
    );

    // A local pickle: warned, the hash is only observed, nothing asks about the licence.
    let ckpt = d.path().join("old.ckpt");
    std::fs::write(&ckpt, [0x80, 2, 3, 4, 5, 6, 7, 8, 9, 10]).unwrap();
    let (code, v) = run(&["models", "add", ckpt.to_str().unwrap()], &[]);
    assert_eq!(code, 2, "the family cannot be read from a pickle: {v}");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("--family"),
        "{v}"
    );
    let (code, v) = run(
        &["models", "add", ckpt.to_str().unwrap(), "--family", "sd1"],
        &[],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["source"]["hash"], "observed, not verified", "{v}");
    assert_eq!(v["findings"][0]["status"], "warn", "{v}");
    assert!(
        v["findings"][0]["detail"]
            .as_str()
            .unwrap()
            .contains("can run code"),
        "{v}"
    );

    // A family with no engine: refused, naming what is missing, and no row.
    let flux = d.path().join("flux.safetensors");
    let body = br#"{"model.diffusion_model.double_blocks.0.x":{}}"#;
    let mut b = (body.len() as u64).to_le_bytes().to_vec();
    b.extend_from_slice(body);
    std::fs::write(&flux, b).unwrap();
    let (code, v) = run(&["models", "add", flux.to_str().unwrap()], &[]);
    assert_eq!(code, 10, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Missing: the VAE"),
        "{v}"
    );

    // A fal endpoint needs no key to add, and its price stays unknown.
    let (code, v) = run(&["models", "add", "fal-ai/new-one/text-to-video"], &[]);
    assert_eq!(code, 0, "{v}");
    let (_, v) = run(&["providers"], &[]);
    let row = v["generators"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["name"] == "new-one-text-to-video")
        .unwrap();
    assert_eq!(row["price"], "unknown", "{row}");

    // The unknown-model error says that adding is possible.
    let (_, v) = run(
        &["generate", "--model", "nothing-here", "--prompt", "x"],
        &[],
    );
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("models add"),
        "{v}"
    );
}

#[test]
fn mcp_add_needs_the_users_yes_on_the_size() {
    let v = scriptvox::mcp::handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "scriptvox_models", "arguments": {"subcommand": "add", "args": ["org/m"]}}}))
    .unwrap();
    assert_eq!(v["result"]["isError"], true, "{v}");
    assert!(
        v["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("--dry-run"),
        "{v}"
    );
}
