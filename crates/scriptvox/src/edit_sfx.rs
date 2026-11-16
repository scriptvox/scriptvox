//! `edit --sfx NAME@SECONDS`: built-in or own sound effects at set times on a finished video (`edit-sfx.md`).
//! One sound-only pass after the cuts and the music: the picture is copied, the length stays.

use crate::audio::Sfx;
use crate::engine_ffmpeg::Ffmpeg;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const MAX: usize = 32;
/// Every sound goes into the mix in this form.
const TO_MIX: &str = "aresample=48000,aformat=channel_layouts=stereo";

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Built(Sfx),
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub name: String,
    pub source: Source,
    pub at: f64,
}

impl Cue {
    /// When the sound starts: a whoosh peaks at its time.
    pub fn starts_at(&self) -> f64 {
        match self.source {
            Source::Built(k) => (self.at - k.lead()).max(0.0),
            Source::File(_) => self.at,
        }
    }
}

/// The path part of a spec, for the checks that `edit` runs on every file it reads.
pub fn file_of(spec: &str) -> Option<PathBuf> {
    let name = spec.rsplit_once('@')?.0;
    Sfx::named(name).is_none().then(|| PathBuf::from(name))
}

/// How far under the base peak each effect peaks, in dB.
pub fn under_db(level: Option<&str>) -> Result<f64, String> {
    match level {
        Some("quiet") => Ok(18.0),
        None | Some("normal") => Ok(12.0),
        Some("loud") => Ok(6.0),
        Some(o) => Err(format!("unknown --sfx-level {o}")),
    }
}

/// `NAME@SECONDS` specs to cues. The error is the message and its fix line.
pub fn parse(specs: &[String]) -> Result<Vec<Cue>, (String, String)> {
    if specs.len() > MAX {
        return Err((
            format!(
                "{} sound effects, and one call takes at most {MAX}",
                specs.len()
            ),
            "Run two calls: the second on the result of the first.".into(),
        ));
    }
    let root = Path::new(".").canonicalize().ok();
    specs
        .iter()
        .map(|spec| {
            let shape = || {
                (
                    format!("--sfx {spec}: no time"),
                    "Give NAME@SECONDS, for example --sfx whoosh@12.0.".to_string(),
                )
            };
            let (name, at) = spec.rsplit_once('@').ok_or_else(shape)?;
            let at: f64 = at
                .trim()
                .parse()
                .ok()
                .filter(|t: &f64| t.is_finite() && *t >= 0.0)
                .ok_or_else(shape)?;
            let source = match Sfx::named(name) {
                Some(k) => Source::Built(k),
                None => {
                    let p = Path::new(name);
                    if !p.is_file() {
                        return Err((
                            format!("no sound effect {name}"),
                            "Use whoosh, pop or tick, or a sound file inside this project.".into(),
                        ));
                    }
                    let inside = p
                        .canonicalize()
                        .ok()
                        .zip(root.as_ref())
                        .is_some_and(|(f, r)| f.starts_with(r));
                    if !inside {
                        return Err((
                            format!("the sound file {name} is outside this project folder"),
                            "Copy the sound file into the project.".into(),
                        ));
                    }
                    Source::File(std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()))
                }
            };
            Ok(Cue {
                name: name.to_string(),
                source,
                at,
            })
        })
        .collect()
}

/// The peak of a sound in dBFS (FFmpeg `volumedetect`), as the mix gets it: a mono source comes out 3 dB lower in
/// stereo (seen 13 Nov 2026: the pop peaked 3 dB under its number). `input`: the FFmpeg input arguments.
fn peak_db(ff: &Ffmpeg, input: &[String]) -> Option<f64> {
    let o = std::process::Command::new(&ff.ffmpeg)
        .args(["-hide_banner", "-nostdin", "-nostats"])
        .args(input)
        .args([
            "-vn",
            "-af",
            &format!("{TO_MIX},volumedetect"),
            "-f",
            "null",
            "-",
        ])
        .output()
        .ok()?;
    let log = String::from_utf8_lossy(&o.stderr);
    let v = log
        .lines()
        .rev()
        .find_map(|l| l.split("max_volume:").nth(1))?;
    v.trim()
        .trim_end_matches("dB")
        .trim()
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
}

/// The FFmpeg input arguments of a source.
fn input_of(s: &Source) -> Result<Vec<String>, String> {
    match s {
        Source::Built(k) => Ok(["-f", "lavfi", "-i", k.source()].map(String::from).to_vec()),
        Source::File(f) => crate::media_gate::input(f),
    }
}

/// The graph that mixes the cues over the sound of FFmpeg input `base`. `sources[i]` is input `base + 1 + i` with
/// its gain in dB; each cue names its source by index.
pub fn graph(base: usize, sources: &[f64], cues: &[(usize, f64)]) -> String {
    let mut parts = vec![format!(
        "[{base}:a]aresample=48000,aformat=channel_layouts=stereo[base]"
    )];
    let mut labels = String::from("[base]");
    for (i, gain) in sources.iter().enumerate() {
        let mine: Vec<f64> = cues.iter().filter(|c| c.0 == i).map(|c| c.1).collect();
        let outs: String = (0..mine.len()).map(|j| format!("[s{i}_{j}]")).collect();
        parts.push(format!(
            "[{}:a]{TO_MIX},volume={gain:.2}dB,asplit={}{outs}",
            base + 1 + i,
            mine.len()
        ));
        for (j, t) in mine.iter().enumerate() {
            let ms = (t * 1000.0).round() as u64;
            parts.push(format!("[s{i}_{j}]adelay={ms}:all=1[d{i}_{j}]"));
            labels.push_str(&format!("[d{i}_{j}]"));
        }
    }
    // `duration=first`: the length is the video's. Not `-shortest`: on a copied picture it dropped the last frames.
    parts.push(format!(
        "{labels}amix=inputs={}:normalize=0:duration=first[m]",
        cues.len() + 1
    ));
    parts.join(";")
}

/// Mixes the cues into `out`'s sound, in place. The picture is copied. Returns the `sfx` part of the result.
pub fn add(ff: &Ffmpeg, out: &Path, cues: &[Cue], level: Option<&str>) -> Result<Value, String> {
    let under = under_db(level)?;
    let abs = |p: &Path| std::path::absolute(p).unwrap_or(p.to_path_buf());
    let secs = ff.duration(out)?;
    let has_sound = ff.has_stream(out, "audio");
    let base = crate::media_gate::input(&abs(out))?;
    // A video with no sound, or with digital silence: the effects sit under full scale.
    let base_peak = has_sound
        .then(|| peak_db(ff, &base))
        .flatten()
        .filter(|p| *p > -60.0)
        .unwrap_or(0.0);
    let mut sources: Vec<&Source> = Vec::new();
    for c in cues {
        if !sources.contains(&&c.source) {
            sources.push(&c.source);
        }
    }
    let mut args = base.clone();
    if !has_sound {
        args.extend(
            ["-f", "lavfi", "-i"]
                .map(String::from)
                .into_iter()
                .chain([format!("anullsrc=r=48000:cl=stereo:d={secs:.3}")]),
        );
    }
    let mut gains = Vec::new();
    for s in &sources {
        let input = input_of(s)?;
        let peak = peak_db(ff, &input).ok_or_else(|| match s {
            Source::File(f) => format!("{} has no sound FFmpeg can read", f.display()),
            Source::Built(_) => "FFmpeg could not make the built-in effect".to_string(),
        })?;
        gains.push(base_peak - under - peak);
        args.extend(input);
    }
    let placed: Vec<(usize, f64)> = cues
        .iter()
        .map(|c| {
            let i = sources.iter().position(|s| **s == c.source).unwrap_or(0);
            (i, c.starts_at())
        })
        .collect();
    // With no sound, input 0 is the silent picture and the silence is input 1.
    let g = graph(usize::from(!has_sound), &gains, &placed);
    let tmp = abs(&out.with_extension("sfx.mp4"));
    args.extend(
        [
            "-filter_complex",
            &g,
            "-map",
            "0:v",
            "-map",
            "[m]",
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-movflags",
            "+faststart",
            &tmp.display().to_string(),
        ]
        .map(String::from),
    );
    ff.run(Path::new("."), &args, None)?;
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())?;
    let r = |v: f64| (v * 100.0).round() / 100.0;
    Ok(json!({
        "level": level.unwrap_or("normal"),
        "under_peak_db": under,
        "base_peak_db": r(base_peak),
        "cues": cues.iter().zip(&placed).map(|(c, p)| json!({
            "name": c.name, "at": c.at, "starts_at": r(p.1), "gain_db": r(gains[p.0]),
        })).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whoosh_starts_before_its_time_and_a_pop_on_it() {
        let cues = parse(&["whoosh@2".into(), "pop@1.5".into(), "whoosh@0.1".into()]).unwrap();
        assert_eq!(cues[0].starts_at(), 1.6);
        assert_eq!(cues[1].starts_at(), 1.5);
        assert_eq!(cues[2].starts_at(), 0.0);
        assert_eq!(file_of("whoosh@2"), None);
        assert_eq!(file_of("a@b.wav@2"), Some(PathBuf::from("a@b.wav")));
    }

    #[test]
    fn bad_specs_say_how_to_write_one() {
        for bad in ["whoosh", "whoosh@", "whoosh@-1", "whoosh@x"] {
            let (msg, fix) = parse(&[bad.into()]).unwrap_err();
            assert!(
                msg.contains("no time") && fix.contains("NAME@SECONDS"),
                "{bad}: {msg}"
            );
        }
        let (msg, fix) = parse(&["boom@1".into()]).unwrap_err();
        assert!(msg.contains("no sound effect boom") && fix.contains("whoosh, pop or tick"));
        assert!(parse(&vec!["pop@1".to_string(); MAX + 1])
            .unwrap_err()
            .0
            .contains("at most 32"));
        assert!(under_db(Some("huge")).is_err());
        assert_eq!(under_db(None), Ok(12.0));
    }

    #[test]
    fn the_graph_places_each_cue_once_and_keeps_the_length_of_the_base() {
        let g = graph(0, &[-3.0, 2.5], &[(0, 1.6), (1, 0.5), (0, 3.0)]);
        assert!(g.contains("[1:a]aresample=48000,aformat=channel_layouts=stereo,volume=-3.00dB,asplit=2[s0_0][s0_1]"), "{g}");
        assert!(
            g.contains("[s0_0]adelay=1600:all=1[d0_0]")
                && g.contains("[s0_1]adelay=3000:all=1[d0_1]"),
            "{g}"
        );
        assert!(
            g.contains("[2:a]") && g.contains("[s1_0]adelay=500:all=1[d1_0]"),
            "{g}"
        );
        assert!(
            g.ends_with("[base][d0_0][d0_1][d1_0]amix=inputs=4:normalize=0:duration=first[m]"),
            "{g}"
        );
        // A video with no sound: the silence is input 1, the first source input 2.
        let g = graph(1, &[0.0], &[(0, 0.0)]);
        assert!(g.starts_with("[1:a]") && g.contains("[2:a]"), "{g}");
    }
}
