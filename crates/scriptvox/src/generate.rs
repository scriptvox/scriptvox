//! `scriptvox generate`: one AI video clip from a generator row (phase 2, P2-002). Same money rules as `make`: a plan
//! with a `plan_id`, `--approve` for any cost above $0, a spend cap in the ledger, and a rerun that resumes the same
//! provider job instead of paying again.

use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::plan::{self, Plan, Stage};
use serde_json::json;
use std::path::{Path, PathBuf};

pub struct Args {
    pub model: String,
    /// With `--model free|cheapest|best`: the kind of output. Default `video`.
    pub kind: Option<String>,
    pub prompt: String,
    pub audio: Option<PathBuf>,
    pub video: Option<PathBuf>,
    pub voice: Option<String>,
    pub lang: Option<String>,
    pub image: Option<PathBuf>,
    pub seconds: Option<f64>,
    pub aspect: String,
    pub out: Option<PathBuf>,
    pub wait_s: u64,
    pub approve: Option<String>,
    pub max_spend: Option<f64>,
    pub plan_only: bool,
    pub idempotency_key: Option<String>,
    /// The same seed and input give the same result on a row that takes one (G075).
    pub seed: Option<u64>,
    /// The picture the clip ends on (G074): needs `image` and a row with `takes_last_frame`.
    pub last_image: Option<PathBuf>,
}

fn fail(exit: Exit, msg: impl Into<String>, fix: impl Into<String>) -> Envelope {
    Envelope::fail("generate", exit, msg.into(), Some(fix.into()))
}

/// A local image as a data URI, for image to video. Only images inside the project, at most 10 MB.
pub(crate) fn image_uri(p: &Path) -> Result<(String, String), Box<Envelope>> {
    file_uri(
        p,
        &[
            ("png", "image/png"),
            ("jpg", "image/jpeg"),
            ("jpeg", "image/jpeg"),
            ("webp", "image/webp"),
        ],
        10_000_000,
    )
}

/// A local audio file as a data URI (speech to text, or a voice sample). Inside the project, at most 25 MB.
fn audio_uri(p: &Path) -> Result<(String, String), Box<Envelope>> {
    file_uri(
        p,
        &[
            ("wav", "audio/wav"),
            ("mp3", "audio/mpeg"),
            ("m4a", "audio/mp4"),
            ("flac", "audio/flac"),
        ],
        25_000_000,
    )
}

fn file_uri(
    p: &Path,
    types: &[(&str, &str)],
    max: usize,
) -> Result<(String, String), Box<Envelope>> {
    let bad = |exit: Exit, msg: String, fix: &str| Box::new(fail(exit, msg, fix));
    let cwd = std::env::current_dir()
        .map_err(|e| bad(Exit::Error, e.to_string(), "Check the folder."))?;
    let full = std::fs::canonicalize(p).map_err(|e| {
        bad(
            Exit::Usage,
            format!("{}: {e}", p.display()),
            "Give a file inside this project.",
        )
    })?;
    if !full.starts_with(std::fs::canonicalize(&cwd).unwrap_or(cwd)) {
        return Err(bad(
            Exit::Usage,
            format!("{} is outside this project", p.display()),
            "Copy the file into the project folder first.",
        ));
    }
    let ext = full
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let Some(&(_, mime)) = types.iter().find(|(e, _)| *e == ext) else {
        let names: Vec<_> = types.iter().map(|t| t.0).collect();
        return Err(bad(
            Exit::Usage,
            format!("{} is not one of {}", p.display(), names.join(", ")),
            "Use a supported file type.",
        ));
    };
    let bytes = std::fs::read(&full)
        .map_err(|e| bad(Exit::Error, e.to_string(), "Check the image file."))?;
    if bytes.len() > max {
        return Err(bad(
            Exit::Usage,
            format!("{} is over {} MB", p.display(), max / 1_000_000),
            "Use a smaller file.",
        ));
    }
    // The name is not proof: the first bytes must be a picture for a picture, and not a picture for a sound.
    let is = crate::media_gate::sniff(&bytes[..bytes.len().min(512)]);
    let picture = mime.starts_with("image/");
    // A picture goes out under the type of its name, so it must be one of the three the rows take (a BMP is not).
    let fits = |k: crate::media_gate::Plain| match picture {
        true => matches!(k.format, "png" | "mjpeg" | "webp"),
        false => k.kind != crate::media_gate::Kind::Picture,
    };
    if !is.is_ok_and(fits) {
        return Err(bad(
            Exit::Usage,
            format!(
                "{} is not {} by its first bytes",
                p.display(),
                if picture {
                    "a PNG, JPEG or WebP picture"
                } else {
                    "a sound or video file"
                }
            ),
            crate::media_gate::ACCEPTED,
        ));
    }
    let hash = blake3::hash(&bytes).to_hex()[..16].to_string();
    use base64::Engine;
    Ok((
        format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        ),
        hash,
    ))
}

/// The smallest call a row can do. The prompt or test file by kind, the row's shortest length, and the same plan and
/// approval as `generate`. The result names the row's terms.
pub fn provider_test(
    row: &str,
    wait_s: u64,
    approve: Option<String>,
    max_spend: Option<f64>,
    plan_only: bool,
    use_system: bool,
) -> Envelope {
    let say = |m: Exit, e: String, fix: &str| Envelope::fail("providers", m, e, Some(fix.into()));
    // A stock row (Pexels, Pixabay, Commons, Archive): one search, no download, no cost.
    if let Some(r) = crate::http_rows::rows().into_iter().find(|r| r.name == row) {
        let Some(key) = crate::keys::resolve(&r.key)
            .map(|(k, _)| k)
            .or_else(|| r.keyless().then(String::new))
        else {
            return say(
                Exit::NeedsInput,
                format!("no {} key", r.key),
                &format!("Run `scriptvox keys set {}` in your terminal.", r.key),
            );
        };
        if plan_only {
            return Envelope::ok("providers").with(
                "test",
                json!({"row": row, "kind": r.kind, "call": "one search for 'ocean', no download", "cost_usd": 0}),
            );
        }
        return match crate::http_rows::search(&r, &key, "ocean", "landscape", 0) {
            Ok(c) => Envelope::ok("providers").with(
                "test",
                json!({"row": row, "kind": r.kind, "query": "ocean", "results": c.len(),
                       "first": c.first().map(|c| json!({"page": c.page, "terms": c.terms}))}),
            ),
            Err(e) => say(
                if e.fallback() {
                    Exit::ProviderRetryable
                } else {
                    Exit::Error
                },
                e.message,
                "Run it again later; a rate limit clears in minutes.",
            ),
        };
    }
    if let Some((why, fix)) = crate::generators::problem(row) {
        return say(Exit::Usage, why, &fix);
    }
    let Some(g) = crate::generators::all().into_iter().find(|g| g.name == row) else {
        return say(
            Exit::Usage,
            format!("no row {row}"),
            "`scriptvox providers` lists the rows.",
        );
    };
    let dir = std::path::PathBuf::from(".scriptvox/provider-test");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return say(Exit::Error, e.to_string(), "Check the folder is writable.");
    }
    let needs_audio = matches!(g.kind.as_str(), "stt" | "stems" | "lipsync");
    let needs_video = matches!(
        g.kind.as_str(),
        "lipsync" | "upscale" | "interpolate" | "matte"
    ) || (g.kind == "sfx.gen"
        && g.t2v.as_deref().is_some_and(|t| t.ends_with("/v2a")));
    let lavfi = |args: &[&str], to: &Path| -> Result<(), String> {
        let ff = crate::engine_ffmpeg::Ffmpeg::locate(use_system, true)?;
        let mut args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        args.push(to.display().to_string());
        ff.run(Path::new("."), &args, None)
    };
    let (wav, clip) = (dir.join("test.wav"), dir.join("test.mp4"));
    if needs_audio || needs_video {
        if let Err(e) = crate::engine_ffmpeg::Ffmpeg::locate(use_system, true) {
            return crate::engine_ffmpeg::locate_failure("providers", e);
        }
    }
    if needs_audio {
        if let Err(e) = lavfi(&["-f", "lavfi", "-i", "sine=f=440:d=2:r=16000"], &wav) {
            return say(Exit::Error, e, "Run `scriptvox setup`.");
        }
    }
    if needs_video {
        let args = [
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=256x256:d=1:r=16",
            "-pix_fmt",
            "yuv420p",
        ];
        if let Err(e) = lavfi(&args, &clip) {
            return say(Exit::Error, e, "Run `scriptvox setup`.");
        }
    }
    let prompt = match g.kind.as_str() {
        "tts" => "This is a test.",
        "music.gen" => "A short calm piano phrase.",
        "sfx.gen" => "A door closes.",
        "image.gen" | "video.gen" => "A red ball on a white floor.",
        _ => "",
    };
    let seconds = g.durations.iter().copied().reduce(f64::min);
    let aspect = g.aspects.first().cloned().unwrap_or_else(|| "16:9".into());
    let terms = json!(g.terms);
    let mut env = run(Args {
        last_image: None,
        model: row.to_string(),
        kind: None,
        prompt: prompt.to_string(),
        audio: needs_audio.then(|| wav.clone()),
        video: needs_video.then(|| clip.clone()),
        voice: None,
        lang: (g.kind == "tts").then(|| "en".to_string()),
        image: None,
        seconds,
        aspect,
        out: None,
        wait_s,
        approve,
        max_spend,
        plan_only,
        idempotency_key: None,
        seed: None,
    });
    env.command = "providers".into();
    if env.ok && g.kind == "tts" {
        let f = listen(&env, prompt, use_system);
        env.findings.push(json!(f));
        if f.status == "fail" {
            let mut e = Envelope::fail("providers", Exit::GateFailed, f.detail, f.fix);
            e.findings = env.findings;
            e.artifacts = env.artifacts;
            env = e;
        }
    }
    env.with(
        "test",
        json!({"row": row, "kind": g.kind, "seconds": seconds, "terms": terms}),
    )
}

/// The listening check of a voice row (G049): a free speech row hears the test line, and each word must come back.
fn listen(env: &Envelope, said: &str, use_system: bool) -> crate::gates::Finding {
    let finding = |status, detail: String, fix: Option<String>| crate::gates::Finding {
        gate: "listen",
        status,
        detail,
        fix,
    };
    let audio = env
        .artifacts
        .get("audio")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    let ff = crate::engine_ffmpeg::Ffmpeg::locate(use_system, false);
    let (Some(audio), Ok(ff), Some(_)) = (audio, ff, crate::opentts::rows("stt")) else {
        return finding(
            "warn",
            "not checked: no free speech to text row heard the voice".into(),
            Some("Run `scriptvox setup speech-local`, then test again.".into()),
        );
    };
    let total = ff.duration(&audio).unwrap_or(0.0);
    let heard = match crate::analyze::transcribe(&ff, &audio, Some("en"), total) {
        Ok((doc, _)) => doc["text"].as_str().unwrap_or("").to_lowercase(),
        Err(e) => {
            let why = e.error.map(|x| x.message).unwrap_or_default();
            return finding("warn", format!("not checked: {why}"), None);
        }
    };
    let norm = |s: &str| -> Vec<String> {
        s.split_whitespace()
            .map(|w| {
                w.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase()
            })
            .filter(|w| !w.is_empty())
            .collect()
    };
    let got = norm(&heard);
    let missing: Vec<String> = norm(said)
        .into_iter()
        .filter(|w| !got.contains(w))
        .collect();
    if missing.is_empty() {
        finding("pass", format!("heard \"{}\"", heard.trim()), None)
    } else {
        finding(
            "fail",
            format!(
                "heard \"{}\", missing: {}",
                heard.trim(),
                missing.join(", ")
            ),
            Some("The row did not say the test line: check its model and voice.".into()),
        )
    }
}

pub fn run(mut a: Args) -> Envelope {
    let mut chosen = None;
    if let Some(picked) =
        crate::generators::choose_by_word(&a.model, a.kind.as_deref().unwrap_or("video"))
    {
        match picked {
            Ok((g, why)) => {
                eprintln!("scriptvox: --model {} picked {}: {why}", a.model, g.name);
                chosen = Some(json!({"word": a.model, "row": g.name, "why": why}));
                a.model = g.name;
            }
            Err(why) => {
                return fail(
                    Exit::NeedsInput,
                    why,
                    "Name a row with `--model`, or run `scriptvox providers`.",
                )
            }
        }
    }
    let mut env = run_row(a);
    if let Some(c) = chosen {
        env = env.with("row_choice", c);
    }
    env
}

/// Up to two other rows of this kind that can take this same request and fit `--max-spend`, cheapest first:
/// each one a command to plan it, with its price after `#`. Empty with no cap.
fn cheaper_commands(
    a: &Args,
    g: &crate::generators::Generator,
    rows: &[crate::generators::Generator],
    want: f64,
    input_s: f64,
) -> Vec<String> {
    let Some(cap) = a.max_spend else {
        return vec![];
    };
    let mut fits: Vec<(f64, f64, &crate::generators::Generator)> = rows
        .iter()
        .filter(|o| {
            o.name != g.name
                && o.kind == g.kind
                && !o.unknown_price()
                && o.why_not().is_none()
                // The same request must work on it: its modes, its frame, its length, a price we can work out.
                && (o.t2v.is_some() || a.image.is_some())
                && (a.video.is_some() || !o.t2v.as_deref().is_some_and(|t| t.ends_with("/v2a")))
                && (a.image.is_none() || o.i2v.is_some() || o.takes_reference)
                && (a.last_image.is_none() || o.takes_last_frame.is_some())
                && (!o.visual() || o.aspects.contains(&a.aspect))
                && (o.durations.is_empty() || o.duration_for(want) + 1e-9 >= want)
                && (o.usd_per_input_s == 0.0 || input_s > 0.0)
        })
        .map(|o| {
            let s = o.duration_for(want);
            let refs = usize::from(a.image.is_some() && o.takes_reference);
            (o.cost_for(s, input_s, refs), s, o)
        })
        .filter(|(cost, ..)| *cost <= cap + 1e-9)
        .collect();
    fits.sort_by(|x, y| x.0.total_cmp(&y.0));
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let path = |flag: &str, p: &Option<PathBuf>| {
        p.as_ref().map_or(String::new(), |p| {
            format!(" {flag} {}", quote(&p.display().to_string()))
        })
    };
    fits.iter()
        .take(2)
        .map(|(cost, s, o)| {
            let mut c = format!("scriptvox generate --model {}", o.name);
            if !a.prompt.is_empty() {
                c += &format!(" --prompt {}", quote(&a.prompt));
            }
            for (flag, p) in [
                ("--audio", &a.audio),
                ("--video", &a.video),
                ("--image", &a.image),
                ("--last-image", &a.last_image),
            ] {
                c += &path(flag, p);
            }
            if let Some(v) = &a.voice {
                c += &format!(" --voice {}", quote(v));
            }
            if let Some(l) = &a.lang {
                c += &format!(" --lang {}", quote(l));
            }
            if o.timed() {
                c += &format!(" --seconds {s}");
            }
            if o.visual() {
                c += &format!(" --aspect {}", a.aspect);
            }
            format!("{c} --plan --max-spend {cap}  # ${cost:.2}")
        })
        .collect()
}

fn run_row(a: Args) -> Envelope {
    if let Some((why, fix)) = crate::generators::problem(&a.model) {
        return fail(Exit::Usage, why, fix);
    }
    let rows = crate::generators::all();
    let Some(g) = rows.iter().find(|g| g.name == a.model) else {
        if a.model == crate::generators::local_image().name {
            return fail(
                Exit::NeedsInput,
                format!("{} is not installed", a.model),
                "Run `scriptvox setup image-local`: about 5 GB, free, on this machine.",
            );
        }
        let near = crate::generators::nearest(&a.model, &rows, a.kind.as_deref());
        return fail(
            Exit::Usage,
            format!("unknown model {}", a.model),
            format!("Nearest: {}. `scriptvox providers` lists them all, or use `--model free`, `cheapest` or `best`. To add one: `scriptvox models add <hf id, local file or fal endpoint>`.", near.join(", ")),
        );
    };
    if let Some(b) = g
        .admission(a.max_spend)
        .filter(|b| b.kind == "retired" || b.kind == "price")
    {
        return fail(Exit::Usage, b.why, b.fix);
    }
    if g.visual() && !g.aspects.contains(&a.aspect) {
        return fail(
            Exit::Usage,
            format!("{} does not offer {}", g.name, a.aspect),
            format!("Use one of: {}.", g.aspects.join(", ")),
        );
    }
    if !matches!(
        g.kind.as_str(),
        "stt" | "stems" | "upscale" | "interpolate" | "matte" | "lipsync"
    ) && a.prompt.trim().is_empty()
    {
        return fail(
            Exit::Usage,
            "the prompt is empty",
            "Give --prompt: the shot, the sound, or the text to speak.",
        );
    }
    if matches!(g.kind.as_str(), "stt" | "stems") && a.audio.is_none() {
        return fail(
            Exit::Usage,
            format!("a {} row needs --audio", g.kind),
            "Give --audio <file inside this project>.",
        );
    }
    if g.kind == "lipsync" && (a.video.is_none() || a.audio.is_none()) {
        return fail(
            Exit::Usage,
            "a lipsync row needs --video and --audio",
            "Give --video <mp4 inside this project> and --audio <the new voice>.",
        );
    }
    if matches!(g.kind.as_str(), "upscale" | "interpolate" | "matte") && a.video.is_none() {
        return fail(
            Exit::Usage,
            format!("a {} row needs --video", g.kind),
            "Give --video <mp4 inside this project>.",
        );
    }
    // 28 MB: base64 makes it about 37 MB, under the 40 MB body limit of scriptvox-gen.
    let (video, video_hash) = match &a.video {
        Some(p) => match file_uri(p, &[("mp4", "video/mp4")], 28_000_000) {
            Ok((u, h)) => (Some(u), Some(h)),
            Err(env) => return *env,
        },
        None => (None, None),
    };
    // A voice sample to clone: a person's voice needs their consent on record (G036).
    if let (true, Some(sample)) = (g.kind == "tts", &a.audio) {
        if crate::voices::consent_id(sample).is_none() {
            let action = format!(
                "Your own voice: scriptvox voices add --sample {0} --own. Another person's voice: scriptvox voices add --sample {0} --consent <proof file>",
                sample.display()
            );
            let mut env = fail(
                Exit::NeedsInput,
                "this row clones a voice, and this sample has no consent record",
                action.clone(),
            );
            env.needs_input =
                vec![json!({"field": "consent", "kind": "consent", "human_action": action})];
            return env;
        }
    }
    let (audio, audio_hash) = match &a.audio {
        Some(p) => match audio_uri(p) {
            Ok((u, h)) => (Some(u), Some(h)),
            Err(env) => return *env,
        },
        None => (None, None),
    };
    let (image, image_hash) = match &a.image {
        Some(p) => match image_uri(p) {
            Ok((u, h)) => (Some(u), Some(h)),
            Err(env) => return *env,
        },
        None => (None, None),
    };
    if image.is_some() && g.i2v.is_none() && !g.takes_reference {
        return fail(
            Exit::Usage,
            format!("{} has no image to video endpoint", g.name),
            "Drop --image, or pick a row with i2v.",
        );
    }
    let (last, last_hash) = match &a.last_image {
        Some(p) => match image_uri(p) {
            Ok((u, h)) => (Some(u), Some(h)),
            Err(env) => return *env,
        },
        None => (None, None),
    };
    if last.is_some() && image.is_none() {
        return fail(
            Exit::Usage,
            "--last-image needs --image: the clip goes from the first picture to the last",
            "Add --image with the first frame.",
        );
    }
    // Sound for a clip (sfx row with --video) defaults to the clip's own length.
    let clip_len = || {
        let v = a.video.as_ref().filter(|_| g.kind == "sfx.gen")?;
        let ff = crate::engine_ffmpeg::Ffmpeg::locate(false, false).ok()?;
        ff.duration(v).ok().map(f64::round)
    };
    // A paid transcript costs by the length of its audio, so that length must be known before a plan.
    let audio_len = if g.kind == "stt" && g.paid() {
        let measured = crate::engine_ffmpeg::Ffmpeg::locate(false, false)
            .ok()
            .zip(a.audio.as_ref())
            .and_then(|(ff, p)| ff.duration(p).ok());
        match measured {
            Some(s) => Some(s.ceil()),
            None => {
                return fail(
                    Exit::Usage,
                    "cannot measure the audio, so this paid row has no price",
                    "Give an audio file FFmpeg can read, and run `scriptvox setup` first.",
                )
            }
        }
    } else {
        None
    };
    let want = audio_len
        .or(a.seconds)
        .or_else(clip_len)
        .unwrap_or_else(|| g.durations.first().copied().unwrap_or(0.0));
    let seconds = g.duration_for(want);
    // A row gives its own lengths: a shorter clip than `--seconds` asked is said, not silent (`--seconds 10` on a row
    // of 5 s planned 5 s with no word, 11 Nov 2026).
    let short = a.seconds.filter(|s| seconds + 0.01 < *s).map(|s| {
        json!({"gate": "length", "status": "warn",
            "detail": format!("--seconds {s} was asked; {} gives {seconds} s at most", g.name),
            "fix": "Pick a row with that length: `scriptvox providers --json` lists `durations` for each row."})
    });
    let input = g.with_seed(
        g.request(&crate::generators::GenInput {
            prompt: a.prompt.clone(),
            seconds,
            aspect: a.aspect.clone(),
            image: image.clone(),
            audio,
            video,
            voice: a.voice.clone(),
            lang: a.lang.clone(),
        }),
        a.seed,
    );
    let Some(input) = g.with_last_frame(input, last.as_deref()) else {
        return fail(
            Exit::Usage,
            format!("{} takes no last frame", g.name),
            "Drop --last-image, or pick a row with `takes_last_frame` in `scriptvox providers`.",
        );
    };
    let input_s =
        if g.usd_per_input_s > 0.0 {
            let measured = crate::engine_ffmpeg::Ffmpeg::locate(false, false)
                .ok()
                .zip(a.video.as_ref())
                .and_then(|(ff, p)| ff.duration(p).ok());
            match measured {
                Some(s) => s,
                None => return fail(
                    Exit::Usage,
                    "cannot measure the source video, so this paid row has no price",
                    "Give --video with a file FFmpeg can read, and run `scriptvox setup` first.",
                ),
            }
        } else {
            0.0
        };
    let references = usize::from(a.image.is_some() && g.takes_reference);
    // A user's row with no known price costs at most what the user allows: no cap, no call.
    let cost = if g.unknown_price() {
        a.max_spend.unwrap_or_default()
    } else {
        g.cost_for(seconds, input_s, references)
    };
    // The plan id covers everything that decides the cost and the clip; the image by its hash, not its bytes.
    let mut id_input = input.clone();
    // Files count by their hash in the plan id, never by their bytes.
    let last_field = g.takes_last_frame.clone().unwrap_or_default();
    for (k, h) in [
        (g.image_key(), &image_hash),
        (last_field.as_str(), &last_hash),
        ("audio_url", &audio_hash),
        ("reference_audio_url", &audio_hash),
        ("video_url", &video_hash),
    ] {
        if let (Some(h), Some(_)) = (h, id_input.get(k)) {
            id_input[k] = json!(format!("blake3:{h}"));
        }
    }
    let plan = Plan {
        plan_id: plan::plan_id(
            &json!({"command": "generate", "model": g.name, "input": id_input, "usd_per_s": g.usd_per_s, "usd_each": g.usd_each, "audio_s": audio_len, "cost": cost, "base": g.base, "key": g.key}),
        ),
        cost_usd: cost,
        stages: vec![Stage {
            stage: "generate",
            tool: format!("video.gen.{}", g.name),
            cost_usd: cost,
            network: g.gateway != "local",
            explain: format!(
                "{} through {}, {} {}{}",
                g.name,
                g.gateway,
                g.cost_words(seconds),
                if g.unknown_price() {
                    format!("(at most your --max-spend of ${cost:.2})")
                } else {
                    format!(
                        "({} price, read {} from {})",
                        g.price, g.price_checked, g.price_source
                    )
                },
                g.stale_note()
                    + &g.takes_s.map_or(String::new(), |t| {
                        format!("; takes about {t:.0} s, measured")
                    })
                    + &g.external_note()
                    + &g.seed_note(a.seed)
            ),
        }],
        downloads: vec![],
    };
    // Before any approval: a row that cannot run says so first. Nothing is called and nothing is spent.
    let say_why_not = |env: &mut Envelope| {
        if let Some(b) = g.why_not() {
            let field = match b.kind {
                "secret" => format!("generate.{}.api_key", g.key),
                _ => "generate.row".to_string(),
            };
            env.needs_input
                .push(crate::voices::needs_input(&field, b.kind, &b.fix, vec![]));
            env.next.insert(0, b.fix);
        }
    };
    // A stale approval is no approval: only the plan's own id goes on to the reserve.
    if a.approve.as_deref() != Some(plan.plan_id.as_str()) {
        let cheaper = cheaper_commands(&a, g, &rows, want, input_s);
        if let Some(env) = plan::over_cap("plan", &plan.plan_id, cost, a.max_spend, cheaper) {
            return env.with("stages", json!(plan.stages));
        }
    }
    if a.plan_only {
        let mut env = Envelope::ok("plan");
        env.plan_id = Some(plan.plan_id.clone());
        // A free plan needs no approval: the same command without --plan runs it.
        env.next = vec![if cost > 0.0 {
            format!("scriptvox generate ... --approve {}", plan.plan_id)
        } else {
            "Run the same command without --plan.".to_string()
        }];
        say_why_not(&mut env);
        env.metrics.insert("cost_usd".into(), json!(cost));
        env.findings.extend(crate::generators::plan_finding());
        env.findings.extend(short);
        return env.with("stages", json!(plan.stages));
    }
    if let Some(why) = plan::needs_approval(&plan.plan_id, plan.cost_usd, a.approve.as_deref()) {
        let mut env = fail(
            Exit::ApprovalNeeded,
            format!(
                "approval needed: {} (${cost:.2})",
                why["reason"].as_str().unwrap_or("")
            ),
            format!("Run the same command with --approve {}", plan.plan_id),
        );
        env.status = "approval_needed";
        env.plan_id = Some(plan.plan_id.clone());
        env.next = vec![format!("scriptvox generate ... --approve {}", plan.plan_id)];
        say_why_not(&mut env);
        env.metrics.insert("cost_usd".into(), json!(cost));
        return env.with("stages", json!(plan.stages));
    }
    // The same plan (and key) is the same run, so a rerun finds the job record and resumes it.
    let run_id = plan::run_id_for(
        &plan.plan_id,
        a.idempotency_key.as_deref().unwrap_or("generate"),
    );
    let project = Path::new(".");
    let dir = project.join(".scriptvox/runs").join(&run_id);
    let cap = a.max_spend.unwrap_or(cost);
    match crate::genjob::run_priced(
        g,
        &input,
        // A reference picture on an image row stays on the row's text endpoint.
        image.is_some() && g.i2v.is_some(),
        project,
        &dir,
        "clip",
        &run_id,
        cap,
        a.wait_s,
        // The plan's own price: the reserve must hold what was approved, whatever the request shows of it.
        g.paid().then_some(cost),
    ) {
        Ok((video, spent)) => {
            let out = a.out.unwrap_or_else(|| {
                PathBuf::from("scriptvox-out").join(format!("{run_id}-{}.{}", g.name, g.ext()))
            });
            if let Some(parent) = out.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::copy(&video, &out) {
                return fail(
                    Exit::Error,
                    format!(
                        "the clip is at {} but copying it failed: {e}",
                        video.display()
                    ),
                    "Copy it by hand.",
                );
            }
            // P2-006: check the clip. No FFmpeg here means no check, said in a finding, never a silent pass.
            let findings = match crate::engine_ffmpeg::Ffmpeg::locate(false, false) {
                _ if g.kind != "video.gen" => vec![],
                Ok(ff) => {
                    crate::gates::clip_check(&ff, &out, seconds, &a.aspect, a.image.is_some())
                }
                Err(e) => vec![crate::gates::Finding {
                    gate: "clip_check",
                    status: "warn",
                    detail: format!("not checked: {e}"),
                    fix: Some("Run `scriptvox setup` to install FFmpeg.".into()),
                }],
            };
            let failed = findings.iter().find(|f| f.status == "fail").cloned();
            let mut env = match &failed {
                Some(f) => fail(
                    Exit::GateFailed,
                    format!("the clip failed a check: {}", f.detail),
                    f.fix.clone().unwrap_or_default(),
                ),
                None => Envelope::ok("generate"),
            };
            env.findings = findings.iter().map(|f| json!(f)).collect();
            env.findings.extend(short);
            env.run_id = Some(run_id);
            env.plan_id = Some(plan.plan_id);
            env.artifacts.insert(
                match g.kind.as_str() {
                    "video.gen" | "upscale" | "interpolate" | "matte" | "lipsync" => "video",
                    "image.gen" => "image",
                    "stt" => "transcript",
                    _ => "audio",
                }
                .into(),
                json!(std::path::absolute(&out).unwrap_or(out)),
            );
            env.metrics.insert("cost_usd".into(), json!(spent));
            env.metrics.insert("seconds".into(), json!(seconds));
            env.with("terms", json!(g.terms))
        }
        Err(e) => {
            let needs = (e.exit == Exit::NeedsInput).then(|| e.fix.clone());
            let mut env = fail(e.exit, e.message, e.fix);
            if let Some(nodes) = e.detail {
                env = env.with("node_errors", nodes);
            }
            if let Some(action) = needs {
                env.needs_input = vec![
                    json!({"field": "generate.row", "kind": "dependency", "human_action": action}),
                ];
            }
            env.run_id = Some(run_id);
            env.plan_id = Some(plan.plan_id);
            env
        }
    }
}
