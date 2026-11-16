//! `scriptvox demo`: the zero-key smoke test (PLAN-R1 §2.3). Kinetic text on an
//! animated gradient with a soft sine chord. No keys, no Python, no Node:
//! only this binary and FFmpeg. The fonts are embedded, so the binary needs no
//! files next to it.

use crate::engine_ffmpeg::{Ffmpeg, Readback};
use crate::filterpath::ass_filter;
use crate::template::{gradient, write_fonts};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct Spec {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub secs: u32,
}

impl Spec {
    pub const FULL: Spec = Spec {
        width: 1080,
        height: 1920,
        fps: 30,
        secs: 8,
    };
    /// Golden size (PLAN-R1 §9 L6): small, short and fast.
    pub const GOLDEN: Spec = Spec {
        width: 180,
        height: 320,
        fps: 12,
        secs: 4,
    };

    pub fn frames(&self) -> u64 {
        u64::from(self.fps * self.secs)
    }
}

/// The gain on the chord. The demo is one FFmpeg pass with no master (two runs must give the same bytes), and its
/// sound is always the same, so one measured number puts it at the -14 LUFS of every other video: 0.9 gave -27.9
/// LUFS, and `check` warned about the first video a person makes (13 Nov 2026). Peak about -5 dBFS.
const GAIN: f64 = 4.46;

/// (text, style) per beat. Beats share the duration equally.
const BEATS: [(&str, &str); 5] = [
    ("ScriptVox", "Hero"),
    ("script in", "Latin"),
    ("video out", "Latin"),
    ("فيديو بالعربية", "Arabic"),
    ("no keys · one binary", "Latin"),
];

fn cs(t: f64) -> String {
    let n = (t * 100.0).round() as u64;
    format!(
        "{}:{:02}:{:02}.{:02}",
        n / 360_000,
        n / 6000 % 60,
        n / 100 % 60,
        n % 100
    )
}

pub fn ass(spec: &Spec) -> String {
    let (w, h) = (spec.width, spec.height);
    let size = |k: f64| (f64::from(h) * k).round() as u32;
    let mut out = format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {w}\nPlayResY: {h}\nWrapStyle: 2\nScaledBorderAndShadow: yes\n\n\
[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
Style: Hero,Noto Sans,{},&H00FFFFFF,&H00FFFFFF,&H00301010,&H00000000,1,0,0,0,100,100,0,0,1,{},0,5,0,0,0,1\n\
Style: Latin,Noto Sans,{},&H00FFFFFF,&H00FFFFFF,&H00301010,&H00000000,1,0,0,0,100,100,0,0,1,{},0,5,0,0,0,1\n\
Style: Arabic,Noto Sans Arabic,{},&H00FFFFFF,&H00FFFFFF,&H00301010,&H00000000,1,0,0,0,100,100,0,0,1,{},0,5,0,0,0,1\n\n\
[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
        size(0.11), size(0.007), size(0.08), size(0.006), size(0.085), size(0.006)
    );
    let beat = f64::from(spec.secs) / BEATS.len() as f64;
    for (i, (text, style)) in BEATS.iter().enumerate() {
        let (a, b) = (i as f64 * beat, (i + 1) as f64 * beat);
        // Pop in from 70% with a fade: the kinetic look, in plain ASS tags.
        out.push_str(&format!(
            "Dialogue: 0,{},{},{style},,0,0,0,,{{\\fad(120,160)\\fscx70\\fscy70\\t(0,200,\\fscx100\\fscy100)}}{text}\n",
            cs(a),
            cs(b)
        ));
    }
    out
}

/// The FFmpeg argv. All inputs are lavfi sources and the run folder is the cwd,
/// so the graph holds only fixed relative names.
pub fn argv(spec: &Spec, out: &Path) -> Vec<String> {
    let Spec {
        width: w,
        height: h,
        fps,
        secs,
    } = *spec;
    let fade_out = f64::from(secs) - 0.7;
    let mut a: Vec<String> = vec![
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        gradient(w, h, fps, f64::from(secs)),
    ];
    for f in ["220", "277.18", "329.63"] {
        a.extend([
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            format!("sine=f={f}:d={secs}"),
        ]);
    }
    let graph = format!(
        "[0:v]{},format=yuv420p[v];[1:a][2:a][3:a]amix=inputs=3,volume={GAIN},afade=t=in:d=0.5,afade=t=out:st={fade_out}:d=0.7[a]",
        ass_filter("demo.ass", "fonts")
    );
    a.extend(
        [
            "-filter_complex",
            &graph,
            "-map",
            "[v]",
            "-map",
            "[a]",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "20",
            "-r",
            &fps.to_string(),
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-t",
            &secs.to_string(),
            "-map_metadata",
            "-1",
            "-fflags",
            "+bitexact",
            "-flags:v",
            "+bitexact",
            "-flags:a",
            "+bitexact",
            "-movflags",
            "+faststart",
        ]
        .map(String::from),
    );
    a.push(out.to_string_lossy().into_owned());
    a
}

/// Renders the demo to `out` and reads it back. The work folder is removed after.
pub fn render(ff: &Ffmpeg, spec: &Spec, out: &Path) -> Result<Readback, String> {
    let work = std::env::temp_dir().join(format!("scriptvox-demo-{}", std::process::id()));
    let result = (|| {
        let fc = write_fonts(&work).map_err(|e| format!("work folder: {e}"))?;
        std::fs::write(work.join("demo.ass"), ass(spec)).map_err(|e| e.to_string())?;
        ff.run(&work, &argv(spec, out), Some(&fc))?;
        ff.readback(out)
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_uses_relative_names_and_fontsdir() {
        let a = argv(&Spec::GOLDEN, Path::new("/x/out.mp4"));
        let g = &a[a.iter().position(|s| s == "-filter_complex").unwrap() + 1];
        assert!(g.contains("ass=filename=demo.ass:fontsdir=fonts"));
        assert_eq!(a.last().unwrap(), "/x/out.mp4");
    }

    #[test]
    fn beats_cover_the_whole_video() {
        let s = ass(&Spec::FULL);
        assert!(s.contains("Dialogue: 0,0:00:00.00,0:00:01.60,Hero"));
        assert!(s.contains("Style: Arabic,Noto Sans Arabic,163,"));
        assert!(s.contains(",0:00:08.00,Latin"));
    }
}
