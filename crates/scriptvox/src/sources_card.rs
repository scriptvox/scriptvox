//! Brief `sources_card` (`docs/specs/sources-card.md`): a last card with the names of the publishers of the facts,
//! built by the code from `sources`. Names, not addresses. Off by default.

use crate::brief::{Brief, Source};

/// The characters of the names that fit one line of the frame. The line of a `lower_third` does not wrap. Measured
/// 13 Nov 2026 on a frame 1080 wide: 30 and 41 (Arabic) characters fit, 36 Latin characters ran 78 px past the edge.
fn room(aspect: Option<&str>) -> usize {
    let width = crate::make::frame(aspect.unwrap_or("9:16")).map_or(1080, |f| f.0) as usize;
    (width.saturating_sub(180) / 30).min(56)
}

/// The name of the publisher of one source: `publisher`, else `title`, else the host of `url`.
fn name(s: &Source) -> Option<String> {
    let text = |t: &Option<String>| {
        t.as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(String::from)
    };
    text(&s.publisher).or_else(|| text(&s.title)).or_else(|| {
        let host = s.url.as_deref()?.split_once("://")?.1;
        let host = host.split(['/', '?', '#']).next()?.rsplit('@').next()?;
        Some(host.strip_prefix("www.").unwrap_or(host).to_string()).filter(|h| !h.is_empty())
    })
}

/// The names in the order of first use, each one time, as many as the label holds.
fn label(sources: &[Source], arabic: bool, room: usize) -> String {
    let mut names: Vec<String> = Vec::new();
    for n in sources.iter().filter_map(name) {
        if !names.iter().any(|m| m.to_lowercase() == n.to_lowercase()) {
            names.push(n);
        }
    }
    let (comma, more) = if arabic {
        ("، ", " وغيرها")
    } else {
        (", ", " and more")
    };
    let mut out = String::new();
    for (i, n) in names.iter().enumerate() {
        let next = if out.is_empty() {
            n.clone()
        } else {
            format!("{out}{comma}{n}")
        };
        // Room for the words that say that names are left out, when this is not the last name.
        let tail = if i + 1 < names.len() {
            more.chars().count()
        } else {
            0
        };
        if next.chars().count() + tail > room {
            return if out.is_empty() {
                n.chars().take(room).collect()
            } else {
                format!("{out}{more}")
            };
        }
        out = next;
    }
    out
}

/// The headings of the card. A `lower_third` on the last scene with one of them is the Sources card.
const HEADINGS: [&str; 2] = ["Sources", "المصادر"];

/// Turns `sources_card: true` into a card on the last scene and clears the field, so a brief that is read a second
/// time (a run's request) does not get a second card. The card is a normal card after this: it is in the plan id only
/// when the field was set.
pub fn apply(b: &mut Brief) -> Result<(), String> {
    if b.sources_card.take() != Some(true) {
        return Ok(());
    }
    put(
        &mut b.scenes,
        &b.sources,
        b.lang.as_deref(),
        b.aspect.as_deref(),
    )
}

/// `resume --patch {"sources_card": true|false|null}`. A run keeps the card, not the field, so the patch works on the
/// card: an earlier Sources card is taken off the last scene, and with `true` a new one is built from the `sources`
/// and the scenes of the patched request. So the patch can be given two times, and `false` or `null` removes the card.
pub fn patch(
    scenes: &mut Option<Vec<crate::brief::Scene>>,
    sources: &[Source],
    lang: Option<&str>,
    aspect: &str,
    on: bool,
) -> Result<(), String> {
    if let Some(last) = scenes.as_mut().and_then(|s| s.last_mut()) {
        let ours = |c: &crate::cards::Card| {
            c.kind == crate::cards::Kind::LowerThird
                && c.text.as_deref().is_some_and(|t| HEADINGS.contains(&t))
        };
        if last.card.as_ref().is_some_and(ours) {
            last.card = None;
        }
        if let Some(v) = last.cards.as_mut() {
            v.retain(|c| !ours(c));
            // One card left: the form of a scene with one card, as a brief writes it.
            if v.len() < 2 {
                last.card = v.pop();
                last.cards = None;
            }
        }
    }
    if on {
        put(scenes, sources, lang, Some(aspect))?;
    }
    Ok(())
}

fn put(
    scenes: &mut Option<Vec<crate::brief::Scene>>,
    sources: &[Source],
    lang: Option<&str>,
    aspect: Option<&str>,
) -> Result<(), String> {
    let arabic = lang == Some("ar")
        || (lang.is_none()
            && scenes.iter().flatten().any(|s| {
                s.script
                    .chars()
                    .any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
            }));
    let names = label(sources, arabic, room(aspect));
    if names.is_empty() {
        return Err("sources_card needs `sources`: the card lists their publishers. Add `sources`, or drop sources_card".into());
    }
    let Some(last) = scenes.as_mut().and_then(|s| s.last_mut()) else {
        return Err("sources_card needs `scenes`: the card is on the last scene. Give the video as `scenes`, or drop sources_card".into());
    };
    let card: crate::cards::Card = serde_json::from_value(serde_json::json!({
        "kind": "lower_third", "text": HEADINGS[usize::from(arabic)], "label": names,
    }))
    .map_err(|e| e.to_string())?;
    // After the cards that the scene has: with no times they share the scene, and this one is the last.
    match (last.card.take(), last.cards.as_mut()) {
        (Some(c), _) => last.cards = Some(vec![c, card]),
        (None, Some(v)) if v.len() < 4 => v.push(card),
        (None, Some(_)) => return Err("sources_card: the last scene has 4 cards already, and a scene holds 4. Remove one, or drop sources_card".into()),
        (None, None) => last.card = Some(card),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn brief(v: serde_json::Value) -> Brief {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn the_card_lists_each_publisher_one_time_and_the_field_is_cleared() {
        let mut b = brief(
            json!({"sources_card": true, "aspect": "16:9", "scenes": [{"script": "One."}, {"script": "Two.", "card": {"kind": "stat", "text": "42%"}}],
            "sources": [{"text": "a", "url": "https://en.wikipedia.org/wiki/A", "publisher": "Wikipedia"},
                {"text": "b", "title": "France 24"}, {"text": "c", "url": "https://www.example.org/x"},
                {"text": "d", "url": "https://ar.wikipedia.org/wiki/B", "publisher": "wikipedia"}]}),
        );
        apply(&mut b).unwrap();
        assert_eq!(b.sources_card, None);
        let v = serde_json::to_value(&b).unwrap();
        assert!(v["scenes"][0].get("card").is_none());
        assert_eq!(
            v["scenes"][1]["cards"],
            json!([{"kind": "stat", "text": "42%"}, {"kind": "lower_third", "text": "Sources", "label": "Wikipedia, France 24, example.org"}])
        );
        // The name of a source is not a claim with no source.
        assert_eq!(
            crate::claims::gate_of(&v, "One. Two.", true)
                .unwrap()
                .status,
            "fail"
        ); // 42% has no source
        let mut ok = b.clone();
        ok.scenes.as_mut().unwrap()[1]
            .cards
            .as_mut()
            .unwrap()
            .remove(0);
        let v = serde_json::to_value(&ok).unwrap();
        assert_eq!(
            crate::claims::gate_of(&v, "One. Two.", true)
                .unwrap()
                .status,
            "pass"
        );
        // A second read changes nothing.
        let again = b.clone();
        apply(&mut b).unwrap();
        assert_eq!(b, again);
    }

    /// `resume --patch`: on a request that has the card already, `true` builds it again from the sources of this
    /// request (no second card), and `false` takes it off and leaves the scene as the brief wrote it.
    #[test]
    fn a_patch_puts_the_card_one_time_and_takes_it_off() {
        let mut b = brief(
            json!({"sources_card": true, "aspect": "16:9", "scenes": [{"script": "One."}, {"script": "Two.", "card": {"kind": "stat", "text": "42%"}}],
            "sources": [{"text": "a", "publisher": "Wikipedia"}]}),
        );
        let plain = {
            let mut p = b.clone();
            p.sources_card = None;
            p.scenes
        };
        apply(&mut b).unwrap();
        let with_card = b.scenes.clone();
        let more: Vec<Source> = serde_json::from_value(
            json!([{"text": "a", "publisher": "Wikipedia"}, {"text": "b", "publisher": "NASA"}]),
        )
        .unwrap();
        patch(&mut b.scenes, &b.sources, None, "16:9", true).unwrap();
        assert_eq!(b.scenes, with_card, "given two times: one card");
        patch(&mut b.scenes, &more, None, "16:9", true).unwrap();
        let v = serde_json::to_value(&b.scenes).unwrap();
        assert_eq!(v[1]["cards"].as_array().unwrap().len(), 2, "{v}");
        assert_eq!(v[1]["cards"][1]["label"], "Wikipedia, NASA");
        patch(&mut b.scenes, &more, None, "16:9", false).unwrap();
        assert_eq!(b.scenes, plain);
        // With no sources the card cannot be built, and the message says what to add.
        assert!(patch(&mut b.scenes, &[], None, "16:9", true)
            .unwrap_err()
            .contains("Add `sources`"));
    }

    #[test]
    fn arabic_and_the_limits() {
        let many: Vec<_> = (0..9)
            .map(|i| json!({"text": "x", "publisher": format!("Publisher number {i}")}))
            .collect();
        let mut b = brief(
            json!({"sources_card": true, "lang": "ar", "scenes": [{"script": "واحد."}],
            "sources": [{"text": "a", "publisher": "ويكيبيديا"}, {"text": "b", "publisher": "ناسا"}]}),
        );
        apply(&mut b).unwrap();
        let c = b.scenes.unwrap()[0].card.clone().unwrap();
        assert_eq!(
            (c.text.as_deref(), c.label.as_deref()),
            (Some("المصادر"), Some("ويكيبيديا، ناسا"))
        );
        // More names than the label holds: the first ones, and words that say that there are more.
        // A wide frame holds more names than a tall one (the default shape).
        let of = |aspect: Option<&str>| {
            let mut long = brief(
                json!({"sources_card": true, "aspect": aspect, "scenes": [{"script": "One."}], "sources": many}),
            );
            apply(&mut long).unwrap();
            long.scenes.unwrap()[0].card.clone().unwrap().label.unwrap()
        };
        assert_eq!(
            of(Some("16:9")),
            "Publisher number 0, Publisher number 1 and more"
        );
        assert_eq!(of(None), "Publisher number 0 and more");
        assert!(of(None).chars().count() <= 30 && of(Some("1:1")) == of(None));
        assert!(crate::claims::read(" and more").is_empty());
        // Off by default, and an error with a fix when it cannot be made.
        let mut off = brief(
            json!({"scenes": [{"script": "One."}], "sources": [{"text": "a", "title": "A"}]}),
        );
        apply(&mut off).unwrap();
        assert!(off.scenes.unwrap()[0].card.is_none());
        let mut none = brief(json!({"sources_card": true, "scenes": [{"script": "One."}]}));
        assert!(apply(&mut none).unwrap_err().contains("needs `sources`"));
        let mut flat = brief(
            json!({"sources_card": true, "script": "One.", "sources": [{"text": "a", "title": "A"}]}),
        );
        assert!(apply(&mut flat).unwrap_err().contains("needs `scenes`"));
    }
}
