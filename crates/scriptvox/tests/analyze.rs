//! `scriptvox analyze` on a test video with one hard cut.
mod common;
use serde_json::Value;

#[test]
fn finds_the_cut_and_reports_pace() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let st = std::process::Command::new(&ff)
        .current_dir(d.path())
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=360x640:rate=24:d=3",
        ])
        .args([
            "-f",
            "lavfi",
            "-i",
            "mandelbrot=size=360x640:rate=24",
            "-f",
            "lavfi",
            "-i",
            "sine=f=300:d=6",
        ])
        .args([
            "-filter_complex",
            "[1:v]trim=duration=3,setpts=PTS-STARTPTS[b];[0:v][b]concat=n=2:v=1:a=0[v]",
        ])
        .args([
            "-map",
            "[v]",
            "-map",
            "2:a",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            "-y",
            "in.mp4",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    let o = common::bin()
        .current_dir(d.path())
        .env("SCRIPTVOX_FFMPEG", &ff)
        .args(["analyze", "in.mp4", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert_eq!(v["metrics"]["shots"], 2, "{v}");
    let cut = v["shot_list"][1]["start"].as_f64().unwrap();
    assert!((cut - 3.0).abs() <= 0.3, "cut at {cut}");
    assert!(std::path::Path::new(v["artifacts"]["shots_image"].as_str().unwrap()).is_file());
}

#[test]
fn beats_gives_the_tempo_of_a_click_track_and_none_for_a_tone() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let make = |name: &str, sound: &str| {
        let st = std::process::Command::new(&ff)
            .current_dir(d.path())
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=25:d=12",
            ])
            .args(["-f", "lavfi", "-i", sound])
            .args([
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-shortest",
                "-y",
                name,
            ])
            .status()
            .unwrap();
        assert!(st.success());
    };
    // A 30 ms click every 0.5 s: 120 beats a minute.
    make(
        "click.mp4",
        "aevalsrc='if(lt(mod(t,0.5),0.03),sin(2*PI*1000*t),0)':d=12:s=44100",
    );
    make("tone.mp4", "sine=f=300:d=12");
    let run = |file: &str, extra: &[&str]| {
        let o = common::bin()
            .current_dir(d.path())
            .env("SCRIPTVOX_FFMPEG", &ff)
            .args(["analyze", file, "--json"])
            .args(extra)
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        (o.status.code(), v)
    };
    let (code, v) = run("click.mp4", &["--beats"]);
    assert_eq!(code, Some(0), "{v}");
    let b = &v["beats"];
    assert!((b["bpm"].as_f64().unwrap() - 120.0).abs() < 1.5, "{b}");
    assert_eq!(b["steady"], true, "{b}");
    let times: Vec<f64> = b["times"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_f64().unwrap())
        .collect();
    assert!(times.len() >= 20, "{b}");
    for t in times.iter().filter(|t| **t < 10.0) {
        let off = t.rem_euclid(0.5);
        assert!(off.min(0.5 - off) < 0.04, "beat at {t}");
    }
    // A steady tone has no beat, and the result says so.
    let (code, v) = run("tone.mp4", &["--beats"]);
    assert_eq!(code, Some(0), "{v}");
    assert_eq!(v["beats"]["steady"], false, "{v}");
    // Without the flag the result has no `beats`.
    let (_, v) = run("click.mp4", &[]);
    assert!(v["beats"].is_null(), "{v}");
}
