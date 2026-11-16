//! The `formula` card: a small TeX-like math subset typeset by ScriptVox itself, with no LaTeX install (the top
//! complaint of Manim users). Superscripts, subscripts, fractions and square roots are laid out as boxes; every
//! piece becomes its own positioned ASS event in Noto Sans Math (OFL), so libass only draws glyphs and lines.
//!
//! Units: `em` is the formula's em in pixels. In Noto Sans Math the ASS font size maps to win ascent plus win
//! descent (5655 units for a 1000-unit em, the same mapping as `captions::Measurer`), so the font size is
//! `em / EM_PER_FS` and an `\an1` line box ends `WIN_DESCENT` em under the baseline.

pub const FAMILY: &str = "Noto Sans Math";
const EM_PER_FS: f32 = 1000.0 / 5655.0;
const WIN_DESCENT: f32 = 2.55;
// Box heights of a text run, em: cap height 0.714 (letters a little over), descenders about 0.22.
const ASC: f32 = 0.75;
const DESC: f32 = 0.22;
// From the font's MATH table (em): axis height, rule thickness, script scale and shifts.
const AXIS: f32 = 0.278;
const RULE: f32 = 0.071;
const SCRIPT: f32 = 0.6;
const SUP_UP: f32 = 0.39;
const SUB_DOWN: f32 = 0.21;
/// A fraction's numerator and denominator, as a share of the size around them (display style, a little smaller).
const FRAC: f32 = 0.85;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Class {
    Ord,
    Bin,
    Rel,
    Punct,
    /// sin, cos, log: upright, a thin space after.
    Op,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Sym(String, Class),
    Space(f32),
    Row(Vec<Node>),
    Scripts(Box<Node>, Option<Box<Node>>, Option<Box<Node>>),
    Frac(Box<Node>, Box<Node>),
    Sqrt(Box<Node>),
    /// A bigger symbol (sum, integral).
    Big(String),
}

const MAX_CHARS: usize = 120;

fn symbol(name: &str) -> Option<(&'static str, Class)> {
    use Class::*;
    Some(match name {
        "times" => ("×", Bin),
        "div" => ("÷", Bin),
        "pm" => ("±", Bin),
        "mp" => ("∓", Bin),
        "cdot" => ("·", Bin),
        "le" | "leq" => ("≤", Rel),
        "ge" | "geq" => ("≥", Rel),
        "ne" | "neq" => ("≠", Rel),
        "approx" => ("≈", Rel),
        "propto" => ("∝", Rel),
        "to" | "rightarrow" => ("→", Rel),
        "Rightarrow" => ("⇒", Rel),
        "in" => ("∈", Rel),
        "infty" => ("∞", Ord),
        "partial" => ("∂", Ord),
        "nabla" => ("∇", Ord),
        "degree" | "circ" => ("°", Ord),
        "%" => ("%", Ord),
        _ => return greek(name).map(|g| (g, Ord)),
    })
}

fn greek(name: &str) -> Option<&'static str> {
    // Lower case in math italic (as TeX sets them), capitals upright.
    Some(match name {
        "alpha" => "𝛼",
        "beta" => "𝛽",
        "gamma" => "𝛾",
        "delta" => "𝛿",
        "epsilon" => "𝜀",
        "theta" => "𝜃",
        "lambda" => "𝜆",
        "mu" => "𝜇",
        "pi" => "𝜋",
        "rho" => "𝜌",
        "sigma" => "𝜎",
        "tau" => "𝜏",
        "phi" => "𝜑",
        "omega" => "𝜔",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Phi" => "Φ",
        "Omega" => "Ω",
        _ => return None,
    })
}

const FUNCTIONS: [&str; 10] = [
    "sin", "cos", "tan", "log", "ln", "exp", "lim", "max", "min", "mod",
];

/// A Latin letter in math italic (U+1D434 on; italic h is U+210E).
fn italic(c: char) -> char {
    match c {
        'h' => 'ℎ',
        'a'..='z' => char::from_u32(0x1D44E + (c as u32 - 'a' as u32)).unwrap_or(c),
        'A'..='Z' => char::from_u32(0x1D434 + (c as u32 - 'A' as u32)).unwrap_or(c),
        _ => c,
    }
}

const HELP: &str = "Use letters, digits, + - * / = < > ( ) , ^ _ { }, and \\frac{a}{b}, \\sqrt{x}, \\text{words}, \\times, \\div, \\pm, \\cdot, \\le, \\ge, \\ne, \\approx, \\to, \\infty, \\sum, \\int, Greek names (\\pi, \\theta, \\Delta), \\sin, \\log.";

struct Parser<'a> {
    s: &'a [char],
    i: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn row(&mut self, until: Option<char>) -> Result<Node, String> {
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None if until.is_some() => return Err("a { has no matching }".into()),
                None => break,
                Some(c) if Some(c) == until => {
                    self.i += 1;
                    break;
                }
                Some('}') => return Err("a } has no matching {".into()),
                Some(' ') => self.i += 1,
                Some('^') | Some('_') => {
                    let base = items.pop().unwrap_or(Node::Row(vec![]));
                    let (mut sup, mut sub) = match base {
                        Node::Scripts(b, p, q) => {
                            items.push(*b);
                            (p, q)
                        }
                        b => {
                            items.push(b);
                            (None, None)
                        }
                    };
                    while let Some(c @ ('^' | '_')) = self.peek() {
                        self.i += 1;
                        let g = Box::new(self.group()?);
                        let slot = if c == '^' { &mut sup } else { &mut sub };
                        if slot.is_some() {
                            return Err(format!("two {c} on one symbol: use braces"));
                        }
                        *slot = Some(g);
                    }
                    let b = items.pop().unwrap();
                    items.push(Node::Scripts(Box::new(b), sup, sub));
                }
                Some(_) => {
                    let a = self.atom()?;
                    items.push(a);
                }
            }
        }
        Ok(Node::Row(items))
    }

    /// One argument: `{...}`, a command, or one character.
    fn group(&mut self) -> Result<Node, String> {
        while self.peek() == Some(' ') {
            self.i += 1;
        }
        match self.peek() {
            Some('{') => {
                self.i += 1;
                self.depth += 1;
                if self.depth > 4 {
                    return Err("nested more than 4 levels deep".into());
                }
                let r = self.row(Some('}'));
                self.depth -= 1;
                r
            }
            Some(_) => self.atom(),
            None => Err("^, _, \\frac or \\sqrt at the end needs an argument".into()),
        }
    }

    fn atom(&mut self) -> Result<Node, String> {
        let c = self.peek().ok_or("unexpected end")?;
        self.i += 1;
        use Class::*;
        Ok(match c {
            '{' => {
                self.i -= 1;
                self.group()?
            }
            '\\' => {
                let start = self.i;
                while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    self.i += 1;
                }
                if self.i == start {
                    // One-character commands: spacing and a literal percent.
                    let k = self.peek().ok_or("a \\ at the end")?;
                    self.i += 1;
                    return match k {
                        ',' => Ok(Node::Space(0.17)),
                        ';' => Ok(Node::Space(0.28)),
                        ' ' => Ok(Node::Space(0.25)),
                        '%' => Ok(Node::Sym("%".into(), Ord)),
                        _ => Err(format!("unknown command \\{k}. {HELP}")),
                    };
                }
                let name: String = self.s[start..self.i].iter().collect();
                match name.as_str() {
                    "frac" => {
                        let a = self.group()?;
                        let b = self.group()?;
                        Node::Frac(Box::new(a), Box::new(b))
                    }
                    "sqrt" => Node::Sqrt(Box::new(self.group()?)),
                    "text" | "mathrm" => {
                        if self.peek() != Some('{') {
                            return Err(format!("\\{name} needs {{words}}"));
                        }
                        self.i += 1;
                        let start = self.i;
                        while self.peek().is_some_and(|c| c != '}') {
                            self.i += 1;
                        }
                        if self.peek().is_none() {
                            return Err("a { has no matching }".into());
                        }
                        let t: String = self.s[start..self.i].iter().collect();
                        self.i += 1;
                        Node::Sym(t, Ord)
                    }
                    "sum" => Node::Big("∑".into()),
                    "prod" => Node::Big("∏".into()),
                    "int" => Node::Big("∫".into()),
                    n if FUNCTIONS.contains(&n) => Node::Sym(n.into(), Op),
                    n => match symbol(n) {
                        Some((g, class)) => Node::Sym(g.into(), class),
                        None => return Err(format!("unknown command \\{n}. {HELP}")),
                    },
                }
            }
            'a'..='z' | 'A'..='Z' => Node::Sym(italic(c).to_string(), Ord),
            '0'..='9' | '.' => {
                // A number stays one run: 3.14 is not three symbols.
                let start = self.i - 1;
                while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                    self.i += 1;
                }
                Node::Sym(self.s[start..self.i].iter().collect(), Ord)
            }
            '+' => Node::Sym("+".into(), Bin),
            '-' => Node::Sym("−".into(), Bin),
            '*' => Node::Sym("·".into(), Bin),
            '=' | '<' | '>' => Node::Sym(c.to_string(), Rel),
            ',' | ';' => Node::Sym(c.to_string(), Punct),
            '(' | ')' | '[' | ']' | '/' | '!' | '|' | ':' => Node::Sym(c.to_string(), Ord),
            '\'' => Node::Sym("′".into(), Ord),
            _ => return Err(format!("the character {c:?} is not supported. {HELP}")),
        })
    }
}

fn parse(text: &str) -> Result<Node, String> {
    if text.trim().is_empty() {
        return Err("a formula card needs text, for example \"E = mc^2\"".into());
    }
    if text.chars().count() > MAX_CHARS {
        return Err(format!("a formula takes at most {MAX_CHARS} characters"));
    }
    let s: Vec<char> = text.chars().collect();
    Parser {
        s: &s,
        i: 0,
        depth: 0,
    }
    .row(None)
}

/// The brief check: why this formula cannot be typeset.
pub fn problem(text: &str) -> Option<String> {
    parse(text).err().map(|e| format!("formula: {e}"))
}

/// How many parts `layout` gives for a node. A node's parts are one run, in the order of the source: a row's items
/// left to right, a base then its scripts, a fraction's top, bottom and bar, a root's body and sign.
fn count(n: &Node) -> usize {
    let opt = |s: &Option<Box<Node>>| s.as_deref().map_or(0, count);
    match n {
        Node::Sym(..) | Node::Big(_) => 1,
        Node::Space(_) => 0,
        Node::Row(items) => items.iter().map(count).sum(),
        Node::Scripts(b, p, q) => count(b) + opt(p) + opt(q),
        Node::Frac(a, b) => count(a) + count(b) + 1,
        Node::Sqrt(b) => count(b) + 1,
    }
}

/// Every place `pat` stands in `n` as a run of neighbours in one row, as part index ranges. `at` is the index of
/// `n`'s first part; `alone` is true for an argument that is not a row (the `2` of `x^2`), which is a row of one.
fn runs(n: &Node, pat: &[Node], alone: bool, at: usize, out: &mut Vec<std::ops::Range<usize>>) {
    match n {
        Node::Row(items) => {
            let mut offs = vec![at];
            for it in items {
                offs.push(offs[offs.len() - 1] + count(it));
            }
            for (k, w) in items.windows(pat.len()).enumerate() {
                if w == pat {
                    out.push(offs[k]..offs[k + pat.len()]);
                }
            }
            for (k, it) in items.iter().enumerate() {
                runs(it, pat, false, offs[k], out);
            }
        }
        _ => {
            if alone && pat.len() == 1 && pat[0] == *n {
                out.push(at..at + count(n));
            }
            let mut at = at;
            let kids: Vec<&Node> = match n {
                Node::Scripts(b, p, q) => std::iter::once(&**b)
                    .chain(p.as_deref())
                    .chain(q.as_deref())
                    .collect(),
                Node::Frac(a, b) => vec![a, b],
                Node::Sqrt(b) => vec![b],
                _ => vec![],
            };
            for k in kids {
                runs(k, pat, true, at, out);
                at += count(k);
            }
        }
    }
}

/// Where a piece of the formula's source sits in the parts `typeset` gives: the index range of its parts. `part`
/// is read by the same parser and must stand in the formula exactly once, as neighbours in one row. The error
/// completes the sentence "the part ...".
pub fn find(text: &str, part: &str) -> Result<std::ops::Range<usize>, String> {
    let tree = parse(text)?;
    if part.trim().is_empty() {
        return Err("is empty".into());
    }
    let mut pat = match parse(part)? {
        Node::Row(items) => items,
        n => vec![n],
    };
    // `{mc^2}` is the same piece as `mc^2`.
    while let [Node::Row(inner)] = pat.as_slice() {
        pat = inner.clone();
    }
    if pat.iter().map(count).sum::<usize>() == 0 {
        return Err("has nothing to colour".into());
    }
    let mut found = Vec::new();
    runs(&tree, &pat, true, 0, &mut found);
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(format!(
            "is not a piece of the formula {text:?}: write it as the formula does, a symbol with its ^ and _ (`c^2`, not `mc`, in `mc^2`)"
        )),
        n => Err(format!(
            "is in the formula {text:?} {n} times: write a longer piece that is there once"
        )),
    }
}

/// A placed piece, in em-relative pixels: x from the left, y from the baseline (down is positive).
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text {
        x: f32,
        y: f32,
        em: f32,
        text: String,
    },
    /// A stroke through these points, `t` thick.
    Stroke { pts: Vec<(f32, f32)>, t: f32 },
}

#[derive(Debug, Default)]
struct Boxed {
    w: f32,
    asc: f32,
    desc: f32,
    parts: Vec<Part>,
}

impl Boxed {
    fn put(&mut self, b: Boxed, dx: f32, dy: f32) {
        for p in b.parts {
            self.parts.push(match p {
                Part::Text { x, y, em, text } => Part::Text {
                    x: x + dx,
                    y: y + dy,
                    em,
                    text,
                },
                Part::Stroke { pts, t } => Part::Stroke {
                    pts: pts.iter().map(|(x, y)| (x + dx, y + dy)).collect(),
                    t,
                },
            });
        }
        self.asc = self.asc.max(b.asc - dy);
        self.desc = self.desc.max(b.desc + dy);
    }
}

fn class_of(n: &Node) -> Class {
    match n {
        Node::Sym(_, c) => *c,
        Node::Scripts(b, _, _) => class_of(b),
        _ => Class::Ord,
    }
}

fn layout(n: &Node, em: f32, measure: &dyn Fn(&str, f32) -> f32) -> Boxed {
    match n {
        Node::Sym(t, _) | Node::Big(t) => {
            let e = if matches!(n, Node::Big(_)) {
                em * 1.4
            } else {
                em
            };
            let w = measure(t, e);
            Boxed {
                w,
                asc: ASC * e,
                desc: DESC * e,
                parts: vec![Part::Text {
                    x: 0.0,
                    y: 0.0,
                    em: e,
                    text: t.clone(),
                }],
            }
        }
        Node::Space(s) => Boxed {
            w: s * em,
            ..Default::default()
        },
        Node::Row(items) => {
            let mut out = Boxed::default();
            for (k, it) in items.iter().enumerate() {
                let c = class_of(it);
                // A + or − that starts a row (or follows a relation) is a sign, not an operation: no spaces.
                let unary = c == Class::Bin
                    && (k == 0
                        || matches!(
                            class_of(&items[k - 1]),
                            Class::Rel | Class::Bin | Class::Punct
                        ));
                let (before, after) = match c {
                    Class::Rel => (0.28, 0.28),
                    Class::Bin if !unary => (0.22, 0.22),
                    Class::Punct => (0.0, 0.17),
                    Class::Op => (0.0, 0.17),
                    _ => (0.0, 0.0),
                };
                out.w += before * em;
                let b = layout(it, em, measure);
                let x = out.w;
                out.w += b.w;
                out.put(b, x, 0.0);
                out.w += after * em;
            }
            out
        }
        Node::Scripts(base, sup, sub) => {
            let b = layout(base, em, measure);
            let mut out = Boxed {
                w: b.w,
                ..Default::default()
            };
            let (bw, basc, bdesc) = (b.w, b.asc, b.desc);
            out.put(b, 0.0, 0.0);
            let mut right = bw;
            let gap = 0.05 * em;
            if let Some(s) = sup {
                let s = layout(s, em * SCRIPT, measure);
                let up = (SUP_UP * em).max(basc - 0.55 * em);
                right = right.max(bw + gap + s.w);
                out.put(s, bw + gap, -up);
            }
            if let Some(s) = sub {
                let s = layout(s, em * SCRIPT, measure);
                let down = (SUB_DOWN * em).max(bdesc - 0.1 * em);
                right = right.max(bw + gap + s.w);
                out.put(s, bw + gap, down);
            }
            out.w = right;
            out
        }
        Node::Frac(a, b) => {
            let (na, nb) = (layout(a, em * FRAC, measure), layout(b, em * FRAC, measure));
            let pad = 0.12 * em;
            let w = na.w.max(nb.w) + 2.0 * pad;
            let gap = 0.16 * em;
            let mut out = Boxed {
                w,
                ..Default::default()
            };
            let up = AXIS * em + RULE * em / 2.0 + gap + na.desc;
            let down = -AXIS * em + RULE * em / 2.0 + gap + nb.asc;
            let (wa, wb) = (na.w, nb.w);
            out.put(na, (w - wa) / 2.0, -up);
            out.put(nb, (w - wb) / 2.0, down);
            out.parts.push(Part::Stroke {
                pts: vec![(0.04 * em, -AXIS * em), (w - 0.04 * em, -AXIS * em)],
                t: RULE * em,
            });
            out
        }
        Node::Sqrt(body) => {
            let b = layout(body, em, measure);
            let gap = 0.14 * em;
            let sign = 0.55 * em;
            let top = -(b.asc + gap);
            let w = sign + b.w + 0.1 * em;
            let mut out = Boxed {
                w,
                ..Default::default()
            };
            let bdesc = b.desc;
            out.put(b, sign + 0.04 * em, 0.0);
            out.parts.push(Part::Stroke {
                pts: vec![
                    (0.02 * em, -0.25 * em),
                    (0.13 * em, -0.32 * em),
                    (0.27 * em, bdesc),
                    (sign, top),
                    (w, top),
                ],
                t: RULE * em,
            });
            out.asc = out.asc.max(-top + RULE * em);
            out
        }
    }
}

/// The formula laid out at `em`: its parts, width, ascent and descent.
pub fn typeset(
    text: &str,
    em: f32,
    measure: &dyn Fn(&str, f32) -> f32,
) -> Result<(Vec<Part>, f32, f32, f32), String> {
    let b = layout(&parse(text)?, em, measure);
    Ok((b.parts, b.w, b.asc, b.desc))
}

/// The ASS font size for an em of `em` pixels.
pub fn font_size(em: f32) -> f32 {
    em / EM_PER_FS
}

/// ASS override tags and text for one part at origin (`ox`, `oy` = baseline), before the shared animation tags.
pub fn ass_part(p: &Part, ox: f32, oy: f32) -> (String, String) {
    match p {
        Part::Text { x, y, em, text } => (
            format!(
                "\\an1\\pos({:.1},{:.1})\\fn{FAMILY}\\fs{:.0}\\b0\\bord0\\shad0\\4a&HFF&",
                ox + x,
                oy + y + WIN_DESCENT * em,
                font_size(*em)
            ),
            text.clone(),
        ),
        Part::Stroke { pts, t } => {
            // Each segment as a quad, joined with a square cap so corners close.
            let mut d = String::new();
            for w in pts.windows(2) {
                let ((x0, y0), (x1, y1)) = (w[0], w[1]);
                let (dx, dy) = (x1 - x0, y1 - y0);
                let len = (dx * dx + dy * dy).sqrt().max(0.01);
                let (ux, uy) = (dx / len * t / 2.0, dy / len * t / 2.0);
                let (nx, ny) = (-uy, ux);
                let pt = |x: f32, y: f32| format!("{:.1} {:.1}", ox + x, oy + y);
                d.push_str(&format!(
                    "m {} l {} l {} l {} ",
                    pt(x0 - ux + nx, y0 - uy + ny),
                    pt(x1 + ux + nx, y1 + uy + ny),
                    pt(x1 + ux - nx, y1 + uy - ny),
                    pt(x0 - ux - nx, y0 - uy - ny)
                ));
            }
            (
                "\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c&HFFFFFF&\\p1".into(),
                d.trim_end().to_string(),
            )
        }
    }
}

/// One glyph of a typeset formula: the character, its x and baseline y, its em. A text part is split into its
/// characters at the shaped width of each prefix, so two formulas can be matched glyph by glyph.
pub fn glyphs(parts: &[Part], measure: &dyn Fn(&str, f32) -> f32) -> Vec<(char, f32, f32, f32)> {
    let mut out = Vec::new();
    for p in parts {
        if let Part::Text { x, y, em, text } = p {
            for (i, c) in text.char_indices() {
                if !c.is_whitespace() {
                    out.push((c, x + measure(&text[..i], *em), *y, *em));
                }
            }
        }
    }
    out
}

/// The glyphs two formulas share, in order (longest common subsequence): pairs of indices into `a` and `b`.
pub fn pair(a: &[char], b: &[char]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut t = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a[i] == b[j] {
                t[i + 1][j + 1] + 1
            } else {
                t[i + 1][j].max(t[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((i, j));
            i += 1;
            j += 1;
        } else if t[i + 1][j] >= t[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// The `\an1` anchor of a glyph at `x`, `y` (baseline) and `em`, with the formula origin at `ox`, `oy`.
pub fn anchor(x: f32, y: f32, em: f32, ox: f32, oy: f32) -> (f32, f32) {
    (ox + x, oy + y + WIN_DESCENT * em)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every character half an em wide.
    fn m(t: &str, em: f32) -> f32 {
        t.chars().count() as f32 * em * 0.5
    }

    #[test]
    fn two_formulas_share_their_glyphs_in_order() {
        let a: Vec<char> = "A=πr2".chars().collect();
        let b: Vec<char> = "A=π(2r)2=4πr2".chars().collect();
        let p = pair(&a, &b);
        assert_eq!(p.len(), 5);
        assert_eq!(p[0], (0, 0));
        let set = typeset("A = \\pi r^2", 10.0, &m).unwrap();
        let g = glyphs(&set.0, &m);
        assert_eq!(g.iter().map(|g| g.0).filter(|c| *c == '2').count(), 1);
        assert!(
            g.windows(2).all(|w| w[0].1 <= w[1].1),
            "left to right: {g:?}"
        );
    }

    #[test]
    fn a_piece_of_the_source_is_found_as_a_run_of_parts() {
        // Parts of `E = mc^2`: E, =, m, c, 2.
        assert_eq!(find("E = mc^2", "mc^2"), Ok(2..5));
        assert_eq!(find("E = mc^2", "c^2"), Ok(3..5));
        assert_eq!(find("E=mc^{2}", " c "), Ok(3..4));
        assert_eq!(find("E = mc^2", "2"), Ok(4..5));
        assert_eq!(find("E = mc^2", "E ="), Ok(0..2));
        assert_eq!(find("E = mc^2", "{mc^2}"), Ok(2..5));
        // A fraction is its top, its bottom and its bar; a root its body and its sign.
        let text = "x = \\frac{a + 1}{b} + \\sqrt{y}";
        assert_eq!(find(text, "\\frac{a + 1}{b}"), Ok(2..7));
        assert_eq!(find(text, "a + 1"), Ok(2..5));
        assert_eq!(find(text, "b"), Ok(5..6));
        assert_eq!(find(text, "\\sqrt{y}"), Ok(8..10));
        // Every subtree's parts are one run: the count is the number of parts `typeset` gives.
        for t in [text, "E = mc^2", "x_{i}^{2} \\, \\sum \\text{area}"] {
            let n = typeset(t, 10.0, &m).unwrap().0.len();
            assert_eq!(find(t, t), Ok(0..n), "{t}");
        }
        // Not there, there more than once, or nothing to colour.
        assert!(find("E = mc^2", "mc").unwrap_err().contains("not a piece"));
        assert!(find("E = mc^2", "x").unwrap_err().contains("not a piece"));
        assert!(find("a + a", "a").unwrap_err().contains("2 times"));
        assert!(find("x^2 + 2", "2").unwrap_err().contains("2 times"));
        assert_eq!(find("x^2 + y", "2"), Ok(1..2));
        assert!(find("a \\, b", "\\,").is_err());
        assert!(find("a + b", "").is_err());
        assert!(find("a + b", "\\nope")
            .unwrap_err()
            .contains("unknown command"));
    }

    #[test]
    fn letters_turn_italic_and_operators_get_space() {
        let (parts, w, _, _) = typeset("E = mc^2", 100.0, &m).unwrap();
        let texts: Vec<&str> = parts
            .iter()
            .filter_map(|p| match p {
                Part::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["𝐸", "=", "𝑚", "𝑐", "2"]);
        // E 50, = 28+50+28, m 50, c 50, gap 5, 2 at 0.6 em = 30.
        assert!((w - 291.0).abs() < 0.5, "{w}");
        let two = parts
            .iter()
            .find(|p| matches!(p, Part::Text { text, .. } if text == "2"))
            .unwrap();
        assert!(
            matches!(two, Part::Text { y, em, .. } if (*y + 39.0).abs() < 0.1 && (*em - 60.0).abs() < 0.1)
        );
    }

    #[test]
    fn a_fraction_stacks_over_a_rule_on_the_axis() {
        let (parts, w, asc, desc) = typeset("\\frac{a}{b}", 100.0, &m).unwrap();
        let ys: Vec<f32> = parts
            .iter()
            .filter_map(|p| match p {
                Part::Text { y, .. } => Some(*y),
                _ => None,
            })
            .collect();
        assert!(ys[0] < -AXIS * 100.0 && ys[1] > 0.0, "{ys:?}");
        assert!(parts.iter().any(|p| matches!(p, Part::Stroke { pts, .. } if pts.len() == 2 && (pts[0].1 + 27.8).abs() < 0.1)));
        assert!(w > 42.5 && asc > 80.0 && desc > 20.0, "{w} {asc} {desc}");
    }

    #[test]
    fn signs_numbers_roots_and_functions() {
        let (p, _, _, _) = typeset("-3.14 + \\sqrt{x} \\le \\sin \\theta", 100.0, &m).unwrap();
        let texts: Vec<String> = p
            .iter()
            .filter_map(|p| match p {
                Part::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["−", "3.14", "+", "𝑥", "≤", "sin", "𝜃"]);
        assert_eq!(
            p.iter()
                .filter(|p| matches!(p, Part::Stroke { .. }))
                .count(),
            1
        );
        // The leading minus is a sign: the number follows it with no space.
        let x: Vec<f32> = p
            .iter()
            .filter_map(|p| match p {
                Part::Text { x, .. } => Some(*x),
                _ => None,
            })
            .collect();
        assert!((x[1] - 50.0).abs() < 0.1, "{x:?}");
    }

    #[test]
    fn mistakes_are_named_with_the_fix() {
        assert!(problem("x^").unwrap().contains("needs an argument"));
        assert!(problem("\\frac{a}{b").unwrap().contains("no matching }"));
        assert!(problem("\\foo").unwrap().contains("unknown command \\foo"));
        assert!(problem("x^2^3").unwrap().contains("two ^"));
        assert!(problem("a & b").unwrap().contains("not supported"));
        assert!(problem("x_{i}^{2} = \\text{area} \\times \\pi r^2").is_none());
        assert!(problem("").is_some());
    }

    #[test]
    fn a_text_part_sits_on_its_baseline() {
        let (tags, text) = ass_part(
            &Part::Text {
                x: 10.0,
                y: -5.0,
                em: 200.0,
                text: "𝑥".into(),
            },
            500.0,
            800.0,
        );
        assert_eq!(text, "𝑥");
        assert!(
            tags.starts_with("\\an1\\pos(510.0,1305.0)\\fnNoto Sans Math\\fs1131"),
            "{tags}"
        );
    }
}
