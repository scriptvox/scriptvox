//! Quality gates (PLAN-R1 §5): one FFmpeg analysis pass over every frame and
//! every sample (OM #358 sampled only 4 frames). ScriptVox's own rubric.

use crate::engine_ffmpeg::Ffmpeg;
use serde::Serialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Finding {
    pub gate: &'static str,
    /// pass, warn or fail.
    pub status: &'static str,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// One word for comparison: lower case, letters and digits only, Arabic-Indic digits as ASCII, Arabic diacritics and
/// tatweel removed, alef forms as bare alef, final ta marbuta as ha, alef maqsura as ya. A number word up to twenty,
/// or a tens word, becomes its digits: the recogniser writes "two" where the script has "2" (4 Nov 2026).
pub fn norm_word(w: &str) -> String {
    // Single words only here; numbers of 21 or more are compared as their words in `heard_counts`.
    const NUMBERS: &[(&str, &str)] = &[
        ("zero", "0"),
        ("one", "1"),
        ("two", "2"),
        ("three", "3"),
        ("four", "4"),
        ("five", "5"),
        ("six", "6"),
        ("seven", "7"),
        ("eight", "8"),
        ("nine", "9"),
        ("ten", "10"),
        ("eleven", "11"),
        ("twelve", "12"),
        ("thirteen", "13"),
        ("fourteen", "14"),
        ("fifteen", "15"),
        ("sixteen", "16"),
        ("seventeen", "17"),
        ("eighteen", "18"),
        ("nineteen", "19"),
        ("twenty", "20"),
        ("thirty", "30"),
        ("forty", "40"),
        ("fifty", "50"),
        ("sixty", "60"),
        ("seventy", "70"),
        ("eighty", "80"),
        ("ninety", "90"),
        ("hundred", "100"),
        // Arabic, after the letter folding above.
        ("واحد", "1"),
        ("اثنان", "2"),
        ("اثنين", "2"),
        ("ثلاثه", "3"),
        ("اربعه", "4"),
        ("خمسه", "5"),
        ("سته", "6"),
        ("سبعه", "7"),
        ("ثمانيه", "8"),
        ("تسعه", "9"),
        ("عشره", "10"),
    ];
    let n = norm_letters(w);
    NUMBERS
        .iter()
        .find(|(word, _)| *word == n)
        .map_or(n, |(_, d)| d.to_string())
}

fn norm_letters(w: &str) -> String {
    w.chars()
        .filter_map(|c| match c {
            '\u{064B}'..='\u{065F}' | '\u{0670}' | '\u{0640}' => None,
            '\u{0622}' | '\u{0623}' | '\u{0625}' | '\u{0671}' => Some('\u{0627}'),
            '\u{0629}' => Some('\u{0647}'),
            '\u{0649}' => Some('\u{064A}'),
            '\u{0660}'..='\u{0669}' => char::from_digit(c as u32 - 0x0660, 10),
            '\u{06F0}'..='\u{06F9}' => char::from_digit(c as u32 - 0x06F0, 10),
            c if c.is_alphanumeric() => Some(c.to_lowercase().next().unwrap_or(c)),
            _ => None,
        })
        .collect()
}

/// An English cardinal in words: 2005 is "two thousand five", 342 is "three hundred forty two".
pub fn cardinal_words(n: u64) -> Vec<String> {
    const ONES: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 10] = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    fn under_1000(n: u64, out: &mut Vec<String>) {
        if n >= 100 {
            out.push(ONES[(n / 100) as usize].into());
            out.push("hundred".into());
        }
        let r = n % 100;
        if r >= 20 {
            out.push(TENS[(r / 10) as usize].into());
            if !r.is_multiple_of(10) {
                out.push(ONES[(r % 10) as usize].into());
            }
        } else if r > 0 || n == 0 {
            out.push(ONES[r as usize].into());
        }
    }
    let mut out = Vec::new();
    let mut rest = n;
    for (size, name) in [
        (1_000_000_000, "billion"),
        (1_000_000, "million"),
        (1_000, "thousand"),
    ] {
        if rest >= size {
            under_1000(rest / size, &mut out);
            out.push(name.into());
            rest %= size;
        }
    }
    if rest > 0 || out.is_empty() {
        under_1000(rest, &mut out);
    }
    out
}

/// How a year is said, when it is not said as a cardinal: 1990 "nineteen ninety", 2025 "twenty twenty five",
/// 1905 "nineteen oh five". None outside 1100 to 2099 and for 2000 to 2009 (said "two thousand five").
fn year_words(n: u64) -> Option<Vec<String>> {
    if !(1100..=2099).contains(&n) || (2000..=2009).contains(&n) {
        return None;
    }
    let (hi, lo) = (n / 100, n % 100);
    let mut out = cardinal_words(hi);
    match lo {
        0 => out.push("hundred".into()),
        1..=9 => {
            out.push("oh".into());
            out.extend(cardinal_words(lo));
        }
        _ => out.extend(cardinal_words(lo)),
    }
    Some(out)
}

/// (script words heard in order, script words, words in the audio), over normalised words.
pub fn heard_counts(script: &str, heard: &str) -> (usize, usize, usize) {
    // A decimal is read as its parts: the recogniser writes "1.4" where the script says "one point four" (4 Nov
    // 2026: three decimals made a clean read fail at 22 of 31). A number of 21 or more is compared as its words, so
    // "two thousand five" in the script matches "2005" heard, and the other way round; a year can also be said
    // "nineteen ninety", and the reading that matches the other side's words is taken.
    let split = |t: &str, other: &std::collections::HashSet<String>| -> Vec<String> {
        let words = |ws: Vec<String>| -> Vec<String> { ws.iter().map(|w| norm_word(w)).collect() };
        t.split_whitespace()
            .flat_map(|tok| {
                let core = tok.trim_matches(|c: char| !c.is_alphanumeric());
                let digits = core.replace(',', "");
                if let Some((i, f)) = core.split_once('.').filter(|(i, f)| {
                    !i.is_empty()
                        && !f.is_empty()
                        && i.chars().all(|c| c.is_ascii_digit())
                        && f.chars().all(|c| c.is_ascii_digit())
                }) {
                    let mut v = vec![i.to_string(), "point".to_string()];
                    v.extend(f.chars().map(|c| c.to_string()));
                    return v;
                }
                if !digits.is_empty()
                    && digits.len() <= 12
                    && digits.chars().all(|c| c.is_ascii_digit())
                {
                    if let Ok(n) = digits.parse::<u64>() {
                        if n >= 21 {
                            let card = words(cardinal_words(n));
                            let score =
                                |v: &Vec<String>| v.iter().filter(|w| other.contains(*w)).count();
                            return match year_words(n).map(words) {
                                Some(y) if score(&y) > score(&card) => y,
                                _ => card,
                            };
                        }
                    }
                }
                // "twenty-five" is two words, as the recogniser writes them.
                core.split('-').map(norm_word).collect()
            })
            .filter(|w| !w.is_empty())
            .collect()
    };
    let plain = |t: &str| -> std::collections::HashSet<String> {
        t.split_whitespace()
            .flat_map(|w| w.split('-').map(norm_word).collect::<Vec<_>>())
            .collect()
    };
    let script = crate::pauses::strip(script);
    let a = split(&script, &plain(heard));
    let b = split(heard, &plain(&script));
    // ponytail: O(n*m) table; fine for a few thousand words, a banded alignment if scripts get longer.
    let mut row = vec![0usize; b.len() + 1];
    for x in &a {
        let mut diag = 0;
        for (j, y) in b.iter().enumerate() {
            let up = row[j + 1];
            row[j + 1] = if x == y { diag + 1 } else { up.max(row[j]) };
            diag = up;
        }
    }
    (row[b.len()], a.len(), b.len())
}

/// Share of the script words heard in order (1.0 for an empty script).
pub fn heard_share(script: &str, heard: &str) -> f64 {
    let (hit, n, _) = heard_counts(script, heard);
    if n == 0 {
        1.0
    } else {
        hit as f64 / n as f64
    }
}

/// Gate `captions`: every caption page is on screen long enough to read and no short page outstays its words. A page
/// under 0.2 s, or one or two words held over 2.5 s, means the captions lost step with the voice (4 Nov 2026: a
/// merged TTS word event left "one" on screen for 3.8 s and the last page with no time). Two pages on screen at one
/// time are a failure. The second language line (`captions_second`) is another track and is not in `pages`.
pub fn caption_pages(pages: &[crate::captions::Page]) -> Finding {
    // Two pages of this track at one time: the renderer draws the second above the first, where it can cover a card.
    if let Some(w) = pages.windows(2).find(|w| w[0].end > w[1].start + 1e-6) {
        return Finding {
            gate: "captions",
            status: "fail",
            detail: format!(
                "two caption pages are on screen together: one ends at {:.2} s, the next starts at {:.2} s",
                w[0].end, w[1].start
            ),
            fix: Some("The word times of the voice are not in order: a fault of the tool, not of the brief. Make the video with another `tts`, and report this run folder.".into()),
        };
    }
    let n_words = |p: &crate::captions::Page| p.lines.iter().map(|l| l.len()).sum::<usize>();
    // Spoken time (`said_end`), not screen time: a page held through a pause is not out of step.
    let short = pages.iter().find(|p| p.said_end - p.start < 0.2);
    let held = pages
        .iter()
        .find(|p| n_words(p) <= 2 && p.said_end - p.start > 2.5);
    let bad = short
        .map(|p| format!("a page has {:.2} s at {:.1} s", p.end - p.start, p.start))
        .or_else(|| {
            held.map(|p| {
                format!(
                    "{} word(s) stay {:.1} s at {:.1} s",
                    n_words(p),
                    p.said_end - p.start,
                    p.start
                )
            })
        });
    match bad {
        Some(d) => Finding {
            gate: "captions",
            status: "warn",
            detail: format!("captions may be out of step with the voice: {d}"),
            fix: Some("Look at a snapshot at that time. Write numbers and symbols as the voice says them, then resume.".into()),
        },
        None => Finding {
            gate: "captions",
            status: "pass",
            detail: format!("{} pages, each readable", pages.len()),
            fix: None,
        },
    }
}

/// A gap from the edge of the frame under this share of the side is a warning. See docs/specs/text-bounds.md.
const TEXT_MARGIN: f32 = 0.02;

/// The value after the last `\\name` in an override block, up to the next `\\` or `)`. The last one is where text
/// rests: `\\fscx80\\t(0,180,\\fscx100)` ends at 100.
fn ass_tag<'a>(tags: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("\\{name}");
    tags.match_indices(key.as_str())
        .map(|(i, _)| &tags[i + key.len()..])
        // `\\fs` must not read `\\fscx`.
        .filter(|v| {
            name == "fn"
                || name.ends_with('(')
                || v.starts_with(|c: char| c.is_ascii_digit() || c == '.')
        })
        .last()
        .map(|v| v.split(['\\', ')']).next().unwrap_or(""))
}

fn ass_seconds(t: &str) -> f64 {
    t.split(':')
        .fold(0.0, |sum, p| sum * 60.0 + p.parse::<f64>().unwrap_or(0.0))
}

/// Every text box in an ASS document against its frame, measured with the same shaper that wrapped it (`Measurer`).
/// `starts`: the time each scene starts. Drawings have no text and are left out. None when there is no text.
pub fn text_bounds(ass: &str, (w, h): (u32, u32), starts: &[f64]) -> Option<Finding> {
    use std::collections::HashMap;
    let (fw, fh) = (w as f32, h as f32);
    let styles: HashMap<&str, Vec<&str>> = ass
        .lines()
        .filter_map(|l| l.strip_prefix("Style: "))
        .filter_map(|l| {
            let v: Vec<&str> = l.split(',').collect();
            (v.len() > 21).then(|| (v[0], v))
        })
        .collect();
    let mut meters: HashMap<(String, u32), Option<crate::captions::Measurer>> = HashMap::new();
    let (mut boxes, mut unmeasured) = (0usize, 0usize);
    // (how far past, scene, words)
    let mut out: Vec<(f32, String)> = vec![];
    let mut tight: Vec<(f32, String)> = vec![];
    // Every measured box with its times, to find a card's text that lies over a caption:
    // (is a caption, left, top, right, bottom, start, end, scene, words).
    type Rect = (bool, f32, f32, f32, f32, f64, f64, Option<usize>, String);
    let mut rects: Vec<Rect> = vec![];
    for l in ass.lines().filter_map(|l| l.strip_prefix("Dialogue: ")) {
        let f: Vec<&str> = l.splitn(10, ',').collect();
        let (Some(text), Some(st)) = (f.get(9), f.get(3).and_then(|s| styles.get(s))) else {
            continue;
        };
        let tags = text
            .strip_prefix('{')
            .and_then(|t| t.split_once('}'))
            .map_or("", |t| t.0);
        if text.contains("\\p1") {
            continue;
        }
        let mut body = String::new();
        let mut rest = *text;
        while let Some((a, b)) = rest.split_once('{') {
            body.push_str(a);
            rest = b.split_once('}').map_or("", |x| x.1);
        }
        body.push_str(rest);
        let body = body.replace("\\h", " ");
        if body.trim().is_empty() {
            continue;
        }
        boxes += 1;
        let num = |s: Option<&str>, d: f32| s.and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
        let size = num(ass_tag(tags, "fs"), num(st.get(2).copied(), 0.0));
        let (sx, sy) = (
            num(ass_tag(tags, "fscx"), 100.0) / 100.0,
            num(ass_tag(tags, "fscy"), 100.0) / 100.0,
        );
        let family = ass_tag(tags, "fn").unwrap_or(st[1]);
        let meter = meters
            .entry((family.to_string(), size.round() as u32))
            .or_insert_with(|| {
                crate::template::font_bytes(family)
                    .and_then(|b| crate::captions::Measurer::new(b.to_vec(), size))
            });
        let Some(meter) = meter else {
            unmeasured += 1;
            continue;
        };
        let widths: Vec<f32> = body
            .split("\\N")
            .map(|t| meter.width(t.trim_start_matches(['\u{200E}', '\u{200F}'])))
            .collect();
        let wide = widths.iter().copied().fold(0.0, f32::max) * sx;
        if wide <= 0.0 {
            unmeasured += 1;
            continue;
        }
        // ponytail: the math font's win metrics are several lines tall, so a glyph's box says nothing of its ink.
        // A formula is checked for width only. Its height is set by the card's own limits.
        let high = if family == crate::formula::FAMILY {
            0.0
        } else {
            widths.len() as f32 * size * sy
        };
        let an = num(ass_tag(tags, "an"), num(st.get(18).copied(), 2.0)) as u32;
        let pair = |v: &str, a: usize| {
            v.split(',')
                .nth(a)
                .and_then(|x| x.trim().parse::<f32>().ok())
        };
        let at = ass_tag(tags, "move(")
            .and_then(|v| Some((pair(v, 2)?, pair(v, 3)?)))
            .or_else(|| ass_tag(tags, "pos(").and_then(|v| Some((pair(v, 0)?, pair(v, 1)?))));
        let (col, row) = ((an.max(1) - 1) % 3, (an.max(1) - 1) / 3);
        let (x, y) = at.unwrap_or_else(|| {
            // A dialogue margin of 0 takes the style's.
            let m = |d: usize, s: usize| {
                Some(num(f.get(d).copied(), 0.0))
                    .filter(|v| *v != 0.0)
                    .unwrap_or_else(|| num(st.get(s).copied(), 0.0))
            };
            let (l, r, v) = (m(5, 19), m(6, 20), m(7, 21));
            (
                [l, (l + fw - r) / 2.0, fw - r][col as usize],
                [fh - v, fh / 2.0, v][row as usize],
            )
        });
        let left = x - wide * [0.0, 0.5, 1.0][col as usize];
        let top = y - high * [1.0, 0.5, 0.0][row as usize];
        let (right, bottom) = (left + wide, top + high);
        // A cut in pixels on each side, largest first. One pixel is rounding.
        let past = [
            (-left, "left"),
            (right - fw, "right"),
            (-top, "top"),
            (bottom - fh, "bottom"),
        ]
        .into_iter()
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap();
        let start = ass_seconds(f[1]);
        // No scene times: no scene to name.
        let scene = (!starts.is_empty())
            .then(|| starts.iter().rposition(|s| *s <= start + 1e-3).unwrap_or(0) + 1);
        let shown: String = body
            .replace("\\N", " ")
            .chars()
            .filter(|c| !matches!(c, '\u{200E}' | '\u{200F}'))
            .take(32)
            .collect();
        let what = |d: &str| match scene {
            Some(n) => format!("scene {n}: \"{shown}\" {d}"),
            None => format!("\"{shown}\" {d}"),
        };
        // A formula has no measured height (see above), so it takes no part in the overlap check.
        if high > 0.0 && (f[3] == "Cap" || f[3] == "Card") {
            rects.push((
                f[3] == "Cap",
                left,
                top,
                right,
                bottom,
                start,
                ass_seconds(f[2]),
                scene,
                shown.clone(),
            ));
        }
        if past.0 > 1.0 {
            out.push((
                past.0,
                what(&format!(
                    "runs {:.0} px past the {} of the {w}x{h} frame",
                    past.0, past.1
                )),
            ));
        } else {
            let gap = [
                (left, "left", fw),
                (fw - right, "right", fw),
                (top, "top", fh),
                (fh - bottom, "bottom", fh),
            ]
            .into_iter()
            .find(|(g, _, side)| *g < side * TEXT_MARGIN - 1.0);
            if let Some((g, side, _)) = gap {
                tight.push((-g, what(&format!("is {g:.0} px from the {side} edge"))));
            }
        }
    }
    if boxes == 0 {
        return None;
    }
    // A card's text over a caption: both on screen for 0.1 s or more, and the two text boxes share pixels. One entry
    // per card text and scene, with the deepest overlap. The boxes are the text itself, without the padding of its
    // background, so any overlap here is letters over letters.
    let mut over: Vec<(f32, String)> = vec![];
    for c in rects.iter().filter(|r| !r.0) {
        let deepest = rects
            .iter()
            .filter(|k| k.0 && k.5.max(c.5) + 0.1 <= k.6.min(c.6))
            .filter(|k| k.1.max(c.1) < k.3.min(c.3))
            .map(|k| k.4.min(c.4) - k.2.max(c.2))
            .fold(0.0f32, f32::max);
        if deepest > 2.0 {
            let what = match c.7 {
                Some(n) => format!(
                    "scene {n}: \"{}\" lies {deepest:.0} px over the captions",
                    c.8
                ),
                None => format!("\"{}\" lies {deepest:.0} px over the captions", c.8),
            };
            match over
                .iter_mut()
                .find(|o| o.1.split(" lies ").next() == what.split(" lies ").next())
            {
                Some(o) if o.0 < deepest => *o = (deepest, what),
                Some(_) => {}
                None => over.push((deepest, what)),
            }
        }
    }
    let not_measured = if unmeasured > 0 {
        format!(" {unmeasured} not measured (no font or no shaping).")
    } else {
        String::new()
    };
    let say = |mut v: Vec<(f32, String)>| {
        v.sort_by(|a, b| b.0.total_cmp(&a.0));
        let more = v.len().saturating_sub(3);
        let mut d: Vec<String> = v.into_iter().take(3).map(|x| x.1).collect();
        if more > 0 {
            d.push(format!("{more} more"));
        }
        d.join("; ")
    };
    Some(if !out.is_empty() {
        f(
            "text_bounds",
            "fail",
            format!(
                "{} of {boxes} text boxes leave the frame: {}.{not_measured}",
                out.len(),
                say(out)
            ),
            Some("Shorten that text, or use shorter words, then resume."),
        )
    } else if !over.is_empty() {
        f(
            "text_bounds",
            "warn",
            format!(
                "{} card texts lie over the captions: {}.{not_measured}",
                over.len(),
                say(over)
            ),
            Some("Shorten the card's text or its label, use `caption_at` to move the captions, or a smaller `caption_size`. Then look at a snapshot at that scene."),
        )
    } else if unmeasured == boxes || !tight.is_empty() {
        let d = if tight.is_empty() {
            format!("no text box was measured.{not_measured}")
        } else {
            format!(
                "{} of {boxes} text boxes are close to the edge: {}.{not_measured}",
                tight.len(),
                say(tight)
            )
        };
        f(
            "text_bounds",
            "warn",
            d,
            Some("Look at a snapshot at that scene."),
        )
    } else {
        f(
            "text_bounds",
            "pass",
            format!("{boxes} text boxes inside the frame.{not_measured}"),
            None,
        )
    })
}

/// The script with each sentence kept once, in order.
fn without_repeats(script: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    script
        .split_inclusive(['.', '!', '?', '\u{061F}'])
        .map(str::trim)
        .filter(|t| !t.is_empty() && seen.insert(t.to_lowercase()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Gate `words`: how many script words were heard in the final audio, in order (longest common subsequence over
/// normalised words). Pass from 90 percent, warn from 75. Catches a dropped or garbled voice and a bed that drowns it.
fn gate_ran(findings: &[serde_json::Value], gate: &str) -> bool {
    findings.iter().any(|f| {
        f["gate"] == gate
            && !f["detail"]
                .as_str()
                .is_some_and(|d| d.starts_with("not checked"))
    })
}

/// How to get gate `words` here, for `not_checked`: `check --hear` when a speech row can run (the local one also
/// under `--local-only`), else the one command that installs the free local row first (13 Nov 2026: the line said
/// only that the speech row was stopped, and an agent could not tell what to do).
pub fn hear_how() -> &'static str {
    if crate::opentts::stt_row(false).is_some() {
        // With the flag the local row hears it; without it a service row goes first.
        if crate::plan::local_only() {
            "run `scriptvox check <run> --hear --local-only`"
        } else {
            "run `scriptvox check <run> --hear`"
        }
    } else {
        "no speech to text here: run `scriptvox setup speech-local` (free, on this machine), then `scriptvox check <run> --hear`"
    }
}

/// What a result did not check, for field `not_checked`, so an agent cannot report a pass it did not get. `want` is the
/// gates this command could have run, each with the reason it did not. A `words` finding that says "not checked" counts
/// as not run.
pub fn not_checked(findings: &[serde_json::Value], want: &[(&str, &str)]) -> Vec<String> {
    let ran = |gate: &str| gate_ran(findings, gate);
    let mut out: Vec<String> = want
        .iter()
        .filter(|(g, _)| !ran(g))
        .map(|(g, why)| format!("{g}: {why}"))
        .collect();
    out.push(if crate::judge::vision_url().is_some() {
        "judge: frames are not scored here; run `scriptvox judge`".into()
    } else {
        "judge: frames are not scored here; no vision service is set (SCRIPTVOX_VISION_URL)".into()
    });
    out.push(
        "person: no person watched or listened; gates measure, they do not see or hear".into(),
    );
    out
}

/// What each gate looks at and what a pass from it cannot show. A gate with no row reports only its own finding.
const GATE_LIMITS: [(&str, &str, &str); 13] = [
    (
        "black",
        "every frame",
        "only stretches of 0.5 s or more count; a dark scene can read as black",
    ),
    (
        "first_frame",
        "the first frame",
        "only black counts; a white, blurred or empty first frame passes",
    ),
    (
        "flashes",
        "every frame",
        "3 hard changes in 1 s count, fast cuts too; a slow pulse or a flash in a small part of the frame passes",
    ),
    (
        "slideshow",
        "every frame",
        "only stills of 1.5 s or more count; it does not say the motion looks good",
    ),
    (
        "silence",
        "the whole sound",
        "only gaps of 2 s or more below -50 dB count; it does not say the sound is clear",
    ),
    (
        "loudness",
        "the whole sound",
        "one value for the file; one quiet or loud part can stay inside it",
    ),
    (
        "av_drift",
        "the lengths of the streams",
        "it compares the 2 lengths, so a mouth out of step inside the file is not seen",
    ),
    (
        "words",
        "the sound, heard by a speech model",
        "the model can mishear; a share of words does not say the voice sounds right",
    ),
    (
        "captions",
        "the time each caption page has, and that one page is on screen at a time",
        "it does not read the words or look at the picture",
    ),
    (
        "text_bounds",
        "each text box in the burned captions and cards, measured, not seen",
        "it does not look at the picture, so text on a busy background passes",
    ),
    (
        "contrast",
        "up to 8 text boxes of the captions and cards, one frame each: the letters against the pixels around them",
        "other frames and other text boxes are not seen; a title behind the subject is not measured; it does not say the text is easy to read",
    ),
    (
        "loop_seam",
        "the first and the last frame, as small grey pictures",
        "the sound at the seam and the motion across it are not checked",
    ),
    (
        "base_text",
        "3 frames of the base video, at 20, 50 and 80 percent",
        "text on other frames is not seen",
    ),
];

/// For fields `coverage` and `limitations`, from the gates that ran: an agent reports what a pass looked at, and what it
/// cannot show. Same "ran" rule as `not_checked`.
pub fn limits(findings: &[serde_json::Value]) -> (Vec<String>, Vec<String>) {
    let ran = |gate: &str| gate_ran(findings, gate);
    let rows = || GATE_LIMITS.iter().filter(|(g, _, _)| ran(g));
    (
        rows().map(|(g, what, _)| format!("{g}: {what}")).collect(),
        rows().map(|(g, _, why)| format!("{g}: {why}")).collect(),
    )
}

pub fn words_heard(script: &str, heard: &str) -> Finding {
    let (hit, n, got) = heard_counts(script, heard);
    if n == 0 {
        return f("words", "pass", "no script to compare".into(), None);
    }
    // Speech to text often hears a sentence said twice in a row only once (5 Nov 2026: a refrain scored 50 percent and
    // failed the video). A script with repeats also counts with each sentence once; the lost words are named.
    let once = without_repeats(script);
    if once != script && (hit as f64 / n as f64) < 0.9 {
        let (h, m, g) = heard_counts(&once, heard);
        if m > 0 && h as f64 / m as f64 >= 0.9 {
            return f(
                "words",
                "pass",
                format!("heard {h} of {m} words of the script with repeated sentences counted once, {g} words in the audio; speech to text may merge a repeat, so listen to it"),
                None,
            );
        }
    }
    let share = hit as f64 / n as f64;
    // More words than the script needs a reason, or the number reads as a fault in the voice.
    let extra = got.saturating_sub(hit);
    let why = if extra >= 5 && got > n {
        format!("; {extra} heard words are not in the script, often a false repeat from speech to text, so listen before you blame the voice")
    } else {
        String::new()
    };
    let detail = format!(
        "heard {hit} of {n} script words ({:.0} percent), {got} words in the audio{why}",
        share * 100.0
    );
    let fix = "Listen to the parts with missing words: a mispronounced or dropped line, or music too loud under the voice. Make the voice again or lower the bed.";
    match share {
        s if s >= 0.9 => f("words", "pass", detail, None),
        s if s >= 0.75 => f("words", "warn", detail, Some(fix)),
        _ => f("words", "fail", detail, Some(fix)),
    }
}

fn f(gate: &'static str, status: &'static str, detail: String, fix: Option<&str>) -> Finding {
    Finding {
        gate,
        status,
        detail,
        fix: fix.map(String::from),
    }
}

/// Black stretch that fails the video, in seconds.
pub const BLACK_FAIL_S: f64 = 2.0;
/// Frozen share above which the video reads as a slideshow.
pub const FROZEN_FAIL: f64 = 0.5;
pub const FROZEN_WARN: f64 = 0.3;

fn probe_durations(ff: &Ffmpeg, file: &Path) -> (Option<f64>, Option<f64>, f64) {
    let o = Command::new(&ff.ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,duration,r_frame_rate",
            "-of",
            "json",
        ])
        // A refused file gives no input: ffprobe opens nothing and the durations stay unknown.
        .args(crate::media_gate::input(file).unwrap_or_default())
        .output();
    let v: serde_json::Value = o
        .ok()
        .and_then(|o| serde_json::from_slice(&o.stdout).ok())
        .unwrap_or_default();
    let mut vd = None;
    let mut ad = None;
    let mut fps = 30.0;
    for s in v["streams"].as_array().cloned().unwrap_or_default() {
        let d = s["duration"].as_str().and_then(|x| x.parse::<f64>().ok());
        match s["codec_type"].as_str() {
            Some("video") => {
                vd = d;
                if let Some((a, b)) = s["r_frame_rate"].as_str().and_then(|r| r.split_once('/')) {
                    fps =
                        a.parse::<f64>().unwrap_or(30.0) / b.parse::<f64>().unwrap_or(1.0).max(1.0);
                }
            }
            Some("audio") => ad = d,
            _ => {}
        }
    }
    (vd, ad, fps)
}

fn sum_after(text: &str, key: &str) -> (f64, f64) {
    // (total, longest) of every `key: <seconds>` in the log.
    let vals: Vec<f64> = text
        .split(key)
        .skip(1)
        .filter_map(|s| s.split_whitespace().next()?.parse().ok())
        .collect();
    // `+ 0.0`: an empty float sum is -0.0 in Rust.
    (
        vals.iter().sum::<f64>() + 0.0,
        vals.iter().cloned().fold(0.0, f64::max),
    )
}

/// Checks one generated clip before it is used (P2-006): it decodes, it is long enough, its frame matches the asked
/// aspect, it is not mostly black, and it moves. A failed clip was still paid for, so the finding says how to retry.
pub fn clip_check(
    ff: &Ffmpeg,
    file: &Path,
    want_s: f64,
    aspect: &str,
    from_picture: bool,
) -> Vec<Finding> {
    let retry =
        "Change the prompt or the model (a new plan), then run again. The clip stays in the run folder.";
    let rb = match ff.readback(file) {
        Ok(r) if r.frames > 0 => r,
        _ => {
            return vec![f(
                "clip_decode",
                "fail",
                format!("{} does not decode", file.display()),
                Some(retry),
            )]
        }
    };
    let mut out = Vec::new();
    let secs = rb.frames as f64 / rb.fps.max(1.0);
    out.push(if secs + 0.25 < want_s * 0.8 {
        f(
            "clip_length",
            "fail",
            format!("{secs:.1} s, asked {want_s} s"),
            Some(retry),
        )
    } else {
        f("clip_length", "pass", format!("{secs:.1} s"), None)
    });
    let (w, h) = (rb.width as f64, rb.height as f64);
    let want = aspect
        .split_once(':')
        .and_then(|(a, b)| Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?))
        .unwrap_or(w / h.max(1.0));
    out.push(if w.min(h) < 320.0 {
        f(
            "clip_size",
            "fail",
            format!("{w}x{h} is too small"),
            Some(retry),
        )
    } else if ((w / h.max(1.0)) / want - 1.0).abs() > 0.06 {
        f(
            "clip_size",
            "warn",
            format!("{w}x{h} is not {aspect}; scriptvox crops it to fit"),
            None,
        )
    } else {
        f("clip_size", "pass", format!("{w}x{h}"), None)
    });
    let log = Command::new(&ff.ffmpeg)
        .args(["-hide_banner", "-nostdin"])
        .args(crate::media_gate::input(file).unwrap_or_default())
        .args([
            "-an",
            "-vf",
            "blackdetect=d=0.3:pix_th=0.10,freezedetect=n=0.003:d=1",
            "-f",
            "null",
            "-",
        ])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stderr).into_owned())
        .unwrap_or_default();
    let total = secs.max(0.001);
    let (black, _) = sum_after(&log, "black_duration:");
    out.push(if black / total > 0.3 {
        f(
            "clip_black",
            "fail",
            format!("{black:.1} s of {total:.1} s is black"),
            Some(retry),
        )
    } else {
        f("clip_black", "pass", format!("{black:.1} s black"), None)
    });
    let (mut frozen, _) = sum_after(&log, "freeze_duration:");
    let starts: Vec<f64> = log
        .split("freeze_start:")
        .skip(1)
        .filter_map(|s| s.split_whitespace().next()?.parse().ok())
        .collect();
    if starts.len() > log.matches("freeze_duration:").count() {
        frozen += total - starts.last().copied().unwrap_or(total);
    }
    out.push(if frozen / total > FROZEN_FAIL {
        f(
            "clip_motion",
            "fail",
            format!("{:.0}% of the clip does not move", frozen / total * 100.0),
            Some(if from_picture {
                "A flat picture such as a logo may not move on this row. Give it as `media` with `camera` in the brief, or change the picture, the prompt or the model. The clip stays in the run folder."
            } else {
                retry
            }),
        )
    } else {
        f(
            "clip_motion",
            "pass",
            format!("{:.0}% frozen", frozen / total * 100.0),
            None,
        )
    });
    out
}

/// Why FFmpeg's log of a full decode says the file is damaged. `full`: the shortest stream length the index states, in
/// seconds; FFmpeg's `time=` can stop at the shortest stream, and a shorter stream is av_drift's job. A truncated MP4 with
/// its index in front opens, decodes what is left and exits 0, so the end of the decode is compared.
fn damaged(log: &str, full: Option<f64>) -> Option<String> {
    let low = log.to_lowercase();
    let said = [
        "error while decoding",
        "error submitting packet",
        "corrupt decoded frame",
        "partial file",
        "ended prematurely",
    ];
    if let Some(what) = said.iter().find(|w| low.contains(**w)) {
        return Some(format!("FFmpeg reports \"{what}\""));
    }
    let full = full?;
    let got = low
        .rsplit(" time=")
        .next()?
        .split_whitespace()
        .next()?
        .split(':')
        .try_fold(0.0, |t: f64, p| Some(t * 60.0 + p.parse::<f64>().ok()?))
        .filter(|t| *t >= 0.0)
        .unwrap_or(0.0);
    (full - got > 0.5).then(|| format!("the decode stops at {got:.1} s of {full:.1} s"))
}

pub fn check(ff: &Ffmpeg, file: &Path) -> Result<Vec<Finding>, String> {
    check_within(ff, file, None)
}

/// The last `time=` FFmpeg printed, in seconds: how far a read got.
fn read_to(log: &str) -> f64 {
    log.rsplit("time=")
        .next()
        .and_then(|t| t.split_whitespace().next())
        .and_then(|t| {
            t.split(':')
                .try_fold(0.0, |sum: f64, p| Some(sum * 60.0 + p.parse::<f64>().ok()?))
        })
        .filter(|t| *t >= 0.0)
        .unwrap_or(0.0)
}

/// `check`, with a time limit for a long file (`check --budget`, G147). With `budget_s`, FFmpeg is stopped after
/// that many seconds and the gates say what the part that was read shows. A gate that needs the whole file (the
/// loudness of the programme, the decode to the end) says it was not measured, and a `budget` finding names the
/// part that was not read. A part that passes is not a pass for the file.
pub fn check_within(
    ff: &Ffmpeg,
    file: &Path,
    budget_s: Option<f64>,
) -> Result<Vec<Finding>, String> {
    if !file.is_file() {
        return Err(format!("{} does not exist", file.display()));
    }
    if !ff.has_stream(file, "video") {
        return Err(format!(
            "{} has no video stream FFmpeg can read",
            file.display()
        ));
    }
    let (vd, ad, fps) = probe_durations(ff, file);
    let full = vd.unwrap_or(0.0).max(0.001);
    let mut cmd = Command::new(&ff.ffmpeg);
    cmd.args(["-hide_banner", "-nostdin"])
        .args(crate::media_gate::input(file)?)
        .args([
            "-vf",
            // Every black stretch from one frame up (the first frame, G188); `scdet` logs each hard change of the picture.
            "blackdetect=d=0.03:pix_th=0.10,freezedetect=n=0.003:d=1.5,scdet=t=30",
        ])
        .args(["-af", "silencedetect=n=-50dB:d=2,ebur128"])
        .args(["-f", "null", "-"]);
    // (the log, FFmpeg ended by itself and well, it was stopped at the limit)
    let (log, ended_well, stopped) = match budget_s {
        None => {
            let o = cmd
                .output()
                .map_err(|e| format!("cannot start FFmpeg: {e}"))?;
            (
                String::from_utf8_lossy(&o.stderr).into_owned(),
                o.status.success(),
                false,
            )
        }
        Some(limit) => {
            let mut child = cmd
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("cannot start FFmpeg: {e}"))?;
            // Read the log on its own thread: a full pipe would stop FFmpeg.
            let mut err = child.stderr.take();
            let reader = std::thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(e) = err.as_mut() {
                    let _ = std::io::Read::read_to_end(e, &mut buf);
                }
                String::from_utf8_lossy(&buf).into_owned()
            });
            let t0 = std::time::Instant::now();
            let mut stopped = false;
            let status = loop {
                match child.try_wait() {
                    Ok(Some(st)) => break Some(st),
                    Ok(None) if t0.elapsed().as_secs_f64() >= limit => {
                        stopped = true;
                        let _ = child.kill();
                        let _ = child.wait();
                        break None;
                    }
                    Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
                    Err(e) => return Err(format!("cannot wait for FFmpeg: {e}")),
                }
            };
            let log = reader.join().unwrap_or_default();
            (log, status.is_some_and(|s| s.success()), stopped)
        }
    };
    if !stopped {
        if !ended_well {
            return Err(format!(
                "FFmpeg could not read {} to the end",
                file.display()
            ));
        }
        if let Some(why) = damaged(&log, vd.into_iter().chain(ad).reduce(f64::min)) {
            return Err(format!("{} is damaged: {why}", file.display()));
        }
    }
    // Stopped at the limit: the gates speak for the part that was read.
    let read = if stopped {
        read_to(&log).min(full)
    } else {
        full
    };
    let total = read.max(0.001);
    let mut out = Vec::new();
    if stopped {
        out.push(f(
            "budget",
            "warn",
            format!(
                "read the first {read:.0} s of {full:.0} s in {:.0} s: the rest is not checked",
                budget_s.unwrap_or(0.0)
            ),
            Some("Give a larger --budget, or run `check` with no --budget."),
        ));
    }

    // (start, seconds) of each black stretch. Only stretches of 0.5 s or more count for the black gate, as before.
    let after = |key: &str| -> Vec<f64> {
        log.split(key)
            .skip(1)
            .filter_map(|s| {
                s.split_whitespace()
                    .next()?
                    .trim_end_matches(',')
                    .parse()
                    .ok()
            })
            .collect()
    };
    let blacks: Vec<(f64, f64)> = after("black_start:")
        .into_iter()
        .zip(after("black_duration:"))
        .collect();
    let long = blacks.iter().map(|b| b.1).filter(|&d| d >= 0.5);
    let (black_total, black_max) = (long.clone().sum::<f64>() + 0.0, long.fold(0.0, f64::max));
    out.push(if black_max >= BLACK_FAIL_S {
        f(
            "black",
            "fail",
            format!("{black_max:.1} s of black in one stretch ({black_total:.1} s in all)"),
            Some("A shot is black: check that media file or stock clip, or remove it."),
        )
    } else {
        f(
            "black",
            "pass",
            format!("{black_total:.1} s of black"),
            None,
        )
    });
    // The first frame is the picture a feed or a player shows before the video plays (G188).
    out.push(match blacks.iter().find(|b| b.0 < 0.001) {
        Some(b) => f(
            "first_frame",
            "warn",
            format!("the video starts black, for {:.2} s", b.1),
            Some("Start on a picture: drop the fade from black, or cut the black frames with `scriptvox edit --keep`."),
        ),
        None => f("first_frame", "pass", "not black".into(), None),
    });
    // Three hard changes of the picture inside one second: a flash or a strobe (G188). A viewer with photosensitive
    // epilepsy is at risk above 3 flashes a second, and platforms warn on it.
    let changes = after("lavfi.scd.time:");
    let worst = changes
        .iter()
        .map(|&t| {
            (
                changes.iter().filter(|&&u| u >= t && u < t + 1.0).count(),
                t,
            )
        })
        .max_by_key(|c| c.0);
    out.push(match worst {
        Some((n, t)) if n >= 3 => f(
            "flashes",
            "warn",
            format!("{n} hard changes of the picture in 1 s, from {t:.1} s"),
            Some("Slow the cuts there, or remove the flash: more than 3 flashes a second can harm a viewer."),
        ),
        _ => f("flashes", "pass", "no 3 hard changes inside 1 s".into(), None),
    });

    let (mut frozen, _) = sum_after(&log, "freeze_duration:");
    // A freeze that lasts to the end of the file has a start and no duration.
    let starts: Vec<f64> = log
        .split("freeze_start:")
        .skip(1)
        .filter_map(|s| s.split_whitespace().next()?.parse().ok())
        .collect();
    if starts.len() > log.matches("freeze_duration:").count() {
        frozen += total - starts.last().copied().unwrap_or(total);
    }
    let share = (frozen / total).min(1.0);
    out.push(match share {
        s if s > FROZEN_FAIL => f("slideshow", "fail", format!("{:.0}% of the video does not move", s * 100.0), Some("Use video clips, the gradient template, or more stills (each still gets a slow zoom).")),
        s if s > FROZEN_WARN => f("slideshow", "warn", format!("{:.0}% of the video does not move", s * 100.0), Some("Add motion: video clips read better than stills.")),
        s => f("slideshow", "pass", format!("{:.0}% frozen", s * 100.0), None),
    });

    let (_, silence_max) = sum_after(&log, "silence_duration:");
    // No sound stream: nothing was examined, so no pass. Gate loudness says "no audio stream".
    if ff.has_stream(file, "audio") {
        out.push(if silence_max >= 4.0 {
            f(
                "silence",
                "warn",
                format!("{silence_max:.1} s without sound"),
                Some("If it is not on purpose: check the pause tags, or give the brief `music`."),
            )
        } else {
            f(
                "silence",
                "pass",
                format!("longest silence {silence_max:.1} s"),
                None,
            )
        });
    }

    let lufs = log.rfind("Summary:").and_then(|i| {
        log[i..]
            .split("I:")
            .nth(1)?
            .split_whitespace()
            .next()?
            .parse::<f64>()
            .ok()
    });
    out.push(match lufs {
        Some(i) if (i - crate::audio::TARGET_LUFS).abs() > 2.0 => f(
            "loudness",
            "warn",
            format!("{i:.1} LUFS (target {})", crate::audio::TARGET_LUFS),
            // A line a person can act on (the first `check` of the stranger test, 13 Nov 2026, had none).
            Some("Write the file again at -14 LUFS: `scriptvox edit <this file> --from 0 --out <new file>.mp4`. Then run `check` on the new file."),
        ),
        Some(i) => f("loudness", "pass", format!("{i:.1} LUFS"), None),
        None if stopped => f(
            "loudness",
            "warn",
            "not measured: the loudness of a programme needs the whole file".into(),
            Some("Run `check` with no --budget for the loudness."),
        ),
        None => f(
            "loudness",
            "warn",
            "no audio stream".into(),
            Some("A video for social media needs sound."),
        ),
    });

    match (vd, ad) {
        (Some(v), Some(a)) if (v - a).abs() > 1.0 / fps + 1e-3 => out.push(f(
            "av_drift",
            "fail",
            format!("video {v:.3} s, audio {a:.3} s"),
            Some("Audio and video lengths differ by more than one frame: re-render, and report it if it repeats."),
        )),
        (Some(v), Some(a)) => out.push(f("av_drift", "pass", format!("{:.1} ms", (v - a).abs() * 1000.0), None)),
        _ => {}
    }
    Ok(out)
}

/// The graph of a contact sheet: `n` inputs, each through `each` (its scale), in `cols` columns, ending in `[o]`.
/// `snapshot` and the storyboard both use it.
pub fn tile_graph(n: usize, cols: usize, each: &str) -> String {
    // An empty place at the end of a grid has the colour of the gaps (5 cards in 3 columns left a black tile).
    // A grey gap after each tile: without it a caption read on into the next tile (4 Nov 2026 eval).
    let gap = if n > 1 {
        ",pad=iw+12:ih+12:0:0:color=0x404040"
    } else {
        ""
    };
    let scaled: String = (0..n)
        .map(|i| format!("[{i}:v]{each}{gap}[f{i}];"))
        .collect();
    let labels: String = (0..n).map(|i| format!("[f{i}]")).collect();
    let layout: Vec<String> = (0..n)
        .map(|i| {
            format!(
                "{}_{}",
                if i % cols == 0 {
                    "0".into()
                } else {
                    (0..i % cols).map(|_| "w0").collect::<Vec<_>>().join("+")
                },
                if i / cols == 0 {
                    "0".into()
                } else {
                    (0..i / cols).map(|_| "h0").collect::<Vec<_>>().join("+")
                }
            )
        })
        .collect();
    if n == 1 {
        format!("{scaled}[f0]null[o]")
    } else {
        format!(
            "{scaled}{labels}xstack=inputs={n}:layout={}:fill=0x404040[o]",
            layout.join("|")
        )
    }
}

/// One PNG: frames at `times` side by side, or a `n`-frame contact sheet.
pub fn snapshot(
    ff: &Ffmpeg,
    file: &Path,
    times: &[f64],
    sheet: Option<u32>,
    out: &Path,
) -> Result<Vec<f64>, String> {
    snapshot_with(ff, file, times, sheet, out, "")
}

/// `snapshot`, with extra filters on each frame before it is scaled (the screen zones of `--safe`). Empty or `,...`.
pub fn snapshot_with(
    ff: &Ffmpeg,
    file: &Path,
    times: &[f64],
    sheet: Option<u32>,
    out: &Path,
    extra: &str,
) -> Result<Vec<f64>, String> {
    let (vd, _, _) = probe_durations(ff, file);
    let total = vd.unwrap_or(1.0);
    let ts: Vec<f64> = match sheet {
        Some(n) => (0..n)
            .map(|i| total * (i as f64 + 0.5) / n as f64)
            .collect(),
        None => times
            .iter()
            .map(|t| t.clamp(0.0, (total - 0.05).max(0.0)))
            .collect(),
    };
    if ts.is_empty() {
        return Err("give --at times or --sheet N".into());
    }
    let cols = match sheet {
        Some(3) => 3, // one row: a 2 by 2 grid left a black tile
        Some(n) => (n as f64).sqrt().ceil() as usize,
        // Up to 4 in one row; 13 cards in a row came back too small to read.
        None if ts.len() > 4 => (ts.len() as f64).sqrt().ceil() as usize,
        None => ts.len(),
    };
    let mut a: Vec<String> = Vec::new();
    for t in &ts {
        a.extend([
            "-ss".into(),
            format!("{t:.3}"),
            "-i".into(),
            file.to_string_lossy().into_owned(),
        ]);
    }
    let each = format!("trim=end_frame=1{extra},scale=360:-2,setsar=1");
    let graph = tile_graph(ts.len(), cols, &each);
    a.extend([
        "-filter_complex".into(),
        graph,
        "-map".into(),
        "[o]".into(),
        "-frames:v".into(),
        "1".into(),
        "-update".into(),
        "1".into(),
        out.to_string_lossy().into_owned(),
    ]);
    ff.run(Path::new("."), &a, None).map(|()| ts)
}

/// The caption text on screen at `t` seconds, from SRT text (cues as scriptvox writes them).
pub fn caption_at(srt: &str, t: f64) -> Option<String> {
    let secs = |s: &str| -> Option<f64> {
        let (hms, ms) = s.trim().split_once(',')?;
        let p: Vec<f64> = hms
            .split(':')
            .map(|x| x.parse().ok())
            .collect::<Option<_>>()?;
        Some(p.first()? * 3600.0 + p.get(1)? * 60.0 + p.get(2)? + ms.parse::<f64>().ok()? / 1000.0)
    };
    srt.replace("\r\n", "\n").split("\n\n").find_map(|cue| {
        let mut lines = cue.lines().skip_while(|l| !l.contains("-->"));
        let (a, b) = lines.next()?.split_once("-->")?;
        // Without the invisible right-to-left marks, so the text compares equal to the brief.
        (secs(a)? <= t && t < secs(b)?).then(|| {
            lines
                .collect::<Vec<_>>()
                .join(" ")
                .replace(['\u{200E}', '\u{200F}'], "")
        })
    })
}

#[cfg(test)]
mod tests {
    /// make-1: FFmpeg 8 logs a corrupt packet this way, decodes to the end and exits 0.
    #[test]
    fn a_corrupt_packet_in_the_log_is_damage() {
        let log = "[vist#0:0/h264] Error submitting packet to decoder: Invalid data found when processing input\n";
        assert!(super::damaged(log, None).is_some());
    }

    #[test]
    fn extra_heard_words_are_named() {
        let f = words_heard(
            "how much water",
            "how much water how much water how much water",
        );
        assert!(
            f.detail.contains("6 heard words are not in the script"),
            "{}",
            f.detail
        );
        assert_eq!(f.status, "pass");
        assert!(!words_heard("how much water", "how much water")
            .detail
            .contains("not in the script"));
    }

    use super::*;

    #[test]
    fn caption_at_finds_the_cue_on_screen() {
        let srt = "1\n00:00:00,000 --> 00:00:01,500\nMost people think\n\n2\n00:00:01,500 --> 00:00:03,000\na desert\nhas no water.\n";
        assert_eq!(caption_at(srt, 0.2).as_deref(), Some("Most people think"));
        assert_eq!(
            caption_at(srt, 2.0).as_deref(),
            Some("a desert has no water.")
        );
        assert_eq!(caption_at(srt, 3.5), None);
    }

    #[test]
    fn sums_durations_from_logs() {
        let log = "black_start:0 black_end:3 black_duration:3\n... black_duration:0.5\n";
        assert_eq!(sum_after(log, "black_duration:"), (3.5, 3.0));
    }

    /// A one-event document; text is a whole ASS body with override tags.
    fn one(font: &str, size: u32, an: u32, text: &str) -> String {
        format!("[V4+ Styles]\nStyle: Cap,{font},{size},&H0,&H0,&H0,&H0,1,0,0,0,100,100,0,0,4,0,0,{an},60,60,100,-1\n\n[Events]\nDialogue: 0,0:00:06.00,0:00:08.00,Cap,,0,0,0,,{text}\n")
    }

    /// A card's text and a caption in one document: the caption at the bottom of a 1920x1080 frame (2 lines, 84 px),
    /// the card text at `y`, on screen from `from` to `to`.
    fn card_and_caption(y: u32, from: &str, to: &str) -> String {
        format!(
            "[V4+ Styles]\nStyle: Cap,Noto Sans,84,&H0,&H0,&H0,&H0,1,0,0,0,100,100,0,0,4,0,0,2,60,60,97,-1\nStyle: Card,Noto Sans,84,&H0,&H0,&H0,&H0,1,0,0,0,100,100,0,0,4,0,0,5,40,40,0,-1\n\n[Events]\nDialogue: 0,0:00:06.00,0:00:08.00,Cap,,0,0,0,,Two lines of\\Ncaption text\nDialogue: 2,{from},{to},Card,,0,0,0,,{{\\an8\\pos(960,{y})\\fs60}}revenue growth\n"
        )
    }

    #[test]
    fn a_card_text_over_the_captions_is_a_warning_with_its_scene() {
        let frame = (1920, 1080);
        // The caption's 2 lines of 84 px stand on a 97 px margin: its text is from y 815 to 983.
        // A label at y 700 with 60 px ends at 760: clear of it.
        let v = text_bounds(
            &card_and_caption(700, "0:00:06.00", "0:00:08.00"),
            frame,
            &[0.0, 5.0],
        )
        .unwrap();
        assert_eq!(v.status, "pass", "{}", v.detail);
        // At y 800 it ends at 860: 45 px into the caption.
        let v = text_bounds(
            &card_and_caption(800, "0:00:06.00", "0:00:08.00"),
            frame,
            &[0.0, 5.0],
        )
        .unwrap();
        assert_eq!(v.status, "warn", "{}", v.detail);
        assert!(
            v.detail
                .contains("scene 2: \"revenue growth\" lies 45 px over the captions"),
            "{}",
            v.detail
        );
        // The same place at another time is no overlap.
        let v = text_bounds(
            &card_and_caption(800, "0:00:02.00", "0:00:04.00"),
            frame,
            &[0.0, 5.0],
        )
        .unwrap();
        assert_eq!(v.status, "pass", "{}", v.detail);
    }

    #[test]
    fn the_last_time_of_a_log_is_how_far_a_read_got() {
        let log = "frame= 10 fps=0.0 time=00:00:01.50 bitrate=N/A\rframe= 900 fps=450 time=00:01:31.25 bitrate=N/A speed=91x";
        assert!((super::read_to(log) - 91.25).abs() < 1e-6);
        assert_eq!(super::read_to("no progress line"), 0.0);
        assert_eq!(super::read_to("time=N/A"), 0.0);
    }

    /// later-3: with no scene times the message names no scene.
    #[test]
    fn a_run_with_no_scenes_names_no_scene() {
        let wide = "Pneumonoultramicroscopicsilicovolcanoconiosis";
        let v = text_bounds(&one("Noto Sans", 84, 2, wide), (1080, 1920), &[]).unwrap();
        assert_eq!(v.status, "fail", "{}", v.detail);
        assert!(!v.detail.contains("scene"), "{}", v.detail);
    }

    #[test]
    fn a_word_too_wide_for_the_frame_fails_with_its_scene() {
        let (w, h) = (1080, 1920);
        let starts = [0.0, 3.0, 5.0, 9.0];
        let wide = "Pneumonoultramicroscopicsilicovolcanoconiosis";
        let v = text_bounds(&one("Noto Sans", 84, 2, wide), (w, h), &starts).unwrap();
        assert_eq!(v.status, "fail", "{}", v.detail);
        assert!(
            v.detail.contains("scene 3") && v.detail.contains("past the"),
            "{}",
            v.detail
        );
        // The same word stays when the size is small enough, measured, not guessed.
        let v = text_bounds(&one("Noto Sans", 30, 2, wide), (w, h), &starts).unwrap();
        assert_eq!(v.status, "pass", "{}", v.detail);
    }

    #[test]
    fn arabic_is_measured_shaped_not_letter_by_letter() {
        let (w, h) = (1080, 1920);
        let word = "المستشفياتالمتخصصةوالجامعاتالحكومية";
        let font = crate::template::FONTS[1].2;
        let m = crate::captions::Measurer::new(font.to_vec(), 84.0).unwrap();
        let joined = m.width(word);
        let alone: f32 = word.chars().map(|c| m.width(&c.to_string())).sum();
        assert!(
            alone > joined + 100.0,
            "shaping must change width: {joined} {alone}"
        );
        // A frame between the two: the shaped width fits, the letter-by-letter one does not.
        let f = ((joined + alone) / 2.0) as u32;
        let ass = one("Noto Sans Arabic", 84, 5, word);
        let v = text_bounds(&ass, (f, h), &[0.0]).unwrap();
        assert_ne!(v.status, "fail", "{}", v.detail);
        let v = text_bounds(
            &one("Noto Sans Arabic", 84, 2, &word.repeat(2)),
            (w, h),
            &[0.0, 5.0],
        )
        .unwrap();
        assert_eq!(v.status, "fail", "{}", v.detail);
        assert!(v.detail.contains("scene 2"), "{}", v.detail);
    }

    #[test]
    fn a_placed_box_resting_outside_fails_and_a_drawing_does_not() {
        let (w, h) = (1080, 1920);
        let at = |t: &str| one("Noto Sans", 84, 2, t);
        // \\pos with centre alignment, close to the right edge.
        let v = text_bounds(&at("{\\an5\\pos(1050,400)}Hello there"), (w, h), &[0.0]).unwrap();
        assert_eq!(v.status, "fail", "{}", v.detail);
        // \\move: the place it stops counts, not the place it comes from.
        let v = text_bounds(
            &at("{\\an5\\move(2000,400,540,400,0,300)}Hello"),
            (w, h),
            &[0.0],
        )
        .unwrap();
        assert_eq!(v.status, "pass", "{}", v.detail);
        // The size it settles at counts.
        let v = text_bounds(
            &at("{\\an5\\pos(540,400)\\fscx300\\t(0,180,\\fscx100)}Hello"),
            (w, h),
            &[0.0],
        )
        .unwrap();
        assert_eq!(v.status, "pass", "{}", v.detail);
        // A vector drawing is no text.
        let v = text_bounds(
            &at("{\\an7\\pos(0,0)\\p1}m 0 0 l 4000 0 4000 4000"),
            (w, h),
            &[0.0],
        );
        assert!(v.is_none());
        // Inside the frame but on the edge: a warning.
        let v = text_bounds(&at("{\\an1\\pos(4,400)}Hello"), (w, h), &[0.0]).unwrap();
        assert_eq!(v.status, "warn", "{}", v.detail);
        // A family that is not ours is said, not silently passed.
        let v = text_bounds(&one("Nope Font", 84, 2, "Hello"), (w, h), &[0.0]).unwrap();
        assert_eq!(v.status, "warn", "{}", v.detail);
        assert!(v.detail.contains("not measured"), "{}", v.detail);
    }

    #[test]
    fn caption_pages_out_of_step_warn() {
        use crate::captions::Page;
        let p = |n: usize, start: f64, end: f64| Page {
            lines: vec![(0..n).collect()],
            start,
            end,
            said_end: end,
        };
        assert_eq!(
            caption_pages(&[p(4, 0.1, 1.5), p(6, 1.9, 4.9)]).status,
            "pass"
        );
        // r_6d3155.1.1: "one" from 3.16 to 7.0 s, the last page with no time.
        assert_eq!(caption_pages(&[p(1, 3.16, 7.0)]).status, "warn");
        assert_eq!(caption_pages(&[p(6, 12.737, 12.737)]).status, "warn");
    }

    /// 13 Nov 2026: the last page of scene 1 ended at 3.83 s, the first page of scene 2 started at 3.48 s.
    #[test]
    fn two_caption_pages_at_one_time_fail() {
        use crate::captions::Page;
        let p = |start: f64, end: f64| Page {
            lines: vec![(0..5).collect()],
            start,
            end,
            said_end: end,
        };
        let f = caption_pages(&[p(0.0, 3.825), p(3.479, 6.9)]);
        assert_eq!(f.status, "fail");
        assert!(
            f.detail.contains("3.83 s") && f.detail.contains("3.48 s"),
            "{}",
            f.detail
        );
        assert!(f.fix.is_some());
        // A page that ends where the next one starts is the normal hold through a pause.
        assert_eq!(
            caption_pages(&[p(0.0, 3.479), p(3.479, 6.9)]).status,
            "pass"
        );
    }

    #[test]
    fn numbers_match_in_digits_or_words_either_way() {
        // years written as words in the script, digits heard (eval fault of 4 Nov)
        assert_eq!(
            heard_counts("In two thousand five it grew.", "In 2005 it grew.").0,
            6
        );
        assert_eq!(
            heard_counts("By twenty twenty-five it passed.", "By 2025 it passed.").0,
            6
        );
        assert_eq!(
            heard_counts("In 1990 it began.", "In nineteen ninety it began.").0,
            5
        );
        assert_eq!(
            heard_counts(
                "It cost 342 dirhams.",
                "It cost three hundred forty-two dirhams."
            )
            .0,
            7
        );
        assert_eq!(
            cardinal_words(1_400_000).join(" "),
            "one million four hundred thousand"
        );
        assert_eq!(year_words(1905).unwrap().join(" "), "nineteen oh five");
    }

    #[test]
    fn a_decimal_is_heard_as_its_spoken_parts() {
        let script = "In 2005 one point four million. Pi is three point one four.";
        let heard = "In 2005 1.4 million. Pi is 3.14.";
        let (hit, n, _) = heard_counts(script, heard);
        assert_eq!(
            (hit, n),
            (14, 14),
            "2005 counts as its three spoken words on both sides"
        );
    }

    #[test]
    fn a_repeated_sentence_heard_once_is_not_a_failed_voice() {
        let script = "Most trips are by car today. Most trips are by car today.";
        let got = words_heard(script, "Most trips are by car today.");
        assert_eq!(got.status, "pass", "{}", got.detail);
        assert!(got.detail.contains("listen"), "{}", got.detail);
        assert_eq!(
            words_heard(
                "One two three four. Five six seven eight.",
                "One two three four."
            )
            .status,
            "fail"
        );
    }

    #[test]
    fn limits_come_only_from_gates_that_ran() {
        let ran = serde_json::json!({"gate": "av_drift", "detail": "1.0 ms"});
        let not = serde_json::json!({"gate": "words", "detail": "not checked: no row"});
        let (cover, limit) = limits(&[
            ran,
            not,
            serde_json::json!({"gate": "no_row", "detail": "x"}),
        ]);
        assert_eq!(cover, ["av_drift: the lengths of the streams"]);
        assert_eq!(limit.len(), 1);
        assert!(limit[0].starts_with("av_drift: "));
        assert_eq!(limits(&[]), (vec![], vec![]));
    }

    #[test]
    fn not_checked_names_what_did_not_run() {
        let ran = serde_json::json!({"gate": "words", "status": "pass", "detail": "heard 9 of 9"});
        let failed_listen = serde_json::json!({"gate": "words", "status": "warn", "detail": "not checked: the row failed"});
        let want = [("words", "no speech to text row")];
        assert_eq!(not_checked(&[ran], &want).len(), 2, "words ran");
        let got = not_checked(&[failed_listen], &want);
        assert_eq!(got[0], "words: no speech to text row");
        assert!(got.iter().any(|g| g.starts_with("judge:")));
        assert!(got.iter().any(|g| g.starts_with("person:")));
    }

    #[test]
    fn words_heard_counts_script_words_in_order() {
        assert_eq!(norm_word("إِنَّ"), "ان");
        assert_eq!(norm_word("٩٠٪"), "90");
        assert_eq!(norm_word("City,"), "city");
        assert_eq!(norm_word("Two"), "2");
        assert_eq!(norm_word("أربعة"), "4");
        assert_eq!(
            heard_counts("about 2× the gap", "about two times the gap").0,
            4
        );
        let g = words_heard(
            "The sea [pause 1] hits the rocks.",
            "the sea hits the rocks",
        );
        assert_eq!(
            (g.status, g.detail.starts_with("heard 5 of 5")),
            ("pass", true)
        );
        assert_eq!(words_heard("one two three four", "one two").status, "fail");
        assert_eq!(
            words_heard("a b c d e f g h", "a b c d e f x h").status,
            "warn"
        );
        assert_eq!(
            words_heard("المدينة القديمة", "المدينه القديمه").status,
            "pass"
        );
    }
}
