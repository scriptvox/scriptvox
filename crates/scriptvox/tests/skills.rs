//! `skills install --agents` and `init --agents` (milestone Agents, A3). Every test has its own temp HOME.
mod common;
use serde_json::Value;
use std::path::Path;

fn run(home: &Path, cwd: &Path, args: &[&str]) -> Value {
    let o = common::bin()
        .current_dir(cwd)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("HERMES_HOME")
        .env_remove("LOCALAPPDATA")
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    assert!(o.status.success(), "{args:?}");
    serde_json::from_slice(&o.stdout).unwrap()
}

#[test]
fn agents_install_writes_the_shared_folder_and_skips_hermes_when_it_is_absent() {
    let home = tempfile::tempdir().unwrap();
    let v = run(home.path(), home.path(), &["skills", "install", "--agents"]);
    assert!(home
        .path()
        .join(".agents/skills/scriptvox/SKILL.md")
        .is_file());
    assert!(home
        .path()
        .join(".agents/skills/scriptvox-brief/SKILL.md")
        .is_file());
    assert!(
        !home.path().join(".hermes").exists(),
        "no Hermes home is made"
    );
    assert!(
        !home.path().join(".claude").exists(),
        "--agents does not write the Claude folder"
    );
    let rows = v["agents"].as_array().unwrap();
    assert!(rows[0]["agents"].as_str().unwrap().contains("opencode"));
    assert!(rows[0]["agents"].as_str().unwrap().contains("Pi"));
    assert_eq!(rows[1]["agents"], "Hermes");
    assert_eq!(rows[1]["state"], "skipped");
    assert!(rows[1]["note"]
        .as_str()
        .unwrap()
        .contains("skills install --agents"));
}

#[test]
fn agents_install_writes_the_hermes_folder_when_hermes_is_there() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(".hermes")).unwrap();
    let v = run(home.path(), home.path(), &["skills", "install", "--agents"]);
    let main = home.path().join(".hermes/skills/scriptvox/SKILL.md");
    assert!(std::fs::read_to_string(main)
        .unwrap()
        .contains("name: scriptvox"));
    assert!(home
        .path()
        .join(".hermes/skills/scriptvox-arabic/SKILL.md")
        .is_file());
    assert_eq!(v["agents"][1]["state"], "installed");
    // Both folders hold every skill.
    assert_eq!(
        v["skills"].as_array().unwrap().len(),
        2 * scriptvox::skill::all().len()
    );
}

#[test]
fn hermes_home_variable_wins() {
    let home = tempfile::tempdir().unwrap();
    let hh = home.path().join("elsewhere");
    std::fs::create_dir(&hh).unwrap();
    let o = common::bin()
        .env("HOME", home.path())
        .env("HERMES_HOME", &hh)
        .args(["skills", "install", "--agents", "--json"])
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(hh.join("skills/scriptvox/SKILL.md").is_file());
}

#[test]
fn plain_install_writes_only_the_claude_folder() {
    let home = tempfile::tempdir().unwrap();
    let v = run(home.path(), home.path(), &["skills", "install"]);
    assert!(home
        .path()
        .join(".claude/skills/scriptvox/SKILL.md")
        .is_file());
    assert!(!home.path().join(".agents").exists());
    assert!(v.get("agents").is_none());
}

#[test]
fn init_agents_writes_both_files_one_time() {
    let home = tempfile::tempdir().unwrap();
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("CLAUDE.md"), "# Mine\n").unwrap();
    for _ in 0..2 {
        run(home.path(), d.path(), &["init", "--agents"]);
    }
    for f in ["AGENTS.md", "CLAUDE.md"] {
        let t = std::fs::read_to_string(d.path().join(f)).unwrap();
        assert_eq!(t.matches("<!-- scriptvox:begin -->").count(), 1, "{f}");
    }
    assert!(std::fs::read_to_string(d.path().join("CLAUDE.md"))
        .unwrap()
        .starts_with("# Mine\n"));
}

#[test]
fn init_agents_leaves_a_claude_md_that_imports_agents_md() {
    let home = tempfile::tempdir().unwrap();
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("CLAUDE.md"), "@AGENTS.md\n").unwrap();
    let v = run(home.path(), d.path(), &["init", "--agents"]);
    assert_eq!(
        std::fs::read_to_string(d.path().join("CLAUDE.md")).unwrap(),
        "@AGENTS.md\n"
    );
    assert_eq!(v["agents_md"], "block added");
    assert_eq!(v["claude_md"], "imports AGENTS.md, not changed");
}

/// `docs/specs/agent-packages.md` names every agent that a row of the answer names.
#[test]
fn the_spec_names_every_agent_of_the_answer() {
    let home = tempfile::tempdir().unwrap();
    let v = run(home.path(), home.path(), &["skills", "install", "--agents"]);
    let spec = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/specs/agent-packages.md"),
    )
    .unwrap();
    let rows = v["agents"].as_array().unwrap();
    assert!(rows.len() >= 2, "{v}");
    for name in rows
        .iter()
        .flat_map(|r| r["agents"].as_str().unwrap().split(", "))
    {
        assert!(spec.contains(name), "the spec does not name {name}");
    }
}
