//! `scriptvox media search`: open images and sound with no key, through Openverse (section 6 row 22; docs/specs/open-search.md).
//! Each result keeps its own licence through `file_terms`; a result with no stated licence is dropped.

use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::http_rows::{agent_ca, download, enc, file_terms, force_ipv4, safe_token};
use crate::make::StockPick;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// A picture smaller than this on its short side is not used: a video frame would show its blocks.
const MIN_SIDE: u64 = 480;
const MAX_FILE: u64 = 64 << 20;

/// A loopback test server, when the variable names one.
fn test_base() -> Option<String> {
    std::env::var("SCRIPTVOX_OPENVERSE_BASE").ok().filter(|b| {
        b.strip_prefix("http://127.0.0.1")
            .is_some_and(|r| r.is_empty() || r.starts_with([':', '/']))
    })
}

fn base() -> String {
    test_base().unwrap_or_else(|| "https://api.openverse.org".into())
}

/// One result, with its own terms. None when it has no licence or no https file.
fn result_of(r: &Value, kind: &str) -> Option<Value> {
    let licence = r["license"].as_str()?;
    let version = r["license_version"].as_str().unwrap_or("");
    let named = if licence == "pdm" {
        "public domain".to_string()
    } else {
        format!("{licence} {version}").trim().to_string()
    };
    let terms = file_terms(&named)?;
    let url = r["url"].as_str().filter(|u| {
        u.starts_with("https://")
            || test_base().is_some_and(|b| u.starts_with(&format!("{}/", b.trim_end_matches('/'))))
    })?;
    Some(json!({
        "id": r["id"], "kind": kind, "title": r["title"], "url": url, "page": r["foreign_landing_url"],
        "credit": r["creator"].as_str().unwrap_or("an unnamed author"), "licence": named,
        "licence_url": r["license_url"], "terms": terms,
        "duration_ms": r["duration"], "width": r["width"], "height": r["height"],
    }))
}

fn ext_of(url: &str, kind: &str) -> String {
    let last = url
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("");
    match last.rsplit_once('.').map(|x| x.1) {
        Some(e)
            if !e.is_empty() && e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            e.to_ascii_lowercase()
        }
        _ => if kind == "image" { "jpg" } else { "mp3" }.to_string(),
    }
}

/// Saves one result, and its terms beside it so they travel with the file.
fn fetch(item: &Value, kind: &str, index: usize) -> Result<PathBuf, String> {
    let url = item["url"].as_str().unwrap_or_default();
    let id = item["id"]
        .as_str()
        .filter(|i| safe_token(i))
        .map_or(format!("result-{index}"), String::from);
    let dir = Path::new("scriptvox-out/open-media");
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dest = dir.join(format!("{id}.{}", ext_of(url, kind)));
    download(url, &dir.join(".part"), &dest, None)?;
    crate::media_gate::downloaded("the search result", &dest)?;
    let side = PathBuf::from(format!("{}.terms.json", dest.display()));
    std::fs::write(side, serde_json::to_string_pretty(item).unwrap_or_default())
        .map_err(|e| e.to_string())?;
    Ok(dest)
}

/// The results of one search. Err: (what went wrong, the fix).
fn find(query: &str, kind: &str, limit: u32) -> Result<Vec<Value>, (String, String)> {
    let path = if kind == "image" { "images" } else { "audio" };
    let url = format!(
        "{}/v1/{path}/?q={}&page_size={}",
        base(),
        enc(query),
        limit.clamp(1, 20)
    );
    let once = || {
        agent_ca(30, None)
            .expect("no CA file to read")
            .get(&url)
            .call()
    };
    let answer = match once() {
        Err(_) if force_ipv4() => once(),
        r => r,
    };
    let body: Option<Value> = match answer {
        Ok(mut r) if r.status().is_success() => r
            .body_mut()
            .read_to_string()
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok()),
        Ok(r) => {
            return Err((
                format!("the search answered HTTP {}", r.status().as_u16()),
                "Run it again later; a rate limit clears in minutes.".into(),
            ))
        }
        Err(e) => {
            return Err((
                format!("no answer from the search: {e}"),
                "Check the network and run it again.".into(),
            ))
        }
    };
    Ok(body
        .as_ref()
        .and_then(|b| b["results"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|r| result_of(r, kind))
        .collect())
}

/// One try, on a short clock: a host that fails costs this search one result, and the next result is on another host.
/// `download` waits up to a minute per fault, which is right for a stock clip that must arrive.
fn fetch_once(url: &str, dest: &Path) -> Result<(), String> {
    use std::io::Read;
    let mut r = agent_ca(30, None)?
        .get(url)
        .header("User-Agent", crate::http_rows::USER_AGENT)
        .call()
        .map_err(|e| format!("{url}: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("{url} answered HTTP {}", r.status().as_u16()));
    }
    let mut b = Vec::new();
    r.body_mut()
        .as_reader()
        .take(MAX_FILE + 1)
        .read_to_end(&mut b)
        .map_err(|e| format!("{url}: {e}"))?;
    if b.len() as u64 > MAX_FILE {
        return Err(format!("{url} is larger than 64 MB"));
    }
    std::fs::write(dest, b).map_err(|e| e.to_string())
}

/// The file is what its kind says, judged by its first bytes.
fn right_kind(b: &[u8], kind: &str) -> bool {
    if kind == "image" {
        return b.starts_with(b"\x89PNG")
            || b.starts_with(b"\xff\xd8\xff")
            || (b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP");
    }
    b.starts_with(b"ID3")
        || b.starts_with(b"OggS")
        || b.starts_with(b"fLaC")
        || (b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WAVE")
        || (b.len() > 2 && b[0] == 0xFF && b[1] & 0xE0 == 0xE0)
}

/// One file for `make`: the one already fetched for this term and position, or the first search result that downloads
/// and is the right kind of file. `used` holds (term, URL) of this run, so two scenes do not get one picture.
pub fn pick(
    kind: &str,
    term: &str,
    used: &mut Vec<(String, String)>,
    dir: &Path,
    cache_only: bool,
) -> Result<(PathBuf, StockPick), String> {
    let cas = crate::store::Cas::open();
    let nth = used.iter().filter(|(t, _)| t == term).count();
    let ck = crate::store::key("open.search", kind, &json!({"term": term, "nth": nth}));
    let out = dir.join("open");
    if let Some(files) = cas.get(&ck, &out) {
        let side = files.iter().find(|(n, _)| n == "pick.json");
        let file = files.iter().find(|(n, _)| n != "pick.json");
        let picked = side
            .and_then(|(_, p)| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<StockPick>(&b).ok());
        if let (Some((_, file)), Some(p)) = (file, picked) {
            used.push((term.to_string(), p.url.clone()));
            return Ok((file.clone(), p));
        }
    }
    if cache_only {
        return Err(format!("no saved {kind} for '{term}' in the cache"));
    }
    let results = find(term, kind, 20).map_err(|(why, _)| why)?;
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let min_side = |r: &Value| {
        r["width"]
            .as_u64()
            .zip(r["height"].as_u64())
            .map(|(w, h)| w.min(h))
    };
    let mut why = vec![];
    let tries = results
        .iter()
        .filter(|r| {
            !used
                .iter()
                .any(|(_, u)| Some(u.as_str()) == r["url"].as_str())
        })
        .filter(|r| kind != "image" || min_side(r).is_none_or(|s| s >= MIN_SIDE))
        .take(5);
    for (i, r) in tries.enumerate() {
        let url = r["url"].as_str().unwrap_or_default();
        let id = r["id"]
            .as_str()
            .filter(|s| safe_token(s))
            .map_or(format!("result-{i}"), String::from);
        let name = format!("{id}.{}", ext_of(url, kind));
        let dest = out.join(&name);
        if let Err(e) = fetch_once(url, &dest) {
            why.push(e);
            continue;
        }
        if !std::fs::read(&dest).is_ok_and(|b| right_kind(&b, kind)) {
            let _ = std::fs::remove_file(&dest);
            why.push(format!("{url} is not a {kind} scriptvox can use"));
            continue;
        }
        let picked = StockPick {
            provider: "openverse".into(),
            id,
            url: url.to_string(),
            credit: r["credit"].as_str().unwrap_or("").to_string(),
            page: r["page"].as_str().unwrap_or("").to_string(),
            terms: serde_json::from_value(r["terms"].clone()).ok(),
            title: None,
            licence: None,
            layer: None,
        };
        let side = out.join("pick.json");
        std::fs::write(&side, serde_json::to_vec(&picked).unwrap_or_default())
            .map_err(|e| e.to_string())?;
        let _ = cas.put(
            &ck,
            "open.search",
            kind,
            &[(name.as_str(), &dest), ("pick.json", &side)],
            r["terms"].clone(),
        );
        used.push((term.to_string(), picked.url.clone()));
        return Ok((dest, picked));
    }
    if why.is_empty() {
        return Err(format!("no {kind} found for '{term}'"));
    }
    Err(format!("no usable {kind} for '{term}': {}", why.join("; ")))
}

pub fn run(query: &str, kind: &str, limit: u32, get: Option<usize>) -> Envelope {
    if !matches!(kind, "image" | "sound") {
        return Envelope::fail(
            "media",
            Exit::Usage,
            format!("{kind} is not image or sound"),
            Some("Use --kind image or --kind sound.".into()),
        );
    }
    let found = match find(query, kind, limit) {
        Ok(f) => f,
        Err((why, fix)) => return Envelope::fail("media", Exit::ProviderRetryable, why, Some(fix)),
    };
    let mut env = Envelope::ok("media")
        .with("query", json!(query))
        .with("source", json!("Openverse, no key"))
        .with("results", json!(found));
    env.metrics.insert("found".into(), json!(found.len()));
    if let Some(n) = get {
        let Some(item) = n.checked_sub(1).and_then(|i| found.get(i)) else {
            return Envelope::fail(
                "media",
                Exit::Usage,
                format!("result {n} is not in this list of {}", found.len()),
                Some("Give --get with a number from 1 to the number of results.".into()),
            );
        };
        return match fetch(item, kind, n) {
            Ok(file) => {
                env.artifacts.insert(
                    kind.into(),
                    json!(std::path::absolute(&file).unwrap_or(file)),
                );
                env.next.push(format!(
                    "Its terms are in the .terms.json file beside it: {}.",
                    item["terms"]["note"].as_str().unwrap_or("")
                ));
                env
            }
            Err(e) => Envelope::fail(
                "media",
                Exit::ProviderRetryable,
                e,
                Some("Run the same command again: the download resumes.".into()),
            ),
        };
    }
    env.next.push(format!(
        "scriptvox media search \"{query}\" --kind {kind} --get 1"
    ));
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_result_keeps_its_own_licence_and_one_with_none_is_dropped() {
        let row = |l: &str, v: &str| json!({"id": "a", "url": "https://x.example/a.jpg", "license": l, "license_version": v});
        let by = result_of(&row("by", "4.0"), "image").unwrap();
        assert_eq!(by["terms"]["commercial"], true);
        assert_eq!(by["terms"]["attribution_required"], true);
        assert_eq!(
            result_of(&row("by-nc", "4.0"), "image").unwrap()["terms"]["commercial"],
            false
        );
        let pd = result_of(&row("pdm", ""), "sound").unwrap();
        assert_eq!(pd["terms"]["attribution_required"], false);
        assert!(result_of(
            &json!({"id": "a", "url": "https://x.example/a.jpg"}),
            "image"
        )
        .is_none());
        for url in [
            "http://127.0.0.1:9200/x.jpg",
            "http://127.0.0.1.evil.example/x",
        ] {
            assert!(
                result_of(&json!({"id": "a", "url": url, "license": "by"}), "image").is_none(),
                "{url}"
            );
        }
        assert!(result_of(
            &json!({"id": "a", "url": "http://x.example/a.jpg", "license": "by"}),
            "image"
        )
        .is_none());
    }

    #[test]
    fn the_file_type_comes_from_the_url() {
        assert_eq!(ext_of("https://a/b/c.PNG?x=1", "image"), "png");
        assert_eq!(ext_of("https://a/b/c", "sound"), "mp3");
    }
}
