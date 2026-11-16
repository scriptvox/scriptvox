//! The `gauge` and `progress` cards: one value on a ring or on a bar that fills to it, with the number counting up.
//! See docs/specs/cards-data.md.

use crate::card_kit::{rect, shape, Area, PANEL, PLAIN};
use crate::cards::{digits_like, esc, line, ring_part, Card, Kind};

pub const MAX_TEXT: usize = 12;
/// The fill and the count take this long, in this many steps.
pub const FILL_S: f64 = 0.9;
const STEPS: usize = 12;

/// The value as a share of its top, 0 to 1.
pub fn share(values: &[f64]) -> Option<f64> {
    let (v, max) = match values {
        [v] => (*v, 100.0),
        [v, max] => (*v, *max),
        _ => return None,
    };
    (v.is_finite() && max.is_finite() && max > 0.0 && (0.0..=max).contains(&v)).then(|| v / max)
}

pub fn problem(c: &Card) -> Option<String> {
    let kind = crate::card_kit::name(c.kind);
    if c.values.as_deref().and_then(share).is_none() {
        return Some(format!(
            "a {kind} card needs `values`: [value] out of 100, or [value, top], with the value from 0 to the top, for example [72] or [3, 5]"
        ));
    }
    if c.text
        .as_deref()
        .is_some_and(|t| t.trim().is_empty() || t.chars().count() > MAX_TEXT)
    {
        return Some(format!(
            "{kind}: `text` is shown in place of the percent and has 1 to {MAX_TEXT} characters, for example \"3 of 5\"; shorten it or remove it"
        ));
    }
    None
}

pub(crate) fn events(c: &Card, a: &Area) -> Vec<String> {
    let Some(full) = c.values.as_deref().and_then(share) else {
        return vec![];
    };
    let label = c.label.as_deref().filter(|l| !l.trim().is_empty());
    let like = [label.unwrap_or("").to_string()];
    let percent = |s: f64| digits_like(&like, format!("{}%", (s * 100.0).round()));
    let room = a.bottom - a.top;
    let accent = crate::captions::ACCENT;
    let fill_end = (a.start + 0.2 + FILL_S).min(a.end - 0.2);
    let t0 = (a.start + 0.2).min(fill_end);
    // The share shown in step k of the fill: fast first, slow as it lands.
    let eased = |k: usize| full * (1.0 - (1.0 - k as f64 / STEPS as f64).powi(3));
    let slot = |k: usize| t0 + (fill_end - t0) * k as f64 / STEPS as f64;
    let mut out = vec![];
    // The number: the percent counting up, or the brief's own text, which does not count.
    let mut number = |tags: &dyn Fn(&str) -> String| match c.text.as_deref() {
        Some(t) => out.push(line(a.start, a.end, &tags("\\fad(200,200)"), &esc(t))),
        None => {
            out.push(line(a.start, t0, &tags("\\fad(200,0)"), &percent(0.0)));
            for k in 0..STEPS {
                out.push(line(
                    slot(k),
                    slot(k + 1),
                    &tags(""),
                    &percent(eased(k + 1)),
                ));
            }
            out.push(line(fill_end, a.end, &tags("\\fad(0,200)"), &percent(full)));
        }
    };
    // A number in Latin digits is set in the Latin font, also in an Arabic video: there the digits come from the
    // Arabic font and the "%" from the Latin one, at two sizes (seen by eye, 13 Nov 2026).
    let shown = c.text.clone().unwrap_or_else(|| percent(1.0));
    let latin_font = if shown.is_ascii() {
        "\\fnNoto Sans"
    } else {
        ""
    };
    let label_size = label.map_or(0, |l| a.fit(l, a.size * 9 / 10, a.w * 0.86));
    // A label's line and its box padding, as a heading's.
    let label_h = a.head_height(label_size);
    if c.kind == Kind::Gauge {
        let r = ((room - label_h) * 0.46).min(a.w * if a.wide { 0.16 } else { 0.3 });
        let (cx, cy) = (
            a.w / 2.0,
            a.top + r + ((room - label_h - 2.0 * r) / 2.0).min(a.h * 0.04),
        );
        let inner = r * 0.74;
        // The widest number the ring's middle has to hold.
        let widest = c.text.clone().unwrap_or_else(|| percent(1.0));
        let ns = if latin_font.is_empty() {
            a.fit(&widest, (r * 0.62 / a.latin.max(0.5)) as u32, inner * 1.6)
        } else {
            // 0.62 em a character is wider than any digit or "%" of the Latin font.
            ((r * 0.62) as u32)
                .min((inner * 1.6 / widest.chars().count().max(1) as f32 / 0.62) as u32)
        };
        number(&|fade: &str| {
            format!("\\an5\\pos({cx:.0},{cy:.0}){latin_font}\\fs{ns}{PLAIN}{fade}")
        });
        let texts = std::mem::take(&mut out);
        // A dark dial, the track, then the arc that grows clockwise from the top.
        out.push(line(
            a.start,
            a.end,
            &shape(PANEL, "&H18&", (200, 200)),
            &ring_part(cx, cy, r * 1.06, 0.0, 0.0, 1.0),
        ));
        out.push(line(
            a.start,
            a.end,
            &shape("&H606060&", "&H00&", (200, 200)),
            &ring_part(cx, cy, r, inner, 0.0, 1.0),
        ));
        if full > 0.0 {
            for k in 1..STEPS {
                out.push(line(
                    slot(k),
                    slot(k + 1),
                    &shape(accent, "&H00&", (0, 0)),
                    &ring_part(cx, cy, r, inner, 0.0, eased(k)),
                ));
            }
            out.push(line(
                fill_end,
                a.end,
                &shape(accent, "&H00&", (0, 200)),
                &ring_part(cx, cy, r, inner, 0.0, full),
            ));
        }
        out.extend(texts);
        if let Some(l) = label {
            out.push(line(
                a.start + 0.2,
                a.end,
                &format!(
                    "\\an8\\pos({cx:.0},{:.0})\\fs{label_size}\\fad(200,200)",
                    cy + r * 1.06 + a.size as f32 * 0.4
                ),
                &esc(l),
            ));
        }
    } else {
        // The number, the bar under it, the label under the bar.
        // In the Latin font the size is the Latin one: the card's size is raised for a font with a taller line.
        let ns = if latin_font.is_empty() {
            a.size * 8 / 5
        } else {
            (a.size as f32 * 1.6 * a.latin) as u32
        };
        let num_h = ns as f32
            * if latin_font.is_empty() {
                (1.2 * a.latin).max(1.0)
            } else {
                1.2
            }
            + a.size as f32 * 0.4;
        let (bw, bh) = (a.w * 0.8, a.size as f32 * 0.5);
        let gap = a.size as f32 * 0.35;
        let total = num_h + bh + gap + label_h;
        let top = a.top + ((room - total) / 2.0).min(a.h * 0.12).max(0.0);
        let (x0, by) = ((a.w - bw) / 2.0, top + num_h);
        number(&|fade: &str| {
            format!(
                "\\an2\\pos({:.0},{:.0}){latin_font}\\fs{ns}\\1c{accent}{fade}",
                a.w / 2.0,
                by - a.size as f32 * 0.35
            )
        });
        out.push(line(
            a.start,
            a.end,
            &shape("&H000000&", "&H40&", (200, 200)),
            &rect(x0 - 3.0, by - 3.0, x0 + bw + 3.0, by + bh + 3.0),
        ));
        out.push(line(
            a.start,
            a.end,
            &shape("&H606060&", "&H00&", (200, 200)),
            &rect(x0, by, x0 + bw, by + bh),
        ));
        if full > 0.0 {
            // The fill grows from the reading side: from the left, in Arabic from the right. A drawing scales
            // from its own origin, so the Arabic one is drawn to the left of it.
            let wide = bw * full as f32;
            let (ox, d) = if a.rtl {
                (x0 + bw, rect(-wide, 0.0, 0.0, bh))
            } else {
                (x0, rect(0.0, 0.0, wide, bh))
            };
            let ms = |t: f64| ((t - a.start) * 1000.0).round() as i64;
            out.push(line(
                a.start,
                a.end,
                &format!(
                    "\\an7\\pos({ox:.0},{by:.0}){PLAIN}\\1c{accent}\\fscx0\\t({},{},0.4,\\fscx100)\\fad(0,200)\\p1",
                    ms(t0),
                    ms(fill_end)
                ),
                &d,
            ));
        }
        if let Some(l) = label {
            out.push(line(
                a.start + 0.2,
                a.end,
                &format!(
                    "\\an8\\pos({:.0},{:.0})\\fs{label_size}\\fad(200,200)",
                    a.w / 2.0,
                    by + bh + gap + a.size as f32 * 0.25
                ),
                &esc(l),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card_kit::testkit::{assert_inside, boxes, events, FRAMES};
    use serde_json::json;

    fn card(v: serde_json::Value) -> Card {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn a_gauge_and_a_progress_card_are_checked() {
        let p = |v: serde_json::Value| card(v).problem().unwrap_or_default();
        assert_eq!(
            p(json!({"kind": "gauge", "values": [72], "label": "Battery"})),
            ""
        );
        assert_eq!(
            p(json!({"kind": "progress", "values": [3, 5], "text": "3 of 5"})),
            ""
        );
        assert!(p(json!({"kind": "gauge"})).contains("a gauge card needs `values`"));
        assert!(p(json!({"kind": "progress", "values": [120]}))
            .contains("a progress card needs `values`"));
        assert!(p(json!({"kind": "gauge", "values": [-1]})).contains("needs `values`"));
        assert!(p(json!({"kind": "gauge", "values": [1, 0]})).contains("needs `values`"));
        assert!(p(json!({"kind": "gauge", "values": [1, 2, 3]})).contains("needs `values`"));
        assert!(
            p(json!({"kind": "gauge", "values": [50], "text": "far too long a text"}))
                .contains("1 to 12 characters")
        );
        assert!(p(json!({"kind": "gauge", "values": [50], "items": ["a"]}))
            .contains("a gauge card does not draw `items`: remove it"));
        assert!(p(json!({"kind": "progress", "values": [50], "from": 10})).contains("takes `from`"));
        assert_eq!(
            (share(&[72.0]), share(&[3.0, 5.0]), share(&[0.0])),
            (Some(0.72), Some(0.6), Some(0.0))
        );
    }

    #[test]
    fn the_ring_fills_to_its_share_and_the_number_counts_with_it() {
        let c = card(json!({"kind": "gauge", "values": [72], "label": "Battery"}));
        let ev = events(&c, "en", (1080, 1920), &[]);
        // The number: 0%, twelve steps, then 72% to the end.
        assert!(ev.iter().any(|e| e.ends_with("}0%")));
        let last = ev
            .iter()
            .find(|e| e.contains(",0:00:01.10,0:00:06.00,") && e.ends_with("72%"))
            .unwrap();
        assert!(last.contains("\\an5"), "{last}");
        let steps: Vec<&String> = ev
            .iter()
            .filter(|e| e.contains("\\an5") && e.ends_with('%'))
            .collect();
        assert_eq!(steps.len(), 14);
        // The arc at rest ends at 72 percent of the turn: its outer point there is on the ring.
        let arc = ev
            .iter()
            .find(|e| {
                e.contains("\\p1")
                    && e.contains(crate::captions::ACCENT)
                    && e.contains(",0:00:01.10,")
            })
            .unwrap();
        let b = boxes(std::slice::from_ref(arc), "en");
        let ring = boxes(std::slice::from_ref(&ev[1]), "en");
        let (cx, r) = ((ring[0].0 + ring[0].2) / 2.0, (ring[0].2 - ring[0].0) / 2.0);
        // 72 percent clockwise from the top: the arc covers the whole right half and reaches left of the middle.
        assert!(
            (b[0].2 - (cx + r)).abs() < 2.0 && b[0].0 < cx - r * 0.9,
            "{b:?} {cx} {r}"
        );
        assert!((b[0].1 - ring[0].1).abs() < 2.0 && (b[0].3 - ring[0].3).abs() < 2.0);
        // A value of 0 draws no arc, and the brief's own text does not count.
        let c = card(json!({"kind": "gauge", "values": [0, 5], "text": "0 of 5"}));
        let ev = events(&c, "en", (1080, 1920), &[]);
        assert!(!ev
            .iter()
            .any(|e| e.contains("\\p1") && e.contains(crate::captions::ACCENT)));
        assert_eq!(ev.iter().filter(|e| e.ends_with("0 of 5")).count(), 1);
    }

    #[test]
    fn the_bar_fills_from_the_reading_side() {
        let c = card(json!({"kind": "progress", "values": [3, 5], "label": "Steps done"}));
        let ev = events(&c, "en", (1920, 1080), &[]);
        let fill = ev.iter().find(|e| e.contains("\\fscx0")).unwrap();
        // 60 percent of a bar of 80 percent of the width, from its left end, growing from 0.2 s to 1.1 s.
        assert!(
            fill.contains("\\pos(192,") && fill.contains("\\t(200,1100,0.4,\\fscx100)"),
            "{fill}"
        );
        assert!(fill.ends_with("m 0 0 l 922 0 l 922 50 l 0 50"), "{fill}");
        assert!(ev.iter().any(|e| e.ends_with("60%")));
        let c = card(json!({"kind": "progress", "values": [3, 5], "label": "الخطوات المنجزة"}));
        let ev = events(&c, "ar", (1920, 1080), &[]);
        let fill = ev.iter().find(|e| e.contains("\\fscx0")).unwrap();
        assert!(
            fill.contains("\\pos(1728,") && fill.contains("}m -922 0 l 0 0"),
            "{fill}"
        );
    }

    #[test]
    fn every_gauge_and_progress_card_stays_in_its_room() {
        for kind in ["gauge", "progress"] {
            for (lang, label, text) in [
                (
                    "en",
                    "Share of the yearly budget that is spent",
                    "1,250 of 2k",
                ),
                ("ar", "حصة الميزانية السنوية التي صُرفت", "١٢٥٠ من ٢٠٠٠"),
            ] {
                for with_text in [false, true] {
                    let mut v = json!({"kind": kind, "values": [100], "label": label});
                    if with_text {
                        v["text"] = json!(text);
                    }
                    let c = card(v);
                    assert!(c.problem().is_none(), "{:?}", c.problem());
                    for frame in FRAMES {
                        let ev = events(&c, lang, frame, &[]);
                        assert_inside(&ev, lang, frame, kind);
                    }
                }
            }
        }
    }
}
