//! Pieces every FFmpeg template shares: embedded fonts, the fontconfig file and
//! the animated gradient background (PLAN-R1 §5).

use crate::filterpath::fontconfig;
use std::path::{Path, PathBuf};

/// Fonts embedded in the binary: (file name, family, bytes).
pub const FONTS: [(&str, &str, &[u8]); 2] = [
    (
        "NotoSans-Bold.ttf",
        "Noto Sans",
        include_bytes!("../../../assets/fonts/NotoSans-Bold.ttf"),
    ),
    (
        "NotoSansArabic-Bold.ttf",
        "Noto Sans Arabic",
        include_bytes!("../../../assets/fonts/NotoSansArabic-Bold.ttf"),
    ),
];

/// The math font of the formula card (Noto Sans Math, OFL); written next to the others so libass finds it.
pub const MATH_FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansMath-Regular.ttf");

/// The code card's font (Noto Sans Mono, OFL).
pub const MONO_FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansMono-Regular.ttf");

/// Symbols Noto Sans lacks (arrows, "≤", "≈", "√"), cut from Noto Sans Math with Noto Sans's metrics
/// (`scripts/make_symbols_font.py`). Without it libass fell back to Noto Sans Math and drew them at a quarter size.
pub const SYMBOLS_FONT: &[u8] = include_bytes!("../../../assets/fonts/RawivoxSymbols-Regular.ttf");

/// A brief's own font for this run: (file name, family, bytes). One run is one process.
static BRAND_FONT: std::sync::OnceLock<(String, String, Vec<u8>)> = std::sync::OnceLock::new();

/// Read a brand font file: (family, bytes). The family is the name libass looks for.
pub fn read_font(path: &Path) -> Result<(String, Vec<u8>), String> {
    use read_fonts::{FontRef, TableProvider};
    let bytes = std::fs::read(path).map_err(|e| format!("font {}: {e}", path.display()))?;
    let bad = || {
        format!(
            "font {} is not a TTF or OTF file with a family name",
            path.display()
        )
    };
    let family = {
        let f = FontRef::new(&bytes).map_err(|_| bad())?;
        let names = f.name().map_err(|_| bad())?;
        let data = names.string_data();
        let found = names
            .name_record()
            .iter()
            .filter(|r| r.name_id().to_u16() == 1)
            .find_map(|r| r.string(data).ok().map(|s| s.to_string()));
        found.filter(|n| !n.is_empty()).ok_or_else(bad)?
    };
    // The caption layout needs more tables than the family name: a font it cannot measure stops here, not after TTS.
    if crate::captions::Measurer::new(bytes.clone(), 84.0).is_none() {
        return Err(bad());
    }
    Ok((family, bytes))
}

/// The letters of `text` a font cannot draw. Other marks fall back to the built-in fonts.
pub fn missing_letters(font: &[u8], text: &str) -> Vec<char> {
    use read_fonts::{FontRef, TableProvider};
    let Some(map) = FontRef::new(font).ok().and_then(|f| f.cmap().ok()) else {
        return vec![];
    };
    let mut out: Vec<char> = text
        .chars()
        .filter(|c| c.is_alphabetic() && map.map_codepoint(*c).is_none_or(|g| g.to_u32() == 0))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Use this font for captions, cards and hooks in this run.
pub fn set_brand_font(path: &Path) -> Result<(), String> {
    let (family, bytes) = read_font(path)?;
    let name = format!(
        "brand-{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("font.ttf")
    );
    let _ = BRAND_FONT.set((name, family, bytes));
    Ok(())
}

/// The caption font for a language: (family, bytes).
pub fn font_for(lang: &str) -> (&'static str, &'static [u8]) {
    if let Some((_, family, bytes)) = BRAND_FONT.get() {
        return (family, bytes);
    }
    let (_, family, bytes) = if lang == "ar" { FONTS[1] } else { FONTS[0] };
    (family, bytes)
}

/// The bytes of a family libass can find in this run: the embedded fonts and the brief's own.
pub fn font_bytes(family: &str) -> Option<&'static [u8]> {
    if let Some((_, _, bytes)) = BRAND_FONT.get().filter(|(_, f, _)| f == family) {
        return Some(bytes);
    }
    match family {
        crate::formula::FAMILY => Some(MATH_FONT),
        crate::code_card::FAMILY => Some(MONO_FONT),
        _ => FONTS.iter().find(|f| f.1 == family).map(|f| f.2),
    }
}

/// Writes `fonts/` and `fonts.conf` into `work`. Returns the fontconfig path.
pub fn write_fonts(work: &Path) -> std::io::Result<PathBuf> {
    let fonts = work.join("fonts");
    std::fs::create_dir_all(&fonts)?;
    for (name, _, bytes) in FONTS {
        std::fs::write(fonts.join(name), bytes)?;
    }
    if let Some((name, _, bytes)) = BRAND_FONT.get() {
        std::fs::write(fonts.join(name), bytes)?;
    }
    std::fs::write(fonts.join("NotoSansMath-Regular.ttf"), MATH_FONT)?;
    std::fs::write(fonts.join("NotoSansMono-Regular.ttf"), MONO_FONT)?;
    std::fs::write(fonts.join("RawivoxSymbols-Regular.ttf"), SYMBOLS_FONT)?;
    let fc = work.join("fonts.conf");
    std::fs::write(&fc, fontconfig(&fonts, &work.join("fc-cache")))?;
    Ok(fc)
}

/// The default gradient's colours.
const DUSK: [&str; 3] = ["1d2b64", "6a3093", "f8b500"];

/// This run's gradient colours from a brief `background`. One run is one process.
static BACKGROUND: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

pub const BACKGROUNDS: &str = "dusk, night, ocean, forest, sand, slate";

/// A brief `background` as gradient colours: a name, or 2 or 3 `#RRGGBB` joined by commas.
pub fn background_colours(spec: &str) -> Result<Vec<String>, String> {
    let named: Option<&[&str]> = match spec {
        "dusk" => Some(&DUSK),
        "night" => Some(&["0b1020", "1b2a4a", "2d1b4e"]),
        "ocean" => Some(&["03263f", "0b5c7a", "0f7f8a"]),
        "forest" => Some(&["0e2a1c", "1d4a33", "3a5a2a"]),
        "sand" => Some(&["5a4632", "8a6a48", "a98a60"]),
        "slate" => Some(&["15181d", "2a3038", "3e4650"]),
        _ => None,
    };
    if let Some(n) = named {
        return Ok(n.iter().map(|c| c.to_string()).collect());
    }
    let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
    let hex = |c: &str| {
        c.strip_prefix('#')
            .filter(|h| h.len() == 6 && h.chars().all(|x| x.is_ascii_hexdigit()))
            .map(str::to_lowercase)
    };
    match parts.iter().map(|c| hex(c)).collect::<Option<Vec<_>>>() {
        Some(v) if (2..=3).contains(&v.len()) => Ok(v),
        _ => Err(format!(
            "background {spec}: use a name ({BACKGROUNDS}) or 2 or 3 colours like #0b1020,#1b2a4a"
        )),
    }
}

/// Use these colours for the gradient in this run.
pub fn set_background(spec: &str) -> Result<(), String> {
    let _ = BACKGROUND.set(background_colours(spec)?);
    Ok(())
}

/// Animated gradient as a lavfi source (fixed seed, so renders repeat exactly).
pub fn gradient(w: u32, h: u32, fps: u32, secs: f64) -> String {
    let dusk = DUSK.map(String::from).to_vec();
    let c = BACKGROUND.get().unwrap_or(&dusk);
    let colours: String = c
        .iter()
        .enumerate()
        .map(|(i, x)| format!(":c{i}=0x{x}"))
        .collect();
    // Brief `size` over 1080: the source gives YUV, not RGBA. FFmpeg holds some frames of every scene's source at
    // once, and at 3840x2160 an RGBA frame is 33 MB against 25 (measured, docs/specs/output-size.md).
    let yuv = if crate::size::large((w, h)) {
        ",format=yuv444p"
    } else {
        ""
    };
    format!("gradients=s={w}x{h}:r={fps}:d={secs}{colours}:nb_colors={}:x0=0:y0=0:x1={w}:y1={h}:seed=7:speed=0.012{yuv}", c.len())
}

#[cfg(test)]
mod brand_font_tests {
    use super::*;

    #[test]
    fn backgrounds_take_names_and_colours() {
        assert_eq!(background_colours("night").unwrap().len(), 3);
        assert_eq!(
            background_colours("#0B1020, #1b2a4a").unwrap(),
            vec!["0b1020", "1b2a4a"]
        );
        assert!(background_colours("#fff").is_err());
        assert!(background_colours("pink").unwrap_err().contains("slate"));
    }

    #[test]
    fn a_font_file_gives_its_family_and_names_the_letters_it_lacks() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("f.ttf");
        std::fs::write(&p, FONTS[0].2).unwrap();
        let (family, bytes) = read_font(&p).unwrap();
        assert_eq!(family, FONTS[0].1);
        assert_eq!(missing_letters(&bytes, "Hello 12 !"), Vec::<char>::new());
        assert_eq!(missing_letters(&bytes, "aب"), vec!['ب']);
        std::fs::write(&p, b"not a font").unwrap();
        assert!(read_font(&p).unwrap_err().contains("not a TTF"));
    }
}
