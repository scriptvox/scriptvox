//! Paths and fonts inside an FFmpeg filtergraph (PLAN-R1 §5, spike answers 1 and 2).
//!
//! Rule: the run folder is FFmpeg's cwd, so generated files use fixed relative
//! names and need no escaping. `filter_path` is only for paths a user supplies.

use std::path::Path;

/// Level 1: an option value inside one filter (`\`, `'` and `:` are special).
pub fn escape_value(s: &str) -> String {
    escape(s, &['\\', '\'', ':'])
}

/// Level 2: the filtergraph text around the filter (`\ ' [ ] , ;`).
pub fn escape_graph(s: &str) -> String {
    escape(s, &['\\', '\'', '[', ']', ',', ';'])
}

fn escape(s: &str, special: &[char]) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if special.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A path ready to be an option value in a `-vf`/`-filter_complex` graph.
pub fn filter_path(p: &Path) -> String {
    filter_text(&p.to_string_lossy(), cfg!(windows))
}

/// On Windows the `\` separators become `/`, which FFmpeg accepts. On Unix a
/// `\` is a legal file-name character and is escaped, never changed.
pub fn filter_text(s: &str, windows: bool) -> String {
    let s = if windows {
        s.replace('\\', "/")
    } else {
        s.to_string()
    };
    escape_graph(&escape_value(&s))
}

/// The only builder for a subtitle filter: `fontsdir` is always set, so libass
/// never falls back to system fonts.
pub fn ass_filter(ass: &str, fontsdir: &str) -> String {
    format!("ass=filename={ass}:fontsdir={fontsdir}")
}

/// A minimal fontconfig file that sees only our fonts (`FONTCONFIG_FILE`).
pub fn fontconfig(fonts_dir: &Path, cache_dir: &Path) -> String {
    let x = |p: &Path| {
        p.to_string_lossy()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    format!(
        // Latin inside Arabic text falls back to Noto Sans. Without the alias fontconfig may pick Noto Sans Math or
        // Mono (formula and code cards), whose tall line metrics draw the Latin letters at about a third of the size
        // (seen 4 Nov 2026 on an Arabic label). A symbol Noto Sans lacks ("≤", "→") falls back to Rawivox Symbols,
        // which has Noto Sans's metrics, for the same reason.
        "<?xml version=\"1.0\"?>\n<!DOCTYPE fontconfig SYSTEM \"fonts.dtd\">\n<fontconfig>\n  <dir>{}</dir>\n  <cachedir>{}</cachedir>\n  <alias binding=\"strong\"><family>Noto Sans Arabic</family><accept><family>Noto Sans</family></accept></alias>\n  <alias binding=\"strong\"><family>Noto Sans</family><accept><family>Rawivox Symbols</family></accept></alias>\n</fontconfig>\n",
        x(fonts_dir),
        x(cache_dir)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_path_escapes_both_levels() {
        let p = r"C:\Users\me\it's [x]\cap.ass";
        assert_eq!(filter_text(p, true), r"C\\:/Users/me/it\\\'s \[x\]/cap.ass");
    }

    #[test]
    fn unix_backslash_is_kept_and_escaped() {
        assert_eq!(filter_text(r"/tmp/a\b", false), r"/tmp/a\\\\b");
    }

    #[test]
    fn ass_filter_always_sets_fontsdir() {
        assert_eq!(
            ass_filter("captions.ass", "fonts"),
            "ass=filename=captions.ass:fontsdir=fonts"
        );
    }

    #[test]
    fn fontconfig_escapes_xml() {
        let c = fontconfig(Path::new("/a&b/fonts"), Path::new("/c"));
        assert!(c.contains("<dir>/a&amp;b/fonts</dir>"));
        assert!(c.contains("<family>Noto Sans Arabic</family><accept><family>Noto Sans</family>"));
        assert!(c.contains("<family>Noto Sans</family><accept><family>Rawivox Symbols</family>"));
    }
}
