//! Upload limits and screen zones per platform, from `registry/platforms.toml` (G057, docs/specs/platforms.md).
//! `check --platform` compares a video with its row. `snapshot --safe` draws the row's zones.

use crate::engine_ffmpeg::Ffmpeg;
use crate::gates::Finding;
use serde::Deserialize;
use std::path::Path;

const TABLE: &str = include_str!("../../../registry/platforms.toml");

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Safe {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Platform {
    pub name: String,
    pub label: String,
    pub aspects: Vec<String>,
    pub max_seconds: Option<f64>,
    pub max_mb: Option<f64>,
    pub safe: Option<Safe>,
    pub read: String,
    pub source: String,
    pub url: String,
    pub note: String,
}

#[derive(Deserialize)]
struct Table {
    platform: Vec<Platform>,
}

pub fn all() -> Vec<Platform> {
    // The table is compiled in, and a unit test parses it.
    toml::from_str::<Table>(TABLE).map_or_else(|_| Vec::new(), |t| t.platform)
}

pub fn find(name: &str) -> Result<Platform, String> {
    let rows = all();
    rows.iter()
        .find(|p| p.name == name)
        .cloned()
        .ok_or_else(|| {
            let names: Vec<&str> = rows.iter().map(|p| p.name.as_str()).collect();
            format!("no platform {name}: use {}", names.join(", "))
        })
}

fn shape((w, h): (u32, u32)) -> String {
    fn gcd(a: u32, b: u32) -> u32 {
        if b == 0 {
            a
        } else {
            gcd(b, a % b)
        }
    }
    let g = gcd(w, h).max(1);
    format!("{}:{}", w / g, h / g)
}

/// The facts compared with the row. Pure, so the rules have a test with no video.
fn compare(p: &Platform, size: (u32, u32), seconds: f64, mb: f64) -> Finding {
    let got = shape(size);
    // Within 1%: an encoder can round 1080x1920 to 1080x1918, which is still 9:16 to every platform.
    let ratio = size.0 as f64 / size.1.max(1) as f64;
    let fits = |a: &String| {
        a.split_once(':')
            .and_then(|(w, h)| Some(w.parse::<f64>().ok()? / h.parse::<f64>().ok()?))
            .is_some_and(|r| (ratio / r - 1.0).abs() < 0.01)
    };
    let mut bad = vec![];
    if !p.aspects.iter().any(fits) {
        bad.push(format!(
            "the frame is {got}, and {} takes {}",
            p.label,
            p.aspects.join(" or ")
        ));
    }
    if let Some(m) = p.max_seconds.filter(|m| seconds > *m) {
        bad.push(format!(
            "it runs {seconds:.1} s, over the limit of {m:.0} s"
        ));
    }
    if let Some(m) = p.max_mb.filter(|m| mb > *m) {
        bad.push(format!(
            "the file is {mb:.0} MB, over the limit of {m:.0} MB"
        ));
    }
    let unchecked: Vec<&str> = [
        (p.max_seconds.is_none(), "length"),
        (p.max_mb.is_none(), "file size"),
    ]
    .into_iter()
    .filter(|x| x.0)
    .map(|x| x.1)
    .collect();
    let mut tail = format!(" Table read {}, source {}.", p.read, p.source);
    if !unchecked.is_empty() {
        tail += &format!(
            " No limit on record, so not checked: {}.",
            unchecked.join(", ")
        );
    }
    Finding {
        gate: "platform",
        status: if bad.is_empty() { "pass" } else { "fail" },
        detail: if bad.is_empty() {
            format!("fits {}.{tail}", p.label)
        } else {
            format!("{}.{tail}", bad.join("; "))
        },
        fix: (!bad.is_empty()).then(|| {
            format!(
                "Use `scriptvox export` or `edit --reframe` to fit it, then check again. {}",
                p.url
            )
        }),
    }
}

pub fn check(ff: &Ffmpeg, video: &Path, p: &Platform) -> Option<Finding> {
    let size = ff.shown_size(video)?;
    let seconds = ff.duration(video).unwrap_or(0.0);
    let mb = std::fs::metadata(video).ok()?.len() as f64 / 1e6;
    Some(compare(p, size, seconds, mb))
}

/// FFmpeg filters that shade the zones the platform's own screen covers, for a picture of any size.
pub fn zones_filter(s: Safe) -> String {
    let edge = |x: &str, y: &str, w: &str, h: &str| {
        format!(",drawbox=x={x}:y={y}:w={w}:h={h}:color=red@0.35:t=fill")
    };
    let mut f = String::new();
    if s.top > 0.0 {
        f += &edge("0", "0", "iw", &format!("ih*{}", s.top / 100.0));
    }
    if s.bottom > 0.0 {
        f += &edge(
            "0",
            &format!("ih*{}", 1.0 - s.bottom / 100.0),
            "iw",
            &format!("ih*{}", s.bottom / 100.0),
        );
    }
    if s.left > 0.0 {
        f += &edge("0", "0", &format!("iw*{}", s.left / 100.0), "ih");
    }
    if s.right > 0.0 {
        f += &edge(
            &format!("iw*{}", 1.0 - s.right / 100.0),
            "0",
            &format!("iw*{}", s.right / 100.0),
            "ih",
        );
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_parses_and_every_row_says_where_it_comes_from() {
        let rows = all();
        assert!(rows.len() >= 4);
        for p in rows {
            assert!(
                matches!(p.source.as_str(), "official" | "secondary"),
                "{}",
                p.name
            );
            assert!(
                p.url.starts_with("https://") && p.read.len() == 10,
                "{}",
                p.name
            );
            assert!(!p.aspects.is_empty(), "{}", p.name);
        }
    }

    #[test]
    fn a_wrong_shape_or_length_fails_and_an_unread_limit_is_said() {
        let p = find("youtube-shorts").unwrap();
        let v = compare(&p, (1080, 1920), 60.0, 20.0);
        assert_eq!(v.status, "pass", "{v:?}");
        assert!(v.detail.contains("file size"), "{v:?}");
        assert_eq!(compare(&p, (1080, 1918), 60.0, 20.0).status, "pass");
        let v = compare(&p, (1920, 1080), 181.0, 20.0);
        assert_eq!(v.status, "fail", "{v:?}");
        assert!(
            v.detail.contains("16:9") && v.detail.contains("181.0"),
            "{v:?}"
        );
        assert_eq!(
            compare(&find("tiktok").unwrap(), (1080, 1920), 3.0, 501.0).status,
            "fail"
        );
        assert!(find("myspace").unwrap_err().contains("youtube-shorts"));
    }

    #[test]
    fn zones_are_drawn_only_where_the_row_has_them() {
        let f = zones_filter(Safe {
            top: 10.0,
            bottom: 0.0,
            left: 0.0,
            right: 25.0,
        });
        assert_eq!(f.matches("drawbox").count(), 2, "{f}");
        assert!(f.contains("ih*0.1") && f.contains("iw*0.75"), "{f}");
    }
}
