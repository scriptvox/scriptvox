//! Receipt, embedded recipe, explain, replay and remake (RVX-030).
#![cfg(unix)]
mod common;
use common::*;
use serde_json::Value;
use std::path::Path;

fn out(o: std::process::Output) -> (i32, Value) {
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

fn comment(ff: &str, video: &str) -> String {
    let o = std::process::Command::new(Path::new(ff).with_file_name("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_entries",
            "format_tags=comment",
            "-of",
            "default=nw=1:nk=1",
            video,
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).into_owned()
}

const KEY: &str = "pexels-never-embedded-0123456789";

/// A 1 s MP4 whose comment tag is this recipe: a file that scriptvox did not make.
fn crafted(ff: &str, file: &Path, recipe: &Value) {
    let st = std::process::Command::new(ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=64x64:d=1",
            "-metadata",
        ])
        .arg(format!("comment={recipe}"))
        .arg(file)
        .status()
        .unwrap();
    assert!(st.success());
}

/// The run folders of a project.
fn runs(e: &Env) -> usize {
    std::fs::read_dir(e.dir.path().join(".scriptvox/runs")).map_or(0, |d| d.count())
}

#[test]
fn recipe_is_embedded_without_keys_and_replays_from_cache() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    with_media(&e);
    std::fs::write(
        e.dir.path().join("s.txt"),
        "Replay me from the cache. Nothing new is fetched.",
    )
    .unwrap();
    let (code, v) = out(e
        .cmd("ok")
        .env("PEXELS_API_KEY", KEY)
        .args([
            "make", "--script", "s.txt", "--media", "media", "--music", "calm", "--json",
        ])
        .output()
        .unwrap());
    assert_eq!(code, 0, "{v}");
    let video = v["artifacts"]["video"].as_str().unwrap().to_string();
    let c = comment(&ff, &video);
    assert!(
        c.contains("Replay me from the cache"),
        "the script is in the recipe"
    );
    assert!(!c.contains(KEY), "no key in the metadata");
    assert!(Path::new(v["artifacts"]["receipt"].as_str().unwrap()).is_file());
    let (code, x) = out(e
        .cmd("ok")
        .args(["explain", &video, "--json"])
        .output()
        .unwrap());
    assert_eq!(code, 0, "{x}");
    assert_eq!(x["recipe"]["request"]["music"], "calm");
    // STUB_MODE=flaky fails any new voice call: exit 0 proves a cache-only rebuild.
    let (code, r) = out(e
        .cmd("flaky")
        .args(["replay", &video, "--out", "again.mp4", "--json"])
        .output()
        .unwrap());
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["command"], "replay");
}

#[test]
fn replay_on_a_cold_cache_is_a_miss_not_a_call() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("Cold cache test.", "ok", &[]);
    assert_eq!(code, 0, "{v}");
    let video = v["artifacts"]["video"].as_str().unwrap().to_string();
    let (code, r) = out(e
        .cmd("ok")
        .env("SCRIPTVOX_DATA", e.dir.path().join("cold"))
        .args(["replay", &video, "--json"])
        .output()
        .unwrap());
    assert_eq!(code, 1, "{r}");
    assert_eq!(r["error"]["code"], "SVX_CACHE_MISS");
    // remake may call providers: a free recipe needs no approval.
    let (code, r) = out(e
        .cmd("ok")
        .env("SCRIPTVOX_DATA", e.dir.path().join("cold"))
        .args(["remake", &video, "--out", "re.mp4", "--json"])
        .output()
        .unwrap());
    assert_eq!(code, 0, "{r}");
}

#[test]
fn no_embed_leaves_no_recipe() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("Private recipe.", "ok", &["--no-embed"]);
    assert_eq!(code, 0, "{v}");
    let video = v["artifacts"]["video"].as_str().unwrap();
    assert_eq!(comment(&ff, video).trim(), "");
    let (code, _) = out(e
        .cmd("ok")
        .args(["explain", video, "--json"])
        .output()
        .unwrap());
    assert_eq!(code, 1);
}

#[test]
fn remake_refuses_a_crafted_recipe_with_a_bad_stock_pick() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    // An MP4 whose comment tag says: write to an absolute path, fetch an internal URL.
    let recipe = serde_json::json!({"scriptvox": "0.0.1", "plan_id": "p_x", "request": {
        "script": "hi there", "lang": "en", "tts": null, "voice": null, "local_only": false, "out": null, "duration": null,
        "aspect": "9:16", "media": null, "stock": "x", "music": null, "run_id": null, "use_system": false, "commercial": false,
        "approve": null, "max_spend": null, "plan_only": false, "idempotency_key": null,
        "stock_picks": [{"provider": "/tmp/scriptvox-evil", "id": "1", "url": "http://169.254.169.254/latest", "credit": "x", "page": "x"}]}});
    let bad = p.join("evil.mp4");
    crafted(&ff, &bad, &recipe);
    let (code, v) = out(e
        .cmd("ok")
        .args(["remake", bad.to_str().unwrap(), "--json"])
        .output()
        .unwrap());
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["error"]["code"], "SVX_BAD_RECIPE");
    assert!(!std::path::Path::new("/tmp/scriptvox-evil_1.mp4").exists());
}

/// robustness-4: with no FFmpeg the fix says so, not that the file has no recipe.
#[test]
fn explain_with_no_ffmpeg_says_ffmpeg_is_missing() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("s.txt"), "x").unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .env("PATH", "")
        .env("SCRIPTVOX_DATA", d.path().join("data"))
        .env_remove("SCRIPTVOX_FFMPEG")
        .args(["explain", "s.txt", "--json"])
        .output()
        .unwrap();
    let (code, v) = out(o);
    assert_eq!(code, 1, "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("Install FFmpeg"),
        "{v}"
    );
}

/// docs/specs/remake-detach.md: `remake --detach` answers at once with the run id, and `wait` gives the video.
#[test]
fn remake_detach_answers_running_and_wait_gives_the_video() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("Remake me in the background.", "ok", &[]);
    assert_eq!(code, 0, "{v}");
    let video = v["artifacts"]["video"].as_str().unwrap().to_string();
    let t0 = std::time::Instant::now();
    let (code, r) = out(e
        .cmd("ok")
        .args(["remake", &video, "--out", "again.mp4", "--detach", "--json"])
        .output()
        .unwrap());
    assert_eq!(code, 0, "{r}");
    assert!(
        t0.elapsed() < std::time::Duration::from_secs(2),
        "remake --detach returns at once"
    );
    assert_eq!(r["status"], "running", "{r}");
    assert_eq!(r["command"], "remake", "{r}");
    let id = r["run_id"].as_str().unwrap().to_string();
    assert_eq!(r["next"][0], format!("scriptvox wait {id}"), "{r}");
    let (code, w) = out(e.cmd("ok").args(["wait", &id, "--json"]).output().unwrap());
    assert_eq!(code, 0, "{w}");
    assert!(
        w["artifacts"]["video"]
            .as_str()
            .unwrap()
            .ends_with("again.mp4"),
        "{w}"
    );
    assert!(e.dir.path().join("again.mp4").is_file());
    let (code, s) = out(e
        .cmd("ok")
        .args(["status", &id, "--json"])
        .output()
        .unwrap());
    assert_eq!(code, 0, "{s}");
}

/// The same detached call twice is one run: a retry finds it, running or finished.
#[test]
fn the_same_remake_detach_twice_is_one_run() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let (code, v) = e.make("One remake. One run.", "ok", &[]);
    assert_eq!(code, 0, "{v}");
    let video = v["artifacts"]["video"].as_str().unwrap().to_string();
    let before = runs(&e);
    let start = || {
        out(e
            .cmd("ok")
            .args(["remake", &video, "--out", "again.mp4", "--detach", "--json"])
            .output()
            .unwrap())
    };
    let (code, a) = start();
    assert_eq!(code, 0, "{a}");
    let (code, b) = start();
    assert_eq!(code, 0, "{b}");
    assert_eq!(a["run_id"], b["run_id"], "{a} {b}");
    let id = a["run_id"].as_str().unwrap().to_string();
    let (code, w) = out(e.cmd("ok").args(["wait", &id, "--json"]).output().unwrap());
    assert_eq!(code, 0, "{w}");
    // After it finished too: the stored result answers, and nothing runs again.
    let (code, c) = start();
    assert_eq!(code, 0, "{c}");
    assert_eq!(c["run_id"], id.as_str(), "{c}");
    assert_eq!(c["idempotent"], true, "{c}");
    assert_eq!(runs(&e), before + 1, "one run folder for three calls");
}

/// A recipe is text from a file. Its checks run before the supervisor starts: a refused recipe leaves no run.
#[test]
fn remake_detach_refuses_a_bad_recipe_before_any_run_starts() {
    let Some(ff) = ready() else { return };
    let e = Env::new(&ff);
    let p = e.dir.path();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("private.wav");
    std::fs::copy(p.join("voice.mp3"), &secret).unwrap();
    let request = |extra: Value| {
        let mut r = serde_json::json!({
            "script": "hi there", "lang": "en", "tts": null, "voice": null, "local_only": false, "out": null,
            "duration": null, "aspect": "9:16", "media": null, "stock": null, "music": null, "run_id": null,
            "use_system": false, "commercial": false, "approve": null, "max_spend": null, "plan_only": false,
            "idempotency_key": null});
        r.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::json!({"scriptvox": "0.0.1", "plan_id": "p_x", "request": r})
    };
    let cases = [
        // A file outside the project as the narration: it would be the sound of the new video.
        (
            request(serde_json::json!({"narration": secret})),
            1,
            "SVX_PATH_OUTSIDE_ROOT",
        ),
        // A stock pick that names a path as its provider and an internal URL.
        (
            request(
                serde_json::json!({"stock": "x", "stock_picks": [{"provider": "/tmp/scriptvox-evil",
                "id": "1", "url": "http://169.254.169.254/latest", "credit": "x", "page": "x"}]}),
            ),
            2,
            "SVX_BAD_RECIPE",
        ),
        // Not a request at all.
        (
            serde_json::json!({"scriptvox": "0.0.1", "plan_id": "p_x", "request": {"script": 7}}),
            1,
            "SVX_ERROR",
        ),
    ];
    for (i, (recipe, exit, error)) in cases.iter().enumerate() {
        let bad = p.join(format!("bad{i}.mp4"));
        crafted(&ff, &bad, recipe);
        let (code, v) = out(e
            .cmd("ok")
            .args(["remake", bad.to_str().unwrap(), "--detach", "--json"])
            .output()
            .unwrap());
        assert_eq!(code, *exit, "{v}");
        assert_eq!(v["error"]["code"], *error, "{v}");
        assert_eq!(v["command"], "remake", "{v}");
        assert!(v["run_id"].is_null(), "no run: {v}");
        assert_eq!(runs(&e), 0, "no run folder: {v}");
    }
    // A file with no recipe, and a file that is not there.
    std::fs::write(p.join("plain.mp4"), "x").unwrap();
    for (file, exit) in [("plain.mp4", 1), ("nope.mp4", 2)] {
        let (code, v) = out(e
            .cmd("ok")
            .args(["remake", file, "--detach", "--json"])
            .output()
            .unwrap());
        assert_eq!(code, exit, "{v}");
        assert_eq!(runs(&e), 0, "no run folder: {v}");
    }
}
