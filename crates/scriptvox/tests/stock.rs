//! Stock rows (RVX-021) against a local server that plays Pexels and Pixabay.
#![cfg(unix)]
mod common;
use common::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Log {
    paths: Vec<String>,
    ranges: Vec<String>,
}

/// `pexels_status`: the HTTP status of the Pexels search (200 or 429). 204: Pexels and Pixabay have nothing for a
/// query of 2 or more words, and clips for one word.
fn server(clip: Vec<u8>, pexels_status: u16) -> (String, Arc<Mutex<Log>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let log = Arc::new(Mutex::new(Log::default()));
    let (lg, b) = (log.clone(), base.clone());
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
            let mut range = None;
            loop {
                let mut h = String::new();
                if r.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                    break;
                }
                if h.to_lowercase().starts_with("range:") {
                    range = Some(h[6..].trim().to_string());
                }
            }
            lg.lock().unwrap().paths.push(path.clone());
            let mut s = s;
            let send = |s: &mut std::net::TcpStream, status: &str, extra: &str, body: &[u8]| {
                let _ = write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(body);
            };
            if pexels_status == 204
                && (path.starts_with("/pexels") || path.starts_with("/pixabay"))
                && (path.contains('+') || path.contains("%20"))
            {
                send(&mut s, "200 OK", "", br#"{"videos": [], "hits": []}"#);
                continue;
            }
            if path.starts_with("/pexels") {
                if pexels_status == 429 {
                    send(&mut s, "429 Too Many Requests", "", b"{}");
                    continue;
                }
                let vids: Vec<Value> = (1..=3)
                    .map(|i| json!({"id": i, "duration": 10, "url": format!("https://pexels.example/{i}"), "user": {"name": format!("Ann {i}")},
                        "video_files": [{"link": format!("{b}/files/p{i}.mp4"), "width": 1080, "height": 1920}]}))
                    .collect();
                send(
                    &mut s,
                    "200 OK",
                    "",
                    json!({"videos": vids}).to_string().as_bytes(),
                );
            } else if path.starts_with("/pixabay") {
                let hits: Vec<Value> = (1..=3)
                    .map(|i| json!({"id": 10 + i, "duration": 10, "pageURL": format!("https://pixabay.example/{i}"), "user": format!("Bo {i}"),
                        "videos": {"large": {"url": format!("{b}/files/x{i}.mp4"), "width": 1920, "height": 1080}}}))
                    .collect();
                send(
                    &mut s,
                    "200 OK",
                    "",
                    json!({"hits": hits}).to_string().as_bytes(),
                );
            } else if path.starts_with("/commons") {
                let page = |id: u32, lic: &str, secs: u32| {
                    json!({"pageid": id, "videoinfo": [{"duration": secs, "descriptionurl": format!("https://commons.example/{id}"),
                    "derivatives": [{"src": format!("{b}/files/c{id}.vp9.webm"), "width": 1080, "height": 1920}, {"src": format!("{b}/files/c{id}.mpeg4.mov"), "width": 1080, "height": 1920}],
                    "extmetadata": {"LicenseShortName": {"value": lic}, "Artist": {"value": "<a href=\"//x\">Cy Lee</a>"}}}]})
                };
                let pages = [
                    page(1, "CC BY-ND 4.0", 20),
                    page(2, "CC BY-NC 4.0", 20),
                    page(3, "GFDL", 20),
                    page(4, "CC BY-SA 4.0", 900),
                    page(5, "CC BY-SA 4.0", 20),
                ];
                send(
                    &mut s,
                    "200 OK",
                    "",
                    json!({"query": {"pages": pages}}).to_string().as_bytes(),
                );
            } else if path.starts_with("/archive/search") {
                let docs = json!([{"identifier": "a.b c", "licenseurl": "https://creativecommons.org/publicdomain/zero/1.0/", "creator": ["Di", "Ed"]}]);
                send(
                    &mut s,
                    "200 OK",
                    "",
                    json!({"response": {"docs": docs}}).to_string().as_bytes(),
                );
            } else if path.starts_with("/archive/meta/") {
                let files = json!([{"name": "x.mp3", "length": "00:30"}, {"name": "my clip.mp4", "width": "1080", "height": "1920", "length": "30.05"}]);
                send(
                    &mut s,
                    "200 OK",
                    "",
                    json!({"files": files}).to_string().as_bytes(),
                );
            } else if path.starts_with("/files/p") && pexels_status == 403 {
                send(&mut s, "403 Forbidden", "", b"");
            } else if path.starts_with("/files/") {
                match range
                    .as_deref()
                    .and_then(|r| r.strip_prefix("bytes="))
                    .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok())
                {
                    Some(from) => {
                        lg.lock().unwrap().ranges.push(format!("bytes={from}-"));
                        let extra = format!(
                            "Content-Range: bytes {from}-{}/{}\r\n",
                            clip.len() - 1,
                            clip.len()
                        );
                        send(&mut s, "206 Partial Content", &extra, &clip[from..]);
                    }
                    None => send(&mut s, "200 OK", "", &clip),
                }
            } else {
                send(&mut s, "404 Not Found", "", b"");
            }
        }
    });
    (base, log)
}

fn providers(e: &Env, base: &str) {
    let real = std::fs::read_to_string(root().join("registry/providers.toml")).unwrap();
    // The keyless rows are real public services: `keyless_row_*` has its own file.
    let t = real
        .split("# Keyless rows")
        .next()
        .unwrap()
        .replace(
            "https://api.pexels.com/videos/search",
            &format!("{base}/pexels/search"),
        )
        .replace(
            "https://pixabay.com/api/videos/",
            &format!("{base}/pixabay/search"),
        )
        .replace(
            r#"hosts = ["pexels.com", "vimeo.com"]"#,
            r#"hosts = ["127.0.0.1"]"#,
        )
        .replace(r#"hosts = ["pixabay.com"]"#, r#"hosts = ["127.0.0.1"]"#);
    std::fs::write(e.dir.path().join("providers.toml"), t).unwrap();
}

/// The real keyless rows of the registry, on the local server.
fn keyless_providers(e: &Env, base: &str, row: &str) {
    let real = std::fs::read_to_string(root().join("registry/providers.toml")).unwrap();
    let rows = format!(
        "[[provider]]{}",
        real.split("# Keyless rows")
            .nth(1)
            .unwrap()
            .split_once("[[provider]]")
            .unwrap()
            .1
    );
    let one = rows
        .split("[[provider]]")
        .find(|r| r.contains(&format!("name = \"{row}\"")))
        .unwrap();
    let local = regex_free_url(one, "search_url", &format!("{base}/{row}/search?q={{q}}"))
        .replace(
            "https://archive.org/metadata/{id}",
            &format!("{base}/archive/meta/{{id}}"),
        )
        .replace(
            "https://archive.org/download/{id}/",
            &format!("{base}/files/{{id}}/"),
        )
        .replace(
            r#"hosts = ["upload.wikimedia.org"]"#,
            r#"hosts = ["127.0.0.1"]"#,
        )
        .replace(r#"hosts = ["archive.org"]"#, r#"hosts = ["127.0.0.1"]"#);
    std::fs::write(
        e.dir.path().join("providers.toml"),
        format!("[[provider]]{local}"),
    )
    .unwrap();
}

fn regex_free_url(row: &str, field: &str, url: &str) -> String {
    row.lines()
        .map(|l| {
            if l.starts_with(field) {
                format!("{field} = \"{url}\"")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn keyless_row_commons_needs_no_key_and_keeps_terms_per_file() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, log) = server(clip(), 200);
    keyless_providers(&e, &base, "commons");
    let (code, v) = make(&e, &["--stock", "ocean"], false);
    assert_eq!(code, 0, "{v}");
    let credits = std::fs::read_to_string(v["artifacts"]["credits"].as_str().unwrap()).unwrap();
    // Order by fit, not licence (owner, 7 Nov 2026): no-derivatives, non-commercial and unread licences are kept, each
    // with its note; the user decides.
    for want in [
        "Shot 1: video by Cy Lee on Wikimedia Commons, https://commons.example/1 (CC BY-ND 4.0, as its uploader states; credit the author; personal use only, do not share an edited copy)",
        "https://commons.example/2 (CC BY-NC 4.0, as its uploader states; credit the author; not for commercial use)",
        "https://commons.example/3 (GFDL, as its uploader states; licence not read",
    ] {
        assert!(credits.contains(want), "{want}\n{credits}");
    }
    // No line about the terms in the output (owner, 13 Nov 2026): they are data in credits.txt and metadata.json.
    assert!(v.get("terms_note").is_none(), "{v}");
    let log = log.lock().unwrap();
    let files: Vec<_> = log
        .paths
        .iter()
        .filter(|p| p.starts_with("/files/"))
        .collect();
    assert!(
        files.iter().any(|p| p.ends_with("c1.vp9.webm")),
        "{files:?}"
    );
    assert!(
        !files.iter().any(|p| p.contains("/c4.")),
        "no 15 min file: {files:?}"
    );
}

#[test]
fn keyless_row_archive_reads_the_item_for_its_files() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, log) = server(clip(), 200);
    keyless_providers(&e, &base, "archive");
    let (code, v) = make(&e, &["--stock", "ocean"], false);
    assert_eq!(code, 0, "{v}");
    let credits = std::fs::read_to_string(v["artifacts"]["credits"].as_str().unwrap()).unwrap();
    assert!(
        credits.starts_with(
            "Shot 1: video by Di on Internet Archive, https://archive.org/details/a.b c ("
        ),
        "{credits}"
    );
    assert!(
        !credits.contains("credit the author"),
        "public domain: {credits}"
    );
    let log = log.lock().unwrap();
    assert!(
        log.paths
            .iter()
            .any(|p| p == "/files/a.b%20c/my%20clip.mp4"),
        "{:?}",
        log.paths
    );
}

fn clip() -> Vec<u8> {
    std::fs::read(root().join("fixtures/media/01-testcard.mp4")).unwrap()
}

fn make(e: &Env, extra: &[&str], keys: bool) -> (i32, Value) {
    std::fs::write(
        e.dir.path().join("s.txt"),
        "Ocean waves at dawn. A city wakes up. Coffee in the morning.",
    )
    .unwrap();
    let mut c = e.cmd("ok");
    c.env("SCRIPTVOX_PROVIDERS", e.dir.path().join("providers.toml"))
        .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"));
    if keys {
        c.env("PEXELS_API_KEY", "pexels-test-key-1234")
            .env("PIXABAY_API_KEY", "pixabay-test-key-1234");
    } else {
        c.env_remove("PEXELS_API_KEY").env_remove("PIXABAY_API_KEY");
    }
    let o = c
        .args(["make", "--script", "s.txt", "--json"])
        .args(extra)
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

#[test]
fn stock_clips_follow_the_script_and_are_credited() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, log) = server(clip(), 200);
    providers(&e, &base);
    let (code, v) = make(&e, &["--stock", "ocean waves,city morning,coffee"], true);
    assert_eq!(code, 0, "{v}");
    let credits = std::fs::read_to_string(v["artifacts"]["credits"].as_str().unwrap()).unwrap();
    assert_eq!(
        credits.lines().count(),
        v["metrics"]["shots"].as_u64().unwrap() as usize,
        "{credits}"
    );
    assert!(
        credits.starts_with("Shot 1: video by Ann 1 on Pexels"),
        "{credits}"
    );
    let searches: Vec<String> = log
        .lock()
        .unwrap()
        .paths
        .iter()
        .filter(|p| p.starts_with("/pexels"))
        .cloned()
        .collect();
    assert!(
        searches[0].contains("query=ocean+waves"),
        "script order: {searches:?}"
    );
}

#[test]
fn a_two_word_term_with_no_clip_is_tried_again_as_its_first_word() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, log) = server(clip(), 204);
    providers(&e, &base);
    // `auto` makes 2-word terms from the script; no row has a clip for a pair.
    let (code, v) = make(&e, &["--stock", "auto"], true);
    assert_eq!(code, 0, "{v}");
    let events = std::fs::read_to_string(v["events_file"].as_str().unwrap()).unwrap();
    assert!(
        events
            .lines()
            .any(|l| l.contains("\"type\":\"fallback\"") && l.contains("full term")),
        "{events}"
    );
    let log = log.lock().unwrap();
    assert!(
        log.paths
            .iter()
            .any(|p| p.starts_with("/pexels") && !p.contains('+') && !p.contains("%20")),
        "{:?}",
        log.paths
    );
}

#[test]
fn pexels_429_falls_back_to_pixabay_with_an_event() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, _log) = server(clip(), 429);
    providers(&e, &base);
    let (code, v) = make(&e, &["--stock", "ocean"], true);
    assert_eq!(code, 0, "{v}");
    let events = std::fs::read_to_string(v["events_file"].as_str().unwrap()).unwrap();
    assert!(
        events
            .lines()
            .any(|l| l.contains("\"type\":\"fallback\"") && l.contains("\"to\":\"pixabay\"")),
        "{events}"
    );
    let credits = std::fs::read_to_string(v["artifacts"]["credits"].as_str().unwrap()).unwrap();
    assert!(credits.contains("on Pixabay"));
}

#[test]
fn a_host_that_refuses_the_download_moves_to_the_next_row() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, log) = server(clip(), 403);
    providers(&e, &base);
    let (code, v) = make(&e, &["--stock", "ocean,city"], true);
    assert_eq!(code, 0, "{v}");
    let credits = std::fs::read_to_string(v["artifacts"]["credits"].as_str().unwrap()).unwrap();
    assert!(
        credits.contains("on Pixabay") && !credits.contains("on Pexels"),
        "{credits}"
    );
    let log = log.lock().unwrap();
    assert_eq!(
        log.paths
            .iter()
            .filter(|p| p.starts_with("/files/p"))
            .count(),
        1,
        "the refused host gets one try: {:?}",
        log.paths
    );
}

#[test]
fn no_key_is_needs_input_with_the_free_template() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    providers(&e, "http://127.0.0.1:1");
    let (code, v) = make(&e, &["--stock", "auto"], false);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["needs_input"][0]["kind"], "secret");
    assert!(v["needs_input"][0]["alternatives"]
        .to_string()
        .contains("kinetic"));
}

#[test]
fn interrupted_download_resumes_with_range() {
    let body = clip();
    let (base, log) = server(body.clone(), 200);
    let d = tempfile::tempdir().unwrap();
    let url = format!("{base}/files/r.mp4");
    std::fs::write(
        d.path().join(scriptvox::http_rows::part_name(&url)),
        &body[..1000],
    )
    .unwrap();
    let dest = d.path().join("r.mp4");
    scriptvox::http_rows::download(&url, d.path(), &dest, None).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    assert_eq!(log.lock().unwrap().ranges, ["bytes=1000-"]);
}

#[test]
fn stock_replay_makes_zero_provider_calls() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (base, log) = server(clip(), 200);
    providers(&e, &base);
    let (code, v) = make(&e, &["--stock", "ocean,city,coffee"], true);
    assert_eq!(code, 0, "{v}");
    let before = log.lock().unwrap().paths.len();
    let o = e
        .cmd("flaky")
        .env("SCRIPTVOX_PROVIDERS", e.dir.path().join("providers.toml"))
        .env_remove("PEXELS_API_KEY")
        .env_remove("PIXABAY_API_KEY")
        .args([
            "replay",
            v["artifacts"]["video"].as_str().unwrap(),
            "--out",
            "r.mp4",
            "--json",
        ])
        .output()
        .unwrap();
    let r: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{r}");
    assert_eq!(
        log.lock().unwrap().paths.len(),
        before,
        "a warm replay makes no HTTP request"
    );
}

/// When every row fails, the error names each row's reason, not only the last (audit H4). Nothing listens on port 1.
#[test]
fn every_row_reason_is_in_the_final_error() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    providers(&e, "http://127.0.0.1:1");
    let (code, v) = make(&e, &["--stock", "ocean"], true);
    assert_ne!(code, 0, "{v}");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(msg.contains("pexels") && msg.contains("pixabay"), "{msg}");
}

/// A loopback Openverse: two pictures with different licences, and one sound.
fn openverse(png: Vec<u8>, wav: Vec<u8>) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let b = base.clone();
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            loop {
                let mut h = String::new();
                if r.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                    break;
                }
            }
            let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
            let item = |id: &str, file: String, lic: &str| {
                json!({"id": id, "title": id, "url": file, "foreign_landing_url": format!("https://example.org/{id}"),
                    "creator": format!("author {id}"), "license": lic, "license_version": "4.0"})
            };
            let body: Vec<u8> = if path.starts_with("/v1/images") {
                json!({"results": [item("p1", format!("{b}/p1.png"), "by"), item("p2", format!("{b}/p2.png"), "by-nc")]})
                    .to_string()
                    .into_bytes()
            } else if path.starts_with("/v1/audio") {
                json!({"results": [item("s1", format!("{b}/s1.wav"), "by")]})
                    .to_string()
                    .into_bytes()
            } else if path.ends_with(".png") {
                png.clone()
            } else {
                wav.clone()
            };
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(&body);
        }
    });
    base
}

#[test]
fn make_picks_keyless_files_and_each_keeps_its_own_licence() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let tmp = e.dir.path().join("in");
    std::fs::create_dir_all(&tmp).unwrap();
    let gen = |args: &[&str], name: &str| {
        let out = tmp.join(name);
        assert!(std::process::Command::new(&ff)
            .args(["-v", "error", "-y"])
            .args(args)
            .arg(&out)
            .status()
            .unwrap()
            .success());
        std::fs::read(out).unwrap()
    };
    let png = gen(
        &["-f", "lavfi", "-i", "testsrc2=s=1280x720", "-frames:v", "1"],
        "a.png",
    );
    let wav = gen(&["-f", "lavfi", "-i", "sine=f=220:d=12"], "a.wav");
    let base = openverse(png, wav);
    let brief = e.dir.path().join("b.json");
    std::fs::write(
        &brief,
        json!({"scenes": [
            {"script": "Rain falls on the city.", "open_image": "rain"},
            {"script": "The drains are working.", "open_image": "rain"}
        ], "music_search": "piano"})
        .to_string(),
    )
    .unwrap();
    let out = e
        .cmd("ok")
        .env("SCRIPTVOX_OPENVERSE_BASE", &base)
        .env("SCRIPTVOX_CONFIG", e.dir.path().join("cfg"))
        .args(["make", "--brief", brief.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "{v}");
    let credits = std::fs::read_to_string(v["artifacts"]["credits"].as_str().unwrap()).unwrap();
    assert!(
        credits.contains("Shot 1: picture: by author p1"),
        "{credits}"
    );
    assert!(
        credits.contains("Shot 2: picture: by author p2"),
        "{credits}"
    );
    assert!(credits.contains("Music: by author s1"), "{credits}");
    assert!(v.get("terms_note").is_none(), "{v}");
}
