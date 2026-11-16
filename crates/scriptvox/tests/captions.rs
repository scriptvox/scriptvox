use proptest::prelude::*;
use scriptvox::captions::*;
use serde_json::Value;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(lang: &str) -> (String, Vec<Word>) {
    let p = root().join(format!("fixtures/golden/captions-{lang}/words.json"));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let words = serde_json::from_value(v["words"].clone()).unwrap();
    (v["text"].as_str().unwrap().to_string(), words)
}

fn measurer(font: &str) -> Measurer {
    Measurer::new(
        std::fs::read(root().join("assets/fonts").join(font)).unwrap(),
        72.0,
    )
    .unwrap()
}

const LAYOUT: Layout = Layout {
    max_width: 900.0,
    max_lines: 2,
    max_gap: 0.6,
    max_page: 4.0,
};

fn line_text(words: &[Word], l: &[usize]) -> String {
    l.iter()
        .map(|&i| words[i].text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn build(
    lang: &str,
    font: &str,
    family: &str,
    layout: &Layout,
) -> (Vec<Word>, Vec<Page>, Measurer, String) {
    let (script, events) = fixture(lang);
    let words = align(&script, &events);
    let m = measurer(font);
    let pages = paginate(&words, layout, |s| m.width(s));
    let st = AssStyle {
        font: family,
        size: 72,
        play_res: (1080, 1920),
        alignment: 2,
        margin_v: 260,
        pop: false,
        reveal: false,
        emphasis: vec![],
        second_speaker: vec![],
    };
    let ass = to_ass(&pages, &words, &st);
    (words, pages, m, ass)
}

/// Every line fits, and is as full as it can be: the next word would not fit
/// unless a sentence end or a page end stopped the line. A line may give one
/// word to a short last line (the orphan rule), so that case is allowed.
fn assert_tight(words: &[Word], pages: &[Page], m: &Measurer) {
    for p in pages {
        for (n, l) in p.lines.iter().enumerate() {
            let t = line_text(words, l);
            assert!(
                l.len() == 1 || m.width(&t) <= LAYOUT.max_width,
                "overflow: {t}"
            );
            if let Some(next) = p.lines.get(n + 1).filter(|l| l.len() > 2) {
                let longer = format!("{t} {}", words[next[0]].text);
                assert!(
                    m.width(&longer) > LAYOUT.max_width,
                    "line could hold more: {t}"
                );
            }
        }
    }
}

#[test]
fn english_golden() {
    let (words, pages, m, ass) = build("en", "NotoSans-Bold.ttf", "Noto Sans", &LAYOUT);
    assert_eq!(words.len(), 28);
    assert!(
        words[8].text.ends_with('.'),
        "punctuation restored: {}",
        words[8].text
    );
    assert_tight(&words, &pages, &m);
    assert!(ass.contains("{\\k"));
    insta::assert_snapshot!("en_ass", ass);
    insta::assert_snapshot!("en_srt", to_srt(&pages, &words));
    insta::assert_snapshot!("en_vtt", to_vtt(&pages, &words));
}

#[test]
fn arabic_golden_wraps_at_measured_break() {
    let (words, pages, m, ass) = build(
        "ar",
        "NotoSansArabic-Bold.ttf",
        "Noto Sans Arabic",
        &Layout {
            max_page: 99.0,
            ..LAYOUT
        },
    );
    assert!(
        pages.iter().any(|p| p.lines.len() == 2),
        "the long Arabic sentence must wrap"
    );
    assert_tight(&words, &pages, &m);
    assert!(ass.contains("\\N"));
    insta::assert_snapshot!("ar_ass", ass);
    insta::assert_snapshot!("ar_srt", to_srt(&pages, &words));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn no_word_lost_and_pages_in_order(texts in proptest::collection::vec("[a-z]{1,12}[.]?|[ء-ي]{2,9}", 1..60)) {
        let mut t = 0.0;
        let words: Vec<Word> = texts.iter().map(|s| { let w = Word { text: s.clone(), start: t, end: t + 0.3 }; t += 0.35; w }).collect();
        let m = measurer("NotoSansArabic-Bold.ttf");
        let pages = paginate(&words, &LAYOUT, |s| m.width(s));
        let flat: Vec<usize> = pages.iter().flat_map(|p| p.lines.iter().flatten().copied()).collect();
        prop_assert_eq!(flat, (0..words.len()).collect::<Vec<_>>());
        for p in &pages {
            prop_assert!(p.lines.len() <= LAYOUT.max_lines);
            prop_assert!(p.start <= p.end);
        }
        prop_assert!(pages.windows(2).all(|w| w[0].end <= w[1].start));
    }
}
