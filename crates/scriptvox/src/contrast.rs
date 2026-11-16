//! Gates `contrast` and `loop_seam`, measured from frames with no model. See docs/specs/gate-contrast-loop.md.
//!
//! `contrast`: for a text event of `captions.ass`, libass draws the letters alone (no outline, no box) on a black and
//! on a white canvas. A pixel that is the same on both is solid letter: that is the mask and the fill colour, with no
//! guess from the picture. The ring of pixels just around the letters is then read from the made video, and the WCAG
//! contrast ratio of the fill against that ring is the measure.

use crate::engine_ffmpeg::Ffmpeg;
use crate::gates::Finding;
use std::path::Path;

/// Under this ratio the gate warns: the WCAG 2 limit for large text (AA).
pub const MIN_RATIO: f64 = 3.0;
/// At most this many text events are measured, so the gate adds about a second to a run.
pub const MAX_SAMPLES: usize = 8;
/// A last frame that differs from the first by more than this share of the grey range is a seam that shows.
pub const SEAM_MAX: f64 = 0.08;

/// Relative luminance of an sRGB pixel (WCAG 2).
fn luminance(p: &[u8]) -> f64 {
    let ch = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(p[0]) + 0.7152 * ch(p[1]) + 0.0722 * ch(p[2])
}

/// WCAG contrast ratio of two luminances, 1 to 21.
fn ratio(a: f64, b: f64) -> f64 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Grows a mask by `r` pixels in every direction (a square), two passes.
fn grow(mask: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let mut rows = vec![false; mask.len()];
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            rows[y * w + x] = mask[y * w + a..=y * w + b].iter().any(|m| *m);
        }
    }
    let mut out = vec![false; mask.len()];
    for y in 0..h {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            out[y * w + x] = (a..=b).any(|k| rows[k * w + x]);
        }
    }
    out
}

/// The contrast of the letters against the pixels around them. `black` and `white`: the letters alone on the two
/// canvases; `frame`: the made video's frame. All RGB24, `w` by `h`. `ring`: how far around a letter counts, in px.
/// Returns the ratio that 80 percent of the ring reaches or passes, for the fill colour that reads worst (karaoke
/// has two). None when the event shows no solid letter (mid fade, or nothing drawn).
pub fn measure(
    black: &[u8],
    white: &[u8],
    frame: &[u8],
    w: usize,
    h: usize,
    ring: usize,
) -> Option<f64> {
    let px = |b: &[u8], i: usize| [b[i * 3], b[i * 3 + 1], b[i * 3 + 2]];
    let solid: Vec<bool> = (0..w * h)
        .map(|i| {
            let (a, b) = (px(black, i), px(white, i));
            (0..3).all(|c| a[c].abs_diff(b[c]) <= 16)
        })
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    let mut fills: Vec<f64> = vec![];
    for (i, _) in solid.iter().enumerate().filter(|(_, s)| **s) {
        let (x, y) = (i % w, i / w);
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        fills.push(luminance(&px(black, i)));
    }
    if fills.len() < 30 {
        return None;
    }
    fills.sort_by(f64::total_cmp);
    let (lo, hi) = (fills[fills.len() / 10], fills[fills.len() * 9 / 10]);
    // Work in the letters' own box plus the ring.
    let (bx, by) = (x0.saturating_sub(ring), y0.saturating_sub(ring));
    let (bw, bh) = (
        (x1 + ring).min(w - 1) - bx + 1,
        (y1 + ring).min(h - 1) - by + 1,
    );
    let part: Vec<bool> = (0..bw * bh)
        .map(|i| solid[(by + i / bw) * w + bx + i % bw])
        .collect();
    // One pixel next to a letter is its soft edge, not its surroundings.
    let (near, far) = (grow(&part, bw, bh, 1), grow(&part, bw, bh, ring));
    let mut ratios: Vec<f64> = (0..bw * bh)
        .filter(|&i| far[i] && !near[i])
        .map(|i| {
            let l = luminance(&px(frame, (by + i / bw) * w + bx + i % bw));
            ratio(lo, l).min(ratio(hi, l))
        })
        .collect();
    if ratios.is_empty() {
        return None;
    }
    ratios.sort_by(f64::total_cmp);
    Some(ratios[ratios.len() / 5])
}

fn seconds(t: &str) -> f64 {
    t.split(':')
        .fold(0.0, |sum, p| sum * 60.0 + p.parse::<f64>().unwrap_or(0.0))
}

fn clock(t: f64) -> String {
    let n = (t.max(0.0) * 100.0).round() as u64;
    format!(
        "{}:{:02}:{:02}.{:02}",
        n / 360000,
        n / 6000 % 60,
        n / 100 % 60,
        n % 100
    )
}

/// The text of a dialogue line without its override blocks.
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in text.chars() {
        match c {
            '{' => tag = true,
            '}' => tag = false,
            c if !tag && !matches!(c, '\u{200E}' | '\u{200F}') => out.push(c),
            _ => {}
        }
    }
    out.replace("\\N", " ").replace("\\h", " ")
}

/// One text event to measure: the time to look at, and an ASS document that draws its letters alone at that time.
struct Sample {
    t: f64,
    rel: f64,
    ass: String,
    shown: String,
}

/// Up to `MAX_SAMPLES` text events of the document, spread over the video: first the hook, the cards and the
/// second line (at most half), then the captions.
fn samples(ass: &str) -> Vec<Sample> {
    // The letters alone: outline style with no outline and no shadow, so no box either.
    let head: String = ass
        .lines()
        .filter(|l| !l.starts_with("Dialogue:"))
        .map(|l| {
            let mut f: Vec<&str> = l.split(',').collect();
            if l.starts_with("Style: ") && f.len() > 17 {
                (f[15], f[16], f[17]) = ("1", "0", "0");
            }
            f.join(",") + "\n"
        })
        .collect();
    let (mut caps, mut others) = (vec![], vec![]);
    for l in ass.lines().filter(|l| l.starts_with("Dialogue:")) {
        let f: Vec<&str> = l.splitn(10, ',').collect();
        let Some(text) = f.get(9) else { continue };
        let (start, end) = (seconds(f[1]), seconds(f[2]));
        let shown = plain(text);
        // A drawing has no letters. A short event is a step of a count, or still fading.
        if text.contains("\\p1") || shown.trim().is_empty() || end - start < 0.8 {
            continue;
        }
        // The middle, or 1.5 s in for a long event (a label can outlast the video): every fade is over by then.
        let rel = ((end - start) / 2.0).min(1.5);
        // An unknown tag is ignored by libass: this takes an event's own outline and shadow away.
        let text = text
            .replace("\\bord", "\\xbord")
            .replace("\\shad", "\\xshad");
        let event = format!(
            // Its own length from time 0, so a `\\move` or a `\\t` with no times is where the video has it.
            "Dialogue: {},0:00:00.00,{},{},{},{},{},{},{},{text}\n",
            f[0].trim_start_matches("Dialogue:").trim(),
            clock(end - start),
            f[3],
            f[4],
            f[5],
            f[6],
            f[7],
            f[8]
        );
        let s = Sample {
            t: start + rel,
            rel,
            ass: format!("{head}{event}"),
            shown: shown.trim().chars().take(32).collect(),
        };
        if f[3] == "Cap" {
            caps.push(s);
        } else {
            others.push(s);
        }
    }
    let spread = |mut v: Vec<Sample>, n: usize| -> Vec<Sample> {
        if v.len() <= n {
            return v;
        }
        let len = v.len();
        let keep: Vec<usize> = (0..n).map(|i| (i * 2 + 1) * len / (n * 2)).collect();
        let mut i = 0;
        v.retain(|_| {
            i += 1;
            keep.contains(&(i - 1))
        });
        v
    };
    let mut out = spread(others, MAX_SAMPLES / 2);
    let room = MAX_SAMPLES - out.len();
    out.extend(spread(caps, room));
    out
}

/// The `contrast` gate for a made video. `dir` is the run folder with `captions.ass` and `fonts`; `fc` its fontconfig
/// file. None when the video burns no text.
pub fn gate(ff: &Ffmpeg, dir: &Path, video: &Path, fc: &Path) -> Option<Finding> {
    let ass = std::fs::read_to_string(dir.join("captions.ass")).ok()?;
    let res = |key: &str| -> Option<usize> {
        ass.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.trim().parse().ok())
    };
    let (w, h) = (res("PlayResX:")?, res("PlayResY:")?);
    let picked = samples(&ass);
    if picked.is_empty() {
        return None;
    }
    let ring = (4 * w.min(h) / 1080).max(3);
    let (mut worst, mut measured, mut failed): (Option<(f64, &Sample)>, usize, Option<String>) =
        (None, 0, None);
    for s in &picked {
        if std::fs::write(dir.join("contrast.ass"), &s.ass).is_err() {
            continue;
        }
        // One frame whose time is `rel`, in a fine time base: in the source's own (1 s) the time rounds to 0, where
        // an event is still fading in and no karaoke word is lit.
        let canvas = |colour: &str, out: &str| {
            format!(
                "color={colour}:s={w}x{h}:r=1:d=1,settb=AVTB,setpts={:.3}/TB,{},setpts=0,format=rgb24[{out}]",
                s.rel,
                crate::filterpath::ass_filter("contrast.ass", "fonts")
            )
        };
        let graph = format!(
            "{};{};[0:v]trim=end_frame=1,scale={w}:{h},setsar=1,setpts=0,format=rgb24[c];[a][b][c]vstack=inputs=3[o]",
            canvas("black", "a"),
            canvas("white", "b")
        );
        let args: Vec<String> = [
            "-ss",
            &format!("{:.3}", s.t),
            "-i",
            &video.to_string_lossy(),
            "-filter_complex",
            &graph,
            "-map",
            "[o]",
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "contrast.rgb",
        ]
        .map(String::from)
        .to_vec();
        if let Err(e) = ff.run(dir, &args, Some(fc)) {
            failed = Some(e.lines().last().unwrap_or("").to_string());
            continue;
        }
        let Ok(raw) = std::fs::read(dir.join("contrast.rgb")) else {
            continue;
        };
        let n = w * h * 3;
        if raw.len() < n * 3 {
            continue;
        }
        if let Some(r) = measure(&raw[..n], &raw[n..n * 2], &raw[n * 2..n * 3], w, h, ring) {
            measured += 1;
            if worst.is_none_or(|(b, _)| r < b) {
                worst = Some((r, s));
            }
        }
    }
    for f in ["contrast.ass", "contrast.rgb"] {
        let _ = std::fs::remove_file(dir.join(f));
    }
    let finding = |status: &'static str, detail: String, fix: Option<&str>| Finding {
        gate: "contrast",
        status,
        detail,
        fix: fix.map(String::from),
    };
    Some(match worst {
        None => finding(
            "warn",
            format!(
                "not measured: no solid letters found in {} text boxes{}",
                picked.len(),
                failed.map_or(String::new(), |e| format!(" ({e})"))
            ),
            Some("Look at a snapshot of the captions."),
        ),
        Some((r, s)) if r < MIN_RATIO => finding(
            "warn",
            format!(
                "\"{}\" at {:.1} s has a contrast of {r:.1} to 1 against the picture around its letters (under {MIN_RATIO} to 1); {measured} text boxes measured",
                s.shown, s.t
            ),
            Some("Give the captions their box back (`caption_box`), a wider `caption_outline`, or a darker picture there. For a card on a light `background`: `card_style: paper`. Then look at a snapshot at that time."),
        ),
        Some((r, _)) => finding(
            "pass",
            format!("{measured} text boxes measured, lowest contrast {r:.1} to 1"),
            None,
        ),
    })
}

/// The share of the grey range by which two frames of the same size differ, 0 to 1.
pub fn frame_difference(a: &[u8], b: &[u8]) -> f64 {
    let sum: u64 = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y) as u64).sum();
    sum as f64 / (a.len().max(1) as f64 * 255.0)
}

/// The `loop_seam` gate (brief `loop`): the last frame against the first, both as 64 by 36 grey pictures.
pub fn loop_seam(ff: &Ffmpeg, dir: &Path, video: &Path) -> Finding {
    let grab = |seek: &[&str], out: &str| -> Result<Vec<u8>, String> {
        let mut args: Vec<String> = seek.iter().map(|s| s.to_string()).collect();
        args.extend(
            [
                "-i",
                &video.to_string_lossy(),
                "-vf",
                "scale=64:36,format=gray",
                "-f",
                "rawvideo",
                out,
            ]
            .map(String::from),
        );
        ff.run(dir, &args, None)?;
        let raw = std::fs::read(dir.join(out)).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(dir.join(out));
        // The last whole frame of what was read.
        let n = 64 * 36;
        let from = (raw.len() / n).checked_sub(1).ok_or("no frame")? * n;
        Ok(raw[from..from + n].to_vec())
    };
    let finding = |status: &'static str, detail: String, fix: Option<&str>| Finding {
        gate: "loop_seam",
        status,
        detail,
        fix: fix.map(String::from),
    };
    let first = grab(&["-t", "0.01"], "seam_first.gray");
    let last = grab(&["-sseof", "-0.5"], "seam_last.gray");
    match (first, last) {
        (Ok(a), Ok(b)) => {
            let d = frame_difference(&a, &b);
            if d > SEAM_MAX {
                finding(
                    "warn",
                    format!(
                        "the last frame differs from the first by {:.0} percent of the grey range (over {:.0}): the loop will jump",
                        d * 100.0,
                        SEAM_MAX * 100.0
                    ),
                    Some("End on the picture the video starts on (the same media, card or background in the first and the last scene), with no outro. Or drop `loop`."),
                )
            } else {
                finding(
                    "pass",
                    format!(
                        "the last frame differs from the first by {:.1} percent of the grey range",
                        d * 100.0
                    ),
                    None,
                )
            }
        }
        (Err(e), _) | (_, Err(e)) => finding(
            "warn",
            format!(
                "not checked: a frame could not be read ({})",
                e.lines().last().unwrap_or("")
            ),
            Some("Look at the first and the last frame with `scriptvox snapshot`."),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `w` by `h` RGB picture of one colour with a block of another.
    fn canvas(w: usize, h: usize, back: [u8; 3], block: Option<[u8; 3]>) -> Vec<u8> {
        let mut v = Vec::with_capacity(w * h * 3);
        for i in 0..w * h {
            let (x, y) = (i % w, i / w);
            let inside = (20..40).contains(&x) && (10..20).contains(&y);
            v.extend(block.filter(|_| inside).unwrap_or(back));
        }
        v
    }

    #[test]
    fn the_ratio_is_the_fill_against_the_ring_around_the_letters() {
        let (w, h) = (60, 30);
        let white = [255u8; 3];
        // The letters alone, drawn white: solid on the black canvas, and no change on the white one.
        let on_black = canvas(w, h, [0; 3], Some(white));
        let on_white = canvas(w, h, white, Some(white));
        let over = |picture: [u8; 3]| {
            measure(
                &on_black,
                &on_white,
                &canvas(w, h, picture, Some(white)),
                w,
                h,
                4,
            )
            .unwrap()
        };
        // White on white cannot be read; white on black is the top of the scale.
        assert!(over([250; 3]) < 1.1);
        assert!(over([0; 3]) > 20.0);
        // White on mid grey (#777) is 4.5 to 1, the well known WCAG pair.
        assert!((over([0x77; 3]) - 4.48).abs() < 0.1, "{}", over([0x77; 3]));
        // A black outline 4 px wide on a white picture: the ring is the outline, so the letters read.
        let mut outlined = canvas(w, h, white, Some(white));
        for i in 0..w * h {
            let (x, y) = (i % w, i / w);
            let edge = (16..44).contains(&x) && (6..24).contains(&y);
            let inside = (20..40).contains(&x) && (10..20).contains(&y);
            if edge && !inside {
                outlined[i * 3..i * 3 + 3].copy_from_slice(&[0; 3]);
            }
        }
        assert!(measure(&on_black, &on_white, &outlined, w, h, 4).unwrap() > 20.0);
        // Two fill colours (karaoke): the one that reads worst counts. Yellow and white on a yellow picture.
        let yellow = [255u8, 229, 0];
        let mut two = on_black.clone();
        for i in 0..w * h {
            if (20..30).contains(&(i % w)) && (10..20).contains(&(i / w)) {
                two[i * 3..i * 3 + 3].copy_from_slice(&yellow);
            }
        }
        let mut two_white = on_white.clone();
        two_white.copy_from_slice(&two);
        for i in 0..w * h {
            let inside = (20..40).contains(&(i % w)) && (10..20).contains(&(i / w));
            if !inside {
                two_white[i * 3..i * 3 + 3].copy_from_slice(&white);
            }
        }
        let r = measure(&two, &two_white, &canvas(w, h, yellow, None), w, h, 4).unwrap();
        assert!(r < 1.1, "{r}");
        // Nothing solid (an event still fading): not measured.
        assert!(measure(
            &canvas(w, h, [0; 3], None),
            &canvas(w, h, white, None),
            &on_black,
            w,
            h,
            4
        )
        .is_none());
    }

    #[test]
    fn samples_take_text_events_and_draw_the_letters_alone() {
        let ass = "PlayResX: 640\nStyle: Cap,Noto Sans,84,&H0000E5FF,&H00FFFFFF,&H59000000,&H59000000,1,0,0,0,100,100,0,0,4,14,0,2,60,60,97,-1\n\
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
Dialogue: 2,0:00:01.00,0:00:03.00,Card,,0,0,0,,{\\an5\\pos(10,10)\\shad5}Big, title\n\
Dialogue: 2,0:00:01.00,0:00:03.00,Card,,0,0,0,,{\\p1}m 0 0 l 10 0\n\
Dialogue: 2,0:00:01.00,0:00:01.10,Card,,0,0,0,,12%\n\
Dialogue: 0,0:00:04.00,0:00:06.00,Cap,,0,0,0,,{\\k50}Hello {\\k50}there\n";
        let s = samples(ass);
        assert_eq!(s.len(), 2);
        assert_eq!(
            (s[0].shown.as_str(), s[0].t, s[0].rel),
            ("Big, title", 2.0, 1.0)
        );
        assert_eq!(s[1].shown, "Hello there");
        // No box, no outline, no shadow, and the event alone from time 0.
        assert!(
            s[0].ass.contains(",0,0,1,0,0,2,60,60,97,-1\n"),
            "{}",
            s[0].ass
        );
        assert!(s[0].ass.ends_with(
            "Dialogue: 2,0:00:00.00,0:00:02.00,Card,,0,0,0,,{\\an5\\pos(10,10)\\xshad5}Big, title\n"
        ));
        assert_eq!(s[0].ass.matches("Dialogue:").count(), 1);
        // Many captions: at most MAX_SAMPLES, spread over the video.
        let many: String = (0..40)
            .map(|i| {
                format!(
                    "Dialogue: 0,0:00:{i:02}.00,0:00:{:02}.00,Cap,,0,0,0,,Word {i}\n",
                    i + 1
                )
            })
            .collect();
        let s = samples(&many);
        assert_eq!(s.len(), MAX_SAMPLES);
        assert!(s[0].t < 5.0 && s[MAX_SAMPLES - 1].t > 35.0);
    }

    #[test]
    fn frames_differ_by_a_share_of_the_grey_range() {
        assert_eq!(frame_difference(&[10, 10], &[10, 10]), 0.0);
        assert_eq!(frame_difference(&[0, 0], &[255, 255]), 1.0);
        assert!((frame_difference(&[0, 100], &[51, 100]) - 0.1).abs() < 1e-9);
    }
}
