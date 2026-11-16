//! The `code` card: a short snippet in a dark editor panel, coloured by a small tokenizer of ScriptVox's own
//! (keywords, strings, numbers, comments, calls), typed in line by line. Noto Sans Mono (OFL) keeps columns aligned.

pub const FAMILY: &str = "Noto Sans Mono";
/// The advance of one character as a share of the ASS font size: 600 of 1000 units per em, and the font size maps
/// to win ascent plus win descent (1618 units).
pub const ADVANCE: f32 = 600.0 / 1618.0;
pub const MAX_LINES: usize = 12;
pub const MAX_COLS: usize = 44;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tok {
    Plain,
    Keyword,
    Str,
    Num,
    Comment,
    Call,
}

const KEYWORDS: &[&str] = &[
    // Python
    "def",
    "class",
    "return",
    "if",
    "elif",
    "else",
    "for",
    "while",
    "in",
    "import",
    "from",
    "as",
    "with",
    "try",
    "except",
    "finally",
    "raise",
    "lambda",
    "yield",
    "pass",
    "break",
    "continue",
    "and",
    "or",
    "not",
    "is",
    "None",
    "True",
    "False",
    "async",
    "await",
    "global",
    // JavaScript and TypeScript
    "function",
    "const",
    "let",
    "var",
    "do",
    "switch",
    "case",
    "new",
    "extends",
    "export",
    "default",
    "catch",
    "throw",
    "typeof",
    "instanceof",
    "null",
    "undefined",
    "true",
    "false",
    "this",
    "of",
    "interface",
    "type",
    // Rust
    "fn",
    "mut",
    "pub",
    "struct",
    "enum",
    "impl",
    "trait",
    "use",
    "mod",
    "match",
    "loop",
    "self",
    "Self",
    "crate",
    "where",
    "ref",
    "move",
    "Some",
    "Ok",
    "Err",
    // Shell
    "echo",
    "then",
    "fi",
    "done",
    "esac",
    "local",
];

/// SQL words count in upper case only (SELECT), so `d.update(x)` in Python stays a call.
const SQL: &[&str] = &[
    "select", "from", "where", "join", "left", "right", "inner", "outer", "on", "group", "by",
    "order", "having", "insert", "into", "values", "update", "set", "delete", "create", "table",
    "limit", "distinct", "union",
];

/// One line split into coloured runs. Strings and comments end at the line end (snippets are short).
pub fn tokens(line: &str) -> Vec<(Tok, String)> {
    let c: Vec<char> = line.chars().collect();
    let mut out: Vec<(Tok, String)> = Vec::new();
    let push = |out: &mut Vec<(Tok, String)>, t: Tok, s: String| match out.last_mut() {
        Some((lt, ls)) if *lt == t => ls.push_str(&s),
        _ => out.push((t, s)),
    };
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let prev_space = i == 0 || c[i - 1].is_whitespace();
        let next = c.get(i + 1).copied();
        // Comments: //, # (not #[ or #! or #{), and -- followed by a space (SQL).
        let comment = (ch == '/' && next == Some('/'))
            || (ch == '#' && prev_space && !matches!(next, Some('[' | '!' | '{')))
            || (ch == '-' && next == Some('-') && c.get(i + 2).is_some_and(|x| x.is_whitespace()));
        if comment {
            push(&mut out, Tok::Comment, c[i..].iter().collect());
            break;
        }
        if matches!(ch, '"' | '\'' | '`') {
            let mut j = i + 1;
            while j < c.len() && c[j] != ch {
                j += if c[j] == '\\' { 2 } else { 1 };
            }
            let j = (j + 1).min(c.len());
            push(&mut out, Tok::Str, c[i..j].iter().collect());
            i = j;
            continue;
        }
        if ch.is_ascii_digit() && (i == 0 || !(c[i - 1].is_alphanumeric() || c[i - 1] == '_')) {
            let mut j = i;
            while j < c.len() && (c[j].is_ascii_alphanumeric() || c[j] == '.' || c[j] == '_') {
                j += 1;
            }
            push(&mut out, Tok::Num, c[i..j].iter().collect());
            i = j;
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            let mut j = i;
            while j < c.len() && (c[j].is_alphanumeric() || c[j] == '_') {
                j += 1;
            }
            let w: String = c[i..j].iter().collect();
            let sql = w.len() > 1
                && w.chars().all(|c| c.is_ascii_uppercase())
                && SQL.contains(&w.to_lowercase().as_str());
            let t = if KEYWORDS.contains(&w.as_str()) || sql {
                Tok::Keyword
            } else if c.get(j) == Some(&'(') {
                Tok::Call
            } else {
                Tok::Plain
            };
            push(&mut out, t, w);
            i = j;
            continue;
        }
        push(&mut out, Tok::Plain, ch.to_string());
        i += 1;
    }
    out
}

/// The colour of a run, ASS order (blue, green, red).
pub fn colour(t: Tok) -> &'static str {
    match t {
        Tok::Plain => "&HE6E6E6&",
        Tok::Keyword => "&HFF9BD9&",
        Tok::Str => "&H6BB2F2&",
        Tok::Num => "&HA6DB9C&",
        Tok::Comment => "&H8D8C7F&",
        Tok::Call => "&H8AE3FF&",
    }
}

/// Code text that libass prints as written: braces escaped, and a backslash before n, N or h kept apart from it
/// by a word joiner so it is not read as a line break or a hard space.
pub fn ass_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\\' if matches!(it.peek(), Some('n' | 'N' | 'h')) => out.push_str("\\\u{2060}"),
            _ => out.push(ch),
        }
    }
    out
}

/// The snippet as display lines: tabs to 4 spaces, trailing space and empty edge lines dropped.
pub fn lines(text: &str) -> Vec<String> {
    let v: Vec<String> = text
        .replace('\t', "    ")
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect();
    let first = v.iter().position(|l| !l.is_empty()).unwrap_or(v.len());
    let last = v
        .iter()
        .rposition(|l| !l.is_empty())
        .map_or(first, |p| p + 1);
    v[first..last].to_vec()
}

/// The brief check.
pub fn problem(text: &str) -> Option<String> {
    let l = lines(text);
    if l.is_empty() {
        return Some("a code card needs text: the snippet, lines split with \\n".into());
    }
    if l.len() > MAX_LINES {
        return Some(format!(
            "a code card takes at most {MAX_LINES} lines (this one has {})",
            l.len()
        ));
    }
    l.iter().find(|x| x.chars().count() > MAX_COLS).map(|x| {
        format!(
            "a code card line takes at most {MAX_COLS} characters: \"{x}\" has {}",
            x.chars().count()
        )
    })
}

/// One line of a terminal card: the prompt green and the command white, output grey. No code colours: output is not
/// code.
pub fn terminal_line(line: &str) -> String {
    let plain = |s: &str| ass_text(s).replace(' ', "\\h");
    match line.strip_prefix("$ ") {
        Some(cmd) => format!("{{\\1c&H7BD88F&\\i0}}$\\h{{\\1c&HF0F0F0&}}{}", plain(cmd)),
        None => format!("{{\\1c&HB8B8B8&\\i0}}{}", plain(line)),
    }
}

/// One line as ASS text with its colours. Spaces become hard spaces (`\h`): libass drops leading spaces, and
/// indentation is meaning in code.
pub fn ass_line(line: &str) -> String {
    tokens(line)
        .into_iter()
        .map(|(t, s)| {
            format!(
                "{{\\1c{}{}}}{}",
                colour(t),
                if t == Tok::Comment { "\\i1" } else { "\\i0" },
                ass_text(&s).replace(' ', "\\h")
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(l: &str) -> Vec<(Tok, &'static str)> {
        tokens(l)
            .into_iter()
            .map(|(t, s)| (t, Box::leak(s.into_boxed_str()) as &str))
            .collect()
    }

    #[test]
    fn python_rust_and_sql_colour_the_right_words() {
        assert_eq!(
            kinds("def area(r): return 3.14 * r ** 2  # pi r squared"),
            [
                (Tok::Keyword, "def"),
                (Tok::Plain, " "),
                (Tok::Call, "area"),
                (Tok::Plain, "(r): "),
                (Tok::Keyword, "return"),
                (Tok::Plain, " "),
                (Tok::Num, "3.14"),
                (Tok::Plain, " * r ** "),
                (Tok::Num, "2"),
                (Tok::Plain, "  "),
                (Tok::Comment, "# pi r squared"),
            ]
        );
        let r = kinds("#[derive(Debug)] let s = \"a\\\"b\"; // done");
        assert_eq!(r[0], (Tok::Plain, "#["));
        assert!(
            r.contains(&(Tok::Str, "\"a\\\"b\"")) && r.last() == Some(&(Tok::Comment, "// done"))
        );
        let q = kinds("SELECT name FROM users -- all");
        assert_eq!(q[0], (Tok::Keyword, "SELECT"));
        assert_eq!(q.last(), Some(&(Tok::Comment, "-- all")));
        assert!(kinds("x--").iter().all(|(t, _)| *t == Tok::Plain));
        assert!(kinds("v2 = x1").iter().all(|(t, _)| *t == Tok::Plain));
    }

    #[test]
    fn braces_and_backslashes_print_as_written() {
        assert_eq!(
            ass_text("fn f() { print(\"a\\n\") }"),
            "fn f() \\{ print(\"a\\\u{2060}n\") \\}"
        );
        assert_eq!(ass_text("C:\\path"), "C:\\path");
    }

    #[test]
    fn indentation_survives_as_hard_spaces() {
        assert_eq!(ass_line("  x"), "{\\1c&HE6E6E6&\\i0}\\h\\hx");
    }

    #[test]
    fn limits_are_checked_and_edges_trimmed() {
        assert_eq!(lines("\n\tx = 1\n\n"), ["    x = 1"]);
        assert!(problem("").is_some());
        assert!(problem(&"x\n".repeat(13)).unwrap().contains("12 lines"));
        assert!(problem(&"y".repeat(45)).unwrap().contains("44 characters"));
        assert!(problem("print('hi')").is_none());
    }
}
