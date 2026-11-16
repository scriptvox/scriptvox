//! `scriptvox fetch` and `setup fetch` (`docs/specs/fetch.md`, section 9). Every test talks to a hand-written server
//! on loopback: no real site. Media is made with FFmpeg; a test that needs it is skipped when FFmpeg is not there.
//! The session id and the query of the media address are made-up values.
mod common;

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The made-up query of the fake media address: no output may hold it.
const QUERY: &str = "sig=MADEUPQUERY123";

#[derive(Clone, Debug)]
struct Req {
    method: String,
    path: String,
    /// The header lines, in lower case.
    head: String,
    body: Vec<u8>,
}

impl Req {
    fn header(&self, name: &str) -> Option<String> {
        self.head
            .lines()
            .find_map(|l| l.strip_prefix(&format!("{name}: ")).map(str::to_string))
    }
    /// The first and the last byte of the `Range` header. An open end is `usize::MAX`.
    fn range(&self) -> Option<(usize, usize)> {
        let r = self.header("range")?;
        let (a, b) = r.strip_prefix("bytes=")?.split_once('-')?;
        Some((a.parse().ok()?, b.parse().unwrap_or(usize::MAX)))
    }
}

struct Resp {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    /// Send only this many bytes of the body, then close: the server "breaks".
    cut: Option<usize>,
}

fn resp(status: u16, body: &[u8]) -> Resp {
    Resp {
        status,
        headers: vec![],
        body: body.to_vec(),
        cut: None,
    }
}

/// `data` as a file: the asked range with 206, or all of it with 200.
fn file(req: &Req, data: &[u8]) -> Resp {
    match req.range() {
        Some((a, _)) if a >= data.len() => resp(416, b""),
        Some((a, b)) => {
            let b = b.min(data.len() - 1);
            let mut r = resp(206, &data[a..=b]);
            r.headers.push((
                "Content-Range".into(),
                format!("bytes {a}-{b}/{}", data.len()),
            ));
            r
        }
        None => resp(200, data),
    }
}

type Log = Arc<Mutex<Vec<Req>>>;

/// A server on loopback. `handler` gets each request and how many came before it.
fn serve(handler: impl Fn(&Req, usize) -> Resp + Send + 'static) -> (String, Log) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let seen = log.clone();
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            // All of the head, then as many body bytes as Content-Length says.
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
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
            let head_text = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let mut lines = head_text.lines();
            let mut first = lines.next().unwrap_or("").split(' ');
            let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
            let head = lines
                .map(str::to_ascii_lowercase)
                .collect::<Vec<_>>()
                .join("\n");
            let mut req = Req {
                method: method.into(),
                path: path.into(),
                head,
                body: buf[head_end + 4..].to_vec(),
            };
            let len: usize = req
                .header("content-length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            while req.body.len() < len {
                match s.read(&mut chunk) {
                    Ok(n) if n > 0 => req.body.extend(&chunk[..n]),
                    _ => break,
                }
            }
            let before = {
                let mut l = seen.lock().unwrap();
                l.push(req.clone());
                l.len() - 1
            };
            let r = handler(&req, before);
            let mut out = format!(
                "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
                r.status,
                r.body.len()
            );
            for (k, v) in &r.headers {
                out.push_str(&format!("{k}: {v}\r\n"));
            }
            out.push_str("\r\n");
            let _ = s.write_all(out.as_bytes());
            let _ = s.write_all(&r.body[..r.cut.unwrap_or(r.body.len())]);
        }
    });
    (base, log)
}

fn ff() -> Option<String> {
    std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| Path::new(p).exists())
}

/// A project folder with its own config and data folder.
struct Home(tempfile::TempDir);

impl Home {
    fn new() -> Self {
        let h = Self(tempfile::tempdir().unwrap());
        // A lock file with no pin: no test may download the real helper. A test that wants a helper writes a fake.
        std::fs::write(h.path().join("no-pins.lock"), "tool = []\n").unwrap();
        h
    }
    fn path(&self) -> &Path {
        self.0.path()
    }
    fn downloads(&self) -> PathBuf {
        self.path().join("data/downloads/fetch")
    }
    /// What is in the downloads folder.
    fn left(&self) -> Vec<String> {
        std::fs::read_dir(self.downloads())
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Exit code, the envelope, and stdout plus stderr as text.
    fn run(&self, base: &str, args: &[&str]) -> (i32, Value, String) {
        let o = common::bin()
            .current_dir(self.path())
            .env("SCRIPTVOX_CONFIG", self.path().join("cfg"))
            .env("SCRIPTVOX_DATA", self.path().join("data"))
            .env("SCRIPTVOX_FETCH_BASE", base)
            .env("SCRIPTVOX_TOOLCHAIN", self.path().join("no-pins.lock"))
            // The fake helper, when the test wrote one; a name with no file is ignored.
            .env("SCRIPTVOX_FETCH_HELPER", self.path().join("helper.sh"))
            .env("SCRIPTVOX_FETCH_HELPER_SECS", "2")
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(!text.contains("MADEUPQUERY"), "the query leaked: {text}");
        let v = serde_json::from_slice(&o.stdout).unwrap_or_else(|_| panic!("no envelope: {text}"));
        (o.status.code().unwrap(), v, text)
    }
    /// A home where `setup fetch` ran.
    fn on() -> Self {
        let h = Self::new();
        let (code, v, _) = h.run("", &["setup", "fetch"]);
        assert_eq!(code, 0, "{v}");
        h
    }
}

/// 3 s of picture and sound, a key frame each second.
fn clip(ff: &str, dir: &Path) -> Vec<u8> {
    common::ffmpeg(
        ff,
        dir,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x240:r=25:d=3",
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=3",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "25",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "clip-src.mp4",
        ],
    );
    std::fs::read(dir.join("clip-src.mp4")).unwrap()
}

fn duration(ff: &str, file: &Path) -> f64 {
    common::probe(ff, file, "a", "format=duration")
        .lines()
        .next()
        .and_then(|d| d.parse().ok())
        .unwrap_or_else(|| panic!("no duration for {}", file.display()))
}

fn code_of(v: &Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}

#[test]
fn fetch_is_off_until_setup() {
    let h = Home::new();
    let (base, log) = serve(|_, _| resp(200, b"x"));
    let (code, v, _) = h.run(&base, &["fetch", &format!("{base}/a.mp4")]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(code_of(&v), "SVX_FETCH_SETUP");
    assert!(v["error"]["fix"].as_str().unwrap().contains("setup fetch"));
    assert!(log.lock().unwrap().is_empty(), "a request went out");
}

#[test]
fn fetch_setup_dry_run_writes_nothing_and_setup_writes_the_marker() {
    let h = Home::new();
    let marker = h.path().join("cfg/fetch-consent.json");
    let (code, v, text) = h.run("", &["setup", "fetch", "--dry-run"]);
    assert_eq!(code, 0, "{v}");
    // One time: a JSON run has the notice in the envelope only. A person at a terminal gets the line on stderr.
    assert_eq!(text.matches(scriptvox::fetch::NOTICE).count(), 1, "{text}");
    assert!(v["notice"]
        .as_str()
        .unwrap()
        .contains("The choice and the responsibility are yours"));
    assert!(!marker.exists(), "the dry run wrote the marker");
    let (code, v, _) = h.run("", &["setup", "fetch"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["notice"], scriptvox::fetch::NOTICE);
    assert_eq!(v["next"], json!(["scriptvox fetch <url>"]));
    let m: Value = serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    assert_eq!(m["schema"], "scriptvox.fetch-consent/v1");
    assert_eq!(m["notice"], 1);
    assert!(m["accepted_at"].as_str().unwrap().ends_with('Z'));
    assert!(h.left().is_empty(), "setup fetch downloaded a file");
}

#[test]
fn fetch_direct_mp4_gives_the_file_and_its_facts() {
    let Some(ff) = ff() else { return };
    let h = Home::on();
    let data = clip(&ff, h.path());
    let served = data.clone();
    let (base, log) = serve(move |r, _| file(r, &served));
    let url = format!("{base}/media/My%20clip.mp4?{QUERY}#t=1");
    let (code, v, _) = h.run(&base, &["fetch", &url, "--out", "got.mp4"]);
    assert_eq!(code, 0, "{v}");
    assert!(log.lock().unwrap().iter().all(|r| r.method == "GET"));
    let out = PathBuf::from(v["artifacts"]["video"].as_str().unwrap());
    assert!(out.is_absolute() && out.ends_with("got.mp4"));
    assert_eq!(std::fs::read(&out).unwrap(), data, "the file as it is");
    assert_eq!(v["source_kind"], "direct");
    assert_eq!(v["source_url"], format!("{base}/media/My%20clip.mp4"));
    assert_eq!(v["title"], "My%20clip");
    assert_eq!(v["licence"], Value::Null);
    assert!(
        (v["metrics"]["duration_s"].as_f64().unwrap() - 3.0).abs() < 0.3,
        "{v}"
    );
    assert_eq!(v["metrics"]["width"], 320);
    assert_eq!(v["metrics"]["bytes"], data.len());
    assert!(h.left().is_empty(), "{:?}", h.left());
    // The same --out a second time is refused before any request.
    let (code, v, _) = h.run(&base, &["fetch", &url, "--out", "got.mp4"]);
    assert_eq!((code, code_of(&v)), (2, "SVX_USAGE"), "{v}");
}

#[test]
fn fetch_direct_file_continues_after_a_broken_download() {
    let Some(ff) = ff() else { return };
    let h = Home::on();
    let data = clip(&ff, h.path());
    let served = data.clone();
    let broke = Arc::new(Mutex::new(false));
    let (base, log) = serve(move |r, _| {
        let mut out = file(r, &served);
        // The first request for the whole file gets half of it, then the line closes.
        if r.range().is_none() && !std::mem::replace(&mut *broke.lock().unwrap(), true) {
            out.cut = Some(served.len() / 2);
        }
        out
    });
    let url = format!("{base}/a.mp4");
    let (code, v, _) = h.run(&base, &["fetch", &url, "--out", "got.mp4"]);
    assert_eq!((code, code_of(&v)), (5, "SVX_PROVIDER_RETRYABLE"), "{v}");
    assert!(!h.path().join("got.mp4").exists());
    let (code, v, _) = h.run(&base, &["fetch", &url, "--out", "got.mp4"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(std::fs::read(h.path().join("got.mp4")).unwrap(), data);
    let half = data.len() / 2;
    let log = log.lock().unwrap();
    assert_eq!(
        log.last().unwrap().range(),
        Some((half, usize::MAX)),
        "the second run asks for the rest only"
    );
}

#[test]
fn fetch_refuses_a_web_page_a_dash_manifest_and_a_refused_file() {
    let Some(_) = ff() else { return };
    let h = Home::on();
    let (base, _) = serve(|r, _| match r.path.as_str() {
        "/page" => resp(200, b"<!doctype html><html><body>a page</body></html>"),
        "/list.mpd" => resp(
            200,
            b"<?xml version=\"1.0\"?><MPD xmlns=\"urn:mpeg:dash:schema:mpd:2011\"></MPD>",
        ),
        _ => resp(404, b"not here"),
    });
    let (code, v, _) = h.run(&base, &["fetch", &format!("{base}/page")]);
    assert_eq!((code, code_of(&v)), (2, "SVX_FETCH_UNSUPPORTED"), "{v}");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .starts_with("fetch cannot read this site"));
    let (code, v, _) = h.run(&base, &["fetch", &format!("{base}/list.mpd")]);
    assert_eq!((code, code_of(&v)), (2, "SVX_FETCH_UNSUPPORTED"), "{v}");
    assert_eq!(
        v["error"]["message"],
        "A DASH manifest (.mpd) is not supported yet."
    );
    assert!(v["error"]["fix"].as_str().unwrap().contains(".m3u8"));
    let (code, v, _) = h.run(&base, &["fetch", &format!("{base}/gone.mp4")]);
    assert_eq!((code, code_of(&v)), (1, "SVX_ERROR"), "{v}");
    assert_eq!(
        v["error"]["message"],
        "The server refused the file (HTTP 404)."
    );
    assert!(h.left().is_empty(), "{:?}", h.left());
}

#[test]
fn fetch_refuses_a_redirect_to_a_private_address() {
    let Some(_) = ff() else { return };
    let h = Home::on();
    let (inside, inside_log) = serve(|_, _| resp(200, b"secret"));
    let to = format!("{inside}/a.mp4");
    let (base, log) = serve(move |r, _| {
        let mut out = resp(302, b"");
        let loc = if r.path == "/ten" {
            "https://10.0.0.7/a.mp4".to_string()
        } else {
            to.clone()
        };
        out.headers.push(("Location".into(), loc));
        out
    });
    for path in ["/hop", "/ten"] {
        let (code, v, _) = h.run(&base, &["fetch", &format!("{base}{path}")]);
        assert_eq!((code, code_of(&v)), (2, "SVX_FETCH_UNSUPPORTED"), "{v}");
    }
    assert_eq!(log.lock().unwrap().len(), 2);
    assert!(
        inside_log.lock().unwrap().is_empty(),
        "the second host got a request"
    );
    // A private address as the first address.
    let (code, v, _) = h.run(&base, &["fetch", "https://192.168.1.10/a.mp4"]);
    assert_eq!((code, code_of(&v)), (2, "SVX_FETCH_UNSUPPORTED"), "{v}");
    assert_eq!(
        v["error"]["message"],
        "fetch does not read from a private address."
    );
}

#[test]
fn fetch_audio_copies_the_sound_and_a_cut_has_its_length() {
    let Some(ff) = ff() else { return };
    let h = Home::on();
    let data = clip(&ff, h.path());
    let (base, _) = serve(move |r, _| file(r, &data));
    let url = format!("{base}/talk.mp4");
    let (code, v, _) = h.run(&base, &["fetch", &url, "--audio", "--out", "a.m4a"]);
    assert_eq!(code, 0, "{v}");
    let audio = PathBuf::from(v["artifacts"]["audio"].as_str().unwrap());
    assert!(v["artifacts"].get("video").is_none());
    assert_eq!(
        common::probe(&ff, &audio, "v", "stream=codec_type"),
        "",
        "a video track stayed"
    );
    assert_eq!(common::probe(&ff, &audio, "a", "stream=codec_name"), "aac");
    assert!((duration(&ff, &audio) - 3.0).abs() < 0.3);

    let (code, v, _) = h.run(&base, &["fetch", &url, "--from", "1", "--to", "0:02"]);
    assert_eq!(code, 0, "{v}");
    let cut = PathBuf::from(v["artifacts"]["video"].as_str().unwrap());
    // The default name: the title, then 8 letters of a hash.
    let name = cut.file_name().unwrap().to_string_lossy().to_string();
    assert!(
        name.starts_with("talk-") && name.ends_with(".mp4") && name.len() == 17,
        "{name}"
    );
    let d = duration(&ff, &cut);
    assert!((d - 1.0).abs() < 0.4, "cut is {d} s");
    assert!((v["metrics"]["duration_s"].as_f64().unwrap() - d).abs() < 0.01);

    for (args, word) in [
        (vec!["--from", "2", "--to", "1"], "--from"),
        (vec!["--from", "soon"], "--from"),
        (vec!["--to", "1:99"], "--to"),
    ] {
        let mut all = vec!["fetch", url.as_str()];
        all.extend(args);
        let (code, v, _) = h.run(&base, &all);
        assert_eq!((code, code_of(&v)), (2, "SVX_USAGE"), "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains(word),
            "{v}"
        );
    }
}

/// An HLS copy of the clip in `dir/<name>/`: the playlist text and each other file by its name.
fn hls(ff: &str, dir: &Path, name: &str, extra: &[&str]) -> (String, Vec<(String, Vec<u8>)>) {
    clip(ff, dir);
    let folder = dir.join(name);
    std::fs::create_dir_all(&folder).unwrap();
    let mut args = vec![
        "-i",
        "../clip-src.mp4",
        "-c",
        "copy",
        "-f",
        "hls",
        "-hls_time",
        "1",
        "-hls_playlist_type",
        "vod",
    ];
    args.extend(extra);
    args.push("list.m3u8");
    common::ffmpeg(ff, &folder, &args);
    let mut files = Vec::new();
    for e in std::fs::read_dir(&folder).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n != "list.m3u8" {
            files.push((n, std::fs::read(e.path()).unwrap()));
        }
    }
    (
        std::fs::read_to_string(folder.join("list.m3u8")).unwrap(),
        files,
    )
}

#[test]
fn fetch_hls_takes_the_larger_variant_and_joins_the_parts() {
    let Some(ff) = ff() else { return };
    let h = Home::on();
    let (list, files) = hls(&ff, h.path(), "ts", &["-hls_segment_filename", "seg%d.ts"]);
    assert_eq!(files.len(), 3, "{list}");
    let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=300000,RESOLUTION=320x240\nlow/list.m3u8\n\
                  #EXT-X-STREAM-INF:BANDWIDTH=900000,RESOLUTION=320x240\nhigh/list.m3u8\n";
    let (base, log) = serve(move |r, _| {
        let path = r.path.split('?').next().unwrap();
        if path == "/show/master.m3u8" {
            return resp(200, master.as_bytes());
        }
        if path.ends_with("/list.m3u8") {
            return resp(200, list.as_bytes());
        }
        match files.iter().find(|(n, _)| path.ends_with(n.as_str())) {
            Some((_, data)) => file(r, data),
            None => resp(404, b""),
        }
    });
    let url = format!("{base}/show/master.m3u8?{QUERY}");
    let (code, v, _) = h.run(&base, &["fetch", &url, "--out", "show.mp4"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["source_kind"], "hls");
    assert_eq!(v["title"], "master");
    assert_eq!(v["source_url"], format!("{base}/show/master.m3u8"));
    let out = h.path().join("show.mp4");
    assert!((duration(&ff, &out) - 3.0).abs() < 0.3);
    assert_eq!(common::probe(&ff, &out, "a", "stream=codec_name"), "aac");
    assert_eq!(v["metrics"]["height"], 240);
    let paths: Vec<String> = log.lock().unwrap().iter().map(|r| r.path.clone()).collect();
    assert!(
        paths.iter().all(|p| !p.starts_with("/show/low/")),
        "{paths:?}"
    );
    assert_eq!(
        paths
            .iter()
            .filter(|p| p.starts_with("/show/high/seg"))
            .count(),
        3,
        "{paths:?}"
    );
    assert!(h.left().is_empty(), "{:?}", h.left());

    // A window keeps only the parts that it touches: the last 2 of 3.
    log.lock().unwrap().clear();
    let (code, v, _) = h.run(&base, &["fetch", &url, "--from", "1.2", "--out", "end.mp4"]);
    assert_eq!(code, 0, "{v}");
    let parts = log
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.path.contains("/seg"))
        .count();
    assert_eq!(parts, 2);
    let d = duration(&ff, &h.path().join("end.mp4"));
    assert!(d > 1.0 && d < 2.4, "the cut is {d} s");
}

#[test]
fn fetch_hls_with_a_start_part_asks_for_the_byte_ranges() {
    let Some(ff) = ff() else { return };
    let h = Home::on();
    let (list, files) = hls(
        &ff,
        h.path(),
        "fmp4",
        &["-hls_segment_type", "fmp4", "-hls_flags", "single_file"],
    );
    assert!(
        list.contains("#EXT-X-MAP:") && list.contains("#EXT-X-BYTERANGE:"),
        "{list}"
    );
    assert_eq!(files.len(), 1, "one file holds all parts");
    let (name, data) = files[0].clone();
    let size = data.len();
    let (base, log) = serve(move |r, _| {
        if r.path == "/v/list.m3u8" {
            resp(200, list.as_bytes())
        } else if r.path == format!("/v/{name}") {
            file(r, &data)
        } else {
            resp(404, b"")
        }
    });
    let (code, v, _) = h.run(
        &base,
        &["fetch", &format!("{base}/v/list.m3u8"), "--out", "v.mp4"],
    );
    assert_eq!(code, 0, "{v}");
    assert!((duration(&ff, &h.path().join("v.mp4")) - 3.0).abs() < 0.3);
    assert_eq!(
        common::probe(&ff, &h.path().join("v.mp4"), "v", "stream=codec_name"),
        "h264"
    );
    // Each request for the media file names a span, and the spans are the file from its first to its last byte.
    let spans: Vec<(usize, usize)> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|r| !r.path.ends_with(".m3u8"))
        .map(|r| r.range().expect("a part request with no Range"))
        .collect();
    assert!(spans.len() >= 4, "{spans:?}");
    assert_eq!(spans[0].0, 0);
    assert!(spans.windows(2).all(|w| w[1].0 == w[0].1 + 1), "{spans:?}");
    assert_eq!(spans.last().unwrap().1, size - 1);
}

#[test]
fn fetch_hls_refuses_live_and_protected_streams_before_any_part() {
    let Some(_) = ff() else { return };
    let h = Home::on();
    let (base, log) = serve(|r, _| {
        match r.path.as_str() {
        "/live.m3u8" => resp(200, b"#EXTM3U\n#EXTINF:4,\na.ts\n#EXTINF:4,\nb.ts\n"),
        "/key.m3u8" => resp(
            200,
            b"#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n#EXTINF:4,\na.ts\n#EXT-X-ENDLIST\n",
        ),
        _ => resp(200, b"G"),
    }
    });
    let (code, v, _) = h.run(&base, &["fetch", &format!("{base}/live.m3u8")]);
    assert_eq!((code, code_of(&v)), (2, "SVX_FETCH_UNSUPPORTED"), "{v}");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .starts_with("This is a live stream."));
    let (code, v, _) = h.run(&base, &["fetch", &format!("{base}/key.m3u8")]);
    assert_eq!((code, code_of(&v)), (2, "SVX_FETCH_UNSUPPORTED"), "{v}");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .starts_with("This stream is protected."));
    let paths: Vec<String> = log.lock().unwrap().iter().map(|r| r.path.clone()).collect();
    assert_eq!(
        paths,
        ["/live.m3u8", "/key.m3u8"],
        "a part or a key was asked for"
    );
}

#[test]
fn fetch_hls_refuses_a_playlist_above_4_gb_before_any_part() {
    let Some(_) = ff() else { return };
    let h = Home::on();
    // 17 spans of 250 MB in one file: 4.25 GB.
    let mut list = String::from("#EXTM3U\n");
    for _ in 0..17 {
        list.push_str("#EXTINF:4,\n#EXT-X-BYTERANGE:250000000\nall.ts\n");
    }
    list.push_str("#EXT-X-ENDLIST\n");
    let (base, log) = serve(move |r, _| match r.path.as_str() {
        "/big.m3u8" => resp(200, list.as_bytes()),
        _ => resp(200, b"G"),
    });
    let (code, v, _) = h.run(&base, &["fetch", &format!("{base}/big.m3u8")]);
    assert_eq!((code, code_of(&v)), (2, "SVX_USAGE"), "{v}");
    assert_eq!(
        v["error"]["message"],
        "The stream is above the limit of 4 GB."
    );
    let paths: Vec<String> = log.lock().unwrap().iter().map(|r| r.path.clone()).collect();
    assert_eq!(paths, ["/big.m3u8"], "a part was asked for");
    assert!(h.left().is_empty(), "{:?}", h.left());
}

#[test]
fn fetch_refuses_pages_and_plain_http_with_no_request() {
    let h = Home::on();
    let (base, log) = serve(|_, _| resp(200, b"x"));
    for url in [
        "http://example.org/a.mp4",
        "ftp://example.org/a.mp4",
        "a.mp4",
    ] {
        let (code, v, _) = h.run(&base, &["fetch", url]);
        assert_eq!((code, code_of(&v)), (2, "SVX_USAGE"), "{url}: {v}");
    }
    let (code, v, _) = h.run(&base, &["fetch", "https://localhost/a.mp4"]);
    assert_eq!((code, code_of(&v)), (2, "SVX_FETCH_UNSUPPORTED"), "{v}");
    assert!(log.lock().unwrap().is_empty(), "a request went out");
}

/// The public shape: a build with no `fetch-youtube` feature reads no site by its name.
#[cfg(not(feature = "fetch-youtube"))]
#[test]
fn fetch_refuses_a_youtube_address_with_no_request_and_no_helper() {
    let h = Home::on();
    let (base, log) = serve(|_, _| resp(200, b"x"));
    // A helper that would leave a file if anything started it.
    let mark = h.path().join("helper.ran");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file = h.path().join("helper.sh");
        std::fs::write(&file, "#!/bin/sh\n: > \"$(dirname \"$0\")/helper.ran\"\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    for url in [
        "https://www.youtube.com/watch?v=aaaaaaaaaaa",
        "https://youtu.be/aaaaaaaaaaa",
        "https://m.youtube.com/shorts/aaaaaaaaaaa",
        "https://www.youtube-nocookie.com/embed/aaaaaaaaaaa",
        "https://www.youtube.com/playlist?list=PLmadeup",
    ] {
        let (code, v, _) = h.run(&base, &["fetch", url]);
        assert_eq!(
            (code, code_of(&v)),
            (2, "SVX_FETCH_UNSUPPORTED"),
            "{url}: {v}"
        );
        assert_eq!(v["error"]["message"], "fetch cannot read this site.");
        assert!(v["error"]["fix"].as_str().unwrap().contains(".mp4"), "{v}");
    }
    // The flag of the private part is not in this build.
    let (code, v, _) = h.run(
        &base,
        &["fetch", "https://example.org/a.mp4", "--quality", "720"],
    );
    assert_eq!(code, 2, "{v}");
    assert!(log.lock().unwrap().is_empty(), "a request went out");
    assert!(!mark.exists(), "the helper started");
    assert!(h.left().is_empty(), "{:?}", h.left());
}

#[test]
fn fetch_is_not_called_by_make_for_an_address_in_a_script() {
    let Some(ff) = common::ready() else { return };
    let (base, log) = serve(|_, _| resp(200, b"x"));
    let env = common::Env::new(&ff);
    let script = format!("Dates are an old food. See {base}/film.mp4 for the film.");
    std::fs::write(env.dir.path().join("s.txt"), script).unwrap();
    let out = env
        .cmd("ok")
        .env("SCRIPTVOX_FETCH_BASE", &base)
        .args(["make", "--script", "s.txt", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).expect("make gave no envelope");
    assert_eq!(v["command"], "make");
    assert_eq!(v["ok"], true, "make did not run to the end: {v}");
    assert!(
        log.lock().unwrap().is_empty(),
        "make sent a request for an address in the script"
    );
}

/// `make --music <address>` (owner decision of 13 Nov 2026): one download through fetch, listed in the plan, kept for a
/// rerun, with its source in the credits and in the export.
#[test]
fn make_gets_music_from_an_address_one_time_and_credits_it() {
    let Some(ff) = common::ready() else { return };
    let env = common::Env::new(&ff);
    let dir = env.dir.path();
    let data = clip(&ff, dir);
    let (base, log) = serve(move |r, _| file(r, &data));
    let url = format!("{base}/media/song.mp4");
    std::fs::write(
        dir.join("s.txt"),
        "Dates are an old food. People like them.",
    )
    .unwrap();
    let run = |args: &[&str]| -> (i32, Value) {
        let o = env
            .cmd("ok")
            .env("SCRIPTVOX_FETCH_BASE", &base)
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        let v = serde_json::from_slice(&o.stdout)
            .unwrap_or_else(|_| panic!("no envelope: {}", String::from_utf8_lossy(&o.stderr)));
        (o.status.code().unwrap(), v)
    };
    let requests = || log.lock().unwrap().len();
    let make = ["make", "--script", "s.txt", "--music", &url];
    let cut = ["--music-from", "1", "--music-to", "0:02"];

    // fetch is off: the error of `fetch`, before any request.
    let (code, v) = run(&make);
    assert_eq!((code, code_of(&v)), (2, "SVX_FETCH_SETUP"), "{v}");
    assert_eq!(v["command"], "make");
    assert!(v["error"]["fix"]
        .as_str()
        .unwrap()
        .contains("scriptvox setup fetch"));
    let (code, v) = run(&["setup", "fetch"]);
    assert_eq!(code, 0, "{v}");

    // The plan names the download and the address, and sends nothing.
    let (code, v) = run(&[&make[..], &cut, &["--plan"]].concat());
    assert_eq!(code, 0, "{v}");
    let stage = v["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tool"] == "fetch")
        .unwrap_or_else(|| panic!("no fetch stage: {v}"));
    assert_eq!(
        (&stage["stage"], &stage["network"]),
        (&json!("music"), &json!(true))
    );
    assert!(stage["explain"].as_str().unwrap().contains(&url), "{stage}");

    // --local-only refuses, with a way out.
    let (code, v) = run(&[&make[..], &["--local-only"]].concat());
    assert_ne!(code, 0, "{v}");
    assert_eq!(v["needs_input"][0]["field"], "music", "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--local-only"),
        "{v}"
    );

    // A cut with no address, a private address and a plain http address are refused.
    let (code, v) = run(&[
        "make",
        "--script",
        "s.txt",
        "--music",
        "calm",
        "--music-from",
        "1",
    ]);
    assert_eq!(code, 2, "{v}");
    let (code, v) = run(&[
        "make",
        "--script",
        "s.txt",
        "--music",
        "https://10.0.0.8/a.mp3",
    ]);
    assert_eq!((code, code_of(&v)), (2, "SVX_FETCH_UNSUPPORTED"), "{v}");
    let (code, v) = run(&[
        "make",
        "--script",
        "s.txt",
        "--music",
        "http://example.org/a.mp3",
    ]);
    assert_eq!((code, code_of(&v)), (2, "SVX_USAGE"), "{v}");
    assert_eq!(requests(), 0, "a request before the first make");

    // The make: the cut of the sound is in the run folder and under the voice.
    let (code, v) = run(&[&make[..], &cut, &["--out", "one.mp4"]].concat());
    assert_eq!(code, 0, "{v}");
    let first = requests();
    assert!(first > 0);
    let run_dir = dir
        .join(".scriptvox/runs")
        .join(v["run_id"].as_str().unwrap());
    let song = std::fs::read_dir(run_dir.join("fetch"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "m4a"))
        .expect("no music file in the run folder");
    assert!((duration(&ff, &song) - 1.0).abs() < 0.3);
    // Owner rule of 13 Nov 2026: no line about commercial use in the output of `make`, and nothing on the picture.
    let said = format!("{} {}", v["findings"], v["next"]).to_lowercase();
    assert!(
        !said.contains("commercial") && !said.contains("personal use"),
        "{said}"
    );
    let (code, plain) = run(&["make", "--script", "s.txt", "--out", "plain.mp4"]);
    assert_eq!(code, 0, "{plain}");
    assert!(
        v.get("terms_note").is_none() && !v["explain"].to_string().contains("commercial"),
        "{v}"
    );
    // The 440 Hz tone of the served file is in the sound (the stub voice is 330 Hz); the picture is the same.
    let ffmpeg = |file: &str, args: &[&str]| {
        let o = std::process::Command::new(&ff)
            .current_dir(dir)
            .args(["-v", "info", "-i", file])
            .args(args)
            .output()
            .unwrap();
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        )
    };
    let tone = |file: &str| -> f64 {
        let t = ffmpeg(
            file,
            &[
                "-vn",
                "-af",
                "bandpass=f=440:width_type=h:w=10,volumedetect",
                "-f",
                "null",
                "-",
            ],
        );
        let at = t.find("mean_volume: ").expect("no volume") + 13;
        t[at..].split_whitespace().next().unwrap().parse().unwrap()
    };
    assert!(
        tone("one.mp4") > tone("plain.mp4") + 10.0,
        "the music is not in the sound"
    );
    let picture = |file: &str| {
        let t = ffmpeg(file, &["-an", "-f", "md5", "-"]);
        t[t.find("MD5=").expect("no md5")..][..36].to_string()
    };
    assert_eq!(
        picture("one.mp4"),
        picture("plain.mp4"),
        "the music changed the picture"
    );
    let credits = std::fs::read_to_string(v["artifacts"]["credits"].as_str().unwrap()).unwrap();
    assert!(
        credits.contains("Music: \"song\"")
            && credits.contains(&url)
            && credits.contains("layer native"),
        "{credits}"
    );
    let receipt: Value = serde_json::from_slice(
        &std::fs::read(v["artifacts"]["receipt"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    let item = &receipt["stock"][0];
    assert_eq!(
        (
            &item["provider"],
            &item["url"],
            &item["title"],
            &item["layer"]
        ),
        (
            &json!("fetch"),
            &json!(url),
            &json!("song"),
            &json!("native")
        ),
        "{receipt}"
    );
    assert_eq!(
        item["terms"]["commercial"], false,
        "no licence stated: not cleared"
    );

    // The recipe keeps the address, not the file.
    let (code, x) = run(&["explain", "one.mp4"]);
    assert_eq!(code, 0, "{x}");
    assert_eq!(x["recipe"]["request"]["music"], json!(url));
    assert_eq!(x["recipe"]["request"]["music_from"], "1");

    // The export shows the source, and the video is not cleared for business use.
    let (code, x) = run(&["export", run_dir.file_name().unwrap().to_str().unwrap()]);
    assert_eq!(code, 0, "{x}");
    assert_eq!(x["metrics"]["commercial_ok"], false, "{x}");
    assert!(x.get("terms_note").is_none(), "{x}");
    let meta = std::fs::read_to_string(x["artifacts"]["metadata"].as_str().unwrap()).unwrap();
    assert!(
        meta.contains(&url) && meta.contains("\"layer\": \"native\""),
        "{meta}"
    );
    assert!(
        std::fs::read_to_string(x["artifacts"]["credits"].as_str().unwrap())
            .unwrap()
            .contains(&url)
    );

    // A second make: the same music, no new request.
    let (code, v) = run(&[&make[..], &cut, &["--out", "two.mp4"]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(requests(), first, "the music was downloaded again");
    let credits = std::fs::read_to_string(v["artifacts"]["credits"].as_str().unwrap()).unwrap();
    assert!(credits.contains(&url), "{credits}");
    // A replay works from the store alone.
    let (code, v) = run(&["replay", "one.mp4", "--out", "again.mp4"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(requests(), first, "replay sent a request");
    // A remake of a video, which may come from someone else, sends no request to the address in its recipe.
    let (code, v) = run(&["remake", "one.mp4", "--out", "re.mp4"]);
    assert_eq!((code, code_of(&v)), (2, "SVX_USAGE"), "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("scriptvox fetch"),
        "{v}"
    );
    assert_eq!(requests(), first, "remake sent a request");
    // Another cut is another download. A query (it can be signed) is not kept in the recipe of the file.
    let signed = format!("{url}?sig=secret123");
    let (code, v) = run(&[
        "make",
        "--script",
        "s.txt",
        "--music",
        &signed,
        "--music-to",
        "2",
        "--out",
        "three.mp4",
    ]);
    assert_eq!(code, 0, "{v}");
    assert!(requests() > first);
    let (code, x) = run(&["explain", "three.mp4"]);
    assert_eq!(code, 0, "{x}");
    assert_eq!(x["recipe"]["request"]["music"], json!(url), "{x}");
    assert!(!x.to_string().contains("secret123"), "{x}");
}

/// `edit --music <address>`: the same code path and rules as `make` (`fetch.md` 10): fetch must be on, a private
/// address and `--local-only` are refused, one download for an address and a cut, the source beside the video.
#[test]
fn edit_gets_music_from_an_address_one_time_and_credits_it() {
    let Some(ff) = ff() else { return };
    let h = Home::new();
    let data = clip(&ff, h.path());
    let (base, log) = serve(move |r, _| file(r, &data));
    let url = format!("{base}/media/song.mp4");
    // The video to edit: 3 s of picture with a quiet tone.
    common::ffmpeg(
        &ff,
        h.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:r=25:d=3",
            "-f",
            "lavfi",
            "-i",
            "sine=f=200:d=3",
            "-af",
            "volume=0.05",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "in.mp4",
        ],
    );
    let requests = || log.lock().unwrap().len();
    let edit = [
        "edit",
        "in.mp4",
        "--music",
        &url,
        "--music-from",
        "1",
        "--music-to",
        "0:02",
    ];
    let run = |args: &[&str]| {
        let (code, v, _) = h.run(&base, args);
        (code, v)
    };

    // fetch is off: the error of `fetch`, under the name of `edit`, before any request.
    let (code, v) = run(&edit);
    assert_eq!((code, code_of(&v)), (2, "SVX_FETCH_SETUP"), "{v}");
    assert_eq!(v["command"], "edit");
    let (code, v) = run(&["setup", "fetch"]);
    assert_eq!(code, 0, "{v}");

    // --local-only, a cut with no address and a private address are refused. No request so far.
    let (code, v) = run(&[&edit[..], &["--local-only"]].concat());
    assert_ne!(code, 0, "{v}");
    assert_eq!(v["needs_input"][0]["field"], "music", "{v}");
    let (code, v) = run(&["edit", "in.mp4", "--music", "calm", "--music-from", "1"]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("https address"),
        "{v}"
    );
    let (code, v) = run(&["edit", "in.mp4", "--music", "https://10.0.0.8/a.mp3"]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(requests(), 0);

    // The edit: the picture is copied, the sound has the music, the source is beside the video and in the answer.
    let (code, v) = run(&[&edit[..], &["--out", "one.mp4"]].concat());
    assert_eq!(code, 0, "{v}");
    let first = requests();
    assert!(first > 0);
    assert!((duration(&ff, &h.path().join("one.mp4")) - 3.0).abs() < 0.2);
    let credits = std::fs::read_to_string(v["artifacts"]["credits"].as_str().unwrap()).unwrap();
    assert!(
        credits.starts_with("Music: ") && credits.contains(&url),
        "{credits}"
    );
    assert!(
        credits.contains("downloaded by scriptvox fetch"),
        "{credits}"
    );
    assert_eq!(v["music"]["address"], json!(url), "{v}");
    assert_eq!(v["music"]["source"]["provider"], "fetch", "{v}");
    assert_eq!(v["music"]["source"]["terms"]["commercial"], false, "{v}");

    // A second edit with the same address and cut: no new request. Another cut is another download.
    let (code, v) = run(&[&edit[..], &["--out", "two.mp4"]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(requests(), first, "the music was downloaded again");
    let (code, v) = run(&[
        "edit",
        "in.mp4",
        "--music",
        &url,
        "--music-to",
        "2",
        "--out",
        "three.mp4",
    ]);
    assert_eq!(code, 0, "{v}");
    assert!(requests() > first);
}

/// The tests of the part that is in the build only with the cargo feature.
#[cfg(feature = "fetch-youtube")]
mod fetch_youtube;

/// `cut_on: beat` with `music` from an address (reel test of 14 Nov 2026, docs/specs/reel-gaps.md): the music is
/// fetched first, and the cuts follow its beat. Before, this pair was exit 2.
#[test]
fn make_cuts_on_the_beat_of_music_from_an_address() {
    let Some(ff) = ff() else { return };
    let h = Home::on();
    let dir = h.path();
    // A 20 ms click every 0.5 s (120 BPM), as an m4a file.
    let clicks = "aevalsrc='0.8*sin(2*PI*1000*t)*lt(mod(t,0.5),0.02)':s=48000:d=30";
    common::ffmpeg(
        &ff,
        dir,
        &[
            "-f",
            "lavfi",
            "-i",
            clicks,
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "click.m4a",
        ],
    );
    for (c, geq) in [
        ("red", "r='200+55*random(1)':g='60':b='60'"),
        ("green", "r='60':g='200+55*random(1)':b='60'"),
        ("blue", "r='60':g='60':b='200+55*random(1)'"),
    ] {
        let src = format!("nullsrc=s=320x180,geq={geq}");
        common::ffmpeg(
            &ff,
            dir,
            &[
                "-f",
                "lavfi",
                "-i",
                &src,
                "-frames:v",
                "1",
                &format!("{c}.png"),
            ],
        );
    }
    let data = std::fs::read(dir.join("click.m4a")).unwrap();
    std::fs::remove_file(dir.join("click.m4a")).unwrap();
    let (base, log) = serve(move |r, _| file(r, &data));
    let url = format!("{base}/media/click.m4a");
    let brief = json!({"tts": "none", "lang": "en", "aspect": "16:9", "transition": "cut", "music": url, "cut_on": "beat",
    "scenes": [
        {"script": "The first scene has a red picture and nine words.", "media": "red.png"},
        {"script": "The second scene is green, and it is a little longer than the first.", "media": "green.png"},
        {"script": "The third scene is blue and it ends the video.", "media": "blue.png"}
    ]});
    std::fs::write(dir.join("b.json"), brief.to_string()).unwrap();
    let (code, v, _) = h.run(&base, &["plan", "--brief", "b.json"]);
    assert_eq!(code, 0, "{v}");
    let stage = v["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tool"] == "fetch")
        .unwrap()
        .clone();
    assert!(
        stage["explain"]
            .as_str()
            .unwrap()
            .contains("before the voice"),
        "{stage}"
    );
    assert_eq!(log.lock().unwrap().len(), 0, "a plan sends nothing");
    let (code, v, text) = h.run(&base, &["make", "--brief", "b.json", "--out", "beat.mp4"]);
    assert_eq!(code, 0, "{v}");
    assert!(
        text.contains("getting the music from its address first"),
        "{text}"
    );
    let c = &v["cut_on"];
    assert!((c["bpm"].as_f64().unwrap() - 120.0).abs() < 2.0, "{v}");
    assert_eq!(c["boundaries"].as_array().unwrap().len(), 2, "{v}");
    let run_dir = dir
        .join(".scriptvox/runs")
        .join(v["run_id"].as_str().unwrap());
    let credits = std::fs::read_to_string(run_dir.join("credits.txt")).unwrap();
    assert!(
        credits.contains("click.m4a"),
        "one credit line for the music: {credits}"
    );
    assert_eq!(credits.matches("click.m4a").count(), 1, "{credits}");
    // A search or a music row still comes after the voice: exit 2, with a fix line.
    let mut late = brief.clone();
    late["music"] = Value::Null;
    late["music_search"] = json!("drums");
    std::fs::write(dir.join("late.json"), late.to_string()).unwrap();
    let (code, v, _) = h.run(&base, &["plan", "--brief", "late.json"]);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("https address"),
        "{v}"
    );
}

/// A site that asked this network to sign in: the fix line gives the route that always works, and a plan with that
/// site says so for a short time. The record is one small file; the plan sends nothing.
#[test]
fn a_plan_says_that_the_site_asked_to_sign_in_a_short_time_ago() {
    let h = Home::on();
    let brief = json!({"script": "Dates are an old food. People like them.", "tts": "none", "lang": "en",
        "music": "https://media.example.org/song.mp3"});
    std::fs::write(h.path().join("b.json"), brief.to_string()).unwrap();
    let explain = |h: &Home| {
        let (code, v, _) = h.run("", &["plan", "--brief", "b.json"]);
        assert_eq!(code, 0, "{v}");
        v["stages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["tool"] == "fetch")
            .unwrap()["explain"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert!(!explain(&h).contains("sign in"));
    std::fs::create_dir_all(h.path().join("data")).unwrap();
    std::fs::write(h.path().join("data/fetch-sign-in.txt"), "media.example.org").unwrap();
    let said = explain(&h);
    assert!(
        said.contains("asked this network to sign in")
            && said.contains("give the local file as `music`"),
        "{said}"
    );
    std::fs::write(h.path().join("data/fetch-sign-in.txt"), "other.example.org").unwrap();
    assert!(!explain(&h).contains("sign in"), "another site");
}
