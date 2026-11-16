//! Shared parts of the data cards `table`, `number_line`, `diagram`, `steps`, `progress` and `gauge`
//! (docs/specs/cards-data.md): the place a card may draw in, a text width measured through the caller's wrapper,
//! the check that refuses a field a kind does not draw, and the one place that sends a kind to its module.

use crate::cards::{Card, Kind};

/// Where a card may draw, and what it draws with. `top` to `bottom` is the room above the captions.
pub(crate) struct Area<'a> {
    pub w: f32,
    pub h: f32,
    /// The card's base text size on this frame.
    pub size: u32,
    pub top: f32,
    pub bottom: f32,
    pub rtl: bool,
    pub wide: bool,
    /// The share of a text size that a Latin font of the same look would have: under 1 for a card font with a
    /// taller line (Arabic, about 0.7), whatever the card's own words are. Row heights are counted with it, so an
    /// Arabic row is not half empty.
    pub latin: f32,
    pub start: f64,
    pub end: f64,
    pub wrap: &'a dyn Fn(&str, u32, f32) -> Vec<String>,
    pub said: &'a [crate::captions::Word],
}

/// Text with no box of its own: it stands on a panel the card draws.
pub(crate) const PLAIN: &str = "\\bord0\\shad0\\4a&HFF&";
/// The panel colours of the `compare` card: dark, and the lit one.
pub(crate) const PANEL: &str = "&H1E1A17&";
pub(crate) const PANEL_LIT: &str = "&H402818&";
/// Text that is there but not the point now. Solid, so it stays readable on the panel.
pub(crate) const DIM: &str = "&HB4B4B4&";

impl Area<'_> {
    /// The width of `t` on one line at size `s`, in pixels. `events` gets a wrapper and no ruler, so the width is
    /// found with it: the smallest width at which the text stays on one line. For one word the result is too large
    /// by half a space, which is on the safe side. A wrapper that never breaks (a unit test) gives 0.6 em a character.
    pub fn width(&self, t: &str, s: u32) -> f32 {
        let t = t.trim();
        if t.is_empty() {
            return 0.0;
        }
        // Measured once at a large size: a width is linear in the size. A text of several words is on one line
        // just when it fits; one word never breaks, so it is asked twice with a space.
        const AT: u32 = 200;
        let several = t.split_whitespace().count() > 1;
        let probe = if several {
            t.to_string()
        } else {
            format!("{t} {t}")
        };
        let n = probe.chars().count() as f32;
        let (mut lo, mut hi) = (0.0f32, n * AT as f32 * 1.5 + AT as f32);
        if (self.wrap)(&probe, AT, lo).len() <= 1 {
            return t.chars().count() as f32 * 0.6 * s as f32;
        }
        while hi - lo > 2.0 {
            let mid = (lo + hi) / 2.0;
            if (self.wrap)(&probe, AT, mid).len() <= 1 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi / if several { 1.0 } else { 2.0 } * s as f32 / AT as f32
    }

    /// The room a heading of size `hs` takes at the top: its line, the padding of its box, and a little air. The
    /// box padding is a quarter of the caption size whatever the text's own size.
    pub fn head_height(&self, hs: u32) -> f32 {
        if hs == 0 {
            return 0.0;
        }
        hs as f32 * (1.3 * self.latin).max(1.0) + self.size as f32 * 0.45
    }

    /// The text of an event at size `s`. In a video whose card font has a taller line (Arabic), a text with no
    /// letter of that script (`Python`, `Q1`, `50x`) is set in the Latin font at the Latin size: else its letters
    /// come from the fallback font and its digits from the card font, at two sizes (seen by eye, 13 Nov 2026).
    pub fn text(&self, t: &str, s: f32) -> String {
        if self.latin < 0.99 && t.is_ascii() && t.chars().any(|c| c.is_ascii_alphanumeric()) {
            format!(
                "{{\\fnNoto Sans\\fs{:.0}}}{}",
                s * self.latin,
                crate::cards::esc(t)
            )
        } else {
            crate::cards::esc(t)
        }
    }

    /// The largest size up to `base` at which `t` is not wider than `max`.
    pub fn fit(&self, t: &str, base: u32, max: f32) -> u32 {
        let at = self.width(t, 100).max(1.0);
        base.min((max / at * 100.0) as u32).max(1)
    }
}

/// A filled box as an ASS drawing.
pub(crate) fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> String {
    format!("m {x0:.0} {y0:.0} l {x1:.0} {y0:.0} l {x1:.0} {y1:.0} l {x0:.0} {y1:.0}")
}

/// The tags of a drawing in frame pixels: colour, alpha (`&H00&` solid), fade in and out in ms.
pub(crate) fn shape(colour: &str, alpha: &str, fade: (u32, u32)) -> String {
    format!(
        "\\an7\\pos(0,0){PLAIN}\\1c{colour}\\1a{alpha}\\fad({},{})\\p1",
        fade.0, fade.1
    )
}

/// The name of a kind as the brief writes it.
pub(crate) fn name(kind: Kind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn data_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Table | Kind::NumberLine | Kind::Diagram | Kind::Steps | Kind::Progress | Kind::Gauge
    )
}

/// The fields a card has set, by their brief names. `on`, `at` and `hold` time every card and are not in the list.
/// A field that holds nothing (`false`, a blank text) is not set: there is nothing that a card could fail to draw.
fn set_fields(c: &Card) -> Vec<&'static str> {
    let text = |t: &Option<String>| t.as_deref().is_some_and(|t| !t.trim().is_empty());
    let yes = |b: Option<bool>| b == Some(true);
    [
        ("text", text(&c.text)),
        ("label", text(&c.label)),
        ("items", c.items.is_some()),
        ("values", c.values.is_some()),
        ("behind", yes(c.behind)),
        ("then", c.then.is_some()),
        ("ease", c.ease.is_some()),
        ("stagger", c.stagger.is_some()),
        ("from", c.from.is_some()),
        ("mark", c.mark.is_some()),
        ("y", c.y.is_some()),
        ("grid", yes(c.grid)),
        ("trace", yes(c.trace)),
        ("rows", c.rows.is_some()),
        ("points", c.points.is_some()),
        ("nodes", c.nodes.is_some()),
        ("edges", c.edges.is_some()),
    ]
    .into_iter()
    .filter_map(|(n, set)| set.then_some(n))
    .collect()
}

/// The fields each kind draws: the one table, read from what each renderer uses. A field that is not in a kind's
/// row was dropped without a word before (a `label` on a title card, seen 13 Nov 2026).
pub(crate) fn allowed(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Title => &["text", "behind", "mark"],
        Kind::Stat => &["text", "label", "from", "mark"],
        Kind::LowerThird => &["text", "label"],
        Kind::List => &["items", "ease", "stagger", "mark"],
        Kind::Bars => &["items", "values", "label", "ease", "stagger"],
        Kind::Line => &["items", "values", "label"],
        Kind::Formula => &["text", "label", "then", "mark"],
        Kind::Code | Kind::Diff | Kind::Terminal => &["text", "label"],
        Kind::Plot => &["text", "values", "label", "y", "grid", "trace"],
        Kind::Quote => &["text", "label", "mark"],
        Kind::Compare => &["items", "values", "label"],
        Kind::Donut => &["items", "values", "text"],
        Kind::Timeline => &["items"],
        Kind::Table => &["rows", "label", "mark"],
        Kind::NumberLine => &["values", "points", "items", "from", "label"],
        Kind::Diagram => &["nodes", "edges", "label"],
        Kind::Steps => &["items"],
        Kind::Progress | Kind::Gauge => &["values", "text", "label"],
    }
}

/// A field the card has set and its kind does not draw, with what to do. For every kind.
pub(crate) fn undrawn(c: &Card) -> Option<String> {
    let allowed = allowed(c.kind);
    let bad = set_fields(c).into_iter().find(|f| !allowed.contains(f))?;
    Some(format!(
        "a {} card does not draw `{bad}`: remove it (it takes {}, and `on` and `hold` for its time)",
        name(c.kind),
        allowed
            .iter()
            .map(|a| format!("`{a}`"))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// A field of the data cards on a kind that does not draw it. For every card, before its own check.
pub(crate) fn foreign(c: &Card) -> Option<String> {
    for (field, owner, set) in [
        ("rows", Kind::Table, c.rows.is_some()),
        ("points", Kind::NumberLine, c.points.is_some()),
        ("nodes", Kind::Diagram, c.nodes.is_some()),
        ("edges", Kind::Diagram, c.edges.is_some()),
    ] {
        if set && c.kind != owner {
            return Some(format!(
                "only a {} card takes `{field}`: remove it, or set kind to {}",
                name(owner),
                name(owner)
            ));
        }
    }
    None
}

/// A problem with a data card, for the brief check: first a field the kind does not draw, then the kind's own
/// rules. Every message ends with what to do.
pub(crate) fn problem(c: &Card) -> Option<String> {
    if !data_kind(c.kind) {
        return None;
    }
    if let Some(p) = undrawn(c) {
        return Some(p);
    }
    if c.label.as_deref().is_some_and(|l| l.chars().count() > 40) {
        return Some(format!(
            "a {} card label has at most 40 characters; shorten it",
            name(c.kind)
        ));
    }
    match c.kind {
        Kind::Table => crate::card_table::problem(c),
        Kind::NumberLine => crate::card_number_line::problem(c),
        Kind::Diagram => crate::card_diagram::problem(c),
        Kind::Steps => crate::card_steps::problem(c),
        _ => crate::card_gauge::problem(c),
    }
}

/// True for the kinds this file sends on.
pub(crate) fn owns(kind: Kind) -> bool {
    data_kind(kind)
}

/// The events of a data card.
pub(crate) fn events(c: &Card, a: &Area) -> Vec<String> {
    match c.kind {
        Kind::Table => crate::card_table::events(c, a),
        Kind::NumberLine => crate::card_number_line::events(c, a),
        Kind::Diagram => crate::card_diagram::events(c, a),
        Kind::Steps => crate::card_steps::events(c, a),
        Kind::Progress | Kind::Gauge => crate::card_gauge::events(c, a),
        _ => vec![],
    }
}

/// A test ruler for the modules: the real font of a language, as `make` wraps with it.
#[cfg(test)]
pub(crate) mod testkit {
    use crate::captions::{em_factor, Measurer, Word};

    /// The frames every data card is checked on: tall, wide, 4:3 and 21:9.
    pub const FRAMES: [(u32, u32); 4] = [(1080, 1920), (1920, 1080), (1440, 1080), (2520, 1080)];

    pub fn wrapper(lang: &str) -> impl Fn(&str, u32, f32) -> Vec<String> {
        let (_, font) = crate::template::font_for(lang);
        move |t: &str, s: u32, max: f32| {
            let m = Measurer::new(font.to_vec(), s as f32).unwrap();
            crate::captions::wrap(t, max, |x| m.width(x))
        }
    }

    /// The caption size of a language on a frame, as `make` sets it.
    pub fn size(lang: &str, (w, h): (u32, u32)) -> u32 {
        let latin = em_factor(crate::template::FONTS[0].2).unwrap();
        let (_, font) = crate::template::font_for(lang);
        (84.0 * w.min(h) as f32 / 1080.0 * em_factor(font).unwrap() / latin).round() as u32
    }

    pub fn word(t: &str, s: f64) -> Word {
        Word {
            text: t.into(),
            start: s,
            end: s + 0.3,
        }
    }

    /// The events of a card on a frame through `cards::events`, with the real wrapper.
    pub fn events(
        c: &crate::cards::Card,
        lang: &str,
        frame: (u32, u32),
        said: &[Word],
    ) -> Vec<String> {
        let f = crate::cards::Frame {
            w: frame.0,
            h: frame.1,
            size: size(lang, frame),
        };
        crate::cards::events(c, 0.0, 6.0, &f, &wrapper(lang), said)
    }

    /// Every drawing point and every text box of the events, as (left, top, right, bottom). Texts are measured
    /// with the real font. A `\move` counts at both ends.
    pub fn boxes(events: &[String], lang: &str) -> Vec<(f32, f32, f32, f32, String)> {
        let (_, font) = crate::template::font_for(lang);
        let mut out = vec![];
        for e in events {
            let (tags, body) = e
                .split_once(",,{")
                .and_then(|x| x.1.split_once('}'))
                .unwrap();
            let nums = |tag: &str| -> Option<Vec<f32>> {
                let inner = tags.split(tag).nth(1)?.split(')').next()?;
                Some(inner.split(',').filter_map(|v| v.parse().ok()).collect())
            };
            let spots: Vec<(f32, f32)> = match (nums("\\move("), nums("\\pos(")) {
                (Some(m), _) => vec![(m[0], m[1]), (m[2], m[3])],
                (_, Some(p)) => vec![(p[0], p[1])],
                _ => panic!("no place: {e}"),
            };
            for (x, y) in spots {
                if tags.contains("\\p1") {
                    let n: Vec<f32> = body
                        .split_whitespace()
                        .filter_map(|v| v.parse().ok())
                        .collect();
                    let xs = n.iter().step_by(2).map(|v| v + x);
                    let ys = n.iter().skip(1).step_by(2).map(|v| v + y);
                    out.push((
                        xs.clone().fold(f32::MAX, f32::min),
                        ys.clone().fold(f32::MAX, f32::min),
                        xs.fold(f32::MIN, f32::max),
                        ys.fold(f32::MIN, f32::max),
                        String::from("drawing"),
                    ));
                    continue;
                }
                let fs: f32 = tags
                    .split("\\fs")
                    .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
                    .last()
                    .and_then(|v| v.split('\\').next())
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| panic!("no size: {e}"));
                // A line may set its own size in an inline block.
                let (mut plain, mut wide, mut high, mut now) = (String::new(), 0.0f32, 0.0f32, fs);
                for (i, l) in body.split("\\N").enumerate() {
                    let mut text = String::new();
                    let mut rest = l;
                    while let Some((a, b)) = rest.split_once('{') {
                        text.push_str(a);
                        let (block, after) = b.split_once('}').unwrap_or((b, ""));
                        if let Some(v) = block
                            .split("\\fs")
                            .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
                            .last()
                            .and_then(|v| v.split('\\').next())
                            .and_then(|v| v.parse().ok())
                        {
                            now = v;
                        }
                        rest = after;
                    }
                    text.push_str(rest);
                    let m = Measurer::new(font.to_vec(), now).unwrap();
                    wide = wide.max(m.width(text.trim_start_matches(['\u{200E}', '\u{200F}'])));
                    high += now;
                    if i > 0 {
                        plain.push_str("\\N");
                    }
                    plain.push_str(&text);
                }
                let an: u32 = tags
                    .split("\\an")
                    .nth(1)
                    .and_then(|v| v[..1].parse().ok())
                    .unwrap();
                let (col, row) = ((an - 1) % 3, (an - 1) / 3);
                let left = x - wide * [0.0, 0.5, 1.0][col as usize];
                let top = y - high * [1.0, 0.5, 0.0][row as usize];
                out.push((left, top, left + wide, top + high, plain));
            }
        }
        out
    }

    /// One card of each data kind, in a language.
    pub fn samples(ar: bool) -> Vec<crate::cards::Card> {
        let j = |v: serde_json::Value| serde_json::from_value::<crate::cards::Card>(v).unwrap();
        use serde_json::json;
        if ar {
            vec![
                j(
                    json!({"kind": "table", "label": "الخطط", "rows": [["الخطة", "السعر", "المقاعد"], ["مجانية", "٠", "١"], ["الفريق", "١٢", "١٠"], ["Python", "50x", "Q1"]]}),
                ),
                j(
                    json!({"kind": "number_line", "label": "درجة الحرارة", "values": [-50, 150], "points": [-25, 0, 100], "items": ["ليلة باردة", "جليد", "Steam"]}),
                ),
                j(
                    json!({"kind": "diagram", "nodes": ["الملخص", "الوسائط", "التركيب", "الفيديو", "MP4"], "edges": [["الملخص", "التركيب"], ["الوسائط", "التركيب"], ["التركيب", "الفيديو"], ["التركيب", "MP4"]]}),
                ),
                j(
                    json!({"kind": "steps", "items": ["اكتب الملخص", "راجع الخطة وما سوف تكلفه", "Run scriptvox make"]}),
                ),
                j(json!({"kind": "gauge", "values": [72], "label": "البطارية"})),
                j(json!({"kind": "progress", "values": [3, 5], "label": "الخطوات المنجزة"})),
            ]
        } else {
            vec![
                j(
                    json!({"kind": "table", "label": "Plans", "rows": [["Plan", "Price", "Seats"], ["Free", "$0", "1"], ["Team", "$12", "10"], ["Company", "$40", "100"]]}),
                ),
                j(
                    json!({"kind": "number_line", "label": "Temperature", "values": [-50, 150], "points": [-25, 0, 100], "items": ["Cold night", "Ice", "Steam"]}),
                ),
                j(
                    json!({"kind": "diagram", "nodes": ["Brief", "Media", "Render", "Video", "Report"], "edges": [["Brief", "Render"], ["Media", "Render"], ["Render", "Video"], ["Render", "Report"]]}),
                ),
                j(
                    json!({"kind": "steps", "items": ["Write the brief", "Check the plan and what it will cost", "Make the video"]}),
                ),
                j(json!({"kind": "gauge", "values": [72], "label": "Battery"})),
                j(json!({"kind": "progress", "values": [3, 5], "label": "Steps done"})),
            ]
        }
    }

    /// Everything a card draws stays inside the frame's sides and between the rows' limits of `a`.
    pub fn assert_inside(events: &[String], lang: &str, (w, h): (u32, u32), what: &str) {
        assert!(!events.is_empty(), "{what}: no events");
        let wide = w > h;
        let top = h as f32 * if wide { 0.08 } else { 0.15 };
        let bottom = h as f32
            * if !wide {
                0.62
            } else if lang == "ar" {
                0.66
            } else {
                0.72
            };
        for (l, t, r, b, text) in boxes(events, lang) {
            assert!(
                l >= w as f32 * 0.02 - 1.0 && r <= w as f32 * 0.98 + 1.0,
                "{what} {w}x{h}: \"{text}\" from x {l:.0} to {r:.0}"
            );
            assert!(
                t >= top - 1.0 && b <= bottom + 1.0,
                "{what} {w}x{h}: \"{text}\" from y {t:.0} to {b:.0}, room {top:.0} to {bottom:.0}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::{assert_inside, events, samples};

    /// The squarer frames (1:1, 4:5, 3:4) have the least room above the captions. A usual card of each kind stays
    /// in it, in English and in Arabic with Latin words in it; a Latin-only text in an Arabic card is set in the
    /// Latin font.
    #[test]
    fn a_usual_card_of_each_kind_stays_in_the_room_of_a_square_frame() {
        for (lang, ar) in [("en", false), ("ar", true)] {
            for c in samples(ar) {
                assert!(c.problem().is_none(), "{:?}", c.problem());
                for frame in [(1080, 1080), (1080, 1350), (1080, 1440)] {
                    let ev = events(&c, lang, frame, &[]);
                    assert_inside(&ev, lang, frame, &format!("{:?}", c.kind));
                }
            }
        }
        let table = &samples(true)[0];
        let ev = events(table, "ar", (1080, 1920), &[]);
        assert!(ev
            .iter()
            .any(|e| e.contains("{\\fnNoto Sans\\fs") && e.ends_with("Python")));
        assert!(ev
            .iter()
            .any(|e| e.ends_with("مجانية") && !e.contains("\\fnNoto Sans")));
        let ev = events(&samples(false)[0], "en", (1080, 1920), &[]);
        assert!(!ev.iter().any(|e| e.contains("\\fnNoto Sans")));
    }
}
