//! fetch, kind HLS (`docs/specs/fetch.md`, section 4.2). The media gate does not let FFmpeg open an address or a
//! playlist, so this code reads the playlist, gets each part with HTTP and appends the parts to one local file.
//! Live and protected streams are refused.

use crate::exit::Exit;
use crate::fetch::{address_ok, join_url, progress, Fail, Fetched, Http, MAX_FILE};
use crate::http_rows::USER_AGENT;
use std::io::Write;
use std::path::{Path, PathBuf};

const MAX_PLAYLIST: u64 = 2 << 20;
const MAX_PARTS: usize = 20_000;
/// ponytail: a part is read into memory before it is appended. A real part is a few MB; a larger one is refused.
const MAX_PART: u64 = 256 << 20;

/// The same content in one size, from a master playlist.
#[derive(Debug, PartialEq)]
pub struct Variant {
    pub uri: String,
    pub bandwidth: u64,
    pub height: Option<u32>,
    pub audio_group: Option<String>,
}

/// One part of a media playlist. `range` is the first and the last byte, when the part is a span of a larger file.
#[derive(Debug, PartialEq, Clone)]
pub struct Part {
    pub uri: String,
    pub duration_s: f64,
    pub range: Option<(u64, u64)>,
}

#[derive(Debug, PartialEq)]
pub struct Media {
    /// The start part of fragmented MP4.
    pub init: Option<Part>,
    pub parts: Vec<Part>,
    pub ended: bool,
    pub encrypted: bool,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Playlist {
    /// `audio`: (group id, address) of each sound entry that has its own playlist.
    Master {
        variants: Vec<Variant>,
        audio: Vec<(String, String)>,
    },
    Media(Media),
}

/// `KEY=value,KEY="va,lue"` as pairs. A comma in quotes does not split.
fn attributes(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let (mut field, mut quoted) = (String::new(), false);
    for c in s.chars().chain([',']) {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                if let Some((k, v)) = field.split_once('=') {
                    out.push((k.trim().to_ascii_uppercase(), v.trim().to_string()));
                }
                field.clear();
            }
            c => field.push(c),
        }
    }
    out
}

fn attr<'a>(a: &'a [(String, String)], key: &str) -> Option<&'a str> {
    a.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// `n` or `n@o` as (first, last) byte. With no `o`, the span starts at `next`.
fn byte_range(s: &str, next: u64) -> Option<(u64, u64)> {
    let (n, o) = match s.split_once('@') {
        Some((n, o)) => (n.trim().parse::<u64>().ok()?, o.trim().parse::<u64>().ok()?),
        None => (s.trim().parse::<u64>().ok()?, next),
    };
    Some((o, o.checked_add(n.checked_sub(1)?)?))
}

pub(crate) fn parse(text: &str) -> Result<Playlist, String> {
    let mut lines = text
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty());
    if !lines.next().is_some_and(|l| l.starts_with("#EXTM3U")) {
        return Err("the text does not start with #EXTM3U".into());
    }
    let (mut variants, mut audio) = (Vec::new(), Vec::new());
    let mut media = Media {
        init: None,
        parts: Vec::new(),
        ended: false,
        encrypted: false,
    };
    // What the next address line is: a variant, or a part with this duration.
    let mut variant: Option<Variant> = None;
    let mut duration: Option<f64> = None;
    let mut range: Option<(u64, u64)> = None;
    // Where a byte span with no offset starts: after the last span.
    let mut next_byte = 0u64;
    for line in lines {
        if let Some(a) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let a = attributes(a);
            variant = Some(Variant {
                uri: String::new(),
                bandwidth: attr(&a, "BANDWIDTH")
                    .and_then(|b| b.parse().ok())
                    .unwrap_or(0),
                height: attr(&a, "RESOLUTION")
                    .and_then(|r| r.to_ascii_lowercase().split_once('x')?.1.parse().ok()),
                audio_group: attr(&a, "AUDIO").map(str::to_string),
            });
        } else if let Some(a) = line.strip_prefix("#EXT-X-MEDIA:") {
            let a = attributes(a);
            if attr(&a, "TYPE") == Some("AUDIO") {
                if let Some(uri) = attr(&a, "URI") {
                    audio.push((
                        attr(&a, "GROUP-ID").unwrap_or("").to_string(),
                        uri.to_string(),
                    ));
                }
            }
        } else if let Some(d) = line.strip_prefix("#EXTINF:") {
            let d = d.split(',').next().unwrap_or("").trim();
            duration = Some(
                d.parse::<f64>()
                    .ok()
                    .filter(|d| d.is_finite() && *d >= 0.0)
                    .ok_or("a part has a duration that is not a number")?,
            );
        } else if let Some(r) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            range = Some(byte_range(r, next_byte).ok_or("a byte span is not a number")?);
        } else if let Some(a) = line.strip_prefix("#EXT-X-MAP:") {
            let a = attributes(a);
            let init = Part {
                uri: attr(&a, "URI")
                    .ok_or("a start part has no address")?
                    .to_string(),
                duration_s: 0.0,
                range: match attr(&a, "BYTERANGE") {
                    Some(r) => Some(byte_range(r, 0).ok_or("a byte span is not a number")?),
                    None => None,
                },
            };
            // ponytail: one start part for the whole list. A list that changes it part way is refused.
            if media.init.as_ref().is_some_and(|old| *old != init) {
                return Err("the playlist has more than one start part".into());
            }
            media.init = Some(init);
        } else if let Some(a) = line.strip_prefix("#EXT-X-KEY:") {
            let a = attributes(a);
            media.encrypted |= attr(&a, "METHOD").is_some_and(|m| m != "NONE");
        } else if line.starts_with("#EXT-X-ENDLIST") {
            media.ended = true;
        } else if !line.starts_with('#') {
            if let Some(mut v) = variant.take() {
                v.uri = line.to_string();
                variants.push(v);
            } else if let Some(d) = duration.take() {
                if let Some((_, last)) = range {
                    next_byte = last.checked_add(1).ok_or("a byte span is not a number")?;
                }
                media.parts.push(Part {
                    uri: line.to_string(),
                    duration_s: d,
                    range: range.take(),
                });
            }
        }
    }
    Ok(if variants.is_empty() {
        Playlist::Media(media)
    } else {
        Playlist::Master { variants, audio }
    })
}

/// The variant with the largest bandwidth whose height is at most 1080. When all are higher: the lowest one.
fn pick(variants: &[Variant]) -> Option<&Variant> {
    variants
        .iter()
        .filter(|v| v.height.is_none_or(|h| h <= 1080))
        .max_by_key(|v| v.bandwidth)
        .or_else(|| variants.iter().min_by_key(|v| v.height))
}

fn too_large() -> Fail {
    Fail::usage(
        "The stream is above the limit of 4 GB.",
        "Use `--from` and `--to` to get a shorter part of it.",
    )
}

fn bad_playlist(why: &str) -> Fail {
    Fail::unsupported(
        &format!("fetch cannot read this HLS playlist: {why}."),
        "Give the address of the media file itself, or of another playlist.",
    )
}

/// The text of a playlist, at most 2 MB.
fn playlist(http: &mut Http, url: &str) -> Result<Playlist, Fail> {
    if !address_ok(url) {
        return Err(Fail::private());
    }
    let r = http.get(url, USER_AGENT, None, MAX_PLAYLIST + 1)?;
    if r.status != 200 {
        return Err(Fail::refused(r.status));
    }
    if r.body.len() as u64 > MAX_PLAYLIST {
        return Err(Fail::usage(
            "The playlist is above the limit of 2 MB.",
            "Give the address of a smaller playlist.",
        ));
    }
    parse(&String::from_utf8_lossy(&r.body)).map_err(|e| bad_playlist(&e))
}

/// Gets the parts of one media playlist into one local file. Returns the file and the media time of its first part.
/// `written`: the bytes of this run, all playlists together; the limit is `MAX_FILE`.
fn parts_to_file(
    http: &mut Http,
    base: &str,
    media: &Media,
    window: Option<(f64, Option<f64>)>,
    dest: &Path,
    written: &mut u64,
) -> Result<(PathBuf, f64), Fail> {
    if media.parts.len() > MAX_PARTS {
        return Err(Fail::usage(
            "The playlist is above the limit of 20,000 parts.",
            "Use `--from` and `--to` for an HLS source.",
        ));
    }
    // The parts whose time span touches the window.
    let (mut at, mut start) = (0.0, None);
    let mut kept: Vec<&Part> = Vec::new();
    for p in &media.parts {
        let end = at + p.duration_s;
        let inside = window.is_none_or(|(from, to)| end > from && to.is_none_or(|to| at < to));
        if inside {
            start.get_or_insert(at);
            kept.push(p);
        }
        at = end;
    }
    if kept.is_empty() {
        return Err(Fail::usage(
            "No part of the stream is in the --from and --to times.",
            &format!("The stream is {at:.1} s long. Give times inside it."),
        ));
    }
    // Each address is checked before the first request.
    let mut todo = Vec::new();
    for p in media.init.iter().chain(kept) {
        let url = join_url(base, &p.uri).filter(|u| address_ok(u));
        todo.push((url.ok_or_else(Fail::private)?, p.range));
    }
    // The sizes that the playlist gives are added up before the first request.
    let declared = todo
        .iter()
        .filter_map(|(_, r)| r.map(|(first, last)| last.saturating_sub(first).saturating_add(1)))
        .fold(*written, u64::saturating_add);
    if declared > MAX_FILE {
        return Err(too_large());
    }
    let dest = dest.with_extension(if media.init.is_some() { "mp4" } else { "ts" });
    let io = |e: std::io::Error| {
        Fail::new(
            Exit::Error,
            "SVX_ERROR",
            &format!("cannot write the download: {e}"),
            "Check the free space of the data folder.",
        )
    };
    let mut file = std::fs::File::create(&dest).map_err(io)?;
    let count = todo.len();
    for (i, (url, range)) in todo.iter().enumerate() {
        let r = http
            .get(
                url,
                USER_AGENT,
                *range,
                MAX_PART.min(MAX_FILE - *written) + 1,
            )
            .inspect_err(|_| {
                let _ = std::fs::remove_file(&dest);
            })?;
        let whole = match range {
            Some((first, last)) => r.status == 206 && r.body.len() as u64 == last - first + 1,
            None => r.status == 200 && r.body.len() as u64 <= MAX_PART,
        };
        *written += r.body.len() as u64;
        if whole && *written > MAX_FILE {
            let _ = std::fs::remove_file(&dest);
            return Err(too_large());
        }
        if !whole {
            let _ = std::fs::remove_file(&dest);
            return Err(if (200..300).contains(&r.status) {
                bad_playlist(
                    "a part does not have the size that the playlist gives, or is above 256 MB",
                )
            } else {
                Fail::refused(r.status)
            });
        }
        file.write_all(&r.body).map_err(io).inspect_err(|_| {
            let _ = std::fs::remove_file(&dest);
        })?;
        progress(&format!("part {} of {count}", i + 1));
    }
    drop(file);
    crate::fetch::gate(&dest)?;
    Ok((dest, start.unwrap_or(0.0)))
}

/// `first_bytes`: the start of the playlist, from the probe. A playlist under 1 KB is whole there.
pub(crate) fn fetch(
    http: &mut Http,
    url: &str,
    first_bytes: &[u8],
    audio: bool,
    window: Option<(f64, Option<f64>)>,
    dir: &Path,
) -> Result<Fetched, Fail> {
    let top = if first_bytes.len() < 1024 {
        parse(&String::from_utf8_lossy(first_bytes)).map_err(|e| bad_playlist(&e))?
    } else {
        playlist(http, url)?
    };
    // The media playlists to get: the picture (with its sound, or not), then a separate sound.
    let mut lists: Vec<(String, Media)> = Vec::new();
    match top {
        Playlist::Media(m) => lists.push((url.to_string(), m)),
        Playlist::Master {
            variants,
            audio: sounds,
        } => {
            let v = pick(&variants).ok_or_else(|| bad_playlist("it lists no stream"))?;
            let sound = sounds
                .iter()
                .find(|(group, _)| Some(group) == v.audio_group.as_ref())
                .or_else(|| sounds.first().filter(|_| audio));
            let mut wanted = Vec::new();
            if !(audio && sound.is_some()) {
                wanted.push(&v.uri);
            }
            wanted.extend(sound.map(|s| &s.1));
            for uri in wanted {
                let at = join_url(url, uri).ok_or_else(|| bad_playlist("an address is empty"))?;
                match playlist(http, &at)? {
                    Playlist::Media(m) => lists.push((at, m)),
                    Playlist::Master { .. } => {
                        return Err(bad_playlist("a stream entry is a list of streams"))
                    }
                }
            }
        }
    }
    // Live and protected streams are refused before any part is asked for.
    for (_, m) in &lists {
        if !m.ended {
            return Err(Fail::live());
        }
        if m.encrypted {
            return Err(Fail::unsupported(
                "This stream is protected. fetch does not read protected streams.",
                "Use a source that is not protected.",
            ));
        }
    }
    let hash = blake3::hash(url.as_bytes()).to_hex()[..8].to_string();
    let (mut files, mut starts, mut written) = (Vec::new(), Vec::new(), 0u64);
    for (i, (base, media)) in lists.iter().enumerate() {
        let dest = dir.join(format!("hls-{hash}-{}-{i}", std::process::id()));
        match parts_to_file(http, base, media, window, &dest, &mut written) {
            Ok((file, start)) => {
                files.push(file);
                starts.push(start);
            }
            Err(e) => {
                for f in files {
                    let _ = std::fs::remove_file(f);
                }
                return Err(e);
            }
        }
    }
    Ok(Fetched {
        files,
        starts,
        kind: "hls",
        source_url: crate::fetch::public_url(url),
        title: crate::fetch::last_part(url).0,
        id: None,
        uploader: None,
        duration_s: None,
        licence: None,
        findings: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reads_a_master_playlist() {
        let text = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"English, main\",URI=\"audio/list.m3u8\"\n\
            #EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"sub\",URI=\"subs.m3u8\"\n\
            #EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360,CODECS=\"avc1.4d401e,mp4a.40.2\"\nlow.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080,AUDIO=\"aud\"\nhigh.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=16000000,RESOLUTION=3840x2160\nuhd.m3u8\n";
        let Playlist::Master { variants, audio } = parse(text).unwrap() else {
            panic!("not a master playlist")
        };
        assert_eq!(
            audio,
            vec![("aud".to_string(), "audio/list.m3u8".to_string())]
        );
        assert_eq!(variants.len(), 3);
        assert_eq!(
            variants[0],
            Variant {
                uri: "low.m3u8".into(),
                bandwidth: 800_000,
                height: Some(360),
                audio_group: None
            }
        );
        assert_eq!(variants[1].audio_group.as_deref(), Some("aud"));
        // The largest bandwidth at 1080 or under.
        assert_eq!(pick(&variants).unwrap().uri, "high.m3u8");
        assert_eq!(pick(&variants[2..]).unwrap().uri, "uhd.m3u8");
    }

    #[test]
    fn parse_reads_a_media_playlist() {
        let text =
            "#EXTM3U\r\n#EXT-X-VERSION:7\r\n#EXT-X-MAP:URI=\"all.m4s\",BYTERANGE=\"800@0\"\r\n\
            #EXTINF:2.000,\r\n#EXT-X-BYTERANGE:1000@800\r\nall.m4s\r\n\
            #EXTINF:1.5,title\r\n#EXT-X-BYTERANGE:500\r\nall.m4s\r\n#EXT-X-ENDLIST\r\n";
        let Playlist::Media(m) = parse(text).unwrap() else {
            panic!("not a media playlist")
        };
        assert!(m.ended && !m.encrypted);
        assert_eq!(m.init.unwrap().range, Some((0, 799)));
        assert_eq!(
            m.parts,
            vec![
                Part {
                    uri: "all.m4s".into(),
                    duration_s: 2.0,
                    range: Some((800, 1799))
                },
                Part {
                    uri: "all.m4s".into(),
                    duration_s: 1.5,
                    range: Some((1800, 2299))
                },
            ]
        );
    }

    #[test]
    fn parse_marks_live_and_protected_lists_and_refuses_bad_text() {
        let live = "#EXTM3U\n#EXTINF:4,\na.ts\n";
        assert!(matches!(
            parse(live),
            Ok(Playlist::Media(Media { ended: false, .. }))
        ));
        let key = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"https://k.example.org/key\"\n#EXTINF:4,\na.ts\n#EXT-X-ENDLIST\n";
        assert!(matches!(
            parse(key),
            Ok(Playlist::Media(Media {
                encrypted: true,
                ..
            }))
        ));
        let none = "#EXTM3U\n#EXT-X-KEY:METHOD=NONE\n#EXTINF:4,\na.ts\n#EXT-X-ENDLIST\n";
        assert!(matches!(
            parse(none),
            Ok(Playlist::Media(Media {
                encrypted: false,
                ..
            }))
        ));
        assert!(parse("<html>").is_err());
        assert!(parse("#EXTM3U\n#EXTINF:abc,\na.ts\n").is_err());
        assert!(parse("#EXTM3U\n#EXT-X-BYTERANGE:x\n").is_err());
        // Numbers that do not fit: an error, not a panic.
        let huge = u64::MAX;
        assert!(parse(&format!("#EXTM3U\n#EXT-X-BYTERANGE:{huge}@5\n")).is_err());
        assert!(parse(&format!(
            "#EXTM3U\n#EXTINF:1,\n#EXT-X-BYTERANGE:{huge}@1\na.ts\n"
        ))
        .is_err());
        // The text of the server is not in the message.
        assert!(!parse("#EXTM3U\n#EXTINF:abc,\na.ts\n")
            .unwrap_err()
            .contains("abc"));
        assert!(parse("#EXTM3U\n#EXT-X-MAP:URI=\"a.mp4\"\n#EXT-X-MAP:URI=\"b.mp4\"\n").is_err());
    }
}
