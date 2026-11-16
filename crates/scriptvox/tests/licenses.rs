//! The embedded notices cover every shipped crate (RVX-031).
use std::collections::BTreeSet;

#[test]
fn notices_list_every_normal_dependency() {
    let o = std::process::Command::new(env!("CARGO"))
        .args([
            "tree",
            "-e",
            "normal,build",
            "--prefix",
            "none",
            "--format",
            "{p}",
            "-p",
            "scriptvox",
            "--offline",
        ])
        .output()
        .unwrap();
    if !o.status.success() {
        return; // no offline index here
    }
    let tree: BTreeSet<String> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let (n, v) = (it.next()?, it.next()?);
            (n != "scriptvox").then(|| format!("{n} {}", v.trim_start_matches('v')))
        })
        .collect();
    let embedded: BTreeSet<String> = scriptvox::licenses::crates().into_iter().collect();
    let missing: Vec<&String> = tree.difference(&embedded).collect();
    assert!(
        missing.is_empty(),
        "notices are stale (run `just notices`): missing {missing:?}"
    );
}

#[test]
fn ofl_and_ffmpeg_are_listed() {
    assert!(scriptvox::licenses::OFL.contains("SIL OPEN FONT LICENSE"));
    assert!(scriptvox::licenses::COMPONENTS
        .iter()
        .any(|(n, l, _)| n.contains("FFmpeg") && l.starts_with("GPL")));
}
