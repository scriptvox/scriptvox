//! Delivery cues (`[whisper]`), IPA forms in `say`, and SSML in a script (docs/specs/delivery-cues.md).
//!
//! A voice row states what it takes: `takes_tags` (the cues) and `takes_ipa`. For a row that does not take one, the
//! cue is dropped or the respelled form is read, with one warning. Captions never show a cue: [`crate::pauses::strip`]
//! removes every known one.

use crate::gates::Finding;
use crate::voices::Voice;

/// The cues a script may hold. A closed list: `[sic]`, `[1]` and `[paused]` stay text, as before.
pub const KNOWN: [&str; 20] = [
    "whisper",
    "whispers",
    "excited",
    "happy",
    "sad",
    "angry",
    "calm",
    "serious",
    "cheerful",
    "curious",
    "sarcastic",
    "shouting",
    "shouts",
    "laughs",
    "laughing",
    "chuckles",
    "sighs",
    "gasps",
    "crying",
    "surprised",
];

fn cue(body: &str) -> Option<&'static str> {
    let b = body.trim().to_lowercase();
    KNOWN.iter().find(|k| **k == b).copied()
}

/// `text` with each known cue kept (`keep` says so) or taken out, and the cues taken out, in order.
fn filter(text: &str, keep: impl Fn(&str) -> bool) -> (String, Vec<&'static str>) {
    let (mut out, mut gone) = (String::with_capacity(text.len()), Vec::new());
    let mut rest = text;
    while let Some(i) = rest.find('[') {
        let Some(j) = rest[i..].find(']').map(|j| i + j) else {
            break;
        };
        match cue(&rest[i + 1..j]) {
            Some(c) if !keep(c) => {
                out.push_str(&rest[..i]);
                gone.push(c);
            }
            _ => out.push_str(&rest[..=j]),
        }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    if gone.is_empty() {
        return (text.to_string(), gone);
    }
    // One space where a cue stood between two words, none before a mark.
    let joined = out.split_whitespace().collect::<Vec<_>>().join(" ");
    (joined, gone)
}

/// The text with no cue: what captions, gates and word counts read.
pub fn remove(text: &str) -> String {
    if !text.contains('[') {
        return text.to_string();
    }
    filter(text, |_| false).0
}

/// The script for one voice: the cues its row takes stay for the engine, the others go. Second: the cues that went.
pub fn for_voice(text: &str, voice: &Voice) -> (String, Vec<&'static str>) {
    filter(text, |c| voice.takes_tags.iter().any(|t| t == c))
}

/// SSML tag names. Only these stop a run: `<3` and `a < b` are text.
const SSML: [&str; 13] = [
    "speak", "break", "emphasis", "prosody", "phoneme", "say-as", "sub", "voice", "lang", "mark",
    "audio", "p", "s",
];

pub const SSML_FIX: &str = "Take the tag out. A pause is `[pause 2]`, the pace of the voice is brief `voice_rate`, the way a word is said is brief `say`, and a voice is chosen with `voice`.";

/// `<break time="4s"/>` becomes `[pause 4]`; the count of such tags is the second value. Any other SSML tag is an
/// error: no voice here takes SSML, so the tag would be read aloud or shown in the captions.
pub fn ssml(text: &str) -> Result<(String, usize), String> {
    if !text.contains('<') {
        return Ok((text.to_string(), 0));
    }
    let (mut out, mut breaks) = (String::with_capacity(text.len()), 0);
    let mut rest = text;
    while let Some(i) = rest.find('<') {
        let tag_end = rest[i..].find('>').map(|j| i + j);
        let body = tag_end.map_or("", |j| rest[i + 1..j].trim());
        let name: String = body
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphabetic() || *c == '-')
            .collect::<String>()
            .to_lowercase();
        let (Some(j), true) = (tag_end, SSML.contains(&name.as_str())) else {
            out.push_str(&rest[..=i]);
            rest = &rest[i + 1..];
            continue;
        };
        if name != "break" {
            return Err(format!(
                "the script has the SSML tag <{name}>, and no voice here takes SSML: it would be read aloud or shown in the captions"
            ));
        }
        out.push_str(&rest[..i]);
        out.push_str(&format!(" [pause {}] ", break_seconds(body)));
        breaks += 1;
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    if breaks == 0 {
        return Ok((text.to_string(), 0));
    }
    // One space around each new pause tag: the same text as the tag written by hand, so the same plan.
    Ok((out.split_whitespace().collect::<Vec<_>>().join(" "), breaks))
}

/// The `time` of a break tag as a pause value: `4s`, `500ms`. No time (a `strength`, or nothing) is 1 second.
fn break_seconds(body: &str) -> String {
    let time = body.split_once("time").map(|(_, r)| {
        r.trim_start_matches(|c: char| c == '=' || c.is_whitespace() || c == '"' || c == '\'')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '.')
            .collect::<String>()
    });
    match time.as_deref() {
        Some(t) if t.ends_with("ms") && t[..t.len() - 2].parse::<f64>().is_ok() => t.to_string(),
        Some(t) if t.trim_end_matches('s').parse::<f64>().is_ok() => {
            t.trim_end_matches('s').to_string()
        }
        _ => "1".to_string(),
    }
}

/// Each script text of the brief, changed in place by `f`: the script, the scenes and the dub lines.
fn each_text(
    a: &mut crate::make::Args,
    mut f: impl FnMut(&mut String) -> Result<(), String>,
) -> Result<(), String> {
    f(&mut a.script)?;
    for sc in a.scenes.iter_mut().flatten() {
        f(&mut sc.script)?;
    }
    a.lines.iter_mut().try_for_each(|l| f(&mut l.text))
}

/// Before anything reads the script: SSML break tags become pause tags. Gives their count.
pub fn take_ssml(a: &mut crate::make::Args) -> Result<usize, String> {
    // The script of a brief with scenes is the scenes joined: the tags of the script are counted, and those of the
    // scenes and the lines only when the script has none.
    let (mut first, mut in_script, mut rest) = (true, 0, 0);
    each_text(a, |t| {
        let (text, n) = ssml(t)?;
        if n > 0 {
            *t = text;
        }
        if std::mem::take(&mut first) {
            in_script = n;
        } else {
            rest += n;
        }
        Ok(())
    })?;
    Ok(if in_script > 0 { in_script } else { rest })
}

/// Once the voice is known: the cues it does not take leave every script text, an IPA form it cannot say stops the
/// run, and the warnings for the plan and the run are made.
pub fn for_run(
    a: &mut crate::make::Args,
    voice: &Voice,
    breaks: usize,
) -> Result<Vec<Finding>, String> {
    if voice.takes_ipa && voice.engine == crate::kokoro_rust::ENGINE {
        for k in crate::say::ipa_terms(&voice.say) {
            crate::kokoro_rust::ipa(crate::say::forms(&voice.say[k]).0.unwrap_or_default())
                .map_err(|e| format!("say \"{k}\": {e}"))?;
        }
    }
    // The script of a brief with scenes is the scenes joined, so a cue is seen twice: each name is listed once.
    let mut gone: Vec<&str> = Vec::new();
    each_text(a, |t| {
        let (text, went) = for_voice(t, voice);
        if !went.is_empty() {
            *t = text;
        }
        for c in went {
            if !gone.contains(&c) {
                gone.push(c);
            }
        }
        Ok(())
    })?;
    Ok(warnings(voice, &gone, breaks))
}

pub const IPA_FIX: &str = "Write the term with the sounds the voice has, or give a respelled form in place of the IPA (\"say\": {\"term\": \"respelled\"}), or choose another voice.";

/// One warning for each kind of thing the script asked for and this voice does not do. `cues` are the cues that went,
/// `breaks` is the count of SSML break tags that became pauses.
pub fn warnings(voice: &Voice, cues: &[&str], breaks: usize) -> Vec<Finding> {
    let mut out = Vec::new();
    if !cues.is_empty() {
        let mut names: Vec<String> = Vec::new();
        for c in cues {
            let n = format!("[{c}]");
            if !names.contains(&n) {
                names.push(n);
            }
        }
        out.push(Finding {
            gate: "cues",
            status: "warn",
            detail: format!(
                "the {} voice {} takes no {}: the voice reads the words with no cue, and the captions do not show it",
                voice.engine,
                voice.id,
                names.join(", ")
            ),
            fix: Some("`scriptvox voices --json` shows `takes_tags` on the rows that take cues. Choose one of them, or take the cue out of the script.".into()),
        });
    }
    let ipa: Vec<&str> = crate::say::ipa_terms(&voice.say);
    if !ipa.is_empty() && !voice.takes_ipa {
        out.push(Finding {
            gate: "say",
            status: "warn",
            detail: format!(
                "the {} voice {} takes no IPA: {}",
                voice.engine,
                voice.id,
                ipa.iter()
                    .map(|k| match crate::say::forms(&voice.say[*k]).1 {
                        Some(r) => format!("{k} is read as \"{r}\""),
                        None => format!("{k} is read as written"),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            fix: Some("`scriptvox voices --json` shows `takes_ipa` on the rows that take IPA. Choose one of them, or give a respelled form after the IPA: \"/ipa/ respelled\".".into()),
        });
    }
    if breaks > 0 {
        out.push(Finding {
            gate: "ssml",
            status: "warn",
            detail: format!(
                "{breaks} SSML <break> tag(s) in the script became pause tags: no voice here takes SSML"
            ),
            fix: Some("Write the pause as `[pause 2]` in the script.".into()),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(tags: &[&str]) -> Voice {
        Voice {
            takes_tags: tags.iter().map(|t| t.to_string()).collect(),
            ..crate::voices::all().remove(0)
        }
    }

    #[test]
    fn a_known_cue_goes_and_other_brackets_stay() {
        assert_eq!(
            remove("[whisper] Come closer. [Excited] We won! See [1] and [sic] and [paused]."),
            "Come closer. We won! See [1] and [sic] and [paused]."
        );
        assert_eq!(remove("no cue [pause 2] here"), "no cue [pause 2] here");
        assert_eq!(
            remove("an open [whisper bracket"),
            "an open [whisper bracket"
        );
    }

    #[test]
    fn a_voice_keeps_the_cues_its_row_takes() {
        let (text, gone) = for_voice("[whisper] Come. [excited] Now!", &voice(&["whisper"]));
        assert_eq!(
            (text.as_str(), gone),
            ("[whisper] Come. Now!", vec!["excited"])
        );
        let (text, gone) = for_voice("[whisper] Come. [excited] Now!", &voice(&[]));
        assert_eq!((text.as_str(), gone.len()), ("Come. Now!", 2));
        // No cue in the script: the same bytes, so the voice key and the plan id do not move.
        let plain = "Two  spaces stay.";
        assert_eq!(for_voice(plain, &voice(&[])).0, plain);
    }

    #[test]
    fn a_break_tag_becomes_a_pause_and_other_ssml_stops() {
        let (t, n) =
            ssml("One.<break time=\"4s\"/>Two. <break time='500ms' /> Three <break/> four")
                .unwrap();
        assert_eq!(n, 3);
        assert_eq!(
            crate::pauses::parse(&t),
            vec![
                crate::pauses::Seg::Speech("One.".into()),
                crate::pauses::Seg::Pause(4.0),
                crate::pauses::Seg::Speech("Two.".into()),
                crate::pauses::Seg::Pause(0.5),
                crate::pauses::Seg::Speech("Three".into()),
                crate::pauses::Seg::Pause(1.0),
                crate::pauses::Seg::Speech("four".into()),
            ]
        );
        let e = ssml("This is <emphasis level=\"strong\">big</emphasis>.").unwrap_err();
        assert!(e.contains("<emphasis>"), "{e}");
        assert!(ssml("<speak>Hello</speak>").is_err());
        // Not SSML: the text stays as it is.
        for plain in ["a < b and b > a", "I <3 dates", "x<y", "<b>bold</b>"] {
            assert_eq!(ssml(plain).unwrap(), (plain.to_string(), 0));
        }
    }

    #[test]
    fn one_warning_for_each_kind() {
        let mut v = voice(&[]);
        v.say.insert("Nguyen".into(), "/ŋwiən/ win".into());
        v.say.insert("Doha".into(), "/doʊhə/".into());
        v.say.insert("ACME".into(), "A C M E".into());
        let w = warnings(&v, &["whisper", "whisper", "excited"], 2);
        assert_eq!(w.len(), 3);
        assert!(
            w[0].detail.contains("[whisper], [excited]"),
            "{}",
            w[0].detail
        );
        assert!(
            w[1].detail.contains("Nguyen is read as \"win\"")
                && w[1].detail.contains("Doha is read as written"),
            "{}",
            w[1].detail
        );
        assert!(!w[1].detail.contains("ACME"));
        assert!(w.iter().all(|f| f.status == "warn" && f.fix.is_some()));
        v.takes_ipa = true;
        assert!(warnings(&v, &[], 0).is_empty());
    }

    /// A row may only state cues from the closed list, so `strip` takes each of them out of the captions.
    #[test]
    fn every_row_states_known_cues() {
        for v in crate::voices::all() {
            for t in &v.takes_tags {
                assert!(KNOWN.contains(&t.as_str()), "{} {}: {t}", v.engine, v.id);
            }
        }
    }
}
