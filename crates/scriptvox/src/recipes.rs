//! `scriptvox recipes`: a starter brief per kind of video (`docs/specs/recipes.md`). A recipe is data: one complete,
//! free-to-run brief in `crates/scriptvox/recipes/` and the notes below. No render code.

use serde_json::{json, Value};

pub struct Recipe {
    pub name: &'static str,
    /// The kind of video, in the words a user says.
    pub kind: &'static str,
    /// The length it is written for.
    pub seconds: u32,
    pub use_for: &'static str,
    /// Hook, beats, ending.
    pub script: &'static str,
    /// Card choices, and why.
    pub cards: &'static str,
    /// Music, transition, captions.
    pub look: &'static str,
    pub adapt: &'static [&'static str],
    pub brief: &'static str,
}

macro_rules! brief {
    ($name:literal) => {
        include_str!(concat!("../recipes/", $name, ".json"))
    };
}

/// What every recipe's `adapt` ends with.
const PICTURES: &str = "No scene has a picture, so the plain `background` carries it. Add `media` (your file), `stock` (search words) or `generate` to a scene for a picture; `plan` shows the cost before anything runs.";
const VOICE: &str = "The voice is the free local one (`scriptvox setup voice-rust` one time). For another voice change `tts` and `voice` (`scriptvox voices --json`), then take the words `plan` names in its `fit` finding.";
const ARABIC_VOICE: &str = "There is no free local Arabic voice, so this is `tts: none` (text at reading pace). With a network: set `\"tts\": \"edge\", \"voice\": \"ar-AE-HamdanNeural\"` and cut the script to about 95 words a minute of video. Load the scriptvox-arabic skill.";
const PLACEHOLDER: &str = "Every fact here is invented. Replace each number in the script and the cards, and replace each `sources` entry with the real fact and its real address; `plan` warns (gate `claims`) about a number that is in no source.";

pub const ALL: [Recipe; 12] = [
    Recipe {
        name: "explainer",
        kind: "explainer",
        seconds: 60,
        use_for: "Teach one idea to a general viewer on a wide screen: how or why something works.",
        script: "Hook: a claim that goes against what people think. Beats: what people believe, why that is wrong, the real reason in two or three steps, one thing to try. Ending: the first claim again, now understood. 8 scenes of 5 to 9 s.",
        cards: "A title at the change of part, a mark on the key words, `steps` for the process, a `diagram` for cause and effect, a `list` for the result, and two cards in turn in the scene that has an example. Two scenes have no card: the rest between cards.",
        look: "Music `calm`, quiet. Transition `fade`. Captions `karaoke` (a whole phrase stays readable). Background `night`.",
        adapt: &[
            "Keep one idea per scene and the order of the beats; change the words and the card texts together, because `steps`, `list` and `on` follow the spoken words.",
            "A diagram takes at most 4 steps in a row. A longer chain is a `steps` card.",
            VOICE,
            PICTURES,
        ],
        brief: brief!("explainer"),
    },
    Recipe {
        name: "explainer-short",
        kind: "explainer",
        seconds: 30,
        use_for: "One small question answered for a phone feed.",
        script: "Hook: the answer turned upside down, in the first sentence. Beats: the cause, the effect, what the body or the thing does. Ending: one thing the viewer can do today. 5 scenes of 4 to 8 s.",
        cards: "One title, one `diagram` (it runs top to bottom on a tall frame), one `list` for the two things to do. The first scene has only the hook.",
        look: "Music `calm`. Transition `rise`. Captions `pop`, the fast look of a vertical short. Background `forest`.",
        adapt: &[
            "Keep it to one cause and one effect. A second idea is a second video.",
            "With `pop` captions keep card texts short: the big words and the card share a tall frame.",
            VOICE,
            PICTURES,
        ],
        brief: brief!("explainer-short"),
    },
    Recipe {
        name: "explainer-ar",
        kind: "explainer, Arabic",
        seconds: 60,
        use_for: "The 60 s explainer in Modern Standard Arabic, right to left.",
        script: "The same beats as `explainer`, written in Arabic from the start and not translated word for word: Arabic carries about half the words of English in the same time. 8 scenes.",
        cards: "The same kinds as `explainer`. Lists and steps stand at the right, a wide diagram runs from the right, and sideways transitions follow the reading direction.",
        look: "Music `calm`, quiet. Transition `fade`. Captions `karaoke`. Background `night`.",
        adapt: &[
            "Write every card in Arabic words and use one digit style in the script and the cards.",
            ARABIC_VOICE,
            PICTURES,
        ],
        brief: brief!("explainer-ar"),
    },
    Recipe {
        name: "explainer-short-ar",
        kind: "explainer, Arabic",
        seconds: 30,
        use_for: "The 30 s phone explainer in Modern Standard Arabic, right to left.",
        script: "The same beats as `explainer-short`, about 50 Arabic words. 5 scenes.",
        cards: "One title, one `diagram`, one `list`, as in `explainer-short`.",
        look: "Music `calm`. Transition `rise`. Captions `karaoke`: Arabic words are long, and a whole phrase reads better than 1 to 3 big words. Background `forest`.",
        adapt: &[
            "Keep diagram labels to one or two short words: a box holds 18 characters.",
            ARABIC_VOICE,
            PICTURES,
        ],
        brief: brief!("explainer-short-ar"),
    },
    Recipe {
        name: "product-demo",
        kind: "product demo",
        seconds: 30,
        use_for: "Show what a product does and what it costs, for a page or a talk.",
        script: "Hook: the problem in the user's words. Beats: the product in one sentence, three things it does (each named), the plans. Ending: one action, said once, and the same action on the outro card. 5 scenes of 4 to 8 s.",
        cards: "A title with the product name, a `list` whose items are marked as the voice names them, a `table` of plans where the light moves from row to row, and an `outro` with the call to action.",
        look: "Music `pulse`, quiet. Transition `push`. Captions `karaoke`. Background `slate`, with the brand colour as `accent`.",
        adapt: &[
            "Replace the invented product and its plans. Add `\"logo\": {\"file\": \"logo.png\"}` and set `accent` to the brand colour (a light shade).",
            "Put a screen recording on each feature scene with `media`, and drop the card of a scene whose picture already shows the point.",
            "The outro adds 2.5 s after the voice: `duration` is the length of the narration alone.",
            VOICE,
        ],
        brief: brief!("product-demo"),
    },
    Recipe {
        name: "ad",
        kind: "ad",
        seconds: 15,
        use_for: "A 15 s vertical ad: one product, one promise, one action.",
        script: "Hook: the viewer's own problem in under 2 s. Beats: the product and what makes it different (three short points), the feeling after. Ending: where to get it, then the outro card. 4 scenes of 2 to 4 s.",
        cards: "A `list` of three points that spring in fast, one `title` for the promise, and the `outro`. No numbers: a number in an ad is a claim that needs proof.",
        look: "Music `pulse` with `cut_on: beat`, so cuts land on the beat. Transition `flash`. Captions `pop`, `large`. `sfx` on. A warm `accent`.",
        adapt: &[
            "For variants keep the scenes and change one thing per version: the hook, the three points or the outro text. `scriptvox batch` makes one video per row of a rows file.",
            "From a product page: take the name, three plain benefits and the action from the page by hand. ScriptVox does not read the page.",
            "The outro adds 2.5 s after the voice, so the narration is written for 12 s.",
            VOICE,
            PICTURES,
        ],
        brief: brief!("ad"),
    },
    Recipe {
        name: "news-brief",
        kind: "news brief",
        seconds: 30,
        use_for: "A short, neutral report of one event, with its sources named on screen.",
        script: "No hook line of opinion: the hook is the event. Beats: what happened, where and when in the first sentence; the key number; what comes next, in time order; who said so. Ending: the next date to watch, and the Sources card. 5 scenes of 4 to 8 s.",
        cards: "A `title` with the place, a `stat` for the one number, a `timeline` for what comes next, and `sources_card` on the last scene. No lower third: it needs footage of the person.",
        look: "Music `none`: a bed colours the news. Transition `cut`. Captions `karaoke`. Background `slate`.",
        adapt: &[
            PLACEHOLDER,
            "`scriptvox research \"<topic>\"` then `scriptvox write --from research.json --sources-card` can fill the facts and `sources` from real pages (they use your text row).",
            "The Sources card shows each `publisher` one time; on a tall frame the line holds about 30 characters.",
            VOICE,
            PICTURES,
        ],
        brief: brief!("news-brief"),
    },
    Recipe {
        name: "tutorial",
        kind: "tutorial",
        seconds: 40,
        use_for: "Teach a task at a keyboard, step by step, on a wide screen.",
        script: "Hook: the problem the viewer has now. Beats: the plan in three steps, then each step with the exact line to type and what it prints. Ending: the whole thing as one small script, and a caution. 6 scenes of 5 to 9 s.",
        cards: "`steps` for the plan (the spoken step lights up), a `terminal` for each command and its output, a `code` card for the script, and a `progress` bar to close.",
        look: "Music `calm`, quiet. Transition `wipe`. Captions `karaoke`, `small`, so the panel has the room. Background `slate`.",
        adapt: &[
            "A code or terminal card holds 12 lines of 44 characters. Show only the lines the voice talks about.",
            "A command the voice reads wrong goes in `say` (`{\"du\": \"D U\"}`): the captions keep the written form.",
            "Run every command yourself before the video says it works.",
            VOICE,
        ],
        brief: brief!("tutorial"),
    },
    Recipe {
        name: "data-story",
        kind: "data story",
        seconds: 40,
        use_for: "Tell what a small set of numbers means: a trend, a move and a comparison.",
        script: "Hook: the change in plain words. Beats: one chart per scene, each with one message; the voice says the trend and one or two numbers, the chart shows the rest. Ending: the reason or the lesson, with no new number, and the Sources card. 5 scenes of 6 to 9 s.",
        cards: "`bars` for the trend over years, a `number_line` whose marker travels from the old value to the new, a `table` where the light moves through the rows as they are named, a `title` to close, and `sources_card`.",
        look: "Music `calm`, quiet. Transition `dissolve`. Captions `karaoke`. Background `ocean`.",
        adapt: &[
            PLACEHOLDER,
            "One unit per chart. A table mark names the whole text of one cell, and its `on` is a word the voice says.",
            VOICE,
        ],
        brief: brief!("data-story"),
    },
    Recipe {
        name: "listicle",
        kind: "listicle, top 5",
        seconds: 30,
        use_for: "Five tips, places or things, counted down for a phone feed.",
        script: "Hook: the promise and the count. Beats: five items from 5 to 1, one short sentence each, the best one last. Ending: the list once more and a question to the viewer. 7 scenes of 3 to 5 s.",
        cards: "A `title` with the number on each item, and a `list` on the last scene that brings the items back as the voice names them.",
        look: "Music `pulse` with `cut_on: beat`. Transition `slide`. Captions `pop`. `sfx` on. A dark purple `background`, because the default one turns yellow under yellow captions.",
        adapt: &[
            "Keep each item to one sentence of about 12 words. A longer item is two scenes.",
            "The closing list uses the narration's own words in the same order, or its items show at the scene's start.",
            VOICE,
            PICTURES,
        ],
        brief: brief!("listicle"),
    },
    Recipe {
        name: "quote-short",
        kind: "quote or motivation short",
        seconds: 10,
        use_for: "One line to remember, as text on screen with music and no voice.",
        script: "Hook: none; the first line is the feeling the viewer has. Beats: the feeling, the line, one small action. 3 scenes of 3 to 5 s.",
        cards: "One `quote` card, with a highlight on its key words. No speaker name: the line is invented. A real quotation needs its speaker in `label` and a source.",
        look: "`tts: none` with captions `reveal`, so the words come at reading pace. Music `calm`. Transition `dissolve`. Background `night`.",
        adapt: &[
            "The quote text is at most 160 characters. The mark's `part` must be in the card exactly once.",
            "For a spoken version set `tts` and `voice`, and keep the pauses: the still beat after the line is the point.",
            PICTURES,
        ],
        brief: brief!("quote-short"),
    },
    Recipe {
        name: "micro-drama",
        kind: "micro-drama, dialogue",
        seconds: 25,
        use_for: "A tiny story told by two people who talk, for a phone feed.",
        script: "Hook: the conflict as a headline. Beats: the accusation, the reason, what is at stake, the turn (a fact that changes the meaning). Ending: one short line that answers the first. 7 scenes, one line of speech each, 2 to 6 s.",
        cards: "One `title` at the start. No other card: in a dialogue the faces and the words carry it.",
        look: "Two voices of one engine: the video's `voice`, and scene `voice` for the second speaker, whose words light up in light blue. Music `calm`, quiet. Transition `cut`. Background `night`.",
        adapt: &[
            "Keep it to two speakers: a third voice shares the second colour. Every speaker comes from one engine.",
            "A line is one breath: 5 to 14 words. Cut the story to one turn.",
            "A drama needs pictures: give each scene a `media` clip, or a `generate` shot with `characters` so the two people stay the same.",
            VOICE,
        ],
        brief: brief!("micro-drama"),
    },
];

fn brief_of(r: &Recipe) -> Value {
    // The files are checked by `tests/recipes.rs`; a broken one is a build fault, not a user's.
    serde_json::from_str(r.brief).unwrap_or(Value::Null)
}

fn row(r: &Recipe) -> Value {
    let b = brief_of(r);
    let voices: std::collections::BTreeSet<&str> = b["scenes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s["voice"].as_str())
        .chain(b["voice"].as_str())
        .collect();
    json!({
        "name": r.name,
        "kind": r.kind,
        "seconds": r.seconds,
        "aspect": b["aspect"],
        "lang": b["lang"],
        "voice": match b["tts"].as_str() {
            Some("none") => "none: text at reading pace".to_string(),
            tts => format!("{}, free and local ({} voice{})", tts.unwrap_or("default"), voices.len(), if voices.len() == 1 { "" } else { "s" }),
        },
        "scenes": b["scenes"].as_array().map_or(0, Vec::len),
        "use_for": r.use_for,
        "command": format!("scriptvox recipes show {} --json", r.name),
    })
}

/// The list of `scriptvox recipes`.
pub fn list() -> Value {
    ALL.iter().map(row).collect()
}

/// `scriptvox recipes show <name>`: the row, the brief and the notes. Err: the message for an unknown name.
pub fn show(name: &str) -> Result<Value, String> {
    let r = ALL.iter().find(|r| r.name == name).ok_or_else(|| {
        format!(
            "no recipe `{name}`. The recipes: {}",
            ALL.map(|r| r.name).join(", ")
        )
    })?;
    let mut v = row(r);
    v["shape"] = json!({"script": r.script, "cards": r.cards, "look": r.look});
    v["adapt"] = json!(r.adapt);
    v["brief"] = brief_of(r);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_recipe_has_a_brief_and_notes_and_an_unknown_name_lists_the_names() {
        for r in &ALL {
            let v = show(r.name).unwrap();
            assert!(v["brief"]["scenes"].is_array(), "{}", r.name);
            assert!(v["brief"]["tts"].is_string(), "{}: set tts", r.name);
            assert!(!r.adapt.is_empty() && r.adapt.len() <= 5, "{}", r.name);
        }
        let names: std::collections::HashSet<_> = ALL.iter().map(|r| r.name).collect();
        assert_eq!(names.len(), ALL.len());
        assert!(show("vlog").unwrap_err().contains("micro-drama"));
    }
}
