//! Brief `size`: the short side of the frame in px, for every frame shape (docs/specs/output-size.md, G194).
//!
//! The layouts already follow the frame (`--draft` runs them at half size), so `make` scales the frame once and the
//! captions, cards, logo and outro follow it. This file holds the allowed sizes, the scaled frame, the estimate that
//! `plan` shows, and the render settings that keep a large frame under the memory cap of the machine.

pub const DEFAULT: u32 = 1080;
pub const SIZES: [u32; 4] = [720, 1080, 1440, 2160];
pub const FIX: &str =
    "Use \"size\": 720, 1080, 1440 or 2160 (the short side in px), or drop the field for 1080.";

pub fn check(size: Option<u32>) -> Result<(), String> {
    match size {
        Some(s) if !SIZES.contains(&s) => Err(format!(
            "size is the short side of the frame: 720, 1080, 1440 or 2160, not {s}"
        )),
        _ => Ok(()),
    }
}

/// The frame of a shape at 1080, at the asked short side. Every shape at every size has even sides.
pub fn scale((w, h): (u32, u32), size: Option<u32>) -> (u32, u32) {
    let s = size.unwrap_or(DEFAULT);
    (w * s / DEFAULT, h * s / DEFAULT)
}

/// A frame from a `size` over 1080: its short side is over 1080 (21:9 at 1080 is 2520 wide, and is not large).
pub fn large((w, h): (u32, u32)) -> bool {
    w.min(h) > DEFAULT
}

/// What `plan` says about the render: seconds of work, megabytes of the file at most, and gigabytes of memory.
///
/// Measured on 13 Nov 2026 with libx264 `veryfast` on a 12-thread CPU (docs/specs/output-size.md): 0.3 s of work
/// for each megapixel and second of video at 30 fps; a file of at most 0.1 bit a pixel; and an address space of
/// 2.3 GB plus, for each megapixel, 0.14 GB and 0.074 GB a scene (FFmpeg holds some frames of every scene at once).
pub struct Estimate {
    pub render_s: f64,
    pub disk_mb: f64,
    pub memory_gb: f64,
    /// A render in groups of scenes (`split.rs`): the groups, and the megabytes of their files while it renders.
    pub groups: Option<(usize, f64)>,
}

/// The fix for a render that needs too much memory, in `plan` and after a failed render.
pub const MEMORY_FIX: &str = "Use a smaller `size`, or fewer scenes in one video (make two videos and `scriptvox join` them).";

/// Over this, a render under an 8 GB memory limit is likely to fail.
pub const MEMORY_WARN_GB: f64 = 7.0;

pub fn estimate(frame: (u32, u32), fps: u32, secs: f64, scenes: usize) -> Estimate {
    estimate_with(
        frame,
        fps,
        secs,
        scenes,
        crate::split::group_size(frame, scenes),
    )
}

/// `per`: the scenes of one group when the picture is made in groups. Then one process holds a group and the
/// first scene of the next one, not all scenes, and each group takes about 3 s more (measured, 3840x2160).
fn estimate_with(
    (w, h): (u32, u32),
    fps: u32,
    secs: f64,
    scenes: usize,
    per: Option<usize>,
) -> Estimate {
    let mp = f64::from(w) * f64::from(h) / 1e6;
    let frames = secs * f64::from(fps);
    let scenes = scenes.max(1);
    let at_once = per.map_or(scenes, |p| (p + 1).min(scenes));
    let groups = per.map(|p| (scenes.div_ceil(p), mp * frames * crate::split::DISK_MB));
    Estimate {
        render_s: 0.3 * mp * frames / 30.0 + groups.map_or(0.0, |g| 3.0 * g.0 as f64),
        disk_mb: mp * frames / 80.0,
        memory_gb: 2.3 + mp * (0.14 + 0.074 * at_once as f64),
        groups,
    }
}

/// The render line of `plan`. A frame over 1080 also says that a smaller clip is scaled.
pub fn explain(frame: (u32, u32), fps: u32, secs: f64, scenes: usize) -> String {
    // A brief whose length the plan does not know (a base video, a narration file): the numbers for 10 s.
    let secs = if secs < 1.0 { 10.0 } else { secs };
    explain_of(frame, fps, secs, &estimate(frame, fps, secs, scenes))
}

fn explain_of(frame: (u32, u32), fps: u32, secs: f64, e: &Estimate) -> String {
    let mut s = format!(
        "{}x{} at {fps} fps, chunked, resumable; about {:.0} s to render, a file of at most {:.0} MB (twice that on disk while it renders), about {:.1} GB of memory (estimates for a {secs:.0} s video on a 12-thread CPU)",
        frame.0, frame.1, e.render_s.ceil(), e.disk_mb.ceil(), e.memory_gb
    );
    if let Some((groups, mb)) = e.groups {
        s.push_str(&format!(
            "; the picture is made in {groups} groups of scenes, one process each, so memory does not grow with the scenes: about {:.0} MB more on disk while it renders (a noisy camera clip can need 7 times that)",
            mb.ceil()
        ));
    }
    if large(frame) {
        s.push_str("; a clip or picture smaller than the frame is scaled up to it, with no new detail (`upscale` is a separate step for generated clips)");
    }
    s
}

/// A warning for `plan` when the render is likely to need more memory than a common limit.
pub fn memory_warning(frame: (u32, u32), secs: f64, scenes: usize) -> Option<serde_json::Value> {
    let e = estimate(frame, 30, secs, scenes);
    (e.memory_gb > MEMORY_WARN_GB).then(|| {
        serde_json::json!({"gate": "size", "status": "warn",
            "detail": format!("{}x{} with {scenes} scenes needs about {:.1} GB of memory in one render, and a render over 8 GB can fail",
                frame.0, frame.1, e.memory_gb),
            "fix": MEMORY_FIX})
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_at_every_size_is_even_and_keeps_its_short_side() {
        for shape in ["9:16", "16:9", "1:1", "4:5", "3:4", "4:3", "2:3", "21:9"] {
            let base = crate::make::frame(shape).unwrap();
            assert_eq!(scale(base, None), base);
            assert_eq!(scale(base, Some(1080)), base);
            for s in SIZES {
                let (w, h) = scale(base, Some(s));
                assert_eq!((w % 2, h % 2, w.min(h)), (0, 0, s), "{shape} at {s}");
                // The shape stays: the same ratio to a pixel.
                assert_eq!(w * base.1, h * base.0, "{shape} at {s}");
            }
        }
        assert_eq!(scale((1920, 1080), Some(2160)), (3840, 2160));
        assert!(check(Some(1000)).unwrap_err().contains("not 1000"));
        assert!(check(None).is_ok());
    }

    #[test]
    fn the_estimate_follows_the_measured_runs_and_warns_only_on_a_large_frame() {
        // Measured: 3840x2160, 5 scenes, 17.5 s: 36 s of render, 6.5 GB. 1920x1080: 11 s, 3.5 GB.
        let e = estimate((3840, 2160), 30, 17.5, 5);
        assert!((30.0..50.0).contains(&e.render_s), "{}", e.render_s);
        assert!((6.2..6.8).contains(&e.memory_gb), "{}", e.memory_gb);
        let e = estimate((1920, 1080), 30, 17.5, 5);
        assert!((9.0..13.0).contains(&e.render_s) && (3.2..3.7).contains(&e.memory_gb));
        assert!(memory_warning((1920, 1080), 60.0, 20).is_none());
        assert!(memory_warning((3840, 2160), 17.5, 5).is_none());
        // 12 scenes at 3840x2160 go in groups of 3 (4 scenes at once, 5.9 GB): no warning, and the plan says so.
        assert!(memory_warning((3840, 2160), 60.0, 12).is_none());
        let line = explain((3840, 2160), 30, 20.0, 12);
        assert!(
            line.contains("made in 4 groups of scenes")
                && line.contains("about 1245 MB more on disk"),
            "{line}"
        );
        assert!(!explain((1920, 1080), 30, 60.0, 8).contains("groups"));
        // In one process the same video is over the limit (measured: 8 scenes fail under 8 GB).
        assert!(estimate_with((3840, 2160), 30, 60.0, 12, None).memory_gb > MEMORY_WARN_GB);
        assert!(estimate_with((3840, 2160), 30, 60.0, 12, Some(3)).memory_gb < 6.0);
        assert!(explain((3840, 2160), 30, 6.0, 1).contains("scaled up"));
        assert!(!explain((1920, 1080), 30, 6.0, 1).contains("scaled up"));
        assert!(explain((1920, 1080), 30, 0.0, 1).contains("for a 10 s video"));
    }
}
