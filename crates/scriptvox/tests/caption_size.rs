//! Brief `caption_size` (docs/specs/caption-size.md), end to end with no voice: the size in `captions.ass`, the
//! wrapping measured again with the shaper, the plan id, the brand file and the refusal of an unknown value.
mod common;
use common::*;
use scriptvox::captions::Measurer;
use serde_json::Value;
use std::path::Path;

const EN: &str =
    "Most people think a good video needs a studio and a big team. It does not need them. \
You need a clear idea and captions that people can read.";
const AR: &str =
    "يظن معظم الناس أن الفيديو الجيد يحتاج إلى استوديو كبير وفريق عمل. هذا غير صحيح أبدا. \
تحتاج إلى فكرة واضحة ونص قصير يقرؤه الناس.";

fn run(p: &Path, ff: &str, args: &[&str]) -> (Option<i32>, Value) {
    let o = common::bin()
        .current_dir(p)
        .env("SCRIPTVOX_DATA", p.join("data"))
        .env("SCRIPTVOX_CONFIG", p.join("cfg"))
        .env("SCRIPTVOX_FFMPEG", ff)
        .args(args)
        .args(["--json"])
        .output()
        .unwrap();
    (
        o.status.code(),
        serde_json::from_slice::<Value>(&o.stdout).unwrap(),
    )
}

fn brief(p: &Path, body: &str) {
    std::fs::write(p.join("b.json"), body).unwrap();
}

/// The `Cap` style's size and the widest caption line of the file, measured at that size with `font`.
fn size_and_widest(ass: &str, font: &str) -> (u32, f32) {
    let style = ass.lines().find(|l| l.starts_with("Style: Cap,")).unwrap();
    let size: u32 = style.split(',').nth(2).unwrap().parse().unwrap();
    let m = Measurer::new(
        std::fs::read(root().join("assets/fonts").join(font)).unwrap(),
        size as f32,
    )
    .unwrap();
    let mut widest = 0f32;
    let mut lines = 0;
    for d in ass.lines().filter(|l| l.contains(",Cap,,")) {
        let text = d.splitn(10, ',').nth(9).unwrap();
        for line in text.split("\\N") {
            // Drop the override tags: what is left is the text the viewer reads.
            let mut plain = String::new();
            let mut tag = false;
            for c in line.chars() {
                match c {
                    '{' => tag = true,
                    '}' => tag = false,
                    c if !tag => plain.push(c),
                    _ => {}
                }
            }
            widest = widest.max(m.width(plain.trim()));
            lines += 1;
        }
    }
    assert!(lines > 2, "the script must wrap: {ass}");
    (size, widest)
}

fn make_ass(p: &Path, ff: &str, body: &str) -> (String, String) {
    brief(p, body);
    let (code, v) = run(p, ff, &["make", "--brief", "b.json"]);
    assert_eq!(code, Some(0), "{v}");
    let bounds = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "text_bounds")
        .map(|f| f["status"].as_str().unwrap().to_string())
        .unwrap_or_default();
    let ass = std::fs::read_to_string(v["artifacts"]["captions_ass"].as_str().unwrap()).unwrap();
    (ass, bounds)
}

#[test]
fn caption_size_changes_the_size_and_the_lines_still_fit() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let en = |size: &str| format!(r#"{{"script": "{EN}", "aspect": "9:16", "tts": "none"{size}}}"#);
    let (none, _) = make_ass(p, &ff, &en(""));
    let (normal, _) = make_ass(p, &ff, &en(r#", "caption_size": "normal""#));
    let (large, large_bounds) = make_ass(p, &ff, &en(r#", "caption_size": "large""#));
    let (small, _) = make_ass(p, &ff, &en(r#", "caption_size": "small""#));
    // Normal is what a brief with no field gets, byte for byte.
    assert_eq!(none, normal);
    // The caption width of a 9:16 frame: 83 percent of 1080.
    let max = 1080.0 * 0.83;
    let (n, nw) = size_and_widest(&none, "NotoSans-Bold.ttf");
    let (l, lw) = size_and_widest(&large, "NotoSans-Bold.ttf");
    let (s, sw) = size_and_widest(&small, "NotoSans-Bold.ttf");
    assert_eq!((s, n, l), (67, 84, 105));
    for w in [nw, lw, sw] {
        assert!(w <= max, "{w} over {max}");
    }
    assert_eq!(large_bounds, "pass");
    // Larger words, so more pages or lines; the same words.
    assert!(large.lines().count() >= none.lines().count());
}

#[test]
fn arabic_large_captions_wrap_inside_the_width() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let ar = |size: &str| {
        format!(r#"{{"script": "{AR}", "lang": "ar", "aspect": "16:9", "tts": "none"{size}}}"#)
    };
    let (normal, _) = make_ass(p, &ff, &ar(""));
    let (large, bounds) = make_ass(p, &ff, &ar(r#", "caption_size": "large""#));
    let (n, _) = size_and_widest(&normal, "NotoSansArabic-Bold.ttf");
    let (l, lw) = size_and_widest(&large, "NotoSansArabic-Bold.ttf");
    assert_eq!((n, l), (120, 150));
    // 16:9: the caption width is 104 percent of the short side.
    assert!(lw <= 1080.0 * 1.04, "{lw}");
    assert_eq!(bounds, "pass");
    // Right to left as before: every caption line starts with the right-to-left mark.
    assert!(large.lines().filter(|l| l.contains(",Cap,,")).all(|l| l
        .splitn(10, ',')
        .nth(9)
        .unwrap()
        .starts_with('\u{200F}')));
}

#[test]
fn an_unknown_caption_size_is_a_usage_error_and_the_plan_id_follows_the_size() {
    let Some(ff) = ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    brief(
        p,
        r#"{"script": "Hi there.", "tts": "none", "caption_size": "huge"}"#,
    );
    let (code, v) = run(p, &ff, &["plan", "--brief", "b.json"]);
    assert_eq!(code, Some(2), "{v}");
    let said = v["error"].to_string();
    for word in ["small", "normal", "large"] {
        assert!(said.contains(word), "{said}");
    }
    let plan = |body: &str| {
        brief(p, body);
        let (code, v) = run(p, &ff, &["plan", "--brief", "b.json"]);
        assert_eq!(code, Some(0), "{v}");
        v["plan_id"].as_str().unwrap().to_string()
    };
    let none = plan(r#"{"script": "Hi there.", "tts": "none"}"#);
    let normal = plan(r#"{"script": "Hi there.", "tts": "none", "caption_size": "normal"}"#);
    let large = plan(r#"{"script": "Hi there.", "tts": "none", "caption_size": "large"}"#);
    assert_eq!(none, normal);
    assert_ne!(none, large);
    // A brand file may carry it; the brief wins.
    std::fs::write(p.join("brand.json"), r#"{"caption_size": "large"}"#).unwrap();
    let brand = plan(r#"{"script": "Hi there.", "tts": "none", "brand": "brand.json"}"#);
    assert_eq!(brand, large);
    let won = plan(
        r#"{"script": "Hi there.", "tts": "none", "brand": "brand.json", "caption_size": "normal"}"#,
    );
    assert_eq!(won, none);
}
