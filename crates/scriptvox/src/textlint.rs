//! Text lint at plan time (PLAN-R1 §5, CAP gap 1; ideas from HF #5885,
//! MPT #1205, MGL #1173, OM #819): problems that would only show up as boxes
//! or broken Arabic after a render are caught before any provider call.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Finding {
    pub code: &'static str,
    pub detail: String,
    pub fix: String,
}

fn is_arabic_letter(c: char) -> bool {
    ('\u{0621}'..='\u{064A}').contains(&c) || ('\u{0671}'..='\u{06D3}').contains(&c)
}

fn is_presentation_form(c: char) -> bool {
    ('\u{FB50}'..='\u{FDFF}').contains(&c) || ('\u{FE70}'..='\u{FEFF}').contains(&c)
}

/// Characters no embedded font can draw (they would render as boxes).
fn uncovered(text: &str) -> Vec<char> {
    use read_fonts::{FontRef, TableProvider};
    let fonts: Vec<FontRef> = crate::template::FONTS
        .iter()
        .map(|(_, _, b)| *b)
        .chain([crate::template::SYMBOLS_FONT])
        .filter_map(|b| FontRef::new(b).ok())
        .collect();
    let mut missing: Vec<char> = text
        .chars()
        .filter(|c| {
            !c.is_whitespace()
                && !c.is_control()
                && !matches!(*c, '\u{200C}' | '\u{200D}' | '\u{200E}' | '\u{200F}')
        })
        .filter(|c| {
            !fonts.iter().any(|f| {
                f.cmap()
                    .ok()
                    .and_then(|m| m.map_codepoint(*c))
                    .is_some_and(|g| g.to_u32() != 0)
            })
        })
        .collect();
    missing.sort_unstable();
    missing.dedup();
    missing
}

pub fn check(text: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let missing = uncovered(text);
    if !missing.is_empty() {
        let shown: String = missing
            .iter()
            .take(12)
            .map(|c| format!("{c} (U+{:04X}) ", *c as u32))
            .collect();
        out.push(Finding {
            code: "SVX_FONT_COVERAGE",
            detail: format!("{} characters have no glyph in the caption fonts: {}", missing.len(), shown.trim()),
            fix: "Remove or replace them (emoji and scripts other than Latin and Arabic are not supported in v0.1 captions).".into(),
        });
    }
    // Letter-by-letter Arabic: 3 or more single Arabic letters in a row cannot join.
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let run = tokens.windows(3).any(|w| {
        w.iter()
            .all(|t| t.chars().count() == 1 && t.chars().all(is_arabic_letter))
    });
    if run {
        out.push(Finding {
            code: "SVX_RTL_SPLIT",
            detail: "Arabic letters are separated by spaces, so they cannot join into words".into(),
            fix: "Write the Arabic words without spaces between the letters.".into(),
        });
    }
    let pre = text.chars().filter(|c| is_presentation_form(*c)).count();
    if pre > 0 {
        out.push(Finding {
            code: "SVX_RTL_PRESHAPED",
            detail: format!("{pre} characters are Arabic presentation forms (pre-shaped text, often from a PDF or an old app)"),
            fix: "Paste the text as normal Arabic letters (Unicode Arabic block); ScriptVox shapes it.".into(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_mixed_text_passes() {
        assert!(check("مرحبا بكم في ScriptVox 2026، فيديو بالعربية.").is_empty());
        // Symbols drawn by the Rawivox Symbols fallback font.
        assert!(check("speed ≤ 40 → slow, gap ≈ 4 s, √9").is_empty());
    }

    #[test]
    fn findings() {
        assert_eq!(check("Hello 😀 你好")[0].code, "SVX_FONT_COVERAGE");
        assert!(check("م ر ح ب ا").iter().any(|f| f.code == "SVX_RTL_SPLIT"));
        assert!(check("ﻣﺮﺣﺒﺎ").iter().any(|f| f.code == "SVX_RTL_PRESHAPED"));
    }
}
