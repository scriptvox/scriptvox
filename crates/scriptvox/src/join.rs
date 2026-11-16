//! `scriptvox join a.mp4 b.mp4 ...`: clips of any size and rate into one video, with a transition, the sound matched to
//! the picture and mastered. Idea: OpenMontage's footage stitch; our own code, FFmpeg filters only.

use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::json;
use std::path::{Path, PathBuf};

pub struct Args {
    pub inputs: Vec<PathBuf>,
    pub transition: Option<String>,
    pub aspect: Option<String>,
    pub fit: String,
    pub out: Option<PathBuf>,
    pub use_system: bool,
}

fn fail(exit: Exit, msg: impl Into<String>, fix: &str) -> Envelope {
    Envelope::fail("join", exit, msg.into(), Some(fix.into()))
}

/// One clip: its length, and whether it has sound.
pub struct Clip {
    pub secs: f64,
    pub sound: bool,
}

/// The filter graph: `[v]` and `[a]`, each clip fitted to the frame and locked to its length. A clip with no sound gets
/// silence, so the sound stays in step. Each transition overlaps the end of a clip and the start of the next.
pub fn graph(
    clips: &[Clip],
    (w, h): (u32, u32),
    fps: u32,
    fit: &str,
    (xf, xd): (&str, f64),
) -> String {
    let mut g = Vec::new();
    for (i, c) in clips.iter().enumerate() {
        let d = c.secs;
        let shape = if fit == "blur" {
            crate::media::fit_blur(w, h, i)
        } else if fit == "pad" {
            format!("scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2")
        } else {
            format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}")
        };
        let is_zoom = xf == "zoom";
        let zoom = crate::media::zoom_step(
            (w, h),
            fps,
            d,
            (is_zoom && i > 0).then_some(xd),
            (is_zoom && i + 1 < clips.len()).then_some(xd),
        )
        .map_or(String::new(), |z| format!(",{z},format=yuv420p"));
        g.push(format!(
            "[{i}:v]{shape},setsar=1,fps={fps},format=yuv420p,tpad=stop_mode=clone:stop_duration=1,trim=duration={d:.3},setpts=PTS-STARTPTS{zoom}[v{i}]"
        ));
        let sound = if c.sound {
            // `async=1:first_pts=0`: sound that starts after time 0 gets silence in front. Without it the sound of a
            // clip from `edit` came out short and the join failed gate av_drift (2 clips: video 24.000 s, sound 23.837 s).
            format!(
                "[{i}:a]aresample=48000:async=1:first_pts=0,aformat=channel_layouts=stereo,apad,"
            )
        } else {
            "anullsrc=r=48000:cl=stereo,".to_string()
        };
        g.push(format!(
            "{sound}atrim=duration={d:.3},asetpts=PTS-STARTPTS[a{i}]"
        ));
    }
    let (mut pv, mut pa) = ("v0".to_string(), "a0".to_string());
    let mut at = 0.0;
    for (i, c) in clips.iter().enumerate().skip(1) {
        at += clips[i - 1].secs - xd;
        let _ = c;
        let (ov, oa) = (format!("xv{i}"), format!("xa{i}"));
        g.push(format!(
            "[{pv}][v{i}]xfade=transition={}:duration={xd:.3}:offset={at:.3}[{ov}]",
            crate::media::xfade_name(xf)
        ));
        g.push(format!(
            "[{pa}][a{i}]acrossfade=d={xd:.3}:c1=tri:c2=tri[{oa}]"
        ));
        (pv, pa) = (ov, oa);
    }
    g.push(format!("[{pv}]null[v];[{pa}]anull[a]"));
    g.join(";")
}

pub fn run(mut a: Args) -> Envelope {
    // Before the HDR step, which swaps an input for a copy.
    let own = a.out.as_deref().and_then(|o| {
        a.inputs
            .iter()
            .find(|i| crate::engine_ffmpeg::same_file(o, i))
    });
    if let Some(i) = own {
        return fail(
            Exit::Usage,
            format!("--out is an input file: {}", i.display()),
            "Give another --out: a join cannot write over a file it reads.",
        );
    }
    if let Some(env) = crate::media_gate::refuse("join", &a.inputs) {
        return env;
    }
    let mut hdr = Vec::new();
    for p in &mut a.inputs {
        match crate::engine_ffmpeg::sdr_input(p, a.use_system) {
            Ok(h) => hdr.extend(h),
            Err(e) => {
                return fail(
                    Exit::Error,
                    e,
                    "Check the file, and that the disk has room.",
                )
            }
        }
    }
    let mut env = run_sdr(a);
    env.findings.extend(hdr);
    env
}

fn run_sdr(a: Args) -> Envelope {
    if a.inputs.len() < 2 {
        return fail(
            Exit::Usage,
            "join needs at least 2 clips",
            "Give the clips in order: scriptvox join a.mp4 b.mp4.",
        );
    }
    if let Some(p) = a.inputs.iter().find(|p| !p.is_file()) {
        return fail(
            Exit::Usage,
            format!("{} does not exist", p.display()),
            "Give video files.",
        );
    }
    if !matches!(a.fit.as_str(), "pad" | "crop" | "blur") {
        return fail(
            Exit::Usage,
            format!("unknown --fit {}", a.fit),
            "Use --fit pad (bars, keeps all of the picture), blur (a blurred copy in place of bars) or crop.",
        );
    }
    let frame = match a.aspect.as_deref().map(crate::make::frame) {
        Some(None) => {
            return fail(
                Exit::Usage,
                "unknown --aspect",
                "Use --aspect 9:16, 16:9, 1:1, 4:5, 3:4, 4:3, 2:3 or 21:9.",
            )
        }
        Some(f) => f,
        None => None,
    };
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return crate::engine_ffmpeg::locate_failure("join", e),
    };
    let mut clips = Vec::new();
    for p in &a.inputs {
        let Ok(secs) = ff.duration(p) else {
            return fail(
                Exit::Usage,
                format!("{} is not a video FFmpeg can read", p.display()),
                "Give video files.",
            );
        };
        if !ff.has_stream(p, "video") {
            return fail(
                Exit::Usage,
                format!("{} has no picture", p.display()),
                "Give video files.",
            );
        }
        clips.push(Clip {
            secs,
            sound: ff.has_stream(p, "audio"),
        });
    }
    // The first clip sets the frame (even sides) and the rate, unless --aspect sets the frame.
    let (w, h) = frame.unwrap_or_else(|| {
        ff.shown_size(&a.inputs[0])
            .map_or((1080, 1920), |(w, h)| (w / 2 * 2, h / 2 * 2))
    });
    let real = ff.frame_rate(&a.inputs[0]).unwrap_or(30.0);
    let fps = crate::like::fps(real);
    let tr = match crate::media::transition(a.transition.as_deref(), false, fps) {
        Ok(t) => t,
        Err(e) => {
            return fail(
                Exit::Usage,
                e,
                "See `scriptvox schema brief` for the names.",
            )
        }
    };
    if let Some((p, c)) = a
        .inputs
        .iter()
        .zip(&clips)
        .find(|(_, c)| c.secs <= tr.1 * 2.0)
    {
        return fail(
            Exit::Usage,
            format!(
                "{} runs {:.2} s, too short for a {:.2} s transition",
                p.display(),
                c.secs,
                tr.1
            ),
            "Use --transition cut, or drop the clip.",
        );
    }
    // Named after the first clip, and never over an earlier join.
    let out = a.out.clone().unwrap_or_else(|| {
        let stem = a.inputs[0]
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("video");
        (0..)
            .map(|n| match n {
                0 => format!("{stem}-join.mp4"),
                n => format!("{stem}-join-{n}.mp4"),
            })
            .map(|name| PathBuf::from("scriptvox-out").join(name))
            .find(|p| !p.exists())
            .unwrap()
    });
    if let Some(d) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(d);
    }
    let abs = |p: &Path| {
        std::path::absolute(p)
            .unwrap_or(p.to_path_buf())
            .display()
            .to_string()
    };
    let (enc, _) = match ff.pick_encoder() {
        Ok(e) => e,
        Err(e) => return fail(Exit::Error, e, "Run `scriptvox doctor`."),
    };
    let mut args = Vec::new();
    for p in &a.inputs {
        args.extend(["-i".to_string(), abs(p)]);
    }
    args.extend([
        "-filter_complex".into(),
        graph(&clips, (w, h), fps, &a.fit, tr),
        "-map".into(),
        "[v]".into(),
        "-map".into(),
        "[a]".into(),
    ]);
    args.extend(enc.args());
    args.extend([
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-movflags".into(),
        "+faststart".into(),
        abs(&out),
    ]);
    if let Err(e) = ff.run(Path::new("."), &args, None) {
        return fail(Exit::Error, e, "Check that each clip plays.");
    }
    let loud = crate::edit::master(&ff, &out);
    let want: f64 = clips.iter().map(|c| c.secs).sum::<f64>() - tr.1 * (clips.len() - 1) as f64;
    let mut env = Envelope::ok("join");
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
                "The video stays for inspection. Run the join again.",
            );
        }
    }
    let got = ff.duration(&out).unwrap_or(want);
    if (got - want).abs() > 0.5 {
        env.findings.push(json!({"gate": "length", "status": "warn",
            "detail": format!("the file runs {got:.2} s, the join should run {want:.2} s"),
            "fix": "Look at the video."}));
    }
    env.metrics.insert("clips".into(), json!(clips.len()));
    env.metrics
        .insert("seconds_after".into(), json!((got * 100.0).round() / 100.0));
    env.metrics.insert("size".into(), json!(format!("{w}x{h}")));
    env.metrics.insert("fps".into(), json!(fps));
    if let Some(l) = loud {
        env.metrics.insert("loudness_lufs".into(), json!(l));
    }
    let v = abs(&out);
    env.next = vec![
        format!("scriptvox snapshot {v} --sheet 9 --json"),
        format!("scriptvox check {v} --json"),
    ];
    env.artifacts.insert("video".into(), json!(v));
    crate::edit::fail_on_gate(env, |g| g == "av_drift")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_overlap_and_a_silent_clip_gets_silence() {
        let g = graph(
            &[
                Clip {
                    secs: 3.0,
                    sound: true,
                },
                Clip {
                    secs: 2.0,
                    sound: false,
                },
                Clip {
                    secs: 4.0,
                    sound: true,
                },
            ],
            (1080, 1920),
            30,
            "pad",
            ("fade", 0.25),
        );
        assert!(
            g.contains("xfade=transition=fade:duration=0.250:offset=2.750[xv1]"),
            "{g}"
        );
        assert!(g.contains("offset=4.500[xv2]"), "{g}");
        assert!(
            g.contains("anullsrc=r=48000:cl=stereo,atrim=duration=2.000"),
            "{g}"
        );
        assert!(
            g.contains("force_original_aspect_ratio=decrease,pad=1080:1920"),
            "{g}"
        );
        assert!(g.ends_with("[xv2]null[v];[xa2]anull[a]"), "{g}");
    }
}
