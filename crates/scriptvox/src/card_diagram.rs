//! The `diagram` card: boxes with labels and arrows between them, from `nodes` and `edges`. The boxes are put in
//! steps by the arrows; a wide frame runs the steps along the reading direction, a tall frame from top to bottom.
//! A graph that has no clean drawing is refused. See docs/specs/cards-data.md.

use crate::card_kit::{rect, Area, PANEL_LIT, PLAIN};
use crate::cards::{esc, line, Card};

pub const MAX_NODES: usize = 8;
pub const MAX_NAME: usize = 18;
/// The most steps in a row, and the most boxes in one step.
pub const MAX_STEPS: usize = 4;
pub const MAX_ACROSS: usize = 3;

/// The boxes in their steps: `steps[s]` holds the node numbers of step `s`, in the order they are drawn.
#[derive(Debug, PartialEq)]
pub struct Layout {
    pub steps: Vec<Vec<usize>>,
    pub edges: Vec<(usize, usize)>,
}

fn permutations(v: &[usize]) -> Vec<Vec<usize>> {
    if v.len() <= 1 {
        return vec![v.to_vec()];
    }
    let mut out = vec![];
    for i in 0..v.len() {
        let mut rest = v.to_vec();
        let first = rest.remove(i);
        for mut p in permutations(&rest) {
            p.insert(0, first);
            out.push(p);
        }
    }
    out
}

/// The layout of a graph, or why it has none, with what to do.
pub fn layout(nodes: &[String], pairs: &[Vec<String>]) -> Result<Layout, String> {
    let n = nodes.len();
    if !(2..=MAX_NODES).contains(&n) {
        return Err(format!(
            "a diagram card needs `nodes`: 2 to {MAX_NODES} box labels; this one has {n}. Split a larger diagram over two scenes"
        ));
    }
    let names: Vec<&str> = nodes.iter().map(|s| s.trim()).collect();
    if let Some(t) = names
        .iter()
        .find(|t| t.is_empty() || t.chars().count() > MAX_NAME || t.contains('\n'))
    {
        return Err(format!(
            "diagram: the node \"{t}\" must have 1 to {MAX_NAME} characters on one line; shorten it"
        ));
    }
    if let Some(t) = names
        .iter()
        .enumerate()
        .find(|(i, t)| names[..*i].contains(t))
    {
        return Err(format!(
            "diagram: the node \"{}\" is in `nodes` twice; each box needs its own label",
            t.1
        ));
    }
    if pairs.is_empty() || pairs.len() > 12 {
        return Err(format!(
            "a diagram card needs `edges`: 1 to 12 arrows, each [\"from\", \"to\"] with two labels of `nodes`; this one has {}",
            pairs.len()
        ));
    }
    let mut edges: Vec<(usize, usize)> = vec![];
    for p in pairs {
        let find = |t: &String| names.iter().position(|x| *x == t.trim());
        let [a, b] = p.as_slice() else {
            return Err(format!(
                "diagram: the edge {p:?} must be two labels, [\"from\", \"to\"]"
            ));
        };
        let (Some(i), Some(j)) = (find(a), find(b)) else {
            let bad = if find(a).is_none() { a } else { b };
            return Err(format!(
                "diagram: the edge [\"{a}\", \"{b}\"] names \"{bad}\", which is not in `nodes`; write it as `nodes` does"
            ));
        };
        if i == j {
            return Err(format!(
                "diagram: the edge [\"{a}\", \"{b}\"] goes from a box to itself; remove it"
            ));
        }
        if edges.contains(&(i, j)) {
            return Err(format!(
                "diagram: the edge [\"{a}\", \"{b}\"] is there twice; remove one"
            ));
        }
        edges.push((i, j));
    }
    if let Some(i) = (0..n).find(|i| !edges.iter().any(|e| e.0 == *i || e.1 == *i)) {
        return Err(format!(
            "diagram: the node \"{}\" has no arrow; give it an edge or remove it",
            names[i]
        ));
    }
    // The step of a box: one more than the latest box that points at it. A loop never settles.
    let mut step = vec![0usize; n];
    for round in 0..=n {
        let mut moved = false;
        for &(a, b) in &edges {
            if step[b] < step[a] + 1 {
                step[b] = step[a] + 1;
                moved = true;
            }
        }
        if !moved {
            break;
        }
        if round == n {
            let (a, b) = edges
                .iter()
                .copied()
                .find(|(a, b)| step[*b] > n || step[*a] >= n)
                .unwrap_or(edges[0]);
            return Err(format!(
                "diagram: the arrows go in a circle through \"{}\" and \"{}\", so there is no first box; remove one arrow of the circle",
                names[a], names[b]
            ));
        }
    }
    // A box with nothing before it stands right before the first box it points at, not always in step 0.
    for i in 0..n {
        if !edges.iter().any(|e| e.1 == i) {
            if let Some(first) = edges.iter().filter(|e| e.0 == i).map(|e| step[e.1]).min() {
                step[i] = first - 1;
            }
        }
    }
    let count = step.iter().max().map_or(0, |m| m + 1);
    if count > MAX_STEPS {
        return Err(format!(
            "diagram: the boxes stand {count} steps in a row and {MAX_STEPS} fit; split the diagram over two scenes, or use a steps card for a plain sequence"
        ));
    }
    if let Some(&(a, b)) = edges.iter().find(|(a, b)| step[*b] != step[*a] + 1) {
        return Err(format!(
            "diagram: the arrow from \"{}\" to \"{}\" would jump over a step and cross a box; remove it, or put the arrow through a box between them",
            names[a], names[b]
        ));
    }
    let base: Vec<Vec<usize>> = (0..count)
        .map(|s| (0..n).filter(|i| step[*i] == s).collect())
        .collect();
    if let Some(s) = base.iter().find(|s| s.len() > MAX_ACROSS) {
        return Err(format!(
            "diagram: {} boxes stand in one step ({}) and {MAX_ACROSS} fit; remove a box or split the diagram over two scenes",
            s.len(),
            s.iter()
                .map(|i| format!("\"{}\"", names[*i]))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    // Every order of every step (at most 6 each, 4 steps): the first with no two arrows crossed and the least
    // slant. Each step is centred, so the place of box k of m is k - (m - 1) / 2.
    let options: Vec<Vec<Vec<usize>>> = base.iter().map(|s| permutations(s)).collect();
    let mut best: Option<(f32, Vec<Vec<usize>>)> = None;
    let mut pick = vec![0usize; count];
    loop {
        let steps: Vec<Vec<usize>> = (0..count).map(|s| options[s][pick[s]].clone()).collect();
        let place = |i: usize| {
            let row = &steps[step[i]];
            row.iter().position(|x| *x == i).unwrap() as f32 - (row.len() as f32 - 1.0) / 2.0
        };
        let rank = |i: usize| steps[step[i]].iter().position(|x| *x == i).unwrap() as i32;
        let crossed = edges.iter().enumerate().any(|(k, e)| {
            edges[k + 1..].iter().any(|f| {
                step[e.0] == step[f.0] && (rank(e.0) - rank(f.0)) * (rank(e.1) - rank(f.1)) < 0
            })
        });
        if !crossed {
            let slant: f32 = edges.iter().map(|e| (place(e.0) - place(e.1)).abs()).sum();
            if best.as_ref().is_none_or(|b| slant < b.0 - 1e-6) {
                best = Some((slant, steps));
            }
        }
        // The next combination; the last step turns first, so the first step keeps the order of `nodes` when it can.
        let mut s = count;
        loop {
            if s == 0 {
                return match best {
                    Some((_, steps)) => Ok(Layout { steps, edges }),
                    None => Err(
                        "diagram: two arrows cross in every order of the boxes; remove an arrow, or split the diagram over two scenes"
                            .into(),
                    ),
                };
            }
            s -= 1;
            pick[s] += 1;
            if pick[s] < options[s].len() {
                break;
            }
            pick[s] = 0;
        }
    }
}

pub fn problem(c: &Card) -> Option<String> {
    layout(
        c.nodes.as_deref().unwrap_or(&[]),
        c.edges.as_deref().unwrap_or(&[]),
    )
    .err()
}

/// An arrow from `a` to `b` as one shape: a shaft and a head.
fn arrow(a: (f32, f32), b: (f32, f32), thick: f32) -> String {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let (ux, uy) = (dx / len, dy / len);
    let (nx, ny) = (-uy, ux);
    let head = (thick * 4.0).min(len * 0.6);
    let (hx, hy) = (b.0 - ux * head, b.1 - uy * head);
    let (t, w) = (thick / 2.0, thick * 1.8);
    let pts = [
        (a.0 + nx * t, a.1 + ny * t),
        (hx + nx * t, hy + ny * t),
        (hx + nx * w, hy + ny * w),
        b,
        (hx - nx * w, hy - ny * w),
        (hx - nx * t, hy - ny * t),
        (a.0 - nx * t, a.1 - ny * t),
    ];
    let mut s = format!("m {:.0} {:.0} l", pts[0].0, pts[0].1);
    for p in &pts[1..] {
        s += &format!(" {:.0} {:.0}", p.0, p.1);
    }
    s
}

pub(crate) fn events(c: &Card, a: &Area) -> Vec<String> {
    let nodes = c.nodes.as_deref().unwrap_or(&[]);
    let Ok(lay) = layout(nodes, c.edges.as_deref().unwrap_or(&[])) else {
        return vec![];
    };
    let count = lay.steps.len();
    let across = lay.steps.iter().map(|s| s.len()).max().unwrap_or(1);
    let head = c.label.as_deref().filter(|l| !l.trim().is_empty());
    let head_size = head.map_or(0, |l| a.fit(l, a.size * 9 / 10, a.w * 0.88));
    let head_h = a.head_height(head_size);
    let (left, avail) = (a.w * 0.05, a.w * 0.9);
    let room = a.bottom - a.top - head_h;
    // Along the steps and across them: a wide frame runs the steps sideways, a tall one downwards.
    let (along, cross, n_along) = if a.wide {
        (avail, room, count)
    } else {
        (room, avail, count)
    };
    // The text: as large as fits two lines in every box; the boxes all have one size.
    let mut s = a.size * 4 / 5;
    let (bw, bh, lines) = loop {
        let lh = s as f32 * (1.25 * a.latin).max(1.0);
        let bh2 = 2.0 * lh + s as f32 * 0.7;
        // The room for the arrows between two steps.
        let gap = (s as f32 * 1.6).max(a.w.min(a.h) * 0.045);
        let (bw, bh_max) = if a.wide {
            (
                ((along - (n_along as f32 - 1.0) * gap) / n_along as f32).min(avail * 0.3),
                (cross - (across as f32 - 1.0) * s as f32 * 0.6) / across as f32,
            )
        } else {
            (
                ((cross - (across as f32 - 1.0) * s as f32 * 0.6) / across as f32).min(avail * 0.6),
                (along - (n_along as f32 - 1.0) * gap) / n_along as f32,
            )
        };
        let inner = bw - s as f32 * 0.7;
        let wrapped: Vec<Vec<String>> = nodes
            .iter()
            .map(|t| crate::cards::balanced(a.wrap, t.trim(), s, inner))
            .collect();
        let fits = wrapped
            .iter()
            .all(|l| l.len() <= 2 && l.iter().all(|x| a.width(x, s) <= inner));
        let two = wrapped.iter().any(|l| l.len() > 1);
        let bh = if two { bh2 } else { lh + s as f32 * 0.7 };
        if (fits && bh <= bh_max) || s <= 6 {
            break (bw, bh, wrapped);
        }
        s -= 1;
    };
    let gap_along =
        (along - n_along as f32 * if a.wide { bw } else { bh }) / (n_along as f32 - 1.0).max(1.0);
    // Not more room between two steps than an arrow needs to read well.
    let gap_along = gap_along.min(if a.wide { bw * 0.6 } else { bh * 1.1 });
    let used = n_along as f32 * if a.wide { bw } else { bh } + (n_along as f32 - 1.0) * gap_along;
    let gap_cross = s as f32 * 0.6;
    // The centre of each box.
    let mut centre = vec![(0.0f32, 0.0f32); nodes.len()];
    for (si, row) in lay.steps.iter().enumerate() {
        let m = row.len() as f32;
        for (k, &i) in row.iter().enumerate() {
            let off = k as f32 - (m - 1.0) / 2.0;
            centre[i] = if a.wide {
                let from_start = (along - used) / 2.0 + bw / 2.0 + si as f32 * (bw + gap_along);
                let x = if a.rtl {
                    left + avail - from_start
                } else {
                    left + from_start
                };
                (x, a.top + head_h + room / 2.0 + off * (bh + gap_cross))
            } else {
                let off = if a.rtl { -off } else { off };
                (
                    a.w / 2.0 + off * (bw + gap_cross),
                    a.top
                        + head_h
                        + ((room - used) / 2.0).min(a.h * 0.05)
                        + bh / 2.0
                        + si as f32 * (bh + gap_along),
                )
            };
        }
    }
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
    // Each step comes after the one before it; an arrow comes with the box it points at.
    let pace = (0.45f64).min((a.end - a.start) * 0.5 / count as f64);
    let at = |si: usize| (a.start + 0.1 + si as f64 * pace).min(a.end - 0.4);
    let step_of = |i: usize| lay.steps.iter().position(|r| r.contains(&i)).unwrap_or(0);
    let thick = (s as f32 * 0.09).max(3.0);
    for &(p, q) in &lay.edges {
        let (cp, cq) = (centre[p], centre[q]);
        // From the side of the box that faces the next step to the facing side of the target, a little clear.
        let clear = thick * 1.2;
        let (from, to) = if a.wide {
            let dir = if cq.0 > cp.0 { 1.0 } else { -1.0 };
            (
                (cp.0 + dir * (bw / 2.0 + clear), cp.1),
                (cq.0 - dir * (bw / 2.0 + clear), cq.1),
            )
        } else {
            (
                (cp.0, cp.1 + bh / 2.0 + clear),
                (cq.0, cq.1 - bh / 2.0 - clear),
            )
        };
        out.push(line(
            at(step_of(q)),
            a.end,
            &format!(
                "\\an7\\pos(0,0)\\bord{:.1}\\shad0\\4a&HFF&\\3c&H000000&\\3a&H40&\\1c&HFFFFFF&\\fad(200,200)\\p1",
                thick * 0.5
            ),
            &arrow(from, to, thick),
        ));
    }
    for (i, (cx, cy)) in centre.iter().enumerate() {
        let t = at(step_of(i));
        out.push(line(
            t,
            a.end,
            &format!(
                "\\an7\\pos(0,0)\\bord{:.1}\\shad0\\4a&HFF&\\3c&HFFFFFF&\\3a&H70&\\1c{PANEL_LIT}\\1a&H10&\\fad(200,200)\\p1",
                (s as f32 * 0.04).max(1.5)
            ),
            &rect(cx - bw / 2.0, cy - bh / 2.0, cx + bw / 2.0, cy + bh / 2.0),
        ));
        out.push(line(
            t,
            a.end,
            &format!("\\an5\\pos({cx:.0},{cy:.0})\\fs{s}{PLAIN}\\fad(200,200)"),
            &lines[i]
                .iter()
                .map(|l| a.text(l, s as f32))
                .collect::<Vec<_>>()
                .join("\\N"),
        ));
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

    fn lay(nodes: &[&str], edges: &[(&str, &str)]) -> Result<Layout, String> {
        layout(
            &nodes.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &edges
                .iter()
                .map(|(a, b)| vec![a.to_string(), b.to_string()])
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn boxes_stand_in_steps_and_arrows_do_not_cross() {
        // A chain.
        let l = lay(&["A", "B", "C"], &[("A", "B"), ("B", "C")]).unwrap();
        assert_eq!(l.steps, [vec![0], vec![1], vec![2]]);
        // Two sources into one box, then two results: 2, 1, 2.
        let l = lay(
            &["Brief", "Media", "Render", "Video", "Report"],
            &[
                ("Brief", "Render"),
                ("Media", "Render"),
                ("Render", "Video"),
                ("Render", "Report"),
            ],
        )
        .unwrap();
        assert_eq!(l.steps, [vec![0, 1], vec![2], vec![3, 4]]);
        // The nodes are given in an order that crosses (A to Y, B to X): the second step is turned.
        let l = lay(&["A", "B", "X", "Y"], &[("A", "Y"), ("B", "X")]).unwrap();
        assert_eq!(l.steps, [vec![0, 1], vec![3, 2]]);
        // A box with nothing before it stands right before its target, not in the first step.
        let l = lay(
            &["A", "B", "C", "Late"],
            &[("A", "B"), ("B", "C"), ("Late", "C")],
        )
        .unwrap();
        assert_eq!(l.steps, [vec![0], vec![1, 3], vec![2]]);
    }

    #[test]
    fn a_graph_with_no_clean_drawing_is_refused_with_what_to_do() {
        let bad = |nodes: &[&str], edges: &[(&str, &str)], want: &str| {
            let e = lay(nodes, edges).unwrap_err();
            assert!(e.contains(want), "{e}");
        };
        bad(&["A"], &[], "2 to 8 box labels");
        bad(&["A", "B"], &[], "needs `edges`");
        bad(&["A", "A"], &[("A", "A")], "twice");
        bad(
            &["A", "B"],
            &[("A", "C")],
            "names \"C\", which is not in `nodes`",
        );
        bad(&["A", "B"], &[("A", "A"), ("A", "B")], "to itself");
        bad(&["A", "B"], &[("A", "B"), ("A", "B")], "there twice");
        bad(&["A", "B", "C"], &[("A", "B")], "\"C\" has no arrow");
        bad(
            &["A", "B", "C"],
            &[("A", "B"), ("B", "C"), ("C", "A")],
            "in a circle",
        );
        bad(
            &["A", "B", "C"],
            &[("A", "B"), ("B", "C"), ("A", "C")],
            "jump over a step",
        );
        bad(
            &["A", "B", "C", "D", "E"],
            &[("A", "B"), ("B", "C"), ("C", "D"), ("D", "E")],
            "5 steps in a row and 4 fit",
        );
        bad(
            &["A", "B", "C", "D", "E"],
            &[("A", "B"), ("A", "C"), ("A", "D"), ("A", "E")],
            "4 boxes stand in one step",
        );
        // Two boxes each to both boxes of the next step: the arrows cross in every order.
        bad(
            &["A", "B", "X", "Y"],
            &[("A", "X"), ("A", "Y"), ("B", "X"), ("B", "Y")],
            "cross in every order",
        );
        bad(
            &["A label that is far too long", "B"],
            &[("A label that is far too long", "B")],
            "1 to 18 characters",
        );
        // Through the card: a field it does not draw, and its fields on another kind.
        let p = |v: serde_json::Value| card(v).problem().unwrap_or_default();
        assert!(p(
            json!({"kind": "diagram", "nodes": ["A", "B"], "edges": [["A", "B"]], "items": ["x"]})
        )
        .contains("does not draw `items`"));
        assert!(
            p(json!({"kind": "diagram", "nodes": ["A", "B"], "edges": [["A"]]}))
                .contains("must be two labels")
        );
        assert!(
            p(json!({"kind": "list", "items": ["x"], "nodes": ["A", "B"]}))
                .contains("only a diagram card takes `nodes`")
        );
        assert!(
            p(json!({"kind": "list", "items": ["x"], "edges": [["A", "B"]]}))
                .contains("only a diagram card takes `edges`")
        );
        assert_eq!(
            p(
                json!({"kind": "diagram", "nodes": ["A", "B"], "edges": [["A", "B"]], "label": "Flow"})
            ),
            ""
        );
    }

    fn worst(ar: bool) -> Vec<Card> {
        let n: Vec<&str> = if ar {
            vec![
                "الملخص المكتوب هنا",
                "الصور والمقاطع",
                "الصوت والموسيقى",
                "خطة العمل الأولى",
                "التركيب النهائي",
                "الفحص والمراجعة",
                "الفيديو الجاهز",
                "التقرير المفصل",
            ]
        } else {
            vec![
                "The written brief",
                "Pictures and clips",
                "Voice and music",
                "A plan with costs",
                "The final render",
                "Checks and gates",
                "The finished video",
                "A detailed report",
            ]
        };
        let e = |list: &[(usize, usize)]| -> Vec<Vec<&str>> {
            list.iter().map(|(a, b)| vec![n[*a], n[*b]]).collect()
        };
        let label = if ar {
            "كيف يُصنع الفيديو من الملخص"
        } else {
            "How a video is made from a brief"
        };
        vec![
            // 3, 3, 2 boxes.
            card(
                json!({"kind": "diagram", "label": label, "nodes": n, "edges": e(&[(0, 3), (1, 4), (2, 5), (3, 6), (4, 6), (5, 7)])}),
            ),
            // 2, 2, 2, 2 boxes: two chains of four.
            card(
                json!({"kind": "diagram", "label": label, "nodes": n, "edges": e(&[(0, 2), (2, 4), (4, 6), (1, 3), (3, 5), (5, 7)])}),
            ),
            // A fan: 1, 3, 1.
            card(
                json!({"kind": "diagram", "nodes": n[..5], "edges": e(&[(0, 1), (0, 2), (0, 3), (1, 4), (2, 4), (3, 4)])}),
            ),
        ]
    }

    #[test]
    fn every_diagram_stays_in_its_room_and_no_box_touches_another() {
        for (lang, cards) in [("en", worst(false)), ("ar", worst(true))] {
            for c in cards {
                assert!(c.problem().is_none(), "{:?}", c.problem());
                for frame in FRAMES {
                    let ev = events(&c, lang, frame, &[]);
                    assert_inside(&ev, lang, frame, "diagram");
                    let b = boxes(&ev, lang);
                    // Boxes are the drawings of 4 points; each text is inside its own box.
                    let rects: Vec<_> = ev
                        .iter()
                        .zip(&b)
                        .filter(|(e, _)| e.contains(PANEL_LIT))
                        .map(|(_, b)| b.clone())
                        .collect();
                    let texts: Vec<_> = ev
                        .iter()
                        .zip(&b)
                        .filter(|(e, _)| e.contains("\\an5"))
                        .map(|(_, b)| b.clone())
                        .collect();
                    assert_eq!(rects.len(), c.nodes.as_ref().unwrap().len());
                    assert_eq!(rects.len(), texts.len());
                    for (i, (r, t)) in rects.iter().zip(&texts).enumerate() {
                        assert!(
                            t.0 >= r.0 && t.2 <= r.2 && t.1 >= r.1 - 1.0 && t.3 <= r.3 + 1.0,
                            "{lang} {frame:?}: {t:?} in {r:?}"
                        );
                        for q in &rects[i + 1..] {
                            assert!(
                                r.2 < q.0 || q.2 < r.0 || r.3 < q.1 || q.3 < r.1,
                                "{lang} {frame:?}: {r:?} on {q:?}"
                            );
                        }
                    }
                    // The letters: 40 percent of the caption size or more.
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
                        fs >= crate::card_kit::testkit::size(lang, frame) as f32 * 0.4,
                        "{lang} {frame:?}: {fs}"
                    );
                }
            }
        }
    }

    #[test]
    fn steps_run_sideways_on_a_wide_frame_down_on_a_tall_one_and_arabic_mirrors() {
        let c = card(
            json!({"kind": "diagram", "nodes": ["Brief", "Plan", "Video"], "edges": [["Brief", "Plan"], ["Plan", "Video"]]}),
        );
        let x = |ev: &[String], t: &str| -> (f32, f32) {
            let e = ev.iter().find(|e| e.ends_with(t)).unwrap();
            let p: Vec<f32> = e
                .split("\\pos(")
                .nth(1)
                .unwrap()
                .split(')')
                .next()
                .unwrap()
                .split(',')
                .map(|v| v.parse().unwrap())
                .collect();
            (p[0], p[1])
        };
        let ev = events(&c, "en", (1920, 1080), &[]);
        assert!(x(&ev, "Brief").0 < x(&ev, "Plan").0 && x(&ev, "Plan").0 < x(&ev, "Video").0);
        assert_eq!(x(&ev, "Brief").1, x(&ev, "Video").1);
        // Two arrows, each a shape of 7 points, the first one pointing right; each comes with its target.
        let arrows: Vec<&String> = ev.iter().filter(|e| e.contains("\\3a&H40&")).collect();
        assert_eq!(arrows.len(), 2);
        let pts: Vec<f32> = arrows[0]
            .rsplit('}')
            .next()
            .unwrap()
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        assert_eq!(pts.len(), 14);
        assert!(
            pts[6] > pts[0] + 20.0,
            "the tip is right of the tail: {pts:?}"
        );
        assert!(pts[0] > x(&ev, "Brief").0 && pts[6] < x(&ev, "Plan").0);
        let secs = |e: &str| e.split(',').nth(1).unwrap().to_string();
        assert_eq!(
            secs(arrows[1]),
            secs(ev.iter().find(|e| e.ends_with("Video")).unwrap())
        );
        assert!(secs(arrows[0]) < secs(arrows[1]));
        let ev = events(&c, "en", (1080, 1920), &[]);
        assert!(x(&ev, "Brief").1 < x(&ev, "Plan").1 && x(&ev, "Plan").1 < x(&ev, "Video").1);
        assert_eq!(x(&ev, "Brief").0, 540.0);
        // Arabic on a wide frame: the first box at the right, the arrow points left.
        let c = card(
            json!({"kind": "diagram", "nodes": ["الملخص", "الخطة", "الفيديو"], "edges": [["الملخص", "الخطة"], ["الخطة", "الفيديو"]]}),
        );
        let ev = events(&c, "ar", (1920, 1080), &[]);
        assert!(x(&ev, "الملخص").0 > x(&ev, "الخطة").0 && x(&ev, "الخطة").0 > x(&ev, "الفيديو").0);
        let a0 = ev.iter().find(|e| e.contains("\\3a&H40&")).unwrap();
        let pts: Vec<f32> = a0
            .rsplit('}')
            .next()
            .unwrap()
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        assert!(pts[6] < pts[0] - 20.0, "{pts:?}");
    }
}
