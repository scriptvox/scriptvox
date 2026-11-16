//! The `steps` card: a numbered process. Every step is on screen from the start; the one the voice is at is lit.
//! See docs/specs/cards-data.md.

use crate::card_kit::{Area, DIM, PLAIN};
use crate::cards::{balanced, digits_like, line, ring_part, Card};

pub const MAX_STEPS: usize = 5;
pub const MAX_TEXT: usize = 40;

pub fn problem(c: &Card) -> Option<String> {
    match &c.items {
        Some(v)
            if (2..=MAX_STEPS).contains(&v.len())
                && v.iter()
                    .all(|i| !i.trim().is_empty() && i.chars().count() <= MAX_TEXT) =>
        {
            None
        }
        _ => Some(format!(
            "a steps card needs `items`: 2 to {MAX_STEPS} steps of at most {MAX_TEXT} characters, in their order; shorten a step or split the process over two scenes"
        )),
    }
}

/// When each step is lit: (from, to). A step's keys are its words of 4 letters or more that no other step has.
/// The step is lit from the first moment, after the step before it, that the voice says a key (a spoken word that
/// starts or ends with the key counts: "checks" for "check", "والخطة" for "الخطة"), to the moment the next step is
/// lit; the last step stays lit. A step the voice never names follows the one before it after an even share of
/// the card's time. The list card's rule takes any word of 3 letters, so "the" lit a step too early (seen by eye).
pub(crate) fn lit(items: &[String], a: &Area) -> Vec<(f64, f64)> {
    let words = |t: &str| -> Vec<String> {
        t.split_whitespace()
            .map(crate::gates::norm_word)
            .filter(|w| w.chars().count() >= 4)
            .collect()
    };
    let all: Vec<Vec<String>> = items.iter().map(|t| words(t)).collect();
    let n = items.len().max(1);
    let share = ((a.end - a.start) * 0.8 / n as f64).min(2.0);
    let (mut cursor, mut prev) = (0usize, a.start);
    let mut at = vec![];
    for (i, mine) in all.iter().enumerate() {
        let keys: Vec<&String> = mine
            .iter()
            .filter(|k| {
                all.iter()
                    .enumerate()
                    .all(|(j, o)| j == i || !o.contains(k))
            })
            .collect();
        let hit = a.said[cursor.min(a.said.len())..].iter().position(|w| {
            let w = crate::gates::norm_word(&w.text);
            keys.iter()
                .any(|k| w == **k || w.starts_with(k.as_str()) || w.ends_with(k.as_str()))
        });
        let t = match hit {
            Some(j) => {
                cursor += j + 1;
                a.said[cursor - 1].start
            }
            None if i == 0 => a.start,
            None => prev + share,
        };
        prev = t.max(prev).min(a.end);
        at.push(prev);
    }
    (0..at.len())
        .map(|i| (at[i], at.get(i + 1).copied().unwrap_or(a.end)))
        .collect()
}

pub(crate) fn events(c: &Card, a: &Area) -> Vec<String> {
    let items = c.items.as_deref().unwrap_or(&[]);
    let n = items.len();
    if n == 0 {
        return vec![];
    }
    let room = a.bottom - a.top;
    let line_h = |s: u32| s as f32 * (1.25 * a.latin).max(1.0);
    // The text: as large as lets every step have at most two lines and all rows fit the room.
    let mut s = a.size;
    let (rows, row_h, r) = loop {
        let r = line_h(s) * 0.42;
        let text_w = a.w * 0.84 - 2.0 * r - s as f32 * 0.5;
        let rows: Vec<Vec<String>> = items
            .iter()
            .map(|t| balanced(a.wrap, t.trim(), s, text_w))
            .collect();
        let most = rows.iter().map(|l| l.len()).max().unwrap_or(1);
        // The box of a text is a fifth of its own size past it on each side (set on the text), plus a little air.
        let row_h = most as f32 * line_h(s) + s as f32 * 0.6;
        if (most <= 2 && row_h * n as f32 <= room) || s <= 6 {
            break (rows, row_h, r);
        }
        s -= 1;
    };
    let top = a.top + ((room - row_h * n as f32) / 2.0).max(0.0);
    let (disc_x, an, text_x) = if a.rtl {
        (a.w * 0.92 - r, 6, a.w * 0.92 - 2.0 * r - s as f32 * 0.5)
    } else {
        (a.w * 0.08 + r, 4, a.w * 0.08 + 2.0 * r + s as f32 * 0.5)
    };
    let accent = crate::captions::ACCENT;
    let times = lit(items, a);
    let mut out = vec![];
    for (i, lines) in rows.iter().enumerate() {
        let y = top + (i as f32 + 0.5) * row_h;
        let (from, to) = times[i];
        // The row in one state over a time. Only the card's own start and end fade.
        let mut put = |t0: f64, t1: f64, on: bool| {
            if t1 - t0 < 0.02 {
                return;
            }
            let fade = format!(
                "\\fad({},{})",
                if t0 <= a.start + 1e-6 { 200 } else { 0 },
                if t1 >= a.end - 1e-6 { 200 } else { 0 }
            );
            let disc = |colour: &str, rad: f32| {
                line(
                    t0,
                    t1,
                    &format!("\\an7\\pos(0,0){PLAIN}\\1c{colour}{fade}\\p1"),
                    &ring_part(disc_x, y, rad, 0.0, 0.0, 1.0),
                )
            };
            out.push(disc("&H000000&", r * 1.12));
            out.push(disc(if on { accent } else { "&H303030&" }, r));
            // The number: Latin digits in the Latin font, which stands in the middle of its line; Arabic-Indic
            // digits (when a step uses them) in the card's own font.
            let number = digits_like(items, (i + 1).to_string());
            let font = if number.is_ascii() {
                "\\fnNoto Sans"
            } else {
                ""
            };
            let digit_size = if number.is_ascii() {
                r * 1.5
            } else {
                r * 1.5 / a.latin
            };
            out.push(line(
                t0,
                t1,
                &format!(
                    "\\an5\\pos({disc_x:.0},{y:.0}){font}\\fs{digit_size:.0}{PLAIN}\\1c{}{fade}",
                    if on { "&H202020&" } else { DIM }
                ),
                &number,
            ));
            out.push(line(
                t0,
                t1,
                &format!(
                    "\\an{an}\\pos({text_x:.0},{y:.0})\\fs{s}\\bord{:.1}\\4a&H30&{}{fade}",
                    s as f32 * 0.2,
                    if on {
                        String::new()
                    } else {
                        format!("\\b0\\1c{DIM}")
                    }
                ),
                &lines
                    .iter()
                    .map(|l| a.text(l, s as f32))
                    .collect::<Vec<_>>()
                    .join("\\N"),
            ));
        };
        put(a.start, from, false);
        put(from.max(a.start), to, true);
        put(to, a.end, false);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card_kit::testkit::{assert_inside, boxes, events, word, FRAMES};
    use serde_json::json;

    fn card(v: serde_json::Value) -> Card {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn steps_are_checked() {
        let p = |v: serde_json::Value| card(v).problem().unwrap_or_default();
        assert_eq!(
            p(json!({"kind": "steps", "items": ["Write", "Plan", "Make"]})),
            ""
        );
        assert!(p(json!({"kind": "steps", "items": ["One"]})).contains("2 to 5 steps"));
        assert!(p(json!({"kind": "steps"})).contains("needs `items`"));
        assert!(
            p(json!({"kind": "steps", "items": ["a", "b", "c", "d", "e", "f"]}))
                .contains("2 to 5 steps")
        );
        assert!(
            p(json!({"kind": "steps", "items": ["a", "b"], "label": "How"}))
                .contains("does not draw `label`: remove it")
        );
        assert!(
            p(json!({"kind": "steps", "items": ["a", "b"], "stagger": 0.3}))
                .contains("does not draw `stagger`")
        );
    }

    #[test]
    fn the_step_the_voice_is_at_is_lit_and_the_others_are_dim() {
        let c = card(
            json!({"kind": "steps", "items": ["Write the brief", "Check the plan", "Making the video"]}),
        );
        // "the" is in every step and is said early: it lights nothing.
        let said = [
            word("write", 0.5),
            word("the", 0.8),
            word("brief", 1.0),
            word("then", 1.6),
            word("checks", 2.0),
            word("the", 2.3),
            word("making", 4.0),
        ];
        let ev = events(&c, "en", (1080, 1920), &said);
        let texts = |t: &str| -> Vec<&String> { ev.iter().filter(|e| e.ends_with(t)).collect() };
        // Step 2: dim to 2.0 s, lit to 4.0 s, dim to the end.
        let two = texts("Check the plan");
        assert_eq!(two.len(), 3, "{ev:#?}");
        assert!(two[0].contains(",0:00:00.00,0:00:02.00,") && two[0].contains(DIM));
        assert!(two[1].contains(",0:00:02.00,0:00:04.00,") && !two[1].contains(DIM));
        assert!(two[2].contains(",0:00:04.00,0:00:06.00,") && two[2].contains(DIM));
        // Only the card's start and end fade: no flicker when the light moves.
        assert!(
            two[0].contains("\\fad(200,0)")
                && two[1].contains("\\fad(0,0)")
                && two[2].contains("\\fad(0,200)")
        );
        // The last step stays lit to the end; exactly one accent disc is on screen at 3 s and it is step 2's.
        assert!(texts("Making the video")
            .last()
            .unwrap()
            .contains(",0:00:04.00,0:00:06.00,"));
        let lit_at_3: Vec<&String> = ev
            .iter()
            .filter(|e| e.contains("\\p1") && e.contains(crate::captions::ACCENT))
            .filter(|e| e.contains(",0:00:02.00,0:00:04.00,"))
            .collect();
        assert_eq!(lit_at_3.len(), 1);
        // The numbers 1, 2, 3 in the Latin font.
        for n in ["1", "2", "3"] {
            assert!(
                ev.iter()
                    .any(|e| e.contains("\\fnNoto Sans") && e.ends_with(&format!("}}{n}"))),
                "{n}"
            );
        }
    }

    #[test]
    fn every_steps_card_stays_in_its_room_and_arabic_mirrors() {
        let en = card(json!({"kind": "steps", "items": [
            "Write the brief with every scene in it", "Check the plan and what it will cost", "Make the video and wait for its end",
            "Look at the frames on a contact sheet", "Fix what is wrong and make it again"]}));
        let ar = card(json!({"kind": "steps", "items": [
            "اكتب الملخص وفيه كل مشهد من المشاهد", "راجع الخطة وما سوف تكلفه من المال", "اصنع الفيديو وانتظر حتى ينتهي تماما",
            "انظر إلى الإطارات في ورقة واحدة", "أصلح الخطأ ثم اصنع الفيديو مرة أخرى"]}));
        for (lang, c) in [("en", en), ("ar", ar)] {
            assert!(c.problem().is_none(), "{:?}", c.problem());
            for frame in FRAMES {
                let ev = events(&c, lang, frame, &[]);
                assert_inside(&ev, lang, frame, "steps");
                // The texts of two steps never share a height.
                let rows: Vec<_> = ev
                    .iter()
                    .zip(boxes(&ev, lang))
                    .filter(|(e, _)| {
                        e.contains(",0:00:00.00,") && (e.contains("\\an4") || e.contains("\\an6"))
                    })
                    .map(|(_, b)| b)
                    .collect();
                assert_eq!(rows.len(), 5, "{lang} {frame:?}");
                for w in rows.windows(2) {
                    assert!(w[0].3 <= w[1].1 + 1.0, "{lang} {frame:?}: {w:?}");
                }
                let fs: f32 = ev
                    .last()
                    .unwrap()
                    .split("\\fs")
                    .nth(1)
                    .unwrap()
                    .split('\\')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                assert!(
                    fs >= crate::card_kit::testkit::size(lang, frame) as f32 * 0.45,
                    "{lang} {frame:?}: {fs}"
                );
                // Arabic: the text ends at the right, left of its disc.
                assert_eq!(ev.iter().any(|e| e.contains("\\an6")), lang == "ar");
            }
        }
    }
}
