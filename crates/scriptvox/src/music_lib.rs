//! The music library (MILESTONE-PARITY 12.5, docs/specs/music-library.md): tracks with a traced source and licence.
//! ScriptVox ships no audio. A track is downloaded on first use from its pinned address, checked against its sha256,
//! and kept in the content store, so a rerun sends no request. The brief names it as `library:<name>`.

use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::make::StockPick;
use serde_json::json;
use std::path::{Path, PathBuf};

const REGISTRY: &str = include_str!("../../../registry/music.toml");
pub const PREFIX: &str = "library:";

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct Track {
    pub name: String,
    pub title: String,
    pub artist: String,
    pub mood: String,
    pub seconds: f64,
    pub bytes: u64,
    pub sha256: String,
    pub url: String,
    pub page: String,
    pub source: String,
    pub licence: String,
}

#[derive(serde::Deserialize)]
struct Table {
    track: Vec<Track>,
}

pub fn tracks() -> Vec<Track> {
    toml::from_str::<Table>(REGISTRY)
        .expect("registry/music.toml does not parse")
        .track
}

/// True for a music value that names a library track (known or not).
pub fn is_library(music: &str) -> bool {
    music.starts_with(PREFIX)
}

/// The track a `library:<name>` value names. Err: the message and the fix line.
pub fn find(music: &str) -> Result<Track, (String, String)> {
    let name = music.strip_prefix(PREFIX).unwrap_or(music);
    let all = tracks();
    all.iter().find(|t| t.name == name).cloned().ok_or_else(|| {
        (
            format!("no library track {name}"),
            format!(
                "Use one of: {}. `scriptvox media music` lists them with their mood and length.",
                all.iter()
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    })
}

/// The content-store key of a track (by its pinned sha256).
pub fn key(t: &Track) -> String {
    crate::store::key("music.library", "1", &json!({"sha256": t.sha256}))
}

fn file_name(t: &Track) -> String {
    format!("{}.ogg", t.name)
}

/// Where a track is copied for the mix: a folder of ScriptVox's own data, never in the project. A project can come
/// from someone else, and a link planted at a fixed path in it would turn the copy into a write anywhere.
pub fn dir() -> PathBuf {
    crate::provision::data_dir().join("music")
}

/// True when the track is in the content store: `make` needs no request for it.
pub fn cached(t: &Track) -> bool {
    crate::store::Cas::open().has(&key(t))
}

/// The record that goes to the receipt's `stock` list, the credits and `metadata.json`.
pub fn pick(t: &Track) -> StockPick {
    StockPick {
        provider: "library".into(),
        id: t.name.clone(),
        url: t.url.clone(),
        credit: t.artist.clone(),
        page: t.source.clone(),
        terms: crate::http_rows::file_terms(&t.licence),
        title: Some(t.title.clone()),
        licence: Some(t.licence.clone()),
        layer: None,
    }
}

/// One credits line.
pub fn credit(t: &Track) -> String {
    format!(
        "Music: \"{}\" by {}, {} ({}, from the ScriptVox music library)",
        t.title, t.artist, t.source, t.licence
    )
}

/// The base address of a loopback test server, when the variable names one.
fn test_url(t: &Track) -> Option<String> {
    std::env::var("SCRIPTVOX_MUSIC_LIBRARY_BASE")
        .ok()
        .filter(|b| {
            b.strip_prefix("http://127.0.0.1")
                .is_some_and(|r| r.is_empty() || r.starts_with([':', '/']))
        })
        .map(|b| format!("{}/{}", b.trim_end_matches('/'), file_name(t)))
}

fn sha256(p: &Path) -> std::io::Result<String> {
    use sha2::Digest;
    // ponytail: whole file in memory; a track is under 8 MB (test below).
    let b = std::fs::read(p)?;
    Ok(sha2::Sha256::digest(&b)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// The track as a file in `dir`: from the content store, or downloaded once and checked. `cache_only`: no request.
pub fn get(t: &Track, dir: &Path, cache_only: bool) -> Result<PathBuf, String> {
    let cas = crate::store::Cas::open();
    let name = file_name(t);
    if let Some(files) = cas.get(&key(t), dir) {
        if let Some((_, p)) = files.into_iter().find(|(n, _)| *n == name) {
            return Ok(p);
        }
    }
    if cache_only {
        return Err(format!("the library track {} is not in the cache", t.name));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&name);
    let url = test_url(t).unwrap_or_else(|| t.url.clone());
    crate::http_rows::download(&url, &dir.join(".part"), &dest, None)
        .map_err(|e| format!("the library track {} did not download: {e}", t.name))?;
    let got = sha256(&dest).map_err(|e| e.to_string())?;
    if got != t.sha256 {
        let _ = std::fs::remove_file(&dest);
        return Err(format!(
            "the library track {} changed at its source (sha256 {got}, expected {}); it is not used",
            t.name, t.sha256
        ));
    }
    let terms = serde_json::to_value(crate::http_rows::file_terms(&t.licence)).unwrap_or_default();
    let _ = cas.put(
        &key(t),
        "music.library",
        "1",
        &[(name.as_str(), &dest)],
        terms,
    );
    Ok(dest)
}

/// `scriptvox media music [--mood M]`: the library with each track's source and licence.
pub fn list(mood: Option<&str>) -> Envelope {
    let rows: Vec<_> = tracks()
        .into_iter()
        .filter(|t| mood.is_none_or(|m| t.mood == m))
        .map(|t| {
            json!({
                "music": format!("{PREFIX}{}", t.name), "title": t.title, "artist": t.artist, "mood": t.mood,
                "seconds": t.seconds, "mb": (t.bytes as f64 / 1e5).round() / 10.0, "licence": t.licence,
                "source": t.source, "commons": t.page, "downloaded": cached(&t),
            })
        })
        .collect();
    if rows.is_empty() {
        let moods: std::collections::BTreeSet<String> =
            tracks().into_iter().map(|t| t.mood).collect();
        return Envelope::fail(
            "media",
            Exit::Usage,
            format!("no library track has the mood {}", mood.unwrap_or("")),
            Some(format!(
                "Use one of: {}.",
                moods.into_iter().collect::<Vec<_>>().join(", ")
            )),
        );
    }
    let mut env = Envelope::ok("media").with("tracks", json!(rows));
    env.metrics.insert("found".into(), json!(rows.len()));
    env.next.push(
        "Give a track to make as \"music\": \"library:<name>\". It downloads once and is kept for a rerun.".into(),
    );
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_track_has_a_traced_source_and_a_free_licence() {
        let all = tracks();
        assert!(all.len() >= 10);
        let mut names = std::collections::HashSet::new();
        for t in &all {
            assert!(names.insert(t.name.clone()), "{} twice", t.name);
            assert!(crate::http_rows::safe_token(&t.name), "{}", t.name);
            assert!(
                t.url.starts_with("https://upload.wikimedia.org/"),
                "{}",
                t.name
            );
            assert!(
                t.page
                    .starts_with("https://commons.wikimedia.org/wiki/File:"),
                "{}",
                t.name
            );
            assert!(
                t.source.starts_with("https://freemusicarchive.org/music/"),
                "{}",
                t.name
            );
            assert_eq!(t.sha256.len(), 64, "{}", t.name);
            assert!(t.seconds > 60.0 && t.bytes < 8 << 20, "{}", t.name);
            let terms = crate::http_rows::file_terms(&t.licence).unwrap();
            assert_eq!(terms.commercial, Some(true), "{}", t.name);
            assert!(!terms.attribution_required, "{}", t.name);
        }
    }

    #[test]
    fn a_name_is_found_and_a_wrong_name_lists_the_right_ones() {
        assert_eq!(
            find("library:picnic").unwrap().artist,
            "Loyalty Freak Music"
        );
        let (msg, fix) = find("library:rock").unwrap_err();
        assert!(
            msg.contains("rock") && fix.contains("serenity"),
            "{msg} {fix}"
        );
        assert!(is_library("library:x") && !is_library("calm"));
    }
}
