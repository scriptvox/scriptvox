//! `plot` card: y = f(x) drawn on axes. A small expression language of our own: numbers, `x`, `+ - * / ^`,
//! brackets, implicit products (`2x`, `3sin(x)`, `x(x-1)`), `pi` and `e`, and sin, cos, tan, sqrt, exp, ln, log, abs.
//! Idea: Manim's function graphs on coordinate axes; the design is ScriptVox's own.

#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    X,
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Call(fn(f64) -> f64, Box<Expr>),
}

impl Expr {
    pub fn eval(&self, x: f64) -> f64 {
        match self {
            Expr::Num(v) => *v,
            Expr::X => x,
            Expr::Neg(a) => -a.eval(x),
            Expr::Call(f, a) => f(a.eval(x)),
            Expr::Bin(op, a, b) => {
                let (a, b) = (a.eval(x), b.eval(x));
                match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => a / b,
                    _ => a.powf(b),
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Id(String),
    Op(char),
}

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch.is_ascii_digit() || ch == '.' {
            let j = (i..c.len())
                .find(|&k| !(c[k].is_ascii_digit() || c[k] == '.'))
                .unwrap_or(c.len());
            let t: String = c[i..j].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| format!("bad number {t}"))?));
            i = j;
        } else if ch.is_alphabetic() {
            let j = (i..c.len())
                .find(|&k| !c[k].is_alphabetic())
                .unwrap_or(c.len());
            out.push(Tok::Id(c[i..j].iter().collect()));
            i = j;
        } else if "+-*/^()·×−".contains(ch) {
            out.push(Tok::Op(match ch {
                '·' | '×' => '*',
                '−' => '-',
                o => o,
            }));
            i += 1;
        } else {
            return Err(format!("cannot read '{ch}'"));
        }
    }
    Ok(out)
}

struct P {
    t: Vec<Tok>,
    i: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn eat(&mut self, op: char) -> bool {
        if self.peek() == Some(&Tok::Op(op)) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn sum(&mut self) -> Result<Expr, String> {
        let mut a = self.product()?;
        loop {
            let op = if self.eat('+') {
                '+'
            } else if self.eat('-') {
                '-'
            } else {
                return Ok(a);
            };
            a = Expr::Bin(op, Box::new(a), Box::new(self.product()?));
        }
    }
    fn product(&mut self) -> Result<Expr, String> {
        let mut a = self.unary()?;
        loop {
            let op = if self.eat('*') {
                '*'
            } else if self.eat('/') {
                '/'
            } else if matches!(self.peek(), Some(Tok::Num(_) | Tok::Id(_) | Tok::Op('('))) {
                '*' // 2x, 3sin(x), x(x-1)
            } else {
                return Ok(a);
            };
            a = Expr::Bin(op, Box::new(a), Box::new(self.unary()?));
        }
    }
    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat('-') {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.eat('+') {
            return self.unary();
        }
        let a = self.atom()?;
        if self.eat('^') {
            // Right to left: 2^x^2 is 2^(x^2); -x^2 is -(x^2) because the minus is read first.
            return Ok(Expr::Bin('^', Box::new(a), Box::new(self.unary()?)));
        }
        Ok(a)
    }
    fn atom(&mut self) -> Result<Expr, String> {
        match self.t.get(self.i).cloned() {
            Some(Tok::Num(v)) => {
                self.i += 1;
                Ok(Expr::Num(v))
            }
            Some(Tok::Op('(')) => {
                self.i += 1;
                let e = self.sum()?;
                if !self.eat(')') {
                    return Err("a bracket is not closed".into());
                }
                Ok(e)
            }
            Some(Tok::Id(id)) => {
                self.i += 1;
                let f: fn(f64) -> f64 = match id.as_str() {
                    "x" => return Ok(Expr::X),
                    "pi" | "π" => return Ok(Expr::Num(std::f64::consts::PI)),
                    "e" => return Ok(Expr::Num(std::f64::consts::E)),
                    "sin" => f64::sin,
                    "cos" => f64::cos,
                    "tan" => f64::tan,
                    "sqrt" => f64::sqrt,
                    "exp" => f64::exp,
                    "ln" => f64::ln,
                    "log" => f64::log10,
                    "abs" => f64::abs,
                    _ => return Err(format!("unknown name '{id}' (use x, pi, e, sin, cos, tan, sqrt, exp, ln, log, abs)")),
                };
                if !self.eat('(') {
                    return Err(format!("{id} needs brackets: {id}(x)"));
                }
                let e = self.sum()?;
                if !self.eat(')') {
                    return Err("a bracket is not closed".into());
                }
                Ok(Expr::Call(f, Box::new(e)))
            }
            Some(Tok::Op(o)) => Err(format!("unexpected '{o}'")),
            None => Err("the function ends too soon".into()),
        }
    }
}

/// Reads `x^2 - 2`, `2sin(x)`, `e^(-x^2)`. A leading `y =` or `f(x) =` is allowed.
pub fn parse(s: &str) -> Result<Expr, String> {
    let s = s.rsplit('=').next().unwrap_or(s);
    let mut p = P { t: lex(s)?, i: 0 };
    let e = p.sum()?;
    if p.i < p.t.len() {
        return Err(format!("cannot read past {:?}", p.t[p.i]));
    }
    Ok(e)
}

/// The default x range when the card gives no `values`.
pub const RANGE: (f64, f64) = (-5.0, 5.0);
const SAMPLES: usize = 160;

/// The curve: (x, y) samples (y not finite where the function has no value) and the y range of the curve.
pub fn sample(e: &Expr, a: f64, b: f64) -> (Vec<(f64, f64)>, f64, f64) {
    let pts: Vec<(f64, f64)> = (0..SAMPLES)
        .map(|i| {
            let x = a + (b - a) * i as f64 / (SAMPLES - 1) as f64;
            (x, e.eval(x))
        })
        .collect();
    let mut ys: Vec<f64> = pts.iter().map(|p| p.1).filter(|y| y.is_finite()).collect();
    ys.sort_by(f64::total_cmp);
    if ys.is_empty() {
        return (pts, -1.0, 1.0);
    }
    // The whole curve, unless a few samples run off far (asymptotes: tan, 1/x), then the 3rd to 97th percentile.
    // ponytail: a function that only spikes near one end loses that end; add a `y` range on the card if a review
    // needs it.
    let q = |f: f64| ys[((ys.len() - 1) as f64 * f).round() as usize];
    let (mut lo, mut hi) = (ys[0], ys[ys.len() - 1]);
    let (clo, chi) = (q(0.03), q(0.97));
    if ys.len() >= 40 && chi - clo > 1e-9 && hi - lo > 4.0 * (chi - clo) {
        (lo, hi) = (clo, chi);
    }
    if hi - lo < 1e-9 {
        (lo, hi) = (lo - 1.0, hi + 1.0);
    }
    (pts, lo, hi)
}

/// Where the curve crosses zero, found from sign changes between samples inside the y range (so the jump of tan at
/// an asymptote is not a root), then refined by halving. At most 4: more would crowd the axis, so none are returned.
pub fn roots(e: &Expr, pts: &[(f64, f64)], lo: f64, hi: f64) -> Vec<f64> {
    let ok = |y: f64| y.is_finite() && y >= lo && y <= hi;
    let mut out: Vec<f64> = Vec::new();
    let span = pts.last().map_or(1.0, |p| p.0) - pts.first().map_or(0.0, |p| p.0);
    for w in pts.windows(2) {
        let ((mut a, ya), (mut b, yb)) = (w[0], w[1]);
        if !(ok(ya) && ok(yb)) || ya.signum() == yb.signum() && ya != 0.0 {
            continue;
        }
        let r = if ya == 0.0 {
            a
        } else {
            let sa = ya.signum();
            for _ in 0..40 {
                let m = (a + b) / 2.0;
                if e.eval(m).signum() == sa {
                    a = m;
                } else {
                    b = m;
                }
            }
            (a + b) / 2.0
        };
        if out.last().is_none_or(|l| (r - l).abs() > span * 0.01) {
            out.push(r);
        }
    }
    if out.len() > 4 {
        out.clear();
    }
    out
}

/// Grid lines between `lo` and `hi`: the multiples of a round step (1, 2 or 5 times a power of ten).
/// The step is the smallest such that is at least an eighth of the range, so there are 3 to 9 lines.
pub fn grid_lines(lo: f64, hi: f64) -> Vec<f64> {
    let raw = (hi - lo) / 8.0;
    if !(raw.is_finite() && raw > 0.0) {
        return vec![];
    }
    let p = 10f64.powf(raw.log10().floor());
    let step = p * match raw / p {
        m if m <= 1.0 => 1.0,
        m if m <= 2.0 => 2.0,
        m if m <= 5.0 => 5.0,
        _ => 10.0,
    };
    // A hair of slack: 0.3 / 0.05 is a little under 6 in binary.
    let (first, last) = (
        (lo / step - 1e-9).ceil() as i64,
        (hi / step + 1e-9).floor() as i64,
    );
    (first..=last).map(|k| k as f64 * step).collect()
}

/// A problem with a y range from the brief (`[min, max]`, checked by the card): no curve has a point inside it,
/// so the plot would be empty.
pub fn y_problem(text: &str, values: Option<&[f64]>, y: &[f64]) -> Option<String> {
    let (a, b) = match values {
        Some([a, b]) => (*a, *b),
        _ => RANGE,
    };
    let [lo, hi] = y else {
        return None;
    };
    let any = curves(text)
        .iter()
        .filter_map(|t| parse(t).ok())
        .any(|e| sample(&e, a, b).0.iter().any(|p| p.1 >= *lo && p.1 <= *hi));
    (!any).then(|| {
        format!("plot: no part of {text} is between y {lo} and {hi} for x from {a} to {b}")
    })
}

/// The functions of a plot card: `text` holds up to 3, separated by ";" (`x^2; 2x + 1`), each drawn in its own colour.
pub const MAX_CURVES: usize = 3;
pub fn curves(text: &str) -> Vec<&str> {
    text.split(';')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect()
}

/// A problem with a plot card, for the brief check.
pub fn problem(text: &str, values: Option<&[f64]>) -> Option<String> {
    let all = curves(text);
    if all.is_empty() {
        return Some("plot: give the function of x in text, for example x^2 - 2".into());
    }
    if all.len() > MAX_CURVES {
        return Some(format!(
            "plot: at most {MAX_CURVES} functions, separated by ';'"
        ));
    }
    all.iter().find_map(|t| one_problem(t, values))
}

fn one_problem(text: &str, values: Option<&[f64]>) -> Option<String> {
    let e = match parse(text) {
        Ok(e) => e,
        Err(m) => return Some(format!("plot: {m}")),
    };
    let (a, b) = match values {
        None => RANGE,
        Some([a, b]) if a.is_finite() && b.is_finite() && a < b => (*a, *b),
        Some(_) => {
            return Some("a plot card takes values [from, to], the x range, from below to".into())
        }
    };
    let (pts, ..) = sample(&e, a, b);
    let ok = pts.iter().filter(|p| p.1.is_finite()).count();
    (ok < SAMPLES / 4).then(|| format!("plot: {text} has no value for most x from {a} to {b}"))
}

/// The function as people read it: `x^2` becomes `x²`, `*` becomes `·`, `sqrt` becomes `√`, `pi` becomes `π`.
pub fn pretty(text: &str) -> String {
    const SUP: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    let mut out = String::new();
    let c: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < c.len() {
        // A lone x as the power: e^x, 2^x.
        if c[i] == '^'
            && c.get(i + 1) == Some(&'x')
            && c.get(i + 2)
                .is_none_or(|n| !n.is_alphanumeric() && *n != '(')
        {
            out.push('ˣ');
            i += 2;
            continue;
        }
        if c[i] == '^' {
            let neg = c.get(i + 1) == Some(&'-');
            let j0 = i + 1 + neg as usize;
            let j = (j0..c.len())
                .find(|&k| !c[k].is_ascii_digit())
                .unwrap_or(c.len());
            if j > j0 && c.get(j).is_none_or(|n| !n.is_alphanumeric() && *n != '.') {
                if neg {
                    out.push('⁻');
                }
                out.extend(
                    c[j0..j]
                        .iter()
                        .map(|d| SUP[d.to_digit(10).unwrap() as usize]),
                );
                i = j;
                continue;
            }
        }
        out.push(match c[i] {
            '*' => '·',
            '-' => '−',
            o => o,
        });
        i += 1;
    }
    out.replace("sqrt", "√").replace("pi", "π")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str, x: f64) -> f64 {
        parse(s).unwrap().eval(x)
    }

    #[test]
    fn reads_the_usual_functions() {
        assert_eq!(at("x^2 - 2", 3.0), 7.0);
        assert_eq!(at("-x^2", 3.0), -9.0);
        assert_eq!(at("2x + 1", 3.0), 7.0);
        assert_eq!(at("x(x-1)", 3.0), 6.0);
        assert_eq!(at("2^x^2", 2.0), 16.0);
        assert_eq!(at("y = 3sin(x)", 0.0), 0.0);
        assert!((at("e^(-x^2)", 0.0) - 1.0).abs() < 1e-12);
        assert!((at("2pi x", 1.0) - std::f64::consts::TAU).abs() < 1e-12);
        assert!(at("sqrt(x)", -1.0).is_nan());
        assert!(parse("foo(x)").is_err());
        assert!(parse("(x").is_err());
        assert!(parse("x +").is_err());
    }

    #[test]
    fn range_cuts_asymptotes_and_checks() {
        let (_, lo, hi) = sample(&parse("tan(x)").unwrap(), -3.0, 3.0);
        assert!(hi < 100.0 && lo > -100.0, "{lo} {hi}");
        let (_, lo, hi) = sample(&parse("x^2 - 2").unwrap(), -3.0, 3.0);
        assert!((hi - 7.0).abs() < 1e-9 && lo > -2.01, "{lo} {hi}");
        let (_, lo, hi) = sample(&parse("x^2").unwrap(), -2.0, 2.0);
        assert!(lo.abs() < 1e-3 && (hi - 4.0).abs() < 1e-9);
        assert!(problem("x^2", None).is_none());
        assert!(problem("sqrt(x)", Some(&[-5.0, -1.0])).is_some());
        assert!(problem("x", Some(&[2.0, 1.0])).is_some());
        assert!(problem("x^", None).is_some());
    }

    #[test]
    fn grid_lines_stand_at_round_steps() {
        assert_eq!(
            grid_lines(-4.0, 4.0),
            [-4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(grid_lines(-5.28, 13.28), [-5.0, 0.0, 5.0, 10.0]);
        assert_eq!(
            grid_lines(0.0, 1000.0),
            [0.0, 200.0, 400.0, 600.0, 800.0, 1000.0]
        );
        let small = grid_lines(0.0, 0.3);
        assert!(
            small.len() == 7 && (small[1] - 0.05).abs() < 1e-12 && (small[6] - 0.3).abs() < 1e-12,
            "{small:?}"
        );
        // 3 to 9 lines on any range, and none for a range that is not one.
        for (lo, hi) in [
            (-1.0, 1.0),
            (0.0, 7.3),
            (-123.0, 4567.0),
            (2.0, 2.001),
            (-6.3, 6.3),
        ] {
            let n = grid_lines(lo, hi).len();
            assert!((3..=9).contains(&n), "{lo} {hi}: {n}");
        }
        assert!(grid_lines(1.0, 1.0).is_empty() && grid_lines(0.0, f64::NAN).is_empty());
    }

    #[test]
    fn finds_the_zero_crossings() {
        let r = |s: &str, a: f64, b: f64| {
            let e = parse(s).unwrap();
            let (pts, lo, hi) = sample(&e, a, b);
            roots(&e, &pts, lo, hi)
        };
        let v = r("x^2 - 4", -4.0, 4.0);
        assert!(
            v.len() == 2 && (v[0] + 2.0).abs() < 1e-6 && (v[1] - 2.0).abs() < 1e-6,
            "{v:?}"
        );
        let v = r("tan(x)", -3.0, 3.0);
        assert!(v.len() == 1 && v[0].abs() < 1e-6, "{v:?}");
        assert!(r("x^2 + 1", -3.0, 3.0).is_empty());
        assert!(
            r("sin(x)", -20.0, 20.0).is_empty(),
            "too many roots: none marked"
        );
    }

    #[test]
    fn several_functions_are_checked_each() {
        assert_eq!(curves("x^2; 2x + 1 ;"), vec!["x^2", "2x + 1"]);
        assert!(problem("x^2; 2x + 1", None).is_none());
        assert!(problem("x^2; foo(x)", None).unwrap().contains("foo"));
        assert!(problem("x; x; x; x", None).unwrap().contains("at most 3"));
    }

    #[test]
    fn pretty_text() {
        assert_eq!(pretty("x^2 - 2*x"), "x² − 2·x");
        assert_eq!(pretty("e^-1 + sqrt(x)"), "e⁻¹ + √(x)");
        assert_eq!(pretty("2^x"), "2ˣ");
        assert_eq!(pretty("2^(x+1)"), "2^(x+1)");
        assert_eq!(pretty("x^2.5"), "x^2.5");
    }
}
