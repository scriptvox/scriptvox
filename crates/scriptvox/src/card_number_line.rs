//! The `number_line` card: a range with ticks and 1 to 4 marked values; the first marker can travel in from
//! another value. See docs/specs/cards-data.md.

use crate::card_kit::{rect, shape, Area, PLAIN};
use crate::cards::{digits_like, esc, format_number, line, ring_part, Card};

pub const MAX_POINTS: usize = 4;
pub const MAX_NAME: usize = 16;
/// The travel: it starts this long after the card and takes this long.
pub const TRAVEL_AT: f64 = 0.9;
pub const TRAVEL_S: f64 = 1.2;
const STEPS: usize = 12;

pub fn problem(c: &Card) -> Option<String> {
    let ok = |v: f64| v.is_finite() && v.abs() <= 1e9;
    let (lo, hi) = match c.values.as_deref() {
        Some([lo, hi]) if ok(*lo) && ok(*hi) && lo < hi => (*lo, *hi),
        _ => {
            return Some(
                "a number_line card needs `values`: [from, to], the two ends of the line, the first below the second, for example [0, 100]"
                    .into(),
            )
        }
    };
    let inside = |v: f64| v.is_finite() && (lo..=hi).contains(&v);
    let points = c.points.as_deref().unwrap_or(&[]);
    if !(1..=MAX_POINTS).contains(&points.len()) {
        return Some(format!(
            "a number_line card needs `points`: 1 to {MAX_POINTS} values to mark on the line, for example [37]; this one has {}",
            points.len()
        ));
    }
    if let Some(p) = points.iter().find(|p| !inside(**p)) {
        return Some(format!(
            "number_line: the point {p} is not between {lo} and {hi}; widen `values` or move the point"
        ));
    }
    if let Some(items) = &c.items {
        if items.len() != points.len()
            || items
                .iter()
                .any(|i| i.trim().is_empty() || i.chars().count() > MAX_NAME)
        {
            return Some(format!(
                "number_line: `items` names the points, one name of at most {MAX_NAME} characters for each of the {} points; fix the list or remove `items`",
                points.len()
            ));
        }
    }
    if let Some(f) = c.from {
        if !inside(f) {
            return Some(format!(
                "number_line: `from` ({f}) is not between {lo} and {hi}; the marker travels from it to the first point, so put it on the line"
            ));
        }
        if f == points[0] {
            return Some(
                "number_line: `from` is the first point itself, so nothing travels; remove `from` or give another value"
                    .into(),
            );
        }
    }
    None
}

/// The step between ticks: 1, 2 or 5 times a power of ten, the smallest that gives at most 6 steps.
pub fn tick_step(lo: f64, hi: f64) -> f64 {
    let span = hi - lo;
    let mut pow = 10f64.powf((span / 6.0).log10().floor());
    loop {
        for m in [1.0, 2.0, 5.0] {
            if span / (m * pow) <= 6.0 + 1e-9 {
                return m * pow;
            }
        }
        pow *= 10.0;
    }
}

/// The ticks: the multiples of the step from `lo` to `hi`.
pub fn ticks(lo: f64, hi: f64) -> Vec<f64> {
    let step = tick_step(lo, hi);
    let first = (lo / step - 1e-9).ceil() as i64;
    let last = (hi / step + 1e-9).floor() as i64;
    (first..=last).map(|k| k as f64 * step).collect()
}

/// The decimal places a value needs, at most 2.
fn places(v: f64) -> usize {
    (0..2usize)
        .find(|d| {
            let x = v * 10f64.powi(*d as i32);
            (x - x.round()).abs() < 1e-6
        })
        .unwrap_or(2)
}

pub(crate) fn events(c: &Card, a: &Area) -> Vec<String> {
    let (Some([lo, hi]), Some(points)) = (c.values.as_deref(), c.points.as_deref()) else {
        return vec![];
    };
    let (lo, hi) = (*lo, *hi);
    let names = c.items.as_deref().unwrap_or(&[]);
    let marks = ticks(lo, hi);
    let decimals = points
        .iter()
        .chain(&marks)
        .chain(c.from.iter())
        .map(|v| places(*v))
        .max()
        .unwrap_or(0);
    let show = |v: f64| {
        // Under half a step of the last place reads as 0, not "-0".
        let v = if v.abs() < 0.5 * 10f64.powi(-(decimals as i32)) {
            0.0
        } else {
            v
        };
        digits_like(names, format_number(v, decimals, v.abs() >= 10000.0))
    };
    // Maths reads left to right in Arabic too (as the plot card): the line always runs from the left.
    let (x0, x1) = (a.w * 0.1, a.w * 0.9);
    let x = |v: f64| x0 + ((v - lo) / (hi - lo)) as f32 * (x1 - x0);
    let (edge_l, edge_r) = (a.w * 0.04, a.w * 0.96);
    let head = c.label.as_deref().filter(|l| !l.trim().is_empty());
    let head_size = head.map_or(0, |l| a.fit(l, a.size * 9 / 10, a.w * 0.88));
    let head_h = a.head_height(head_size);
    let room = a.bottom - a.top - head_h;

    // Sizes at scale 1: the value, the name over it, the numbers under the ticks.
    let mut k = 1.0f32;
    let layout = loop {
        let vs = (a.size as f32 * 0.9 * k).floor().max(1.0);
        let name_s = (a.size as f32 * 0.6 * k).floor().max(1.0);
        let ns = (a.size as f32 * 0.6 * k).floor().max(1.0);
        let r = vs * 0.24;
        // The box around a small text: the style's padding is for caption-size text, so each text sets its own.
        let (bord, nbord) = (vs * 0.18, ns * 0.18);
        let gap = vs * 0.12 + 2.0 * bord;
        let block = vs + if names.is_empty() { 0.0 } else { name_s };
        // A label box: centred on its marker, pushed inside the frame.
        let span = |i: usize, v: f64| {
            let wide = a
                .width(&show(v), vs as u32)
                .max(names.get(i).map_or(0.0, |n| a.width(n, name_s as u32)));
            let half = wide / 2.0 + bord + vs * 0.06;
            let cx = x(v).clamp(edge_l + half, (edge_r - half).max(edge_l + half));
            (cx - half, cx + half)
        };
        // Levels: a label goes one level up when it would touch a label of its level. The traveller holds the
        // whole way from its start to its end.
        let mut order: Vec<usize> = (0..points.len()).collect();
        order.sort_by(|p, q| points[*p].total_cmp(&points[*q]));
        let mut level = vec![0usize; points.len()];
        let mut taken: Vec<Vec<(f32, f32)>> = vec![];
        for &i in &order {
            let mut s = span(i, points[i]);
            if let Some(f) = c.from.filter(|_| i == 0) {
                let t = span(i, f);
                s = (s.0.min(t.0), s.1.max(t.1));
            }
            let l = (0..)
                .find(|l| {
                    taken
                        .get(*l)
                        .is_none_or(|row| row.iter().all(|o| s.1 <= o.0 || o.1 <= s.0))
                })
                .unwrap();
            if taken.len() <= l {
                taken.resize(l + 1, vec![]);
            }
            taken[l].push(s);
            level[i] = l;
        }
        let levels = taken.len() as f32;
        let above = r + levels * (block + gap);
        let below = r + nbord * 2.0 + ns * 1.1;
        // The numbers under the ticks: all, every second, or the two ends, whichever does not touch.
        let fits = |every: usize| {
            let shown: Vec<f64> = marks.iter().copied().step_by(every).collect();
            shown.windows(2).all(|p| {
                let need = (a.width(&show(p[0]), ns as u32) + a.width(&show(p[1]), ns as u32))
                    / 2.0
                    + nbord * 2.0
                    + ns * 0.15;
                x(p[1]) - x(p[0]) >= need
            })
        };
        let every = [1usize, 2, marks.len().max(2) - 1]
            .into_iter()
            .find(|e| fits(*e));
        let ends_in = marks
            .first()
            .is_none_or(|m| x(*m) - a.width(&show(*m), ns as u32) / 2.0 - nbord >= a.w * 0.02)
            && marks
                .last()
                .is_none_or(|m| x(*m) + a.width(&show(*m), ns as u32) / 2.0 + nbord <= a.w * 0.98);
        if (above + below <= room && every.is_some() && ends_in) || k < 0.3 {
            break (
                vs,
                name_s,
                ns,
                r,
                gap,
                block,
                level,
                above,
                below,
                every.unwrap_or(1),
                bord,
                nbord,
            );
        }
        k -= 0.04;
    };
    let (vs, name_s, ns, r, gap, block, level, above, below, every, bord, nbord) = layout;
    // The line stands so that the whole card is in the middle of its room, a little high.
    let free = (room - above - below).max(0.0);
    let ay = a.top + head_h + above + free * 0.4;
    let mut out = vec![];
    if let Some(l) = head {
        out.push(line(
            a.start,
            a.end,
            &format!(
                "\\an8\\pos({},{})\\fs{head_size}\\fad(200,200)",
                a.w / 2.0,
                a.top
            ),
            &esc(l),
        ));
    }
    // The line with a dark edge under it, and the ticks: one drawing each.
    let half = (vs * 0.05).max(2.0);
    let tick = |v: f64, h: f32, grow: f32| {
        rect(
            x(v) - half - grow,
            ay - h - grow,
            x(v) + half + grow,
            ay + h + grow,
        )
    };
    let axis = |grow: f32| {
        let mut d = rect(x0 - grow, ay - half - grow, x1 + grow, ay + half + grow);
        for m in &marks {
            d += " ";
            d += &tick(*m, r * 0.9, grow);
        }
        d
    };
    out.push(line(
        a.start,
        a.end,
        &shape("&H000000&", "&H50&", (200, 200)),
        &axis(half),
    ));
    out.push(line(
        a.start,
        a.end,
        &shape("&HFFFFFF&", "&H00&", (200, 200)),
        &axis(0.0),
    ));
    for m in marks.iter().step_by(every) {
        out.push(line(
            a.start + 0.1,
            a.end,
            &format!(
                "\\an8\\pos({:.0},{:.0})\\fs{ns:.0}\\bord{nbord:.1}\\b0\\fad(200,200)",
                x(*m),
                ay + r + nbord
            ),
            &format!("\u{200E}{}", esc(&show(*m))),
        ));
    }
    // The markers: a dot on the line, a stem up to its label, and the label: the name over the value, one text
    // with one box. All stems come first, then the dots, then the labels, so no stem lies over a label.
    let accent = crate::captions::ACCENT;
    let (mut stems, mut dots, mut labels) = (vec![], vec![], vec![]);
    for (i, p) in points.iter().enumerate() {
        let at = (a.start + 0.3 + i as f64 * 0.15).min(a.end - 0.4);
        let up = r + bord + vs * 0.1 + level[i] as f32 * (block + gap);
        // The label's middle at a value: on the marker, pushed inside the frame.
        let label_x = |v: f64| {
            let wide = a
                .width(&show(v), vs as u32)
                .max(names.get(i).map_or(0.0, |n| a.width(n, name_s as u32)));
            let half = wide / 2.0 + bord + vs * 0.06;
            x(v).clamp(edge_l + half, (edge_r - half).max(edge_l + half))
        };
        // The marker over a time, standing (`v0` equals `v1`) or moving. The label shows the value `v1`.
        let mut put = |from_t: f64, to_t: f64, v0: f64, v1: f64, fade: (u32, u32)| {
            let place = |fx: &dyn Fn(f64) -> f32, y: f32| {
                if v0 == v1 {
                    format!("\\pos({:.0},{y:.0})", fx(v0))
                } else {
                    format!("\\move({:.0},{y:.0},{:.0},{y:.0})", fx(v0), fx(v1))
                }
            };
            let draw = |c: &str, d: String| {
                line(
                    from_t,
                    to_t,
                    &format!(
                        "\\an7{}{PLAIN}\\1c{c}\\fad({},{})\\p1",
                        place(&x, ay),
                        fade.0,
                        fade.1
                    ),
                    &d,
                )
            };
            stems.push(draw(accent, rect(-half, -up, half, -r)));
            dots.push(draw(
                "&H000000&",
                ring_part(0.0, 0.0, r * 1.35, 0.0, 0.0, 1.0),
            ));
            dots.push(draw(accent, ring_part(0.0, 0.0, r, 0.0, 0.0, 1.0)));
            let name = names.get(i).map_or(String::new(), |n| {
                format!("{{\\fs{name_s:.0}\\b0}}{}\\N", a.text(n, name_s))
            });
            labels.push(line(
                from_t,
                to_t,
                &format!(
                    "\\an2{}\\fs{vs:.0}\\bord{bord:.1}\\4a&H20&\\fad({},{})",
                    place(&label_x, ay - up),
                    fade.0,
                    fade.1
                ),
                &format!(
                    "{name}{{\\fs{vs:.0}\\b1\\1c{accent}}}\u{200E}{}",
                    esc(&show(v1))
                ),
            ));
        };
        match c.from.filter(|_| i == 0) {
            Some(f) if a.end - a.start > TRAVEL_AT + 0.9 => {
                let t0 = a.start + TRAVEL_AT;
                let t1 = (t0 + TRAVEL_S).min(a.end - 0.5);
                put(at, t0, f, f, (200, 0));
                // ASS cannot change a text as it moves: the value counts in steps, each moving its own part.
                for s in 0..STEPS {
                    let (u0, u1) = (s as f64 / STEPS as f64, (s + 1) as f64 / STEPS as f64);
                    put(
                        t0 + u0 * (t1 - t0),
                        t0 + u1 * (t1 - t0),
                        f + (p - f) * u0,
                        f + (p - f) * u1,
                        (0, 0),
                    );
                }
                put(t1, a.end, *p, *p, (0, 200));
            }
            _ => put(at, a.end, *p, *p, (200, 200)),
        }
    }
    out.extend(stems);
    out.extend(dots);
    out.extend(labels);
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
    fn ticks_stand_at_round_steps() {
        assert_eq!(ticks(0.0, 100.0), [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
        assert_eq!(ticks(-1.0, 1.0), [-1.0, -0.5, 0.0, 0.5, 1.0]);
        assert_eq!(ticks(3.0, 47.0), [10.0, 20.0, 30.0, 40.0]);
        assert_eq!(tick_step(0.0, 1e9), 2e8);
        assert_eq!(
            (places(2.0), places(2.5), places(0.25), places(1.0 / 3.0)),
            (0, 1, 2, 2)
        );
    }

    #[test]
    fn a_number_line_is_checked() {
        let bad = |v: serde_json::Value, want: &str| {
            let p = card(v).problem().unwrap_or_default();
            assert!(p.contains(want), "{p}");
        };
        assert!(
            card(json!({"kind": "number_line", "values": [0, 100], "points": [37]}))
                .problem()
                .is_none()
        );
        assert!(card(json!({"kind": "number_line", "values": [0, 100], "points": [80, 20], "items": ["After", "Before"], "from": 20, "label": "Score"})).problem().is_none());
        bad(
            json!({"kind": "number_line", "points": [3]}),
            "needs `values`",
        );
        bad(
            json!({"kind": "number_line", "values": [5, 1], "points": [3]}),
            "needs `values`",
        );
        bad(
            json!({"kind": "number_line", "values": [0, 10]}),
            "needs `points`",
        );
        bad(
            json!({"kind": "number_line", "values": [0, 10], "points": [1, 2, 3, 4, 5]}),
            "1 to 4 values",
        );
        bad(
            json!({"kind": "number_line", "values": [0, 10], "points": [11]}),
            "not between 0 and 10",
        );
        bad(
            json!({"kind": "number_line", "values": [0, 10], "points": [1], "items": ["a", "b"]}),
            "one name",
        );
        bad(
            json!({"kind": "number_line", "values": [0, 10], "points": [1], "from": 12}),
            "`from` (12) is not between",
        );
        bad(
            json!({"kind": "number_line", "values": [0, 10], "points": [1], "from": 1}),
            "nothing travels",
        );
        bad(
            json!({"kind": "number_line", "values": [0, 10], "points": [1], "text": "x"}),
            "does not draw `text`",
        );
        bad(
            json!({"kind": "number_line", "values": [0, 10], "points": [1], "mark": [{"part": "1", "on": "x"}]}),
            "does not draw `mark`",
        );
        bad(
            json!({"kind": "stat", "text": "4", "points": [1]}),
            "only a number_line card takes `points`",
        );
        // `from` stays a stat field too, and no other kind takes it.
        assert!(card(json!({"kind": "stat", "text": "42%", "from": 30}))
            .problem()
            .is_none());
        bad(
            json!({"kind": "title", "text": "Hi", "from": 3}),
            "takes `from`",
        );
    }

    fn pos(e: &str) -> Vec<f32> {
        let tag = if e.contains("\\move(") {
            "\\move("
        } else {
            "\\pos("
        };
        let inner = e.split(tag).nth(1).unwrap().split(')').next().unwrap();
        inner.split(',').map(|v| v.parse().unwrap()).collect()
    }

    #[test]
    fn markers_stand_at_their_values_and_close_labels_take_two_levels() {
        let c = card(
            json!({"kind": "number_line", "values": [0, 100], "points": [50, 52, 90], "items": ["Median", "Mean", "Best"]}),
        );
        let ev = events(&c, "en", (1080, 1920), &[]);
        // The dots (accent discs): at 50, 52 and 90 percent of the line from 10 to 90 percent of the width.
        let dots: Vec<f32> = ev
            .iter()
            .filter(|e| {
                e.contains("\\p1") && e.contains(crate::captions::ACCENT) && e.contains(" l ")
            })
            .filter(|e| e.matches(' ').count() > 40)
            .map(|e| pos(e)[0])
            .collect();
        assert_eq!(dots.len(), 3, "{ev:#?}");
        for (d, v) in dots.iter().zip([50.0, 52.0, 90.0]) {
            assert!((d - (108.0 + 864.0 * v / 100.0)).abs() < 1.0, "{dots:?}");
        }
        // 50 and 52 are too close for one level: their values stand at two heights, and no two labels touch.
        let y = |t: &str| pos(ev.iter().find(|e| e.ends_with(t)).unwrap())[1];
        assert!(y("52") < y("50") - 40.0, "{} {}", y("52"), y("50"));
        assert_eq!(y("90"), y("50"));
        // A label is one text: the name over the value.
        assert!(
            ev.iter()
                .any(|e| e.contains("Median\\N") && e.ends_with("50")),
            "{ev:#?}"
        );
        let texts: Vec<_> = boxes(&ev, "en")
            .into_iter()
            .filter(|b| b.4.contains("\\N"))
            .collect();
        assert_eq!(texts.len(), 3);
        for (i, p) in texts.iter().enumerate() {
            for q in &texts[i + 1..] {
                assert!(
                    p.2 <= q.0 || q.2 <= p.0 || p.3 <= q.1 + 1.0 || q.3 <= p.1 + 1.0,
                    "{p:?} over {q:?}"
                );
            }
        }
    }

    #[test]
    fn the_first_marker_travels_from_its_start_value_and_its_number_counts() {
        let c =
            card(json!({"kind": "number_line", "values": [0, 100], "points": [80], "from": 20}));
        let ev = events(&c, "en", (1920, 1080), &[]);
        let (xa, xb) = (192.0 + 1536.0 * 0.2, 192.0 + 1536.0 * 0.8);
        // The dot rests at 20, moves from 0.9 s to 2.1 s, then rests at 80.
        // Each of the 12 steps moves the stem and the two discs its own twelfth of the way.
        let moving: Vec<&String> = ev
            .iter()
            .filter(|e| e.contains("\\move(") && e.contains("\\p1"))
            .collect();
        assert_eq!(moving.len(), 36, "{ev:#?}");
        let (first, last) = (pos(moving[0]), pos(moving[35]));
        assert!(
            (first[0] - xa).abs() < 1.0 && (last[2] - xb).abs() < 1.0,
            "{moving:#?}"
        );
        assert!(
            moving[0].contains(",0:00:00.90,0:00:01.00,")
                && moving[35].contains(",0:00:02.00,0:00:02.10,")
        );
        // The value: 12 steps that end on 25, 30, ... 80, each moving its own twelfth of the way.
        let steps: Vec<&String> = ev
            .iter()
            .filter(|e| e.contains("\\move(") && !e.contains("\\p1"))
            .collect();
        assert_eq!(steps.len(), 12);
        assert!(
            steps[0].ends_with("25") && steps[11].ends_with("80"),
            "{steps:#?}"
        );
        let p = pos(steps[5]);
        assert!(
            (p[0] - (xa + (xb - xa) * 5.0 / 12.0)).abs() < 1.5
                && (p[2] - (xa + (xb - xa) * 0.5)).abs() < 1.5
        );
        // Before and after it stands still with 20 and with 80.
        assert!(ev
            .iter()
            .any(|e| e.contains(",0:00:00.30,0:00:00.90,") && e.ends_with("20")));
        assert!(ev
            .iter()
            .any(|e| e.contains(",0:00:02.10,0:00:06.00,") && e.ends_with("80")));
    }

    #[test]
    fn every_number_line_stays_in_its_room() {
        let worst = |ar: bool| {
            let names = if ar {
                json!([
                    "القيمة المتوسطة",
                    "الحد الأدنى هنا",
                    "الهدف المطلوب",
                    "أفضل نتيجة لنا"
                ])
            } else {
                json!([
                    "Median of values",
                    "The lowest one",
                    "Target for 2030",
                    "Best result yet"
                ])
            };
            card(
                json!({"kind": "number_line", "values": [0, 1000000], "points": [990000, 5000, 500000, 510000],
                "items": names, "from": 10000, "label": if ar { "عدد الزوار في السنة الواحدة" } else { "Visitors in one year of work" }}),
            )
        };
        for (lang, c) in [("en", worst(false)), ("ar", worst(true))] {
            assert!(c.problem().is_none(), "{:?}", c.problem());
            for frame in FRAMES {
                let ev = events(&c, lang, frame, &[]);
                assert_inside(&ev, lang, frame, "number_line");
            }
        }
        // A range with a negative end and decimals.
        let c = card(json!({"kind": "number_line", "values": [-1, 1], "points": [-0.25, 0.75]}));
        for frame in FRAMES {
            let ev = events(&c, "en", frame, &[]);
            assert_inside(&ev, "en", frame, "number_line");
            assert!(
                ev.iter().any(|e| e.ends_with("-0.25")) && ev.iter().any(|e| e.ends_with("0.50"))
            );
        }
    }
}
