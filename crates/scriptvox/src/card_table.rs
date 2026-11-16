//! The `table` card: a header row and up to 7 more rows of up to 5 cells, on a dark panel. A `mark` lights a row, a
//! column or one cell from a spoken word. See docs/specs/cards-data.md.

use crate::card_kit::{rect, shape, Area, PANEL, PANEL_LIT, PLAIN};
use crate::cards::{esc, line, Card};

pub const MAX_ROWS: usize = 8;
pub const MAX_COLS: usize = 5;
pub const MAX_CELL: usize = 24;
/// The longest cell of each column, added up: more has no readable size on a tall frame.
pub const MAX_WIDTH: usize = 40;

fn key(s: &str) -> String {
    s.split_whitespace()
        .map(crate::gates::norm_word)
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The cell a mark names, as (row, column). The part must be the whole text of exactly one cell.
fn cell_of(rows: &[Vec<String>], part: &str) -> Result<(usize, usize), usize> {
    let want = key(part);
    let found: Vec<(usize, usize)> = rows
        .iter()
        .enumerate()
        .flat_map(|(r, row)| row.iter().enumerate().map(move |(c, t)| (r, c, t)))
        .filter(|(_, _, t)| !want.is_empty() && key(t) == want)
        .map(|(r, c, _)| (r, c))
        .collect();
    match found.as_slice() {
        [one] => Ok(*one),
        _ => Err(found.len()),
    }
}

pub fn problem(c: &Card) -> Option<String> {
    let rows = c.rows.as_deref().unwrap_or(&[]);
    let cols = rows.first().map_or(0, |r| r.len());
    if !(2..=MAX_ROWS).contains(&rows.len()) || !(2..=MAX_COLS).contains(&cols) {
        return Some(format!(
            "a table card needs `rows`: 2 to {MAX_ROWS} rows (the first is the header) of 2 to {MAX_COLS} cells; this one has {} rows and {cols} cells in the first. Split a larger table over two scenes",
            rows.len()
        ));
    }
    if let Some(r) = rows.iter().position(|r| r.len() != cols) {
        return Some(format!(
            "table: row {} has {} cells and the header has {cols}; give every row {cols} cells (an empty cell is \"\")",
            r + 1,
            rows[r].len()
        ));
    }
    if rows[0].iter().any(|t| t.trim().is_empty()) {
        return Some("table: every cell of the header row needs a name".into());
    }
    if let Some(t) = rows
        .iter()
        .flatten()
        .find(|t| t.chars().count() > MAX_CELL || t.contains('\n'))
    {
        return Some(format!(
            "table: the cell \"{t}\" is too long; a cell has one line of at most {MAX_CELL} characters, so shorten it"
        ));
    }
    let wide: usize = (0..cols)
        .map(|k| rows.iter().map(|r| r[k].chars().count()).max().unwrap_or(0))
        .sum();
    if wide > MAX_WIDTH {
        return Some(format!(
            "table: the longest cells of the {cols} columns have {wide} characters together and {MAX_WIDTH} fit a tall frame; shorten the cells or drop a column"
        ));
    }
    None
}

/// The marks of a table: each names one cell by its whole text.
pub fn mark_problem(c: &Card) -> Option<String> {
    let rows = c.rows.as_deref().unwrap_or(&[]);
    for m in c.mark.iter().flatten() {
        if m.style.is_some() {
            return Some(
                "a table mark takes no `style`: the row, column or cell gets a band, so remove `style`"
                    .into(),
            );
        }
        match cell_of(rows, &m.part) {
            Ok((0, 0)) => {
                return Some(format!(
                    "mark: \"{}\" is the corner cell; name a cell of the first column (its row is lit), of the header (its column is lit) or any other cell",
                    m.part
                ))
            }
            Ok(_) => {}
            Err(n) => {
                return Some(format!(
                    "mark: the part \"{}\" is the whole text of {n} cells of the table; it must be the whole text of one cell (write it as the cell does)",
                    m.part
                ))
            }
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

pub(crate) fn events(c: &Card, a: &Area) -> Vec<String> {
    let rows = c.rows.as_deref().unwrap_or(&[]);
    let (n, cols) = (rows.len(), rows.first().map_or(0, |r| r.len()));
    if n == 0 || cols == 0 {
        return vec![];
    }
    let mut out = vec![];
    // The heading, on one line, above the panel.
    // Cells are at most as large as the captions.
    let base = a.size;
    let avail = a.w * 0.88;
    let label = c.label.as_deref().filter(|l| !l.trim().is_empty());
    // Each column is as wide as its widest cell and 0.9 em of padding, measured at size 100.
    let pad = 90.0;
    let natural: Vec<f32> = (0..cols)
        .map(|k| rows.iter().map(|r| a.width(&r[k], 100)).fold(0.0, f32::max) + pad)
        .collect();
    let sum: f32 = natural.iter().sum();
    // The size: the base, less when the columns are wider than the frame or the rows at their tightest are taller
    // than the room. The heading is at most 1.2 times the cells, so a full table keeps its room.
    // A row is 1.9 lines of a Latin font, 1.5 at its tightest; never under the font's own line.
    let (row, tight) = ((1.9 * a.latin).max(1.1), (1.5 * a.latin).max(1.05));
    let cell_size = |head_h: f32| {
        (base as f32)
            .min(avail / sum * 100.0)
            .min((a.bottom - a.top - head_h) / n as f32 / tight)
            .floor()
            .max(1.0)
    };
    let mut head = label.map(|l| (l, a.fit(l, a.size * 9 / 10, avail)));
    let mut s = cell_size(head.map_or(0.0, |h| a.head_height(h.1)));
    if let Some(h) = head.as_mut().filter(|h| h.1 as f32 > s * 1.2) {
        h.1 = (s * 1.2) as u32;
        s = cell_size(a.head_height(h.1));
    }
    let head_h = head.map_or(0.0, |h| a.head_height(h.1));
    let room = a.bottom - a.top - head_h;
    let row_h = (s * row).min(room / n as f32);
    // A narrow table takes at least 60 percent of the width, so two short columns do not huddle.
    let tw = (sum * s / 100.0).max(avail * 0.6).min(avail);
    let widths: Vec<f32> = natural.iter().map(|w| w / sum * tw).collect();
    let (x0, th) = ((a.w - tw) / 2.0, row_h * n as f32);
    let y0 = a.top + head_h + ((room - th) / 2.0).min(a.h * 0.06).max(0.0);
    // Column k from the reading side: its left and right edge.
    let edges = |k: usize| {
        let before: f32 = widths[..k].iter().sum();
        if a.rtl {
            (x0 + tw - before - widths[k], x0 + tw - before)
        } else {
            (x0 + before, x0 + before + widths[k])
        }
    };
    let y = |r: usize| y0 + r as f32 * row_h;
    if let Some((l, hs)) = head {
        out.push(line(
            a.start,
            a.end,
            &format!("\\an8\\pos({},{})\\fs{hs}\\fad(200,200)", a.w / 2.0, a.top),
            &esc(l),
        ));
    }
    out.push(line(
        a.start,
        a.end,
        &shape(PANEL, "&H18&", (200, 200)),
        &rect(x0, y0, x0 + tw, y0 + th),
    ));
    out.push(line(
        a.start,
        a.end,
        &shape(PANEL_LIT, "&H18&", (200, 200)),
        &rect(x0, y0, x0 + tw, y0 + row_h),
    ));
    // Thin lines between the rows, one drawing.
    let hair = (s * 0.04).max(1.0);
    let rules: Vec<String> = (2..n)
        .map(|r| rect(x0, y(r) - hair / 2.0, x0 + tw, y(r) + hair / 2.0))
        .collect();
    if !rules.is_empty() {
        out.push(line(
            a.start,
            a.end,
            &shape("&HFFFFFF&", "&HC0&", (200, 200)),
            &rules.join(" "),
        ));
    }
    // The lit band: under the letters, a third solid, so white letters stay readable (card-mark.md).
    let mut lit: Vec<(f64, (usize, usize))> = c
        .mark
        .iter()
        .flatten()
        .filter_map(|m| {
            let at = crate::cards::word_start(&m.on, a.said).unwrap_or(a.start);
            Some((at.max(a.start + 0.3), cell_of(rows, &m.part).ok()?))
        })
        .collect();
    lit.sort_by(|p, q| p.0.total_cmp(&q.0));
    for (i, (at, (r, k))) in lit.iter().enumerate() {
        // To the next mark, or to the card's end.
        let to = lit.get(i + 1).map_or(a.end, |nx| nx.0);
        if to - at < 0.1 {
            continue;
        }
        let band = match (*r, *k) {
            (r, 0) => rect(x0, y(r), x0 + tw, y(r) + row_h),
            (0, k) => rect(edges(k).0, y0, edges(k).1, y0 + th),
            (r, k) => rect(edges(k).0, y(r), edges(k).1, y(r) + row_h),
        };
        out.push(line(
            *at,
            to,
            &shape(crate::captions::ACCENT, "&HA8&", (150, 150)),
            &band,
        ));
    }
    // The cells: the first column from the reading side, the others centred. Rows come one after another.
    let step = (0.12f64).min((a.end - a.start) * 0.4 / n as f64);
    for (r, row) in rows.iter().enumerate() {
        let at = a.start + 0.1 + r as f64 * step;
        for (k, t) in row.iter().enumerate().filter(|(_, t)| !t.trim().is_empty()) {
            let (l, rt) = edges(k);
            let (an, x) = match (k, a.rtl) {
                (0, false) => (4, l + s * 0.45),
                (0, true) => (6, rt - s * 0.45),
                _ => (5, (l + rt) / 2.0),
            };
            let look = if r == 0 {
                format!("\\1c{}", crate::captions::ACCENT)
            } else {
                "\\b0".to_string()
            };
            out.push(line(
                at,
                a.end,
                &format!(
                    "\\an{an}\\pos({x:.0},{:.0})\\fs{s:.0}{PLAIN}{look}\\fad(200,200)",
                    y(r) + row_h / 2.0
                ),
                &a.text(t, s),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::card_kit::testkit::{assert_inside, boxes, events, word, FRAMES};
    use crate::cards::Card;
    use serde_json::json;

    fn card(v: serde_json::Value) -> Card {
        serde_json::from_value(v).unwrap()
    }

    fn en() -> Card {
        card(json!({"kind": "table", "label": "Plans", "rows": [
            ["Plan", "Price", "Seats"], ["Free", "$0", "1"], ["Team", "$12", "10"], ["Company", "$40", "100"]],
            "mark": [{"part": "Team", "on": "team"}, {"part": "Seats", "on": "seats"}, {"part": "$40", "on": "forty"}]}))
    }

    /// The widest and tallest table the brief check lets through: 8 rows, 5 columns, 40 characters across.
    fn worst(ar: bool) -> Card {
        let cell = |k: usize| {
            if ar {
                ["المنطقة الشرقية", "الأولى", "الثانية", "الثالث", "الرابع"][k].to_string()
            } else {
                ["Northern region", "Quarter", "Second", "Fourth", "Annual"][k].to_string()
            }
        };
        let rows: Vec<Vec<String>> = (0..8).map(|_| (0..5).map(cell).collect()).collect();
        card(
            json!({"kind": "table", "label": if ar { "المبيعات حسب المنطقة والربع" } else { "Sales by region and quarter" }, "rows": rows}),
        )
    }

    #[test]
    fn a_table_is_checked() {
        assert!(en().problem().is_none(), "{:?}", en().problem());
        assert!(worst(false).problem().is_none());
        assert!(worst(true).problem().is_none());
        let bad = |v: serde_json::Value, want: &str| {
            let p = card(v).problem().unwrap_or_default();
            assert!(p.contains(want), "{p}");
        };
        bad(json!({"kind": "table"}), "needs `rows`");
        bad(
            json!({"kind": "table", "rows": [["A", "B"], ["1"]]}),
            "row 2 has 1 cells",
        );
        bad(
            json!({"kind": "table", "rows": [["A", "B", "C", "D", "E", "F"], ["1", "2", "3", "4", "5", "6"]]}),
            "2 to 5 cells",
        );
        bad(
            json!({"kind": "table", "rows": [["A", "B"], ["a cell with far too many letters", "2"]]}),
            "at most 24 characters",
        );
        bad(
            json!({"kind": "table", "rows": [["Twelve chars", "Twelve chars", "Twelve chars", "Twelve chars"], ["1", "2", "3", "4"]]}),
            "48 characters together",
        );
        // A field the table does not draw, with what to do.
        bad(
            json!({"kind": "table", "text": "Plans", "rows": [["A", "B"], ["1", "2"]]}),
            "does not draw `text`: remove it",
        );
        bad(
            json!({"kind": "table", "values": [1.0], "rows": [["A", "B"], ["1", "2"]]}),
            "does not draw `values`",
        );
        // `rows` on another kind.
        bad(
            json!({"kind": "list", "items": ["a"], "rows": [["A", "B"], ["1", "2"]]}),
            "only a table card takes `rows`",
        );
        // Marks: one cell by its whole text, not the corner, no style.
        let marked = |part: &str| json!({"kind": "table", "rows": [["Plan", "Price"], ["Free", "$0"], ["Team", "$0"]], "mark": [{"part": part, "on": "x"}]});
        bad(marked("$0"), "of 2 cells");
        bad(marked("Gold"), "of 0 cells");
        bad(marked("Plan"), "corner cell");
        assert!(card(marked("team")).problem().is_none());
        bad(
            json!({"kind": "table", "rows": [["A", "B"], ["1", "2"]], "mark": [{"part": "2", "on": "x", "style": "ring"}]}),
            "takes no `style`",
        );
    }

    #[test]
    fn an_old_card_is_written_as_before() {
        let old = json!({"kind": "stat", "text": "42%", "label": "growth"});
        assert_eq!(serde_json::to_value(card(old.clone())).unwrap(), old);
    }

    #[test]
    fn every_table_stays_in_its_room_and_its_cells_in_their_columns() {
        for (lang, c) in [("en", en()), ("en", worst(false)), ("ar", worst(true))] {
            for frame in FRAMES {
                let ev = events(&c, lang, frame, &[]);
                assert_inside(&ev, lang, frame, "table");
                let b = boxes(&ev, lang);
                // The panel is the first drawing; every cell text is inside it and no two texts of a row touch.
                let panel = b.iter().find(|x| x.4 == "drawing").unwrap();
                let cells: Vec<_> = b
                    .iter()
                    .filter(|x| x.4 != "drawing" && x.1 >= panel.1 - 1.0)
                    .collect();
                assert_eq!(cells.len(), c.rows.iter().flatten().flatten().count());
                for (i, p) in cells.iter().enumerate() {
                    assert!(
                        p.0 >= panel.0 && p.2 <= panel.2 && p.3 <= panel.3 + 1.0,
                        "{frame:?} {p:?} {panel:?}"
                    );
                    for q in &cells[i + 1..] {
                        let same_row = (p.1 - q.1).abs() < 1.0;
                        assert!(
                            !same_row || p.2 <= q.0 || q.2 <= p.0,
                            "{lang} {frame:?}: {p:?} over {q:?}"
                        );
                        assert!(same_row || p.3 <= q.1 + 1.0 || q.3 <= p.1 + 1.0);
                    }
                }
                // The letters stay readable: 45 percent of the caption size or more (38 px at 1080 in English).
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
            }
        }
    }

    #[test]
    fn a_mark_lights_a_row_a_column_and_a_cell_from_its_word_and_arabic_mirrors() {
        let said = [word("team", 1.0), word("seats", 2.5), word("forty", 4.0)];
        let ev = events(&en(), "en", (1920, 1080), &said);
        let b = boxes(&ev, "en");
        let panel = b.iter().find(|x| x.4 == "drawing").unwrap().clone();
        let bands: Vec<_> = ev
            .iter()
            .zip(&b)
            .filter(|(e, _)| e.contains("\\1a&HA8&"))
            .collect();
        assert_eq!(bands.len(), 3, "{ev:#?}");
        // Each band from its word to the next one's; the last to the card's end.
        for ((e, _), (from, to)) in bands.iter().zip([
            ("0:00:01.00", "0:00:02.50"),
            ("0:00:02.50", "0:00:04.00"),
            ("0:00:04.00", "0:00:06.00"),
        ]) {
            assert!(e.contains(&format!(",{from},{to},")), "{e}");
        }
        let (row, col, cell) = (bands[0].1, bands[1].1, bands[2].1);
        let row_h = (panel.3 - panel.1) / 4.0;
        // "Team" is in the first column: its whole row, the third of four.
        assert!((row.0 - panel.0).abs() < 1.5 && (row.2 - panel.2).abs() < 1.5);
        assert!(
            (row.1 - (panel.1 + 2.0 * row_h)).abs() < 1.5 && (row.3 - row.1 - row_h).abs() < 1.5
        );
        // "Seats" is in the header: its whole column, the last one, so at the right in English.
        assert!((col.1 - panel.1).abs() < 1.5 && (col.3 - panel.3).abs() < 1.5);
        assert!((col.2 - panel.2).abs() < 1.5 && col.0 > panel.0 + (panel.2 - panel.0) * 0.5);
        // "$40" is one cell: the last row, the middle column.
        assert!((cell.3 - panel.3).abs() < 1.5 && (cell.3 - cell.1 - row_h).abs() < 1.5);
        assert!(cell.0 > panel.0 + 10.0 && cell.2 < col.0 + 1.5);
        // Arabic: the first column stands at the right, its text from the right edge; the last column at the left.
        let ar = card(
            json!({"kind": "table", "rows": [["الخطة", "السعر"], ["مجانية", "٠"], ["الفريق", "١٢"]],
            "mark": [{"part": "السعر", "on": "السعر"}]}),
        );
        assert!(ar.problem().is_none(), "{:?}", ar.problem());
        let ev = events(&ar, "ar", (1080, 1920), &[word("السعر", 1.0)]);
        let b = boxes(&ev, "ar");
        let panel = b.iter().find(|x| x.4 == "drawing").unwrap();
        let first = ev.iter().find(|e| e.contains("الخطة")).unwrap();
        assert!(first.contains("\\an6"), "{first}");
        let band = ev
            .iter()
            .zip(&b)
            .find(|(e, _)| e.contains("\\1a&HA8&"))
            .unwrap()
            .1;
        assert!((band.0 - panel.0).abs() < 1.5 && band.2 < panel.0 + (panel.2 - panel.0) * 0.6);
    }
}
