//! Declarative HTTP provider rows (PLAN-R1 D3): `registry/providers.toml`,
//! run in Rust. v0.1 rows: Pexels and Pixabay stock video. Ideas from
//! MoneyPrinterTurbo `app/services/material.py` (MIT): script-order search
//! terms, and a Range header to resume an interrupted download.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const REGISTRY: &str = include_str!("../../../registry/providers.toml");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Terms {
    /// `None`: not checked. Data only: `providers`, the receipt and `metadata.json` show it.
    #[serde(default)]
    pub commercial: Option<bool>,
    pub attribution_required: bool,
    pub redistribution: bool,
    pub note: String,
}

/// The one word for a row's terms: data for the user, nothing is refused on it.
pub fn business_use(commercial: Option<bool>) -> &'static str {
    match commercial {
        Some(true) => "Business use yes",
        Some(false) => "Personal use only",
        None => "Not confirmed",
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Row {
    pub name: String,
    pub kind: String,
    pub key: String,
    pub search_url: String,
    pub auth_header: Option<String>,
    pub results: String,
    pub id: String,
    pub duration: String,
    pub files: String,
    pub file_url: String,
    pub file_w: String,
    pub file_h: String,
    pub credit_name: String,
    pub credit_url: String,
    pub site: String,
    pub terms: Terms,
    /// Hosts a clip URL may use (the host or a subdomain of it).
    #[serde(default)]
    pub hosts: Vec<String>,
    /// Path to the licence of one result. Its terms replace `terms`; a result with no licence text is dropped.
    pub license: Option<String>,
    /// The end of a file URL, in place of `.mp4`.
    pub file_ext: Option<String>,
    /// A result longer than this many seconds is not downloaded: size is not in the search answer.
    pub max_duration: Option<f64>,
    /// A second request per result, `{id}` in the URL, whose answer holds `files`.
    pub detail_url: Option<String>,
    /// Path on a file to its seconds, where the result has none.
    pub file_duration: Option<String>,
    /// Goes before `file_url`, `{id}` in it, when the answer gives only a file name.
    pub file_base: Option<String>,
    /// Goes before `credit_url`.
    pub credit_base: Option<String>,
    /// Path to a result's search rank, where the answer is not in rank order (Commons).
    pub rank: Option<String>,
}

impl Row {
    pub fn keyless(&self) -> bool {
        self.key.is_empty()
    }
    /// Can run now: no key needed, or the key is set.
    pub fn ready(&self) -> bool {
        self.keyless() || crate::keys::resolve(&self.key).is_some()
    }
    pub fn service(&self) -> String {
        if self.keyless() {
            "free external API, no key".into()
        } else {
            format!(
                "free external API, your own key {}",
                crate::keys::env_var(&self.key).unwrap_or("not set up".into())
            )
        }
    }
}

#[derive(Deserialize)]
struct Registry {
    provider: Vec<Row>,
}

/// The rows. `SCRIPTVOX_PROVIDERS` points at another file (tests, mirrors).
pub fn rows() -> Vec<Row> {
    let text = std::env::var_os("SCRIPTVOX_PROVIDERS")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_else(|| REGISTRY.to_string());
    toml::from_str::<Registry>(&text)
        .map(|r| r.provider)
        .unwrap_or_default()
}

/// Reads `a.b`, `a[*]` and `a.*` paths. Every match is returned.
pub fn get<'a>(v: &'a Value, path: &str) -> Vec<&'a Value> {
    let mut cur = vec![v];
    for seg in path.split('.') {
        let (name, all) = match seg.strip_suffix("[*]") {
            Some(n) => (n, true),
            None => (seg, false),
        };
        cur = cur
            .into_iter()
            .flat_map(|x| -> Vec<&Value> {
                if name == "*" {
                    return x
                        .as_object()
                        .map(|o| o.values().collect())
                        .unwrap_or_default();
                }
                let child = if name.is_empty() {
                    Some(x)
                } else {
                    x.get(name)
                };
                match (child, all) {
                    (Some(Value::Array(a)), true) => a.iter().collect(),
                    (Some(c), false) => vec![c],
                    _ => vec![],
                }
            })
            .collect();
    }
    cur
}

fn text(v: &Value, path: &str) -> String {
    get(v, path)
        .first()
        .map(|x| {
            let x = x.get(0).unwrap_or(x);
            x.as_str()
                .map(String::from)
                .unwrap_or_else(|| x.to_string())
        })
        .unwrap_or_default()
}

fn num(v: &Value, path: &str) -> f64 {
    get(v, path)
        .first()
        .and_then(|x| x.as_f64().or_else(|| x.as_str()?.parse().ok()))
        .unwrap_or(0.0)
}

/// Commons gives the author as HTML. Only the text goes on a credit line.
fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    // The common entities, so a credit reads "CSU & NOAA", not "CSU &amp; NOAA" (seen on a real run).
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// The terms of one file from its licence name or URL (Commons, Archive.org). Every stated licence is kept, the user
/// decides (owner, 7 Nov 2026): no-derivatives and non-commercial files carry a note and `commercial = false`; a licence
/// this code cannot read carries "licence not read" and `commercial = null`. `None` only when no licence is stated.
pub fn file_terms(license: &str) -> Option<Terms> {
    if license.trim().is_empty() {
        return None;
    }
    let low = license.to_ascii_lowercase();
    let t: Vec<&str> = low.split(|c: char| !c.is_ascii_alphanumeric()).collect();
    let has = |w: &str| t.contains(&w);
    let mut note = format!("{license}, as its uploader states");
    let free = has("cc0") || has("publicdomain") || (has("public") && has("domain"));
    if !free && !has("by") {
        note.push_str("; licence not read: check it before you share");
        return Some(Terms {
            commercial: None,
            attribution_required: true,
            redistribution: false,
            note,
        });
    }
    let (nc, nd) = (has("nc"), has("nd"));
    if !free {
        note.push_str("; credit the author");
    }
    if has("sa") {
        note.push_str("; the finished video takes the same licence");
    }
    if nd {
        // `make` cuts and captions every clip, so the video is an edited copy.
        note.push_str("; personal use only, do not share an edited copy");
    } else if nc {
        note.push_str("; not for commercial use");
    }
    Some(Terms {
        commercial: Some(!nc && !nd),
        attribution_required: !free,
        redistribution: false,
        note,
    })
}

/// Public APIs (Wikimedia) ask for a User-Agent that says where to reach the client.
pub const USER_AGENT: &str = concat!(
    "scriptvox/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/scriptvox/scriptvox)"
);

/// Set after a connect fault: from then on this process connects over IPv4 only. A host with an AAAA record and no
/// IPv6 route gave "Network is unreachable" (audit H4), and curl -4 reached it.
static IPV4_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One GET that must answer 200 with JSON. A connect fault is tried once more over IPv4.
fn get_json(row: &Row, url: &str, key: &str) -> Result<Value, HttpErr> {
    use std::sync::atomic::Ordering::Relaxed;
    match get_json_once(row, url, key) {
        Err(e) if e.status.is_none() && !IPV4_ONLY.swap(true, Relaxed) => {
            get_json_once(row, url, key).map_err(|e2| HttpErr {
                status: e2.status,
                message: format!("{} (then over IPv4: {})", e.message, e2.message),
            })
        }
        r => r,
    }
}

fn get_json_once(row: &Row, url: &str, key: &str) -> Result<Value, HttpErr> {
    let mut req = agent().get(url).header("User-Agent", USER_AGENT);
    if let Some(h) = &row.auth_header {
        req = req.header(h, key);
    }
    let mut resp = req.call().map_err(|e| HttpErr {
        status: None,
        message: format!("{}: {e}", row.name),
    })?;
    let status = resp.status().as_u16();
    if status != 200 {
        return Err(HttpErr {
            status: Some(status),
            message: format!("{} search answered HTTP {status}", row.name),
        });
    }
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    serde_json::from_str(&body).map_err(|e| HttpErr {
        status: Some(status),
        message: format!("{} answered bad JSON: {e}", row.name),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct Clip {
    pub provider: String,
    pub id: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub credit: String,
    pub page: String,
    pub terms: Terms,
}

#[derive(Debug)]
pub struct HttpErr {
    pub status: Option<u16>,
    pub message: String,
}

impl HttpErr {
    /// Rate limits and server errors move on to the next provider.
    pub fn fallback(&self) -> bool {
        self.status.is_none_or(|s| s == 429 || s >= 500)
    }
}

pub fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// For a URL path: a space must be `%20` there, and `/` stays.
pub fn enc_path(s: &str) -> String {
    s.split('/')
        .map(|p| enc(p).replace('+', "%20"))
        .collect::<Vec<_>>()
        .join("/")
}

/// After a connect fault: true the first time, and from then on this process connects over IPv4 only.
pub fn force_ipv4() -> bool {
    !IPV4_ONLY.swap(true, std::sync::atomic::Ordering::Relaxed)
}

fn agent() -> ureq::Agent {
    agent_ca(60, None).expect("no CA file to read")
}

/// An agent with `timeout_s`. With `ca`, it trusts only the certificates in `<config>/ca/<ca>.pem`.
pub fn agent_ca(timeout_s: u64, ca: Option<&str>) -> Result<ureq::Agent, String> {
    Ok(config_ca(timeout_s, ca)?.into())
}

/// `agent_ca` whose connections go only to public addresses: the name is resolved once, and the request is refused
/// when an answer is a private address. The address that is checked is the address that is used, so a name that
/// changes its answer after a first check (DNS rebinding) cannot reach a host inside the network. Through a proxy,
/// the proxy resolves the name, so the check is the one on the text of the address only.
pub fn agent_public(timeout_s: u64, ca: Option<&str>) -> Result<ureq::Agent, String> {
    Ok(ureq::Agent::with_parts(
        config_ca(timeout_s, ca)?,
        ureq::unversioned::transport::DefaultConnector::new(),
        PublicResolver,
    ))
}

#[derive(Debug)]
struct PublicResolver;

impl ureq::unversioned::resolver::Resolver for PublicResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: ureq::unversioned::transport::NextTimeout,
    ) -> Result<ureq::unversioned::resolver::ResolvedSocketAddrs, ureq::Error> {
        let addrs = ureq::unversioned::resolver::DefaultResolver::default()
            .resolve(uri, config, timeout)?;
        // A loopback test server named by its exact base in a test variable.
        let text = uri.to_string();
        let test = crate::fetch::under_test_base(&text) || crate::research::test_url(&text);
        if !test && config.proxy().is_none() && addrs.iter().any(|a| local_ip(a.ip())) {
            return Err(ureq::Error::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "the host resolves to a private address",
            )));
        }
        Ok(addrs)
    }
}

fn config_ca(timeout_s: u64, ca: Option<&str>) -> Result<ureq::config::Config, String> {
    let mut b = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(timeout_s)))
        .http_status_as_error(false);
    if IPV4_ONLY.load(std::sync::atomic::Ordering::Relaxed) {
        b = b.ip_family(ureq::config::IpFamily::Ipv4Only);
    }
    if let Some(name) = ca {
        use ureq::tls::{parse_pem, PemItem, RootCerts, TlsConfig};
        let path = crate::keys::config_dir()
            .join("ca")
            .join(format!("{name}.pem"));
        let pem = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let certs: Vec<_> = parse_pem(&pem)
            .filter_map(|i| match i {
                Ok(PemItem::Certificate(c)) => Some(c),
                _ => None,
            })
            .collect();
        if certs.is_empty() {
            return Err(format!("{} holds no certificate", path.display()));
        }
        b = b.tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::new_with_certs(&certs))
                .build(),
        );
    }
    Ok(b.build())
}

/// Searches one row. Clips come back with the file closest above the short
/// side `min_side` (else the largest), MP4 only.
pub fn search(
    row: &Row,
    key: &str,
    query: &str,
    orientation: &str,
    min_side: u32,
) -> Result<Vec<Clip>, HttpErr> {
    let url = row
        .search_url
        .replace("{q}", &enc(query))
        .replace("{orientation}", orientation)
        .replace("{key}", &enc(key));
    let v = get_json(row, &url, key)?;
    let mut results = get(&v, &row.results);
    if let Some(r) = &row.rank {
        results.sort_by(|a, b| num(a, r).total_cmp(&num(b, r)));
    }
    let mut clips: Vec<Clip> = results
        .into_iter()
        .filter_map(|item| {
            let raw_id = text(item, &row.id);
            let terms = match &row.license {
                Some(p) => file_terms(&text(item, p))?,
                None => row.terms.clone(),
            };
            // One request per result, so a row with `detail_url` must ask for few results.
            let detail = match &row.detail_url {
                Some(u) => get_json(row, &u.replace("{id}", &enc_path(&raw_id)), key).ok(),
                None => None,
            };
            let base = row
                .file_base
                .as_deref()
                .map(|b| b.replace("{id}", &enc_path(&raw_id)));
            let mut files: Vec<(u32, u32, String, f64)> =
                get(detail.as_ref().unwrap_or(item), &row.files)
                    .into_iter()
                    .map(|f| {
                        let name = text(f, &row.file_url);
                        (
                            num(f, &row.file_w) as u32,
                            num(f, &row.file_h) as u32,
                            match &base {
                                Some(b) => format!("{b}{}", enc_path(&name)),
                                None => name,
                            },
                            row.file_duration.as_deref().map_or(0.0, |p| num(f, p)),
                        )
                    })
                    .filter(|(w, h, u, _)| {
                        *w > 0
                            && *h > 0
                            && u.starts_with("http")
                            && match &row.file_ext {
                                Some(e) => u.ends_with(e.as_str()),
                                None => u.contains(".mp4") || !u.contains('.'),
                            }
                    })
                    .collect();
            files.sort_by_key(|(w, h, _, _)| (*w).min(*h));
            files.retain(|(_, _, u, _)| url_allowed(u, &row.hosts));
            let pick = files
                .iter()
                .find(|(w, h, _, _)| (*w).min(*h) >= min_side)
                .or(files.last())?
                .clone();
            let duration = if row.file_duration.is_some() {
                pick.3
            } else {
                num(item, &row.duration)
            };
            if row
                .max_duration
                .is_some_and(|m| duration <= 0.0 || duration > m)
            {
                return None;
            }
            let credit = strip_tags(&text(item, &row.credit_name));
            Some(Clip {
                provider: row.name.clone(),
                // An id goes in a file name, and Archive.org ids may hold dots.
                id: if safe_token(&raw_id) {
                    raw_id
                } else {
                    blake3::hash(raw_id.as_bytes()).to_hex()[..16].to_string()
                },
                url: pick.2,
                width: pick.0,
                height: pick.1,
                duration,
                credit: if credit.is_empty() {
                    "an unnamed author".into()
                } else {
                    credit
                },
                page: format!(
                    "{}{}",
                    row.credit_base.as_deref().unwrap_or(""),
                    text(item, &row.credit_url)
                ),
                terms,
            })
        })
        .collect();
    // A row with no orientation filter: the frame's shape, then the size. Terms play no part (they are data).
    clips.sort_by_key(|c| {
        (
            match orientation {
                "portrait" => c.height <= c.width,
                "landscape" => c.width <= c.height,
                _ => false,
            },
            c.width.min(c.height) < min_side,
        )
    });
    Ok(clips)
}

/// True when `url` is https (or http on loopback, for tests) on one of `hosts`.
pub fn url_allowed(url: &str, hosts: &[String]) -> bool {
    let (scheme, rest) = match url.split_once("://") {
        Some(x) => x,
        None => return false,
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    let host = host.split(':').next().unwrap_or("").to_ascii_lowercase();
    let loopback = host == "127.0.0.1" || host == "localhost";
    (scheme == "https" || (scheme == "http" && loopback))
        && hosts
            .iter()
            .any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

/// True when an `https://` URL names a machine that is not on the public internet by its form: an address in a
/// private, loopback or link-local range, or a name with no dot or under `.local`, `.localhost`, `.internal`, `.lan`
/// or `.home.arpa`. A result URL comes from a service's answer, so it is not trusted text: without this a connected
/// service could make the user's machine fetch from a host inside the user's own network. A public name that
/// resolves to a private address is not caught here.
pub fn local_host(url: &str) -> bool {
    let rest = url.split_once("://").map_or("", |x| x.1);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    // [v6]:port, v4:port or name:port
    let (host, port) = match host.strip_prefix('[') {
        Some(v6) => (
            v6.split(']').next().unwrap_or(""),
            v6.split_once("]:").map(|x| x.1),
        ),
        None => (host.split(':').next().unwrap_or(""), host.split(':').nth(1)),
    };
    // A name may end in one dot ("host.internal."): the same host.
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return local_ip(ip);
    }
    // Forms a resolver can read as an address though they are not the plain one: "127.1", "0x7f.0.0.1",
    // "017700000001", "2130706433". No public name ends in a label of digits, and none has a label that starts "0x".
    let numeric = |l: &str| !l.is_empty() && l.chars().all(|c| c.is_ascii_digit());
    if host.is_empty()
        || !host.contains('.')
        || host.rsplit('.').next().is_some_and(numeric)
        || host.split('.').any(|l| l.starts_with("0x"))
        || [".local", ".localhost", ".internal", ".lan", ".home.arpa"]
            .iter()
            .any(|t| host.ends_with(t))
    {
        return true;
    }
    // A public name that points at a machine inside a network: ask the resolver and look at every address. A name
    // that does not resolve is not called local here; the download then fails by itself. The downloads of fetch, of
    // research and of a result URL connect through `agent_public`, which checks the address it connects to.
    let port: u16 = port.and_then(|p| p.parse().ok()).unwrap_or(443);
    std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), port))
        .is_ok_and(|mut addrs| addrs.any(|a| local_ip(a.ip())))
}

/// An address in a private, loopback, link-local, shared, reserved, multicast or unspecified range, also when it is
/// carried inside an IPv6 form (mapped, compatible, NAT64 64:ff9b::/96 and 64:ff9b:1::/48, 6to4 2002::/16).
fn local_ip(ip: std::net::IpAddr) -> bool {
    use std::net::{IpAddr, Ipv4Addr};
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            a.is_private()
                || a.is_loopback()
                || a.is_link_local()
                || a.is_unspecified()
                || a.is_broadcast()
                || a.is_multicast()
                || o[0] == 0 // 0.0.0.0/8
                || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64.0.0/10, shared address space
                || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0.0/24
                || (o[0] == 198 && (o[1] & 0xfe) == 18) // 198.18.0.0/15
                || o[0] >= 240 // 240.0.0.0/4
        }
        IpAddr::V6(a) => {
            let g = a.segments();
            let v4 = |hi: u16, lo: u16| {
                local_ip(IpAddr::V4(Ipv4Addr::new(
                    (hi >> 8) as u8,
                    hi as u8,
                    (lo >> 8) as u8,
                    lo as u8,
                )))
            };
            a.is_loopback()
                || a.is_unspecified()
                || a.is_multicast()
                || (g[0] & 0xfe00) == 0xfc00
                || (g[0] & 0xffc0) == 0xfe80
                // ::a.b.c.d (compatible) and ::ffff:a.b.c.d (mapped)
                || (g[..5] == [0; 5] && (g[5] == 0 || g[5] == 0xffff) && v4(g[6], g[7]))
                || (g[0] == 0x64 && g[1] == 0xff9b && g[2..6] == [0; 4] && v4(g[6], g[7]))
                || (g[0] == 0x64 && g[1] == 0xff9b && g[2] == 1)
                || (g[0] == 0x2002 && v4(g[1], g[2]))
        }
    }
}

/// Provider names and clip ids end up in file names: letters, digits, `-` and `_` only.
pub fn safe_token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The `.part` name for a URL: stable, so a second attempt finds it.
pub fn part_name(url: &str) -> String {
    format!("{}.part", &blake3::hash(url.as_bytes()).to_hex()[..20])
}

/// Downloads `url` to `dest`. An existing `.part` continues with a Range
/// header; a server that ignores Range restarts from zero.
pub fn download(url: &str, dir: &Path, dest: &Path, ca: Option<&str>) -> Result<PathBuf, String> {
    download_try(url, dir, dest, ca, 0, true, false)
}

/// `download` that follows no redirect: a 3xx answer is an error. For a URL that was checked before the call (a
/// result URL of a connected service): a redirect would send the request to a host that nothing checked.
/// `public`: the connection goes only to a public address (`agent_public`), for a URL that is not the service's own host.
pub fn download_no_redirect(
    url: &str,
    dir: &Path,
    dest: &Path,
    ca: Option<&str>,
    public: bool,
) -> Result<PathBuf, String> {
    download_try(url, dir, dest, ca, 0, false, public)
}

fn download_try(
    url: &str,
    dir: &Path,
    dest: &Path,
    ca: Option<&str>,
    busy: u32,
    follow: bool,
    public: bool,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let part = dir.join(part_name(url));
    let have = std::fs::metadata(&part).map_or(0, |m| m.len());
    // The limit covers the whole body: a keyless 1080p clip took over 60 s here.
    let agent = if public {
        agent_public(600, ca)?
    } else {
        agent_ca(600, ca)?
    };
    let mut req = agent.get(url).header("User-Agent", USER_AGENT);
    if have > 0 {
        req = req.header("Range", &format!("bytes={have}-"));
    }
    let req = if follow {
        req
    } else {
        req.config().max_redirects(0).build()
    };
    // DNS and connect faults came and went here within a minute: wait, as for a rate limit.
    let mut resp = match req.call() {
        Ok(r) => r,
        Err(_) if busy < 6 => {
            // A connect fault may be a missing IPv6 route: the next try goes over IPv4.
            IPV4_ONLY.store(true, std::sync::atomic::Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_secs(1 << busy));
            return download_try(url, dir, dest, ca, busy + 1, follow, public);
        }
        Err(e) => return Err(format!("download failed: {e}")),
    };
    let status = resp.status().as_u16();
    if status == 416 && have > 0 {
        // The part is already whole (or stale): start again without Range.
        let _ = std::fs::remove_file(&part);
        return download_try(url, dir, dest, ca, busy, follow, public);
    }
    // A rate limit on a shared address clears in seconds, then comes back (measured on upload.wikimedia.org:
    // Retry-After 1 to 10, and 5 of 8 calls refused in 16 s). At most about 2 min in all.
    if status == 429 && busy < 6 {
        let wait = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok()?.trim().parse::<u64>().ok())
            .unwrap_or(5)
            .max(if busy == 0 { 0 } else { 1 << busy })
            .min(30);
        std::thread::sleep(std::time::Duration::from_secs(wait));
        return download_try(url, dir, dest, ca, busy + 1, follow, public);
    }
    let append = status == 206 && have > 0;
    if !(status == 200 || status == 206) {
        return Err(format!("download answered HTTP {status}"));
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(&part)
        .map_err(|e| e.to_string())?;
    let mut r = resp.body_mut().with_config().limit(4_000_000_000).reader();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = r
            .read(&mut buf)
            .map_err(|e| format!("download broke (it resumes on the next run): {e}"))?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
    }
    drop(f);
    // The data folder and the project may be on different disks (found on a real run).
    if std::fs::rename(&part, dest).is_err() {
        std::fs::copy(&part, dest).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&part);
    }
    Ok(dest.to_path_buf())
}

/// Search terms when the agent gave none: per caption page, the two longest
/// words that are not common words. Weak on purpose: agents should pass terms.
pub fn auto_terms(page_texts: &[String]) -> Vec<String> {
    const STOP: [&str; 24] = [
        "about", "after", "again", "their", "there", "these", "those", "which", "while", "would",
        "could", "should", "every", "where", "being", "other", "words", "thing", "things",
        "really", "because", "before", "first", "into",
    ];
    page_texts
        .iter()
        .map(|t| {
            let mut w: Vec<String> = t
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.chars().count() >= 4 && !STOP.contains(&w.to_lowercase().as_str()))
                .map(|w| w.to_lowercase())
                .collect();
            w.sort_by_key(|x| std::cmp::Reverse(x.chars().count()));
            w.dedup();
            w.into_iter().take(2).collect::<Vec<_>>().join(" ")
        })
        .map(|t| {
            if t.is_empty() {
                "abstract background".into()
            } else {
                t
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_read_arrays_and_object_values() {
        let v = json!({"hits": [{"videos": {"large": {"url": "a"}, "small": {"url": "b"}}}]});
        let urls: Vec<String> = get(&v, "hits[*]")
            .iter()
            .flat_map(|h| get(h, "videos.*"))
            .map(|f| text(f, "url"))
            .collect();
        assert_eq!(urls, ["a", "b"]);
        assert_eq!(text(&json!({"user": {"name": "Ann"}}), "user.name"), "Ann");
    }

    #[test]
    fn registry_rows_parse() {
        let r = rows();
        assert_eq!(
            r.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
            ["pexels", "pixabay", "commons", "archive"]
        );
        assert!(r.iter().all(|x| !x.terms.redistribution));
    }

    #[test]
    fn clip_urls_must_be_https_on_the_provider_hosts() {
        let h = vec!["pexels.com".to_string()];
        assert!(url_allowed(
            "https://videos.pexels.com/video-files/1/a.mp4",
            &h
        ));
        assert!(
            !url_allowed("http://videos.pexels.com/a.mp4", &h),
            "plain http"
        );
        assert!(
            !url_allowed("https://evilpexels.com/a.mp4", &h),
            "suffix without a dot"
        );
        assert!(!url_allowed("https://pexels.com.evil.net/a.mp4", &h));
        assert!(!url_allowed("https://user@169.254.169.254/latest", &h));
        // A result URL on a file host is public; one that names a machine inside a network is not.
        for public in [
            "https://8.8.8.8/a.png",
            "https://[2606:4700::1]/a.png",
            "https://[::ffff:8.8.8.8]/a.png",
            // a name that does not resolve is not called local: the download fails by itself
            "https://no-such-host.example.invalid/a.png",
        ] {
            assert!(!local_host(public), "{public}");
        }
        for local in [
            "https://10.0.0.5/a.png",
            "https://192.168.1.10:8443/a",
            "https://172.16.3.3/a",
            "https://127.0.0.1/a",
            "https://169.254.169.254/latest",
            "https://100.64.1.1/a",
            "https://[::1]/a",
            "https://[fd00::1]/a",
            "https://[fe80::1]/a",
            "https://[::ffff:10.0.0.1]/a",
            "https://localhost/a",
            "https://intranet/a",
            "https://nas.local/a",
            "https://svc.internal/a",
            "https://user@10.0.0.5/a",
            "https:///a",
            // forms a resolver may read as an address, a trailing dot, a mapped loopback
            "https://127.1/a",
            "https://0x7f.0.0.1/a",
            "https://2130706433/a",
            "https://017700000001/a",
            "https://10.0.0.5./a",
            "https://svc.internal./a",
            "https://localhost.:8443/a",
            "https://[::ffff:127.0.0.1]:8443/a",
        ] {
            assert!(local_host(local), "{local}");
        }
        assert!(
            url_allowed("http://127.0.0.1:8080/f.mp4", &["127.0.0.1".to_string()]),
            "loopback for tests"
        );
        assert!(
            safe_token("pexels")
                && safe_token("12345")
                && !safe_token("../x")
                && !safe_token("/abs")
        );
    }

    #[test]
    fn strip_tags_keeps_text_and_decodes_entities() {
        assert_eq!(
            strip_tags("<a href=\"//x\">CSU &amp; NOAA</a>  &quot;x&quot;"),
            "CSU & NOAA \"x\""
        );
    }

    #[test]
    fn file_terms_read_each_licence_form() {
        let by = file_terms("CC BY-SA 4.0").unwrap();
        assert!(
            by.commercial == Some(true)
                && by.attribution_required
                && by.note.contains("same licence")
        );
        let zero = file_terms("CC0").unwrap();
        assert!(zero.commercial == Some(true) && !zero.attribution_required);
        let pd = file_terms("https://creativecommons.org/publicdomain/mark/1.0/").unwrap();
        assert!(pd.commercial == Some(true) && !pd.attribution_required);
        assert_eq!(
            file_terms("https://creativecommons.org/licenses/by-nc/4.0/")
                .unwrap()
                .commercial,
            Some(false)
        );
        let nd = file_terms("https://creativecommons.org/licenses/by-nc-nd/4.0/").unwrap();
        assert!(nd.commercial == Some(false) && nd.note.contains("do not share an edited copy"));
        let nd = file_terms("CC BY-ND 4.0").unwrap();
        assert!(nd.commercial == Some(false) && !nd.note.contains("not for commercial use"));
        let gfdl = file_terms("GFDL").unwrap();
        assert!(gfdl.commercial.is_none() && gfdl.note.contains("licence not read"));
        assert!(file_terms("").is_none());
    }

    #[test]
    fn download_waits_out_a_rate_limit() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/f", l.local_addr().unwrap());
        std::thread::spawn(move || {
            let answers = [
                "HTTP/1.1 429 Busy\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
            ];
            for (mut s, a) in l.incoming().flatten().zip(answers) {
                let mut b = [0u8; 2048];
                let _ = s.read(&mut b);
                let _ = s.write_all(a.as_bytes());
            }
        });
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("f");
        download(&url, d.path(), &dest, None).unwrap();
        assert_eq!(std::fs::read(dest).unwrap(), b"ok");
    }

    /// A checked URL must not be sent on to another host: the second server stands for a machine nobody checked.
    #[test]
    fn a_result_download_follows_no_redirect() {
        let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let h = hits.clone();
        let to = format!("http://{}/secret", target.local_addr().unwrap());
        std::thread::spawn(move || {
            for mut s in target.incoming().flatten() {
                h.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut b = [0u8; 2048];
                let _ = s.read(&mut b);
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nsecret",
                );
            }
        });
        let first = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/f", first.local_addr().unwrap());
        std::thread::spawn(move || {
            for mut s in first.incoming().flatten() {
                let mut b = [0u8; 2048];
                let _ = s.read(&mut b);
                let a = format!("HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = s.write_all(a.as_bytes());
            }
        });
        let d = tempfile::tempdir().unwrap();
        let dest = d.path().join("f");
        let e = download_no_redirect(&url, d.path(), &dest, None, false).unwrap_err();
        assert!(e.contains("302"), "{e}");
        assert!(!dest.exists());
        assert_eq!(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "the second host got no request"
        );
        // The plain download follows it, as before.
        download(&url, d.path(), &dest, None).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"secret");
    }

    #[test]
    fn credit_is_text_and_a_name_keeps_its_spaces() {
        assert_eq!(
            strip_tags("<a href=\"//x\">Ann  Lee</a> (<i>own</i>)"),
            "Ann Lee (own)"
        );
        assert_eq!(enc_path("a b/c.mp4"), "a%20b/c.mp4");
    }

    #[test]
    fn auto_terms_pick_long_words() {
        assert_eq!(
            auto_terms(&["The ocean waves crash on rocks.".into()]),
            ["ocean waves"]
        );
    }
}

#[cfg(test)]
mod public_agent_tests {
    /// A name that resolves to this machine is refused at connect time: the address that is checked is the one used.
    #[test]
    fn a_name_that_resolves_to_a_private_address_is_refused() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let url = format!("http://localhost:{port}/x");
        let e = super::agent_public(5, None)
            .unwrap()
            .get(&url)
            .call()
            .unwrap_err();
        assert!(e.to_string().contains("private address"), "{e}");
        // The plain agent connects (the listener accepts), so the refusal is the resolver's.
        let t = std::thread::spawn(move || drop(l.accept()));
        assert!(!super::agent_ca(5, None)
            .unwrap()
            .get(&url)
            .call()
            .is_err_and(|e| e.to_string().contains("private address")));
        let _ = t.join();
    }
}

#[cfg(test)]
mod local_ip_tests {
    #[test]
    fn private_v4_inside_v6_forms_and_reserved_ranges_are_local() {
        for a in [
            "::ffff:10.0.0.1",
            "::10.0.0.1",
            "64:ff9b::a00:1",
            "64:ff9b:1::1",
            "2002:a00:1::",
            "2002:7f00:1::",
            "ff02::1",
            "0.1.2.3",
            "192.0.0.8",
            "198.18.0.1",
            "240.0.0.1",
            "224.0.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "fd00::1",
            "fe80::1",
        ] {
            assert!(super::local_ip(a.parse().unwrap()), "{a}");
        }
        for a in [
            "8.8.8.8",
            "2606:4700::1111",
            "64:ff9b::808:808",
            "2002:808:808::",
            "1.1.1.1",
        ] {
            assert!(!super::local_ip(a.parse().unwrap()), "{a}");
        }
    }
}
