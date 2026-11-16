//! Generative job runner (phase 2, P2-002): one clip from one generator row, through a gateway queue (fal first).
//!
//! Money rules, in order:
//! 1. The estimate is reserved in the ledger before the submit (the spend cap holds in one transaction).
//! 2. A `.submitting` marker is written before the submit and the job record right after it. A job record means a
//!    later run polls that job again and never submits twice. A marker without a record means the submit may have
//!    reached the provider: that is an unconfirmed charge (exit 6), never a blind retry.
//! 3. A clear refusal before the job exists (bad key, bad input) releases the reservation.
//! 4. The cost is committed when the result is fetched.
//!
//! fal queue protocol (fal.ai/docs, read 2 Nov 2026): POST `https://queue.fal.run/<endpoint>` with
//! `Authorization: Key <FAL_KEY>`; the answer gives `request_id`, `status_url`, `response_url`, `cancel_url`.
//! Status is `IN_QUEUE`, `IN_PROGRESS` or `COMPLETED` (with `error` on failure). Use the returned URLs: a status URL
//! built from the full endpoint path answers 405.

use crate::exit::Exit;
use crate::generators::Generator;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const FAL_BASE: &str = "https://queue.fal.run";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub generator: String,
    pub request_id: String,
    pub status_url: String,
    pub response_url: String,
    pub cancel_url: Option<String>,
    pub est_usd: f64,
    pub reservation: i64,
    pub submitted: String,
    /// Set when the clip is saved.
    pub done: Option<String>,
    /// The hash of the request. A resume takes this job over only for the same request. None on an older record.
    #[serde(default)]
    pub input: Option<String>,
}

#[derive(Debug)]
pub struct JobErr {
    pub exit: Exit,
    pub message: String,
    pub fix: String,
    /// More for the envelope than the two texts say: a refused ComfyUI graph's node errors.
    pub detail: Option<Value>,
}

fn err(exit: Exit, message: impl Into<String>, fix: impl Into<String>) -> JobErr {
    JobErr {
        exit,
        message: message.into(),
        fix: fix.into(),
        detail: None,
    }
}

/// The queue base and the hosts its returned URLs may use: the row's `base` (selfhost rows), else the fal queue.
/// `SCRIPTVOX_FAL_BASE` (tests) must be loopback http.
fn queue_base(g: &Generator) -> Result<(String, Vec<String>), JobErr> {
    match std::env::var("SCRIPTVOX_FAL_BASE") {
        Ok(b) if crate::generators::loopback_base_ok(&b) => Ok((
            b.trim_end_matches('/').to_string(),
            vec!["127.0.0.1".into()],
        )),
        Ok(b) => Err(err(
            Exit::Usage,
            format!("SCRIPTVOX_FAL_BASE={b} is not allowed"),
            "Unset it: only loopback test servers may replace queue.fal.run.",
        )),
        Err(_) => match g.base.as_deref() {
            Some(b) => {
                let host = b
                    .trim_start_matches("https://")
                    .split(['/', ':'])
                    .next()
                    .unwrap_or("")
                    .to_string();
                Ok((b.trim_end_matches('/').to_string(), vec![host]))
            }
            None => Ok((FAL_BASE.into(), vec!["queue.fal.run".into()])),
        },
    }
}

/// The row's agent; `run_clip` checked first that its CA file loads.
/// `cancel`: ask the provider to stop every open job of a run (a paid job left running is charged). Returns how many
/// stopped and how many keep running. A stopped job's file goes, so a resume submits it again; a job that keeps running
/// keeps its file, so a resume fetches its result instead of paying twice.
pub fn cancel_jobs(project: &Path, dir: &Path) -> (usize, usize) {
    let (mut stopped, mut running) = (0, 0);
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = e.path();
        if !path.to_string_lossy().ends_with(".job.json") {
            continue;
        }
        let Some(job) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Job>(&t).ok())
            .filter(|j| j.done.is_none())
        else {
            continue;
        };
        let row = crate::generators::all()
            .into_iter()
            .find(|g| g.name == job.generator);
        let sent = row
            .as_ref()
            .zip(job.cancel_url.as_ref())
            .and_then(|(g, url)| {
                // A job file may be older than the host check, or from someone else: the key goes only to the queue.
                let (_, hosts) = queue_base(g).ok()?;
                if !crate::http_rows::url_allowed(url, &hosts) {
                    return None;
                }
                let (key, _) = crate::keys::resolve(&g.key)?;
                crate::http_rows::agent_ca(20, g.ca.as_deref())
                    .ok()?
                    .put(url)
                    .header("Authorization", &format!("Key {key}"))
                    .send_empty()
                    .ok()
            });
        let code = sent.map(|r| r.status().as_u16());
        // 200 is the provider's word that the job is gone: nothing more can charge, so a resume may reserve again.
        // 202 only asks a running job to stop and it may still finish: the job stays, so a resume fetches its result.
        if code == Some(200) {
            if let Ok((c, _)) = crate::run::open_state(project) {
                let _ = crate::plan::settle(&c, job.reservation, None);
            }
            forget_copies(dir, &job.request_id);
            let _ = std::fs::remove_file(&path);
            stopped += 1;
        } else {
            running += 1;
        }
    }
    (stopped, running)
}

/// The hash of a request, kept with its submit, so a resume can tell a submit of this request from one of a changed scene.
fn input_id(input: &Value) -> String {
    blake3::hash(input.to_string().as_bytes())
        .to_hex()
        .to_string()
}

/// The submit marker: the time, then the request's hash on a second line.
fn mark(f: &ShotFiles, input: &Value) -> std::io::Result<()> {
    write_atomic(
        &f.marker,
        &format!("{}\n{}", crate::run::now_utc(), input_id(input)),
    )
}

/// The other run folders of this run's family: `r_x` and its children `r_x.1`, `r_x.1.1`. A resume works in a child
/// folder, so only here can it see what the parent or a sister already sent.
pub fn family(dir: &Path) -> Vec<PathBuf> {
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let root = name.split('.').next().unwrap_or(name);
    let Some(runs) = dir.parent().filter(|p| p.ends_with(".scriptvox/runs")) else {
        return vec![];
    };
    if root.is_empty() {
        return vec![];
    }
    let child = format!("{root}.");
    std::fs::read_dir(runs)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n != name && (n == root || n.starts_with(&child)))
        })
        .collect()
}

/// The files of one kind in a folder, this shot's own first: a patch that inserts a scene moves a request to another name.
fn same_shot_first(folder: &Path, shot: &str, ending: &str) -> Vec<PathBuf> {
    let own = format!("{shot}{ending}");
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(ending))
        .collect();
    files.sort_by_key(|p| p.file_name().and_then(|n| n.to_str()) != Some(own.as_str()));
    files
}

/// A provider job that is gone must not be taken over from a copy in another folder of the family, under any shot name.
fn forget_copies(dir: &Path, request_id: &str) {
    for other in family(dir) {
        for copy in same_shot_first(&other, "", ".job.json") {
            let same = std::fs::read_to_string(&copy)
                .ok()
                .and_then(|t| serde_json::from_str::<Job>(&t).ok())
                .is_some_and(|j| j.request_id == request_id);
            if same {
                let _ = std::fs::remove_file(copy);
            }
        }
    }
}

/// Before a paid submit: is this request already out, in another folder of the family? A finished result comes here at
/// no charge, an open job is copied here and then polled like our own. An unanswered submit points `f.marker` at that
/// folder, so the caller stops at exit 6 and its fix names the file that counts.
// ponytail: a record from before `input` existed counts as the same request under its own shot name only; such a record only lives across an upgrade.
fn adopt(
    g: &Generator,
    input: &Value,
    dir: &Path,
    shot: &str,
    f: &mut ShotFiles,
) -> Result<(), JobErr> {
    if !g.paid() || f.job.is_file() || f.marker.is_file() || f.video.is_file() {
        return Ok(());
    }
    let want = input_id(input);
    let matches = |path: &Path, ending: &str, input: Option<&str>| match input {
        Some(i) => i == want,
        None => path.file_name().and_then(|n| n.to_str()) == Some(&format!("{shot}{ending}")),
    };
    // A failed copy must stop here: the next step would be a new submit.
    let keep = |e: std::io::Error| {
        err(
            Exit::Error,
            e.to_string(),
            "Check the run folder is writable.",
        )
    };
    let video_of = |file: &Path, ending: &str| {
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let ext = f.video.extension().and_then(|e| e.to_str()).unwrap_or("");
        file.with_file_name(format!("{}.{ext}", name.trim_end_matches(ending)))
    };
    let copy_video = |from: &Path| {
        let tmp = f.video.with_extension("copy");
        std::fs::copy(from, &tmp)
            .and_then(|_| std::fs::rename(&tmp, &f.video))
            .map_err(keep)
    };
    let folders = family(dir);
    for other in &folders {
        for file in same_shot_first(other, shot, ".job.json") {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let Ok(j) = serde_json::from_str::<Job>(&text) else {
                continue;
            };
            if j.generator != g.name || !matches(&file, ".job.json", j.input.as_deref()) {
                continue;
            }
            let made = video_of(&file, ".job.json");
            match j.done {
                None => return write_atomic(&f.job, &text).map_err(keep),
                Some(_) if made.is_file() => {
                    copy_video(&made)?;
                    return write_atomic(&f.job, &text).map_err(keep);
                }
                Some(_) => {}
            }
        }
    }
    for other in folders {
        for marker in same_shot_first(&other, shot, ".submitting") {
            let same = std::fs::read_to_string(&marker)
                .is_ok_and(|t| matches(&marker, ".submitting", t.lines().nth(1)));
            if same {
                // A connected row keeps its marker after the picture came.
                let made = video_of(&marker, ".submitting");
                if made.is_file() {
                    return copy_video(&made);
                }
                f.marker = marker;
                return Ok(());
            }
        }
    }
    Ok(())
}

/// What a row charges for this request, when the caller gave no estimate: the length in the request (a row may write
/// it "10s") and the reference pictures it carries.
fn row_est(g: &Generator, input: &Value) -> f64 {
    let seconds = match &input["duration"] {
        Value::String(s) => s
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok(),
        v => v.as_f64(),
    }
    .unwrap_or(g.durations.first().copied().unwrap_or(0.0));
    let refs = input["image_urls"].as_array().map_or(0, Vec::len)
        + usize::from(input[g.image_key()].is_string());
    g.cost_for(seconds, 0.0, if g.takes_reference { refs } else { 0 })
}

fn agent(g: &Generator, timeout_s: u64) -> ureq::Agent {
    crate::http_rows::agent_ca(timeout_s, g.ca.as_deref()).expect("CA checked in run_clip")
}

fn json_body(resp: &mut ureq::http::Response<ureq::Body>) -> Result<Value, String> {
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

/// One call to a speech service with the OpenAI speech API: the text in, a WAV back. A finished file is kept, so a
/// rerun asks nothing.
fn run_speech(g: &Generator, input: &Value, dir: &Path, shot: &str) -> Result<PathBuf, JobErr> {
    let f = shot_files(dir, shot, "wav");
    if f.video.is_file() {
        return Ok(f.video);
    }
    std::fs::create_dir_all(dir).map_err(|e| {
        err(
            Exit::Error,
            e.to_string(),
            "Check the project folder is writable.",
        )
    })?;
    let base = g.base.as_deref().unwrap_or("").trim_end_matches('/');
    let body = serde_json::json!({
        "model": g.t2v,
        "input": input["text"],
        "voice": input.get("voice").cloned().unwrap_or(Value::Null),
        "response_format": "wav",
    });
    let mut req = agent(g, 300).post(&format!("{base}/v1/audio/speech"));
    if g.key != "none" {
        let (key, _) = crate::keys::resolve(&g.key).ok_or_else(|| {
            err(
                Exit::NeedsInput,
                format!("no {} key", g.key),
                format!("Run `scriptvox keys set {}` in your terminal.", g.key),
            )
        })?;
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    let down = |e: String| {
        err(
            Exit::ProviderRetryable,
            format!("{} at {base}: {e}", g.name),
            "Start your speech service, or check `base` in the row.",
        )
    };
    // A paid call with no clear answer may have been charged (rule 2): it is never retried blindly.
    let unclear = |e: String| {
        if g.paid() {
            err(
                Exit::UnconfirmedCharge,
                format!("{} at {base}: {e}", g.name),
                format!(
                    "Check your service for this request. To submit again anyway (it may charge twice), delete {} and give a --max-spend that covers both.",
                    f.marker.display()
                ),
            )
        } else {
            down(e)
        }
    };
    let mut resp = req
        .header("Content-Type", "application/json")
        .send(body.to_string().as_bytes())
        .map_err(|e| match e {
            // The request never left this machine.
            ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => down(e.to_string()),
            e => unclear(e.to_string()),
        })?;
    if (400..500).contains(&resp.status().as_u16()) {
        return Err(down(format!(
            "the service refused the request: HTTP {}",
            resp.status().as_u16()
        )));
    }
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(200_000_000)
        .read_to_vec()
        .map_err(|e| unclear(e.to_string()))?;
    // A service that answers with JSON or an error page has not spoken.
    if !(bytes.len() > 44 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE") {
        return Err(unclear(format!(
            "the answer is not a WAV file ({} bytes)",
            bytes.len()
        )));
    }
    let tmp = f.video.with_extension("part");
    std::fs::write(&tmp, &bytes)
        .and_then(|()| std::fs::rename(&tmp, &f.video))
        .map_err(|e| {
            err(
                Exit::Error,
                e.to_string(),
                "Check the project folder is writable.",
            )
        })?;
    Ok(f.video)
}

/// The first bytes of a picture.
fn is_image(b: &[u8]) -> bool {
    b.starts_with(b"\x89PNG")
        || b.starts_with(b"\xff\xd8\xff")
        || (b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP")
}

/// A service named a parameter it does not take. `n` alone is too short to look for without its quotes.
fn refuses(answer: &str, param: &str) -> bool {
    if param.len() > 1 {
        answer.contains(param)
    } else {
        [
            format!("'{param}'"),
            format!("\"{param}\""),
            format!("parameter {param}"),
        ]
        .iter()
        .any(|w| answer.contains(w))
    }
}

/// One call to a service added with `connect`: an OpenAI-compatible image API, or ComfyUI. It may charge, so it takes the
/// same reservation and submit marker as a queue row: only an answer that says "refused" clears them.
#[allow(clippy::too_many_arguments)]
fn run_connected(
    g: &Generator,
    input: &Value,
    project: &Path,
    dir: &Path,
    shot: &str,
    run: &str,
    cap: f64,
    est: Option<f64>,
    wait_s: u64,
) -> Result<(PathBuf, f64), JobErr> {
    use crate::comfy::Failure;
    let mut f = shot_files(dir, shot, g.ext());
    adopt(g, input, dir, shot, &mut f)?;
    let f = f;
    if f.video.is_file() {
        return Ok((f.video, 0.0));
    }
    let unconfirmed = |why: String| {
        err(
            Exit::UnconfirmedCharge,
            why,
            format!(
                "Check your service for this request. To submit again anyway (it may charge twice), delete {} and give a --max-spend that covers both.",
                f.marker.display()
            ),
        )
    };
    if f.marker.is_file() && g.paid() {
        return Err(unconfirmed(format!(
            "a submit for {shot} started but its answer was never recorded"
        )));
    }
    let key = if g.key == "none" {
        None
    } else {
        let (k, _) = crate::keys::resolve(&g.key).ok_or_else(|| {
            err(
                Exit::NeedsInput,
                format!("no {} key", g.key),
                format!("Run `scriptvox keys set {}` in your terminal.", g.key),
            )
        })?;
        Some(k)
    };
    let folder =
        |e: std::io::Error| err(Exit::Error, e.to_string(), "Check the folder is writable.");
    std::fs::create_dir_all(dir).map_err(folder)?;
    // A caller that gives no estimate has a row with a real price: that price counts against the cap.
    let est = est.unwrap_or_else(|| row_est(g, input));
    let (mut c, _) = crate::run::open_state(project)
        .map_err(|e| err(Exit::Error, e.to_string(), "Check state.db."))?;
    let reservation = crate::plan::reserve(&mut c, run, &g.name, est, cap).map_err(|e| {
        err(
            Exit::ApprovalNeeded,
            e,
            "Approve a plan that covers this cost, or raise --max-spend.",
        )
    })?;
    mark(&f, input).map_err(folder)?;
    let answer = if g.gateway == crate::generators::COMFYUI {
        crate::comfy::run(g, input, key.as_deref(), wait_s)
    } else if g.gateway == crate::generators::DATA_FILE {
        crate::data_row::run(
            g,
            input,
            key.as_deref(),
            wait_s,
            dir,
            &f.video.with_extension("download"),
        )
    } else {
        openai_image(g, input, key.as_deref(), dir, &f)
    };
    let bytes = match answer {
        Ok(b) => b,
        Err(Failure::Unconfirmed(why)) => return Err(unconfirmed(why)),
        Err(refused) => {
            let (exit, why, fix, detail) = match refused {
                Failure::RefusedNodes(exit, why, fix, nodes) => (exit, why, fix, Some(nodes)),
                Failure::Refused(exit, why, fix) => (exit, why, fix, None),
                Failure::Unconfirmed(_) => unreachable!("matched above"),
            };
            let _ = crate::plan::settle(&c, reservation, None);
            let _ = std::fs::remove_file(&f.marker);
            return Err(JobErr {
                detail,
                ..err(exit, why, fix)
            });
        }
    };
    let good = if g.kind == "video.gen" {
        is_mp4(&bytes)
    } else {
        is_image(&bytes)
    };
    if !good {
        return Err(unconfirmed(format!(
            "{} gave {} bytes that are not a {}",
            g.name,
            bytes.len(),
            if g.kind == "video.gen" {
                "mp4 video"
            } else {
                "picture"
            }
        )));
    }
    let tmp = f.video.with_extension("part");
    std::fs::write(&tmp, &bytes)
        .and_then(|()| std::fs::rename(&tmp, &f.video))
        .map_err(folder)?;
    crate::plan::settle(&c, reservation, Some(est))
        .map_err(|e| err(Exit::Error, e.to_string(), "Check state.db."))?;
    Ok((f.video, est))
}

fn is_mp4(b: &[u8]) -> bool {
    b.len() > 8 && &b[4..8] == b"ftyp"
}

/// The picture from an OpenAI-compatible service. An optional field the service refuses is dropped and the call
/// repeated, at most once per field: a 400 is not a charge.
fn openai_image(
    g: &Generator,
    input: &Value,
    key: Option<&str>,
    dir: &Path,
    f: &ShotFiles,
) -> Result<Vec<u8>, crate::comfy::Failure> {
    use crate::comfy::Failure::{Refused, Unconfirmed};
    use base64::Engine;
    let base = g.base.as_deref().unwrap_or("").trim_end_matches('/');
    let size = match input["aspect_ratio"].as_str() {
        Some("16:9") => "1792x1024",
        Some("1:1") => "1024x1024",
        _ => "1024x1792",
    };
    let mut optional = vec![
        ("size", serde_json::json!(size)),
        ("n", serde_json::json!(1)),
        ("response_format", serde_json::json!("b64_json")),
    ];
    let text = loop {
        let mut body = serde_json::json!({"model": g.t2v, "prompt": input["prompt"]});
        for (k, v) in &optional {
            body[*k] = v.clone();
        }
        let mut req = agent(g, 600)
            .post(&format!("{base}/v1/images/generations"))
            .header("Content-Type", "application/json");
        if let Some(k) = key {
            req = req.header("Authorization", &format!("Bearer {k}"));
        }
        let mut resp = req
            .send(body.to_string().as_bytes())
            .map_err(|e| Unconfirmed(format!("{} did not answer: {e}", g.name)))?;
        let code = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        if code < 400 {
            break text;
        }
        let low = text.to_ascii_lowercase();
        let before = optional.len();
        if code == 400 || code == 422 {
            optional.retain(|(k, _)| !refuses(&low, k));
        }
        if optional.len() < before {
            continue;
        }
        if code >= 500 {
            return Err(Unconfirmed(format!("{} answered HTTP {code}", g.name)));
        }
        let exit = if code == 401 || code == 403 {
            Exit::NeedsInput
        } else {
            Exit::Error
        };
        return Err(Refused(
            exit,
            format!("{} refused the request: HTTP {code} {}", g.name, text.chars().take(300).collect::<String>()),
            format!("Check the row in `scriptvox providers`; if the key is wrong, run `scriptvox keys set {}`.", g.key),
        ));
    };
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| Unconfirmed(format!("{} gave no JSON: {e}", g.name)))?;
    let item = &doc["data"][0];
    let bytes = if let Some(b) = item["b64_json"].as_str() {
        base64::engine::general_purpose::STANDARD
            .decode(b.trim())
            .map_err(|e| Unconfirmed(format!("{} gave bad base64: {e}", g.name)))?
    } else if let Some(url) = item["url"].as_str() {
        fetch_result(g, url, dir, &f.video.with_extension("download"))?
    } else {
        return Err(Unconfirmed(format!(
            "{} gave neither b64_json nor url",
            g.name
        )));
    };
    Ok(bytes)
}

/// The bytes behind a result URL of a connected row: the service's own host, or a public https host. The key is not
/// sent.
pub(crate) fn fetch_result(
    g: &Generator,
    url: &str,
    dir: &Path,
    part: &Path,
) -> Result<Vec<u8>, crate::comfy::Failure> {
    use crate::comfy::Failure::Unconfirmed;
    // The service's own hosts as the row lists them, or a public https host (a file host that cannot be listed in
    // advance). An https URL that names a machine inside a network is refused: the URL is the service's text.
    let own = crate::http_rows::url_allowed(url, &g.hosts);
    if !own && (!url.starts_with("https://") || crate::http_rows::local_host(url)) {
        return Err(Unconfirmed(format!(
            "the result URL is not on the service's own host and not a public https address: {url}"
        )));
    }
    // No redirect is followed: the URL above was checked, a redirect's target would not be.
    let got = crate::http_rows::download_no_redirect(url, dir, part, g.ca.as_deref(), !own)
        .map_err(|e| Unconfirmed(format!("the result download failed: {e}")))?;
    let b =
        std::fs::read(&got).map_err(|e| Unconfirmed(format!("the result download failed: {e}")))?;
    let _ = std::fs::remove_file(&got);
    Ok(b)
}

/// Paths of one shot's files inside `dir`.
pub struct ShotFiles {
    pub job: PathBuf,
    pub marker: PathBuf,
    pub video: PathBuf,
}

pub fn shot_files(dir: &Path, shot: &str, ext: &str) -> ShotFiles {
    ShotFiles {
        job: dir.join(format!("{shot}.job.json")),
        marker: dir.join(format!("{shot}.submitting")),
        video: dir.join(format!("{shot}.{ext}")),
    }
}

/// Runs (or resumes) one clip. `project` holds state.db (the ledger); `dir` holds the shot files. Returns the video
/// path and the committed cost. `wait_s` bounds the polling: past it the job stays on record and exit 9 says so.
#[allow(clippy::too_many_arguments)]
pub fn run_clip(
    g: &Generator,
    input: &Value,
    image: bool,
    project: &Path,
    dir: &Path,
    shot: &str,
    run: &str,
    cap: f64,
    wait_s: u64,
) -> Result<(PathBuf, f64), JobErr> {
    run_priced(g, input, image, project, dir, shot, run, cap, wait_s, None)
}

/// `run_clip` with the estimate given, for a row whose request does not show its length (speech to text: the audio).
#[allow(clippy::too_many_arguments)]
pub fn run_priced(
    g: &Generator,
    input: &Value,
    image: bool,
    project: &Path,
    dir: &Path,
    shot: &str,
    run: &str,
    cap: f64,
    wait_s: u64,
    est: Option<f64>,
) -> Result<(PathBuf, f64), JobErr> {
    if g.gateway == "local" {
        let done = if g.kind == "image.gen" {
            run_local_image(g, input, dir, shot)
        } else {
            run_local(input, dir, shot)
        };
        return done.map(|p| (p, 0.0));
    }
    // The last guard: every row that is not local goes through here.
    if crate::plan::local_only() {
        return Err(err(
            Exit::NeedsInput,
            format!(
                "{}: {} is a network row",
                crate::plan::LOCAL_ONLY_STOP,
                g.name
            ),
            "Run without --local-only, or use a local row (`scriptvox providers`).",
        ));
    }
    if let Some((why, fix)) = g.retired_why() {
        return Err(err(Exit::Usage, why, fix));
    }
    if g.gateway == crate::generators::OPENAI_SPEECH {
        // A finished file costs nothing again. A paid row (known price or not) always goes against the cap.
        let f = shot_files(dir, shot, "wav");
        let est = est.or_else(|| g.paid().then(|| row_est(g, input)));
        let Some(est) = est.filter(|_| !f.video.is_file()) else {
            return run_speech(g, input, dir, shot).map(|p| (p, 0.0));
        };
        if f.marker.is_file() {
            return Err(err(
                Exit::UnconfirmedCharge,
                format!("a call for {shot} started but its answer was never recorded; {} may have charged it", g.name),
                format!("Check your service for this request. To submit again anyway (it may charge twice), delete {} and give a --max-spend that covers both.", f.marker.display()),
            ));
        }
        let db = |e: String| err(Exit::Error, e, "Check state.db.");
        std::fs::create_dir_all(dir).map_err(|e| db(e.to_string()))?;
        let (mut c, _) = crate::run::open_state(project).map_err(|e| db(e.to_string()))?;
        let reservation = crate::plan::reserve(&mut c, run, &g.name, est, cap).map_err(|e| {
            err(
                Exit::ApprovalNeeded,
                e,
                "Approve a plan that covers this cost, or raise --max-spend.",
            )
        })?;
        mark(&f, input).map_err(|e| db(e.to_string()))?;
        let done = run_speech(g, input, dir, shot);
        // An unclear failure keeps the marker and the reservation: the service may have charged.
        if !matches!(&done, Err(e) if matches!(e.exit, Exit::UnconfirmedCharge | Exit::Error)) {
            crate::plan::settle(&c, reservation, done.is_ok().then_some(est))
                .map_err(|e| db(e.to_string()))?;
            let _ = std::fs::remove_file(&f.marker);
        }
        return done.map(|p| (p, est));
    }
    if g.gateway == crate::generators::OPENAI_IMAGES
        || g.gateway == crate::generators::COMFYUI
        || g.gateway == crate::generators::DATA_FILE
    {
        return run_connected(g, input, project, dir, shot, run, cap, est, wait_s);
    }
    if let Some(b) = g.why_not().filter(|b| b.kind == "dependency") {
        return Err(err(Exit::NeedsInput, b.why, b.fix));
    }
    let (key, _) = crate::keys::resolve(&g.key).ok_or_else(|| {
        err(
            Exit::NeedsInput,
            format!("no {} key", g.key),
            format!("Run `scriptvox keys set {}` in your terminal.", g.key),
        )
    })?;
    std::fs::create_dir_all(dir).map_err(|e| {
        err(
            Exit::Error,
            e.to_string(),
            "Check the project folder is writable.",
        )
    })?;
    let mut f = shot_files(dir, shot, g.ext());
    adopt(g, input, dir, shot, &mut f)?;
    let f = f;
    let (base, queue_hosts) = queue_base(g)?;
    if let Err(e) = crate::http_rows::agent_ca(1, g.ca.as_deref()) {
        return Err(err(
            Exit::Usage,
            format!("{}: {e}", g.name),
            "Save the gateway's CA certificate (PEM) at that path, then run again.",
        ));
    }

    let job: Job = if f.job.is_file() {
        let text = std::fs::read_to_string(&f.job)
            .map_err(|e| err(Exit::Error, e.to_string(), "Check the run folder."))?;
        serde_json::from_str(&text).map_err(|e| {
            err(
                Exit::Error,
                format!("{}: {e}", f.job.display()),
                "The job record is damaged; check the provider dashboard before you delete it.",
            )
        })?
    } else if f.marker.is_file() && g.paid() {
        return Err(err(
            Exit::UnconfirmedCharge,
            format!("a submit for {shot} started but its answer was never recorded; the provider may have queued and charged it"),
            format!("Check your {} dashboard for this request. To submit again anyway (it may charge twice), delete {} and give a --max-spend that covers both.", g.gateway, f.marker.display()),
        ));
    } else {
        submit(
            g,
            input,
            image,
            &key,
            &base,
            &queue_hosts,
            project,
            &f,
            run,
            cap,
            est,
        )?
    };

    if job.done.is_some() && f.video.is_file() {
        return Ok((f.video, job.est_usd));
    }
    // A job file may be older than the host check, or from someone else: no key goes to a URL off the queue.
    let urls = [
        Some(&job.status_url),
        Some(&job.response_url),
        job.cancel_url.as_ref(),
    ];
    if let Some(u) = urls
        .into_iter()
        .flatten()
        .find(|u| !crate::http_rows::url_allowed(u, &queue_hosts))
    {
        return Err(err(
            Exit::Error,
            format!("the job record names a URL off the queue host: {u}"),
            format!(
                "Nothing was sent. Do not trust {}: check your {} dashboard for this job.",
                f.job.display(),
                g.gateway
            ),
        ));
    }

    // Poll the returned status URL, slower over time.
    let started = Instant::now();
    let mut delay = 2u64;
    let status = loop {
        let mut resp = agent(g, 30)
            .get(&job.status_url)
            .header("Authorization", &format!("Key {key}"))
            .call()
            .map_err(|e| {
                err(
                    Exit::ProviderRetryable,
                    format!("status check failed: {e}"),
                    "Run the same command again: it polls the same job, it does not submit again.",
                )
            })?;
        let code = resp.status().as_u16();
        let body: Value = json_body(&mut resp).unwrap_or(Value::Null);
        if code >= 400 && code != 202 {
            return Err(err(
                Exit::ProviderRetryable,
                format!("status check answered HTTP {code}"),
                "Run the same command again: it polls the same job.",
            ));
        }
        match body["status"].as_str() {
            Some("COMPLETED") => break body,
            Some("IN_QUEUE") | Some("IN_PROGRESS") | None => {}
            Some(other) => {
                return Err(err(
                    Exit::ProviderRetryable,
                    format!("unknown status {other}"),
                    "Run the same command again.",
                ))
            }
        }
        if started.elapsed().as_secs() >= wait_s {
            return Err(err(
                Exit::StillRunning,
                format!(
                    "{shot} is still {} at the provider",
                    body["status"].as_str().unwrap_or("queued")
                ),
                "Run the same command again later: it polls the same job and never submits twice.",
            ));
        }
        std::thread::sleep(Duration::from_secs(delay));
        delay = (delay * 3 / 2).min(10);
    };
    if let Some(e) = status["error"].as_str().filter(|e| !e.is_empty()) {
        // Our own GPU queue charges nothing: a failed job may simply run again, so the record goes.
        if g.gateway == "selfhost" && g.usd_per_s == 0.0 {
            let _ = std::fs::remove_file(&f.job);
            let _ = std::fs::remove_file(&f.marker);
            return Err(err(
                Exit::ProviderRetryable,
                format!("{} finished {shot} with an error: {e}", g.name),
                "Run the same command again: this free row submits again (it charges nothing).",
            ));
        }
        return Err(err(
            Exit::UnconfirmedCharge,
            format!("the provider finished {shot} with an error: {e}"),
            format!(
                "Check your {} dashboard to see if it was charged. The job record stays in {}.",
                g.gateway,
                f.job.display()
            ),
        ));
    }

    // Fetch the result, then the video.
    let mut resp = agent(g, 60)
        .get(&job.response_url)
        .header("Authorization", &format!("Key {key}"))
        .call()
        .map_err(|e| {
            err(
                Exit::ProviderRetryable,
                format!("result fetch failed: {e}"),
                "Run the same command again: it fetches the same result.",
            )
        })?;
    let code = resp.status().as_u16();
    if code >= 400 {
        return Err(err(
            Exit::ProviderRetryable,
            format!("result fetch answered HTTP {code}"),
            "Run the same command again: it fetches the same result.",
        ));
    }
    let result: Value = json_body(&mut resp).map_err(|e| {
        err(
            Exit::ProviderRetryable,
            format!("result is not JSON: {e}"),
            "Run the same command again.",
        )
    })?;
    if g.output == "inline" {
        if g.kind == "stt" && result.get("text").is_none() && result.get("words").is_none() {
            return Err(err(
                Exit::ProviderRetryable,
                format!("the result of {shot} has no text and no words"),
                "Run the same command again: it fetches the same result.",
            ));
        }
        write_atomic(&f.video, &result.to_string())
            .map_err(|e| err(Exit::Error, e.to_string(), "Check the folder."))?;
        return finish(project, job, &f);
    }
    let url = crate::http_rows::get(&result, &g.output)
        .first()
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            err(
                Exit::Error,
                format!("no video URL at {} in the result", g.output),
                "The row's `output` path may be wrong; check the model's API page.",
            )
        })?;
    if !crate::http_rows::url_allowed(&url, &g.hosts) {
        return Err(err(
            Exit::Error,
            format!("the video URL is not on an allowed host ({:?})", g.hosts),
            "Add the provider's real download host to the row's `hosts` only after checking it.",
        ));
    }
    crate::http_rows::download(&url, dir, &f.video, g.ca.as_deref()).map_err(|e| {
        err(
            Exit::ProviderRetryable,
            e,
            "Run the same command again: the download resumes.",
        )
    })?;
    // The file is from a service: only plain media stays, before FFmpeg or ffprobe is given it (`media-inputs.md`).
    // A speech to text row answers with a transcript, not media: ScriptVox reads it itself as JSON and FFmpeg never
    // sees it, so the check is that it is a JSON object.
    let checked = if g.kind == "stt" {
        match std::fs::read(&f.video)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        {
            Some(v) if v.is_object() => Ok(()),
            _ => {
                let _ = std::fs::remove_file(&f.video);
                Err(format!(
                    "{} answered with a file that is not a transcript (a JSON object)",
                    g.name
                ))
            }
        }
    } else {
        crate::media_gate::downloaded(&g.name, &f.video).map(|_| ())
    };
    if let Err(e) = checked {
        // As for a mask (`edit_matte`): a free row's bad answer is forgotten, so the same command asks the row again.
        // A paid job stays on record: it was charged, and the same command must not buy it again.
        let fix = if g.paid() {
            "Nothing was kept. The paid job stays on record, so the same command fetches the same result and buys nothing. Check the service: `scriptvox providers test <row>`."
        } else {
            let _ = std::fs::remove_file(&f.job);
            let _ = std::fs::remove_file(&f.marker);
            "Nothing was kept. Run the same command again: this free row is asked again. If it repeats, the row is at fault: `scriptvox providers test <row>`."
        };
        return Err(err(
            Exit::Error,
            format!("{e}. {}", crate::media_gate::ACCEPTED),
            fix,
        ));
    }

    finish(project, job, &f)
}

/// The local speech to text plugin: one process, no ledger, nothing leaves this machine. A rerun reuses the transcript.
fn run_local(input: &Value, dir: &Path, shot: &str) -> Result<PathBuf, JobErr> {
    use base64::Engine;
    let failed = |e: String| {
        err(
            Exit::Error,
            format!("local speech to text failed: {e}"),
            "Run `scriptvox setup speech-local` again, or check the audio.",
        )
    };
    // The language picks the model folder (the Arabic model only for Arabic). Its transcript has its own cache name, so
    // audio heard by the present model is heard again; the old names stay valid.
    let folder = crate::provision::whisper_folder_now(input["language"].as_str());
    let out = if folder == "model-ar" {
        dir.join(format!("{shot}.{folder}.json"))
    } else {
        dir.join(format!("{shot}.json"))
    };
    if out.is_file() {
        return Ok(out);
    }
    std::fs::create_dir_all(dir).map_err(|e| failed(e.to_string()))?;
    // ponytail: the callers build a data URI for every row, so the bytes go through base64 and back; small next to the model.
    let wav = dir.join(format!("{shot}.input.wav"));
    let data = input["audio_url"]
        .as_str()
        .and_then(|u| u.split_once(";base64,"))
        .map(|u| base64::engine::general_purpose::STANDARD.decode(u.1))
        .ok_or_else(|| failed("no audio in the request".into()))?
        .map_err(|e| failed(e.to_string()))?;
    std::fs::write(&wav, data).map_err(|e| failed(e.to_string()))?;
    let tmp = out.with_extension("tmp");
    let mut c = crate::provision::whisper_command(folder).map_err(failed)?;
    c.arg(&wav).arg(&tmp);
    if let Some(l) = input["language"].as_str() {
        c.args(["--lang", l]);
    }
    if let Some(p) = input["prompt"].as_str() {
        c.args(["--prompt", p]);
    }
    let done = c.output().map_err(|e| failed(e.to_string()))?;
    let _ = std::fs::remove_file(&wav);
    if !done.status.success() {
        let text = String::from_utf8_lossy(&done.stderr);
        let tail: Vec<&str> = text.lines().rev().take(3).collect();
        return Err(failed(tail.join(" | ")));
    }
    std::fs::rename(&tmp, &out).map_err(|e| failed(e.to_string()))?;
    Ok(out)
}

/// One picture from the local image model: one sd.cpp process, one at a time on this machine (a file lock).
fn run_local_image(
    g: &Generator,
    input: &Value,
    dir: &Path,
    shot: &str,
) -> Result<PathBuf, JobErr> {
    use crate::provision as p;
    use base64::Engine;
    let failed = |e: String| {
        err(
            Exit::Error,
            format!("the local image model failed: {e}"),
            g.source.as_ref().map_or_else(
                || "Run `scriptvox setup image-local` again, or check the prompt and the reference picture.".to_string(),
                crate::model_add::Source::add_again,
            ),
        )
    };
    let added = g.source.as_ref();
    if let Some(b) = added.and_then(|_| g.why_not()) {
        return Err(err(Exit::NeedsInput, b.why, b.fix));
    }
    let out = dir.join(format!("{shot}.png"));
    if out.is_file() {
        return Ok(out);
    }
    std::fs::create_dir_all(dir).map_err(|e| failed(e.to_string()))?;
    let file = |n: &str| p::pin_file(n).ok_or_else(|| failed(format!("{n} is not installed")));
    // SD 1.x and 2.x were trained at 512; a larger picture repeats its subject.
    let small = added.is_some_and(|s| s.family != "sdxl");
    let (w, h) = match (input["aspect_ratio"].as_str(), small) {
        (Some("16:9"), false) => (1344, 768),
        (Some("1:1"), false) => (1024, 1024),
        (_, false) => (768, 1344),
        (Some("16:9"), true) => (896, 512),
        (Some("1:1"), true) => (512, 512),
        (_, true) => (512, 896),
    };
    // sd-cli picks the format from the name.
    let tmp = dir.join(format!("{shot}.tmp.png"));
    let mut c = std::process::Command::new(file("sd-cli")?);
    if let Some(s) = added {
        c.arg("-m")
            .arg(&s.file)
            .args(["--cfg-scale", "7.0", "--steps", "20"]);
    } else {
        c.arg("--diffusion-model")
            .arg(file("klein-4b-dit")?)
            .arg("--llm")
            .arg(file("klein-4b-text")?)
            .arg("--vae")
            .arg(file("klein-4b-vae")?)
            .args(["--cfg-scale", "1.0", "--steps", "4"]);
    }
    c.arg("-p")
        .arg(input["prompt"].as_str().unwrap_or_default())
        .args(["--offload-to-cpu", "--mmap"])
        .args(["-W", &w.to_string(), "-H", &h.to_string()])
        // sd-cli's own default is 42; a request's seed replaces it.
        .args(["-s", &input["seed"].as_u64().unwrap_or(42).to_string()])
        .arg("-o")
        .arg(&tmp);
    let mut refs = Vec::new();
    for (i, url) in input["image_urls"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let data = url
            .as_str()
            .and_then(|u| u.split_once(";base64,"))
            .map(|u| base64::engine::general_purpose::STANDARD.decode(u.1))
            .ok_or_else(|| failed("a reference picture is not a local file".into()))?
            .map_err(|e| failed(e.to_string()))?;
        let f = dir.join(format!("{shot}.ref{i}"));
        std::fs::write(&f, data).map_err(|e| failed(e.to_string()))?;
        c.arg("-r").arg(&f);
        refs.push(f);
    }
    // The knob for a small machine: later flags win, such as `-W 512 -H 896 --vae-tiling -t 4`.
    if let Ok(extra) = std::env::var("SCRIPTVOX_SD_ARGS") {
        c.args(extra.split_whitespace());
    }
    let lock = p::data_dir().join("image-local.lock");
    let lock = std::fs::File::create(lock).map_err(|e| failed(e.to_string()))?;
    lock.lock().map_err(|e| failed(e.to_string()))?;
    let done = c.output().map_err(|e| failed(e.to_string()));
    drop(lock);
    for f in refs {
        let _ = std::fs::remove_file(f);
    }
    let done = done?;
    let text = String::from_utf8_lossy(&done.stderr);
    if out_of_memory(&done.status, &text) {
        let _ = std::fs::remove_file(&tmp);
        return Err(err(
            Exit::Error,
            "the local image model ran out of memory",
            "Close other programs and run again. For a smaller picture and less memory, set SCRIPTVOX_SD_ARGS=\"-W 512 -H 896 --vae-tiling\".",
        ));
    }
    if !done.status.success() || !tmp.is_file() {
        let tail: Vec<&str> = text.lines().rev().take(3).collect();
        return Err(failed(format!("{} {}", done.status, tail.join(" | "))));
    }
    std::fs::rename(&tmp, &out).map_err(|e| failed(e.to_string()))?;
    Ok(out)
}

/// Killed by the system's memory limit, or the process or its card said so.
fn out_of_memory(status: &std::process::ExitStatus, stderr: &str) -> bool {
    #[cfg(unix)]
    let killed = std::os::unix::process::ExitStatusExt::signal(status) == Some(9);
    #[cfg(not(unix))]
    let killed = false;
    let text = stderr.to_lowercase();
    !status.success()
        && (killed
            || status.code() == Some(137)
            || [
                "out of memory",
                "outofdevicememory",
                "bad_alloc",
                "failed to allocate",
                "not enough memory",
            ]
            .iter()
            .any(|w| text.contains(w)))
}

/// The file is here: the charge goes in the ledger and the job is marked done.
fn finish(project: &Path, mut job: Job, f: &ShotFiles) -> Result<(PathBuf, f64), JobErr> {
    let (c, _) = crate::run::open_state(project)
        .map_err(|e| err(Exit::Error, e.to_string(), "Check state.db."))?;
    crate::plan::settle(&c, job.reservation, Some(job.est_usd))
        .map_err(|e| err(Exit::Error, e.to_string(), "Check state.db."))?;
    job.done = Some(crate::run::now_utc());
    let _ = write_atomic(
        &f.job,
        &serde_json::to_string_pretty(&job).unwrap_or_default(),
    );
    Ok((f.video.clone(), job.est_usd))
}

#[allow(clippy::too_many_arguments)]
fn submit(
    g: &Generator,
    input: &Value,
    image: bool,
    key: &str,
    base: &str,
    queue_hosts: &[String],
    project: &Path,
    f: &ShotFiles,
    run: &str,
    cap: f64,
    est: Option<f64>,
) -> Result<Job, JobErr> {
    let endpoint = g.endpoint(image).ok_or_else(|| {
        err(
            Exit::Usage,
            format!(
                "{} has no {} endpoint",
                g.name,
                if image {
                    "image to video"
                } else {
                    "text to video"
                }
            ),
            "Pick another row (scriptvox providers).",
        )
    })?;
    let est = est.unwrap_or_else(|| row_est(g, input));
    let (mut c, _) = crate::run::open_state(project)
        .map_err(|e| err(Exit::Error, e.to_string(), "Check state.db."))?;
    let reservation = crate::plan::reserve(&mut c, run, &g.name, est, cap).map_err(|e| {
        err(
            Exit::ApprovalNeeded,
            e,
            "Approve a plan that covers this cost, or raise --max-spend.",
        )
    })?;
    let release = |c: &rusqlite::Connection| {
        let _ = crate::plan::settle(c, reservation, None);
    };
    if mark(f, input).is_err() {
        release(&c);
        return Err(err(
            Exit::Error,
            "cannot write the submit marker",
            "Check the run folder is writable.",
        ));
    }
    let sent = agent(g, 60)
        .post(&format!("{base}/{endpoint}"))
        .header("Authorization", &format!("Key {key}"))
        .header("Content-Type", "application/json")
        .send(input.to_string().as_bytes());
    let mut resp = match sent {
        Ok(r) => r,
        // The request may have reached the provider: keep the marker and the reservation.
        Err(e) => {
            return Err(err(
                Exit::UnconfirmedCharge,
                format!("submit did not get an answer: {e}"),
                format!(
                "Check your {} dashboard. To submit again anyway (it may charge twice), delete {} and give a --max-spend that covers both.",
                g.gateway,
                f.marker.display()
            ),
            ))
        }
    };
    let code = resp.status().as_u16();
    let body: Value = json_body(&mut resp).unwrap_or(Value::Null);
    if (400..500).contains(&code) {
        // A clear refusal: no job exists, nothing is charged.
        release(&c);
        let _ = std::fs::remove_file(&f.marker);
        let detail = body["detail"].to_string();
        let exit = if code == 401 || code == 403 {
            Exit::NeedsInput
        } else {
            Exit::Error
        };
        return Err(err(
            exit,
            format!(
                "{} refused the request: HTTP {code} {}",
                g.gateway,
                detail.chars().take(300).collect::<String>()
            ),
            if exit == Exit::NeedsInput {
                format!("Check the {} key: `scriptvox keys set {}`.", g.key, g.key)
            } else {
                "Check the prompt and the row's inputs.".into()
            },
        ));
    }
    let field = |k: &str| body[k].as_str().map(str::to_string);
    let (Some(request_id), Some(status_url), Some(response_url)) = (
        field("request_id"),
        field("status_url"),
        field("response_url"),
    ) else {
        return Err(err(
            Exit::UnconfirmedCharge,
            format!("submit answered HTTP {code} without a request id"),
            format!(
                "Check your {} dashboard. To submit again anyway (it may charge twice), delete {} and give a --max-spend that covers both.",
                g.gateway,
                f.marker.display()
            ),
        ));
    };
    let cancel_url = field("cancel_url");
    for u in [Some(&status_url), Some(&response_url), cancel_url.as_ref()]
        .into_iter()
        .flatten()
    {
        if !crate::http_rows::url_allowed(u, queue_hosts) {
            return Err(err(
                Exit::UnconfirmedCharge,
                format!("the provider returned a URL off its queue host: {u}"),
                "Check your dashboard; do not trust this answer.",
            ));
        }
    }
    let job = Job {
        generator: g.name.clone(),
        request_id,
        status_url,
        response_url,
        cancel_url,
        est_usd: est,
        reservation,
        submitted: crate::run::now_utc(),
        done: None,
        input: Some(input_id(input)),
    };
    write_atomic(
        &f.job,
        &serde_json::to_string_pretty(&job).unwrap_or_default(),
    )
    .map_err(|e| {
        err(
            Exit::UnconfirmedCharge,
            format!(
                "the job {} was queued but its record could not be written: {e}",
                job.request_id
            ),
            "Keep this request id; check your dashboard.",
        )
    })?;
    let _ = std::fs::remove_file(&f.marker);
    Ok(job)
}
