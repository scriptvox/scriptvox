//! `setup image-local` and its row, with a stub `sd-cli` for the model. No real image here: docs/specs/image-local.md.

mod common;
use serde_json::Value;

#[cfg(unix)]
#[test]
fn local_image_is_opt_in_and_runs_one_process() {
    use scriptvox::provision as p;
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let data = d.path().join("data");
    std::env::set_var("SCRIPTVOX_DATA", &data);
    let run = |args: &[&str], extra: &[(&str, &str)]| -> (i32, Value) {
        let mut c = common::bin();
        c.current_dir(d.path())
            .env("SCRIPTVOX_DATA", &data)
            .arg("--json")
            .args(args);
        for (k, v) in extra {
            c.env(k, v);
        }
        let o = c.output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice(&o.stdout).unwrap(),
        )
    };
    let gen = |shot: &str, extra: &[(&str, &str)]| {
        run(
            &[
                "generate",
                "--model",
                "flux2-klein-4b-local",
                "--prompt",
                shot,
                "--aspect",
                "16:9",
            ],
            extra,
        )
    };

    let (code, v) = run(&["setup", "image-local", "--dry-run"], &[]);
    assert_eq!(code, 0, "{v}");
    assert!(
        v["metrics"]["download_bytes"].as_u64().unwrap() > 5_000_000_000,
        "{v}"
    );
    // The note is for the build of this system: only the Linux build was run (main.rs, `setup image-local`).
    let want = if cfg!(target_os = "linux") {
        "no CUDA pack"
    } else {
        "was not run on this system"
    };
    assert!(v["note"].as_str().unwrap().contains(want), "{v}");
    assert!(v["expected"]["tier"].is_string(), "{v}");
    assert_eq!(v["expected"]["measured"][0]["seconds"], 446, "{v}");
    assert!(!data.join("tools").exists(), "a dry run fetches nothing");

    let (code, v) = gen("a", &[]);
    assert_ne!(code, 0, "{v}");
    assert!(v.to_string().contains("setup image-local"), "{v}");
    let (_, v) = run(&["providers"], &[]);
    assert_eq!(v["image_local"]["installed"], false, "{v}");

    // The stub writes its arguments to the -o file, and dies as the memory limit would when the prompt says so.
    for n in p::IMAGE_PINS {
        let pin = p::pin(n).unwrap();
        let f = p::tool_dir(&pin).join(&pin.bins[0]);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, "#!/bin/sh\ncase \"$*\" in *oom*) kill -9 $$;; esac\nwhile [ $# -gt 0 ]; do [ \"$1\" = -o ] && o=$2; a=\"$a $1\"; shift; done\necho \"$a\" > \"$o\"\n").unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (_, v) = run(&["providers"], &[]);
    assert_eq!(v["image_local"]["installed"], true, "{v}");
    let row = v["generators"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["name"] == "flux2-klein-4b-local")
        .unwrap();
    assert_eq!(row["usd_per_s"], 0.0);
    assert_eq!(row["key_set"], true);
    // `tools` says what `providers` says: no charge, and no network.
    let (_, v) = run(&["tools"], &[]);
    let row = v["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "image.gen.flux2-klein-4b-local")
        .unwrap();
    assert_eq!(row["cost"], "free", "{row}");
    assert_eq!(row["network"], false, "{row}");

    let (code, v) = gen("ok", &[("SCRIPTVOX_SD_ARGS", "--vae-tiling")]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 0.0, "{v}");
    let args = std::fs::read_to_string(v["artifacts"]["image"].as_str().unwrap()).unwrap();
    for want in [
        "-W 1344 -H 768",
        "--steps 4",
        "--offload-to-cpu",
        "--vae-tiling",
    ] {
        assert!(args.contains(want), "{args}");
    }

    let (code, v) = gen("oom", &[]);
    assert_ne!(code, 0, "{v}");
    let text = v.to_string();
    assert!(
        text.contains("ran out of memory") && text.contains("SCRIPTVOX_SD_ARGS"),
        "{v}"
    );
}
