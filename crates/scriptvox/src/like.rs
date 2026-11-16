//! `scriptvox analyze <video> --like brief.json`: a starter brief in the style of a reference video. Frame shape, frame
//! rate, pace and scene breaks are measured; the look, the caption style and one shot prompt per scene come from one
//! request to the free vision row, when it answers. The narration holds the reference's words only as a guide: the
//! agent writes its own. Idea from OpenMontage's reference-video analysis; our own design and code.

use serde_json::{json, Value};

const MAX_SCENES: usize = 12;

/// One sentence of the reference's narration.
pub struct Said {
    /// The middle of the sentence, not its start: speech can lead the picture, so a start falls in the shot before
    /// (5 of 6 sentences in the test reference did, and each scene got the words of the next).
    pub at: f64,
    pub text: String,
}

/// What the vision row saw.
#[derive(Default)]
pub struct Seen {
    pub look: Option<String>,
    pub captions: Option<String>,
    pub prompts: Vec<String>,
}

/// The nearest frame shape ScriptVox makes.
pub fn aspect(w: u64, h: u64) -> &'static str {
    let r = w as f64 / h.max(1) as f64;
    let shapes = [
        ("9:16", 9.0 / 16.0),
        ("4:5", 0.8),
        ("3:4", 0.75),
        ("4:3", 4.0 / 3.0),
        ("2:3", 2.0 / 3.0),
        ("21:9", 21.0 / 9.0),
        ("1:1", 1.0),
        ("16:9", 16.0 / 9.0),
    ];
    shapes
        .iter()
        .min_by(|a, b| {
            let d = |x: f64| (x / r).ln().abs();
            d(a.1).total_cmp(&d(b.1))
        })
        .unwrap()
        .0
}

/// The nearest frame rate a brief accepts.
pub fn fps(real: f64) -> u32 {
    [24u32, 25, 30, 60]
        .into_iter()
        .min_by(|a, b| {
            (f64::from(*a) - real)
                .abs()
                .total_cmp(&(f64::from(*b) - real).abs())
        })
        .unwrap()
}

/// Scene spans: shots merged so that each holds a sentence that starts inside it (when there is speech), at most
/// `MAX_SCENES`. A fast-cut reel has many shots per sentence; scriptvox cuts on scenes, so they merge.
pub fn scenes(shots: &[(f64, f64)], said: &[Said]) -> Vec<(f64, f64)> {
    let holds = |a: f64, b: f64| said.iter().any(|s| s.at >= a && s.at < b);
    let mut out: Vec<(f64, f64)> = Vec::new();
    for &(a, b) in shots {
        // A new scene starts only on a shot where a sentence starts, and only when the last scene has its own.
        match out.last_mut() {
            Some(l) if !said.is_empty() && !(holds(a, b) && holds(l.0, l.1)) => l.1 = b,
            _ => out.push((a, b)),
        }
    }
    while out.len() > MAX_SCENES {
        // The shortest neighbours merge first, so the pace of the rest stays.
        let i = (0..out.len() - 1)
            .min_by(|&i, &j| (out[i + 1].1 - out[i].0).total_cmp(&(out[j + 1].1 - out[j].0)))
            .unwrap();
        out[i].1 = out[i + 1].1;
        out.remove(i + 1);
    }
    out
}

/// The starter brief.
pub fn brief(
    (w, h): (u64, u64),
    real_fps: f64,
    spans: &[(f64, f64)],
    said: &[Said],
    seen: &Seen,
) -> Value {
    let list: Vec<Value> = spans
        .iter()
        .enumerate()
        .map(|(i, &(a, b))| {
            let words: Vec<&str> = said
                .iter()
                .filter(|s| s.at >= a && s.at < b)
                .map(|s| s.text.as_str())
                .collect();
            let script = if words.is_empty() {
                // 2.5 words a second is an easy reading pace.
                format!(
                    "Write about {} words for this scene.",
                    ((b - a) * 2.5).round().max(3.0)
                )
            } else {
                words.join(" ")
            };
            let mut sc = json!({"script": script});
            if let Some(p) = seen.prompts.get(i).filter(|p| !p.is_empty()) {
                sc["generate"] = json!({"prompt": p});
            }
            sc
        })
        .collect();
    let total: f64 = spans.last().map_or(0.0, |l| l.1);
    let mean = total / spans.len().max(1) as f64;
    let mut v = json!({
        "aspect": aspect(w, h),
        "fps": fps(real_fps),
        "duration": (total * 10.0).round() / 10.0,
        "transition": if mean < 3.0 { "cut" } else { "fade" },
        "scenes": list,
    });
    if let Some(l) = seen.look.as_deref().filter(|l| *l != "natural") {
        v["look"] = json!(l);
    }
    if let Some(c) = seen
        .captions
        .as_deref()
        .filter(|c| *c == "pop" || *c == "karaoke")
    {
        v["captions"] = json!(c);
    }
    v
}

/// The vision request for one frame per scene, and its reading.
pub fn request(model: &str, frames: &[Vec<u8>]) -> Value {
    use base64::Engine as _;
    let mut content = vec![];
    for (i, jpg) in frames.iter().enumerate() {
        content.push(json!({"type": "text", "text": format!("Scene {}:", i + 1)}));
        content.push(json!({"type": "image_url", "image_url": {"url": format!(
            "data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpg))}}));
    }
    content.push(json!({"type": "text", "text": format!(
        "These {} frames come from one video, one per scene, in time order. We want a new video with the same style \
         and new content. Reply with JSON. `look`: the colour grade, one of natural, warm, cool, film, punchy, mono. \
         `captions`: `pop` for big captions of 1 to 3 words, `karaoke` for one or two lines of smaller text, `none` \
         when no captions show. `prompts`: for each scene a video prompt of 15 to 30 words for the same kind of shot: \
         the subject type, action, framing, camera move, light and style. Describe only what the frame shows: add \
         nothing that is not in it, and do not change a road, a plant or a building into another kind. Name no real \
         person, brand or place, and quote no text from the frame.",
        frames.len()
    )}));
    json!({
        "model": model,
        "temperature": 0,
        "max_tokens": 200 + 80 * frames.len(),
        "messages": [{"role": "user", "content": content}],
        "response_format": {"type": "json_schema", "json_schema": {"name": "style", "schema": {
            "type": "object",
            "properties": {
                "look": {"type": "string", "enum": ["natural", "warm", "cool", "film", "punchy", "mono"]},
                "captions": {"type": "string", "enum": ["pop", "karaoke", "none"]},
                "prompts": {"type": "array", "items": {"type": "string"}, "minItems": frames.len(), "maxItems": frames.len()}
            },
            "required": ["look", "captions", "prompts"]
        }}},
        "chat_template_kwargs": {"enable_thinking": false}
    })
}

pub fn parse_seen(text: &str) -> Result<Seen, String> {
    let (a, b) = (text.find('{'), text.rfind('}'));
    let body = match (a, b) {
        (Some(a), Some(b)) if a < b => &text[a..=b],
        _ => return Err("the reply holds no JSON object".into()),
    };
    let v: Value = serde_json::from_str(body).map_err(|e| format!("the reply is not JSON: {e}"))?;
    let word = |k: &str| v[k].as_str().map(str::to_string);
    Ok(Seen {
        look: word("look"),
        captions: word("captions"),
        prompts: v["prompts"]
            .as_array()
            .map(|p| {
                p.iter()
                    .map(|x| x.as_str().unwrap_or("").to_string())
                    .collect()
            })
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(at: f64) -> Said {
        Said {
            at,
            text: format!("Said at {at}."),
        }
    }

    #[test]
    fn a_sentence_goes_to_the_shot_that_holds_its_middle() {
        // Times from the test reference: 6 shots, 6 sentences, each starting up to 0.5 s before its cut.
        let shots = [
            (0.0, 2.5),
            (2.5, 5.5),
            (5.5, 8.5),
            (8.5, 10.75),
            (10.75, 13.75),
            (13.75, 17.28),
        ];
        let mids = [1.0, 3.47, 6.42, 9.09, 11.76, 14.78].map(said);
        assert_eq!(scenes(&shots, &mids).len(), 6);
    }

    #[test]
    fn shape_and_rate_snap_to_what_a_brief_takes() {
        assert_eq!(aspect(720, 1280), "9:16");
        assert_eq!(aspect(1920, 1080), "16:9");
        assert_eq!(aspect(1080, 1350), "4:5");
        assert_eq!(aspect(1000, 1000), "1:1");
        assert_eq!(aspect(2560, 1080), "21:9");
        assert_eq!(aspect(1440, 1080), "4:3");
        assert_eq!(fps(29.97), 30);
        assert_eq!(fps(23.976), 24);
        assert_eq!(fps(59.94), 60);
    }

    #[test]
    fn fast_cuts_merge_until_each_scene_holds_a_sentence() {
        let shots: Vec<_> = (0..8).map(|i| (i as f64, i as f64 + 1.0)).collect();
        let s = scenes(&shots, &[said(0.1), said(3.5), said(6.2)]);
        assert_eq!(s, vec![(0.0, 3.0), (3.0, 6.0), (6.0, 8.0)]);
        // No speech: one scene per shot, at most 12, and the end stays the end.
        let many: Vec<_> = (0..30).map(|i| (i as f64, i as f64 + 1.0)).collect();
        let s = scenes(&many, &[]);
        assert_eq!(s.len(), MAX_SCENES);
        assert_eq!((s[0].0, s[MAX_SCENES - 1].1), (0.0, 30.0));
    }

    #[test]
    fn the_starter_is_a_brief_scriptvox_accepts() {
        let spans = [(0.0, 4.0), (4.0, 9.0)];
        let seen = parse_seen(
            "```json\n{\"look\":\"film\",\"captions\":\"pop\",\"prompts\":[\"a cook stirs a pot\",\"\"]}\n```",
        )
        .unwrap();
        let v = brief((1080, 1920), 29.97, &spans, &[said(0.5)], &seen);
        assert_eq!(v["look"], "film");
        assert_eq!(v["captions"], "pop");
        assert_eq!(v["scenes"][0]["script"], "Said at 0.5.");
        assert_eq!(v["scenes"][0]["generate"]["prompt"], "a cook stirs a pot");
        assert!(v["scenes"][1]["generate"].is_null());
        assert!(v["scenes"][1]["script"]
            .as_str()
            .unwrap()
            .contains("13 words"));
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("b.json");
        std::fs::write(&p, v.to_string()).unwrap();
        crate::brief::Brief::load(&p).unwrap();
        // Without a vision answer it still works.
        let plain = brief((1920, 1080), 25.0, &spans, &[], &Seen::default());
        assert!(plain["look"].is_null());
        std::fs::write(&p, plain.to_string()).unwrap();
        crate::brief::Brief::load(&p).unwrap();
    }
}
