//! Chunked render with resume (PLAN-R1 §5, spike answer 3).
//!
//! One FFmpeg pass writes video-only chunks through the `segment` muxer, with
//! a keyframe forced at every chunk start. Each attempt keeps its own segment
//! list; a chunk counts as done once a list names it and its file exists.
//! A rerun of the same render (same key) starts at the first missing chunk.
//! The chunks are joined by stream copy, and the audio is encoded once in the
//! final mux, so there is no AAC priming at the joins.

use crate::engine_ffmpeg::{Encoder, Ffmpeg, RunError};
use crate::ranges::{per_chunk, segment_frames};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub struct Job<'a> {
    pub inputs: &'a [String],
    /// Video graph that ends in `[v]` (visual track plus captions).
    pub graph: &'a str,
    pub enc: &'a Encoder,
    pub fps: u32,
    pub secs: f64,
    pub audio: &'a Path,
    pub out: &'a Path,
    pub fontconfig: &'a Path,
    /// The recipe for the MP4 `comment` tag (None with `--no-embed`).
    pub metadata: Option<&'a str>,
}

#[derive(Debug, PartialEq)]
pub struct Stats {
    pub chunks: u64,
    pub resumed_from: u64,
}

/// Render key: what decides the pixels. Run folders and the audio are not in it,
/// so a rerun of the same command finds the chunks of the earlier attempt.
/// The bytes of every input file are in it: a file changed in place starts over.
pub fn key(job: &Job, captions: &str) -> String {
    let mut h = blake3::Hasher::new();
    for s in [
        env!("CARGO_PKG_VERSION"),
        &job.inputs.join("\u{1f}"),
        job.graph,
        captions,
        &job.enc.name,
        &job.fps.to_string(),
        &format!("{:.3}", job.secs),
    ] {
        h.update(s.as_bytes());
        h.update(&[0]);
    }
    // ponytail: reads each input once per render; blake3 is near memory speed, so this is small beside the encode.
    for input in job.inputs.windows(2).filter(|w| w[0] == "-i") {
        // A lavfi source is not a file and has no bytes to add.
        if let Ok(hash) = crate::store::hash_file(Path::new(&input[1])) {
            h.update(hash.as_bytes());
        }
    }
    h.finalize().to_hex()[..16].to_string()
}

/// Chunks 0..n that are done (named by a segment list and present on disk).
pub fn done(dir: &Path) -> u64 {
    let mut named = BTreeSet::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("list_"))
        {
            for line in std::fs::read_to_string(&p).unwrap_or_default().lines() {
                if let Some(n) = line
                    .split(',')
                    .next()
                    .and_then(|f| f.strip_prefix("chunk_"))
                    .and_then(|f| f.strip_suffix(".mp4"))
                {
                    if let Ok(i) = n.parse::<u64>() {
                        named.insert(i);
                    }
                }
            }
        }
    }
    (0..)
        .take_while(|i| named.contains(i) && chunk(dir, *i).is_file())
        .count() as u64
}

fn chunk(dir: &Path, i: u64) -> PathBuf {
    dir.join(format!("chunk_{i:04}.mp4"))
}

pub fn render(ff: &Ffmpeg, run_dir: &Path, chunk_dir: &Path, job: &Job) -> Result<Stats, RunError> {
    let fail = |e: std::io::Error| RunError::Failed(e.to_string());
    std::fs::create_dir_all(chunk_dir).map_err(fail)?;
    let total = (job.secs * f64::from(job.fps)).round() as u64;
    let per = per_chunk(job.fps);
    let n_chunks = total.div_ceil(per);
    let have = done(chunk_dir).min(n_chunks);
    let start = have * per;
    if start < total {
        let graph = match start {
            0 => job.graph.to_string(),
            s => format!(
                "{},trim=start_frame={s},setpts=PTS-STARTPTS[v]",
                job.graph.strip_suffix("[v]").unwrap_or(job.graph)
            ),
        };
        // Test hook: stop cleanly after N chunks, as a kill would.
        let fault = std::env::var("SCRIPTVOX_FAULT_AFTER_CHUNKS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok());
        let frames = fault.map_or(total, |n| (n * per).clamp(start, total)) - start;
        let mut a = job.inputs.to_vec();
        a.extend([
            "-filter_complex".into(),
            graph,
            "-map".into(),
            "[v]".into(),
            "-an".into(),
        ]);
        a.extend(job.enc.args());
        a.extend([
            "-r".into(),
            job.fps.to_string(),
            "-force_key_frames".into(),
            format!("expr:eq(mod(n,{per}),0)"),
        ]);
        a.extend([
            "-frames:v".into(),
            frames.to_string(),
            "-f".into(),
            "segment".into(),
        ]);
        let cuts = segment_frames(start, total, per);
        if cuts.is_empty() {
            // One chunk left: without this the muxer splits every 2 s at any
            // natural keyframe (scene cuts), and the stitch would miss frames.
            a.extend(["-segment_time".into(), "1000000".into()]);
        } else {
            let list = cuts
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            a.extend(["-segment_frames".into(), list]);
        }
        let list = chunk_dir.join(format!("list_{start:08}.csv"));
        a.extend([
            "-segment_start_number".into(),
            have.to_string(),
            "-segment_list".into(),
            list.to_string_lossy().into_owned(),
        ]);
        a.extend(
            [
                "-segment_list_type",
                "csv",
                "-reset_timestamps",
                "1",
                "-segment_format",
                "mp4",
            ]
            .map(String::from),
        );
        a.push(
            chunk_dir
                .join("chunk_%04d.mp4")
                .to_string_lossy()
                .into_owned(),
        );
        ff.run_watched(
            run_dir,
            &a,
            Some(job.fontconfig),
            crate::engine_ffmpeg::watchdog(),
        )?;
        if fault.is_some_and(|n| n < n_chunks) {
            return Err(RunError::Failed(format!(
                "fault injected after {} chunks",
                fault.unwrap()
            )));
        }
    }
    let done_now = done(chunk_dir);
    if done_now < n_chunks {
        return Err(RunError::Failed(format!(
            "only {done_now} of {n_chunks} chunks were written"
        )));
    }
    // Stitch by stream copy, then mux the audio once.
    let list: String = (0..n_chunks)
        .map(|i| {
            format!(
                "file '{}'\n",
                chunk(chunk_dir, i).to_string_lossy().replace('\'', "'\\''")
            )
        })
        .collect();
    let list_file = chunk_dir.join("concat.txt");
    std::fs::write(&list_file, list).map_err(fail)?;
    let mut a: Vec<String> = ["-f", "concat", "-safe", "0", "-i"]
        .map(String::from)
        .to_vec();
    a.push(list_file.to_string_lossy().into_owned());
    a.extend(["-i".into(), job.audio.to_string_lossy().into_owned()]);
    a.extend(
        [
            "-filter_complex",
            &format!(
                "[1:a]aresample=48000,apad,atrim=end_sample={}[a]",
                (job.secs * 48000.0).round() as u64
            ),
            "-map",
            "0:v",
            "-map",
            "[a]",
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-b:a",
            "160k",
        ]
        .map(String::from),
    );
    if let Some(m) = job.metadata {
        a.extend(["-metadata".into(), format!("comment={m}")]);
    }
    a.extend([
        "-t".into(),
        format!("{:.3}", job.secs),
        "-movflags".into(),
        "+faststart".into(),
        job.out.to_string_lossy().into_owned(),
    ]);
    ff.run(run_dir, &a, None).map_err(RunError::Failed)?;
    Ok(Stats {
        chunks: n_chunks,
        resumed_from: have,
    })
}
