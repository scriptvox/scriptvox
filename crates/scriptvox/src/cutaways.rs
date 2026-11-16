//! Brief `cutaways` (G027): pictures over a base video, some of them generated, in one plan. Each generated cutaway is
//! one `generate` call; their stages and costs add up to one plan with one `plan_id`, one approval covers them all, and
//! `edit --broll` then lays the files over the base video. Its own sound plays on, captions stay on top.

use crate::brief::Cutaway;
use crate::envelope::Envelope;
use crate::exit::Exit;
use crate::plan::{self, Stage};
use serde_json::json;
use std::path::PathBuf;

pub struct Args {
    pub base_video: PathBuf,
    pub cutaways: Vec<Cutaway>,
    pub style: Option<String>,
    pub seed: Option<u64>,
    pub captions: Option<String>,
    pub hook: Option<String>,
    pub logo: Option<PathBuf>,
    pub accent: Option<String>,
    pub lang: Option<String>,
    pub out: Option<PathBuf>,
    pub approve: Option<String>,
    pub max_spend: Option<f64>,
    pub plan_only: bool,
    pub use_system: bool,
}

fn fail(exit: Exit, msg: impl Into<String>, fix: &str) -> Envelope {
    Envelope::fail("make", exit, msg.into(), Some(fix.into()))
}

/// The row a generated cutaway uses: its own `model`, else the cheapest ready row of the kind that fits the shape.
fn row_for(c: &Cutaway, aspect: &str) -> Result<String, String> {
    if let Some(m) = &c.model {
        return Ok(m.clone());
    }
    let kind = if c.still == Some(true) {
        "image.gen"
    } else {
        "video.gen"
    };
    crate::generators::all()
        .into_iter()
        .filter(|g| {
            g.kind == kind
                && g.ready()
                && !g.unknown_price()
                && g.usd_per_input_s == 0.0
                && g.aspects.iter().any(|x| x == aspect)
        })
        .min_by(|x, y| x.cost(1.0).total_cmp(&y.cost(1.0)))
        .map(|g| g.name)
        .ok_or_else(|| format!("no {kind} row with a key fits {aspect}"))
}

fn gen_args(
    c: &Cutaway,
    model: String,
    style: Option<&str>,
    aspect: &str,
    seed: Option<u64>,
) -> crate::generate::Args {
    let prompt = c.prompt.clone().unwrap_or_default();
    crate::generate::Args {
        kind: None,
        last_image: None,
        model,
        prompt: match style {
            Some(s) if !s.trim().is_empty() => format!("{prompt}, {s}"),
            _ => prompt,
        },
        audio: None,
        video: None,
        voice: None,
        lang: None,
        image: None,
        seconds: Some(c.seconds),
        aspect: aspect.into(),
        out: None,
        wait_s: 1800,
        approve: None,
        max_spend: None,
        plan_only: true,
        idempotency_key: None,
        seed,
    }
}

/// Checks that need no network: each cutaway has one source, the times fit the base video.
fn check(cuts: &[Cutaway], total: f64) -> Result<(), String> {
    for (i, c) in cuts.iter().enumerate() {
        match (&c.file, &c.prompt) {
            (Some(_), Some(_)) | (None, None) => {
                return Err(format!(
                    "cutaways[{i}]: give `file` or `prompt`, one of them"
                ))
            }
            (Some(f), None) if !f.is_file() => {
                return Err(format!("cutaways[{i}]: {} does not exist", f.display()))
            }
            (None, Some(p)) if p.trim().is_empty() => {
                return Err(format!("cutaways[{i}]: the prompt is empty"))
            }
            _ => {}
        }
    }
    let specs: Vec<String> = cuts
        .iter()
        .map(|c| format!("cutaway@{}:{}", c.at, c.seconds))
        .collect();
    crate::edit::parse_broll(&specs, total).map(|_| ())
}

pub fn run(a: Args) -> Envelope {
    let files = std::iter::once(&a.base_video)
        .chain(a.logo.iter())
        .chain(a.cutaways.iter().filter_map(|c| c.file.as_ref()));
    if let Some(env) = crate::media_gate::refuse("make", files) {
        return env;
    }
    let ff = match crate::engine_ffmpeg::Ffmpeg::locate(a.use_system, false) {
        Ok(f) => f,
        Err(e) => {
            return fail(
                Exit::NeedsInput,
                e,
                "Run `scriptvox setup` to install FFmpeg.",
            )
        }
    };
    let (Ok(total), Some((w, h))) = (
        ff.video_duration(&a.base_video),
        ff.shown_size(&a.base_video),
    ) else {
        return fail(
            Exit::Usage,
            format!("cannot read the base video {}", a.base_video.display()),
            "Check the path: base_video must be a video FFmpeg can read.",
        );
    };
    if let Err(why) = check(&a.cutaways, total) {
        return fail(
            Exit::Usage,
            why,
            "Each cutaway needs `at` and `seconds` (0.5 s or more), in order, no overlap, inside the base video.",
        );
    }
    // Rows are asked for 9:16 or 16:9, and `edit` crops each picture to fill the frame.
    let aspect = if w >= h { "16:9" } else { "9:16" };
    // One plan: every generated cutaway is a `generate` plan, priced the same way.
    let mut stages = vec![];
    let mut subs = vec![];
    let mut cost = 0.0;
    for (i, c) in a.cutaways.iter().enumerate() {
        if c.file.is_some() {
            continue;
        }
        let model = match row_for(c, aspect) {
            Ok(m) => m,
            Err(why) => {
                return fail(
                    Exit::NeedsInput,
                    format!("cutaways[{i}]: {why}"),
                    "Give `model` with a row from `scriptvox providers`, or `file`.",
                )
            }
        };
        let p = crate::generate::run(gen_args(
            c,
            model.clone(),
            a.style.as_deref(),
            aspect,
            a.seed,
        ));
        if !p.ok {
            let mut p = p;
            p.command = "make".into();
            return p;
        }
        let c_usd = p
            .metrics
            .get("cost_usd")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        cost += c_usd;
        if let Some(s) = p.extra.get("stages").and_then(|s| s.as_array()) {
            for s in s {
                let mut s = s.clone();
                s["stage"] = json!(format!("cutaway{i}"));
                stages.push(s);
            }
        }
        subs.push((i, model, p.plan_id.unwrap_or_default()));
    }
    stages.push(json!(Stage {
        stage: "edit",
        tool: "edit --broll".into(),
        cost_usd: 0.0,
        network: false,
        explain: format!(
            "{} cutaways over {}; its own sound stays{}",
            a.cutaways.len(),
            a.base_video.display(),
            if a.captions.is_some() {
                ", captions on top from its own speech"
            } else {
                ""
            }
        ),
    }));
    let files: Vec<_> = a
        .cutaways
        .iter()
        .map(|c| {
            c.file
                .as_ref()
                .and_then(|f| crate::store::hash_file(f).ok())
        })
        .collect();
    let plan_id = plan::plan_id(&json!({
        "command": "cutaways",
        "base": crate::store::hash_file(&a.base_video).ok(),
        "times": a.cutaways.iter().map(|c| [c.at, c.seconds]).collect::<Vec<_>>(),
        "subs": subs.iter().map(|s| &s.2).collect::<Vec<_>>(),
        "files": files,
        "captions": a.captions, "hook": a.hook, "logo": a.logo.as_ref().and_then(|l| crate::store::hash_file(l).ok()),
    }));
    let approve_line = format!("scriptvox make --brief <brief> --approve {plan_id}");
    if a.plan_only || a.approve.as_deref() != Some(plan_id.as_str()) {
        let command = if a.plan_only { "plan" } else { "make" };
        if let Some(env) = plan::over_cap(command, &plan_id, cost, a.max_spend, vec![]) {
            return env.with("stages", json!(stages));
        }
    }
    if a.plan_only {
        let mut env = Envelope::ok("plan");
        env.plan_id = Some(plan_id);
        env.next = vec![approve_line];
        env.metrics.insert("cost_usd".into(), json!(cost));
        return env.with("stages", json!(stages));
    }
    if let Some(why) = plan::needs_approval(&plan_id, cost, a.approve.as_deref()) {
        let mut env = fail(
            Exit::ApprovalNeeded,
            format!(
                "approval needed: {} (${cost:.2})",
                why["reason"].as_str().unwrap_or("")
            ),
            &format!("Run the same command with --approve {plan_id}"),
        );
        env.status = "approval_needed";
        env.plan_id = Some(plan_id);
        env.next = vec![approve_line];
        env.metrics.insert("cost_usd".into(), json!(cost));
        return env.with("stages", json!(stages));
    }
    // Approved as a whole: each part runs on its own plan id, so a rerun resumes its provider job, not a new one.
    let mut spent = 0.0;
    let mut made: Vec<(usize, PathBuf)> = vec![];
    for (i, model, sub) in subs {
        let c = &a.cutaways[i];
        let mut g = gen_args(c, model, a.style.as_deref(), aspect, a.seed);
        g.plan_only = false;
        g.approve = Some(sub);
        g.max_spend = a.max_spend.map(|m| (m - spent).max(0.0));
        let env = crate::generate::run(g);
        if !env.ok {
            let mut env = env;
            env.command = "make".into();
            return env.with("failed_cutaway", json!(i));
        }
        spent += env
            .metrics
            .get("cost_usd")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let file = env
            .artifacts
            .get("video")
            .or(env.artifacts.get("image"))
            .and_then(|v| v.as_str())
            .map(PathBuf::from);
        match file {
            Some(f) => made.push((i, f)),
            None => {
                return fail(
                    Exit::Error,
                    format!("cutaways[{i}]: the row gave no file"),
                    "Run make again.",
                )
            }
        }
    }
    let broll: Vec<String> = a
        .cutaways
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let f = c
                .file
                .clone()
                .or_else(|| made.iter().find(|m| m.0 == i).map(|m| m.1.clone()))
                .unwrap_or_default();
            format!("{}@{}:{}", f.display(), c.at, c.seconds)
        })
        .collect();
    let mut env = crate::edit::run(edit_args(&a, broll));
    env.command = "make".into();
    env.plan_id = Some(plan_id);
    env.metrics.insert("cost_usd".into(), json!(spent));
    let list: Vec<_> = made
        .iter()
        .map(|(i, f)| json!({"cutaway": i, "file": f}))
        .collect();
    env.with("generated", json!(list))
}

fn edit_args(a: &Args, broll: Vec<String>) -> crate::edit::Args {
    crate::edit::Args {
        input: a.base_video.clone(),
        broll,
        captions: a.captions.clone(),
        hook: a.hook.clone(),
        logo: a.logo.clone(),
        accent: a.accent.clone(),
        lang: a.lang.clone(),
        out: a.out.clone(),
        use_system: a.use_system,
        terms: None,
        label: None,
        keep_words: None,
        bleep: None,
        bleep_with: None,
        bleep_pad: None,
        words: None,
        cut_retakes: false,
        clean_voice: None,
        zoom: vec![],
        captions_file: None,
        captions_second: None,
        text: None,
        fit: None,
        zoom_on_emphasis: false,
        plan: false,
        cut_silence: false,
        min_silence: 0.8,
        pad: 0.15,
        noise_db: -35.0,
        reframe: None,
        track: false,
        crop_text: false,
        cut_fillers: false,
        subject: None,
        keep: None,
        speed: None,
        duration: None,
        cut_still: false,
        blur: vec![],
        from: None,
        to: None,
        emphasis: None,
        music: None,
        background: None,
        alpha: false,
        enhance: false,
        max_spend: None,
        music_level: None,
        sfx: vec![],
        sfx_level: None,
        speed_ranges: None,
        small: Default::default(),
        // ponytail: a brief with `cutaways` does not carry the caption style fields yet; add them to `Args` here when asked.
        caption_style: Default::default(),
        music_cut: (None, None),
        lipsync: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut(at: f64, seconds: f64, file: Option<&str>, prompt: Option<&str>) -> Cutaway {
        Cutaway {
            at,
            seconds,
            file: file.map(PathBuf::from),
            prompt: prompt.map(String::from),
            model: None,
            still: None,
        }
    }

    #[test]
    fn check_refuses_bad_cutaways() {
        let here = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        assert!(check(&[cut(1.0, 2.0, None, Some("a road"))], 10.0).is_ok());
        assert!(check(&[cut(1.0, 2.0, Some(here), None)], 10.0).is_ok());
        for bad in [
            vec![cut(1.0, 2.0, None, None)],
            vec![cut(1.0, 2.0, Some(here), Some("x"))],
            vec![cut(1.0, 2.0, Some("no/such.mp4"), None)],
            vec![cut(1.0, 2.0, None, Some(" "))],
            vec![cut(9.0, 2.0, None, Some("x"))],
            vec![cut(1.0, 0.2, None, Some("x"))],
            vec![
                cut(1.0, 3.0, None, Some("x")),
                cut(2.0, 1.0, None, Some("y")),
            ],
        ] {
            assert!(check(&bad, 10.0).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn style_goes_on_the_prompt() {
        let c = cut(0.0, 2.0, None, Some("a road"));
        assert_eq!(
            gen_args(&c, "m".into(), Some("ink"), "16:9", None).prompt,
            "a road, ink"
        );
        assert_eq!(
            gen_args(&c, "m".into(), None, "16:9", None).prompt,
            "a road"
        );
    }
}
