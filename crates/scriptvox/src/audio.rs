//! Audio master (PLAN-R1 §5): optional music bed ducked under the narration,
//! then two-pass loudness normalisation. Idea: RM #3102 (ducking was declined
//! there). The beds are synthesised by FFmpeg recipes: no third-party audio,
//! no licence question (open question Q10 resolved this way).

use crate::engine_ffmpeg::Ffmpeg;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Short-form platforms normalise to about -14 LUFS.
pub const TARGET_LUFS: f64 = -14.0;
pub const TRUE_PEAK: f64 = -1.5;

/// A built-in bed as an `aevalsrc` expression (stereo-free, 48 kHz).
pub fn bed(name: &str) -> Option<&'static str> {
    Some(match name {
        // A minor pad that breathes every 4 s.
        "calm" => "0.10*(sin(2*PI*220*t)+0.8*sin(2*PI*261.63*t)+0.7*sin(2*PI*329.63*t))*(0.65+0.35*sin(2*PI*0.25*t))",
        // A low pulse at 100 BPM under a fifth.
        "pulse" => "0.12*sin(2*PI*55*t)*(0.5+0.5*cos(2*PI*1.6667*t))+0.06*(sin(2*PI*220*t)+sin(2*PI*329.63*t))",
        _ => return None,
    })
}

/// A brief's `music_level`: how many dB the bed sits under the voice in the voice's pauses; the duck adds about 9 dB
/// while the voice speaks. `quiet`, `normal` (default) or `loud`.
pub fn music_gap_db(name: Option<&str>) -> Result<f64, String> {
    match name.unwrap_or("normal") {
        "quiet" => Ok(14.0),
        "normal" => Ok(10.0),
        "loud" => Ok(7.0),
        other => Err(format!(
            "unknown music_level {other}: use quiet, normal or loud"
        )),
    }
}

/// Every voice enters the mix at this level (Edge voices arrive near it), so the bed, the duck, the effects and the
/// ambience sit the same way under any engine. The master pass then brings the whole to -14 LUFS.
const VOICE_IN_MIX: f64 = -22.0;

/// The most a track is raised on the way into the mix: a file with only room tone stays quiet, under the bed.
const MAX_GAIN_DB: f64 = 14.0;

/// The ducking chain: input `[bed]` is compressed by the voice `[vs]` (sidechain). It lowers the bed about 9 dB under
/// a voice at `VOICE_IN_MIX` (5 Nov 2026: threshold 0.02, ratio 10 took the bed 31 dB under the voice, so it could not
/// be heard and `music_level` changed nothing).
pub fn duck_chain() -> &'static str {
    "[bed][vs]sidechaincompress=threshold=0.04:ratio=3:attack=15:release=450:makeup=1[duck]"
}

/// A file's integrated loudness; None for silence.
fn loudness(ff: &Ffmpeg, dir: &Path, file: &Path) -> Option<f64> {
    measure(ff, dir, file).ok()?["input_i"]
        .as_str()?
        .parse()
        .ok()
        .filter(|l: &f64| l.is_finite() && *l > -70.0)
}

/// Built-in sound effects, synthesised like the beds (no third-party audio).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sfx {
    /// Pink noise that swells into a scene cut (starts 0.4 s before it).
    Whoosh,
    /// A short falling blip when a card or a list item appears.
    Pop,
    /// A click for each step of a counting number.
    Tick,
}

impl Sfx {
    const ALL: [Sfx; 3] = [Sfx::Whoosh, Sfx::Pop, Sfx::Tick];

    /// A built-in effect by its name, for `edit --sfx`.
    pub fn named(name: &str) -> Option<Sfx> {
        Sfx::ALL
            .into_iter()
            .find(|k| format!("{k:?}").eq_ignore_ascii_case(name))
    }

    /// The lavfi source of one copy of the sound.
    pub(crate) fn source(self) -> &'static str {
        match self {
            Sfx::Whoosh => "anoisesrc=d=0.5:c=pink:a=0.6:r=48000,highpass=f=300,lowpass=f=2500,afade=t=in:d=0.35:curve=exp,afade=t=out:st=0.35:d=0.15",
            Sfx::Pop => "aevalsrc='0.5*sin(2*PI*(500+700*exp(-30*t))*t)*exp(-28*t)':s=48000:d=0.15",
            Sfx::Tick => "aevalsrc='0.25*sin(2*PI*1800*t)*exp(-150*t)':s=48000:d=0.03",
        }
    }

    /// How far before its cue the sound starts (a whoosh peaks at the cut).
    pub fn lead(self) -> f64 {
        if self == Sfx::Whoosh {
            0.4
        } else {
            0.0
        }
    }
}

/// FFmpeg inputs and a graph part that places every cue on one `[sfx]` track. Input numbers start at `first`.
pub fn sfx_track(cues: &[(f64, Sfx)], first: usize) -> (Vec<String>, String) {
    let mut inputs = Vec::new();
    let mut parts = Vec::new();
    let mut labels = String::new();
    let mut idx = first;
    for kind in Sfx::ALL {
        let times: Vec<f64> = cues
            .iter()
            .filter(|c| c.1 == kind)
            .map(|c| (c.0 - kind.lead()).max(0.0))
            .collect();
        if times.is_empty() {
            continue;
        }
        inputs.extend([
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            kind.source().to_string(),
        ]);
        let k = format!("{kind:?}").to_lowercase();
        let outs: String = (0..times.len()).map(|j| format!("[{k}{j}]")).collect();
        parts.push(format!("[{idx}:a]asplit={}{outs}", times.len()));
        for (j, t) in times.iter().enumerate() {
            let ms = (t * 1000.0).round() as u64;
            parts.push(format!("[{k}{j}]adelay={ms}:all=1[{k}d{j}]"));
            labels.push_str(&format!("[{k}d{j}]"));
        }
        idx += 1;
    }
    let n = cues.len();
    parts.push(format!(
        "{labels}amix=inputs={n}:normalize=0,volume=0.6[sfx]"
    ));
    (inputs, parts.join(";"))
}

pub enum Music {
    None,
    Bed(&'static str),
    File(PathBuf),
    /// The base video's own sound (brief `base_sound`): already mixed at its own level, so no extra cut and no
    /// loop; still ducked under the new voice.
    Base(PathBuf),
}

/// Why a music name cannot be used.
pub enum MusicRefused {
    Missing,
    Outside,
}

/// A built-in bed by name, or a music file inside this project.
pub fn pick_music(name: &str) -> Result<Music, MusicRefused> {
    if let Some(expr) = bed(name) {
        return Ok(Music::Bed(expr));
    }
    let p = Path::new(name);
    if !p.is_file() {
        return Err(MusicRefused::Missing);
    }
    let inside = p
        .canonicalize()
        .ok()
        .zip(Path::new(".").canonicalize().ok())
        .is_some_and(|(f, r)| f.starts_with(r));
    if !inside {
        return Err(MusicRefused::Outside);
    }
    Ok(Music::File(
        std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()),
    ))
}

/// A scene clip's own sound (scene `sound`), already cut to its scene: see `clip_sound`.
pub struct ClipSound {
    /// The scene's start on the final timeline, in seconds.
    pub at: f64,
    pub file: PathBuf,
    /// `keep`: at the voice's level and ducked under it. Else `under`: `CLIP_UNDER_DB` below the voice, no duck.
    pub keep: bool,
}

/// How far under the voice a clip's sound sits with `sound: under`: the level of the scene ambience.
pub const CLIP_UNDER_DB: f64 = 12.0;

/// Cuts a clip's sound for its scene into `clip_sound_<n>.wav` (48 kHz stereo): `secs` seconds from `from`, at the
/// clip's own speed, less when the file's sound ends first. Its own bounded pass, so the mix reads a plain file.
/// 30 ms fades, so a cut does not click.
pub fn clip_sound(
    ff: &Ffmpeg,
    dir: &Path,
    src: &Path,
    from: f64,
    secs: f64,
    n: usize,
) -> Result<PathBuf, String> {
    let out = dir.join(format!("clip_sound_{n}.wav"));
    let fades = format!(
        "afade=t=in:d=0.03,afade=t=out:st={:.3}:d=0.03",
        (secs - 0.03).max(0.0)
    );
    let a = [
        "-ss",
        &format!("{from:.3}"),
        "-t",
        &format!("{secs:.3}"),
        "-i",
        &src.to_string_lossy(),
        "-vn",
        "-af",
        &fades,
        "-ar",
        "48000",
        "-ac",
        "2",
        "-c:a",
        "pcm_s16le",
        &out.to_string_lossy(),
    ]
    .map(String::from);
    ff.run(dir, &a, None)?;
    Ok(out)
}

/// Mixes voice and music into `master.wav` at the target loudness. `secs` is the video length.
#[allow(clippy::too_many_arguments)] // one call site
pub fn master(
    ff: &Ffmpeg,
    dir: &Path,
    voice: &Path,
    music: &Music,
    sfx: &[(f64, Sfx)],
    ambience: &[(f64, PathBuf)],
    clips: &[ClipSound],
    secs: f64,
    gap_db: f64,
) -> Result<PathBuf, String> {
    let mix = dir.join("mix.wav");
    let mut a: Vec<String> = vec!["-i".into(), voice.to_string_lossy().into_owned()];
    let fade = (secs - 1.0).max(0.0);
    let voice_db = loudness(ff, dir, voice).map_or(0.0, |l| (VOICE_IN_MIX - l).min(MAX_GAIN_DB));
    let voice_in = format!("[0:a]aresample=48000,volume={voice_db:.2}dB,apad");
    // A built-in bed becomes a file, so every bed is measured and placed the same way.
    let built;
    let bed_file = match music {
        Music::None => None,
        Music::Bed(expr) => {
            built = dir.join("bed.wav");
            let lavfi = format!("aevalsrc='{expr}':s=48000:d={secs:.3}");
            let args = ["-f", "lavfi", "-i", &lavfi, "-ac", "2"].map(String::from);
            let mut args = args.to_vec();
            args.push(built.to_string_lossy().into_owned());
            ff.run(dir, &args, None)?;
            Some(&built)
        }
        Music::File(f) | Music::Base(f) => Some(f),
    };
    let graph = match bed_file {
        None => format!("{voice_in}[m]"),
        Some(f) => {
            let bed_db =
                loudness(ff, dir, f).map_or(0.0, |l| (VOICE_IN_MIX - gap_db - l).min(MAX_GAIN_DB));
            // A music file loops to the length; a base video's sound plays once.
            let (tail, inputs) = if matches!(music, Music::Base(_)) {
                ("apad,", vec![])
            } else {
                ("", vec!["-stream_loop".to_string(), "-1".to_string()])
            };
            a.extend(inputs);
            a.extend(["-i".into(), f.to_string_lossy().into_owned()]);
            format!(
                "{voice_in},asplit[v][vs];[1:a]aresample=48000,{tail}volume={bed_db:.2}dB,afade=t=in:d=1,afade=t=out:st={fade:.3}:d=1[bed];{};[v][duck]amix=inputs=2:normalize=0[m]",
                duck_chain()
            )
        }
    };
    // Sound effects sit on top of the voice and bed, before the loudness pass, never ducked (they are short).
    let graph = if sfx.is_empty() {
        graph
    } else {
        let first = if matches!(music, Music::None) { 1 } else { 2 };
        let (inputs, part) = sfx_track(sfx, first);
        a.extend(inputs);
        format!(
            "{}[m0];{part};[sfx]apad[sp];[m0][sp]amix=inputs=2:normalize=0[m]",
            graph.strip_suffix("[m]").unwrap_or(&graph)
        )
    };
    // Scene sound (brief `ambience`): each clip's sound from its scene start, about 12 dB under the voice, short
    // fades so a cut does not click.
    let graph = if ambience.is_empty() {
        graph
    } else {
        let first = a.iter().filter(|x| *x == "-i").count();
        let mut parts = Vec::new();
        let mut labels = String::new();
        for (j, (t, f)) in ambience.iter().enumerate() {
            a.extend(["-i".into(), f.to_string_lossy().into_owned()]);
            let ms = (t * 1000.0).round() as u64;
            let n = first + j;
            parts.push(format!(
                "[{n}:a]aresample=48000,volume=0.25,afade=t=in:d=0.3,areverse,afade=t=in:d=0.3,areverse,adelay={ms}:all=1[amb{j}]"
            ));
            labels.push_str(&format!("[amb{j}]"));
        }
        format!(
            "{}[m1];{};{labels}amix=inputs={}:normalize=0,apad[ap];[m1][ap]amix=inputs=2:normalize=0[m]",
            graph.strip_suffix("[m]").unwrap_or(&graph),
            parts.join(";"),
            ambience.len()
        )
    };
    // Scene clip sound (scene `sound`): each file from its scene start, placed by its measured loudness as the bed
    // is. `under` sits `CLIP_UNDER_DB` below the voice; `keep` sits at the voice's level and goes through the bed's
    // duck, with the voice read once more as a plain input for the side chain. Last stage: no clip, the old graph.
    let graph = if clips.is_empty() {
        graph
    } else {
        let first = a.iter().filter(|x| *x == "-i").count();
        let mut parts = Vec::new();
        let (mut under, mut keep) = (Vec::new(), Vec::new());
        for (j, c) in clips.iter().enumerate() {
            a.extend(["-i".into(), c.file.to_string_lossy().into_owned()]);
            let aim = VOICE_IN_MIX - if c.keep { 0.0 } else { CLIP_UNDER_DB };
            let db = loudness(ff, dir, &c.file).map_or(0.0, |l| (aim - l).min(MAX_GAIN_DB));
            let ms = (c.at * 1000.0).round() as u64;
            parts.push(format!(
                "[{}:a]aresample=48000,volume={db:.2}dB,adelay={ms}:all=1[cs{j}]",
                first + j
            ));
            if c.keep { &mut keep } else { &mut under }.push(format!("[cs{j}]"));
        }
        let mut ends = String::from("[m2]");
        if !under.is_empty() {
            parts.push(format!(
                "{}amix=inputs={}:normalize=0,apad[cu]",
                under.concat(),
                under.len()
            ));
            ends.push_str("[cu]");
        }
        if !keep.is_empty() {
            a.extend(["-i".into(), voice.to_string_lossy().into_owned()]);
            parts.push(format!(
                "{}amix=inputs={}:normalize=0,apad[ck];[{}:a]aresample=48000,volume={voice_db:.2}dB,apad[cv];{}",
                keep.concat(),
                keep.len(),
                first + clips.len(),
                duck_chain()
                    .replace("[bed]", "[ck]")
                    .replace("[vs]", "[cv]")
                    .replace("[duck]", "[cd]")
            ));
            ends.push_str("[cd]");
        }
        format!(
            "{}[m2];{};{ends}amix=inputs={}:normalize=0[m]",
            graph.strip_suffix("[m]").unwrap_or(&graph),
            parts.join(";"),
            ends.matches('[').count()
        )
    };
    a.extend([
        "-filter_complex".into(),
        graph,
        "-map".into(),
        "[m]".into(),
        "-t".into(),
        format!("{secs:.3}"),
        "-ac".into(),
        "2".into(),
    ]);
    // Float: a peak over full scale in the mix survives to the loudness pass, which brings it down.
    a.extend([
        "-c:a".into(),
        "pcm_f32le".into(),
        mix.to_string_lossy().into_owned(),
    ]);
    ff.run(dir, &a, None)?;
    // Pass 1: measure. Pass 2: linear normalisation with the measured values.
    let m = measure(ff, dir, &mix)?;
    let get = |k: &str| m[k].as_str().unwrap_or("0").to_string();
    // Plain silence (no voice, no music) has no level to normalise.
    let silent = !get("input_i").parse::<f64>().is_ok_and(f64::is_finite);
    let ln = format!(
        "loudnorm=I={TARGET_LUFS}:TP={TRUE_PEAK}:LRA=11:measured_I={}:measured_TP={}:measured_LRA={}:measured_thresh={}:offset={}:linear=true",
        get("input_i"),
        get("input_tp"),
        get("input_lra"),
        get("input_thresh"),
        get("target_offset")
    );
    let out = dir.join("master.wav");
    let a: Vec<String> = vec![
        "-i".into(),
        mix.to_string_lossy().into_owned(),
        "-af".into(),
        if silent {
            "aresample=48000".into()
        } else {
            format!("{ln},aresample=48000")
        },
        "-c:a".into(),
        "pcm_s16le".into(),
        out.to_string_lossy().into_owned(),
    ];
    ff.run(dir, &a, None)?;
    Ok(out)
}

/// `loudnorm print_format=json` values for a file.
pub fn measure(ff: &Ffmpeg, dir: &Path, file: &Path) -> Result<Value, String> {
    let o = std::process::Command::new(&ff.ffmpeg)
        .current_dir(dir)
        .args(["-hide_banner", "-nostdin"])
        .args(crate::media_gate::input_in(dir, file)?)
        .args([
            "-af",
            &format!("loudnorm=I={TARGET_LUFS}:TP={TRUE_PEAK}:LRA=11:print_format=json"),
            "-f",
            "null",
            "-",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    let err = String::from_utf8_lossy(&o.stderr);
    let start = err.rfind('{').ok_or("loudnorm gave no measurement")?;
    let end = err[start..]
        .find('}')
        .map(|e| start + e + 1)
        .ok_or("loudnorm gave no measurement")?;
    serde_json::from_str(&err[start..end]).map_err(|e| format!("loudnorm output: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sfx_track_places_each_cue() {
        let (inputs, g) = sfx_track(
            &[(2.0, Sfx::Whoosh), (3.0, Sfx::Pop), (5.5, Sfx::Whoosh)],
            2,
        );
        assert_eq!(
            inputs.iter().filter(|a| *a == "lavfi").count(),
            2,
            "one source per kind used"
        );
        assert!(g.starts_with("[2:a]asplit=2[whoosh0][whoosh1]"), "{g}");
        assert!(g.contains("[whoosh0]adelay=1600:all=1[whooshd0]"), "{g}");
        assert!(
            g.contains("[3:a]asplit=1[pop0];[pop0]adelay=3000:all=1[popd0]"),
            "{g}"
        );
        assert!(
            g.ends_with("amix=inputs=3:normalize=0,volume=0.6[sfx]"),
            "{g}"
        );
    }

    #[test]
    fn beds_exist() {
        assert!(bed("calm").is_some() && bed("pulse").is_some() && bed("rock").is_none());
    }
}
