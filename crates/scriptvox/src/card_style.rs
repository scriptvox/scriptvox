//! Brief `card_style` (`docs/specs/card-style.md`): `paper` draws every card, the hook and the end card for a light
//! page. Dark text, light boxes, thin dark rules, and the accent for the header, the marks and the numbers.
//!
//! The cards are drawn as before (`cards.rs` and the `card_*.rs` files), with their dark-page colours. This file
//! then changes each colour of the Card and Hook lines of the finished ASS document to its part on a light page.
//! So the layout, the times and the Arabic mirror are the same in both styles, and a brief with no `card_style`
//! never comes here: its file is the same, byte for byte.
//! ponytail: a table of colours, not a palette passed through 21 card kinds. A new colour in a card file must get
//! a row in `part`; the test `no_dark_page_colour_is_left` in `tests/card_style.rs` fails until it has one.

/// The text and the rules, `#16181a`. All colours here are ASS order (`BBGGRR`).
const INK: &str = "1A1816";
/// The box behind text and the edge around a dot: almost white, so it reads on any light page.
const LIGHT: &str = "FAFCFD";
/// A panel (table, code, dial), and the lit panel (table header, diagram box).
const PANEL: &str = "DFE8EC";
const PANEL_LIT: &str = "CAD6DC";
/// Text that is not the point now. 6.5 to 1 on the page, 4.9 to 1 on the lit panel.
const DIM: &str = "5B5855";
/// The accent of a paper card when the brief gives none: the default yellow cannot be read on a light page.
pub const ACCENT: &str = "#B3421F";

/// The contrast, 1 to 21, of two `#RRGGBB` colours.
fn contrast(a: &str, b: &str) -> Option<f64> {
    let (a, b) = (
        crate::captions::contrast_on_black(a)?,
        crate::captions::contrast_on_black(b)?,
    );
    Some(a.max(b) / a.min(b))
}

pub fn is_paper(style: Option<&str>) -> bool {
    style == Some("paper")
}

/// What is wrong with a brief's `card_style`, with what to do. `paper` needs a light page and an accent that can
/// be read on it.
pub fn problem(
    style: Option<&str>,
    background: Option<&str>,
    accent: Option<&str>,
) -> Option<String> {
    match style {
        None | Some("dark") => return None,
        Some("paper") => {}
        Some(s) => return Some(format!("card_style is dark or paper, not {s}")),
    }
    let light = background
        .and_then(|b| crate::template::background_colours(b).ok())
        .is_some_and(|c| {
            c.iter()
                .all(|c| contrast(&format!("#{c}"), "#16181a").is_some_and(|k| k >= 7.0))
        });
    if !light {
        return Some(format!(
            "card_style paper draws dark text for a light page, and background {} is not light: set `background` to light colours, for example \"#f6f4ee,#ebe7db\", or drop card_style",
            background.unwrap_or("dusk (the default)")
        ));
    }
    let k = accent.and_then(|a| contrast(a, "#fdfcfa")).unwrap_or(21.0);
    (k < 3.0).then(|| {
        format!(
            "accent {} is too light on a paper card (contrast {k:.1}, needs 3); pick a darker shade, for example {ACCENT}",
            accent.unwrap_or_default()
        )
    })
}

/// The colour on a light page for one colour of a dark-page card. `which`: `1` the fill, `3` the edge. `drawing`:
/// a shape, not letters. `alpha`: the fill's own alpha in that block. Returns the colour and a new fill alpha.
fn part(
    which: char,
    c: &str,
    drawing: bool,
    alpha: Option<&str>,
    dark_accent: bool,
) -> Option<(&'static str, Option<&'static str>)> {
    Some(match (which, c) {
        // Letters, axes, rules, arrows and dots. A box edge (diagram) is a thin ink rule.
        (_, "FFFFFF") | (_, "E6E6E6") | (_, "F0F0F0") => (INK, None),
        // The dark edge that kept an arrow apart from the picture.
        ('3', "000000") => (LIGHT, None),
        // A see-through black shape is a backdrop or a track: a light grey wash of ink. A wide one (plot, bars)
        // is faint, a thin one (the rail of a timeline, the edge of a bar) is stronger.
        (_, "000000") if drawing && alpha.is_some_and(|a| a >= "80") => (INK, Some("E6")),
        (_, "000000") if drawing && alpha.is_some_and(|a| a != "00") => (INK, Some("B8")),
        // A solid black shape is the edge around an accent dot.
        (_, "000000") if drawing => (LIGHT, None),
        (_, "1E1A17") => (PANEL, None),
        (_, "402818") | (_, "3A322C") => (PANEL_LIT, None),
        (_, "B4B4B4") | (_, "B0B0B0") | (_, "B8B8B8") | (_, "8D8C7F") => (DIM, None),
        // The track of a gauge, and the disc of a step that is not said now.
        (_, "606060") => ("B4BCC2", None),
        (_, "303030") => ("C2CCD2", None),
        // The digit on the accent disc of the step that is said now.
        (_, "202020") if dark_accent => (LIGHT, None),
        (_, "202020") => (INK, None),
        // A highlight (a mark, a marked row or column): fainter, so ink and accent letters read on it.
        (_, "00E5FF") if drawing && alpha == Some("A8") => ("00E5FF", Some("D8")),
        // The line of code that the voice talks about: a wash of the accent.
        (_, "5A4A3C") => ("00E5FF", Some("D0")),
        // The added and the removed line of a diff.
        (_, "2F5A1F") => ("A9DDB5", None),
        (_, "2A2A78") => ("B4B9F2", None),
        // Code: keyword, text, number, call. The prompt of a terminal is the accent.
        (_, "FF9BD9") => ("A52D8A", None),
        (_, "6BB2F2") => ("004AA0", None),
        (_, "A6DB9C") => ("3A6B1E", None),
        (_, "8AE3FF") => ("AD5C0B", None),
        (_, "7BD88F") => ("00E5FF", None),
        // The series of a donut and of a plot, after the accent.
        (_, "FFD040") => ("A67312", None),
        (_, "B070FF") => ("6E30B0", None),
        (_, "60D070") => ("327D2F", None),
        (_, "4080FF") => ("005CB8", None),
        (_, "C0C0C0") => ("807D7A", None),
        _ => return None,
    })
}

/// One override block (the text between `{` and `}`) with its colours changed.
fn block(b: &str, dark_accent: bool) -> String {
    let drawing = b.contains("\\p1");
    let alpha = b
        .split("\\1a&H")
        .nth(1)
        .and_then(|a| a.get(..2))
        .map(str::to_uppercase);
    let mut out = String::with_capacity(b.len() + 8);
    let mut new_alpha = None;
    let mut rest = b;
    while let Some(i) = rest.find("c&H") {
        let which = rest[..i].chars().last().unwrap_or('1');
        let tail = &rest[i + 3..];
        let n = tail.bytes().take_while(u8::is_ascii_hexdigit).count();
        out.push_str(&rest[..i + 3]);
        // `&HBBGGRR&` or `&H00BBGGRR`: the colour is the last 6 digits.
        let found = (n == 6 || n == 8)
            .then(|| {
                part(
                    if which == '3' { '3' } else { '1' },
                    &tail[n - 6..n].to_uppercase(),
                    drawing,
                    alpha.as_deref(),
                    dark_accent,
                )
            })
            .flatten();
        match found {
            Some((c, a)) => {
                out.push_str(&tail[..n - 6]);
                // The accent keeps its own 8 digits: `make` puts the brief's accent in place of that text.
                out.push_str(if c == "00E5FF" && n == 6 { "00" } else { "" });
                out.push_str(c);
                if which != '3' {
                    new_alpha = new_alpha.or(a);
                }
            }
            None => out.push_str(&tail[..n]),
        }
        rest = &tail[n..];
    }
    out.push_str(rest);
    match (new_alpha, alpha) {
        (Some(a), Some(old)) => out.replace(&format!("\\1a&H{old}&"), &format!("\\1a&H{a}&")),
        (Some(a), None) => out.replace("\\p1", &format!("\\1a&H{a}&\\p1")),
        _ => out,
    }
}

/// The Card and Hook styles of an ASS document for a light page: ink letters, a light box.
pub fn styles(ass: &str) -> String {
    let mut out = String::with_capacity(ass.len() + 64);
    for l in ass.lines() {
        if l.starts_with("Style: Card,") || l.starts_with("Style: Hook,") {
            // Name, font, size, then the fill, the second fill, the edge and the box, as `&HAABBGGRR`.
            let mut f: Vec<String> = l.split(',').map(String::from).collect();
            for (i, v) in f.iter_mut().enumerate().take(7).skip(3) {
                if let Some(a) = v.strip_suffix(if i < 5 { "FFFFFF" } else { "000000" }) {
                    *v = format!("{a}{}", if i < 5 { INK } else { LIGHT });
                }
            }
            out.push_str(&f.join(","));
        } else {
            out.push_str(l);
        }
        out.push('\n');
    }
    out
}

/// One line of an ASS document for a light page. Only a Card or Hook event changes. `accent`: the brief's own.
/// Call it before `make` puts the brief's accent in place of the default one, and once for each line.
pub fn event(l: &str, accent: Option<&str>) -> String {
    if !l.starts_with("Dialogue:") || !matches!(l.split(',').nth(3), Some("Card" | "Hook")) {
        return l.to_string();
    }
    let dark_accent = contrast(accent.unwrap_or(ACCENT), "#fdfcfa").is_some_and(|k| k >= 3.0);
    let mut line = String::with_capacity(l.len() + 16);
    for (i, piece) in l.split('{').enumerate() {
        match piece.split_once('}') {
            Some((tags, text)) if i > 0 => {
                line.push('{');
                line.push_str(&block(tags, dark_accent));
                line.push('}');
                line.push_str(text);
            }
            _ => {
                if i > 0 {
                    line.push('{');
                }
                line.push_str(piece);
            }
        }
    }
    match accent {
        Some(_) => line,
        None => line.replace(
            crate::captions::ACCENT,
            &crate::captions::ass_colour(ACCENT).unwrap_or_default(),
        ),
    }
}

/// An ASS document with its cards and its hook drawn for a light page. The caption lines do not change.
pub fn paper(ass: &str, accent: Option<&str>) -> String {
    styles(ass)
        .lines()
        .map(|l| event(l, accent) + "\n")
        .collect()
}

/// The end card's part of the video graph for a light page: the picture under the end card is blurred and not
/// made dark, so the ink text stands on the page.
pub fn outro(graph: &str) -> String {
    graph.replace("eq=brightness=-0.22,lutyuv=y='max(val,40)',", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_gets_ink_light_boxes_and_keeps_the_accent() {
        // White letters and rules are ink; a see-through black backdrop is a faint wash of ink.
        assert_eq!(block("\\an5\\1c&HFFFFFF&", true), "\\an5\\1c&H1A1816&");
        assert_eq!(
            block("\\1c&H000000&\\1a&H90&\\fad(200,200)\\p1", true),
            "\\1c&H1A1816&\\1a&HE6&\\fad(200,200)\\p1"
        );
        // A solid black disc is the light edge of a dot; the edge of an arrow is light, its fill ink.
        assert_eq!(block("\\1c&H000000&\\p1", true), "\\1c&HFAFCFD&\\p1");
        assert_eq!(
            block("\\3c&H000000&\\3a&H40&\\1c&HFFFFFF&\\p1", true),
            "\\3c&HFAFCFD&\\3a&H40&\\1c&H1A1816&\\p1"
        );
        // The digit on the accent disc: light on a dark accent, ink on a light one.
        assert_eq!(block("\\1c&H202020&", true), "\\1c&HFAFCFD&");
        assert_eq!(block("\\1c&H202020&", false), "\\1c&H1A1816&");
        // The accent stays as the text that `make` replaces; a series colour of 8 digits keeps its form.
        let a = format!("\\1c{}&", crate::captions::ACCENT);
        assert_eq!(block(&a, true), a);
        assert_eq!(block("\\1c&H7BD88F&", true), a);
        assert_eq!(block("\\1c&H00FFD040\\p1", true), "\\1c&H00A67312\\p1");
        // Twice is once.
        let once = block("\\3c&HFFFFFF&\\3a&H70&\\1c&H402818&\\1a&H10&\\p1", true);
        assert_eq!(block(&once, true), once);
    }

    #[test]
    fn only_the_card_and_hook_lines_change() {
        let ass = "Style: Cap,Noto Sans,84,&H0000E5FF,&H00FFFFFF,&H59000000,&H59000000,1,0,0,0,100,100,0,0,4,14,0,2,60,60,172,-1\n\
Style: Card,Noto Sans,84,&H00FFFFFF,&H00FFFFFF,&H66000000,&H66000000,1,0,0,0,100,100,0,0,4,21,0,5,40,40,0,-1\n\
Dialogue: 0,0:00:00.00,0:00:01.00,Cap,,0,0,0,,{\\1c&HFFFFFF&}Hi\n\
Dialogue: 2,0:00:00.00,0:00:01.00,Card,,0,0,0,,{\\1c&HFFFFFF&}Hi {\\1c&H0000E5FF&}5\n";
        let p = paper(ass, None);
        let l: Vec<&str> = p.lines().collect();
        assert_eq!(l[0], ass.lines().next().unwrap());
        assert!(
            l[1].contains(",&H001A1816,&H001A1816,&H66FAFCFD,&H66FAFCFD,1,"),
            "{}",
            l[1]
        );
        assert_eq!(l[2], ass.lines().nth(2).unwrap());
        // With no accent in the brief, a card takes the paper accent; the captions keep theirs.
        assert!(
            l[3].ends_with("{\\1c&H1A1816&}Hi {\\1c&H001F42B3&}5"),
            "{}",
            l[3]
        );
        assert!(paper(ass, Some("#1F6FEB")).contains("{\\1c&H0000E5FF&}5"));
        assert_eq!(styles(&p), p);
    }

    #[test]
    fn paper_needs_a_light_page_and_a_dark_accent() {
        let page = Some("#f6f4ee,#ebe7db");
        assert_eq!(problem(None, None, None), None);
        assert_eq!(problem(Some("dark"), None, None), None);
        assert_eq!(problem(Some("paper"), page, Some("#b3421f")), None);
        assert_eq!(problem(Some("paper"), page, None), None);
        assert!(problem(Some("light"), page, None)
            .unwrap()
            .contains("dark or paper"));
        assert!(problem(Some("paper"), None, None)
            .unwrap()
            .contains("is not light"));
        assert!(problem(Some("paper"), Some("night"), None)
            .unwrap()
            .contains("is not light"));
        assert!(problem(Some("paper"), page, Some("#FFD040"))
            .unwrap()
            .contains("too light"));
    }

    #[test]
    fn the_end_card_on_paper_is_not_made_dark() {
        let v = crate::make::Visual {
            inputs: vec![],
            graph: "[0:v]null[bg]".into(),
            label: "[bg]".into(),
        };
        let g = crate::make::outro_graph(&v, 9.5, None, (1920, 1080));
        assert!(g.contains("brightness"));
        let p = outro(&g);
        assert!(
            !p.contains("brightness") && !p.contains("lutyuv") && p.contains("boxblur"),
            "{p}"
        );
    }
}
