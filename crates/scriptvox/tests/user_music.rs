//! The user's own music (docs/specs/user-music.md): any song from a local file, with a cut, under a voice, with no
//! words, on the beat, longer or shorter than the video, in the formats a user really gives. Each case is a real
//! `make` with the gates. The "song" is made by FFmpeg. No network.
mod common;
use serde_json::{json, Value};
use std::path::Path;

/// A 10 s "song": 4 s at 440 Hz, then 6 s at 1760 Hz, so a cut can be heard by its pitch.
const SONG: &str = "aevalsrc='0.5*sin(2*PI*if(lt(t,4),440,1760)*t)':s=44100:d=10";

fn song(ff: &str, dir: &Path, name: &str, more: &[&str]) {
    let args = [&["-f", "lavfi", "-i", SONG][..], more, &[name]].concat();
    common::ffmpeg(ff, dir, &args);
}

fn sv(ff: &str, dir: &Path, cmd: &str, name: &str, b: &Value) -> (Option<i32>, Value) {
    std::fs::write(dir.join(format!("{name}.json")), b.to_string()).unwrap();
    let mut c = common::bin();
    c.current_dir(dir)
        .env("SCRIPTVOX_FFMPEG", ff)
        .env("SCRIPTVOX_DATA", dir.join("no-data"))
        .env("SCRIPTVOX_CONFIG", dir.join("cfg"))
        .args([cmd, "--brief", &format!("{name}.json"), "--local-only"]);
    if cmd == "make" {
        c.args(["--out", &format!("{name}.mp4")]);
    }
    let o = c.arg("--json").output().unwrap();
    (
        o.status.code(),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

/// A good run: exit 0, no failed gate, and no line about rights in what the user reads.
fn made(ff: &str, dir: &Path, name: &str, b: &Value) -> Value {
    let (code, v) = sv(ff, dir, "make", name, b);
    assert_eq!(code, Some(0), "{name}: {v}");
    let gates = v["findings"].as_array().unwrap();
    assert!(gates.iter().all(|f| f["status"] != "fail"), "{name}: {v}");
    let said = format!("{} {} {}", v["findings"], v["next"], v["error"]).to_lowercase();
    for word in ["right to", "licen", "commercial", "copyright", "responsib"] {
        assert!(
            !said.contains(word),
            "{name}: a note about rights ({word}): {v}"
        );
    }
    v
}

/// The mean level in dB of a part of a file, in a band around `hz` (0: the whole sound).
fn level(ff: &str, file: &Path, from: f64, secs: f64, hz: u32) -> f64 {
    let band = match hz {
        0 => "volumedetect".to_string(),
        hz => format!("bandpass=f={hz}:width_type=h:w={},volumedetect", hz / 4),
    };
    let o = std::process::Command::new(ff)
        .args([
            "-hide_banner",
            "-nostdin",
            "-ss",
            &from.to_string(),
            "-t",
            &secs.to_string(),
            "-i",
        ])
        .arg(file)
        .args(["-vn", "-af", &band, "-f", "null", "-"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    let line = err
        .lines()
        .find(|l| l.contains("mean_volume"))
        .unwrap_or_else(|| panic!("{err}"));
    line.split("mean_volume:")
        .nth(1)
        .unwrap()
        .trim()
        .trim_end_matches(" dB")
        .parse()
        .unwrap()
}

fn length(ff: &str, file: &Path) -> f64 {
    common::probe(ff, file, "a:0", "format=duration")
        .parse()
        .unwrap()
}

fn lufs(v: &Value) -> f64 {
    let f = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "loudness")
        .unwrap();
    f["detail"]
        .as_str()
        .unwrap()
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn silence_s(v: &Value) -> f64 {
    let f = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["gate"] == "silence")
        .unwrap();
    assert_eq!(f["status"], "pass", "{v}");
    f["detail"]
        .as_str()
        .unwrap()
        .trim_start_matches("longest silence ")
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn no_words(music: &str, seconds: f64) -> Value {
    json!({"aspect": "16:9", "size": 720, "music": music, "scenes": [{"seconds": seconds}]})
}

/// (a) and (b): a wordless video with a cut of a local song. The cut as numbers and as clock text gives the same
/// part; the music runs the whole length, fades at the end, and the video lands at the loudness target.
#[test]
fn a_local_song_is_cut_by_numbers_or_a_clock_and_is_the_only_sound_of_a_video_with_no_words() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    song(&ff, d.path(), "song.mp3", &[]);
    let mut by_numbers = no_words("song.mp3", 5.0);
    by_numbers["music_from"] = json!(4);
    by_numbers["music_to"] = json!(9.5);
    let mut by_clock = by_numbers.clone();
    by_clock["music_from"] = json!("0:04");
    by_clock["music_to"] = json!("0:09.5");
    // The plan says what the mix does, and has no sentence about rights.
    let (code, plan) = sv(&ff, d.path(), "plan", "numbers", &by_numbers);
    assert_eq!(code, Some(0), "{plan}");
    let stage = plan["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["stage"] == "music")
        .unwrap()
        .clone();
    let said = stage["explain"].as_str().unwrap();
    assert!(
        said.contains("your file song.mp3, from 4 s to 9.5 s") && said.contains("-14 LUFS"),
        "{said}"
    );
    assert!(!said.to_lowercase().contains("right"), "{said}");
    assert_eq!(stage["network"], false);
    for (name, b) in [("numbers", &by_numbers), ("clock", &by_clock)] {
        let v = made(&ff, d.path(), name, b);
        let run = d
            .path()
            .join(".scriptvox/runs")
            .join(v["run_id"].as_str().unwrap());
        assert!(
            (length(&ff, &run.join("music-cut.wav")) - 5.5).abs() < 0.1,
            "{name}"
        );
        let video = d.path().join(format!("{name}.mp4"));
        assert!((length(&ff, &video) - 5.0).abs() < 0.1, "{name}");
        // The part from 4 s on is the high tone: the low tone of the first 4 s is not in the video.
        let (high, low) = (
            level(&ff, &video, 1.5, 2.0, 1760),
            level(&ff, &video, 1.5, 2.0, 440),
        );
        assert!(
            high - low > 20.0,
            "{name}: 1760 Hz {high} dB, 440 Hz {low} dB"
        );
        // The whole length, a fade over the last second, and the loudness target.
        assert!(silence_s(&v) < 0.5, "{name}: {v}");
        let (middle, end) = (
            level(&ff, &video, 2.0, 1.0, 0),
            level(&ff, &video, 4.7, 0.3, 0),
        );
        assert!(
            middle - end > 8.0,
            "{name}: the end is not faded: {middle} dB, then {end} dB"
        );
        assert!((lufs(&v) + 14.0).abs() < 1.5, "{name}: {v}");
    }
    // The same song with no cut starts with its low tone.
    let v = made(&ff, d.path(), "whole", &no_words("song.mp3", 3.0));
    assert!(v["run_id"].is_string());
    let video = d.path().join("whole.mp4");
    assert!(level(&ff, &video, 1.0, 1.5, 440) - level(&ff, &video, 1.0, 1.5, 1760) > 20.0);
}

/// (c): music under a voice. The voice is a recording (brief `narration`): a 300 Hz tone from 1 s to 3 s of 6 s.
/// The music goes lower while the voice speaks and the voice stays well above it.
#[test]
fn the_song_ducks_under_a_voice_and_the_voice_stays_clear() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    common::ffmpeg(
        &ff,
        d.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "aevalsrc='0.5*sin(2*PI*1760*t)':s=48000:d=10",
            "song.wav",
        ],
    );
    common::ffmpeg(
        &ff,
        d.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "aevalsrc='0.5*sin(2*PI*300*t)*between(t,1,3)':s=48000:d=6",
            "voice.wav",
        ],
    );
    let b = json!({"aspect": "16:9", "size": 720, "lang": "en", "music": "song.wav", "narration": "voice.wav",
        "captions": "karaoke", "script": "Dates are an old food, and people like them."});
    let (_, plan) = sv(&ff, d.path(), "plan", "duck", &b);
    let stage = plan["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["stage"] == "music")
        .unwrap()
        .clone();
    assert!(
        stage["explain"]
            .as_str()
            .unwrap()
            .contains("10 dB under the voice"),
        "{stage}"
    );
    let v = made(&ff, d.path(), "duck", &b);
    let video = d.path().join("duck.mp4");
    let music_under = level(&ff, &video, 1.5, 1.0, 1760);
    let music_alone = level(&ff, &video, 4.2, 1.0, 1760);
    let voice = level(&ff, &video, 1.5, 1.0, 300);
    assert!(
        music_alone - music_under > 3.0,
        "not ducked: {music_alone} dB alone, {music_under} dB under the voice: {v}"
    );
    assert!(
        voice - music_under > 10.0,
        "the voice is not clear: voice {voice} dB, music {music_under} dB"
    );
    assert!((lufs(&v) + 14.0).abs() < 1.5, "{v}");
}

/// (d): `cut_on: beat` with a local file: a click every 0.5 s, and scenes with `seconds` that are off the beat.
#[test]
fn the_cuts_follow_the_beat_of_a_local_song() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let clicks = "aevalsrc='0.8*sin(2*PI*1000*t)*lt(mod(t,0.5),0.02)':s=48000:d=30";
    common::ffmpeg(
        &ff,
        d.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            clicks,
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "clicks.m4a",
        ],
    );
    let b = json!({"aspect": "16:9", "size": 720, "music": "clicks.m4a", "cut_on": "beat", "transition": "cut",
        "scenes": [{"seconds": 2.2}, {"seconds": 2.1}, {"seconds": 2.0}]});
    let v = made(&ff, d.path(), "beat", &b);
    let c = &v["cut_on"];
    assert!((c["bpm"].as_f64().unwrap() - 120.0).abs() < 2.0, "{v}");
    for row in c["boundaries"].as_array().unwrap() {
        let to = row["to"].as_f64().unwrap();
        assert!(
            (to * 2.0 - (to * 2.0).round()).abs() < 0.12,
            "a cut off the beat: {row}"
        );
    }
}

/// (e): a song shorter than the video starts again (no gap of silence); a longer one stops with the video.
#[test]
fn a_short_song_starts_again_and_a_long_song_stops_with_the_video() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    common::ffmpeg(
        &ff,
        d.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "aevalsrc='0.5*sin(2*PI*440*t)':s=48000:d=2",
            "short.wav",
        ],
    );
    song(&ff, d.path(), "long.wav", &[]);
    let v = made(&ff, d.path(), "short", &no_words("short.wav", 7.0));
    let video = d.path().join("short.mp4");
    assert!((length(&ff, &video) - 7.0).abs() < 0.1);
    assert!(silence_s(&v) < 0.5, "{v}");
    // The fourth pass of the 2 s file, at 6 s, is as loud as the first.
    assert!((level(&ff, &video, 1.2, 0.6, 0) - level(&ff, &video, 5.2, 0.6, 0)).abs() < 1.5);
    let v = made(&ff, d.path(), "long", &no_words("long.wav", 3.0));
    assert!(
        (length(&ff, &d.path().join("long.mp4")) - 3.0).abs() < 0.1,
        "{v}"
    );
}

/// (f): the files a user really gives: mp3, m4a, wav in mono at 44.1 kHz, wav in stereo at 48 kHz, and a video file
/// for its sound.
#[test]
fn every_usual_kind_of_file_works_as_the_music() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    song(&ff, d.path(), "a.mp3", &["-ac", "2"]);
    song(&ff, d.path(), "b.m4a", &["-c:a", "aac"]);
    song(&ff, d.path(), "c.wav", &["-ac", "1", "-ar", "44100"]);
    song(&ff, d.path(), "d.wav", &["-ac", "2", "-ar", "48000"]);
    common::ffmpeg(
        &ff,
        d.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:r=25:d=10",
            "-f",
            "lavfi",
            "-i",
            SONG,
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "e.mp4",
        ],
    );
    for (n, file) in ["a.mp3", "b.m4a", "c.wav", "d.wav", "e.mp4"]
        .iter()
        .enumerate()
    {
        let v = made(&ff, d.path(), &format!("kind{n}"), &no_words(file, 3.0));
        let video = d.path().join(format!("kind{n}.mp4"));
        assert!((lufs(&v) + 14.0).abs() < 1.5, "{file}: {v}");
        assert!(silence_s(&v) < 0.5, "{file}: {v}");
        assert_eq!(
            common::probe(&ff, &video, "a:0", "stream=sample_rate"),
            "48000",
            "{file}"
        );
        assert_eq!(
            common::probe(&ff, &video, "a:0", "stream=channels"),
            "2",
            "{file}"
        );
        // The song's low tone is the sound of the video.
        assert!(
            level(&ff, &video, 1.0, 1.5, 440) - level(&ff, &video, 1.0, 1.5, 1760) > 20.0,
            "{file}"
        );
    }
}

/// A file that cannot be the music: the error names the file and has a fix line, at `plan`, before any work.
#[test]
fn a_broken_music_file_is_named_with_a_fix_line() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("words.mp3"), "this is not a song").unwrap();
    common::ffmpeg(
        &ff,
        d.path(),
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x180:r=25:d=2",
            "-an",
            "-pix_fmt",
            "yuv420p",
            "silent.mp4",
        ],
    );
    song(&ff, d.path(), "song.wav", &[]);
    let cut = |from: Value, to: Value| {
        let mut b = no_words("song.wav", 3.0);
        b["music_from"] = from;
        b["music_to"] = to;
        b
    };
    let mut bed = no_words("pulse", 3.0);
    bed["music_from"] = json!(1);
    for (cmd, b, name) in [
        ("plan", no_words("words.mp3", 3.0), "words.mp3"),
        ("plan", no_words("silent.mp4", 3.0), "silent.mp4"),
        ("plan", no_words("gone.mp3", 3.0), "gone.mp3"),
        (
            "plan",
            cut(json!(5), json!(2)),
            "music_to is after music_from",
        ),
        ("plan", cut(json!("soon"), Value::Null), "music_from"),
        ("plan", bed, "music_from and music_to cut a music file"),
        ("make", cut(json!(60), Value::Null), "song.wav"),
    ] {
        let (code, v) = sv(&ff, d.path(), cmd, "bad", &b);
        assert_eq!(code, Some(2), "{name}: {v}");
        assert!(
            v["error"]["message"].as_str().unwrap().contains(name),
            "{name}: {v}"
        );
        assert!(
            v["error"]["fix"].as_str().is_some_and(|f| !f.is_empty()),
            "{name}: {v}"
        );
    }
}

/// A file server on loopback that answers every request with `body`, and counts the requests.
fn serve(body: Vec<u8>) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{BufRead, BufReader, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            h.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).is_ok_and(|n| n > 2) {
                line.clear();
            }
            let mut w = s;
            let _ = write!(
                w,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = w.write_all(&body);
        }
    });
    (base, hits)
}

/// The music library (docs/specs/music-library.md). A track in the content store is used with no request: the music
/// is the sound of the video, the credits name the title, the composer, the source and the licence, and `export`
/// says the video may be used commercially. The store copy is put there by the test (the real file needs the network).
#[test]
fn a_library_track_from_the_store_is_the_music_and_its_source_goes_to_the_credits() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    song(&ff, d.path(), "seed.ogg", &["-c:a", "libvorbis"]);
    let t = scriptvox::music_lib::find("library:picnic").unwrap();
    let store = scriptvox::store::Cas::new(d.path().join("no-data").join("cache"));
    store
        .put(
            &scriptvox::music_lib::key(&t),
            "music.library",
            "1",
            &[("picnic.ogg", &d.path().join("seed.ogg"))],
            json!({}),
        )
        .unwrap();
    let (code, plan) = sv(
        &ff,
        d.path(),
        "plan",
        "lib",
        &no_words("library:picnic", 5.0),
    );
    assert_eq!(code, Some(0), "{plan}");
    let stage = plan.to_string();
    assert!(
        stage.contains("music.library") && stage.contains("Go to the Picnic"),
        "{plan}"
    );
    let v = made(&ff, d.path(), "lib", &no_words("library:picnic", 5.0));
    let out = d.path().join("lib.mp4");
    assert!((length(&ff, &out) - 5.0).abs() < 0.2, "{v}");
    assert!(
        level(&ff, &out, 0.5, 3.0, 440) > -40.0,
        "the track is the sound: {v}"
    );
    let credits = std::fs::read_to_string(d.path().join("lib.credits.txt")).unwrap();
    assert!(
        credits.contains("Go to the Picnic")
            && credits.contains("Loyalty Freak Music")
            && credits.contains("CC0"),
        "{credits}"
    );
    let run = v["run_id"].as_str().unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_DATA", d.path().join("no-data"))
        .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
        .args(["export", run, "--json"])
        .output()
        .unwrap();
    let ex: Value = serde_json::from_slice(&o.stdout).unwrap();
    let meta: Value = serde_json::from_slice(
        &std::fs::read(ex["artifacts"]["metadata"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(meta["commercial_ok"], true, "{meta}");
    // The copy for the mix is in ScriptVox's data folder, not at a fixed path of the project.
    assert!(d.path().join("no-data/music/picnic.ogg").is_file());
    assert!(!d.path().join(".scriptvox/music").exists());
}

/// A wrong name lists the right ones. With --local-only, a track that is not in the store asks for the network.
/// A file that does not match the pinned sha256 is refused and not kept.
#[test]
fn a_library_track_is_named_right_downloaded_only_with_the_network_and_checked_by_its_hash() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let (code, v) = sv(&ff, d.path(), "plan", "bad", &no_words("library:rock", 5.0));
    assert_eq!(code, Some(2), "{v}");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("picnic"),
        "{v}"
    );
    let (code, v) = sv(
        &ff,
        d.path(),
        "make",
        "off",
        &no_words("library:picnic", 5.0),
    );
    assert_eq!(code, Some(10), "{v}");
    assert_eq!(v["status"], "needs_input", "{v}");
    song(&ff, d.path(), "other.ogg", &["-c:a", "libvorbis"]);
    let (base, hits) = serve(std::fs::read(d.path().join("other.ogg")).unwrap());
    std::fs::write(
        d.path().join("swap.json"),
        no_words("library:picnic", 5.0).to_string(),
    )
    .unwrap();
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .env("SCRIPTVOX_DATA", d.path().join("no-data"))
        .env("SCRIPTVOX_CONFIG", d.path().join("cfg"))
        .env("SCRIPTVOX_MUSIC_LIBRARY_BASE", &base)
        .args([
            "make",
            "--brief",
            "swap.json",
            "--out",
            "swap.mp4",
            "--json",
        ])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(!v["ok"].as_bool().unwrap(), "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("changed at its source"),
        "{v}"
    );
    assert!(hits.load(std::sync::atomic::Ordering::SeqCst) >= 1);
    assert!(!d.path().join("no-data/music/picnic.ogg").exists());
    assert!(!d.path().join("swap.mp4").exists());
}
