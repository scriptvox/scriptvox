//! The Claude Code skill, generated from the binary itself (PLAN-R1 §4): the
//! command list comes from clap, the exit table from `exit.rs`, and the brief
//! fields from the brief schema. A test fails when the committed
//! `plugin/skills/scriptvox/SKILL.md` differs, so help text and docs never drift
//! (the drift issues of OM #493, HF #5025, MGL #1243).

use crate::exit::Exit;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The first line that carries the version, read back by `doctor`.
pub fn stamp() -> String {
    format!("<!-- scriptvox-skill-version: {VERSION} -->")
}

pub fn render() -> String {
    let mut s = String::new();
    let push = |s: &mut String, l: &str| {
        s.push_str(l);
        s.push('\n');
    };
    push(&mut s, "---");
    push(&mut s, "name: scriptvox");
    push(
        &mut s,
        "description: Make narrated, captioned short videos (MP4) from a script with the scriptvox CLI, in English, Arabic, Spanish, French, German, Portuguese or Indonesian, from your own clips, stock footage or a built-in background. Use when the user asks for a video, a short, a reel, a voice-over video or captions. Do not use for video editing of an existing timeline.",
    );
    push(&mut s, "---");
    push(&mut s, &stamp());
    push(&mut s, "");
    push(&mut s, "# scriptvox");
    push(&mut s, "");
    push(&mut s, "You write the script. scriptvox makes the video. Every command prints one JSON envelope with `--json`. Read `status`, `error.fix`, `needs_input` and `next`.");
    push(&mut s, "");
    push(&mut s, "## Steps");
    push(&mut s, "");
    for l in [
        "1. Run `scriptvox --version --json`. If it fails, tell the user to install scriptvox, and stop.",
        "2. Run `scriptvox doctor --json`. If exit 1, run `scriptvox doctor --fix --json`. If exit 10, show `needs_input[].human_action` to the user.",
        "3. Write `brief.json` with the full narration (fields below). You are the script writer. For a script that states numbers or dates, research first, then write: collect the facts with their sources, write the script from those facts only, and put each fact in `sources`. With your own web search you do this yourself. With none, and only when the user has set up a text row: `scriptvox research \"<topic>\" --json`, then `scriptvox write --from research.json --json` (with no text row both stop with exit 10 and the one command for the user to run).",
        "4. Run `scriptvox plan --brief brief.json --json`. With `sources` in the brief, read the `claims` finding: it names each number or date of the script that is in no source (`--strict` makes that a failure). If `needs_input` has an item of kind `approval`, or `metrics.cost_usd` is above 0: show the price to the person and ask. Add `--approve <plan_id>` only after the person says yes. An agent must not approve, or raise `--max-spend`, by its own decision. For other `needs_input` items, take a free alternative from `alternatives`.",
        "5. Run `scriptvox make --brief brief.json --detach --json`, then `scriptvox wait <run_id> --json`. Always `--detach`: a make takes minutes, and without it the run stops when your shell limit stops the command. `wait` returns after 100 s; exit 9 is not a failure, the run is still going: run the command in `error.fix` again (with a 10-minute shell limit, `--timeout 540`). Never use sleep. If you are not sure a run is going, `scriptvox status <run_id> --json` says `alive` (true or false), `stage` and `idle_s`; when `alive` is false and there is no video, `scriptvox resume <run_id> --json` goes on from what is finished. Never start a second `make` for the same video.",
        "6. Run `scriptvox snapshot <run_id> --sheet 9 --json` and Read the PNG at `artifacts.image`. Check the frames and the captions. With scene cards, also `--cards` (one frame per card, after it has landed).",
        "7. To fix, edit `brief.json` and run `scriptvox resume <run_id> --brief brief.json --json`, then `wait` on the new `run_id`. The new video is next to the old one (`name.1.mp4`; the answer's `out` field says where). Do at most 2 rounds. For a video over about 30 s, add `--draft` to the first `make` (half size, 15 fps, about 8 times faster on an 84 s video), and when the frames look right run `scriptvox resume <run_id> --final --json`. A draft has no `words` gate: the final runs it. To check the voice and the fit before any picture, run `scriptvox make --brief brief.json --until voice --json`, then `scriptvox resume <run_id> --json`. With `generate` scenes, run `scriptvox make --brief brief.json --storyboard --json` first: one still per scene on one sheet, and no clip.",
        "8. Report `artifacts.video` (in `scriptvox-out/`), `metrics.duration_s`, the cost, and the voice terms from the result's `explain` field (the `scriptvox explain` command shows only the recipe).",
    ] {
        push(&mut s, l);
    }
    push(&mut s, "");
    push(&mut s, "## Rules");
    push(&mut s, "");
    push(&mut s, "- Never ask the user for a key in chat. On `kind:\"secret\"`, show the `human_action` (the user runs `scriptvox keys set <provider>` in a terminal).");
    push(
        &mut s,
        "- Never print a key. Never poll with sleep: use `wait`.",
    );
    push(&mut s, "- A file is opened only as what its first bytes say: PNG, JPEG, WebP, BMP, MP4, MOV, MKV, WebM, AVI, MPEG-TS, WAV, MP3, FLAC, OGG, M4A, AAC. A GIF, a TIFF, a playlist or a URL as `media`, `logo`, `narration` or `base_video` stops at exit 2. Only `music` takes an https address (after `scriptvox setup fetch`, which the person runs).");
    push(&mut s, "- Free first: Edge TTS, own media, the built-in background. Paid stages always need `--approve`. The approval belongs to the person.");
    push(
        &mut s,
        "- Terms: every row is allowed and the user decides. The terms are data in `providers`, `voices`, `credits.txt` and `metadata.json` (`commercial_ok`). Give them when the person asks.",
    );
    push(&mut s, "");
    push(&mut s, "## Brief fields");
    push(&mut s, "");
    let (fields, more) = brief_fields();
    s.push_str(&fields(true));
    push(&mut s, &format!("- More fields, each with one line in the skill `{REFERENCE}` (read it when you need one; `scriptvox schema brief --json` has every field of a scene and of a card): {more}."));
    push(&mut s, "");
    push(&mut s, "Example: `{\"script\": \"Dates are an old food of the Gulf. [pause 1] They give fast energy.\", \"lang\": \"en\", \"duration\": 20, \"media\": \"clips\", \"music\": \"calm\"}`");
    push(&mut s, "");
    push(&mut s, "Scene cards: a scene can carry a `card` (motion text over it): `title`, `stat`, `lower_third`, `list`, `bars`, `line`, `formula`, `code`, `diff`, `terminal`, `plot`, `quote`, `compare`, `donut`, `timeline`, `table`, `number_line`, `diagram`, `steps`, `progress` or `gauge`, for example `{\"script\": \"...\", \"card\": {\"kind\": \"list\", \"items\": [\"Shade\", \"Cool air\"]}}`. Or `cards`, 1 to 4 cards that show one after the other in the scene, each with `on` (a spoken word) or `at` (seconds from the scene's start) and `hold`. The fields of each kind: `scriptvox schema brief`, or the scriptvox-brief skill.");
    push(&mut s, "Pause tags: `[pause 1.5]`, `(silence: 800ms)`, Arabic `[توقف ١٫٥ ثانية]`. Arabic scripts get Arabic voices and correct right-to-left captions. Spanish, French, German, Portuguese and Indonesian scripts get their own voices (`lang` es, fr, de, pt, id).");
    push(&mut s, "");
    push(&mut s, "## Commands");
    push(&mut s, "");
    let (commands, more) = command_lines();
    s.push_str(&commands(true));
    push(&mut s, &format!("- More commands, each with one line in the skill `{REFERENCE}` (or `scriptvox --help`): {more}."));
    push(&mut s, "");
    push(&mut s, "## Exit codes");
    push(&mut s, "");
    for e in Exit::ALL {
        push(
            &mut s,
            &format!("- {}: {} ({})", e.code(), e.meaning(), e.name()),
        );
    }
    s
}

/// The second generated skill: the brief fields and the commands the main skill does not list.
pub const REFERENCE: &str = "scriptvox-reference";

/// One line per brief field, from the brief schema: a writer of the common ones (`true`, `brief::COMMON_FIELDS`)
/// or of the others, and the names of the others.
fn brief_fields() -> (impl Fn(bool) -> String, String) {
    let props = crate::brief::schema("brief")
        .and_then(|v| v["properties"].as_object().cloned())
        .unwrap_or_default();
    let common = |k: &str| crate::brief::COMMON_FIELDS.contains(&k);
    let more: Vec<String> = props
        .keys()
        .filter(|k| !common(k))
        .map(|k| format!("`{k}`"))
        .collect();
    let lines = move |want: bool| {
        props
            .iter()
            .filter(|(k, _)| common(k) == want)
            .map(|(k, v)| {
                let d = v["description"].as_str().unwrap_or("").replace('\n', " ");
                format!("- `{k}`: {d}\n")
            })
            .collect()
    };
    (lines, more.join(", "))
}

/// One line per command, from clap: a writer of the common ones (`true`, `cli::COMMON_COMMANDS`) or of the
/// others, and the names of the others.
fn command_lines() -> (impl Fn(bool) -> String, String) {
    let cmd = crate::cli::tree();
    let common = |n: &str| crate::cli::COMMON_COMMANDS.contains(&n);
    let more: Vec<String> = cmd
        .get_subcommands()
        .filter(|c| !c.is_hide_set() && !common(c.get_name()))
        .map(|c| format!("`{}`", c.get_name()))
        .collect();
    let lines = move |want: bool| {
        let mut s = String::new();
        for sub in cmd
            .get_subcommands()
            .filter(|c| !c.is_hide_set() && common(c.get_name()) == want)
        {
            let about = sub.get_about().map(|a| a.to_string()).unwrap_or_default();
            s.push_str(&format!("- `scriptvox {}`: {about}\n", sub.get_name()));
            #[cfg(feature = "fetch-youtube")]
            if sub.get_name() == "fetch" {
                for l in crate::fetch_helper::SKILL {
                    s.push_str(l);
                    s.push('\n');
                }
            }
        }
        s
    };
    (lines, more.join(", "))
}

/// The second skill, generated like the main one: every brief field and command that the main skill names but
/// does not describe. The lists `brief::COMMON_FIELDS` and `cli::COMMON_COMMANDS` decide what stays in the main one.
pub fn render_reference() -> String {
    format!(
        "---\nname: {REFERENCE}\ndescription: The less common brief fields and commands of the scriptvox CLI, one line each. Read it when the scriptvox skill names a brief field or a command and does not describe it, for example a dub on a base video, generated clips, a brand file, a batch, joining clips or the MCP server.\n---\n{}\n\n# scriptvox: more brief fields and commands\n\nThe `scriptvox` skill has the steps, the rules, the common brief fields and commands, and the exit codes. This file has the rest. Both are generated from the tool. For the fields of a scene and of a card: `scriptvox schema brief --json`. For the flags of a command: `scriptvox <command> --help`.\n\n## Brief fields\n\n{}\n## Commands\n\n{}",
        stamp(),
        brief_fields().0(false),
        command_lines().0(false)
    )
}

/// Where `skills install` writes: the user's Claude skills folder.
pub fn install_dir() -> std::path::PathBuf {
    home().join(".claude/skills/scriptvox")
}

fn home() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| ".".into())
}

/// The folder that Codex, Cursor and Gemini CLI read (their docs, read 7 Nov 2026), and also opencode and Pi
/// (13 Nov 2026: `opencode debug skill` listed a skill from it; the Pi docs name it).
pub fn agents_dir() -> std::path::PathBuf {
    home().join(".agents/skills/scriptvox")
}

/// The home folder of Hermes Agent: `HERMES_HOME`, else its default place. Hermes does not read `~/.agents/skills`;
/// it reads `skills/` in this folder (13 Nov 2026: `hermes skills list` listed a skill from it).
pub fn hermes_home() -> std::path::PathBuf {
    hermes_home_from(
        &home(),
        std::env::var_os("HERMES_HOME").map(Into::into),
        std::env::var_os("LOCALAPPDATA").map(Into::into),
    )
}

fn hermes_home_from(
    home: &std::path::Path,
    hermes: Option<std::path::PathBuf>,
    local_appdata: Option<std::path::PathBuf>,
) -> std::path::PathBuf {
    let set =
        |p: &Option<std::path::PathBuf>| p.as_ref().filter(|p| !p.as_os_str().is_empty()).cloned();
    set(&hermes)
        .or_else(|| {
            set(&local_appdata)
                .filter(|_| cfg!(windows))
                .map(|p| p.join("hermes"))
        })
        .unwrap_or_else(|| home.join(".hermes"))
}

/// One row of the answer of `skills install --agents`: which agents, where, and what was done.
#[derive(Debug, serde::Serialize)]
pub struct AgentTarget {
    pub agents: &'static str,
    pub path: std::path::PathBuf,
    /// `installed`, or `skipped` with the reason in `note`.
    pub state: &'static str,
    pub note: Option<String>,
}

/// `skills install --agents`: the shared folder, and the Hermes folder when Hermes is on this machine. It never
/// makes a Hermes home: with none, the row says what to run later. Returns the rows and every file written.
pub fn install_agents() -> Result<(Vec<AgentTarget>, Vec<std::path::PathBuf>), String> {
    install_agents_at(agents_dir(), hermes_home())
}

pub fn install_agents_at(
    shared: std::path::PathBuf,
    hermes_home: std::path::PathBuf,
) -> Result<(Vec<AgentTarget>, Vec<std::path::PathBuf>), String> {
    let mut paths = install_at(shared.clone())?;
    let mut rows = vec![AgentTarget {
        agents: "Codex, Cursor, Gemini CLI, opencode, Pi",
        path: shared,
        state: "installed",
        note: None,
    }];
    let hermes = hermes_home.join("skills/scriptvox");
    if hermes_home.is_dir() {
        paths.extend(install_at(hermes.clone())?);
        rows.push(AgentTarget {
            agents: "Hermes",
            path: hermes,
            state: "installed",
            note: None,
        });
    } else {
        rows.push(AgentTarget {
            agents: "Hermes",
            path: hermes,
            state: "skipped",
            note: Some(format!(
                "No Hermes home at {}. After you install Hermes, run `scriptvox skills install --agents` again.",
                hermes_home.display()
            )),
        });
    }
    Ok((rows, paths))
}

/// The topic skills, next to the main one.
const TOPICS: [(&str, &str); 13] = [
    (
        "scriptvox-known-errors",
        include_str!("../../../plugin/skills/scriptvox-known-errors/SKILL.md"),
    ),
    (
        "scriptvox-talking-head",
        include_str!("../../../plugin/skills/scriptvox-talking-head/SKILL.md"),
    ),
    (
        "scriptvox-dub",
        include_str!("../../../plugin/skills/scriptvox-dub/SKILL.md"),
    ),
    (
        "scriptvox-arabic",
        include_str!("../../../plugin/skills/scriptvox-arabic/SKILL.md"),
    ),
    (
        "scriptvox-brief",
        include_str!("../../../plugin/skills/scriptvox-brief/SKILL.md"),
    ),
    (
        "scriptvox-genres",
        include_str!("../../../plugin/skills/scriptvox-genres/SKILL.md"),
    ),
    (
        "scriptvox-edit",
        include_str!("../../../plugin/skills/scriptvox-edit/SKILL.md"),
    ),
    (
        "scriptvox-motion",
        include_str!("../../../plugin/skills/scriptvox-motion/SKILL.md"),
    ),
    (
        "scriptvox-prompts",
        include_str!("../../../plugin/skills/scriptvox-prompts/SKILL.md"),
    ),
    (
        "scriptvox-local",
        include_str!("../../../plugin/skills/scriptvox-local/SKILL.md"),
    ),
    (
        "scriptvox-providers",
        include_str!("../../../plugin/skills/scriptvox-providers/SKILL.md"),
    ),
    (
        "scriptvox-review",
        include_str!("../../../plugin/skills/scriptvox-review/SKILL.md"),
    ),
    (
        "scriptvox-sound",
        include_str!("../../../plugin/skills/scriptvox-sound/SKILL.md"),
    ),
];

/// All the skills as (name, text), the main one first, then the generated reference: what `install` writes and
/// what an MCP client reads.
pub fn all() -> Vec<(String, String)> {
    let mut v = vec![
        ("scriptvox".to_string(), render()),
        (REFERENCE.to_string(), render_reference()),
    ];
    v.extend(TOPICS.iter().map(|(n, t)| (n.to_string(), t.to_string())));
    v
}

/// Writes all the skills; the main one first. Returns their paths.
pub fn install() -> Result<Vec<std::path::PathBuf>, String> {
    install_at(install_dir())
}

/// Writes all the skills, with the main one in `main` and each other one in its own folder beside it.
pub fn install_at(main: std::path::PathBuf) -> Result<Vec<std::path::PathBuf>, String> {
    let root = main.parent().unwrap_or(&main).to_path_buf();
    let mut paths = Vec::new();
    for (i, (name, text)) in all().into_iter().enumerate() {
        let dir = if i == 0 {
            main.clone()
        } else {
            root.join(name)
        };
        let file = dir.join("SKILL.md");
        std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(&file, text))
            .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
        paths.push(file);
    }
    Ok(paths)
}

/// The version stamp of an installed skill, if any.
pub fn installed_version() -> Option<String> {
    let text = std::fs::read_to_string(install_dir().join("SKILL.md")).ok()?;
    text.lines().find_map(|l| {
        l.strip_prefix("<!-- scriptvox-skill-version: ")?
            .strip_suffix(" -->")
            .map(String::from)
    })
}

/// The block `init --agents` adds to a project's AGENTS.md and CLAUDE.md.
pub const AGENTS_BLOCK: &str = "<!-- scriptvox:begin -->
## Video (scriptvox)
- Make videos with `scriptvox` (see `scriptvox --help`). Every command takes `--json`.
- Loop: write brief.json, `scriptvox plan --brief brief.json --json`, `scriptvox make --brief brief.json --detach --json`, `scriptvox wait <id> --json`.
- Never ask for API keys in chat: the user runs `scriptvox keys set <provider>`.
- Paid stages need `--approve <plan_id>`. Never poll with sleep: use `scriptvox wait`.
- Check the result with `scriptvox snapshot <id> --sheet 9 --json` and read the PNG.
<!-- scriptvox:end -->
";

/// `init --agents` for one file in `dir`: adds the block one time. A CLAUDE.md that imports AGENTS.md (`@AGENTS.md`)
/// already gets the block through it, so it is not changed. Returns what was done, in words.
pub fn init_file(dir: &std::path::Path, name: &str) -> Result<&'static str, String> {
    let path = dir.join(name);
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    if old.contains("<!-- scriptvox:begin -->") {
        return Ok("already has the scriptvox block");
    }
    if name == "CLAUDE.md" && old.lines().any(|l| l.trim() == "@AGENTS.md") {
        return Ok("imports AGENTS.md, not changed");
    }
    let sep = if old.is_empty() || old.ends_with("\n\n") {
        ""
    } else if old.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    std::fs::write(&path, format!("{old}{sep}{AGENTS_BLOCK}"))
        .map(|()| "block added")
        .map_err(|e| format!("cannot write {name}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_every_skill_beside_the_main_one() {
        let d = tempfile::tempdir().unwrap();
        let paths = install_at(d.path().join("skills/scriptvox")).unwrap();
        assert_eq!(paths.len(), all().len());
        assert!(d.path().join("skills/scriptvox-dub/SKILL.md").is_file());
        assert_eq!(paths[0], d.path().join("skills/scriptvox/SKILL.md"));
        let reference =
            std::fs::read_to_string(d.path().join("skills/scriptvox-reference/SKILL.md")).unwrap();
        assert_eq!(reference, render_reference());
    }

    #[test]
    fn every_field_and_command_is_in_one_of_the_two_generated_skills() {
        let (main, reference) = (render(), render_reference());
        let props = crate::brief::schema("brief").unwrap()["properties"]
            .as_object()
            .unwrap()
            .clone();
        // A name in a common list that the tool does not have would hide nothing and help nobody.
        for k in crate::brief::COMMON_FIELDS {
            assert!(
                props.contains_key(k),
                "COMMON_FIELDS: `{k}` is not a brief field"
            );
        }
        let names: Vec<String> = crate::cli::tree()
            .get_subcommands()
            .filter(|c| !c.is_hide_set())
            .map(|c| c.get_name().to_string())
            .collect();
        for k in crate::cli::COMMON_COMMANDS {
            assert!(
                names.iter().any(|n| n == k),
                "COMMON_COMMANDS: `{k}` is not a command"
            );
        }
        let has = |text: &str, line: &str| text.lines().any(|l| l.starts_with(line));
        for k in props.keys() {
            let line = format!("- `{k}`: ");
            assert!(has(&main, &line) != has(&reference, &line), "{k}");
            assert_eq!(
                has(&main, &line),
                crate::brief::COMMON_FIELDS.contains(&k.as_str())
            );
            // The main skill names every field it does not describe.
            assert!(has(&main, &line) || main.contains(&format!("`{k}`")), "{k}");
        }
        for n in &names {
            let line = format!("- `scriptvox {n}`: ");
            assert!(has(&main, &line) != has(&reference, &line), "{n}");
            assert!(has(&main, &line) || main.contains(&format!("`{n}`")), "{n}");
        }
        // Every command the steps of the main skill use is described in the main skill.
        for n in [
            "doctor", "research", "write", "make", "wait", "status", "resume", "snapshot",
            "explain", "keys",
        ] {
            assert!(has(&main, &format!("- `scriptvox {n}`: ")), "{n}");
        }
        assert!(main.contains(REFERENCE));
    }

    #[test]
    fn hermes_home_is_the_variable_then_the_default() {
        let home = std::path::Path::new("/h");
        assert_eq!(
            hermes_home_from(home, Some("/x/hermes".into()), None),
            std::path::PathBuf::from("/x/hermes")
        );
        assert_eq!(
            hermes_home_from(home, Some("".into()), None),
            home.join(".hermes")
        );
        if !cfg!(windows) {
            assert_eq!(
                hermes_home_from(home, None, Some("/appdata".into())),
                home.join(".hermes")
            );
        }
    }
}
