//! Pause tags: `[pause 1.5]`, `(silence: 800ms)`, `[توقف ١٫٥ ثانية]`.
//!
//! Tag rules ported from MoneyPrinterTurbo `app/utils/utils.py`
//! (Copyright (c) 2024 harry0703, MIT): keyword list, units, 1 s default,
//! 0.1 s to 10 s clamp, merged runs, and invalid tags removed so TTS never
//! reads them. ScriptVox adds Arabic keywords and units, Arabic-Indic digits,
//! and a hand-written scanner (no regex dependency).

#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    Speech(String),
    Pause(f64),
}

const KEYWORDS: [&str; 23] = [
    "pause",
    "pausa",
    "silence",
    "silencio",
    "silêncio",
    "silenzio",
    "stille",
    "пауза",
    "тишина",
    "停顿",
    "暂停",
    "静音",
    "ポーズ",
    "一時停止",
    "無音",
    "일시중지",
    "정지", // MPT list
    "توقف",
    "وقفة",
    "صمت",
    "سكوت",
    "استراحة",
    "مهلة", // Arabic (ScriptVox)
];
const MIN_S: f64 = 0.1;
const MAX_S: f64 = 10.0;

/// Arabic-Indic and Persian digits, and the Arabic decimal separator, to ASCII.
fn ascii_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{0660}'..='\u{0669}' => char::from(b'0' + (c as u32 - 0x0660) as u8),
            '\u{06F0}'..='\u{06F9}' => char::from(b'0' + (c as u32 - 0x06F0) as u8),
            '\u{066B}' | ',' => '.',
            _ => c,
        })
        .collect()
}

/// Parses a tag body (without brackets). `None`: not a pause tag.
/// `Some(None)`: a pause tag with an invalid value (removed, no silence).
fn tag(body: &str) -> Option<Option<f64>> {
    tag_raw(body).map(|t| t.map(|s| s.clamp(MIN_S, MAX_S)))
}

/// [`tag`] before the limits.
fn tag_raw(body: &str) -> Option<Option<f64>> {
    let b = body.trim();
    let lower = b.to_lowercase();
    let kw = KEYWORDS.iter().find(|k| lower.starts_with(*k))?;
    let rest = &b[b
        .char_indices()
        .nth(kw.chars().count())
        .map_or(b.len(), |(i, _)| i)..];
    // A letter right after the keyword means a normal word ("[paused]").
    if rest.chars().next().is_some_and(|c| c.is_alphabetic()) {
        return None;
    }
    let arg = ascii_digits(
        rest.trim_start_matches(|c: char| c.is_whitespace() || c == ':' || c == '：')
            .trim(),
    );
    if arg.is_empty() {
        return Some(Some(1.0));
    }
    let split = arg
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '+' || c == '-'))
        .unwrap_or(arg.len());
    let (num, unit) = (arg[..split].trim(), arg[split..].trim().to_lowercase());
    let Ok(v) = num.parse::<f64>() else {
        return Some(None);
    };
    let secs = match unit.as_str() {
        "" | "s" | "sec" | "secs" | "second" | "seconds" | "秒" | "ث" | "ثانية" | "ثوان"
        | "ثواني" | "ثوانٍ" => v,
        "ms" | "msec" | "msecs" | "毫秒" | "مللي" | "ملي" => v / 1000.0,
        _ => return Some(None),
    };
    if secs <= 0.0 || !secs.is_finite() {
        return Some(None);
    }
    Some(Some(secs))
}

/// A pause tag of the script asks for more than the limit of 10 s, so it is cut: the caller says so.
/// ponytail: one tag only; a run of tags that add up to more than 10 s is cut without this.
pub fn capped(text: &str) -> bool {
    text.match_indices(['[', '(']).any(|(i, _)| {
        let body = &text[i + 1..];
        let body = &body[..body.find([']', ')']).unwrap_or(body.len())];
        matches!(tag_raw(body), Some(Some(s)) if s > MAX_S)
    })
}

/// Splits a script into speech and pause segments. Consecutive pauses merge
/// (and stay within the 10 s cap).
pub fn parse(text: &str) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    let mut speech = String::new();
    let push_speech = |speech: &mut String, out: &mut Vec<Seg>| {
        let t = speech.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.is_empty() {
            out.push(Seg::Speech(t));
        }
        speech.clear();
    };
    let mut rest = text;
    while let Some(i) = rest.find(['[', '(']) {
        let close = if rest[i..].starts_with('[') { ']' } else { ')' };
        let Some(j) = rest[i + 1..].find(close).map(|j| i + 1 + j) else {
            break;
        };
        match tag(&rest[i + 1..j]) {
            // An invalid tag disappears and the speech around it stays one segment.
            Some(None) => speech.push_str(&rest[..i]),
            Some(Some(s)) => {
                speech.push_str(&rest[..i]);
                push_speech(&mut speech, &mut out);
                match out.last_mut() {
                    Some(Seg::Pause(p)) => *p = (*p + s).min(MAX_S),
                    _ => out.push(Seg::Pause(s)),
                }
            }
            None => speech.push_str(&rest[..=j]),
        }
        rest = &rest[j + 1..];
    }
    speech.push_str(rest);
    push_speech(&mut speech, &mut out);
    out
}

/// The script without pause tags and without delivery cues (`[whisper]`): what captions show and what is counted.
pub fn strip(text: &str) -> String {
    parse(&crate::cues::remove(text))
        .into_iter()
        .filter_map(|s| {
            if let Seg::Speech(t) = s {
                Some(t)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use Seg::*;

    #[test]
    fn english_and_arabic_tags() {
        assert_eq!(
            parse("Hi. [pause 1.5] Bye."),
            vec![Speech("Hi.".into()), Pause(1.5), Speech("Bye.".into())]
        );
        assert_eq!(
            parse("مرحبا [توقف ١٫٥ ثانية] وداعا"),
            vec![Speech("مرحبا".into()), Pause(1.5), Speech("وداعا".into())]
        );
        assert_eq!(
            parse("a (silence: 800ms) b"),
            vec![Speech("a".into()), Pause(0.8), Speech("b".into())]
        );
        assert_eq!(parse("a [صمت] b")[1], Pause(1.0));
    }

    #[test]
    fn clamps_merges_and_removes_invalid() {
        assert_eq!(parse("a [pause 0.01] b")[1], Pause(0.1));
        assert_eq!(parse("a [pause 60] b")[1], Pause(10.0));
        assert_eq!(parse("a [pause 2][pause 3] b")[1], Pause(5.0));
        assert_eq!(
            parse("a [pause: nope] b [pause -2] c"),
            vec![Speech("a b c".into())]
        );
    }

    #[test]
    fn a_tag_above_the_limit_is_seen() {
        assert!(capped("Dance. [pause 11]"));
        assert!(capped("a (silence: 12000ms) b"));
        assert!(!capped("a [pause 10] b [paused 99] (see 11)"));
    }

    #[test]
    fn normal_brackets_are_speech() {
        assert_eq!(
            strip("We [paused] here (see note) [pause]."),
            "We [paused] here (see note) ."
        );
        assert_eq!(
            parse("no tags at all"),
            vec![Speech("no tags at all".into())]
        );
    }
}
