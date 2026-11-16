//! `scriptvox edit --background` and `--alpha` (G056): the subject stays, by the matte row's mask, and the rest of the
//! picture is replaced or made clear. The mask is asked through `generate` (one plan, one approval, one job on
//! record), and it is a plain file input of one FFmpeg pass. Spec: `docs/specs/edit-background.md`.
//! `--enhance` (G122) sends the picture to the upscale row the same way and puts the source's sound back
//! (`docs/specs/upscale.md`).

use crate::edit::{fail, Args};
use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::json;
use std::path::{Path, PathBuf};

/// The longest clip that goes to the matte row: the row is a shared GPU, and the copy must stay under the upload limit.
const MATTE_MAX_S: f64 = 60.0;
/// The same limit as `generate --video`: the service takes 40 MB bodies, and base64 adds a third.
const SEND_MAX_BYTES: u64 = 28_000_000;
/// The long side of the copy that is sent. The matte model works at 1024 px, so a larger copy buys nothing.
const SEND_LONG_SIDE: u32 = 1280;

/// The longest clip that goes to the upscale row: the row is a shared GPU, and 10 s holds it for about 5 minutes.
const ENHANCE_MAX_S: f64 = 10.0;
/// The time of one call, measured on one H100 (`upscale.md`): about a minute to start, then about 25 s for each second
/// of the clip (3 s at 640x360 took 134 s on 11 Nov 2026; 5 s at 704x1280 took 102 to 118 s on 3 Nov).
const ENHANCE_START_S: f64 = 60.0;
const ENHANCE_S_PER_S: f64 = 25.0;

/// What `edit` does after this step.
pub enum Step {
    /// The answer of the whole command: a refusal, a plan, or the finished file.
    Done(Box<Envelope>),
    /// Go on with the other options. `Some`: the row's cost, and `input` is now the new picture.
    Next(Option<f64>),
}

enum Back {
    /// `RRGGBB`, checked.
    Colour(String),
    Blur,
    /// The real path inside the project, and the decoder for its kind.
    Picture(PathBuf, &'static str),
    /// The real path inside the project, and the demuxer for its container.
    Clip(PathBuf, &'static str),
}

fn done(exit: Exit, msg: impl Into<String>, fix: &str) -> Step {
    Step::Done(Box::new(fail(exit, msg, fix)))
}

/// What a file is, by its first bytes (`media_gate`): `(true, decoder)` for one picture, `(false, demuxer)` for a plain
/// clip container. `Err` names what else it is.
fn plain_media(head: &[u8]) -> Result<(bool, &'static str), &'static str> {
    use crate::media_gate::Kind;
    match crate::media_gate::sniff(head) {
        Ok(p) if p.kind == Kind::Sound => Err("a sound file"),
        Ok(p) => Ok((p.kind == Kind::Picture, p.format)),
        Err(what) => Err(what),
    }
}

/// The arguments that open a checked file, right before `-i` (`media_gate::Plain::args`).
fn plain_input(picture: bool, format: &'static str) -> Vec<String> {
    use crate::media_gate::{Kind, Plain};
    let kind = if picture { Kind::Picture } else { Kind::Clip };
    Plain { kind, format }.args()
}

/// `--background` as a checked value. Only a checked colour, or the real path of a plain picture or clip inside the
/// project, goes on to FFmpeg.
fn parse_background(ff: &Ffmpeg, v: &str) -> Result<Back, String> {
    if v == "blur" {
        return Ok(Back::Blur);
    }
    if let Some(hex) = v.strip_prefix('#') {
        return match crate::captions::ass_colour(v) {
            Some(_) => Ok(Back::Colour(hex.to_ascii_lowercase())),
            None => Err(format!("--background {v} is not a colour: give #RRGGBB")),
        };
    }
    if !Path::new(v).is_file() {
        return Err(format!("the background {v} does not exist"));
    }
    // The real path, with every link followed: a link inside the project to a file outside it is outside.
    let full = std::env::current_dir()
        .and_then(std::fs::canonicalize)
        .ok()
        .zip(std::fs::canonicalize(v).ok())
        .filter(|(cwd, full)| full.starts_with(cwd))
        .map(|(_, full)| full)
        .ok_or_else(|| format!("the background {v} is outside this project folder"))?;
    let mut head = Vec::new();
    {
        use std::io::Read;
        std::fs::File::open(&full)
            .and_then(|f| f.take(4096).read_to_end(&mut head))
            .map_err(|e| format!("the background {v}: {e}"))?;
    }
    let (picture, format) = plain_media(&head).map_err(|what| {
        format!("the background {v} is not a picture or a video this option takes: it is {what}")
    })?;
    // ffprobe, told the format, must find a picture stream in it.
    let seen = std::process::Command::new(&ff.ffprobe)
        .args(["-v", "error"])
        .args(plain_input(picture, format))
        .args([
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_type",
        ])
        .args(["-of", "csv=p=0", "-i"])
        .arg(&full)
        .output()
        .is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("video"));
    if !seen {
        return Err(format!(
            "the background {v} is not a picture or a video this option takes: it starts as {format} and holds no picture FFmpeg can read"
        ));
    }
    Ok(if picture {
        Back::Picture(full, format)
    } else {
        Back::Clip(full, format)
    })
}

/// The copy that goes to the row: no sound, and for a mask the long side at most 1280. Named by the source's bytes and the settings,
/// and made once: the plan id is a hash of this file, so a plan, its approval and every later background on the same
/// clip must send the same bytes.
fn small_copy(
    ff: &Ffmpeg,
    input: &Path,
    work: &Path,
    name: &str,
    filter: Option<&str>,
    enc: &[String],
) -> Result<PathBuf, String> {
    let mut h = blake3::Hasher::new();
    h.update(filter.unwrap_or("").as_bytes());
    h.update(enc.join(" ").as_bytes());
    let mut f = std::fs::File::open(input).map_err(|e| e.to_string())?;
    std::io::copy(&mut f, &mut h).map_err(|e| e.to_string())?;
    let copy = work.join(format!("{name}-in-{}.mp4", &h.finalize().to_hex()[..16]));
    if copy.is_file() {
        return Ok(copy);
    }
    let part = work.join(format!("{name}-in.part.mp4"));
    let mut args: Vec<String> = vec!["-i".into(), input.display().to_string(), "-an".into()];
    if let Some(f) = filter {
        args.extend(["-vf".into(), f.into()]);
    }
    args.extend(enc.iter().cloned());
    args.extend(["-map_metadata", "-1", "-fflags", "+bitexact"].map(String::from));
    args.push(part.display().to_string());
    ff.run(Path::new("."), &args, None)?;
    std::fs::rename(&part, &copy).map_err(|e| e.to_string())?;
    Ok(copy)
}

/// One clip through the first row of `kind` with a key, by `generate`: its plan, approval, job record and download.
/// A free row first; a paid row runs only with the approved plan. `Ok`: the row's name and the cost. `Err`: the answer
/// of the command (a plan, a refusal, a failure), with `about` under `key`.
fn ask_row(
    a: &Args,
    kind: &str,
    key: &str,
    copy: &Path,
    result: &Path,
    total: f64,
    about: impl Fn(&crate::generators::Generator) -> serde_json::Value,
) -> Result<(String, f64, Job), Step> {
    let rows: Vec<_> = crate::generators::all()
        .into_iter()
        .filter(|g| g.kind == kind && crate::keys::resolve(&g.key).is_some())
        .collect();
    let Some(row) = rows.iter().find(|g| !g.paid()).or(rows.first()) else {
        return Err(done(
            Exit::NeedsInput,
            format!("no {kind} row with a key on this install"),
            &format!("`scriptvox providers --kind {kind}` lists the rows and what each needs."),
        ));
    };
    let mut got = crate::generate::run(crate::generate::Args {
        kind: None,
        last_image: None,
        model: row.name.clone(),
        prompt: String::new(),
        audio: None,
        video: Some(copy.to_path_buf()),
        voice: None,
        lang: None,
        image: None,
        // The length the row is paid for, rounded up: the plan never shows less than the bill.
        seconds: Some(total.ceil()),
        aspect: "9:16".into(),
        out: Some(result.to_path_buf()),
        wait_s: 1800,
        approve: crate::plan::approved().map(str::to_string),
        max_spend: a.max_spend,
        plan_only: a.plan,
        idempotency_key: None,
        seed: None,
    });
    if a.plan || !got.ok {
        if !got.ok {
            got.command = "edit".into();
        }
        for n in &mut got.next {
            *n = n.replace("scriptvox generate ...", "scriptvox edit ...");
        }
        return Err(Step::Done(Box::new(got.with(key, about(row)))));
    }
    let cost = got
        .metrics
        .get("cost_usd")
        .and_then(|c| c.as_f64())
        .unwrap_or(0.0);
    let job = Job {
        folder: got
            .run_id
            .as_ref()
            .map(|id| PathBuf::from(".scriptvox/runs").join(id)),
        paid: row.paid(),
    };
    Ok((row.name.clone(), cost, job))
}

/// Where a row's finished job is on record, to drop a result that cannot be used.
struct Job {
    folder: Option<PathBuf>,
    paid: bool,
}

impl Job {
    /// After a result that cannot be used: a free row's job is forgotten, so the same command asks the row again.
    /// A paid job stays on record (it was charged), and the fix line names its folder. Returns the fix line.
    fn drop_result(&self) -> String {
        match (&self.folder, self.paid) {
            (Some(f), false) => {
                for name in ["clip.mp4", "clip.job.json", "clip.submitting"] {
                    let _ = std::fs::remove_file(f.join(name));
                }
                "Nothing was written. Run the same command again: this free row is asked again. If it repeats, the row is at fault: `scriptvox providers test <row>`.".into()
            }
            (Some(f), true) => format!(
                "Nothing was written. The paid job stays on record in {}, so the same command gives the same result and buys nothing. Check the service; to buy again, remove that folder and approve the plan again.",
                f.display()
            ),
            (None, _) => "Nothing was written. The row is at fault: `scriptvox providers test <row>`.".into(),
        }
    }
}

/// The demuxer for a clip that a row gave back. The file comes from a service, so it is not trusted: only a plain
/// container by its first bytes passes, before any FFmpeg pass is given the file. A list (HLS, ffconcat) is removed.
fn plain_result(row: &str, file: &Path, job: &Job) -> Result<&'static str, Step> {
    use std::io::Read;
    let mut head = Vec::new();
    let read = std::fs::File::open(file).and_then(|f| f.take(4096).read_to_end(&mut head));
    let what = match read.map(|_| plain_media(&head)) {
        Ok(Ok((false, demuxer))) => return Ok(demuxer),
        Ok(Ok((true, _))) => "a picture".to_string(),
        Ok(Err(what)) => what.to_string(),
        Err(e) => e.to_string(),
    };
    let _ = std::fs::remove_file(file);
    Err(done(
        Exit::Error,
        format!("{row} answered with a file that is not a plain clip: it is {what}"),
        &job.drop_result(),
    ))
}

/// The frames, width and height of a row's result, read as `demuxer` from the file protocol only.
fn result_shape(ff: &Ffmpeg, file: &Path, demuxer: &'static str) -> Option<(u64, u32, u32)> {
    let o = std::process::Command::new(&ff.ffprobe)
        .args(["-v", "error"])
        .args(plain_input(false, demuxer))
        .args(["-select_streams", "v:0", "-count_packets", "-show_entries"])
        .args(["stream=width,height,nb_read_packets", "-of", "json", "-i"])
        .arg(file)
        .output()
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).ok()?;
    let s = &v["streams"][0];
    let num = |k: &str| s[k].as_u64().or_else(|| s[k].as_str()?.parse().ok());
    Some((
        num("nb_read_packets")?,
        num("width")? as u32,
        num("height")? as u32,
    ))
}

/// The frame counts of what was sent and what came back, when they are more than one frame apart.
fn frames_differ(ff: &Ffmpeg, sent: &Path, back: Option<u64>) -> Option<(u64, u64)> {
    let s = ff.readback(sent).map_or(0, |r| r.frames);
    let b = back.unwrap_or(0);
    (s == 0 || s.abs_diff(b) > 1).then_some((s, b))
}

/// FFmpeg with the sound copied; when the container cannot hold that codec, once more with `codec`.
fn run_keeping_sound(ff: &Ffmpeg, args: &[String], codec: &[&str]) -> Result<(), String> {
    let with = |sound: &[&str]| {
        let mut a = args.to_vec();
        let out = a.pop().unwrap_or_default();
        a.extend(sound.iter().map(|s| s.to_string()));
        a.push(out);
        ff.run(Path::new("."), &a, None)
    };
    with(&["-c:a", "copy"]).or_else(|_| with(codec))
}

/// The `edit` checks on a file this step wrote by itself: a picture stream, `av_drift`, and the length.
fn checked(ff: &Ffmpeg, out: &Path, total: f64, size: (u32, u32), cost: f64) -> Envelope {
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
    let got = ff.duration(out).unwrap_or(total);
    if (got - total).abs() > 0.5 {
        env.findings.push(json!({"gate": "length", "status": "warn",
            "detail": format!("the file runs {got:.2} s, the source {total:.2} s"),
            "fix": "Look at the video. Its picture or sound may be shorter than the other."}));
    }
    let want = [
        (
            "black",
            "`edit` runs only av_drift; run `scriptvox check` on the result",
        ),
        (
            "loudness",
            "the sound is the source's own, not measured; run `scriptvox check` on the result",
        ),
        (
            "matte",
            "no gate judges the mask: look at the frames (`scriptvox snapshot <file> --sheet 4`)",
        ),
    ];
    let not_checked = crate::gates::not_checked(&env.findings, &want);
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
    env.metrics.insert("seconds_before".into(), json!(r(total)));
    env.metrics.insert("seconds_after".into(), json!(r(got)));
    env.metrics.insert("cuts".into(), json!(0));
    env.metrics
        .insert("size".into(), json!(format!("{}x{}", size.0, size.1)));
    env.metrics.insert("cost_usd".into(), json!(cost));
    crate::edit::fail_on_gate(env, |g| g == "av_drift")
}

/// The other options of the same call, which run after this step on the new picture.
fn more_to_do(a: &Args) -> bool {
    a.captions.is_some()
        || a.hook.is_some()
        || a.label.is_some()
        || a.logo.is_some()
        || a.music.is_some()
        || !a.sfx.is_empty()
        || a.clean_voice.is_some()
        || a.bleep.is_some()
        || !a.zoom.is_empty()
        || !a.broll.is_empty()
        || !a.blur.is_empty()
}

/// Any option of the other steps of `edit`: the small operations (`edit_small`) take none of them in the same call.
pub(crate) fn other_options(a: &Args) -> bool {
    a.background.is_some()
        || a.alpha
        || a.enhance
        || a.captions_file.is_some()
        || a.zoom_on_emphasis
        || more_to_do(a)
        || cuts_or_shapes(a)
}

/// The options that cut, crop, speed up or remake the picture.
fn cuts_or_shapes(a: &Args) -> bool {
    a.reframe.is_some()
        || a.fit.is_some()
        || a.track
        || a.cut_silence
        || a.cut_still
        || a.cut_fillers
        || a.cut_retakes
        || a.keep.is_some()
        || a.keep_words.is_some()
        || a.from.is_some()
        || a.to.is_some()
        || a.speed.is_some()
        || a.lipsync.is_some()
}

/// `--enhance`: the picture through the upscale row, then the source's sound beside it again.
fn enhance(a: &Args) -> Step {
    if a.background.is_some() || a.alpha || more_to_do(a) || cuts_or_shapes(a) {
        return done(
            Exit::Usage,
            "--enhance works alone, on the whole clip: use two calls",
            "Run the other edit first, then `scriptvox edit <result> --enhance`. Cut before, so the row gets only what stays; captions and a logo after, so they are drawn at the new size.",
        );
    }
    if !a.input.is_file() {
        return done(
            Exit::Usage,
            format!("{} does not exist", a.input.display()),
            "Give a video file.",
        );
    }
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return Step::Done(Box::new(crate::engine_ffmpeg::locate_failure("edit", e))),
    };
    let (Ok(total), Some((w, h))) = (ff.duration(&a.input), ff.shown_size(&a.input)) else {
        return done(
            Exit::Usage,
            format!("{} has no picture FFmpeg can read", a.input.display()),
            "Give a video file.",
        );
    };
    if total > ENHANCE_MAX_S {
        return done(
            Exit::Usage,
            format!("the video runs {total:.1} s, and --enhance takes at most {ENHANCE_MAX_S:.0} s in one call: the upscale row is a shared GPU and works about {ENHANCE_S_PER_S:.0} s for each second"),
            &format!(
                "Cut it first: `scriptvox edit {} --from 0 --to {ENHANCE_MAX_S:.0} --out part.mp4`, then `scriptvox edit part.mp4 --enhance`.",
                a.input.display()
            ),
        );
    }
    let stem = a
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video")
        .to_string();
    let work = PathBuf::from(".scriptvox/edit").join(&stem);
    if let Err(e) = std::fs::create_dir_all(&work) {
        return done(Exit::Error, e.to_string(), "Check the folder is writable.");
    }
    // The picture as it is when an MP4 can hold it (no second encode before the row), else encoded once.
    let enc = if ff
        .video_codec(&a.input)
        .is_some_and(|c| matches!(c.as_str(), "h264" | "hevc" | "av1" | "mpeg4"))
    {
        vec!["-c:v".to_string(), "copy".to_string()]
    } else {
        match ff.pick_encoder() {
            Ok((e, _)) => e.args(),
            Err(e) => {
                return done(
                    Exit::Error,
                    e,
                    "Install an FFmpeg build with libx264 or libopenh264.",
                )
            }
        }
    };
    let copy = match small_copy(&ff, &a.input, &work, "enhance", None, &enc) {
        Ok(c) => c,
        Err(e) => return done(Exit::Error, e, "Check the input video plays."),
    };
    if std::fs::metadata(&copy).map_or(0, |m| m.len()) > SEND_MAX_BYTES {
        return done(
            Exit::Usage,
            format!("the video is too large to send: its picture is more than 28 MB for {total:.1} s"),
            &format!(
                "Cut it first: `scriptvox edit {} --from 0 --to {:.0} --out part.mp4`, then --enhance on each part.",
                a.input.display(),
                (total / 2.0).floor().max(1.0)
            ),
        );
    }
    let size_in = format!("{w}x{h}");
    let seconds = (total * 100.0).round() / 100.0;
    // The row's size: its short side, and the long side by the clip's own shape.
    let about = |g: &crate::generators::Generator| {
        let short = g
            .extra
            .get("resolution")
            .and_then(|r| r.as_u64())
            .unwrap_or(1080) as f64;
        let k = short / f64::from(w.min(h));
        let even = |x: u32| (f64::from(x) * k / 2.0).floor() as u32 * 2;
        json!({"row": g.name, "size_in": size_in, "size_out_about": format!("{}x{}", even(w), even(h)), "grows": k,
            "seconds": seconds, "takes_s_about": (ENHANCE_START_S + total * ENHANCE_S_PER_S).ceil(),
            "note": "The time is for one H100 with nothing else in its queue."})
    };
    // Before any call: a clip that is already at the row's size gains nothing.
    if let Some(g) = crate::generators::all()
        .into_iter()
        .find(|g| g.kind == "upscale" && crate::keys::resolve(&g.key).is_some())
    {
        if about(&g)["grows"].as_f64().is_some_and(|k| k <= 1.0) {
            return done(
                Exit::Usage,
                format!(
                    "the video is {size_in}: already at the size {} gives",
                    g.name
                ),
                "Drop --enhance: the row makes the short side 1080, and this clip has it.",
            );
        }
    }
    let big = copy.with_file_name(
        copy.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .replace("enhance-in-", "enhance-"),
    );
    let started = std::time::Instant::now();
    let (row, cost, job) = match ask_row(a, "upscale", "enhance", &copy, &big, total, about) {
        Ok(r) => r,
        Err(step) => return step,
    };
    let waited = started.elapsed().as_secs_f64().round();
    let demuxer = match plain_result(&row, &big, &job) {
        Ok(d) => d,
        Err(step) => return step,
    };
    let Some((frames, bw, bh)) = result_shape(&ff, &big, demuxer) else {
        return done(
            Exit::Error,
            format!("the result of {row} has no picture"),
            &job.drop_result(),
        );
    };
    let size = (bw, bh);
    // The same frames, or the sound would not fit the new picture.
    if let Some((sent, back)) = frames_differ(&ff, &copy, Some(frames)) {
        return done(
            Exit::Error,
            format!(
                "the result of {row} does not fit the clip: {sent} frames sent, {back} came back"
            ),
            &job.drop_result(),
        );
    }
    let out = a
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("scriptvox-out").join(format!("{stem}-edit.mp4")));
    if let Some(p) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(p);
    }
    // The row's picture and the source's sound, both copied: no encode here.
    let mut args = plain_input(false, demuxer);
    args.extend(["-i".into(), big.display().to_string()]);
    args.extend(["-i".into(), a.input.display().to_string()]);
    args.extend(["-map", "0:v:0", "-map", "1:a?", "-c:v", "copy"].map(String::from));
    args.extend(["-movflags".into(), "+faststart".into()]);
    args.push(out.display().to_string());
    if let Err(e) = run_keeping_sound(&ff, &args, &["-c:a", "aac", "-b:a", "192k"]) {
        let _ = std::fs::remove_file(&out);
        return done(Exit::Error, e, "Check the input video plays.");
    }
    let env = checked(&ff, &out, total, size, cost).with(
        "enhance",
        json!({"row": row, "size_in": size_in, "size_out": format!("{}x{}", size.0, size.1),
            "row_seconds": waited, "sound": "copied from the source"}),
    );
    Step::Done(Box::new(env))
}

pub fn step(a: &mut Args) -> Step {
    if a.enhance {
        return enhance(a);
    }
    if a.background.is_none() && !a.alpha {
        return Step::Next(None);
    }
    if a.alpha && (a.background.is_some() || more_to_do(a)) {
        return done(
            Exit::Usage,
            "--alpha writes the subject alone on a clear background, so it takes no --background and no other edit option",
            "Run --alpha alone. For a new background, give --background without --alpha.",
        );
    }
    // A cut or a clip would send frames to the row that the result drops; a crop or a new speed changes what the mask
    // must fit; lip sync makes a new picture.
    if cuts_or_shapes(a) {
        return done(
            Exit::Usage,
            "--background and --alpha do not mix with a cut, --keep, --keep-words, --from/--to, --speed, --duration, --reframe or --lipsync: use two calls",
            "Cut, crop or lip sync first, then run `scriptvox edit <result> --background ...` on the result.",
        );
    }
    let stem = a
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video")
        .to_string();
    let out = match (&a.out, a.alpha) {
        (Some(o), true) if o.extension().is_none_or(|e| !e.eq_ignore_ascii_case("mov")) => {
            return done(
                Exit::Usage,
                "--alpha writes a ProRes 4444 .mov, and a .webm of the same name beside it",
                "Give --out <name>.mov, or drop --out.",
            )
        }
        (Some(o), _) => o.clone(),
        (None, true) => PathBuf::from("scriptvox-out").join(format!("{stem}-alpha.mov")),
        (None, false) => PathBuf::from("scriptvox-out").join(format!("{stem}-edit.mp4")),
    };
    if !a.input.is_file() {
        return done(
            Exit::Usage,
            format!("{} does not exist", a.input.display()),
            "Give a video file.",
        );
    }
    let ff = match Ffmpeg::locate(a.use_system, true) {
        Ok(f) => f,
        Err(e) => return Step::Done(Box::new(crate::engine_ffmpeg::locate_failure("edit", e))),
    };
    let back = match a.background.as_deref().map(|v| parse_background(&ff, v)) {
        Some(Err(e)) => {
            return done(
                Exit::Usage,
                e,
                "Give --background #RRGGBB, blur, or a picture or a clip inside this project.",
            )
        }
        Some(Ok(b)) => Some(b),
        None => None,
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
    if total > MATTE_MAX_S {
        return done(
            Exit::Usage,
            format!("the video runs {total:.1} s, and the matte row takes at most {MATTE_MAX_S:.0} s in one call"),
            &format!(
                "Cut it first: `scriptvox edit {} --from 0 --to {MATTE_MAX_S:.0} --out part.mp4`, then --background on each part.",
                a.input.display()
            ),
        );
    }
    // Even sizes: the encoders need them.
    let (w, h) = (w & !1, h & !1);
    // A variable-rate clip is made constant, the same way in the copy and in the result, so each mask frame meets
    // its own picture.
    let nominal = rb.fps.max(1.0);
    let (fps, steady) = match ff.frame_rate(&a.input) {
        Ok(avg) if crate::engine_ffmpeg::variable_rate(nominal, avg) => {
            let real = avg.round().clamp(15.0, 60.0);
            (real, format!("fps={real},"))
        }
        _ => (nominal, String::new()),
    };
    let work = PathBuf::from(".scriptvox/edit").join(&stem);
    if let Err(e) = std::fs::create_dir_all(&work) {
        return done(Exit::Error, e.to_string(), "Check the folder is writable.");
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
    let k = f64::from(SEND_LONG_SIDE) / f64::from(w.max(h));
    let (sw, sh) = if k < 1.0 {
        let half = |x: u32| ((f64::from(x) * k / 2.0).round() as u32).max(1) * 2;
        (half(w), half(h))
    } else {
        (w, h)
    };
    let copy = match small_copy(
        &ff,
        &a.input,
        &work,
        "matte",
        Some(&format!("{steady}scale={sw}:{sh},format=yuv420p")),
        &enc,
    ) {
        Ok(c) => c,
        Err(e) => return done(Exit::Error, e, "Check the input video plays."),
    };
    if std::fs::metadata(&copy).map_or(0, |m| m.len()) > SEND_MAX_BYTES {
        return done(
            Exit::Usage,
            format!("the video is too large to send: its {total:.1} s make more than 28 MB at {sw}x{sh}"),
            &format!(
                "Cut it first: `scriptvox edit {} --from 0 --to {:.0} --out part.mp4`, then --background on each part.",
                a.input.display(),
                (total / 2.0).floor().max(1.0)
            ),
        );
    }
    let mask = copy.with_file_name(
        copy.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .replace("matte-in-", "matte-"),
    );
    let about = |g: &crate::generators::Generator| json!({"row": g.name, "sent": format!("{sw}x{sh}"), "seconds": (total * 100.0).round() / 100.0});
    let (row, cost, job) = match ask_row(a, "matte", "matte", &copy, &mask, total, about) {
        Ok(r) => r,
        Err(step) => return step,
    };
    let demuxer = match plain_result(&row, &mask, &job) {
        Ok(d) => d,
        Err(step) => return step,
    };
    // One mask frame for each frame sent, or the subject and its mask drift apart.
    let shape = result_shape(&ff, &mask, demuxer);
    if let Some((sent, back)) = frames_differ(&ff, &copy, shape.map(|s| s.0)) {
        return done(
            Exit::Error,
            format!(
                "the mask from {row} does not fit the clip: {sent} frames sent, {back} came back"
            ),
            &job.drop_result(),
        );
    }
    let abs = |p: &Path| {
        std::path::absolute(p)
            .unwrap_or(p.to_path_buf())
            .display()
            .to_string()
    };
    if let Some(p) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(p);
    }
    // The mask is a grey picture in video levels: stretched to 0..255, so the subject is solid and the rest is gone.
    let cut = format!(
        "[1:v]scale={w}:{h}:in_range=tv:out_range=pc,format=gray[m];[0:v]{steady}scale={w}:{h},setsar=1[s0]"
    );
    let mut inputs: Vec<String> = vec!["-i".into(), abs(&a.input)];
    inputs.extend(plain_input(false, demuxer));
    inputs.extend(["-i".into(), abs(&mask)]);
    if a.alpha {
        let graph = format!("{cut};[s0][m]alphamerge[v]");
        // The settings of `export --overlay`: ProRes 4444 for an editor, VP9 with alpha for the web.
        let mov: &[&str] = &[
            "-c:v",
            "prores_ks",
            "-profile:v",
            "4",
            "-pix_fmt",
            "yuva444p10le",
            "-qscale:v",
            "9",
        ];
        let webm: &[&str] = &[
            "-c:v",
            "libvpx-vp9",
            "-pix_fmt",
            "yuva420p",
            "-crf",
            "30",
            "-b:v",
            "0",
            "-row-mt",
            "1",
            "-cpu-used",
            "4",
            "-an",
        ];
        let side = out.with_extension("webm");
        for (codec, file, sound) in [(mov, &out, true), (webm, &side, false)] {
            let mut args = inputs.clone();
            args.extend(["-filter_complex".into(), graph.clone()]);
            args.extend(["-map", "[v]"].map(String::from));
            if sound {
                args.extend(["-map", "0:a?"].map(String::from));
            }
            args.extend(codec.iter().map(|s| s.to_string()));
            args.push(abs(file));
            let ran = if sound {
                run_keeping_sound(&ff, &args, &["-c:a", "pcm_s16le"])
            } else {
                ff.run(Path::new("."), &args, None)
            };
            if let Err(e) = ran {
                let _ = std::fs::remove_file(file);
                return done(Exit::Error, e, "Check the input video plays.");
            }
        }
        let mut env = checked(&ff, &out, total, (w, h), cost);
        if env.ok {
            env.artifacts.insert("webm".into(), json!(abs(&side)));
            env = env.with(
                "alpha",
                json!({"row": row, "mov": "ProRes 4444 with the source's sound", "webm": "VP9 with alpha, no sound"}),
            );
        }
        return Step::Done(Box::new(env));
    }
    // The background is one more plain input at the picture's own rate, a little longer than the clip, so the clip
    // sets the end.
    let len = format!("{:.3}", total + 1.0);
    let cover = format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1");
    let mut args = inputs;
    let behind = match back {
        Some(Back::Colour(rgb)) => {
            args.extend(["-f", "lavfi", "-i"].map(String::from));
            args.push(format!("color=c=0x{rgb}:s={w}x{h}:r={fps}:d={len}"));
            "[s0]null[s];[2:v]null[bg]".to_string()
        }
        Some(Back::Picture(p, decoder)) => {
            args.extend(plain_input(true, decoder));
            args.extend(["-loop", "1", "-framerate"].map(String::from));
            args.extend([fps.to_string(), "-t".into(), len, "-i".into(), abs(&p)]);
            format!("[s0]null[s];[2:v]{cover}[bg]")
        }
        Some(Back::Clip(p, demuxer)) => {
            // Made first as its own file at the picture's rate and size: two streams of different speeds in one graph
            // make FFmpeg queue frames with no limit (`audiogram.md`, fault found).
            let conformed = work.join("background-in.mp4");
            let mut pre = plain_input(false, demuxer);
            pre.extend(["-stream_loop", "-1", "-i"].map(String::from));
            pre.extend([abs(&p), "-t".into(), len, "-an".into(), "-vf".into()]);
            pre.push(format!("fps={fps},{cover},format=yuv420p"));
            pre.extend(enc.iter().cloned());
            pre.push(abs(&conformed));
            if let Err(e) = ff.run(Path::new("."), &pre, None) {
                return done(Exit::Error, e, "Check the background clip plays.");
            }
            args.extend(["-i".into(), abs(&conformed)]);
            "[s0]null[s];[2:v]null[bg]".to_string()
        }
        // The same picture, small, blurred and large again: one source, so both branches run at one speed.
        Some(Back::Blur) | None => format!(
            "[s0]split[s][b];[b]scale={}:{},gblur=sigma=4,scale={w}:{h}[bg]",
            (w / 8).max(2) & !1,
            (h / 8).max(2) & !1
        ),
    };
    let graph =
        format!("{cut};{behind};[s][m]alphamerge[fg];[bg][fg]overlay=shortest=1,format=yuv420p[v]");
    let alone = !more_to_do(a);
    // With more to do, the new picture keeps the source's name in the work folder, so the result is named as usual.
    let made = if alone {
        out.clone()
    } else {
        let d = work.join("background");
        let _ = std::fs::create_dir_all(&d);
        d.join(format!("{stem}.mp4"))
    };
    args.extend(["-filter_complex".into(), graph]);
    args.extend(["-map", "[v]", "-map", "0:a?"].map(String::from));
    args.extend(enc.iter().cloned());
    args.extend(["-movflags".into(), "+faststart".into(), abs(&made)]);
    if let Err(e) = run_keeping_sound(&ff, &args, &["-c:a", "aac", "-b:a", "192k"]) {
        let _ = std::fs::remove_file(&made);
        return done(Exit::Error, e, "Check the input video plays.");
    }
    if !alone {
        a.input = made;
        a.background = None;
        return Step::Next(Some(cost));
    }
    let env = checked(&ff, &out, total, (w, h), cost).with(
        "background",
        json!({"row": row, "value": a.background, "sound": "copied from the source"}),
    );
    Step::Done(Box::new(env))
}
