//! Brief `caption_outline` and `caption_box`: the outline and the box of the main captions. See
//! docs/specs/caption-style.md. The values go into the `Style: Cap,` line that `captions::to_ass` wrote; with
//! neither field the file is not touched.

/// The widest outline, in px on a 1080 px short side.
pub const MAX_OUTLINE: f64 = 12.0;

/// The three style fields of the main captions, as every command takes them: brief `caption_size`,
/// `caption_outline` and `caption_box` (`make`, `resume`), and the flags of `edit`, `dub` and `clips`.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct Style {
    /// Caption size: small, normal, large or 50% to 200%.
    #[arg(long = "caption-size", value_name = "SIZE")]
    pub size: Option<String>,
    /// Caption outline: 0 to 12 px.
    #[arg(long = "caption-outline", value_name = "PX")]
    pub outline: Option<f64>,
    /// Caption box: 0 (none) to 1 (black).
    #[arg(long = "caption-box", value_name = "0-1")]
    pub box_: Option<f64>,
}

impl Style {
    pub fn is_set(&self) -> bool {
        self.size.is_some() || self.outline.is_some() || self.box_.is_some()
    }

    /// A problem with the fields and its fix line: the one check of every command. `captions` is the caption style.
    pub fn check(&self, captions: Option<&str>) -> Option<(String, &'static str)> {
        if let Err(e) = crate::captions::size_scale(self.size.as_deref()) {
            return Some((
                e,
                "Use caption_size small, normal, large or a percent such as 115% (brief: \"caption_size\": \"115%\"; flag: --caption-size 115%).",
            ));
        }
        problem(self.outline, self.box_, captions).map(|e| {
            (
                e,
                "Example: \"caption_box\": 0, \"caption_outline\": 4 (flags: --caption-box 0 --caption-outline 4). See `scriptvox schema brief`.",
            )
        })
    }

    /// The size as a fraction, for `make::caption_pages`. A bad value is refused by `check` before this.
    pub fn scale(&self) -> (u32, u32) {
        crate::captions::size_scale(self.size.as_deref()).unwrap_or((1, 1))
    }

    /// The caption file with the outline and the box. `short` is the frame's short side in px.
    pub fn apply(&self, ass: &str, short: u32) -> String {
        apply(ass, self.outline, self.box_, short)
    }
}

/// A problem with the two fields, for the brief check. `captions` is the caption style (`karaoke`, `pop`, `reveal`).
pub fn problem(outline: Option<f64>, box_: Option<f64>, captions: Option<&str>) -> Option<String> {
    if outline.is_some_and(|o| !(0.0..=MAX_OUTLINE).contains(&o)) {
        return Some(format!(
            "caption_outline is 0 to {MAX_OUTLINE} (px on a 1080 px short side)"
        ));
    }
    if box_.is_some_and(|b| !(0.0..=1.0).contains(&b)) {
        return Some("caption_box is the box's opacity, 0 (no box) to 1 (solid black)".into());
    }
    if box_.is_some_and(|b| b > 0.0) && captions == Some("reveal") {
        return Some(
            "captions reveal has no box (a box would show the page's width before its words): remove caption_box, or use captions karaoke or pop"
                .into(),
        );
    }
    None
}

/// The caption style with the brief's outline and box. `short` is the frame's short side in px.
///
/// Measured with libass (13 Nov 2026): with BorderStyle 4 the outline is drawn around the letters in OutlineColour,
/// and the box is padded by Outline plus Shadow. So a box and a real outline can stand together, and Shadow keeps the
/// padding the box had before.
pub fn apply(ass: &str, outline: Option<f64>, box_: Option<f64>, short: u32) -> String {
    if outline.is_none() && box_.is_none() {
        return ass.to_string();
    }
    let mut out = String::with_capacity(ass.len() + 16);
    for l in ass.lines() {
        let mut f: Vec<String> = l.split(',').map(String::from).collect();
        if l.starts_with("Style: Cap,") && f.len() > 17 {
            let size: f64 = f[2].parse().unwrap_or(84.0);
            // `reveal` writes BorderStyle 1: it has no box.
            let had_box = f[15] == "4";
            let boxed = had_box && box_ != Some(0.0);
            let pad = (size / 6.0).floor();
            // No box and no outline asked: the outline of `reveal`, a tenth of the size, so the text is not bare.
            let line = outline
                .map(|o| (o * short as f64 / 1080.0).round())
                .or((!boxed).then_some((size / 10.0).floor()));
            if let Some(b) = box_.filter(|_| boxed) {
                let alpha = ((1.0 - b) * 255.0).round() as u8;
                // With no outline asked the soft edge inside the box keeps the box's own colour, as before.
                f[5] = format!("&H{alpha:02X}000000");
                f[6] = f[5].clone();
            }
            if let Some(o) = line {
                f[5] = "&H00000000".into();
                f[16] = format!("{o}");
                f[17] = if boxed {
                    format!("{}", (pad - o).max(0.0))
                } else {
                    "0".into()
                };
            }
            f[15] = if boxed { "4" } else { "1" }.into();
        }
        out.push_str(&f.join(","));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: &str = "Style: Cap,Noto Sans,84,&H0000E5FF,&H00FFFFFF,&H59000000,&H59000000,1,0,0,0,100,100,0,0,4,14,0,2,60,60,172,-1\nDialogue: 0,0:00:00.00,0:00:01.00,Cap,,0,0,0,,{\\k50}Hi, there\n";

    fn style(outline: Option<f64>, box_: Option<f64>) -> String {
        let s = apply(CAP, outline, box_, 1080);
        // Only the style line changes: a comma in the spoken text stays.
        assert!(s.ends_with("{\\k50}Hi, there\n"));
        s.lines().next().unwrap().to_string()
    }

    #[test]
    fn the_style_line_takes_the_outline_and_the_box() {
        // Neither field: the same text, byte for byte.
        assert_eq!(apply(CAP, None, None, 1080), CAP);
        // A lighter box: only the two alphas change (0.4 is 60 percent clear, 0x99).
        assert!(
            style(None, Some(0.4)).contains(",&H99000000,&H99000000,1,0,0,0,100,100,0,0,4,14,0,2,")
        );
        // An outline inside the box: opaque black, and Shadow keeps the padding at 14.
        assert!(
            style(Some(4.0), None).contains(",&H00000000,&H59000000,1,0,0,0,100,100,0,0,4,4,10,2,")
        );
        // No box: an outline style. With no outline asked, a tenth of the size.
        assert!(
            style(None, Some(0.0)).contains(",&H00000000,&H59000000,1,0,0,0,100,100,0,0,1,8,0,2,")
        );
        assert!(style(Some(5.0), Some(0.0)).contains(",1,5,0,2,"));
        assert!(style(Some(0.0), Some(0.0)).contains(",1,0,0,2,"));
        // The outline follows the frame: 6 px on 1080 is 4 px on 720.
        assert!(apply(CAP, Some(6.0), None, 720).contains(",4,4,10,2,"));
    }

    #[test]
    fn values_out_of_range_and_a_box_on_reveal_are_refused() {
        assert!(problem(None, None, Some("reveal")).is_none());
        assert!(problem(Some(3.0), Some(0.0), Some("reveal")).is_none());
        assert!(problem(Some(13.0), None, None)
            .unwrap()
            .contains("caption_outline"));
        assert!(problem(Some(-1.0), None, None).is_some());
        assert!(problem(None, Some(1.5), None)
            .unwrap()
            .contains("caption_box"));
        assert!(problem(None, Some(0.5), Some("reveal"))
            .unwrap()
            .contains("reveal"));
    }
}
