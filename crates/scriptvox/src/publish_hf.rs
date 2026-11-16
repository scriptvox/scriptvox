//! `scriptvox publish hf` (`docs/specs/publish.md`, section 3): an export folder to a Hugging Face dataset or Space,
//! over the plain HTTP API of the Hub. The token is the `hf` key. It goes only to the Hub itself, never to the
//! address that takes the bytes of a large file.

use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::publish::{
    approval, http, publish_id, reason, record, save_record, usage, Folder, COMMAND,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The largest file of one plain LFS request.
const MAX_FILE: u64 = 5_000_000_000;

/// The Hub. `SCRIPTVOX_HF_BASE` (only `http://127.0.0.1:<port>`) names a test server, as in `models add`.
fn base() -> String {
    std::env::var("SCRIPTVOX_HF_BASE")
        .ok()
        .filter(|b| crate::generators::loopback_base_ok(b))
        .unwrap_or_else(|| "https://huggingface.co".into())
}

struct Item {
    path: String,
    /// None: made here (the page of a Space).
    file: Option<PathBuf>,
    bytes: Vec<u8>,
    size: u64,
    hash: String,
}

/// Every file of the folder, in name order. `publish.json` stays here: it is the record of this machine. A link is
/// refused, not followed: it could name a file outside the folder (a key file), and the person approves a folder.
fn walk(root: &Path, dir: &Path, out: &mut Vec<Item>) -> Result<(), String> {
    let at = |p: &Path, e: std::io::Error| format!("cannot read {}: {e}", p.display());
    let real_root = root.canonicalize().map_err(|e| at(root, e))?;
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .and_then(|d| d.collect::<Result<_, _>>())
        .map_err(|e| at(dir, e))?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        // The type of the entry itself: `is_file` and `is_dir` would follow a link.
        let kind = e.file_type().map_err(|e| at(&p, e))?;
        if kind.is_symlink() {
            return Err(format!("{rel} is a link, and a link is not uploaded"));
        }
        if !p
            .canonicalize()
            .map_err(|e| at(&p, e))?
            .starts_with(&real_root)
        {
            return Err(format!("{rel} is not inside the folder"));
        }
        if kind.is_dir() {
            walk(root, &p, out)?;
        } else if kind.is_file() && rel != "publish.json" && rel != "publish.json.tmp" {
            out.push(Item {
                size: p.metadata().map_err(|e| at(&p, e))?.len(),
                hash: crate::store::hash_file(&p).map_err(|e| at(&p, e))?,
                path: rel,
                file: Some(p),
                bytes: vec![],
            });
        } else if !kind.is_file() {
            return Err(format!("{rel} is not a plain file"));
        }
    }
    Ok(())
}

/// The page of a static Space: the video with its title. Only when the folder has no index.html of its own.
fn page(title: &str) -> String {
    let safe = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>{safe}</title></head>\n<body style=\"margin:0;background:#111;color:#eee;font-family:sans-serif;text-align:center\">\n<h1 style=\"font-size:1.2rem;padding:1rem\">{safe}</h1>\n<video controls playsinline poster=\"cover.jpg\" src=\"video.mp4\" style=\"max-width:100%;max-height:85vh\"></video>\n</body></html>\n"
    )
}

/// The two hashes of an open file, read through this one handle: blake3 (what the person approved) and SHA-256 (the
/// name of a large file on the Hub). The handle is at byte 0 again after, so the same handle sends the bytes.
fn hashes(f: &mut std::fs::File) -> std::io::Result<(String, String)> {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Seek};
    f.seek(std::io::SeekFrom::Start(0))?;
    let (mut b3, mut sha) = (blake3::Hasher::new(), Sha256::new());
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        b3.update(&buf[..n]);
        sha.update(&buf[..n]);
    }
    f.seek(std::io::SeekFrom::Start(0))?;
    Ok((
        b3.finalize().to_hex().to_string(),
        sha.finalize().iter().map(|b| format!("{b:02x}")).collect(),
    ))
}

/// A file of the folder cannot be read at upload time.
fn unreadable(path: &str, e: std::io::Error) -> Envelope {
    Envelope::fail(
        COMMAND,
        Exit::Error,
        format!("cannot read {path}: {e}. No commit was made"),
        Some(
            "Run `scriptvox export <run_id>` again, then the publish command without --approve."
                .into(),
        ),
    )
}

/// A file is not what the person approved any more. The whole publish stops.
fn changed(path: &str) -> Envelope {
    Envelope::fail(
        COMMAND,
        Exit::Error,
        format!("{path} changed after the approval. The upload stopped, and no commit was made"),
        Some("Run the same command without --approve: it shows the upload as it is now, for a new yes of the person.".into()),
    )
}

/// One JSON call of the Hub with the token: (HTTP code, body). 0: no answer.
fn hub(method: &str, url: &str, token: &str, kind: &str, body: Option<String>) -> (u16, String) {
    let agent = http(120);
    let auth = format!("Bearer {token}");
    let answer = match body {
        Some(b) => agent
            .post(url)
            .header("Authorization", &auth)
            .header("Content-Type", kind)
            .header(
                "Accept",
                if kind.contains("lfs") {
                    kind
                } else {
                    "application/json"
                },
            )
            .send(b),
        None if method == "GET" => agent.get(url).header("Authorization", &auth).call(),
        None => return (0, String::new()),
    };
    match answer {
        Ok(mut r) => (
            r.status().as_u16(),
            r.body_mut().read_to_string().unwrap_or_default(),
        ),
        Err(_) => (0, String::new()),
    }
}

/// The answer for a call that failed: the token (401, 403), a fault that a retry can cure, or a refusal.
fn fault(step: &str, code: u16, body: &str, id: &str) -> Envelope {
    let why = reason(body);
    match code {
        401 | 403 => Envelope::fail(
            COMMAND,
            Exit::NeedsInput,
            format!("Hugging Face refused the token at the step \"{step}\": HTTP {code}{why}"),
            Some("The token must be able to write to this repo. Make one at huggingface.co/settings/tokens, then run `scriptvox keys set hf` in your terminal.".into()),
        ),
        0 | 408 | 429 | 500..=599 => Envelope::fail(
            COMMAND,
            Exit::ProviderRetryable,
            format!("Hugging Face gave no good answer at the step \"{step}\": HTTP {code}{why}"),
            Some(format!("Run the same command again with --approve {id}: files that arrived are not sent again.")),
        ),
        _ => Envelope::fail(
            COMMAND,
            Exit::Error,
            format!("Hugging Face refused the step \"{step}\": HTTP {code}{why}"),
            Some("Check --repo (<owner>/<name>, an owner that you can write to) and --type.".into()),
        ),
    }
}

pub fn run(f: &Folder, repo: &str, kind: &str, public: bool, plan_only: bool) -> Envelope {
    if !["dataset", "space"].contains(&kind) {
        return usage(
            format!("--type {kind} is not a repo type"),
            "Use dataset (the default) or space.",
        );
    }
    let name_ok = |s: &str| {
        !s.is_empty()
            && s.len() <= 96
            && !s.starts_with(['.', '-'])
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    };
    let Some((owner, name)) = repo
        .split_once('/')
        .filter(|(o, n)| name_ok(o) && name_ok(n))
    else {
        return usage(
            format!("--repo {repo} is not <owner>/<name>"),
            "Give your user or org name, a slash and the repo name: letters, digits, '.', '_' and '-'.",
        );
    };
    let mut items = vec![];
    if let Err(e) = walk(&f.dir, &f.dir, &mut items) {
        return usage(
            format!("{e}. Nothing was sent"),
            "Put a copy of the file in the folder in place of the link, or remove it. Or run `scriptvox export <run_id>` again.",
        );
    }
    if kind == "space" && !items.iter().any(|i| i.path == "index.html") {
        let bytes = page(f.meta["title"].as_str().unwrap_or("Video")).into_bytes();
        items.push(Item {
            path: "index.html".into(),
            file: None,
            size: bytes.len() as u64,
            hash: blake3::hash(&bytes).to_hex().to_string(),
            bytes,
        });
    }
    if let Some(big) = items.iter().find(|i| i.size > MAX_FILE) {
        return usage(
            format!("{} is {} bytes: above the 5 GB of one upload request", big.path, big.size),
            "Export a smaller file (`scriptvox export <run_id> --max-mb N`), or remove that file from the folder.",
        );
    }
    let private = !public;
    let listed: Vec<Value> = items
        .iter()
        .map(|i| json!({"path": i.path, "bytes": i.size, "hash": i.hash[..16]}))
        .collect();
    let id = publish_id(&json!({
        "platform": "hf", "repo": repo, "type": kind, "private": private,
        "files": items.iter().map(|i| json!([i.path, i.hash])).collect::<Vec<_>>(),
    }));
    let done = record(&f.dir, "hf");
    if done["publish_id"] == id.as_str() && done["commit"].is_string() {
        let mut env = Envelope::ok(COMMAND)
            .with("platform", json!("hf"))
            .with("publish_id", json!(id))
            .with("already_published", json!(true))
            .with("repo", done["repo"].clone())
            .with("commit", done["commit"].clone())
            .with("private", done["private"].clone());
        env.artifacts.insert("url".into(), done["url"].clone());
        env.metrics.insert("bytes_sent".into(), json!(0));
        return env;
    }
    let Some((token, _)) = crate::keys::resolve("hf") else {
        let fix = "Run `scriptvox keys set hf` in your terminal, with a token that can write (huggingface.co/settings/tokens).";
        let mut env = Envelope::fail(
            COMMAND,
            Exit::NeedsInput,
            "no Hugging Face token is set",
            Some(fix.into()),
        );
        env.needs_input = vec![crate::voices::needs_input("hf", "secret", fix, vec![])];
        env.next = vec!["scriptvox keys set hf".into()];
        return env;
    };
    let total: u64 = items.iter().map(|i| i.size).sum();
    let visibility = if private { "private" } else { "public" };
    if plan_only || crate::plan::approved() != Some(id.as_str()) {
        let mut show = json!({
            "platform": "hf", "repo": repo, "type": kind, "visibility": visibility,
            "folder": f.dir, "files": listed, "bytes": total,
        });
        let list: Vec<String> = items
            .iter()
            .map(|i| format!("{} ({} bytes)", i.path, i.size))
            .collect();
        let mut what = format!(
            "This uploads {} files ({total} bytes) of {} to the Hugging Face {kind} {repo}, {visibility}: {}. A repo that does not exist is made, {visibility}.",
            items.len(),
            f.dir.display(),
            list.join(", ")
        );
        if !private {
            what += " Public: everyone can see and download these files.";
        }
        if let Some(c) = done["commit"].as_str() {
            show["already_uploaded_as"] = json!(c);
            what += &format!(
                " This folder was uploaded before (commit {c}, repo {}).",
                done["repo"].as_str().unwrap_or("")
            );
        }
        return approval(&id, &what, show);
    }

    if crate::plan::local_only() {
        return usage(
            crate::plan::LOCAL_ONLY_STOP.to_string(),
            "Run without --local-only.",
        );
    }
    let hub_base = base();
    let api = format!("{hub_base}/api/{kind}s/{repo}");
    let web = format!("{hub_base}/{kind}s/{repo}");
    // 1. The repo. A new one is made with the visibility that was approved.
    let mut create = json!({"type": kind, "name": name, "organization": owner, "private": private});
    if kind == "space" {
        create["sdk"] = json!("static");
    }
    let (code, body) = hub(
        "POST",
        &format!("{hub_base}/api/repos/create"),
        &token,
        "application/json",
        Some(create.to_string()),
    );
    let created = code == 200 || code == 201;
    let mut is_private = private;
    if code == 409 {
        // It exists. "Private by default" must hold for a repo that someone made public before.
        let (code, body) = hub("GET", &api, &token, "", None);
        if code != 200 {
            return fault("read the repo", code, &body, &id);
        }
        is_private = serde_json::from_str::<Value>(&body).unwrap_or_default()["private"] == true;
        if !is_private && private {
            return usage(
                format!("the {kind} {repo} is public, and this upload was approved as private. Nothing was uploaded"),
                "Make the repo private on huggingface.co, or use another --repo. For a public upload: --public, with its own approval.",
            );
        }
    } else if !created {
        return fault("make the repo", code, &body, &id);
    }
    // 2. Which files are large files: the Hub decides, from the name, the size and the first bytes.
    for i in items.iter_mut().filter(|i| i.file.is_some()) {
        use std::io::Read;
        let mut head = vec![0u8; 512.min(i.size as usize)];
        let read =
            std::fs::File::open(i.file.as_ref().unwrap()).and_then(|mut f| f.read_exact(&mut head));
        if let Err(e) = read {
            return unreadable(&i.path, e);
        }
        i.bytes = head;
    }
    use base64::Engine;
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    let ask = json!({"files": items.iter().map(|i| json!({"path": i.path, "size": i.size, "sample": b64(&i.bytes[..i.bytes.len().min(512)])})).collect::<Vec<_>>()});
    let (code, body) = hub(
        "POST",
        &format!("{api}/preupload/main"),
        &token,
        "application/json",
        Some(ask.to_string()),
    );
    if code != 200 {
        return fault("ask how to upload", code, &body, &id);
    }
    let modes: Value = serde_json::from_str(&body).unwrap_or_default();
    let mode = |path: &str| {
        modes["files"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|m| m["path"] == path)
            .cloned()
            .unwrap_or_default()
    };
    // 3. Large files: the Git LFS batch API names an address for each one that the Hub does not have yet.
    let mut lines = vec![
        json!({"key": "header", "value": {"summary": "Upload from scriptvox", "description": ""}}),
    ];
    let (mut sent, mut skipped) = (0u64, vec![]);
    for i in &items {
        let m = mode(&i.path);
        if m["shouldIgnore"] == true {
            skipped.push(i.path.clone());
            continue;
        }
        let Some(file) = i.file.as_ref().filter(|_| m["uploadMode"] == "lfs") else {
            // Read one time: the bytes that are checked against the approval are the bytes that go in the commit.
            let bytes = match &i.file {
                Some(p) => match std::fs::read(p) {
                    Ok(b) => b,
                    Err(e) => return unreadable(&i.path, e),
                },
                None => i.bytes.clone(),
            };
            if blake3::hash(&bytes).to_hex().as_str() != i.hash {
                return changed(&i.path);
            }
            sent += bytes.len() as u64;
            lines.push(json!({"key": "file", "value": {"path": i.path, "content": b64(&bytes), "encoding": "base64"}}));
            continue;
        };
        // One open handle: it is hashed again here, right before the send, and the same handle gives the bytes.
        let mut handle = match std::fs::File::open(file) {
            Ok(f) => f,
            Err(e) => return unreadable(&i.path, e),
        };
        let oid = match hashes(&mut handle) {
            Ok((b3, _)) if b3 != i.hash => return changed(&i.path),
            Ok((_, sha)) => sha,
            Err(e) => return unreadable(&i.path, e),
        };
        let batch = json!({"operation": "upload", "transfers": ["basic"], "hash_algo": "sha256",
                           "objects": [{"oid": oid, "size": i.size}]});
        let (code, body) = hub(
            "POST",
            &format!("{web}.git/info/lfs/objects/batch"),
            &token,
            "application/vnd.git-lfs+json",
            Some(batch.to_string()),
        );
        if code != 200 {
            return fault("ask where a large file goes", code, &body, &id);
        }
        let object = serde_json::from_str::<Value>(&body).unwrap_or_default()["objects"][0].clone();
        if object["error"].is_object() {
            return fault(
                "ask where a large file goes",
                object["error"]["code"].as_u64().unwrap_or(0) as u16,
                "",
                &id,
            );
        }
        // No `upload` action: the Hub has these bytes already (a run that stopped after this file).
        if let Some(href) = object["actions"]["upload"]["href"].as_str() {
            if !(href.starts_with("https://") || href.starts_with(&format!("{hub_base}/"))) {
                return Envelope::fail(
                    COMMAND,
                    Exit::Error,
                    "the upload address of a large file is not https. No commit was made",
                    Some("Run the same command again. If it stays, report it: the Hub gave an address that is not safe.".into()),
                );
            }
            // The address is signed for this one file. The token does not go there.
            let mut put = http(12 * 3600)
                .put(href)
                .header("Content-Length", i.size.to_string());
            for (k, v) in object["actions"]["upload"]["header"]
                .as_object()
                .into_iter()
                .flatten()
            {
                if let Some(v) = v.as_str() {
                    put = put.header(k.as_str(), v);
                }
            }
            let answer = put
                .send(ureq::SendBody::from_reader(&mut handle))
                .map_err(|_| 0u16);
            match answer.map(|r| r.status().as_u16()) {
                Ok(200..=299) => sent += i.size,
                Ok(code) | Err(code) => {
                    return fault(
                        &format!("send {}", i.path),
                        if code == 403 { 400 } else { code },
                        "",
                        &id,
                    )
                }
            }
            if let Some(verify) = object["actions"]["verify"]["href"]
                .as_str()
                .filter(|v| v.starts_with(&format!("{hub_base}/")))
            {
                let (code, body) = hub(
                    "POST",
                    verify,
                    &token,
                    "application/vnd.git-lfs+json",
                    Some(json!({"oid": oid, "size": i.size}).to_string()),
                );
                if code != 200 {
                    return fault(&format!("confirm {}", i.path), code, &body, &id);
                }
            }
        }
        lines.push(json!({"key": "lfsFile", "value": {"path": i.path, "algo": "sha256", "oid": oid, "size": i.size}}));
    }
    // 4. One commit with all files.
    let ndjson: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let (code, body) = hub(
        "POST",
        &format!("{api}/commit/main"),
        &token,
        "application/x-ndjson",
        Some(ndjson),
    );
    if code != 200 {
        return fault("commit", code, &body, &id);
    }
    let commit = serde_json::from_str::<Value>(&body).unwrap_or_default()["commitOid"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let url = format!("https://huggingface.co/{kind}s/{repo}");
    let entry = json!({
        "publish_id": id, "repo": repo, "type": kind, "private": is_private, "commit": commit, "url": url,
        "files": items.len() - skipped.len(), "at": crate::run::now_utc(),
    });
    let saved = save_record(&f.dir, "hf", entry);
    let mut env = Envelope::ok(COMMAND)
        .with("platform", json!("hf"))
        .with("publish_id", json!(id))
        .with("already_published", json!(false))
        .with("repo", json!(repo))
        .with("type", json!(kind))
        .with("private", json!(is_private))
        .with("repo_created", json!(created))
        .with("commit", json!(commit))
        .with("skipped", json!(skipped));
    env.artifacts.insert("url".into(), json!(url));
    env.artifacts
        .insert("record".into(), json!(f.dir.join("publish.json")));
    env.metrics
        .insert("files".into(), json!(items.len() - skipped.len()));
    env.metrics.insert("bytes".into(), json!(total));
    env.metrics.insert("bytes_sent".into(), json!(sent));
    if is_private && public {
        env.findings.push(json!({"gate": "visibility", "status": "warn",
            "detail": "the repo was there and is private; --public does not change a repo that exists",
            "fix": "Change the visibility in the settings of the repo on huggingface.co."}));
    }
    if let Err(e) = saved {
        env.findings.push(json!({"gate": "publish_record", "status": "warn",
            "detail": format!("the files are uploaded, and publish.json was not written: {e}"),
            "fix": "Run the same command again: no large file is sent again, and one more commit is made."}));
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_of_a_space_holds_the_title_as_text() {
        let p = page("A <b> & C");
        assert!(
            p.contains("<title>A &lt;b&gt; &amp; C</title>") && p.contains("src=\"video.mp4\"")
        );
    }

    #[test]
    fn the_record_of_this_machine_is_not_a_file_to_upload() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("hls")).unwrap();
        for f in [
            "video.mp4",
            "publish.json",
            "hls/index.m3u8",
            "metadata.json",
        ] {
            std::fs::write(d.path().join(f), f).unwrap();
        }
        let mut items = vec![];
        walk(d.path(), d.path(), &mut items).unwrap();
        let mut f = std::fs::File::open(d.path().join("video.mp4")).unwrap();
        let (b3, sha) = hashes(&mut f).unwrap();
        assert_eq!(b3, items[2].hash);
        // SHA-256 of "video.mp4".
        assert_eq!(sha.len(), 64);
        let names: Vec<&str> = items.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(names, ["hls/index.m3u8", "metadata.json", "video.mp4"]);
    }
}
