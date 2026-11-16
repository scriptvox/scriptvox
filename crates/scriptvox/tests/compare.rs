//! `scriptvox compare` on generated clips.
mod common;
use serde_json::Value;
use std::path::Path;

fn clip(ff: &str, dir: &Path, name: &str, filter: &str, size: &str, secs: &str) {
    let st = std::process::Command::new(ff)
        .current_dir(dir)
        .args(["-v", "error", "-f", "lavfi", "-i"])
        .arg(format!("testsrc2=size={size}:rate=25:d={secs}"))
        .args([
            "-vf", filter, "-c:v", "libx264", "-pix_fmt", "yuv420p", "-y", name,
        ])
        .status()
        .unwrap();
    assert!(st.success());
}

fn compare(dir: &Path, a: &str, b: &str) -> Value {
    let o = common::bin()
        .current_dir(dir)
        .args(["compare", a, b, "--json"])
        .output()
        .unwrap();
    serde_json::from_slice(&o.stdout).unwrap()
}

#[test]
fn compare_names_the_times_that_differ() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    clip(&ff, p, "a.mp4", "null", "320x180", "4");
    clip(&ff, p, "copy.mp4", "null", "320x180", "4");
    // A white box from 1 s to 2 s only.
    let boxed = "drawbox=x=0:y=0:w=320:h=180:c=white:t=fill:enable='between(t,1,2)'";
    clip(&ff, p, "box.mp4", boxed, "320x180", "4");
    clip(&ff, p, "short.mp4", "null", "320x180", "2");
    clip(&ff, p, "wide.mp4", "null", "640x360", "4");

    let v = compare(p, "a.mp4", "copy.mp4");
    assert_eq!(v["same"], true, "{v}");
    assert_eq!(v["stretches"], 0);

    let v = compare(p, "a.mp4", "box.mp4");
    assert_eq!(v["same"], false, "{v}");
    assert_eq!(v["stretches"], 1);
    let s = &v["differs"][0];
    let (start, end) = (s["start"].as_f64().unwrap(), s["end"].as_f64().unwrap());
    assert!((start - 1.0).abs() < 0.1 && (end - 2.0).abs() < 0.1, "{v}");
    assert!(v["not_checked"][0].as_str().unwrap().starts_with("sound"));

    let v = compare(p, "a.mp4", "short.mp4");
    assert_eq!(v["same"], false, "{v}");
    assert!(v["findings"][0]["gate"] == "length", "{v}");

    let v = compare(p, "a.mp4", "wide.mp4");
    assert!(v["findings"][0]["gate"] == "size", "{v}");

    let o = common::bin()
        .current_dir(p)
        .args(["compare", "a.mp4", "nope.mp4", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
}
