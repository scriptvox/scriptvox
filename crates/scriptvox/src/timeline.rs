//! `export --timeline fcpxml|otio` (G053): the video as an editor's timeline. The picture is cut at its shot changes
//! (one clip per shot, all from the one file), and the sound is the narration and music stems, or the video's own
//! sound. FCPXML 1.10 (Final Cut Pro, DaVinci Resolve) and OpenTimelineIO JSON (Kdenlive, Resolve, many tools) are
//! written from their public specifications; no code read.

use serde_json::{json, Value};
use std::path::Path;

/// One file on the timeline, from 0 for `frames`.
pub struct Sound<'a> {
    pub name: &'a str,
    pub file: &'a Path,
    pub frames: u64,
}

/// Shots as (first frame, frames), from cut times in seconds; together they cover `total` frames.
pub fn shots(cuts: &[f64], fps: u32, total: u64) -> Vec<(u64, u64)> {
    let mut at: Vec<u64> = cuts
        .iter()
        .map(|c| (c * f64::from(fps)).round() as u64)
        .filter(|&f| f > 0 && f < total)
        .collect();
    at.sort_unstable();
    at.dedup();
    let mut out = Vec::new();
    let mut start = 0;
    for f in at.into_iter().chain([total]) {
        if f > start {
            out.push((start, f - start));
            start = f;
        }
    }
    out
}

fn url(p: &Path) -> String {
    let abs = std::path::absolute(p).unwrap_or(p.to_path_buf());
    let s = abs.to_string_lossy().replace('\\', "/");
    let s = if s.starts_with('/') {
        s
    } else {
        format!("/{s}")
    };
    // Only the characters a path may hold that break a URL or XML.
    let enc: String = s
        .chars()
        .map(|c| match c {
            ' ' => "%20".into(),
            '#' => "%23".into(),
            '%' => "%25".into(),
            '&' => "%26".into(),
            '<' => "%3C".into(),
            '>' => "%3E".into(),
            '"' => "%22".into(),
            c => c.to_string(),
        })
        .collect();
    format!("file://{enc}")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// FCPXML 1.10: one sequence; the shots in the spine; each stem a connected clip on its own lane under the picture.
pub fn fcpxml(
    name: &str,
    video: &Path,
    (w, h): (u32, u32),
    fps: u32,
    shots: &[(u64, u64)],
    sounds: &[Sound],
) -> String {
    let t = |f: u64| format!("{f}/{fps}s");
    let total: u64 = shots.iter().map(|s| s.1).sum();
    let mut res = format!(
        "    <format id=\"r1\" name=\"FFVideoFormat{h}p{fps}\" frameDuration=\"1/{fps}s\" width=\"{w}\" height=\"{h}\"/>\n    <asset id=\"r2\" name=\"{}\" start=\"0s\" duration=\"{}\" hasVideo=\"1\" hasAudio=\"1\" format=\"r1\" audioSources=\"1\" audioChannels=\"2\" audioRate=\"48000\">\n      <media-rep kind=\"original-media\" src=\"{}\"/>\n    </asset>\n",
        esc(&video.file_name().unwrap_or_default().to_string_lossy()),
        t(total),
        esc(&url(video))
    );
    for (i, s) in sounds.iter().enumerate() {
        res.push_str(&format!(
            "    <asset id=\"r{}\" name=\"{}\" start=\"0s\" duration=\"{}\" hasAudio=\"1\" audioSources=\"1\" audioChannels=\"2\" audioRate=\"48000\">\n      <media-rep kind=\"original-media\" src=\"{}\"/>\n    </asset>\n",
            i + 3,
            esc(s.name),
            t(s.frames),
            esc(&url(s.file))
        ));
    }
    let mut spine = String::new();
    for (k, (start, len)) in shots.iter().enumerate() {
        // The stems hang on the first shot; with stems, the picture's own sound is turned down.
        spine.push_str(&format!(
            "          <asset-clip ref=\"r2\" name=\"shot {}\" offset=\"{}\" start=\"{}\" duration=\"{}\">\n",
            k + 1,
            t(*start),
            t(*start),
            t(*len)
        ));
        if !sounds.is_empty() {
            spine.push_str("            <adjust-volume amount=\"-96dB\"/>\n");
        }
        if k == 0 {
            for (i, s) in sounds.iter().enumerate() {
                spine.push_str(&format!(
                    "            <asset-clip ref=\"r{}\" lane=\"-{}\" name=\"{}\" offset=\"0s\" start=\"0s\" duration=\"{}\" audioRole=\"{}\"/>\n",
                    i + 3,
                    i + 1,
                    esc(s.name),
                    t(s.frames),
                    if s.name.starts_with("music") { "music" } else { "dialogue" }
                ));
            }
        }
        spine.push_str("          </asset-clip>\n");
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n<fcpxml version=\"1.10\">\n  <resources>\n{res}  </resources>\n  <library>\n    <event name=\"scriptvox\">\n      <project name=\"{}\">\n        <sequence format=\"r1\" duration=\"{}\" tcStart=\"0s\" tcFormat=\"NDF\" audioLayout=\"stereo\" audioRate=\"48k\">\n          <spine>\n{spine}          </spine>\n        </sequence>\n      </project>\n    </event>\n  </library>\n</fcpxml>\n",
        esc(name),
        t(total)
    )
}

fn range(start: u64, len: u64, fps: u32) -> Value {
    json!({"OTIO_SCHEMA": "TimeRange.1",
        "start_time": {"OTIO_SCHEMA": "RationalTime.1", "rate": f64::from(fps), "value": start as f64},
        "duration": {"OTIO_SCHEMA": "RationalTime.1", "rate": f64::from(fps), "value": len as f64}})
}

fn clip(name: &str, file: &Path, start: u64, len: u64, avail: u64, fps: u32) -> Value {
    json!({"OTIO_SCHEMA": "Clip.2", "name": name, "metadata": {}, "effects": [], "markers": [], "enabled": true,
        "source_range": range(start, len, fps),
        "active_media_reference_key": "DEFAULT_MEDIA",
        "media_references": {"DEFAULT_MEDIA": {"OTIO_SCHEMA": "ExternalReference.1", "name": name, "metadata": {},
            "target_url": url(file), "available_range": range(0, avail, fps), "available_image_bounds": null}}})
}

fn track(name: &str, kind: &str, children: Vec<Value>) -> Value {
    json!({"OTIO_SCHEMA": "Track.1", "name": name, "kind": kind, "metadata": {}, "effects": [], "markers": [],
        "enabled": true, "source_range": null, "children": children})
}

/// OpenTimelineIO JSON: a video track of shots, and an audio track per stem, or the video's own sound.
pub fn otio(name: &str, video: &Path, fps: u32, shots: &[(u64, u64)], sounds: &[Sound]) -> Value {
    let total: u64 = shots.iter().map(|s| s.1).sum();
    let pic: Vec<Value> = shots
        .iter()
        .enumerate()
        .map(|(k, (s, l))| clip(&format!("shot {}", k + 1), video, *s, *l, total, fps))
        .collect();
    let mut tracks = vec![track("V1", "Video", pic)];
    if sounds.is_empty() {
        tracks.push(track(
            "A1",
            "Audio",
            vec![clip("sound", video, 0, total, total, fps)],
        ));
    }
    for (i, s) in sounds.iter().enumerate() {
        tracks.push(track(
            &format!("A{}", i + 1),
            "Audio",
            vec![clip(s.name, s.file, 0, s.frames, s.frames, fps)],
        ));
    }
    json!({"OTIO_SCHEMA": "Timeline.1", "name": name, "metadata": {}, "global_start_time": null,
        "tracks": {"OTIO_SCHEMA": "Stack.1", "name": "tracks", "metadata": {}, "effects": [], "markers": [],
            "enabled": true, "source_range": null, "children": tracks}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shots_cover_the_video_once() {
        assert_eq!(
            shots(&[1.0, 2.5, 2.5, 0.0, 9.0], 30, 120),
            vec![(0, 30), (30, 45), (75, 45)]
        );
        assert_eq!(shots(&[], 25, 50), vec![(0, 50)]);
    }

    /// Parse-back: the OTIO tracks add up to the video, and the FCPXML spine does too.
    #[test]
    fn timelines_read_back() {
        let v = Path::new("/tmp/a b&c.mp4");
        let n = Path::new("/tmp/narration.wav");
        let sh = shots(&[2.0, 5.0], 30, 300);
        let sounds = [Sound {
            name: "narration",
            file: n,
            frames: 300,
        }];
        let o = otio("x", v, 30, &sh, &sounds);
        let back: Value = serde_json::from_str(&o.to_string()).unwrap();
        let tr = back["tracks"]["children"].as_array().unwrap();
        assert_eq!(tr.len(), 2);
        let sum: f64 = tr[0]["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["source_range"]["duration"]["value"].as_f64().unwrap())
            .sum();
        assert_eq!(sum, 300.0);
        assert_eq!(
            tr[0]["children"][0]["media_references"]["DEFAULT_MEDIA"]["target_url"],
            "file:///tmp/a%20b%26c.mp4"
        );
        let x = fcpxml("x", v, (1080, 1920), 30, &sh, &sounds);
        let durs: u64 = x
            .lines()
            .filter(|l| l.contains("name=\"shot "))
            .map(|l| {
                let d = l.split("duration=\"").nth(1).unwrap();
                d.split('/').next().unwrap().parse::<u64>().unwrap()
            })
            .sum();
        assert_eq!(durs, 300);
        assert_eq!(
            x.matches("<asset-clip").count(),
            x.matches("</asset-clip>").count() + 1
        );
        assert!(x.contains("lane=\"-1\""));
    }
}
