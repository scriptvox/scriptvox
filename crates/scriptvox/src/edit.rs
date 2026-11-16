//! `scriptvox edit`: change an existing video, locally and free. `--cut-silence` removes pauses longer than
//! `--min-silence`, keeping `--pad` seconds of air at each cut; `--cut-fillers` cuts "um", "uh" and "اممم" found by
//! speech to text; `--reframe` crops to another frame shape around the
//! centre, or with `--track` along the subject (see `track.rs`). One FFmpeg pass, re-encoded with the same encoder choice as `make`.

use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Args {
    pub input: PathBuf,
    pub cut_silence: bool,
    /// Cut where the picture stands still for --min-silence or more, and the sound is silent too (G070).
    pub cut_still: bool,
    pub min_silence: f64,
    pub pad: f64,
    pub noise_db: f64,
    pub reframe: Option<String>,
    /// `blur`: the whole picture over a blurred copy. Default: the crop.
    pub fit: Option<String>,
    /// Follow the subject (a vision row gives its box) instead of the centre crop.
    pub track: bool,
    /// Crop a ScriptVox MP4 anyway (its burned captions and cards get cut).
    pub crop_text: bool,
    /// Cut hesitation sounds ("um", "uh", "اممم") with the pause around each, from the video's own speech.
    pub cut_fillers: bool,
    /// What to follow, for example "the camel" (default: the main subject).
    pub subject: Option<String>,
    /// Keep only these parts, "a-b,c-d" in seconds, in time order.
    pub keep: Option<String>,
    /// A transcript with words removed: the rest is kept.
    pub keep_words: Option<PathBuf>,
    /// Words to hide in the sound, "word,two words": a tone or silence over each, the length stays (G102).
    pub bleep: Option<String>,
    /// `tone` (default) or `silence`.
    pub bleep_with: Option<String>,
    /// Seconds of air on each side of a bleeped word, 0 to 1. Default: 0.05 before and 0.2 after.
    pub bleep_pad: Option<f64>,
    /// The heard words with times for `bleep`, instead of speech to text.
    pub words: Option<PathBuf>,
    /// Pictures over the talk, `file@12.0:4`: the file, seconds of the result, how long. The sound stays.
    pub broll: Vec<String>,
    /// Blur a fixed box, "x,y,w,h@12-15": pixels and seconds of the source (G068).
    pub blur: Vec<String>,
    /// Slow pushes in, "12-15@1.2" or "12-15@1.2,0.3,0.4": seconds of the result, the zoom, the centre as shares.
    pub zoom: Vec<String>,
    /// A small push in on each `--emphasis` word.
    pub zoom_on_emphasis: bool,
    /// Lower the noise under a voice: light, medium or strong.
    pub clean_voice: Option<String>,
    /// Cut a sentence said again: the last whole take stays.
    pub cut_retakes: bool,
    /// With `cut_retakes`: list the removals and render nothing.
    pub plan: bool,
    /// Play faster or slower, 0.5 to 2.
    pub speed: Option<f64>,
    /// Speed over time ranges with an ease, "12-15@0.5,20-22@2": seconds of the video after any cut (G195).
    pub speed_ranges: Option<String>,
    /// The length of the result in seconds: the speed is the kept length over it, 0.5 to 2 (G069).
    pub duration: Option<f64>,
    /// Keep only this part of the input (seconds).
    pub from: Option<f64>,
    pub to: Option<f64>,
    /// Burn captions from the video's own speech: "karaoke", "pop" or "reveal" ("boxed" is the old name of karaoke).
    pub captions: Option<String>,
    /// Captions from a file, SRT or word JSON, on its own times. No speech to text.
    pub captions_file: Option<PathBuf>,
    /// The words the speaker said, as a text file: the captions spell these, on the heard times.
    pub text: Option<PathBuf>,
    /// A small second caption line under the captions, one line of this file per sentence (`captions-second.md`).
    pub captions_second: Option<PathBuf>,
    /// Language of the speech for the captions; None: detected.
    pub lang: Option<String>,
    /// A big headline for the first 1.6 s.
    pub hook: Option<String>,
    /// A small line in the corner for the whole video.
    pub label: Option<String>,
    /// Words of the speech to mark in the captions, comma separated.
    pub emphasis: Option<String>,
    /// A music bed under the sound: "calm", "pulse", a file in this project, or an https address (`fetch.md` 10).
    pub music: Option<String>,
    /// A new voice: the row for lip sync moves the mouth in the video to it, first.
    pub lipsync: Option<PathBuf>,
    /// quiet, normal or loud.
    pub music_level: Option<String>,
    /// Sound effects at set times, `whoosh@12.0`: a built-in name or a file, and seconds of the result (G191).
    pub sfx: Vec<String>,
    /// quiet, normal or loud.
    pub sfx_level: Option<String>,
    /// A logo (PNG) in the top right corner, 16 percent of the width.
    pub logo: Option<PathBuf>,
    /// The brand colour for the spoken-word highlight (#RRGGBB).
    pub accent: Option<String>,
    pub terms: Option<crate::say::Terms>,
    /// A new background behind the subject: `#RRGGBB`, `blur`, or a picture or clip in this project (G056).
    pub background: Option<String>,
    /// The subject on a transparent background: a `.mov` and a `.webm` with alpha.
    pub alpha: bool,
    /// The picture through the upscale row: the same clip, larger (G122).
    pub enhance: bool,
    /// The spend cap for a paid row (`--max-spend`).
    pub max_spend: Option<f64>,
    pub out: Option<PathBuf>,
    pub use_system: bool,
    /// `--remove-bars`, `--hold`, `--loop`, `--loop-to`, `--deinterlace`, `--tilt` (`edit_small.rs`, G133).
    pub small: crate::edit_small::Ops,
    /// `--caption-size`, `--caption-outline`, `--caption-box`: the style of the captions, as in a brief.
    pub caption_style: crate::caption_style::Style,
    /// `--music-from`, `--music-to`: the cut of music from an address, as `fetch` takes its times.
    pub music_cut: (Option<String>, Option<String>),
}

pub(crate) fn fail(exit: Exit, msg: impl Into<String>, fix: &str) -> Envelope {
    if exit == Exit::ApprovalNeeded {
        return crate::plan::approval_fail("edit", msg.into());
    }
    Envelope::fail("edit", exit, msg.into(), Some(fix.into()))
}

/// The parts to keep, given the silences. Each kept part grows by `pad` into the silence on both sides; parts that
/// then touch are merged; parts shorter than 0.1 s are dropped.
/// The select expression for kept parts, the same for picture and sound. Each part is snapped to the frame grid
/// and kept as a half-open window centred between frames, so the frames kept times the frame length equals the
/// sound kept. With `between(t,s,e)` each cut could keep one frame more or less than the sound: 4 cuts drifted
/// 47 ms (4 Nov 2026 eval, gate av_drift failed after `edit --cut-silence`).
pub fn select_expr(parts: &[(f64, f64)], fps: f64) -> String {
    let half = 0.5 / fps;
    parts
        .iter()
        .map(|(s, e)| {
            let (a, b) = ((s * fps).round() / fps, (e * fps).round() / fps);
            format!("gte(t,{:.5})*lt(t,{:.5})", a - half, b - half)
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// The sound filter for kept parts: the same snapped windows as the picture, on frames of 64 samples. A decoded AAC
/// frame is 21 ms and `aselect` takes or drops a whole frame, so each cut lost or gained up to 21 ms against the
/// picture (12 Nov 2026: `edit --cut-silence` of a 24 s talk, video 17.080 s, sound 17.024 s).
pub fn sound_select(parts: &[(f64, f64)], fps: f64) -> String {
    let expr = parts
        .iter()
        .map(|(s, e)| {
            let (a, b) = ((s * fps).round() / fps, (e * fps).round() / fps);
            format!("gte(t,{a:.5})*lt(t,{b:.5})")
        })
        .collect::<Vec<_>>()
        .join("+");
    format!("asetnsamples=n=64:p=0,aselect='{expr}',asetpts=N/SR/TB")
}

/// Sound to the same -14 LUFS as `make` (a cut of a quiet talk came out at -20.6): measure, then one linear loudnorm
/// pass on the sound only; the picture is copied. A video with no sound is left as it is.
pub fn master(ff: &Ffmpeg, out: &Path) -> Option<f64> {
    let abs = |p: &Path| {
        std::path::absolute(p)
            .unwrap_or(p.to_path_buf())
            .display()
            .to_string()
    };
    let m = crate::audio::measure(ff, Path::new("."), out).ok()?;
    let get = |k: &str| m[k].as_str().unwrap_or("0").to_string();
    let i: f64 = get("input_i")
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())?;
    if (i - crate::audio::TARGET_LUFS).abs() <= 1.0 {
        return Some(i);
    }
    let tmp = out.with_extension("loud.mp4");
    // The new AAC encode adds its own start delay, so the sound is cut at the picture's end. Not with -shortest: on a
    // copied picture it dropped the last 2 or 3 frames (`edit --keep 0-2`: video 1.90 s, sound 1.98 s, gate av_drift failed).
    let end = ff.video_duration(out).ok()?;
    let ln = format!(
        "loudnorm=I={}:TP={}:LRA=11:measured_I={}:measured_TP={}:measured_LRA={}:measured_thresh={}:offset={}:linear=true",
        crate::audio::TARGET_LUFS,
        crate::audio::TRUE_PEAK,
        get("input_i"),
        get("input_tp"),
        get("input_lra"),
        get("input_thresh"),
        get("target_offset")
    );
    let args: Vec<String> = [
        "-i".to_string(),
        abs(out),
        "-c:v".into(),
        "copy".into(),
        "-af".into(),
        // loudnorm works at 192 kHz inside; without this the AAC came out at 96 kHz.
        // `asetpts`: loudnorm's times had a jump of 0.08 s on a cut talk, so `atrim` dropped sound that was there and
        // the AAC got a hole (12 Nov 2026: `edit --keep` of 2 parts, then `join` or a second cut failed gate av_drift).
        format!("{ln},aresample=48000,asetpts=N/SR/TB,atrim=end={end:.5}"),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-movflags".into(),
        "+faststart".into(),
        abs(&tmp),
    ]
    .to_vec();
    ff.run(Path::new("."), &args, None).ok()?;
    std::fs::rename(&tmp, out).ok()?;
    Some(crate::audio::TARGET_LUFS)
}

pub fn keep_segments(silences: &[(f64, f64)], total: f64, pad: f64) -> Vec<(f64, f64)> {
    let mut keep = Vec::new();
    let mut t = 0.0;
    for &(s, e) in silences {
        if s > t {
            keep.push((t, s));
        }
        t = t.max(e);
    }
    if total > t {
        keep.push((t, total));
    }
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (a, b) in keep {
        let (a, b) = ((a - pad).max(0.0), (b + pad).min(total));
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out.retain(|(a, b)| b - a >= 0.1);
    out
}

/// Silences from FFmpeg's silencedetect log: (start, end) pairs. A silence that runs to the end has no end line.
fn parse_silences(log: &str, total: f64) -> Vec<(f64, f64)> {
    let num = |s: &str| {
        s.split_whitespace()
            .next()?
            .trim_end_matches('|')
            .parse::<f64>()
            .ok()
    };
    let starts: Vec<f64> = log
        .split("silence_start:")
        .skip(1)
        .filter_map(num)
        .collect();
    let ends: Vec<f64> = log.split("silence_end:").skip(1).filter_map(num).collect();
    starts
        .iter()
        .enumerate()
        .map(|(i, &s)| (s.max(0.0), ends.get(i).copied().unwrap_or(total)))
        .collect()
}

/// Still pictures from FFmpeg's freezedetect log: (start, end). A freeze that runs to the end has no end line.
fn parse_freezes(log: &str, total: f64) -> Vec<(f64, f64)> {
    let num = |s: &str| s.split_whitespace().next()?.parse::<f64>().ok();
    let starts: Vec<f64> = log.split("freeze_start:").skip(1).filter_map(num).collect();
    let ends: Vec<f64> = log.split("freeze_end:").skip(1).filter_map(num).collect();
    starts
        .iter()
        .enumerate()
        .map(|(i, &s)| (s.max(0.0), ends.get(i).copied().unwrap_or(total)))
        .collect()
}

/// Where both lists hold: a still picture that is also silent.
fn overlap(a: &[(f64, f64)], b: &[(f64, f64)], min: f64) -> Vec<(f64, f64)> {
    let mut out = vec![];
    for &(s, e) in a {
        for &(t, u) in b {
            let (x, y) = (s.max(t), e.min(u));
            if y - x >= min {
                out.push((x, y));
            }
        }
    }
    out.sort_by(|p, q| p.0.total_cmp(&q.0));
    out
}

/// One picture over the talk. Seconds of the result.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Broll {
    file: PathBuf,
    at: f64,
    seconds: f64,
}

/// `file@12.0:4`, in order, not overlapping, inside the video. The last `@` splits it: a file name can hold one.
pub fn parse_broll(specs: &[String], total: f64) -> Result<Vec<Broll>, String> {
    let mut out: Vec<Broll> = Vec::new();
    for spec in specs {
        let parts = spec.rsplit_once('@').and_then(|(file, when)| {
            let (at, seconds) = when.split_once(':')?;
            let num = |s: &str| s.trim().parse::<f64>().ok().filter(|v| v.is_finite());
            Some((file, num(at)?, num(seconds)?))
        });
        let Some((file, at, seconds)) = parts.filter(|p| !p.0.is_empty()) else {
            return Err(format!("{spec} is not file@start:seconds"));
        };
        if at < 0.0 || seconds < 0.5 {
            return Err(format!("{spec} runs under 0.5 s"));
        }
        if at + seconds > total + 0.01 {
            return Err(format!("{spec} runs past the end ({total:.2} s)"));
        }
        if out.last().is_some_and(|l| at < l.at + l.seconds) {
            return Err(format!("{spec} starts before the one in front of it ends"));
        }
        out.push(Broll {
            file: file.into(),
            at,
            seconds,
        });
    }
    Ok(out)
}

fn is_picture(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp"))
}

/// A picture repeats for its time. A clip reads no further than its time, and only its picture is used.
fn broll_inputs(brolls: &[Broll], abs: &dyn Fn(&Path) -> String) -> Vec<String> {
    brolls
        .iter()
        .flat_map(|b| {
            let mut a = vec![];
            if is_picture(&b.file) {
                a.extend(["-loop".to_string(), "1".to_string()]);
            }
            a.extend([
                "-t".to_string(),
                format!("{:.3}", b.seconds),
                "-i".to_string(),
                abs(&b.file),
            ]);
            a
        })
        .collect()
}

/// The picture graph: `vf` on the talk, then each file over it. Each fills the frame (a crop, nothing stretched), and a
/// clip shorter than its time holds its last frame.
fn broll_graph(
    vf: &[String],
    brolls: &[Broll],
    first_input: usize,
    (w, h): (u32, u32),
    fps: f64,
) -> (String, String) {
    let base = if vf.is_empty() {
        "null".to_string()
    } else {
        vf.join(",")
    };
    let mut g = format!("[0:v]{base}[base]");
    let mut label = "[base]".to_string();
    for (i, b) in brolls.iter().enumerate() {
        g.push_str(&format!(
            ";[{n}:v]scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1,fps={fps},format=yuv420p,tpad=stop_mode=clone:stop_duration={s:.3},trim=duration={s:.3},setpts=PTS-STARTPTS+{at:.3}/TB[br{i}];{label}[br{i}]overlay=enable='between(t,{at:.3},{end:.3})':eof_action=pass[over{i}]",
            n = first_input + i,
            s = b.seconds,
            at = b.at,
            end = b.at + b.seconds,
        ));
        label = format!("[over{i}]");
    }
    (g, label)
}

/// One blurred box: pixels and seconds of the source.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Blur {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    from: f64,
    to: f64,
}

/// `x,y,w,h@12-15`, inside a `fw` by `fh` picture of `total` seconds; each box 8 pixels or more each way.
pub fn parse_blur(specs: &[String], (fw, fh): (u32, u32), total: f64) -> Result<Vec<Blur>, String> {
    specs
        .iter()
        .map(|s| {
            let bad = || format!("{s} is not x,y,w,h@from-to");
            let (bx, t) = s.split_once('@').ok_or_else(bad)?;
            let n: Vec<u32> = bx.split(',').map(|v| v.trim().parse().map_err(|_| bad())).collect::<Result<_, _>>()?;
            let (from, to) = t.split_once('-').ok_or_else(bad)?;
            let (from, to): (f64, f64) = (from.trim().parse().map_err(|_| bad())?, to.trim().parse().map_err(|_| bad())?);
            let [x, y, w, h] = n[..] else { return Err(bad()) };
            if w < 8 || h < 8 || x + w > fw || y + h > fh {
                return Err(format!("{s}: the box must be 8 pixels or more each way and inside the {fw}x{fh} picture"));
            }
            if !(from >= 0.0 && to > from && to <= total + 0.01) {
                return Err(format!("{s}: the time must be inside the video ({total:.2} s), start before end"));
            }
            Ok(Blur { x, y, w, h, from, to })
        })
        .collect()
}

/// The box cut out, blurred hard (a radius of half its short side, three passes) and laid back for its time.
fn blur_filter(b: &Blur, i: usize) -> String {
    // boxblur's radius must fit the plane: the colour planes are half size.
    let short = b.w.min(b.h);
    let (r, cr) = (
        (short / 2).saturating_sub(1).clamp(1, 60),
        (short / 4).saturating_sub(1).clamp(1, 30),
    );
    format!(
        "split[bl{i}a][bl{i}b];[bl{i}b]crop={}:{}:{}:{},boxblur=lr={r}:lp=3:cr={cr}:cp=3[bl{i}c];[bl{i}a][bl{i}c]overlay={}:{}:enable='between(t,{:.3},{:.3})'",
        b.w, b.h, b.x, b.y, b.x, b.y, b.from, b.to
    )
}

/// One push in. Seconds of the result; `x` and `y` are the centre as shares of the frame.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Zoom {
    start: f64,
    end: f64,
    z: f64,
    x: f64,
    y: f64,
}

/// `12-15@1.2` or `12-15@1.2,0.3,0.4`, in order, not overlapping, inside the video.
pub fn parse_zooms(specs: &[String], total: f64) -> Result<Vec<Zoom>, String> {
    let mut out: Vec<Zoom> = Vec::new();
    for spec in specs {
        let bad = || format!("{spec} is not start-end@zoom or start-end@zoom,x,y");
        let (span, rest) = spec.split_once('@').ok_or_else(bad)?;
        let (start, end) = span.split_once('-').ok_or_else(bad)?;
        let num = |s: &str| s.trim().parse::<f64>().ok().filter(|v| v.is_finite());
        let (start, end) = (num(start).ok_or_else(bad)?, num(end).ok_or_else(bad)?);
        let nums: Vec<f64> = rest
            .split(',')
            .map(num)
            .collect::<Option<_>>()
            .ok_or_else(bad)?;
        let (z, x, y) = match nums[..] {
            [z] => (z, 0.5, 0.5),
            [z, x, y] => (z, x, y),
            _ => return Err(bad()),
        };
        if !(1.05..=2.0).contains(&z) {
            return Err(format!("{spec}: the zoom is {z}, and it must be 1.05 to 2"));
        }
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return Err(format!("{spec}: the centre must be 0 to 1"));
        }
        if start < 0.0 || end - start < 0.8 {
            return Err(format!("{spec} runs under 0.8 s"));
        }
        if end > total + 0.01 {
            return Err(format!("{spec} runs past the end ({total:.2} s)"));
        }
        if out.last().is_some_and(|l| start < l.end) {
            return Err(format!("{spec} starts before the zoom in front of it ends"));
        }
        out.push(Zoom {
            start,
            end,
            z,
            x,
            y,
        });
    }
    Ok(out)
}

/// A small push on spoken words, from a little before each until a beat after. Close ones become one.
fn emphasis_zooms(words: &[(f64, f64)]) -> Vec<Zoom> {
    let mut out: Vec<Zoom> = Vec::new();
    for &(s, e) in words {
        let (start, end) = ((s - 0.3).max(0.0), (e + 0.5).max(s + 0.9));
        match out.last_mut() {
            Some(l) if start <= l.end + 0.3 => l.end = end,
            _ => out.push(Zoom {
                start,
                end,
                z: 1.12,
                x: 0.5,
                y: 0.5,
            }),
        }
    }
    out
}

/// One `zoompan` for all pushes, on a 2x frame for half-pixel steps, as `media::camera` does. Each eases in and out over
/// up to 0.4 s. It stays on the frame's edge when the centre is near it.
fn zoom_filter(zooms: &[Zoom], (w, h): (u32, u32), fps: f64) -> String {
    let t = format!("(in/{fps})");
    let mut z = String::from("1");
    let (mut cx, mut cy) = (String::from("0.5"), String::from("0.5"));
    for k in zooms {
        let r = ((k.end - k.start) / 2.0).min(0.4);
        let e = format!(
            "clip(min(({t}-{s})/{r},({e}-{t})/{r}),0,1)",
            s = k.start,
            e = k.end
        );
        z = format!("{z}+{}*{e}*{e}*(3-2*{e})", k.z - 1.0);
        let on = format!("between({t},{},{})", k.start, k.end);
        cx = format!("if({on},{},{cx})", k.x);
        cy = format!("if({on},{},{cy})", k.y);
    }
    format!(
        "scale={}:{},zoompan=z='{z}':x='clip({cx}*iw-iw/zoom/2,0,iw-iw/zoom)':y='clip({cy}*ih-ih/zoom/2,0,ih-ih/zoom)':d=1:s={w}x{h}:fps={fps}",
        w * 2,
        h * 2
    )
}

/// The noise filter of a `--clean-voice` level. Measured 7 Nov 2026 on a voice over pink noise: `afftdn` took the floor
/// from -44 dB to -49, -50.5 and -52 dB and left the voice level alone (-17.26 dB RMS). `anlmdn` moved it 0.4 dB.
fn clean_filter(level: &str) -> Option<&'static str> {
    match level {
        "light" => Some("afftdn=nr=8:nf=-45:tn=1"),
        "medium" => Some("afftdn=nr=14:nf=-50:tn=1"),
        "strong" => Some("afftdn=nr=24:nf=-60:tn=1"),
        _ => None,
    }
}

/// The noise floor of a file's sound in dB, from FFmpeg `astats`. None: no sound, or digital silence under the voice.
pub fn noise_floor(ff: &Ffmpeg, input: &Path) -> Option<f64> {
    let o = std::process::Command::new(&ff.ffmpeg)
        .args(["-hide_banner", "-nostats"])
        .args(crate::media_gate::input(input).ok()?)
        .args([
            "-vn",
            "-af",
            "astats=measure_perchannel=none:measure_overall=Noise_floor",
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
        .find_map(|l| l.split("Noise floor dB:").nth(1))?;
    v.trim().parse().ok().filter(|v: &f64| v.is_finite())
}

#[derive(serde::Deserialize)]
struct Sentence {
    text: String,
    start: f64,
    end: f64,
}

/// A sentence to cut, and the take that stays.
#[derive(Debug, serde::Serialize)]
pub struct Retake {
    start: f64,
    end: f64,
    text: String,
    kept: String,
    differs: Vec<String>,
}

/// A later take must be this close. A sentence said again after 2 others, or after 10 s, is a choice, not a retake.
const RETAKE_LOOK: usize = 2;
const RETAKE_GAP_S: f64 = 10.0;
/// Shared words over the longer take. "One liter in every four leaks away", then "... three leaks away underground".
const RETAKE_SAME: f64 = 0.75;

/// Words only one of two takes has. None: the two are not takes of one sentence. `first` being the start of `then`
/// (a false start) is a take, and the rest of `then` is no difference. Words pass through `gates::norm_word`, so Arabic
/// spellings and short vowel marks do not count as different.
fn take_diff(first: &str, then: &str) -> Option<Vec<String>> {
    let words = |s: &str| -> Vec<String> {
        s.split_whitespace()
            .map(crate::gates::norm_word)
            .filter(|w| !w.is_empty())
            .collect()
    };
    let (a, b) = (words(first), words(then));
    if a.len() < 2 || b.len() < a.len().div_ceil(2) {
        return None;
    }
    if b.starts_with(&a) {
        return Some(vec![]);
    }
    let mut table = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            table[i][j] = if a[i] == b[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    if (table[0][0] as f64) < RETAKE_SAME * a.len().max(b.len()) as f64 {
        return None;
    }
    let (mut i, mut j) = (0, 0);
    let mut diff = Vec::new();
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            (i, j) = (i + 1, j + 1);
        } else if j == b.len() || (i < a.len() && table[i + 1][j] >= table[i][j + 1]) {
            diff.push(a[i].clone());
            i += 1;
        } else {
            diff.push(b[j].clone());
            j += 1;
        }
    }
    Some(diff)
}

/// Each sentence with a later take close by. In a chain of takes only the last stays.
/// ponytail: by words, so two meant sentences that look alike are found too; `--plan` and `retake_differs` show them.
fn retakes(said: &[Sentence]) -> Vec<Retake> {
    let mut out = Vec::new();
    for (i, s) in said.iter().enumerate() {
        let later = said[i + 1..]
            .iter()
            .take(RETAKE_LOOK)
            .take_while(|n| n.start - s.end <= RETAKE_GAP_S)
            .find_map(|n| take_diff(&s.text, &n.text).map(|d| (n, d)));
        if let Some((n, differs)) = later {
            out.push(Retake {
                start: s.start,
                end: s.end,
                text: s.text.clone(),
                kept: n.text.clone(),
                differs,
            });
        }
    }
    out
}

/// A part of the video, start and end in seconds.
type Span = (f64, f64);

/// A caption file: SRT, or word JSON by its extension.
fn read_caption_file(file: &Path) -> Result<Vec<crate::captions::Word>, String> {
    let is_json = file
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    let words = if is_json {
        read_words(file)?
    } else {
        let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
        parse_srt(&decode_srt(&bytes).map_err(|e| format!("{}: {e}", file.display()))?)
    };
    if words.is_empty() {
        return Err(format!("{} has no captions", file.display()));
    }
    Ok(words)
}

/// Windows-1256 for bytes 0x80 to 0xFF, as published by the code page table.
const CP1256: &str = "€پ‚ƒ„…†‡ˆ‰ٹ‹Œچژڈگ‘’“”•–—ک™ڑ›œ\u{200c}\u{200d}ں\u{a0}،¢£¤¥¦§¨©ھ«¬\u{ad}®¯°±²³´µ¶·¸¹؛»¼½¾؟ہءآأؤإئابةتثجحخدذرزسشصض×طظعغـفقكàلâمنهوçèéêëىيîïً\u{64c}\u{64d}\u{64e}ô\u{64f}\u{650}÷\u{651}ù\u{652}ûü\u{200e}\u{200f}ے";

/// A caption file's bytes as text: a BOM (UTF-8, UTF-16), then UTF-8, then Windows-1256 when the high bytes read as
/// Arabic. Any other bytes, or replacement characters already in the file, stop with the cause: text read wrong burns
/// garbage into a video without any error.
pub fn decode_srt(bytes: &[u8]) -> Result<String, String> {
    let bad = "the file holds replacement characters (U+FFFD), so it was damaged by an earlier conversion. Export it again from the original as UTF-8";
    let text = if let Some(b) = bytes.strip_prefix(b"\xef\xbb\xbf") {
        String::from_utf8(b.to_vec())
            .map_err(|_| "it has a UTF-8 mark but is not UTF-8. Save it as UTF-8".to_string())?
    } else if let Some((b, le)) = bytes
        .strip_prefix(b"\xff\xfe")
        .map(|b| (b, true))
        .or(bytes.strip_prefix(b"\xfe\xff").map(|b| (b, false)))
    {
        let units = b.as_chunks::<2>().0.iter().map(|c| {
            if le {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        });
        char::decode_utf16(units)
            .collect::<Result<String, _>>()
            .map_err(|_| "it has a UTF-16 mark but is not UTF-16. Save it as UTF-8".to_string())?
    } else if let Ok(t) = std::str::from_utf8(bytes) {
        t.to_string()
    } else {
        let table: Vec<char> = CP1256.chars().collect();
        let high: Vec<char> = bytes
            .iter()
            .filter(|b| **b >= 0x80)
            .map(|b| table[usize::from(b - 0x80)])
            .collect();
        let arabic = high
            .iter()
            .filter(|c| ('\u{600}'..='\u{6ff}').contains(*c))
            .count();
        // ponytail: 70 percent separates Arabic from Latin accents in these code pages; a short file can still pass wrongly.
        if high.is_empty() || arabic * 10 < high.len() * 7 {
            return Err(format!(
                "it is not UTF-8 and its bytes do not read as Arabic in Windows-1256 ({arabic} of {} high bytes). Save it as UTF-8",
                high.len()
            ));
        }
        bytes
            .iter()
            .map(|b| {
                if *b < 0x80 {
                    char::from(*b)
                } else {
                    table[usize::from(b - 0x80)]
                }
            })
            .collect()
    };
    if text.contains('\u{fffd}') {
        return Err(bad.to_string());
    }
    Ok(text)
}

/// `00:00:01,500` or `00:00:01.500` in seconds.
fn srt_time(t: &str) -> Option<f64> {
    let mut p = t.trim().split(':');
    let (h, m, s) = (p.next()?, p.next()?, p.next()?);
    let s: f64 = s.replace(',', ".").parse().ok()?;
    Some(h.parse::<f64>().ok()? * 3600.0 + m.parse::<f64>().ok()? * 60.0 + s)
}

/// Words from SRT cues: each cue's time is shared among its words by their letters. Tags such as `<i>` and `{\\an8}` go.
pub fn parse_srt(text: &str) -> Vec<crate::captions::Word> {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut out = Vec::new();
    for cue in text.split("\n\n") {
        let mut lines = cue.lines().skip_while(|l| !l.contains("-->"));
        let Some(times) = lines.next() else { continue };
        let Some((a, b)) = times.split_once("-->") else {
            continue;
        };
        let b = b.split_whitespace().next().unwrap_or("");
        let (Some(start), Some(end)) = (srt_time(a), srt_time(b)) else {
            continue;
        };
        let mut clean = String::new();
        let mut skip = None;
        for c in lines.collect::<Vec<_>>().join(" ").chars() {
            match (skip, c) {
                (None, '<') => skip = Some('>'),
                (None, '{') => skip = Some('}'),
                (None, c) => clean.push(c),
                (Some(close), c) if c == close => skip = None,
                _ => {}
            }
        }
        let words: Vec<&str> = clean.split_whitespace().collect();
        let letters = |w: &str| w.chars().count().max(1) as f64;
        let all: f64 = words.iter().map(|w| letters(w)).sum();
        let mut at = start;
        for w in words {
            let until = at + (end - start).max(0.0) * letters(w) / all;
            out.push(crate::captions::Word {
                text: w.to_string(),
                start: at,
                end: until,
            });
            at = until;
        }
    }
    out
}

/// The words of a `--keep-words` file: the transcript file, or its `words` list alone.
fn read_words(file: &Path) -> Result<Vec<crate::captions::Word>, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let doc = if v.is_array() { json!({"words": v}) } else { v };
    serde_json::from_value(json!(crate::analyze::stt_words(&doc))).map_err(|e| {
        format!(
            "{}: each word needs text, start and end: {e}",
            file.display()
        )
    })
}

/// The parts to keep from the whole transcript and the words left in the file, and the words that went. A word stays
/// when its middle is inside a word of the file, so a word that a term fix joined still counts. A part runs up to
/// `pad` past its words, never into a removed word, and to the end of the video where nothing was removed.
pub fn word_parts(
    all: &[crate::captions::Word],
    kept: &[crate::captions::Word],
    total: f64,
    pad: f64,
) -> Result<(Vec<Span>, Vec<String>), String> {
    let inside = |t: f64, w: &crate::captions::Word| t >= w.start - 0.001 && t <= w.end + 0.001;
    let mid = |w: &crate::captions::Word| (w.start + w.end) / 2.0;
    if kept.is_empty() {
        return Err("the file has no words".into());
    }
    if let Some(w) = kept.iter().find(|k| !all.iter().any(|a| inside(mid(a), k))) {
        return Err(format!(
            "\"{}\" at {:.2} s is not in this video's transcript",
            w.text.trim(),
            w.start
        ));
    }
    let stays: Vec<bool> = all
        .iter()
        .map(|a| kept.iter().any(|k| inside(mid(a), k)))
        .collect();
    let mut parts = Vec::new();
    let mut gone = Vec::new();
    let mut i = 0;
    while i < all.len() {
        if !stays[i] {
            gone.push(all[i].text.trim().to_string());
            i += 1;
            continue;
        }
        let first = i;
        while i + 1 < all.len() && stays[i + 1] {
            i += 1;
        }
        let start = if first == 0 {
            0.0
        } else {
            (all[first].start - pad).max(all[first - 1].end)
        };
        let end = if i + 1 == all.len() {
            total
        } else {
            (all[i].end + pad).min(all[i + 1].start)
        };
        parts.push((start, end));
        i += 1;
    }
    Ok((parts, gone))
}

/// Parts to keep from "a-b,c-d": in order, not overlapping, each at least 0.3 s.
pub fn parse_keep(spec: &str) -> Result<Vec<(f64, f64)>, String> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = part
            .split_once('-')
            .and_then(|(a, b)| {
                let num = |x: &str| {
                    x.trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite() && *v >= 0.0)
                };
                Some((num(a)?, num(b)?))
            })
            .ok_or_else(|| format!("{part} is not start-end in seconds"))?;
        if b - a < 0.3 {
            return Err(format!("{part} keeps under 0.3 s"));
        }
        if out.last().is_some_and(|l| a < l.1) {
            return Err(format!("{part} starts before the part in front of it ends"));
        }
        out.push((a, b));
    }
    if out.is_empty() {
        return Err("no part to keep".into());
    }
    Ok(out)
}

/// The air before and after a bleeped word, and the fade at each end of the bleep, in seconds. The fades are inside the
/// air, so the word's own time holds nothing of the source. The air is wide because a speech row's word edges are not
/// the sound's: against the exact times of a made voice (32 words, 10 Nov 2026) the local row put a start up to 82 ms
/// late and an end up to 185 ms early, and with 0.04 s the end of the word was heard again ("pineapple" as "people").
/// With 0.1 s before, the word in front was lost ("fresh" as "f***"); with 0.05 s it stays.
// ponytail: fixed air, so a short word next to the bleep can go with it; snap to the sound's own quiet edges (as
// `filler_keep` does) if that costs too much.
const BLEEP_PAD: (f64, f64) = (0.05, 0.2);
const BLEEP_FADE: f64 = 0.01;
/// The bleep works on 48 kHz stereo, 16 bit: 4 bytes a frame.
const BLEEP_RATE: f64 = 48000.0;

/// One place where a `--bleep` entry is said: the entry as the user gave it, and the heard times.
#[derive(serde::Serialize, Debug, PartialEq)]
struct Bleeped {
    word: String,
    start: f64,
    end: f64,
}

/// What `--bleep` found: the entries with their compared words, the heard words, and the stretches to replace.
struct Bleep {
    wanted: Vec<(String, Vec<String>)>,
    heard: Vec<crate::captions::Word>,
    matches: Vec<Bleeped>,
    not_heard: Vec<String>,
    windows: Vec<Span>,
    pad: (f64, f64),
}

/// `--bleep "a,b c"`: each entry with its words as `gates::norm_word` compares them (case, punctuation and Arabic
/// letter forms do not count). An entry with no letter or digit is refused: it would hide nothing and say nothing.
fn parse_bleep(spec: &str) -> Result<Vec<(String, Vec<String>)>, String> {
    let mut out = Vec::new();
    for entry in spec.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let words: Vec<String> = entry
            .split_whitespace()
            .map(crate::gates::norm_word)
            .filter(|w| !w.is_empty())
            .collect();
        if words.is_empty() {
            return Err(format!("\"{entry}\" has no letter or digit"));
        }
        out.push((entry.to_string(), words));
    }
    if out.is_empty() {
        return Err("no word to hide".into());
    }
    Ok(out)
}

/// Where each entry is said, as (first word, last word, entry) indexes, and the entries never said. An entry of
/// several words is a run of heard words in a row; a heard item with no letter (a lone dash) is stepped over.
fn bleep_hits(
    words: &[crate::captions::Word],
    wanted: &[(String, Vec<String>)],
) -> (Vec<(usize, usize, usize)>, Vec<String>) {
    let said: Vec<(usize, String)> = words
        .iter()
        .enumerate()
        .map(|(i, w)| (i, crate::gates::norm_word(&w.text)))
        .filter(|(_, n)| !n.is_empty())
        .collect();
    let mut runs = Vec::new();
    let mut missing = Vec::new();
    for (e, (entry, want)) in wanted.iter().enumerate() {
        let before = runs.len();
        for i in 0..said.len() {
            if said[i..].iter().take(want.len()).map(|s| &s.1).eq(want) {
                runs.push((said[i].0, said[i + want.len() - 1].0, e));
            }
        }
        if runs.len() == before {
            missing.push(entry.clone());
        }
    }
    runs.sort_unstable();
    (runs, missing)
}

/// The stretches to replace: each match with its air before and after, inside the video, and joined where they meet.
fn bleep_windows(matches: &[Bleeped], total: f64, (lead, tail): (f64, f64)) -> Vec<Span> {
    let mut all: Vec<Span> = matches
        .iter()
        .map(|m| {
            let ms = |t: f64| (t * 1000.0).round() / 1000.0;
            (ms((m.start - lead).max(0.0)), ms((m.end + tail).min(total)))
        })
        .filter(|(s, e)| e > s)
        .collect();
    all.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<Span> = Vec::new();
    for (s, e) in all {
        match out.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

/// One stretch of 16 bit stereo frames, replaced: the source fades to nothing in [`BLEEP_FADE`], stays at exactly zero,
/// and fades back in at the end. `tone` is the peak of a 1 kHz sine that takes its place (0: silence); it is never
/// mixed with the source outside the two fades.
fn bleep_window(frames: &mut [[u8; 4]], tone: f64) {
    let n = frames.len();
    let fade = ((BLEEP_FADE * BLEEP_RATE) as usize).min(n / 2).max(1);
    for (i, f) in frames.iter_mut().enumerate() {
        // The share of the source that stays: 1 at both edges, 0 in between.
        let keep = if i < fade {
            1.0 - (i + 1) as f64 / fade as f64
        } else if i >= n - fade {
            (i + fade + 1 - n) as f64 / fade as f64
        } else {
            0.0
        };
        let beep = tone * (std::f64::consts::TAU * 1000.0 * i as f64 / BLEEP_RATE).sin();
        for ch in 0..2 {
            let src = f64::from(i16::from_le_bytes([f[ch * 2], f[ch * 2 + 1]]));
            let v = src * keep + beep * (1.0 - keep) * 32767.0;
            let b = (v.round().clamp(-32768.0, 32767.0) as i16).to_le_bytes();
            (f[ch * 2], f[ch * 2 + 1]) = (b[0], b[1]);
        }
    }
}

/// The video `from` as `to`, with the sound of each window replaced by a tone or by silence. The picture is copied.
/// The sound is decoded once to raw samples and the windows are changed in that file by this code: no word and no
/// time reaches an FFmpeg filter. Memory is one window at a time.
fn bleep_sound(
    ff: &Ffmpeg,
    from: &Path,
    to: &Path,
    work: &Path,
    windows: &[Span],
    tone: bool,
) -> Result<(), String> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let abs = |p: &Path| {
        std::path::absolute(p)
            .unwrap_or(p.to_path_buf())
            .display()
            .to_string()
    };
    let pcm = work.join("bleep.pcm");
    let raw = ["-f", "s16le", "-ar", "48000", "-ac", "2"].map(String::from);
    let mut args = vec!["-i".into(), abs(from), "-vn".into()];
    args.extend(raw.clone());
    args.push(abs(&pcm));
    ff.run(Path::new("."), &args, None)?;
    let io = |e: std::io::Error| format!("{}: {e}", pcm.display());
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&pcm)
        .map_err(io)?;
    let frames = f.metadata().map_err(io)?.len() / 4;
    // The tone's peak is the level (RMS) of the whole sound, so the tone itself is 3 dB under it: a quiet recording
    // gets a quiet tone, and the master step that follows raises both alike.
    let level = if tone {
        let (mut sum, mut buf) = (0.0, vec![0u8; 1 << 16]);
        loop {
            let got = f.read(&mut buf).map_err(io)?;
            if got == 0 {
                break;
            }
            for s in buf[..got].as_chunks::<2>().0 {
                sum += (f64::from(i16::from_le_bytes(*s)) / 32768.0).powi(2);
            }
        }
        (sum / (frames * 2).max(1) as f64).sqrt().clamp(0.003, 0.25)
    } else {
        0.0
    };
    for (s, e) in windows {
        let at = |t: f64| ((t * BLEEP_RATE).round() as u64).min(frames);
        let (s, e) = (at(*s), at(*e));
        if e <= s {
            continue;
        }
        let mut buf = vec![0u8; ((e - s) * 4) as usize];
        f.seek(SeekFrom::Start(s * 4)).map_err(io)?;
        f.read_exact(&mut buf).map_err(io)?;
        bleep_window(buf.as_chunks_mut::<4>().0, level);
        f.seek(SeekFrom::Start(s * 4)).map_err(io)?;
        f.write_all(&buf).map_err(io)?;
    }
    f.sync_all().map_err(io)?;
    drop(f);
    let mut args = vec!["-i".into(), abs(from)];
    args.extend(raw);
    // The sound ends with the picture, as in `master`: a new AAC encode adds its own start delay. Not -shortest.
    if let Ok(end) = ff.video_duration(from) {
        args.extend(["-t".into(), format!("{end:.5}")]);
    }
    args.extend(["-i".into(), abs(&pcm)]);
    args.extend(
        [
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-movflags",
            "+faststart",
        ]
        .map(String::from),
    );
    args.push(abs(to));
    let done = ff.run(Path::new("."), &args, None);
    let _ = std::fs::remove_file(&pcm);
    done
}

/// A caption word with each letter and digit as `*`. Punctuation at its ends stays, so a sentence still ends where it
/// did; Arabic marks go, so none is left on a star.
fn mask(text: &str) -> String {
    // Unicode counts the Arabic marks as alphabetic.
    let mark = |c: char| matches!(c, '\u{064B}'..='\u{065F}' | '\u{0670}');
    let letter = |c: char| c.is_alphanumeric() && !mark(c);
    let (Some(a), Some(b)) = (text.find(letter), text.rfind(letter)) else {
        return text.to_string();
    };
    let b = b + text[b..].chars().next().map_or(1, char::len_utf8);
    let stars = "*".repeat(text[a..b].chars().filter(|c| letter(*c)).count());
    format!("{}{stars}{}", &text[..a], &text[b..])
        .chars()
        .filter(|c| !mark(*c))
        .collect()
}

/// Masks the caption words that `--bleep` hides, and says how many: each word of an entry, by its text. The captions
/// may spell other words than the speech row heard (--text, --captions-file), so a matched stretch with no such word
/// on it masks the word whose middle is in the matched word's own time. Not the air around it: the word in front must
/// stay readable.
fn mask_words(words: &mut [crate::captions::Word], b: &Bleep) -> usize {
    let (runs, _) = bleep_hits(words, &b.wanted);
    let by_text: Vec<bool> = (0..words.len())
        .map(|i| runs.iter().any(|r| (r.0..=r.1).contains(&i)))
        .collect();
    let bare: Vec<&Bleeped> = b
        .matches
        .iter()
        .filter(|m| {
            !words
                .iter()
                .zip(&by_text)
                .any(|(w, hit)| *hit && w.start < m.end && w.end > m.start)
        })
        .collect();
    let mut n = 0;
    for (w, hit) in words.iter_mut().zip(by_text) {
        let mid = (w.start + w.end) / 2.0;
        if hit || bare.iter().any(|m| mid >= m.start && mid <= m.end) {
            w.text = mask(&w.text);
            n += 1;
        }
    }
    n
}

pub fn run(mut a: Args) -> Envelope {
    if let Some(d) = a.duration {
        if a.speed.is_some() || !(d.is_finite() && d > 0.0) {
            return fail(
                Exit::Usage,
                "--duration is a length in seconds above 0, and not with --speed",
                "Give one of --duration or --speed.",
            );
        }
        // ponytail: a stand-in speed, so every rule for --speed holds; the real one comes from the kept length below.
        a.speed = Some(1.0);
    }
    if a.captions_file.is_some() && a.captions.is_none() {
        a.captions = Some("karaoke".into());
    }
    if a.caption_style.is_set() && a.captions.is_none() {
        return fail(
            Exit::Usage,
            "--caption-size, --caption-outline and --caption-box need --captions",
            "Give --captions karaoke, pop or reveal, or drop the caption style flags.",
        );
    }
    if let Some((e, fix)) = a.caption_style.check(a.captions.as_deref()) {
        return fail(Exit::Usage, e, fix);
    }
    if let Some(spec) = &a.speed_ranges {
        if a.duration.is_some() {
            return fail(
                Exit::Usage,
                "--duration sets one speed for the whole video, so it does not go with speed ranges",
                "Give one of --duration or --speed.",
            );
        }
        // The form now, before any work; the fit in the length when the cuts are known.
        if let Err(e) = crate::edit_speed::parse(spec, f64::INFINITY) {
            return fail(Exit::Usage, e, crate::edit_speed::FIX);
        }
        // ponytail: a stand-in speed, as for --duration, so every rule for --speed holds.
        a.speed = Some(1.0);
    }
    // Before the HDR step, which swaps the input for a copy.
    let reads = std::iter::once(a.input.clone())
        .chain(
            [
                &a.captions_file,
                &a.text,
                &a.captions_second,
                &a.keep_words,
                &a.words,
                &a.lipsync,
                &a.logo,
            ]
            .into_iter()
            .flatten()
            .cloned(),
        )
        .chain(music_file(&a.music))
        .chain(a.background.iter().map(PathBuf::from))
        .chain(a.sfx.iter().filter_map(|s| crate::edit_sfx::file_of(s)))
        .chain(
            a.broll
                .iter()
                .filter_map(|b| Some(PathBuf::from(b.rsplit_once('@')?.0))),
        );
    let own = a.out.as_deref().and_then(|o| {
        reads
            .into_iter()
            .find(|i| crate::engine_ffmpeg::same_file(o, i))
    });
    if let Some(i) = own {
        return fail(
            Exit::Usage,
            format!("--out is the input file: {}", i.display()),
            "Give another --out: an edit cannot write over a file it reads.",
        );
    }
    // Each picture, clip and sound file is plain media by its first bytes (`media-inputs.md`), before FFmpeg, ffprobe
    // or a row is given one. `--background` has its own, narrower check in `edit_matte`.
    let media = std::iter::once(a.input.clone())
        .chain(a.lipsync.clone())
        .chain(a.logo.clone())
        .chain(music_file(&a.music))
        .chain(a.sfx.iter().filter_map(|s| crate::edit_sfx::file_of(s)))
        .chain(
            a.broll
                .iter()
                .filter_map(|b| Some(PathBuf::from(b.rsplit_once('@')?.0))),
        );
    if let Some(env) = crate::media_gate::refuse("edit", media) {
        return env;
    }
    let hdr = match crate::engine_ffmpeg::sdr_input(&mut a.input, a.use_system) {
        Ok(h) => h,
        Err(e) => {
            return fail(
                Exit::Error,
                e,
                "Check the file, and that the disk has room.",
            )
        }
    };
    // The small operations work alone (`edit_small.rs`): they answer here, or pass on when none is asked.
    if let crate::edit_matte::Step::Done(mut env) = crate::edit_small::step(&a) {
        env.findings.extend(hdr);
        return *env;
    }
    // The matte or upscale row first, on the file itself: every other step then works on the new picture.
    let cost = match crate::edit_matte::step(&mut a) {
        crate::edit_matte::Step::Next(c) => c,
        crate::edit_matte::Step::Done(mut env) => {
            env.findings.extend(hdr);
            return *env;
        }
    };
    let mut env = run_sdr(a);
    if let Some(c) = cost {
        env.metrics.insert("cost_usd".into(), json!(c));
    }
    env.findings.extend(hdr);
    env
}

fn run_sdr(mut a: Args) -> Envelope {
    if a.cut_still {
        if a.cut_silence {
            return fail(
                Exit::Usage,
                "--cut-still and --cut-silence are two ways to choose the cuts",
                "Use one of them.",
            );
        }
        // ponytail: a still cut counts as a silence cut for every rule that mixes cuts; the cut itself differs below.
        a.cut_silence = true;
    }
    let wanted = match a.bleep.as_deref().map(parse_bleep) {
        None if a.bleep_with.is_some() || a.bleep_pad.is_some() || a.words.is_some() => {
            return fail(
                Exit::Usage,
                "--bleep-with, --bleep-pad and --words need --bleep",
                "Give the words to hide: --bleep \"word,two words\".",
            )
        }
        None => None,
        Some(Err(e)) => {
            return fail(
                Exit::Usage,
                format!("--bleep: {e}"),
                "Give the words to hide, with commas between them: --bleep \"word,two words\".",
            )
        }
        Some(Ok(w)) => Some(w),
    };
    if wanted.is_some() {
        if !matches!(a.bleep_with.as_deref(), None | Some("tone" | "silence")) {
            return fail(
                Exit::Usage,
                "--bleep-with takes tone or silence",
                "Give --bleep-with tone or --bleep-with silence.",
            );
        }
        if a.bleep_pad.is_some_and(|p| !(0.0..=1.0).contains(&p)) {
            return fail(
                Exit::Usage,
                "--bleep-pad takes 0 to 1 second",
                "Give for example --bleep-pad 0.3, or drop it for 0.05 s before and 0.2 s after each word.",
            );
        }
        // The heard times are the source's. A cut or a speed change moves them, and a new voice says other words.
        if a.cut_silence
            || a.cut_fillers
            || a.cut_retakes
            || a.keep.is_some()
            || a.keep_words.is_some()
            || a.speed.is_some()
            || a.from.is_some()
            || a.to.is_some()
            || a.lipsync.is_some()
        {
            return fail(
                Exit::Usage,
                "--bleep works on the source's own times, so it does not mix with a cut, --keep, --keep-words, --speed, --duration, --from/--to or --lipsync: use two calls",
                "Run `scriptvox edit <video> --bleep ...` first, then the other edit on its result.",
            );
        }
    }
    // Lip sync first, on the file itself: every other step then works on its result.
    let only_lipsync = a.lipsync.is_some()
        && !a.cut_silence
        && a.reframe.is_none()
        && a.from.is_none()
        && a.to.is_none()
        && a.captions.is_none()
        && a.logo.is_none()
        && a.keep.is_none()
        && a.keep_words.is_none()
        && !a.cut_retakes
        && a.clean_voice.is_none()
        && a.zoom.is_empty()
        && a.broll.is_empty()
        && a.blur.is_empty()
        && a.speed.is_none()
        && !a.cut_fillers
        && a.music.is_none()
        && a.sfx.is_empty()
        && a.hook.is_none()
        && a.label.is_none();
    if let Some(voice) = a.lipsync.take() {
        let Some(row) = crate::generators::all()
            .into_iter()
            // Free rows only: `edit` has no plan, and a paid row needs one. A paid row runs through `generate`.
            .find(|g| g.kind == "lipsync" && !g.paid() && crate::keys::resolve(&g.key).is_some())
        else {
            return fail(
                Exit::NeedsInput,
                "no lip sync row with a key on this install",
                "Drop --lipsync. `scriptvox providers` lists what this install has.",
            );
        };
        let stem = a
            .input
            .file_stem()
            .map_or("video".into(), |s| s.to_string_lossy().into_owned());
        let synced =
            a.out.clone().filter(|_| only_lipsync).unwrap_or_else(|| {
                PathBuf::from("scriptvox-out").join(format!("{stem}-lipsync.mp4"))
            });
        let got = crate::generate::run(crate::generate::Args {
            kind: None,
            last_image: None,
            model: row.name,
            prompt: String::new(),
            audio: Some(voice),
            video: Some(a.input.clone()),
            voice: None,
            lang: None,
            image: None,
            seconds: None,
            aspect: "9:16".into(),
            out: Some(synced.clone()),
            wait_s: 1800,
            approve: None,
            max_spend: None,
            plan_only: false,
            idempotency_key: None,
            seed: None,
        });
        if only_lipsync || !got.ok {
            let mut got = got;
            got.command = "edit".into();
            return got;
        }
        a.input = synced;
    }
    let clip = a.from.is_some() || a.to.is_some();
    if !a.cut_silence
        && a.reframe.is_none()
        && !clip
        && a.captions.is_none()
        && a.logo.is_none()
        && a.keep.is_none()
        && a.keep_words.is_none()
        && a.bleep.is_none()
        && !a.cut_retakes
        && a.clean_voice.is_none()
        && a.zoom.is_empty()
        && a.broll.is_empty()
        && a.blur.is_empty()
        && a.speed.is_none()
        && !a.cut_fillers
        && a.music.is_none()
        && a.sfx.is_empty()
        && a.hook.is_none()
        && a.label.is_none()
    {
        return fail(
            Exit::Usage,
            "nothing to do",
            "Give --cut-silence, --cut-fillers, --reframe 9:16, --from/--to, --keep, --keep-words, --cut-retakes, --bleep, --clean-voice, --zoom, --broll, --blur, --background, --alpha, --enhance, --captions, --hook, --music, --sfx, or a mix. Alone: --remove-bars, --hold, --loop, --loop-to, --deinterlace, --tilt.",
        );
    }
    let music_level = match crate::audio::music_gap_db(a.music_level.as_deref()) {
        Ok(l) => l,
        Err(e) => return fail(Exit::Usage, e, "Use --music-level quiet, normal or loud."),
    };
    // Music from an address: the checks of `fetch` that cost no request, before any other work (as `make`).
    let music_url = a.music.clone().filter(|m| crate::fetch::is_address(m));
    match &music_url {
        Some(url) => {
            if let Err(f) =
                crate::fetch::music_check(url, a.music_cut.0.as_deref(), a.music_cut.1.as_deref())
            {
                return Envelope::fail_code("edit", f.exit, f.code, f.message, f.fix);
            }
            if crate::plan::local_only() {
                let action = "Music from an address needs the network. Run `scriptvox fetch <address> --audio` first and give that file as --music, or run without --local-only.";
                let mut env = Envelope::fail("edit", Exit::NeedsInput, action, None);
                env.needs_input = vec![crate::voices::needs_input(
                    "music",
                    "dependency",
                    action,
                    vec![
                        json!({"args": ["--music", "calm"], "cost_usd": 0, "note": "built-in bed, offline"}),
                    ],
                )];
                return env;
            }
        }
        None if a.music_cut.0.is_some() || a.music_cut.1.is_some() => {
            return fail(
                Exit::Usage,
                "--music-from and --music-to cut music from an address only",
                "Give --music an https address, or drop --music-from and --music-to.",
            );
        }
        None => {}
    }
    let local_music = a.music.as_deref().filter(|_| music_url.is_none());
    let mut music = match local_music.map(crate::audio::pick_music) {
        None => None,
        Some(Ok(m)) => Some(m),
        Some(Err(crate::audio::MusicRefused::Missing)) => {
            return fail(
                Exit::Usage,
                format!("no music {}", a.music.as_deref().unwrap_or("")),
                "Use --music calm, --music pulse, a music file inside this project, or an https address.",
            )
        }
        Some(Err(crate::audio::MusicRefused::Outside)) => {
            return fail(
                Exit::Usage,
                "the music file is outside this project folder",
                "Copy the music file into the project.",
            )
        }
    };
    let sfx = match crate::edit_sfx::parse(&a.sfx) {
        Ok(c) => c,
        Err((e, fix)) => return fail(Exit::Usage, e, &fix),
    };
    if let Err(e) = crate::edit_sfx::under_db(a.sfx_level.as_deref()) {
        return fail(Exit::Usage, e, "Use --sfx-level quiet, normal or loud.");
    }
    if a.speed.is_some_and(|s| !(0.5..=2.0).contains(&s)) {
        return fail(
            Exit::Usage,
            "--speed takes 0.5 to 2",
            "Give for example --speed 1.25.",
        );
    }
    if a.cut_fillers && (a.keep.is_some() || a.cut_silence) {
        return fail(
            Exit::Usage,
            "--cut-fillers does not mix with --keep or --cut-silence",
            "Use one way to choose the parts; --cut-fillers already cuts the pause around each filler.",
        );
    }
    let mut keep_parts = match a.keep.as_deref().map(parse_keep) {
        Some(Err(e)) => {
            return fail(
                Exit::Usage,
                e,
                "Give --keep like 12.3-30.5,41-58.2 (seconds, in time order: --keep cannot move a later part to the front).",
            )
        }
        Some(Ok(_)) if clip || a.cut_silence => {
            return fail(
                Exit::Usage,
                "--keep does not mix with --from/--to or --cut-silence",
                "Use one way to choose the parts.",
            )
        }
        Some(Ok(k)) => Some(k),
        None => None,
    };
    if let Some(c) = a
        .captions
        .as_deref()
        .filter(|c| !matches!(*c, "boxed" | "karaoke" | "pop" | "reveal"))
    {
        return fail(
            Exit::Usage,
            format!("unknown caption style {c}"),
            "Use --captions karaoke, pop or reveal.",
        );
    }
    if let Some(c) = &a.accent {
        if crate::captions::contrast_on_black(c).is_none_or(|k| k < 3.0)
            || crate::captions::ass_colour(c).is_none()
        {
            return fail(
                Exit::Usage,
                format!("accent {c} is not a colour that reads on the caption box"),
                "Give #RRGGBB with contrast 3 or more on black, for example #1F6FEB.",
            );
        }
    }
    for (what, h) in [("hook", &a.hook), ("label", &a.label)] {
        if let Some(bad) = h
            .as_deref()
            .and_then(|h| crate::textlint::check(h).into_iter().next())
        {
            return fail(
                Exit::Usage,
                format!("the {what} cannot be drawn: {}", bad.detail),
                "Change the text.",
            );
        }
    }
    if a.logo.as_ref().is_some_and(|l| !l.is_file()) {
        return fail(
            Exit::Usage,
            "the logo file does not exist",
            "Give a PNG file.",
        );
    }
    // With --words the bleep and the captions both read that file.
    let hears = a.cut_fillers
        || (a.words.is_none()
            && (a.bleep.is_some() || (a.captions.is_some() && a.captions_file.is_none())));
    if hears && crate::opentts::stt_row(false).is_none() {
        return fail(
            Exit::NeedsInput,
            crate::analyze::NO_STT,
            "Drop --captions, --cut-fillers and --bleep, or add a speech to text row. With --bleep, --words <file> gives the heard words without one. `scriptvox providers` lists what this install has.",
        );
    }
    let blur = a.fit.as_deref() == Some("blur");
    if let Some(fit) = a.fit.as_deref().filter(|f| !matches!(*f, "blur" | "crop")) {
        return fail(
            Exit::Usage,
            format!("unknown --fit {fit}"),
            "Use --fit blur or crop.",
        );
    }
    if a.fit.is_some() && a.reframe.is_none() {
        return fail(
            Exit::Usage,
            "--fit needs --reframe",
            "Give --reframe 9:16 --fit blur.",
        );
    }
    if blur && a.track {
        return fail(
            Exit::Usage,
            "--fit blur cuts nothing, so there is nothing for --track to follow",
            "Drop --track or --fit.",
        );
    }
    if a.track && a.reframe.is_none() {
        return fail(
            Exit::Usage,
            "--track needs --reframe",
            "Add --reframe 9:16.",
        );
    }
    if !a.input.is_file() {
        return fail(
            Exit::Usage,
            format!("{} does not exist", a.input.display()),
            "Give a video file.",
        );
    }
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("edit", e),
    };
    // A ScriptVox MP4 carries burned captions and cards laid out for its own frame: a crop cuts them at the edges.
    if let (Some(shape), false) = (&a.reframe, a.crop_text || blur) {
        if let Some(made) = crop_cuts_text(&ff, &a.input, shape) {
            return fail(
                Exit::Usage,
                format!("this ScriptVox video was made in {made}: a {shape} crop cuts its burned captions and cards"),
                &format!("If its run is in this project: put \"aspect\": \"{shape}\" in the brief and run `scriptvox resume <run_id> --brief <file>`. Otherwise use the original footage in a new brief, or add --crop-text to crop anyway and lose the text at the sides."),
            );
        }
    }
    let total = match ff.duration(&a.input) {
        Ok(d) => d,
        Err(e) => return fail(Exit::Usage, e, "Give a video FFmpeg can read."),
    };
    let stem = a
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video")
        .to_string();
    let work = PathBuf::from(".scriptvox/edit").join(&stem);
    let _ = std::fs::create_dir_all(&work);
    // The one download, before the encode, so a refused address leaves no half-made video. `fetch::music` keeps the
    // file in the store: a second edit with the same address and cut sends no request.
    let mut music_info = None;
    if let Some(url) = &music_url {
        match crate::fetch::music(
            url,
            a.music_cut.0.as_deref(),
            a.music_cut.1.as_deref(),
            &work,
            false,
            &ff,
        ) {
            Ok((path, pick)) => {
                // The mix runs inside the work folder.
                music = Some(crate::audio::Music::File(
                    std::path::absolute(&path).unwrap_or(path),
                ));
                music_info = Some(pick);
            }
            Err(f) => return Envelope::fail_code("edit", f.exit, f.code, f.message, f.fix),
        }
    }
    // A clip is read with -ss/-to on the input: frame-exact on a re-encode, and every filter sees clip time from 0.
    // Only --track needs the clip as its own file (it samples frames from a path): one extra encode there.
    let mut seek: Vec<String> = Vec::new();
    let total = if clip {
        // On the frame grid: picture ends on a frame and the sound on the asked time, which differ by up to a frame
        // (25 fps talk: video 58.84 s, audio 58.90 s, gate av_drift failed).
        let rate = ff.frame_rate(&a.input).unwrap_or(30.0).max(1.0);
        let grid = |t: f64| (t * rate).round() / rate;
        let (s, e) = (
            grid(a.from.unwrap_or(0.0).max(0.0)),
            grid(a.to.unwrap_or(total).min(total)),
        );
        if e - s < 0.5 {
            return fail(
                Exit::Usage,
                format!("--from {s} --to {e} keeps under 0.5 s"),
                "Give a longer part.",
            );
        }
        if !a.track {
            seek = vec![
                "-ss".into(),
                format!("{s:.3}"),
                "-to".into(),
                format!("{e:.3}"),
            ];
            e - s
        } else {
            let part = work.join(format!("clip-{s:.2}-{e:.2}.mp4"));
            let args: Vec<String> = vec![
                "-ss".into(),
                format!("{s:.3}"),
                "-to".into(),
                format!("{e:.3}"),
                "-i".into(),
                a.input.display().to_string(),
                "-c:v".into(),
                "libx264".into(),
                "-crf".into(),
                "16".into(),
                "-preset".into(),
                "fast".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
                part.display().to_string(),
            ];
            if let Err(e) = ff.run(Path::new("."), &args, None) {
                return fail(Exit::Error, e, "Check the input video plays.");
            }
            a.input = part;
            e - s
        }
    } else {
        total
    };
    let rb = ff.readback(&a.input).ok();
    let fps = rb.as_ref().map_or(30.0, |r| r.fps).max(1.0);
    let mut vf: Vec<String> = Vec::new();
    // A variable-rate clip (phones, screen recordings): its nominal rate says nothing about its frames, and the cut's
    // `setpts=N/FRAME_RATE` would squeeze the picture, then `-shortest` the sound (8 s became 2.3 s, 5 Nov 2026). Make
    // it constant first, at its real rate.
    let fps = match ff.frame_rate(&a.input) {
        Ok(avg) if crate::engine_ffmpeg::variable_rate(fps, avg) => {
            let real = avg.round().clamp(15.0, 60.0);
            vf.push(format!("fps={real}"));
            real
        }
        _ => fps,
    };
    let mut af: Vec<String> = Vec::new();
    let mut hum = None;
    // First, so the noise is learned over the whole recording and not across a cut.
    if let Some(level) = &a.clean_voice {
        let Some(f) = clean_filter(level) else {
            return fail(
                Exit::Usage,
                format!("unknown --clean-voice {level}"),
                "Use light, medium or strong.",
            );
        };
        if !ff.has_stream(&a.input, "audio") {
            return fail(Exit::Usage, "the video has no sound", "Drop --clean-voice.");
        }
        // Mains hum and rumble first: what is found gets its own narrow filter (`edit-hum-rumble.md`).
        hum = crate::hum::read(&ff, &a.input).map(|pcm| crate::hum::find(&pcm));
        af.extend(hum.iter().flat_map(crate::hum::Report::filters));
        af.push(f.to_string());
    }
    let mut kept = total;
    let mut cuts = 0;
    let silence_log = |min: f64| {
        Command::new(&ff.ffmpeg)
            .args(["-hide_banner", "-nostdin"])
            .args(&seek)
            // The input was checked at the start of `edit`.
            .args(crate::media_gate::input(&a.input).unwrap_or_default())
            .args([
                "-vn",
                "-af",
                &format!("silencedetect=n={}dB:d={min}", a.noise_db),
                "-f",
                "null",
                "-",
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stderr).into_owned())
    };
    let mut filler_info = None;
    let mut still_info = None;
    if a.cut_fillers {
        let mut sound = seek.clone();
        sound.extend(["-i".into(), a.input.display().to_string()]);
        let lang = a.lang.as_deref();
        let prompt = if lang == Some("ar") {
            FILLER_PROMPT_AR
        } else {
            FILLER_PROMPT_EN
        };
        let words = match hear(&ff, sound, &work, lang, Some(prompt), "fillers") {
            Ok(w) => w,
            Err((exit, e, fix)) => return fail(exit, e, fix),
        };
        // Silences of 0.1 s or more: the sound islands a filler cut snaps to.
        let quiet = silence_log(0.1).map_or(vec![], |l| parse_silences(&l, total));
        let (parts, cut, skipped) = filler_keep(&words, total, a.pad, &quiet);
        filler_info =
            Some(json!({"heard": words.len(), "cut": cut.len(), "skipped": skipped, "words": cut}));
        keep_parts = Some(parts);
    }
    let mut word_info = None;
    if let Some(file) = &a.keep_words {
        if a.keep.is_some() || a.cut_fillers || a.cut_silence || clip {
            return fail(
                Exit::Usage,
                "--keep-words does not mix with --keep, --cut-fillers, --cut-silence or --from/--to",
                "Use one way to choose the parts.",
            );
        }
        let kept = match read_words(file) {
            Ok(k) => k,
            Err(e) => {
                return fail(
                    Exit::Usage,
                    e,
                    "Copy the `transcript.file` of `analyze --transcript` and delete words from its `words` list.",
                )
            }
        };
        // The same transcript as `analyze`, from its cache: what is gone is what the file no longer holds.
        let all = match crate::analyze::transcribe_terms(
            &ff,
            &a.input,
            a.lang.as_deref(),
            total,
            a.terms.as_ref(),
        ) {
            Ok((doc, _)) => crate::analyze::timed_words(&doc).unwrap_or_default(),
            Err(mut env) => {
                env.command = "edit".into();
                return *env;
            }
        };
        match word_parts(&all, &kept, total, a.pad) {
            Ok((parts, gone)) => {
                word_info = Some(json!({"heard": all.len(), "kept": all.len() - gone.len(),
                    "removed": gone, "parts": parts}));
                keep_parts = Some(parts);
            }
            Err(e) => {
                return fail(
                    Exit::Usage,
                    e,
                    "Use the transcript of this video, and leave at least one word.",
                )
            }
        }
    }
    // Before any encode: a run that hides nothing must write nothing.
    let bleep = match wanted {
        None => None,
        Some(wanted) => {
            if !ff.has_stream(&a.input, "audio") {
                return fail(Exit::Usage, "the video has no sound", "Drop --bleep.");
            }
            let heard = match &a.words {
                Some(file) => match read_words(file) {
                    Ok(w) => w,
                    Err(e) => {
                        return fail(
                            Exit::Usage,
                            e,
                            "Give the `transcript.file` of `analyze --transcript`, or JSON with `words`: text, start and end.",
                        )
                    }
                },
                // The same transcript as `analyze`, from its cache.
                None => match crate::analyze::transcribe_terms(
                    &ff,
                    &a.input,
                    a.lang.as_deref(),
                    total,
                    a.terms.as_ref(),
                ) {
                    Ok((doc, _)) => crate::analyze::timed_words(&doc).unwrap_or_default(),
                    Err(mut env) => {
                        env.command = "edit".into();
                        return *env;
                    }
                },
            };
            let (runs, not_heard) = bleep_hits(&heard, &wanted);
            if runs.is_empty() {
                return fail(
                    Exit::GateFailed,
                    format!(
                        "none of the --bleep words was heard in {} words: {}",
                        heard.len(),
                        not_heard.join(", ")
                    ),
                    "Nothing was written. `scriptvox analyze <video> --transcript` shows how the speech row spells each word: give that spelling, or the right times in a --words file.",
                );
            }
            let ms = |t: f64| (t * 1000.0).round() / 1000.0;
            let matches: Vec<Bleeped> = runs
                .iter()
                .map(|&(first, last, entry)| Bleeped {
                    word: wanted[entry].0.clone(),
                    start: ms(heard[first].start),
                    end: ms(heard[last].end),
                })
                .collect();
            let pad = a.bleep_pad.map_or(BLEEP_PAD, |p| (p, p));
            let windows = bleep_windows(&matches, total, pad);
            Some(Bleep {
                wanted,
                heard,
                matches,
                not_heard,
                windows,
                pad,
            })
        }
    };
    let mut zooms = match parse_zooms(&a.zoom, total) {
        Ok(z) => z,
        Err(e) => {
            return fail(
                Exit::Usage,
                e,
                "Give --zoom like 12-15@1.2 or 12-15@1.2,0.3,0.4: seconds, zoom 1.05 to 2, centre 0 to 1.",
            )
        }
    };
    if !zooms.is_empty()
        && (a.keep.is_some()
            || a.keep_words.is_some()
            || a.cut_retakes
            || a.cut_fillers
            || a.cut_silence
            || a.speed.is_some())
    {
        return fail(
            Exit::Usage,
            "--zoom times are seconds of the result, so it does not mix with a cut or --speed",
            "Cut first, then run edit --zoom on the result. --zoom-on-emphasis follows a cut by itself.",
        );
    }
    let src_size = ff.shown_size(&a.input).unwrap_or((0, 0));
    let blurs = match parse_blur(&a.blur, src_size, total) {
        Ok(b) => b,
        Err(e) => return fail(Exit::Usage, e, "Give --blur like 100,80,320,120@12-15: x, y, width, height in pixels of the source, then the seconds."),
    };
    if !blurs.is_empty()
        && (a.keep.is_some()
            || a.keep_words.is_some()
            || a.cut_retakes
            || a.cut_fillers
            || a.cut_silence
            || clip)
    {
        return fail(
            Exit::Usage,
            "--blur times are seconds of the source, so it does not mix with a cut or --from/--to",
            "Blur first, then cut the result.",
        );
    }
    let brolls = match parse_broll(&a.broll, total) {
        Ok(b) => b,
        Err(e) => {
            return fail(
                Exit::Usage,
                e,
                "Give --broll like road.mp4@12.0:4 or map.png@12.0:4: the file, the second it starts, how long.",
            )
        }
    };
    if !brolls.is_empty()
        && (a.keep.is_some()
            || a.keep_words.is_some()
            || a.cut_retakes
            || a.cut_fillers
            || a.cut_silence
            || a.speed.is_some())
    {
        return fail(
            Exit::Usage,
            "--broll times are seconds of the result, so it does not mix with a cut or --speed",
            "Cut first, then run edit --broll on the result.",
        );
    }
    if let Some(b) = brolls.iter().find(|b| !b.file.is_file()) {
        return fail(
            Exit::Usage,
            format!("{} does not exist", b.file.display()),
            "Give a clip or a picture in this project.",
        );
    }
    if a.zoom_on_emphasis && (a.emphasis.is_none() || a.captions.is_none()) {
        return fail(
            Exit::Usage,
            "--zoom-on-emphasis needs --emphasis and --captions",
            "Give the words with --emphasis, and --captions.",
        );
    }
    if a.captions_file.is_some()
        && (a.keep.is_some()
            || a.keep_words.is_some()
            || a.cut_retakes
            || a.cut_fillers
            || a.cut_silence
            || a.speed.is_some()
            || a.text.is_some()
            || a.terms.is_some())
    {
        return fail(
            Exit::Usage,
            "the times in --captions-file are the source's, so it does not mix with a cut, --speed, --text or --terms",
            "Cut first, or give a file with the times of the result.",
        );
    }
    // The second line's entries, read and checked before any speech step or encode.
    let second = match &a.captions_second {
        None => Vec::new(),
        Some(_) if a.captions.is_none() => {
            return fail(
                Exit::Usage,
                "--captions-second needs --captions",
                "Give --captions, or drop --captions-second.",
            )
        }
        Some(f) => match crate::make::read_second(f) {
            Ok(l) => l,
            Err(e) => {
                return fail(
                    Exit::Usage,
                    e,
                    "Give a text file with one line per sentence of the speech.",
                )
            }
        },
    };
    // With --text the sentences are known now; else after the speech step, still before the encode.
    if let Some(Ok(script)) = a
        .text
        .as_ref()
        .filter(|_| !second.is_empty())
        .map(std::fs::read_to_string)
    {
        let words: Vec<&str> = script.split_whitespace().collect();
        let n = crate::captions::sentences(&words).len();
        if n != second.len() {
            return fail(
                Exit::Usage,
                format!(
                    "--captions-second has {} lines, --text has {n} sentences",
                    second.len()
                ),
                "Give one line per sentence of the speech, in order.",
            );
        }
    }
    if a.text.is_some() && a.captions.is_none() {
        return fail(
            Exit::Usage,
            "--text needs --captions",
            "Give --captions, or drop --text.",
        );
    }
    let mut retake_info = None;
    let mut retake_warns = vec![];
    if a.cut_retakes {
        if a.keep.is_some() || a.keep_words.is_some() || a.cut_fillers || a.cut_silence || clip {
            return fail(
                Exit::Usage,
                "--cut-retakes does not mix with --keep, --keep-words, --cut-fillers, --cut-silence or --from/--to",
                "Use one way to choose the parts.",
            );
        }
        let doc = match crate::analyze::transcribe_terms(
            &ff,
            &a.input,
            a.lang.as_deref(),
            total,
            a.terms.as_ref(),
        ) {
            Ok((doc, _)) => doc,
            Err(mut env) => {
                env.command = "edit".into();
                return *env;
            }
        };
        let said: Vec<Sentence> =
            serde_json::from_value(doc["sentences"].clone()).unwrap_or_default();
        let found = retakes(&said);
        for r in found.iter().filter(|r| !r.differs.is_empty()) {
            retake_warns.push(json!({"gate": "retake_differs", "status": "warn",
                "detail": format!("the takes at {:.1} s differ: {}", r.start, r.differs.join(", ")),
                "fix": "Say which take you want. If both are meant, drop --cut-retakes and use --keep-words."}));
        }
        let all = crate::analyze::timed_words(&doc).unwrap_or_default();
        let kept: Vec<_> = all
            .iter()
            .filter(|w| {
                let mid = (w.start + w.end) / 2.0;
                !found.iter().any(|r| mid >= r.start && mid <= r.end)
            })
            .cloned()
            .collect();
        let parts = word_parts(&all, &kept, total, a.pad);
        if a.plan {
            let mut env = Envelope::ok("edit")
                .with("retakes", json!(found))
                .with("parts", json!(parts.map_or(vec![], |p| p.0)));
            env.findings = retake_warns;
            return env;
        }
        retake_info = Some(json!(found));
        match parts {
            Ok((p, _)) => keep_parts = Some(p),
            Err(e) => return fail(Exit::Usage, e, "Check the sound of the video."),
        }
    }
    if a.cut_still {
        let mut args: Vec<String> = vec!["-hide_banner".into(), "-nostdin".into()];
        args.extend(seek.clone());
        args.extend([
            "-i".into(),
            a.input.display().to_string(),
            "-an".into(),
            "-vf".into(),
            format!("freezedetect=n=-60dB:d={}", a.min_silence),
            "-f".into(),
            "null".into(),
            "-".into(),
        ]);
        // The input was checked at the start of `edit`; a refused file leaves FFmpeg with nothing to open.
        let args = crate::media_gate::guard(Path::new(""), &args).unwrap_or_default();
        let freezes = match Command::new(&ff.ffmpeg).args(&args).output() {
            Ok(o) => parse_freezes(&String::from_utf8_lossy(&o.stderr), total),
            Err(e) => {
                return fail(
                    Exit::Error,
                    format!("cannot start FFmpeg: {e}"),
                    "Check FFmpeg.",
                )
            }
        };
        // With sound, only where it is silent too: a still slide under a voice stays.
        let still = match silence_log(a.min_silence) {
            Ok(l) if l.contains("Audio:") => {
                overlap(&freezes, &parse_silences(&l, total), a.min_silence)
            }
            _ => freezes,
        };
        let keep = keep_segments(&still, total, a.pad);
        if keep.is_empty() {
            return fail(
                Exit::GateFailed,
                "the whole video stands still",
                "Raise --min-silence, or check the input.",
            );
        }
        still_info = Some(
            json!({"still": still.len(), "seconds": (still.iter().map(|(s, e)| e - s).sum::<f64>() * 100.0).round() / 100.0 + 0.0}),
        );
        cuts = keep.len().saturating_sub(1);
        kept = keep.iter().map(|(s, e)| e - s).sum();
        let expr = select_expr(&keep, fps);
        vf.push(format!("select='{expr}',setpts=N/FRAME_RATE/TB"));
        af.push(sound_select(&keep, fps));
    } else if a.cut_silence {
        let log = match silence_log(a.min_silence) {
            Ok(l) => l,
            Err(e) => {
                return fail(
                    Exit::Error,
                    format!("cannot start FFmpeg: {e}"),
                    "Check FFmpeg.",
                )
            }
        };
        if !log.contains("Audio:") {
            return fail(
                Exit::Usage,
                "the video has no audio track",
                "Silence cutting needs sound; drop --cut-silence.",
            );
        }
        let keep = keep_segments(&parse_silences(&log, total), total, a.pad);
        if keep.is_empty() {
            return fail(
                Exit::GateFailed,
                "the whole video is silent at this level",
                "Lower --noise-db (for example -45).",
            );
        }
        cuts = keep.len().saturating_sub(1);
        kept = keep.iter().map(|(s, e)| e - s).sum();
        let expr = select_expr(&keep, fps);
        vf.push(format!("select='{expr}',setpts=N/FRAME_RATE/TB"));
        af.push(sound_select(&keep, fps));
    }
    if let Some(parts) = &keep_parts {
        if let Some(p) = parts.iter().find(|p| p.1 > total + 0.05) {
            return fail(
                Exit::Usage,
                format!("{:.2}-{:.2} runs past the end ({total:.2} s)", p.0, p.1),
                "Take the times from `scriptvox analyze --transcript` of this video.",
            );
        }
        cuts = parts.len().saturating_sub(1);
        kept = parts.iter().map(|(s, e)| e - s).sum();
        let expr = select_expr(parts, fps);
        vf.push(format!("select='{expr}',setpts=N/FRAME_RATE/TB"));
        af.push(sound_select(parts, fps));
    }
    let mut size = rb.as_ref().map(|r| (r.width, r.height));
    let mut tracked: Option<serde_json::Value> = None;
    if let Some(shape) = &a.reframe {
        let Some((w, h)) = crate::make::frame(shape) else {
            return fail(
                Exit::Usage,
                format!("unknown frame {shape}"),
                "Use --reframe 9:16, 16:9, 1:1, 4:5, 3:4, 4:3, 2:3 or 21:9.",
            );
        };
        // The crop runs before the silence cut, so `t` in a tracked crop is the time in the input.
        let crop = if blur {
            crate::media::fit_blur(w, h, 0)
        } else if a.track {
            match crate::track::track(&ff, &a.input, total, a.subject.as_deref()) {
                Ok(tr) => {
                    let (ts, cx, cy) = &tr.path;
                    tracked = Some(
                        json!({"samples": tr.samples, "shots": tr.shots, "requests": tr.requests, "lost": tr.lost, "subject_size": tr.subject,
                        "x": cx.iter().map(|c| (c * 1000.0).round() / 1000.0).collect::<Vec<_>>()}),
                    );
                    crate::track::crop_filter(w, h, ts, cx, cy)
                }
                Err((exit, e)) => {
                    return fail(exit, e, "Retry, or drop --track for the centre crop.")
                }
            }
        } else {
            format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1")
        };
        vf.insert(0, crop);
        size = Some((w.into(), h.into()));
    }
    // First of all: the boxes are in the source's pixels and seconds.
    for (i, b) in blurs.iter().enumerate().rev() {
        vf.insert(0, blur_filter(b, i));
    }
    // Last, so cuts and a tracked crop still read the input's own times.
    if let Some(target) = a.duration {
        let s = kept / target;
        if !(0.5..=2.0).contains(&s) {
            return fail(
                Exit::Usage,
                format!(
                    "{kept:.2} s in {target:.2} s needs {s:.2}x, and the speed stays within 0.5 to 2"
                ),
                &format!(
                    "Give --duration from {:.1} to {:.1}, or cut first.",
                    kept / 2.0,
                    kept * 2.0
                ),
            );
        }
        a.speed = Some(s);
    }
    let mut speed_steps = Vec::new();
    let mut speed_info = None;
    let mut uncut = false;
    if let Some(spec) = a.speed_ranges.as_deref() {
        let ranges = match crate::edit_speed::parse(spec, kept) {
            Ok(r) => r,
            Err(e) => {
                return fail(
                    Exit::Usage,
                    e,
                    &format!("Give times from 0 to {kept:.2}. {}", crate::edit_speed::FIX),
                )
            }
        };
        speed_steps = crate::edit_speed::steps(&ranges);
        let at = |t: f64| (crate::edit_speed::map(&speed_steps, t) * 1000.0).round() / 1000.0;
        speed_info = Some(json!({
            "ease_s": crate::edit_speed::EASE_S, "steps": crate::edit_speed::EASE_STEPS,
            "ranges": ranges.iter().map(|r| json!({"from": r.from, "to": r.to, "speed": r.speed,
                "result_from": at(r.from), "result_to": at(r.to)})).collect::<Vec<_>>(),
        }));
        // The same `fps` after it as for one speed, below.
        vf.push(format!(
            "{},fps={fps}",
            crate::edit_speed::setpts(&speed_steps)
        ));
        af.push(crate::edit_speed::sound(&speed_steps));
        uncut = (kept - total).abs() < 1e-6 && !clip;
        kept = crate::edit_speed::map(&speed_steps, kept);
        // The stand-in has done its work.
        a.speed = None;
    }
    if let Some(s) = a.speed {
        // One speed for the whole video is one step with no ease: the caption files move through the same map as
        // for a range (a plain `--speed 1.25` left them at the old times, 13 Nov 2026).
        if (s - 1.0).abs() > 1e-9 {
            uncut = (kept - total).abs() < 1e-6 && !clip;
            speed_steps = vec![crate::edit_speed::Step {
                from: 0.0,
                to: kept,
                speed: s,
            }];
            speed_info = Some(json!({"speed": (s * 1000.0).round() / 1000.0}));
        }
        // `fps` again: the stretched picture must land on the same frame grid, or the encoder drops frames (slow play
        // drifted 186 ms from the sound).
        vf.push(format!("setpts=PTS/{s},fps={fps}"));
        af.push(format!("atempo={s}"));
        kept /= s;
    }
    // The caption files of the run move with the map, when no cut has changed their times first.
    let retime_sidecars = uncut && a.captions.is_none();
    let out = a
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("scriptvox-out").join(format!("{stem}-edit.mp4")));
    if let Some(p) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(p);
    }
    let enc = match ff.pick_encoder() {
        Ok((e, _)) => e,
        Err(e) => {
            return fail(
                Exit::Error,
                e,
                "Install an FFmpeg build with libx264 or libopenh264.",
            )
        }
    };
    let abs = |p: &Path| {
        std::path::absolute(p)
            .unwrap_or(p.to_path_buf())
            .display()
            .to_string()
    };
    let mut args = seek.clone();
    args.extend(["-i".into(), abs(&a.input)]);
    let burned = a.captions.is_some() || a.hook.is_some() || a.label.is_some();
    // Captions: the speech comes from a sound-only pass with the same cuts, so the one video encode burns them in.
    let mut caption_info = None;
    let mut text_finding = None;
    let mut bounds = None;
    let mut cap: Option<String> = None;
    if burned {
        let Some((w, h)) = size else {
            return fail(
                Exit::Usage,
                "cannot read the video size",
                "Give a video FFmpeg can read.",
            );
        };
        let mut sound = args.clone();
        if !af.is_empty() {
            sound.extend(["-af".into(), af.join(",")]);
        }
        match write_captions(
            &ff,
            sound,
            (w as u32, h as u32),
            &work,
            &a,
            &second,
            bleep.as_ref(),
        ) {
            Ok(v) => caption_info = a.captions.is_some().then_some(v),
            Err((exit, e, fix)) => return fail(exit, e, fix),
        }
        // The same gate as `make`, on every box this edit burns. No scene numbers: an edit has no scenes.
        bounds = std::fs::read_to_string(work.join("captions.ass"))
            .ok()
            .and_then(|ass| crate::gates::text_bounds(&ass, (w as u32, h as u32), &[]));
        cap = Some(format!(
            "{},format=yuv420p",
            crate::filterpath::ass_filter("captions.ass", "fonts")
        ));
    }
    if let Some(gate) = caption_info
        .as_mut()
        .and_then(|c: &mut serde_json::Value| c.as_object_mut().and_then(|o| o.remove("text_gate")))
    {
        text_finding = (!gate.is_null()).then_some(gate);
    }
    if a.zoom_on_emphasis {
        let at: Vec<(f64, f64)> = caption_info
            .as_ref()
            .and_then(|c: &serde_json::Value| serde_json::from_value(c["emphasis_at"].clone()).ok())
            .unwrap_or_default();
        zooms = emphasis_zooms(&at);
    }
    if !zooms.is_empty() {
        let Some((fw, fh)) = size else {
            return fail(
                Exit::Usage,
                "cannot read the size of this video",
                "Drop --zoom.",
            );
        };
        // Before the captions and the logo, so a push never moves or hides them.
        vf.push(zoom_filter(&zooms, (fw as u32, fh as u32), fps));
    }
    // Music alone does not touch the picture.
    // Only a codec an MP4 can hold: VP8 or ProRes would stop the copy.
    let picture_copied =
        (a.music.is_some() || !sfx.is_empty() || a.clean_voice.is_some() || bleep.is_some())
            && brolls.is_empty()
            && !clip
            && vf.is_empty()
            && cap.is_none()
            && a.logo.is_none()
            && ff
                .video_codec(&a.input)
                .is_some_and(|c| matches!(c.as_str(), "h264" | "hevc" | "av1" | "mpeg4"));
    match (&a.logo, size) {
        (Some(l), Some((w, h))) => {
            // The logo needs a second input, so the picture goes through a graph (same overlay as `make`).
            let (graph, label) = broll_graph(&vf, &brolls, 2, (w as u32, h as u32), fps);
            let v = crate::make::Visual {
                inputs: vec![],
                graph,
                label,
            };
            let logo = crate::brief::Logo {
                file: l.clone(),
                corner: None,
                width: None,
                opacity: None,
            };
            args.extend(["-i".into(), abs(l)]);
            args.extend(broll_inputs(&brolls, &abs));
            let mut g = crate::make::logo_graph(&v, &logo, 1, (w as u32, h as u32), None);
            // Captions over the logo, as in `make`.
            if let Some(c) = &cap {
                g = format!("{g};[logo]{c}[capt]");
            }
            args.extend([
                "-filter_complex".into(),
                g,
                "-map".into(),
                if cap.is_some() { "[capt]" } else { "[logo]" }.into(),
                "-map".into(),
                "0:a?".into(),
            ]);
        }
        (None, Some((w, h))) if !brolls.is_empty() => {
            args.extend(broll_inputs(&brolls, &abs));
            let (mut g, mut label) = broll_graph(&vf, &brolls, 1, (w as u32, h as u32), fps);
            // Captions over the pictures, as with the logo.
            if let Some(c) = &cap {
                g = format!("{g};{label}{c}[capt]");
                label = "[capt]".into();
            }
            args.extend([
                "-filter_complex".into(),
                g,
                "-map".into(),
                label,
                "-map".into(),
                "0:a?".into(),
            ]);
        }
        _ => {
            vf.extend(cap);
            if !vf.is_empty() {
                args.extend(["-vf".into(), vf.join(",")]);
            }
        }
    }
    if !af.is_empty() {
        args.extend(["-af".into(), af.join(",")]);
    }
    if picture_copied {
        args.extend(["-c:v".into(), "copy".into()]);
    } else {
        args.extend(enc.args());
    }
    args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
    // With a bleep this pass writes into the work folder: the file at --out never holds the word, even if the run
    // stops between the two passes.
    let first = if bleep.is_some() {
        work.join("before-bleep.mp4")
    } else {
        out.clone()
    };
    args.extend(["-movflags".into(), "+faststart".into(), abs(&first)]);
    // With captions FFmpeg runs in the work folder: the subtitle filter reads captions.ass and fonts/ there.
    let run = if burned {
        let fc = match crate::template::write_fonts(&work) {
            Ok(fc) => fc,
            Err(e) => return fail(Exit::Error, e.to_string(), "Check the folder is writable."),
        };
        ff.run(&work, &args, Some(&fc))
    } else {
        ff.run(Path::new("."), &args, None)
    };
    if let Err(e) = run {
        return fail(Exit::Error, e, "Check the input video plays.");
    }
    if let Some(b) = &bleep {
        let tone = a.bleep_with.as_deref() != Some("silence");
        let done = bleep_sound(&ff, &first, &out, &work, &b.windows, tone);
        let _ = std::fs::remove_file(&first);
        if let Err(e) = done {
            let _ = std::fs::remove_file(&out);
            return fail(
                Exit::Error,
                format!("the bleep failed, and no file was written: {e}"),
                "Check the input's sound plays, and that the disk has room.",
            );
        }
    }
    if let Some(m) = &music {
        if let Err(e) = add_music(&ff, &out, &work, m, music_level) {
            return fail(Exit::Error, e, "Check the music file plays.");
        }
    }
    // After the cuts, the speed and the bed, so each time is a time of the result.
    let mut sfx_info = None;
    if !sfx.is_empty() {
        let got = ff.duration(&out).unwrap_or(kept);
        if let Some(c) = sfx.iter().find(|c| c.at > got) {
            let _ = std::fs::remove_file(&out);
            return fail(
                Exit::Usage,
                format!(
                    "--sfx {}@{}: the time is after the end of the video ({got:.2} s)",
                    c.name, c.at
                ),
                &format!("Give a time from 0 to {got:.2}."),
            );
        }
        match crate::edit_sfx::add(&ff, &out, &sfx, a.sfx_level.as_deref()) {
            Ok(v) => sfx_info = Some(v),
            Err(e) => {
                let _ = std::fs::remove_file(&out);
                return fail(
                    Exit::Usage,
                    format!("--sfx: {e}"),
                    "Give a real sound file inside this project, or whoosh, pop or tick.",
                );
            }
        }
    }
    let loud = master(&ff, &out);
    let mut env = Envelope::ok("edit");
    // The edit's own sync check: a cut that shifts sound against picture shows here, not only in a later `check`.
    // Only av_drift: the other gates judge a made video, and a talk with a still camera is not a slideshow.
    if !ff.has_stream(&out, "video") {
        return fail(
            Exit::Error,
            format!("{} came out with no video stream", out.display()),
            "Give another --out, and check that the input plays.",
        );
    }
    match crate::gates::check(&ff, &out) {
        Ok(found) => {
            env.findings = found
                .iter()
                .filter(|f| f.gate == "av_drift")
                .map(|f| json!(f))
                .collect();
        }
        Err(e) => {
            env = fail(
                Exit::GateFailed,
                format!("gate failed: {e}"),
                "The video stays for inspection. Run the edit again.",
            );
        }
    }
    // New captions on a video that has its own burned text: both show, one over the other (test 4). Free vision row;
    // if it does not answer, no finding.
    if a.captions.is_some() {
        if let Ok(Some(what)) = crate::judge::burned_text(&ff, &a.input, total, &work) {
            env.findings.push(json!({"gate": "base_text", "status": "warn",
                "detail": format!("the source has its own burned text ({what}): the new captions land on or near it"),
                "fix": "Use footage without burned text, or look at the frames and drop --captions."}));
        }
        // The default name is this tool's own; a named --out, or a caption file given to read, may be the user's.
        let keep = a.out.is_some() || a.captions_file.is_some();
        crate::captions::sidecars(&work, &out, keep, &mut env);
    }
    // A push moves burned text with the picture, and can cut it off. New captions stay still on top.
    if !zooms.is_empty() && a.captions.is_none() {
        if let Ok(Some(what)) = crate::judge::burned_text(&ff, &a.input, total, &work) {
            env.findings.push(json!({"gate": "base_text", "status": "warn",
                "detail": format!("the source has its own burned text ({what}): a push moves it and can cut it off"),
                "fix": "Look at the frames during each push. A smaller zoom or a centre near the text keeps it."}));
        }
    }
    // What a crop keeps, so a report can say it without reading frames.
    if let (Some(_), Some(rb), Some((w, h)), false) = (&a.reframe, &rb, size, blur) {
        let (sw, sh) = (rb.width as f64, rb.height as f64);
        let keep = (sh * w as f64 / h as f64 / sw).min(1.0);
        env.metrics
            .insert("crop_keeps_width_pct".into(), json!((keep * 100.0).round()));
        // A crop cannot follow a subject bigger than itself: it stays on the middle and cuts both ends.
        let keep_h = (sw * h as f64 / w as f64 / sh).min(1.0);
        let boxed = tracked
            .as_ref()
            .and_then(|t| serde_json::from_value::<(f64, f64)>(t["subject_size"].clone()).ok());
        if let Some(what) = boxed.and_then(|b| too_big_for_crop(b, keep, keep_h)) {
            env.findings.push(json!({"gate": "track_subject", "status": "warn",
                "detail": what,
                "fix": "Look at the frames. A wider frame such as --reframe 1:1 or 4:5 may hold the subject, or keep the source shape."}));
        }
        if !a.track && (keep < 0.95 || keep_h < 0.95) {
            let span = (a.from.unwrap_or(0.0).max(0.0), total);
            if let Some(what) = crate::track::centre_crop_cuts(
                &ff,
                &a.input,
                span,
                (keep, keep_h),
                a.subject.as_deref(),
                &work,
            ) {
                env.findings.push(json!({"gate": "centre_crop", "status": "warn",
                    "detail": what,
                    "fix": "Add --track, so the crop follows the subject, or use a wider frame. Then look at the frames."}));
            }
        }
    }
    // The output must be as long as the edit says, whatever the reason it is not.
    let got = ff.duration(&out).unwrap_or(kept);
    if (got - kept).abs() > 0.5 {
        env.findings.push(json!({
            "gate": "length",
            "status": "warn",
            "detail": format!("the file runs {got:.2} s, the edit kept {kept:.2} s"),
            "fix": "Look at the video. Its picture or sound may be shorter than the other.",
        }));
    }
    if let Some(l) = loud {
        env.metrics.insert("loudness_lufs".into(), json!(l));
    }
    if let Some(v) = filler_info {
        env = env.with("fillers", v);
    }
    if let Some(v) = still_info {
        env = env.with("cut_still", v);
    }
    if let Some(v) = retake_info {
        env = env.with("retakes", v);
    }
    env.findings.extend(text_finding);
    env.findings.extend(bounds.map(|mut f: crate::gates::Finding| {
        if f.status == "fail" {
            f.fix = Some("Shorten the --hook or --label text, or use shorter words, then run the edit again.".into());
        }
        json!(f)
    }));
    let mut want = vec![
        ("black", "`edit` runs only av_drift; run `scriptvox check` on the result"),
        ("loudness", "`edit` runs only av_drift; run `scriptvox check` on the result"),
        ("words", "`edit` does not compare words unless --text is given; run `scriptvox check --hear --script`"),
    ];
    if a.captions.is_some() || !zooms.is_empty() {
        want.push((
            "base_text",
            "no warning, or no vision service answered; look at the frames",
        ));
    }
    let not_checked = crate::gates::not_checked(&env.findings, &want);
    env = env.with("not_checked", json!(not_checked));
    let (coverage, limitations) = crate::gates::limits(&env.findings);
    env = env
        .with("coverage", json!(coverage))
        .with("limitations", json!(limitations));
    if !zooms.is_empty() {
        env = env.with("zoom", json!(zooms));
    }
    if !blurs.is_empty() {
        // A frame from the middle of each box, so the agent or a person can see what the blur hides and what it does
        // not.
        let mids: Vec<f64> = blurs.iter().map(|b| (b.from + b.to) / 2.0).collect();
        let check = PathBuf::from("scriptvox-out").join(format!("{stem}-blur.png"));
        if crate::gates::snapshot(&ff, &out, &mids, None, &check).is_ok() {
            env.artifacts.insert(
                "blur_check".into(),
                json!(std::path::absolute(&check).unwrap_or(check)),
            );
        }
        env = env.with("blur", json!(blurs)).with(
            "blur_note",
            json!("A blur is not proof of redaction: blurred text or a face can sometimes be read or matched. Look at blur_check; for a release under a redaction rule, use your agency's redaction tool."),
        );
    }
    if !brolls.is_empty() {
        env = env.with("broll", json!(brolls));
    }
    if let Some(v) = sfx_info {
        env = env.with("sfx", v);
    }
    // Music from an address: its source beside the video and in the answer. Nothing is drawn on the picture.
    if let Some(p) = &music_info {
        let credits = out.with_extension("credits.txt");
        let credits = std::path::absolute(&credits).unwrap_or(credits);
        if std::fs::write(&credits, format!("{}\n", crate::make::fetch_credit(p))).is_ok() {
            env.artifacts.insert("credits".into(), json!(credits));
        }
        env = env.with(
            "music",
            json!({"address": music_url, "from": a.music_cut.0, "to": a.music_cut.1, "source": p}),
        );
    }
    if let Some(mut v) = speed_info {
        let mut moved = Vec::new();
        if retime_sidecars {
            let retimed = work.join("retimed");
            let _ = std::fs::remove_dir_all(&retimed);
            let _ = std::fs::create_dir_all(&retimed);
            for ext in ["srt", "vtt"] {
                let Ok(bytes) = std::fs::read(a.input.with_extension(ext)) else {
                    continue;
                };
                let Ok(text) = decode_srt(&bytes) else {
                    continue;
                };
                let new = crate::edit_speed::retime(&text, &speed_steps);
                if std::fs::write(retimed.join(format!("captions.{ext}")), new).is_ok() {
                    moved.push(ext);
                }
            }
            // The same rule as for new captions: a file of the user's with that name stays.
            crate::captions::sidecars(&retimed, &out, a.out.is_some(), &mut env);
        }
        v["captions_retimed"] = json!(moved);
        env = env.with("speed", v);
    }
    if let Some(level) = &a.clean_voice {
        let mut v = json!({"level": level, "noise_floor_db_before": noise_floor(&ff, &a.input),
                "noise_floor_db_after": noise_floor(&ff, &out)});
        if let Some(h) = &hum {
            let after = crate::hum::read(&ff, &out);
            (v["hum"], v["rumble"]) = h.result(after.as_deref());
        }
        env = env.with("clean_voice", v);
    }
    env.findings.extend(retake_warns);
    if let Some(b) = &bleep {
        if !b.not_heard.is_empty() {
            env.findings.push(json!({"gate": "bleep_not_heard", "status": "warn",
                "detail": format!("not heard, so not hidden: {}", b.not_heard.join(", ")),
                "fix": "`scriptvox analyze <video> --transcript` shows how the speech row spells each word: give that spelling, or the right times in a --words file. Then listen."}));
        }
        env = env.with(
            "bleep",
            json!({"with": a.bleep_with.as_deref().unwrap_or("tone"), "pad_s": b.pad, "fade_s": BLEEP_FADE, "replaced": b.windows,
                "heard": b.heard.len(), "matches": b.matches, "not_heard": b.not_heard,
                "note": "Only what speech to text heard and matched is hidden, in the sound and in new captions. Text already burned in the picture stays. Listen to the result before a release."}),
        );
    }
    if let Some(v) = word_info {
        env = env.with("keep_words", v);
    }
    if let Some(v) = caption_info {
        env = env.with("captions", v);
    }
    env.artifacts.insert(
        "video".into(),
        json!(std::path::absolute(&out).unwrap_or(out)),
    );
    env.metrics.insert(
        "seconds_before".into(),
        json!((total * 100.0).round() / 100.0),
    );
    // The file's real length: `kept` is the plan, and a cut lands on frames.
    env.metrics
        .insert("seconds_after".into(), json!((got * 100.0).round() / 100.0));
    env.metrics.insert("cuts".into(), json!(cuts));
    if picture_copied {
        env.metrics.insert("picture_copied".into(), json!(true));
    }
    if let Some((w, h)) = size {
        env.metrics.insert("size".into(), json!(format!("{w}x{h}")));
    }
    fail_on_gate(
        match tracked {
            Some(t) => env.with("track", t),
            None => env,
        },
        |g| g == "av_drift" || g == "text_bounds",
    )
}

/// The words for a subject bigger than the crop, from its box and the share of the frame that the crop keeps.
fn too_big_for_crop(subject: (f64, f64), keep_w: f64, keep_h: f64) -> Option<String> {
    let pct = |x: f64| (x * 100.0).round();
    if subject.0 > keep_w {
        Some(format!(
            "the subject takes {} percent of the frame width and the crop keeps {} percent: the crop cuts its sides",
            pct(subject.0),
            pct(keep_w)
        ))
    } else if subject.1 > keep_h {
        Some(format!(
            "the subject takes {} percent of the frame height and the crop keeps {} percent: the crop cuts its top or bottom",
            pct(subject.1),
            pct(keep_h)
        ))
    } else {
        None
    }
}

/// Exit 3 when a gate that `pick` names failed, as `check` does. The file stays, and the report keeps its facts.
pub fn fail_on_gate(mut env: Envelope, pick: impl Fn(&str) -> bool) -> Envelope {
    let Some(f) = env
        .findings
        .iter()
        .find(|f| f["status"] == "fail" && pick(f["gate"].as_str().unwrap_or("")))
    else {
        return env;
    };
    let failed = Envelope::fail(
        &env.command,
        Exit::GateFailed,
        format!(
            "gate failed: {} ({})",
            f["gate"].as_str().unwrap_or(""),
            f["detail"].as_str().unwrap_or("")
        ),
        Some(format!(
            "The file is kept at the path in `artifacts`. {}",
            f["fix"].as_str().unwrap_or("")
        )),
    );
    env.ok = failed.ok;
    env.status = failed.status;
    env.exit = failed.exit;
    env.error = failed.error;
    env
}

/// The frame a ScriptVox MP4 was made in, when a crop to `shape` would cut its burned captions and cards.
pub fn crop_cuts_text(ff: &Ffmpeg, input: &Path, shape: &str) -> Option<String> {
    let r = ff.recipe(input).ok()?;
    let made = r["request"]["aspect"]
        .as_str()
        .unwrap_or("9:16")
        .to_string();
    (made != shape).then_some(made)
}

/// `--music` as a file to check: a bed name and a file go on, an address is not a file (it is fetched later).
fn music_file(music: &Option<String>) -> Option<PathBuf> {
    music
        .as_deref()
        .filter(|m| !crate::fetch::is_address(m))
        .map(PathBuf::from)
}

/// Puts the bed under the file's own sound with the same mix as `make` (the speech ducks it, -14 LUFS). A file with no
/// sound gets the bed alone. The picture is copied.
fn add_music(
    ff: &Ffmpeg,
    out: &Path,
    work: &Path,
    music: &crate::audio::Music,
    level: f64,
) -> Result<(), String> {
    let abs = |p: &Path| std::path::absolute(p).unwrap_or(p.to_path_buf());
    let secs = ff.duration(out)?;
    let voice = if ff.has_stream(out, "audio") {
        abs(out)
    } else {
        let quiet = abs(&work.join("quiet.wav"));
        let lavfi = format!("anullsrc=r=48000:cl=stereo:d={secs:.3}");
        ff.run(
            work,
            &[
                "-f".into(),
                "lavfi".into(),
                "-i".into(),
                lavfi,
                quiet.display().to_string(),
            ],
            None,
        )?;
        quiet
    };
    let mix = crate::audio::master(ff, &abs(work), &voice, music, &[], &[], &[], secs, level)?;
    let tmp = abs(&out.with_extension("music.mp4"));
    let args: Vec<String> = [
        "-i",
        &abs(out).display().to_string(),
        "-i",
        &mix.display().to_string(),
        "-map",
        "0:v",
        "-map",
        "1:a",
        "-c:v",
        "copy",
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-shortest",
        "-movflags",
        "+faststart",
        &tmp.display().to_string(),
    ]
    .map(String::from)
    .to_vec();
    ff.run(work, &args, None)?;
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())
}

/// Speech to text (a speech to text row) on the sound that `input` (FFmpeg input and sound filter arguments)
/// gives, then the same captions as `make` in `work/captions.ass` for a `(w, h)` picture.
fn write_captions(
    ff: &Ffmpeg,
    input: Vec<String>,
    (w, h): (u32, u32),
    work: &Path,
    a: &Args,
    second: &[String],
    bleep: Option<&Bleep>,
) -> Result<serde_json::Value, (Exit, String, &'static str)> {
    let style = a.captions.as_deref().unwrap_or("karaoke");
    let (lang, accent) = (a.lang.as_deref(), a.accent.as_deref());
    // A hook alone needs no speech.
    let hint = a.terms.as_ref().map(|t| t.hint());
    let mut words = if let Some(file) = &a.captions_file {
        let words = read_caption_file(file).map_err(|e| {
            (
                Exit::Usage,
                e,
                "Give an SRT file, or JSON with `words`: text, start and end.",
            )
        })?;
        // With --from/--to the file keeps the source's times: the words of that part, moved to start at 0.
        let (from, to) = (a.from.unwrap_or(0.0), a.to.unwrap_or(f64::MAX));
        words
            .into_iter()
            .filter(|w| (w.start + w.end) / 2.0 >= from && (w.start + w.end) / 2.0 < to)
            .map(|w| crate::captions::Word {
                start: (w.start - from).max(0.0),
                end: (w.end.min(to) - from).max(0.0),
                ..w
            })
            .collect()
    } else if let (Some(b), true) = (bleep, a.captions.is_some()) {
        // The words the bleep was matched on: one speech pass, and no word that only one of two passes heard.
        b.heard.clone()
    } else if a.captions.is_some() {
        hear(
            ff,
            input,
            work,
            lang,
            hint.as_deref().filter(|h| !h.is_empty()),
            "speech",
        )?
    } else {
        Vec::new()
    };
    let term_fixes = a.terms.as_ref().map_or(vec![], |t| t.fix_words(&mut words));
    let mut text_gate = None;
    if let Some(file) = &a.text {
        let script = std::fs::read_to_string(file).map_err(|e| {
            (
                Exit::Usage,
                format!("{}: {e}", file.display()),
                "Give a text file with the words of the speech.",
            )
        })?;
        let heard: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        let gate = crate::gates::words_heard(&script, &heard.join(" "));
        if gate.status == "fail" {
            return Err((
                Exit::GateFailed,
                format!("this text is not what the video says: {}", gate.detail),
                "Give the words of this speech. After a cut, give only the words that stay.",
            ));
        }
        text_gate = Some(gate);
        words = crate::captions::align(&script, &words);
    }
    if words.is_empty() && a.captions.is_some() {
        return Err((
            Exit::GateFailed,
            "no speech found for captions".into(),
            "Check the sound, or drop --captions.",
        ));
    }
    let all: String = words
        .iter()
        .map(|w| w.text.as_str())
        .chain(a.hook.as_deref())
        .chain(a.label.as_deref())
        .collect::<Vec<_>>()
        .join(" ");
    let lang = lang
        .map(String::from)
        .unwrap_or_else(|| crate::make::detect_lang(&all).to_string());
    // After the term fixes, the words gate and the alignment to --text, which all need the real words.
    let masked = bleep.map(|b| mask_words(&mut words, b));
    let (family, font, size, pages, mut st) =
        crate::make::caption_pages(&words, &lang, (w, h), style, a.caption_style.scale());
    let sentences = {
        let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        crate::captions::sentences(&texts).len()
    };
    if !second.is_empty() && sentences != second.len() {
        return Err((
            Exit::Usage,
            format!(
                "--captions-second has {} lines, the captions have {sentences} sentences",
                second.len()
            ),
            "Give one line per sentence, in order. With --text the sentences are yours; else they are what speech to text heard.",
        ));
    }
    // The second line under the main captions, which move up (as `make` does with brief `captions_second`).
    let second =
        (!second.is_empty()).then(|| crate::make::second_layout(second, None, font, size, w));
    let base_margin = st.margin_v;
    if let Some((_, size2, lines)) = &second {
        st.margin_v += crate::make::second_raise(lines, *size2, st.size);
    }
    let wanted: Vec<String> = a
        .emphasis
        .iter()
        .flat_map(|e| e.split(','))
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
        .collect();
    let (found, missing) = crate::captions::find_emphasis(&words, &wanted);
    let emphasis_at: Vec<(f64, f64)> = found
        .iter()
        .map(|&i| (words[i].start, words[i].end))
        .collect();
    st.emphasis = found;
    let mut ass = crate::captions::to_ass(&pages, &words, &st);
    // The same order as `make`: the style line first, then the second line, the hook and the label.
    ass = a.caption_style.apply(&ass, w.min(h));
    if let Some((fam2, size2, lines)) = &second {
        let cues = crate::make::second_cues(&words, &pages, lines);
        ass = crate::captions::with_second(&ass, &cues, fam2, *size2, base_margin);
    }
    if let Some(hook) = a.hook.as_deref().filter(|h| !h.trim().is_empty()) {
        ass = crate::make::hook_ass(&ass, hook, family, font, size, w);
    }
    if let Some(label) = a.label.as_deref().filter(|l| !l.trim().is_empty()) {
        ass = crate::captions::with_label(&ass, label.trim(), family, (size * 3 / 5).max(14));
    }
    if let Some(c) = accent.and_then(crate::captions::ass_colour) {
        ass = ass.replace(crate::captions::ACCENT, &c);
    }
    let _ = std::fs::write(
        work.join("captions.srt"),
        crate::captions::to_srt(&pages, &words),
    );
    let _ = std::fs::write(
        work.join("captions.vtt"),
        crate::captions::to_vtt(&pages, &words),
    );
    std::fs::write(work.join("captions.ass"), ass)
        .map_err(|e| (Exit::Error, e.to_string(), "Check the folder is writable."))?;
    Ok(
        json!({"words": words.len(), "pages": pages.len(), "lang": lang, "style": style, "second_lines": second.as_ref().map(|s| s.2.len()), "emphasis_not_heard": missing, "emphasis_at": emphasis_at, "term_fixes": term_fixes, "text_gate": text_gate, "masked": masked}),
    )
}

/// The timed words of the sound that `input` (FFmpeg input and sound filter arguments) gives, from a
/// Whisper row. `prompt` steers Whisper's style; `name` keeps the WAV of each use apart in `work`.
fn hear(
    ff: &Ffmpeg,
    input: Vec<String>,
    work: &Path,
    lang: Option<&str>,
    prompt: Option<&str>,
    name: &str,
) -> Result<Vec<crate::captions::Word>, (Exit, String, &'static str)> {
    let stt = crate::opentts::stt_row(false).ok_or((
        Exit::NeedsInput,
        crate::analyze::NO_STT.to_string(),
        "Run `scriptvox setup speech-local`. `scriptvox providers` lists what this install has.",
    ))?;
    let wav = work.join(format!("{name}.wav"));
    let mut args = input;
    args.extend(
        [
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-c:a",
            "pcm_s16le",
            &wav.display().to_string(),
        ]
        .map(String::from),
    );
    ff.run(Path::new("."), &args, None).map_err(|e| {
        (
            Exit::Error,
            e,
            "The video needs a sound track for --captions and --cut-fillers.",
        )
    })?;
    crate::opentts::stt_gate(&stt, &wav).map_err(|m| {
        (
            Exit::ApprovalNeeded,
            m,
            "Run the same command with --approve <plan_id>.",
        )
    })?;
    if std::fs::metadata(&wav).is_ok_and(|m| m.len() > crate::analyze::WINDOW_BYTES) {
        // Too big for one request: the same overlapping windows as `analyze`, each its own job.
        let secs = ff
            .duration(&wav)
            .map_err(|e| (Exit::Error, e, "Check the video's sound."))?;
        let mut parts = Vec::new();
        for (i, (start, len)) in crate::analyze::windows(secs).into_iter().enumerate() {
            let piece = work.join(format!("{name}-w{i}.wav"));
            let args: Vec<String> = vec![
                "-ss".into(),
                format!("{start:.3}"),
                "-t".into(),
                format!("{len:.3}"),
                "-i".into(),
                wav.display().to_string(),
                "-c".into(),
                "copy".into(),
                piece.display().to_string(),
            ];
            ff.run(Path::new("."), &args, None)
                .map_err(|e| (Exit::Error, e, "Check the disk has room."))?;
            let words = hear_one(&stt, &piece, work, lang, prompt)?;
            let _ = std::fs::remove_file(&piece);
            parts.push((start, serde_json::json!({ "words": words })));
        }
        let merged = crate::analyze::merge_windows(&parts);
        return serde_json::from_value(merged["words"].clone())
            .map_err(|e| (Exit::Error, e.to_string(), "Run the same command again."));
    }
    hear_one(&stt, &wav, work, lang, prompt)
}

/// One speech to text job on one WAV.
fn hear_one(
    stt: &crate::generators::Generator,
    wav: &Path,
    work: &Path,
    lang: Option<&str>,
    prompt: Option<&str>,
) -> Result<Vec<crate::captions::Word>, (Exit, String, &'static str)> {
    // The job record is keyed by the speech itself, so a rerun of the same edit reuses the transcript.
    // The version in the key: a change in the speech to text engine (3 Nov: a punctuation prompt) must not be hidden
    // by an old transcript.
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"stt-v2");
    hasher.update(prompt.unwrap_or_default().as_bytes());
    hasher.update(&std::fs::read(wav).unwrap_or_default());
    let key = hasher.finalize().to_hex()[..12].to_string();
    crate::opentts::transcribe(
        stt,
        wav,
        lang,
        work,
        &format!("stt_{key}"),
        &format!("edit_{key}"),
        prompt,
    )
    .map_err(|e| {
        (
            Exit::ProviderRetryable,
            e.message,
            "Run the same command again.",
        )
    })
}

/// Whisper leaves out "um" and "uh" unless its prompt has them: the prompt's style is copied.
const FILLER_PROMPT_EN: &str = "Umm, so, uh, let me think. Hmm, okay, um, the plan is, uh, simple.";
const FILLER_PROMPT_AR: &str = "يعني، اممم، خلني أفكر. ااا، طيب، إمم، الفكرة، يعني، بسيطة.";

/// A hesitation sound. Letters run together ("ummm", "اااا") count once. Arabic "أم" and "ام" are also the
/// word "or", so they count only when a letter is drawn out ("أممم").
fn hesitation(text: &str) -> bool {
    let raw: Vec<char> = text
        .chars()
        .filter(|c| c.is_alphabetic())
        .flat_map(char::to_lowercase)
        .collect();
    let mut t = raw.clone();
    t.dedup();
    let t: String = t.into_iter().collect();
    match t.as_str() {
        "um" | "uh" | "uhm" | "erm" | "er" | "ah" | "eh" | "ehm" | "hm" | "m" | "mhm" => true,
        "ا" | "آ" | "م" | "اه" => raw.len() > 1 || t == "م",
        "ام" | "أم" | "إم" | "اام" => raw.len() > t.chars().count(),
        _ => false,
    }
}

/// Spoken words that are fillers only with a pause in the sound on both sides ("يعني" mid-sentence can mean "it
/// means"; Whisper's times showed no gap where the sound had 0.31 s and 0.25 s, 4 Nov 2026).
const SOFT_FILLERS: &[&str] = &["يعني"];
const SOFT_PAUSE_S: f64 = 0.2;

/// The longest sound a filler alone makes, and the slack after Whisper's word end where its sound may start.
const FILLER_MAX_S: f64 = 0.8;
const FILLER_LAG_S: f64 = 0.4;

/// The parts to keep without the fillers, each filler cut, and how many were skipped (no clear sound of their own).
/// Whisper's time for a filler is often the pause before it (4 Nov 2026: all 5 fillers of a talk sat in the
/// pause, the sound started at their end), so the cut snaps to the sound: the short island between two silences of
/// `quiet` that overlaps most with the word's time plus `FILLER_LAG_S`. The cut takes the island and its pauses, less
/// `pad` of air on each side; fillers in a row make one cut.
pub fn filler_keep(
    words: &[crate::captions::Word],
    total: f64,
    pad: f64,
    quiet: &[(f64, f64)],
) -> (Vec<(f64, f64)>, Vec<serde_json::Value>, usize) {
    let r = |t: f64| (t * 100.0).round() / 100.0;
    // Sound islands, each with the silence before and after: (silence start, sound start, sound end, silence end).
    let mut islands = Vec::new();
    let mut at = (0.0, 0.0);
    for &(a, b) in quiet.iter().chain([&(total, total)]) {
        if a > at.1 {
            islands.push((at.0, at.1, a, b));
        }
        at = (a, b);
    }
    let mut cuts: Vec<(f64, f64)> = Vec::new();
    let mut said = Vec::new();
    let mut skipped = 0;
    for w in words {
        let bare: String = w.text.chars().filter(|c| c.is_alphabetic()).collect();
        let soft = SOFT_FILLERS.contains(&bare.as_str());
        if !(hesitation(&w.text) || soft) {
            continue;
        }
        let (lo, hi) = (w.start, w.end + FILLER_LAG_S);
        let best = islands
            .iter()
            .filter(|x| x.2 - x.1 <= FILLER_MAX_S)
            .map(|x| ((x.2.min(hi) - x.1.max(lo)).max(0.0), x))
            .max_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, &(s0, a, b, e1))) = best.filter(|(o, x)| {
            *o >= 0.1 && !(soft && (x.1 - x.0 < SOFT_PAUSE_S || x.3 - x.2 < SOFT_PAUSE_S))
        }) else {
            skipped += 1;
            continue;
        };
        let (lo, hi) = ((s0 + pad).min(a), (e1 - pad).max(b).min(total));
        said.push(json!({"text": w.text, "start": r(a), "end": r(b)}));
        match cuts.last_mut() {
            Some(c) if lo <= c.1 => c.1 = c.1.max(hi),
            _ => cuts.push((lo, hi)),
        }
    }
    let mut keep = Vec::new();
    let mut at = 0.0;
    for (lo, hi) in cuts.iter().copied().chain([(total, total)]) {
        if lo - at >= 0.1 {
            keep.push((at, lo));
        }
        at = hi;
    }
    (keep, said, skipped)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_subject_bigger_than_the_crop_is_named() {
        let w = too_big_for_crop((0.5, 0.4), 0.31, 1.0).unwrap();
        assert!(w.contains("50 percent") && w.contains("31 percent"), "{w}");
        assert!(too_big_for_crop((0.2, 0.9), 0.31, 1.0).is_none());
        assert!(too_big_for_crop((0.9, 0.9), 1.0, 0.5)
            .unwrap()
            .contains("height"));
    }

    use super::*;

    #[test]
    fn bleep_entries_match_whole_words_in_a_row_in_both_scripts() {
        let w = |t: &str, s: f64| crate::captions::Word {
            text: t.into(),
            start: s,
            end: s + 0.4,
        };
        let heard = [
            w(" New", 0.0),
            w("York,", 0.5),
            w("-", 1.0),
            w("city", 1.1),
            w("york", 2.0),
            w("new", 2.5),
            w(
                "\u{0627}\u{0644}\u{0642}\u{064E}\u{0627}\u{0647}\u{0650}\u{0631}\u{0629}\u{060C}",
                3.0,
            ),
            w("Three", 4.0),
        ];
        // "القاهره" for a heard "القَاهِرة،": marks, the last letter's form and the comma do not count.
        let wanted = parse_bleep(
            "new YORK!, york city ,\u{0627}\u{0644}\u{0642}\u{0627}\u{0647}\u{0631}\u{0647}, 3, yor, paris",
        )
        .unwrap();
        let (runs, missing) = bleep_hits(&heard, &wanted);
        // "york city" steps over the lone dash; "york new" is not "new york"; "yor" is not a part of "york".
        assert_eq!(runs, [(0, 1, 0), (1, 3, 1), (6, 6, 2), (7, 7, 3)]);
        assert_eq!(missing, ["yor", "paris"]);
        assert!(parse_bleep(" , ").is_err());
        assert!(parse_bleep("ok, ?!").unwrap_err().contains("?!"));
        // Each match with its air (here 0.04 s), joined where two meet, and never outside the video.
        let m = |s: f64, e: f64| Bleeped {
            word: String::new(),
            start: s,
            end: e,
        };
        let got = bleep_windows(
            &[m(2.0, 2.4), m(0.0, 0.9), m(0.5, 1.5), m(4.0, 4.99)],
            5.0,
            (0.04, 0.04),
        );
        let near = |a: Span, b: Span| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
        assert!(
            got.len() == 3
                && near(got[0], (0.0, 1.54))
                && near(got[1], (1.96, 2.44))
                && near(got[2], (3.96, 5.0)),
            "{got:?}"
        );
    }

    #[test]
    fn a_bleeped_stretch_holds_nothing_of_the_source() {
        let frame = |v: i16| {
            let b = v.to_le_bytes();
            [b[0], b[1], b[0], b[1]]
        };
        let left = |f: &[u8; 4]| i16::from_le_bytes([f[0], f[1]]);
        // 0.1 s of a full-scale source. Silence: 10 ms down, exactly zero, 10 ms up, the same on both channels.
        let mut quiet = vec![frame(20000); 4800];
        bleep_window(&mut quiet, 0.0);
        assert!(quiet[480..4320].iter().all(|f| *f == [0; 4]));
        assert!(
            left(&quiet[0]) > 19000 && left(&quiet[240]) < 10100 && left(&quiet[4799]) == 20000
        );
        assert!(
            quiet
                .windows(2)
                .all(|p| (left(&p[0]) - left(&p[1])).abs() < 50),
            "a step clicks"
        );
        // Tone: between the fades only the 1 kHz sine at the asked peak (48 frames a cycle), whatever the source was.
        let mut tone = vec![frame(20000); 4800];
        let mut other = vec![frame(-7000); 4800];
        bleep_window(&mut tone, 0.1);
        bleep_window(&mut other, 0.1);
        assert_eq!(tone[480..4320], other[480..4320]);
        assert_eq!(left(&tone[480]), 0);
        assert_eq!(left(&tone[480 + 12]), 3277);
        assert_eq!(left(&tone[480 + 36]), -3277);
        // A stretch shorter than two fades still reaches zero and does not panic.
        let mut short = vec![frame(20000); 3];
        bleep_window(&mut short, 0.0);
        bleep_window(&mut [], 0.0);
        assert_eq!(left(&short[1]), 0);
    }

    #[test]
    fn captions_with_other_words_are_masked_by_text_then_by_time() {
        let w = |t: &str, s: f64, e: f64| crate::captions::Word {
            text: t.into(),
            start: s,
            end: e,
        };
        // The speech row heard "pineapple" at 4.0 to 4.4 s and at 6.5 to 6.9 s.
        let b = Bleep {
            wanted: parse_bleep("pineapple").unwrap(),
            heard: vec![],
            matches: [(4.0, 4.4), (6.5, 6.9)]
                .map(|(start, end)| Bleeped {
                    word: "pineapple".into(),
                    start,
                    end,
                })
                .into(),
            not_heard: vec![],
            windows: vec![],
            pad: BLEEP_PAD,
        };
        // A caption file with rough times: the word itself is masked, and "fresh" whose middle is in the heard time
        // stays, because the word was found by its text. At the second place the file has another word: the one on
        // the heard time is masked.
        let mut words = [
            w("fresh", 3.9, 4.3),
            w("PINEAPPLE", 4.3, 4.9),
            w("the", 6.2, 6.4),
            w("fruit", 6.4, 6.9),
            w("is", 6.9, 7.1),
        ];
        assert_eq!(mask_words(&mut words, &b), 2);
        let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["fresh", "*********", "the", "*****", "is"]);
    }

    #[test]
    fn a_mask_keeps_the_length_and_the_end_of_the_sentence() {
        assert_eq!(mask(" Pine-apple's?"), " **********?");
        assert_eq!(mask("3,000."), "****.");
        assert_eq!(mask("..."), "...");
        // "القَاهِرَةِ،": 7 letters; the marks go, the Arabic comma stays.
        assert_eq!(
            mask("\u{0627}\u{0644}\u{0642}\u{064E}\u{0627}\u{0647}\u{0650}\u{0631}\u{064E}\u{0629}\u{0650}\u{060C}"),
            "*******\u{060C}"
        );
    }

    #[test]
    fn fillers_snap_to_their_sound_and_go_with_their_pause() {
        let w = |t: &str, s: f64, e: f64| crate::captions::Word {
            text: t.into(),
            start: s,
            end: e,
        };
        // The 4 Nov talk: Whisper puts "um" in the pause (0.52-1.10); its sound is 1.06-1.33, then a pause to 1.71.
        let quiet = [(0.0, 0.17), (0.58, 1.06), (1.33, 1.71), (2.6, 3.0)];
        let words = [
            w("So,", 0.0, 0.5),
            w("um,", 0.52, 1.1),
            w("today", 1.28, 1.8),
            w("we", 1.8, 2.04),
        ];
        let (keep, cut, skipped) = filler_keep(&words, 3.0, 0.15, &quiet);
        assert_eq!((cut.len(), skipped), (1, 0));
        assert_eq!(
            (cut[0]["start"].as_f64(), cut[0]["end"].as_f64()),
            (Some(1.06), Some(1.33))
        );
        assert_eq!(keep, vec![(0.0, 0.73), (1.56, 3.0)]);
        // No short sound near it (one long island): the filler stays and is counted.
        let (keep, cut, skipped) = filler_keep(&words, 3.0, 0.15, &[]);
        assert_eq!((keep, cut.len(), skipped), (vec![(0.0, 3.0)], 0, 1));
        // Arabic: a drawn-out sound goes, "أم" (or) and "يعني" inside a sentence stay, "يعني" between pauses goes.
        assert!(
            hesitation("اااا") && hesitation("أممم،") && hesitation("ممم") && hesitation("Ummm...")
        );
        assert!(
            !hesitation("أم") && !hesitation("am") && !hesitation("hmmm-no") && !hesitation("the")
        );
        let ar = [
            w("هذا", 0.0, 0.3),
            w("يعني", 0.32, 0.6),
            w("مهم", 0.62, 0.9),
            w("يعني", 1.5, 1.8),
            w("نبدأ", 2.4, 2.8),
        ];
        let quiet = [(0.9, 1.5), (1.8, 2.4)];
        let (keep, cut, _) = filler_keep(&ar, 3.0, 0.15, &quiet);
        assert_eq!(cut.len(), 1);
        assert_eq!(cut[0]["start"], 1.5);
        assert_eq!(keep, vec![(0.0, 1.05), (2.25, 3.0)]);
        // The same "يعني" with only 0.12 s of pause before it stays.
        let (_, cut, skipped) = filler_keep(&ar, 3.0, 0.15, &[(1.38, 1.5), (1.8, 2.4)]);
        assert_eq!((cut.len(), skipped), (0, 2));
    }

    #[test]
    fn kept_parts_sit_on_the_frame_grid() {
        // 25 fps: 1.01 s snaps to 1.00, 2.03 to 2.04; the window starts and ends half a frame early.
        assert_eq!(
            select_expr(&[(1.01, 2.03)], 25.0),
            "gte(t,0.98000)*lt(t,2.02000)"
        );
        assert_eq!(
            select_expr(&[(0.0, 1.0), (2.0, 3.0)], 25.0)
                .matches('+')
                .count(),
            1
        );
    }

    #[test]
    fn a_retake_is_close_and_alike_and_the_last_take_stays() {
        let s = |t: &str, a, b| Sentence {
            text: t.into(),
            start: a,
            end: b,
        };
        let said = [
            s("Most cities lose a third.", 0.0, 1.2),
            s("Most cities lose a third of their water.", 1.7, 5.0),
            s("One liter in every four leaks away.", 7.0, 9.0),
            s(
                "One liter in every three leaks away underground.",
                10.0,
                13.0,
            ),
            s("Fix it.", 14.0, 14.5),
            s("Fix the pipes.", 15.0, 16.0),
            s("Fix the pipes.", 16.5, 17.5),
            s("Fix the pipes.", 18.0, 19.0),
            s("Fix the pipes.", 40.0, 41.0),
            s("Another subject entirely, with its own words.", 42.0, 45.0),
        ];
        let found = retakes(&said);
        let at: Vec<f64> = found.iter().map(|r| r.start).collect();
        assert_eq!(at, [0.0, 7.0, 15.0, 16.5], "no match past 10 s");
        assert!(
            found[0].differs.is_empty(),
            "a false start has no difference"
        );
        assert_eq!(found[1].differs, ["4", "3", "underground"]);
        // Arabic: marks and spelling do not count.
        assert_eq!(
            take_diff("نصلح الأنابيب أولا.", "نصلح الأنابيب أولاً، ثم نقيس ما بقي."),
            Some(vec![])
        );
        // Far from alike.
        assert_eq!(
            take_diff(
                "We fix roads in the north.",
                "Then we measure what is left."
            ),
            None
        );
    }

    #[test]
    fn removed_words_become_cuts_and_nothing_else_does() {
        let w = |t: &str, s, e| crate::captions::Word {
            text: t.into(),
            start: s,
            end: e,
        };
        let all = [
            w("So", 1.0, 1.3),
            w("this", 1.3, 1.6),
            w("is", 2.6, 2.8),
            w("wrong", 2.8, 3.2),
            w("right", 3.2, 3.6),
            w("now", 3.6, 4.0),
        ];
        // Nothing removed: the whole video, with its pauses.
        let (parts, gone) = word_parts(&all, &all, 6.0, 0.15).unwrap();
        assert_eq!(parts, [(0.0, 6.0)]);
        assert!(gone.is_empty());
        // "wrong" goes: the cut stays off its neighbours, and the long pause inside a part stays.
        let kept = [&all[..3], &all[4..]].concat();
        let (parts, gone) = word_parts(&kept[..], &kept, 6.0, 0.15).unwrap();
        assert_eq!(parts, [(0.0, 6.0)], "the list itself holds no removal");
        assert!(gone.is_empty());
        let (parts, gone) = word_parts(&all, &kept, 6.0, 0.15).unwrap();
        assert_eq!(gone, ["wrong"]);
        assert_eq!(parts, [(0.0, 2.8), (3.2, 6.0)]);
        // The first words go: the kept part starts with room, not at 0.
        let (parts, _) = word_parts(&all, &all[2..], 6.0, 0.15).unwrap();
        assert_eq!(parts, [(2.45, 6.0)]);
        // A term fix joined two words: both stay.
        let joined = [w("this is", 1.3, 2.8)];
        let (_, gone) = word_parts(&all, &joined, 6.0, 0.15).unwrap();
        assert_eq!(gone, ["So", "wrong", "right", "now"]);
        // Another video's words.
        assert!(word_parts(&all, &[w("x", 50.0, 51.0)], 6.0, 0.15).is_err());
        assert!(word_parts(&all, &[], 6.0, 0.15).is_err());
    }

    #[test]
    fn srt_cues_become_timed_words() {
        let srt = "\u{feff}1\r\n00:00:01,000 --> 00:00:02,000\r\n<i>ab</i> cd\r\nef\r\n\r\n2\r\n00:00:03.500 --> 00:00:04.500 X1:0\r\n{\\an8}مرحبا\r\n\r\nnot a cue\r\n";
        let w = parse_srt(srt);
        let text: Vec<&str> = w.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(text, ["ab", "cd", "ef", "مرحبا"]);
        assert!((w[1].start - (1.0 + 1.0 / 3.0)).abs() < 1e-6);
        assert!((w[2].end - 2.0).abs() < 1e-6);
        assert_eq!((w[3].start, w[3].end), (3.5, 4.5));
        assert!(parse_srt("nothing here").is_empty());
    }

    #[test]
    fn caption_bytes_decode_or_stop_with_a_cause() {
        let ar = "1\n00:00:00,000 --> 00:00:01,000\nمرحبا بكم\n";
        // Each byte of the table round-trips and the table is 128 characters.
        assert_eq!(CP1256.chars().count(), 128);
        assert_eq!(decode_srt(ar.as_bytes()).unwrap(), ar);
        let mut bom = b"\xef\xbb\xbf".to_vec();
        bom.extend(ar.as_bytes());
        assert_eq!(decode_srt(&bom).unwrap(), ar);
        let mut utf16 = b"\xff\xfe".to_vec();
        utf16.extend(ar.encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode_srt(&utf16).unwrap(), ar);
        // The same text in Windows-1256.
        let cp: Vec<u8> = ar
            .chars()
            .map(|c| {
                if c.is_ascii() {
                    c as u8
                } else {
                    0x80 + CP1256.chars().position(|x| x == c).unwrap() as u8
                }
            })
            .collect();
        assert_eq!(decode_srt(&cp).unwrap(), ar);
        let latin = b"1\n00:00:00,000 --> 00:00:01,000\nCaf\xe9 cr\xe8me\n";
        assert!(decode_srt(latin).unwrap_err().contains("Windows-1256"));
        assert!(decode_srt("a\u{fffd}b".as_bytes())
            .unwrap_err()
            .contains("U+FFFD"));
    }

    #[test]
    fn stills_parse_and_meet_silences() {
        let log = "[freezedetect @ 0x1] lavfi.freezedetect.freeze_start: 3.0\n[freezedetect @ 0x1] lavfi.freezedetect.freeze_duration: 4.0\n[freezedetect @ 0x1] lavfi.freezedetect.freeze_end: 7.0\n[freezedetect @ 0x1] lavfi.freezedetect.freeze_start: 9.0\n";
        let f = parse_freezes(log, 10.0);
        assert_eq!(f, vec![(3.0, 7.0), (9.0, 10.0)]);
        assert_eq!(
            overlap(&f, &[(2.0, 4.0), (6.5, 9.5)], 0.5),
            vec![(3.0, 4.0), (6.5, 7.0), (9.0, 9.5)]
        );
        assert!(overlap(&f, &[(4.0, 4.2)], 0.5).is_empty());
    }

    #[test]
    fn blur_parses_and_refuses() {
        let b = parse_blur(&["10,20,100,50@1.5-3".into()], (640, 360), 10.0).unwrap();
        assert_eq!(
            (b[0].x, b[0].y, b[0].w, b[0].h, b[0].from, b[0].to),
            (10, 20, 100, 50, 1.5, 3.0)
        );
        let f = blur_filter(&b[0], 0);
        assert!(
            f.contains("crop=100:50:10:20") && f.contains("between(t,1.500,3.000)"),
            "{f}"
        );
        assert!(f.contains("lr=24") && f.contains("cr=11"), "{f}");
        for bad in [
            "10,20,100@1-2",
            "600,20,100,50@1-2",
            "1,1,4,50@1-2",
            "1,1,50,50@3-2",
            "1,1,50,50@1-11",
            "x",
        ] {
            assert!(
                parse_blur(&[bad.into()], (640, 360), 10.0).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn broll_parses_and_refuses() {
        let b = parse_broll(&["a@b.mp4@1.5:2".into(), "c.png@4:1".into()], 10.0).unwrap();
        assert_eq!(b[0].file, PathBuf::from("a@b.mp4"));
        assert_eq!((b[0].at, b[0].seconds), (1.5, 2.0));
        for bad in [
            "x.mp4",
            "x.mp4@1",
            "@1:2",
            "x.mp4@1:0.2",
            "x.mp4@9:2",
            "x.mp4@-1:2",
        ] {
            assert!(parse_broll(&[bad.to_string()], 10.0).is_err(), "{bad}");
        }
        assert!(parse_broll(&["a.mp4@1:2".into(), "b.mp4@2:2".into()], 10.0).is_err());
        let (g, label) = broll_graph(&[], &b, 1, (1280, 720), 30.0);
        assert_eq!(label, "[over1]");
        assert!(g.starts_with("[0:v]null[base];[1:v]scale=1280:720"));
        assert!(g.contains("[over0][br1]overlay=enable='between(t,4.000,5.000)'"));
    }

    #[test]
    fn zooms_parse_and_refuse() {
        let z = parse_zooms(&["1-3@1.2".into(), "4-6.5@1.5,0.3,0.4".into()], 10.0).unwrap();
        assert_eq!(
            z[1],
            Zoom {
                start: 4.0,
                end: 6.5,
                z: 1.5,
                x: 0.3,
                y: 0.4
            }
        );
        for bad in [
            "1-3",
            "1-3@1",
            "1-3@3",
            "3-3.2@1.2",
            "1-3@1.2,2,0.5",
            "1-3@1.2,0.5",
            "9-12@1.2",
        ] {
            assert!(parse_zooms(&[bad.to_string()], 10.0).is_err(), "{bad}");
        }
        assert!(parse_zooms(&["1-3@1.2".into(), "2-4@1.2".into()], 10.0).is_err());
        let f = zoom_filter(&z, (1280, 720), 30.0);
        assert!(f.starts_with("scale=2560:1440,zoompan=z='1+"));
        assert!(f.ends_with(":d=1:s=1280x720:fps=30"));
    }

    #[test]
    fn close_emphasis_words_share_one_push() {
        let z = emphasis_zooms(&[(1.0, 1.2), (1.4, 1.8), (9.0, 9.1)]);
        assert_eq!(z.len(), 2);
        assert_eq!((z[0].start, z[0].end), (0.7, 2.3));
        assert_eq!((z[1].start, z[1].end), (8.7, 9.9));
    }

    #[test]
    fn clean_voice_has_three_levels() {
        for l in ["light", "medium", "strong"] {
            assert!(clean_filter(l).is_some_and(|f| f.starts_with("afftdn")));
        }
        assert_eq!(clean_filter("max"), None);
    }

    #[test]
    fn keep_parts_parse_in_order() {
        assert_eq!(
            parse_keep("12.3-30.5, 41-58.2").unwrap(),
            vec![(12.3, 30.5), (41.0, 58.2)]
        );
        assert!(parse_keep("10-20,15-30").unwrap_err().contains("before"));
        assert!(parse_keep("10-10.1").unwrap_err().contains("0.3"));
        assert!(parse_keep("ten-20").is_err());
        assert!(parse_keep("nan-5").is_err() && parse_keep("nan-nan").is_err());
        assert!(parse_keep("").is_err());
    }

    #[test]
    fn keeps_speech_with_air_and_merges_close_parts() {
        // speech 0-2, silence 2-3, speech 3-5, silence 5-5.2 (shorter than the pad overlap), speech 5.2-8, silence to end
        let s = [(2.0, 3.0), (5.0, 5.2), (8.0, 10.0)];
        let k = keep_segments(&s, 10.0, 0.15);
        assert_eq!(k, vec![(0.0, 2.15), (2.85, 8.15)]);
        assert!(keep_segments(&[(0.0, 10.0)], 10.0, 0.15).is_empty());
        assert_eq!(keep_segments(&[], 4.0, 0.15), vec![(0.0, 4.0)]);
    }

    #[test]
    fn parses_a_silence_that_runs_to_the_end() {
        let log = "[silencedetect] silence_start: 1.5\n[silencedetect] silence_end: 2.75 | silence_duration: 1.25\n[silencedetect] silence_start: 9.1\n";
        assert_eq!(parse_silences(log, 10.0), vec![(1.5, 2.75), (9.1, 10.0)]);
    }
}
