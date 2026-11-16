//! `scriptvox publish` (`docs/specs/publish.md`): send an export folder to a platform. Opt-in: nothing here runs
//! until the person set the platform up. An upload is an outward action, so each one stops for the approval of the
//! person. This file holds what the platforms share: the folder, `publish.json` and the stop.

use crate::envelope::Envelope;
use crate::exit::Exit;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, clap::Subcommand)]
pub enum PublishCmd {
    /// Connect your own account one time. Only a person can do this, in a terminal: an agent gets exit 10 and the
    /// one command to give to the person.
    ///
    /// `youtube`: you make your own Google OAuth client (type "Desktop app", YouTube Data API v3 on; the steps are
    /// in docs/specs/publish.md). ScriptVox ships no client. The command asks for the client id and the client
    /// secret (input hidden, never in argv), prints an address to open in your browser, and waits for your yes
    /// there. It asks for the scope `youtube.upload` only. The secrets go to keys.toml (user-only) and are never
    /// printed. `hf` needs no setup: `scriptvox keys set hf` with a token that can write.
    Setup {
        /// youtube.
        platform: String,
        /// The name of your channel, for the approval text. Google does not give it with the upload scope, so it is
        /// the name that you give here.
        #[arg(long)]
        channel: Option<String>,
    },
    /// Upload the video of an export folder to your YouTube channel. Private unless you say otherwise. Stops with
    /// exit 4 until the person approves: `--approve <publish_id>`.
    ///
    /// Title, description and tags come from `metadata.json` of the folder (`scriptvox export <run> --tags a,b`).
    /// The first call sends nothing: it shows the channel, the title, the privacy and the file, and a publish id
    /// made from them. Ask the person, then run the same command with `--approve <publish_id>`. An agent must not
    /// approve by its own decision. A broken upload goes on when you run the same command again. A folder that
    /// was uploaded is not uploaded again: `publish.json` in the folder holds the video id.
    Youtube {
        /// The export folder, or the run id when `scriptvox-out/<run>-export` exists.
        from: String,
        /// private (default), unlisted or public.
        #[arg(long, default_value = "private")]
        privacy: String,
        /// The "made for kids" declaration: true or false.
        #[arg(long, default_value = "false", value_name = "BOOL")]
        made_for_kids: String,
        /// The altered or synthetic content declaration: true or false. Default: true when a model made a picture,
        /// a clip, music or a sound of the run (`ai_parts` of metadata.json).
        #[arg(long, value_name = "BOOL")]
        ai_content: Option<String>,
        /// The YouTube category id (default 22, People and Blogs).
        #[arg(long, default_value = "22", value_name = "ID")]
        category: String,
    },
    /// Upload an export folder to a Hugging Face dataset or Space with your `hf` key. Private unless `--public`.
    /// Stops with exit 4 until the person approves: `--approve <publish_id>`.
    ///
    /// Needs `scriptvox keys set hf` with a token that can write. The first call sends nothing: it shows the repo,
    /// the visibility and the files. A repo that does not exist is made. A repo that is public is refused without
    /// `--public`. `publish.json` holds the commit, and the same folder is not sent a second time.
    Hf {
        /// The export folder, or the run id when `scriptvox-out/<run>-export` exists.
        from: String,
        /// The repo: <user or org>/<name>.
        #[arg(long)]
        repo: String,
        /// dataset (default) or space (a static page that plays the video).
        #[arg(long = "type", default_value = "dataset", value_name = "TYPE")]
        kind: String,
        /// Make a new repo public, and accept a repo that is public.
        #[arg(long)]
        public: bool,
    },
}

pub const COMMAND: &str = "publish";

pub fn usage(message: impl Into<String>, fix: &str) -> Envelope {
    Envelope::fail(COMMAND, Exit::Usage, message, Some(fix.to_string()))
}

pub fn run(action: PublishCmd, agent: bool, plan_only: bool) -> Envelope {
    match action {
        PublishCmd::Setup { platform, channel } => match platform.as_str() {
            "youtube" => crate::publish_youtube::setup(channel, agent),
            "hf" => usage(
                "hf needs no setup here",
                "Run `scriptvox keys set hf` in your terminal, with a token that can write.",
            ),
            other => usage(format!("{other} is not a platform"), "Use youtube."),
        },
        PublishCmd::Youtube {
            from,
            privacy,
            made_for_kids,
            ai_content,
            category,
        } => {
            let flag = |name: &str, v: &str| match v {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(Box::new(usage(
                    format!("{name} is true or false, not {v}"),
                    "Give true or false.",
                ))),
            };
            let kids = match flag("--made-for-kids", &made_for_kids) {
                Ok(b) => b,
                Err(e) => return *e,
            };
            let ai = match ai_content
                .as_deref()
                .map(|v| flag("--ai-content", v))
                .transpose()
            {
                Ok(b) => b,
                Err(e) => return *e,
            };
            match folder(&from) {
                Ok(f) => crate::publish_youtube::run(&f, &privacy, kids, ai, &category, plan_only),
                Err(e) => *e,
            }
        }
        PublishCmd::Hf {
            from,
            repo,
            kind,
            public,
        } => match folder(&from) {
            Ok(f) => crate::publish_hf::run(&f, &repo, &kind, public, plan_only),
            Err(e) => *e,
        },
    }
}

/// An export folder: `video.mp4` and `metadata.json` of `scriptvox export`.
pub struct Folder {
    pub dir: PathBuf,
    pub video: PathBuf,
    pub meta: Value,
}

/// The folder itself, else `scriptvox-out/<run>-export`. Nothing is exported here: the person sees the folder first.
pub fn folder(from: &str) -> Result<Folder, Box<Envelope>> {
    let named = PathBuf::from(from);
    let dir = if named.is_dir() {
        named
    } else {
        Path::new("scriptvox-out").join(format!("{from}-export"))
    };
    let fix = format!(
        "Run `scriptvox export <run_id>` first, then `scriptvox publish <platform> scriptvox-out/<run_id>-export`. Looked in {}.",
        dir.display()
    );
    // The two files that are read here must be plain files of this folder, not links to somewhere else.
    for name in ["video.mp4", "metadata.json"] {
        if std::fs::symlink_metadata(dir.join(name)).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Box::new(usage(
                format!("{name} in {} is a link, and a link is not uploaded. Nothing was sent", dir.display()),
                "Put a copy of the file in the folder in place of the link, or run `scriptvox export <run_id>` again.",
            )));
        }
    }
    let video = dir.join("video.mp4");
    let meta = std::fs::read_to_string(dir.join("metadata.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    match meta {
        Some(meta) if video.is_file() => Ok(Folder { dir, video, meta }),
        _ => Err(Box::new(usage(
            format!("{from} is not an export folder: it needs video.mp4 and metadata.json"),
            &fix,
        ))),
    }
}

/// `pub_` plus 16 hex of blake3 over everything that the person approves.
pub fn publish_id(inputs: &Value) -> String {
    format!(
        "pub_{}",
        &blake3::hash(inputs.to_string().as_bytes()).to_hex()[..16]
    )
}

/// What `publish.json` of the folder says about one platform. No file, or a file that does not parse: nothing.
pub fn record(dir: &Path, platform: &str) -> Value {
    std::fs::read_to_string(dir.join("publish.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .map_or(Value::Null, |v| v[platform].clone())
}

/// Writes one platform's entry, through a temp file. The other platform's entry stays.
pub fn save_record(dir: &Path, platform: &str, entry: Value) -> std::io::Result<()> {
    let path = dir.join("publish.json");
    let mut all = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({"schema": "scriptvox.publish/v1"}));
    all[platform] = entry;
    let tmp = dir.join("publish.json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&all).unwrap_or_default())?;
    std::fs::rename(tmp, path)
}

/// The stop before an upload: exit 4 with one `needs_input` item of kind `approval` that shows what goes where.
/// The same rule as for money: the person decides, and an agent never approves by itself.
pub fn approval(id: &str, what: &str, mut show: Value) -> Envelope {
    let wrong = crate::plan::approved().is_some_and(|a| a != id);
    let fix = format!("Run the same command with --approve {id}, only after the person says yes.");
    let mut env = Envelope::fail(
        COMMAND,
        Exit::ApprovalNeeded,
        if wrong {
            format!("the approval is for another upload: what is sent has changed. Nothing was sent. This upload is publish_id {id}")
        } else {
            format!("approval needed: an upload leaves this machine and other people can see it. Nothing was sent. publish_id {id}")
        },
        Some(fix.clone()),
    );
    env.status = "approval_needed";
    env.plan_id = Some(id.to_string());
    let mut item = crate::voices::needs_input(
        "approve",
        "approval",
        &format!(
            "{what} Show this to the person and ask. Give --approve {id} only after the person says yes. An agent \
             must not approve an upload by its own decision."
        ),
        vec![],
    );
    item["publish_id"] = json!(id);
    show["publish_id"] = json!(id);
    item["upload"] = show.clone();
    env.needs_input = vec![item];
    env.next = vec![
        "Ask the person to approve this upload. Go on only after the person says yes.".to_string(),
        fix,
    ];
    env.with("publish_id", json!(id)).with("upload", show)
}

/// An agent for the calls of a platform. No redirect is followed: a 308 is an answer of the resumable upload, and a
/// token must not travel to another host.
pub fn http(timeout_s: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(timeout_s)))
        .http_status_as_error(false)
        .max_redirects(0)
        .build()
        .into()
}

/// A short word from an error answer (`error.errors[0].reason`, `error.status` or `error`), letters and digits
/// only. The body itself is never printed.
pub fn reason(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    let word = v["error"]["errors"][0]["reason"]
        .as_str()
        .or(v["error"]["status"].as_str())
        .or(v["error"].as_str())
        .unwrap_or("");
    let clean: String = word
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || " _-".contains(*c))
        .take(60)
        .collect();
    if clean.is_empty() {
        String::new()
    } else {
        format!(" ({clean})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_changes_with_what_is_sent_and_the_record_keeps_both_platforms() {
        let a = publish_id(&json!({"title": "A", "privacy": "private"}));
        assert_ne!(a, publish_id(&json!({"title": "A", "privacy": "public"})));
        assert!(a.starts_with("pub_") && a.len() == 20);
        let d = tempfile::tempdir().unwrap();
        save_record(d.path(), "youtube", json!({"video_id": "v"})).unwrap();
        save_record(d.path(), "hf", json!({"commit": "c"})).unwrap();
        assert_eq!(record(d.path(), "youtube")["video_id"], "v");
        assert_eq!(record(d.path(), "hf")["commit"], "c");
        assert_eq!(record(d.path(), "x"), Value::Null);
    }

    #[test]
    fn an_error_body_gives_one_clean_word() {
        assert_eq!(
            reason(r#"{"error":{"errors":[{"reason":"quotaExceeded"}]}}"#),
            " (quotaExceeded)"
        );
        assert_eq!(reason(r#"{"error":"invalid_grant"}"#), " (invalid_grant)");
        assert_eq!(reason("<html>secret</html>"), "");
    }
}
