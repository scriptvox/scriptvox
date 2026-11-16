//! Subject tracking for `edit --reframe --track` (P3-006). A vision model gives the main subject's box on frames
//! sampled through the video; the box centres are smoothed and become a time expression for FFmpeg `crop`, so the
//! new frame follows the subject instead of cutting the centre. Same free self-hosted row as `scriptvox judge`.

use crate::engine_ffmpeg::Ffmpeg;
use crate::exit::Exit;
use base64::Engine as _;
use serde_json::{json, Value};
use std::path::Path;

/// Frames in one vision request.
pub const BATCH: usize = 12;
/// Most samples for one video (4 requests); a longer video gets fewer than 2 a second.
pub const MAX_SAMPLES: usize = 96;

/// Sample times per shot: about 2 a second (fewer when the video would need more than `MAX_SAMPLES`), at least one
/// a shot, at the middle of equal slices. Returns (shot index, time).
pub fn sample_plan(shots: &[(f64, f64)]) -> Vec<(usize, f64)> {
    let total: f64 = shots.iter().map(|(a, b)| b - a).sum();
    let rate = 2.0_f64.min(MAX_SAMPLES as f64 / total.max(0.001));
    let mut out = vec![];
    for (i, &(a, b)) in shots.iter().enumerate() {
        let n = (((b - a) * rate).ceil() as usize).max(1);
        out.extend((0..n).map(|k| (i, a + (b - a) * (k as f64 + 0.5) / n as f64)));
    }
    out
}

/// Box centres (0 to 1) from the reply `{"boxes": [[x1,y1,x2,y2] or null, ...]}` in 0 to 1000 units; `None` where
/// the box is missing or broken.
pub fn parse_centres(text: &str, n: usize) -> Result<Vec<Option<(f64, f64)>>, String> {
    let (a, b) = (text.find('{'), text.rfind('}'));
    let body = match (a, b) {
        (Some(a), Some(b)) if a < b => &text[a..=b],
        _ => return Err("the reply holds no JSON object".into()),
    };
    let v: Value = serde_json::from_str(body).map_err(|e| format!("the reply is not JSON: {e}"))?;
    let boxes = v["boxes"].as_array().ok_or("the reply has no boxes")?;
    if boxes.len() != n {
        return Err(format!("the reply has {} boxes, not {n}", boxes.len()));
    }
    Ok(boxes
        .iter()
        .map(|b| {
            let c: Vec<f64> = b.as_array()?.iter().filter_map(|x| x.as_f64()).collect();
            (c.len() == 4 && c[0] < c[2] && c[1] < c[3]).then(|| {
                (
                    ((c[0] + c[2]) / 2000.0).clamp(0.0, 1.0),
                    ((c[1] + c[3]) / 2000.0).clamp(0.0, 1.0),
                )
            })
        })
        .collect())
}

/// Boxes (x1, y1, x2, y2, 0 to 1) from the same reply, for the samples that have a box.
fn parse_boxes(text: &str) -> Vec<[f64; 4]> {
    let (a, b) = (text.find('{').unwrap_or(0), text.rfind('}').unwrap_or(0));
    let v: Value = serde_json::from_str(text.get(a..=b).unwrap_or("")).unwrap_or_default();
    v["boxes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| {
            let c: Vec<f64> = b.as_array()?.iter().filter_map(|x| x.as_f64()).collect();
            (c.len() == 4 && c[0] < c[2] && c[1] < c[3])
                .then(|| [c[0] / 1000.0, c[1] / 1000.0, c[2] / 1000.0, c[3] / 1000.0])
        })
        .collect()
}

/// Box sizes (0 to 1) from the same reply, for the samples that have a box.
pub fn parse_sizes(text: &str) -> Vec<(f64, f64)> {
    parse_boxes(text)
        .iter()
        .map(|b| (b[2] - b[0], b[3] - b[1]))
        .collect()
}

/// How many boxes reach outside a crop in the middle of the frame. The 2 percent margin is for the error of a box.
fn cut_by_centre(boxes: &[[f64; 4]], keep_w: f64, keep_h: f64) -> usize {
    let (x, y) = ((1.0 - keep_w) / 2.0 - 0.02, (1.0 - keep_h) / 2.0 - 0.02);
    boxes
        .iter()
        .filter(|b| b[0] < x || b[2] > 1.0 - x || b[1] < y || b[3] > 1.0 - y)
        .count()
}

/// A centre crop has no eye: ask the vision row, on 3 frames of `from` to `from + secs`, if the crop cuts the face or
/// subject. No vision row or no usable reply gives no opinion.
pub fn centre_crop_cuts(
    ff: &Ffmpeg,
    video: &Path,
    (from, secs): (f64, f64),
    (keep_w, keep_h): (f64, f64),
    subject: Option<&str>,
    dir: &Path,
) -> Option<String> {
    let vision = crate::judge::need_vision().ok()?;
    let mut content = vec![];
    for (i, f) in [0.2, 0.5, 0.8].iter().enumerate() {
        let jpg = crate::judge::grab(
            ff,
            video,
            from + secs * f,
            512,
            &dir.join(format!("centre{}.jpg", i + 1)),
        )
        .ok()?;
        content.push(json!({"type": "image_url", "image_url": {"url": format!(
            "data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&jpg))}}));
    }
    let what = subject.unwrap_or(
        "the face of the person a viewer watches, or with no person the main animal or thing",
    );
    content.push(json!({"type": "text", "text": format!(
        "Three frames of one video. For each frame give the bounding box of {what} as [x1, y1, x2, y2] in units of 0 \
         to 1000 of the frame width and height, or null when it is not in the frame. Reply with JSON only: \
         {{\"boxes\": [...]}} with one entry per frame.")}));
    let reply = crate::judge::ask(
        &vision,
        &json!({
            "model": crate::judge::vision_model(),
            "temperature": 0,
            "max_tokens": 160,
            "messages": [{"role": "user", "content": content}],
            "chat_template_kwargs": {"enable_thinking": false}
        }),
    )
    .ok()?;
    let boxes = parse_boxes(&reply);
    let cut = cut_by_centre(&boxes, keep_w, keep_h);
    (cut > 0).then(|| {
        format!(
            "the centre crop cuts {} on {cut} of {} frames that show it",
            subject.unwrap_or("the face or main subject"),
            boxes.len()
        )
    })
}

/// The middle value of each side of the box sizes.
pub fn median_size(sizes: &[(f64, f64)]) -> Option<(f64, f64)> {
    let mid = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    (!sizes.is_empty()).then(|| {
        (
            mid(sizes.iter().map(|s| s.0).collect()),
            mid(sizes.iter().map(|s| s.1).collect()),
        )
    })
}

/// Fill the gaps in one shot: a missing box repeats the last one, missing boxes at the start take the first box
/// found, and a shot with no box at all keeps the middle.
pub fn fill(c: &[Option<(f64, f64)>]) -> Vec<(f64, f64)> {
    let mut prev = c.iter().flatten().next().copied().unwrap_or((0.5, 0.5));
    c.iter()
        .map(|p| {
            if let Some(p) = p {
                prev = *p;
            }
            prev
        })
        .collect()
}

/// The crop path through all shots: per shot the smoothed samples, held flat from the shot start to its first sample
/// and from its last sample to the cut, then a jump of 2 ms at the cut. Never smoothed across a cut.
pub fn path(
    shots: &[(f64, f64)],
    plan: &[(usize, f64)],
    c: &[Option<(f64, f64)>],
    dead: f64,
) -> Path3 {
    let (mut ts, mut xs, mut ys) = (vec![], vec![], vec![]);
    for (i, &(a, b)) in shots.iter().enumerate() {
        let idx: Vec<usize> = (0..plan.len()).filter(|&k| plan[k].0 == i).collect();
        if idx.is_empty() {
            continue;
        }
        let f = fill(&idx.iter().map(|&k| c[k]).collect::<Vec<_>>());
        let sx = smooth(&f.iter().map(|p| p.0).collect::<Vec<_>>(), dead);
        let sy = smooth(&f.iter().map(|p| p.1).collect::<Vec<_>>(), dead);
        let end = if i + 1 < shots.len() { b - 0.002 } else { b };
        let mut tt = vec![a];
        tt.extend(idx.iter().map(|&k| plan[k].1));
        tt.push(end);
        let mut cx = vec![sx[0]];
        cx.extend(&sx);
        cx.push(*sx.last().unwrap());
        let mut cy = vec![sy[0]];
        cy.extend(&sy);
        cy.push(*sy.last().unwrap());
        ts.extend(tt);
        xs.extend(cx);
        ys.extend(cy);
    }
    (ts, xs, ys)
}

/// Calm the path: a 3-point median removes single wrong boxes, then a dead zone holds the frame still until the
/// subject moves more than `dead` (share of the frame).
pub fn smooth(xs: &[f64], dead: f64) -> Vec<f64> {
    let med: Vec<f64> = (0..xs.len())
        .map(|i| {
            let mut w: Vec<f64> = xs[i.saturating_sub(1)..(i + 2).min(xs.len())].to_vec();
            w.sort_by(|a, b| a.total_cmp(b));
            w[w.len() / 2]
        })
        .collect();
    let mut out = Vec::with_capacity(med.len());
    let mut held = *med.first().unwrap_or(&0.5);
    for &m in &med {
        if (m - held).abs() > dead {
            held = m;
        }
        out.push(held);
    }
    out
}

/// A piecewise-linear FFmpeg expression of `t` through the points, held flat before the first and after the last.
pub fn lerp_expr(ts: &[f64], cs: &[f64]) -> String {
    if cs.len() < 2 || cs.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-6) {
        return format!("{:.4}", cs.first().copied().unwrap_or(0.5));
    }
    let seg = |i: usize| {
        format!(
            "({:.4}+({:.4})*clip((t-{:.3})/{:.3},0,1))",
            cs[i],
            cs[i + 1] - cs[i],
            ts[i],
            (ts[i + 1] - ts[i]).max(0.001)
        )
    };
    // A balanced tree: FFmpeg's expression parser stops at a depth of about 100, and a chain of one `if` per sample
    // failed on any tracked clip over about 50 s (a real talk, 5 Nov 2026).
    fn tree(ts: &[f64], seg: &dyn Fn(usize) -> String, lo: usize, hi: usize) -> String {
        if hi - lo == 1 {
            return seg(lo);
        }
        let mid = (lo + hi) / 2;
        format!(
            "if(lt(t,{:.3}),{},{})",
            ts[mid],
            tree(ts, seg, lo, mid),
            tree(ts, seg, mid, hi)
        )
    }
    tree(ts, &seg, 0, cs.len() - 1)
}

/// The FFmpeg filter that scales to cover `w`x`h` and crops along the tracked centres.
pub fn crop_filter(w: u32, h: u32, ts: &[f64], cx: &[f64], cy: &[f64]) -> String {
    format!(
        "scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}:x='clip(({})*iw-ow/2,0,iw-ow)':y='clip(({})*ih-oh/2,0,ih-oh)',setsar=1",
        lerp_expr(ts, cx),
        lerp_expr(ts, cy)
    )
}

/// Sample times with smoothed x and y centres.
pub type Path3 = (Vec<f64>, Vec<f64>, Vec<f64>);

/// What one tracking run found.
pub struct Tracked {
    pub path: Path3,
    pub samples: usize,
    pub shots: usize,
    pub requests: usize,
    /// Samples with no answer from the model: the crop holds the last box there.
    pub lost: usize,
    /// Median size of the subject's box, as a share of the frame width and height.
    pub subject: Option<(f64, f64)>,
}

/// Ask the vision row for the subject's centre on frames sampled per shot (cuts as `analyze` finds them), in
/// requests of at most `BATCH` frames, and build the crop path.
pub fn track(
    ff: &Ffmpeg,
    video: &Path,
    total: f64,
    subject: Option<&str>,
) -> Result<Tracked, (Exit, String)> {
    let vision = crate::judge::need_vision()?;
    let cuts = crate::analyze::picture_cuts(ff, video, 0.25).map_err(|e| (Exit::Error, e))?;
    let shots = crate::analyze::shot_stats(&cuts, total).0;
    let plan = sample_plan(&shots);
    // The sampled frames stay next to the output, so a person can check what the model saw.
    let stem = video
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let dir = std::path::PathBuf::from("scriptvox-out").join(format!("{stem}.track"));
    std::fs::create_dir_all(&dir).map_err(|e| (Exit::Error, format!("{}: {e}", dir.display())))?;
    let what = subject.unwrap_or("the main subject (the person, animal or thing a viewer watches)");
    let mut found = vec![];
    let mut requests = 0;
    let mut lost = 0;
    let mut last_error = None;
    let mut sizes = vec![];
    for (b, batch) in plan.chunks(BATCH).enumerate() {
        let mut content = vec![];
        for (j, (_, t)) in batch.iter().enumerate() {
            let i = b * BATCH + j;
            let jpg =
                crate::judge::grab(ff, video, *t, 512, &dir.join(format!("f{:02}.jpg", i + 1)))
                    .map_err(|e| (Exit::Error, e))?;
            content.push(json!({"type": "text", "text": format!("Frame {}:", j + 1)}));
            content.push(json!({"type": "image_url", "image_url": {"url": format!(
                "data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&jpg))}}));
        }
        content.push(json!({"type": "text", "text": format!(
            "These {} frames come from one video, in time order. For each frame give the bounding box of {what} as \
             [x1, y1, x2, y2] in units of 0 to 1000 of the frame width and height, or null when it is not in the \
             frame. Reply with JSON only: {{\"boxes\": [...]}} with one entry per frame.", batch.len())}));
        let body = json!({
            "model": crate::judge::vision_model(),
            "temperature": 0,
            "max_tokens": 64 + 32 * batch.len(),
            "messages": [{"role": "user", "content": content}],
            "chat_template_kwargs": {"enable_thinking": false}
        });
        // One reply in prose must not lose a long clip: ask once more, then hold the last box for this batch.
        let mut got = None;
        for _ in 0..2 {
            requests += 1;
            let text = crate::judge::ask(&vision, &body)?;
            match parse_centres(&text, batch.len()) {
                Ok(c) => {
                    sizes.extend(parse_sizes(&text));
                    got = Some(c);
                    break;
                }
                Err(e) => last_error = Some(e),
            }
        }
        if got.is_none() {
            lost += batch.len();
        }
        found.extend(got.unwrap_or_else(|| vec![None; batch.len()]));
    }
    if lost == plan.len() {
        let why = last_error.unwrap_or_else(|| "no frames".into());
        return Err((Exit::ProviderRetryable, why));
    }
    Ok(Tracked {
        path: path(&shots, &plan, &found, 0.04),
        samples: plan.len(),
        shots: shots.len(),
        requests,
        lost,
        subject: median_size(&sizes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_outside_the_middle_is_cut() {
        // 9:16 from 16:9 keeps 32 percent: the middle is 340 to 660.
        let b = parse_boxes(
            "{\"boxes\": [[400,100,600,300], null, [700,100,800,300], [330,100,670,300]]}",
        );
        assert_eq!(b.len(), 3);
        assert_eq!(cut_by_centre(&b[..1], 0.32, 1.0), 0);
        assert_eq!(
            cut_by_centre(&b, 0.32, 1.0),
            1,
            "the small overhang is in the margin"
        );
        assert_eq!(cut_by_centre(&b, 1.0, 1.0), 0);
    }

    #[test]
    fn box_sizes_give_the_median() {
        let s = parse_sizes(
            "```json\n{\"boxes\": [[0,0,700,500], null, [100,0,500,100], [0,0,600,300]]}\n```",
        );
        assert_eq!(s, vec![(0.7, 0.5), (0.4, 0.1), (0.6, 0.3)]);
        assert_eq!(median_size(&s), Some((0.6, 0.3)));
        assert_eq!(median_size(&parse_sizes("no json")), None);
    }

    #[test]
    fn centres_fill_gaps_with_the_last_box() {
        let c = parse_centres(
            "```json\n{\"boxes\": [[100,200,300,400], null, [600,0,800,1000]]}\n```",
            3,
        )
        .unwrap();
        assert_eq!(c, vec![Some((0.2, 0.3)), None, Some((0.7, 0.5))]);
        assert_eq!(fill(&c), vec![(0.2, 0.3), (0.2, 0.3), (0.7, 0.5)]);
        assert_eq!(fill(&[None, Some((0.1, 0.2))]), vec![(0.1, 0.2); 2]);
        assert_eq!(fill(&[None]), vec![(0.5, 0.5)]);
        assert!(parse_centres("{\"boxes\": []}", 2).is_err());
    }

    #[test]
    fn smoothing_drops_a_single_jump_and_holds_small_moves() {
        assert_eq!(smooth(&[0.3, 0.3, 0.9, 0.3, 0.3], 0.04), vec![0.3; 5]);
        assert_eq!(
            smooth(&[0.5, 0.52, 0.51, 0.7, 0.72], 0.04),
            vec![0.52, 0.52, 0.52, 0.7, 0.7]
        );
    }

    #[test]
    fn a_long_path_is_a_shallow_expression() {
        let n = 400;
        let ts: Vec<f64> = (0..n).map(|i| i as f64 * 0.5).collect();
        let cs: Vec<f64> = (0..n).map(|i| (i % 7) as f64 / 10.0).collect();
        let e = lerp_expr(&ts, &cs);
        let (mut depth, mut most) = (0, 0);
        for c in e.chars() {
            if c == '(' {
                depth += 1;
                most = most.max(depth);
            } else if c == ')' {
                depth -= 1;
            }
        }
        // 9 levels of `if` for 400 points, plus the parentheses of one segment.
        assert!(most < 25, "{most}");
    }

    #[test]
    fn expression_holds_at_both_ends_and_interpolates() {
        assert_eq!(lerp_expr(&[1.0, 2.0], &[0.5, 0.5]), "0.5000");
        let e = lerp_expr(&[1.0, 2.0, 3.0], &[0.2, 0.4, 0.8]);
        assert!(
            e.starts_with("if(lt(t,2.000),(0.2000+(0.2000)*clip((t-1.000)/1.000,0,1))"),
            "{e}"
        );
        assert!(
            e.contains("(0.4000+(0.4000)*clip((t-2.000)/1.000,0,1))"),
            "{e}"
        );
    }

    #[test]
    fn samples_per_shot_within_bounds() {
        assert_eq!(sample_plan(&[(0.0, 5.0)]).len(), 10);
        assert_eq!(sample_plan(&[(0.0, 0.3)]).len(), 1);
        assert!(sample_plan(&[(0.0, 600.0)]).len() <= MAX_SAMPLES + 1);
        // Every shot gets a sample, inside it.
        let p = sample_plan(&[(0.0, 2.0), (2.0, 2.3), (2.3, 4.0)]);
        for i in 0..3 {
            assert!(p.iter().any(|s| s.0 == i), "{p:?}");
        }
        assert!(p
            .iter()
            .all(|&(i, t)| [(0.0, 2.0), (2.0, 2.3), (2.3, 4.0)][i].0 < t));
    }

    #[test]
    fn path_jumps_at_a_cut_and_does_not_smooth_across_it() {
        let shots = [(0.0, 2.0), (2.0, 4.0)];
        let plan = sample_plan(&shots);
        let c: Vec<_> = plan
            .iter()
            .map(|&(i, _)| Some(if i == 0 { (0.2, 0.5) } else { (0.8, 0.5) }))
            .collect();
        let (ts, xs, _) = path(&shots, &plan, &c, 0.04);
        assert!(ts.windows(2).all(|w| w[0] < w[1]), "{ts:?}");
        let k = ts.iter().position(|&t| t >= 2.0).unwrap();
        assert!((ts[k - 1] - 1.998).abs() < 1e-9 && ts[k] == 2.0);
        assert_eq!((xs[k - 1], xs[k]), (0.2, 0.8));
        // Only two values: the first shot never leans toward the second.
        assert!(xs.iter().all(|&x| x == 0.2 || x == 0.8));
    }
}
