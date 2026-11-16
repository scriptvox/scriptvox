//! A mark on a phrase of a text card: an underline, a ring or a highlight that comes in on a spoken word. See
//! docs/specs/card-mark.md. The marks are drawings added to the card's own events; where the phrase stands is
//! measured from the card's text event with the shaper that wrapped it.

use crate::captions::Measurer;
use crate::cards::{Card, Kind, Mark, MarkStyle};

/// The kinds whose text can take a mark (a formula has its own rule, `cards::events`).
const KINDS: [Kind; 4] = [Kind::Title, Kind::Stat, Kind::Quote, Kind::List];

fn words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(crate::gates::norm_word)
        .filter(|w| !w.is_empty())
        .collect()
}

/// How many times the words of `part` stand in a row in `text`.
fn count(text: &str, part: &[String]) -> usize {
    let w = words(text);
    (0..w.len()).filter(|&i| w[i..].starts_with(part)).count()
}

/// A problem with the marks of a text card, for the brief check.
pub fn problem(card: &Card) -> Option<String> {
    let marks = card.mark.as_deref()?;
    if !KINDS.contains(&card.kind) {
        return Some("only a formula, title, stat, quote or list card takes `mark`".into());
    }
    if card.behind == Some(true) {
        return Some("a title behind the subject takes no `mark`".into());
    }
    for m in marks {
        let part = words(&m.part);
        if part.is_empty() {
            return Some(format!(
                "mark: the part \"{}\" needs a word of the card's text",
                m.part
            ));
        }
        let n: usize = card.texts().iter().map(|t| count(t, &part)).sum();
        if n != 1 {
            return Some(format!(
                "mark: the part \"{}\" is in the card's text {n} times as whole words; it must be there once (write it as the card does{})",
                m.part,
                if n > 1 { ", with a word more" } else { "" }
            ));
        }
        if crate::gates::norm_word(&m.on).is_empty() {
            return Some(format!(
                "mark: the part \"{}\" needs `on`, one word of the scene's script",
                m.part
            ));
        }
    }
    None
}

fn seconds(t: &str) -> f64 {
    t.split(':')
        .fold(0.0, |sum, p| sum * 60.0 + p.parse::<f64>().unwrap_or(0.0))
}

/// The value after the last `\name` in an override block (the place where the text rests).
fn tag<'a>(tags: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("\\{name}");
    tags.match_indices(key.as_str())
        .map(|(i, _)| &tags[i + key.len()..])
        .filter(|v| name.ends_with('(') || v.starts_with(|c: char| c.is_ascii_digit()))
        .last()
        .map(|v| v.split(['\\', ')']).next().unwrap_or(""))
}

/// One piece of a part on one line: left, right, the line's top and its size, and the reading side.
struct Piece {
    x0: f32,
    x1: f32,
    top: f32,
    size: f32,
    rtl: bool,
}

/// A text event's start and end, and the pieces of a part on its lines.
type Placed = (f64, f64, Vec<Piece>);

/// A card text event: its times and the pieces of `part` on its lines. None when the event does not hold the part.
fn place(
    event: &str,
    part: &[String],
    font: &[u8],
    base_size: u32,
) -> Option<Result<Placed, String>> {
    let f: Vec<&str> = event.strip_prefix("Dialogue: ")?.splitn(10, ',').collect();
    let text = f.get(9)?;
    if text.contains("\\p1") {
        return None;
    }
    let (tags, body) = text.strip_prefix('{')?.split_once('}')?;
    // The font blocks of `cards::latin_runs` change no size or place: the text is measured as before.
    let body = &body.replace("{\\fnNoto Sans}", "").replace("{\\fn}", "");
    // An event with a second override block (a lower third) changes its size inside the text: not measured.
    if body.contains('{') {
        return None;
    }
    let num = |s: Option<&str>| s.and_then(|v| v.trim().parse::<f32>().ok());
    let size = num(tag(tags, "fs")).unwrap_or(base_size as f32);
    let an = num(tag(tags, "an"))? as u32;
    let mut xy = tag(tags, "pos(")?.split(',');
    let (x, y) = (num(xy.next())?, num(xy.next())?);
    let lines: Vec<String> = body
        .split("\\N")
        .map(|l| l.replace(['\u{200E}', '\u{200F}'], "").replace("\\h", " "))
        .collect();
    // The part's words run over the event's words; a line break may fall inside it.
    let all: Vec<(usize, usize, String)> = lines
        .iter()
        .enumerate()
        .flat_map(|(li, l)| {
            l.split_whitespace()
                .enumerate()
                .map(move |(wi, w)| (li, wi, crate::gates::norm_word(w)))
        })
        .filter(|w| !w.2.is_empty())
        .collect();
    let at = (0..all.len()).find(|&i| {
        all[i..]
            .iter()
            .map(|w| &w.2)
            .take(part.len())
            .eq(part.iter())
            && all.len() - i >= part.len()
    })?;
    let hit = &all[at..at + part.len()];
    let m = Measurer::new(font.to_vec(), size)?;
    let (col, row) = ((an.max(1) - 1) % 3, (an.max(1) - 1) / 3);
    let top = y - lines.len() as f32 * size * [1.0, 0.5, 0.0][row as usize];
    let mut pieces = vec![];
    for (li, l) in lines.iter().enumerate() {
        let on_line: Vec<usize> = hit.iter().filter(|w| w.0 == li).map(|w| w.1).collect();
        let (Some(&a), Some(&b)) = (on_line.first(), on_line.last()) else {
            continue;
        };
        let arabic = l.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c));
        if arabic && l.chars().any(|c| c.is_ascii_alphabetic()) {
            return Some(Err(format!(
                "the line \"{l}\" mixes Arabic and Latin letters: the place of a part in it is not measured (mark a line in one script)"
            )));
        }
        let w: Vec<&str> = l.split_whitespace().collect();
        let before = if a == 0 {
            0.0
        } else {
            m.width(&format!("{} ", w[..a].join(" ")))
        };
        let wide = m.width(&w[a..=b].join(" "));
        let full = m.width(l.trim());
        let left = x - full * [0.0, 0.5, 1.0][col as usize];
        let (x0, x1) = if arabic {
            (left + full - before - wide, left + full - before)
        } else {
            (left + before, left + before + wide)
        };
        pieces.push(Piece {
            x0,
            x1,
            top: top + li as f32 * size,
            size,
            rtl: arabic,
        });
    }
    Some(Ok((seconds(f[1]), seconds(f[2]), pieces)))
}

/// An oval as an ASS drawing: four Bezier quarters around (0, 0).
fn oval(rx: f32, ry: f32) -> String {
    let (kx, ky) = (rx * 0.5523, ry * 0.5523);
    format!(
        "m {rx:.0} 0 b {rx:.0} {ky:.0} {kx:.0} {ry:.0} 0 {ry:.0} b {:.0} {ry:.0} {:.0} {ky:.0} {:.0} 0 b {:.0} {:.0} {:.0} {:.0} 0 {:.0} b {kx:.0} {:.0} {rx:.0} {:.0} {rx:.0} 0",
        -kx, -rx, -rx, -rx, -ky, -kx, -ry, -ry, -ry, -ky
    )
}

/// The drawing events for the card's marks. `events`: what `cards::events` made for this card. `font`: the card
/// font's bytes; `base_size`: the `Card` style's size. `said`: the scene's words with their times.
pub fn events(
    card: &Card,
    events: &[String],
    font: &[u8],
    base_size: u32,
    said: &[crate::captions::Word],
) -> Result<Vec<String>, String> {
    let marks: &[Mark] = match &card.mark {
        Some(m) if KINDS.contains(&card.kind) => m,
        _ => return Ok(vec![]),
    };
    // Where the baseline is in a line, and the em, as shares of the font size (libass maps the size to the win
    // ascent plus descent).
    let (base, em) = {
        use read_fonts::{FontRef, TableProvider};
        let metrics = FontRef::new(font).ok().and_then(|f| {
            let os2 = f.os2().ok()?;
            let (a, d) = (os2.us_win_ascent() as f32, os2.us_win_descent() as f32);
            Some((a / (a + d), f.head().ok()?.units_per_em() as f32 / (a + d)))
        });
        metrics.unwrap_or((0.8, 0.73))
    };
    let accent = crate::captions::ACCENT;
    let mut out = vec![];
    for m in marks {
        let part = words(&m.part);
        // The text at rest is the last event that holds the part (a stat's count steps come before its number).
        let placed = events
            .iter()
            .rev()
            .find_map(|e| place(e, &part, font, base_size))
            .ok_or(format!(
                "the mark on \"{}\" found no text of the card to stand on",
                m.part
            ))??;
        let (start, end, pieces) = placed;
        let said_at = crate::cards::word_start(&m.on, said).ok_or(format!(
            "the voice's words do not have the `on` word \"{}\" of the mark on \"{}\"",
            m.on, m.part
        ))?;
        let t = said_at.max(start + 0.3);
        if t > end - 0.5 {
            return Err(format!(
                "the mark on \"{}\" has no time: its word \"{}\" is said at {said_at:.1} s and that text shows from {start:.1} to {end:.1} s (pick an earlier word, or move the card with `on`)",
                m.part, m.on
            ));
        }
        let style = m.style.unwrap_or(MarkStyle::Underline);
        if style == MarkStyle::Ring && pieces.len() > 1 {
            return Err(format!(
                "the ring on \"{}\" would break: this frame wraps the part over {} lines (mark fewer words, or use the style underline)",
                m.part,
                pieces.len()
            ));
        }
        let time = |t: f64| {
            let n = (t.max(0.0) * 100.0).round() as u64;
            format!(
                "{}:{:02}:{:02}.{:02}",
                n / 360000,
                n / 6000 % 60,
                n / 100 % 60,
                n % 100
            )
        };
        for p in &pieces {
            let (e, wide) = (p.size * em, p.x1 - p.x0);
            let baseline = p.top + p.size * base;
            // A box that grows from the reading side: its corner on that side stays in place.
            let grown = |layer: u32, top: f32, high: f32, pad: f32, alpha: &str| {
                // As the bars of a chart: drawn from the corner at `\pos`, to the left for Arabic.
                let (x, w) = if p.rtl {
                    (p.x1 + pad, -(wide + 2.0 * pad))
                } else {
                    (p.x0 - pad, wide + 2.0 * pad)
                };
                format!(
                    "Dialogue: {layer},{},{},Card,,0,0,0,,{{\\an7\\pos({x:.0},{top:.0})\\bord0\\shad0\\4a&HFF&\\1c{accent}&\\1a{alpha}\\fad(0,200)\\fscx0\\t(0,250,\\fscx100)\\p1}}m 0 0 l {w:.0} 0 l {w:.0} {high:.0} l 0 {high:.0}",
                    time(t),
                    time(end)
                )
            };
            out.push(match style {
                MarkStyle::Underline => grown(
                    3,
                    baseline + e * if p.rtl { 0.34 } else { 0.14 },
                    (e * 0.08).max(3.0),
                    0.0,
                    "&H00&",
                ),
                // Over the text, two thirds clear, as a marker pen: the letters stay light on the band.
                MarkStyle::Highlight => {
                    grown(3, baseline - e * 0.86, e * 1.16, e * 0.12, "&HA8&")
                }
                MarkStyle::Ring => format!(
                    "Dialogue: 3,{},{},Card,,0,0,0,,{{\\an7\\pos({:.0},{:.0})\\shad0\\4a&HFF&\\1a&HFF&\\bord{:.0}\\3c{accent}&\\3a&H00&\\fad(150,200)\\p1}}{}",
                    time(t),
                    time(end),
                    (p.x0 + p.x1) / 2.0,
                    baseline - e * 0.30,
                    (e * 0.07).max(3.0),
                    oval(wide * 0.61 + e * 0.25, e * 0.85)
                ),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::captions::Word;
    use crate::cards::Frame;

    fn card(json: serde_json::Value) -> Card {
        serde_json::from_value(json).unwrap()
    }

    fn said(words: &[(&str, f64)]) -> Vec<Word> {
        words
            .iter()
            .map(|(t, s)| Word {
                text: t.to_string(),
                start: *s,
                end: s + 0.3,
            })
            .collect()
    }

    /// The card's events and its marks on a 1920x1080 frame, from 0 to 6 s, with the real font and wrapping.
    fn made(c: &Card, arabic: bool, said: &[Word]) -> (Vec<String>, Result<Vec<String>, String>) {
        let font = crate::template::FONTS[usize::from(arabic)].2;
        let wrap = |t: &str, s: u32, max: f32| {
            let m = Measurer::new(font.to_vec(), s as f32).unwrap();
            crate::captions::wrap(t, max, |x| m.width(x))
        };
        let f = Frame {
            w: 1920,
            h: 1080,
            size: 84,
        };
        let ev = crate::cards::events(c, 0.0, 6.0, &f, &wrap, said);
        let marks = events(c, &ev, font, 84, said);
        (ev, marks)
    }

    /// The numbers of `\pos(x,y)` and of the drawing `m 0 0 l w 0 l w h` of a grown box.
    fn boxed(e: &str) -> (f32, f32, f32, f32) {
        let n = |s: &str| s.parse::<f32>().unwrap();
        let pos = e.split("\\pos(").nth(1).unwrap().split(')').next().unwrap();
        let (x, y) = pos.split_once(',').unwrap();
        let d: Vec<&str> = e.rsplit('}').next().unwrap().split(' ').collect();
        (n(x), n(y), n(d[4]), n(d[8]))
    }

    #[test]
    fn an_underline_stands_under_its_words_from_the_spoken_word() {
        let c = card(
            serde_json::json!({"kind": "title", "text": "The plan for next quarter",
            "mark": [{"part": "Next quarter", "on": "next"}]}),
        );
        assert_eq!(c.problem(), None);
        let words = said(&[("Here", 0.0), ("is", 0.4), ("next", 1.0), ("quarter", 1.4)]);
        let (ev, marks) = made(&c, false, &words);
        let marks = marks.unwrap();
        assert_eq!((ev.len(), marks.len()), (1, 1));
        // From the word (1.0 s) to the end of the title's own event (2.5 s), over the text (layer 3), in the accent.
        assert!(
            marks[0].starts_with("Dialogue: 3,0:00:01.00,0:00:02.50,Card,,0,0,0,,{\\an7\\pos("),
            "{}",
            marks[0]
        );
        assert!(
            marks[0].contains("\\1c&H0000E5FF&")
                && marks[0].contains("\\fscx0\\t(0,250,\\fscx100)\\p1}")
        );
        // A wide title is 189 px, centred at 960: the line starts where the words before the part end.
        assert!(ev[0].contains("\\fs189\\"), "{}", ev[0]);
        let m = Measurer::new(crate::template::FONTS[0].2.to_vec(), 189.0).unwrap();
        let left = 960.0 - m.width("The plan for next quarter") / 2.0;
        let (x, y, w, h) = boxed(&marks[0]);
        assert!((x - (left + m.width("The plan for "))).abs() <= 1.0, "{x}");
        assert!((w - m.width("next quarter")).abs() <= 1.0, "{w}");
        // Under the letters: below the middle of the line (h * 0.42) and above its bottom.
        let middle = 1080.0 * 0.42;
        assert!(y > middle + 30.0 && y + h < middle + 95.0, "{y} {h}");
        // No mark: no event, and the card's own events are the same.
        let plain = card(serde_json::json!({"kind": "title", "text": "The plan for next quarter"}));
        let (ev0, none) = made(&plain, false, &words);
        assert_eq!(ev0, ev);
        assert!(none.unwrap().is_empty());
        // `style` is not written when absent, so an old formula mark keeps its plan id.
        let old = serde_json::json!({"part": "mc^2", "on": "mass"});
        let m: Mark = serde_json::from_value(old.clone()).unwrap();
        assert_eq!(serde_json::to_value(&m).unwrap(), old);
    }

    #[test]
    fn arabic_is_measured_from_the_right_and_grows_to_the_left() {
        let c = card(
            serde_json::json!({"kind": "title", "text": "خطة العمل للربع القادم",
            "mark": [{"part": "للربع القادم", "on": "للربع", "style": "highlight"}]}),
        );
        assert_eq!(c.problem(), None);
        let words = said(&[("هذه", 0.0), ("خطة", 0.4), ("للربع", 1.0), ("القادم", 1.4)]);
        let marks = made(&c, true, &words).1.unwrap();
        assert_eq!(marks.len(), 1);
        // The title's size on a wide frame, after the Arabic font's own scale: read it from the card's event.
        let ev = made(&c, true, &words).0;
        let size: f32 = ev[0]
            .split("\\fs")
            .nth(1)
            .unwrap()
            .split('\\')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let m = Measurer::new(crate::template::FONTS[1].2.to_vec(), size).unwrap();
        let right = 960.0 + m.width("خطة العمل للربع القادم") / 2.0;
        let (x, _, w, _) = boxed(&marks[0]);
        // The part is the last two words, so it stands at the left; its right edge is after the first two words.
        let pad = w.abs() - m.width("للربع القادم");
        assert!(w < 0.0 && pad > 0.0 && pad < size * 0.3, "{w} {pad}");
        assert!(
            (x - pad / 2.0 - (right - m.width("خطة العمل "))).abs() <= 1.5,
            "{x}"
        );
        // The band is over the text and two thirds clear.
        assert!(marks[0].starts_with("Dialogue: 3,") && marks[0].contains("\\1a&HA8&"));
    }

    #[test]
    fn a_ring_goes_around_a_stat_number_once_the_count_has_landed() {
        let c = card(
            serde_json::json!({"kind": "stat", "text": "42%", "label": "revenue growth in year one",
            "mark": [{"part": "42%", "on": "forty", "style": "ring"}, {"part": "year one", "on": "year"}]}),
        );
        assert_eq!(c.problem(), None);
        let words = said(&[
            ("Revenue", 0.0),
            ("grew", 0.3),
            ("forty", 0.5),
            ("in", 2.0),
            ("year", 2.5),
        ]);
        let marks = made(&c, false, &words).1.unwrap();
        assert_eq!(marks.len(), 2);
        // "forty" is said at 0.5 s, during the count: the ring waits for the landed number (0.8 s) plus 0.3 s.
        assert!(
            marks[0].starts_with("Dialogue: 3,0:00:01.10,0:00:06.00,Card,,0,0,0,,{\\an7\\pos(960,"),
            "{}",
            marks[0]
        );
        // An outline only: a clear fill, an accent border, four curves.
        assert!(marks[0].contains("\\1a&HFF&") && marks[0].contains("\\3c&H0000E5FF&"));
        assert_eq!(marks[0].matches(" b ").count(), 4);
        assert!(
            marks[1].starts_with("Dialogue: 3,0:00:02.50,"),
            "{}",
            marks[1]
        );
    }

    #[test]
    fn a_mark_that_cannot_be_placed_is_refused() {
        let title = |mark: serde_json::Value| {
            card(
                serde_json::json!({"kind": "title", "text": "Step by step by hand", "mark": [mark]}),
            )
        };
        // Not there, there twice, half a word, no word.
        for (part, said) in [
            ("stairs", "0 times"),
            ("by", "2 times"),
            ("ste", "0 times"),
            ("...", "needs a word"),
        ] {
            let p = title(serde_json::json!({"part": part, "on": "step"}))
                .problem()
                .unwrap();
            assert!(p.contains(said) && p.contains(part), "{p}");
        }
        assert_eq!(
            title(serde_json::json!({"part": "by hand", "on": "step"})).problem(),
            None
        );
        assert!(title(serde_json::json!({"part": "hand", "on": "?"}))
            .problem()
            .unwrap()
            .contains("`on`"));
        // A kind that takes none, and a title behind the subject.
        let bars = card(
            serde_json::json!({"kind": "bars", "items": ["a", "b"], "values": [1, 2],
            "mark": [{"part": "a", "on": "x"}]}),
        );
        assert!(bars
            .problem()
            .unwrap()
            .contains("only a formula, title, stat, quote or list"));
        let mut behind = title(serde_json::json!({"part": "hand", "on": "step"}));
        behind.behind = Some(true);
        assert!(behind.problem().unwrap().contains("behind"));
        // A word of the script that is not there: the scene's check names it.
        let c = title(serde_json::json!({"part": "hand", "on": "never"}));
        assert!(c.on_problem("Step by step.").unwrap().contains("never"));
        // The word comes after the title has gone (2.5 s): the render stops and gives the times.
        let c = title(serde_json::json!({"part": "hand", "on": "late"}));
        let e = made(&c, false, &said(&[("Step", 0.0), ("late", 4.0)]))
            .1
            .unwrap_err();
        assert!(e.contains("4.0 s") && e.contains("0.0 to 2.5 s"), "{e}");
        // A ring over a line break: refused; an underline is drawn once per line.
        let long = "Simple is better than complex and flat is better than nested every single time";
        let quote = |style: &str| {
            card(serde_json::json!({"kind": "quote", "text": long,
                "mark": [{"part": "complex and flat is better than nested every", "on": "simple", "style": style}]}))
        };
        let words = said(&[("simple", 1.0)]);
        let e = made(&quote("ring"), false, &words).1.unwrap_err();
        assert!(e.contains("lines") && e.contains("underline"), "{e}");
        assert!(made(&quote("underline"), false, &words).1.unwrap().len() > 1);
        // A line in two scripts is not measured.
        let mixed = card(
            serde_json::json!({"kind": "title", "text": "خطة ScriptVox الجديدة",
            "mark": [{"part": "الجديدة", "on": "خطة"}]}),
        );
        let e = made(&mixed, true, &said(&[("خطة", 0.5)])).1.unwrap_err();
        assert!(e.contains("mixes Arabic and Latin"), "{e}");
    }
}
