//! What the voice reads: symbols in the script become words. Captions keep the symbol ("≤ 40"), the voice says
//! "at most 40". Measured 4 Nov 2026: the open voice read "≤" as "SF", "2×" as "2km" and "≈ 4" as "04".

use crate::captions::Word;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// (symbol, English, Arabic). An empty word is a short pause (a comma).
// ponytail: one reading per symbol; a context-aware reading ("→" as "to" between numbers) when a review asks.
const TABLE: &[(char, &str, &str)] = &[
    ('≤', "at most", "حتى"),
    ('≥', "at least", "لا يقل عن"),
    ('≠', "is not", "لا يساوي"),
    ('≈', "about", "حوالي"),
    ('×', "times", "في"),
    ('÷', "divided by", "على"),
    ('±', "plus or minus", "زائد أو ناقص"),
    ('√', "the square root of", "الجذر التربيعي لـ"),
    ('∞', "infinity", "ما لا نهاية"),
    ('→', "", ""),
    ('←', "", ""),
    ('⇒', "", ""),
];

/// The same symbols in Spanish, French, German, Portuguese and Indonesian (docs/specs/latin-languages.md).
const LATIN: &[(char, [&str; 5])] = &[
    (
        '≤',
        [
            "como máximo",
            "au plus",
            "höchstens",
            "no máximo",
            "paling banyak",
        ],
    ),
    (
        '≥',
        [
            "al menos",
            "au moins",
            "mindestens",
            "pelo menos",
            "paling sedikit",
        ],
    ),
    (
        '≠',
        [
            "no es igual a",
            "n'est pas égal à",
            "ist nicht gleich",
            "não é igual a",
            "tidak sama dengan",
        ],
    ),
    (
        '≈',
        [
            "aproximadamente",
            "environ",
            "ungefähr",
            "aproximadamente",
            "sekitar",
        ],
    ),
    ('×', ["por", "fois", "mal", "vezes", "kali"]),
    (
        '÷',
        [
            "dividido por",
            "divisé par",
            "geteilt durch",
            "dividido por",
            "dibagi",
        ],
    ),
    (
        '±',
        [
            "más o menos",
            "plus ou moins",
            "plus oder minus",
            "mais ou menos",
            "plus minus",
        ],
    ),
    (
        '√',
        [
            "la raíz cuadrada de",
            "la racine carrée de",
            "die Quadratwurzel aus",
            "a raiz quadrada de",
            "akar kuadrat dari",
        ],
    ),
    (
        '∞',
        [
            "infinito",
            "l'infini",
            "unendlich",
            "infinito",
            "tak terhingga",
        ],
    ),
];

/// The words of one symbol in `lang`; English for a language with no column.
fn symbol_words(c: char, lang: &str) -> Option<&'static str> {
    let &(_, en, ar) = TABLE.iter().find(|t| t.0 == c)?;
    let col = ["es", "fr", "de", "pt", "id"]
        .iter()
        .position(|l| *l == lang);
    Some(match (lang, col) {
        ("ar", _) => ar,
        (_, Some(i)) if !en.is_empty() => LATIN.iter().find(|t| t.0 == c).map_or(en, |t| t.1[i]),
        _ => en,
    })
}

/// A `say` value in its two forms: `/ŋwiən/` is IPA, `/ŋwiən/ win` is IPA and a respelled form, `win` is a respelled
/// form only (docs/specs/delivery-cues.md).
pub fn forms(value: &str) -> (Option<&str>, Option<&str>) {
    let v = value.trim();
    let ipa = v
        .strip_prefix('/')
        .and_then(|r| r.split_once('/'))
        .filter(|(ipa, _)| !ipa.trim().is_empty());
    match ipa {
        Some((ipa, rest)) => (
            Some(ipa.trim()),
            Some(rest.trim()).filter(|r| !r.is_empty()),
        ),
        None => (None, Some(value)),
    }
}

/// The `say` terms that carry an IPA form.
pub fn ipa_terms(say: &BTreeMap<String, String>) -> Vec<&str> {
    say.iter()
        .filter(|(_, v)| forms(v).0.is_some())
        .map(|(k, _)| k.as_str())
        .collect()
}

/// The word that stands for the `n`th IPA term in the text a `takes_ipa` engine gets: letters only, so the engine
/// reads it as one word and finds its sounds in [`ipa_words`].
fn ipa_word(n: usize) -> String {
    let letter = |i: usize| char::from(b'a' + (i % 26) as u8);
    format!("svxipa{}{}", letter(n / 26), letter(n))
}

/// For a `takes_ipa` engine: each stand-in word of [`apply_ipa`] and its IPA form.
pub fn ipa_words(say: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    ipa_terms(say)
        .iter()
        .enumerate()
        .map(|(n, k)| {
            (
                ipa_word(n),
                forms(&say[*k]).0.unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// [`apply`] for a `takes_ipa` engine: a term with an IPA form becomes its stand-in word.
pub fn apply_ipa(text: &str, say: &BTreeMap<String, String>) -> String {
    let ipa = ipa_terms(say);
    if ipa.is_empty() {
        return apply(text, say);
    }
    let mut with: BTreeMap<String, String> = say.clone();
    for (n, k) in ipa.iter().enumerate() {
        with.insert(k.to_string(), ipa_word(n));
    }
    apply(text, &with)
}

/// Written forms the voice must say another way (brief `say`, `{"ACME": "A C M E"}`). Whole words only, longest first; the
/// captions keep the written form. A value with an IPA form gives its respelled form, or the written word when it has
/// none: an engine that takes IPA goes through [`apply_ipa`].
pub fn apply(text: &str, say: &BTreeMap<String, String>) -> String {
    if say.is_empty() {
        return text.to_string();
    }
    let mut keys: Vec<&String> = say.keys().filter(|k| !k.is_empty()).collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut before = None::<char>;
    'next: while let Some(c) = rest.chars().next() {
        if !before.is_some_and(char::is_alphanumeric) {
            for k in &keys {
                if let Some(tail) = rest.strip_prefix(k.as_str()) {
                    if !tail.chars().next().is_some_and(char::is_alphanumeric) {
                        out.push_str(forms(&say[k.as_str()]).1.unwrap_or(k));
                        before = k.chars().last();
                        rest = tail;
                        continue 'next;
                    }
                }
            }
        }
        out.push(c);
        before = Some(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// A term file (`--terms`): words speech to text must get right.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    /// Right spellings: a hint to speech to text, and any case of one is set right.
    #[serde(default)]
    pub terms: Vec<String>,
    /// What it hears, then what it must write: `{"ak me": "ACME"}`.
    #[serde(default)]
    pub fix: BTreeMap<String, String>,
}

impl Terms {
    pub fn load(path: &Path) -> Result<Terms, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| {
            format!(
                "{}: {e}. It holds `terms` (a list) and `fix` (an object).",
                path.display()
            )
        })
    }

    /// Whisper's prompt: it copies the spellings it sees there.
    pub fn hint(&self) -> String {
        let all: Vec<&str> = self
            .terms
            .iter()
            .map(String::as_str)
            .chain(self.fix.values().map(String::as_str))
            .collect();
        if all.is_empty() {
            String::new()
        } else {
            format!("{}.", all.join(", "))
        }
    }

    /// Sets the listed words right, on their times, and lists each change. Several heard words become one, from the
    /// start of the first to the end of the last; the last word's punctuation stays.
    pub fn fix_words(&self, words: &mut Vec<Word>) -> Vec<Value> {
        let norm = |s: &str| -> String {
            s.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect()
        };
        let mut pairs: Vec<(Vec<String>, &str)> = self
            .terms
            .iter()
            .map(|t| (t.as_str(), t.as_str()))
            .chain(self.fix.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .map(|(heard, to)| (heard.split_whitespace().map(norm).collect(), to))
            .collect();
        pairs.sort_by_key(|(h, _)| std::cmp::Reverse(h.len()));
        let mut fixes = Vec::new();
        let mut i = 0;
        while i < words.len() {
            let hit = pairs.iter().find(|(h, _)| {
                !h.is_empty()
                    && i + h.len() <= words.len()
                    && h.iter().zip(&words[i..]).all(|(t, w)| *t == norm(&w.text))
            });
            if let Some((h, to)) = hit {
                let last = &words[i + h.len() - 1];
                let end = last.end;
                let tail: String = last
                    .text
                    .trim_end()
                    .chars()
                    .rev()
                    .take_while(|c| !c.is_alphanumeric())
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                let was: Vec<&str> = words[i..i + h.len()]
                    .iter()
                    .map(|w| w.text.trim())
                    .collect();
                let was = was.join(" ");
                let now = format!("{to}{tail}");
                if was != now {
                    fixes.push(json!({"at": words[i].start, "heard": was, "wrote": now}));
                    words[i].text = now;
                    words[i].end = end;
                    words.drain(i + 1..i + h.len());
                }
            }
            i += 1;
        }
        fixes
    }
}

/// The script as the voice must read it, for `lang` ("ar" or other).
pub fn spoken(text: &str, lang: &str) -> String {
    if !text.chars().any(|c| TABLE.iter().any(|t| t.0 == c)) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 16);
    for c in text.chars() {
        match symbol_words(c, lang) {
            Some(w) => {
                if w.is_empty() {
                    out.push_str(if lang == "ar" { "،" } else { "," });
                } else {
                    out.push(' ');
                    out.push_str(w);
                }
                out.push(' ');
            }
            None => out.push(c),
        }
    }
    // One space between words, no space before a comma.
    let joined = out.split_whitespace().collect::<Vec<_>>().join(" ");
    joined
        .replace(" ,", ",")
        .replace(" ،", "،")
        .replace(",,", ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn say_changes_whole_words_only() {
        let say = BTreeMap::from([("ACME".to_string(), "A C M E".to_string())]);
        assert_eq!(apply("ACME, not XACMEX.", &say), "A C M E, not XACMEX.");
        assert_eq!(apply("عند ACME اليوم", &say), "عند A C M E اليوم");
        let long = BTreeMap::from([
            ("Al Ain".to_string(), "Al-Ayn".to_string()),
            ("Al".to_string(), "x".to_string()),
        ]);
        assert_eq!(apply("Al Ain and Al.", &long), "Al-Ayn and x.");
    }

    #[test]
    fn a_say_value_carries_ipa_a_respelled_form_or_both() {
        assert_eq!(forms("A C M E"), (None, Some("A C M E")));
        assert_eq!(forms("/ŋwiən/ win"), (Some("ŋwiən"), Some("win")));
        assert_eq!(forms("/ŋwiən/"), (Some("ŋwiən"), None));
        // Not IPA: one slash, or nothing between two.
        assert_eq!(forms("km/h"), (None, Some("km/h")));
        assert_eq!(forms("// x"), (None, Some("// x")));
        let say = BTreeMap::from([
            ("Nguyen".to_string(), "/ŋwiən/ win".to_string()),
            ("Doha".to_string(), "/doʊhə/".to_string()),
            ("ACME".to_string(), "A C M E".to_string()),
        ]);
        // A voice with no IPA: the respelled form, else the written word.
        assert_eq!(
            apply("Nguyen of ACME in Doha.", &say),
            "win of A C M E in Doha."
        );
        // A voice with IPA: a stand-in word for each IPA term, and its sounds beside it.
        assert_eq!(
            apply_ipa("Nguyen of ACME in Doha.", &say),
            "svxipaab of A C M E in svxipaaa."
        );
        assert_eq!(
            ipa_words(&say),
            BTreeMap::from([
                ("svxipaaa".to_string(), "doʊhə".to_string()),
                ("svxipaab".to_string(), "ŋwiən".to_string())
            ])
        );
        let plain = BTreeMap::from([("ACME".to_string(), "A C M E".to_string())]);
        assert_eq!(apply_ipa("ACME.", &plain), apply("ACME.", &plain));
    }

    #[test]
    fn terms_fix_words_on_their_times_and_list_each() {
        let w = |t: &str, s, e| Word {
            text: t.into(),
            start: s,
            end: e,
        };
        let terms = Terms {
            terms: vec!["ACME".into(), "Al Ain".into()],
            fix: BTreeMap::from([("ak me".to_string(), "ACME".to_string())]),
        };
        let mut words = vec![
            w("acme", 0.0, 0.5),
            w("in", 0.5, 0.7),
            w("al", 0.7, 0.9),
            w("ain.", 0.9, 1.2),
            w("Ak", 1.5, 1.7),
            w("me,", 1.7, 1.9),
            w("ACME", 2.0, 2.5),
        ];
        let fixes = terms.fix_words(&mut words);
        let text: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(text, ["ACME", "in", "Al Ain.", "ACME,", "ACME"]);
        assert_eq!((words[2].start, words[2].end), (0.7, 1.2));
        assert_eq!((words[3].start, words[3].end), (1.5, 1.9));
        assert_eq!(fixes.len(), 3, "a word already right is not listed");
        assert_eq!(fixes[1]["heard"], "al ain.");
        assert_eq!(terms.hint(), "ACME, Al Ain, ACME.");
    }

    #[test]
    fn symbols_become_words() {
        assert_eq!(spoken("speed ≤ 40 km/h", "en"), "speed at most 40 km/h");
        assert_eq!(
            spoken("about 2× the distance", "en"),
            "about 2 times the distance"
        );
        assert_eq!(spoken("Rain → slow down.", "en"), "Rain, slow down.");
        assert_eq!(spoken("السرعة ≤ 40", "ar"), "السرعة حتى 40");
        assert_eq!(spoken("velocidad ≤ 40", "es"), "velocidad como máximo 40");
        assert_eq!(spoken("vitesse ≈ 40", "fr"), "vitesse environ 40");
        assert_eq!(spoken("Regen → langsam", "de"), "Regen, langsam");
        assert_eq!(spoken("2× a distância", "pt"), "2 vezes a distância");
        assert_eq!(
            spoken("kecepatan ≥ 40", "id"),
            "kecepatan paling sedikit 40"
        );
        assert_eq!(
            spoken("plain text, no symbols", "en"),
            "plain text, no symbols"
        );
    }
}
