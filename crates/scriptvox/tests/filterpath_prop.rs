//! Each escaped path must survive a real FFmpeg filtergraph parse: libass opens
//! the file only if the path arrives intact. Skipped when SCRIPTVOX_FFMPEG is unset.
use proptest::prelude::*;
use scriptvox::filterpath::{ass_filter, filter_path};
use std::process::Command;

const ASS: &str =
    "[Script Info]\nScriptType: v4.00+\n\n[Events]\nFormat: Layer, Start, End, Style, Text\n";

fn ffmpeg() -> Option<String> {
    std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| std::path::Path::new(p).exists())
}

fn name() -> impl Strategy<Value = String> {
    // Drive-letter colons, spaces, quotes, brackets and graph separators.
    proptest::collection::vec(
        prop_oneof![
            Just("C:".to_string()),
            Just(" ".to_string()),
            Just("'".to_string()),
            Just("[".to_string()),
            Just("]".to_string()),
            Just(",".to_string()),
            Just(";".to_string()),
            Just("\\".to_string()),
            "[a-z0-9]{1,4}",
            Just("مرحبا".to_string()),
        ],
        1..6,
    )
    .prop_map(|v| v.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]
    #[test]
    fn escaped_path_survives_ffmpeg_parse(dir in name(), file in name()) {
        let Some(ff) = ffmpeg() else { return Ok(()) };
        let root = tempfile::tempdir().unwrap();
        let d = root.path().join(&dir);
        std::fs::create_dir_all(&d).unwrap();
        let ass = d.join(format!("{file}.ass"));
        std::fs::write(&ass, ASS).unwrap();
        let vf = format!("nullsrc=s=64x64:d=0.04,{}", ass_filter(&filter_path(&ass), &filter_path(&d)));
        let out = Command::new(ff)
            .args(["-hide_banner", "-loglevel", "error", "-filter_complex", &vf, "-frames:v", "1", "-f", "null", "-"])
            .output()
            .unwrap();
        prop_assert!(out.status.success(), "path {:?}: {}", ass, String::from_utf8_lossy(&out.stderr));
    }
}
