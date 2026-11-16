//! Brief `cut_on: "beat"`: scene boundaries move to the beat of the music bed, at most 0.25 s, and the captions
//! do not move. No network and no voice: `tts: none` reads at a pace.
mod common;
use serde_json::{json, Value};
use std::path::Path;

/// Three pictures and a click track: a 20 ms click every 0.5 s (120 BPM) for 30 s.
fn files(ff: &str, dir: &Path) {
    let run = |args: &[&str]| {
        let st = std::process::Command::new(ff)
            .current_dir(dir)
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .args(args)
            .status()
            .unwrap();
        assert!(st.success(), "{args:?}");
    };
    // Bright noise with one strong colour: not black, and the slow zoom on a still shows as motion.
    for (c, i) in [("red", 0), ("green", 1), ("blue", 2)] {
        let ch = |k: usize| {
            if k == i {
                "200+55*random(1)"
            } else {
                "60+60*random(1)"
            }
        };
        let geq = format!(
            "nullsrc=s=320x180,geq=r='{}':g='{}':b='{}'",
            ch(0),
            ch(1),
            ch(2)
        );
        run(&[&geq, "-frames:v", "1", &format!("{c}.png")]);
    }
    run(&[
        "aevalsrc='0.8*sin(2*PI*1000*t)*lt(mod(t,0.5),0.02)':s=48000:d=30",
        "click.wav",
    ]);
}

fn brief(music: &str, cut_on: bool) -> Value {
    let mut b = json!({
        "tts": "none", "lang": "en", "aspect": "16:9", "transition": "cut", "music": music,
        "scenes": [
            {"script": "The first scene has a red picture and nine words.", "media": "red.png"},
            {"script": "The second scene is green, and it is a little longer than the first.", "media": "green.png",
                "card": {"kind": "title", "text": "Green"}},
            {"script": "The third scene is blue and it ends the video.", "media": "blue.png"}
        ]
    });
    if cut_on {
        b["cut_on"] = json!("beat");
    }
    b
}

fn sv(ff: &str, dir: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let o = common::bin()
        .current_dir(dir)
        .env("SCRIPTVOX_FFMPEG", ff)
        .env("SCRIPTVOX_DATA", dir.join("no-data"))
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .env_remove("SCRIPTVOX_SELFHOST_KEY")
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    (
        o.status.code(),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

fn make(ff: &str, dir: &Path, name: &str, b: &Value) -> (Option<i32>, Value) {
    std::fs::write(dir.join(format!("{name}.json")), b.to_string()).unwrap();
    sv(
        ff,
        dir,
        &[
            "make",
            "--brief",
            &format!("{name}.json"),
            "--local-only",
            "--out",
            &format!("{name}.mp4"),
        ],
    )
}

/// The times where the picture changes colour: one pixel per frame at 30 fps.
fn picture_cuts(ff: &str, video: &Path) -> Vec<f64> {
    let o = std::process::Command::new(ff)
        .args(["-v", "error", "-i"])
        .arg(video)
        .args([
            "-vf",
            "scale=1:1:flags=area",
            "-pix_fmt",
            "rgb24",
            "-f",
            "rawvideo",
            "-",
        ])
        .output()
        .unwrap();
    assert!(o.status.success());
    let main = |p: &[u8; 3]| (0..3).max_by_key(|i| p[*i]).unwrap();
    let px: Vec<usize> = o.stdout.as_chunks::<3>().0.iter().map(main).collect();
    (1..px.len())
        .filter(|i| px[*i] != px[i - 1])
        .map(|i| i as f64 / 30.0)
        .collect()
}

#[test]
fn boundaries_move_to_the_clicks_and_the_captions_stay() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    files(&ff, d.path());
    let (code, plain) = make(&ff, d.path(), "plain", &brief("click.wav", false));
    assert_eq!(code, Some(0), "{plain}");
    assert!(plain.get("cut_on").is_none(), "{plain}");
    let (code, v) = make(&ff, d.path(), "beat", &brief("click.wav", true));
    assert_eq!(code, Some(0), "{v}");
    let plan =
        |n: &str| sv(&ff, d.path(), &["plan", "--brief", n, "--local-only"]).1["plan_id"].clone();
    assert!(plan("plain.json").is_string(), "a plan has an id");
    assert_ne!(
        plan("plain.json"),
        plan("beat.json"),
        "cut_on is part of the plan id"
    );
    let c = &v["cut_on"];
    assert!((c["bpm"].as_f64().unwrap() - 120.0).abs() < 2.0, "{v}");
    assert_eq!(c["limit_s"], 0.25);
    let rows = c["boundaries"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{v}");
    let before = picture_cuts(&ff, &d.path().join("plain.mp4"));
    let after = picture_cuts(&ff, &d.path().join("beat.mp4"));
    assert_eq!((before.len(), after.len()), (2, 2), "{before:?} {after:?}");
    for (i, row) in rows.iter().enumerate() {
        let (from, to) = (
            row["from"].as_f64().unwrap(),
            row["to"].as_f64().unwrap_or_else(|| panic!("{v}")),
        );
        // With a beat every 0.5 s, every boundary has one inside the limit.
        assert!((to - from).abs() <= 0.25 + 1e-9, "{row}");
        assert!(
            (row["moved_s"].as_f64().unwrap() - (to - from)).abs() < 0.002,
            "{row}"
        );
        // On a click: the grid is within 60 ms of the clicks at 0, 0.5, 1.0, ...
        let off = (to / 0.5 - (to / 0.5).round()).abs() * 0.5;
        assert!(off < 0.06, "{to} is {off} s from a click");
        // The picture: the plain video cuts at `from`, this one at `to`, to the frame.
        assert!((before[i] - from).abs() < 0.04, "{before:?} {from}");
        assert!((after[i] - to).abs() < 0.04, "{after:?} {to}");
    }
    assert_eq!(c["moved"], 2);
    // The card of scene 2 is on for its scene: it follows both cuts. Without the field it is on the word times.
    let card = |v: &Value| {
        let run = Path::new(".scriptvox/runs").join(v["run_id"].as_str().unwrap());
        let t = std::fs::read_to_string(d.path().join(run).join("cards.json")).unwrap();
        let c: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(c[0]["scene"], 2, "{c}");
        (
            c[0]["start"].as_f64().unwrap(),
            c[0]["end"].as_f64().unwrap(),
        )
    };
    let at = |k: &str| [rows[0][k].as_f64().unwrap(), rows[1][k].as_f64().unwrap()];
    let (start, end) = card(&v);
    assert!(
        (start - at("to")[0]).abs() < 0.002 && (end - at("to")[1]).abs() < 0.002,
        "{start} {end} {rows:?}"
    );
    let (start, end) = card(&plain);
    assert!(
        (start - at("from")[0]).abs() < 0.002 && (end - at("from")[1]).abs() < 0.002,
        "{start} {end} {rows:?}"
    );
    // The captions and the length are the same: only the picture moved.
    let srt = |n: &str| std::fs::read_to_string(d.path().join(n)).unwrap();
    assert_eq!(srt("plain.srt"), srt("beat.srt"));
    assert_eq!(
        plain["metrics"]["duration_s"], v["metrics"]["duration_s"],
        "{v}"
    );
}

#[test]
fn a_bed_with_no_beat_moves_nothing_and_says_so() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    files(&ff, d.path());
    let (code, plain) = make(&ff, d.path(), "plain", &brief("calm", false));
    assert_eq!(code, Some(0), "{plain}");
    let (code, v) = make(&ff, d.path(), "beat", &brief("calm", true));
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["cut_on"]["moved"], 0, "{v}");
    assert_eq!(v["cut_on"]["stayed"], 2, "{v}");
    assert_eq!(
        v["cut_on"]["boundaries"][0]["stayed"],
        "the music has no steady beat"
    );
    let warn = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "cut_on")
        .unwrap_or_else(|| panic!("{v}"));
    assert_eq!(warn["status"], "warn");
    assert!(warn["fix"].as_str().unwrap().contains("pulse"), "{v}");
    assert_eq!(
        picture_cuts(&ff, &d.path().join("plain.mp4")),
        picture_cuts(&ff, &d.path().join("beat.mp4"))
    );
}

#[test]
fn cut_on_is_refused_before_any_work_when_it_cannot_do_its_job() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    files(&ff, d.path());
    let mut wrong = brief("click.wav", true);
    wrong["cut_on"] = json!("bar");
    let mut no_music = brief("click.wav", true);
    no_music.as_object_mut().unwrap().remove("music");
    let mut no_scenes = brief("click.wav", true);
    no_scenes.as_object_mut().unwrap().remove("scenes");
    no_scenes["script"] = json!("One script and no scenes.");
    // Music from an address is fetched first (tests/fetch.rs); a search or a music row still comes too late.
    let mut late = brief("click.wav", true);
    late.as_object_mut().unwrap().remove("music");
    late["music_search"] = json!("drums");
    for (b, says, fix) in [
        (wrong, "unknown cut_on bar", "\"cut_on\": \"beat\""),
        (no_music, "needs a music bed", "pulse"),
        (no_scenes, "no scenes", "`scenes`"),
        (late, "before the voice is made", "https address"),
    ] {
        std::fs::write(d.path().join("b.json"), b.to_string()).unwrap();
        let (code, v) = sv(
            &ff,
            d.path(),
            &["plan", "--brief", "b.json", "--local-only"],
        );
        assert_eq!(code, Some(2), "{v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains(says),
            "{v}"
        );
        assert!(v["error"]["fix"].as_str().unwrap().contains(fix), "{v}");
    }
}
