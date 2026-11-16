//! `scriptvox publish youtube` and `publish setup youtube` (`docs/specs/publish.md`). The person's own Google OAuth
//! client, the scope `youtube.upload` only, and the resumable upload of the YouTube Data API v3. The client secret,
//! the refresh token and the upload session address are values of keys.toml, so `Redactor::from_env` covers them.
//! The access token lives only in this process. No answer body and no address with a secret goes into a message.

use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::publish::{
    approval, http, publish_id, reason, record, save_record, usage, Folder, COMMAND,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{IsTerminal, Read, Seek, Write};

const SCOPE: &str = "https://www.googleapis.com/auth/youtube.upload";
const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const SETUP: &str = "scriptvox publish setup youtube";
const K_ID: &str = "youtube_client_id";
const K_SECRET: &str = "youtube_client_secret";
const K_REFRESH: &str = "youtube_refresh_token";
/// The session address of a started upload. Whoever has it can send bytes to that upload, so it is kept as a key.
const K_SESSION: &str = "youtube_session";
/// Tries of one command after a fault that a retry can cure. The next run of the command goes on from the same byte.
const TRIES: u32 = 3;

struct Ends {
    token: String,
    upload: String,
}

/// Google's addresses. `SCRIPTVOX_YOUTUBE_BASE` (only `http://127.0.0.1:<port>`) names a test server in their place.
fn ends() -> Ends {
    match std::env::var("SCRIPTVOX_YOUTUBE_BASE")
        .ok()
        .filter(|b| crate::generators::loopback_base_ok(b))
    {
        Some(b) => Ends {
            token: format!("{b}/token"),
            upload: format!("{b}/upload/youtube/v3/videos"),
        },
        None => Ends {
            token: "https://oauth2.googleapis.com/token".into(),
            upload: "https://www.googleapis.com/upload/youtube/v3/videos".into(),
        },
    }
}

/// A session address gets the access token, so it must be on the host that the upload started on (a test server),
/// or on a googleapis.com host.
fn session_ok(e: &Ends, uri: &str) -> bool {
    let host = |u: &str| {
        u.split_once("://").map(|(s, rest)| {
            (
                s.to_string(),
                rest.split('/').next().unwrap_or("").to_string(),
            )
        })
    };
    match (host(uri), host(&e.upload)) {
        (Some(a), Some(b)) if a == b => true,
        (Some((s, h)), _) => s == "https" && h.ends_with(".googleapis.com"),
        _ => false,
    }
}

/// The part of the setup that is not a secret: the channel name that the person gave, and which upload the stored
/// session belongs to. `publish.toml` in the config folder.
fn notes_file() -> std::path::PathBuf {
    crate::keys::config_dir().join("publish.toml")
}

fn notes() -> BTreeMap<String, String> {
    std::fs::read_to_string(notes_file())
        .ok()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default()
}

fn note_set(key: &str, value: &str) -> std::io::Result<()> {
    let mut all = notes();
    all.insert(key.into(), value.into());
    std::fs::create_dir_all(crate::keys::config_dir())?;
    std::fs::write(
        notes_file(),
        toml::to_string(&all).map_err(std::io::Error::other)?,
    )
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The text of one query value: `%2F` is `/`, `+` is a space.
fn dec(s: &str) -> String {
    let b = s.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < b.len() {
        let hex = (b[i] == b'%')
            .then(|| {
                s.get(i + 1..i + 3)
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
            })
            .flatten();
        match hex {
            Some(v) => {
                out.push(v);
                i += 3;
            }
            None => {
                out.push(if b[i] == b'+' { b' ' } else { b[i] });
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// `n` bytes of the OS random source as URL-safe text: the PKCE verifier and the `state` of the sign in. One source on
/// every platform, and no fallback: a machine with no random source gets an error, never a value that can be guessed.
fn random(n: usize) -> Result<String, String> {
    use base64::Engine;
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b)
        .map_err(|e| format!("no random bytes from the operating system: {e}"))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b))
}

fn challenge(verifier: &str) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn auth_address(client_id: &str, redirect: &str, verifier: &str, state: &str) -> String {
    format!(
        "{AUTH}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        enc(client_id),
        enc(redirect),
        enc(SCOPE),
        challenge(verifier),
        enc(state)
    )
}

/// Waits for the browser to come back to the loopback port with `code` and our `state`. Any other request (an icon,
/// a wrong state) gets a short answer and is not used.
fn wait_code(
    l: &std::net::TcpListener,
    state: &str,
    limit: std::time::Duration,
) -> Result<String, String> {
    l.set_nonblocking(true).map_err(|e| e.to_string())?;
    let end = std::time::Instant::now() + limit;
    while std::time::Instant::now() < end {
        let Ok((mut s, _)) = l.accept() else {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        };
        let _ = s.set_nonblocking(false);
        let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(5)));
        let mut buf = [0u8; 4096];
        let n = s.read(&mut buf).unwrap_or(0);
        let head = String::from_utf8_lossy(&buf[..n]).to_string();
        let query = head
            .split(' ')
            .nth(1)
            .and_then(|p| p.split_once('?'))
            .map_or("", |(_, q)| q)
            .to_string();
        let get = |k: &str| {
            query
                .split('&')
                .filter_map(|kv| kv.split_once('='))
                .find(|(key, _)| *key == k)
                .map(|(_, v)| dec(v))
        };
        let mine = get("state").as_deref() == Some(state);
        let answer = match (mine, get("code"), get("error")) {
            (true, Some(code), _) => Ok(code),
            (true, None, Some(e)) => Err(format!(
                "Google gave no code: {}",
                e.chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .take(40)
                    .collect::<String>()
            )),
            _ => {
                let _ = s.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                continue;
            }
        };
        let page = if answer.is_ok() {
            "ScriptVox has the answer. You can close this page."
        } else {
            "No access was given. You can close this page."
        };
        let _ = s.write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
                page.len()
            )
            .as_bytes(),
        );
        return answer;
    }
    Err("no answer from the browser in time".into())
}

/// One call of the token address. The error is the HTTP code (0: no answer) and one clean word of the answer.
fn token_call(e: &Ends, form: &[(&str, &str)]) -> Result<Value, (u16, String)> {
    let mut resp = http(60)
        .post(&e.token)
        .send_form(form.iter().copied())
        .map_err(|_| (0, String::new()))?;
    let code = resp.status().as_u16();
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    if code != 200 {
        return Err((code, reason(&body)));
    }
    serde_json::from_str(&body).map_err(|_| (code, String::new()))
}

/// `publish setup youtube`: only a person, only in a terminal.
pub fn setup(channel: Option<String>, agent: bool) -> Envelope {
    if agent || !std::io::stdin().is_terminal() {
        let mut env = Envelope::fail(
            COMMAND,
            Exit::NeedsInput,
            "only the person can connect a YouTube account: it takes the person's own Google OAuth client and a yes in the browser",
            Some(format!("Run `{SETUP}` in your terminal. docs/specs/publish.md has the steps to make the client.")),
        );
        env.needs_input = vec![crate::voices::needs_input(
            "youtube.oauth",
            "secret",
            &format!("Run `{SETUP}` in your terminal."),
            vec![],
        )];
        env.next = vec![SETUP.to_string()];
        return env;
    }
    let fail = |m: String| {
        Envelope::fail(
            COMMAND,
            Exit::Error,
            m,
            Some(format!("Run `{SETUP}` again.")),
        )
    };
    let ask = |what: &str| crate::keys::prompt(what).ok().filter(|v| !v.is_empty());
    let (Some(id), Some(secret)) = (
        ask("Google OAuth client id"),
        ask("Google OAuth client secret"),
    ) else {
        return fail("the client id and the client secret are both needed".into());
    };
    let channel = channel.or_else(|| {
        eprint!("Channel name (shown in the approval text): ");
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).ok()?;
        Some(s)
    });
    let channel = channel.unwrap_or_default().trim().to_string();
    let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(e) => return fail(format!("cannot open a port on this machine: {e}")),
    };
    let redirect = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().map_or(0, |a| a.port())
    );
    let (verifier, state) = match (random(48), random(24)) {
        (Ok(v), Ok(s)) => (v, s),
        (Err(e), _) | (_, Err(e)) => return fail(e),
    };
    // To the terminal directly: the address holds the client id, and the envelope would print it as `***`.
    eprintln!(
        "\nOpen this address in a browser on this machine, choose the channel, and say yes:\n\n{}\n\nWaiting for at most 5 minutes...",
        auth_address(&id, &redirect, &verifier, &state)
    );
    let code = match wait_code(&listener, &state, std::time::Duration::from_secs(300)) {
        Ok(c) => c,
        Err(e) => return fail(e),
    };
    let e = ends();
    let answer = token_call(
        &e,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("code_verifier", &verifier),
            ("redirect_uri", &redirect),
            ("client_id", &id),
            ("client_secret", &secret),
        ],
    );
    let refresh = match answer {
        Ok(v) => match v["refresh_token"].as_str() {
            Some(t) => t.to_string(),
            None => return fail("Google gave no refresh token".into()),
        },
        Err((code, why)) => return fail(format!("Google refused the code: HTTP {code}{why}")),
    };
    for (k, v) in [(K_ID, &id), (K_SECRET, &secret), (K_REFRESH, &refresh)] {
        if let Err(e) = crate::keys::set(k, v) {
            return fail(format!("cannot store the sign-in: {e}"));
        }
    }
    let _ = note_set("youtube_channel", &channel);
    let mut env = Envelope::ok(COMMAND)
        .with("platform", json!("youtube"))
        .with("channel", json!(channel))
        .with("scope", json!(SCOPE))
        .with("stored_in", json!(crate::keys::file()));
    env.next = vec!["scriptvox publish youtube <export folder> --json".into()];
    env
}

struct Creds {
    id: String,
    secret: String,
    refresh: String,
}

fn creds() -> Option<Creds> {
    let get = |k| crate::keys::resolve(k).map(|(v, _)| v);
    Some(Creds {
        id: get(K_ID)?,
        secret: get(K_SECRET)?,
        refresh: get(K_REFRESH)?,
    })
}

/// A new access token from the refresh token.
fn access(e: &Ends, c: &Creds) -> Result<String, Box<Envelope>> {
    let answer = token_call(
        e,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &c.refresh),
            ("client_id", &c.id),
            ("client_secret", &c.secret),
        ],
    );
    match answer {
        Ok(v) => v["access_token"].as_str().map(String::from).ok_or_else(|| {
            Box::new(Envelope::fail(
                COMMAND,
                Exit::ProviderRetryable,
                "the token answer of Google has no access token",
                Some("Run the same command again.".into()),
            ))
        }),
        Err((code @ (400 | 401), why)) => Err(Box::new(Envelope::fail(
            COMMAND,
            Exit::NeedsInput,
            format!("Google does not accept the stored sign-in any more: HTTP {code}{why}"),
            Some(format!("Run `{SETUP}` in your terminal.")),
        ))),
        Err((code, why)) => Err(Box::new(Envelope::fail(
            COMMAND,
            Exit::ProviderRetryable,
            format!("no token from Google: HTTP {code}{why}"),
            Some("Check the network and run the same command again. Nothing was uploaded.".into()),
        ))),
    }
}

/// What one request of the upload says.
enum Step {
    /// The video resource: the upload is complete.
    Done(Value),
    /// The server has the bytes before this one.
    At(u64),
    /// The session is not there any more.
    Gone,
    /// The access token was refused.
    Denied,
    /// A fault that a retry can cure (no answer, 500, 502, 503, 504).
    Again(String),
    /// A refusal that a retry does not cure.
    Fatal(String),
    /// The video file is not the one that the person approved.
    Changed,
}

fn read(answer: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Step {
    let mut resp = match answer {
        Ok(r) => r,
        // The error text of a transport fault can hold the address: only its kind is kept.
        Err(ureq::Error::Timeout(_)) => return Step::Again("timeout".into()),
        Err(_) => return Step::Again("the connection broke".into()),
    };
    let code = resp.status().as_u16();
    match code {
        200 | 201 => {
            let body = resp.body_mut().read_to_string().unwrap_or_default();
            match serde_json::from_str::<Value>(&body) {
                Ok(v) if v["id"].is_string() => Step::Done(v),
                _ => Step::Fatal(format!("HTTP {code} with no video id")),
            }
        }
        308 => Step::At(
            resp.headers()
                .get("range")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.rsplit_once('-'))
                .and_then(|(_, last)| last.trim().parse::<u64>().ok())
                .map_or(0, |last| last + 1),
        ),
        401 => Step::Denied,
        404 | 410 => Step::Gone,
        500 | 502 | 503 | 504 => Step::Again(format!("HTTP {code}")),
        _ => {
            let body = resp.body_mut().read_to_string().unwrap_or_default();
            Step::Fatal(format!("HTTP {code}{}", reason(&body)))
        }
    }
}

/// Starts a session: the video resource goes up, the session address comes back in `Location`.
fn start(e: &Ends, token: &str, resource: &Value, size: u64) -> Result<String, Step> {
    let answer = http(60)
        .post(format!(
            "{}?uploadType=resumable&part=snippet,status",
            e.upload
        ))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json; charset=UTF-8")
        .header("X-Upload-Content-Length", size.to_string())
        .header("X-Upload-Content-Type", "video/mp4")
        .send(resource.to_string());
    let location = answer
        .as_ref()
        .ok()
        .filter(|r| r.status() == 200)
        .and_then(|r| {
            r.headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        });
    match location {
        Some(uri) if session_ok(e, &uri) => Ok(uri),
        Some(_) => Err(Step::Fatal("the session address is on another host".into())),
        None => Err(match read(answer) {
            Step::Done(_) | Step::At(_) => Step::Fatal("no session address in the answer".into()),
            other => other,
        }),
    }
}

/// How many bytes the server has.
fn status(uri: &str, token: &str, size: u64) -> Step {
    read(
        http(60)
            .put(uri)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Range", format!("bytes */{size}"))
            .send(&[] as &[u8]),
    )
}

/// Sends the file from `offset` to its end.
fn send(
    uri: &str,
    token: &str,
    f: &mut std::fs::File,
    approved: &str,
    offset: u64,
    size: u64,
) -> Step {
    // Hashed again right before the bytes go, through the handle that sends them: a file that was changed after
    // the approval is not sent.
    match hash_of(f) {
        Ok(h) if h == approved => {}
        Ok(_) => return Step::Changed,
        Err(e) => return Step::Fatal(format!("cannot read the video: {e}")),
    }
    if let Err(e) = f.seek(std::io::SeekFrom::Start(offset)) {
        return Step::Fatal(format!("cannot read the video: {e}"));
    }
    // ponytail: one request for the rest of the file, 12 hours at most. Parts of a fixed size when one request
    // per file is too long for a slow line.
    let mut req = http(12 * 3600)
        .put(uri)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "video/mp4")
        .header("Content-Length", (size - offset).to_string());
    if offset > 0 {
        req = req.header(
            "Content-Range",
            format!("bytes {offset}-{}/{size}", size - 1),
        );
    }
    read(req.send(ureq::SendBody::from_reader(f)))
}

fn cut(s: &str, max: usize) -> String {
    // YouTube refuses the two angle brackets in a title and a description.
    s.replace(['<', '>'], " ")
        .chars()
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

/// blake3 of an open file, read through this handle from byte 0.
fn hash_of(f: &mut std::fs::File) -> std::io::Result<String> {
    f.seek(std::io::SeekFrom::Start(0))?;
    let mut h = blake3::Hasher::new();
    h.update_reader(&mut *f)?;
    Ok(h.finalize().to_hex().to_string())
}

pub fn run(
    f: &Folder,
    privacy: &str,
    kids: bool,
    ai: Option<bool>,
    category: &str,
    plan_only: bool,
) -> Envelope {
    if !["private", "unlisted", "public"].contains(&privacy) {
        return usage(
            format!("--privacy {privacy} is not a privacy"),
            "Use private (the default), unlisted or public.",
        );
    }
    if category.is_empty() || category.len() > 3 || !category.chars().all(|c| c.is_ascii_digit()) {
        return usage(
            format!("--category {category} is not a category id"),
            "Give the number of a YouTube category, for example 22 (People and Blogs) or 27 (Education).",
        );
    }
    let y = &f.meta["platforms"]["youtube"];
    let text = |a: &Value, b: &Value| a.as_str().or(b.as_str()).unwrap_or("").to_string();
    let title = cut(&text(&y["title"], &f.meta["title"]), 100);
    if title.is_empty() {
        return usage(
            "metadata.json of the folder has no title",
            "Run `scriptvox export <run_id> --title \"<title>\"` again.",
        );
    }
    let description = cut(&text(&y["description"], &f.meta["description"]), 5000);
    // YouTube takes 500 characters of tags in all; a tag with a space counts two more.
    let mut room = 450usize;
    let tags: Vec<String> = f.meta["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t.as_str())
        .map(|t| cut(t, 60))
        .filter(|t| !t.is_empty())
        .take_while(|t| {
            let need = t.chars().count() + 3;
            let fits = need <= room;
            room = room.saturating_sub(need);
            fits
        })
        .collect();
    let generated: Vec<&str> = f.meta["ai_parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["kind"].as_str())
        .filter(|k| ["video.gen", "image.gen", "music.gen", "sfx.gen", "lipsync"].contains(k))
        .collect();
    let ai_content = ai.unwrap_or(!generated.is_empty());
    let notes = notes();
    let channel = notes
        .get("youtube_channel")
        .filter(|c| !c.is_empty())
        .cloned()
        .unwrap_or_else(|| "(no name given at setup)".into());
    // One open handle for the whole command: what is hashed for the approval is what is sent.
    let opened = std::fs::File::open(&f.video).and_then(|mut v| {
        let hash = hash_of(&mut v)?;
        Ok((v.metadata()?.len(), hash, v))
    });
    let (size, hash, mut video) = match opened {
        Ok((size, hash, v)) if size > 0 => (size, hash, v),
        _ => {
            return usage(
                format!("cannot read {}", f.video.display()),
                "Run `scriptvox export <run_id>` again.",
            )
        }
    };
    let id = publish_id(&json!({
        "platform": "youtube", "channel": channel, "title": title, "description": description, "tags": tags,
        "privacy": privacy, "made_for_kids": kids, "ai_content": ai_content, "category": category, "file": hash,
    }));
    let done = record(&f.dir, "youtube");
    if done["publish_id"] == id.as_str() && done["video_id"].is_string() {
        let mut env = Envelope::ok(COMMAND)
            .with("platform", json!("youtube"))
            .with("publish_id", json!(id))
            .with("already_published", json!(true))
            .with("video_id", done["video_id"].clone())
            .with("privacy", done["privacy"].clone());
        env.artifacts.insert("url".into(), done["url"].clone());
        env.metrics.insert("bytes_sent".into(), json!(0));
        return env;
    }
    let Some(creds) = creds() else {
        let mut env = Envelope::fail(
            COMMAND,
            Exit::NeedsInput,
            "no YouTube account is connected",
            Some(format!(
                "Run `{SETUP}` in your terminal. Only the person can do this step."
            )),
        );
        env.next = vec![SETUP.to_string()];
        return env;
    };
    if plan_only || crate::plan::approved() != Some(id.as_str()) {
        // The person sees all that the id covers and that goes to YouTube. A long description: its first 300
        // characters, its length and its hash; metadata.json of the folder has all of it.
        let count = description.chars().count();
        let seen: String = if count > 300 {
            format!("{}...", description.chars().take(300).collect::<String>())
        } else {
            description.clone()
        };
        let mut show = json!({
            "platform": "youtube", "channel": channel, "title": title, "privacy": privacy,
            "file": f.video, "bytes": size, "made_for_kids": kids, "ai_content": ai_content,
            "tags": tags, "category": category,
            "description": seen, "description_chars": description.chars().count(),
            "description_hash": blake3::hash(description.as_bytes()).to_hex()[..16].to_string(),
        });
        let mut what = format!(
            "This uploads {} ({size} bytes) to the YouTube channel \"{channel}\" with the title \"{title}\" and the privacy {privacy}.",
            f.video.display()
        );
        what += &format!(
            " Description ({count} characters{}): \"{seen}\". Tags: {}. Category {category}. Made for kids: {kids}. Altered or synthetic content: {ai_content}.",
            if count > 300 { ", the start is shown, all of it is in metadata.json of the folder" } else { "" },
            if tags.is_empty() { "none".to_string() } else { tags.join(", ") }
        );
        if let Some(v) = done["video_id"].as_str() {
            show["already_uploaded_as"] = json!(v);
            what += &format!(
                " This folder was uploaded before as video {v}: a yes makes a second video."
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
    let e = ends();
    let mut token = match access(&e, &creds) {
        Ok(t) => t,
        Err(env) => return *env,
    };
    let resource = json!({
        "snippet": {"title": title, "description": description, "tags": tags, "categoryId": category},
        "status": {"privacyStatus": privacy, "selfDeclaredMadeForKids": kids, "containsSyntheticMedia": ai_content},
    });
    // The session of this same upload, from a run that stopped.
    let mut session = crate::keys::resolve(K_SESSION)
        .map(|(v, _)| v)
        .filter(|u| notes.get("youtube_session_for") == Some(&id) && session_ok(&e, u));
    let resumed = session.is_some();
    // None: not known, so the server is asked before any byte goes.
    let mut offset: Option<u64> = None;
    let (mut faults, mut refreshed, mut sent_from) = (0u32, false, None::<u64>);
    let mut last = String::from("no progress");
    let mut waited = 0u32;
    // ponytail: a fixed number of rounds ends every loop of answers that make no progress.
    for _ in 0..24 {
        let step = match (&session, offset) {
            (None, _) => match start(&e, &token, &resource, size) {
                Ok(uri) => {
                    // Stored before the first byte: a stop after this never makes a second video.
                    if let Err(err) = crate::keys::set(K_SESSION, &uri)
                        .and_then(|_| note_set("youtube_session_for", &id))
                    {
                        return Envelope::fail(COMMAND, Exit::Error, format!("cannot store the upload session: {err}"), Some("Correct the config folder and run the same command again. Nothing was uploaded.".into()));
                    }
                    session = Some(uri);
                    offset = Some(0);
                    continue;
                }
                Err(step) => step,
            },
            (Some(uri), None) => status(uri, &token, size),
            (Some(uri), Some(at)) => {
                offset = None;
                sent_from.get_or_insert(at);
                send(uri, &token, &mut video, &hash, at, size)
            }
        };
        match step {
            Step::Done(v) => {
                let video_id = v["id"].as_str().unwrap_or_default().to_string();
                let url = format!("https://youtu.be/{video_id}");
                let entry = json!({
                    "publish_id": id, "video_id": video_id, "url": url, "privacy": privacy, "channel": channel,
                    "title": title, "ai_content": ai_content, "at": crate::run::now_utc(),
                });
                let saved = save_record(&f.dir, "youtube", entry);
                let mut env = Envelope::ok(COMMAND)
                    .with("platform", json!("youtube"))
                    .with("publish_id", json!(id))
                    .with("already_published", json!(false))
                    .with("video_id", json!(video_id))
                    .with("privacy", json!(privacy))
                    .with("channel", json!(channel))
                    .with("resumed", json!(resumed));
                env.artifacts.insert("url".into(), json!(url));
                env.artifacts
                    .insert("record".into(), json!(f.dir.join("publish.json")));
                env.metrics.insert("bytes".into(), json!(size));
                env.metrics
                    .insert("bytes_sent".into(), json!(size - sent_from.unwrap_or(size)));
                match saved {
                    // The session stays when the record is not written: the next run asks it and gets this video.
                    Ok(()) => {
                        let _ = crate::keys::set(K_SESSION, "");
                    }
                    Err(err) => env.findings.push(json!({"gate": "publish_record", "status": "warn",
                        "detail": format!("the video is uploaded, and publish.json was not written: {err}"),
                        "fix": "Run the same command again: it reads the finished upload and writes the file."})),
                }
                return env;
            }
            Step::At(n) => offset = Some(n.min(size)),
            Step::Gone => {
                faults += 1;
                last = "the upload session ended".into();
                let _ = crate::keys::set(K_SESSION, "");
                session = None;
            }
            Step::Denied if !refreshed => {
                refreshed = true;
                token = match access(&e, &creds) {
                    Ok(t) => t,
                    Err(env) => return *env,
                };
            }
            Step::Denied => {
                return Envelope::fail(
                    COMMAND,
                    Exit::NeedsInput,
                    "YouTube refused a new access token (HTTP 401)",
                    Some(format!("Run `{SETUP}` in your terminal, then the same command again.")),
                )
            }
            Step::Again(why) => {
                faults += 1;
                last = why;
            }
            Step::Changed => {
                // The bytes that went belong to the old file: that session must not be used again.
                let _ = crate::keys::set(K_SESSION, "");
                return Envelope::fail(
                    COMMAND,
                    Exit::Error,
                    "the video file changed after the approval. The upload stopped, and no video was made",
                    Some("Run the same command without --approve: it shows the upload as it is now, for a new yes of the person.".into()),
                );
            }
            Step::Fatal(why) => {
                return Envelope::fail(
                    COMMAND,
                    Exit::Error,
                    format!("YouTube refused the upload: {why}"),
                    Some("quotaExceeded or uploadLimitExceeded: wait a day. forbidden or youtubeSignupRequired: the account needs a channel, and the Google project needs the YouTube Data API v3. Else `scriptvox export` again and check the title.".into()),
                )
            }
        }
        if faults >= TRIES {
            break;
        }
        if faults > waited {
            waited = faults;
            std::thread::sleep(std::time::Duration::from_secs(1 << (faults - 1)));
        }
    }
    let mut env = Envelope::fail(
        COMMAND,
        Exit::ProviderRetryable,
        format!("the upload stopped: {last}. The session is kept"),
        Some(format!("Run the same command again with --approve {id}: it goes on from the byte that YouTube has, and makes no second video.")),
    );
    env.plan_id = Some(id.clone());
    env.with("publish_id", json!(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_asks_for_upload_only_with_a_challenge() {
        let a = auth_address("my-client.apps", "http://127.0.0.1:4000", "verifier", "st");
        assert!(
            a.starts_with("https://accounts.google.com/o/oauth2/v2/auth?client_id=my-client.apps&")
        );
        assert!(a.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A4000&"));
        assert!(a.contains("scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fyoutube.upload&"));
        assert!(a.contains("code_challenge_method=S256") && !a.contains("verifier"));
        assert_eq!(a.matches("scope=").count(), 1);
        // RFC 7636, appendix B.
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_ne!(random(32).unwrap(), random(32).unwrap());
        assert_eq!(random(48).unwrap().len(), 64);
    }

    #[test]
    fn the_loopback_port_takes_only_the_answer_with_our_state() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let t = std::thread::spawn(move || {
            let mut answers = vec![];
            for path in [
                "/favicon.ico",
                "/?state=other&code=bad",
                "/?state=mine&code=4%2Fgood",
            ] {
                let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
                    .unwrap();
                let mut out = String::new();
                let _ = s.read_to_string(&mut out);
                answers.push(out);
            }
            answers
        });
        let code = wait_code(&l, "mine", std::time::Duration::from_secs(10));
        assert_eq!(code.as_deref(), Ok("4/good"));
        let answers = t.join().unwrap();
        assert!(answers[0].starts_with("HTTP/1.1 404") && answers[1].starts_with("HTTP/1.1 404"));
        assert!(answers[2].contains("close this page"));
    }

    #[test]
    fn a_session_address_stays_on_the_upload_host() {
        let google = Ends {
            token: String::new(),
            upload: "https://www.googleapis.com/upload/youtube/v3/videos".into(),
        };
        assert!(session_ok(
            &google,
            "https://www.googleapis.com/upload/youtube/v3/videos?upload_id=x"
        ));
        assert!(session_ok(
            &google,
            "https://youtube.googleapis.com/upload?upload_id=x"
        ));
        assert!(!session_ok(
            &google,
            "https://evil.example/upload?upload_id=x"
        ));
        assert!(!session_ok(
            &google,
            "http://www.googleapis.com.evil.example/x"
        ));
        assert!(!session_ok(
            &google,
            "https://googleapis.com.evil.example/x"
        ));
        let test = Ends {
            token: String::new(),
            upload: "http://127.0.0.1:9/upload/youtube/v3/videos".into(),
        };
        assert!(session_ok(&test, "http://127.0.0.1:9/session/1"));
        assert!(!session_ok(&test, "http://127.0.0.1:10/session/1"));
    }
}
