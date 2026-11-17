use clap::error::ErrorKind;
use clap::{ColorChoice, CommandFactory, FromArgMatches};
use scriptvox::agent_mode;
use scriptvox::cli::{Cli, Cmd, KeysCmd, MediaCmd, SkillsCmd};
use scriptvox::envelope::Envelope;
use scriptvox::exit::Exit;
use scriptvox::redact::Redactor;
use serde_json::json;
use std::io::IsTerminal;
use std::process::ExitCode;

/// Internal RVX-RPC protocol version (PLAN-R1 §2.4).
const PROTO: &str = "1.0";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().collect();
    // `scriptvox plan ...` is `scriptvox make ... --plan`.
    if args.get(1).is_some_and(|a| a == "plan") {
        args[1] = "make".into();
        args.push("--plan".into());
    }
    let agent = agent_mode::from_env();
    let json_mode = agent || args.iter().any(|a| a == "--json") || !std::io::stdout().is_terminal();
    if args.len() == 1 && !json_mode {
        // A bare `scriptvox` from a person gets three ways in, not an error.
        println!("Start with one of these:\n  scriptvox demo                 make a short test video\n  scriptvox make --script s.txt  your own text in a file\n  scriptvox doctor               check this machine");
        return ExitCode::SUCCESS;
    }
    // No ANSI for agents or pipes; humans in a terminal keep clap's colours.
    let color = if json_mode {
        ColorChoice::Never
    } else {
        ColorChoice::Auto
    };
    let parsed = scriptvox::cli::command()
        .color(color)
        .try_get_matches_from(&args)
        .and_then(|m| Cli::from_arg_matches(&m))
        // ponytail: `plan` is an alias of `make`; clap does not say which name was typed, so read the first word.
        .map(|mut cli| {
            cli.plan |= args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with('-'))
                .is_some_and(|a| a == "plan");
            cli
        });

    let env = match parsed {
        Ok(cli) => {
            scriptvox::plan::set_approve(cli.approve.clone());
            scriptvox::plan::set_local_only(cli.local_only);
            run(cli, agent)
        }
        // Explicit --help always prints help. A missing subcommand prints help
        // only for a human; an agent or a pipe gets a usage envelope.
        Err(e)
            if e.kind() == ErrorKind::DisplayHelp
                || (!json_mode
                    && e.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand) =>
        {
            let _ = e.print();
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            // Clap's own first line names the bad argument, and its tip names the right one (test 5, 5 Nov 2026:
            // `snapshot --sheets 9` said only "unexpected argument found").
            let text = e.to_string();
            let first = text
                .lines()
                .next()
                .unwrap_or("a subcommand is missing")
                .trim_start_matches("error: ");
            // "the following required arguments were not provided:" names them on the next lines.
            let named: Vec<&str> = text
                .lines()
                .skip(1)
                .map(str::trim)
                .take_while(|l| !l.is_empty())
                .collect();
            let msg = match text.lines().map(str::trim).find(|l| l.starts_with("tip:")) {
                Some(tip) => format!("{first}; {tip}"),
                None if first.ends_with(':') => format!("{first} {}", named.join(", ")),
                None => first.to_string(),
            };
            let command = args.get(1).cloned().unwrap_or_default();
            Envelope::fail(
                &command,
                Exit::Usage,
                msg,
                Some(
                    if command.is_empty()
                        || command.starts_with('-')
                        || Cli::command().find_subcommand(&command).is_none()
                    {
                        "Run `scriptvox --help` to see the commands.".into()
                    } else {
                        format!("Run `scriptvox {command} --help` to see its usage.")
                    },
                ),
            )
        }
    };
    // A supervisor (`make --run-id X`, `_run X`) whose run stopped before it wrote a result (a check before the run
    // folder is used, such as a script far too long for its duration): record it, or `wait` reports a dead
    // supervisor instead of the real error (4 Nov 2026, found recording the §4 session).
    let supervised = args
        .windows(2)
        .find(|w| w[0] == "--run-id" || w[0] == "_run")
        .map(|w| w[1].clone());
    let mut env = env;
    if let Some(id) = supervised {
        if let Ok(run) = scriptvox::run::Run::attach(std::path::Path::new("."), &id) {
            if !run.dir.join("result.json").exists() {
                env.run_id = Some(id);
                run.finish(std::path::Path::new("."), &env);
            }
        }
    }
    if env.command == "make" && std::env::args().nth(1).is_some_and(|a| a == "plan") {
        env.command = "plan".into();
    }
    name_run(&mut env, &args);
    default_next(&mut env);
    env.ask_before_paying();
    emit(&env, json_mode, &Redactor::from_env());
    env.exit.into()
}

/// `snapshot`, `judge`, `export`, `check` on a run: the result names the run (test 2, 4 Nov 2026: run_id was null).
fn name_run(env: &mut Envelope, args: &[String]) {
    if env.run_id.is_some()
        || !matches!(
            env.command.as_str(),
            "snapshot" | "judge" | "export" | "check"
        )
    {
        return;
    }
    if let Some(id) = args
        .iter()
        .skip(2)
        .find(|a| !a.starts_with('-') && std::path::Path::new(".scriptvox/runs").join(a).is_dir())
    {
        env.run_id = Some(id.clone());
    }
}

/// The usual next commands when a command left `next` empty, so an agent always has a step (4 Nov 2026 eval:
/// `next` was empty after plan, wait and edit, though the skill says to read it).
fn default_next(env: &mut Envelope) {
    if !env.ok || !env.next.is_empty() {
        return;
    }
    let video = env
        .artifacts
        .get("video")
        .and_then(|v| v.as_str())
        .map(String::from);
    env.next = match (env.command.as_str(), &env.run_id, &env.plan_id) {
        ("make" | "plan", None, Some(p)) => {
            let mut steps = vec!["Make it: the same brief with `scriptvox make` (or without --plan). Free plans need no approval.".to_string()];
            if env.metrics.get("cost_usd").and_then(|c| c.as_f64()).is_some_and(|c| c > 0.0) {
                steps.push(format!("It costs money: add --approve {p}."));
            }
            steps
        }
        ("make" | "resume", Some(id), _) if video.is_none() => {
            vec![format!("scriptvox wait {id} --json")]
        }
        ("make" | "wait" | "resume", Some(id), _) if env.metrics.contains_key("draft") => vec![
            format!("scriptvox snapshot {id} --sheet 9 --json"),
            format!("When the frames look right: scriptvox resume {id} --final --json"),
        ],
        ("make" | "wait" | "resume", Some(id), _) => {
            let mut steps = vec![format!("scriptvox snapshot {id} --sheet 9 --json")];
            if scriptvox::judge::vision_url().is_some() {
                steps.push(format!("scriptvox judge {id} --json"));
            }
            steps.push(format!("scriptvox export {id} --json"));
            steps
        }
        ("snapshot", _, _) => {
            let after = env.run_id.as_ref().filter(|_| scriptvox::judge::vision_url().is_some()).map_or(String::new(), |id| {
                format!(", then `scriptvox judge {id} --json`")
            });
            vec![format!("Read the PNG at artifacts.image{after}.")]
        }
        ("check", Some(id), _) => vec![format!("scriptvox snapshot {id} --sheet 9 --json")],
        ("analyze", _, _) => vec![
            "Read artifacts.shots_image. To copy this style: `scriptvox analyze <video> --like new.json --json`.".into(),
            "To change this video: `scriptvox edit <video> ... --json`.".into(),
        ],
        ("batch", _, _) => vec!["For each row's run: `scriptvox snapshot <run_id> --sheet 9 --json`.".into()],
        ("setup", _, _) if env.extra.contains_key("voice_rust") => vec!["scriptvox make --script script.txt --tts kokoro-rust".into()],
        ("setup", _, _) if env.extra.contains_key("voice_local") => vec!["scriptvox make --script script.txt --tts kokoro".into()],
        ("setup", _, _) if env.extra.contains_key("speech_local") || env.extra.contains_key("speech_large") => vec!["scriptvox check <video> --hear".into()],
        ("setup", _, _) if env.extra.contains_key("image_local") => vec!["scriptvox generate --model flux2-klein-4b-local --prompt \"A desert at dawn\"".into()],
        ("setup", _, _) if env.extra.contains_key("notice") => vec![if env.extra.contains_key("dry_run") { "scriptvox setup fetch" } else { "scriptvox fetch <url>" }.into()],
        ("doctor" | "setup", _, _) => vec!["scriptvox demo --json".into()],
        ("demo", _, _) => vec!["Read `scriptvox skills render` or `scriptvox --help`, write a brief, then `scriptvox plan --brief brief.json --json`.".into()],
        ("templates", _, _) => vec!["Run the `command` of the template you pick.".into()],
        ("recipes", _, _) if env.extra.contains_key("recipe") => vec!["Save `recipe.brief` as brief.json and change the words (`recipe.adapt`), then `scriptvox plan --brief brief.json --json`.".into()],
        ("recipes", _, _) => vec!["Run the `command` of the recipe you pick. The skill scriptvox-genres says which kind fits.".into()],
        ("voices" | "tools" | "providers", _, _) => {
            vec!["Name your pick in a brief, then `scriptvox plan --brief brief.json --json`.".into()]
        }
        // The list has run ids and no step of its own: one run's events are the usual next look.
        ("status", None, _) => vec!["scriptvox status <run_id> --json for one run's last events.".into()],
        ("explain", _, _) => vec!["scriptvox remake <video> --json makes it again; `scriptvox replay <video> --json` uses the cache only.".into()],
        ("export", _, _) => vec![
            "Upload artifacts.video with the platform text in artifacts.metadata (nothing was uploaded).".into(),
        ],
        ("edit", _, _) => video
            .map(|v| {
                vec![
                    format!("scriptvox snapshot {v} --sheet 9 --json"),
                    format!("scriptvox check {v} --json"),
                ]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
}

fn run(mut cli: Cli, agent: bool) -> Envelope {
    if cli.version {
        return Envelope::ok("version")
            .with("version", json!(env!("CARGO_PKG_VERSION")))
            .with("proto", json!(PROTO))
            .with(
                "ffmpeg_pin",
                json!(scriptvox::provision::pin("ffmpeg").map(|p| p.version)),
            );
    }
    // Take the command out, so arms can still borrow the global flags in `cli`.
    match cli.command.take() {
        Some(Cmd::Status { id: None }) => Envelope::ok("status")
            .with("runs", json!(scriptvox::run::list(std::path::Path::new("."), 20)))
            .with("agent_mode", json!(agent)),
        Some(Cmd::Status { id: Some(id) }) => {
            let dir = std::path::Path::new(".scriptvox/runs").join(&id);
            let events: Vec<serde_json::Value> = std::fs::read_to_string(dir.join("events.jsonl"))
                .unwrap_or_default()
                .lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect();
            match scriptvox::run::list(std::path::Path::new("."), 1000).into_iter().find(|r| r["id"] == id.as_str()) {
                Some(r) => {
                    // Plain words for "is it still going": the worker process lives, where it is, and how long it has been quiet.
                    let project = std::path::Path::new(".");
                    let alive = scriptvox::supervise::running(project, &id);
                    let state = r["status"].as_str().unwrap_or("").to_string();
                    let mut env = Envelope::ok("status").with("run", r).with("events", json!(events[events.len().saturating_sub(10)..])).with("alive", json!(alive));
                    if alive || state == "died" {
                        let (stage, idle) = scriptvox::supervise::progress(project, &id);
                        env = env.with("stage", json!(stage)).with("idle_s", json!(idle));
                        if alive {
                            env.next = vec![format!("scriptvox wait {id} --json")];
                        } else if scriptvox::supervise::resumable(project, &id) {
                            env.next = vec![format!("scriptvox resume {id} --json")];
                        }
                    }
                    run_metrics(&mut env, &id);
                    env.run_id = Some(id);
                    env
                }
                None => Envelope::fail("status", Exit::Usage, format!("no run {id}"), Some("Run `scriptvox status` to see the runs.".into())),
            }
        }
        Some(Cmd::Generate { model, kind, prompt, audio, video, voice, lang, image, seconds, aspect, out, wait_s, seed, last_image, detach, run_id }) => {
            if detach && run_id.is_none() && !cli.plan && !cfg!(windows) {
                if let Some(env) = detach_generate(cli.approve.as_deref(), cli.idempotency_key.as_deref()) {
                    return env;
                }
            }
            scriptvox::generate::run(scriptvox::generate::Args {
            seed,
            last_image,
            model,
            kind,
            prompt: prompt.unwrap_or_default(),
            audio,
            video,
            voice,
            lang,
            image,
            seconds,
            aspect,
            out,
            wait_s,
            approve: cli.approve.clone(),
            max_spend: cli.max_spend,
            plan_only: cli.plan,
            idempotency_key: cli.idempotency_key.clone(),
            })
        }
        Some(Cmd::Batch { brief, rows, jobs, describe }) => match rows {
            Some(rows) if !describe => scriptvox::batch::run(scriptvox::batch::Args { brief, rows, jobs, local_only: cli.local_only }),
            _ => scriptvox::batch::describe(&brief),
        },
        Some(Cmd::Mcp { read_root, write_root }) => match scriptvox::mcp::serve(&read_root, &write_root) {
            Ok(()) => std::process::exit(0),
            Err(e) => Envelope::fail("mcp", Exit::Error, format!("mcp: {e}"), Some("Each --read-root and --write-root must be a folder that exists.".into())),
        },
        Some(Cmd::Export { run, dir, gif, title, tags, format, overlay, timeline, stems, max_mb, soft_subs, cover_prompt, cover_model, check_links }) => { let links = check_links.then(|| run.clone()); let env = scriptvox::export::run(scriptvox::export::Args { run, dir, gif, title, tags, formats: format, overlay, timeline, stems, max_mb, soft_subs, use_system: cli.use_system,
            cover: cover_prompt.map(|prompt| scriptvox::export::Cover { prompt, model: cover_model, approve: cli.approve, max_spend: cli.max_spend, plan_only: cli.plan, idempotency_key: cli.idempotency_key }) });
            // R9: behind a flag, because it needs the network.
            match links { Some(run) if env.ok && !cli.plan => scriptvox::research::link_check(env, &run), _ => env } }
        Some(Cmd::Analyze { input, threshold, transcript, beats, speakers, lang, like, terms }) => scriptvox::analyze::run(scriptvox::analyze::Args {
            beats,
            input,
            threshold,
            transcript,
            speakers,
            lang,
            like,
            terms: match load_terms("analyze", terms) {
                Ok(t) => t,
                Err(e) => return *e,
            },
            use_system: cli.use_system,
        }),
        Some(Cmd::Judge { target, brief, frames, endpoint, model, min_score }) => judge(target, brief, frames, endpoint, model, min_score, cli.use_system),
        Some(Cmd::Clips { input, count, min, max, aspect, track, captions, caption_style, lang, plan, terms }) => scriptvox::clips::run(scriptvox::clips::Args {
            caption_style,
            terms: match load_terms("clips", terms) {
                Ok(t) => t,
                Err(e) => return *e,
            },
            input,
            count,
            min,
            max,
            aspect,
            track,
            captions,
            lang,
            plan,
            use_system: cli.use_system,
        }),
        Some(Cmd::Fetch {
            url,
            audio,
            from,
            to,
            out,
            #[cfg(feature = "fetch-youtube")]
            quality,
        }) => scriptvox::fetch::run(scriptvox::fetch::Args {
            url,
            audio,
            from,
            to,
            out,
            dir: None,
            #[cfg(feature = "fetch-youtube")]
            quality,
            use_system: cli.use_system,
        }),
        Some(Cmd::Compare { a, b, threshold }) => scriptvox::compare::run(scriptvox::compare::Args { a, b, threshold, use_system: cli.use_system }),
        Some(Cmd::Join { inputs, transition, aspect, fit, out }) => scriptvox::join::run(scriptvox::join::Args {
            inputs,
            transition,
            aspect,
            fit,
            out,
            use_system: cli.use_system,
        }),
        Some(Cmd::Edit { input, cut_silence, cut_still, min_silence, pad, noise_db, reframe, fit, track, crop_text, cut_fillers, subject, keep, keep_words, bleep, bleep_with, bleep_pad, words, clean_voice, captions_file, captions_second, text, broll, zoom, zoom_on_emphasis, cut_retakes, speed, duration, blur, from, to, captions, lang, hook, label, emphasis, music, music_from, music_to, caption_style, music_level, sfx, sfx_level, remove_bars, hold, r#loop, loop_to, deinterlace, tilt, lipsync, logo, accent, background, alpha, enhance, out, terms }) => scriptvox::edit::run(scriptvox::edit::Args {
            small: scriptvox::edit_small::Ops { remove_bars, hold, loop_n: r#loop, loop_to, deinterlace, tilt },
            caption_style,
            music_cut: (music_from, music_to),
            background,
            alpha,
            enhance,
            max_spend: cli.max_spend,
            cut_still,
            duration,
            blur,
            terms: match load_terms("edit", terms) {
                Ok(t) => t,
                Err(e) => return *e,
            },
            input,
            cut_silence,
            min_silence,
            pad,
            noise_db,
            reframe,
            fit,
            track,
            crop_text,
            cut_fillers,
            subject,
            keep,
            keep_words,
            bleep,
            bleep_with,
            bleep_pad,
            words,
            clean_voice,
            captions_file,
            captions_second,
            text,
            broll,
            zoom,
            zoom_on_emphasis,
            cut_retakes,
            plan: cli.plan,
            // A plain number is the whole video's speed; anything else is read as ranges (`edit-speed-range.md`).
            speed: speed.as_deref().and_then(|s| s.trim().parse().ok()),
            speed_ranges: speed.filter(|s| s.trim().parse::<f64>().is_err()),
            from,
            to,
            captions,
            lang,
            hook,
            label,
            emphasis,
            music,
            music_level,
            sfx,
            sfx_level,
            lipsync,
            logo,
            accent,
            out,
            use_system: cli.use_system,
        }),
        Some(Cmd::Demo { out, golden }) => demo(out, golden, cli.use_system),
        Some(Cmd::Worker) => worker(),
        Some(Cmd::Dub { input, lines, caption_style, lang, tts, voice, base_sound, lipsync, speakers, voices, captions_second, out, terms }) => {
            use scriptvox::dub;
            let terms = match load_terms("dub", terms) {
                Ok(t) => t,
                Err(e) => return *e,
            };
            let Some(lines) = lines else {
                if !cli.plan {
                    return Envelope::fail("dub", Exit::Usage, "dub needs --plan for the sheet, or --lines to run", Some("Run `scriptvox dub <video> --plan` first.".into()));
                }
                return dub::sheet(&input, lang.as_deref(), terms.as_ref(), speakers, cli.use_system);
            };
            if out.as_deref().is_some_and(|o| scriptvox::engine_ffmpeg::same_file(o, &input)) {
                return Envelope::fail("dub", Exit::Usage, format!("--out is the input file: {}", input.display()), Some("Give another --out: a dub cannot write over the video it reads.".into()));
            }
            if lipsync && (cli.local_only || dub::lipsync_row().is_none()) {
                return Envelope::fail("dub", Exit::NeedsInput, "no free lip sync row with a key on this install, or --local-only is on", Some("Drop --lipsync. `scriptvox providers` lists what this install has.".into()));
            }
            let (plan_only, use_system) = (cli.plan, cli.use_system);
            let brief = dub::brief(&dub::Brief { input: &input, lines: &lines, lang: lang.as_deref(), tts: tts.as_deref(), voice: voice.as_deref(), voices: voices.as_deref(), captions_second: captions_second.as_deref(), caption_style: &caption_style, base_sound: base_sound.as_deref(), local_only: cli.local_only, use_system });
            let brief = match brief {
                Ok(b) => b,
                Err(e) => return *e,
            };
            cli.command = Some(Cmd::Make { topic: None, from_topic: None, script: None, brief: Some(brief), lang: None, tts: None, voice: None, out, duration: None, aspect: None, media: None, stock: None, music: None, music_from: None, music_to: None, hook: None, draft: false, until: None, storyboard: false, detach: false, strict: false, run_id: None });
            let mut env = run(cli, agent);
            dub::check_terms(&mut env, &lines, terms.as_ref());
            if plan_only {
                let mut env = env.with("lipsync", json!(lipsync));
                env.command = "dub".into();
                return env;
            }
            dub::finish(env, &input, lipsync, use_system)
        }
        Some(Cmd::Make {
            topic,
            from_topic,
            script,
            brief,
            lang,
            tts,
            voice,
            out,
            duration,
            aspect,
            media,
            stock,
            music,
            music_from,
            music_to,
            hook,
            draft,
            until,
            storyboard,
            detach,
            strict,
            run_id,
        }) => {
            if let Some(env) = scriptvox::make::until_error(until.as_deref()) {
                return env;
            }
            // `make --topic`: research and write give the brief, then all goes on as `make --brief` (docs/specs/make-topic.md).
            let brief = match from_topic {
                Some(topic) => match scriptvox::topic::brief(scriptvox::topic::Args { topic, lang: lang.clone(), duration, tts: tts.clone(), plan_only: cli.plan, max_spend: cli.max_spend, local_only: cli.local_only }) {
                    Ok(p) => Some(p),
                    Err(env) => return *env,
                },
                None => brief,
            };
            if detach && run_id.is_none() && !cfg!(windows) {
                return detach_make(cli.idempotency_key.as_deref(), cli.approve.as_deref(), cli.max_spend);
            }
            // `scriptvox make brief.json`, `scriptvox plan script.txt`: a file named as the topic is what was meant.
            let named = topic.as_deref().filter(|_| brief.is_none() && script.is_none()).map(std::path::Path::new);
            let (brief, script, topic) = match named {
                Some(p) if p.is_file() && p.extension().is_some_and(|e| e == "json") => (Some(p.to_path_buf()), None, None),
                // Only a text file: a topic word that is also a file name here stays a topic.
                Some(p) if p.is_file() && p.extension().is_some_and(|e| e == "txt" || e == "md") => (None, Some(p.to_path_buf()), None),
                // One word that ends like a file and is not there: a mistyped path. "Explain package.json" stays a topic.
                Some(p) if !p.to_string_lossy().contains(char::is_whitespace) && p.extension().is_some_and(|e| e == "json" || e == "txt" || e == "md") => {
                    return Envelope::fail("make", Exit::Usage, format!("no such file: {}", p.display()), Some("Check the path, or run `scriptvox schema brief` to write a brief.".into()));
                }
                _ => (brief, script, topic),
            };
            let b = match &brief {
                Some(p) => match scriptvox::brief::Brief::load(p) {
                    Ok(b) => b,
                    Err(e) => {
                        let fix = scriptvox::brief::load_fix(&e, p);
                        return Envelope::fail("make", Exit::Usage, e, Some(fix));
                    }
                },
                None => scriptvox::brief::Brief::default(),
            };
            if let Some(p) = scriptvox::brief::file_outside_project(&b) {
                return outside_project("make", &p);
            }
            if let (false, Some(base_video)) = (b.cutaways.is_empty(), b.base_video.clone()) {
                if script.is_some() {
                    return Envelope::fail("make", Exit::Usage, "cutaways keep the base video's own sound, so --script does not go with them", Some("Drop --script, or drop cutaways.".into()));
                }
                if until.is_some() {
                    return Envelope::fail("make", Exit::Usage, "cutaways make no voice, so --until voice has nothing to stop after", Some("Drop --until, or drop cutaways.".into()));
                }
                return scriptvox::cutaways::run(scriptvox::cutaways::Args {
                    base_video,
                    cutaways: b.cutaways.clone(),
                    style: b.style.clone(),
                    seed: b.seed,
                    captions: b.captions.clone(),
                    hook: hook.or(b.hook),
                    logo: b.logo.as_ref().map(|l| l.file.clone()),
                    accent: b.accent.clone(),
                    lang: lang.or(b.lang),
                    out,
                    approve: cli.approve.clone(),
                    max_spend: cli.max_spend,
                    plan_only: cli.plan,
                    use_system: cli.use_system,
                });
            }
            if script.is_none() && brief.is_none() {
                // v0.1: the calling agent writes the script (PLAN-R1 D3). A topic alone asks for it.
                let mut env = Envelope::fail("make", Exit::NeedsInput, "a script is needed: scriptvox does not write scripts in v0.1", None);
                let about = topic.unwrap_or_default();
                env.needs_input = vec![json!({
                    "field": "script", "kind": "script",
                    "human_action": format!("Write the narration{} into script.txt, then run `scriptvox make --script script.txt` (an agent can write brief.json instead: `scriptvox schema brief`).", if about.is_empty() { String::new() } else { format!(" about \"{about}\"") }),
                    "alternatives": [{"args": ["--script", "script.txt"], "cost_usd": 0, "note": "a plain text file with the narration"}],
                })];
                env.next = vec!["scriptvox schema brief --json".into()];
                return env;
            }
            let script_path = script.or(b.script_file.clone());
            if b.lines.is_some() && script_path.is_some() {
                return Envelope::fail("make", Exit::Usage, "lines replaces --script", Some("Keep lines, or drop --script.".into()));
            }
            let text = match scriptvox::brief::script_text(script_path.as_deref(), &b) {
                Ok(Some(t)) => t,
                // A brief with `duration` alone: a video with no words (docs/specs/no-words.md).
                Ok(None) if b.duration.is_some() => String::new(),
                Ok(None) => return Envelope::fail("make", Exit::Usage, "no script", Some("Pass --script f or --brief f.".into())),
                Err(e) => return Envelope::fail("make", Exit::Usage, e, None),
            };
            // Gate `claims` (docs/specs/research.md): only for a brief with `sources`. With --strict no work starts.
            let mut said = serde_json::to_value(&b).unwrap_or_default();
            if let Some(h) = &hook {
                said["hook"] = json!(h);
            }
            let claim = scriptvox::claims::gate_of(&said, &text, strict);
            if claim.as_ref().is_some_and(|f| f.status == "fail") {
                return scriptvox::claims::apply(Envelope::ok("make"), claim);
            }
            let mut env = scriptvox::make::run(scriptvox::make::Args {
                script: text,
                lang: lang.or(b.lang),
                // A recording is the voice: no engine speaks.
                tts: if b.narration.is_some() { Some("none".into()) } else { tts.or(b.tts) },
                narration: b.narration.clone(),
                voice: voice.or(b.voice),
                local_only: cli.local_only,
                out,
                duration: duration.or(b.duration),
                aspect: aspect.or(b.aspect).unwrap_or_else(|| "9:16".into()),
                fps: b.fps,
                draft,
                until,
                storyboard,
                media: media.or(b.media),
                stock: stock.or(b.stock.map(|v| v.join(","))),
                music: music.or(b.music),
                music_from: music_from.or(b.music_from),
                music_to: music_to.or(b.music_to),
                music_prompt: b.music_prompt.clone(),
                music_search: b.music_search.clone(),
                sources: b.sources.clone(),
                base_video: b.base_video.clone(),
                base_sound: b.base_sound.clone(),
                say: b.say.clone(),
                lines: b.lines.clone().unwrap_or_default(),
                hook: hook.or(b.hook),
                sfx: b.sfx.unwrap_or(false),
                cut_on: b.cut_on.clone(),
                upscale: b.upscale.unwrap_or(false),
                smooth: b.smooth.unwrap_or(false),
                ambience: b.ambience.unwrap_or(false),
                logo: b.logo.clone(),
                accent: b.accent.clone(),
                music_level: b.music_level.clone(),
                background: b.background.clone(),
                waveform: b.waveform.unwrap_or(false),
                style: b.style.clone(),
                seed: b.seed,
                character: b.character.clone(),
                characters: b.characters.clone(),
                emphasis: b.emphasis.clone(),
                captions_second: b.captions_second.clone(),
                media_match: b.media_match.unwrap_or(false),
                font: b.font.clone(),
                caption_at: b.caption_at.clone(),
                caption_size: b.caption_size.clone(),
                card_style: b.card_style.clone(),
                size: b.size,
                caption_outline: b.caption_outline,
                caption_box: b.caption_box,
                loop_seam: b.r#loop.unwrap_or(false),
                voice_rate: b.voice_rate,
                outro: b.outro.clone(),
                captions: b.captions.clone(),
                transition: b.transition.clone(),
                look: b.look.clone(),
                scenes: b.scenes.clone(),
                run_id,
                use_system: cli.use_system,
                approve: cli.approve.clone(),
                max_spend: cli.max_spend,
                plan_only: cli.plan,
                idempotency_key: cli.idempotency_key.clone(),
                parent_run: None,
                pace: None,
                no_embed: cli.no_embed,
                cache_only: false,
                stock_picks: None,
            });
            if detach && cfg!(windows) {
                // Flip rule (d): breakaway is untested on Windows, so the run stayed in the foreground.
                env = env.with("warning", json!("--detach is not available on Windows in v0.1; the run ran in the foreground"));
            }
            scriptvox::claims::apply(env, claim)
        }
        Some(Cmd::Resume { id, brief, patch, duration, final_video, wait, only_scene, .. }) => resume(&id, brief, patch, duration, final_video, wait, only_scene, cli.use_system, cli.approve.clone(), cli.max_spend, cli.plan),
        Some(Cmd::RunRequest { id }) => {
            let path = std::path::Path::new(".scriptvox/runs").join(&id).join("request.json");
            match std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<scriptvox::make::Args>(&s).ok()) {
                Some(mut a) => {
                    a.run_id = Some(id);
                    scriptvox::make::run(a)
                }
                None => Envelope::fail("make", Exit::Error, format!("no request at {}", path.display()), None),
            }
        }
        Some(Cmd::Explain { file }) => match read_recipe("explain", &file) {
            Ok(r) => Envelope::ok("explain").with("recipe", r),
            Err(e) => *e,
        },
        Some(Cmd::Replay { file, out }) => run_recipe("replay", &file, from_recipe(&file, out, true, &cli)),
        Some(Cmd::Remake { file, out, detach, run_id }) => {
            if detach && run_id.is_none() && !cli.plan && !cfg!(windows) {
                return detach_remake(&file, out, &cli);
            }
            let mut a = from_recipe(&file, out, false, &cli);
            if let Ok(a) = &mut a {
                a.run_id = run_id;
            }
            let mut env = run_recipe("remake", &file, a);
            if detach && cfg!(windows) && !cli.plan {
                env = env.with("warning", json!("--detach is not available on Windows in v0.1; the run ran in the foreground"));
            }
            env
        }
        Some(Cmd::Wait { id, timeout }) => {
            let env = scriptvox::supervise::wait(std::path::Path::new("."), &id, std::time::Duration::from_secs(timeout));
            // The `claims` finding of `plan`, `make` and `check <run>`: the result file of a run is written before
            // `make` adds it, so `wait` adds it from the run's request. Only for a run that has ended.
            if std::path::Path::new(".scriptvox/runs").join(&id).join("result.json").is_file() {
                scriptvox::claims::apply(env, scriptvox::claims::gate_of_run(&id, false))
            } else {
                env
            }
        }
        Some(Cmd::Cancel { id }) => scriptvox::supervise::cancel(std::path::Path::new("."), &id),
        Some(Cmd::Media { action: MediaCmd::Add { dir } }) => media_add(&dir),
        Some(Cmd::Media { action: MediaCmd::Search { query, kind, limit, get } }) => scriptvox::open_search::run(&query, &kind, limit, get),
        Some(Cmd::Media { action: MediaCmd::Music { mood } }) => scriptvox::music_lib::list(mood.as_deref()),
        Some(Cmd::Setup { what: Some(w), dry_run, arabic, from, .. }) if w == "speech-local" => {
            if arabic { scriptvox::speech_arabic::setup(dry_run, from.as_deref(), || setup_speech_local(dry_run)) } else { setup_speech_local(dry_run) }
        }
        Some(Cmd::Setup { what: Some(w), dry_run, .. }) if w == "speech-large" => setup_speech_large(dry_run),
        Some(Cmd::Setup { what: Some(w), dry_run, .. }) if w == "image-local" => setup_image_local(dry_run),
        Some(Cmd::Setup { what: Some(w), dry_run, client, config, .. }) if w == "mcp" => {
            scriptvox::mcp_setup::run(client.as_deref(), config.as_deref(), dry_run)
        }
        Some(Cmd::Setup { what: Some(w), dry_run, accept_gpl, .. }) if w == "voice-local" => setup_voice_local(dry_run, accept_gpl),
        Some(Cmd::Setup { what: Some(w), dry_run, .. }) if w == "voice-rust" => setup_voice_rust(dry_run),
        Some(Cmd::Setup { what: Some(w), dry_run, .. }) if w == "fetch" => scriptvox::fetch::setup(dry_run, agent || cli.json || !std::io::stdout().is_terminal()),
        Some(Cmd::Setup { what: Some(w), .. }) => Envelope::fail("setup", Exit::Usage, format!("unknown setup target {w}"), Some("Use `scriptvox setup`, `scriptvox setup voice-local`, `scriptvox setup voice-rust`, `scriptvox setup speech-local`, `scriptvox setup image-local` or `scriptvox setup fetch`.".into())),
        Some(Cmd::Setup { dry_run, .. }) => setup(dry_run, cli.use_system),
        Some(Cmd::Doctor { fix, redact, offline }) => doctor(fix, redact, offline, cli.use_system),
        Some(Cmd::Uninstall { purge }) => {
            let data = scriptvox::provision::data_dir();
            if purge {
                match std::fs::remove_dir_all(&data) {
                    Ok(()) => Envelope::ok("uninstall").with("removed", json!(data)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Envelope::ok("uninstall").with("removed", json!(null)),
                    Err(e) => Envelope::fail("uninstall", Exit::Error, format!("cannot remove {}: {e}", data.display()), None),
                }
            } else {
                Envelope::ok("uninstall").with("would_remove", json!(data)).with("next", json!(["scriptvox uninstall --purge"]))
            }
        }
        Some(Cmd::Models { action: scriptvox::cli::ModelsCmd::Scan { folder } }) => models_scan(folder),
        Some(Cmd::Models { action: scriptvox::cli::ModelsCmd::Add { source, name, file, family, kind, dry_run } }) => {
            scriptvox::model_add::add(scriptvox::model_add::Args { source, name, file, family, kind, dry_run })
        }
        Some(Cmd::Keys { action: KeysCmd::List }) => {
            let mut env = Envelope::ok("keys").with(
                "keys",
                json!(scriptvox::keys::all()
                    .iter()
                    .map(|(p, var)| json!({"provider": p, "env": var, "source": scriptvox::keys::resolve(p).map_or("none", |(_, s)| s)}))
                    .collect::<Vec<_>>()),
            );
            if let Some(e) = scriptvox::keys::file_error() {
                env.findings.push(json!({"gate": "keys_file", "status": "warn", "detail": e,
                    "fix": "Until it is corrected, every key in that file counts as not set."}));
            }
            env
        }
        Some(Cmd::Keys { action: KeysCmd::Set { provider, stdin } }) => keys_set(&provider, stdin, agent),
        Some(Cmd::Providers { action: Some(scriptvox::cli::ProvidersCmd::Test { row, wait_s }), .. }) => {
            scriptvox::generate::provider_test(&row, wait_s, cli.approve.clone(), cli.max_spend, cli.plan, cli.use_system)
        }
        Some(Cmd::Providers { action: Some(scriptvox::cli::ProvidersCmd::Template { gateway }), .. }) => match scriptvox::generators::template(&gateway) {
            Some(t) => Envelope::ok("providers").with("template", json!(t)),
            None => Envelope::fail("providers", Exit::Usage, format!("{gateway} is not a gateway"), Some("Use fal, selfhost or openai-speech.".into())),
        },
        Some(Cmd::Providers { action: Some(scriptvox::cli::ProvidersCmd::Add { name, gateway, kind, t2v, i2v, base }), .. }) => providers_add(&name, &gateway, &kind, t2v, i2v, base),
        Some(Cmd::Providers { action: None, kind: Some(k), .. }) if !scriptvox::generators::KINDS.iter().any(|x| x.starts_with(k.as_str())) => {
            Envelope::fail("providers", Exit::Usage, format!("no row has the kind '{k}'"), Some(format!("Use one of: {}.", scriptvox::generators::KINDS.join(", "))))
        }
        Some(Cmd::Providers { action: None, kind, ready, free }) => Envelope::ok("providers").with(
            "providers",
            json!(scriptvox::http_rows::rows()
                .iter()
                .map(|r| json!({"name": r.name, "kind": r.kind, "site": r.site, "key": scriptvox::keys::env_var(&r.key),
                    "service": r.service(),
                    "key_set": r.ready(), "runnable": r.ready(), "business_use": scriptvox::http_rows::business_use(r.terms.commercial),
                    "why_not": (!r.ready()).then(|| format!("no {0} key. Run `scriptvox keys set {0}` in your terminal.", r.key)),
                    "network": true, "cost": "free", "output_terms": r.terms}))
                .collect::<Vec<_>>()),
        )
        .with("generators", json!(scriptvox::generators::all().iter().filter(|g| kind.as_deref().is_none_or(|k| g.kind.starts_with(k))).filter(|g| !ready || g.ready()).filter(|g| !free || !g.paid()).map(|g| json!({
            "name": g.name, "kind": g.kind, "gateway": g.gateway, "service": g.service(), "key": scriptvox::keys::env_var(&g.key),
            "key_set": g.ready(), "business_use": scriptvox::http_rows::business_use(g.terms.commercial), "runnable": g.why_not().is_none(), "why_not": g.why_not().map(|b| format!("{}. {}", b.why, b.fix)), "modes": g.modes(), "takes_start_picture": (g.kind == "video.gen").then(|| g.i2v.is_some()), "takes_seed": g.takes_seed, "takes_last_frame": g.takes_last_frame, "retired": g.retired, "instead": g.instead,
            "durations_s": g.durations, "aspects": g.aspects, "usd_per_s": g.usd_per_s, "usd_each": g.usd_each, "price_also": g.shape_words(), "price": g.price,
            "price_source": g.price_source, "price_checked": g.price_checked, "output_terms": g.terms,
            "price_age_days": g.price_age_days(),
            "price_stale": g.price_age_days().is_some_and(|a| a > scriptvox::generators::PRICE_STALE_DAYS),
        })).collect::<Vec<_>>()))
        .with("speech_local", json!({"installed": scriptvox::provision::whisper_ready(), "model": scriptvox::provision::whisper_row_name(), "large_installed": scriptvox::provision::whisper_large_ready(), "arabic": scriptvox::speech_arabic::listing(), "service": scriptvox::generators::local_stt().service()}))
        .with("image_local", json!({"installed": scriptvox::provision::image_ready(), "service": scriptvox::generators::local_image().service()}))
        .with("generator_errors", json!(scriptvox::generators::problems()))
        .with("text_rows", scriptvox::text_rows::listing()),
        Some(Cmd::Tools) => {
            let mut tools = vec![json!({"name": "engine.ffmpeg", "network": false, "cost": "free", "process": "native", "licence": "GPL-3.0-or-later (pinned BtbN build)"})];
            for engine in ["edge", "elevenlabs"] {
                for v in scriptvox::voices::all().iter().filter(|v| v.engine == engine).take(1) {
                    tools.push(json!({"name": format!("tts.{engine}"), "network": v.network, "cost": v.cost, "service": v.service(), "process": "python worker", "timing": v.timing, "terms": v.terms}));
                }
            }
            for r in scriptvox::http_rows::rows() {
                tools.push(json!({"name": format!("{}.{}", r.kind, r.name), "network": true, "cost": "free", "service": r.service(), "process": "native http row", "output_terms": r.terms}));
            }
            for g in scriptvox::generators::all().iter().filter(|g| g.gateway != "selfhost") {
                tools.push(json!({"name": format!("{}.{}", g.kind, g.name), "network": g.gateway != "local", "cost": if g.paid() { "paid" } else { "free" }, "service": g.service(), "process": if g.gateway == "local" { "local process" } else { "native http row" }, "output_terms": g.terms}));
            }
            Envelope::ok("tools").with("tools", json!(tools))
        }
        Some(Cmd::Check { target, hear, script, lang, budget, platform, strict }) => {
            let claim = scriptvox::claims::gate_of_run(&target, strict);
            scriptvox::claims::apply(check(&target, hear, script, lang, budget, platform, cli.use_system), claim)
        }
        Some(Cmd::Snapshot { target, at, sheet, cards, safe, out }) => snapshot(&target, at, sheet, cards, safe, out, cli.use_system),
        Some(Cmd::Schema { name }) => match scriptvox::brief::schema(&name) {
            Some(doc) => Envelope::ok("schema").with("name", json!(name)).with("json_schema", doc),
            None => Envelope::fail("schema", Exit::Usage, format!("no schema {name}"), Some(format!("Use one of: {}.", scriptvox::brief::SCHEMAS.join(", ")))),
        },
        Some(Cmd::Skills { action: SkillsCmd::Render }) => Envelope::ok("skills").with("skill", json!(scriptvox::skill::render())).with("reference", json!(scriptvox::skill::render_reference())),
        Some(Cmd::Skills { action: SkillsCmd::Install { agents } }) => {
            let done = if agents { scriptvox::skill::install_agents() } else { scriptvox::skill::install().map(|p| (vec![], p)) };
            match done {
                Ok((rows, paths)) => {
                    let e = Envelope::ok("skills").with("installed", json!(paths[0])).with("skills", json!(paths)).with("version", json!(scriptvox::skill::VERSION));
                    if agents { e.with("agents", json!(rows)) } else { e }
                }
                Err(e) => Envelope::fail("skills", Exit::Error, e, None),
            }
        }
        Some(Cmd::Init { agents }) => {
            if !agents {
                return Envelope::fail("init", Exit::Usage, "nothing to do", Some("Use `scriptvox init --agents`.".into()));
            }
            let dir = std::path::Path::new(".");
            match (scriptvox::skill::init_file(dir, "AGENTS.md"), scriptvox::skill::init_file(dir, "CLAUDE.md")) {
                (Ok(a), Ok(c)) => Envelope::ok("init").with("agents_md", json!(a)).with("claude_md", json!(c)),
                (Err(e), _) | (_, Err(e)) => Envelope::fail("init", Exit::Error, e, None),
            }
        }
        Some(Cmd::Connect { name, url, key_name, models, workflow, spec, maps, dry_run, text, usd_in, usd_out }) => {
            scriptvox::connect::run(scriptvox::connect::Args { name, url, key_name, models, workflow, spec, maps, dry_run, text, usd_in, usd_out })
        }
        Some(Cmd::Publish { action }) => scriptvox::publish::run(action, agent, cli.plan),
        Some(Cmd::Research { topic, lang, depth, out, urls, text_row, pages_only }) => scriptvox::research::run(scriptvox::research::Args {
            topic, lang, depth, out, urls, text_row, pages_only, plan_only: cli.plan, max_spend: cli.max_spend, local_only: cli.local_only,
        }),
        Some(Cmd::Write { from, topic, duration, lang, out, text_row, tts, sources_card }) => scriptvox::write::run(scriptvox::write::Args {
            from, topic, duration, lang, out, text_row, tts, sources_card, plan_only: cli.plan, max_spend: cli.max_spend, local_only: cli.local_only,
        }),
        Some(Cmd::Licenses { full }) => {
            let mut env = Envelope::ok("licenses")
                .with("license", json!("Apache-2.0"))
                .with("crates", json!(scriptvox::licenses::crates()))
                .with("components", json!(scriptvox::licenses::COMPONENTS.iter().map(|(n, l, h)| json!({"name": n, "license": l, "how": h})).collect::<Vec<_>>()))
                .with("python_packages", json!(scriptvox::licenses::python_packages()))
                .with("downloads", json!(scriptvox::licenses::downloads()));
            if full {
                env = env.with("notices", json!(scriptvox::licenses::THIRD_PARTY)).with("ofl", json!(scriptvox::licenses::OFL)).with("gpl", json!(scriptvox::licenses::GPL));
            }
            env
        }
        Some(Cmd::Report { run, out }) => scriptvox::report::run(scriptvox::report::Args { run, out, use_system: cli.use_system }),
        Some(Cmd::Bench) => {
            let ff = match scriptvox::engine_ffmpeg::Ffmpeg::locate(cli.use_system, true) {
                Ok(f) => f,
                Err(e) => return scriptvox::engine_ffmpeg::locate_failure("bench", e),
            };
            match scriptvox::bench::run(&ff) {
                Ok(b) => {
                    let _ = std::fs::create_dir_all("scriptvox-out");
                    let path = std::path::Path::new("scriptvox-out").join(format!("bench-{}.json", &scriptvox::run::now_utc()[..10]));
                    let _ = std::fs::write(&path, serde_json::to_string_pretty(&b).unwrap_or_default());
                    let mut env = Envelope::ok("bench").with("bench", b);
                    env.artifacts.insert("bench".into(), json!(std::path::absolute(&path).unwrap_or(path)));
                    env
                }
                Err(e) => Envelope::fail("bench", Exit::Error, e, None),
            }
        }
        Some(Cmd::Recipes { action: None }) => Envelope::ok("recipes").with("recipes", scriptvox::recipes::list()),
        Some(Cmd::Recipes { action: Some(scriptvox::cli::RecipesCmd::Show { name }) }) => match scriptvox::recipes::show(&name) {
            Ok(r) => Envelope::ok("recipes").with("recipe", r),
            Err(e) => Envelope::fail("recipes", Exit::Usage, e, Some("Run `scriptvox recipes --json` and take a `name` from the list.".into())),
        },
        Some(Cmd::Templates) => Envelope::ok("templates").with(
            "templates",
            json!([
                {"name": "narrated-gradient", "status": "available", "aspects": ["9:16", "16:9", "1:1", "4:5", "3:4", "4:3", "2:3", "21:9"], "command": "scriptvox make --script f", "keys": "none"},
                {"name": "kinetic-text", "status": "available", "aspects": ["9:16"], "command": "scriptvox demo", "keys": "none"},
                {"name": "own-media-reel", "status": "available", "aspects": ["9:16", "16:9", "1:1", "4:5", "3:4", "4:3", "2:3", "21:9"], "command": "scriptvox make --script f --media dir", "keys": "none"},
                {"name": "stock-short", "status": "available", "aspects": ["9:16", "16:9", "1:1", "4:5", "3:4", "4:3", "2:3", "21:9"], "command": "scriptvox make --script f --stock auto", "keys": "none", "optional_keys": "PEXELS_API_KEY or PIXABAY_API_KEY add those sources"},
            ]),
        ),
        Some(Cmd::Voices { action: Some(scriptvox::cli::VoicesCmd::Add { sample, consent, own: _, name }), .. }) => {
            // clap: `--consent` or `--own`, never both; no `--consent` means the user's own statement.
            match scriptvox::voices::add_consent(&sample, consent.as_deref(), name.as_deref()) {
                Ok(record) => Envelope::ok("voices").with("consent", record),
                Err(e) => Envelope::fail("voices", Exit::Usage, e, Some("Give two files that exist: the sample and your proof.".into())),
            }
        }
        Some(Cmd::Voices { lang, preview: Some(id), text, brief, tts, out, action: None }) => {
            scriptvox::voices::preview(&scriptvox::voices::Preview {
                id: &id,
                text: text.as_deref(),
                brief: brief.as_deref(),
                lang: lang.as_deref(),
                tts: tts.as_deref(),
                out,
                approve: cli.approve.as_deref(),
                max_spend: cli.max_spend,
                plan_only: cli.plan,
                local_only: cli.local_only,
                use_system: cli.use_system,
            })
        }
        Some(Cmd::Voices { lang, action: None, .. }) => {
            let list: Vec<_> = scriptvox::voices::all()
                .into_iter()
                .filter(|v| lang.as_deref().is_none_or(|l| v.speaks(&scriptvox::voices::lang_code(l))))
                .collect();
            let list: Vec<_> = list
                .iter()
                .map(|v| {
                    let mut j = json!(v);
                    j["service"] = json!(v.service());
                    j["business_use"] = json!(scriptvox::http_rows::business_use(Some(v.commercial)));
                    j
                })
                .collect();
            Envelope::ok("voices").with("voices", json!(list))
        }
        None => Envelope::fail(
            "",
            Exit::Usage,
            "no command given",
            Some("Run `scriptvox --help`.".into()),
        ),
    }
}

fn demo(out: Option<std::path::PathBuf>, golden: bool, use_system: bool) -> Envelope {
    use scriptvox::demo::{render, Spec};
    let ff = match scriptvox::engine_ffmpeg::Ffmpeg::locate(use_system, true) {
        Ok(ff) => ff,
        Err(e) => return scriptvox::engine_ffmpeg::locate_failure("demo", e),
    };
    let spec = if golden { Spec::GOLDEN } else { Spec::FULL };
    let out = out.unwrap_or_else(|| "scriptvox-demo.mp4".into());
    let out = std::path::absolute(&out).unwrap_or(out);
    let t0 = std::time::Instant::now();
    let rb = match render(&ff, &spec, &out) {
        Ok(rb) => rb,
        Err(e) => return Envelope::fail("demo", Exit::Error, e, None),
    };
    let mut env = Envelope::ok("demo");
    // Readback gate: the file must hold exactly the planned frames at the planned fps.
    if rb.frames != spec.frames() || (rb.fps - f64::from(spec.fps)).abs() > 0.01 {
        env = Envelope::fail(
            "demo",
            Exit::GateFailed,
            format!(
                "readback: {} frames at {} fps, planned {} at {}",
                rb.frames,
                rb.fps,
                spec.frames(),
                spec.fps
            ),
            None,
        );
    }
    env.artifacts.insert("video".into(), json!(out));
    for (k, v) in [
        ("frames", json!(rb.frames)),
        ("fps", json!(rb.fps)),
        ("width", json!(rb.width)),
        ("height", json!(rb.height)),
        ("render_ms", json!(t0.elapsed().as_millis() as u64)),
    ] {
        env.metrics.insert(k.into(), v);
    }
    env
}

fn worker() -> Envelope {
    use scriptvox::{provision, rpc};
    let started = provision::worker_dir()
        .and_then(|d| provision::worker_command(&d))
        .map_err(|e| rpc::RpcError {
            code: rpc::WORKER_CRASHED,
            message: e,
        })
        .and_then(rpc::Worker::spawn);
    let mut w = match started {
        Ok(w) => w,
        Err(e) => {
            return Envelope::fail(
                "worker",
                Exit::Error,
                e.message,
                Some("Install uv: https://docs.astral.sh/uv/".into()),
            )
        }
    };
    let t0 = std::time::Instant::now();
    match w.hello(false) {
        Ok(r) => {
            let ms = t0.elapsed().as_millis() as u64;
            w.shutdown();
            let mut env = Envelope::ok("worker").with("worker", r);
            env.metrics.insert("hello_ms".into(), json!(ms));
            env
        }
        Err(e) if e.code == rpc::PROTO_MISMATCH => Envelope::fail_code(
            "worker",
            Exit::Error,
            "SVX_PROTO_MISMATCH",
            e.message,
            Some(
                "The worker and the binary come from different versions. Unset SCRIPTVOX_WORKER_SRC."
                    .into(),
            ),
        ),
        Err(e) => Envelope::fail_code("worker", Exit::Error, "SVX_WORKER_CRASHED", e.message, None),
    }
}

fn outside_project(command: &str, p: &std::path::Path) -> Envelope {
    Envelope::fail_code(
        command,
        Exit::Error,
        "SVX_PATH_OUTSIDE_ROOT",
        format!("{} is outside this project folder", p.display()),
        Some("Copy the file into the project.".into()),
    )
}

fn media_add(dir: &std::path::Path) -> Envelope {
    use scriptvox::media::{list, Kind, MediaError};
    let items = match list(dir, std::path::Path::new(".")) {
        Ok(i) => i,
        Err(MediaError::OutsideRoot(p)) => {
            return Envelope::fail_code(
                "media",
                Exit::Error,
                "SVX_PATH_OUTSIDE_ROOT",
                format!("{} is outside this project folder", p.display()),
                Some("Copy or link the media into the project, or run scriptvox from a folder that contains it.".into()),
            )
        }
        Err(MediaError::Missing(p)) => return Envelope::fail("media", Exit::Error, format!("{} does not exist", p.display()), None),
        Err(MediaError::Empty(p)) => {
            return Envelope::fail("media", Exit::Error, format!("no clips or stills in {}", p.display()), Some("Add .mp4, .mov, .jpg or .png files.".into()))
        }
    };
    let ff = scriptvox::engine_ffmpeg::Ffmpeg::locate(true, false).ok();
    let rows: Vec<serde_json::Value> = items
        .iter()
        .map(|i| {
            let kind = if i.kind == Kind::Video { "video" } else { "image" };
            let rb = ff.as_ref().and_then(|f| f.readback(&i.path).ok());
            let dur = if i.kind == Kind::Video { ff.as_ref().and_then(|f| f.duration(&i.path).ok()) } else { None };
            let hdr = if i.kind == Kind::Video { ff.as_ref().and_then(|f| f.hdr_kind(&i.path)) } else { None };
            json!({"path": i.path, "kind": kind, "hdr": hdr, "width": rb.as_ref().map(|r| r.width), "height": rb.as_ref().map(|r| r.height), "duration_s": dur})
        })
        .collect();
    let index = json!({"dir": dir, "items": rows});
    let _ = std::fs::create_dir_all(".scriptvox")
        .and_then(|_| std::fs::write(".scriptvox/media.json", index.to_string()));
    Envelope::ok("media").with("items", json!(rows)).with(
        "next",
        json!([format!(
            "scriptvox make --script script.txt --media {}",
            dir.display()
        )]),
    )
}

/// `make --detach`: create the run, start the supervisor, return at once. A plan with generated scenes, or a key, names
/// its run as `make` does, so a retry finds the same job and pays nothing more.
fn detach_make(key: Option<&str>, approve: Option<&str>, max_spend: Option<f64>) -> Envelope {
    // The free checks of `plan` first: a bad brief fails here, not only at `wait` (test 3).
    let plan = std::env::current_exe().ok().and_then(|exe| {
        std::process::Command::new(exe)
            .args(
                std::env::args()
                    .skip(1)
                    .filter(|a| a != "--detach" && a != "--json"),
            )
            .args(["--plan", "--json"])
            .stdin(std::process::Stdio::null())
            .output()
            .ok()
    });
    let code = plan.as_ref().and_then(|p| p.status.code());
    let v: serde_json::Value = plan
        .and_then(|p| serde_json::from_slice(&p.stdout).ok())
        .unwrap_or_default();
    // Only a usage error stops here. A script too long for its time stays a run, so `resume` can fix it.
    if code == Some(2) {
        return Envelope::fail(
            "make",
            Exit::Usage,
            v["error"]["message"]
                .as_str()
                .unwrap_or("the plan failed")
                .to_string(),
            v["error"]["fix"].as_str().map(String::from),
        );
    }
    detach_start("make", &v, key, None, approve, max_spend)
}

/// The shared part of `make --detach` and `remake --detach`, after the plan `v` passed its checks: name the run, answer
/// from a finished one, stop a paid plan with no approval, else create the run and start the supervisor. `fallback` is
/// the key of a plan that has none, so `remake` is always one run for one plan.
fn detach_start(
    cmd: &str,
    v: &serde_json::Value,
    key: Option<&str>,
    fallback: Option<&str>,
    approve: Option<&str>,
    max_spend: Option<f64>,
) -> Envelope {
    let project = std::path::Path::new(".");
    let has_gen = v["stages"].as_array().is_some_and(|s| {
        s.iter().any(|x| {
            x["stage"] == "generate"
                || (x["stage"] == "music" && x["cost_usd"].as_f64().is_some_and(|c| c > 0.0))
        })
    });
    let plan_id = v["plan_id"].as_str();
    let id = plan_id
        .zip(key.or(has_gen.then_some("make")).or(fallback))
        .map(|(p, k)| scriptvox::plan::run_id_for(p, k));
    if let Some(id) = &id {
        // A finished run answers for itself, as a foreground rerun does. One that failed is attached and resumed.
        let result = project.join(".scriptvox/runs").join(id).join("result.json");
        let prev = std::fs::read_to_string(&result)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        if let Some(prev) = prev {
            if scriptvox::supervise::replayable(project, &prev) {
                let mut env = scriptvox::supervise::replay(cmd, prev);
                env.plan_id = plan_id.map(String::from);
                return env.with("idempotent", json!(true));
            }
            let _ = std::fs::remove_file(&result);
        }
    }
    // A paid plan with no approval stops here, as the plain command does, and no run starts. Before 10 Nov 2026 the
    // answer was `running`, and only `wait` said approval_needed (seen through the MCP tool, which always detaches).
    if let Some(p) = plan_id {
        let cost = v["metrics"]["cost_usd"].as_f64().unwrap_or(0.0);
        if let Some(env) =
            scriptvox::make::approval_stop(p, cost, approve, max_spend, v["stages"].clone())
        {
            return as_command(cmd, env);
        }
    }
    let created = match &id {
        Some(id) => scriptvox::run::Run::create_with_id(project, "make", id),
        None => scriptvox::run::Run::create(project, "make"),
    };
    let run = match created {
        Ok(r) => r,
        Err(e) => {
            return Envelope::fail(
                cmd,
                Exit::Error,
                format!("cannot create the run folder: {e}"),
                None,
            )
        }
    };
    let running = |run: &scriptvox::run::Run| {
        let mut env = Envelope::ok(cmd);
        env.status = "running";
        env.run_id = Some(run.id.clone());
        env.plan_id = plan_id.map(String::from);
        env.events_file = Some(run.dir.join("events.jsonl").to_string_lossy().into_owned());
        env.next = vec![format!("scriptvox wait {}", run.id)];
        env
    };
    if id.is_some() && scriptvox::supervise::running(project, &run.id) {
        return running(&run);
    }
    let mut args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--detach" && a != "--json")
        .collect();
    args.extend(["--run-id".into(), run.id.clone()]);
    match scriptvox::supervise::spawn(&run, &args) {
        Ok(pid) => {
            run.event(
                "progress",
                json!({"explain": format!("supervisor started, pid {pid}")}),
            );
            running(&run)
        }
        Err(e) => Envelope::fail(
            cmd,
            Exit::Error,
            format!("cannot start the supervisor: {e}"),
            None,
        ),
    }
}

/// `generate --detach`: the job goes to a supervisor under the plan's own run id, so a second call finds the same job and
/// pays nothing more. None: the plain command answers now, and it stops before any call. That is a usage error, a missing
/// key or service, or a paid plan with no approval.
fn detach_generate(approve: Option<&str>, key: Option<&str>) -> Option<Envelope> {
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--detach" && a != "--json")
        .collect();
    let plan = std::process::Command::new(std::env::current_exe().ok()?)
        .args(&args)
        .args(["--plan", "--json"])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&plan.stdout).ok()?;
    let plan_id = v["plan_id"].as_str()?;
    let cost = v["metrics"]["cost_usd"].as_f64().unwrap_or(0.0);
    if !plan.status.success()
        || v["needs_input"].as_array().is_some_and(|n| !n.is_empty())
        || scriptvox::plan::needs_approval(plan_id, cost, approve).is_some()
    {
        return None;
    }
    let project = std::path::Path::new(".");
    let id = scriptvox::plan::run_id_for(plan_id, key.unwrap_or("generate"));
    let run = match scriptvox::run::Run::create_with_id(project, "generate", &id) {
        Ok(r) => r,
        Err(e) => {
            return Some(Envelope::fail(
                "generate",
                Exit::Error,
                format!("cannot create the run folder: {e}"),
                None,
            ))
        }
    };
    let running = |run: &scriptvox::run::Run| {
        let mut env = Envelope::ok("generate");
        env.status = "running";
        env.run_id = Some(run.id.clone());
        env.plan_id = Some(plan_id.to_string());
        env.events_file = Some(run.dir.join("events.jsonl").to_string_lossy().into_owned());
        env.next = vec![format!("scriptvox wait {}", run.id)];
        env
    };
    if scriptvox::supervise::running(project, &id) {
        return Some(running(&run));
    }
    // An earlier result of this plan must not answer for this start.
    let _ = std::fs::remove_file(run.dir.join("result.json"));
    let mut child = args;
    child.extend(["--run-id".into(), id]);
    Some(match scriptvox::supervise::spawn(&run, &child) {
        Ok(pid) => {
            run.event(
                "progress",
                json!({"explain": format!("supervisor started, pid {pid}")}),
            );
            running(&run)
        }
        Err(e) => Envelope::fail(
            "generate",
            Exit::Error,
            format!("cannot start the supervisor: {e}"),
            None,
        ),
    })
}

fn load_terms(
    command: &str,
    path: Option<std::path::PathBuf>,
) -> Result<Option<scriptvox::say::Terms>, Box<Envelope>> {
    path.map(|p| scriptvox::say::Terms::load(&p))
        .transpose()
        .map_err(|e| {
            Box::new(Envelope::fail(
                command,
                Exit::Usage,
                e,
                Some(r#"Example: {"terms": ["ACME"], "fix": {"ak me": "ACME"}}"#.into()),
            ))
        })
}

fn setup(dry_run: bool, use_system: bool) -> Envelope {
    use scriptvox::provision;
    let todo = provision::pending(use_system, true);
    if dry_run {
        // The voice worker's Python and packages come through uv with no pin of ours to read a size from (audit L1).
        return scriptvox::doctor::setup_dry_run(&todo).with("note", json!("The list has the pinned files. When missing, the voice worker's Python (about 103 MB) and its packages (about 13 MB) come on top through uv (measured 7 Nov 2026)."));
    }
    let rows: Vec<serde_json::Value> = todo
        .iter()
        .map(|p| json!({"name": p.name, "version": p.version, "bytes": p.size, "licence": p.licence, "url": p.url}))
        .collect();
    let bytes: u64 = todo.iter().map(|p| p.size).sum();
    for p in &todo {
        if let Err(e) = provision::install(p, &|m| eprintln!("scriptvox: {m}")) {
            return scriptvox::doctor::setup_failure(e);
        }
        if p.name == "ffmpeg" {
            let bins = provision::installed(p).unwrap_or_default();
            if bins.first().and_then(|b| provision::ffmpeg_class(b).ok()) == Some("nonfree") {
                let _ = std::fs::remove_dir_all(provision::tool_dir(p));
                return Envelope::fail_code(
                    "setup",
                    Exit::Error,
                    "SVX_NONFREE_FFMPEG",
                    "the pinned FFmpeg build is nonfree; it was removed",
                    None,
                );
            }
        }
    }
    // Pre-warm the voice worker, so the first voice run does not wait for it.
    if let Err(e) = provision::worker_dir()
        .and_then(|d| provision::prepare_worker(&d, &|m| eprintln!("scriptvox: {m}")))
    {
        return scriptvox::doctor::setup_failure(e);
    }
    let mut env = Envelope::ok("setup").with("installed", json!(rows));
    env.metrics.insert("download_bytes".into(), json!(bytes));
    env
}

fn doctor(fix: bool, redact: bool, offline: bool, use_system: bool) -> Envelope {
    use scriptvox::doctor;
    let (mut checks, mut extra) = doctor::run(use_system, !offline);
    if fix
        && checks.iter().any(|c| {
            c.status != "ok"
                && c.fix_safe
                && c.fix
                    .as_deref()
                    .is_some_and(|f| f.starts_with("scriptvox setup"))
        })
    {
        for p in scriptvox::provision::pending(use_system, true) {
            if let Err(e) = scriptvox::provision::install(&p, &|m| eprintln!("scriptvox: {m}")) {
                eprintln!("scriptvox: fix failed: {e}");
            }
        }
        (checks, extra) = doctor::run(use_system, !offline);
    }
    let exit = doctor::exit_for(&checks);
    let mut env = match exit {
        Exit::Ok => Envelope::ok("doctor"),
        Exit::NeedsInput => {
            let mut e = Envelope::fail(
                "doctor",
                exit,
                "a check needs a fix that only you can do",
                None,
            );
            e.needs_input = checks
                .iter()
                .filter(|c| c.status == "fail" && !c.fix_safe)
                .map(|c| json!({"field": c.name, "kind": "dependency", "human_action": c.fix}))
                .collect();
            e
        }
        _ => Envelope::fail(
            "doctor",
            exit,
            "a check failed",
            Some("scriptvox doctor --fix".into()),
        ),
    };
    env.findings = checks.iter().map(|c| json!(c)).collect();
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        env = env.with(&k, v);
    }
    if redact {
        // Round-trip through text: every string loses home paths and the user name.
        let text = doctor::redact_paths(&serde_json::to_string(&env.findings).unwrap_or_default());
        env.findings = serde_json::from_str(&text).unwrap_or_default();
    }
    env
}

fn keys_set(provider: &str, stdin: bool, agent: bool) -> Envelope {
    use std::io::IsTerminal;
    if scriptvox::keys::env_var(provider).is_none() {
        return Envelope::fail(
            "keys",
            Exit::Usage,
            format!("unknown provider {provider}"),
            Some(format!(
                "Use one of: {}. Or add a [[key]] block with name and env to generators.toml.",
                scriptvox::keys::all()
                    .into_iter()
                    .map(|(p, _)| p)
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        );
    }
    let value = if stdin {
        let mut s = String::new();
        std::io::stdin()
            .read_line(&mut s)
            .map(|_| s.trim().to_string())
    } else if std::io::stdin().is_terminal() && !agent {
        scriptvox::keys::prompt(provider)
    } else {
        // An agent must never ask for the value in chat: the human types it.
        let mut env = Envelope::fail(
            "keys",
            Exit::NeedsInput,
            "the key must come from the user",
            None,
        );
        env.needs_input = vec![json!({
            "field": format!("{provider}.api_key"), "kind": "secret",
            "human_action": format!("Run `scriptvox keys set {provider}` in your terminal, or pipe it: `scriptvox keys set {provider} --stdin`"),
        })];
        return env;
    };
    match value {
        Ok(v) if !v.is_empty() => match scriptvox::keys::set(provider, &v) {
            Ok(path) => {
                Envelope::ok("keys").with("stored", json!({"provider": provider, "file": path}))
            }
            Err(e) => Envelope::fail(
                "keys",
                Exit::Error,
                format!("cannot write keys.toml: {e}"),
                None,
            ),
        },
        _ => Envelope::fail("keys", Exit::Usage, "no key value was given", None),
    }
}

/// A run id (its video) or a file path.
/// The run folder of a run id, or of a video a run made (its result names that video).
fn run_dir_of(target: &str, video: &std::path::Path) -> Option<std::path::PathBuf> {
    let runs = std::path::Path::new(".scriptvox/runs");
    if runs.join(target).join("result.json").is_file() {
        return Some(runs.join(target));
    }
    let want = std::path::absolute(video).ok()?;
    std::fs::read_dir(runs)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|d| {
            std::fs::read_to_string(d.join("result.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|r| {
                    r["artifacts"]["video"]
                        .as_str()
                        .map(std::path::PathBuf::from)
                })
                .and_then(|v| std::path::absolute(v).ok())
                .is_some_and(|v| v == want)
        })
}

/// The error when a target has no video: a run that failed says why, not only that it has no video.
fn no_video(command: &'static str, target: &str) -> Envelope {
    let result = std::fs::read_to_string(
        std::path::Path::new(".scriptvox/runs")
            .join(target)
            .join("result.json"),
    )
    .ok()
    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    match result {
        Some(r) if r["ok"] == false => Envelope::fail(
            command,
            Exit::Usage,
            format!(
                "run {target} failed, so it has no video: {}",
                r["error"]["message"].as_str().unwrap_or("no message")
            ),
            Some(r["error"]["fix"].as_str().map_or(
                format!("Run `scriptvox resume {target} --json` to try again."),
                String::from,
            )),
        ),
        _ => Envelope::fail(
            command,
            Exit::Usage,
            format!("{target} is not a video file or a finished run"),
            Some("Run `scriptvox status` to see the runs.".into()),
        ),
    }
}

fn resolve_video(target: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(target);
    if p.is_file() {
        return Some(p.to_path_buf());
    }
    // Only a run id as ScriptVox writes one becomes a path under `.scriptvox/runs`.
    if !scriptvox::run::safe_id(target) {
        return None;
    }
    let made = std::fs::read_to_string(
        std::path::Path::new(".scriptvox/runs")
            .join(target)
            .join("result.json"),
    )
    .ok()
    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    .and_then(|r| {
        r["artifacts"]["video"]
            .as_str()
            .map(std::path::PathBuf::from)
    });
    // A run of `generate` has no result.json: its clip is `scriptvox-out/<run id>-<row>.mp4`.
    made.or_else(|| {
        std::fs::read_dir("scriptvox-out")
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.extension()
                    .is_some_and(|x| x == "mp4" || x == "webm" || x == "mov")
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.strip_prefix(target).is_some_and(|r| r.starts_with('-')))
            })
    })
}

fn check(
    target: &str,
    hear: bool,
    script: Option<std::path::PathBuf>,
    lang: Option<String>,
    budget: Option<f64>,
    platform: Option<String>,
    use_system: bool,
) -> Envelope {
    if let Some(b) = budget {
        if !(b.is_finite() && (1.0..=3600.0).contains(&b)) {
            return Envelope::fail(
                "check",
                Exit::Usage,
                "--budget is 1 to 3600 seconds",
                Some("For example --budget 60.".into()),
            );
        }
        if hear {
            return Envelope::fail(
                "check",
                Exit::Usage,
                "--hear needs the whole sound, so it does not go with --budget",
                Some("Drop one of the two.".into()),
            );
        }
    }
    let row = match platform
        .as_deref()
        .map(scriptvox::platforms::find)
        .transpose()
    {
        Ok(r) => r,
        Err(e) => {
            return Envelope::fail(
                "check",
                Exit::Usage,
                e,
                Some("Give one of the names in the message.".into()),
            )
        }
    };
    let Some(video) = resolve_video(target) else {
        return no_video("check", target);
    };
    if let Some(env) = scriptvox::media_gate::refuse("check", [&video]) {
        return env;
    }
    let ff = match scriptvox::engine_ffmpeg::Ffmpeg::locate(use_system, true) {
        Ok(f) => f,
        Err(e) => return scriptvox::engine_ffmpeg::locate_failure("check", e),
    };
    let mut findings = match scriptvox::gates::check_within(&ff, &video, budget) {
        Ok(f) => f,
        Err(e) => {
            return Envelope::fail(
                "check",
                Exit::Usage,
                e,
                Some("Give a whole video file that plays, or make the video again.".into()),
            )
        }
    };
    if hear {
        // The script: the given file, else the run's own (its script, or its scenes' scripts in order).
        let req: serde_json::Value = std::fs::read_to_string(
            std::path::Path::new(".scriptvox/runs")
                .join(target)
                .join("request.json"),
        )
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        // A file: the request in its recipe (T3, 13 Nov 2026: `check out.mp4 --hear` asked for a script the file held).
        .or_else(|| ff.recipe(&video).ok().map(|r| r["request"].clone()))
        .unwrap_or_default();
        let text = match &script {
            Some(p) => match std::fs::read_to_string(p) {
                Ok(t) => t,
                Err(e) => {
                    return Envelope::fail(
                        "check",
                        Exit::Usage,
                        format!("{}: {e}", p.display()),
                        None,
                    )
                }
            },
            None => req["script"].as_str().map(String::from).unwrap_or_else(|| {
                req["scenes"].as_array().map_or(String::new(), |sc| {
                    sc.iter()
                        .filter_map(|s| s["script"].as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
            }),
        };
        if text.trim().is_empty() {
            return Envelope::fail(
                "check",
                Exit::Usage,
                "--hear needs a script, and this video holds none",
                Some("Give `--script <file>` with the words the video must say. A run id, or an MP4 made by scriptvox without --no-embed, needs no --script.".into()),
            );
        }
        let lang = lang.or_else(|| req["lang"].as_str().map(String::from));
        let total = ff.duration(&video).unwrap_or(0.0);
        let say: std::collections::BTreeMap<String, String> =
            serde_json::from_value(req["say"].clone()).unwrap_or_default();
        match scriptvox::analyze::transcribe(&ff, &video, lang.as_deref(), total) {
            Ok((doc, _)) => findings.push(scriptvox::gates::words_heard(
                &scriptvox::say::apply(&text, &say),
                doc["text"].as_str().unwrap_or(""),
            )),
            Err(mut env) => {
                env.command = "check".into();
                return *env;
            }
        }
    }
    findings.extend(
        row.as_ref()
            .and_then(|p| scriptvox::platforms::check(&ff, &video, p)),
    );
    let mut env = gate_envelope("check", findings);
    run_metrics(&mut env, target);
    if let Some(db) = scriptvox::edit::noise_floor(&ff, &video) {
        env.metrics.insert("noise_floor_db".into(), json!(db));
    }
    // A plain file has no run for `default_next` to name (fresh-agent test 4, 5 Nov 2026: `next` was empty).
    if env.ok
        && !std::path::Path::new(".scriptvox/runs")
            .join(target)
            .is_dir()
    {
        env.next = vec![format!("scriptvox snapshot {target} --sheet 9 --json")];
    }
    // The run's own words and captions gates (they need the transcript and the caption pages), so `check <run>`
    // shows all the gates `wait` showed (4 Nov 2026 eval: 5 of 7).
    let r: serde_json::Value = std::fs::read_to_string(
        std::path::Path::new(".scriptvox/runs")
            .join(target)
            .join("result.json"),
    )
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default();
    for f in r["findings"].as_array().into_iter().flatten() {
        if !env.findings.iter().any(|g| g["gate"] == f["gate"]) {
            env.findings.push(f.clone());
        }
    }
    // A gate that failed when the run was made still counts: `check` and `wait` must agree on the exit.
    env.artifacts
        .entry("video".to_string())
        .or_insert(json!(video));
    env = scriptvox::edit::fail_on_gate(env, |_| true);
    let how = scriptvox::gates::hear_how();
    let words_why = if how.starts_with("no speech") {
        how.to_string()
    } else {
        "give --hear to listen".to_string()
    };
    let not_checked = scriptvox::gates::not_checked(&env.findings, &[("words", &words_why)]);
    env = env.with("not_checked", json!(not_checked));
    // `--budget` stopped the read: the result speaks for a part of the file.
    if env.findings.iter().any(|g| g["gate"] == "budget") {
        env = env.with("partial", json!(true));
    }
    let (coverage, limitations) = scriptvox::gates::limits(&env.findings);
    env = env
        .with("coverage", json!(coverage))
        .with("limitations", json!(limitations));
    env
}

/// A finished run's metrics (voice_rate_pct, voice_tempo, duration_s ...) from its result.json, so `check` and
/// `status` show them too (hand eval 4 Nov 2026: only `wait` had them).
fn run_metrics(env: &mut Envelope, run: &str) {
    let r: serde_json::Value = std::fs::read_to_string(
        std::path::Path::new(".scriptvox/runs")
            .join(run)
            .join("result.json"),
    )
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default();
    for (k, v) in r["metrics"].as_object().into_iter().flatten() {
        env.metrics.entry(k.clone()).or_insert(v.clone());
    }
}

/// Exit 3 when a gate fails; the fix of the first failure is the envelope fix.
fn gate_envelope(command: &str, findings: Vec<scriptvox::gates::Finding>) -> Envelope {
    let fails: Vec<&scriptvox::gates::Finding> =
        findings.iter().filter(|f| f.status == "fail").collect();
    let mut env = if fails.is_empty() {
        Envelope::ok(command)
    } else {
        let names: Vec<&str> = fails.iter().map(|f| f.gate).collect();
        Envelope::fail(
            command,
            Exit::GateFailed,
            format!("gate failed: {}", names.join(", ")),
            fails[0].fix.clone(),
        )
    };
    env.findings = findings.iter().map(|f| json!(f)).collect();
    env
}

/// `judge`: the brief is the given file, else the run's own request (script, hook, scenes).
fn judge(
    target: String,
    brief: Option<std::path::PathBuf>,
    frames: u32,
    endpoint: Option<String>,
    model: String,
    min_score: u8,
    use_system: bool,
) -> Envelope {
    let Some(video) = resolve_video(&target) else {
        return no_video("judge", &target);
    };
    let brief = match brief {
        Some(p) => match std::fs::read_to_string(&p) {
            Ok(s) => Some(s),
            Err(e) => {
                return Envelope::fail("judge", Exit::Usage, format!("{}: {e}", p.display()), None)
            }
        },
        None => std::fs::read_to_string(
            std::path::Path::new(".scriptvox/runs")
                .join(&target)
                .join("request.json"),
        )
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .map(|r| {
            json!({"script": r["script"], "hook": r["hook"], "scenes": r["scenes"]}).to_string()
        }),
    };
    // The run's cards, so a frame is not taken while a card is still coming in.
    let cards = std::fs::read_to_string(
        std::path::Path::new(".scriptvox/runs")
            .join(&target)
            .join("cards.json"),
    )
    .ok()
    .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
    .unwrap_or_default()
    .iter()
    .filter_map(|c| Some((c["start"].as_f64()?, c["landed"].as_f64()?)))
    .collect();
    scriptvox::judge::run(scriptvox::judge::Args {
        video,
        cards,
        brief,
        frames,
        endpoint,
        model,
        min_score,
        use_system,
    })
}

fn snapshot(
    target: &str,
    at: Option<String>,
    sheet: Option<u32>,
    cards: bool,
    safe: Option<String>,
    out: Option<std::path::PathBuf>,
    use_system: bool,
) -> Envelope {
    let Some(video) = resolve_video(target) else {
        return no_video("snapshot", target);
    };
    if let Some(env) = scriptvox::media_gate::refuse("snapshot", [&video]) {
        return env;
    }
    let ff = match scriptvox::engine_ffmpeg::Ffmpeg::locate(use_system, true) {
        Ok(f) => f,
        Err(e) => return scriptvox::engine_ffmpeg::locate_failure("snapshot", e),
    };
    let card_list: Vec<serde_json::Value> = if cards {
        let list = run_dir_of(target, &video)
            .and_then(|d| std::fs::read_to_string(d.join("cards.json")).ok())
            .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
            .unwrap_or_default();
        if list.is_empty() {
            return Envelope::fail(
                "snapshot",
                Exit::Usage,
                format!(
                    "{target} has no scene cards (or was made before scriptvox wrote cards.json)"
                ),
                Some("Use --at with the scene times, or --sheet 9.".into()),
            );
        }
        list
    } else {
        Vec::new()
    };
    let times: Vec<f64> = if cards {
        card_list
            .iter()
            .filter_map(|c| c["landed"].as_f64())
            .collect()
    } else {
        let mut times = Vec::new();
        for s in at.iter().flat_map(|a| a.split(',')).map(str::trim) {
            match s.parse::<f64>() {
                Ok(t) if t.is_finite() => times.push(t),
                _ => {
                    return Envelope::fail(
                        "snapshot",
                        Exit::Usage,
                        format!("--at has \"{s}\", which is not a time in seconds"),
                        Some("Give seconds with commas: --at 1,2.5.".into()),
                    )
                }
            }
        }
        times
    };
    let sheet = if times.is_empty() && sheet.is_none() {
        Some(9)
    } else {
        sheet
    };
    // One file per kind of shot, so a later snapshot does not replace the one an agent is about to read.
    let out = out.unwrap_or_else(|| {
        // Next to a made video; for any other file in scriptvox-out, like every other output (test 4).
        let base = if video.parent().is_some_and(|p| p.ends_with("scriptvox-out")) {
            video.clone()
        } else {
            let _ = std::fs::create_dir_all("scriptvox-out");
            std::path::Path::new("scriptvox-out").join(video.file_name().unwrap_or_default())
        };
        base.with_extension(if let Some(n) = sheet {
            // The count is in the name: `--sheet 6` then `--sheet 9` wrote one file, and the second replaced the first.
            format!("sheet{n}.png")
        } else if cards {
            "cards.png".to_string()
        } else {
            let at: Vec<String> = times.iter().map(|t| format!("{t}")).collect();
            format!("snap-{}.png", at.join("_"))
        })
    });
    let out = std::path::absolute(&out).unwrap_or(out);
    if scriptvox::engine_ffmpeg::same_file(&out, &video) {
        return Envelope::fail(
            "snapshot",
            Exit::Usage,
            format!("--out is the input file: {}", video.display()),
            Some(
                "Give another --out, such as a .png: a snapshot cannot write over the video."
                    .into(),
            ),
        );
    }
    let zones = match safe.as_deref().map(scriptvox::platforms::find).transpose() {
        Ok(Some(p)) => match p.safe {
            Some(s) => Some(scriptvox::platforms::zones_filter(s)),
            None => {
                return Envelope::fail(
                    "snapshot",
                    Exit::Usage,
                    format!("{} has no screen zones on record", p.name),
                    Some("Use instagram-reels or tiktok, or drop --safe.".into()),
                )
            }
        },
        Ok(None) => None,
        Err(e) => {
            return Envelope::fail(
                "snapshot",
                Exit::Usage,
                e,
                Some("Give one of the names in the message.".into()),
            )
        }
    };
    match scriptvox::gates::snapshot_with(
        &ff,
        &video,
        &times,
        sheet,
        &out,
        zones.as_deref().unwrap_or(""),
    ) {
        Ok(ts) => {
            // Each frame's time and the caption on screen then, so a reviewer can tie a frame to a line of the brief.
            let srt = std::fs::read_to_string(video.with_extension("srt")).unwrap_or_default();
            let frames: Vec<_> = ts
                .iter()
                .enumerate()
                .map(|(i, &t)| {
                    let mut f = json!({"t": (t * 100.0).round() / 100.0, "caption": scriptvox::gates::caption_at(&srt, t)});
                    if let Some(c) = card_list.get(i) {
                        f["scene"] = c["scene"].clone();
                        f["card"] = c["kind"].clone();
                    }
                    f
                })
                .collect();
            let mut env = Envelope::ok("snapshot");
            env.artifacts.insert("image".into(), json!(out));
            env.with("frames", json!(frames))
        }
        Err(e) => Envelope::fail("snapshot", Exit::Error, e, None),
    }
}

/// One scene of a made video as its own MP4, next to it. The times are the ones `make` placed the scene at.
fn cut_scene(run: &str, video: &std::path::Path, n: usize, use_system: bool) -> Envelope {
    let usage =
        |msg: String, fix: &str| Envelope::fail("resume", Exit::Usage, msg, Some(fix.to_string()));
    let starts: Option<Vec<f64>> = std::fs::read_to_string(
        std::path::Path::new(".scriptvox/runs")
            .join(run)
            .join("scenes.json"),
    )
    .ok()
    .and_then(|t| serde_json::from_str(&t).ok());
    let Some(starts) = starts else {
        return usage(format!("{run} has no scene times"), "Use a run made from `scenes`. A run made before this option needs one full `scriptvox resume` first.");
    };
    if n == 0 || n > starts.len() {
        return usage(
            format!("{run} has {} scenes, not scene {n}", starts.len()),
            "Scenes count from 1.",
        );
    }
    let ff = match scriptvox::engine_ffmpeg::Ffmpeg::locate(use_system, true) {
        Ok(f) => f,
        Err(e) => return scriptvox::engine_ffmpeg::locate_failure("resume", e),
    };
    let (enc, _) = match ff.pick_encoder() {
        Ok(e) => e,
        Err(e) => return Envelope::fail("resume", Exit::Error, e, Some("scriptvox doctor".into())),
    };
    let total = match ff.duration(video) {
        Ok(t) => t,
        Err(e) => return Envelope::fail("resume", Exit::Error, e, None),
    };
    // The opening before the first word belongs to scene 1.
    let start = if n == 1 { 0.0 } else { starts[n - 1] };
    let end = starts.get(n).copied().unwrap_or(total).min(total);
    if end <= start {
        return Envelope::fail(
            "resume",
            Exit::Error,
            format!("scene {n} has no time in the video ({start:.2} to {end:.2} s)"),
            None,
        );
    }
    let stem = video
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let out = video.with_file_name(format!("{stem}.scene{n}.mp4"));
    let mut args: Vec<String> = [
        "-ss".to_string(),
        format!("{start:.3}"),
        "-i".into(),
        video.to_string_lossy().into_owned(),
        "-t".into(),
        format!("{:.3}", end - start),
    ]
    .to_vec();
    args.extend(enc.args());
    args.extend([
        "-c:a".into(),
        "aac".into(),
        out.to_string_lossy().into_owned(),
    ]);
    if let Err(e) = ff.run(std::path::Path::new("."), &args, None) {
        return Envelope::fail("resume", Exit::Error, e, None);
    }
    let mut env = Envelope::ok("resume").with("scene", json!(n));
    env.run_id = Some(run.to_string());
    env.artifacts.insert("video".into(), json!(out));
    for (k, v) in [
        ("cost_usd", 0.0),
        ("start_s", (start * 100.0).round() / 100.0),
        ("end_s", (end * 100.0).round() / 100.0),
    ] {
        env.metrics.insert(k.into(), json!(v));
    }
    env.next = vec![format!("scriptvox snapshot {} --sheet 4", out.display())];
    env
}

/// `resume <run> --brief f`: a child run `<run>.N` from the parent's request,
/// with the brief's fields and new flags on top. Detached unless `--wait`.
#[allow(clippy::too_many_arguments)]
fn resume(
    parent: &str,
    brief: Option<std::path::PathBuf>,
    patch: Option<std::path::PathBuf>,
    duration: Option<f64>,
    final_video: bool,
    wait: bool,
    only_scene: Option<usize>,
    use_system: bool,
    approve: Option<String>,
    max_spend: Option<f64>,
    plan_only: bool,
) -> Envelope {
    let busy = scriptvox::genjob::family(&std::path::Path::new(".scriptvox/runs").join(parent))
        .into_iter()
        .filter_map(|d| d.file_name()?.to_str().map(String::from))
        .chain([parent.to_string()])
        .find(|id| scriptvox::supervise::running(std::path::Path::new("."), id));
    if let Some(busy) = busy {
        return Envelope::fail(
            "resume",
            Exit::Usage,
            format!("{busy} is still running"),
            Some(format!(
                "Run `scriptvox wait {busy}` first, or `scriptvox cancel {busy}`."
            )),
        );
    }
    // Nothing changes: no plan, no provider, no new run. The scene comes from the video the run already has.
    if let (Some(n), false) = (
        only_scene,
        brief.is_some() || patch.is_some() || duration.is_some() || final_video,
    ) {
        return match resolve_video(parent) {
            Some(video) => cut_scene(parent, &video, n, use_system),
            None => no_video("resume", parent),
        };
    }
    let runs = std::path::Path::new(".scriptvox/runs");
    let req = runs.join(parent).join("request.json");
    let Some(mut a) = std::fs::read_to_string(&req)
        .ok()
        .and_then(|s| serde_json::from_str::<scriptvox::make::Args>(&s).ok())
    else {
        return Envelope::fail(
            "resume",
            Exit::Usage,
            format!("{parent} has no saved request"),
            Some("Run `scriptvox status` to see the runs.".into()),
        );
    };
    // A parent that kept a pace has no `duration` of its own: the one its pace was fitted to is the request's.
    let was_kept = a
        .pace
        .take()
        .map(|p| a.duration = a.duration.or(p.duration));
    let before = a.clone();
    if let Some(p) = brief {
        let b = match scriptvox::brief::Brief::load(&p).and_then(|b| {
            match scriptvox::brief::file_outside_project(&b) {
                Some(f) => Err(format!("{} is outside this project folder", f.display())),
                None => Ok(b),
            }
        }) {
            Ok(b) => b,
            Err(e) => {
                let fix = scriptvox::brief::load_fix(&e, &p);
                return Envelope::fail("resume", Exit::Usage, e, Some(fix));
            }
        };
        let line_words = b.lines.as_ref().map(|ls| {
            ls.iter()
                .map(|l| l.text.trim())
                .collect::<Vec<_>>()
                .join(" ")
        });
        if let Some(t) = line_words.or_else(|| b.script.clone()).or_else(|| {
            b.script_file
                .as_ref()
                .and_then(|f| std::fs::read_to_string(f).ok())
        }) {
            // New words: the scenes, cards and picks of the old ones no longer fit. The same words keep them.
            if a.media_match || t.trim() != a.script.trim() {
                a.scenes = None;
            }
            a.script = t;
        }
        a.lang = b.lang.or(a.lang);
        a.tts = b.tts.or(a.tts);
        a.voice = b.voice.or(a.voice);
        a.duration = b.duration.or(a.duration);
        a.aspect = b.aspect.unwrap_or(a.aspect);
        a.fps = b.fps.or(a.fps);
        a.media = b.media.or(a.media);
        a.stock = b.stock.map(|v| v.join(",")).or(a.stock);
        // The look comes from the revised brief alone: a field taken out of it is dropped (4 Nov 2026 eval: a
        // deleted hook came back). Language, voice, length, aspect and media keep the parent's when left out.
        a.music = b.music;
        a.music_from = b.music_from;
        a.music_to = b.music_to;
        a.music_prompt = b.music_prompt;
        a.base_video = b.base_video.or(a.base_video);
        a.base_sound = b.base_sound.or(a.base_sound);
        a.lines = b.lines.unwrap_or_default();
        a.hook = b.hook;
        a.sfx = b.sfx.unwrap_or(false);
        a.cut_on = b.cut_on;
        a.upscale = b.upscale.unwrap_or(false);
        a.smooth = b.smooth.unwrap_or(false);
        a.ambience = b.ambience.unwrap_or(false);
        a.logo = b.logo;
        a.accent = b.accent;
        a.music_level = b.music_level;
        a.background = b.background;
        a.waveform = b.waveform.unwrap_or(false);
        a.style = b.style;
        a.seed = b.seed.or(a.seed);
        a.character = b.character;
        a.characters = b.characters;
        a.emphasis = b.emphasis;
        a.captions_second = b.captions_second;
        a.media_match = b.media_match.unwrap_or(a.media_match);
        a.font = b.font;
        a.narration = b.narration;
        a.caption_at = b.caption_at;
        a.caption_size = b.caption_size;
        a.card_style = b.card_style;
        a.size = b.size.or(a.size);
        a.caption_outline = b.caption_outline;
        a.caption_box = b.caption_box;
        a.loop_seam = b.r#loop.unwrap_or(false);
        a.voice_rate = b.voice_rate.or(a.voice_rate);
        a.outro = b.outro;
        a.captions = b.captions;
        a.transition = b.transition;
        a.look = b.look;
        if let Some(sc) = b.scenes.clone() {
            a.script = sc
                .iter()
                .map(|s| s.script.trim())
                .collect::<Vec<_>>()
                .join(" ");
            // Scenes with `seconds` and no `duration` in the new brief: the length is theirs, not the parent's.
            if b.duration.is_none() && sc.iter().any(|s| s.seconds.is_some()) {
                a.duration = None;
            }
            a.scenes = Some(sc);
        }
    }
    let mut patched = vec![];
    if let Some(p) = patch {
        let v = match std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => {
                return Envelope::fail(
                    "resume",
                    Exit::Usage,
                    format!("{}: {e}", p.display()),
                    Some("Give a JSON object with the fields that change.".into()),
                )
            }
        };
        match scriptvox::make::patch_request(&a, &v) {
            Ok((b, keys)) => {
                a = b;
                patched = keys;
            }
            Err(e) => {
                return Envelope::fail(
                    "resume",
                    Exit::Usage,
                    e,
                    Some("Run `scriptvox schema brief` for the fields.".into()),
                )
            }
        }
    }
    if final_video {
        a.draft = false;
    }
    // A run that stopped on purpose (`make --until voice`): its child goes on to the video, and to the `--out` the
    // parent never wrote.
    let stopped = a.until.take().is_some();
    // Some scenes keep their words and voice: the child keeps the parent's pace, so their voice comes from the cache
    // with the same samples, and only the other scenes are voiced (docs/specs/resume-scene-voice.md). `--duration`
    // asks for a new fit of all scenes.
    let pace = std::fs::read_to_string(runs.join(parent).join(scriptvox::make::PACE_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<scriptvox::make::Pace>(&s).ok());
    if let (Some(p), None, Some(old), Some(new)) = (pace, duration, &before.scenes, &a.scenes) {
        let (again, kept) = scriptvox::make::scenes_voiced_again(old, new);
        let same_voice = (&a.tts, &a.voice, &a.voice_rate, &a.lang)
            == (&before.tts, &before.voice, &before.voice_rate, &before.lang)
            && a.duration == p.duration
            && a.lines.is_empty()
            && a.narration.is_none()
            // A base video sets the length itself. `media_match` is voiced as one part.
            && a.base_video.is_none()
            && !a.media_match;
        if same_voice && !kept.is_empty() && (!again.is_empty() || was_kept.is_some()) {
            a.duration = None;
            a.pace = Some(p);
        }
    }
    a.duration = duration.or(a.duration);
    a.approve = approve;
    a.max_spend = max_spend.or(a.max_spend);
    let child = (1..)
        .map(|n| format!("{parent}.{n}"))
        .find(|c| !runs.join(c).exists())
        .unwrap();
    a.parent_run = Some(parent.to_string());
    a.idempotency_key = None;
    // The child writes next to the parent's video, not over it: rushed.mp4, then rushed.1.mp4, rushed.1.1.mp4
    // (hand eval 4 Nov 2026: each resume overwrote the last video, so before and after could not be compared).
    // A parent that wrote no video (stopped, failed or killed before the render) leaves the name free: the child takes it.
    if let Some(out) = a.out.as_ref().filter(|o| !stopped && o.exists()) {
        let n = child.rsplit('.').next().unwrap_or("1");
        let stem = out
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = out
            .extension()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "mp4".into());
        a.out = Some(out.with_file_name(format!("{stem}.{n}.{ext}")));
    }
    // `--plan`: the plan of the child, and no run. The child id stays free.
    if plan_only {
        a.plan_only = true;
        a.run_id = None;
        let env = scriptvox::make::run(a).with("parent_run", json!(parent));
        return if patched.is_empty() {
            env
        } else {
            env.with("patched", json!(patched))
        };
    }
    if wait || cfg!(windows) || only_scene.is_some() {
        a.run_id = Some(child.clone());
        let mut env = scriptvox::make::run(a);
        // The normal resume's plan and approval decided this run. A stop before the video leaves nothing to cut.
        if let (Some(n), Some(video)) = (
            only_scene,
            env.artifacts
                .get("video")
                .and_then(|v| v.as_str())
                .map(std::path::PathBuf::from),
        ) {
            let cut = cut_scene(&child, &video, n, use_system);
            match (cut.ok, cut.error) {
                (true, _) => {
                    env.artifacts
                        .insert("scene_video".into(), cut.artifacts["video"].clone());
                }
                // The paid run is done: its id and video stay in the answer, or a retry would pay again.
                (false, e) => env
                    .findings
                    .push(json!({"gate": "only_scene", "status": "warn",
                    "detail": e.as_ref().map_or("the scene was not cut", |e| e.message.as_str()),
                    "fix": e.as_ref().and_then(|e| e.fix.clone())})),
            }
        }
        return if patched.is_empty() {
            env
        } else {
            env.with("patched", json!(patched))
        };
    }
    let run = match scriptvox::run::Run::create_with_id(std::path::Path::new("."), "make", &child) {
        Ok(r) => r,
        Err(e) => {
            return Envelope::fail(
                "resume",
                Exit::Error,
                format!("cannot create {child}: {e}"),
                None,
            )
        }
    };
    run.event("progress", json!({"explain": format!("child of {parent}")}));
    let _ = std::fs::write(
        run.dir.join("request.json"),
        serde_json::to_string_pretty(&a).unwrap_or_default(),
    );
    match scriptvox::supervise::spawn(&run, &["_run".to_string(), child.clone()]) {
        Ok(_) => {
            let mut env = Envelope::ok("resume").with("parent_run", json!(parent));
            // Where the video will be, so nobody looks for the parent's name.
            let out = a.out.clone().unwrap_or_else(|| {
                std::path::Path::new("scriptvox-out").join(format!("{child}.mp4"))
            });
            env = env.with("out", json!(out));
            if !patched.is_empty() {
                env = env.with("patched", json!(patched));
            }
            env.status = "running";
            env.run_id = Some(child.clone());
            env.events_file = Some(run.dir.join("events.jsonl").to_string_lossy().into_owned());
            env.next = vec![format!("scriptvox wait {child}")];
            env
        }
        Err(e) => Envelope::fail(
            "resume",
            Exit::Error,
            format!("cannot start the supervisor: {e}"),
            None,
        ),
    }
}

/// The recipe in an MP4's `comment` tag.
fn read_recipe(cmd: &str, file: &std::path::Path) -> Result<serde_json::Value, Box<Envelope>> {
    if !file.is_file() {
        return Err(Box::new(Envelope::fail(
            cmd,
            Exit::Usage,
            format!("{} does not exist or is not a file", file.display()),
            Some("Give the path of an MP4 made by scriptvox.".into()),
        )));
    }
    let ff = scriptvox::engine_ffmpeg::Ffmpeg::locate(true, false)
        .map_err(|e| Box::new(scriptvox::engine_ffmpeg::locate_failure(cmd, e)))?;
    ff.recipe(file).map_err(|e| {
        Box::new(Envelope::fail(
            cmd,
            Exit::Error,
            e,
            Some("Only MP4s made by scriptvox (without --no-embed) carry a recipe.".into()),
        ))
    })
}

/// The request of `replay` (cache only) or `remake` (provider calls allowed), from the recipe in a file. A recipe is
/// text from a file the user may not have made, so only the video's own fields come from it. Where it is written, the
/// run it joins, its key, the approval and the spend cap are this command's, never the recipe's.
fn from_recipe(
    file: &std::path::Path,
    out: Option<std::path::PathBuf>,
    cache_only: bool,
    cli: &Cli,
) -> Result<scriptvox::make::Args, Box<Envelope>> {
    let cmd = if cache_only { "replay" } else { "remake" };
    let r = read_recipe(cmd, file)?;
    let mut a: scriptvox::make::Args =
        serde_json::from_value(r["request"].clone()).map_err(|e| {
            Box::new(Envelope::fail(
                cmd,
                Exit::Error,
                format!("the recipe cannot be read by this version: {e}"),
                None,
            ))
        })?;
    // The MP4 may come from someone else: its recipe does not make this machine send a request to an address in it.
    // A replay reads only the store and sends nothing.
    if let Some(m) = a
        .music
        .as_deref()
        .filter(|m| !cache_only && scriptvox::fetch::is_address(m))
    {
        return Err(Box::new(Envelope::fail(
            cmd,
            Exit::Usage,
            format!(
                "the recipe takes music from an address ({}), and a recipe does not download from an address",
                scriptvox::fetch::public_url(m)
            ),
            Some(format!(
                "If you trust the address, download it yourself (`scriptvox fetch <address> --audio`). Then run `scriptvox explain {}`, save recipe.request as brief.json with that file as music, and run `scriptvox make --brief brief.json`.",
                file.display()
            )),
        )));
    }
    a.out = out;
    a.run_id = None;
    a.idempotency_key = None;
    a.parent_run = None;
    a.cache_only = cache_only;
    // `--local-only` can only add a limit: a recipe that asks for it keeps it.
    a.local_only |= cli.local_only;
    a.approve = cli.approve.clone();
    // Not the recipe's cap: a high one would let a run spend more than the plan the user approved.
    a.max_spend = cli.max_spend;
    a.no_embed = cli.no_embed;
    a.plan_only = cli.plan;
    a.use_system = cli.use_system;
    Ok(a)
}

/// An answer that `make` wrote, as the answer of `cmd`.
fn as_command(cmd: &str, mut env: Envelope) -> Envelope {
    env.command = cmd.to_string();
    env
}

fn run_recipe(
    cmd: &str,
    file: &std::path::Path,
    a: Result<scriptvox::make::Args, Box<Envelope>>,
) -> Envelope {
    match a {
        Ok(a) => recipe_answer(cmd, file, scriptvox::make::run(a)),
        Err(e) => *e,
    }
}

/// `make` wrote this answer; the user ran `replay` or `remake`, so the next step names that command.
fn recipe_answer(cmd: &str, file: &std::path::Path, env: Envelope) -> Envelope {
    let mut env = as_command(cmd, env);
    if let (true, Some(p)) = (env.status == "approval_needed", &env.plan_id) {
        if let Some(n) = env
            .next
            .first_mut()
            .filter(|n| n.starts_with("scriptvox make "))
        {
            *n = format!("scriptvox {cmd} {} --approve {p}", file.display());
        }
    }
    env.with("from", json!(file))
}

/// `remake --detach`: every check of the recipe runs here, in this process, before a run exists. Reading the file, the
/// request, `make`'s own checks up to its plan (files inside the project, stock picks, names and sizes), then the
/// approval. Any stop is answered at once with no run. The supervisor reads the file again and makes the same checks.
fn detach_remake(file: &std::path::Path, out: Option<std::path::PathBuf>, cli: &Cli) -> Envelope {
    let mut a = match from_recipe(file, out, false, cli) {
        Ok(a) => a,
        Err(e) => return *e,
    };
    a.plan_only = true;
    let plan = scriptvox::make::run(a);
    let v = serde_json::to_value(&plan).unwrap_or_default();
    if plan.exit != Exit::Ok || v["plan_id"].as_str().is_none() {
        return recipe_answer("remake", file, plan);
    }
    let env = detach_start(
        "remake",
        &v,
        cli.idempotency_key.as_deref(),
        Some("remake"),
        cli.approve.as_deref(),
        cli.max_spend,
    );
    recipe_answer("remake", file, env)
}

const KOKORO_NOTICE: &str = "The local voice uses Kokoro (model: Apache-2.0) with phonemizer and espeak-ng (GPL-3.0). They run in a separate plugin process, never inside the Apache-2.0 binary. Download: about 120 MB of model files plus about 60 MB of Python packages.";

/// `setup speech-local`: opt-in, because it is about 490 MB. All its parts are MIT, so no notice to accept.
fn setup_speech_local(dry_run: bool) -> Envelope {
    use scriptvox::provision as p;
    let done = || {
        Envelope::ok("setup")
            .with("speech_local", json!("installed"))
            .with("licence", json!("faster-whisper and the small model: MIT"))
    };
    if p::whisper_small_ready() {
        return done();
    }
    if dry_run {
        let pins = plugin_pins(&p::WHISPER_PINS);
        return scriptvox::doctor::setup_dry_run(&pins)
            .with("licence", json!("faster-whisper and the small model: MIT"))
            .with("note", json!(PLUGIN_PACKAGES_NOTE));
    }
    match p::install_whisper(&p::WHISPER_PINS, "model", &|m| eprintln!("scriptvox: {m}")) {
        Ok(()) => done(),
        Err(e) => scriptvox::doctor::setup_failure(e),
    }
}

/// `setup speech-large`: opt-in, about 1.6 GB. Better Arabic, and it runs when free memory holds it.
fn setup_speech_large(dry_run: bool) -> Envelope {
    use scriptvox::provision as p;
    const LICENCE: &str = "faster-whisper and large-v3-turbo: MIT";
    let done = || {
        Envelope::ok("setup")
            .with("speech_large", json!("installed"))
            .with("licence", json!(LICENCE))
    };
    if p::whisper_large_ready() {
        return done();
    }
    if dry_run {
        let pins = plugin_pins(&p::WHISPER_LARGE_PINS);
        return scriptvox::doctor::setup_dry_run(&pins)
            .with("licence", json!(LICENCE))
            .with("note", json!(PLUGIN_PACKAGES_NOTE));
    }
    match p::install_whisper(&p::WHISPER_LARGE_PINS, "model-large", &|m| {
        eprintln!("scriptvox: {m}")
    }) {
        Ok(()) => done(),
        Err(e) => scriptvox::doctor::setup_failure(e),
    }
}

/// `setup image-local`: opt-in, because it is about 5 GB. No token: the model repos are not gated.
fn setup_image_local(dry_run: bool) -> Envelope {
    use scriptvox::provision as p;
    const LICENCE: &str = "sd.cpp: MIT. FLUX.2 klein 4B and Qwen3 4B: Apache-2.0";
    let note = if cfg!(target_os = "linux") {
        "The Linux build uses Vulkan or the CPU and needs glibc 2.38 (Ubuntu 24.04, Debian 13 or newer). Speed on a card is not measured. NVIDIA on Linux has no CUDA pack here yet, because that pack has no host. The next upstream build (1052) on 2 CPU cores: 446 s at 512x512. An own CUDA build, not this one, on one H100: about 44 s a picture, 3.9 GB on the card."
    } else {
        "This build was not run on this system. An own CUDA build, not this one, on one H100: about 44 s a picture, 3.9 GB on the card."
    };
    let done = || {
        Envelope::ok("setup")
            .with("image_local", json!("installed"))
            .with("licence", json!(LICENCE))
            .with("note", json!(note))
    };
    if p::image_ready() {
        return done();
    }
    let Some(tool) = p::pin("sd-cli") else {
        return Envelope::fail(
            "setup",
            Exit::Usage,
            format!(
                "no image build is pinned for this machine ({})",
                p::target()
            ),
            Some("Use a self-hosted or paid image row: `scriptvox providers`.".into()),
        );
    };
    if dry_run {
        let pins: Vec<_> = p::IMAGE_PINS.iter().filter_map(|n| p::pin(n)).collect();
        let env = scriptvox::doctor::setup_dry_run(&pins);
        let expected = scriptvox::doctor::image_tier(&scriptvox::doctor::gpus());
        return env
            .with("expected", expected)
            .with("licence", json!(LICENCE))
            .with("note", json!(note));
    }
    let say = |m: &str| eprintln!("scriptvox: {m}");
    let net = scriptvox::doctor::setup_failure;
    let bin = match p::install(&tool, &say) {
        Ok(mut b) => b.remove(0),
        Err(e) => return net(e),
    };
    // Before the 5 GB of model files.
    if let Err(e) = p::sd_cli_starts(&bin) {
        return Envelope::fail(
            "setup",
            Exit::Usage,
            format!("the pinned image build does not start on this machine: {e}"),
            Some("This system is too old for the upstream build, and scriptvox has no other pack yet. Use a self-hosted or paid image row: `scriptvox providers`.".into()),
        );
    }
    for n in &p::IMAGE_PINS[1..] {
        let Some(pin) = p::pin(n) else {
            return net(format!("no pin for {n}"));
        };
        if let Err(e) = p::install(&pin, &say) {
            return net(e);
        }
    }
    done()
}

/// The plugin needs the Python packages too, and uv fetches them with no pin of ours to read a size from.
const PLUGIN_PACKAGES_NOTE: &str =
    "The list has the pinned files. The Python packages of the plugin come on top, through uv (about 400 MB for speech-local, measured 5 Nov).";

/// What a plugin setup fetches: uv when it is missing, and the plugin's own pinned files.
fn plugin_pins(names: &[&str]) -> Vec<scriptvox::provision::Pin> {
    let tools = scriptvox::provision::pending(false, true)
        .into_iter()
        .filter(|p| p.name == "uv");
    tools
        .chain(names.iter().filter_map(|n| scriptvox::provision::pin(n)))
        .collect()
}

fn setup_voice_local(dry_run: bool, accept_gpl: bool) -> Envelope {
    // Before any download: past this length the voice installs well and then crashes at the first `make`.
    if let Some((n, most)) = scriptvox::provision::kokoro_path_excess() {
        return Envelope::fail(
            "setup",
            Exit::Usage,
            format!(
                "the local voice cannot run from this data folder: its path is {n} characters too long for espeak-ng ({})",
                scriptvox::provision::data_dir().display()
            ),
            Some(format!(
                "Set SCRIPTVOX_DATA to a folder with a path {n} characters shorter (at most {most} in all), and run this again with it set for every scriptvox command."
            )),
        );
    }
    let sizes: u64 = ["kokoro-model", "kokoro-voices"]
        .iter()
        .filter_map(|n| scriptvox::provision::pin(n))
        .map(|p| p.size)
        .sum();
    if scriptvox::provision::kokoro_ready() {
        return Envelope::ok("setup")
            .with("voice_local", json!("installed"))
            .with("notice", json!(KOKORO_NOTICE));
    }
    if dry_run {
        let mut env =
            scriptvox::doctor::setup_dry_run(&plugin_pins(&["kokoro-model", "kokoro-voices"]))
                .with("note", json!(PLUGIN_PACKAGES_NOTE));
        env.metrics.insert("download_bytes".into(), json!(sizes));
        return env.with("notice", json!(KOKORO_NOTICE));
    }
    // GPL-3.0 needs no acceptance to run a program: the notice is shown once, at the install (audit M3).
    let _ = accept_gpl; // ponytail: the flag is accepted and ignored for one version, then removed.
    eprintln!("scriptvox: {KOKORO_NOTICE}");
    match scriptvox::provision::install_kokoro(&|m| eprintln!("scriptvox: {m}")) {
        Ok(()) => Envelope::ok("setup")
            .with("voice_local", json!("installed"))
            .with("voices", json!(["af_heart", "am_michael"]))
            .with("notice", json!(KOKORO_NOTICE)),
        Err(e) => scriptvox::doctor::setup_failure(e),
    }
}

fn providers_add(
    name: &str,
    gateway: &str,
    kind: &str,
    t2v: Option<String>,
    i2v: Option<String>,
    base: Option<String>,
) -> Envelope {
    // Quoted by the TOML writer: a value cannot add a line such as `key = "fal"` to the row.
    let q = |v: &str| toml::Value::String(v.to_string()).to_string();
    let mut block = format!(
        "[[generator]]\nname = {}\ngateway = {}\nkind = {}\n",
        q(name),
        q(gateway),
        q(kind)
    );
    for (k, v) in [("t2v", t2v), ("i2v", i2v), ("base", base)] {
        if let Some(v) = v {
            block.push_str(&format!("{k} = {}\n", q(&v)));
        }
    }
    match scriptvox::generators::add(&block) {
        Ok(file) => {
            let mut env = Envelope::ok("providers").with("added", json!(name)).with("file", json!(file));
            env.next.push(format!("scriptvox generate --model {name} --prompt \"...\" --max-spend 1"));
            env.next.push(format!("Write the real price and terms into {} to run it without --max-spend.", file.display()));
            env
        }
        Err(errs) => Envelope::fail("providers", Exit::Usage, errs.join("; "), Some("Correct these fields; nothing was written. `scriptvox providers template <gateway>` shows a whole block.".into())),
    }
}

fn models_scan(folder: Option<std::path::PathBuf>) -> Envelope {
    let Some(dir) = folder.or_else(scriptvox::provision::hf_cache) else {
        return Envelope::fail(
            "models",
            Exit::Usage,
            "no folder and no Hugging Face cache".to_string(),
            Some("Give a folder: `scriptvox models scan <folder>`.".into()),
        );
    };
    if !dir.is_dir() {
        return Envelope::fail(
            "models",
            Exit::Usage,
            format!("{} is not a folder", dir.display()),
            Some("Give a folder that holds model files.".into()),
        );
    }
    match scriptvox::provision::scan_models(&dir) {
        Ok((found, seen, hashed)) => {
            let list: Vec<serde_json::Value> = found
                .iter()
                .map(|(n, p, b)| json!({"pin": n, "file": p, "bytes": b}))
                .collect();
            let mut env = Envelope::ok("models")
                .with("folder", json!(dir))
                .with("found", json!(list));
            env.metrics.insert("files_seen".into(), json!(seen));
            env.metrics.insert("files_hashed".into(), json!(hashed));
            env.metrics.insert("found".into(), json!(found.len()));
            if found.is_empty() {
                env.next.push("Nothing matched. `scriptvox setup <target> --dry-run` shows what would be downloaded.".into());
            } else {
                env.next
                    .push("`scriptvox setup <target> --dry-run` now skips the files found.".into());
            }
            env
        }
        Err(e) => Envelope::fail(
            "models",
            Exit::Error,
            e,
            Some("Check that this folder can be read and the data folder can be written.".into()),
        ),
    }
}

fn setup_voice_rust(dry_run: bool) -> Envelope {
    use scriptvox::kokoro_rust as k;
    const NOTE: &str = "A second local English voice in pure Rust on the CPU: Kokoro-82M (Apache-2.0) and CMUdict (BSD-2-Clause), no Python and no GPL part. Use it with `--tts kokoro-rust`.";
    if k::ready() {
        return Envelope::ok("setup")
            .with("voice_rust", json!("installed"))
            .with("notice", json!(NOTE));
    }
    if dry_run {
        let pins: Vec<_> = k::PINS
            .iter()
            .filter_map(|n| scriptvox::provision::pin(n))
            .collect();
        return scriptvox::doctor::setup_dry_run(&pins).with("notice", json!(NOTE));
    }
    match k::install_files(&|m| eprintln!("scriptvox: {m}")) {
        Ok(()) => Envelope::ok("setup")
            .with("voice_rust", json!("installed"))
            .with("voices", json!(["af_heart", "am_michael"]))
            .with("notice", json!(NOTE)),
        Err(e) => scriptvox::doctor::setup_failure(e),
    }
}

fn emit(env: &Envelope, json_mode: bool, r: &Redactor) {
    use std::io::Write;
    // A closed pipe (`scriptvox ... | head -1`) ends the output quietly, never with a panic.
    let out = |s: String| {
        let _ = writeln!(std::io::stdout(), "{s}");
    };
    let err = |s: String| {
        let _ = writeln!(std::io::stderr(), "{s}");
    };
    if json_mode {
        out(r.apply(&env.to_json()));
        return;
    }
    // A person at a terminal gets words. `--json`, an agent and a pipe never reach this line.
    for line in env.human_lines() {
        if env.error.is_some() {
            err(r.apply(&line));
        } else {
            out(r.apply(&line));
        }
    }
}
