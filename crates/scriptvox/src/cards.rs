//! Scene cards: motion text drawn over a scene, in the same ASS file as the captions (one libass pass, no browser).
//! Kinds: `title` (a section headline that pops in), `stat` (a number that counts up, with a label), `lower_third`
//! (a name and a role that slide in), `list` (up to 5 points that appear one by one), charts, `formula`, `code`, `plot`,
//! `quote`, `compare`, `donut` and `timeline`, and the data cards of `card_kit.rs` (`table`, `number_line`, `diagram`,
//! `steps`, `progress`, `gauge`), each in its own file. Arabic text mirrors the side. Ideas: HyperFrames' small verified block set and Manim's counters; the design is ScriptVox's own.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Title,
    Stat,
    LowerThird,
    List,
    Bars,
    Line,
    /// A math formula in a small TeX-like subset, typeset by scriptvox (no LaTeX install).
    Formula,
    /// A code snippet in an editor panel, coloured and typed in line by line.
    Code,
    /// A code change (G187): a code card where a line that starts with `+` is added (green) and one that starts
    /// with `-` is removed (red). Other lines are context.
    Diff,
    /// A terminal (G187): a line that starts with `$ ` is a command and is typed in; the other lines are its
    /// output and come whole.
    Terminal,
    /// y = f(x) drawn on axes: `text` is the function (`x^2 - 2`, `2sin(x)`), `values` the x range.
    Plot,
    /// A quotation in `text`, with the speaker in `label`.
    Quote,
    /// Two panels side by side: two `items`, two `values`, the unit in `label`. The second panel is lit.
    Compare,
    /// A donut chart: `items` and `values` (parts of a whole), an optional middle text in `text`.
    Donut,
    /// Events down a line: 2 to 5 `items`. Each comes when the voice says it.
    Timeline,
    /// A table: `rows`, the first row is the header. A `mark` lights a row, a column or a cell on a spoken word.
    Table,
    /// A line from `values[0]` to `values[1]` with ticks and the `points` marked on it; `from` makes the first
    /// marker travel.
    NumberLine,
    /// Boxes and arrows: `nodes` are the box labels, `edges` the arrows. Laid out in steps by the arrows.
    Diagram,
    /// A numbered process: 2 to 5 `items`. The step the voice is at is lit.
    Steps,
    /// One value on a bar that fills to it: `values` is `[value]` out of 100 or `[value, top]`.
    Progress,
    /// One value on a ring that fills to it: `values` as for `progress`.
    Gauge,
}

/// How a list item or a bar moves in.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Ease {
    /// The same speed all the way.
    Linear,
    /// Fast at first, then slows as it lands.
    Out,
    /// Goes a little past its place and settles back.
    Spring,
}

/// Motion text over this scene.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Card {
    /// title, stat, lower_third, list, bars, line, formula, code, diff, terminal, plot, quote, compare, donut,
    /// timeline, table, number_line, diagram, steps, progress or gauge.
    pub kind: Kind,
    /// The headline, the number with its unit ("87%", "$1.2 billion"), the name, or the formula (`E = mc^2`,
    /// `\frac{d}{t}`, `\sqrt{x}`, `\pi r^2`). Refused on `bars`, `line`, `list`, `compare` and `timeline`, which do
    /// not draw it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The line under a stat, the role under a name, or the heading above a `bars` or `line` chart. Refused on
    /// `title`, `list`, `donut`, `timeline` and `steps`, which do not draw it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The points of a list (1 to 5, at most 40 characters each), the bar names of a `bars` chart, the point
    /// names of a `line` chart (3 to 8, at most 12 characters, usually years), the 2 to 5 steps of a `steps` card,
    /// or the names of the `points` of a `number_line`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<String>>,
    /// The numbers of a `bars` chart (one per item, 0 or more), of a `line` chart (one per point, any sign), the
    /// x range of a `plot` (`[from, to]`, default `[-5, 5]`), the two ends of a `number_line`, or `[value]` (out of
    /// 100) or `[value, top]` of a `progress` or `gauge` card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<f64>>,
    /// Title only: the title stands behind the subject of the shot (the subject is cut out on every frame by the
    /// free matte row and laid back over the text). Use it when the subject is large and crosses the middle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind: Option<bool>,
    /// Formula only: a second formula the first one turns into. Pieces both share (same symbols, in order) move
    /// to their new place; the rest fades out and in. Example: `A = \pi r^2` then `A = \pi (2r)^2 = 4\pi r^2`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
    /// A word of the scene's narration, as the script writes it: the card comes in when the voice says it, not at
    /// the scene's start (`"on": "forty"` for "revenue grew by forty two percent"). `plan` refuses a word the
    /// scene's script does not have. A formula with `then` is the exception: there `on` is when the change to the
    /// second formula starts (default: halfway through the scene).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    /// Or seconds from the scene's start when the card comes in, 0 or more. Give `on` or `at`, not both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<f64>,
    /// Seconds the card stays after it comes in; then it fades out and the scene goes on. Default: to the end of the
    /// scene. Above 0; a `hold` past the scene's end stops at the end. Not for a formula with `then`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<f64>,
    /// List and bars only: how each item moves in, `out`, `spring` or `linear`. Default: list items fade in, bars
    /// grow at one speed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<Ease>,
    /// List and bars only: seconds from one item to the next (0.05 to 1.5). Default: a list item comes when the
    /// voice says it; bars come 0.15 s apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stagger: Option<f64>,
    /// Stat: the number the count starts from (default 0). Number line: the value the first marker travels from. `"text": "42%", "from": 30` counts from 30% to 42%,
    /// and a `from` above the number counts down. It takes the unit and the decimals of `text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<f64>,
    /// Up to 4 pieces of the card that are marked when the voice says a word. Formula: the piece takes the accent
    /// colour, for example `[{"part": "mc^2", "on": "energy"}]` on `E = mc^2`; not with `then`. Title, stat, quote
    /// and list: words of the card get an underline, a ring or a highlight, for example
    /// `[{"part": "forty percent", "on": "forty", "style": "ring"}]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark: Option<Vec<Mark>>,
    /// Plot only: the y range `[min, max]`. Default: the range of the curves. A curve is cut where it leaves it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<Vec<f64>>,
    /// Plot only: `true` draws light grid lines at round steps (1, 2 or 5 times a power of ten). Default: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<bool>,
    /// Plot only: `true` puts a dot on the first curve that runs with it as it is drawn, with its x and y beside it;
    /// it rests at the curve's right end. Default: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<bool>,
    /// Table only: 2 to 8 rows of 2 to 5 cells each (at most 24 characters a cell); the first row is the header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<Vec<Vec<String>>>,
    /// Number line only: 1 to 4 values to mark on the line, each inside `values`; `items` may name them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<f64>>,
    /// Diagram only: 2 to 8 box labels, each at most 18 characters and there once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<String>>,
    /// Diagram only: 1 to 12 arrows, each `["from", "to"]` with two labels of `nodes`; no circle, no arrow over a step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edges: Option<Vec<Vec<String>>>,
}

/// How a phrase of a text card is marked.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MarkStyle {
    /// A line under the words; it grows from the reading side.
    Underline,
    /// An oval line around the words.
    Ring,
    /// A band behind the words.
    Highlight,
}

/// One piece of a card and the spoken word that marks it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Mark {
    /// Formula: a piece of the card's `text`, written as the formula writes it (`mc^2`, `\frac{a}{b}`). It must be
    /// there exactly once. A symbol keeps its `^` and `_`: in `mc^2` the pieces are `m`, `c`, `2`, `c^2` and `mc^2`.
    /// Title, stat, quote, list: one or more whole words of the card's `text`, `label` or `items`, in a row, there
    /// exactly once.
    pub part: String,
    /// A word of the scene's narration, as the script writes it: from this word on the piece is marked, to the end
    /// of the card.
    pub on: String,
    /// Title, stat, quote, list: `underline` (default), `ring` or `highlight`, in the accent colour. Not for a
    /// formula, whose piece takes the accent colour itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<MarkStyle>,
}

impl Card {
    /// A problem with this card, for the brief check.
    pub fn problem(&self) -> Option<String> {
        let has = |s: &Option<String>| s.as_deref().is_some_and(|t| !t.trim().is_empty());
        if let Some(p) = crate::card_kit::foreign(self) {
            return Some(p);
        }
        if let Some(f) = self.from {
            if !matches!(self.kind, Kind::Stat | Kind::NumberLine) {
                return Some(
                    "only a stat card (the number the count starts from) or a number_line card (the value the first marker travels from) takes `from`: remove it".into(),
                );
            }
            if !f.is_finite() || f.abs() > 1e12 {
                return Some("`from` is a plain number".into());
            }
            if self.kind == Kind::Stat && self.text.as_deref().and_then(split_number).is_none() {
                return Some(
                    "`from` needs a stat `text` that holds a number, for example \"42%\"".into(),
                );
            }
        }
        // A field the kind does not draw is an error, not dropped without a word: a bars card with its heading in
        // `text` rendered with no heading (seen 13 Nov 2026).
        // These two keep their own words (what to rename); every other field goes through the one table below.
        if has(&self.text) {
            let kind = format!("{:?}", self.kind).to_lowercase();
            match self.kind {
                Kind::Bars | Kind::Line => {
                    return Some(format!(
                        "a {kind} card does not draw `text`: its heading is `label`, so rename `text` to `label`"
                    ))
                }
                Kind::List | Kind::Compare | Kind::Timeline => {
                    return Some(format!(
                        "a {kind} card does not draw `text`: remove it (a heading can go in a title card of its own scene)"
                    ))
                }
                _ => {}
            }
        }
        if self.behind == Some(true) && self.kind != Kind::Title {
            return Some("only a title card can stand behind the subject (behind: true)".into());
        }
        if self.then.is_some() && self.kind != Kind::Formula {
            return Some("only a formula card takes then".into());
        }
        // A formula's `on` is the moment of its change, so it has no meaning without `then`.
        if self.on.is_some() && self.then.is_none() && self.kind == Kind::Formula {
            return Some("on needs then (the formula to turn into)".into());
        }
        if self
            .on
            .as_deref()
            .is_some_and(|o| crate::gates::norm_word(o).is_empty())
        {
            return Some("`on` takes one word of the scene's script".into());
        }
        if let Some(h) = self.hold {
            if !h.is_finite() || h <= 0.0 {
                return Some("`hold` takes seconds above 0".into());
            }
            if self.then.is_some() {
                return Some(
                    "a formula with then takes no `hold`: it stays to show the second formula"
                        .into(),
                );
            }
        }
        if let Some(p) = self.then.as_deref().and_then(crate::formula::problem) {
            return Some(format!("then: {p}"));
        }
        if (self.y.is_some() || self.grid.is_some() || self.trace.is_some())
            && self.kind != Kind::Plot
        {
            return Some("only a plot card takes `y`, `grid` and `trace`".into());
        }
        if self
            .y
            .as_deref()
            .is_some_and(|y| !matches!(y, [lo, hi] if lo.is_finite() && hi.is_finite() && lo < hi))
        {
            return Some(
                "`y` takes [min, max], the y range of a plot: two numbers, min below max".into(),
            );
        }
        if let Some(marks) = &self.mark {
            if marks.is_empty() || marks.len() > MAX_MARKS {
                return Some(format!(
                    "`mark` takes at most {MAX_MARKS} entries (and at least 1), this card has {}",
                    marks.len()
                ));
            }
            // A phrase of a text card: `card_mark.rs`. A table lights cells by its own rule.
            if self.kind == Kind::Table {
                if let Some(p) = crate::card_table::mark_problem(self) {
                    return Some(p);
                }
            } else if crate::card_kit::owns(self.kind) {
                // Refused below with the fields the kind takes.
            } else if self.kind != Kind::Formula {
                if let Some(p) = crate::card_mark::problem(self) {
                    return Some(p);
                }
            } else {
                if marks.iter().any(|m| m.style.is_some()) {
                    return Some(
                        "a formula mark takes no `style`: its piece takes the accent colour".into(),
                    );
                }
                if self.then.is_some() {
                    return Some(
                        "a formula with then takes no `mark`: the pieces move when it changes"
                            .into(),
                    );
                }
                let text = self.text.as_deref().unwrap_or("");
                // A formula that does not parse is named by its own check below.
                for m in marks
                    .iter()
                    .filter(|_| crate::formula::problem(text).is_none())
                {
                    if let Err(e) = crate::formula::find(text, &m.part) {
                        return Some(format!("mark: the part \"{}\" {e}", m.part));
                    }
                    if crate::gates::norm_word(&m.on).is_empty() {
                        return Some(format!(
                            "mark: the part \"{}\" needs `on`, one word of the scene's script",
                            m.part
                        ));
                    }
                }
            }
        }
        if (self.ease.is_some() || self.stagger.is_some())
            && !matches!(self.kind, Kind::List | Kind::Bars)
            && !crate::card_kit::owns(self.kind)
        {
            return Some("ease and stagger work on list and bars cards only".into());
        }
        // Any other field that this kind does not draw: one table for every kind (`card_kit::allowed`).
        if let Some(p) = crate::card_kit::undrawn(self) {
            return Some(p);
        }
        // A label longer than this has no room on a 4:3 frame even at its smallest size.
        let label_max = match self.kind {
            Kind::Compare => 30,
            Kind::Plot => 40,
            _ => 60,
        };
        // The data cards have their own limit and message (`card_kit::problem`).
        if !crate::card_kit::owns(self.kind)
            && self
                .label
                .as_deref()
                .is_some_and(|l| l.chars().count() > label_max)
        {
            return Some(format!(
                "a {:?} card label has at most {label_max} characters",
                self.kind
            ));
        }
        match self.kind {
            k if crate::card_kit::owns(k) => crate::card_kit::problem(self),
            Kind::Bars => {
                let (n, ok) = match (&self.items, &self.values) {
                    (Some(i), Some(v)) => (
                        i.len(),
                        i.len() == v.len()
                            && i.iter()
                                .all(|x| !x.trim().is_empty() && x.chars().count() <= 24)
                            && v.iter().all(|x| x.is_finite() && *x >= 0.0)
                            && v.iter().any(|x| *x > 0.0),
                    ),
                    _ => (0, false),
                };
                (!(ok && (2..=6).contains(&n))).then(|| "a bars card needs 2 to 6 items (at most 24 characters) and as many values, 0 or more, not all 0".into())
            }
            Kind::Line => {
                let ok = match (&self.items, &self.values) {
                    (Some(i), Some(v)) => {
                        (3..=8).contains(&i.len())
                            && i.len() == v.len()
                            && i.iter()
                                .all(|x| !x.trim().is_empty() && x.chars().count() <= 12)
                            && v.iter().all(|x| x.is_finite())
                    }
                    _ => false,
                };
                (!ok).then(|| "a line card needs 3 to 8 items (at most 12 characters, for example years) and as many values".into())
            }
            _ if self.stagger.is_some_and(|s| !(0.05..=1.5).contains(&s)) => {
                Some("stagger takes 0.05 to 1.5 seconds".into())
            }
            Kind::Compare => {
                let ok = match (&self.items, &self.values) {
                    (Some(i), Some(v)) => {
                        i.len() == 2
                            && v.len() == 2
                            && i.iter()
                                .all(|x| !x.trim().is_empty() && x.chars().count() <= 20)
                            && v.iter().all(|x| x.is_finite())
                    }
                    _ => false,
                };
                (!ok).then(|| {
                    "a compare card needs 2 items (at most 20 characters) and 2 values".into()
                })
            }
            Kind::Donut => {
                let ok = match (&self.items, &self.values) {
                    (Some(i), Some(v)) => {
                        (2..=6).contains(&i.len())
                            && i.len() == v.len()
                            && i.iter()
                                .all(|x| !x.trim().is_empty() && x.chars().count() <= 20)
                            && v.iter().all(|x| x.is_finite() && *x >= 0.0)
                            && v.iter().any(|x| *x > 0.0)
                    }
                    _ => false,
                };
                (!ok).then(|| "a donut card needs 2 to 6 items (at most 20 characters) and as many values, 0 or more, not all 0".into())
            }
            Kind::Timeline => match &self.items {
                Some(v)
                    if (2..=5).contains(&v.len())
                        && v.iter()
                            .all(|i| !i.trim().is_empty() && i.chars().count() <= 48) =>
                {
                    None
                }
                _ => Some("a timeline card needs 2 to 5 items of at most 48 characters".into()),
            },
            Kind::Quote
                if self
                    .text
                    .as_deref()
                    .is_some_and(|t| t.chars().count() > 160) =>
            {
                Some("a quote card takes at most 160 characters".into())
            }
            Kind::List => match &self.items {
                Some(v)
                    if (1..=5).contains(&v.len())
                        && v.iter()
                            .all(|i| !i.trim().is_empty() && i.chars().count() <= 40) =>
                {
                    None
                }
                _ => Some("a list card needs 1 to 5 items of at most 40 characters".into()),
            },
            _ if !has(&self.text) => {
                Some(format!("a {:?} card needs text", self.kind).to_lowercase())
            }
            Kind::Formula => crate::formula::problem(self.text.as_deref().unwrap_or("")),
            Kind::Code | Kind::Diff | Kind::Terminal => {
                crate::code_card::problem(self.text.as_deref().unwrap_or(""))
            }
            Kind::Plot => {
                let (text, values) = (self.text.as_deref().unwrap_or(""), self.values.as_deref());
                crate::plot::problem(text, values).or_else(|| {
                    crate::plot::y_problem(text, values, self.y.as_deref().unwrap_or(&[]))
                })
            }
            Kind::Title if self.text.as_ref().unwrap().chars().count() > 60 => {
                Some("a title card takes at most 60 characters".into())
            }
            _ => None,
        }
    }

    /// A problem with `on` or `hold` against the scene's own script, for the brief check.
    pub fn on_problem(&self, script_raw: &str) -> Option<String> {
        let script = crate::pauses::strip(script_raw);
        let says = |on: &str| {
            let key = crate::gates::norm_word(on);
            script
                .split_whitespace()
                .any(|w| crate::gates::norm_word(w).starts_with(&key))
        };
        if let Some(on) = self.on.as_deref().filter(|_| self.then.is_none()) {
            if !says(on) {
                return Some(format!(
                    "the card's `on` word \"{on}\" is not in this scene's script (write it as the script does)"
                ));
            }
        }
        if let Some(m) = self.mark.iter().flatten().find(|m| !says(&m.on)) {
            return Some(format!(
                "mark: the `on` word \"{}\" of the part \"{}\" is not in this scene's script (write it as the script does)",
                m.on, m.part
            ));
        }
        // ponytail: the real length is known only after the voice is made, so this refuses only what no voice
        // could reach (4 s a word plus 10 s, plus the script's own pauses); `window` stops a hold at the scene's
        // real end.
        let pauses: f64 = crate::pauses::parse(script_raw)
            .iter()
            .map(|s| match s {
                crate::pauses::Seg::Pause(p) => *p,
                _ => 0.0,
            })
            .sum();
        let most = script.split_whitespace().count() as f64 * 4.0 + 10.0 + pauses;
        self.hold.filter(|h| *h > most).map(|h| {
            format!(
                "`hold` of {h} s is longer than this scene can last (about {most:.0} s at most)"
            )
        })
    }

    /// Every string the card puts on screen (for the text lint).
    pub fn texts(&self) -> Vec<&str> {
        // A formula's text is math and a code card's is code, not words: only the label is checked.
        let text = if matches!(
            self.kind,
            Kind::Formula | Kind::Code | Kind::Diff | Kind::Terminal | Kind::Plot
        ) {
            &None
        } else {
            &self.text
        };
        let mut v: Vec<&str> = [text, &self.label]
            .into_iter()
            .flatten()
            .map(|s| s.as_str())
            .collect();
        v.extend(self.items.iter().flatten().map(|s| s.as_str()));
        v.extend(self.rows.iter().flatten().flatten().map(|s| s.as_str()));
        v.extend(self.nodes.iter().flatten().map(|s| s.as_str()));
        v
    }
}

/// The most pieces of one formula that can be marked.
pub const MAX_MARKS: usize = 4;

/// When the voice starts the first word that starts with `on`, both folded as the words gate folds them (Arabic
/// letter forms, "four" equals "4").
pub(crate) fn word_start(on: &str, said: &[crate::captions::Word]) -> Option<f64> {
    let key = crate::gates::norm_word(on);
    said.iter()
        .find(|w| crate::gates::norm_word(&w.text).starts_with(&key))
        .map(|w| w.start)
}

/// A title behind the subject: font size as a multiple of the caption size, and its centre as a share of the height.
pub const BEHIND_SCALE: u32 = 3;
pub const BEHIND_Y: f32 = 0.36;

/// The Card style for `behind.ass`: an outline style (1) instead of the box (4), shadow colour 40 percent black.
pub fn behind_style(ass: &str) -> String {
    ass.lines()
        .map(|l| match l.strip_prefix("Style: Card,") {
            Some(rest) => {
                let mut f: Vec<String> = rest.split(',').map(String::from).collect();
                // Fields after the name: font 0, size 1, colours 2 to 5, bold 6 ... BorderStyle 14, Outline 15, Shadow 16.
                f[5] = "&H66000000".into();
                f[14] = "1".into();
                f[15] = "0".into();
                format!("Style: Card,{}\n", f.join(","))
            }
            None => format!("{l}\n"),
        })
        .collect()
}

/// The same ASS line with its `\\pos(x,y)` moved to height `y`.
pub fn move_to(line: &str, y: f32) -> String {
    let Some(a) = line.find("\\pos(") else {
        return line.to_string();
    };
    let Some(b) = line[a..].find(')').map(|b| a + b) else {
        return line.to_string();
    };
    let x = line[a + 5..b].split(',').next().unwrap_or("0");
    format!("{}\\pos({x},{y}){}", &line[..a], &line[b + 1..])
}

/// The number inside a stat: the text around it, its value, decimals, thousands separators, and whether it is
/// written in Arabic-Indic digits (٠ to ٩, with ٬ and ٫), so it counts in the same script.
#[derive(Debug, PartialEq)]
pub struct Num {
    pub pre: String,
    pub value: f64,
    pub post: String,
    pub decimals: usize,
    pub commas: bool,
    pub arabic: bool,
}

fn to_ascii(c: char) -> Option<char> {
    match c {
        '0'..='9' | ',' | '.' => Some(c),
        '\u{0660}'..='\u{0669}' => char::from_digit(c as u32 - 0x0660, 10),
        '\u{066C}' => Some(','),
        '\u{066B}' => Some('.'),
        _ => None,
    }
}

pub fn split_number(text: &str) -> Option<Num> {
    let chars: Vec<char> = text.chars().collect();
    let start = chars
        .iter()
        .position(|c| to_ascii(*c).is_some_and(|a| a.is_ascii_digit()))?;
    let mut end = start;
    while end < chars.len() && to_ascii(chars[end]).is_some() {
        end += 1;
    }
    while end > start && !to_ascii(chars[end - 1]).unwrap().is_ascii_digit() {
        end -= 1;
    }
    let raw: String = chars[start..end]
        .iter()
        .filter_map(|c| to_ascii(*c))
        .collect();
    Some(Num {
        pre: chars[..start].iter().collect(),
        value: raw.replace(',', "").parse().ok()?,
        post: chars[end..].iter().collect(),
        decimals: raw.split_once('.').map_or(0, |(_, d)| d.len()),
        commas: raw.contains(','),
        arabic: chars[start..end]
            .iter()
            .any(|c| ('\u{0660}'..='\u{066C}').contains(c)),
    })
}

impl Num {
    /// `v` written like the original number.
    pub fn show(&self, v: f64) -> String {
        let s = format_number(v, self.decimals, self.commas);
        let body: String = if self.arabic {
            s.chars()
                .map(|c| match c {
                    '0'..='9' => char::from_u32(0x0660 + c.to_digit(10).unwrap()).unwrap(),
                    ',' => '\u{066C}',
                    '.' => '\u{066B}',
                    c => c,
                })
                .collect()
        } else {
            s
        };
        format!("{}{body}{}", self.pre, self.post)
    }
}

/// `v` written with `decimals` places, with thousands commas when `commas`.
pub fn format_number(v: f64, decimals: usize, commas: bool) -> String {
    let s = format!("{v:.decimals$}");
    if !commas {
        return s;
    }
    let (int, frac) = s
        .split_once('.')
        .map_or((s.as_str(), None), |(a, b)| (a, Some(b)));
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    match frac {
        Some(f) => format!("{out}.{f}"),
        None => out,
    }
}

/// A number written with ASCII digits in Arabic-Indic digits (٠ to ٩, ٬ and ٫).
pub fn indic(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0'..='9' => char::from_u32(0x0660 + c.to_digit(10).unwrap()).unwrap(),
            ',' => '\u{066C}',
            '.' => '\u{066B}',
            c => c,
        })
        .collect()
}

/// A chart's values follow its names: Arabic-Indic digits when a name uses them.
pub(crate) fn digits_like(items: &[String], s: String) -> String {
    if items
        .iter()
        .any(|i| i.chars().any(|c| ('\u{0660}'..='\u{0669}').contains(&c)))
    {
        indic(&s)
    } else {
        s
    }
}

/// Where and how big: the frame size and the caption size in pixels.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub size: u32,
}

pub(crate) fn esc(s: &str) -> String {
    let body = s
        .replace('\\', "\u{29F5}")
        .replace('{', "(")
        .replace('}', ")");
    format!("{}{}", crate::captions::rtl_mark(s), latin_runs(&body))
}

/// In a text with Arabic letters, each run of marks the Arabic font cannot draw (Latin letters, `%`) is set in Noto
/// Sans by name, then the style's font comes back (`\fn` with no name). On Linux fontconfig gave Noto Sans already
/// (the alias in `fontconfig()`); libass on macOS asks CoreText, which gave a system font at about twice the size
/// (`API`, `%` in the Arabic cards of the macOS run, 14 Nov 2026). A brief's own font is left to the user.
fn latin_runs(s: &str) -> String {
    use read_fonts::{FontRef, TableProvider};
    if !s.chars().any(crate::captions::is_arabic)
        || crate::template::font_for("ar").0 != "Noto Sans Arabic"
    {
        return s.to_string();
    }
    let Some(map) = FontRef::new(crate::template::FONTS[1].2)
        .ok()
        .and_then(|f| f.cmap().ok())
    else {
        return s.to_string();
    };
    let foreign =
        |c: char| !c.is_whitespace() && map.map_codepoint(c).is_none_or(|g| g.to_u32() == 0);
    let mut out = String::with_capacity(s.len() + 32);
    let mut open = false;
    for c in s.chars() {
        // A run ends at the first mark the Arabic font has, a space included: spaces keep the font they had.
        if foreign(c) && !open {
            out.push_str("{\\fnNoto Sans}");
            open = true;
        } else if open && !foreign(c) {
            out.push_str("{\\fn}");
            open = false;
        }
        out.push(c);
    }
    if open {
        out.push_str("{\\fn}");
    }
    out
}

fn ass_time(t: f64) -> String {
    let n = (t.max(0.0) * 100.0).round() as u64;
    format!(
        "{}:{:02}:{:02}.{:02}",
        n / 360000,
        n / 6000 % 60,
        n / 100 % 60,
        n % 100
    )
}

/// The same event, ending at `end`: an end card holds its title for the whole card, past the 2.5 s title limit.
pub fn until(line: &str, end: f64) -> String {
    let f: Vec<&str> = line.splitn(4, ',').collect();
    match f.as_slice() {
        [a, b, _, rest] => format!("{a},{b},{},{rest}", ass_time(end)),
        _ => line.to_string(),
    }
}

pub(crate) fn line(start: f64, end: f64, tags: &str, text: &str) -> String {
    let c = ass_time;
    format!(
        "Dialogue: 2,{},{},Card,,0,0,0,,{{{tags}}}{text}",
        c(start),
        c(end)
    )
}

/// Lines of even length: the narrowest width (down to half) that still gives the same number of lines, so a
/// two-line label never ends on one lone word.
pub fn balanced(
    wrap: &dyn Fn(&str, u32, f32) -> Vec<String>,
    t: &str,
    s: u32,
    max: f32,
) -> Vec<String> {
    let mut best = wrap(t, s, max);
    if best.len() < 2 {
        return best;
    }
    let n = best.len();
    let mut width = max;
    while width > max * 0.5 {
        width -= max * 0.04;
        let next = wrap(t, s, width);
        if next.len() != n {
            break;
        }
        best = next;
    }
    best
}

/// Scale tags that grow a part from `from` percent to 100 between `t0` and `t1` (ms after the line starts):
/// width only (`xy` false, a bar) or both. ASS `\\t` takes an acceleration: under 1 is fast first, then slow.
/// `spring` goes to 108 percent in the first 70 percent of the time, then settles.
fn scale_in(xy: bool, from: u32, t0: i64, t1: i64, ease: Ease) -> String {
    let s = |p: u32| {
        if xy {
            format!("\\fscx{p}\\fscy{p}")
        } else {
            format!("\\fscx{p}")
        }
    };
    match ease {
        Ease::Linear => format!("{}\\t({t0},{t1},{})", s(from), s(100)),
        Ease::Out => format!("{}\\t({t0},{t1},0.4,{})", s(from), s(100)),
        Ease::Spring => {
            let tm = t0 + (t1 - t0) * 7 / 10;
            format!(
                "{}\\t({t0},{tm},0.5,{})\\t({tm},{t1},{})",
                s(from),
                s(108),
                s(100)
            )
        }
    }
}

/// The ASS events for one card shown from `start` to `end`. `wrap` breaks text to a pixel width at a font size.
pub fn events(
    card: &Card,
    start: f64,
    end: f64,
    f: &Frame,
    wrap: &dyn Fn(&str, u32, f32) -> Vec<String>,
    said: &[crate::captions::Word],
) -> Vec<String> {
    // A wide frame has much more room than a tall one: text cards 1.5 times bigger, panels and charts 1.2 times.
    // Rows end above a two-line caption.
    let wide = f.w > f.h;
    // The caption size is raised for a script whose font has a taller line (Arabic). Formulas and plot numbers are
    // set in Latin and math fonts, so they scale back; else an Arabic plot's numbers cover its curve.
    let latin = ((84 * f.w.min(f.h) / 1080) as f32 / f.size as f32).min(1.0);
    let f = &Frame {
        size: match card.kind {
            _ if !wide => f.size,
            Kind::Title
            | Kind::Stat
            | Kind::LowerThird
            | Kind::List
            | Kind::Formula
            | Kind::Compare => f.size * 3 / 2,
            _ => f.size * 6 / 5,
        },
        ..*f
    };
    let (w, h, size) = (f.w as f32, f.h as f32, f.size);
    // A taller caption font (Arabic) starts its two-line caption higher on a wide frame.
    let rows_bottom = h * match (wide, latin < 0.95) {
        (true, true) => 0.66,
        (true, false) => 0.72,
        _ => 0.62,
    };
    let rows_top = h * if wide { 0.08 } else { 0.15 };
    let rtl = card
        .texts()
        .iter()
        .any(|t| !crate::captions::rtl_mark(t).is_empty());
    let joined = |t: &str, s: u32, max: f32| {
        balanced(wrap, t, s, max)
            .iter()
            .map(|l| esc(l))
            .collect::<Vec<_>>()
            .join("\\N")
    };
    let end = end.max(start + 0.5);
    let mut out = Vec::new();
    match card.kind {
        // The data cards, each in its own file.
        k if crate::card_kit::owns(k) => {
            out = crate::card_kit::events(
                card,
                &crate::card_kit::Area {
                    w,
                    h,
                    size,
                    top: rows_top,
                    bottom: rows_bottom,
                    rtl,
                    wide,
                    latin,
                    start,
                    end,
                    wrap,
                    said,
                },
            );
        }
        Kind::Title => {
            // Behind the subject: twice as big, higher, a soft shadow and no box (the box would hide the cut-out).
            let (s, y, look) = if card.behind == Some(true) {
                // One line when it can be: a second line of this size left the top of a wide frame (seen 13 Nov
                // 2026, "THE LAST SEMICOLON" at 16:9). It shrinks no further than a plain title.
                let text = card.text.as_deref().unwrap_or("");
                let mut s = size * BEHIND_SCALE;
                while s > size * 3 / 2 && balanced(wrap, text, s, w * 0.86).len() > 1 {
                    s = s * 9 / 10;
                }
                (s, h * BEHIND_Y, "\\shad5")
            } else {
                (size * 3 / 2, h * 0.42, "")
            };
            let tags = format!("\\an5\\pos({},{y}){look}\\fs{s}\\fad(120,200)\\fscx85\\fscy85\\t(0,200,\\fscx100\\fscy100)", w / 2.0);
            out.push(line(
                start,
                end.min(start + 2.5),
                &tags,
                &joined(card.text.as_deref().unwrap_or(""), s, w * 0.86),
            ));
        }
        Kind::Stat => {
            // The number and its label stay between the rows' limits: the label shrinks first, up to under half the
            // room, then the number moves up and shrinks (an Arabic label on 4:3 lay over the captions).
            let room = rows_bottom - rows_top;
            let label = card
                .label
                .as_deref()
                .map(|l| fit_label(&joined, l, size, w * 0.86, room * 0.45));
            let label_h = label.as_ref().map_or(0.0, |l| l.2);
            let s = (size * 13 / 5).min(((room - label_h) / 1.35) as u32);
            let y = (h * 0.36).min(rows_bottom - label_h - s as f32 * 0.75);
            let text = card.text.as_deref().unwrap_or("");
            let tags = |fade: &str| format!("\\an5\\pos({},{y})\\fs{s}{fade}", w / 2.0);
            let count_end = (start + 0.8).min(end);
            match split_number(text) {
                Some(num) if count_end > start + 0.2 => {
                    // 12 steps that ease out, so the number slows as it lands.
                    for k in 0..12 {
                        let (a, b) = (k as f64 / 12.0, (k + 1) as f64 / 12.0);
                        // From `from` (default 0) to the number: up or down.
                        let from = card.from.unwrap_or(0.0);
                        let shown = from + (num.value - from) * (1.0 - (1.0 - a).powi(3));
                        let fade = if k == 0 { "\\fad(120,0)" } else { "" };
                        out.push(line(
                            start + a * (count_end - start),
                            start + b * (count_end - start),
                            &tags(fade),
                            &esc(&num.show(shown)),
                        ));
                    }
                    out.push(line(count_end, end, &tags("\\fad(0,200)"), &esc(text)));
                }
                _ => out.push(line(start, end, &tags("\\fad(120,200)"), &esc(text))),
            }
            if let Some((ls, t, _)) = &label {
                let tags = format!(
                    "\\an8\\pos({},{})\\fs{ls}\\fad(300,200)",
                    w / 2.0,
                    y + s as f32 * 0.75
                );
                out.push(line(start + 0.3, end, &tags, t));
            }
        }
        Kind::LowerThird => {
            let (an, x0, x1) = if rtl {
                (3, w * 1.1, w * 0.94)
            } else {
                (1, -w * 0.1, w * 0.06)
            };
            let y = h * 0.62;
            let mut text = format!(
                "{{\\fs{}}}{}",
                size * 11 / 10,
                esc(card.text.as_deref().unwrap_or(""))
            );
            if let Some(l) = card.label.as_deref() {
                text += &format!("\\N{{\\fs{}\\b0}}{}", size * 4 / 5, esc(l));
            }
            let tags = format!("\\an{an}\\move({x0},{y},{x1},{y},0,300)\\fad(0,250)");
            out.push(line(start, end.min(start + 4.0), &tags, &text));
        }
        Kind::Bars => {
            let items = card.items.as_deref().unwrap_or(&[]);
            let values = card.values.as_deref().unwrap_or(&[]);
            let max = values.iter().cloned().fold(0.0, f64::max).max(1e-9);
            let decimals = if values.iter().all(|v| v.fract() == 0.0) {
                0
            } else {
                1
            };
            let mut top = rows_top;
            // The label is the chart's title, above the bars.
            if let Some(l) = card.label.as_deref() {
                // A long title shrinks, and the rows start under all its lines.
                let (ls, t, th) =
                    fit_label(&joined, l, size, w * 0.86, (rows_bottom - rows_top) * 0.25);
                let tags = format!("\\an8\\pos({},{top})\\fs{ls}\\fad(200,200)", w / 2.0);
                out.push(line(start, end, &tags, &t));
                top += th + size as f32 * 0.3;
            }
            // A row is a name over a bar. When rows do not fit above the captions (wide frames), the whole row
            // shrinks, text and bar together, so a name never touches the bar above it.
            // On a wide frame the name stands beside its bar, which leaves the rows tall enough to read.
            // Long names on a frame without the width (4:3, or 16:9 with very long names) would leave no bar: they go over it.
            // Joined Arabic letters are narrower for their size than Latin ones (0.6 em put short names over the bars
            // and shrank every row on 4:3).
            let letter = if rtl { 0.45 } else { 0.6 };
            let longest = items.iter().map(|i| i.chars().count()).max().unwrap_or(0) as f32;
            // Room for the value at the bar's end; on a wide frame, what the values need (a fixed 3.5 em left Arabic
            // bars no room for names beside them).
            let value_room = if wide {
                let chars = values
                    .iter()
                    .map(|v| format_number(*v, decimals, *v >= 10000.0).chars().count())
                    .max()
                    .unwrap_or(2);
                size as f32 * 0.5 * (chars + 1).max(3) as f32
            } else {
                size as f32 * 3.5
            };
            let side = wide
                && w * 0.84 - value_room - longest * size as f32 * 0.8 * letter - size as f32 * 0.4
                    >= w * 0.3;
            let natural = size as f32 * if side { 1.3 } else { 0.8 * 1.25 + 0.6 + 0.45 };
            let k = ((rows_bottom - top) / items.len().max(1) as f32 / natural).min(1.0);
            let row = natural * k;
            let small = (size as f32 * 0.8 * k).round() as u32;
            let bar_h = size as f32 * 0.6 * k;
            let (xl, an_l, an_v) = if rtl {
                (w * 0.92, 9, 6)
            } else {
                (w * 0.08, 7, 4)
            };
            let names = if side {
                longest * small as f32 * letter + size as f32 * 0.4
            } else {
                0.0
            };
            let full = w * 0.84 - value_room - names;
            let x0 = if rtl { xl - names } else { xl + names };
            let ms = |t: f64| (t * 1000.0).round() as i64;
            for (i, (it, v)) in items.iter().zip(values).enumerate() {
                let y = top + i as f32 * row;
                let grow0 = 0.2 + i as f64 * card.stagger.unwrap_or(0.15);
                let grow1 = grow0 + 0.6;
                let bw = (full * (*v / max) as f32).max(2.0);
                let label = if side {
                    let an = if rtl { 6 } else { 4 };
                    format!(
                        "\\an{an}\\pos({xl},{})\\fs{small}\\fad(200,200)",
                        y + bar_h / 2.0
                    )
                } else {
                    format!("\\an{an_l}\\pos({xl},{y})\\fs{small}\\fad(200,200)")
                };
                out.push(line(start + grow0 - 0.1, end, &label, &esc(it)));
                // The bar: a box drawn from the label side; it grows from 0 to full width.
                let by = if side { y } else { y + small as f32 * 1.25 };
                let rect = |len: f32| {
                    let l = if rtl { -len } else { len };
                    format!("m 0 0 l {l:.0} 0 l {l:.0} {bar_h:.0} l 0 {bar_h:.0}")
                };
                // A dark track at full length keeps the bar readable on any background.
                let track = format!("\\an7\\pos({x0},{by})\\bord0\\shad0\\4a&HFF&\\1c&H000000&\\1a&H90&\\fad(200,200)\\p1");
                out.push(line(start + grow0 - 0.1, end, &track, &rect(full)));
                let shape = rect(bw);
                let tags = format!(
                    "\\an7\\pos({x0},{by})\\bord0\\shad0\\4a&HFF&\\1c{}&{}\\fad(0,200)\\p1",
                    crate::captions::ACCENT,
                    scale_in(
                        false,
                        0,
                        ms(grow0),
                        ms(grow1),
                        card.ease.unwrap_or(Ease::Linear)
                    )
                );
                out.push(line(start, end, &tags, &shape));
                let vx = if rtl {
                    x0 - bw - size as f32 * 0.4
                } else {
                    x0 + bw + size as f32 * 0.4
                };
                let vt = format!(
                    "\\an{an_v}\\pos({vx},{})\\fs{small}\\fad(150,200)",
                    by + bar_h / 2.0
                );
                out.push(line(
                    start + grow1,
                    end,
                    &vt,
                    &esc(&digits_like(
                        items,
                        format_number(*v, decimals, *v >= 10000.0),
                    )),
                ));
            }
        }
        Kind::Code | Kind::Diff | Kind::Terminal => {
            let lines = crate::code_card::lines(card.text.as_deref().unwrap_or(""));
            let n = lines.len().max(1);
            let cols = lines
                .iter()
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(1)
                .max(16);
            // The caption size, smaller when the longest line would pass 84 percent of the width.
            let fs = (size as f32).min(w * 0.84 / (cols as f32 * crate::code_card::ADVANCE));
            let (tw, lh) = (cols as f32 * crate::code_card::ADVANCE * fs, fs * 1.05);
            let pad = fs * 0.6;
            let bar = if card.label.is_some() { fs * 1.3 } else { 0.0 };
            let block = bar + n as f32 * lh + 2.0 * pad;
            let (bx0, bx1) = ((w - tw) / 2.0 - pad, (w + tw) / 2.0 + pad);
            let by0 = (h * if wide { 0.38 } else { 0.40 } - block / 2.0).max(h * 0.06);
            let by1 = by0 + block;
            let shape = |x0: f32, y0: f32, x1: f32, y1: f32| {
                format!("m {x0:.0} {y0:.0} l {x1:.0} {y0:.0} l {x1:.0} {y1:.0} l {x0:.0} {y1:.0}")
            };
            let fill = |c: &str| {
                format!("\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c{c}\\1a&H10&\\fad(200,200)\\p1")
            };
            out.push(line(
                start,
                end,
                &fill("&H1E1A17&"),
                &shape(bx0, by0, bx1, by1),
            ));
            if let Some(l) = card.label.as_deref() {
                out.push(line(
                    start,
                    end,
                    &fill("&H3A322C&"),
                    &shape(bx0, by0, bx1, by0 + bar),
                ));
                // An Arabic file name: the video's own font (the mono font has no Arabic), on the right.
                let ar = !crate::captions::rtl_mark(l).is_empty();
                let (an, x, font, text) = if ar {
                    (6, bx1 - pad, String::new(), esc(l))
                } else {
                    let f = format!("\\fn{}", crate::code_card::FAMILY);
                    (4, bx0 + pad, f, crate::code_card::ass_text(l))
                };
                let tags = format!(
                    "\\an{an}\\pos({x:.0},{:.0}){font}\\fs{:.0}\\b0\\bord0\\shad0\\4a&HFF&\\1c&HB0B0B0&\\fad(200,200)",
                    by0 + bar / 2.0,
                    fs * 0.8
                );
                out.push(line(start, end, &tags, &text));
            }
            // Each line types in: shown through a clip that opens left to right, one line after another.
            let (first, step) = code_step(start, end, n);
            // A soft bar lights the line the voice is talking about (its words heard), under the text.
            for (i, t0, t1) in code_focus(&lines, said, start + first, step, end) {
                let y = by0 + bar + pad + i as f32 * lh;
                out.push(line(
                    t0,
                    t1,
                    "\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c&H5A4A3C&\\1a&H30&\\fad(150,150)\\p1",
                    &shape(bx0 + pad * 0.4, y - lh * 0.08, bx1 - pad * 0.4, y + lh * 1.02),
                ));
            }
            for (i, l) in lines.iter().enumerate() {
                if l.is_empty() {
                    continue;
                }
                let at = start + first + i as f64 * step;
                let (x, y) = (bx0 + pad, by0 + bar + pad + i as f32 * lh);
                let lw = l.chars().count() as f32 * crate::code_card::ADVANCE * fs;
                let mut typing = (l.chars().count() as f64 * 0.025).clamp(0.12, 0.6) * 1000.0;
                // A changed line of a diff: a green or red bar under it, from the moment the line comes.
                let tint = match (card.kind, l.chars().next()) {
                    (Kind::Diff, Some('+')) => Some("&H2F5A1F&"),
                    (Kind::Diff, Some('-')) => Some("&H2A2A78&"),
                    _ => None,
                };
                if let Some(c) = tint {
                    out.push(line(
                        at.min(end - 0.2),
                        end,
                        &format!("\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c{c}\\1a&H40&\\fad(150,200)\\p1"),
                        &shape(bx0 + pad * 0.4, y - lh * 0.08, bx1 - pad * 0.4, y + lh * 1.02),
                    ));
                }
                // The output of a terminal command is not typed: it comes whole.
                let output = card.kind == Kind::Terminal && !l.starts_with("$ ");
                if output {
                    typing = 1.0;
                }
                let tags = format!(
                    "\\an7\\pos({x:.0},{y:.0})\\fn{}\\fs{fs:.0}\\b0\\bord0\\shad0\\4a&HFF&\\clip({x:.0},{y:.0},{x:.0},{:.0})\\t(0,{typing:.0},\\clip({x:.0},{y:.0},{:.0},{:.0}))\\fad(0,200)",
                    crate::code_card::FAMILY,
                    y + lh,
                    x + lw + fs,
                    y + lh
                );
                out.push(line(
                    at.min(end - 0.2),
                    end,
                    &tags,
                    &match card.kind {
                        Kind::Terminal => crate::code_card::terminal_line(l),
                        _ => crate::code_card::ass_line(l),
                    },
                ));
            }
        }
        Kind::Formula => {
            let measure = |t: &str, em: f32| {
                crate::captions::Measurer::new(
                    crate::template::MATH_FONT.to_vec(),
                    crate::formula::font_size(em),
                )
                .map_or(0.0, |m| m.width(t))
            };
            let text = card.text.as_deref().unwrap_or("");
            let then = card.then.as_deref();
            // 1.5 times the caption size, smaller when a formula is wider than 84 percent of the frame. A second
            // formula is set at the same size, so shared symbols keep their size when they move.
            let mut em = size as f32 * 1.5 * latin;
            let widest = |em: f32| -> Option<f32> {
                let a = crate::formula::typeset(text, em, &measure).ok()?.1;
                let b = then.map_or(Some(0.0), |t| {
                    crate::formula::typeset(t, em, &measure).ok().map(|s| s.1)
                })?;
                Some(a.max(b))
            };
            let Some(wide) = widest(em) else {
                return out;
            };
            if wide > w * 0.84 {
                em *= w * 0.84 / wide;
            }
            // The formula and its label stay between the rows' limits: an Arabic label on 4:3 lay over the captions.
            let room = rows_bottom - rows_top;
            let label = card
                .label
                .as_deref()
                .map(|l| fit_label(&joined, l, size, w * 0.86, room * 0.4));
            let label_h = label.as_ref().map_or(0.0, |l| l.2 + size as f32 * 0.9);
            // Both formulas of a morph count: the second can be the taller one (a fraction).
            let tallest = [Some(text), then]
                .into_iter()
                .flatten()
                .filter_map(|t| crate::formula::typeset(t, em, &measure).ok())
                .map(|(_, _, asc, desc)| asc + desc)
                .fold(0.0, f32::max);
            let (panel, room) = (tallest + em * 0.9, room - label_h);
            if tallest > 0.0 && panel > room {
                em *= room / panel;
            }
            let place = |t: &str| {
                crate::formula::typeset(t, em, &measure)
                    .ok()
                    .map(|(parts, fw, asc, desc)| {
                        (
                            (w - fw) / 2.0,
                            h * if f.w > f.h { 0.32 } else { 0.40 } + (asc - desc) / 2.0,
                            parts,
                            fw,
                            asc,
                            desc,
                        )
                    })
            };
            let Some((ox, oy, parts, fw, asc, desc)) = place(text) else {
                return out;
            };
            let pad = em * 0.45;
            let second = then.and_then(place);
            let lowest = second
                .as_ref()
                .map_or(oy + desc, |p| (oy + desc).max(p.1 + p.5));
            let highest = second
                .as_ref()
                .map_or(oy - asc, |p| (oy - asc).min(p.1 - p.4));
            // Up to end above the captions, but never over the top of the rows (panel and label fit between them).
            let lift = (lowest + pad + label_h - rows_bottom)
                .max(0.0)
                .min((highest - pad - rows_top).max(0.0));
            let oy = oy - lift;
            let second = second.map(|mut p| {
                p.1 -= lift;
                p
            });
            let (mut bx0, mut by0, mut bx1, mut by1) =
                (ox - pad, oy - asc - pad, ox + fw + pad, oy + desc + pad);
            if let Some((ox2, oy2, _, fw2, asc2, desc2)) = &second {
                bx0 = bx0.min(ox2 - pad);
                by0 = by0.min(oy2 - asc2 - pad);
                bx1 = bx1.max(ox2 + fw2 + pad);
                by1 = by1.max(oy2 + desc2 + pad);
            }
            // When the first formula turns into the second: on the spoken word, else halfway; never before it has
            // been written (1.3 s) and with 0.9 s left to settle.
            let morph_at = second.as_ref().and_then(|_| {
                // Folded like the words gate: Arabic letter forms, and "four" equals "4".
                let key = card
                    .on
                    .as_deref()
                    .map(crate::gates::norm_word)
                    .unwrap_or_default();
                let spoken = said
                    .iter()
                    .find(|x| {
                        !key.is_empty()
                            && x.start >= start + 1.3
                            && crate::gates::norm_word(&x.text).starts_with(&key)
                    })
                    .map(|x| x.start);
                // A word near the scene's end: the change starts a little early so the result is seen for 0.3 s
                // (4 Nov 2026, Arabic: "أربعة" 0.87 s before the cut gave no change at all).
                let t = spoken
                    .unwrap_or(((start + end) / 2.0).max(start + 1.6))
                    .min(end - 0.9);
                (t >= start + 1.3).then_some(t)
            });
            let first_end = morph_at.unwrap_or(end);
            let panel =
                "\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c&H000000&\\1a&H90&\\fad(200,200)\\p1";
            out.push(line(start, end, panel, &format!("m {bx0:.0} {by0:.0} l {bx1:.0} {by0:.0} l {bx1:.0} {by1:.0} l {bx0:.0} {by1:.0}")));
            // The formula writes itself left to right: every piece is shown through one clip that sweeps across.
            let fade_out = if morph_at.is_some() {
                ""
            } else {
                "\\fad(0,200)"
            };
            let clip = format!(
                "\\clip({bx0:.0},{by0:.0},{bx0:.0},{by1:.0})\\t(250,1250,\\clip({bx0:.0},{by0:.0},{bx1:.0},{by1:.0}))"
            );
            let sweep = format!("{clip}{fade_out}");
            // Marked pieces: the part range of each and the time its word is said, after the write-on (1.3 s) and
            // with 0.9 s left to be seen, as the change of a morph. `problem` refuses marks with `then`.
            let marks: Vec<(std::ops::Range<usize>, f64)> = card
                .mark
                .iter()
                .flatten()
                .filter(|_| then.is_none())
                .filter_map(|m| {
                    let t = word_start(&m.on, said)?.min(end - 0.9).max(start + 1.3);
                    (t < end).then_some((crate::formula::find(text, &m.part).ok()?, t))
                })
                .collect();
            for (i, p) in parts.iter().enumerate() {
                let (tags, body) = crate::formula::ass_part(p, ox, oy);
                let body = if matches!(p, crate::formula::Part::Text { .. }) {
                    esc(&body)
                } else {
                    body
                };
                // A piece in two marks takes the first word. The accent stays to the card's end.
                let lit = marks
                    .iter()
                    .filter(|m| m.0.contains(&i))
                    .map(|m| m.1)
                    .reduce(f64::min);
                match lit {
                    None => out.push(line(start, first_end, &format!("{tags}{sweep}"), &body)),
                    Some(t) => {
                        // One event to the word and one from it, at the same place: the colour turns in 0.15 s.
                        // The colour is last, so it also wins over a bar's own white.
                        out.push(line(start, t, &format!("{tags}{clip}"), &body));
                        let turn = format!(
                            "{tags}\\t(0,150,\\1c{}&)\\fad(0,200)",
                            crate::captions::ACCENT
                        );
                        out.push(line(t, end, &turn, &body));
                    }
                }
            }
            if let (Some(t), Some((ox2, oy2, parts2, ..))) = (morph_at, &second) {
                out.extend(morph(
                    &parts,
                    (ox, oy),
                    parts2,
                    (*ox2, *oy2),
                    t,
                    end,
                    &measure,
                ));
            }
            if let Some((ls, t, _)) = &label {
                let tags = format!(
                    "\\an8\\pos({},{:.0})\\fs{ls}\\fad(200,200)",
                    w / 2.0,
                    by1 + size as f32 * 0.9
                );
                out.push(line((start + 1.3).min(end), end, &tags, t));
            }
        }
        Kind::Line => {
            let items = card.items.as_deref().unwrap_or(&[]);
            let values = card.values.as_deref().unwrap_or(&[]);
            let n = items.len().min(values.len()).max(2);
            let (lo, hi) = values
                .iter()
                .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
            let span = if hi > lo { hi - lo } else { 1.0 };
            let decimals = if values.iter().all(|v| v.fract() == 0.0) {
                0
            } else {
                1
            };
            let small = size * 4 / 5;
            let mut top = rows_top;
            if let Some(l) = card.label.as_deref() {
                // A long title shrinks, and the rows start under all its lines.
                let (ls, t, th) =
                    fit_label(&joined, l, size, w * 0.86, (rows_bottom - rows_top) * 0.25);
                let tags = format!("\\an8\\pos({},{top})\\fs{ls}\\fad(200,200)", w / 2.0);
                out.push(line(start, end, &tags, &t));
                top += th + size as f32 * 0.3;
            }
            // The plot: values above each point need a row on top, names a row below; all above the captions.
            let plot_top = top + small as f32 * 1.6;
            // The names stand under the plot (1.9 rows), so a wide frame's plot ends above them.
            let plot_bottom = if wide {
                (rows_bottom - small as f32 * 1.9).max(plot_top + size as f32)
            } else {
                (h * 0.55)
                    .min(plot_top + w * 0.6)
                    .max(plot_top + size as f32 * 2.0)
            };
            // The first and last points sit inside the panel, so their centred value and name stay on it.
            let (xl, xr) = (w * 0.2, w * 0.8);
            // Time runs with the reading direction: right to left in Arabic.
            let px = |i: usize| {
                let f = i as f32 / (n - 1) as f32;
                if rtl {
                    xr - f * (xr - xl)
                } else {
                    xl + f * (xr - xl)
                }
            };
            let py = |v: f64| plot_bottom - ((v - lo) / span) as f32 * (plot_bottom - plot_top);
            // A dark panel behind the chart keeps it readable on any picture.
            let pad = size as f32 * 0.6;
            let (bx0, by0, bx1, by1) = (
                w * 0.12 - pad,
                plot_top - small as f32 * 1.6,
                w * 0.88 + pad,
                plot_bottom + small as f32 * 1.9,
            );
            let panel =
                "\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c&H000000&\\1a&H90&\\fad(200,200)\\p1";
            out.push(line(start, end, panel, &format!("m {bx0:.0} {by0:.0} l {bx1:.0} {by0:.0} l {bx1:.0} {by1:.0} l {bx0:.0} {by1:.0}")));
            // The line draws itself: each segment is its own shape, shown through a clip that sweeps across.
            let ms = |t: f64| (t * 1000.0).round() as i64;
            let (draw0, draw1) = (0.3, 1.5);
            let (c0, c1) = if rtl { (bx1, bx0) } else { (bx0, bx1) };
            let clip = |x: f32| {
                let (a, b) = if rtl { (x, c0) } else { (c0, x) };
                format!("{a:.0},{by0:.0},{b:.0},{by1:.0}")
            };
            let t = (size as f32 * 0.14).max(3.0);
            for i in 0..n - 1 {
                let (x0, y0, x1, y1) = (px(i), py(values[i]), px(i + 1), py(values[i + 1]));
                let (dx, dy) = (x1 - x0, y1 - y0);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (nx, ny) = (-dy / len * t / 2.0, dx / len * t / 2.0);
                let quad = format!(
                    "m {:.1} {:.1} l {:.1} {:.1} l {:.1} {:.1} l {:.1} {:.1}",
                    x0 + nx,
                    y0 + ny,
                    x1 + nx,
                    y1 + ny,
                    x1 - nx,
                    y1 - ny,
                    x0 - nx,
                    y0 - ny
                );
                let tags = format!(
                    "\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c{}&\\clip({})\\t({},{},\\clip({}))\\fad(0,200)\\p1",
                    crate::captions::ACCENT,
                    clip(c0),
                    ms(draw0),
                    ms(draw1),
                    clip(c1)
                );
                out.push(line(start, end, &tags, &quad));
            }
            // Each point, its value and its name appear when the line reaches it.
            for i in 0..n {
                let (x, y) = (px(i), py(values[i]));
                let f = (x - c0).abs() / (c1 - c0).abs();
                let at = start + draw0 + f as f64 * (draw1 - draw0);
                let d = t * 1.6;
                let dot = "\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c&HFFFFFF&\\fad(120,200)\\p1";
                out.push(line(
                    at,
                    end,
                    dot,
                    &format!(
                        "m {:.0} {:.0} l {:.0} {:.0} l {:.0} {:.0} l {:.0} {:.0}",
                        x - d,
                        y - d,
                        x + d,
                        y - d,
                        x + d,
                        y + d,
                        x - d,
                        y + d
                    ),
                ));
                let vt = format!(
                    "\\an2\\pos({x},{})\\fs{small}\\fad(150,200)",
                    y - d - small as f32 * 0.2
                );
                out.push(line(
                    at,
                    end,
                    &vt,
                    &esc(&digits_like(
                        items,
                        format_number(values[i], decimals, values[i].abs() >= 10000.0),
                    )),
                ));
                let nt = format!(
                    "\\an8\\pos({x},{})\\fs{small}\\fad(200,200)",
                    plot_bottom + small as f32 * 0.5
                );
                out.push(line(start + 0.2, end, &nt, &esc(&items[i])));
            }
        }
        Kind::Plot => out.extend(plot_events(card, start, end, f, latin, &joined)),
        Kind::Quote => {
            let body = card.text.as_deref().unwrap_or("");
            let s = size * 23 / 20;
            // libass draws these marks unmirrored, so Arabic uses the pair that points outward on screen.
            let (open, close) = if rtl {
                ('»', '«')
            } else {
                ('\u{201C}', '\u{201D}')
            };
            let text = joined(&format!("{open}{body}{close}"), s, w * 0.82);
            let lines = text.matches("\\N").count() as f32 + 1.0;
            let y = h * if wide { 0.30 } else { 0.38 };
            let tags = format!(
                "\\an5\\pos({},{y})\\fs{s}\\fad(250,200)\\fscx96\\fscy96\\t(0,350,\\fscx100\\fscy100)",
                w / 2.0
            );
            out.push(line(start, end, &tags, &text));
            if let Some(l) = card.label.as_deref() {
                let ly = y + lines * s as f32 * 0.72 + size as f32 * 0.4;
                let t = format!(
                    "\\an8\\pos({},{ly})\\fs{}\\b0\\fad(300,200)",
                    w / 2.0,
                    size * 4 / 5
                );
                out.push(line(
                    start + 0.5,
                    end,
                    &t,
                    &joined(l, size * 4 / 5, w * 0.8),
                ));
            }
        }
        Kind::Compare => {
            let items = card.items.as_deref().unwrap_or(&[]);
            let values = card.values.as_deref().unwrap_or(&[]);
            // Reading order: the first panel on the reading side.
            let (pw, gap) = (w * 0.42, w * 0.04);
            let lefts = [w / 2.0 - gap / 2.0 - pw, w / 2.0 + gap / 2.0];
            let order: [usize; 2] = if rtl { [1, 0] } else { [0, 1] };
            // A long unit wraps inside the panel, which grows down for it (test 3: it ran past both edges at 9:16).
            // Up to two lines; a longer one shrinks (four lines lay over the number on 4:3). A panel taller than the
            // rows' room scales as a whole, so the unit never lies over the number.
            let mut size = size;
            let (fitted, unit_size, ph) = loop {
                let base_unit = size * 7 / 10;
                let fitted = card
                    .label
                    .as_deref()
                    .map(|l| fit_label(&joined, l, base_unit, pw * 0.86, base_unit as f32 * 2.6));
                let unit_size = fitted.as_ref().map_or(base_unit, |u| u.0);
                let unit_lines = fitted
                    .as_ref()
                    .map_or(1, |u| u.1.matches("\\N").count() + 1);
                let ph = size as f32 * 4.6 + (unit_lines - 1) as f32 * unit_size as f32 * 1.25;
                if ph <= rows_bottom - rows_top || size <= 8 {
                    break (fitted, unit_size, ph);
                }
                size -= 1;
            };
            let unit = fitted.map(|u| u.1);
            let body = size as f32 * 4.6;
            // A long name shrinks until it fits one line of its panel. Measured when it has a space to break at (an
            // estimate of 0.6 em a letter drew Arabic names at half the size they need); else about 0.6 em a letter.
            let name_fits = |s: u32| {
                items.iter().all(|i| {
                    if i.contains(char::is_whitespace) {
                        wrap(i, s, pw * 0.86).len() <= 1
                    } else {
                        i.chars().count() as f32 * if rtl { 0.45 } else { 0.6 } * s as f32
                            <= pw * 0.86
                    }
                })
            };
            let mut name_size = size * 4 / 5;
            while name_size > size / 3 && !name_fits(name_size) {
                name_size -= 1;
            }
            let top = (h * if wide { 0.40 } else { 0.38 } - ph / 2.0).max(rows_top);
            let decimals = if values.iter().all(|v| v.fract() == 0.0) {
                0
            } else {
                1
            };
            // The bigger the better, as long as the longer number fits its panel.
            let longest = values
                .iter()
                .map(|v| {
                    format_number(*v, decimals, v.abs() >= 10000.0)
                        .chars()
                        .count()
                })
                .max()
                .unwrap_or(1)
                .max(2);
            let num = (size * 5 / 2).min((pw * 0.8 / longest as f32 / 0.6) as u32);
            for (slot, i) in order.into_iter().enumerate() {
                let (x0, x1) = (lefts[slot], lefts[slot] + pw);
                let cx = (x0 + x1) / 2.0;
                let at = start + i as f64 * 0.35;
                let colour = if i == 1 { "&H402818&" } else { "&H1E1A17&" };
                let box_ = format!(
                    "m {x0:.0} {top:.0} l {x1:.0} {top:.0} l {x1:.0} {:.0} l {x0:.0} {:.0}",
                    top + ph,
                    top + ph
                );
                out.push(line(
                    at,
                    end,
                    &format!("\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c{colour}\\1a&H18&\\fad(200,200)\\p1"),
                    &box_,
                ));
                let plain = "\\bord0\\shad0\\4a&HFF&";
                out.push(line(
                    at,
                    end,
                    &format!(
                        "\\an8\\pos({cx},{})\\fs{name_size}{plain}\\fad(200,200)",
                        top + ph * 0.1
                    ),
                    &esc(&items[i]),
                ));
                // The number counts up from 0 over 0.7 s, in the digits of the names.
                let colour = if i == 1 {
                    format!("\\1c{}", crate::captions::ACCENT)
                } else {
                    String::new()
                };
                let v = values[i];
                let cy = top + body.min(ph) * 0.56;
                let tags = format!("\\an5\\pos({cx},{cy})\\fs{num}{plain}{colour}");
                let show = |x: f64| {
                    esc(&digits_like(
                        items,
                        format_number(x, decimals, v.abs() >= 10000.0),
                    ))
                };
                for k in 0..10 {
                    let (a, b) = (k as f64 / 10.0, (k + 1) as f64 / 10.0);
                    let shown = v * (1.0 - (1.0 - a).powi(3));
                    out.push(line(at + a * 0.7, at + b * 0.7, &tags, &show(shown)));
                }
                out.push(line(
                    at + 0.7,
                    end,
                    &format!("{tags}\\fad(0,200)"),
                    &show(v),
                ));
                if let Some(u) = &unit {
                    out.push(line(
                        at + 0.3,
                        end,
                        &format!(
                            "\\an2\\pos({cx},{})\\fs{unit_size}\\b0{plain}\\fad(200,200)",
                            top + ph - size as f32 * 0.32
                        ),
                        u,
                    ));
                }
            }
        }
        Kind::Donut => {
            let items = card.items.as_deref().unwrap_or(&[]);
            let values = card.values.as_deref().unwrap_or(&[]);
            let total: f64 = values.iter().sum::<f64>().max(1e-9);
            let shares = whole_percents(values);
            const COLOURS: [&str; 6] = [
                crate::captions::ACCENT,
                "&H00FFD040",
                "&H00B070FF",
                "&H0060D070",
                "&H004080FF",
                "&H00C0C0C0",
            ];
            let small = size * 4 / 5;
            // The legend under the ring only on a truly tall frame; 1:1 and 4:5 have no room for it there. Long names
            // get a smaller ring and more of the width.
            let beside = h < w * 1.5;
            let longest = items.iter().map(|i| i.chars().count()).max().unwrap_or(1) as f32 + 6.0;
            let long = longest > 20.0;
            let r = if wide {
                h * if long { 0.2 } else { 0.28 }
            } else if beside {
                w * if long { 0.16 } else { 0.22 }
            } else {
                (w * 0.22).min(h * 0.16)
            };
            let ring_x = if long { 0.20 } else { 0.28 };
            let (cx, cy) = match (beside, rtl) {
                (true, false) => (w * ring_x, rows_top + (rows_bottom - rows_top) / 2.0),
                (true, true) => (
                    w * (1.0 - ring_x),
                    rows_top + (rows_bottom - rows_top) / 2.0,
                ),
                _ => (w * 0.5, rows_top + r + size as f32 * 0.2),
            };
            let row_h = small as f32 * 1.4;
            let n = items.len().max(1);
            let (lx, ly, row) = if beside {
                let gap = if long { 0.40 } else { 0.52 };
                let lx = if rtl { w * (1.0 - gap) } else { w * gap };
                (lx, cy - row_h * n as f32 / 2.0, row_h)
            } else {
                let lx = if rtl { w * 0.86 } else { w * 0.14 };
                let top = cy + r + size as f32 * 0.5;
                (lx, top, row_h.min((rows_bottom - top) / n as f32))
            };
            // Whatever is still too long shrinks to its room (about 0.55 em a letter).
            let room = if !beside {
                w * 0.72
            } else if long {
                w * 0.54
            } else {
                w * 0.42
            };
            let fs = small
                .min((row / 1.25) as u32)
                .min((room / longest / if rtl { 0.42 } else { 0.55 }) as u32);
            let mut from = 0.0;
            let sweep = 0.9 / n as f64;
            for (i, v) in values.iter().enumerate() {
                let to = from + v / total;
                let at = start + 0.2 + i as f64 * sweep;
                let tags = format!(
                    "\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c{}\\p1",
                    COLOURS[i % 6]
                );
                // The ring draws itself clockwise from the top, one part after another.
                const STEPS: usize = 6;
                for k in 1..STEPS {
                    let a = from + (to - from) * k as f64 / STEPS as f64;
                    let t0 = at + sweep * (k - 1) as f64 / STEPS as f64;
                    let t1 = at + sweep * k as f64 / STEPS as f64;
                    out.push(line(t0, t1, &tags, &ring_part(cx, cy, r, r * 0.6, from, a)));
                }
                let done = at + sweep * (STEPS - 1) as f64 / STEPS as f64;
                out.push(line(
                    done,
                    end,
                    &format!("{tags}\\fad(0,200)"),
                    &ring_part(cx, cy, r, r * 0.6, from, to),
                ));
                // The legend row: a colour square, the name, its share.
                let y = ly + i as f32 * row;
                let d = fs as f32 * 0.6;
                let (sx, an, tx) = if rtl {
                    (lx - d, 6, lx - d * 1.6)
                } else {
                    (lx, 4, lx + d * 1.6)
                };
                let top = y + (row - d) / 2.0;
                out.push(line(
                    at,
                    end,
                    &format!("{tags}\\fad(150,200)"),
                    &format!(
                        "m {sx:.0} {top:.0} l {:.0} {top:.0} l {:.0} {:.0} l {sx:.0} {:.0}",
                        sx + d,
                        sx + d,
                        top + d,
                        top + d
                    ),
                ));
                let share = digits_like(items, format!("{}%", shares[i]));
                out.push(line(
                    at,
                    end,
                    &format!(
                        "\\an{an}\\pos({tx},{})\\fs{fs}\\fad(150,200)",
                        y + row / 2.0
                    ),
                    &esc(&format!("{}  {share}", items[i])),
                ));
                from = to;
            }
            if let Some(t) = card.text.as_deref() {
                let tags =
                    format!("\\an5\\pos({cx},{cy})\\fs{size}\\bord0\\shad0\\4a&HFF&\\fad(300,200)");
                out.push(line(start + 0.4, end, &tags, &esc(t)));
            }
        }
        Kind::Timeline => {
            let items = card.items.as_deref().unwrap_or(&[]);
            let n = items.len().max(1);
            let top = rows_top;
            let step_y = ((rows_bottom - top) / n as f32).min(size as f32 * 2.3);
            let at = reveal_times(items, said, start, end);
            let (rail, an, tx) = if rtl {
                (w * 0.92, 6, w * 0.87)
            } else {
                (w * 0.08, 4, w * 0.13)
            };
            let r = size as f32 * 0.22;
            let fs = size * 9 / 10;
            let y = |i: usize| top + (i as f32 + 0.5) * step_y;
            let fill = |c: &str| {
                format!("\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c{c}\\fad(150,200)\\p1")
            };
            for (i, it) in items.iter().enumerate() {
                if i > 0 {
                    // The line reaches this event when it comes.
                    let (a, b) = (y(i - 1), y(i));
                    let hw = (size as f32 * 0.05).max(2.0);
                    out.push(line(
                        at[i],
                        end,
                        "\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&\\1c&H000000&\\1a&H60&\\fad(150,200)\\p1",
                        &format!("m {:.0} {a:.0} l {:.0} {a:.0} l {:.0} {b:.0} l {:.0} {b:.0}", rail - hw, rail + hw, rail + hw, rail - hw),
                    ));
                }
                // A dark edge keeps a yellow dot readable on a yellow background.
                out.push(line(
                    at[i],
                    end,
                    &fill("&H000000&"),
                    &ring_part(rail, y(i), r * 1.35, 0.0, 0.0, 1.0),
                ));
                out.push(line(
                    at[i],
                    end,
                    &fill(crate::captions::ACCENT),
                    &ring_part(rail, y(i), r, 0.0, 0.0, 1.0),
                ));
                let tags = format!("\\an{an}\\pos({tx},{})\\fs{fs}\\fad(200,200)", y(i));
                out.push(line(at[i], end, &tags, &joined(it, fs, w * 0.79)));
            }
        }
        // Sent on above; the compiler wants each kind named.
        Kind::Table
        | Kind::NumberLine
        | Kind::Diagram
        | Kind::Steps
        | Kind::Progress
        | Kind::Gauge => {}
        Kind::List => {
            let items = card.items.as_deref().unwrap_or(&[]);
            let n = items.len().max(1);
            let top = rows_top;
            let step_y = (size as f32 * 1.5).min((rows_bottom - top) / n as f32);
            // Many items on a wide frame: the text shrinks with its row, so two rows never touch.
            let size = size.min((step_y / 1.25) as u32);
            // With a stagger the first item still waits for its word; the rest follow at the fixed gap.
            let mut at = reveal_times(items, said, start, end);
            if let Some(gap) = card.stagger {
                let first = at[0];
                for (i, t) in at.iter_mut().enumerate() {
                    *t = (first + i as f64 * gap).min(end - 0.3).max(start);
                }
            }
            let (an, x) = if rtl { (9, w * 0.92) } else { (7, w * 0.08) };
            // The item grows from its reading-side corner.
            let motion = card
                .ease
                .map_or(String::new(), |e| scale_in(true, 70, 0, 350, e));
            for (i, it) in items.iter().enumerate() {
                let tags = format!(
                    "\\an{an}\\pos({x},{})\\fs{size}\\fad(200,200){motion}",
                    top + i as f32 * step_y
                );
                out.push(line(at[i], end, &tags, &esc(&format!("• {it}"))));
            }
        }
    }
    out
}

/// Whole percents that add up to 100 (largest remainders), so a legend never reads 99 or 101.
fn whole_percents(values: &[f64]) -> Vec<u32> {
    let total: f64 = values.iter().sum::<f64>().max(1e-9);
    let exact: Vec<f64> = values.iter().map(|v| v / total * 100.0).collect();
    let mut out: Vec<u32> = exact.iter().map(|e| e.floor() as u32).collect();
    let mut by_rest: Vec<usize> = (0..exact.len()).collect();
    by_rest
        .sort_by(|&a, &b| (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())));
    let left = 100u32.saturating_sub(out.iter().sum());
    for &i in by_rest.iter().take(left as usize) {
        out[i] += 1;
    }
    out
}

/// A part of a ring as an ASS shape: from share `a` to share `b` of the whole, clockwise from the top. An inner
/// radius of 0 and the whole range gives a disc.
pub(crate) fn ring_part(cx: f32, cy: f32, r_out: f32, r_in: f32, a: f64, b: f64) -> String {
    use std::f64::consts::TAU;
    let n = (((b - a) * 120.0).ceil() as usize).max(2);
    let pt = |r: f32, t: f64| {
        let ang = t * TAU;
        (cx + r * ang.sin() as f32, cy - r * ang.cos() as f32)
    };
    let mut pts: Vec<(f32, f32)> = (0..=n)
        .map(|i| pt(r_out, a + (b - a) * i as f64 / n as f64))
        .collect();
    if r_in > 0.0 {
        pts.extend(
            (0..=n)
                .rev()
                .map(|i| pt(r_in, a + (b - a) * i as f64 / n as f64)),
        );
    }
    let mut s = format!("m {:.0} {:.0} l", pts[0].0, pts[0].1);
    for (x, y) in &pts[1..] {
        s += &format!(" {x:.0} {y:.0}");
    }
    s
}

/// A label that fits its room: the largest size down to 55 percent where its wrapped lines fit `max_h`. Returns the
/// size, the text and its height. `problem` limits the length, so the smallest size fits on every frame.
fn fit_label(
    joined: &dyn Fn(&str, u32, f32) -> String,
    l: &str,
    base: u32,
    max_w: f32,
    max_h: f32,
) -> (u32, String, f32) {
    let floor = (base * 55 / 100).max(1);
    let mut s = base;
    loop {
        let t = joined(l, s, max_w);
        let height = s as f32 * 1.3 * (t.matches("\\N").count() + 1) as f32;
        if height <= max_h || s <= floor {
            return (s, t, height);
        }
        s -= 1;
    }
}

/// A `plot` card: the panel, the two axes, the function written above, and the curve drawing itself left to right
/// (maths reads left to right in Arabic too). The plot stays above the captions (bottom at 55 percent).
fn plot_events(
    card: &Card,
    start: f64,
    end: f64,
    f: &Frame,
    latin: f32,
    joined: &dyn Fn(&str, u32, f32) -> String,
) -> Vec<String> {
    let (w, h, size) = (f.w as f32, f.h as f32, f.size);
    let mut out = Vec::new();
    let text = card.text.as_deref().unwrap_or("");
    // Up to 3 functions on one plot (`x^2; 2x + 1`), each in its own colour; the first gets the zero crossings.
    let funcs = crate::plot::curves(text);
    let Ok(exprs) = funcs
        .iter()
        .take(crate::plot::MAX_CURVES)
        .map(|t| crate::plot::parse(t))
        .collect::<Result<Vec<_>, _>>()
    else {
        return out;
    };
    let Some(e) = exprs.first().cloned() else {
        return out;
    };
    let (a, b) = match card.values.as_deref() {
        Some([a, b]) => (*a, *b),
        _ => crate::plot::RANGE,
    };
    let sampled: Vec<_> = exprs.iter().map(|e| crate::plot::sample(e, a, b)).collect();
    let pts = sampled[0].0.clone();
    let y0 = sampled.iter().map(|s| s.1).fold(f64::INFINITY, f64::min);
    let y1 = sampled
        .iter()
        .map(|s| s.2)
        .fold(f64::NEG_INFINITY, f64::max);
    // A y range from the brief takes the place of the curves' own, and a curve is cut at it.
    let set_y = match card.y.as_deref() {
        Some([lo, hi]) => Some((*lo, *hi)),
        _ => None,
    };
    let (y0, y1) = set_y.unwrap_or((y0, y1));
    const COLOURS: [&str; 3] = [crate::captions::ACCENT, "&H00FFD040", "&H00B070FF"];
    // A label that only repeats the function would show it twice (hand eval, 4 Nov 2026): it is left out.
    let key = |t: &str| {
        crate::plot::pretty(t.rsplit('=').next().unwrap_or(t))
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
    };
    let label = card.label.as_deref().filter(|l| key(l) != key(text));
    // A little room above and below the curve; the labels name the curve's own range.
    let (lo, hi) = (y0 - (y1 - y0) * 0.08, y1 + (y1 - y0) * 0.08);
    let small = ((size * 4 / 5) as f32 * latin) as u32;
    // Maths reads left to right in every language and its digits are Latin: the Latin font and a left-to-right
    // mark (in an Arabic video the Arabic font drew a small "2" and bidi turned "-3" into "3-").
    let math = |t: &str| format!("{{\\fnNoto Sans}}\u{200E}{}", esc(t));
    // The function, as people read it, in the accent colour.
    let read = |f: &str| crate::plot::pretty(f.rsplit('=').next().unwrap_or(f).trim());
    let eq = if funcs.len() == 1 {
        math(&format!("y = {}", read(funcs[0])))
    } else {
        // Each function in the colour of its curve, so the line is also the legend.
        let mut s = String::from("{\\fnNoto Sans}\u{200E}");
        for (i, f) in funcs.iter().take(crate::plot::MAX_CURVES).enumerate() {
            let gap = if i > 0 { "   " } else { "" };
            s += &format!(
                "{{\\1c{}&}}{gap}{}",
                COLOURS[i],
                esc(&format!("y = {}", read(f)))
            );
        }
        s
    };
    // Written on one line: it shrinks to the room it has (about 0.55 em a letter), else it ran off a 4:3 frame.
    let eq_len: usize = funcs
        .iter()
        .take(crate::plot::MAX_CURVES)
        .map(|t| read(t).chars().count() + 7)
        .sum::<usize>()
        - 3;
    let eq_tags = |x: f32, y: f32, room: f32| {
        let s = (size as f32 * latin).min(room / eq_len as f32 / 0.55) as u32;
        format!(
            "\\an8\\pos({x:.0},{y:.0})\\fs{s}\\1c{}&\\fad(200,200)",
            crate::captions::ACCENT
        )
    };
    // The y range is written in a margin left of the plot, so no label box sits on the curve.
    let margin = small as f32 * 3.6;
    let pad = size as f32 * 0.6;
    let (plot_top, plot_bottom, xl, xr, panel_top);
    if w > h && w < h * 1.6 {
        // A 4:3 frame: too narrow for a column beside the plot, too low for two rows above it. The title and the
        // function share one row, the plot takes the full width under it.
        let top = h * 0.08;
        // Up to two lines of title at its largest; a long one shrinks (a tall title once left the plot upside down).
        let eq_h = size as f32 * 1.3;
        let mut used = eq_h;
        if let Some(l) = label {
            // Two or three functions need the width: they take a row under the title.
            let beside = eq_len as f32 * 0.55 * size as f32 * latin * 0.6 <= w * 0.42;
            let (ls, t, th) = fit_label(
                joined,
                l,
                size,
                if beside { w * 0.42 } else { w * 0.86 },
                h * 0.16,
            );
            let x = if beside { w * 0.27 } else { w / 2.0 };
            let tags = format!("\\an8\\pos({x:.0},{top:.0})\\fs{ls}\\fad(200,200)");
            out.push(line(start, end, &tags, &t));
            out.push(if beside {
                used = th.max(eq_h);
                line(start, end, &eq_tags(w * 0.73, top, w * 0.42), &eq)
            } else {
                used = th + eq_h;
                line(start, end, &eq_tags(w / 2.0, top + th, w * 0.86), &eq)
            });
        } else {
            out.push(line(start, end, &eq_tags(w / 2.0, top, w * 0.86), &eq));
        }
        panel_top = top - size as f32 * 0.3;
        plot_top = top + used + small as f32 * 0.6;
        // Above a two-line caption, which starts higher in a taller caption font (Arabic).
        plot_bottom = (h * if latin < 0.95 { 0.66 } else { 0.72 } - small as f32 * 2.4)
            .max(plot_top + size as f32 * 2.0);
        let pw = w * 0.92 - margin - 2.0 * pad;
        xl = (w - pw - margin) / 2.0 + margin;
        xr = xl + pw;
    } else if w > h {
        // A wide frame: the caption size is large against the height, so the title and the function stand in a
        // column on the left and the plot on the right, ending by 64 percent of the height (above the captions).
        plot_top = h * 0.08 + small as f32;
        plot_bottom = (h * 0.64 - small as f32 * 2.4).max(plot_top + size as f32 * 2.0);
        let pw = ((plot_bottom - plot_top) * 2.0).min(w * 0.5);
        xr = w * 0.94 - pad;
        xl = xr - pw;
        panel_top = plot_top - small as f32;
        let col = (xl - margin - pad) * 0.5;
        let mut y = (plot_top + plot_bottom) / 2.0 - size as f32 * 1.5;
        if let Some(l) = label {
            let t = joined(l, size, col * 1.7);
            // Anchored at its bottom: a tall Arabic title must not leave the top of the frame.
            let lines = t.matches("\\N").count() + 1;
            y = y.max(size as f32 * 1.3 * lines as f32);
            let tags = format!("\\an2\\pos({col:.0},{y:.0})\\fs{size}\\fad(200,200)");
            out.push(line(start, end, &tags, &t));
        }
        y += size as f32 * 0.6;
        out.push(line(start, end, &eq_tags(col, y, col * 1.9), &eq));
    } else {
        let mut top = h * 0.15;
        if let Some(l) = label {
            let tags = format!("\\an8\\pos({},{top})\\fs{size}\\fad(200,200)", w / 2.0);
            out.push(line(start, end, &tags, &joined(l, size, w * 0.86)));
            top += size as f32 * 2.0;
        }
        out.push(line(start, end, &eq_tags(w / 2.0, top, w * 0.9), &eq));
        panel_top = top - size as f32 * 0.4;
        plot_top = top + size as f32 * 1.8;
        // Above the captions: the plot ends by 55 percent of a tall frame, 71 percent of a wide one.
        plot_bottom = (h * if f.w > f.h { 0.71 } else { 0.55 })
            .min(plot_top + w * 0.6)
            .max(plot_top + size as f32 * 2.0);
        // The margin grows with the Arabic font size; the panel stays inside 4 to 96 percent of the width (the card
        // golden plot-ar ran off the right edge at 0.64 of the width).
        let pw = (w * 0.92 - margin - 2.0 * pad).min(w * 0.64);
        xl = (w - pw - margin) / 2.0 + margin;
        xr = xl + pw;
    }
    let px = |x: f64| xl + ((x - a) / (b - a)) as f32 * (xr - xl);
    let py = |y: f64| plot_bottom - ((y - lo) / (hi - lo)) as f32 * (plot_bottom - plot_top);
    let (bx0, by0, bx1, by1) = (
        xl - margin - pad,
        panel_top,
        xr + pad,
        plot_bottom + small as f32 * 2.4,
    );
    let shape = "\\an7\\pos(0,0)\\bord0\\shad0\\4a&HFF&";
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
        format!("m {x0:.1} {y0:.1} l {x1:.1} {y0:.1} l {x1:.1} {y1:.1} l {x0:.1} {y1:.1} ")
    };
    // First, under the title and the function: drawn over them it dimmed both (seen on 4:3, 5 Nov 2026).
    out.insert(
        0,
        line(
            start,
            end,
            &format!("{shape}\\1c&H000000&\\1a&H90&\\fad(200,200)\\p1"),
            &rect(bx0, by0, bx1, by1),
        ),
    );
    // Axes through zero when zero is in range, else along the plot's edge.
    let t = (size as f32 * 0.14).max(3.0);
    let ax_y = py(if lo <= 0.0 && 0.0 <= hi { 0.0 } else { lo });
    let ax_x = px(if a <= 0.0 && 0.0 <= b { 0.0 } else { a });
    // The grid, when asked: thin, a quarter white, under the axes and the curves, so the curves stay the subject.
    if card.grid == Some(true) {
        let g = (t / 10.0).max(0.5);
        let mut d = String::new();
        for x in crate::plot::grid_lines(a, b) {
            d += &rect(px(x) - g, plot_top, px(x) + g, plot_bottom);
        }
        for y in crate::plot::grid_lines(lo, hi) {
            d += &rect(xl, py(y) - g, xr, py(y) + g);
        }
        out.push(line(
            start,
            end,
            &format!("{shape}\\1c&HFFFFFF&\\1a&HC0&\\fad(200,200)\\p1"),
            d.trim_end(),
        ));
    }
    let axes = rect(xl, ax_y - t / 4.0, xr, ax_y + t / 4.0)
        + &rect(ax_x - t / 4.0, plot_top, ax_x + t / 4.0, plot_bottom);
    out.push(line(
        start,
        end,
        &format!("{shape}\\1c&HFFFFFF&\\1a&H60&\\fad(200,200)\\p1"),
        &axes,
    ));
    // The x range under the plot, the y range beside the y axis.
    let num = |v: f64| {
        // A root found at -0.0000001 reads "0", not "-0".
        let v = if v.abs() < 0.05 { 0.0 } else { v };
        let d = if (v - v.round()).abs() < 0.05 { 0 } else { 1 };
        format_number(v, d, v.abs() >= 10000.0)
    };
    let lab = |an: u32, x: f32, y: f32, v: f64| {
        line(
            start + 0.2,
            end,
            &format!("\\an{an}\\pos({x:.0},{y:.0})\\fs{small}\\fad(200,200)"),
            &math(&num(v)),
        )
    };
    // The x range starts at the left end and ends at the right end, so it runs away from the y labels, which stand
    // left of the axis (the two "-4" met in the corner, 4 Nov 2026).
    out.push(lab(7, xl, plot_bottom + small as f32 * 0.9, a));
    out.push(lab(9, xr, plot_bottom + small as f32 * 0.9, b));
    for v in [y0, y1] {
        let y = py(v);
        out.push(lab(6, xl - t * 2.5, y, v));
        out.push(line(
            start,
            end,
            &format!("{shape}\\1c&HFFFFFF&\\1a&H60&\\fad(200,200)\\p1"),
            &rect(xl - t * 1.5, y - t / 4.0, xl, y + t / 4.0),
        ));
    }
    // The curve: one drawing of short quads, shown through a clip that sweeps across. A sample with no value or
    // outside the y range breaks the curve (asymptotes, sqrt of a negative).
    let (cut_lo, cut_hi) = if set_y.is_some() { (y0, y1) } else { (lo, hi) };
    let inside = |y: f64| y.is_finite() && y >= cut_lo && y <= cut_hi;
    let draw = |pts: &[(f64, f64)]| -> String {
        let mut curve = String::new();
        for s in pts.windows(2) {
            let ((x0, y0), (x1, y1)) = (s[0], s[1]);
            if !(inside(y0) && inside(y1)) {
                continue;
            }
            let (x0, y0, x1, y1) = (px(x0), py(y0), px(x1), py(y1));
            let (dx, dy) = (x1 - x0, y1 - y0);
            let len = (dx * dx + dy * dy).sqrt().max(0.01);
            // A little past each end, so the joins have no gaps.
            let (ux, uy) = (dx / len * t / 2.0, dy / len * t / 2.0);
            let (nx, ny) = (-uy, ux);
            curve += &format!(
                "m {:.1} {:.1} l {:.1} {:.1} l {:.1} {:.1} l {:.1} {:.1} ",
                x0 - ux + nx,
                y0 - uy + ny,
                x1 + ux + nx,
                y1 + uy + ny,
                x1 + ux - nx,
                y1 + uy - ny,
                x0 - ux - nx,
                y0 - uy - ny
            );
        }
        curve
    };
    // Where the curve crosses zero: a dot on the axis and its x value under it, shown as the sweep reaches it.
    if lo <= 0.0 && 0.0 <= hi {
        // A value that would touch the one before it is left out; its dot stays (values at -3.1 and 0 met on a
        // narrow plot).
        let mut last_right = f32::MIN;
        for r in crate::plot::roots(&e, &pts, lo, hi) {
            let x = px(r);
            let at = start + 0.4 + 1.6 * ((x - bx0) / (bx1 - bx0)) as f64;
            let d = t * 1.3;
            out.push(line(
                at,
                end,
                &format!("{shape}\\1c&HFFFFFF&\\fad(120,200)\\p1"),
                &rect(x - d, ax_y - d, x + d, ax_y + d),
            ));
            // Under the axis on the side the curve is not: left of a falling crossing, right of a rising one.
            let h = (b - a) * 0.01;
            let rising = e.eval(r + h) > e.eval(r - h);
            let (an, lx) = if rising { (7, x + d) } else { (9, x - d) };
            let text_w = num(r).chars().count() as f32 * small as f32 * 0.65;
            let (left, right) = if rising {
                (lx, lx + text_w)
            } else {
                (lx - text_w, lx)
            };
            if left < last_right {
                continue;
            }
            last_right = right + small as f32 * 0.3;
            let tags = format!(
                "\\an{an}\\pos({lx:.0},{:.0})\\fs{small}\\fad(150,200)",
                ax_y + d
            );
            out.push(line(at, end, &tags, &math(&num(r))));
        }
    }
    let ms = |t: f64| (t * 1000.0).round() as i64;
    for (i, s) in sampled.iter().enumerate() {
        let tags = format!(
            "{shape}\\1c{}&\\clip({bx0:.0},{by0:.0},{bx0:.0},{by1:.0})\\t({},{},\\clip({bx0:.0},{by0:.0},{bx1:.0},{by1:.0}))\\fad(0,200)\\p1",
            COLOURS[i],
            ms(0.4),
            ms(2.0)
        );
        out.push(line(start, end, &tags, draw(&s.0).trim_end()));
    }
    // The running dot, when asked: on the first curve, at the tip of the sweep that opens it, with its x and y
    // beside it. ASS moves a thing in a straight line and cannot change a text, so the way is cut into 24 short
    // steps, each its own event; the value is the one at the step's end. Then it rests on the last point.
    if card.trace == Some(true) {
        const STEPS: usize = 24;
        let r = t * 1.2;
        // A circle of radius r from four curves, centred by \an5.
        let k = r * 0.5523;
        let c = |v: &[f32]| {
            v.iter()
                .map(|n| format!("{n:.1}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let circle = format!(
            "m {} b {} b {} b {} b {}",
            c(&[r, 0.0]),
            c(&[r + k, 0.0, 2.0 * r, r - k, 2.0 * r, r]),
            c(&[2.0 * r, r + k, r + k, 2.0 * r, r, 2.0 * r]),
            c(&[r - k, 2.0 * r, 0.0, r + k, 0.0, r]),
            c(&[0.0, r - k, r - k, 0.0, r, 0.0])
        );
        let dot = "\\an5\\bord0\\shad0\\4a&HFF&\\1c&HFFFFFF&";
        // The sweep runs over the panel's width in 1.6 s, as for the zero crossings.
        let at = |x: f32| start + 0.4 + 1.6 * ((x - bx0) / (bx1 - bx0)) as f64;
        // Digits by the size of the range: 0.05 steps on a short one, whole numbers on a long one.
        let dec = |span: f64| match span {
            s if s >= 50.0 => 0,
            s if s >= 5.0 => 1,
            _ => 2,
        };
        let val = |v: f64, d: usize| {
            // No "-0.0".
            let v = if v.abs() < 0.5 * 10f64.powi(-(d as i32)) {
                0.0
            } else {
                v
            };
            format_number(v, d, v.abs() >= 10000.0)
        };
        let (dx, dy) = (dec(b - a), dec(y1 - y0));
        let text = |x: f64, y: f64| format!("({}, {})", val(x, dx), val(y, dy));
        // The label, a little smaller than the scale's numbers, stands up and right of the dot, and left of it
        // where it would leave the panel on the right. On the left it keeps off the drawn curve: over the dot when
        // the curve rises to it, under it when the curve falls. With no room in the plot on that side, it stands
        // beside the dot. Returns the anchor and the offset from the dot.
        let ts = small * 3 / 4;
        // No box behind it, only a thin dark edge: a box hid the end of the curve (seen 10 Nov 2026).
        let thin = format!("\\fs{ts}\\bord{}\\shad0\\4a&HFF&", (ts / 14).max(2));
        let place = |x: f32, y: f32, rising: bool, s: &str| {
            let gap = r * 1.5;
            // Digits are 0.42 of the font size wide, brackets and points less: 0.45 a character is on the safe
            // side (0.65 flipped the label at the middle of the plot, onto the curve, seen 10 Nov 2026).
            let (wide, high) = (s.chars().count() as f32 * ts as f32 * 0.45, ts as f32 * 1.2);
            // Left only when there is more room there: a very long value stays on the wider side.
            let left = x + gap + wide > bx1 && x - bx0 > bx1 - x;
            let row = if left && !rising {
                if y + gap + high <= plot_bottom {
                    2
                } else {
                    1
                }
            } else if y - gap - high >= plot_top {
                0
            } else {
                1
            };
            (
                [1, 4, 7][row] + if left { 2 } else { 0 },
                if left { -gap } else { gap },
                [-gap, 0.0, gap][row],
            )
        };
        let way: Vec<(f64, f64)> = (0..=STEPS)
            .map(|k| {
                let x = a + (b - a) * k as f64 / STEPS as f64;
                (x, e.eval(x))
            })
            .collect();
        for s in way.windows(2) {
            // Hidden where the curve is broken or outside the y range.
            if !(inside(s[0].1) && inside(s[1].1)) {
                continue;
            }
            let (xa, ya, xb, yb) = (px(s[0].0), py(s[0].1), px(s[1].0), py(s[1].1));
            let (t0, t1) = (at(xa), at(xb).min(end));
            if t0 >= t1 {
                break;
            }
            out.push(line(
                t0,
                t1,
                &format!("{dot}\\move({xa:.1},{ya:.1},{xb:.1},{yb:.1})\\p1"),
                &circle,
            ));
            let label = text(s[1].0, s[1].1);
            let (an, ox, oy) = place(xb, yb, s[1].1 > s[0].1, &label);
            let tags = format!(
                "\\an{an}\\move({:.1},{:.1},{:.1},{:.1}){thin}",
                xa + ox,
                ya + oy,
                xb + ox,
                yb + oy
            );
            out.push(line(t0, t1, &tags, &math(&label)));
        }
        if let Some(&(x, y)) = way.iter().rev().find(|p| inside(p.1)) {
            let (xp, yp) = (px(x), py(y));
            if at(xp) < end {
                out.push(line(
                    at(xp),
                    end,
                    &format!("{dot}\\pos({xp:.1},{yp:.1})\\fad(0,200)\\p1"),
                    &circle,
                ));
                let label = text(x, y);
                let rising = y > e.eval(x - (b - a) / STEPS as f64);
                let (an, ox, oy) = place(xp, yp, rising, &label);
                let tags = format!(
                    "\\an{an}\\pos({:.1},{:.1}){thin}\\fad(0,200)",
                    xp + ox,
                    yp + oy
                );
                out.push(line(at(xp), end, &tags, &math(&label)));
            }
        }
    }
    out
}

/// When the card shows inside its scene (`from` to `end`, with the scene's spoken words): from the `on` word, for
/// `hold` seconds. With neither field, the scene's own times.
pub fn window(
    card: &Card,
    from: f64,
    end: f64,
    said: &[crate::captions::Word],
) -> Result<(f64, f64), String> {
    // No silent fallback: a mark whose word the voice did not give stops the run, as a card's own `on` does.
    if let Some(m) = card
        .mark
        .iter()
        .flatten()
        .find(|m| word_start(&m.on, said).is_none())
    {
        return Err(format!(
            "the voice's words do not have the `on` word \"{}\" of the mark on \"{}\"",
            m.on, m.part
        ));
    }
    // A formula with `then` keeps `on` for its change (see `events`).
    let start = match card.on.as_deref().filter(|_| card.then.is_none()) {
        Some(on) => {
            let t = word_start(on, said).ok_or_else(|| {
                format!("the voice's words do not have the card's `on` word \"{on}\"")
            })?;
            // Not before the scene's own start (the hook's wait), and with 0.9 s left to be seen.
            t.min(end - 0.9).max(from)
        }
        None => from,
    };
    // `events` shows a card for at least 0.5 s, so a shorter hold is recorded as that.
    let end = card.hold.map_or(end, |h| (start + h.max(0.5)).min(end));
    // A mark needs the card on screen at its word, after the 1.3 s the formula takes to write itself.
    for m in card.mark.iter().flatten() {
        let t = word_start(&m.on, said).unwrap_or(start);
        if t >= end || end - start <= 1.3 {
            return Err(format!(
                "the mark on \"{}\" has no time: its word \"{}\" is said at {t:.1} s and the card shows from {start:.1} to {end:.1} s (give it a longer `hold`, or none)",
                m.part, m.on
            ));
        }
    }
    Ok((start, end))
}

/// Sound cues for a card: a pop when it (or each list item) appears, a tick per counting step of a stat.
pub fn cues(
    card: &Card,
    start: f64,
    end: f64,
    said: &[crate::captions::Word],
) -> Vec<(f64, crate::audio::Sfx)> {
    use crate::audio::Sfx;
    match card.kind {
        Kind::List | Kind::Timeline => {
            reveal_times(card.items.as_deref().unwrap_or(&[]), said, start, end)
                .into_iter()
                .map(|t| (t, Sfx::Pop))
                .collect()
        }
        Kind::Stat
            if card.text.as_deref().and_then(split_number).is_some()
                && (start + 0.8).min(end) > start + 0.2 =>
        {
            let d = (start + 0.8).min(end) - start;
            (0..12)
                .map(|k| (start + k as f64 / 12.0 * d, Sfx::Tick))
                .collect()
        }
        Kind::Bars => (0..card.items.as_ref().map_or(0, |v| v.len()))
            .map(|i| (start + 0.2 + i as f64 * 0.15, Sfx::Pop))
            .collect(),
        Kind::Line | Kind::Formula | Kind::Plot | Kind::Donut => vec![(start + 0.3, Sfx::Whoosh)],
        Kind::Code | Kind::Diff | Kind::Terminal => {
            let n = crate::code_card::lines(card.text.as_deref().unwrap_or("")).len();
            let (first, step) = code_step(start, end, n);
            (0..n)
                .map(|i| (start + first + i as f64 * step, Sfx::Tick))
                .collect()
        }
        _ => vec![(start, Sfx::Pop)],
    }
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// A formula turning into another at `t`: shared glyphs glide to their new place in 0.6 s, the first formula's
/// other glyphs and lines fade out in 0.3 s, the second's fade in from 0.3 s.
fn morph(
    a: &[crate::formula::Part],
    (ax, ay): (f32, f32),
    b: &[crate::formula::Part],
    (bx, by): (f32, f32),
    t: f64,
    end: f64,
    measure: &dyn Fn(&str, f32) -> f32,
) -> Vec<String> {
    use crate::formula::{anchor, ass_part, font_size, glyphs, pair, Part, FAMILY};
    let (ga, gb) = (glyphs(a, measure), glyphs(b, measure));
    let ca: Vec<char> = ga.iter().map(|g| g.0).collect();
    let cb: Vec<char> = gb.iter().map(|g| g.0).collect();
    let pairs = pair(&ca, &cb);
    let glyph = |g: &(char, f32, f32, f32), o: (f32, f32)| {
        let (x, y) = anchor(g.1, g.2, g.3, o.0, o.1);
        (x, y, font_size(g.3), esc(&g.0.to_string()))
    };
    let base = format!("\\an1\\fn{FAMILY}\\b0\\bord0\\shad0\\4a&HFF&");
    let mut out = Vec::new();
    for (i, g) in ga.iter().enumerate() {
        let (x, y, fs, c) = glyph(g, (ax, ay));
        match pairs.iter().find(|p| p.0 == i) {
            Some(&(_, j)) => {
                let (x2, y2, fs2, _) = glyph(&gb[j], (bx, by));
                out.push(line(t, end, &format!("{base}\\move({x:.1},{y:.1},{x2:.1},{y2:.1},0,600)\\fs{fs:.0}\\t(0,600,\\fs{fs2:.0})\\fad(0,200)"), &c));
            }
            None => out.push(line(
                t,
                t + 0.3,
                &format!("{base}\\pos({x:.1},{y:.1})\\fs{fs:.0}\\fad(0,300)"),
                &c,
            )),
        }
    }
    for (j, g) in gb.iter().enumerate() {
        if !pairs.iter().any(|p| p.1 == j) {
            let (x, y, fs, c) = glyph(g, (bx, by));
            out.push(line(
                t + 0.3,
                end,
                &format!("{base}\\pos({x:.1},{y:.1})\\fs{fs:.0}\\fad(300,200)"),
                &c,
            ));
        }
    }
    // Fraction bars and root signs are not matched: out with the first formula, in with the second.
    for p in a.iter().filter(|p| matches!(p, Part::Stroke { .. })) {
        let (tags, body) = ass_part(p, ax, ay);
        out.push(line(t, t + 0.3, &format!("{tags}\\fad(0,300)"), &body));
    }
    for p in b.iter().filter(|p| matches!(p, Part::Stroke { .. })) {
        let (tags, body) = ass_part(p, bx, by);
        out.push(line(t + 0.3, end, &format!("{tags}\\fad(300,200)"), &body));
    }
    out
}

/// A time when a card has fully landed and is not yet fading: 75 percent through it (lists and code lines finish
/// by 70 percent), at least 1.5 s in (a formula writes for 1.25 s), and 0.25 s before it ends. A formula with
/// `then` is caught at the end, after its change (see `landed_card`).
pub fn landed(start: f64, end: f64) -> f64 {
    (start + (0.75 * (end - start)).max(1.5))
        .min(end - 0.25)
        .max(start)
}

/// `landed` for a card: a formula with `then` 0.25 s before its end, when the second formula is in place.
pub fn landed_card(card: &Card, start: f64, end: f64) -> f64 {
    if card.then.is_some() {
        (end - 0.25).max(start)
    } else {
        landed(start, end)
    }
}

/// `landed_card`, moved inside the time when every part of the card is on screen (from the last part in, plus
/// 0.5 s of fade, to the first part out, less 0.25 s), as the card's own events give it. A title goes after
/// 2.5 s and a list item comes with its word, so 75 percent of the window missed both (4 Nov 2026 eval: the
/// title shot at 7.16 s showed no card, the list shot showed 3 of 4 items).
pub fn landed_in(card: &Card, events: &[String], start: f64, end: f64) -> f64 {
    let t = landed_card(card, start, end);
    if card.then.is_some() {
        return t;
    }
    let secs = |x: &str| {
        let mut p = x.split(':').map(|v| v.parse::<f64>().ok());
        Some(p.next()?? * 3600.0 + p.next()?? * 60.0 + p.next()??)
    };
    let spans: Vec<(f64, f64)> = events
        .iter()
        .filter_map(|e| {
            let mut f = e.strip_prefix("Dialogue: ")?.split(',').skip(1);
            Some((secs(f.next()?)?, secs(f.next()?)?))
        })
        .collect();
    // The first part out is the first that ends after the last part came in: the steps of a stat's count-up end
    // before its final number starts, so they do not count (test 2, 4 Nov 2026: "2,889 years" for "3,000").
    let Some(lo) = spans.iter().map(|s| s.0).reduce(f64::max) else {
        return t;
    };
    let Some(hi) = spans
        .iter()
        .map(|s| s.1)
        .filter(|&e| e > lo)
        .reduce(f64::min)
    else {
        return t;
    };
    let (lo, hi) = (lo + 0.5, hi - 0.25);
    if lo > hi {
        return (lo + hi) / 2.0;
    }
    t.clamp(lo, hi)
}

/// Which code line the voice talks about, and when: (line, from, to). A line's own words (3 letters or more, found
/// in no other line) are its keys; a spoken word that is a key, or starts with a key of 4 letters or more ("returns"
/// for `return`), lights that line, from the moment it has typed in. The light stays until another line is named or the card
/// ends. One-line snippets get none.
fn code_focus(
    lines: &[String],
    said: &[crate::captions::Word],
    first: f64,
    step: f64,
    end: f64,
) -> Vec<(usize, f64, f64)> {
    if lines.len() < 2 {
        return Vec::new();
    }
    let words = |l: &str| -> Vec<String> {
        l.split(|c: char| !c.is_alphanumeric())
            .map(|w| w.to_lowercase())
            .filter(|w| w.chars().count() >= 3)
            .collect()
    };
    let per_line: Vec<Vec<String>> = lines.iter().map(|l| words(l)).collect();
    let keys: Vec<Vec<&String>> = per_line
        .iter()
        .enumerate()
        .map(|(i, ws)| {
            ws.iter()
                .filter(|w| {
                    !per_line
                        .iter()
                        .enumerate()
                        .any(|(j, o)| j != i && o.contains(w))
                })
                .collect()
        })
        .collect();
    let mut out: Vec<(usize, f64, f64)> = Vec::new();
    for w in said.iter().filter(|w| w.start < end) {
        let spoken = norm(&w.text);
        let hit = keys.iter().position(|ks| {
            ks.iter().any(|k| {
                spoken == **k || (k.chars().count() >= 4 && spoken.starts_with(k.as_str()))
            })
        });
        if let Some(i) = hit {
            // Said while the line is still typing (the voice is often ahead): the light waits for the line.
            let typed = first + i as f64 * step + 0.3;
            let t0 = w.start.max(typed).max(out.last().map_or(0.0, |l| l.1));
            if t0 >= end - 0.3 {
                continue;
            }
            match out.last_mut() {
                Some(last) if last.0 == i => {}
                // A light shorter than 0.4 s would only flicker: the newer line takes its place.
                Some(last) if t0 - last.1 < 0.4 => *last = (i, last.1, end),
                Some(last) => {
                    last.2 = t0;
                    out.push((i, t0, end));
                }
                None => out.push((i, t0, end)),
            }
        }
    }
    out
}

/// When the first code line starts typing, and the time between lines: the whole snippet is in by 60 percent of
/// the card's time, at most 0.6 s a line.
fn code_step(start: f64, end: f64, n: usize) -> (f64, f64) {
    (0.3, ((end - start) * 0.6 / n.max(1) as f64).min(0.6))
}

/// When each list item appears: the moment the voice says one of its first two longer words (a word that ends with
/// it counts, for Arabic prefixes like و), in order; an item never said falls back to an even spread.
pub fn reveal_times(
    items: &[String],
    said: &[crate::captions::Word],
    start: f64,
    end: f64,
) -> Vec<f64> {
    let n = items.len().max(1);
    let step = ((end - start) * 0.7 / n as f64).min(1.2);
    let mut cursor = 0usize;
    let mut prev = start;
    items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let keys: Vec<String> = it
                .split_whitespace()
                .map(norm)
                .filter(|k| k.chars().count() >= 3)
                .take(2)
                .collect();
            let hit = said[cursor.min(said.len())..]
                .iter()
                .position(|w| {
                    let w = norm(&w.text);
                    keys.iter().any(|k| w == *k || w.ends_with(k.as_str()))
                })
                .map(|j| cursor + j);
            let t = match hit {
                Some(j) => {
                    cursor = j + 1;
                    said[j].start
                }
                None => start + i as f64 * step,
            };
            prev = t.max(prev);
            prev
        })
        .collect()
}

/// Adds the Card style after the caption style and the events after the format line.
pub fn insert(ass: &str, font: &str, size: u32, events: &[String]) -> String {
    if events.is_empty() {
        return ass.to_string();
    }
    let style = format!(
        "Style: Card,{font},{size},&H00FFFFFF,&H00FFFFFF,&H66000000,&H66000000,1,0,0,0,100,100,0,0,4,{},0,5,40,40,0,-1",
        size / 4
    );
    let mut out = String::with_capacity(ass.len() + events.len() * 120);
    for l in ass.lines() {
        out.push_str(l);
        out.push('\n');
        if l.starts_with("Style: Cap,") && !ass.contains("Style: Card,") {
            out.push_str(&style);
            out.push('\n');
        }
        if l.starts_with("Format: Layer,") {
            for e in events {
                out.push_str(e);
                out.push('\n');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(kind: Kind, text: Option<&str>, items: Option<Vec<&str>>) -> Card {
        Card {
            kind,
            text: text.map(String::from),
            // Only on a kind that draws a label.
            label: crate::card_kit::allowed(kind)
                .contains(&"label")
                .then(|| "of the water".into()),
            items: items.map(|v| v.into_iter().map(String::from).collect()),
            values: None,
            behind: None,
            then: None,
            on: None,
            at: None,
            ease: None,
            stagger: None,
            from: None,
            hold: None,
            mark: None,
            y: None,
            grid: None,
            trace: None,
            rows: None,
            points: None,
            nodes: None,
            edges: None,
        }
    }

    /// The numbers inside the first `tag(...)` of an event (`\\pos(`, `\\move(`).
    fn nums(l: &str, tag: &str) -> Vec<f32> {
        let inner = l.split(tag).nth(1).unwrap().split(')').next().unwrap();
        inner.split(',').map(|v| v.parse().unwrap()).collect()
    }

    /// An event's start (1) or end (2), in seconds.
    fn secs(l: &str, k: usize) -> f64 {
        let t = l.split(',').nth(k).unwrap();
        t.split(':')
            .fold(0.0, |sum, p| sum * 60.0 + p.parse::<f64>().unwrap())
    }

    #[test]
    fn a_plot_takes_a_y_range_a_grid_and_a_dot_that_runs_along_the_curve() {
        const W: Frame = Frame {
            w: 1920,
            h: 1080,
            size: 84,
        };
        let mut c = card(Kind::Plot, Some("x^2 - 4"), None);
        c.label = None;
        c.values = Some(vec![-4.0, 4.0]);
        let plain = events(&c, 0.0, 6.0, &W, &no_wrap, &[]);
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("grid") && !json.contains("trace") && !json.contains("\"y\""));
        // The plot's own frame, read from its events: the panel, the axes (x axis, then y axis top to bottom) and
        // the two numbers of the y scale with their height.
        let shape = |l: &str| -> Vec<f32> {
            let body = l.rsplit('}').next().unwrap();
            body.split_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect()
        };
        let panel = shape(&plain[0]);
        let (bx0, bx1) = (panel[0], panel[2]);
        let axes = shape(plain.iter().find(|l| l.contains("\\1a&H60&")).unwrap());
        let (xl, xr, top, bottom) = (axes[0], axes[2], axes[9], axes[13]);
        let scale = |ev: &[String]| -> Vec<(f64, f32)> {
            ev.iter()
                .filter(|l| l.contains("\\an6") && !l.ends_with(')'))
                .map(|l| {
                    let v = l.rsplit('}').next().unwrap().trim_start_matches('\u{200E}');
                    (v.parse().unwrap(), nums(l, "\\pos(")[1])
                })
                .collect()
        };
        // The value a height stands for, from the scale.
        let value = |ev: &[String], y: f32| {
            let s = scale(ev);
            s[0].0 + f64::from((y - s[0].1) / (s[1].1 - s[0].1)) * (s[1].0 - s[0].0)
        };
        let is_grid = |l: &&String| l.contains("\\1a&HC0&");
        let is_dot = |l: &&String| l.contains("\\an5\\") && l.contains("\\p1");
        assert!(!plain.iter().any(|l| is_grid(&l) || is_dot(&l)));

        // trace: a dot in steps along the first curve, then at rest on its right end.
        c.trace = Some(true);
        assert_eq!(c.problem(), None);
        let ev = events(&c, 0.0, 6.0, &W, &no_wrap, &[]);
        assert_eq!(
            scale(&ev).iter().map(|s| s.0).collect::<Vec<_>>(),
            [-4.0, 12.0]
        );
        let dots: Vec<&String> = ev.iter().filter(is_dot).collect();
        let moving: Vec<&String> = dots
            .iter()
            .copied()
            .filter(|l| l.contains("\\move("))
            .collect();
        assert_eq!(moving.len(), 24, "{dots:#?}");
        let f = |x: f64| x * x - 4.0;
        let x_of = |px: f32| -4.0 + f64::from((px - xl) / (xr - xl)) * 8.0;
        for t in [1.0, 1.4, 1.8] {
            let seg = moving
                .iter()
                .find(|l| secs(l, 1) <= t && t < secs(l, 2))
                .unwrap_or_else(|| panic!("no dot at {t}: {moving:#?}"));
            let m = nums(seg, "\\move(");
            for (x, y) in [(m[0], m[1]), (m[2], m[3])] {
                // On the curve: the height of the dot stands for f(x), within a few pixels of rounding.
                let (want, got) = (f(x_of(x)), value(&ev, y));
                assert!(
                    (want - got).abs() < 0.25,
                    "{t}: f = {want}, dot at {got}\n{seg}"
                );
            }
            // At the tip of the curve: the sweep that opens the curve reaches the dot's x at the step's start.
            let tip = 0.4 + 1.6 * f64::from((m[0] - bx0) / (bx1 - bx0));
            assert!((secs(seg, 1) - tip).abs() < 0.011, "{t}: {tip}\n{seg}");
        }
        // Left to right, each step from where the last one ended.
        for w in moving.windows(2) {
            let (a, b) = (nums(w[0], "\\move("), nums(w[1], "\\move("));
            assert!(a[2] > a[0] && a[2] == b[0] && a[3] == b[1], "{w:?}");
            assert_eq!(secs(w[0], 2), secs(w[1], 1));
        }
        // At rest on (4, 12), to the card's end, with its value. The label says x and y.
        let rest = dots.last().unwrap();
        let at = nums(rest, "\\pos(");
        assert!(
            (at[0] - xr).abs() < 0.2 && (value(&ev, at[1]) - 12.0).abs() < 0.1,
            "{rest}"
        );
        assert!(rest.contains(",0:00:06.00,"));
        let labels: Vec<&String> = ev
            .iter()
            .filter(|l| l.contains(", ") && l.ends_with(')'))
            .collect();
        assert_eq!(labels.len(), 25, "{labels:#?}");
        assert!(
            labels.last().unwrap().ends_with("(4.0, 12.0)"),
            "{labels:#?}"
        );
        // The label stands right of the dot, and left of it where it would leave the panel on the right.
        // Beside the dot at the two ends, where the curve is at the top of the plot; over it lower down, on the
        // right at first and on the left near the right edge.
        assert!(labels[0].contains("\\an4\\move(") && labels[5].contains("\\an1\\move("));
        assert!(labels[12].contains("\\an1\\move("), "{}", labels[12]);
        assert!(labels[18].contains("\\an3\\move("), "{}", labels[18]);
        assert!(labels.last().unwrap().contains("\\an6\\pos("));
        for l in &labels {
            let n = if l.contains("\\move(") {
                nums(l, "\\move(")
            } else {
                nums(l, "\\pos(")
            };
            let x = n[n.len() - 2];
            let text = l.rsplit('}').next().unwrap().trim_start_matches('\u{200E}');
            let wide = text.chars().count() as f32 * 60.0 * 0.45;
            let (left, right) = if l.contains("\\an6") || l.contains("\\an3") {
                (x - wide, x)
            } else {
                (x, x + wide)
            };
            assert!(
                left >= bx0 && right <= bx1,
                "{left} {right} in {bx0} {bx1}: {l}"
            );
        }
        let doc = insert("Style: Cap,X\nFormat: Layer,\n", "Noto Sans", 100, &ev);
        let b = crate::gates::text_bounds(&doc, (1920, 1080), &[]).unwrap();
        assert_eq!(b.status, "pass", "{}", b.detail);
        // A very long value on a tall frame stays in the frame, on the side with more room.
        let mut long = card(Kind::Plot, Some("x^5"), None);
        long.values = Some(vec![-100.0, 100.0]);
        long.trace = Some(true);
        let lev = events(&long, 0.0, 6.0, &F, &no_wrap, &[]);
        assert!(
            lev.iter().any(|l| l.ends_with("(100, 10,000,000,000)")),
            "{lev:#?}"
        );
        let doc = insert("Style: Cap,X\nFormat: Layer,\n", "Noto Sans", 84, &lev);
        // (The scale's own "10,000,000,000" is too wide for this frame, as before: the gate names it, and no
        // value label of the dot.)
        let b = crate::gates::text_bounds(&doc, (1080, 1920), &[]).unwrap();
        assert!(!b.detail.contains("\"("), "{}", b.detail);
        // Without the dot's events the card is the old one.
        let rest_of: Vec<&String> = ev
            .iter()
            .filter(|l| !is_dot(l) && !labels.contains(l))
            .collect();
        assert_eq!(rest_of, plain.iter().collect::<Vec<_>>());

        // y: the scale names the brief's range, and the same point (4, 12) stands lower than the top.
        c.y = Some(vec![-20.0, 20.0]);
        assert_eq!(c.problem(), None);
        let ev = events(&c, 0.0, 6.0, &W, &no_wrap, &[]);
        let s = scale(&ev);
        assert_eq!((s[0].0, s[1].0), (-20.0, 20.0));
        let at = nums(ev.iter().rfind(is_dot).unwrap(), "\\pos(");
        let want = s[0].1 + (12.0 + 20.0) / 40.0 * (s[1].1 - s[0].1);
        assert!(
            (at[1] - want).abs() < 1.0 && at[1] > s[1].1 + 20.0,
            "{at:?} {want} {s:?}"
        );
        // A curve is cut where it leaves the range: with y to 5 the dot is hidden above it and rests at the last
        // point inside.
        c.y = Some(vec![-5.0, 5.0]);
        let ev = events(&c, 0.0, 6.0, &W, &no_wrap, &[]);
        let dots: Vec<&String> = ev.iter().filter(is_dot).collect();
        assert!(
            dots.len() < 20 && nums(dots.last().unwrap(), "\\pos(")[0] < xr - 10.0,
            "{dots:#?}"
        );

        // grid: one drawing of thin light lines inside the plot, only when asked. x step 1 (9 lines), y step 5
        // (-5, 0, 5, 10).
        c.y = None;
        c.trace = None;
        c.grid = Some(true);
        assert_eq!(c.problem(), None);
        let ev = events(&c, 0.0, 6.0, &W, &no_wrap, &[]);
        let grid: Vec<&String> = ev.iter().filter(is_grid).collect();
        assert_eq!((grid.len(), ev.len()), (1, plain.len() + 1));
        assert_eq!(grid[0].matches("m ").count(), 13, "{}", grid[0]);
        let n = shape(grid[0]);
        for (x, y) in n.as_chunks::<2>().0.iter().map(|p| (p[0], p[1])) {
            assert!(
                x >= xl - 2.0 && x <= xr + 2.0 && y >= top - 2.0 && y <= bottom + 2.0,
                "{x} {y} outside {xl} {xr} {top} {bottom}"
            );
        }
        assert!(n.as_chunks::<8>().0.iter().all(|q| {
            let (w, h) = ((q[2] - q[0]).abs(), (q[5] - q[1]).abs());
            w.min(h) <= 4.0
        }));
        // Under the axes and the curve.
        let pos = |f: &dyn Fn(&&String) -> bool| ev.iter().position(|l| f(&l)).unwrap();
        assert!(pos(&is_grid) < pos(&|l| l.contains("\\1a&H60&")));
    }

    #[test]
    fn plot_extras_are_checked() {
        let mut c = card(Kind::Plot, Some("x^2 - 4"), None);
        c.values = Some(vec![-4.0, 4.0]);
        for bad in [
            vec![1.0],
            vec![2.0, 1.0],
            vec![1.0, 1.0],
            vec![f64::NAN, 1.0],
            vec![0.0, f64::INFINITY],
            vec![0.0, 1.0, 2.0],
        ] {
            c.y = Some(bad.clone());
            assert!(c.problem().unwrap().contains("`y`"), "{bad:?}");
        }
        // A range the curve never enters would give an empty plot.
        c.y = Some(vec![100.0, 200.0]);
        let p = c.problem().unwrap();
        assert!(
            p.contains("no part") && p.contains("100") && p.contains("200"),
            "{p}"
        );
        c.y = Some(vec![0.0, 10.0]);
        assert_eq!(c.problem(), None);
        for field in 0..3 {
            let mut s = card(Kind::Stat, Some("42%"), None);
            match field {
                0 => s.y = Some(vec![0.0, 1.0]),
                1 => s.grid = Some(true),
                _ => s.trace = Some(true),
            }
            assert!(s.problem().unwrap().contains("only a plot"), "{field}");
        }
    }

    #[test]
    fn a_heading_in_text_is_refused_on_a_card_that_does_not_draw_it() {
        let mut c = card(Kind::Bars, Some("Scattering"), Some(vec!["Red", "Blue"]));
        c.values = Some(vec![1.0, 5.9]);
        let p = c.problem().unwrap();
        assert!(p.contains("rename `text` to `label`"), "{p}");
        // The same heading in `label` is drawn.
        c.label = c.text.take();
        assert_eq!(c.problem(), None);
        assert!(events(&c, 0.0, 4.0, &F, &no_wrap, &[])
            .iter()
            .any(|l| l.contains("Scattering")));
        for kind in [Kind::Line, Kind::List, Kind::Compare, Kind::Timeline] {
            let p = card(kind, Some("x"), None).problem().unwrap();
            assert!(p.contains("does not draw `text`"), "{kind:?}: {p}");
        }
    }

    /// Every kind refuses a field it does not draw, from the one table; a field that holds nothing passes.
    #[test]
    fn a_field_that_a_kind_does_not_draw_is_refused_for_every_kind() {
        for (kind, text, items, values) in [
            (Kind::Title, Some("Hi"), None, None),
            (Kind::List, None, Some(vec!["a", "b"]), None),
            (
                Kind::Donut,
                None,
                Some(vec!["a", "b"]),
                Some(vec![1.0, 2.0]),
            ),
            (Kind::Timeline, None, Some(vec!["a", "b"]), None),
        ] {
            let mut c = card(kind, text, items);
            c.values = values;
            assert_eq!(c.problem(), None, "{kind:?}");
            c.label = Some("  ".into());
            c.behind = Some(false);
            assert_eq!(c.problem(), None, "{kind:?}: nothing to draw");
            c.label = Some("A heading".into());
            let p = c.problem().unwrap();
            assert!(
                p.contains("does not draw `label`: remove it (it takes `"),
                "{kind:?}: {p}"
            );
        }
        let mut c = card(Kind::Title, Some("Hi"), Some(vec!["a"]));
        assert!(c
            .problem()
            .unwrap()
            .contains("a title card does not draw `items`"));
        c.items = None;
        c.values = Some(vec![1.0]);
        assert!(c.problem().unwrap().contains("does not draw `values`"));
    }

    fn mark(part: &str, on: &str) -> Mark {
        Mark {
            part: part.into(),
            on: on.into(),
            style: None,
        }
    }

    #[test]
    fn a_formula_part_takes_the_accent_from_its_word() {
        let said = [
            word("This", 1.0),
            word("term", 2.0),
            word("is", 2.5),
            word("energy.", 3.0),
            word("Mass", 5.0),
        ];
        let accent = format!("\\1c{}&", crate::captions::ACCENT);
        let mut c = card(Kind::Formula, Some("E = mc^2"), None);
        let plain = events(&c, 0.0, 8.0, &F, &no_wrap, &said);
        assert!(plain.iter().all(|l| !l.contains(&accent)));
        // No `mark`: nothing is written to the plan, so an old brief keeps its plan id and its events.
        assert!(!serde_json::to_string(&c).unwrap().contains("mark"));
        c.mark = Some(vec![mark("mc^2", "Energy")]);
        assert_eq!(c.problem(), None);
        assert_eq!(c.on_problem("This term is energy. Mass too."), None);
        assert_eq!(window(&c, 0.0, 8.0, &said), Ok((0.0, 8.0)));
        let ev = events(&c, 0.0, 8.0, &F, &no_wrap, &said);
        // m, c and 2 each get a second event from the word on; E, = and the rest are the events of before.
        assert_eq!(ev.len(), plain.len() + 3, "{ev:#?}");
        for l in &plain {
            let piece = ["}𝑚", "}𝑐", "}2"].iter().any(|g| l.ends_with(g));
            assert_eq!(ev.contains(l), !piece, "{l}");
        }
        for g in ["}𝑚", "}𝑐", "}2"] {
            let two: Vec<&String> = ev.iter().filter(|l| l.ends_with(g)).collect();
            assert_eq!(two.len(), 2, "{g}");
            // Before the word: the normal colour, with the write-on. From the word on: the accent colour.
            assert!(
                two[0].contains(",0:00:00.00,0:00:03.00,")
                    && !two[0].contains(&accent)
                    && two[0].contains("\\clip("),
                "{}",
                two[0]
            );
            assert!(
                two[1].contains(",0:00:03.00,0:00:08.00,") && two[1].contains(&accent),
                "{}",
                two[1]
            );
            assert_eq!(
                two[0].split("\\pos(").nth(1).unwrap().split(')').next(),
                two[1].split("\\pos(").nth(1).unwrap().split(')').next()
            );
        }
        assert!(ev
            .iter()
            .all(|l| !l.contains(&accent) || l.contains(",0:00:03.00,")));
        // `snapshot --cards` takes the frame after the last mark.
        assert!(landed_in(&c, &ev, 0.0, 8.0) >= 3.5);
        // Two marks: each piece from its own word; a piece in both takes the first word.
        c.mark = Some(vec![mark("mc^2", "mass"), mark("c^2", "term")]);
        assert_eq!(c.problem(), None);
        let ev = events(&c, 0.0, 8.0, &F, &no_wrap, &said);
        let from = |g: &str| {
            let l = ev.iter().find(|l| l.ends_with(g) && l.contains(&accent));
            l.unwrap().split(',').nth(1).unwrap().to_string()
        };
        assert_eq!(
            (from("}𝑚"), from("}𝑐"), from("}2")),
            (
                "0:00:05.00".into(),
                "0:00:02.00".into(),
                "0:00:02.00".into()
            )
        );
        // A word said while the formula still writes itself waits for the write-on (1.3 s); a fraction's bar is
        // part of the fraction.
        let mut fr = card(Kind::Formula, Some("v = \\frac{d}{t}"), None);
        fr.mark = Some(vec![mark("\\frac{d}{t}", "this")]);
        assert_eq!(fr.problem(), None);
        let ev = events(&fr, 0.0, 8.0, &F, &no_wrap, &said);
        let lit: Vec<&String> = ev.iter().filter(|l| l.contains(&accent)).collect();
        assert_eq!(lit.len(), 3, "{lit:#?}");
        assert!(lit.iter().all(|l| l.contains(",0:00:01.30,0:00:08.00,")));
        assert!(lit.iter().any(|l| l.contains("\\p1")), "the bar");
        // Arabic folds as the words gate does.
        let ar = [word("هذه", 1.0), word("الطاقة", 2.4)];
        c.mark = Some(vec![mark("E", "الطاقه")]);
        let ev = events(&c, 0.0, 6.0, &F, &no_wrap, &ar);
        assert!(ev
            .iter()
            .any(|l| l.contains(",0:00:02.40,0:00:06.00,") && l.contains(&accent)));
    }

    #[test]
    fn a_mark_that_cannot_be_placed_is_refused() {
        let mut c = card(Kind::Formula, Some("a + a = 2a"), None);
        c.mark = Some(vec![mark("b", "twice")]);
        let p = c.problem().unwrap();
        assert!(p.contains("\"b\"") && p.contains("not a piece"), "{p}");
        c.mark = Some(vec![mark("a", "twice")]);
        let p = c.problem().unwrap();
        assert!(
            p.contains("\"a\"") && p.contains("3 times") && p.contains("longer"),
            "{p}"
        );
        c.mark = Some(vec![mark("2a", "twice")]);
        assert_eq!(c.problem(), None);
        // The word must be in the scene's script, and in the voice's words at render.
        let p = c.on_problem("It is the same thing.").unwrap();
        assert!(
            p.contains("twice") && p.contains("\"2a\"") && p.contains("script"),
            "{p}"
        );
        assert_eq!(c.on_problem("It is twice the same."), None);
        let e = window(&c, 0.0, 8.0, &[word("same", 1.0)]).unwrap_err();
        assert!(e.contains("twice") && e.contains("2a"), "{e}");
        // A `hold` that ends the card before the word, or before the formula has written itself: no silent drop.
        let said = [word("twice", 3.0)];
        c.hold = Some(5.0);
        assert_eq!(window(&c, 0.0, 8.0, &said), Ok((0.0, 5.0)));
        for short in [2.0, 1.0] {
            c.hold = Some(short);
            let e = window(&c, 0.0, 8.0, &said).unwrap_err();
            assert!(e.contains("2a") && e.contains("`hold`"), "{e}");
        }
        c.hold = Some(1.0);
        assert!(window(&c, 0.0, 8.0, &[word("twice", 0.2)]).is_err());
        c.hold = None;
        c.mark = Some(vec![mark("2a", "...")]);
        assert!(c.problem().unwrap().contains("`on`"));
        c.mark = Some(vec![mark(" ", "twice")]);
        assert!(c.problem().unwrap().contains("mark"));
        // At most 4, a formula only, and not with `then`.
        c.mark = Some(vec![mark("2a", "twice"); 5]);
        assert!(c.problem().unwrap().contains("at most 4"));
        c.mark = Some(vec![]);
        assert!(c.problem().unwrap().contains("mark"));
        c.mark = Some(vec![mark("2a", "twice")]);
        c.then = Some("a = a".into());
        assert!(c.problem().unwrap().contains("`mark`"));
        let mut s = card(Kind::Stat, Some("42%"), None);
        // A stat takes a mark on its own words (`card_mark.rs`); a chart does not.
        s.mark = Some(vec![mark("42", "forty")]);
        assert!(s.problem().is_none());
        let mut b = card(Kind::Bars, None, Some(vec!["a", "b"]));
        b.values = Some(vec![1.0, 2.0]);
        assert!(b.problem().is_none());
        b.mark = Some(vec![mark("a", "forty")]);
        assert!(b.problem().unwrap().contains("only a formula, title"));
        c.then = None;
        c.mark = Some(vec![Mark {
            style: Some(MarkStyle::Ring),
            ..mark("2a", "twice")
        }]);
        assert!(c.problem().unwrap().contains("no `style`"));
    }

    fn word(t: &str, s: f64) -> crate::captions::Word {
        crate::captions::Word {
            text: t.into(),
            start: s,
            end: s + 0.3,
        }
    }

    #[test]
    fn a_card_lands_on_its_word_and_hold_ends_it_early() {
        let said = [
            word("Revenue", 2.0),
            word("grew", 2.5),
            word("by", 2.9),
            word("forty", 4.2),
            word("two", 4.6),
            word("percent.", 4.9),
        ];
        let mut c = card(Kind::Stat, Some("42%"), None);
        // Neither field: the scene's own times, so the events are the ones of today.
        assert_eq!(window(&c, 2.0, 8.0, &said), Ok((2.0, 8.0)));
        let plain = events(&c, 2.0, 8.0, &F, &no_wrap, &said);
        assert!(plain[0].contains(",0:00:02.00,") && plain[12].contains("0:00:02.80,0:00:08.00"));
        // `on`: the count starts at the word, not at the scene's start.
        c.on = Some("Forty".into());
        assert_eq!(c.problem(), None);
        let (s, e) = window(&c, 2.0, 8.0, &said).unwrap();
        assert_eq!((s, e), (4.2, 8.0));
        let ev = events(&c, s, e, &F, &no_wrap, &said);
        assert!(
            ev[0].contains(",0:00:04.20,") && ev[0].ends_with("}0%"),
            "{}",
            ev[0]
        );
        assert!(ev[12].contains("0:00:05.00,0:00:08.00") && ev[12].ends_with("}42%"));
        assert!(
            ev.iter().all(|l| !l.contains(",0:00:02.")),
            "nothing at the scene start"
        );
        assert_eq!(cues(&c, s, e, &said)[0].0, 4.2);
        assert!(landed_in(&c, &ev, s, e) >= 5.0);
        // `hold`: the card leaves early, with its fade.
        c.hold = Some(2.0);
        assert_eq!(c.problem(), None);
        let (s, e) = window(&c, 2.0, 8.0, &said).unwrap();
        assert_eq!((s, e), (4.2, 6.2));
        let ev = events(&c, s, e, &F, &no_wrap, &said);
        assert!(
            ev[12].contains("0:00:05.00,0:00:06.20") && ev[12].contains("\\fad("),
            "{}",
            ev[12]
        );
        // A hold past the scene's end stops at the end; hold alone starts with the scene.
        c.hold = Some(60.0);
        assert_eq!(window(&c, 2.0, 8.0, &said), Ok((4.2, 8.0)));
        c.on = None;
        c.hold = Some(1.5);
        assert_eq!(window(&c, 2.0, 8.0, &said), Ok((2.0, 3.5)));
        // A card shows for at least 0.5 s.
        c.hold = Some(0.1);
        assert_eq!(window(&c, 2.0, 8.0, &said), Ok((2.0, 2.5)));
        // The hook's wait stays: a word said before the card may start does not pull it earlier.
        c.on = Some("grew".into());
        c.hold = None;
        assert_eq!(window(&c, 3.0, 8.0, &said), Ok((3.0, 8.0)));
        // A word in the scene's last moments: the card still gets 0.9 s.
        c.on = Some("percent".into());
        let (s, e) = window(&c, 2.0, 5.3, &said).unwrap();
        assert!((s - 4.4).abs() < 1e-9 && e == 5.3, "{s} {e}");
        // Arabic folds as in the words gate.
        let ar = [
            word("ارتفعت", 1.0),
            word("الإيرادات", 1.6),
            word("أربعين", 2.8),
        ];
        c.on = Some("اربعين".into());
        assert_eq!(window(&c, 1.0, 6.0, &ar), Ok((2.8, 6.0)));
    }

    #[test]
    fn a_word_the_scene_does_not_say_is_refused() {
        let mut c = card(Kind::Stat, Some("42%"), None);
        c.on = Some("fifty".into());
        let p = c.on_problem("Revenue grew by forty two percent.").unwrap();
        assert!(p.contains("fifty") && p.contains("script"), "{p}");
        let e = window(&c, 2.0, 8.0, &[word("forty", 4.2)]).unwrap_err();
        assert!(e.contains("fifty"), "{e}");
        c.on = Some("forty".into());
        assert_eq!(c.on_problem("Revenue grew by forty two percent."), None);
        c.on = Some("...".into());
        assert!(c.problem().unwrap().contains("`on`"));
        // hold: above 0, a number, and not far past what the scene can last.
        c.on = None;
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            c.hold = Some(bad);
            assert!(c.problem().unwrap().contains("`hold`"), "{bad}");
        }
        c.hold = Some(500.0);
        assert_eq!(c.problem(), None);
        assert!(c
            .on_problem("Five words in this scene.")
            .unwrap()
            .contains("`hold`"));
        c.hold = Some(3.0);
        assert_eq!(c.on_problem("Five words in this scene."), None);
        // A formula keeps its own `on` (the change to `then`), found or not, and takes no hold with `then`.
        let mut f = card(Kind::Formula, Some("A = \\pi r^2"), None);
        f.then = Some("A = 4\\pi r^2".into());
        f.on = Some("nowhere".into());
        assert_eq!(f.on_problem("The area is four times."), None);
        assert_eq!(window(&f, 2.0, 8.0, &[word("four", 4.0)]), Ok((2.0, 8.0)));
        f.hold = Some(2.0);
        assert!(f.problem().unwrap().contains("`hold`"));
    }

    #[test]
    fn list_items_said_before_the_card_come_with_it() {
        let said = [
            word("Use", 1.0),
            word("solar", 1.2),
            word("power,", 1.5),
            word("recover", 2.4),
            word("the", 2.7),
            word("brine,", 2.8),
            word("and", 3.3),
            word("fix", 3.5),
            word("leaks.", 3.8),
        ];
        let mut c = card(
            Kind::List,
            None,
            Some(vec![
                "Use solar power",
                "Recover the brine",
                "Fix leaks first",
            ]),
        );
        c.on = Some("recover".into());
        let (s, e) = window(&c, 1.0, 9.0, &said).unwrap();
        assert_eq!((s, e), (2.4, 9.0));
        let items = c.items.clone().unwrap();
        assert_eq!(reveal_times(&items, &said, s, e), vec![2.4, 2.4, 3.5]);
    }

    #[test]
    fn ease_and_stagger_move_list_items_and_bars() {
        // List: a fixed gap instead of the voice, each item grows from 70 percent with the ease.
        let mut l = card(Kind::List, None, Some(vec!["Fast", "Free", "Open"]));
        l.label = None;
        l.stagger = Some(0.4);
        l.ease = Some(Ease::Spring);
        assert!(l.problem().is_none());
        let e = events(&l, 2.0, 8.0, &F, &no_wrap, &[]);
        let starts: Vec<&str> = e.iter().map(|x| x.split(',').nth(1).unwrap()).collect();
        let t0 = reveal_times(&["Fast".into()], &[], 2.0, 8.0)[0];
        let secs = |x: &str| x.rsplit(':').next().unwrap().parse::<f64>().unwrap();
        for (i, s) in starts.iter().enumerate() {
            assert!(
                (secs(s) - (t0 + 0.4 * i as f64)).abs() < 0.011,
                "{starts:?}"
            );
        }
        assert!(
            e[0].contains(
                "\\fscx70\\fscy70\\t(0,245,0.5,\\fscx108\\fscy108)\\t(245,350,\\fscx100\\fscy100)"
            ),
            "{}",
            e[0]
        );
        // A short scene: late items still come in before the end.
        let e = events(&l, 2.0, 2.8, &F, &no_wrap, &[]);
        assert!(e
            .iter()
            .all(|x| x.split(',').nth(1).unwrap() <= "0:00:02.50"));
        // The first item waits for its word ("Fast" said at 4.0 s), the rest follow 0.4 s apart.
        let said = [crate::captions::Word {
            text: "fast".into(),
            start: 4.0,
            end: 4.3,
        }];
        let e = events(&l, 2.0, 8.0, &F, &no_wrap, &said);
        let starts: Vec<&str> = e.iter().map(|x| x.split(',').nth(1).unwrap()).collect();
        assert_eq!(starts, ["0:00:04.00", "0:00:04.40", "0:00:04.80"]);
        // Bars: the gap and an ease-out growth.
        let mut b = card(Kind::Bars, None, Some(vec!["2015", "2025"]));
        b.values = Some(vec![550.0, 275.0]);
        b.stagger = Some(0.5);
        b.ease = Some(Ease::Out);
        let e: Vec<String> = events(&b, 1.0, 6.0, &F, &no_wrap, &[])
            .into_iter()
            .filter(|x| x.contains("\\p1") && !x.contains("1a&H90&"))
            .collect();
        assert!(
            e[0].contains("\\fscx0\\t(200,800,0.4,\\fscx100)"),
            "{}",
            e[0]
        );
        assert!(e[1].contains("\\t(700,1300,0.4,"), "{}", e[1]);
        // Only list and bars take them; the gap has bounds.
        let mut t = card(Kind::Title, Some("Hi"), None);
        t.ease = Some(Ease::Out);
        assert!(t.problem().unwrap().contains("list and bars"));
        l.stagger = Some(3.0);
        assert!(l.problem().unwrap().contains("0.05 to 1.5"));
    }
    #[test]
    fn behind_titles_drop_the_box_and_only_titles_may_stand_behind() {
        let ass = insert(
            "[V4+ Styles]\nStyle: Cap,x\n[Events]\nFormat: Layer, Start\n",
            "Noto Sans",
            84,
            &["Dialogue: x".into()],
        );
        let out = behind_style(&ass);
        assert!(out.contains("Style: Card,Noto Sans,84,&H00FFFFFF,&H00FFFFFF,&H66000000,&H66000000,1,0,0,0,100,100,0,0,1,0,0,5,"), "{out}");
        let mut c = card(Kind::Stat, Some("87%"), None);
        c.behind = Some(true);
        assert!(c.problem().unwrap().contains("only a title"));
        let mut t = card(Kind::Title, Some("THE GULF"), None);
        t.behind = Some(true);
        assert_eq!(t.problem(), None);
        let f = Frame {
            w: 1080,
            h: 1920,
            size: 84,
        };
        let ev = events(&t, 0.0, 4.0, &f, &no_wrap, &[]);
        assert!(ev[0].contains("\\pos(540,691.2)\\shad5\\fs252"), "{ev:?}");
        assert!(move_to(&ev[0], 800.0).contains("\\pos(540,800)\\shad5"));
        // The title stops at 2.5 s; an end card holds it to its end.
        assert!(
            ev[0].starts_with("Dialogue: 2,0:00:00.00,0:00:02.50,"),
            "{ev:?}"
        );
        assert!(until(&ev[0], 12.34).starts_with("Dialogue: 2,0:00:00.00,0:00:12.34,Card,"));
        // A title that would wrap at the full size shrinks until it is one line (here: two lines above size 200).
        let two_above_200 = |t: &str, s: u32, _: f32| {
            if s > 200 {
                vec![t.to_string(), t.to_string()]
            } else {
                vec![t.to_string()]
            }
        };
        let ev = events(&t, 0.0, 4.0, &f, &two_above_200, &[]);
        assert!(
            ev[0].contains("\\shad5\\fs182") && !ev[0].contains("\\N"),
            "{ev:?}"
        );
    }

    fn no_wrap(t: &str, _: u32, _: f32) -> Vec<String> {
        vec![t.to_string()]
    }
    const F: Frame = Frame {
        w: 1080,
        h: 1920,
        size: 84,
    };

    #[test]
    fn every_labelled_card_stays_on_a_4_3_frame_with_real_wrapping() {
        // The real caption size and font per language, measured as `make` does.
        use crate::captions::{em_factor, Measurer};
        let latin = em_factor(crate::template::FONTS[0].2).unwrap();
        for (lang, long) in [
            ("en", "The speed of a falling object over time"),
            ("ar", "منحنى الموجة عبر الزمن في التجربة الأولى"),
            (
                "en",
                "The speed of a falling object through air over time in the a",
            ),
            (
                "ar",
                "العلاقة بين الكتلة والطاقة في النظرية النسبية عند سرعة الضوء أ",
            ),
        ] {
            let (_, font) = crate::template::font_for(lang);
            let size = (84.0 * em_factor(font).unwrap() / latin).round() as u32;
            for (w, h) in [(1440u32, 1080u32), (2520, 1080), (1920, 1080), (1080, 1920)] {
                let f = Frame {
                    w,
                    h,
                    size: size * w.min(h) / 1080,
                };
                let wrap = |t: &str, s: u32, max: f32| {
                    let m = Measurer::new(font.to_vec(), s as f32).unwrap();
                    crate::captions::wrap(t, max, |x| m.width(x))
                };
                let kinds = [
                    (Kind::Stat, Some("90%"), None, None),
                    (Kind::Formula, Some("E = mc^2"), None, None),
                    (Kind::Plot, Some("x^3 - x"), None, Some(vec![-2.0, 2.0])),
                    (
                        Kind::Compare,
                        None,
                        Some(vec!["A", "B"]),
                        Some(vec![1.0, 2.0]),
                    ),
                    (
                        Kind::Line,
                        None,
                        Some(vec!["A", "B", "C"]),
                        Some(vec![1.0, 3.0, 2.0]),
                    ),
                    (
                        Kind::Bars,
                        None,
                        Some(vec!["A", "B", "C"]),
                        Some(vec![1.0, 3.0, 2.0]),
                    ),
                ];
                for (kind, text, items, values) in kinds {
                    let mut c = card(kind, text, None);
                    let most = match kind {
                        Kind::Compare => 30,
                        Kind::Plot => 40,
                        _ => 60,
                    };
                    c.label = Some(long.chars().take(most).collect());
                    c.items = items.map(|v| v.iter().map(|x| x.to_string()).collect());
                    c.values = values;
                    assert_eq!(c.problem(), None, "{kind:?}");
                    let ev = events(&c, 0.0, 4.0, &f, &wrap, &[]);
                    // The gate agrees: every text box of every kind stays in the frame.
                    let head = "Style: Cap,X\nFormat: Layer,\n";
                    let family = crate::template::FONTS[usize::from(lang == "ar")].1;
                    let doc = insert(head, family, f.size, &ev);
                    if let Some(b) = crate::gates::text_bounds(&doc, (w, h), &[]) {
                        assert_eq!(b.status, "pass", "{kind:?} {lang} {w}x{h}: {}", b.detail);
                    }
                    let (mut top, mut bottom) = (f32::MAX, f32::MIN);
                    let mut scale_ys = Vec::new();
                    for e in &ev {
                        if let Some(p) = e.split("\\pos(").nth(1).and_then(|p| p.split(')').next())
                        {
                            let (_, y) = p.split_once(',').unwrap();
                            let y: f32 = y.parse().unwrap();
                            if y > 0.0 || !e.contains("\\p1") {
                                top = top.min(y);
                                bottom = bottom.max(y);
                            }
                            if e.contains("\\an6") {
                                scale_ys.push(y);
                            }
                        }
                        // The y of each point of a drawing placed from the frame's corner.
                        if e.contains("\\pos(0,0)") {
                            let body = e.rsplit('}').next().unwrap();
                            let n: Vec<f32> = body
                                .split_whitespace()
                                .filter_map(|x| x.parse().ok())
                                .collect();
                            for y in n.iter().skip(1).step_by(2) {
                                top = top.min(*y);
                                bottom = bottom.max(*y);
                            }
                        }
                    }
                    let tag = format!("{kind:?} {lang} {w}x{h} top {top} bottom {bottom}");
                    assert!(top >= 0.0 && bottom <= h as f32, "{tag}\n{ev:?}");
                    // A plot is never upside down: the larger value stands higher.
                    if kind == Kind::Plot {
                        assert!(
                            scale_ys.len() == 2 && scale_ys[1] < scale_ys[0],
                            "{tag}\n{ev:?}"
                        );
                        // With the grid and the running dot, every value label still stays in the frame.
                        c.grid = Some(true);
                        c.trace = Some(true);
                        let more = events(&c, 0.0, 4.0, &f, &wrap, &[]);
                        assert!(more.len() > ev.len() + 20, "{tag}");
                        let doc = insert(head, family, f.size, &more);
                        let b = crate::gates::text_bounds(&doc, (w, h), &[]).unwrap();
                        assert_eq!(b.status, "pass", "{tag}: {}", b.detail);
                    }
                }
            }
        }
        let mut c = card(Kind::Stat, Some("9"), None);
        c.label = Some("x".repeat(61));
        assert!(c.problem().unwrap().contains("at most 60"));
    }

    #[test]
    fn wide_frames_get_bigger_cards_and_names_beside_their_bars() {
        const W: Frame = Frame {
            w: 1920,
            h: 1080,
            size: 84,
        };
        let title = card(Kind::Title, Some("Hello"), None);
        assert!(events(&title, 0.0, 3.0, &W, &no_wrap, &[])[0].contains("\\fs189"));
        let mut bars = card(Kind::Bars, None, Some(vec!["North", "South"]));
        bars.values = Some(vec![1.0, 2.0]);
        let ev = events(&bars, 0.0, 3.0, &W, &no_wrap, &[]);
        let name = ev.iter().find(|e| e.ends_with("North")).unwrap();
        assert!(name.contains("\\an4"), "{name}");
        // Five list rows stay above a two-line caption and never touch.
        let list = card(
            Kind::List,
            None,
            Some(vec!["a b", "c d", "e f", "g h", "i j"]),
        );
        let ev = events(&list, 0.0, 3.0, &W, &no_wrap, &[]);
        let y = |e: &str| -> f32 {
            let t = e.split("\\pos(").nth(1).unwrap();
            let t = t.split(')').next().unwrap();
            t.split(',').nth(1).unwrap().parse().unwrap()
        };
        assert!(y(&ev[4]) + 126.0 <= 1080.0 * 0.78, "{}", ev[4]);
        assert!(y(&ev[1]) - y(&ev[0]) >= 100.0);
    }

    #[test]
    fn shares_add_up_and_new_kinds_are_checked() {
        assert_eq!(whole_percents(&[1.0, 1.0, 1.0]), vec![34, 33, 33]);
        assert_eq!(whole_percents(&[62.0, 21.0, 17.0]), vec![62, 21, 17]);
        let mut c = card(Kind::Donut, None, Some(vec!["a", "b"]));
        c.values = Some(vec![1.0, 3.0]);
        assert_eq!(c.problem(), None);
        c.values = Some(vec![0.0, 0.0]);
        assert!(c.problem().unwrap().contains("donut"));
        let mut c = card(Kind::Compare, None, Some(vec!["Before", "After"]));
        assert!(c.problem().unwrap().contains("compare"));
        c.values = Some(vec![12.0, 3.0]);
        assert_eq!(c.problem(), None);
        assert!(card(Kind::Timeline, None, Some(vec!["only"]))
            .problem()
            .is_some());
        assert!(card(Kind::Quote, None, None).problem().is_some());
        let long = "x".repeat(161);
        assert!(card(Kind::Quote, Some(&long), None).problem().is_some());
        // A whole ring closes on itself.
        let shape = ring_part(100.0, 100.0, 50.0, 0.0, 0.0, 1.0);
        assert!(shape.starts_with("m 100 50 l"));
        assert!(shape.ends_with(" 100 50"));
    }

    #[test]
    fn numbers_split_and_format() {
        let n = split_number("$1,250.5 m").unwrap();
        assert_eq!(
            (
                n.pre.as_str(),
                n.value,
                n.post.as_str(),
                n.decimals,
                n.commas
            ),
            ("$", 1250.5, " m", 1, true)
        );
        assert_eq!(split_number("87%").unwrap().show(0.0), "0%");
        assert_eq!(split_number("about 3.").unwrap().post, ".");
        assert_eq!(split_number("none"), None);
        assert_eq!(format_number(1234567.0, 0, true), "1,234,567");
        assert_eq!(format_number(12.345, 1, false), "12.3");
        // Arabic-Indic digits count in the same digits.
        let ar = split_number("٩٠٪").unwrap();
        assert!(ar.arabic && ar.value == 90.0);
        assert_eq!(ar.show(45.0), "٤٥٪");
        assert_eq!(
            split_number("١٬٢٥٠٫٥ مليون").unwrap().show(1250.5),
            "١٬٢٥٠٫٥ مليون"
        );
    }

    #[test]
    fn a_stat_counts_from_its_start_number_up_or_down() {
        let mut c = card(Kind::Stat, Some("42%"), None);
        c.from = Some(30.0);
        let e = events(&c, 2.0, 6.0, &F, &no_wrap, &[]);
        assert!(e[0].ends_with("}30%"), "{}", e[0]);
        assert!(e[12].ends_with("}42%"), "{}", e[12]);
        // No step leaves the range between the two numbers.
        for l in &e[..12] {
            let n: f64 = l
                .rsplit('}')
                .next()
                .unwrap()
                .trim_end_matches('%')
                .parse()
                .unwrap();
            assert!((30.0..=42.0).contains(&n), "{l}");
        }
        c.from = Some(90.0);
        let e = events(&c, 2.0, 6.0, &F, &no_wrap, &[]);
        assert!(
            e[0].ends_with("}90%") && e[12].ends_with("}42%"),
            "{} {}",
            e[0],
            e[12]
        );
        // Only a stat with a number takes it.
        let mut t = card(Kind::Title, Some("Hello"), None);
        t.from = Some(1.0);
        assert!(t.problem().unwrap().contains("stat"));
        let mut w = card(Kind::Stat, Some("many"), None);
        w.from = Some(1.0);
        assert!(w.problem().unwrap().contains("number"));
        assert!(c.problem().is_none());
    }

    #[test]
    fn a_stat_counts_up_and_lands_on_the_text() {
        let e = events(
            &card(Kind::Stat, Some("87%"), None),
            2.0,
            6.0,
            &F,
            &no_wrap,
            &[],
        );
        assert_eq!(e.len(), 14, "12 steps, the final value, the label");
        assert!(
            e[0].contains("0:00:02.00") && e[0].ends_with("}0%"),
            "{}",
            e[0]
        );
        assert!(
            e[12].contains("0:00:02.80,0:00:06.00") && e[12].ends_with("}87%"),
            "{}",
            e[12]
        );
    }

    #[test]
    fn lists_reveal_in_order_and_arabic_mirrors() {
        let e = events(
            &card(Kind::List, None, Some(vec!["Dig", "Line", "Share"])),
            0.0,
            6.0,
            &F,
            &no_wrap,
            &[],
        );
        assert_eq!(e.len(), 3);
        assert!(
            e[1].contains("0:00:01.20") && e[1].contains("\\an7"),
            "{}",
            e[1]
        );
        let ar = events(
            &card(Kind::LowerThird, Some("آمنة العلي"), None),
            0.0,
            6.0,
            &F,
            &no_wrap,
            &[],
        );
        assert!(ar[0].contains("\\an3"), "{}", ar[0]);
    }

    #[test]
    fn balanced_wrap_avoids_a_lone_word() {
        // 10 px a character, greedy at 330 px.
        let greedy = |t: &str, _: u32, max: f32| {
            crate::captions::wrap(t, max, |x| x.chars().count() as f32 * 10.0)
        };
        let t = "of drinking water comes from the sea";
        assert_eq!(greedy(t, 0, 330.0).last().unwrap(), "sea");
        let b = balanced(&greedy, t, 0, 330.0);
        assert_eq!(b.len(), 2);
        assert!(b.iter().all(|l| l.chars().count() >= 12), "{b:?}");
    }

    #[test]
    fn list_items_appear_when_spoken() {
        let w = |t: &str, s: f64| crate::captions::Word {
            text: t.into(),
            start: s,
            end: s + 0.3,
        };
        let said = [
            w("Use", 1.0),
            w("solar", 1.2),
            w("power.", 1.5),
            w("Recover", 2.4),
            w("the", 2.7),
            w("brine.", 2.8),
        ];
        let items: Vec<String> = ["Use solar power", "Recover the brine", "Fix leaks first"]
            .map(String::from)
            .into();
        assert_eq!(reveal_times(&items, &said, 0.0, 6.0), vec![1.0, 2.4, 2.4]);
        let ar = [w("أفكارها", 0.0), w("الطاقة", 0.8), w("واستعادة", 2.0)];
        let it: Vec<String> = ["الطاقة الشمسية", "استعادة المحلول"]
            .map(String::from)
            .into();
        assert_eq!(reveal_times(&it, &ar, 0.0, 6.0), vec![0.8, 2.0]);
    }

    #[test]
    fn cues_follow_the_card() {
        use crate::audio::Sfx;
        let stat = cues(&card(Kind::Stat, Some("87%"), None), 2.0, 6.0, &[]);
        assert_eq!(stat.len(), 12);
        assert!(stat.iter().all(|c| c.1 == Sfx::Tick) && stat[0].0 == 2.0);
        let plain = cues(&card(Kind::Stat, Some("many"), None), 2.0, 6.0, &[]);
        assert_eq!(plain, vec![(2.0, Sfx::Pop)]);
        let list = cues(
            &card(Kind::List, None, Some(vec!["a b", "c d"])),
            0.0,
            4.0,
            &[],
        );
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn then_and_on_belong_to_a_formula() {
        let mut c = card(Kind::Formula, Some("A = \\pi r^2"), None);
        c.then = Some("A = 4\\pi r^2".into());
        c.on = Some("four".into());
        assert_eq!(c.problem(), None);
        c.then = Some("\\nope".into());
        assert!(c.problem().unwrap().starts_with("then: formula"));
        c.then = None;
        assert!(c.problem().unwrap().contains("on needs then"));
        let mut t = card(Kind::Title, Some("x"), None);
        t.then = Some("y".into());
        assert!(t.problem().unwrap().contains("only a formula"));
    }

    #[test]
    fn landed_waits_for_every_part_and_leaves_before_the_first_goes() {
        let c = card(Kind::Title, Some("Trees"), None);
        let ev = |a: f64, b: f64| line(a, b, "", "x");
        // A title from 1.6 s to 4.1 s in a window to 9 s: 75 percent (7.15 s) is after it has gone.
        assert!((landed_in(&c, &[ev(1.6, 4.1)], 1.6, 9.0) - 3.85).abs() < 1e-9);
        // A list whose last item comes at 15.6 s: not before 16.1 s.
        let items = [ev(9.0, 18.2), ev(12.0, 18.2), ev(15.6, 18.2)];
        assert!((landed_in(&c, &items, 9.0, 18.2) - 16.1).abs() < 1e-9);
        // A stat counting up in steps to 2.4 s, then the number to 9 s: after the count.
        let count = [ev(1.6, 1.8), ev(1.8, 2.0), ev(2.0, 2.4), ev(2.4, 9.0)];
        assert!(landed_in(&c, &count, 1.6, 9.0) >= 2.9);
        // No events: the plain rule.
        assert_eq!(landed_in(&c, &[], 2.0, 10.0), 8.0);
    }

    #[test]
    fn landed_is_inside_the_card() {
        assert_eq!(landed(2.0, 10.0), 8.0);
        assert_eq!(landed(0.0, 1.8), 1.5);
        assert_eq!(landed(0.0, 1.0), 0.75);
    }

    #[test]
    fn the_spoken_line_lights_up() {
        let w = |t: &str, s: f64| crate::captions::Word {
            text: t.into(),
            start: s,
            end: s + 0.3,
        };
        let lines = vec![
            "def area(r):".to_string(),
            "    return 3.14159 * r ** 2".to_string(),
        ];
        let said = [
            w("This", 0.5),
            w("area", 1.0),
            w("function", 1.3),
            w("returns", 3.0),
            w("pi.", 3.4),
        ];
        assert_eq!(
            code_focus(&lines, &said, 0.3, 0.6, 6.0),
            vec![(0, 1.0, 3.0), (1, 3.0, 6.0)]
        );
        // Said before the line typed in: the light waits for it.
        assert_eq!(
            code_focus(&lines, &[w("returns", 0.5)], 0.3, 0.6, 6.0),
            vec![(1, 1.2, 6.0)]
        );
        assert!(code_focus(&lines[..1], &said, 0.3, 0.6, 6.0).is_empty());
    }

    // G187.
    #[test]
    fn a_diff_marks_changed_lines_and_a_terminal_types_only_commands() {
        let mut c = card(Kind::Diff, Some(" a = 1\n-b = 2\n+b = 3"), None);
        assert!(c.problem().is_none());
        let ev = events(&c, 0.0, 6.0, &F, &no_wrap, &[]);
        let bars = |col: &str| ev.iter().filter(|e| e.contains(col)).count();
        assert_eq!((bars("&H2F5A1F&"), bars("&H2A2A78&")), (1, 1), "{ev:#?}");
        // The same text as plain code has no bar.
        c.kind = Kind::Code;
        let ev = events(&c, 0.0, 6.0, &F, &no_wrap, &[]);
        assert!(!ev.iter().any(|e| e.contains("&H2F5A1F&")));
        c.kind = Kind::Terminal;
        c.text = Some("$ cargo test\ntest result: ok".into());
        let ev = events(&c, 0.0, 6.0, &F, &no_wrap, &[]);
        let of = |t: &str| ev.iter().find(|e| e.contains(t)).unwrap().clone();
        // The command opens over its typing time, the output at once; output has no code colours.
        assert!(!of("cargo").contains("\\t(0,1,"), "{}", of("cargo"));
        assert!(of("result").contains("\\t(0,1,"), "{}", of("result"));
        assert!(of("cargo").contains("&H7BD88F&") && of("result").contains("&HB8B8B8&"));
        // The text lint reads only the label of these cards.
        assert_eq!(c.texts(), vec!["of the water"]);
    }

    #[test]
    fn chart_values_follow_the_digits_of_the_names() {
        assert_eq!(indic("1,250.5"), "١٬٢٥٠٫٥");
        assert_eq!(digits_like(&["٢٠٢٥".to_string()], "3.8".into()), "٣٫٨");
        assert_eq!(digits_like(&["2025".to_string()], "3.8".into()), "3.8");
        let mut c = card(Kind::Line, None, Some(vec!["2005", "2015", "2025"]));
        c.values = Some(vec![1.1, 2.7, 3.8]);
        assert!(c.problem().is_none());
        c.values = Some(vec![1.1, 2.7]);
        assert!(c.problem().unwrap().contains("line card"));
    }

    #[test]
    fn bars_grow_in_turn_from_the_label_side() {
        let mut c = card(Kind::Bars, None, Some(vec!["2015", "2025"]));
        c.values = Some(vec![550.0, 275.0]);
        assert!(c.problem().is_none());
        let e = events(&c, 1.0, 6.0, &F, &no_wrap, &[]);
        assert_eq!(
            e.len(),
            9,
            "title, then label, track, bar and value per row"
        );
        let e: Vec<&String> = e.iter().filter(|x| !x.contains("1a&H90&")).collect();
        assert!(
            e[2].contains("\\fscx0\\t(200,800,\\fscx100)") && e[2].contains("\\p1"),
            "{}",
            e[2]
        );
        assert!(
            e[5].contains("\\t(350,950,"),
            "the second bar starts 0.15 s later: {}",
            e[5]
        );
        let full: f32 = e[2]
            .split(" l ")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let half: f32 = e[5]
            .split(" l ")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!((full / half - 2.0).abs() < 0.02, "{full} {half}");
        assert!(
            e[3].ends_with("}550") && e[3].contains("0:00:01.80"),
            "{}",
            e[3]
        );
        c.values = Some(vec![0.0, 0.0]);
        assert!(c.problem().is_some());
    }

    #[test]
    fn problems_are_named() {
        assert!(card(Kind::List, None, Some(vec![])).problem().is_some());
        assert!(card(Kind::Stat, None, None)
            .problem()
            .unwrap()
            .contains("stat card needs text"));
        assert!(card(Kind::Title, Some("Part 2"), None).problem().is_none());
    }

    #[test]
    fn insert_adds_style_and_events_once() {
        let ass = "[V4+ Styles]\nStyle: Cap,x\n[Events]\nFormat: Layer, Start\nDialogue: 0,a\n";
        let out = insert(ass, "Noto Sans", 84, &["Dialogue: 2,b".into()]);
        assert_eq!(out.matches("Style: Card,").count(), 1);
        assert!(out.contains("Format: Layer, Start\nDialogue: 2,b\nDialogue: 0,a"));
    }
}

#[cfg(test)]
mod latin_runs_tests {
    #[test]
    fn latin_marks_in_arabic_text_name_noto_sans_and_english_text_is_unchanged() {
        assert_eq!(
            super::latin_runs("مجاني وفيه API"),
            "مجاني وفيه {\\fnNoto Sans}API{\\fn}"
        );
        assert_eq!(
            super::latin_runs("السيارة 70%"),
            "السيارة 70{\\fnNoto Sans}%{\\fn}"
        );
        assert_eq!(super::latin_runs("Free API, 70%"), "Free API, 70%");
    }
}
