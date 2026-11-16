//! `scriptvox edit`, the small operations (docs/specs/edit-small-ops.md, G133): `--remove-bars`, `--hold`, `--loop`,
//! `--loop-to`, `--deinterlace` and `--tilt`. Local and free. They work alone or with each other, in a fixed order:
//! deinterlace, bars, tilt, hold, loop. Every other `edit` option needs its own call.

use crate::edit::{fail, Args};
use crate::edit_matte::Step;
use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The small operations of one `edit` call. All empty: this file does nothing.
#[derive(Default, Debug, Clone)]
pub struct Ops {
    /// Crop black bars that FFmpeg's `cropdetect` finds.
    pub remove_bars: bool,
    /// `AT@SECONDS`: hold the frame at AT (seconds of the source) for SECONDS more.
    pub hold: Vec<String>,
    /// Play the video this many times.
    pub loop_n: Option<u32>,
    /// Repeat the video up to this length in seconds.
    pub loop_to: Option<f64>,
    pub deinterlace: bool,
    /// Turn the picture by this many degrees, clockwise, and zoom so no corner is empty.
    pub tilt: Option<f64>,
}

impl Ops {
    pub fn any(&self) -> bool {
        self.remove_bars
            || !self.hold.is_empty()
            || self.loop_n.is_some()
            || self.loop_to.is_some()
            || self.deinterlace
            || self.tilt.is_some()
    }
}

pub const MAX_HOLDS: usize = 16;
pub const MAX_HOLD_S: f64 = 60.0;
pub const MAX_TILT: f64 = 15.0;
pub const MAX_LOOPS: u32 = 100;
/// The longest result of a loop, in seconds.
pub const MAX_LOOP_S: f64 = 600.0;
const HOLD_FIX: &str =
    "Give --hold AT@SECONDS, for example --hold 4.0@2: the frame at 4.0 s stays for 2 s more. At most 16 holds of 60 s each.";

fn done(exit: Exit, msg: impl Into<String>, fix: &str) -> Step {
    Step::Done(Box::new(fail(exit, msg, fix)))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hold {
    pub at: f64,
    pub seconds: f64,
}

/// `--hold` values, in time order. `total` is the length of the source.
pub fn parse_holds(given: &[String], total: f64) -> Result<Vec<Hold>, String> {
    if given.len() > MAX_HOLDS {
        return Err(format!(
            "--hold takes at most {MAX_HOLDS} holds, not {}",
            given.len()
        ));
    }
    let mut out = Vec::new();
    for g in given {
        let bad = || format!("--hold {g}: not AT@SECONDS");
        let (at, s) = g.split_once('@').ok_or_else(bad)?;
        let at: f64 = at.trim().parse().map_err(|_| bad())?;
        let seconds: f64 = s.trim().parse().map_err(|_| bad())?;
        if !(at.is_finite() && (0.0..=total).contains(&at)) {
            return Err(format!(
                "--hold {g}: the time is not inside the video (0 to {total:.2} s)"
            ));
        }
        if !(seconds.is_finite() && seconds > 0.0 && seconds <= MAX_HOLD_S) {
            return Err(format!(
                "--hold {g}: the length is above 0 and at most {MAX_HOLD_S:.0} s"
            ));
        }
        out.push(Hold { at, seconds });
    }
    out.sort_by(|a, b| a.at.total_cmp(&b.at));
    if let Some(w) = out.windows(2).find(|w| w[1].at - w[0].at < 0.1) {
        return Err(format!(
            "two holds are at the same place ({:.2} and {:.2} s)",
            w[0].at, w[1].at
        ));
    }
    Ok(out)
}

/// The last `crop=w:h:x:y` of a `cropdetect` log.
pub fn parse_crop(log: &str) -> Option<(u32, u32, u32, u32)> {
    let line = log.lines().rev().find_map(|l| l.split("crop=").nth(1))?;
    let mut n = line
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u32>().ok());
    Some((n.next()??, n.next()??, n.next()??, n.next()??))
}

/// The counts of FFmpeg's `idet` filter, from its "Multi frame detection" line: (interlaced, progressive).
pub fn parse_idet(log: &str) -> Option<(u64, u64)> {
    let line = log
        .lines()
        .rev()
        .find(|l| l.contains("Multi frame detection"))?;
    let after = |key: &str| -> Option<u64> {
        line.split(key)
            .nth(1)?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    };
    Some((after("TFF:")? + after("BFF:")?, after("Progressive:")?))
}

/// How much the picture grows before it is turned, so that the frame has no empty corner.
pub fn tilt_zoom(deg: f64, (w, h): (u32, u32)) -> f64 {
    let a = deg.abs().to_radians();
    let shape = (f64::from(w) / f64::from(h)).max(f64::from(h) / f64::from(w));
    a.cos() + a.sin() * shape
}

/// The filter graph for the picture and the sound of pass 1. `[0:v]` to `[v]`, and `[0:a]` to `[a]` with `sound`.
/// `crop` is the box of `--remove-bars`; `size` is the frame after it; then the frame rate and the length of the source.
pub fn graph(
    deinterlace: bool,
    crop: Option<(u32, u32, u32, u32)>,
    tilt: Option<f64>,
    holds: &[Hold],
    size: (u32, u32),
    (fps, total): (f64, f64),
    sound: bool,
) -> String {
    let mut v: Vec<String> = Vec::new();
    if deinterlace {
        // One frame out for each frame in: the length and the frame rate stay.
        v.push("bwdif=mode=send_frame:parity=auto:deint=all".into());
    }
    if let Some((w, h, x, y)) = crop {
        v.push(format!("crop={w}:{h}:{x}:{y}"));
    }
    if let Some(deg) = tilt {
        let k = tilt_zoom(deg, size);
        let up = |x: u32| ((f64::from(x) * k / 2.0).ceil() as u32) * 2;
        v.push(format!(
            "scale={}:{}:flags=lanczos,rotate={:.6}:ow={}:oh={}:bilinear=1",
            up(size.0),
            up(size.1),
            deg.to_radians(),
            size.0,
            size.1
        ));
    }
    v.push("setsar=1".into());
    if holds.is_empty() {
        let mut g = format!("[0:v]{}[v]", v.join(","));
        if sound {
            g.push_str(";[0:a]anull[a]");
        }
        return g;
    }
    // A hold is on a frame boundary, so the picture and the sound are cut at the same place.
    let snap = |t: f64| ((t * fps).round().max(1.0) / fps).min(total);
    let mut cuts: Vec<(f64, f64, f64)> = Vec::new();
    let mut from = 0.0;
    for h in holds {
        let at = snap(h.at);
        cuts.push((from, at, h.seconds));
        from = at;
    }
    // The rest of the video after the last hold, when there is one frame of it or more.
    if total - from > 0.5 / fps {
        cuts.push((from, total, 0.0));
    }
    let n = cuts.len();
    v.insert(0, format!("fps={fps}"));
    let names = |p: &str| (0..n).map(|i| format!("[{p}{i}]")).collect::<String>();
    let mut g = format!("[0:v]{},split={n}{}", v.join(","), names("vi"));
    for (i, (a, b, hold)) in cuts.iter().enumerate() {
        let pad = if *hold > 0.0 {
            format!(",tpad=stop_mode=clone:stop_duration={hold:.3}")
        } else {
            String::new()
        };
        g.push_str(&format!(
            ";[vi{i}]trim=start={a:.4}:end={b:.4},setpts=PTS-STARTPTS{pad}[vo{i}]"
        ));
    }
    g.push_str(&format!(";{}concat=n={n}:v=1:a=0[v]", names("vo")));
    if sound {
        g.push_str(&format!(";[0:a]asplit={n}{}", names("ai")));
        for (i, (a, b, hold)) in cuts.iter().enumerate() {
            let pad = if *hold > 0.0 {
                format!(",apad=pad_dur={hold:.3}")
            } else {
                String::new()
            };
            g.push_str(&format!(
                ";[ai{i}]atrim=start={a:.4}:end={b:.4},asetpts=PTS-STARTPTS{pad}[ao{i}]"
            ));
        }
        g.push_str(&format!(";{}concat=n={n}:v=0:a=1[a]", names("ao")));
    }
    g
}

/// How many times the input plays for `--loop` or `--loop-to`, and the length of the result.
pub fn loop_plan(n: Option<u32>, to: Option<f64>, total: f64) -> Result<(u32, f64), String> {
    let (times, secs) = match (n, to) {
        (Some(_), Some(_)) => {
            return Err("--loop and --loop-to are two ways to say how long".into())
        }
        (Some(n), None) if (2..=MAX_LOOPS).contains(&n) => (n, total * f64::from(n)),
        (Some(n), None) => return Err(format!("--loop takes 2 to {MAX_LOOPS} times, not {n}")),
        (None, Some(t)) if t.is_finite() && t > total => ((t / total).ceil() as u32, t),
        (None, Some(t)) => {
            return Err(format!(
                "--loop-to {t} is not longer than the video ({total:.2} s)"
            ))
        }
        (None, None) => return Ok((1, total)),
    };
    if secs > MAX_LOOP_S || times > MAX_LOOPS {
        return Err(format!(
            "the result would run {secs:.0} s in {times} plays: the limit is {MAX_LOOP_S:.0} s and {MAX_LOOPS} plays"
        ));
    }
    Ok((times, secs))
}

/// FFmpeg's log for a measuring pass (no output file).
fn log(ff: &Ffmpeg, args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let args = crate::media_gate::guard(Path::new(""), &args)?;
    Command::new(&ff.ffmpeg)
        .args(["-hide_banner", "-nostdin"])
        .args(&args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stderr).into_owned())
        .map_err(|e| format!("cannot start FFmpeg: {e}"))
}

fn idet(ff: &Ffmpeg, file: &Path) -> Option<(u64, u64)> {
    let f = file.display().to_string();
    let l = log(
        ff,
        &[
            "-i",
            &f,
            "-an",
            "-vf",
            "idet",
            "-frames:v",
            "300",
            "-f",
            "null",
            "-",
        ],
    )
    .ok()?;
    parse_idet(&l)
}

/// The stream's own flag: true when it names a field order (`tt`, `bb`, `tb`, `bt`).
fn field_order(ff: &Ffmpeg, file: &Path) -> bool {
    let Ok(gate) = crate::media_gate::input(file) else {
        return false;
    };
    Command::new(&ff.ffprobe)
        .args(["-v", "error", "-select_streams", "v:0"])
        .args(["-show_entries", "stream=field_order", "-of", "csv=p=0"])
        .args(gate)
        .output()
        .is_ok_and(|o| {
            matches!(
                String::from_utf8_lossy(&o.stdout).trim(),
                "tt" | "bb" | "tb" | "bt"
            )
        })
}

/// Runs the small operations, or passes on when none is asked.
pub fn step(a: &Args) -> Step {
    let o = &a.small;
    if !o.any() {
        return Step::Next(None);
    }
    if crate::edit_matte::other_options(a) {
        return done(
            Exit::Usage,
            "--remove-bars, --hold, --loop, --loop-to, --deinterlace and --tilt work with each other only: use two calls for another edit option",
            "Run these first, then `scriptvox edit <result>` with the other options. Order in one call: deinterlace, bars, tilt, hold, loop.",
        );
    }
    if !a.input.is_file() {
        return done(
            Exit::Usage,
            format!("{} does not exist", a.input.display()),
            "Give a video file.",
        );
    }
    if let Some(t) = o.tilt {
        if !(t.is_finite() && t != 0.0 && t.abs() <= MAX_TILT) {
            return done(
                Exit::Usage,
                format!("--tilt takes -{MAX_TILT} to {MAX_TILT} degrees and not 0, not {t}"),
                "Give for example --tilt -2.5 (a minus turns the picture to the left).",
            );
        }
    }
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return Step::Done(Box::new(crate::engine_ffmpeg::locate_failure("edit", e))),
    };
    let (Ok(total), Some((w, h)), Ok(rb)) = (
        ff.duration(&a.input),
        ff.shown_size(&a.input),
        ff.readback(&a.input),
    ) else {
        return done(
            Exit::Usage,
            format!("{} has no picture FFmpeg can read", a.input.display()),
            "Give a video file.",
        );
    };
    let fps = rb.fps.clamp(1.0, 120.0);
    let holds = match parse_holds(&o.hold, total) {
        Ok(h) => h,
        Err(e) => return done(Exit::Usage, e, HOLD_FIX),
    };
    let held: f64 = holds.iter().map(|h| h.seconds).sum();
    let (times, final_s) = match loop_plan(o.loop_n, o.loop_to, total + held) {
        Ok(p) => p,
        Err(e) => {
            return done(
                Exit::Usage,
                e,
                "Give --loop 3 (three plays), or --loop-to 30 (repeat up to 30 s; the last play is cut there).",
            )
        }
    };
    let input = a.input.display().to_string();
    let mut info = serde_json::Map::new();
    if o.deinterlace {
        let Some((inter, prog)) = idet(&ff, &a.input) else {
            return done(
                Exit::Error,
                "FFmpeg could not measure the fields of the video",
                "Check the input video plays.",
            );
        };
        // The stream's own flag decides first. `idet` alone is not safe: it called 80 of 120 frames of a progressive
        // test picture with fine lines interlaced (13 Nov 2026), so with no flag it must be 90 percent sure.
        let flagged = field_order(&ff, &a.input);
        // A flat picture gives no count at all (0 and 0): that is not a yes.
        if !flagged && (inter == 0 || inter * 10 < (inter + prog) * 9) {
            return done(
                Exit::Usage,
                format!("the video is not interlaced: its stream says progressive, and {inter} of {} measured frames look interlaced (90 percent are needed)", inter + prog),
                "Drop --deinterlace: it would only soften this picture.",
            );
        }
        info.insert(
            "deinterlace".into(),
            json!({"filter": "bwdif", "stream_says_interlaced": flagged, "before": {"interlaced": inter, "progressive": prog}}),
        );
    }
    let mut size = (w & !1, h & !1);
    let mut crop = None;
    if o.remove_bars {
        // Two frames a second of the first two minutes, after the deinterlace when there is one.
        let chain = format!(
            "{}fps=2,cropdetect=limit=24:round=2:skip=0:reset=0",
            if o.deinterlace {
                "bwdif=mode=send_frame,"
            } else {
                ""
            }
        );
        let found = log(
            &ff,
            &[
                "-t", "120", "-i", &input, "-an", "-vf", &chain, "-f", "null", "-",
            ],
        )
        .ok()
        .and_then(|l| parse_crop(&l));
        let Some((cw, ch, cx, cy)) = found else {
            return done(
                Exit::Error,
                "FFmpeg could not measure the black bars",
                "Check the input video plays.",
            );
        };
        if w.saturating_sub(cw) < 8 && h.saturating_sub(ch) < 8 {
            return done(
                Exit::Usage,
                format!("the video has no black bars: the picture fills {cw}x{ch} of {w}x{h}"),
                "Drop --remove-bars. Nothing was written.",
            );
        }
        if cw * 3 < w || ch * 3 < h || cw < 16 || ch < 16 {
            return done(
                Exit::Usage,
                format!("the picture found is only {cw}x{ch} of {w}x{h}: the video is too dark to find its bars"),
                "Crop by hand with --reframe, or give a brighter part of the video (--from/--to in an earlier call).",
            );
        }
        crop = Some((cw, ch, cx, cy));
        size = (cw, ch);
        info.insert(
            "bars".into(),
            json!({"crop": format!("{cw}:{ch}:{cx}:{cy}"), "size_in": format!("{w}x{h}"), "size_out": format!("{cw}x{ch}"),
                "removed_px": {"left": cx, "right": w.saturating_sub(cw + cx), "top": cy, "bottom": h.saturating_sub(ch + cy)}}),
        );
    }
    if let Some(t) = o.tilt {
        info.insert(
            "tilt".into(),
            json!({"degrees": t, "zoom": (tilt_zoom(t, size) * 1000.0).round() / 1000.0}),
        );
    }
    if !holds.is_empty() {
        info.insert(
            "hold".into(),
            json!(holds
                .iter()
                .map(|h| json!({"at": h.at, "seconds": h.seconds}))
                .collect::<Vec<_>>()),
        );
    }
    let stem = a
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video")
        .to_string();
    let out = a
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("scriptvox-out").join(format!("{stem}-edit.mp4")));
    if let Some(p) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(p);
    }
    let enc = match ff.pick_encoder() {
        Ok((e, _)) => e.args(),
        Err(e) => {
            return done(
                Exit::Error,
                e,
                "Install an FFmpeg build with libx264 or libopenh264.",
            )
        }
    };
    let sound = ff.has_stream(&a.input, "audio");
    let encode = |pre: &[String], maps: &[&str], to: &Path| -> Result<(), String> {
        let mut args = pre.to_vec();
        args.extend(maps.iter().map(|s| s.to_string()));
        args.extend(enc.clone());
        if sound {
            args.extend(["-c:a", "aac", "-b:a", "192k"].map(String::from));
        }
        args.extend(["-movflags".into(), "+faststart".into()]);
        args.push(to.display().to_string());
        ff.run(Path::new("."), &args, None)
    };
    let picture = o.deinterlace || crop.is_some() || o.tilt.is_some() || !holds.is_empty();
    // ponytail: a picture change with a loop is two encodes (the change, then the loop of its result). One graph
    // would need every hold again in each play; do that when the second encode costs too much.
    let work = PathBuf::from(".scriptvox/edit").join(&stem);
    let first = if times > 1 && picture {
        if let Err(e) = std::fs::create_dir_all(&work) {
            return done(Exit::Error, e.to_string(), "Check the folder is writable.");
        }
        work.join("small-pass1.mp4")
    } else {
        out.clone()
    };
    if picture {
        let g = graph(
            o.deinterlace,
            crop,
            o.tilt,
            &holds,
            size,
            (fps, total),
            sound,
        );
        let pre = vec!["-i".into(), input.clone(), "-filter_complex".into(), g];
        let maps: &[&str] = if sound {
            &["-map", "[v]", "-map", "[a]"]
        } else {
            &["-map", "[v]"]
        };
        if let Err(e) = encode(&pre, maps, &first) {
            let _ = std::fs::remove_file(&first);
            return done(Exit::Error, e, "Check the input video plays.");
        }
    }
    if times > 1 {
        let src = if picture {
            first.clone()
        } else {
            a.input.clone()
        };
        let mut pre = vec![
            "-stream_loop".into(),
            (times - 1).to_string(),
            "-i".into(),
            src.display().to_string(),
        ];
        // `--loop-to` cuts the last play. `--loop` keeps every frame of every play.
        if o.loop_to.is_some() {
            pre.extend(["-t".into(), format!("{final_s:.3}")]);
        }
        let maps: &[&str] = if sound {
            &["-map", "0:v:0", "-map", "0:a:0"]
        } else {
            &["-map", "0:v:0"]
        };
        let done_loop = encode(&pre, maps, &out);
        if picture {
            let _ = std::fs::remove_file(&first);
        }
        if let Err(e) = done_loop {
            let _ = std::fs::remove_file(&out);
            return done(Exit::Error, e, "Check the input video plays.");
        }
        info.insert(
            "loop".into(),
            json!({"plays": times, "seconds": (final_s * 100.0).round() / 100.0}),
        );
    }
    if o.deinterlace {
        if let (Some((inter, prog)), Some(d)) = (idet(&ff, &out), info.get_mut("deinterlace")) {
            d["after"] = json!({"interlaced": inter, "progressive": prog});
        }
    }
    Step::Done(Box::new(result(&ff, &out, total, final_s, info)))
}

/// The answer: the file, its measured length and size, and the `av_drift` gate.
fn result(
    ff: &Ffmpeg,
    out: &Path,
    before: f64,
    want: f64,
    info: serde_json::Map<String, Value>,
) -> Envelope {
    if !ff.has_stream(out, "video") {
        return fail(
            Exit::Error,
            format!("{} came out with no video stream", out.display()),
            "Give another --out, and check that the input plays.",
        );
    }
    let mut env = Envelope::ok("edit");
    match crate::gates::check(ff, out) {
        Ok(found) => {
            env.findings = found
                .iter()
                .filter(|f| f.gate == "av_drift")
                .map(|f| json!(f))
                .collect();
        }
        Err(e) => {
            return fail(
                Exit::GateFailed,
                format!("gate failed: {e}"),
                "The video stays for inspection. Run the edit again.",
            )
        }
    }
    let got = ff.duration(out).unwrap_or(want);
    if (got - want).abs() > 0.5 {
        env.findings.push(json!({"gate": "length", "status": "warn",
            "detail": format!("the file runs {got:.2} s, and {want:.2} s was expected"),
            "fix": "Look at the video. Its picture or sound may be shorter than the other."}));
    }
    let want_gates = [
        (
            "black",
            "`edit` runs only av_drift; run `scriptvox check` on the result",
        ),
        (
            "picture",
            "no gate judges the crop, the turn or the fields: look at the frames (`scriptvox snapshot <file> --sheet 4`)",
        ),
    ];
    let not_checked = crate::gates::not_checked(&env.findings, &want_gates);
    env = env.with("not_checked", json!(not_checked));
    let (coverage, limitations) = crate::gates::limits(&env.findings);
    env = env
        .with("coverage", json!(coverage))
        .with("limitations", json!(limitations));
    env.artifacts.insert(
        "video".into(),
        json!(std::path::absolute(out).unwrap_or(out.to_path_buf())),
    );
    let r = |x: f64| (x * 100.0).round() / 100.0;
    env.metrics
        .insert("seconds_before".into(), json!(r(before)));
    env.metrics.insert("seconds_after".into(), json!(r(got)));
    env.metrics.insert("cuts".into(), json!(0));
    if let Some((w, h)) = ff.shown_size(out) {
        env.metrics.insert("size".into(), json!(format!("{w}x{h}")));
    }
    env.metrics.insert("cost_usd".into(), json!(0.0));
    for (k, v) in info {
        env = env.with(&k, v);
    }
    crate::edit::fail_on_gate(env, |g| g == "av_drift")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn holds_are_parsed_sorted_and_checked() {
        let h = parse_holds(&s(&["4@2", "1.5@0.5"]), 10.0).unwrap();
        assert_eq!(
            h[0],
            Hold {
                at: 1.5,
                seconds: 0.5
            }
        );
        assert_eq!(
            h[1],
            Hold {
                at: 4.0,
                seconds: 2.0
            }
        );
        for (bad, word) in [
            ("4", "AT@SECONDS"),
            ("x@2", "AT@SECONDS"),
            ("11@2", "inside the video"),
            ("4@0", "above 0"),
            ("4@61", "at most 60"),
        ] {
            let e = parse_holds(&s(&[bad]), 10.0).unwrap_err();
            assert!(e.contains(word), "{bad}: {e}");
        }
        assert!(parse_holds(&s(&["4@1", "4.05@1"]), 10.0)
            .unwrap_err()
            .contains("same place"));
        assert!(parse_holds(&vec!["1@1".to_string(); 17], 10.0).is_err());
    }

    #[test]
    fn the_logs_of_cropdetect_and_idet_are_read() {
        let log = "[Parsed_cropdetect_1 @ 0x1] x1:0 x2:1279 y1:90 y2:629 w:1280 h:540 x:0 y:90 pts:1 t:0.5 limit:0.094118 crop=1280:544:0:88\n\
                   [Parsed_cropdetect_1 @ 0x1] x1:0 x2:1279 y1:90 y2:629 w:1280 h:540 x:0 y:90 pts:2 t:1.0 limit:0.094118 crop=1280:540:0:90\nframe=6\n";
        assert_eq!(parse_crop(log), Some((1280, 540, 0, 90)));
        assert_eq!(parse_crop("no line"), None);
        let idet = "[Parsed_idet_0 @ 0x1] Single frame detection: TFF:   140 BFF:     0 Progressive:     2 Undetermined:     8\n\
                    [Parsed_idet_0 @ 0x1] Multi frame detection: TFF:   148 BFF:     1 Progressive:     0 Undetermined:     1\n";
        assert_eq!(parse_idet(idet), Some((149, 0)));
        assert_eq!(parse_idet(""), None);
    }

    #[test]
    fn a_turn_zooms_enough_to_fill_the_frame() {
        // 0 degrees: no zoom. A wide frame needs more than a square one.
        assert!((tilt_zoom(0.0, (1920, 1080)) - 1.0).abs() < 1e-9);
        let wide = tilt_zoom(5.0, (1920, 1080));
        let square = tilt_zoom(5.0, (1080, 1080));
        assert!((wide - 1.1511).abs() < 1e-3, "{wide}");
        assert!(square < wide && square > 1.0);
        assert_eq!(tilt_zoom(-5.0, (1920, 1080)), wide);
        assert_eq!(tilt_zoom(5.0, (1080, 1920)), wide);
    }

    #[test]
    fn the_graph_holds_on_a_frame_boundary_and_pads_the_sound_the_same() {
        let holds = [Hold {
            at: 2.01,
            seconds: 1.5,
        }];
        let g = graph(false, None, None, &holds, (1280, 720), (25.0, 6.0), true);
        // 2.01 s at 25 fps is frame 50: 2.0 s.
        assert!(
            g.starts_with("[0:v]fps=25,setsar=1,split=2[vi0][vi1]"),
            "{g}"
        );
        assert!(g.contains("[vi0]trim=start=0.0000:end=2.0000,setpts=PTS-STARTPTS,tpad=stop_mode=clone:stop_duration=1.500[vo0]"), "{g}");
        assert!(
            g.contains("[vi1]trim=start=2.0000:end=6.0000,setpts=PTS-STARTPTS[vo1]"),
            "{g}"
        );
        assert!(
            g.contains(
                "[ai0]atrim=start=0.0000:end=2.0000,asetpts=PTS-STARTPTS,apad=pad_dur=1.500[ao0]"
            ),
            "{g}"
        );
        assert!(g.ends_with("[ao0][ao1]concat=n=2:v=0:a=1[a]"), "{g}");
        // A hold at the end has no part after it; with no sound there is no sound graph.
        let end = graph(
            false,
            None,
            None,
            &[Hold {
                at: 6.0,
                seconds: 1.0,
            }],
            (1280, 720),
            (25.0, 6.0),
            false,
        );
        assert!(
            end.contains("split=1[vi0]") && !end.contains("[0:a]"),
            "{end}"
        );
        // The fixed order: fields, bars, turn.
        let all = graph(
            true,
            Some((1280, 540, 0, 90)),
            Some(-2.0),
            &[],
            (1280, 540),
            (25.0, 6.0),
            true,
        );
        let at = |w: &str| all.find(w).unwrap_or_else(|| panic!("{w} in {all}"));
        assert!(
            at("bwdif") < at("crop=1280:540:0:90")
                && at("crop=") < at("rotate=-0.034907:ow=1280:oh=540")
        );
        assert!(all.ends_with(";[0:a]anull[a]"), "{all}");
    }

    #[test]
    fn a_loop_is_a_number_of_plays_or_a_length() {
        assert_eq!(loop_plan(None, None, 4.0), Ok((1, 4.0)));
        assert_eq!(loop_plan(Some(3), None, 4.0), Ok((3, 12.0)));
        assert_eq!(loop_plan(None, Some(10.0), 4.0), Ok((3, 10.0)));
        for (n, to, word) in [
            (Some(1), None, "2 to 100"),
            (Some(2), Some(9.0), "two ways"),
            (None, Some(3.0), "not longer"),
            (Some(100), None, "limit"),
            (None, Some(601.0), "limit"),
        ] {
            let e = loop_plan(n, to, 8.0).unwrap_err();
            assert!(e.contains(word), "{e}");
        }
    }
}
