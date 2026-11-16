//! Several cards in one scene, one after the other (docs/specs/scene-cards.md): the scene field `cards` and the
//! card field `at`. The cards of a scene, the check of the brief, and each card's time window from the voice's
//! words. Drawing a card stays in `cards.rs`.

use crate::brief::Scene;
use crate::captions::Word;
use crate::cards::Card;

/// The most cards one scene takes.
pub const MAX: usize = 4;
/// Seconds between the end of a card and the start of the next, so one fade does not lie under the other.
const GAP: f64 = 0.2;
/// The least time a card shows before the next one cuts it.
const FLOOR: f64 = 0.9;

/// The cards of a scene, from `card` or `cards`. Every "has this scene a card" question reads this.
pub fn of(sc: &Scene) -> impl Iterator<Item = &Card> {
    sc.card.iter().chain(sc.cards.iter().flatten())
}

/// A refusal `make` gives after the voice is made, with its fix line (exit 2).
#[derive(Debug)]
pub struct Refusal {
    pub msg: String,
    pub fix: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for Refusal {}

/// A problem with the scene's `card` or `cards` that the brief alone shows (for `plan` and `make`, exit 2).
pub fn problem(sc: &Scene) -> Option<String> {
    let timing = |c: &Card| match (c.at, &c.on) {
        (Some(_), Some(_)) if c.then.is_none() => {
            Some("give the card `on` (a spoken word) or `at` (seconds), not both".to_string())
        }
        (Some(at), _) if !at.is_finite() || at < 0.0 => Some(format!(
            "`at` of {at} is not a time: give seconds from the scene's start, 0 or more"
        )),
        _ => None,
    };
    let one = |c: &Card| {
        c.problem()
            .or_else(|| timing(c))
            .or_else(|| c.on_problem(&sc.script))
    };
    let Some(cards) = &sc.cards else {
        return sc.card.as_ref().and_then(one);
    };
    if sc.card.is_some() {
        return Some(
            "the scene has `card` and `cards`: give one of them (put the single card in the `cards` list)"
                .into(),
        );
    }
    if cards.is_empty() || cards.len() > MAX {
        return Some(format!(
            "`cards` takes 1 to {MAX} cards, and this scene has {} (use another scene for more)",
            cards.len()
        ));
    }
    // Where each `on` word is first said in the script, as `cards::word_start` reads the voice's words.
    let script = crate::pauses::strip(&sc.script);
    let place = |on: &str| {
        let key = crate::gates::norm_word(on);
        script
            .split_whitespace()
            .position(|w| crate::gates::norm_word(w).starts_with(&key))
    };
    let mut last_on: Option<(usize, usize)> = None;
    let mut last_at: Option<(usize, f64)> = None;
    for (j, c) in cards.iter().enumerate() {
        let n = j + 1;
        if c.behind == Some(true) || c.then.is_some() {
            return Some(format!(
                "card {n} of `cards`: a card with `behind` or `then` owns its whole scene (give it as the scene's single `card`)"
            ));
        }
        if let Some(p) = one(c) {
            return Some(format!("card {n} of `cards`: {p}"));
        }
        if let Some(at) = c.at {
            if let Some((m, before)) = last_at.filter(|(_, b)| at <= *b) {
                return Some(format!(
                    "card {n} of `cards` has `at` {at} and card {m} before it has `at` {before}: list the cards in time order"
                ));
            }
            last_at = Some((n, at));
        }
        if let Some(i) = c.on.as_deref().and_then(place) {
            if let Some((m, _)) = last_on.filter(|(_, b)| i <= *b) {
                return Some(format!(
                    "card {n} of `cards`: its `on` word \"{}\" is not said after the `on` word of card {m} (list the cards in the order the script says their words, each with its own word)",
                    c.on.as_deref().unwrap_or("")
                ));
            }
            last_on = Some((n, i));
        }
    }
    None
}

/// Each card's (start, end) inside its scene. `cut` is the scene's own start (`at` counts from it), `from` is where
/// a card may start (the cut, or later when the hook is still up), `end` is the scene's end, and `said` holds the
/// scene's spoken words. A single `card` with no `at` gets the times of `cards::window`, as before.
pub fn windows(
    scene: usize,
    cards: &[&Card],
    cut: f64,
    from: f64,
    end: f64,
    said: &[Word],
) -> std::io::Result<Vec<(f64, f64)>> {
    let plain = |e: String| std::io::Error::other(format!("scene {scene}: {e}"));
    let refuse = |msg: String, fix: &str| {
        std::io::Error::other(Refusal {
            msg: format!("scene {scene}: {msg}"),
            fix: fix.into(),
        })
    };
    if let [c] = cards {
        if c.at.is_none() {
            return crate::cards::window(c, from, end, said)
                .map(|w| vec![w])
                .map_err(plain);
        }
    }
    // The starts the brief gives: a word's time or `at`, after the hook and with 0.9 s left in the scene.
    let mut starts: Vec<Option<f64>> = Vec::new();
    for (j, c) in cards.iter().enumerate() {
        let t = match (c.at, c.on.as_deref().filter(|_| c.then.is_none())) {
            (Some(at), _) if cut + at >= end => {
                return Err(refuse(
                    format!(
                        "card {}: `at` {at} s is after the scene's end (the scene is {:.1} s long)",
                        j + 1,
                        end - cut
                    ),
                    "Give a smaller `at`, or use `on` with a word of the scene's script.",
                ));
            }
            (Some(at), _) => Some(cut + at),
            // A word the voice did not give: the message of `cards::window`.
            (None, Some(on)) => match crate::cards::word_start(on, said) {
                Some(t) => Some(t),
                None => {
                    return Err(crate::cards::window(c, from, end, said)
                        .err()
                        .map_or_else(|| plain(format!("card {} has no time", j + 1)), plain))
                }
            },
            (None, None) => None,
        };
        starts.push(t.map(|t| t.min(end - FLOOR).max(from)));
    }
    // A card with no start of its own: after the hold of the card before it, or an even share of the time up to
    // the next card that has a start.
    for j in 0..cards.len() {
        if starts[j].is_some() {
            continue;
        }
        starts[j] = Some(if j == 0 {
            from
        } else {
            let before = starts[j - 1].unwrap_or(from);
            match cards[j - 1].hold {
                Some(h) => before + h.max(0.5) + GAP,
                None => {
                    let next = (j + 1..cards.len()).find(|&k| starts[k].is_some());
                    let (slots, bound) = match next {
                        Some(k) => (k - j + 1, starts[k].unwrap_or(end)),
                        None => (cards.len() - j + 1, end),
                    };
                    before + (bound - before) / slots as f64
                }
            }
        });
    }
    let starts: Vec<f64> = starts.into_iter().flatten().collect();
    let mut out = Vec::new();
    for (j, c) in cards.iter().enumerate() {
        let lim = match starts.get(j + 1) {
            Some(&next) if next <= starts[j] => {
                return Err(refuse(
                    format!(
                        "card {} starts at {:.1} s and card {} at {next:.1} s: a card must start after the card before it",
                        j + 1,
                        starts[j],
                        j + 2
                    ),
                    "List the cards in time order, and give each its own `on` word or a later `at`. On scene one a card waits 1.6 s for the hook.",
                ));
            }
            Some(&next) if next - GAP - starts[j] < FLOOR => {
                return Err(refuse(
                    format!(
                        "card {} starts at {:.1} s and card {} at {next:.1} s: card {} has less than {FLOOR} s on screen (one card shows at a time)",
                        j + 1,
                        starts[j],
                        j + 2,
                        j + 1
                    ),
                    "Give the first of the two an earlier `on` word or `at`, give the second a later one, or use one card less.",
                ));
            }
            Some(&next) => next - GAP,
            None => end,
        };
        // With `on` the card finds its word again from the scene's start; else it starts where its turn is.
        let lo = if c.on.is_some() && c.then.is_none() {
            from
        } else {
            starts[j]
        };
        out.push(
            crate::cards::window(c, lo, lim, said)
                .map_err(|e| plain(format!("card {}: {e}", j + 1)))?,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(v: serde_json::Value) -> Scene {
        serde_json::from_value(v).unwrap()
    }

    fn said(words: &[(&str, f64)]) -> Vec<Word> {
        words
            .iter()
            .map(|(t, s)| Word {
                text: t.to_string(),
                start: *s,
                end: s + 0.3,
            })
            .collect()
    }

    fn wins(
        sc: &Scene,
        cut: f64,
        from: f64,
        end: f64,
        w: &[Word],
    ) -> Result<Vec<(f64, f64)>, String> {
        let cards: Vec<&Card> = of(sc).collect();
        windows(2, &cards, cut, from, end, w).map_err(|e| {
            let fix = e
                .get_ref()
                .and_then(|r| r.downcast_ref::<Refusal>())
                .map(|r| r.fix.clone())
                .unwrap_or_default();
            format!("{e} | {fix}")
        })
    }

    fn close(a: &[(f64, f64)], b: &[(f64, f64)]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(x, y)| (x.0 - y.0).abs() < 1e-9 && (x.1 - y.1).abs() < 1e-9)
    }

    const SCRIPT: &str = "Revenue grew by forty two percent. Three things made it happen today.";

    fn words() -> Vec<Word> {
        said(&[
            ("Revenue", 10.0),
            ("grew", 10.5),
            ("by", 11.0),
            ("forty", 11.5),
            ("two", 12.0),
            ("percent.", 12.5),
            ("Three", 14.0),
            ("things", 14.5),
            ("made", 15.0),
            ("it", 15.5),
            ("happen", 16.0),
            ("today.", 17.0),
        ])
    }

    #[test]
    fn three_cards_take_turns_with_a_gap() {
        let sc = scene(serde_json::json!({"script": SCRIPT, "cards": [
            {"kind": "title", "text": "Growth"},
            {"kind": "stat", "text": "42%", "on": "forty", "hold": 1.5},
            {"kind": "list", "items": ["A", "B"], "at": 4.5}]}));
        assert_eq!(problem(&sc), None);
        let w = wins(&sc, 10.0, 10.0, 18.0, &words()).unwrap();
        // The title from the scene's start to 0.2 s before "forty"; the stat for its hold; the list from 4.5 s
        // after the cut to the scene's end.
        assert!(
            close(&w, &[(10.0, 11.3), (11.5, 13.0), (14.5, 18.0)]),
            "{w:?}"
        );
    }

    #[test]
    fn a_hold_is_cut_at_the_next_card() {
        let sc = scene(serde_json::json!({"script": SCRIPT, "cards": [
            {"kind": "stat", "text": "42%", "on": "forty", "hold": 9},
            {"kind": "title", "text": "Three", "on": "three"}]}));
        let w = wins(&sc, 10.0, 10.0, 18.0, &words()).unwrap();
        assert!(close(&w, &[(11.5, 13.8), (14.0, 18.0)]), "{w:?}");
    }

    #[test]
    fn cards_with_no_times_share_the_scene() {
        let sc = scene(serde_json::json!({"script": SCRIPT, "cards": [
            {"kind": "title", "text": "One"}, {"kind": "title", "text": "Two"}]}));
        assert_eq!(problem(&sc), None);
        let w = wins(&sc, 10.0, 10.0, 18.0, &words()).unwrap();
        assert!(close(&w, &[(10.0, 13.8), (14.0, 18.0)]), "{w:?}");
        // After a hold: 0.2 s after the card before it ends. Then an even share up to the card with a start.
        let sc = scene(serde_json::json!({"script": SCRIPT, "cards": [
            {"kind": "title", "text": "One", "hold": 1.0}, {"kind": "title", "text": "Two"},
            {"kind": "title", "text": "Three"}, {"kind": "title", "text": "Four", "at": 6.2}]}));
        let w = wins(&sc, 10.0, 10.0, 18.0, &words()).unwrap();
        assert!(
            close(
                &w,
                &[(10.0, 11.0), (11.2, 13.5), (13.7, 16.0), (16.2, 18.0)]
            ),
            "{w:?}"
        );
    }

    #[test]
    fn at_counts_from_the_cut_and_waits_for_the_hook() {
        let sc = scene(
            serde_json::json!({"script": SCRIPT, "card": {"kind": "title", "text": "Go", "at": 2, "hold": 1}}),
        );
        assert_eq!(problem(&sc), None);
        assert!(close(
            &wins(&sc, 10.0, 10.0, 18.0, &words()).unwrap(),
            &[(12.0, 13.0)]
        ));
        // A formula with `then` keeps `on` for its change, and `at` is still its start.
        let sc = scene(
            serde_json::json!({"script": SCRIPT, "card": {"kind": "formula", "text": "a", "then": "b", "on": "three", "at": 2}}),
        );
        assert_eq!(problem(&sc), None);
        assert!(close(
            &wins(&sc, 10.0, 10.0, 18.0, &words()).unwrap(),
            &[(12.0, 18.0)]
        ));
        // Scene one under a hook: `at` 0.5 is under the hook, so the card starts when the hook goes.
        let sc = scene(
            serde_json::json!({"script": SCRIPT, "card": {"kind": "title", "text": "Go", "at": 0.5}}),
        );
        assert!(close(
            &wins(&sc, 0.0, 1.6, 8.0, &words()).unwrap(),
            &[(1.6, 8.0)]
        ));
    }

    #[test]
    fn a_single_card_keeps_the_times_of_cards_window() {
        for card in [
            serde_json::json!({"kind": "stat", "text": "42%", "on": "forty", "hold": 2.5}),
            serde_json::json!({"kind": "title", "text": "Go"}),
            serde_json::json!({"kind": "title", "text": "Late", "on": "today"}),
        ] {
            let one = scene(serde_json::json!({"script": SCRIPT, "card": card}));
            let list = scene(serde_json::json!({"script": SCRIPT, "cards": [card]}));
            let want =
                crate::cards::window(one.card.as_ref().unwrap(), 10.0, 18.0, &words()).unwrap();
            assert_eq!(wins(&one, 10.0, 10.0, 18.0, &words()).unwrap(), vec![want]);
            assert_eq!(wins(&list, 10.0, 10.0, 18.0, &words()).unwrap(), vec![want]);
        }
    }

    #[test]
    fn make_refuses_with_a_fix_line() {
        let two = |a: serde_json::Value, b: serde_json::Value| {
            let sc = scene(serde_json::json!({"script": SCRIPT, "cards": [a, b]}));
            assert_eq!(problem(&sc), None);
            wins(&sc, 10.0, 10.0, 18.0, &words()).unwrap_err()
        };
        let title = |extra: serde_json::Value| {
            let mut c = serde_json::json!({"kind": "title", "text": "T"});
            c.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            c
        };
        // "forty" at 11.5 s and "two" at 12.0 s: 0.3 s for the first card.
        let e = two(
            title(serde_json::json!({"on": "forty"})),
            title(serde_json::json!({"on": "two"})),
        );
        assert!(
            e.contains("scene 2: card 1 starts at 11.5 s and card 2 at 12.0 s"),
            "{e}"
        );
        assert!(
            e.contains("less than 0.9 s") && e.contains("| Give the first"),
            "{e}"
        );
        // `at` 1 s (11.0 s) for the second card, and the first comes with "forty" (11.5 s).
        let e = two(
            title(serde_json::json!({"on": "forty"})),
            title(serde_json::json!({"at": 1})),
        );
        assert!(
            e.contains("must start after the card before it") && e.contains("| List the cards"),
            "{e}"
        );
        let e = two(
            title(serde_json::json!({})),
            title(serde_json::json!({"at": 8})),
        );
        assert!(
            e.contains("card 2: `at` 8 s is after the scene's end (the scene is 8.0 s long)"),
            "{e}"
        );
        assert!(e.contains("| Give a smaller `at`"), "{e}");
        // A word the voice did not give keeps the message of a single card.
        let sc = scene(
            serde_json::json!({"script": SCRIPT, "cards": [title(serde_json::json!({"on": "absent"}))]}),
        );
        let e = wins(&sc, 10.0, 10.0, 18.0, &words()).unwrap_err();
        assert!(
            e.contains("scene 2: the voice's words do not have the card's `on` word"),
            "{e}"
        );
    }

    #[test]
    fn the_brief_check_names_the_card() {
        let bad = |extra: serde_json::Value| {
            let mut sc = serde_json::json!({"script": SCRIPT});
            sc.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            problem(&scene(sc)).unwrap()
        };
        let t = |extra: serde_json::Value| {
            let mut c = serde_json::json!({"kind": "title", "text": "T"});
            c.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            c
        };
        let none = serde_json::json!({});
        assert!(
            bad(serde_json::json!({"cards": []})).contains("1 to 4 cards, and this scene has 0")
        );
        let five: Vec<_> = (0..5).map(|_| t(none.clone())).collect();
        assert!(bad(serde_json::json!({"cards": five})).contains("has 5"));
        assert!(
            bad(serde_json::json!({"card": t(none.clone()), "cards": [t(none.clone())]}))
                .contains("`card` and `cards`")
        );
        let on_at = serde_json::json!({"on": "forty", "at": 1});
        assert!(
            bad(serde_json::json!({"cards": [t(none.clone()), t(on_at.clone())]}))
                .starts_with("card 2 of `cards`: give the card `on`")
        );
        assert!(bad(serde_json::json!({"card": t(on_at)})).starts_with("give the card `on`"));
        assert!(
            bad(serde_json::json!({"card": t(serde_json::json!({"at": -1}))}))
                .contains("`at` of -1 is not a time")
        );
        assert!(
            bad(serde_json::json!({"cards": [t(serde_json::json!({"on": "absent"}))]}))
                .contains("card 1 of `cards`: the card's `on` word \"absent\"")
        );
        let (a, b) = (
            t(serde_json::json!({"on": "three"})),
            t(serde_json::json!({"on": "forty"})),
        );
        assert!(bad(serde_json::json!({"cards": [a.clone(), b]})).contains(
            "card 2 of `cards`: its `on` word \"forty\" is not said after the `on` word of card 1"
        ));
        assert!(bad(serde_json::json!({"cards": [a.clone(), a]})).contains("not said after"));
        let (a, b) = (
            t(serde_json::json!({"at": 3})),
            t(serde_json::json!({"at": 3})),
        );
        assert!(bad(serde_json::json!({"cards": [a, b]}))
            .contains("card 2 of `cards` has `at` 3 and card 1 before it has `at` 3"));
        assert!(
            bad(serde_json::json!({"cards": [t(serde_json::json!({"behind": true}))]}))
                .contains("owns its whole scene")
        );
        let f = serde_json::json!({"kind": "formula", "text": "a", "then": "b"});
        assert!(bad(serde_json::json!({"cards": [f]})).contains("owns its whole scene"));
        // A single card's own refusals, with the card's place.
        let long = t(serde_json::json!({"text": "x".repeat(61)}));
        assert!(bad(serde_json::json!({"cards": [t(none), long]}))
            .starts_with("card 2 of `cards`: a title card takes at most 60"));
    }
}
