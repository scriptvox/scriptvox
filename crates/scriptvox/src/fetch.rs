//! `scriptvox fetch <url>`: get a video or audio file from an address (`docs/specs/fetch.md`). Two kinds: a media
//! file and an HLS playlist. Off until the user runs `scriptvox setup fetch`. FFmpeg only sees local files: every byte
//! comes through our own HTTP calls, and each file passes the media gate.

use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::http_rows::{local_host, USER_AGENT};
use crate::media_gate::{self, Kind as Media};
use serde_json::json;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const NOTICE: &str = "fetch gets a file from any address that you give. The choice and the responsibility are yours: ScriptVox does not check the rights to a file.";
/// Raised when the notice text changes: `fetch` then asks for `setup fetch` again. Not raised for the shorter text
/// of 14 Nov 2026: it says less than the first one, so a person who read that one has nothing new to read.
const NOTICE_NUMBER: u64 = 1;
/// The limit of `http_rows::download`.
pub(crate) const MAX_FILE: u64 = 4_000_000_000;

/// `--from` and `--to` as seconds; no end when `--to` was not given.
pub(crate) type Window = Option<(f64, Option<f64>)>;

pub struct Args {
    pub url: String,
    pub audio: bool,
    pub from: Option<String>,
    pub to: Option<String>,
    pub out: Option<PathBuf>,
    /// The folder for the file when `out` is not given (`make --music`); none is the working folder.
    pub dir: Option<PathBuf>,
    /// The largest picture height that the user wants.
    #[cfg(feature = "fetch-youtube")]
    pub quality: Option<u32>,
    pub use_system: bool,
}

/// The kind of a source, after the tests of spec 3.1.
#[derive(Debug, PartialEq)]
pub enum Kind {
    Direct,
    Hls,
}

pub struct Fetched {
    /// 1 file, or the video then the sound.
    pub files: Vec<PathBuf>,
    /// Media time at which each file starts (not 0 when HLS parts before a window were skipped).
    pub starts: Vec<f64>,
    pub kind: &'static str,
    pub source_url: String,
    pub title: String,
    /// For the default file name: the video id, else none (a hash of the address is used).
    pub id: Option<String>,
    pub uploader: Option<String>,
    pub duration_s: Option<f64>,
    pub licence: Option<String>,
    pub findings: Vec<String>,
}

#[derive(Debug)]
pub struct Fail {
    pub exit: Exit,
    pub code: &'static str,
    pub message: String,
    pub fix: Option<String>,
    /// True for a limit of our own code (a site or a format that it does not read). Never for a rule: a private
    /// address, a live or a protected stream.
    pub helper: bool,
}

impl Fail {
    pub(crate) fn new(exit: Exit, code: &'static str, message: &str, fix: &str) -> Self {
        Self {
            exit,
            code,
            message: message.into(),
            fix: Some(fix.into()),
            helper: false,
        }
    }
    pub(crate) fn or_helper(mut self) -> Self {
        self.helper = true;
        self
    }
    pub(crate) fn usage(message: &str, fix: &str) -> Self {
        Self::new(Exit::Usage, "SVX_USAGE", message, fix)
    }
    pub(crate) fn unsupported(message: &str, fix: &str) -> Self {
        Self::new(Exit::Usage, "SVX_FETCH_UNSUPPORTED", message, fix)
    }
    /// A site that is not a media file and not a playlist.
    pub(crate) fn no_site() -> Self {
        Self::unsupported(
            "fetch cannot read this site.",
            "Give the address of the media file itself (it often ends in .mp4, .m4a or .m3u8).",
        )
    }
    pub(crate) fn private() -> Self {
        Self::unsupported(
            "fetch does not read from a private address.",
            "Give a public https address.",
        )
    }
    pub(crate) fn live() -> Self {
        Self::unsupported(
            "This is a live stream. fetch reads complete recordings only.",
            "Fetch it after the stream has ended.",
        )
    }
    /// A direct file or an HLS part that the server did not give.
    pub(crate) fn refused(status: u16) -> Self {
        if status == 429 || status >= 500 {
            return Self::stopped(&format!("the server answered HTTP {status}"));
        }
        Self::new(
            Exit::Error,
            "SVX_ERROR",
            &format!("The server refused the file (HTTP {status})."),
            "Check the address in a browser. A signed address can be too old.",
        )
    }
    /// `cause` is our own short text: never an address, never the text of the HTTP library.
    pub(crate) fn stopped(cause: &str) -> Self {
        Self::new(
            Exit::ProviderRetryable,
            "SVX_PROVIDER_RETRYABLE",
            &format!("The download stopped: {cause}"),
            "Run the same command again. A download of one file continues from the part that is on disk.",
        )
    }
    fn envelope(self) -> Envelope {
        Envelope::fail_code("fetch", self.exit, self.code, self.message, self.fix)
    }
}

/// A network fault with no address in its text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HttpFault {
    Connect,
    Timeout,
    Read,
}

impl From<HttpFault> for Fail {
    fn from(f: HttpFault) -> Self {
        Fail::stopped(match f {
            HttpFault::Connect => "connect",
            HttpFault::Timeout => "timeout",
            HttpFault::Read => "read",
        })
    }
}

/// One answer, with its body in memory (the callers ask for at most 8 MiB, or cap the size).
pub(crate) struct Ranged {
    pub status: u16,
    /// The size of the whole file, from `Content-Range`.
    pub total: Option<u64>,
    pub body: Vec<u8>,
    pub location: Option<String>,
    pub disposition: Option<String>,
}

type Answer = Result<ureq::http::Response<ureq::Body>, ureq::Error>;

/// The HTTP client of fetch: no automatic redirect, 3 tries on a network fault, the waits of `http_rows::download`
/// on 429.
pub(crate) struct Http {
    agent: ureq::Agent,
}

impl Http {
    pub(crate) fn new() -> Self {
        Self { agent: agent() }
    }

    /// A GET, with a `Range` header when `range` (first and last byte) is given. At most `limit` bytes are read.
    pub(crate) fn get(
        &mut self,
        url: &str,
        user_agent: &str,
        range: Option<(u64, u64)>,
        limit: u64,
    ) -> Result<Ranged, HttpFault> {
        self.send(limit, &|agent| {
            let mut req = agent.get(url).header("User-Agent", user_agent);
            if let Some((start, end)) = range {
                req = req.header("Range", &format!("bytes={start}-{end}"));
            }
            req.config().max_redirects(0).build().call()
        })
    }

    #[cfg(feature = "fetch-youtube")]
    pub(crate) fn post_json(
        &mut self,
        url: &str,
        headers: &[(&str, String)],
        body: &serde_json::Value,
        limit: u64,
    ) -> Result<Ranged, HttpFault> {
        let text = body.to_string();
        self.send(limit, &|agent| {
            let mut req = agent.post(url).header("Content-Type", "application/json");
            for (k, v) in headers {
                req = req.header(*k, v);
            }
            req.config().max_redirects(0).build().send(text.as_str())
        })
    }

    fn send(
        &mut self,
        limit: u64,
        call: &dyn Fn(&ureq::Agent) -> Answer,
    ) -> Result<Ranged, HttpFault> {
        let (mut faults, mut busy) = (0u32, 0u32);
        loop {
            let fault = match call(&self.agent) {
                Ok(mut resp) => {
                    let status = resp.status().as_u16();
                    let header = |name: &str| {
                        resp.headers()
                            .get(name)
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_string)
                    };
                    if status == 429 && busy < 6 {
                        let wait = header("retry-after")
                            .and_then(|v| v.trim().parse::<u64>().ok())
                            .unwrap_or(5)
                            .max(if busy == 0 { 0 } else { 1 << busy })
                            .min(30);
                        std::thread::sleep(std::time::Duration::from_secs(wait));
                        busy += 1;
                        continue;
                    }
                    let (total, location, disposition) = (
                        header("content-range")
                            .and_then(|v| v.rsplit('/').next()?.trim().parse::<u64>().ok()),
                        header("location"),
                        header("content-disposition"),
                    );
                    // The body of a refusal is not used: read little of it.
                    let limit = if (200..300).contains(&status) {
                        limit
                    } else {
                        4096
                    };
                    let mut body = Vec::new();
                    match resp
                        .body_mut()
                        .as_reader()
                        .take(limit)
                        .read_to_end(&mut body)
                    {
                        Ok(_) => {
                            return Ok(Ranged {
                                status,
                                total,
                                body,
                                location,
                                disposition,
                            });
                        }
                        Err(_) => HttpFault::Read,
                    }
                }
                Err(ureq::Error::Timeout(_)) => HttpFault::Timeout,
                Err(_) => {
                    // A connect fault may be a missing IPv6 route: the next try goes over IPv4.
                    if crate::http_rows::force_ipv4() {
                        self.agent = agent();
                    }
                    HttpFault::Connect
                }
            };
            faults += 1;
            if faults >= 3 {
                return Err(fault);
            }
            std::thread::sleep(std::time::Duration::from_secs(1 << faults));
        }
    }
}

fn agent() -> ureq::Agent {
    // 600 s for one request with its body, as the other downloads of this crate: 120 s stopped an 8 MiB range or a
    // large HLS part on a slow line (below about 70 KB/s).
    crate::http_rows::agent_public(600, None).expect("no CA file to read")
}

/// A loopback test server, when `SCRIPTVOX_FETCH_BASE` names one (as `open_search::test_base`).
pub(crate) fn test_base() -> Option<String> {
    std::env::var("SCRIPTVOX_FETCH_BASE")
        .ok()
        .filter(|b| {
            b.strip_prefix("http://127.0.0.1")
                .is_some_and(|r| r.is_empty() || r.starts_with([':', '/']))
                // `host:port@other` would name another host.
                && !b.contains('@')
        })
        .map(|b| b.trim_end_matches('/').to_string())
}

/// True when `url` is on the test server.
pub(crate) fn under_test_base(url: &str) -> bool {
    test_base().is_some_and(|b| {
        url.strip_prefix(&b)
            .is_some_and(|r| r.is_empty() || r.starts_with(['/', '?']))
    })
}

/// An address that fetch may send a request to: `https` on a public host.
pub(crate) fn address_ok(url: &str) -> bool {
    under_test_base(url) || (url.starts_with("https://") && !local_host(url))
}

fn marker() -> PathBuf {
    crate::keys::config_dir().join("fetch-consent.json")
}

/// The fix line when a site asks this network to sign in. The second route always works, and it is the only one that
/// ScriptVox offers: no sign-in, no cookies of a browser, no way around the check of the site.
pub(crate) const SIGN_IN_FIX: &str = "Try again later or from another network. Or save the file yourself (a way that the site allows) and give the local file: `scriptvox analyze <file>`, `scriptvox make --music <file>` or `--media <folder>`.";

/// How long a plan says that a site asked this network to sign in.
const SIGN_IN_KEPT_S: u64 = 1800;

fn sign_in_file() -> PathBuf {
    crate::provision::data_dir().join("fetch-sign-in.txt")
}

/// The site of an address, for the sign-in note: its host, and one name for all the hosts of YouTube.
fn site(url: &str) -> String {
    if youtube_host(url) {
        return "youtube".into();
    }
    parts(url).map_or(String::new(), |(_, host, _, _)| host)
}

/// A download that the site refused with a sign-in check: the site is kept for a later plan.
fn keep_sign_in(url: &str, f: &Fail) {
    if f.code == "SVX_FETCH_BOT_CHECK" {
        let _ = std::fs::write(sign_in_file(), site(url));
    }
}

/// For `plan`: this site asked this network to sign in a short time ago, so the download will likely fail. Empty when
/// it did not. It reads one small file and sends nothing.
pub fn sign_in_note(url: &str) -> String {
    let file = sign_in_file();
    let fresh = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age.as_secs() < SIGN_IN_KEPT_S);
    if fresh && std::fs::read_to_string(&file).is_ok_and(|s| s == site(url)) {
        "; a short time ago this site asked this network to sign in, and ScriptVox does not sign in, so this download will likely fail: save the file yourself and give the local file as `music`".into()
    } else {
        String::new()
    }
}

/// The marker exists with the current notice number.
pub fn ready() -> bool {
    std::fs::read_to_string(marker())
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .is_some_and(|v| v["notice"].as_u64() == Some(NOTICE_NUMBER))
}

/// `setup fetch`: shows the notice and writes the marker. It downloads nothing.
pub fn setup(dry_run: bool, json: bool) -> Envelope {
    let file = marker();
    // The person must see the notice: the line for a person does not print the fields of the envelope. A JSON run
    // has it in the `notice` field, so it is not printed a second time.
    if !json {
        progress(NOTICE);
    }
    let env = Envelope::ok("setup")
        .with("target", json!("fetch"))
        .with("notice", json!(NOTICE))
        .with("marker", json!(file));
    if dry_run {
        return env.with("dry_run", json!(true));
    }
    let text = json!({"schema": "scriptvox.fetch-consent/v1", "notice": NOTICE_NUMBER,
                      "accepted_at": crate::run::now_utc()});
    let written = std::fs::create_dir_all(crate::keys::config_dir())
        .and_then(|()| std::fs::write(&file, format!("{text}\n")));
    match written {
        Ok(()) => env.with("fetch", json!("on")),
        Err(e) => Envelope::fail(
            "setup",
            Exit::Error,
            format!("cannot write {}: {e}", file.display()),
            Some("Check that you can write to the config folder, then run `scriptvox setup fetch` again.".into()),
        ),
    }
}

/// Scheme, host (lower case, no user part, no port), path, query.
pub(crate) fn parts(url: &str) -> Option<(&str, String, &str, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    let rest = rest.split('#').next().unwrap_or("");
    let (before, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (authority, path) = match before.find('/') {
        Some(i) => before.split_at(i),
        None => (before, "/"),
    };
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("").to_ascii_lowercase();
    Some((scheme, host, path, query))
}

const YOUTUBE_HOSTS: [&str; 7] = [
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
    "youtube-nocookie.com",
    "www.youtube-nocookie.com",
    "youtu.be",
];

/// True for an address on a YouTube host. The test costs no request.
pub(crate) fn youtube_host(url: &str) -> bool {
    parts(url).is_some_and(|(_, host, _, _)| YOUTUBE_HOSTS.contains(&host.as_str()))
}

/// Seconds (`90`, `90.5`) or a clock (`1:30`, `0:01:30.5`).
pub(crate) fn parse_time(s: &str) -> Option<f64> {
    let fields: Vec<&str> = s.trim().split(':').collect();
    if fields.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for (i, f) in fields.iter().enumerate() {
        let last = i + 1 == fields.len();
        let plain = !f.is_empty()
            && f.bytes().all(|b| b.is_ascii_digit() || (last && b == b'.'))
            && f.bytes().filter(|b| *b == b'.').count() <= 1;
        let v: f64 = f.parse().ok().filter(|_| plain)?;
        // In a clock, the minutes and seconds stay under 60.
        if i > 0 && v >= 60.0 {
            return None;
        }
        total = total * 60.0 + v;
    }
    Some(total)
}

/// `reference` (absolute, `//host`, `/path` or relative) as an address, read against `base`.
pub(crate) fn join_url(base: &str, reference: &str) -> Option<String> {
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }
    if reference.contains("://") && !reference.starts_with('/') {
        return Some(reference.to_string());
    }
    let (scheme, rest) = base.split_once("://")?;
    if let Some(r) = reference.strip_prefix("//") {
        return Some(format!("{scheme}://{r}"));
    }
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (authority, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => (rest, "/"),
    };
    let (ref_path, ref_tail) = match reference.find(['?', '#']) {
        Some(i) => reference.split_at(i),
        None => (reference, ""),
    };
    let joined = if ref_path.starts_with('/') {
        ref_path.to_string()
    } else {
        let folder = &path[..=path.rfind('/').unwrap_or(0)];
        format!("{folder}{ref_path}")
    };
    // `.` and `..` parts.
    let mut out: Vec<&str> = Vec::new();
    for p in joined.split('/').skip(1) {
        match p {
            "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    let slash = if joined.ends_with("/.") || joined.ends_with("/..") {
        "/"
    } else {
        ""
    };
    Some(format!(
        "{scheme}://{authority}/{}{slash}{ref_tail}",
        out.join("/")
    ))
}

/// The address with no query, no fragment and no user part: a query can hold a signed value.
pub fn public_url(url: &str) -> String {
    let plain = url.split(['?', '#']).next().unwrap_or("");
    match plain.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = match rest.find('/') {
                Some(i) => rest.split_at(i),
                None => (rest, ""),
            };
            let host = authority.rsplit('@').next().unwrap_or("");
            format!("{scheme}://{host}{path}")
        }
        None => plain.to_string(),
    }
}

/// The last path part of an address, as (name with no extension, extension in lower case).
pub(crate) fn last_part(url: &str) -> (String, String) {
    let plain = public_url(url);
    let path = plain.split_once("://").map_or("", |x| x.1);
    let name = match path.split_once('/') {
        Some((_, p)) => p.rsplit('/').find(|s| !s.is_empty()).unwrap_or(""),
        None => "",
    };
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= 5 => {
            (stem.to_string(), ext.to_ascii_lowercase())
        }
        _ => (name.to_string(), String::new()),
    }
}

/// A title as a file name: letters, digits and `-`, at most 60.
fn safe_name(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
        if out.chars().count() >= 60 {
            break;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "fetch".into()
    } else {
        out
    }
}

/// The `filename` of a `Content-Disposition` header, as a plain name.
fn disposition_name(header: &str) -> Option<String> {
    let v = header
        .split(';')
        .find_map(|f| f.trim().strip_prefix("filename="))?;
    let name = v.trim_matches('"').rsplit(['/', '\\']).next()?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

const KNOWN_EXT: [&str; 20] = [
    "mp4", "mov", "m4v", "m4a", "mkv", "mka", "webm", "avi", "ts", "wav", "mp3", "flac", "ogg",
    "opus", "aac", "png", "jpg", "jpeg", "webp", "bmp",
];

fn ext_of(plain: media_gate::Plain, url_ext: &str) -> String {
    if KNOWN_EXT.contains(&url_ext) {
        return url_ext.to_string();
    }
    match plain.format {
        "mov" => "mp4",
        "matroska" => "mkv",
        "mpegts" => "ts",
        "mjpeg" => "jpg",
        other => other,
    }
    .to_string()
}

pub(crate) fn progress(text: &str) {
    eprintln!("scriptvox: {text}");
}

/// Where the downloads and the part files are.
fn downloads() -> PathBuf {
    crate::provision::data_dir().join("downloads").join("fetch")
}

#[cfg(feature = "fetch-youtube")]
use crate::fetch_helper::{layers, source};

pub fn run(a: Args) -> Envelope {
    let checked = match check(&a) {
        Ok(c) => c,
        Err(f) => return f.envelope(),
    };
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("fetch", e),
    };
    get(&a, &ff, checked).unwrap_or_else(Fail::envelope)
}

/// The checks that cost no request: the marker, the times, the `--out` file and the address.
fn check(a: &Args) -> Result<(Window, Option<String>), Fail> {
    if !ready() {
        return Err(Fail::new(
            Exit::Usage,
            "SVX_FETCH_SETUP",
            "fetch is off.",
            "Run `scriptvox setup fetch` one time and read the notice.",
        ));
    }
    let window = window_of(a)?;
    if a.out.as_ref().is_some_and(|o| o.exists()) {
        return Err(Fail::usage(
            "The --out file is there already.",
            "Give --out a file name that does not exist.",
        ));
    }
    let url = a.url.trim();
    if !(url.starts_with("https://") || under_test_base(url)) {
        return Err(Fail::usage(
            "fetch reads https addresses only.",
            "Give an address that starts with https://.",
        ));
    }
    Ok((window, source(a, url)?))
}

/// The download and the FFmpeg step, after `check`: the envelope of the command, or why there is no file.
fn get(a: &Args, ff: &Ffmpeg, checked: (Window, Option<String>)) -> Result<Envelope, Fail> {
    let (window, id) = checked;
    let url = a.url.trim();
    let dir = downloads();
    std::fs::create_dir_all(&dir).map_err(|e| {
        Fail::new(
            Exit::Error,
            "SVX_ERROR",
            &format!("{}: {e}", dir.display()),
            "Check that you can write to the data folder and that the disk has free space.",
        )
    })?;
    let (fetched, window, helper) =
        layers(a, url, id, ff, window, &dir).inspect_err(|f| keep_sign_in(url, f))?;
    let files = fetched.files.clone();
    let env = finish(a, ff, fetched, window, &dir).map(|env| match helper {
        None => env.with("layer", json!("native")),
        Some(version) => env
            .with("layer", json!("helper"))
            .with("helper_version", json!(version)),
    });
    for f in files {
        let _ = std::fs::remove_file(f);
    }
    env
}

/// True for an address: `make` gets the sound of such a `music` value through fetch.
pub fn is_address(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

fn sound_args(url: &str, from: Option<&str>, to: Option<&str>, dir: Option<PathBuf>) -> Args {
    Args {
        url: url.into(),
        audio: true,
        from: from.map(String::from),
        to: to.map(String::from),
        out: None,
        dir,
        #[cfg(feature = "fetch-youtube")]
        quality: None,
        use_system: false,
    }
}

/// `make --music <address>`, before the plan: the checks of `fetch` that cost no request.
pub fn music_check(url: &str, from: Option<&str>, to: Option<&str>) -> Result<(), Fail> {
    check(&sound_args(url, from, to, None)).map(|_| ())
}

/// `make --music <address>`: the sound of the address (and its cut) in `<run>/fetch/`, with its source for the
/// credits. One download for an address and a cut: a later run takes the file from the store, as `open_search::pick`.
pub fn music(
    url: &str,
    from: Option<&str>,
    to: Option<&str>,
    run_dir: &Path,
    cache_only: bool,
    ff: &Ffmpeg,
) -> Result<(PathBuf, crate::make::StockPick), Fail> {
    let cas = crate::store::Cas::open();
    let ck = crate::store::key(
        "fetch",
        "music",
        &json!({"url": url, "from": from, "to": to}),
    );
    let out = run_dir.join("fetch");
    if let Some(files) = cas.get(&ck, &out) {
        let picked = files
            .iter()
            .find(|(n, _)| n == "pick.json")
            .and_then(|(_, p)| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok());
        if let (Some((_, file)), Some(p)) = (files.iter().find(|(n, _)| n != "pick.json"), picked) {
            return Ok((file.clone(), p));
        }
    }
    if cache_only {
        return Err(Fail::new(
            Exit::Error,
            "SVX_CACHE_MISS",
            "The music of this address is not in the cache.",
            "Use `scriptvox make --from <video>` to fetch it again.",
        ));
    }
    // A file of an earlier try that did not reach the store would stop the move of the new one.
    let _ = std::fs::remove_dir_all(&out);
    let cannot = |e: std::io::Error| {
        Fail::new(
            Exit::Error,
            "SVX_ERROR",
            &format!("{}: {e}", out.display()),
            "Check that you can write to the project folder.",
        )
    };
    std::fs::create_dir_all(&out).map_err(cannot)?;
    let a = sound_args(url, from, to, Some(out.clone()));
    let env = get(&a, ff, check(&a)?)?;
    let text = |k: &str| env.extra.get(k).and_then(|v| v.as_str()).map(String::from);
    let Some(file) = env.artifacts.get("audio").and_then(|v| v.as_str()) else {
        let _ = std::fs::remove_dir_all(&out);
        return Err(Fail::usage(
            "This address gives a picture, not sound.",
            "Give the address of a video or a sound file.",
        ));
    };
    let file = PathBuf::from(file);
    let licence = text("licence");
    // Rule of the milestone: fetched material is not cleared unless its licence field states a free licence.
    let terms = licence.as_deref().and_then(crate::http_rows::file_terms);
    let source_url = text("source_url").unwrap_or_default();
    let picked = crate::make::StockPick {
        provider: "fetch".into(),
        id: ck[..12].to_string(),
        url: source_url.clone(),
        credit: text("uploader").unwrap_or_default(),
        page: source_url,
        terms: Some(crate::http_rows::Terms {
            commercial: Some(terms.as_ref().is_some_and(|t| t.commercial == Some(true))),
            attribution_required: true,
            redistribution: false,
            note: terms.map_or(
                "no licence stated; the user gave this address".into(),
                |t| t.note,
            ),
        }),
        title: text("title"),
        licence,
        layer: text("layer"),
    };
    let side = out.join("pick.json");
    std::fs::write(&side, serde_json::to_vec(&picked).unwrap_or_default()).map_err(cannot)?;
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let _ = cas.put(
        &ck,
        "fetch",
        "music",
        &[(name.as_str(), &file), ("pick.json", &side)],
        json!(picked.terms),
    );
    Ok((file, picked))
}

/// The checks of an address that cost no request. This build reads no site by its name, so the answer is never an id.
#[cfg(not(feature = "fetch-youtube"))]
fn source(_a: &Args, url: &str) -> Result<Option<String>, Fail> {
    if youtube_host(url) {
        return Err(Fail::no_site());
    }
    if !address_ok(url) {
        return Err(Fail::private());
    }
    Ok(None)
}

/// The file, the cut that FFmpeg must still make, and the version of a helper (none in this build).
#[cfg(not(feature = "fetch-youtube"))]
fn layers(
    a: &Args,
    url: &str,
    _id: Option<String>,
    _ff: &Ffmpeg,
    window: Window,
    dir: &Path,
) -> Result<(Fetched, Window, Option<String>), Fail> {
    other(&mut Http::new(), url, a.audio, window, dir).map(|f| (f, window, None))
}

/// `--from` and `--to` as seconds. A side that was not given is open: 0, or no end.
fn window_of(a: &Args) -> Result<Window, Fail> {
    let read = |flag: &str, v: &Option<String>| match v {
        None => Ok(None),
        Some(s) => parse_time(s).map(Some).ok_or_else(|| {
            Fail::usage(
                &format!("{flag} is not a time."),
                "Give seconds (90.5) or a clock (1:30).",
            )
        }),
    };
    let (from, to) = (read("--from", &a.from)?, read("--to", &a.to)?);
    if from.zip(to).is_some_and(|(f, t)| f >= t) {
        return Err(Fail::usage(
            "--from is not less than --to.",
            "Give a --from time that is before the --to time.",
        ));
    }
    Ok((from.is_some() || to.is_some()).then(|| (from.unwrap_or(0.0), to)))
}

/// The probe of spec 3.1 step 4, then the direct file or the playlist.
pub(crate) fn other(
    http: &mut Http,
    url: &str,
    audio: bool,
    window: Option<(f64, Option<f64>)>,
    dir: &Path,
) -> Result<Fetched, Fail> {
    let mut at = url.to_string();
    let mut hops = 0;
    let probe = loop {
        let r = http.get(&at, USER_AGENT, Some((0, 1023)), 1024)?;
        if !(300..400).contains(&r.status) {
            break r;
        }
        hops += 1;
        let next = r.location.as_deref().and_then(|l| join_url(&at, l));
        match next {
            Some(n) if hops <= 5 && address_ok(&n) => at = n,
            Some(n) if hops <= 5 && n.starts_with("https://") => return Err(Fail::private()),
            _ => {
                return Err(Fail::unsupported(
                    "The address redirects to one that fetch does not read.",
                    "Give the https address of the media file itself.",
                ))
            }
        }
    };
    if !(probe.status == 200 || probe.status == 206) {
        return Err(Fail::refused(probe.status));
    }
    let head = &probe.body;
    let text = String::from_utf8_lossy(&head[..head.len().min(64)]).to_ascii_lowercase();
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let plain = match media_gate::sniff(head) {
        Ok(p) => p,
        Err(_) if text.starts_with("#extm3u") => {
            // The parts are read against `at`. What the user sees comes from the address that the user gave.
            let mut f = crate::fetch_hls::fetch(http, &at, head, audio, window, dir)?;
            f.source_url = public_url(url);
            f.title = last_part(url).0;
            return Ok(f);
        }
        Err(_)
            if String::from_utf8_lossy(head)
                .to_ascii_lowercase()
                .contains("<mpd") =>
        {
            return Err(Fail::unsupported(
                "A DASH manifest (.mpd) is not supported yet.",
                "Give the address of the media file itself, or of the HLS playlist (.m3u8) if the site has one.",
            )
            .or_helper());
        }
        Err(_) => {
            return Err(Fail::no_site().or_helper());
        }
    };
    if probe.total.is_some_and(|t| t > MAX_FILE) {
        return Err(Fail::usage(
            "The file is above the limit of 4 GB.",
            "Give the address of a smaller file.",
        ));
    }
    let (stem, url_ext) = last_part(&at);
    let hash = blake3::hash(at.as_bytes()).to_hex()[..8].to_string();
    let dest = dir.join(format!("direct-{hash}.{}", ext_of(plain, &url_ext)));
    if let Some(t) = probe.total {
        progress(&format!("downloading {t} bytes"));
    }
    crate::http_rows::download_no_redirect(&at, dir, &dest, None, true).map_err(|e| {
        // The text of the HTTP library is not passed on: only the status, or a short cause.
        match e.strip_prefix("download answered HTTP ").map(str::parse) {
            Some(Ok(status)) => Fail::refused(status),
            _ if e.starts_with("download broke") => Fail::stopped("read"),
            _ if e.starts_with("download failed") => Fail::stopped("connect"),
            // The rest are errors of the local disk. `e` holds a file error, not an address.
            _ => Fail::new(
                Exit::Error,
                "SVX_ERROR",
                &format!("cannot write the download: {e}"),
                "Check the free space of the data folder.",
            ),
        }
    })?;
    gate(&dest)?;
    let title = probe
        .disposition
        .as_deref()
        .and_then(disposition_name)
        .map(|n| n.rsplit_once('.').map_or(n.clone(), |x| x.0.to_string()))
        .unwrap_or(stem);
    Ok(Fetched {
        files: vec![dest],
        starts: vec![0.0],
        kind: "direct",
        source_url: public_url(url),
        title,
        id: None,
        uploader: None,
        duration_s: None,
        licence: None,
        findings: vec![],
    })
}

/// The media gate on a downloaded file. A file that is not plain media is removed.
pub(crate) fn gate(file: &Path) -> Result<media_gate::Plain, Fail> {
    media_gate::downloaded("The server", file).map_err(|why| {
        Fail::new(
            Exit::Usage,
            "SVX_FETCH_NOT_MEDIA",
            &why,
            "Give the address of the media file itself.",
        )
    })
}

/// The codec of the first sound stream, from ffprobe.
fn audio_codec(ff: &Ffmpeg, file: &Path) -> Option<String> {
    let gate = media_gate::input(file).ok()?;
    let o = std::process::Command::new(&ff.ffprobe)
        .args(["-v", "error", "-select_streams", "a:0"])
        .args(["-show_entries", "stream=codec_name", "-of", "csv=p=0"])
        .args(gate)
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// Step 6 and 7 of the flow: the audio copy, the cut or the join (one FFmpeg run, stream copy), then the move to
/// `--out` and the envelope.
fn finish(
    a: &Args,
    ff: &Ffmpeg,
    f: Fetched,
    window: Option<(f64, Option<f64>)>,
    dir: &Path,
) -> Result<Envelope, Fail> {
    let first = &f.files[0];
    let plain = gate(first)?;
    let picture = plain.kind == Media::Picture;
    let has_video = !picture && ff.has_stream(first, "video");
    let sound_only = a.audio || !has_video;
    let in_ext = first
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_string();
    // FFmpeg runs for a join, a cut, an audio copy out of a video, and always for HLS parts.
    let work = !picture
        && (f.files.len() > 1 || window.is_some() || f.kind == "hls" || (a.audio && has_video));
    let ext = if !work {
        in_ext
    } else if sound_only && (has_video || f.kind == "hls") {
        let codec = f.files.iter().rev().find_map(|x| audio_codec(ff, x));
        match codec.as_deref() {
            None => {
                return Err(Fail::usage(
                    "This file has no sound, so --audio gives nothing.",
                    "Run the command again with no --audio.",
                ))
            }
            Some("aac") => "m4a",
            Some("opus" | "vorbis") => "ogg",
            Some("mp3") => "mp3",
            Some(_) => "mka",
        }
        .to_string()
    } else if f.kind == "direct" {
        in_ext
    } else {
        "mp4".to_string()
    };
    let made = if work {
        let mut args: Vec<String> = Vec::new();
        for (file, start) in f.files.iter().zip(&f.starts) {
            if let Some((from, to)) = window {
                // Input options: the times are read from the start of each file.
                args.extend(["-ss".into(), format!("{:.3}", (from - start).max(0.0))]);
                if let Some(to) = to {
                    args.extend(["-to".into(), format!("{:.3}", (to - start).max(0.0))]);
                }
            }
            args.extend(["-i".into(), file.display().to_string()]);
        }
        let sound = f.files.len() - 1;
        if sound_only {
            args.extend(["-vn".into(), "-map".into(), format!("{sound}:a:0")]);
        } else if sound > 0 {
            args.extend(["-map", "0:v:0", "-map", "1:a:0"].map(String::from));
        }
        args.extend(["-c", "copy", "-avoid_negative_ts", "make_zero"].map(String::from));
        let tmp = dir.join(format!("out-{}.{ext}", std::process::id()));
        args.push(tmp.display().to_string());
        ff.run(dir, &args, None).map_err(|_| {
            let _ = std::fs::remove_file(&tmp);
            Fail::new(
                Exit::Error,
                "SVX_ERROR",
                "FFmpeg could not copy the streams of this file into the output.",
                "Run the command again with no --audio, --from and --to to get the file as it is.",
            )
        })?;
        tmp
    } else {
        first.clone()
    };
    // A source that gives its length may have read only the start of the file, up to a size from the mean bit rate.
    // When the start of the file has more bytes per second than the mean, the cut is short: that must not pass as a
    // good result.
    if let (Some((from, Some(to))), Some(length)) = (window, f.duration_s) {
        let want = to.min(length) - from;
        if ff.duration(&made).is_ok_and(|d| d < want - 1.0) {
            let _ = std::fs::remove_file(&made);
            return Err(Fail::new(
                Exit::Error,
                "SVX_ERROR",
                "The cut is shorter than the times that you gave: too few bytes of the file were downloaded.",
                "Run the command again with --from only; the whole file is then downloaded.",
            ));
        }
    }
    let hash = blake3::hash(f.source_url.as_bytes()).to_hex()[..8].to_string();
    let out = a.out.clone().unwrap_or_else(|| {
        a.dir.clone().unwrap_or_default().join(format!(
            "{}-{}.{ext}",
            safe_name(&f.title),
            f.id.as_deref().unwrap_or(&hash)
        ))
    });
    if out.exists() {
        let _ = std::fs::remove_file(&made);
        return Err(Fail::usage(
            &format!("{} is there already.", out.display()),
            "Give --out a file name that does not exist.",
        ));
    }
    // The data folder and the output may be on different disks.
    if std::fs::rename(&made, &out).is_err() {
        let copied = std::fs::copy(&made, &out);
        let _ = std::fs::remove_file(&made);
        copied.map_err(|e| {
            Fail::new(
                Exit::Error,
                "SVX_ERROR",
                &format!("cannot write {}: {e}", out.display()),
                "Give --out a file in a folder that you can write to.",
            )
        })?;
    }
    let out = std::path::absolute(&out).unwrap_or(out);
    let mut env = Envelope::ok("fetch");
    let key = if picture {
        "image"
    } else if sound_only {
        "audio"
    } else {
        "video"
    };
    env.artifacts.insert(key.into(), json!(out));
    if let Ok(m) = std::fs::metadata(&out) {
        env.metrics.insert("bytes".into(), json!(m.len()));
    }
    if !picture {
        if let Ok(d) = ff.duration(&out) {
            env.metrics
                .insert("duration_s".into(), json!((d * 1000.0).round() / 1000.0));
        }
    }
    if key == "video" {
        if let Some((w, h)) = ff.shown_size(&out) {
            env.metrics.insert("width".into(), json!(w));
            env.metrics.insert("height".into(), json!(h));
        }
    }
    for m in &f.findings {
        // `detail` is the field that the line for a person reads.
        env.findings
            .push(json!({"gate": "quality", "status": "warn", "message": m, "detail": m}));
    }
    Ok(env
        .with("source_kind", json!(f.kind))
        .with("source_url", json!(f.source_url))
        .with("title", json!(f.title))
        .with("uploader", json!(f.uploader))
        .with("licence", json!(f.licence)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_takes_seconds_and_clocks() {
        for (s, want) in [
            ("90", Some(90.0)),
            ("90.5", Some(90.5)),
            ("1:30", Some(90.0)),
            ("0:01:30.5", Some(90.5)),
            ("2:00:00", Some(7200.0)),
            ("0", Some(0.0)),
            ("", None),
            ("-5", None),
            ("1:75", None),
            ("1:2:3:4", None),
            ("1.5:30", None),
            ("abc", None),
            ("1e3", None),
            ("inf", None),
        ] {
            assert_eq!(parse_time(s), want, "{s}");
        }
    }

    #[test]
    fn join_url_resolves_each_form() {
        let base = "https://cdn.example.org/a/b/list.m3u8?sig=1";
        for (r, want) in [
            (
                "https://other.example.org/x.ts",
                "https://other.example.org/x.ts",
            ),
            ("//other.example.org/x.ts", "https://other.example.org/x.ts"),
            ("/root/x.ts", "https://cdn.example.org/root/x.ts"),
            ("x.ts", "https://cdn.example.org/a/b/x.ts"),
            ("x.ts?k=2", "https://cdn.example.org/a/b/x.ts?k=2"),
            ("./c/x.ts", "https://cdn.example.org/a/b/c/x.ts"),
            ("../x.ts", "https://cdn.example.org/a/x.ts"),
            ("../../../x.ts", "https://cdn.example.org/x.ts"),
        ] {
            assert_eq!(join_url(base, r).as_deref(), Some(want), "{r}");
        }
        assert_eq!(
            join_url("https://example.org", "x.ts").as_deref(),
            Some("https://example.org/x.ts")
        );
        assert_eq!(join_url(base, " "), None);
        assert_eq!(join_url("no scheme", "x.ts"), None);
    }

    #[test]
    fn public_url_drops_the_query_and_the_user_part() {
        for (u, want) in [
            (
                "https://a.example.org/v/x.mp4?sig=abc#t=1",
                "https://a.example.org/v/x.mp4",
            ),
            (
                "https://user:pw@a.example.org/x.mp4",
                "https://a.example.org/x.mp4",
            ),
            ("https://a.example.org", "https://a.example.org"),
            ("https://a.example.org?x=1", "https://a.example.org"),
        ] {
            assert_eq!(public_url(u), want, "{u}");
        }
    }

    #[test]
    fn fetch_names_come_from_the_address_and_the_title() {
        assert_eq!(
            last_part("https://a.example.org/v/My.Clip.MP4?x=1"),
            ("My.Clip".into(), "mp4".into())
        );
        assert_eq!(
            last_part("https://a.example.org/"),
            (String::new(), String::new())
        );
        assert_eq!(
            safe_name("Big Buck Bunny: 60fps / 4K!"),
            "big-buck-bunny-60fps-4k"
        );
        assert_eq!(safe_name("../../etc"), "etc");
        assert_eq!(safe_name("???"), "fetch");
        assert!(safe_name(&"a".repeat(200)).len() <= 60);
        assert_eq!(
            disposition_name("attachment; filename=\"../clip one.mp4\"").as_deref(),
            Some("clip one.mp4")
        );
        assert_eq!(disposition_name("inline"), None);
    }
}
