//! Generated docs stay true to the binary (RVX-032).
mod common;
use serde_json::Value;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `docs/command-status.md` has one row for each command, no row for a command that is gone, and each state from the
/// legend with a date and how it was checked.
#[test]
fn the_command_status_table_matches_the_commands() {
    // `tree`: the derived builder needs more stack than a test thread has.
    let mut cli = scriptvox::cli::tree();
    cli.build();
    let mut want: Vec<String> = cli
        .get_subcommands()
        .filter(|c| !c.is_hide_set() && c.get_name() != "help")
        .map(|c| c.get_name().to_string())
        .collect();
    want.sort();
    let text = std::fs::read_to_string(root().join("docs/command-status.md")).unwrap();
    let rows: Vec<Vec<&str>> = text
        .lines()
        .filter(|l| l.starts_with("| `"))
        .map(|l| l.trim_matches('|').split(" | ").map(str::trim).collect())
        .collect();
    let mut got: Vec<String> = rows
        .iter()
        .map(|r| r[0].trim_matches('`').to_string())
        .collect();
    got.sort();
    assert_eq!(got, want, "docs/command-status.md: one row per command");
    for r in &rows {
        assert_eq!(r.len(), 4, "{r:?}");
        assert!(
            ["Verified", "Partial", "Unverified"].contains(&r[1]),
            "{r:?}"
        );
        assert!(r[2].len() == 10 && r[2].starts_with("20"), "date in {r:?}");
        assert!(r[3].len() > 10, "how it was checked: {r:?}");
    }
}

#[test]
fn committed_skill_matches_the_binary_and_is_short() {
    let committed =
        std::fs::read_to_string(root().join("plugin/skills/scriptvox/SKILL.md")).unwrap();
    assert_eq!(
        committed,
        scriptvox::skill::render(),
        "plugin/skills/scriptvox/SKILL.md is stale: run `just docs`"
    );
    // 150 again since the split of 13 Nov 2026 (docs/specs/skill-split.md): the main skill lists the common brief
    // fields and commands (`brief::COMMON_FIELDS`, `cli::COMMON_COMMANDS`), and the second generated skill the rest.
    assert!(committed.lines().count() < 150);
    let reference =
        std::fs::read_to_string(root().join("plugin/skills/scriptvox-reference/SKILL.md")).unwrap();
    assert_eq!(
        reference,
        scriptvox::skill::render_reference(),
        "plugin/skills/scriptvox-reference/SKILL.md is stale: run `just docs`"
    );
    // scripts/skills_check.py: a skill has at most 300 lines.
    assert!(reference.lines().count() < 300);
}

#[test]
fn readme_exec_blocks_run() {
    let Some(ff) = std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| std::path::Path::new(p).exists())
    else {
        return;
    };
    let readme = std::fs::read_to_string(root().join("README.md")).unwrap();
    let mut ran = 0;
    for block in readme.split("```sh\n").skip(1) {
        let body = block.split("```").next().unwrap();
        if !body.starts_with("# exec\n") {
            continue;
        }
        let d = tempfile::tempdir().unwrap();
        for line in body.lines().filter(|l| l.starts_with("scriptvox ")) {
            let args: Vec<&str> = line.split_whitespace().skip(1).collect();
            let o = common::bin()
                .current_dir(d.path())
                .env("SCRIPTVOX_FFMPEG", &ff)
                .env("SCRIPTVOX_DATA", d.path().join("data"))
                .args(&args)
                .arg("--json")
                .output()
                .unwrap();
            let v: Value = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
            assert_eq!(o.status.code(), Some(0), "README: `{line}` → {v}");
            ran += 1;
        }
    }
    assert!(ran >= 3, "the README has an exec block");
}

#[test]
fn init_agents_adds_the_block_once() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("AGENTS.md"), "# My project\n").unwrap();
    for _ in 0..2 {
        assert!(common::bin()
            .current_dir(d.path())
            .args(["init", "--agents", "--json"])
            .output()
            .unwrap()
            .status
            .success());
    }
    let t = std::fs::read_to_string(d.path().join("AGENTS.md")).unwrap();
    assert!(t.starts_with("# My project\n"));
    assert_eq!(t.matches("<!-- scriptvox:begin -->").count(), 1);
}

#[test]
fn doctor_warns_about_a_stale_skill() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".claude/skills/scriptvox");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "<!-- scriptvox-skill-version: 0.0.0-old -->\n",
    )
    .unwrap();
    let o = common::bin()
        .env("HOME", home.path())
        .args(["doctor", "--offline", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let skill = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "skill")
        .unwrap()
        .clone();
    assert_eq!(skill["status"], "warn", "{skill}");
    assert_eq!(skill["fix"], "scriptvox skills install");
}

#[test]
fn bench_records_hardware_and_timings() {
    let Some(ff) = std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| std::path::Path::new(p).exists())
    else {
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", ff)
        .args(["bench", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    let b = &v["bench"];
    assert!(b["hardware"]["cores"].as_u64().unwrap() >= 1);
    assert!(b["demo_render"]["realtime_x"].as_f64().unwrap() > 0.0);
    assert!(std::path::Path::new(v["artifacts"]["bench"].as_str().unwrap()).is_file());
}

#[test]
fn other_agent_manifests_match_the_claude_one() {
    let read = |p: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(root().join(p)).unwrap()).unwrap()
    };
    let claude = read("plugin/.claude-plugin/plugin.json");
    for p in [
        "plugin/.codex-plugin/plugin.json",
        "plugin/.cursor-plugin/plugin.json",
    ] {
        let m = read(p);
        assert_eq!(m["name"], claude["name"], "{p}");
        assert_eq!(m["version"], claude["version"], "{p}");
        assert_eq!(m["skills"], "./skills/", "{p}");
    }
    let g = read("plugin/gemini-extension.json");
    assert_eq!(g["version"], claude["version"]);
    assert_eq!(g["mcpServers"]["scriptvox"]["args"][0], "mcp");
}

#[test]
fn the_registry_has_no_stale_refusal_words() {
    // Every row is allowed and the note says so (`docs/specs/commercial-note.md`).
    for name in ["generators.toml", "voices.toml", "providers.toml"] {
        let text = std::fs::read_to_string(root().join("registry").join(name)).unwrap();
        for stale in ["--commercial", "Testing only"] {
            assert!(!text.contains(stale), "{name}: remove {stale:?}");
        }
    }
}

#[test]
fn report_writes_a_local_text_without_keys_paths_or_the_script() {
    let d = tempfile::tempdir().unwrap();
    let runs = d.path().join(".scriptvox/runs/r_abc123");
    std::fs::create_dir_all(&runs).unwrap();
    // A run folder as `make` leaves it, with a key value and the home folder in a log line.
    let key = "not-a-real-key-0123456789-for-the-redactor-test";
    let home = d.path().to_string_lossy().into_owned();
    std::fs::write(
        runs.join("request.json"),
        r#"{"script": "The codename is Bluebird.", "aspect": "9:16", "scenes": [{"script": "Bluebird ships.", "generate": {"prompt": "a blue bird"}}]}"#,
    )
    .unwrap();
    std::fs::write(
        runs.join("result.json"),
        r#"{"status": "error", "error": {"code": "SVX_PROVIDER_REFUSED", "message": "the service said no"}, "findings": [], "metrics": {}}"#,
    )
    .unwrap();
    std::fs::write(
        runs.join("events.jsonl"),
        format!(
            "{{\"type\":\"progress\",\"explain\":\"header {key} sent, file {home}/clip.mp4\"}}\n"
        ),
    )
    .unwrap();
    let cmd = |args: &[&str]| {
        let o = common::bin()
            .current_dir(d.path())
            .env("HOME", &home)
            .env("USER", "someuser")
            .env("ELEVENLABS_API_KEY", key)
            .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
            .env("SCRIPTVOX_DATA", d.path().join("data"))
            .args(args)
            .output()
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        (o.status.code().unwrap(), v)
    };
    let (code, v) = cmd(&["report", "r_abc123", "--json"]);
    assert_eq!(code, 0, "{v}");
    let file = v["artifacts"]["report"].as_str().unwrap().to_string();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("SVX_PROVIDER_REFUSED") && text.contains("<25 characters>"),
        "{text}"
    );
    for gone in [key, "Bluebird", "blue bird", home.as_str()] {
        assert!(!text.contains(gone), "{gone} is in the report");
    }
    // A second report takes the next free name, and a named file that exists is refused.
    let (_, v2) = cmd(&["report", "r_abc123", "--json"]);
    assert_ne!(v2["artifacts"]["report"], v["artifacts"]["report"], "{v2}");
    let (code, _) = cmd(&["report", "r_abc123", "--out", &file, "--json"]);
    assert_eq!(code, 2);
    // Only a run id reaches a path.
    for bad in ["../../etc", "r_abc123/../x", "nope"] {
        let (code, v) = cmd(&["report", bad, "--json"]);
        assert_eq!(code, 2, "{bad}: {v}");
    }
}

/// An FFmpeg the user brings can lack the caption filter (the Homebrew build on 11 Nov 2026 did). `doctor` must say
/// so, and a render must say what to do, not only FFmpeg's own words.
#[cfg(unix)]
#[test]
fn an_ffmpeg_without_the_caption_filter_is_named_by_doctor_and_by_a_failed_render() {
    use std::os::unix::fs::PermissionsExt;
    // As the other test files: skipped when no FFmpeg is set for the tests.
    let Some(ff) = std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| std::path::Path::new(p).is_file())
    else {
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let bin = d.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let real = std::path::Path::new(&ff);
    std::os::unix::fs::symlink(real.with_file_name("ffprobe"), bin.join("ffprobe")).unwrap();
    // The real FFmpeg, but its filter list has no `ass`, and a graph that uses `ass` fails as that build would.
    let script = format!(
        "#!/bin/sh\nfor a in \"$@\"; do case \"$a\" in -filters) '{0}' \"$@\" | grep -v ' ass '; exit 0;; esac; done\nfor a in \"$@\"; do case \"$a\" in *ass=*) echo \"[AVFilterGraph @ 0x1] No such filter: 'ass'\" >&2; echo 'Error : Filter not found' >&2; exit 8;; esac; done\nexec '{0}' \"$@\"\n",
        real.display()
    );
    let fake = bin.join("ffmpeg");
    std::fs::write(&fake, script).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let run = |args: &[&str]| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &fake)
            .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
            .env("SCRIPTVOX_DATA", d.path().join("data"))
            .args(args)
            .output()
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        (o.status.code().unwrap(), v)
    };
    let (_, v) = run(&["doctor", "--offline", "--json"]);
    let f = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "ffmpeg")
        .unwrap()
        .clone();
    assert_eq!(f["status"], "fail", "{v}");
    assert!(
        f["detail"].as_str().unwrap().contains("no filter ass"),
        "{f}"
    );
    let (code, v) = run(&["demo", "--json", "--out", "d.mp4"]);
    assert_ne!(code, 0, "{v}");
    let m = v["error"]["message"].as_str().unwrap();
    assert!(
        m.contains("lacks a filter") && m.contains("scriptvox doctor"),
        "{m}"
    );
}

/// G086: the known-errors skill has a row for every exit code and every gate, so it cannot go stale.
#[test]
fn the_known_errors_skill_covers_every_exit_and_gate() {
    let text =
        std::fs::read_to_string(root().join("plugin/skills/scriptvox-known-errors/SKILL.md"))
            .unwrap();
    for e in scriptvox::exit::Exit::ALL {
        assert!(
            text.contains(&format!("| {} | `SVX_{}`", e.code(), e.name())),
            "{e:?}"
        );
    }
    let gates = std::fs::read_to_string(root().join("crates/scriptvox/src/gates.rs")).unwrap();
    for g in [
        "black",
        "first_frame",
        "flashes",
        "slideshow",
        "silence",
        "loudness",
        "av_drift",
        "words",
        "captions",
        "text_bounds",
        "clip_black",
        "clip_decode",
        "clip_length",
        "clip_motion",
        "clip_size",
    ] {
        assert!(
            gates.contains(&format!("\"{g}\"")),
            "the gate {g} is gone from gates.rs"
        );
        assert!(text.contains(&format!("`{g}`")), "no row for {g}");
    }
    // Warn-only findings made outside gates.rs.
    let make = std::fs::read_to_string(root().join("crates/scriptvox/src/make.rs")).unwrap();
    for g in ["lang", "picture_shape", "emphasis", "behind"] {
        assert!(
            make.contains(&format!("gate: \"{g}\"")),
            "the finding {g} is gone from make.rs"
        );
        assert!(text.contains(&format!("| `{g}` |")), "no row for {g}");
    }
}
