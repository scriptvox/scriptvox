//! FFmpeg as a subprocess (PLAN-R1 §5). Rust builds the argv; FFmpeg runs with
//! the run folder as cwd, so generated files use fixed relative names.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// The filters every video needs: captions and cards (`ass`), the plain background, crossfades, layers, and the
/// gates. An FFmpeg that the user brings (`--use-system`, `SCRIPTVOX_FFMPEG`) can lack one: on 11 Nov 2026 the
/// FFmpeg from Homebrew had no `ass` filter, `doctor` said ok, and `demo` failed with FFmpeg's own words.
pub const NEEDED_FILTERS: [&str; 8] = [
    "ass",
    "gradients",
    "xfade",
    "overlay",
    "ebur128",
    "blackdetect",
    "freezedetect",
    "silencedetect",
];

/// The names in the output of `ffmpeg -filters` (the second column of each row after the `------` line).
pub fn filter_names(listing: &str) -> Vec<&str> {
    listing
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("---"))
        .skip(1)
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect()
}

/// The message of a failed FFmpeg run: its last lines, and for a missing filter what that means and what to do.
fn failed(status: std::process::ExitStatus, err: &str) -> String {
    let tail: Vec<&str> = err.lines().rev().take(5).collect();
    let said = tail.into_iter().rev().collect::<Vec<_>>().join(" | ");
    if err.contains("No such filter") {
        return format!(
            "FFmpeg failed ({status}): {said} | This FFmpeg lacks a filter that ScriptVox needs. Run `scriptvox doctor`, then `scriptvox setup` for the pinned build, or use a full FFmpeg build (with libass)."
        );
    }
    format!("FFmpeg failed ({status}): {said}")
}

pub struct Ffmpeg {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

/// The answer to a failed `Ffmpeg::locate`. A failed automatic download gets the same classes as `setup`.
pub fn locate_failure(command: &str, e: String) -> crate::envelope::Envelope {
    let absent =
        e == "FFmpeg was not found" || e.starts_with("ffprobe is missing") || e.contains("nonfree");
    if absent {
        return crate::envelope::Envelope::fail(
            command,
            crate::exit::Exit::Error,
            e,
            Some(
                "Install FFmpeg, set SCRIPTVOX_FFMPEG to the ffmpeg binary, or run `scriptvox setup`."
                    .into(),
            ),
        );
    }
    let mut env = crate::doctor::setup_failure(e);
    env.command = command.into();
    env
}

/// What ffprobe reads back from a finished file.
#[derive(Debug, PartialEq)]
pub struct Readback {
    pub fps: f64,
    pub frames: u64,
    pub width: u64,
    pub height: u64,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

impl Ffmpeg {
    /// The scriptvox recipe in an MP4's `comment` tag.
    pub fn recipe(&self, file: &Path) -> Result<serde_json::Value, String> {
        // Checked by its first bytes, then opened as that format from the file protocol only.
        let gate = crate::media_gate::input(file)?;
        let o = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-show_entries",
                "format_tags=comment",
                "-of",
                "json",
            ])
            .args(gate)
            .output()
            .map_err(|e| e.to_string())?;
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).map_err(|e| e.to_string())?;
        let c = v["format"]["tags"]["comment"]
            .as_str()
            .ok_or("this MP4 has no scriptvox recipe")?;
        let r: serde_json::Value = serde_json::from_str(c)
            .map_err(|_| "the comment tag is not a scriptvox recipe".to_string())?;
        if r.get("scriptvox").is_none() {
            return Err("the comment tag is not a scriptvox recipe".into());
        }
        Ok(r)
    }

    /// Order: `SCRIPTVOX_FFMPEG`; the pinned build if installed; the system
    /// FFmpeg with `--use-system`; else the pinned build is downloaded (when
    /// `allow_download`). A pinned build must not be `nonfree`.
    pub fn locate(use_system: bool, allow_download: bool) -> Result<Self, String> {
        let on_path = || {
            std::env::var_os("PATH").and_then(|path| {
                std::env::split_paths(&path)
                    .map(|d| d.join(exe("ffmpeg")))
                    .find(|p| p.is_file())
            })
        };
        let env = std::env::var_os("SCRIPTVOX_FFMPEG")
            .map(PathBuf::from)
            .filter(|p| p.is_file());
        let pin = crate::provision::pin("ffmpeg");
        let ffmpeg = if let Some(p) = env {
            p
        } else if let Some(b) = pin.as_ref().and_then(crate::provision::installed) {
            b[0].clone()
        } else if let Some(p) = use_system.then(on_path).flatten() {
            p
        } else if let (true, Some(p)) = (allow_download, pin.as_ref()) {
            let bins = crate::provision::install(p, &|m| eprintln!("scriptvox: {m}"))?;
            if crate::provision::ffmpeg_class(&bins[0])? == "nonfree" {
                let _ = std::fs::remove_dir_all(crate::provision::tool_dir(p));
                return Err(
                    "the pinned FFmpeg build is nonfree; ScriptVox refuses to use it".into(),
                );
            }
            bins[0].clone()
        } else {
            return Err("FFmpeg was not found".into());
        };
        let ffprobe = ffmpeg.with_file_name(exe("ffprobe"));
        if !ffprobe.is_file() {
            return Err(format!("ffprobe is missing next to {}", ffmpeg.display()));
        }
        Ok(Self { ffmpeg, ffprobe })
    }

    /// Runs FFmpeg in `cwd`. On failure the error holds the last stderr lines.
    pub fn run(
        &self,
        cwd: &Path,
        args: &[String],
        fontconfig: Option<&Path>,
    ) -> Result<(), String> {
        let args = crate::media_gate::guard(cwd, args)?;
        let mut cmd = Command::new(&self.ffmpeg);
        cmd.current_dir(cwd)
            .args(["-hide_banner", "-nostdin", "-loglevel", "error", "-y"])
            .args(args);
        if let Some(fc) = fontconfig {
            cmd.env("FONTCONFIG_FILE", fc);
        }
        let out = cmd
            .output()
            .map_err(|e| format!("cannot start FFmpeg: {e}"))?;
        if out.status.success() {
            return Ok(());
        }
        let err = String::from_utf8_lossy(&out.stderr);
        Err(failed(out.status, &err))
    }

    /// Runs FFmpeg with a watchdog: `-progress` lines are the heartbeat, and
    /// no line for `stall` kills the process (exit 7 for the run).
    pub fn run_watched(
        &self,
        cwd: &Path,
        args: &[String],
        fontconfig: Option<&Path>,
        stall: Duration,
    ) -> Result<(), RunError> {
        use std::io::{BufRead, BufReader, Read};
        let args = crate::media_gate::guard(cwd, args).map_err(RunError::Failed)?;
        let mut cmd = Command::new(&self.ffmpeg);
        cmd.current_dir(cwd)
            .args([
                "-hide_banner",
                "-nostdin",
                "-loglevel",
                "error",
                "-y",
                "-nostats",
                "-progress",
                "pipe:1",
            ])
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(fc) = fontconfig {
            cmd.env("FONTCONFIG_FILE", fc);
        }
        // glibc keeps 64 MB of address space for each thread that allocates, and a render has some hundred threads.
        // Two arenas cost no real memory and no speed that was measured, and keep a large frame under an
        // address-space limit (`ulimit -v`): 0.6 GB less at 3840x2160 with 5 scenes (docs/specs/output-size.md).
        if std::env::var_os("MALLOC_ARENA_MAX").is_none() {
            cmd.env("MALLOC_ARENA_MAX", "2");
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| RunError::Failed(format!("cannot start FFmpeg: {e}")))?;
        let (tx, rx) = std::sync::mpsc::channel();
        let stdout = child.stdout.take().unwrap();
        std::thread::spawn(move || {
            for _ in BufReader::new(stdout).lines() {
                if tx.send(()).is_err() {
                    break;
                }
            }
        });
        let mut stderr = child.stderr.take().unwrap();
        let err_thread = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = stderr.read_to_string(&mut s);
            s
        });
        loop {
            match rx.recv_timeout(stall) {
                Ok(()) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(RunError::Stalled(stall.as_secs()));
                }
            }
        }
        let status = child.wait().map_err(|e| RunError::Failed(e.to_string()))?;
        let err = err_thread.join().unwrap_or_default();
        if status.success() {
            return Ok(());
        }
        Err(RunError::Failed(failed(status, &err)))
    }

    pub fn readback(&self, file: &Path) -> Result<Readback, String> {
        // Checked by its first bytes, then opened as that format from the file protocol only.
        let gate = crate::media_gate::input(file)?;
        let out = Command::new(&self.ffprobe)
            .args(["-v", "error", "-select_streams", "v:0", "-count_packets"])
            .args([
                "-show_entries",
                "stream=r_frame_rate,nb_read_packets,width,height",
                "-of",
                "json",
            ])
            .args(gate)
            .output()
            .map_err(|e| format!("cannot start ffprobe: {e}"))?;
        let v: Value =
            serde_json::from_slice(&out.stdout).map_err(|e| format!("ffprobe output: {e}"))?;
        let s = &v["streams"][0];
        let num = |k: &str| s[k].as_u64().or_else(|| s[k].as_str()?.parse().ok());
        let fps = s["r_frame_rate"]
            .as_str()
            .and_then(|r| r.split_once('/'))
            .and_then(|(a, b)| Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?))
            .ok_or("ffprobe gave no frame rate")?;
        Ok(Readback {
            fps,
            frames: num("nb_read_packets").ok_or("ffprobe gave no frame count")?,
            width: num("width").unwrap_or(0),
            height: num("height").unwrap_or(0),
        })
    }

    /// Container duration in seconds.
    pub fn duration(&self, file: &Path) -> Result<f64, String> {
        // Checked by its first bytes, then opened as that format from the file protocol only.
        let gate = crate::media_gate::input(file)?;
        let out = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "csv=p=0",
            ])
            .args(gate)
            .output()
            .map_err(|e| format!("cannot start ffprobe: {e}"))?;
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .map_err(|_| format!("ffprobe gave no duration for {}", file.display()))
    }

    /// True when the file has a stream of this type (`audio`, `video`), from ffprobe, not from words in the banner.
    /// The picture's length: the container's can be longer when the sound runs on. Falls back to the container's.
    pub fn video_duration(&self, file: &Path) -> Result<f64, String> {
        // Checked by its first bytes, then opened as that format from the file protocol only.
        let gate = crate::media_gate::input(file)?;
        let o = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=duration",
                "-of",
                "csv=p=0",
            ])
            .args(gate)
            .output()
            .map_err(|e| e.to_string())?;
        match String::from_utf8_lossy(&o.stdout).trim().parse::<f64>() {
            Ok(d) if d > 0.0 => Ok(d),
            _ => self.duration(file),
        }
    }

    /// `pq` or `hlg` when the picture is HDR, read from the transfer tag of the stream. A file with no tag reads as SDR.
    pub fn hdr_kind(&self, file: &Path) -> Option<&'static str> {
        // Checked by its first bytes, then opened as that format from the file protocol only.
        let gate = crate::media_gate::input(file).ok()?;
        let o = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=color_transfer",
                "-of",
                "csv=p=0",
            ])
            .args(gate)
            .output()
            .ok()?;
        // A phone clip has rotation side data, which adds a comma and more to this line.
        match String::from_utf8_lossy(&o.stdout)
            .split([',', '\n'])
            .next()?
            .trim()
        {
            "smpte2084" => Some("pq"),
            "arib-std-b67" => Some("hlg"),
            _ => None,
        }
    }

    /// An SDR copy of an HDR file, made once in `.scriptvox/hdr/` by the file's hash. Every step then works on plain BT.709.
    /// Any other file comes back as it is. The folder keeps the stem, so an output name does not change.
    pub fn sdr_source(&self, file: &Path) -> Result<PathBuf, String> {
        if self.hdr_kind(file).is_none() {
            return Ok(file.to_path_buf());
        }
        let hash = crate::store::hash_file(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let stem = file.file_stem().unwrap_or_default().to_string_lossy();
        // Absolute: `make` runs FFmpeg in the run's folder.
        let out = std::path::absolute(".scriptvox/hdr")
            .map_err(|e| e.to_string())?
            .join(hash)
            .join(format!("{stem}.mp4"));
        if out.is_file() {
            return Ok(out);
        }
        std::fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
        let (enc, _) = self.pick_encoder()?;
        let part = part_path(&out);
        let mut args: Vec<String> = [
            "-i",
            &file.to_string_lossy(),
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
        ]
        .map(String::from)
        .into();
        args.extend(["-vf".to_string(), TONE_MAP.to_string()]);
        args.extend(enc.args());
        args.extend(["-fps_mode", "passthrough", "-c:a", "aac", "-b:a", "256k"].map(String::from));
        args.push(part.to_string_lossy().into_owned());
        self.run(Path::new("."), &args, None)?;
        std::fs::rename(&part, &out).map_err(|e| e.to_string())?;
        Ok(out)
    }

    /// The video codec name, as ffprobe gives it.
    pub fn video_codec(&self, file: &Path) -> Option<String> {
        // Checked by its first bytes, then opened as that format from the file protocol only.
        let gate = crate::media_gate::input(file).ok()?;
        let o = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=codec_name",
                "-of",
                "csv=p=0",
            ])
            .args(gate)
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    /// The first picture stream keeps an alpha channel: an alpha pixel format, or WebM's `alpha_mode` tag (VP8 and VP9
    /// keep alpha in a side stream, and only libvpx decodes it).
    pub fn has_alpha(&self, file: &Path) -> bool {
        // Checked by its first bytes, then opened as that format from the file protocol only.
        let Ok(gate) = crate::media_gate::input(file) else {
            return false;
        };
        let Ok(o) = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=pix_fmt:stream_tags=alpha_mode,ALPHA_MODE",
                "-of",
                "csv=p=0",
            ])
            .args(gate)
            .output()
        else {
            return false;
        };
        let out = String::from_utf8_lossy(&o.stdout).to_lowercase();
        let mut f = out.trim().split(',');
        let pix = f.next().unwrap_or("");
        let tag = f.any(|t| t.trim() == "1");
        tag || pix.starts_with("yuva")
            || pix.starts_with("gbrap")
            || pix.starts_with("ya")
            || [
                "rgba", "bgra", "argb", "abgr", "rgba64le", "rgba64be", "pal8",
            ]
            .contains(&pix)
    }

    /// A "video" stream moves: a still image and a song's cover art are not one.
    pub fn has_stream(&self, file: &Path, kind: &str) -> bool {
        // Checked by its first bytes, then opened as that format from the file protocol only.
        let Ok(gate) = crate::media_gate::input(file) else {
            return false;
        };
        let Ok(o) = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                &kind[..1],
                "-show_entries",
                "stream=index:stream_disposition=attached_pic:format=format_name",
                "-of",
                "json",
            ])
            .args(gate)
            .output()
        else {
            return false;
        };
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
        let picture = v["format"]["format_name"]
            .as_str()
            .is_some_and(|f| f.split(',').any(|n| n == "image2" || n.ends_with("_pipe")));
        v["streams"].as_array().is_some_and(|s| {
            s.iter()
                .any(|x| kind != "video" || (!picture && x["disposition"]["attached_pic"] != 1))
        })
    }

    /// The picture's size as shown: a phone clip stored 1920x1080 with a -90 degree mark plays as 1080x1920, and FFmpeg
    /// turns it on decode.
    pub fn shown_size(&self, file: &Path) -> Option<(u32, u32)> {
        // Checked by its first bytes, then opened as that format from the file protocol only.
        let gate = crate::media_gate::input(file).ok()?;
        let out = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height:stream_side_data=rotation",
                "-of",
                "json",
            ])
            .args(gate)
            .output()
            .ok()?;
        let v: Value = serde_json::from_slice(&out.stdout).ok()?;
        let s = &v["streams"][0];
        let (w, h) = (s["width"].as_u64()? as u32, s["height"].as_u64()? as u32);
        let turned = s["side_data_list"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| d["rotation"].as_i64())
            .any(|r| r.abs() % 180 == 90);
        Some(if turned { (h, w) } else { (w, h) })
    }

    /// Average frame rate of the first video stream. For a variable-rate clip this is the real rate; `readback`'s
    /// `r_frame_rate` is only the highest rate or the time base.
    pub fn frame_rate(&self, file: &Path) -> Result<f64, String> {
        // Checked by its first bytes, then opened as that format from the file protocol only.
        let gate = crate::media_gate::input(file)?;
        let out = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=avg_frame_rate",
                "-of",
                "csv=p=0",
            ])
            .args(gate)
            .output()
            .map_err(|e| format!("cannot start ffprobe: {e}"))?;
        let s = String::from_utf8_lossy(&out.stdout);
        let (n, d) = s.trim().split_once('/').unwrap_or((s.trim(), "1"));
        match (n.parse::<f64>(), d.parse::<f64>()) {
            (Ok(n), Ok(d)) if n > 0.0 && d > 0.0 => Ok(n / d),
            _ => Err(format!("ffprobe gave no frame rate for {}", file.display())),
        }
    }

    /// True when `encoder` writes one test frame. Catches missing encoders and
    /// hardware encoders without a usable device, before a real render.
    pub fn probe_encoder(&self, encoder: &str) -> Result<(), String> {
        let out = Command::new(&self.ffmpeg)
            .args([
                "-hide_banner",
                "-nostdin",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=s=256x256:d=0.04",
            ])
            .args([
                "-frames:v",
                "1",
                "-pix_fmt",
                "yuv420p",
                "-c:v",
                encoder,
                "-f",
                "null",
                "-",
            ])
            .output()
            .map_err(|e| format!("cannot start FFmpeg: {e}"))?;
        if out.status.success() {
            return Ok(());
        }
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err
            .lines()
            .next()
            .unwrap_or("probe failed")
            .trim()
            .to_string())
    }

    /// The needed filters this FFmpeg does not have. Empty when the listing cannot be read: then nothing is claimed.
    pub fn missing_filters(&self) -> Vec<&'static str> {
        let Ok(o) = Command::new(&self.ffmpeg)
            .args(["-hide_banner", "-filters"])
            .output()
        else {
            return vec![];
        };
        let text = String::from_utf8_lossy(&o.stdout);
        let have = filter_names(&text);
        if have.is_empty() {
            return vec![];
        }
        NEEDED_FILTERS
            .iter()
            .copied()
            .filter(|f| !have.contains(f))
            .collect()
    }

    /// Picks the first encoder that passes the probe. Returns it and the
    /// reasons for every encoder skipped (they go into `explain`).
    pub fn pick_encoder(&self) -> Result<(Encoder, Vec<String>), String> {
        let mut skipped = Vec::new();
        for name in encoder_candidates() {
            match self.probe_encoder(&name) {
                Ok(()) => return Ok((Encoder::new(&name), skipped)),
                Err(e) => skipped.push(format!("encoder {name} skipped: {e}")),
            }
        }
        Err(format!("no H.264 encoder works: {}", skipped.join("; ")))
    }
}

/// HDR to BT.709. Mobius kept the colour closest to the same picture in SDR (`hdr-source.md`); hable and linear came out darker.
const TONE_MAP: &str = "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=mobius:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p";

/// For a command that takes one file: swaps an HDR input for its SDR copy, and returns the finding to report.
pub fn sdr_input(
    input: &mut PathBuf,
    use_system: bool,
) -> Result<Option<serde_json::Value>, String> {
    if !input.is_file() {
        return Ok(None);
    }
    // No FFmpeg yet: the command's own step says so.
    let Ok(ff) = Ffmpeg::locate(use_system, true) else {
        return Ok(None);
    };
    let Some(kind) = ff.hdr_kind(input) else {
        return Ok(None);
    };
    *input = ff.sdr_source(input)?;
    Ok(Some(hdr_finding(kind)))
}

/// Whether writing `out` would write over `input`: canonical paths, so a symlink or a `..` segment counts, and device
/// and inode, so a hard link counts. FFmpeg's own check compares the strings only. `out`'s folder is made first, as every
/// caller does next, because a `..` through a folder that does not exist yet still lands on the input.
// ponytail: on Windows a hard link is not seen (the file index is an unstable std API); add the `same-file` crate if one ever bites.
pub fn same_file(out: &Path, input: &Path) -> bool {
    if let Some(p) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(p);
    }
    #[cfg(unix)]
    if let (Ok(a), Ok(b)) = (std::fs::metadata(out), std::fs::metadata(input)) {
        use std::os::unix::fs::MetadataExt;
        return (a.dev(), a.ino()) == (b.dev(), b.ino());
    }
    matches!(
        (std::fs::canonicalize(out), std::fs::canonicalize(input)),
        (Ok(a), Ok(b)) if a == b
    )
}

fn hdr_finding(kind: &str) -> serde_json::Value {
    serde_json::json!({"gate": "hdr", "status": "warn",
        "detail": format!("the source is HDR ({kind}), so it was tone mapped to SDR before this step"),
        "fix": "Look at the colours with `scriptvox snapshot`. The copy is in .scriptvox/hdr/."})
}

/// Encoder order. Default: CPU only (libx264, then libopenh264 for LGPL builds).
/// `SCRIPTVOX_ENCODER=auto` tries hardware encoders first; any other value names
/// a list, for example `h264_nvenc,libx264`.
// Idea from MoneyPrinterTurbo `app/services/video.py` (MIT): try an encoder and
// fall back to libx264. ScriptVox probes before the render instead of failing one.
pub fn encoder_candidates() -> Vec<String> {
    const CPU: [&str; 2] = ["libx264", "libopenh264"];
    const HW: [&str; 4] = ["h264_nvenc", "h264_qsv", "h264_amf", "h264_videotoolbox"];
    match std::env::var("SCRIPTVOX_ENCODER").ok().as_deref() {
        None | Some("") => CPU.map(String::from).to_vec(),
        Some("auto") => HW.iter().chain(CPU.iter()).map(|s| s.to_string()).collect(),
        Some(list) => list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Encoder {
    pub name: String,
}

impl Encoder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }

    /// Video codec arguments, with BT.709 tags so players show the right colours.
    pub fn args(&self) -> Vec<String> {
        let q: &[&str] = match self.name.as_str() {
            "libx264" => &["-preset", "veryfast", "-crf", "20"],
            "h264_nvenc" => &["-preset", "p5", "-cq", "21"],
            "h264_qsv" => &["-global_quality", "21"],
            _ => &["-b:v", "8M"],
        };
        let mut a = vec!["-c:v".to_string(), self.name.clone()];
        a.extend(q.iter().map(|s| s.to_string()));
        a.extend(
            [
                "-pix_fmt",
                "yuv420p",
                "-colorspace",
                "bt709",
                "-color_primaries",
                "bt709",
                "-color_trc",
                "bt709",
            ]
            .map(String::from),
        );
        a
    }
}

/// Writes through `<out>.part.mp4` and renames on success, so a killed render
/// never leaves a half file at the final path.
pub fn part_path(out: &Path) -> PathBuf {
    let mut name = out
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".part.mp4");
    out.with_file_name(name)
}

#[derive(Debug, PartialEq)]
pub enum RunError {
    Failed(String),
    /// No progress for this many seconds: the watchdog killed FFmpeg.
    Stalled(u64),
}

/// Seconds without FFmpeg progress before the watchdog kills it
/// (`SCRIPTVOX_WATCHDOG_S`, default 120).
pub fn watchdog() -> Duration {
    Duration::from_secs(
        std::env::var("SCRIPTVOX_WATCHDOG_S")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(120),
    )
}
/// A clip whose nominal rate and real average differ by more than 1 percent has no fixed frame grid.
pub fn variable_rate(nominal: f64, avg: f64) -> bool {
    avg > 0.0 && (nominal - avg).abs() / avg > 0.01
}

#[cfg(test)]
mod filter_tests {
    #[test]
    fn filter_names_come_from_the_second_column_after_the_rule() {
        let listing = "Filters:\n  T.. = Timeline support\n  A = Audio input/output\n  ------\n TS aap               AA->A      Apply.\n .. ass               V->V       Render ASS subtitles.\n T. overlay           VV->V      Overlay.\n";
        assert_eq!(super::filter_names(listing), ["aap", "ass", "overlay"]);
        assert!(super::filter_names("no rule line here").is_empty());
    }
}
