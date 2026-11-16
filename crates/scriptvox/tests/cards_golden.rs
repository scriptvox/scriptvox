//! Pixel goldens for the scene cards: each kind, English and Arabic, drawn by libass with our fonts over a flat
//! frame and compared with a blessed PNG (PSNR). A font fallback or layout change fails here before a real run sees
//! it (4 Nov 2026: Latin letters in Arabic text fell back to the math font at a third of the size, found by eye).
//! Skipped when SCRIPTVOX_FFMPEG is unset. After a deliberate visual change: `SCRIPTVOX_BLESS=1 cargo test --test
//! cards_golden`, look at the new PNGs, commit them. The same cases are drawn a second time as brief `card_style: paper`
//! draws them, on a light page (`fixtures/golden/cards-paper/`), and go through the gates `contrast` and `text_bounds`.
use scriptvox::captions::{em_factor, to_ass, AssStyle, Measurer};
use scriptvox::cards::{events, insert, Card, Ease, Frame, Kind};
use std::path::{Path, PathBuf};
use std::process::Command;

const W: u32 = 540;
const H: u32 = 960;
/// Identical renders give inf; a changed glyph or position drops far below this.
const PSNR_MIN: f64 = 45.0;

fn dir(paper: bool) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(if paper {
        "../../fixtures/golden/cards-paper"
    } else {
        "../../fixtures/golden/cards"
    })
}

/// The page of the paper goldens, and the accent of their brief (`docs/specs/card-style.md`).
const PAGE: &str = "0xf6f4ee";
const ACCENT: &str = "#b3421f";

fn card(kind: Kind, text: Option<&str>, label: Option<&str>) -> Card {
    serde_json::from_value(serde_json::json!({"kind": kind, "text": text, "label": label})).unwrap()
}

fn cases() -> Vec<(&'static str, &'static str, Card)> {
    let mut list = card(Kind::List, None, None);
    list.items = Some(vec!["Fast".into(), "Free".into(), "Open".into()]);
    let mut list_ar = card(Kind::List, None, None);
    list_ar.items = Some(vec!["سريع".into(), "مجاني وفيه API".into()]);
    let mut line_ar = card(Kind::Line, None, Some("السكان"));
    line_ar.items = Some(vec!["٢٠٠٥".into(), "٢٠١٥".into(), "٢٠٢٥".into()]);
    line_ar.values = Some(vec![1.4, 2.6, 3.8]);
    let mut spring = list.clone();
    spring.ease = Some(Ease::Spring);
    spring.stagger = Some(0.3);
    let mut bars = card(Kind::Bars, None, Some("Trips by mode"));
    bars.items = Some(vec!["Car".into(), "Bus".into(), "Walk".into()]);
    bars.values = Some(vec![62.0, 21.0, 17.0]);
    bars.ease = Some(Ease::Out);
    let mut compare = card(Kind::Compare, None, Some("days"));
    compare.items = Some(vec!["Before".into(), "After".into()]);
    compare.values = Some(vec![12.0, 3.0]);
    let mut donut = card(Kind::Donut, Some("Trips"), None);
    donut.items = Some(vec!["Car".into(), "Bus".into(), "Walk".into()]);
    donut.values = Some(vec![62.0, 26.0, 12.0]);
    let mut donut_ar = card(Kind::Donut, None, None);
    donut_ar.items = Some(vec!["السيارة".into(), "الحافلة".into()]);
    donut_ar.values = Some(vec![70.0, 30.0]);
    let mut timeline = card(Kind::Timeline, None, None);
    timeline.items = Some(vec!["2019: founded".into(), "2024: launched".into()]);
    let mut plot = card(Kind::Plot, Some("x^2 - 4"), Some("A parabola"));
    plot.values = Some(vec![-4.0, 4.0]);
    // The y range from the brief, the grid, and the running dot at rest on the curve's end with its value.
    let mut plot_more = plot.clone();
    plot_more.y = Some(vec![-6.0, 14.0]);
    plot_more.grid = Some(true);
    plot_more.trace = Some(true);
    let data = |v: serde_json::Value| serde_json::from_value::<Card>(v).unwrap();
    let mut more = vec![
        (
            "table-en",
            "en",
            data(
                serde_json::json!({"kind": "table", "label": "Plans", "rows": [
                ["Plan", "Price", "Seats"], ["Free", "$0", "1"], ["Team", "$12", "10"], ["Company", "$40", "100"]],
                "mark": [{"part": "Team", "on": "team"}]}),
            ),
        ),
        (
            "table-ar",
            "ar",
            data(
                serde_json::json!({"kind": "table", "label": "الخطط", "rows": [
                ["الخطة", "السعر", "المقاعد"], ["مجانية", "٠", "١"], ["الفريق", "١٢", "١٠"], ["الشركة", "٤٠", "١٠٠"]],
                "mark": [{"part": "المقاعد", "on": "المقاعد"}]}),
            ),
        ),
        (
            "number-line-en",
            "en",
            data(
                serde_json::json!({"kind": "number_line", "label": "Customer score", "values": [0, 100],
                "points": [80, 35], "items": ["Now", "Before"], "from": 20}),
            ),
        ),
        (
            "number-line-ar",
            "ar",
            data(
                serde_json::json!({"kind": "number_line", "label": "درجة الحرارة", "values": [-50, 150],
                "points": [-25, 0, 100], "items": ["ليلة باردة", "جليد", "بخار"]}),
            ),
        ),
        (
            "diagram-en",
            "en",
            data(
                serde_json::json!({"kind": "diagram", "nodes": ["Brief", "Media", "Render", "Video", "Report"],
                "edges": [["Brief", "Render"], ["Media", "Render"], ["Render", "Video"], ["Render", "Report"]]}),
            ),
        ),
        (
            "diagram-ar",
            "ar",
            data(
                serde_json::json!({"kind": "diagram", "label": "من الملخص إلى الفيديو",
                "nodes": ["الملخص المكتوب", "الخطة", "الفيديو", "التقرير"],
                "edges": [["الملخص المكتوب", "الخطة"], ["الخطة", "الفيديو"], ["الخطة", "التقرير"]]}),
            ),
        ),
        (
            "steps-en",
            "en",
            data(
                serde_json::json!({"kind": "steps", "items": ["Write the brief", "Check the plan and what it will cost", "Make the video"]}),
            ),
        ),
        (
            "steps-ar",
            "ar",
            data(
                serde_json::json!({"kind": "steps", "items": ["اكتب الملخص", "راجع الخطة وما سوف تكلفه", "اصنع الفيديو"]}),
            ),
        ),
        (
            "gauge-en",
            "en",
            data(serde_json::json!({"kind": "gauge", "values": [72], "label": "Battery"})),
        ),
        (
            "gauge-ar",
            "ar",
            data(
                serde_json::json!({"kind": "gauge", "values": [12, 16], "text": "١٢ من ١٦", "label": "مساحة القرص المستخدمة"}),
            ),
        ),
        (
            "progress-en",
            "en",
            data(serde_json::json!({"kind": "progress", "values": [3, 5], "label": "Steps done"})),
        ),
        (
            "progress-ar",
            "ar",
            data(
                serde_json::json!({"kind": "progress", "values": [3, 5], "label": "الخطوات المنجزة"}),
            ),
        ),
    ];
    let mut all = vec![
        (
            "title-en",
            "en",
            card(Kind::Title, Some("Three ideas"), None),
        ),
        (
            "stat-en",
            "en",
            card(Kind::Stat, Some("87%"), Some("of trips by car")),
        ),
        ("list-en", "en", list),
        ("list-ar", "ar", list_ar),
        ("list-en-spring", "en", spring.clone()),
        // During the motion: item 1 past its size (spring), item 2 just in, item 3 not yet.
        ("list-en-spring-mid", "en", spring),
        ("bars-en", "en", bars),
        ("line-ar", "ar", line_ar),
        (
            "formula-en",
            "en",
            card(Kind::Formula, Some(r"A = \pi r^{2}"), Some("area")),
        ),
        (
            "formula-ar",
            "ar",
            card(Kind::Formula, Some(r"x \le 2"), Some("المساحة r")),
        ),
        (
            "code-en",
            "en",
            card(
                Kind::Code,
                Some("def area(r):\n    return 3.14 * r ** 2"),
                Some("area.py"),
            ),
        ),
        (
            "quote-en",
            "en",
            card(
                Kind::Quote,
                Some("Plans are nothing, planning is everything."),
                Some("A saying"),
            ),
        ),
        (
            "quote-ar",
            "ar",
            card(Kind::Quote, Some("التخطيط هو كل شيء"), Some("قول قديم")),
        ),
        ("compare-en", "en", compare),
        ("donut-en", "en", donut),
        ("donut-ar", "ar", donut_ar),
        ("timeline-en", "en", timeline),
        ("plot-en", "en", plot),
        ("plot-en-grid-trace", "en", plot_more),
        (
            "plot-ar",
            "ar",
            card(Kind::Plot, Some("e^x"), Some("دالة أسية")),
        ),
    ];
    all.append(&mut more);
    // Kinds and parts with no picture before 14 Nov 2026: the lower third, the hook (alone: a card waits
    // for it), the diff, the terminal, and the three marks.
    let mut late = vec![
        (
            "lower-third-en",
            "en",
            card(Kind::LowerThird, Some("Sara Ali"), Some("Engineer")),
        ),
        (
            "lower-third-ar",
            "ar",
            card(Kind::LowerThird, Some("سارة علي"), Some("مهندسة")),
        ),
        (
            "hook-en",
            "en",
            card(Kind::Title, Some("Three ideas"), None),
        ),
        ("hook-ar", "ar", card(Kind::Title, Some("ثلاث أفكار"), None)),
        (
            "diff-en",
            "en",
            card(
                Kind::Diff,
                Some(" fn area(r) {\n-    3.0 * r * r\n+    3.14 * r * r\n }"),
                Some("area.rs"),
            ),
        ),
        (
            "terminal-en",
            "en",
            card(
                Kind::Terminal,
                Some("$ scriptvox plan --brief b.json\nplan p_41c2: free\n$ scriptvox make"),
                Some("Commands"),
            ),
        ),
        (
            "mark-title-en",
            "en",
            data(
                serde_json::json!({"kind": "title", "text": "Operation Manual", "mark": [{"part": "Manual", "on": "manual"}]}),
            ),
        ),
        (
            "mark-quote-en",
            "en",
            data(
                serde_json::json!({"kind": "quote", "text": "A plan that costs money stops.", "label": "Cost control",
                "mark": [{"part": "stops", "on": "stops", "style": "highlight"}, {"part": "money", "on": "money", "style": "ring"}]}),
            ),
        ),
        (
            "mark-list-ar",
            "ar",
            data(
                serde_json::json!({"kind": "list", "items": ["سريع جدا", "مجاني"],
                "mark": [{"part": "مجاني", "on": "مجاني", "style": "highlight"}]}),
            ),
        ),
        (
            "mark-table-en",
            "en",
            data(
                serde_json::json!({"kind": "table", "label": "Data", "rows": [
                ["Item", "Data"], ["Form", "One binary"], ["Licence", "Apache-2.0"]],
                "mark": [{"part": "Form", "on": "binary"}]}),
            ),
        ),
    ];
    all.append(&mut late);
    all
}

/// The ASS file for one card shown from 0 to 4 s, sized as `make` sizes it. A `mark-` case: the voice says each
/// mark's word at 1 s. A `hook-` case: with the hook of the first 1.6 s.
fn ass_for(name: &str, lang: &str, c: &Card) -> String {
    let (family, font) = scriptvox::template::font_for(lang);
    let latin = em_factor(scriptvox::template::FONTS[0].2).unwrap();
    let size = (42.0 * em_factor(font).unwrap() / latin).round() as u32;
    let st = AssStyle {
        font: family,
        size,
        play_res: (W, H),
        alignment: 2,
        margin_v: 160,
        pop: false,
        reveal: false,
        emphasis: vec![],
        second_speaker: vec![],
    };
    let wrap = |t: &str, s: u32, max: f32| {
        let m = Measurer::new(font.to_vec(), s as f32).unwrap();
        scriptvox::captions::wrap(t, max, |x| m.width(x))
    };
    let said: Vec<scriptvox::captions::Word> = c
        .mark
        .iter()
        .flatten()
        .filter(|_| name.starts_with("mark-"))
        .map(|m| scriptvox::captions::Word {
            text: m.on.clone(),
            start: 1.0,
            end: 1.4,
        })
        .collect();
    let mut ev = events(c, 0.0, 4.0, &Frame { w: W, h: H, size }, &wrap, &said);
    if !said.is_empty() {
        ev.extend(scriptvox::card_mark::events(c, &ev, font, size, &said).unwrap());
    }
    if name.starts_with("hook-") {
        // The card's text as the hook, as `make` adds it.
        let hook = c.text.as_deref().unwrap();
        return scriptvox::make::hook_ass(&to_ass(&[], &[], &st), hook, family, font, size, W);
    }
    insert(&to_ass(&[], &[], &st), family, size, &ev)
}

/// The same document as `make` writes it for brief `card_style: paper` with an accent.
fn on_paper(ass: &str) -> String {
    let c = scriptvox::captions::ass_colour(ACCENT).unwrap();
    scriptvox::card_style::paper(ass, Some(ACCENT)).replace(scriptvox::captions::ACCENT, &c)
}

fn psnr(ff: &str, a: &Path, b: &Path) -> f64 {
    let o = Command::new(ff)
        .args(["-hide_banner", "-i"])
        .arg(a)
        .arg("-i")
        .arg(b)
        .args(["-lavfi", "[0:v][1:v]psnr", "-f", "null", "-"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    let avg = err
        .split("average:")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .unwrap_or("0");
    if avg == "inf" {
        f64::INFINITY
    } else {
        avg.parse().unwrap_or(0.0)
    }
}

/// Draws every case on a flat page and compares each with its blessed PNG. `paper`: as brief `card_style: paper`
/// draws it, on a light page.
fn compare(paper: bool) {
    let Some(ff) = std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| Path::new(p).exists())
    else {
        return;
    };
    let bless = std::env::var("SCRIPTVOX_BLESS").is_ok();
    let work = tempfile::tempdir().unwrap();
    let fc = scriptvox::template::write_fonts(work.path()).unwrap();
    let mut bad = Vec::new();
    for (name, lang, c) in cases() {
        assert!(c.problem().is_none(), "{name}: {:?}", c.problem());
        let ass = ass_for(name, lang, &c);
        let ass = if paper { on_paper(&ass) } else { ass };
        std::fs::write(work.path().join(format!("{name}.ass")), ass).unwrap();
        let png = work.path().join(format!("{name}.png"));
        // 3.5 s: every card has landed (the plot sweep ends at 2 s, the stat counts in 0.8 s); a title leaves at
        // 2.5 s, so 1.5 s. The hook goes at 1.6 s.
        let at = if name.starts_with("hook-") {
            "1.0"
        } else if c.kind == Kind::Title {
            "1.5"
        } else if name.ends_with("-mid") {
            "0.6"
        } else {
            "3.5"
        };
        let o = Command::new(&ff)
            .current_dir(work.path())
            .env("FONTCONFIG_FILE", &fc)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
            ])
            .arg(format!(
                "color=c={}:s={W}x{H}:r=10:d=4",
                if paper { PAGE } else { "0x303040" }
            ))
            .arg("-vf")
            .arg(format!("ass=filename={name}.ass:fontsdir=fonts"))
            .args(["-ss", at, "-frames:v", "1"])
            .arg(&png)
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let golden = dir(paper).join(format!("{name}.png"));
        if bless {
            std::fs::create_dir_all(dir(paper)).unwrap();
            std::fs::copy(&png, &golden).unwrap();
            continue;
        }
        assert!(
            golden.is_file(),
            "no golden for {name}: SCRIPTVOX_BLESS=1 cargo test --test cards_golden"
        );
        let p = psnr(&ff, &png, &golden);
        if p < PSNR_MIN {
            let keep = std::env::temp_dir().join(format!(
                "scriptvox-card-{}{name}.png",
                if paper { "paper-" } else { "" }
            ));
            std::fs::copy(&png, &keep).ok();
            bad.push(format!(
                "{name}: PSNR {p:.1} (new frame: {})",
                keep.display()
            ));
        }
    }
    assert!(bad.is_empty(), "cards changed: {bad:?}");
}

#[test]
fn every_card_kind_matches_its_golden() {
    compare(false);
}

/// Brief `card_style: paper` (`docs/specs/card-style.md`): the same cases on a light page.
#[test]
fn every_card_kind_matches_its_paper_golden() {
    compare(true);
}

/// The colours of an ASS line of a card: each `c&H...` value, as 6 digits.
fn colours(line: &str) -> Vec<String> {
    line.split("c&H")
        .skip(1)
        .map(|t| {
            let n = t.bytes().take_while(u8::is_ascii_hexdigit).count();
            t[n.saturating_sub(6)..n].to_uppercase()
        })
        .collect()
}

/// A colour that a card file draws and `card_style.rs` does not know stays as it is: light letters on a light
/// page. So every colour of every case must be one that the paper style gives.
#[test]
fn no_dark_page_colour_is_left_on_paper() {
    let accent = &scriptvox::captions::ass_colour(ACCENT).unwrap()[4..];
    // Ink, the light box, the two panels, dim text, the two tracks, the diff lines, the code and series colours.
    let paper = [
        "1A1816", "FAFCFD", "DFE8EC", "CAD6DC", "5B5855", "B4BCC2", "C2CCD2", "A9DDB5", "B4B9F2",
        "A52D8A", "004AA0", "3A6B1E", "AD5C0B", "A67312", "6E30B0", "327D2F", "005CB8", "807D7A",
        accent,
    ];
    for (name, lang, c) in cases() {
        let ass = on_paper(&ass_for(name, lang, &c));
        // The Card style, or the Hook style with its own box alpha.
        assert!(
            ass.contains(",&H001A1816,&H001A1816,&H66FAFCFD,&H66FAFCFD,")
                || ass.contains(",&H001A1816,&H001A1816,&H59FAFCFD,&H59FAFCFD,"),
            "{name}: the style"
        );
        for l in ass.lines().filter(|l| {
            l.starts_with("Dialogue:") && matches!(l.split(',').nth(3), Some("Card" | "Hook"))
        }) {
            for c in colours(l) {
                assert!(paper.contains(&c.as_str()), "{name}: colour {c} in {l}");
            }
        }
    }
}

/// The tool's own gates on a light page: each case as a short video on #f6f4ee, then `contrast` (the letters
/// against the pixels around them) and `text_bounds` (each text box inside the frame).
#[test]
fn every_paper_card_passes_contrast_and_text_bounds_on_a_light_page() {
    let Some(path) = std::env::var("SCRIPTVOX_FFMPEG")
        .ok()
        .filter(|p| Path::new(p).exists())
    else {
        return;
    };
    let ff = scriptvox::engine_ffmpeg::Ffmpeg {
        ffprobe: Path::new(&path).with_file_name("ffprobe"),
        ffmpeg: PathBuf::from(&path),
    };
    // The finding of `contrast` for one document on the light page.
    let contrast = |ass: &str| {
        let work = tempfile::tempdir().unwrap();
        let fc = scriptvox::template::write_fonts(work.path()).unwrap();
        std::fs::write(work.path().join("captions.ass"), ass).unwrap();
        let o = Command::new(&path)
            .current_dir(work.path())
            .env("FONTCONFIG_FILE", &fc)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi"])
            .arg("-i")
            .arg(format!("color=c={PAGE}:s={W}x{H}:r=10:d=4"))
            .args(["-vf", "ass=filename=captions.ass:fontsdir=fonts"])
            .args([
                "-c:v", "libx264", "-qp", "0", "-pix_fmt", "yuv444p", "v.mp4",
            ])
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        scriptvox::contrast::gate(&ff, work.path(), &work.path().join("v.mp4"), &fc)
    };
    let mut bad = Vec::new();
    for (name, lang, c) in cases() {
        let dark = ass_for(name, lang, &c);
        let ass = on_paper(&dark);
        match contrast(&ass) {
            Some(f) if f.status == "pass" => {}
            f => bad.push(format!("{name}: contrast {:?}", f.map(|f| f.detail))),
        }
        if let Some(f) = scriptvox::gates::text_bounds(&ass, (W, H), &[]) {
            if f.status != "pass" {
                bad.push(format!("{name}: text_bounds {}", f.detail));
            }
        }
        // The measure sees the fault that was found on 14 Nov 2026: the dark-page steps with this accent do not
        // pass (the dark digit on the accent disc, 2.4 to 1).
        if name == "steps-en" {
            let accent = scriptvox::captions::ass_colour(ACCENT).unwrap();
            let dark = dark.replace(scriptvox::captions::ACCENT, &accent);
            assert_eq!(contrast(&dark).map(|f| f.status), Some("warn"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
