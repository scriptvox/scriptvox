//! `scriptvox publish` (`docs/specs/publish.md`). No network and no real account: Google's token address, the
//! YouTube upload address and the Hugging Face Hub are hand-written servers on loopback. Every test ends with a
//! search for the fake secrets in all that the commands printed and wrote.
mod common;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const CLIENT_SECRET: &str = "FAKECLIENTSECRET123";
const REFRESH: &str = "1//FAKEREFRESHTOKEN456";
const ACCESS: &str = "ya29.FAKEACCESS";
const SESSION: &str = "SESSIONSECRETXYZ789";
const HF_TOKEN: &str = "hf_FAKEHUBTOKEN123456";

struct Head {
    method: String,
    path: String,
    /// The header lines, in lower case.
    lines: Vec<String>,
    length: usize,
}

impl Head {
    fn get(&self, name: &str) -> Option<&str> {
        self.lines
            .iter()
            .find_map(|l| l.strip_prefix(&format!("{name}: ")))
    }
}

/// (status, more header lines, body). None: the server drops the connection with no answer.
type Answer = Option<(u16, Vec<String>, String)>;

/// A server on loopback. The handler reads the body itself with `take(n)`, so it can stop in the middle.
fn serve<S: Send + 'static>(
    state: Arc<Mutex<S>>,
    handler: impl Fn(&str, &mut S, &Head, &mut dyn FnMut(usize) -> Vec<u8>) -> Answer + Send + 'static,
) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let own = base.clone();
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 16384];
            let head_end = loop {
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(i);
                }
                match s.read(&mut chunk) {
                    Ok(n) if n > 0 => buf.extend(&chunk[..n]),
                    _ => break None,
                }
            };
            let Some(head_end) = head_end else { continue };
            let text = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let mut lines = text.lines();
            let mut first = lines.next().unwrap_or("").split(' ');
            let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
            let lines: Vec<String> = lines
                .map(|l| {
                    let (k, v) = l.split_once(':').unwrap_or((l, ""));
                    format!("{}: {}", k.to_ascii_lowercase(), v.trim())
                })
                .collect();
            let mut head = Head {
                method: method.into(),
                path: path.into(),
                lines,
                length: 0,
            };
            head.length = head
                .get("content-length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let mut early = buf[head_end + 4..].to_vec();
            let mut left = head.length;
            let mut stream: TcpStream = s.try_clone().unwrap();
            let mut take = |n: usize| -> Vec<u8> {
                let want = n.min(left);
                let mut out: Vec<u8> = early.drain(..want.min(early.len())).collect();
                while out.len() < want {
                    let room = (want - out.len()).min(chunk.len());
                    match stream.read(&mut chunk[..room]) {
                        Ok(n) if n > 0 => out.extend(&chunk[..n]),
                        _ => break,
                    }
                }
                left -= out.len();
                out
            };
            let answer = handler(&own, &mut state.lock().unwrap(), &head, &mut take);
            match answer {
                Some((status, more, body)) => {
                    let more: String = more.iter().map(|h| format!("{h}\r\n")).collect();
                    let _ = s.write_all(
                        format!(
                            "HTTP/1.1 {status} X\r\n{more}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    );
                }
                None => {
                    let _ = s.shutdown(std::net::Shutdown::Both);
                }
            }
        }
    });
    base
}

/// A project folder with its own config folder and one export folder `exp`.
struct Home {
    dir: tempfile::TempDir,
    envs: Vec<(&'static str, String)>,
    /// All that the commands printed.
    seen: RefCell<String>,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let exp = dir.path().join("exp");
        std::fs::create_dir_all(&exp).unwrap();
        // 300 kB that are not all the same.
        let video: Vec<u8> = (0..300_000u32).map(|i| (i * 31 % 251) as u8).collect();
        std::fs::write(exp.join("video.mp4"), video).unwrap();
        std::fs::write(
            exp.join("captions.srt"),
            "1\n00:00:00,000 --> 00:00:01,000\nHello\n",
        )
        .unwrap();
        std::fs::write(
            exp.join("cover.jpg"),
            [0xFFu8, 0xD8, 0xFF, 0xE0, 0, 1, 2, 3],
        )
        .unwrap();
        std::fs::write(
            exp.join("metadata.json"),
            json!({
                "title": "Why the sky is blue", "description": "The air scatters blue light.",
                "tags": ["sky", "blue light"],
                "platforms": {"youtube": {"title": "Why the sky is blue", "description": "The air scatters blue light. #sky"}},
                "ai_parts": [{"kind": "tts", "name": "v"}, {"kind": "image.gen", "name": "row"}],
            })
            .to_string(),
        )
        .unwrap();
        Self {
            dir,
            envs: vec![],
            seen: RefCell::new(String::new()),
        }
    }
    fn path(&self) -> &Path {
        self.dir.path()
    }
    fn exp(&self) -> PathBuf {
        self.path().join("exp")
    }
    fn video(&self) -> Vec<u8> {
        std::fs::read(self.exp().join("video.mp4")).unwrap()
    }
    /// The account as `publish setup youtube` stores it.
    fn youtube_account(&self) {
        let cfg = self.path().join("cfg");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("keys.toml"),
            format!("youtube_client_id = \"FAKECLIENTID.apps.example\"\nyoutube_client_secret = \"{CLIENT_SECRET}\"\nyoutube_refresh_token = \"{REFRESH}\"\n"),
        )
        .unwrap();
        std::fs::write(
            cfg.join("publish.toml"),
            "youtube_channel = \"Test Channel\"\n",
        )
        .unwrap();
    }
    /// Exit code and the envelope.
    fn run(&self, args: &[&str]) -> (i32, Value) {
        let mut c = common::bin();
        c.current_dir(self.path())
            .env("SCRIPTVOX_CONFIG", self.path().join("cfg"))
            .env("SCRIPTVOX_DATA", self.path().join("data"))
            .env_remove("SCRIPTVOX_GENERATORS")
            .env_remove("SCRIPTVOX_YOUTUBE_BASE")
            .env_remove("SCRIPTVOX_HF_BASE")
            .env_remove("HF_TOKEN")
            .env_remove("SCRIPTVOX_AGENT");
        for (k, v) in &self.envs {
            c.env(k, v);
        }
        let o = c.args(args).arg("--json").write_stdin("").output().unwrap();
        let (out, err) = (
            String::from_utf8_lossy(&o.stdout).to_string(),
            String::from_utf8_lossy(&o.stderr).to_string(),
        );
        *self.seen.borrow_mut() += &format!("{out}\n{err}\n");
        let v = serde_json::from_str(&out).unwrap_or_else(|_| panic!("no envelope: {out} {err}"));
        (o.status.code().unwrap(), v)
    }
    fn record(&self) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.exp().join("publish.json")).unwrap())
            .unwrap()
    }
    /// No fake secret in anything printed, and none in the export folder.
    fn no_secret_anywhere(&self) {
        let mut all = self.seen.borrow().clone();
        for f in std::fs::read_dir(self.exp()).unwrap().flatten() {
            if f.path().extension().is_some_and(|x| x == "json") {
                all += &std::fs::read_to_string(f.path()).unwrap();
            }
        }
        assert!(all.len() > 100);
        for secret in [
            CLIENT_SECRET,
            REFRESH,
            "FAKEREFRESHTOKEN456",
            ACCESS,
            SESSION,
            HF_TOKEN,
            "FAKECLIENTID",
        ] {
            assert!(!all.contains(secret), "{secret} is in the output:\n{all}");
        }
    }
}

// ---------- YouTube ----------

#[derive(Default)]
struct Tube {
    received: Vec<u8>,
    tokens: u32,
    inits: u32,
    data_puts: u32,
    /// The JSON of the video resource.
    resource: String,
    /// Each data PUT takes half of what is left, then the connection drops.
    break_puts: bool,
    /// The next start of a session answers 401.
    deny_once: bool,
    /// A file that is written over when the session starts: after the approval, before the bytes go.
    tamper: Option<PathBuf>,
    requests: u32,
}

fn tube() -> (String, Arc<Mutex<Tube>>) {
    let state = Arc::new(Mutex::new(Tube::default()));
    let base = serve(state.clone(), |base, t: &mut Tube, h, take| {
        t.requests += 1;
        let bearer = h.get("authorization").unwrap_or("").to_string();
        match (h.method.as_str(), h.path.as_str()) {
            ("POST", "/token") => {
                let body = String::from_utf8_lossy(&take(h.length)).to_string();
                // The form is percent-encoded: "1//" is "1%2F%2F".
                assert!(
                    body.contains("grant_type=refresh_token")
                        && body.contains("FAKEREFRESHTOKEN456"),
                    "{body}"
                );
                assert!(
                    body.contains(&format!("client_secret={CLIENT_SECRET}")),
                    "{body}"
                );
                t.tokens += 1;
                Some((200, vec![], json!({"access_token": format!("{ACCESS}{}", t.tokens), "expires_in": 3599, "token_type": "Bearer"}).to_string()))
            }
            ("POST", p) if p.starts_with("/upload/youtube/v3/videos?") => {
                assert!(
                    p.contains("uploadType=resumable") && p.contains("part=snippet,status"),
                    "{p}"
                );
                let body = String::from_utf8_lossy(&take(h.length)).to_string();
                if t.deny_once {
                    t.deny_once = false;
                    return Some((
                        401,
                        vec![],
                        json!({"error": {"errors": [{"reason": "authError"}]}}).to_string(),
                    ));
                }
                assert_eq!(bearer, format!("Bearer {ACCESS}{}", t.tokens));
                assert_eq!(h.get("x-upload-content-length"), Some("300000"));
                t.inits += 1;
                if let Some(p) = &t.tamper {
                    std::fs::write(p, vec![7u8; 300_000]).unwrap();
                }
                t.received.clear();
                t.resource = body;
                Some((
                    200,
                    vec![format!("Location: {base}/session/{SESSION}")],
                    String::new(),
                ))
            }
            ("PUT", p) if p == format!("/session/{SESSION}") => {
                assert!(bearer.starts_with(&format!("Bearer {ACCESS}")), "{bearer}");
                let range = h.get("content-range").unwrap_or("").to_string();
                let done = || {
                    Some((
                        201,
                        vec![],
                        json!({"id": "vidFAKE0001", "status": {"uploadStatus": "uploaded"}})
                            .to_string(),
                    ))
                };
                if range.starts_with("bytes */") {
                    assert_eq!(h.length, 0);
                    if t.received.len() == 300_000 {
                        return done();
                    }
                    let more = if t.received.is_empty() {
                        vec![]
                    } else {
                        vec![format!("Range: bytes=0-{}", t.received.len() - 1)]
                    };
                    return Some((308, more, String::new()));
                }
                t.data_puts += 1;
                let from: usize = range
                    .strip_prefix("bytes ")
                    .and_then(|r| r.split('-').next())
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                if from != t.received.len() || from + h.length != 300_000 {
                    return Some((
                        400,
                        vec![],
                        json!({"error": {"errors": [{"reason": "badRange"}]}}).to_string(),
                    ));
                }
                if t.break_puts {
                    let part = take(h.length / 2);
                    t.received.extend(part);
                    return None;
                }
                let all = take(h.length);
                t.received.extend(all);
                done()
            }
            _ => Some((404, vec![], "{}".into())),
        }
    });
    (base, state)
}

fn tube_home() -> (Home, Arc<Mutex<Tube>>) {
    let (base, state) = tube();
    let mut h = Home::new();
    h.youtube_account();
    h.envs.push(("SCRIPTVOX_YOUTUBE_BASE", base));
    (h, state)
}

/// The publish id of the stop, with the checks that every stop must pass.
fn stop_id(code: i32, v: &Value) -> String {
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["status"], "approval_needed", "{v}");
    assert_eq!(v["error"]["code"], "SVX_APPROVAL_NEEDED", "{v}");
    assert_eq!(v["needs_input"].as_array().unwrap().len(), 1, "{v}");
    let item = &v["needs_input"][0];
    assert_eq!(item["kind"], "approval", "{v}");
    let action = item["human_action"].as_str().unwrap();
    assert!(
        action.contains("An agent must not approve an upload by its own decision"),
        "{action}"
    );
    assert!(
        v["next"][0].as_str().unwrap().starts_with("Ask the person"),
        "{v}"
    );
    let id = v["publish_id"].as_str().unwrap().to_string();
    assert!(id.starts_with("pub_"));
    assert_eq!(v["plan_id"], id.as_str());
    assert_eq!(item["publish_id"], id.as_str());
    id
}

#[test]
fn youtube_without_approval_is_exit_4_and_nothing_is_sent() {
    let (h, t) = tube_home();
    let (code, v) = h.run(&["publish", "youtube", "exp"]);
    let id = stop_id(code, &v);
    let up = &v["needs_input"][0]["upload"];
    assert_eq!(up["channel"], "Test Channel", "{v}");
    assert_eq!(up["title"], "Why the sky is blue");
    assert_eq!(up["privacy"], "private");
    assert_eq!(up["bytes"], 300_000);
    assert_eq!(up["ai_content"], true);
    assert_eq!(up["made_for_kids"], false);
    assert!(up["file"].as_str().unwrap().ends_with("video.mp4"));
    let action = v["needs_input"][0]["human_action"].as_str().unwrap();
    assert!(
        action.contains("Test Channel")
            && action.contains("Why the sky is blue")
            && action.contains("privacy private"),
        "{action}"
    );

    // An approval of something else is no approval.
    let (code, v) = h.run(&[
        "publish",
        "youtube",
        "exp",
        "--approve",
        "pub_0000000000000000",
    ]);
    assert_eq!(stop_id(code, &v), id);
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("another upload"),
        "{v}"
    );

    // Public is another upload: the yes for private does not cover it.
    let (code, v) = h.run(&[
        "publish",
        "youtube",
        "exp",
        "--privacy",
        "public",
        "--approve",
        &id,
    ]);
    let public_id = stop_id(code, &v);
    assert_ne!(public_id, id);
    assert_eq!(v["upload"]["privacy"], "public");

    let (code, v) = h.run(&["publish", "youtube", "exp", "--privacy", "friends"]);
    assert_eq!(code, 2, "{v}");
    let (code, v) = h.run(&["publish", "youtube", "nothing-here"]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("scriptvox export"),
        "{v}"
    );

    // The person sees all that the id covers: another description is another id, and the item shows it.
    assert_eq!(up["description"], "The air scatters blue light. #sky");
    assert_eq!(up["tags"], json!(["sky", "blue light"]));
    assert_eq!(up["category"], "22");
    assert!(
        action.contains("The air scatters blue light. #sky") && action.contains("sky, blue light"),
        "{action}"
    );
    let meta_file = h.exp().join("metadata.json");
    let mut meta: Value =
        serde_json::from_str(&std::fs::read_to_string(&meta_file).unwrap()).unwrap();
    meta["platforms"]["youtube"]["description"] = json!("Buy this now at a link.");
    std::fs::write(&meta_file, meta.to_string()).unwrap();
    let (code, v) = h.run(&["publish", "youtube", "exp", "--approve", &id]);
    let changed = stop_id(code, &v);
    assert_ne!(changed, id);
    assert_eq!(
        v["needs_input"][0]["upload"]["description"],
        "Buy this now at a link."
    );
    assert!(
        v["needs_input"][0]["human_action"]
            .as_str()
            .unwrap()
            .contains("Buy this now at a link."),
        "{v}"
    );
    // A long one: the start, the length and a hash; a change after the part that is shown changes the hash and the id.
    let long = "word ".repeat(200);
    let mut ids = vec![];
    for end in ["A", "B"] {
        meta["platforms"]["youtube"]["description"] = json!(format!("{long}{end}"));
        std::fs::write(&meta_file, meta.to_string()).unwrap();
        let (code, v) = h.run(&["publish", "youtube", "exp"]);
        let up = v["needs_input"][0]["upload"].clone();
        assert_eq!(up["description_chars"], 1001, "{v}");
        assert_eq!(up["description"].as_str().unwrap().chars().count(), 303);
        ids.push((stop_id(code, &v), up["description_hash"].clone()));
    }
    assert!(ids[0].0 != ids[1].0 && ids[0].1 != ids[1].1, "{ids:?}");

    assert_eq!(t.lock().unwrap().requests, 0);
    assert!(!h.exp().join("publish.json").exists());
    h.no_secret_anywhere();
}

#[test]
fn youtube_uploads_one_time_and_the_second_call_sends_nothing() {
    let (h, t) = tube_home();
    let (code, v) = h.run(&["publish", "youtube", "exp"]);
    let id = stop_id(code, &v);
    let (code, v) = h.run(&["publish", "youtube", "exp", "--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["video_id"], "vidFAKE0001");
    assert_eq!(v["privacy"], "private");
    assert_eq!(v["already_published"], false);
    assert_eq!(v["artifacts"]["url"], "https://youtu.be/vidFAKE0001");
    assert_eq!(v["metrics"]["bytes_sent"], 300_000);
    {
        let t = t.lock().unwrap();
        assert_eq!((t.tokens, t.inits, t.data_puts), (1, 1, 1));
        assert_eq!(t.received, h.video());
        let r: Value = serde_json::from_str(&t.resource).unwrap();
        assert_eq!(
            r["status"],
            json!({"privacyStatus": "private", "selfDeclaredMadeForKids": false, "containsSyntheticMedia": true})
        );
        assert_eq!(r["snippet"]["title"], "Why the sky is blue");
        assert_eq!(
            r["snippet"]["description"],
            "The air scatters blue light. #sky"
        );
        assert_eq!(r["snippet"]["tags"], json!(["sky", "blue light"]));
        assert_eq!(r["snippet"]["categoryId"], "22");
    }
    let rec = h.record();
    assert_eq!(rec["schema"], "scriptvox.publish/v1");
    assert_eq!(rec["youtube"]["video_id"], "vidFAKE0001");
    assert_eq!(rec["youtube"]["publish_id"], id.as_str());

    // The same upload again, with and without the approval: no request at all.
    let before = t.lock().unwrap().requests;
    for args in [
        vec!["publish", "youtube", "exp", "--approve", &id],
        vec!["publish", "youtube", "exp"],
    ] {
        let (code, v) = h.run(&args);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["already_published"], true);
        assert_eq!(v["video_id"], "vidFAKE0001");
    }
    assert_eq!(t.lock().unwrap().requests, before);

    // Another privacy is another upload, and the stop says that this folder is on YouTube already.
    let (code, v) = h.run(&["publish", "youtube", "exp", "--privacy", "unlisted"]);
    stop_id(code, &v);
    assert_eq!(v["upload"]["already_uploaded_as"], "vidFAKE0001");
    h.no_secret_anywhere();
}

#[test]
fn youtube_public_and_the_other_declarations_go_only_with_their_flags() {
    let (h, t) = tube_home();
    let args = [
        "publish",
        "youtube",
        "exp",
        "--privacy",
        "public",
        "--made-for-kids",
        "true",
        "--ai-content",
        "false",
        "--category",
        "27",
    ];
    let (code, v) = h.run(&args);
    let id = stop_id(code, &v);
    let mut approved = args.to_vec();
    approved.extend(["--approve", &id]);
    let (code, v) = h.run(&approved);
    assert_eq!(code, 0, "{v}");
    let r: Value = serde_json::from_str(&t.lock().unwrap().resource).unwrap();
    assert_eq!(
        r["status"],
        json!({"privacyStatus": "public", "selfDeclaredMadeForKids": true, "containsSyntheticMedia": false})
    );
    assert_eq!(r["snippet"]["categoryId"], "27");
    h.no_secret_anywhere();
}

#[test]
fn youtube_gets_a_new_token_when_the_first_is_refused() {
    let (h, t) = tube_home();
    t.lock().unwrap().deny_once = true;
    let (code, v) = h.run(&["publish", "youtube", "exp"]);
    let id = stop_id(code, &v);
    let (code, v) = h.run(&["publish", "youtube", "exp", "--approve", &id]);
    assert_eq!(code, 0, "{v}");
    let t = t.lock().unwrap();
    assert_eq!((t.tokens, t.inits, t.data_puts), (2, 1, 1));
    assert_eq!(t.received, h.video());
    drop(t);
    h.no_secret_anywhere();
}

#[test]
fn youtube_goes_on_after_a_broken_upload_and_makes_one_video() {
    let (h, t) = tube_home();
    t.lock().unwrap().break_puts = true;
    let (code, v) = h.run(&["publish", "youtube", "exp"]);
    let id = stop_id(code, &v);
    let (code, v) = h.run(&["publish", "youtube", "exp", "--approve", &id]);
    assert_eq!(code, 5, "{v}");
    assert_eq!(v["error"]["retryable"], true);
    assert!(v["error"]["fix"].as_str().unwrap().contains(&id), "{v}");
    assert_eq!(
        v["needs_input"],
        json!([]),
        "a broken upload is not a question of money: {v}"
    );
    let got = {
        let t = t.lock().unwrap();
        // Each try went on from the byte that the server had: one session, and more bytes each time.
        assert_eq!(t.inits, 1);
        assert_eq!(t.data_puts, 3);
        assert!(
            t.received.len() > 150_000 && t.received.len() < 300_000,
            "{}",
            t.received.len()
        );
        t.received.len()
    };
    assert!(!h.exp().join("publish.json").exists());

    // The next call of the same command: the line is good again.
    t.lock().unwrap().break_puts = false;
    let (code, v) = h.run(&["publish", "youtube", "exp", "--approve", &id]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["resumed"], true);
    assert_eq!(v["video_id"], "vidFAKE0001");
    assert_eq!(v["metrics"]["bytes_sent"], 300_000 - got);
    let t = t.lock().unwrap();
    assert_eq!(t.inits, 1);
    assert_eq!(t.data_puts, 4);
    assert_eq!(t.received, h.video());
    drop(t);
    h.no_secret_anywhere();
}

#[test]
fn youtube_setup_is_for_the_person_only() {
    let h = Home::new();
    // No terminal: the test runs the command with a pipe on stdin.
    let (code, v) = h.run(&["publish", "setup", "youtube"]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["status"], "needs_input");
    assert_eq!(v["needs_input"][0]["kind"], "secret");
    assert_eq!(v["next"], json!(["scriptvox publish setup youtube"]));
    assert!(!h.path().join("cfg/keys.toml").exists());

    // No account: exit 10 with the same one command, before any stop for approval.
    let (code, v) = h.run(&["publish", "youtube", "exp"]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["next"], json!(["scriptvox publish setup youtube"]));
    let (code, v) = h.run(&["publish", "setup", "tiktok"]);
    assert_eq!(code, 2, "{v}");
}

// ---------- Hugging Face ----------

#[derive(Default)]
struct Hub {
    /// Repo name and whether it is private.
    repos: Vec<(String, bool)>,
    creates: Vec<Value>,
    /// The large files that arrived: (oid, bytes).
    lfs: Vec<(String, Vec<u8>)>,
    commits: Vec<String>,
    /// The next PUT of a large file drops the connection.
    break_put: bool,
    /// A file that is written over when the Hub is asked how to upload: after the approval, before the bytes go.
    tamper: Option<PathBuf>,
    requests: u32,
}

fn hub() -> (String, Arc<Mutex<Hub>>) {
    let state = Arc::new(Mutex::new(Hub::default()));
    let base = serve(state.clone(), |base, s: &mut Hub, h, take| {
        s.requests += 1;
        let auth = h.get("authorization").map(String::from);
        let path = h.path.clone();
        if let Some(oid) = path.strip_prefix("/lfs-upload/") {
            // The signed address of the file store: the token must not come here.
            assert_eq!(auth, None, "the token went to the upload address");
            assert_eq!(h.get("x-sig"), Some("signed-for-this-file"));
            if s.break_put {
                s.break_put = false;
                take(h.length / 2);
                return None;
            }
            let bytes = take(h.length);
            s.lfs.push((oid.to_string(), bytes));
            return Some((200, vec![], String::new()));
        }
        assert_eq!(
            auth.as_deref(),
            Some(&*format!("Bearer {HF_TOKEN}")),
            "{path}"
        );
        let body = String::from_utf8_lossy(&take(h.length)).to_string();
        let ok = |v: Value| Some((200, vec![], v.to_string()));
        let (kind, rest) = match path.strip_prefix("/api/datasets/") {
            Some(r) => ("datasets", r.to_string()),
            None => (
                "spaces",
                path.strip_prefix("/api/spaces/").unwrap_or("").to_string(),
            ),
        };
        match (h.method.as_str(), path.as_str()) {
            ("POST", "/api/repos/create") => {
                let v: Value = serde_json::from_str(&body).unwrap();
                let name = format!(
                    "{}/{}",
                    v["organization"].as_str().unwrap(),
                    v["name"].as_str().unwrap()
                );
                if s.repos.iter().any(|(n, _)| *n == name) {
                    return Some((
                        409,
                        vec![],
                        json!({"error": "You already created this repo", "url": "x"}).to_string(),
                    ));
                }
                s.repos.push((name.clone(), v["private"] == true));
                s.creates.push(v);
                ok(
                    json!({"url": format!("{base}/{name}"), "name": name, "id": "0123456789abcdef01234567"}),
                )
            }
            ("GET", _) if !rest.is_empty() => match s.repos.iter().find(|(n, _)| *n == rest) {
                Some((_, private)) => ok(json!({"id": rest, "private": private})),
                None => Some((404, vec![], "{}".into())),
            },
            ("POST", _) if rest.ends_with("/preupload/main") => {
                if let Some(p) = &s.tamper {
                    std::fs::write(p, "other words").unwrap();
                }
                let v: Value = serde_json::from_str(&body).unwrap();
                let files: Vec<Value> = v["files"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| {
                        let p = f["path"].as_str().unwrap();
                        assert!(f["sample"].is_string() && f["size"].is_number());
                        json!({"path": p, "uploadMode": if p.ends_with(".mp4") || p.ends_with(".jpg") { "lfs" } else { "regular" }, "shouldIgnore": false})
                    })
                    .collect();
                ok(json!({"files": files, "commitOid": "0000"}))
            }
            ("POST", p) if p.ends_with(".git/info/lfs/objects/batch") => {
                assert!(
                    p.starts_with(&format!("/{kind}/"))
                        || p.starts_with("/datasets/")
                        || p.starts_with("/spaces/"),
                    "{p}"
                );
                let v: Value = serde_json::from_str(&body).unwrap();
                assert_eq!(v["operation"], "upload");
                let o = &v["objects"][0];
                let oid = o["oid"].as_str().unwrap();
                let mut object = json!({"oid": oid, "size": o["size"]});
                if !s.lfs.iter().any(|(have, _)| have == oid) {
                    object["actions"] = json!({
                        "upload": {"href": format!("{base}/lfs-upload/{oid}"), "header": {"X-Sig": "signed-for-this-file"}},
                        "verify": {"href": format!("{base}/lfs-verify"), "header": {}},
                    });
                }
                ok(json!({"transfer": "basic", "objects": [object]}))
            }
            ("POST", "/lfs-verify") => ok(json!({})),
            ("POST", _) if rest.ends_with("/commit/main") => {
                assert_eq!(h.get("content-type"), Some("application/x-ndjson"));
                s.commits.push(body);
                ok(
                    json!({"success": true, "commitOid": "c0ffee1234", "commitUrl": "x", "hookOutput": ""}),
                )
            }
            _ => Some((404, vec![], "{}".into())),
        }
    });
    (base, state)
}

fn hub_home() -> (Home, Arc<Mutex<Hub>>) {
    let (base, state) = hub();
    let mut h = Home::new();
    h.envs.push(("SCRIPTVOX_HF_BASE", base));
    h.envs.push(("HF_TOKEN", HF_TOKEN.into()));
    (h, state)
}

/// The lines of a commit: (key, path).
fn commit_lines(body: &str) -> Vec<(String, String)> {
    body.lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .map(|v| {
            (
                v["key"].as_str().unwrap().to_string(),
                v["value"]["path"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

#[test]
fn hf_without_approval_is_exit_4_and_shows_the_repo_and_each_file() {
    let (h, s) = hub_home();
    let args = ["publish", "hf", "exp", "--repo", "me/sky-video"];
    let (code, v) = h.run(&args);
    let id = stop_id(code, &v);
    let up = &v["needs_input"][0]["upload"];
    assert_eq!(up["repo"], "me/sky-video", "{v}");
    assert_eq!(up["type"], "dataset");
    assert_eq!(up["visibility"], "private");
    let files: Vec<(&str, u64)> = up["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["path"].as_str().unwrap(), f["bytes"].as_u64().unwrap()))
        .collect();
    assert_eq!(
        files.iter().map(|f| f.0).collect::<Vec<_>>(),
        ["captions.srt", "cover.jpg", "metadata.json", "video.mp4"]
    );
    assert_eq!(files[3].1, 300_000);
    let action = v["needs_input"][0]["human_action"].as_str().unwrap();
    for word in [
        "me/sky-video",
        "private",
        "video.mp4 (300000 bytes)",
        "cover.jpg (8 bytes)",
    ] {
        assert!(action.contains(word), "{word}: {action}");
    }
    // Public, another repo, another type and another file are each another upload.
    let (code, v) = h.run(&[&args[..], &["--public", "--approve", &id]].concat());
    assert_ne!(stop_id(code, &v), id);
    assert_eq!(v["upload"]["visibility"], "public");
    assert!(
        v["needs_input"][0]["human_action"]
            .as_str()
            .unwrap()
            .contains("everyone can see"),
        "{v}"
    );
    let (code, v) = h.run(&[
        "publish",
        "hf",
        "exp",
        "--repo",
        "me/other",
        "--approve",
        &id,
    ]);
    assert_ne!(stop_id(code, &v), id);
    std::fs::write(h.exp().join("captions.srt"), "changed").unwrap();
    let (code, v) = h.run(&[&args[..], &["--approve", &id]].concat());
    assert_ne!(stop_id(code, &v), id);

    for bad in [
        vec!["publish", "hf", "exp", "--repo", "no-slash"],
        vec!["publish", "hf", "exp", "--repo", "a/b", "--type", "model"],
        vec!["publish", "hf", "exp", "--repo", "../x/y"],
    ] {
        let (code, v) = h.run(&bad);
        assert_eq!(code, 2, "{v}");
    }
    assert_eq!(s.lock().unwrap().requests, 0);
    h.no_secret_anywhere();

    // No token: exit 10 with the one command for the person.
    let mut none = Home::new();
    none.envs
        .push(("SCRIPTVOX_HF_BASE", "http://127.0.0.1:9".into()));
    let (code, v) = none.run(&args);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["next"], json!(["scriptvox keys set hf"]));
}

#[test]
fn hf_uploads_a_private_dataset_one_time() {
    let (h, s) = hub_home();
    // A record of another platform stays, and it is not uploaded.
    std::fs::write(
        h.exp().join("publish.json"),
        json!({"schema": "scriptvox.publish/v1", "youtube": {"video_id": "keepme"}}).to_string(),
    )
    .unwrap();
    let args = ["publish", "hf", "exp", "--repo", "me/sky-video"];
    let (code, v) = h.run(&args);
    let id = stop_id(code, &v);
    let approved = [&args[..], &["--approve", &id]].concat();
    let (code, v) = h.run(&approved);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["private"], true);
    assert_eq!(v["repo_created"], true);
    assert_eq!(v["commit"], "c0ffee1234");
    assert_eq!(
        v["artifacts"]["url"],
        "https://huggingface.co/datasets/me/sky-video"
    );
    {
        let s = s.lock().unwrap();
        assert_eq!(
            s.creates,
            vec![
                json!({"type": "dataset", "name": "sky-video", "organization": "me", "private": true})
            ]
        );
        // The two large files went to the file store, whole; the oid is the SHA-256 of the bytes.
        assert_eq!(s.lfs.len(), 2);
        let video = s.lfs.iter().find(|(_, b)| b.len() == 300_000).unwrap();
        assert_eq!(video.1, h.video());
        assert_eq!(video.0.len(), 64);
        assert_eq!(s.commits.len(), 1);
        assert_eq!(
            commit_lines(&s.commits[0]),
            [
                ("header", ""),
                ("file", "captions.srt"),
                ("lfsFile", "cover.jpg"),
                ("file", "metadata.json"),
                ("lfsFile", "video.mp4")
            ]
            .map(|(k, p)| (k.to_string(), p.to_string()))
        );
        assert!(s.commits[0].contains(&format!("\"oid\":\"{}\"", video.0)));
    }
    let rec = h.record();
    assert_eq!(rec["hf"]["commit"], "c0ffee1234");
    assert_eq!(rec["hf"]["private"], true);
    assert_eq!(rec["youtube"]["video_id"], "keepme");

    let before = s.lock().unwrap().requests;
    let (code, v) = h.run(&approved);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["already_published"], true);
    assert_eq!(s.lock().unwrap().requests, before);
    h.no_secret_anywhere();
}

#[test]
fn hf_refuses_a_public_repo_without_the_flag() {
    let (h, s) = hub_home();
    s.lock().unwrap().repos.push(("me/open".into(), false));
    let args = ["publish", "hf", "exp", "--repo", "me/open"];
    let (code, v) = h.run(&args);
    let id = stop_id(code, &v);
    let (code, v) = h.run(&[&args[..], &["--approve", &id]].concat());
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("is public"),
        "{v}"
    );
    assert!(s.lock().unwrap().lfs.is_empty() && s.lock().unwrap().commits.is_empty());
    assert!(!h.exp().join("publish.json").exists());

    // With the flag, and the yes for the public upload.
    let public = [&args[..], &["--public"]].concat();
    let (code, v) = h.run(&public);
    let public_id = stop_id(code, &v);
    let (code, v) = h.run(&[&public[..], &["--approve", &public_id]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["private"], false);
    assert_eq!(v["repo_created"], false);
    assert_eq!(s.lock().unwrap().commits.len(), 1);
    h.no_secret_anywhere();
}

#[test]
fn hf_goes_on_after_a_broken_file_and_a_space_gets_a_page() {
    let (h, s) = hub_home();
    s.lock().unwrap().break_put = true;
    let args = [
        "publish", "hf", "exp", "--repo", "me/sky", "--type", "space",
    ];
    let (code, v) = h.run(&args);
    let id = stop_id(code, &v);
    assert!(
        v["upload"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == "index.html"),
        "{v}"
    );
    let approved = [&args[..], &["--approve", &id]].concat();
    let (code, v) = h.run(&approved);
    assert_eq!(code, 5, "{v}");
    assert_eq!(v["error"]["retryable"], true);
    assert_eq!(v["needs_input"], json!([]), "{v}");
    assert!(s.lock().unwrap().commits.is_empty());
    assert!(!h.exp().join("publish.json").exists());

    let (code, v) = h.run(&approved);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["repo_created"], false);
    let s = s.lock().unwrap();
    assert_eq!(s.creates[0]["sdk"], "static");
    assert_eq!(s.creates[0]["private"], true);
    assert_eq!(s.commits.len(), 1);
    assert_eq!(s.lfs.len(), 2, "each large file is stored one time");
    let lines = commit_lines(&s.commits[0]);
    assert!(
        lines.contains(&("file".to_string(), "index.html".to_string())),
        "{lines:?}"
    );
    drop(s);
    h.no_secret_anywhere();
}

// ---------- What the person approved is what goes ----------

#[test]
fn plan_and_local_only_send_nothing_also_with_the_approval() {
    let (h, t) = tube_home();
    let (code, v) = h.run(&["publish", "youtube", "exp"]);
    let id = stop_id(code, &v);
    let (code, v) = h.run(&["publish", "youtube", "exp", "--approve", &id, "--plan"]);
    assert_eq!(stop_id(code, &v), id);
    assert!(
        !v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("another upload"),
        "{v}"
    );
    let (code, v) = h.run(&[
        "publish",
        "youtube",
        "exp",
        "--approve",
        &id,
        "--local-only",
    ]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--local-only"),
        "{v}"
    );
    assert_eq!(t.lock().unwrap().requests, 0);

    let (h, s) = hub_home();
    let args = ["publish", "hf", "exp", "--repo", "me/sky-video"];
    let (code, v) = h.run(&args);
    let id = stop_id(code, &v);
    let (code, v) = h.run(&[&args[..], &["--approve", &id, "--plan"]].concat());
    assert_eq!(stop_id(code, &v), id);
    let (code, v) = h.run(&[&args[..], &["--approve", &id, "--local-only"]].concat());
    assert_eq!(code, 2, "{v}");
    assert_eq!(s.lock().unwrap().requests, 0);
}

#[cfg(unix)]
#[test]
fn a_link_in_the_folder_is_refused_and_nothing_is_sent() {
    // A file outside the folder, such as a key file.
    let (h, s) = hub_home();
    let outside = h.path().join("outside-keys.toml");
    std::fs::write(&outside, "key = \"OUTSIDESECRETVALUE\"\n").unwrap();
    std::os::unix::fs::symlink(&outside, h.exp().join("notes.toml")).unwrap();
    let args = ["publish", "hf", "exp", "--repo", "me/sky-video"];
    let (code, v) = h.run(&args);
    assert_eq!(code, 2, "{v}");
    let message = v["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("notes.toml is a link") && message.contains("Nothing was sent"),
        "{v}"
    );
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("copy of the file"),
        "{v}"
    );
    assert!(
        v["needs_input"].as_array().unwrap().is_empty(),
        "no approval item for a folder with a link: {v}"
    );
    // A linked folder is refused in the same way.
    std::fs::remove_file(h.exp().join("notes.toml")).unwrap();
    std::fs::create_dir_all(h.path().join("elsewhere")).unwrap();
    std::fs::write(h.path().join("elsewhere/x.txt"), "OUTSIDESECRETVALUE").unwrap();
    std::os::unix::fs::symlink(h.path().join("elsewhere"), h.exp().join("more")).unwrap();
    let (code, v) = h.run(&args);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("more is a link"),
        "{v}"
    );
    assert_eq!(s.lock().unwrap().requests, 0);
    assert!(!h.seen.borrow().contains("OUTSIDESECRETVALUE"));

    // The video of a YouTube upload: a link in its place is refused before the stop for approval.
    let (h, t) = tube_home();
    let real = h.path().join("other.mp4");
    std::fs::rename(h.exp().join("video.mp4"), &real).unwrap();
    std::os::unix::fs::symlink(&real, h.exp().join("video.mp4")).unwrap();
    let (code, v) = h.run(&["publish", "youtube", "exp"]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("video.mp4")
            && v["error"]["message"]
                .as_str()
                .unwrap()
                .contains("is a link"),
        "{v}"
    );
    assert_eq!(t.lock().unwrap().requests, 0);
}

#[test]
fn a_file_changed_after_the_approval_stops_the_upload() {
    // YouTube: the video is written over after the yes and before its bytes go.
    let (h, t) = tube_home();
    let (code, v) = h.run(&["publish", "youtube", "exp"]);
    let id = stop_id(code, &v);
    t.lock().unwrap().tamper = Some(h.exp().join("video.mp4"));
    let (code, v) = h.run(&["publish", "youtube", "exp", "--approve", &id]);
    assert_eq!(code, 1, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("changed after the approval"),
        "{v}"
    );
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("without --approve"),
        "{v}"
    );
    {
        let t = t.lock().unwrap();
        assert_eq!(
            (t.inits, t.data_puts),
            (1, 0),
            "no byte of the changed file was sent"
        );
        assert!(t.received.is_empty());
    }
    assert!(!h.exp().join("publish.json").exists());
    // The old yes does not cover the new file, and the old session is not used again.
    t.lock().unwrap().tamper = None;
    let (code, v) = h.run(&["publish", "youtube", "exp", "--approve", &id]);
    assert_ne!(stop_id(code, &v), id);
    assert_eq!(t.lock().unwrap().data_puts, 0);
    h.no_secret_anywhere();

    // Hugging Face: a small file is written over after the yes.
    let (h, s) = hub_home();
    let args = ["publish", "hf", "exp", "--repo", "me/sky-video"];
    let (code, v) = h.run(&args);
    let id = stop_id(code, &v);
    s.lock().unwrap().tamper = Some(h.exp().join("captions.srt"));
    let (code, v) = h.run(&[&args[..], &["--approve", &id]].concat());
    assert_eq!(code, 1, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("captions.srt changed after the approval"),
        "{v}"
    );
    {
        let s = s.lock().unwrap();
        // The repo and the question how to upload: nothing after the change was seen.
        assert_eq!(s.requests, 2);
        assert!(s.lfs.is_empty() && s.commits.is_empty());
    }
    assert!(!h.exp().join("publish.json").exists());

    // The same for a large file: the video is hashed again through the handle that sends it.
    let (h, s) = hub_home();
    let (code, v) = h.run(&args);
    let id = stop_id(code, &v);
    s.lock().unwrap().tamper = Some(h.exp().join("video.mp4"));
    let (code, v) = h.run(&[&args[..], &["--approve", &id]].concat());
    assert_eq!(code, 1, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("video.mp4 changed after the approval"),
        "{v}"
    );
    let s = s.lock().unwrap();
    assert!(s.commits.is_empty());
    assert!(
        s.lfs.iter().all(|(_, b)| b.len() == 8),
        "only the cover, which is as approved, went to the file store"
    );
    drop(s);
    h.no_secret_anywhere();
}
