//! Numbers and dates in a text, and gate `claims` (`docs/specs/research.md`, sections 4 and 8). No model: plain text
//! and number matching. `research` (the number of a fact must be in its sentence), `write` (the self-check of a
//! brief) and the gate use the same reader.

use crate::brief::Source;
use crate::gates::Finding;
use serde_json::Value;

/// A number written in digits, or a month name next to a number.
#[derive(Debug, Clone, PartialEq)]
pub enum Claim {
    Number(f64),
    Month(u8),
}

impl Claim {
    fn same(&self, other: &Claim) -> bool {
        match (self, other) {
            (Claim::Number(a), Claim::Number(b)) => {
                (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
            }
            (Claim::Month(a), Claim::Month(b)) => a == b,
            _ => false,
        }
    }
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];
/// Compared in folded form ([`fold`]), so `ألف` and `الف` are one word.
const SCALES: [(&str, f64); 11] = [
    ("thousand", 1e3),
    ("million", 1e6),
    ("billion", 1e9),
    ("trillion", 1e12),
    ("ألف", 1e3),
    ("آلاف", 1e3),
    ("مليون", 1e6),
    ("ملايين", 1e6),
    ("مليار", 1e9),
    ("مليارات", 1e9),
    ("تريليون", 1e12),
];
/// The Arabic names of the months, in both sets that are in use. Compared in folded form.
// ponytail: a month of two words (كانون الثاني, تشرين الأول) is not read; `LIMITS` says so.
const MONTHS_AR: [(&str, u8); 20] = [
    ("يناير", 1),
    ("فبراير", 2),
    ("مارس", 3),
    ("أبريل", 4),
    ("مايو", 5),
    ("يونيو", 6),
    ("يوليو", 7),
    ("أغسطس", 8),
    ("سبتمبر", 9),
    ("أكتوبر", 10),
    ("نوفمبر", 11),
    ("ديسمبر", 12),
    ("شباط", 2),
    ("آذار", 3),
    ("نيسان", 4),
    ("أيار", 5),
    ("حزيران", 6),
    ("تموز", 7),
    ("آب", 8),
    ("أيلول", 9),
];

/// A text folded for a match, never for display: compatibility forms (NFKD), no combining marks (the Arabic
/// harakat, the hamza on an alef, a Latin accent), no tatweel, no zero-width or direction marks, digits and the
/// percent sign as ASCII, one letter for the Arabic and Persian forms of yeh and kaf, plain quote marks and dashes,
/// and each run of white space as one space. The second value gives, for each char of the folded text, the byte
/// offset of the char of `text` that it comes from: a match can be cut out of the original text.
pub fn fold(text: &str) -> (String, Vec<usize>) {
    use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
    let (mut out, mut map) = (String::with_capacity(text.len()), Vec::new());
    for (at, c) in text.char_indices() {
        if c.is_whitespace() {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
                map.push(at);
            }
            continue;
        }
        for d in std::iter::once(c).nfkd() {
            let d = match d {
                '\u{0640}' | '\u{00AD}' | '\u{FEFF}' => continue,
                '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => {
                    continue
                }
                d if is_combining_mark(d) => continue,
                '\u{0660}'..='\u{0669}' => (b'0' + (d as u32 - 0x0660) as u8) as char,
                '\u{06F0}'..='\u{06F9}' => (b'0' + (d as u32 - 0x06F0) as u8) as char,
                '\u{066A}' => '%',
                '\u{066B}' => '.',
                '\u{066C}' => ',',
                '\u{06CC}' | '\u{0649}' => '\u{064A}',
                '\u{06A9}' => '\u{0643}',
                '‘' | '’' => '\'',
                '“' | '”' | '«' | '»' | '„' => '"',
                '–' | '—' | '−' => '-',
                d => d,
            };
            out.push(d);
            map.push(at);
        }
    }
    if out.ends_with(' ') {
        out.pop();
        map.pop();
    }
    (out, map)
}

/// Where the folded `needle` is in `text`: the byte range of `text` that holds it. So a sentence that a model copied
/// with other digit forms, without the harakat or without a tatweel is still found, and the caller keeps the words of
/// the page.
pub fn find_folded(text: &str, needle: &str) -> Option<std::ops::Range<usize>> {
    let ((hay, map), (needle, _)) = (fold(text), fold(needle));
    if needle.is_empty() {
        return None;
    }
    let at = hay.find(&needle)?;
    let first = hay[..at].chars().count();
    let last = first + needle.chars().count();
    // To the start of the next original char: a mark that was folded away after the last letter stays in the cut.
    let end = map[last..]
        .iter()
        .find(|o| **o > map[last - 1])
        .copied()
        .unwrap_or(text.len());
    Some(map[first]..end)
}

/// `needle` is in `text`, both folded.
pub fn has_folded(text: &str, needle: &str) -> bool {
    find_folded(text, needle).is_some()
}

/// Arabic-Indic and Persian digits as ASCII, with their decimal and thousands marks.
fn ascii_digits(text: &str) -> Vec<char> {
    text.chars()
        .map(|c| match c {
            '\u{0660}'..='\u{0669}' => (b'0' + (c as u32 - 0x0660) as u8) as char,
            '\u{06F0}'..='\u{06F9}' => (b'0' + (c as u32 - 0x06F0) as u8) as char,
            '\u{066B}' => '.',
            '\u{066C}' => ',',
            c => c,
        })
        .collect()
}

pub fn month(word: &str) -> Option<u8> {
    let w = word.to_lowercase();
    if let Some(i) = MONTHS.iter().position(|m| *m == w) {
        return Some(i as u8 + 1);
    }
    // Arabic, also with one joined letter before it ("ومارس", "بمارس").
    let w = fold(word).0;
    let bare = w
        .strip_prefix(['و', 'ب', 'ل', 'ف'])
        .filter(|r| r.chars().count() >= 4);
    MONTHS_AR
        .iter()
        .find(|(m, _)| {
            let m = fold(m).0;
            m == w || bare == Some(m.as_str())
        })
        .map(|m| m.1)
}

/// Every claim of `text` with the text it was read from. `1,234.5`, `42%`, `3 million`, `٤٢`. Digits that follow a
/// letter (`CO2`, `f1`, `COVID-19`) are part of a name, not a number.
// ponytail: number words ("three", "forty") are not read; `limitations` says so. Add a word reader when users ask.
pub fn read(text: &str) -> Vec<(Claim, String)> {
    let c = ascii_digits(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if !c[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        // Latin letters only: an Arabic word joins a number with no space ("و3", "الـ20", "بـ450"), and that is a
        // number, not a name.
        let latin = |ch: char| ch.is_alphabetic() && (ch as u32) < 0x0250;
        let named = start > 0
            && (latin(c[start - 1]) || (c[start - 1] == '-' && start > 1 && latin(c[start - 2])));
        let mut num = String::new();
        while i < c.len() {
            let digits_after =
                |n: usize| c[i + 1..].iter().take_while(|d| d.is_ascii_digit()).count() == n;
            if c[i].is_ascii_digit() {
                num.push(c[i]);
            } else if c[i] == ',' && digits_after(3) {
                // A thousands mark: exactly 3 digits follow.
            } else if c[i] == '.'
                && !num.contains('.')
                && c.get(i + 1).is_some_and(char::is_ascii_digit)
            {
                num.push('.');
            } else {
                break;
            }
            i += 1;
        }
        let Ok(mut n) = num.parse::<f64>() else {
            continue;
        };
        if named {
            continue;
        }
        // A scale word after the number: "3 million".
        let rest: String = c[i..].iter().take(12).collect();
        let next = fold(
            &rest
                .trim_start()
                .split(|ch: char| !ch.is_alphabetic() && !('\u{064B}'..='\u{0655}').contains(&ch))
                .next()
                .unwrap_or("")
                .to_lowercase(),
        )
        .0;
        let mut raw: String = c[start..i].iter().collect();
        if let Some((w, s)) = SCALES.iter().find(|(w, _)| fold(w).0 == next) {
            n *= s;
            raw = format!("{raw} {w}");
        }
        out.push((Claim::Number(n), raw));
    }
    // A month name counts only next to a number ("12 March", "March 2020"): "may" and "march" are also plain words.
    let words: Vec<&str> = text.split_whitespace().collect();
    let has_digit = |w: &&str| ascii_digits(w).iter().any(char::is_ascii_digit);
    for (k, w) in words.iter().enumerate() {
        let bare = w.trim_matches(|ch: char| !ch.is_alphabetic());
        if let Some(m) = month(bare) {
            if (k > 0 && has_digit(&words[k - 1])) || words.get(k + 1).is_some_and(has_digit) {
                out.push((Claim::Month(m), bare.to_string()));
            }
        }
    }
    out
}

pub fn numbers(text: &str) -> Vec<f64> {
    read(text)
        .into_iter()
        .filter_map(|(c, _)| match c {
            Claim::Number(n) => Some(n),
            Claim::Month(_) => None,
        })
        .collect()
}

/// The claims of `text` that are in none of `facts`, each one time, as written.
pub fn unmatched<'a>(text: &str, facts: impl Iterator<Item = &'a str>) -> Vec<String> {
    let known: Vec<Claim> = facts.flat_map(read).map(|(c, _)| c).collect();
    let mut out: Vec<String> = Vec::new();
    for (claim, raw) in read(text) {
        if !known.iter().any(|k| k.same(&claim)) && !out.contains(&raw) {
            out.push(raw);
        }
    }
    out
}

/// The text a viewer hears or sees, from a brief or a run's request as JSON: the script, the hook, the cards.
pub fn text_of(v: &Value, script: &str) -> String {
    let mut parts: Vec<String> = vec![script.to_string()];
    let mut push = |x: &Value| match x {
        Value::String(s) => parts.push(s.clone()),
        Value::Number(n) => parts.push(n.to_string()),
        _ => {}
    };
    push(&v["hook"]);
    for s in v["scenes"].as_array().into_iter().flatten() {
        // The single `card`, and each card of a `cards` list.
        for card in std::iter::once(&s["card"]).chain(s["cards"].as_array().into_iter().flatten()) {
            // A formula's text is math and a code card's is code, as in `Card::texts`.
            let code = ["formula", "code", "diff", "terminal", "plot"]
                .contains(&card["kind"].as_str().unwrap_or(""));
            if !code {
                push(&card["text"]);
            }
            push(&card["label"]);
            // A number line's `values` are the two ends of its scale, not facts: its facts are `points` and `from`.
            let scale = card["kind"] == "number_line";
            for list in ["items", "values", "points"] {
                if !(scale && list == "values") {
                    card[list]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .for_each(&mut push);
                }
            }
            if scale {
                push(&card["from"]);
            }
            // The cells of a table.
            card["rows"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|r| r.as_array().into_iter().flatten())
                .for_each(&mut push);
        }
    }
    parts.join(" \n ")
}

/// The script of a brief or a request as JSON: `script`, else the scenes' or the lines' text.
pub fn script_of(v: &Value) -> String {
    if let Some(s) = v["script"].as_str() {
        return s.to_string();
    }
    let join = |list: &str, field: &str| {
        v[list]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s[field].as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!("{} {}", join("scenes", "script"), join("lines", "text"))
}

const FIX: &str = "Add the fact and its source to `sources` in the brief, or correct the number. A count that is not a fact (\"three steps\") can be written as a word.";

/// Gate `claims`. None when the brief has no sources: such a brief is as before, with no finding.
pub fn gate(text: &str, sources: &[Source], strict: bool) -> Option<Finding> {
    if sources.is_empty() {
        return None;
    }
    // A pause tag (`[pause 1.5]`) is not said and not shown.
    let text = &crate::pauses::strip(text);
    let total = read(text).len();
    // The name of a source is not a claim: "France 24" on a Sources card must not ask for a source of its own. The
    // name is `publisher`, else `title`, as the card takes it. A title next to a publisher is the title of a page
    // ("2024 election"), and its numbers are not facts of the script.
    let missing = unmatched(
        text,
        sources.iter().flat_map(|s| {
            [
                Some(s.text.as_str()),
                s.publisher.as_deref().or(s.title.as_deref()),
            ]
            .into_iter()
            .flatten()
        }),
    );
    Some(if missing.is_empty() {
        Finding {
            gate: "claims",
            status: "pass",
            detail: format!(
                "{total} numbers and dates in the script, each one is in a source ({} sources)",
                sources.len()
            ),
            fix: None,
        }
    } else {
        let more = missing.len().saturating_sub(12);
        let shown = missing.iter().take(12).cloned().collect::<Vec<_>>();
        Finding {
            gate: "claims",
            status: if strict { "fail" } else { "warn" },
            detail: format!(
                "{} of {total} numbers and dates in the script are in no source: {}{}",
                missing.len(),
                shown.join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            ),
            fix: Some(FIX.into()),
        }
    })
}

/// What the gate covers and what it cannot see, for `coverage` and `limitations`.
pub const COVERS: &str =
    "each number written in digits, and each month name next to a number, in the script, the hook and the cards";
pub const LIMITS: &str = "names and facts with no number are not checked; a number written as a word is not seen; a match means the number is in a source text, not that the two sentences say the same thing; an Arabic month name of two words is not seen; a number in the name of a source (`publisher`, else `title`) counts as in a source; no model reads the script";

/// Adds the finding to an envelope, with its limits. With `--strict` a claim with no source fails the command.
pub fn apply(mut env: crate::envelope::Envelope, f: Option<Finding>) -> crate::envelope::Envelope {
    let Some(f) = f else { return env };
    env.findings.retain(|g| g["gate"] != "claims");
    env.findings.push(serde_json::json!(f));
    for (key, line) in [("coverage", COVERS), ("limitations", LIMITS)] {
        let line = Value::String(format!("claims: {line}"));
        match env.extra.get_mut(key).and_then(Value::as_array_mut) {
            Some(a) if !a.contains(&line) => a.push(line),
            Some(_) => {}
            None => {
                env.extra.insert(key.into(), Value::Array(vec![line]));
            }
        }
    }
    if f.status == "fail" && env.ok {
        let failed = crate::envelope::Envelope::fail(
            &env.command,
            crate::exit::Exit::GateFailed,
            format!("gate failed: claims ({})", f.detail),
            f.fix.clone(),
        );
        env.ok = false;
        env.status = failed.status;
        env.exit = failed.exit;
        env.error = failed.error;
    }
    env
}

/// The gate for a brief or a run's request as JSON.
pub fn gate_of(v: &Value, script: &str, strict: bool) -> Option<Finding> {
    let sources: Vec<Source> = serde_json::from_value(v["sources"].clone()).unwrap_or_default();
    gate(&text_of(v, script), &sources, strict)
}

/// The gate for a finished run: its request holds the script, the hook, the cards and the sources.
pub fn gate_of_run(run: &str, strict: bool) -> Option<Finding> {
    let v: Value = std::fs::read_to_string(
        std::path::Path::new(".scriptvox/runs")
            .join(run)
            .join("request.json"),
    )
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())?;
    gate_of(&v, &script_of(&v), strict)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nums(t: &str) -> Vec<f64> {
        numbers(t)
    }

    #[test]
    fn numbers_are_read_in_their_written_forms() {
        assert_eq!(
            nums("It grew 42% to 1,234.5 in 2020."),
            [42.0, 1234.5, 2020.0]
        );
        assert_eq!(nums("about 3 million people and 2.5 billion"), [3e6, 2.5e9]);
        assert_eq!(nums("٤٢ في المئة"), [42.0]);
        assert_eq!(nums("lists 1, 2, 3"), [1.0, 2.0, 3.0]);
        // Part of a name, not a number.
        assert!(nums("CO2 and COVID-19 and H2O").is_empty());
        assert_eq!(nums("the 1990s, 450 nm"), [1990.0, 450.0]);
    }

    #[test]
    fn arabic_numbers_months_and_forms_are_read() {
        // A number joined to an Arabic word, with and without a tatweel, in three digit forms.
        assert_eq!(
            nums("و3 أشياء، الـ20، بـ٤٥٠ نانومتر، ۷ أيام"),
            [3.0, 20.0, 450.0, 7.0]
        );
        assert_eq!(nums("٣ ملايين نسمة و 5 آلاف و٢٫٥ مليار"), [3e6, 5e3, 2.5e9]);
        // The alef with and without its hamza is one word.
        assert_eq!(nums("3 الف"), [3e3]);
        assert!(read("في 12 مارس 2020").contains(&(Claim::Month(3), "مارس".into())));
        assert!(read("في ١٢ آذار ٢٠٢٠")
            .iter()
            .any(|c| c.0 == Claim::Month(3)));
        assert!(read("12 ومارس").iter().any(|c| c.0 == Claim::Month(3)));
        // The same number in a source and in a script, in two digit forms.
        assert!(unmatched("طوله ٤٥٠ نانومتر", std::iter::once("about 450 nm")).is_empty());
        assert_eq!(
            unmatched("طوله ٤٧٠ نانومتر", std::iter::once("about 450 nm")),
            ["470"]
        );
    }

    #[test]
    fn a_folded_match_gives_the_words_of_the_page() {
        // The page has harakat, a tatweel, Arabic-Indic digits and a line break; the copy has none of them.
        let page = "مقدمة. يَبلُغ طُول المـوجة\n ٤٥٠ نانومتر تقريبًا. خاتمة";
        let r = find_folded(page, "يبلغ طول الموجة 450 نانومتر تقريبا").unwrap();
        assert_eq!(&page[r], "يَبلُغ طُول المـوجة\n ٤٥٠ نانومتر تقريبًا");
        assert!(has_folded(
            "Café “Zürich” — 10 km",
            "Cafe \"Zurich\" - 10 km"
        ));
        // A presentation form and a Persian yeh are the plain letters.
        assert!(has_folded("ﻻ يوجد علی", "لا يوجد علي"));
        assert!(find_folded(page, "طول الموجة 470").is_none());
    }

    #[test]
    fn the_text_has_every_card_of_a_scene() {
        let v = serde_json::json!({"scenes": [{"script": "x", "card": {"kind": "stat", "text": "42%"}},
            {"script": "y", "cards": [{"kind": "stat", "text": "17%", "label": "in 2020"},
                {"kind": "bars", "items": ["A"], "values": [9]}]}]});
        assert_eq!(nums(&text_of(&v, "x y")), [42.0, 17.0, 2020.0, 9.0]);
    }

    #[test]
    fn a_number_line_gives_its_points_not_its_scale_and_a_table_gives_its_cells() {
        let v = serde_json::json!({"scenes": [{"script": "x", "cards": [
            {"kind": "number_line", "values": [0, 30], "points": [17], "from": 6},
            {"kind": "table", "rows": [["Area", "2021"], ["North", "5%"]]}]}]});
        assert_eq!(nums(&text_of(&v, "x")), [17.0, 6.0, 2021.0, 5.0]);
    }

    #[test]
    fn a_month_counts_only_next_to_a_number() {
        assert!(read("They may march on.").is_empty());
        assert!(read("On 12 March 2020 it began.").contains(&(Claim::Month(3), "March".into())));
    }

    #[test]
    fn the_gate_warns_on_a_number_with_no_source_and_passes_on_a_match() {
        let src = |t: &str| Source {
            text: t.into(),
            url: None,
            title: Some("A book".into()),
            publisher: None,
        };
        let s = [
            src("Blue light has a wavelength near 450 nm"),
            src("It grew 5,000,000 in March 2020"),
        ];
        assert_eq!(
            gate("Blue is near 450 nm.", &s, false).unwrap().status,
            "pass"
        );
        assert_eq!(
            gate("It grew 5 million in March 2020.", &s, false)
                .unwrap()
                .status,
            "pass"
        );
        let w = gate("Blue is near 470 nm, said in April 2020.", &s, false).unwrap();
        assert_eq!(w.status, "warn");
        assert!(
            w.detail.contains("470") && w.detail.contains("April"),
            "{}",
            w.detail
        );
        assert_eq!(gate("Near 470 nm.", &s, true).unwrap().status, "fail");
        // A pause tag is not a claim.
        assert_eq!(
            gate("Blue is near 450 nm. [pause 1.5] Yes.", &s, true)
                .unwrap()
                .status,
            "pass"
        );
        // No sources: no finding, as before.
        assert!(gate("Near 470 nm.", &[], true).is_none());
        // The name of a source is not a claim. The title of a page next to a publisher is not a name.
        let named = |title: &str, publisher: Option<&str>| {
            [Source {
                text: "A fact.".into(),
                url: None,
                title: Some(title.into()),
                publisher: publisher.map(String::from),
            }]
        };
        let status = |text: &str, s: &[Source]| gate(text, s, false).unwrap().status;
        assert_eq!(
            status("Sources: France 24", &named("France 24", None)),
            "pass"
        );
        assert_eq!(
            status("Sources: France 24", &named("A page", Some("France 24"))),
            "pass"
        );
        assert_eq!(
            status("It was in 2024.", &named("2024 election", Some("A site"))),
            "warn"
        );
    }
}
