//! harfrust widths must match what libass draws, or line breaks drift.
//! Skipped when SCRIPTVOX_FFMPEG is unset.
use scriptvox::captions::Measurer;
use scriptvox::filterpath::{ass_filter, fontconfig};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Inked width in pixels of one line drawn by libass (white on black, no outline).
fn rendered_width(ff: &str, family: &str, text: &str) -> usize {
    let (w, h) = (1920usize, 200usize);
    let dir = tempfile::tempdir().unwrap();
    let ass = format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {w}\nPlayResY: {h}\nWrapStyle: 2\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: S,{family},72,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,1,0,0,0,100,100,0,0,1,0,0,5,0,0,0,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:00.00,0:00:01.00,S,,0,0,0,,{text}\n"
    );
    std::fs::write(dir.path().join("t.ass"), ass).unwrap();
    std::fs::write(
        dir.path().join("fonts.conf"),
        fontconfig(&root().join("assets/fonts"), dir.path()),
    )
    .unwrap();
    let fonts = root().join("assets/fonts");
    let vf = format!(
        "color=c=black:s={w}x{h}:d=1,{}",
        ass_filter("t.ass", &scriptvox::filterpath::filter_path(&fonts))
    );
    let out = Command::new(ff)
        .current_dir(dir.path())
        .env("FONTCONFIG_FILE", dir.path().join("fonts.conf"))
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-filter_complex",
            &vf,
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "gray",
            "-",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cols: Vec<usize> = (0..w)
        .filter(|&x| (0..h).any(|y| out.stdout[y * w + x] > 40))
        .collect();
    cols.last().unwrap() - cols.first().unwrap() + 1
}

#[test]
fn shaped_width_matches_libass() {
    let Some(ff) = std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| std::path::Path::new(p).exists())
    else {
        return;
    };
    for (file, family, text) in [
        (
            "NotoSans-Bold.ttf",
            "Noto Sans",
            "ScriptVox turns your own clips into video",
        ),
        (
            "NotoSansArabic-Bold.ttf",
            "Noto Sans Arabic",
            "هذه الأداة تحول مقاطعك الخاصة إلى فيديو",
        ),
    ] {
        let m = Measurer::new(
            std::fs::read(root().join("assets/fonts").join(file)).unwrap(),
            72.0,
        )
        .unwrap();
        let adv = m.width(text);
        let ink = rendered_width(&ff, family, text) as f32;
        eprintln!("{family}: shaped {adv:.0} px, drawn {ink:.0} px");
        // The ink box is a little narrower than the advance width (side bearings).
        assert!(
            (adv - ink).abs() <= adv * 0.04 + 8.0,
            "{family}: shaped {adv:.0} px, drawn {ink:.0} px"
        );
    }
}
