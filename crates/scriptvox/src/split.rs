//! The picture in more than one FFmpeg process (docs/specs/render-split.md).
//!
//! One FFmpeg process holds some frames of every scene's source at once, so its memory grows with the number of
//! scenes. Here the shot track (`[bg]` of `media::graph`) is made in groups of scenes. Each group is its own process
//! and writes a window of whole frames of the track to a lossless file. The final render reads the files as one
//! input and puts every other layer on it, as before. A finished group file is kept until the video is done.

use crate::engine_ffmpeg::{Encoder, Ffmpeg, RunError};
use crate::media::{frames_of, graph_part, shot_frame, Item, Part};
use std::path::{Path, PathBuf};

/// Megapixels times shots: over this the picture is made in groups. Measured under an 8 GB limit of address space:
/// 3840x2160 (8.3 MP) passes with 5 scenes at 6.5 GB and with 7 at 7.7 GB, and fails with 8.
pub const SPLIT_OVER: f64 = 45.0;

/// Megapixels times shots of one group (the shot after the group is read too, for the transition at the border),
/// and the most shots of one group. Measured: 3 shots of 3840x2160 need 2.9 GB of address space, and 12 shots of
/// 1920x1080 need 5.3 GB (more threads and more RGBA sources), so a small frame does not get large groups.
pub const GROUP_MP: f64 = 25.0;
pub const GROUP_MOST: usize = 6;

/// Megabytes of group files for each megapixel and frame (FFV1, measured): 0.23 with a clean clip and photos,
/// 0.01 on the plain background. A noisy camera clip needed 1.8.
pub const DISK_MB: f64 = 0.25;

/// The shots of one group for this frame and shot count, or None for the render in one process.
/// `SCRIPTVOX_SPLIT_SCENES` decides when it is set: 0 is never, another number is that many shots a group.
pub fn group_size((w, h): (u32, u32), shots: usize) -> Option<usize> {
    let mp = f64::from(w) * f64::from(h) / 1e6;
    let n = match std::env::var("SCRIPTVOX_SPLIT_SCENES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
    {
        Some(0) => return None,
        Some(n) => n,
        None if mp * shots as f64 > SPLIT_OVER => ((GROUP_MP / mp) as usize).clamp(1, GROUP_MOST),
        None => return None,
    };
    (n < shots).then_some(n)
}

/// The text of a render in one process that failed for memory. FFmpeg says it in more than one way: the plain
/// error; too few chunks with no error, when a source failed for memory (3840x2160 with 8 scenes); and a PNG
/// decoder that gets no memory for its frames (`Decode error rate`, 34 scenes with 18 overlays, 14 Nov 2026).
fn memory_fault(e: &str) -> bool {
    [
        "Cannot allocate memory",
        "malloc of size",
        "chunks were written",
        "Decode error rate",
        // The same fault when the line above is cut from the message: FFmpeg's "error rate exceeded" number.
        "-1145393733",
    ]
    .iter()
    .any(|m| e.contains(m))
}

/// The shots of a group for one more attempt, after a render in one process failed with `error`. None: the error
/// is not a memory fault, or there are too few shots for groups. The load rule of [`group_size`] counts megapixels
/// times shots. A small frame with many scenes and overlays is under it and can still fail: each FFmpeg input
/// brings its own threads, and their address space does not follow the frame size.
fn retry_per(error: &str, (w, h): (u32, u32), shots: usize) -> Option<usize> {
    let mp = f64::from(w) * f64::from(h) / 1e6;
    let n = ((GROUP_MP / mp) as usize).clamp(1, GROUP_MOST);
    (memory_fault(error) && n < shots).then_some(n)
}

/// [`retry_per`], when the user did not decide: with `SCRIPTVOX_SPLIT_SCENES` set there is no second attempt.
pub fn retry_size(error: &str, frame: (u32, u32), shots: usize) -> Option<usize> {
    if std::env::var_os("SCRIPTVOX_SPLIT_SCENES").is_some() {
        return None;
    }
    retry_per(error, frame, shots)
}

/// The fix line of a render that failed for memory. `one_process`: the picture has shots and was made in one
/// process, so groups are the remedy. If not, the final pass failed (the layers: overlays, insets), or the video
/// has one shot.
pub fn memory_fix(one_process: bool) -> &'static str {
    if one_process {
        "Set the environment variable SCRIPTVOX_SPLIT_SCENES=4 (the picture is then made in groups of 4 scenes, each in its own process) and run the same command again. If it fails again: a smaller `size`, or fewer scenes in one video (make two videos and `scriptvox join` them)."
    } else {
        crate::size::MEMORY_FIX
    }
}

/// One group: shots `shots.0..=shots.1` of the full list go into its graph (the last one is the first shot of the
/// next group, read for the border transition only), and frames `start..end` of the full track come out.
#[derive(Debug, PartialEq)]
pub struct Group {
    pub shots: (usize, usize),
    pub start: u64,
    pub end: Option<u64>,
}

/// The groups for these shots, `per` shots each. `xfade_s(i)` is the length of the transition into shot `i`.
/// A border is at the start of a shot, and the cut is 2 frames after the end of the transition into it: from there
/// the track is that shot alone. A shot too short for that (the next transition starts first) cannot be a border,
/// and the border moves to the next shot. Fewer than 2 groups: empty, the render stays in one process.
pub fn groups(
    shots: &[(f64, f64)],
    xfade_s: impl Fn(usize) -> f64,
    fps: u32,
    total: u64,
    per: usize,
) -> Vec<Group> {
    let n = shots.len();
    // The first frame after the transition into shot `i`, plus one: xfade mixes frames `at..=at+d`.
    let cut = |i: usize| shot_frame(shots, i, fps) + frames_of(xfade_s(i), fps) + 2;
    let free = |i: usize| {
        cut(i)
            <= if i + 1 < n {
                shot_frame(shots, i + 1, fps)
            } else {
                total.saturating_sub(1)
            }
    };
    let mut out = vec![];
    let (mut first, mut start) = (0, 0);
    while let Some(next) = (first + per.max(1)..n).find(|&i| free(i)) {
        out.push(Group {
            shots: (first, next),
            start,
            end: Some(cut(next)),
        });
        (first, start) = (next, cut(next));
    }
    if out.is_empty() {
        return out;
    }
    out.push(Group {
        shots: (first, n - 1),
        start,
        end: None,
    });
    out
}

/// The FFV1 arguments that store a track format with no change of the numbers, or None when there are none.
/// The planar RGB formats go in as packed RGB (the same samples in another order), and full-range YUV as YUV with
/// the full-range mark (without the mark FFmpeg squeezes it to the limited range).
pub fn storage(format: &str) -> Option<&'static [&'static str]> {
    Some(match format {
        "yuv444p" => &["-pix_fmt", "yuv444p"],
        "yuvj444p" => &["-pix_fmt", "yuv444p", "-color_range", "pc"],
        "yuva444p" => &["-pix_fmt", "yuva444p"],
        "gbrp" => &["-pix_fmt", "bgr0"],
        "gbrap" => &["-pix_fmt", "bgra"],
        "gray" => &["-pix_fmt", "gray"],
        "yuv444p10le" => &["-pix_fmt", "yuv444p10le"],
        "gbrp10le" => &["-pix_fmt", "gbrp10le"],
        _ => return None,
    })
}

/// The shots of a video, as `media::graph` takes them. `make` keeps them from the place where it builds the
/// one-process graph to the render.
pub struct Shots {
    pub items: Vec<Item>,
    pub lens: Vec<Option<(f64, f64)>>,
    pub moves: Vec<Option<String>>,
    pub cuts: Vec<(f64, f64)>,
    pub frame: (u32, u32),
    pub fps: u32,
    pub tr: (&'static str, f64),
    pub into: Vec<Option<(&'static str, f64)>>,
}

/// The rest of the one-process render: every FFmpeg input and the whole video graph (shot track first, then the
/// layers, ending in `[v]`), and the font settings of the render.
pub struct Layers<'a> {
    pub inputs: &'a [String],
    pub graph: &'a str,
    pub fontconfig: &'a Path,
}

impl Shots {
    /// The one-process graph of the shot track, or with `part` a part of it.
    pub fn graph(&self, frame: (u32, u32), part: Option<&Part>) -> (Vec<String>, String) {
        graph_part(
            &self.items,
            &self.lens,
            &self.moves,
            &self.cuts,
            frame,
            self.fps,
            self.tr,
            &self.into,
            part,
        )
    }

    /// The pixel format, the colour matrix and the range of the track in the one-process graph. FFmpeg chooses them
    /// from all shots and from the filters after the track together (plain backgrounds alone: `gbrap`; a clip, a
    /// JPEG and an end card: `yuv444p` with the full range), so a group alone can get other ones. They do not
    /// follow the frame size: the shots are built on a very small frame, padded to the real one, and the real
    /// layers (`tail`: their inputs and their graph text, which starts with `;`) follow. One frame is made.
    /// None when this FFmpeg cannot report its graph (before version 7.1).
    pub fn track(&self, ff: &Ffmpeg, cwd: &Path, tail: Option<&Layers>) -> Option<Track> {
        let (w, h) = self.frame;
        let small = ((w / 32).max(8) * 2, (h / 32).max(8) * 2);
        let (mut inputs, mut graph) = self.graph(small, None);
        if crate::size::large(self.frame) {
            // The plain background of a large frame gives YUV (`template::gradient`); the small one must too.
            for i in inputs.iter_mut().filter(|i| i.starts_with("gradients=")) {
                i.push_str(",format=yuv444p");
            }
        }
        let mut out = "[bg]";
        if let Some(t) = tail {
            // `pad` changes the size and lets every track format through, so the layers still take part in the choice.
            graph = format!(
                "{}[bgs];[bgs]pad={w}:{h}[bg]{}",
                graph.strip_suffix("[bg]")?,
                t.graph
            );
            inputs.extend(t.inputs.iter().cloned());
            out = "[v]";
        }
        // FFmpeg writes what it chose for every link of the graph.
        let report = cwd.join("split-track.json");
        inputs.extend(["-filter_complex", &graph, "-map", out].map(String::from));
        inputs.extend(["-frames:v", "1", "-f", "null", "-"].map(String::from));
        let args = crate::media_gate::guard(cwd, &inputs).ok()?;
        let mut cmd = std::process::Command::new(&ff.ffmpeg);
        cmd.current_dir(cwd)
            .args(["-hide_banner", "-nostdin", "-nostats", "-loglevel", "error"])
            // One frame needs no worker threads, and each thread of each input costs address space.
            .args(["-threads", "1", "-filter_threads", "1"])
            .args([
                "-filter_complex_threads",
                "1",
                "-print_graphs_format",
                "json",
            ])
            .arg("-print_graphs_file")
            .arg(&report)
            .args(args);
        if let Some(t) = tail {
            cmd.env("FONTCONFIG_FILE", t.fontconfig);
        }
        // As for the render (`Ffmpeg::run_watched`): less address space for the threads of many inputs.
        if std::env::var_os("MALLOC_ARENA_MAX").is_none() {
            cmd.env("MALLOC_ARENA_MAX", "2");
        }
        let done = cmd.output().ok()?;
        let text = std::fs::read_to_string(&report).ok();
        let _ = std::fs::remove_file(&report);
        if !done.status.success() {
            return None;
        }
        let v: serde_json::Value = serde_json::from_str(&text?).ok()?;
        let link = v["graphs"][0]["filters"]
            .as_array()?
            .iter()
            .find(|f| f["filter_name"] == "xfade")?["filter_outputs"][0]
            .clone();
        // Only plain names reach a filter graph.
        let word = |key: &str| {
            let w = link[key].as_str().unwrap_or("unknown");
            w.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
                .then(|| w.to_string())
        };
        Some(Track {
            format: link["format"].as_str().and(word("format"))?,
            space: word("color_space")?,
            range: word("color_range")?,
        })
    }
}

/// The pixel format, colour matrix and range of a shot track, in FFmpeg's names (`unknown` when not set).
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub format: String,
    pub space: String,
    pub range: String,
}

impl Track {
    /// The arguments of a `format` filter that lets only this track through.
    pub fn pin(&self) -> String {
        format!(
            "pix_fmts={}:color_spaces={}:color_ranges={}",
            self.format, self.space, self.range
        )
    }
}

/// A group file is whole and is what this code wrote: a plain file (no link) inside the group folder, Matroska by
/// its first bytes, with FFV1 and the planned frames (`exact`), or for the last group one frame less at most (the
/// track ends with its last shot; the readback of the video allows the same frame). A file from an earlier attempt
/// is not trusted: someone can put another file under its name.
fn whole(ff: &Ffmpeg, dir: &Path, file: &Path, frames: u64, exact: bool) -> bool {
    let plain = std::fs::symlink_metadata(file).is_ok_and(|m| m.file_type().is_file());
    let inside = match (file.canonicalize(), dir.canonicalize()) {
        (Ok(f), Ok(d)) => f.parent() == Some(d.as_path()),
        _ => false,
    };
    plain
        && inside
        && crate::media_gate::check(file).is_ok_and(|p| p.format == "matroska")
        && ff.video_codec(file).as_deref() == Some("ffv1")
        && ff
            .readback(file)
            .is_ok_and(|r| r.frames == frames || (!exact && r.frames + 1 == frames))
}

/// Graph text with every FFmpeg input `[N:v]` moved down by `by` (the shot inputs are now one input).
fn renumber(graph: &str, by: usize) -> String {
    let mut out = String::with_capacity(graph.len());
    let mut rest = graph;
    while let Some(at) = rest.find('[') {
        out.push_str(&rest[..=at]);
        rest = &rest[at + 1..];
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 && rest[digits..].starts_with(":v]") {
            let n: usize = rest[..digits].parse().unwrap_or(0);
            out.push_str(&n.saturating_sub(by).to_string());
            rest = &rest[digits..];
        }
    }
    out.push_str(rest);
    out
}

/// What `make` gets back: the render with the shot track as one FFmpeg input.
pub struct Split {
    pub inputs: Vec<String>,
    /// The whole video graph, ending in `[v]`.
    pub graph: String,
    /// The folder of the group files: removed when the video is done.
    pub dir: PathBuf,
    pub groups: usize,
    /// Groups that an earlier attempt finished.
    pub kept: usize,
}

/// Makes the shot track in groups of scenes when the frame and the shot count ask for it ([`group_size`]), and
/// gives the render that reads the group files. `Ok(Err(..))`: the render stays in one process. With no reason
/// the load is small. With a reason the load asked for groups and they cannot be made, and `make` says why.
/// `work` is the folder for the group folders; it does not follow the run, so a rerun finds the finished groups.
/// `per`: this many shots a group at any load ([`retry_size`]).
pub fn render(
    ff: &Ffmpeg,
    run_dir: &Path,
    work: &Path,
    s: &Shots,
    secs: f64,
    layers: &Layers,
    per: Option<usize>,
) -> Result<Result<Split, Option<&'static str>>, RunError> {
    let fail = |e: std::io::Error| RunError::Failed(e.to_string());
    let Some(per) = per.or_else(|| group_size(s.frame, s.cuts.len())) else {
        return Ok(Err(None));
    };
    let fps = s.fps;
    let total = (secs * f64::from(fps)).round() as u64;
    let tr_into = |i: usize| s.into.get(i).copied().flatten().unwrap_or(s.tr).1;
    let groups = groups(&s.cuts, tr_into, fps, total, per);
    if groups.len() < 2 {
        return Ok(Err(Some(
            "the scenes are too short for a border between groups",
        )));
    }
    // The one-process render starts with the shot track: its inputs first, its graph first.
    let (one_inputs, one_graph) = s.graph(s.frame, None);
    let (Some(tail_inputs), Some(tail_graph)) = (
        layers.inputs.strip_prefix(one_inputs.as_slice()),
        layers.graph.strip_prefix(one_graph.as_str()),
    ) else {
        return Ok(Err(Some(
            "the video graph does not start with the shot track",
        )));
    };
    let tail = Layers {
        inputs: tail_inputs,
        graph: tail_graph,
        fontconfig: layers.fontconfig,
    };
    let Some(track) = s.track(ff, run_dir, Some(&tail)) else {
        return Ok(Err(Some(
            "this FFmpeg did not report the pixel format of the shot track (it needs version 7.1 or later)",
        )));
    };
    let Some(store) = storage(&track.format) else {
        return Ok(Err(Some(
            "the pixel format of the shot track has no lossless group file",
        )));
    };
    // The group files keep the marks of the track, so FFmpeg changes no number on the way in or out.
    let mut store: Vec<&str> = store.to_vec();
    if track.range != "unknown" && !store.contains(&"-color_range") {
        store.extend(["-color_range", &track.range]);
    }
    if track.space != "unknown" && track.space != "gbr" {
        store.extend(["-colorspace", &track.space]);
    }
    let pin = track.pin();
    let jobs: Vec<(Vec<String>, String, u64)> = groups
        .iter()
        .map(|g| {
            let origin = shot_frame(&s.cuts, g.shots.0, fps);
            let part = Part {
                shots: g.shots,
                origin,
                format: &pin,
            };
            let (inputs, graph) = s.graph(s.frame, Some(&part));
            let end = g
                .end
                .map_or(String::new(), |e| format!(":end_frame={}", e - origin));
            let graph = format!(
                "{graph};[bg]trim=start_frame={}{end},setpts=PTS-STARTPTS[v]",
                g.start - origin
            );
            (inputs, graph, g.end.unwrap_or(total) - g.start)
        })
        .collect();
    // One key for the whole track, as for the chunks: what decides the pixels, with the bytes of every file.
    let all_inputs: Vec<String> = jobs.iter().flat_map(|j| j.0.clone()).collect();
    let all_graphs: Vec<&str> = jobs.iter().map(|j| j.1.as_str()).collect();
    let enc = Encoder::new("ffv1");
    let key = crate::render::key(
        &crate::render::Job {
            inputs: &all_inputs,
            graph: &all_graphs.join("\n"),
            enc: &enc,
            fps,
            secs,
            audio: Path::new(""),
            out: Path::new(""),
            fontconfig: Path::new(""),
            metadata: None,
        },
        &store.join(" "),
    );
    let dir = work.join(format!("split-{key}"));
    std::fs::create_dir_all(&dir).map_err(fail)?;
    // The group folder is a real folder of the work folder, not a link to another place.
    let real = |p: &Path| p.canonicalize().ok();
    if std::fs::symlink_metadata(&dir).map_or(true, |m| !m.file_type().is_dir())
        || real(&dir).as_deref().and_then(Path::parent) != real(work).as_deref()
    {
        return Err(RunError::Failed(format!(
            "{} is not a folder of {}",
            dir.display(),
            work.display()
        )));
    }
    // Test hook: stop after N groups made in this process, as a kill would.
    let fault = std::env::var("SCRIPTVOX_FAULT_AFTER_GROUPS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    let (mut kept, mut made) = (0, 0);
    // FFmpeg never guesses what a listed file is: Matroska only, from the file protocol only.
    let mut list = String::from("ffconcat version 1.0\n");
    for (i, (inputs, graph, frames)) in jobs.iter().enumerate() {
        let file = dir.join(format!("group_{i:04}.mkv"));
        list.push_str(&format!(
            "file '{}'\noption format_whitelist matroska,webm\n",
            file.to_string_lossy().replace('\'', "'\\''")
        ));
        // The last group ends with its last shot; every other group has a fixed window.
        let exact = i + 1 < jobs.len();
        if std::fs::symlink_metadata(&file).is_ok() {
            if whole(ff, &dir, &file, *frames, exact) {
                kept += 1;
                continue;
            }
            // Not the file of an earlier attempt: made again. (`remove_file` takes a link away, not its target.)
            std::fs::remove_file(&file).map_err(fail)?;
        }
        if fault == Some(made) {
            return Err(RunError::Failed(format!(
                "fault injected after {made} groups"
            )));
        }
        // A part file first: a group file with its last name is whole.
        let part = dir.join(format!("group_{i:04}.part.mkv"));
        let _ = std::fs::remove_file(&part);
        let mut a = inputs.clone();
        a.extend(["-filter_complex", graph, "-map", "[v]", "-an"].map(String::from));
        a.extend(["-c:v", "ffv1", "-level", "3"].map(String::from));
        a.extend(store.iter().map(|x| x.to_string()));
        a.extend(["-frames:v", &frames.to_string(), "-f", "matroska"].map(String::from));
        a.push(part.to_string_lossy().into_owned());
        ff.run_watched(run_dir, &a, None, crate::engine_ffmpeg::watchdog())?;
        if !whole(ff, &dir, &part, *frames, exact) {
            let got = ff.readback(&part).map_or(0, |r| r.frames);
            return Err(RunError::Failed(format!(
                "group {} of {} has {got} frames, planned {frames}",
                i + 1,
                jobs.len()
            )));
        }
        std::fs::rename(&part, &file).map_err(fail)?;
        made += 1;
    }
    let list_file = dir.join("groups.txt");
    // A link under this name would send the list to another file.
    let _ = std::fs::remove_file(&list_file);
    std::fs::write(&list_file, list).map_err(fail)?;
    let shot_inputs = one_inputs.iter().filter(|x| *x == "-i").count();
    Ok(Ok(Split {
        inputs: ["-protocol_whitelist", "file", "-f", "concat", "-safe", "0"]
            .into_iter()
            .chain(["-c:v", "ffv1", "-i"])
            .map(String::from)
            .chain([list_file.to_string_lossy().into_owned()])
            .chain(tail_inputs.iter().cloned())
            .collect(),
        // The files' own times are in milliseconds: frame N is at N again, with the marks and format of the track.
        graph: format!(
            "[0:v]settb=1/{fps},setpts=N,setparams=range={}:colorspace={},format={pin}[bg]{}",
            track.range,
            track.space,
            renumber(tail_graph, shot_inputs - 1)
        ),
        dir,
        groups: jobs.len(),
        kept,
    }))
}

/// A failed group as `make` reports it: exit, message, fix line.
pub fn failure(e: RunError) -> (crate::exit::Exit, String, &'static str) {
    use crate::exit::Exit;
    const AGAIN: &str =
        "Run the same command again: finished groups of scenes are kept and the render resumes.";
    match e {
        RunError::Stalled(s) => (
            Exit::WatchdogKill,
            format!("FFmpeg made no progress for {s} s and was stopped"),
            AGAIN,
        ),
        RunError::Failed(e) if e.contains("Cannot allocate memory") || e.contains("malloc of size") => (
            Exit::Error,
            format!("{e} (a group of scenes ran out of memory)"),
            "Set SCRIPTVOX_SPLIT_SCENES=1 for groups of one scene and run the same command again, or use a smaller `size`.",
        ),
        RunError::Failed(e) => (Exit::Error, e, AGAIN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A render in one process that fails for memory gets one more attempt in groups: the faults of 14 Nov 2026
    /// (960x540 draft, 34 and 18 scenes, load 18 and 9, under the limit of 45).
    #[test]
    fn a_memory_fault_in_one_process_asks_for_groups() {
        let draft = (960, 540);
        assert_eq!(group_size(draft, 34), None);
        for e in [
            "FFmpeg failed (exit status: 244): [fc#0 @ 0x5a] Terminating thread with return code -12 (Cannot allocate memory)",
            "only 6 of 10 chunks were written",
            "FFmpeg failed (exit status: 69): [vist#40:0/png @ 0x5a] [dec:png @ 0x5a] Decode error rate 0.694444 exceeds maximum 0.666667",
            "FFmpeg failed (exit status: 69): [vist#43:0/png @ 0x58] [dec:png @ 0x58] Task finished with error code: -1145393733 (Error number -1145393733 occurred)",
        ] {
            assert_eq!(retry_per(e, draft, 34), Some(6), "{e}");
            assert_eq!(retry_per(e, draft, 18), Some(6), "{e}");
        }
        // 3 shots a group at 3840x2160, as the load rule gives.
        assert_eq!(
            retry_per("Cannot allocate memory", (3840, 2160), 5),
            Some(3)
        );
        // Not a memory fault, or too few shots for two groups: no second attempt.
        assert_eq!(retry_per("fault injected after 2 chunks", draft, 34), None);
        assert_eq!(retry_per("No such file or directory", draft, 34), None);
        assert_eq!(retry_per("Cannot allocate memory", draft, 6), None);
        assert_eq!(retry_per("Cannot allocate memory", draft, 1), None);
        // The fix line names the real remedy when the picture was in one process.
        assert!(memory_fix(true).contains("SCRIPTVOX_SPLIT_SCENES=4"));
        assert_eq!(memory_fix(false), crate::size::MEMORY_FIX);
    }

    #[test]
    fn groups_cut_after_the_border_transition_and_skip_a_shot_that_is_too_short() {
        // 30 fps, fade 0.25 s (8 frames as xfade rounds 7.5): the cut is 10 frames after the shot starts.
        let shots: Vec<(f64, f64)> = (0..6)
            .map(|i| (i as f64 * 2.0, i as f64 * 2.0 + 2.0))
            .collect();
        let g = groups(&shots, |_| 0.25, 30, 360, 2);
        assert_eq!(
            g,
            vec![
                Group {
                    shots: (0, 2),
                    start: 0,
                    end: Some(130)
                },
                Group {
                    shots: (2, 4),
                    start: 130,
                    end: Some(250)
                },
                Group {
                    shots: (4, 5),
                    start: 250,
                    end: None
                },
            ]
        );
        // Every frame is in one group.
        assert!(g.windows(2).all(|w| w[0].end == Some(w[1].start)));
        // Shot 2 is 0.2 s: the transition into shot 3 starts before the cut, so the border moves to shot 3.
        let short = [(0.0, 2.0), (2.0, 4.0), (4.0, 4.2), (4.2, 6.0), (6.0, 8.0)];
        let g = groups(&short, |_| 0.25, 30, 240, 2);
        assert_eq!(g[0].shots, (0, 3));
        assert_eq!(
            g[1],
            Group {
                shots: (3, 4),
                start: 136,
                end: None
            }
        );
        // One group is no split.
        assert!(groups(&shots, |_| 0.25, 30, 360, 6).is_empty());
        assert!(groups(&shots[..1], |_| 0.25, 30, 60, 1).is_empty());
    }

    #[test]
    fn the_split_starts_only_over_the_measured_load() {
        // No variable set in the tests. 1920x1080 is 2.07 MP: 21 shots stay in one process, 22 do not.
        assert_eq!(group_size((1920, 1080), 6), None);
        assert_eq!(group_size((1920, 1080), 21), None);
        assert_eq!(group_size((1920, 1080), 30), Some(6));
        assert_eq!(group_size((3840, 2160), 5), None);
        assert_eq!(group_size((3840, 2160), 8), Some(3));
        assert_eq!(storage("gbrap").unwrap()[1], "bgra");
        assert!(storage("yuvj444p").unwrap().contains(&"pc"));
        assert_eq!(storage("yuv420p"), None);
        // The layers after 6 shot inputs: input 6 is input 1 of the render that reads the group files.
        assert_eq!(
            renumber(";[bg][6:v]overlay[a];[7:v]scale=2:2[l];[a][l]overlay[v]", 5),
            ";[bg][1:v]overlay[a];[2:v]scale=2:2[l];[a][l]overlay[v]"
        );
    }
}
