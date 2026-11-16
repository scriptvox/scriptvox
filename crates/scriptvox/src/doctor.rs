//! `scriptvox doctor`: a first-run check in Rust, under a second, with no Python
//! and no Node (PLAN-R1 §5). Idea from HyperFrames `doctor.ts` (Apache-2.0):
//! a list of named checks, each with a hint. ScriptVox adds `fix_safe`, licence
//! class, a real encoder probe, filesystem placement and TLS interception.

use crate::engine_ffmpeg::Ffmpeg;
use crate::envelope::Envelope;
use crate::exit::Exit;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Check {
    pub name: &'static str,
    /// ok, warn or fail.
    pub status: &'static str,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    /// True when an agent may run the fix without asking (for example a download of a pinned tool).
    pub fix_safe: bool,
}

fn ok(name: &'static str, detail: impl Into<String>) -> Check {
    Check {
        name,
        status: "ok",
        detail: detail.into(),
        fix: None,
        fix_safe: true,
    }
}

fn bad(
    name: &'static str,
    status: &'static str,
    detail: impl Into<String>,
    fix: impl Into<String>,
    fix_safe: bool,
) -> Check {
    Check {
        name,
        status,
        detail: detail.into(),
        fix: Some(fix.into()),
        fix_safe,
    }
}

/// The exit code for a set of checks (locked by a snapshot test).
pub fn exit_for(checks: &[Check]) -> Exit {
    let fails: Vec<&Check> = checks.iter().filter(|c| c.status == "fail").collect();
    if fails.iter().any(|c| !c.fix_safe) {
        Exit::NeedsInput
    } else if !fails.is_empty() {
        Exit::Error
    } else {
        Exit::Ok
    }
}

/// A TLS error from a test fetch: corporate interception or a blocked host.
pub fn classify_net_error(e: &str) -> &'static str {
    let l = e.to_lowercase();
    if [
        "certificate",
        "unknownissuer",
        "unknown issuer",
        "self signed",
        "self-signed",
        "invalid peer certificate",
        "badsignature",
    ]
    .iter()
    .any(|k| l.contains(k))
    {
        "tls_intercept"
    } else {
        "blocked"
    }
}

pub fn meminfo(key: &str) -> Option<u64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    s.lines()
        .find(|l| l.starts_with(key))?
        .split_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()
        .map(|kb| kb * 1024)
}

/// Name and memory of each NVIDIA card (`nvidia-smi`, which only reads). Empty when none is seen.
// ponytail: NVIDIA only; add AMD and Apple when a local row can use them (G006 answers that).
pub fn gpus() -> Value {
    let out = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => parse_gpus(&String::from_utf8_lossy(&o.stdout)),
        _ => json!([]),
    }
}

fn parse_gpus(csv: &str) -> Value {
    let cards: Vec<Value> = csv
        .lines()
        .filter_map(|l| {
            let (name, mb) = l.rsplit_once(',')?;
            let mb: u64 = mb.trim().parse().ok()?;
            Some(json!({"name": name.trim(), "memory_mb": mb}))
        })
        .collect();
    json!(cards)
}

/// `setup <what> --dry-run`: what would be downloaded, the room for it, and the cards found. Nothing is fetched.
/// The envelope for a failed download in any `setup`. A wrong checksum is not a network hiccup: never retry it blindly.
pub fn setup_failure(e: String) -> Envelope {
    if let Some((page, action)) = e
        .strip_prefix(crate::provision::GATED)
        .and_then(|r| r.split_once(' '))
    {
        let mut env = Envelope::fail("setup", Exit::NeedsInput, e.clone(), Some(action.into()));
        env.needs_input = vec![serde_json::json!({
            "field": "hf", "kind": "secret", "url": page, "human_action": format!("{page}: {action}"),
        })];
        return env;
    }
    if let Some((cause, fix)) = e
        .strip_prefix(crate::provision::STOP)
        .and_then(|r| r.split_once('|'))
    {
        return Envelope::fail(
            "setup",
            Exit::Error,
            cause.to_string(),
            Some(fix.to_string()),
        );
    }
    if e.contains("sha256") {
        return Envelope::fail_code(
            "setup",
            Exit::Error,
            "SVX_CHECKSUM",
            e,
            Some("The download does not match its pin. Report it; do not retry.".into()),
        );
    }
    if crate::provision::network_fault(&e) {
        return Envelope::fail(
            "setup",
            Exit::ProviderRetryable,
            e,
            Some("Check the network and run the same command again.".into()),
        );
    }
    // Everything else is a folder, a disk or an unpack on this machine: the network is not the cause.
    Envelope::fail(
        "setup",
        Exit::Error,
        e,
        Some(format!(
            "Check that {} can be written and has free space, or set SCRIPTVOX_DATA to another folder.",
            crate::provision::data_dir().display()
        )),
    )
}

pub fn setup_dry_run(pins: &[crate::provision::Pin]) -> Envelope {
    // A file already installed, or found by `models scan`, costs no download.
    let pins: Vec<&crate::provision::Pin> = pins
        .iter()
        .filter(|p| crate::provision::installed(p).is_none())
        .collect();
    let rows: Vec<Value> = pins
        .iter()
        .map(|p| json!({"name": p.name, "version": p.version, "bytes": p.size, "licence": p.licence, "url": p.url}))
        .collect();
    let bytes: u64 = pins.iter().map(|p| p.size).sum();
    let data = crate::provision::data_dir();
    // The data folder may not exist yet: ask about the nearest folder that does.
    let free = data.ancestors().find(|d| d.is_dir()).and_then(free_bytes);
    let mut env = Envelope::ok("setup")
        .with("downloads", json!(rows))
        .with("data_folder", json!(data))
        .with("gpus", gpus());
    env.metrics.insert("download_bytes".into(), json!(bytes));
    if let Some(free) = free {
        env.metrics.insert("disk_free_bytes".into(), json!(free));
        if free < bytes {
            env.findings.push(json!({"gate": "disk", "status": "warn",
                "detail": format!("{bytes} bytes to download, {free} free"),
                "fix": "Free some disk space, or set SCRIPTVOX_DATA to a folder on a larger disk."}));
        }
    }
    env
}

/// What `setup image-local` can promise on these cards. Only measured figures: no time is guessed for a card or a build
/// that did not run. `seconds_per_picture` is null when the time on this machine is unknown.
pub fn image_tier(gpus: &Value) -> Value {
    /// Card memory that the row's `--offload-to-cpu` used on the H100 (G006): 3,882 MiB.
    const CARD_MB: u64 = 3882;
    let best = gpus
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| g["memory_mb"].as_u64())
        .max();
    let (tier, detail) = match best {
        Some(mb) if mb >= CARD_MB => (
            "card",
            format!("A card with {mb} MB is at or above the 3,882 MB measured on the H100. Time on this build and this card is unknown."),
        ),
        Some(mb) => (
            "small_card",
            format!("A card with {mb} MB is below the 3,882 MB measured on the H100, so it may not fit. Time is unknown."),
        ),
        None => (
            "cpu",
            "No NVIDIA card was seen, so expect the CPU, unless another card works through Vulkan. The only CPU figure is 512x512 on 2 cores. A full-size picture on the CPU was not run.".to_string(),
        ),
    };
    json!({
        "tier": tier,
        "detail": detail,
        "seconds_per_picture": if tier == "cpu" { json!(446) } else { Value::Null },
        // The row's own sizes are larger: `SCRIPTVOX_SD_ARGS` made this one.
        "seconds_are_for_size": if tier == "cpu" { json!("512x512") } else { Value::Null },
        "measured": [
            {"where": "this pinned build, CPU only, 2 cores, 4 GB limit (6 Nov 2026)", "size": "512x512", "seconds": 446, "with_reference_seconds": 743},
            {"where": "own CUDA build, not this pinned build, one H100 NVL, weights quantised while they load (6 Nov 2026)", "size": "768x1344", "seconds": 44, "card_mb": CARD_MB},
        ],
    })
}

/// Free bytes on the filesystem of `path` (`df -Pk`).
fn free_bytes(path: &Path) -> Option<u64> {
    let o = std::process::Command::new("df")
        .args(["-Pk"])
        .arg(path)
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&o.stdout);
    s.lines()
        .nth(1)?
        .split_whitespace()
        .nth(3)?
        .parse::<u64>()
        .ok()
        .map(|kb| kb * 1024)
}

pub fn run(use_system: bool, network: bool) -> (Vec<Check>, Value) {
    let mut c = Vec::new();
    let gb = |b: u64| b as f64 / 1e9;

    // System.
    let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
    c.push(ok(
        "system",
        format!(
            "{} {}, {cpus} CPUs",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    ));
    if let Some(avail) = meminfo("MemAvailable:") {
        c.push(if avail < 1_000_000_000 {
            bad(
                "memory",
                "warn",
                format!("{:.1} GB available", gb(avail)),
                "Close other programs: a 1080p render needs about 1 GB.",
                false,
            )
        } else {
            ok("memory", format!("{:.1} GB available", gb(avail)))
        });
    }
    if let Some(free) = free_bytes(Path::new(".")) {
        c.push(if free < 2_000_000_000 {
            bad(
                "disk",
                "warn",
                format!("{:.1} GB free here", gb(free)),
                "Free 2 GB or more: chunks and the cache need space.",
                false,
            )
        } else {
            ok("disk", format!("{:.1} GB free here", gb(free)))
        });
    }
    if std::fs::read_to_string("/proc/version")
        .is_ok_and(|v| v.to_lowercase().contains("microsoft"))
    {
        let fs = crate::run::fs_type(Path::new(".")).unwrap_or_else(|| "unknown".into());
        let moved = crate::run::needs_rollback_journal(Path::new("."));
        let detail = format!(
            "WSL, project filesystem {fs}{}",
            if moved {
                " (state.db kept in the data folder, journal DELETE)"
            } else {
                ""
            }
        );
        c.push(if moved {
            bad(
                "wsl",
                "warn",
                detail,
                "For speed, keep projects on the Linux filesystem (~/...), not /mnt/c.",
                false,
            )
        } else {
            ok("wsl", detail)
        });
    }
    if Path::new("/.dockerenv").exists() {
        if let Some(shm) = free_bytes(Path::new("/dev/shm")) {
            if shm < 512_000_000 {
                c.push(bad(
                    "shm",
                    "warn",
                    format!("/dev/shm has {} MB", shm / 1_000_000),
                    "Start the container with --shm-size=1g.",
                    false,
                ));
            }
        }
    }

    // FFmpeg.
    let mut ffmpeg_ok = false;
    match Ffmpeg::locate(use_system, false) {
        Err(_) => c.push(bad(
            "ffmpeg",
            "fail",
            "FFmpeg was not found",
            "scriptvox setup",
            true,
        )),
        Ok(ff) => {
            let class = crate::provision::ffmpeg_class(&ff.ffmpeg).unwrap_or("unknown");
            let missing = ff.missing_filters();
            match ff.pick_encoder() {
                Ok(_) if !missing.is_empty() => c.push(bad(
                    "ffmpeg",
                    "fail",
                    format!(
                        "{} has no filter {}: captions, cards or the checks cannot run with it",
                        ff.ffmpeg.display(),
                        missing.join(", ")
                    ),
                    "scriptvox setup (the pinned build), or a full FFmpeg build with libass",
                    true,
                )),
                Ok((enc, _)) if class != "nonfree" => {
                    ffmpeg_ok = true;
                    c.push(ok(
                        "ffmpeg",
                        format!(
                            "{} ({class}), encoder {}, ffprobe {}",
                            ff.ffmpeg.display(),
                            enc.name,
                            ff.ffprobe.display()
                        ),
                    ));
                }
                Ok(_) => c.push(bad(
                    "ffmpeg",
                    "fail",
                    format!("{} is a nonfree build", ff.ffmpeg.display()),
                    "scriptvox setup (the pinned gpl build)",
                    true,
                )),
                Err(e) => c.push(bad(
                    "ffmpeg",
                    "fail",
                    e,
                    "scriptvox setup (the pinned build has libx264)",
                    true,
                )),
            }
        }
    }

    // The user's own rows: a bad block costs only itself, and this says which.
    let problems = crate::generators::problems();
    if !problems.is_empty() {
        c.push(bad(
            "generators",
            "warn",
            problems.join("; "),
            format!(
                "Correct or remove these blocks in {}.",
                crate::keys::overlay_file().display()
            ),
            false,
        ));
    }

    // Fonts: the embedded fonts parse and measure.
    let fonts_ok = crate::template::FONTS.iter().all(|(_, _, b)| {
        crate::captions::Measurer::new(b.to_vec(), 72.0)
            .is_some_and(|m| m.width("ScriptVox مرحبا") > 0.0)
    });
    c.push(if fonts_ok {
        ok("fonts", "Noto Sans and Noto Sans Arabic embedded")
    } else {
        bad(
            "fonts",
            "fail",
            "an embedded font does not parse",
            "Reinstall scriptvox.",
            false,
        )
    });

    // uv: needed for voices (the worker).
    let uv = std::env::var_os("SCRIPTVOX_UV")
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(|| {
            crate::provision::pin("uv")
                .and_then(|p| crate::provision::installed(&p))
                .map(|b| b[0].display().to_string())
        })
        .or_else(|| {
            std::env::var_os("PATH")
                .and_then(|p| {
                    std::env::split_paths(&p)
                        .map(|d| d.join(if cfg!(windows) { "uv.exe" } else { "uv" }))
                        .find(|p| p.is_file())
                })
                .map(|p| p.display().to_string())
        });
    let uv_ok = uv.is_some();
    c.push(match uv {
        Some(p) => ok("uv", p),
        None => bad(
            "uv",
            "warn",
            "uv was not found (needed for voices)",
            "scriptvox setup",
            true,
        ),
    });

    // The installed Claude skill must match this binary (an RM idea).
    if let Some(v) = crate::skill::installed_version() {
        c.push(if v == crate::skill::VERSION {
            ok("skill", format!("Claude skill {v}"))
        } else {
            bad(
                "skill",
                "warn",
                format!(
                    "the installed Claude skill is {v}, the binary is {}",
                    crate::skill::VERSION
                ),
                "scriptvox skills install",
                true,
            )
        });
    }

    // Keys: by name and source only.
    let keys: Vec<Value> = crate::keys::all()
        .iter()
        .map(|(p, var)| json!({"provider": p, "env": var, "source": crate::keys::resolve(p).map_or("none", |(_, s)| s)}))
        .collect();
    let set = keys.iter().filter(|k| k["source"] != "none").count();
    c.push(ok(
        "keys",
        format!("{set} of {} provider keys set", keys.len()),
    ));
    if let Some(e) = crate::keys::file_error() {
        c.push(bad(
            "keys_file",
            "warn",
            e,
            "Until it is corrected, every key in that file counts as not set.",
            false,
        ));
    }

    // Network and TLS.
    if network {
        for host in ["https://github.com", "https://speech.platform.bing.com"] {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(std::time::Duration::from_secs(6)))
                .build()
                .into();
            let name = if host.contains("github") {
                "net:github"
            } else {
                "net:edge-tts"
            };
            c.push(match agent.head(host).call() {
                Ok(_) | Err(ureq::Error::StatusCode(_)) => ok(name, format!("{host} reachable")),
                Err(e) => {
                    let kind = classify_net_error(&e.to_string());
                    let fix = if kind == "tls_intercept" {
                        "A proxy replaces TLS certificates: set SSL_CERT_FILE to your company CA bundle."
                    } else {
                        "Check the network or the proxy (HTTPS_PROXY)."
                    };
                    bad(name, "warn", format!("{kind}: {e}"), fix, false)
                }
            });
        }
    }

    let mut ready = Vec::new();
    if ffmpeg_ok {
        ready.push("local");
        if uv_ok {
            ready.push("hybrid");
        }
        if uv_ok && crate::keys::resolve("elevenlabs").is_some() {
            ready.push("cloud");
        }
    }
    let extra = json!({"mode_ready": ready, "keys": keys, "gpus": gpus(),
        "speech_local": {"installed": crate::provision::whisper_ready(), "arabic_installed": crate::provision::whisper_arabic_ready(), "arabic_add": "scriptvox setup speech-local --arabic"},
        "note": "local renders work offline; `scriptvox setup voice-local` adds a local English voice"});
    (c, extra)
}

/// Replaces the home folder and the user name, for output pasted into an issue.
pub fn redact_paths(text: &str) -> String {
    let mut t = text.to_string();
    if let Some(home) = std::env::var_os("HOME")
        .map(|h| h.to_string_lossy().into_owned())
        .filter(|h| h.len() > 1)
    {
        t = t.replace(&home, "~");
    }
    if let Some(user) = std::env::var_os("USER")
        .map(|u| u.to_string_lossy().into_owned())
        .filter(|u| u.len() > 2)
    {
        t = t.replace(&user, "<user>");
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gated_download_asks_the_user_and_exits_10() {
        let e = setup_failure(format!(
            "{}https://huggingface.co/o/m Open the page.",
            crate::provision::GATED
        ));
        assert_eq!(e.status, "needs_input");
        assert_eq!(e.needs_input[0]["url"], "https://huggingface.co/o/m");
        assert_eq!(e.needs_input[0]["field"], "hf");
        assert_eq!(e.exit, Exit::NeedsInput);
    }

    #[test]
    fn image_tier_gives_a_time_only_where_it_was_measured() {
        let t = |v: Value| image_tier(&v);
        let cpu = t(json!([]));
        assert_eq!(cpu["tier"], "cpu");
        assert_eq!(cpu["seconds_per_picture"], 446);
        let card = t(json!([{"name": "a", "memory_mb": 1024}, {"name": "b", "memory_mb": 8192}]));
        assert_eq!(card["tier"], "card", "the largest card counts");
        assert!(card["seconds_per_picture"].is_null());
        let small = t(json!([{"name": "a", "memory_mb": 2048}]));
        assert_eq!(small["tier"], "small_card");
        assert!(small["seconds_per_picture"].is_null());
    }

    #[test]
    fn gpu_lines_parse_and_noise_is_skipped() {
        let v = parse_gpus("NVIDIA H100 NVL, 95830\nNo devices\nCard, with comma, 8192\n");
        assert_eq!(v[0]["memory_mb"], 95830);
        assert_eq!(v[1]["name"], "Card, with comma");
        assert_eq!(v.as_array().unwrap().len(), 2);
    }

    #[test]
    fn exit_codes_follow_fix_safe() {
        let table: Vec<String> = [
            ("all ok", vec![ok("a", "")]),
            ("warn only", vec![bad("a", "warn", "", "x", false)]),
            (
                "fail, safe fix",
                vec![bad("a", "fail", "", "scriptvox setup", true)],
            ),
            (
                "fail, unsafe fix",
                vec![bad("a", "fail", "", "apt install x", false)],
            ),
            (
                "both fails",
                vec![
                    bad("a", "fail", "", "x", true),
                    bad("b", "fail", "", "y", false),
                ],
            ),
        ]
        .iter()
        .map(|(n, cs)| {
            let e = exit_for(cs);
            format!("{n}: {} {}", e.code(), e.name())
        })
        .collect();
        insta::assert_snapshot!(table.join("\n"));
    }

    #[test]
    fn tls_interception_is_not_blocked() {
        assert_eq!(
            classify_net_error("io: invalid peer certificate: UnknownIssuer"),
            "tls_intercept"
        );
        assert_eq!(
            classify_net_error("Connection refused (os error 111)"),
            "blocked"
        );
        assert_eq!(
            classify_net_error("dns error: failed to lookup address"),
            "blocked"
        );
    }
}
