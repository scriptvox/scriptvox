//! `scriptvox bench`: ScriptVox's own numbers on this machine, with the hardware
//! recorded, so a claim like "renders faster than real time" can be checked.

use serde_json::{json, Value};
use std::time::Instant;

fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|m| m.trim().to_string())
        })
        .unwrap_or_else(|| std::env::consts::ARCH.to_string())
}

fn mem_total_gb() -> Option<f64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: f64 = s
        .lines()
        .find(|l| l.starts_with("MemTotal:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some((kb / 1e6 * 10.0).round() / 10.0)
}

pub fn run(ff: &crate::engine_ffmpeg::Ffmpeg) -> Result<Value, String> {
    let hardware = json!({
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH, "cpu": cpu_model(),
        "cores": std::thread::available_parallelism().map_or(1, |n| n.get()), "ram_gb": mem_total_gb(),
    });
    // Caption engine: shape and page 2,000 words with the Arabic font.
    let m =
        crate::captions::Measurer::new(crate::template::FONTS[1].2.to_vec(), 84.0).ok_or("font")?;
    let words: Vec<crate::captions::Word> = (0..2000)
        .map(|i| crate::captions::Word {
            text: ["مرحبا", "بكم", "في", "راويفوكس."][i % 4].into(),
            start: i as f64 * 0.3,
            end: i as f64 * 0.3 + 0.25,
        })
        .collect();
    let t = Instant::now();
    let layout = crate::captions::Layout {
        max_width: 900.0,
        max_lines: 2,
        max_gap: 0.6,
        max_page: 4.0,
    };
    let pages = crate::captions::paginate(&words, &layout, |s| m.width(s));
    let captions_ms = t.elapsed().as_secs_f64() * 1000.0;
    // Full demo: 8 s at 1080x1920, 30 fps.
    let out = std::env::temp_dir().join(format!("scriptvox-bench-{}.mp4", std::process::id()));
    let t = Instant::now();
    crate::demo::render(ff, &crate::demo::Spec::FULL, &out)?;
    let render_s = t.elapsed().as_secs_f64();
    let _ = std::fs::remove_file(&out);
    let video_s = f64::from(crate::demo::Spec::FULL.secs);
    Ok(json!({
        "hardware": hardware,
        "version": env!("CARGO_PKG_VERSION"),
        "captions": {"words": 2000, "pages": pages.len(), "ms": (captions_ms * 10.0).round() / 10.0},
        "demo_render": {"video_s": video_s, "wall_s": (render_s * 100.0).round() / 100.0, "realtime_x": (video_s / render_s * 10.0).round() / 10.0, "size": "1080x1920", "fps": 30},
    }))
}
