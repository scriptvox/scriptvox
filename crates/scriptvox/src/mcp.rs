//! `scriptvox mcp`: most commands (`TOOLS`) as tools for any MCP client (Claude Desktop, Cursor, Codex), over stdio,
//! newline-delimited JSON-RPC 2.0. Each tool runs this binary with the given arguments and `--json`, and returns its
//! result envelope as the tool text, so the MCP surface can never drift from the CLI. Tool descriptions are the
//! clap help of each command.
//!
//! A client with no file tool still gets what the CLI gives: the skills as `scriptvox_guide`, as resources and as prompts
//! (all from the same text as `scriptvox skills install`), and a snapshot's picture as image content.
//!
//! Left out on purpose: `keys` (a key goes in only through `scriptvox keys set --stdin`, typed by the user), `uninstall`,
//! `init`, `mcp` itself, `bench`, `licenses`, `skills` and `fetch` (it downloads from an address, and its opt-in
//! `setup fetch` must come from the person: that target of the `setup` tool is refused here).

use serde_json::{json, Value};
use std::io::{BufRead, Write};

/// The commands offered as tools, in the order an agent uses them.
pub const TOOLS: [&str; 35] = [
    "connect",
    "research",
    "write",
    "plan",
    "make",
    "batch",
    "wait",
    "status",
    "cancel",
    "resume",
    "check",
    "snapshot",
    "judge",
    "analyze",
    "edit",
    "join",
    "compare",
    "clips",
    "dub",
    "export",
    "generate",
    "providers",
    "voices",
    "tools",
    "doctor",
    "schema",
    "setup",
    "media",
    "explain",
    "templates",
    "recipes",
    "models",
    "demo",
    "replay",
    "remake",
];

/// What a tool does to the world, one row per tool. (tool, read only, may overwrite or stop something, the same call gives
/// the same result, reaches a network, can cost money, what it writes.)
type Row = (&'static str, bool, bool, bool, bool, bool, &'static str);
const ANNOTATIONS: [Row; 35] = [
    // Both call the user's text row; a paid row stops for the approval. Neither replaces a file.
    ("research", false, false, false, true, true, "research.json or --out, a new file"),
    ("write", false, false, false, true, true, "brief.json or --out, a new file"),
    ("connect", false, false, true, true, false, "generators.toml in the config folder"),
    ("models", false, false, false, true, false, "scan: models-found.json, the scanned files are only read. add: generators.toml, and the model and sd.cpp in the data folder"),
    ("plan", true, false, true, false, false, "nothing"),
    (
        "make",
        false,
        true,
        false,
        true,
        true,
        "scriptvox-out/, .scriptvox/runs/; --out names a file it may replace",
    ),
    (
        "batch",
        false,
        true,
        false,
        true,
        true,
        "as make, one run per brief",
    ),
    ("wait", true, false, true, false, false, "nothing"),
    ("status", true, false, true, false, false, "nothing"),
    (
        "cancel",
        false,
        true,
        true,
        false,
        false,
        "stops a run; its files stay",
    ),
    (
        "resume",
        false,
        false,
        false,
        true,
        true,
        "a new run in .scriptvox/runs/",
    ),
    (
        "check",
        true,
        false,
        true,
        true,
        false,
        "nothing; --hear sends the sound to the speech row",
    ),
    (
        "snapshot",
        false,
        false,
        true,
        false,
        false,
        "a picture in scriptvox-out/",
    ),
    (
        "judge",
        true,
        false,
        false,
        true,
        false,
        "nothing; frames go to your vision service",
    ),
    (
        "analyze",
        false,
        false,
        true,
        true,
        false,
        ".scriptvox/analyze/; --like writes a brief; --transcript sends the sound to the speech row",
    ),
    (
        "edit",
        false,
        true,
        false,
        true,
        true,
        "scriptvox-out/; --out names a file it may replace; --background and --alpha send the picture to the matte row, --enhance to the upscale row, and a paid row needs --approve",
    ),
    (
        "join",
        false,
        true,
        true,
        false,
        false,
        "scriptvox-out/; --out names a file it may replace",
    ),
    (
        "clips",
        false,
        false,
        false,
        true,
        false,
        "scriptvox-out/; sends the sound and the words to your services",
    ),
    (
        "dub",
        false,
        true,
        false,
        true,
        true,
        "as make; --plan writes the line sheet",
    ),
    (
        "export",
        false,
        true,
        false,
        true,
        true,
        "the folder or file it is given; --cover-prompt calls an image row, and a paid row needs --approve",
    ),
    (
        "generate",
        false,
        false,
        false,
        true,
        true,
        "scriptvox-out/; a paid row needs --approve",
    ),
    ("compare", true, false, true, false, false, "nothing"),
    ("providers", false, false, true, false, false, "generators.toml with `add`; nothing otherwise"),
    (
        "voices",
        false,
        false,
        false,
        true,
        true,
        "nothing for the list; --preview writes one audio file in scriptvox-out/ (or --out, never over a different file), and a paid voice needs --approve",
    ),
    ("tools", true, false, true, false, false, "nothing"),
    (
        "doctor",
        true,
        false,
        true,
        true,
        false,
        "nothing without --fix; --fix downloads the pinned tools",
    ),
    ("schema", true, false, true, false, false, "nothing"),
    ("explain", true, false, true, false, false, "nothing"),
    // G135. `demo`: the first run on a machine may download the pinned FFmpeg, so it can reach a network.
    ("demo", false, false, true, true, false, "one short test video (scriptvox-out/, or --out)"),
    // `replay` renders from the cache of the recipe's run and makes no provider call.
    ("replay", false, false, true, false, false, "a video rendered again from the cache, no provider call"),
    // `remake` is `make` from the recipe in a file: the same row as `make`, and the server sends it to the background.
    (
        "remake",
        false,
        true,
        false,
        true,
        true,
        "a new video (scriptvox-out/, or --out, a file it may replace), .scriptvox/runs/; a paid recipe needs --approve",
    ),
    ("templates", true, false, true, false, false, "nothing"),
    ("recipes", true, false, true, false, false, "nothing"),
    (
        "setup",
        false,
        false,
        true,
        true,
        false,
        "the data folder, or with `mcp` a client's config and a backup copy; needs --dry-run or the user's yes",
    ),
    (
        "media",
        false,
        false,
        true,
        true,
        false,
        "the project's media index; search reaches Openverse, and --get saves under scriptvox-out/open-media/",
    ),
];

fn annotations(tool: &str) -> Option<Value> {
    let &(_, read_only, destructive, idempotent, open_world, paid, writes) =
        ANNOTATIONS.iter().find(|r| r.0 == tool)?;
    Some(json!({
        "annotations": {"readOnlyHint": read_only, "destructiveHint": destructive,
            "idempotentHint": idempotent, "openWorldHint": open_world},
        "_meta": {"scriptvox/paid": paid, "scriptvox/writes": writes},
    }))
}

/// A download or a change to a client's config is the user's choice, as money is: `setup` runs only as `--dry-run`, or with `--yes` after the user said
/// yes to the size the dry run showed. `--yes` stops here: the CLI has no such flag. `None`: refused.
fn setup_args(args: &[String]) -> Option<Vec<String>> {
    if args.iter().any(|a| a == "--dry-run") {
        return Some(args.to_vec());
    }
    args.iter()
        .any(|a| a == "--yes")
        .then(|| args.iter().filter(|a| *a != "--yes").cloned().collect())
}

/// The clap command of a tool. `plan` is `make --plan`.
fn command_of<'a>(cli: &'a mut clap::Command, tool: &str) -> Option<&'a mut clap::Command> {
    cli.find_subcommand_mut(if tool == "plan" { "make" } else { tool })
}

/// Flags an agent has no use for here: the server adds `--json`, and clap adds help.
fn hidden_arg(a: &clap::Arg) -> bool {
    a.is_hide_set()
        || matches!(
            a.get_id().as_str(),
            "json" | "help" | "version" | "use_system"
        )
}

/// The first sentence of a help text, at most 160 characters.
fn first_sentence(text: &str) -> String {
    let text = text.trim();
    let end = text
        .match_indices('.')
        .map(|(i, _)| i + 1)
        .find(|&i| text[i..].starts_with([' ', '\n']) || i == text.len())
        .unwrap_or(text.len());
    let mut cut = end.min(160);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text[..cut].to_string()
}

/// The JSON type of one clap argument.
fn arg_schema(a: &clap::Arg) -> Value {
    use clap::builder::ValueParser;
    let many = matches!(a.get_action(), clap::ArgAction::Append)
        || a.get_num_args().is_some_and(|n| n.max_values() > 1);
    let id = a.get_value_parser().type_id();
    let is = |p: ValueParser| p.type_id() == id;
    let one = if is(clap::value_parser!(f64).into()) {
        "number"
    } else if is(clap::value_parser!(u32).into())
        || is(clap::value_parser!(u64).into())
        || is(clap::value_parser!(usize).into())
    {
        "integer"
    } else {
        "string"
    };
    let mut s = match a.get_action() {
        clap::ArgAction::SetTrue => json!({"type": "boolean"}),
        _ if many => json!({"type": "array", "items": {"type": one}}),
        _ => json!({"type": one}),
    };
    // The list holds the first sentence of each help (G186: the list was 81 KB, read by a client at every start);
    // the whole text is `scriptvox_guide` with topic `help-<command>`.
    let mut text = first_sentence(&a.get_help().map_or(String::new(), |h| h.to_string()));
    if a.is_required_set() {
        text.insert_str(0, "Required. ");
    }
    if !text.is_empty() {
        s["description"] = text.into();
    }
    s
}

/// The input of a tool, from the clap tree, so it cannot drift from the command. `args` stays for one version.
fn input_schema(sub: &clap::Command, tool: &str) -> Value {
    let mut props = serde_json::Map::new();
    props.insert(
        "args".into(),
        json!({"type": "array", "items": {"type": "string"}}),
    );
    // These tools make no provider call and take no plan: the money and network flags, which every command accepts,
    // are left out of their list (G186). Given through `args`, they still work.
    const NO_SPEND: [&str; 17] = [
        "wait",
        "status",
        "cancel",
        "snapshot",
        "join",
        "compare",
        "tools",
        "doctor",
        "schema",
        "setup",
        "media",
        "explain",
        "templates",
        "recipes",
        "models",
        "demo",
        "replay",
    ];
    const SPEND_FLAGS: [&str; 6] = [
        "approve",
        "max_spend",
        "idempotency_key",
        "plan",
        "no_embed",
        "local_only",
    ];
    for a in sub.get_arguments().filter(|a| !hidden_arg(a)) {
        let id = a.get_id().as_str();
        if NO_SPEND.contains(&tool) && SPEND_FLAGS.contains(&id) {
            continue;
        }
        let mut schema = arg_schema(a);
        // Said once, in the description of `scriptvox_guide`, not in every tool.
        if SPEND_FLAGS.contains(&id) {
            schema.as_object_mut().map(|o| o.remove("description"));
        }
        props.insert(id.to_string(), schema);
    }
    let subs: Vec<String> = sub
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .filter(|n| n != "help")
        .collect();
    if !subs.is_empty() {
        props.insert(
            "subcommand".into(),
            json!({"type": "string", "enum": subs, "description": "Which part of this command. Its own flags go in `args`."}),
        );
    }
    if tool == "setup" || tool == "models" {
        props.insert(
            "yes".into(),
            json!({"type": "boolean", "description": "The user agreed to the size that the dry run showed."}),
        );
    }
    json!({"type": "object", "properties": props, "additionalProperties": false})
}

/// The command line from a tool's named fields. Err: a field this tool does not have.
fn typed_args(tool: &str, input: &Value) -> Result<Vec<String>, String> {
    let mut cli = crate::cli::tree();
    cli.build();
    let sub = command_of(&mut cli, tool).ok_or(format!("no tool {tool}"))?;
    let text = |v: &Value| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let mut positional: Vec<(usize, Vec<String>)> = Vec::new();
    let mut flags = Vec::new();
    let mut head = Vec::new();
    let mut raw = Vec::new();
    for (key, v) in input.as_object().into_iter().flatten() {
        match key.as_str() {
            "args" => {
                raw = v
                    .as_array()
                    .map_or(vec![], |a| a.iter().map(text).collect())
            }
            "subcommand" => head.push(text(v)),
            "yes" if tool == "setup" || tool == "models" => {
                if v == &json!(true) {
                    flags.push("--yes".to_string());
                }
            }
            _ => {
                let a = sub
                    .get_arguments()
                    .find(|a| a.get_id().as_str() == key && !hidden_arg(a))
                    .ok_or(format!("{tool} has no field {key}"))?;
                let vals: Vec<String> = match v {
                    Value::Array(l) => l.iter().map(text).collect(),
                    other => vec![text(other)],
                };
                match (a.get_long(), a.get_index()) {
                    (None, Some(i)) => positional.push((i, vals)),
                    (Some(long), _) if matches!(a.get_action(), clap::ArgAction::SetTrue) => {
                        if v == &json!(true) {
                            flags.push(format!("--{long}"));
                        }
                    }
                    (Some(long), _) => {
                        for val in vals {
                            flags.extend([format!("--{long}"), val]);
                        }
                    }
                    _ => return Err(format!("{tool} cannot take field {key}")),
                }
            }
        }
    }
    positional.sort_by_key(|p| p.0);
    head.extend(positional.into_iter().flat_map(|p| p.1));
    head.extend(flags);
    head.extend(raw);
    Ok(head)
}

/// One tool per command: named fields from the clap tree, or the command line in `args`.
pub fn tool_list() -> Vec<Value> {
    let mut cli = crate::cli::tree();
    cli.build();
    let mut tools: Vec<Value> = TOOLS
        .iter()
        .filter_map(|name| {
            // `plan` is `make --plan`; its help is make's.
            let sub = command_of(&mut cli, name)?;
            // Each flag's help is in the schema now, so this holds only the command's own text.
            let help = sub
                .get_long_about()
                .or(sub.get_about())
                .map_or(String::new(), |h| h.to_string());
            let help = help.split("\n\n").next().unwrap_or_default();
            let mut description = format!("{help} Whole help: scriptvox_guide, topic help-{name}.");
            if *name == "plan" {
                description.insert_str(
                    0,
                    "Plan a video and its cost without making it (make --plan). ",
                );
            }
            description.truncate(6000);
            let mut tool = json!({
                "name": format!("scriptvox_{name}"),
                "description": description,
                "inputSchema": input_schema(sub, name),
            });
            for (k, v) in annotations(name)?.as_object()? {
                tool[k] = v.clone();
            }
            Some(tool)
        })
        .collect();
    tools.push(json!({
        "name": "scriptvox_guide",
        "description": "The scriptvox skills: how to write a brief, prompts, edits, review, sound, Arabic and which row to use. With no topic it lists them and gives the main guide. Topic help-<command> (help-edit, help-make) gives the whole help of one tool: every field here has only its first sentence. Each tool takes named fields and answers with the JSON envelope (ok, status, error.fix, artifacts, metrics, findings). Fields with no text mean the same in every tool: plan (show the plan and its cost, run nothing), approve (the plan_id of a plan that costs money, after the user agreed), max_spend (a cap in USD), idempotency_key (the same key again gives the first result, no second charge), local_only (send nothing to a network service), no_embed (do not put the recipe in the MP4), args (older form: the command line as a list; use the named fields).",
        "inputSchema": {
            "type": "object",
            "properties": {"topic": {"type": "string", "description": format!("One of: {}", topics().join(", "))}},
            "additionalProperties": false
        }
    }));
    tools.push(json!({
        "name": "scriptvox_page",
        "description": "Read the rest of a big result. A result above 40 KB comes as a summary with page.result_id: give that id and the path of a big field. A list pages by items, a text by characters.",
        "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false},
        "_meta": {"scriptvox/paid": false, "scriptvox/writes": "nothing"},
        "inputSchema": {
            "type": "object",
            "properties": {
                "result_id": {"type": "string", "description": "Required. page.result_id from the summary."},
                "path": {"type": "string", "description": "Required. A JSON pointer, for example /transcript/sentences."},
                "offset": {"type": "integer", "description": "The first item or character. next_offset in the last page gives it."},
                "limit": {"type": "integer", "description": "How many. Default 100 items or 8000 characters."}
            },
            "required": ["result_id", "path"],
            "additionalProperties": false
        }
    }));
    tools
}

fn topics() -> Vec<String> {
    crate::skill::all().into_iter().map(|(n, _)| n).collect()
}

fn skill_text(name: &str) -> Option<String> {
    crate::skill::all()
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, t)| t)
}

/// The `description:` line of a skill's front matter.
fn skill_description(text: &str) -> &str {
    text.lines()
        .find_map(|l| l.strip_prefix("description:"))
        .map_or("", |d| d.trim().trim_matches('"'))
}

fn guide(topic: Option<&str>) -> (String, bool) {
    match topic {
        None => (
            format!(
                "Topics: {}.\n\n{}",
                topics().join(", "),
                crate::skill::render()
            ),
            false,
        ),
        Some(t) => match skill_text(t)
            .or_else(|| skill_text(&format!("scriptvox-{t}")))
            .or_else(|| command_help(t.strip_prefix("help-")?))
        {
            Some(text) => (text, false),
            None => (
                format!("unknown topic {t}. Topics: {}.", topics().join(", ")),
                true,
            ),
        },
    }
}

/// The whole help of one tool's command, as `scriptvox <command> --help` prints it.
fn command_help(name: &str) -> Option<String> {
    let mut cli = crate::cli::command();
    cli.build();
    TOOLS.contains(&name).then_some(())?;
    Some(command_of(&mut cli, name)?.render_long_help().to_string())
}

/// A picture up to this size goes to the client as image content; a client limits what it takes.
const MAX_IMAGE: u64 = 5_000_000;

/// The picture named in a snapshot's envelope, as MCP image content. A client with no file tool cannot open a path.
fn image_content(envelope: &str) -> Option<Value> {
    use base64::Engine as _;
    let v: Value = serde_json::from_str(envelope).ok()?;
    let path = std::path::PathBuf::from(v["artifacts"]["image"].as_str()?);
    let mime = match path.extension()?.to_str()? {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => return None,
    };
    if std::fs::metadata(&path).ok()?.len() > MAX_IMAGE {
        return None;
    }
    let data = base64::engine::general_purpose::STANDARD.encode(std::fs::read(&path).ok()?);
    Some(json!({"type": "image", "data": data, "mimeType": mime}))
}

/// A result above this size goes to the client as a summary, and the rest through `scriptvox_page`. Measured 7 Nov 2026: 2.9 KB
/// a minute for `analyze --transcript`, so 25 min is 73 KB, and the largest other result is 19 KB. Clients warn at about
/// 10,000 tokens.
const MAX_TEXT: usize = 40_000;
/// Results the server keeps for paging. ponytail: memory only, last 8; a client that needs more runs the tool again.
const KEPT: usize = 8;
static KEPT_RESULTS: std::sync::Mutex<(u64, Vec<(String, Value)>)> =
    std::sync::Mutex::new((0, Vec::new()));

/// The envelope with each big string and list cut down, and the sizes kept.
fn shrink(v: &Value) -> Value {
    match v {
        Value::String(t) if t.len() > 1000 => {
            let head: String = t.chars().take(300).collect();
            json!({"chars": t.chars().count(), "start": head})
        }
        Value::Array(a) if v.to_string().len() > 2000 => {
            json!({"items": a.len(), "first": a.iter().take(2).map(shrink).collect::<Vec<_>>()})
        }
        Value::Array(a) => a.iter().map(shrink).collect(),
        Value::Object(o) => o.iter().map(|(k, x)| (k.clone(), shrink(x))).collect(),
        other => other.clone(),
    }
}

/// A big JSON result: keep it, and give a summary that says how to read the rest. Other text stays as it is.
fn summary_first(text: String) -> String {
    if text.len() <= MAX_TEXT {
        return text;
    }
    let Ok(full) = serde_json::from_str::<Value>(&text) else {
        return text;
    };
    let mut small = shrink(&full);
    let Ok(mut kept) = KEPT_RESULTS.lock() else {
        return text;
    };
    kept.0 += 1;
    let id = format!("p{}", kept.0);
    kept.1.push((id.clone(), full));
    if kept.1.len() > KEPT {
        kept.1.remove(0);
    }
    small["page"] = json!({"result_id": id, "bytes": text.len(),
        "how": "A summary: big fields show their size and their start. Read one with scriptvox_page, for example path /transcript/sentences, offset 0."});
    small.to_string()
}

/// One page of a kept result: part of a list or of a text at a JSON pointer, no larger than `MAX_TEXT`.
fn page(a: &Value) -> Result<Value, String> {
    let id = a["result_id"].as_str().unwrap_or("");
    let path = a["path"].as_str().unwrap_or("");
    let offset = a["offset"].as_u64().unwrap_or(0) as usize;
    let kept = KEPT_RESULTS
        .lock()
        .map_err(|_| "the kept results are not readable")?;
    let (_, full) = kept.1.iter().find(|(i, _)| i == id).ok_or(format!(
        "no kept result {id}; the server keeps the last {KEPT}. Run the tool again."
    ))?;
    let at = full.pointer(path).ok_or(format!(
        "no field at {path:?}; a path looks like /transcript/sentences"
    ))?;
    Ok(match at {
        Value::Array(l) => {
            let limit = a["limit"].as_u64().unwrap_or(100) as usize;
            let (mut items, mut size) = (Vec::new(), 0);
            for x in l.iter().skip(offset).take(limit) {
                size += x.to_string().len();
                if size > MAX_TEXT && !items.is_empty() {
                    break;
                }
                items.push(x.clone());
            }
            let next = offset + items.len();
            json!({"path": path, "offset": offset, "total": l.len(), "items": items,
                   "next_offset": (next < l.len()).then_some(next)})
        }
        Value::String(t) => {
            let limit = (a["limit"].as_u64().unwrap_or(8000) as usize).min(MAX_TEXT / 4);
            let total = t.chars().count();
            let part: String = t.chars().skip(offset).take(limit).collect();
            let next = offset + part.chars().count();
            json!({"path": path, "offset": offset, "total": total, "text": part,
                   "next_offset": (next < total).then_some(next)})
        }
        other => json!({"path": path, "value": shrink(other)}),
    })
}

/// Folders given to `scriptvox mcp --read-root --write-root`. No flag: no limit, as before. Spec: `docs/specs/mcp-roots.md`.
#[derive(Debug, Default)]
pub struct Roots {
    pub read: Vec<std::path::PathBuf>,
    pub write: Vec<std::path::PathBuf>,
}
static ROOTS: std::sync::OnceLock<Roots> = std::sync::OnceLock::new();

/// The real path, for a file that does not exist yet too: its nearest real folder plus the rest. A `..` in the rest is
/// kept, so it matches no root.
fn real_path(p: &std::path::Path, cwd: &std::path::Path) -> std::path::PathBuf {
    let full = cwd.join(p);
    let mut at = full.as_path();
    let mut rest = Vec::new();
    loop {
        if let Ok(r) = at.canonicalize() {
            return rest.iter().rev().fold(r, |r, n| r.join(n));
        }
        match (at.parent(), at.file_name()) {
            (Some(up), Some(name)) => {
                rest.push(name.to_os_string());
                at = up;
            }
            _ => return full,
        }
    }
}

/// Each part of a value that names a file: `a.mp4@12:4,b.mp4` holds 2. ponytail: a value names a path only if it exists,
/// because a path that does not exist cannot be read; `--out` and the other write flags are checked as paths.
fn named_paths(value: &str, base: &[&std::path::Path]) -> Vec<std::path::PathBuf> {
    if value.contains("://") {
        return vec![];
    }
    std::iter::once(value)
        .chain(value.split([',', '@', '=', ';', ':']))
        .filter(|v| !v.is_empty() && v.len() < 4096)
        .flat_map(|v| base.iter().map(move |b| b.join(v)))
        .filter(|p| p.exists())
        .collect()
}

/// Every string in a JSON value.
fn strings_in<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
    match v {
        Value::String(s) => out.push(s),
        Value::Array(a) => a.iter().for_each(|x| strings_in(x, out)),
        Value::Object(o) => o.values().for_each(|x| strings_in(x, out)),
        _ => {}
    }
}

/// Why this call is refused under the roots, or None. A brief or other JSON file names files too, so those are read.
fn outside_roots(
    tool: &str,
    args: &[String],
    cwd: &std::path::Path,
    roots: &Roots,
) -> Option<String> {
    if roots.read.is_empty() && roots.write.is_empty() {
        return None;
    }
    let in_any = |p: &std::path::Path, l: &[std::path::PathBuf]| {
        let p = real_path(p, cwd);
        l.iter().any(|r| p.starts_with(r))
    };
    let readable = |p: &std::path::Path| in_any(p, &roots.read) || in_any(p, &roots.write);
    let say = |what: &str, p: &std::path::Path| {
        Some(format!(
            "{} is outside the {what} folders of this server: {:?}. The user sets them with `scriptvox mcp --read-root --write-root`.",
            p.display(),
            if what == "write" { &roots.write } else { &roots.read }
        ))
    };
    let writes = ANNOTATIONS
        .iter()
        .find(|r| r.0 == tool)
        .is_none_or(|r| !r.1);
    if writes && !in_any(cwd, &roots.write) {
        return say("write", cwd);
    }
    if !readable(cwd) {
        return say("read", cwd);
    }
    let flag_value = |names: &[&str]| -> Vec<String> {
        let mut out = Vec::new();
        for (i, a) in args.iter().enumerate() {
            for n in names {
                if a == n {
                    out.extend(args.get(i + 1).cloned());
                } else if let Some(v) = a.strip_prefix(&format!("{n}=")) {
                    out.push(v.to_string());
                }
            }
        }
        out
    };
    for v in flag_value(&["--out", "--dir", "--config", "--like"]) {
        if !in_any(std::path::Path::new(&v), &roots.write) {
            return say("write", std::path::Path::new(&v));
        }
    }
    // These write outside the project unless a flag above names the place.
    let first = args.first().map_or("", String::as_str);
    if tool == "setup" && first == "mcp" && flag_value(&["--config"]).is_empty() {
        return say(
            "write",
            std::path::Path::new("the client's own config file"),
        );
    }
    if tool == "snapshot"
        && flag_value(&["--out"]).is_empty()
        && !first.starts_with("r_")
        && !in_any(std::path::Path::new(first), &roots.write)
    {
        return say("write", std::path::Path::new(first));
    }
    for a in args {
        let value = a
            .split_once('=')
            .filter(|_| a.starts_with("--"))
            .map_or(a.as_str(), |x| x.1);
        for p in named_paths(value, &[cwd]) {
            if !readable(&p) {
                return say("read", &p);
            }
            if !p.extension().is_some_and(|e| e == "json") {
                continue;
            }
            let Some(doc) = std::fs::read_to_string(&p)
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            else {
                continue;
            };
            let mut inner = Vec::new();
            strings_in(&doc, &mut inner);
            let dir = p.parent().unwrap_or(cwd);
            for q in inner.iter().flat_map(|s| named_paths(s, &[cwd, dir])) {
                if !readable(&q) {
                    return say("read", &q);
                }
            }
        }
    }
    None
}

/// The server handles one call at a time, so a long `make`, `remake` or `generate` must not hold it: each goes to the
/// background and answers with the run id. A plan runs nothing, so it stays as it is. A storyboard is a few stills and
/// answers with the sheet itself: it stays in the foreground.
fn with_detach(cmd: &str, args: &[String]) -> Vec<String> {
    let mut args = args.to_vec();
    if matches!(cmd, "make" | "remake" | "generate")
        && !args
            .iter()
            .any(|a| a == "--plan" || a == "--detach" || a == "--storyboard")
    {
        args.push("--detach".into());
    }
    args
}

/// This call downloads files or changes a client's config, so it needs `--dry-run` or `--yes`.
fn gated_download(cmd: &str, args: &[String]) -> bool {
    cmd == "setup" || (cmd == "models" && args.iter().any(|a| a == "add"))
}

fn call(name: &str, args: &[String]) -> (String, bool) {
    let Some(cmd) = name
        .strip_prefix("scriptvox_")
        .filter(|c| TOOLS.contains(c))
    else {
        return (format!("unknown tool {name}"), true);
    };
    // The consent of `setup fetch` must come from the person: an agent can add `--yes` by itself.
    if cmd == "setup" && args.iter().any(|a| a == "fetch") {
        return (
            "Run `scriptvox setup fetch` in a terminal yourself.".into(),
            true,
        );
    }
    let setup;
    let args = if gated_download(cmd, args) {
        match setup_args(args) {
            Some(a) => {
                setup = a;
                setup.as_slice()
            }
            None => return (format!("{cmd} downloads files or changes a client's config. Run scriptvox_{cmd} with --dry-run, show the user the plan (the size and disk free, or the file and the entry), and only after their yes run it again with --yes."), true),
        }
    } else {
        args
    };
    // `--run-id` is the supervisor's own flag: it names a run folder and keeps the call in the foreground. A client
    // has no use for it, so it is refused here (as a flag and as `--run-id=...`).
    if args
        .iter()
        .any(|a| a == "--run-id" || a.starts_with("--run-id="))
    {
        return (
            "--run-id is not an argument for a client: scriptvox gives each run its id. Use the run_id of the answer with scriptvox_wait.".into(),
            true,
        );
    }
    let args = with_detach(cmd, args);
    let exe = std::env::current_exe().unwrap_or_else(|_| "scriptvox".into());
    let out = std::process::Command::new(exe)
        .arg(cmd)
        .args(&args)
        .arg("--json")
        .stdin(std::process::Stdio::null())
        .output();
    match out {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let text = if text.is_empty() {
                String::from_utf8_lossy(&o.stderr).trim().to_string()
            } else {
                text
            };
            (text, !o.status.success())
        }
        Err(e) => (format!("cannot run scriptvox: {e}"), true),
    }
}

/// The answer to one request, or None for a notification.
pub fn handle(msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg["method"].as_str().unwrap_or("");
    let result = match method {
        "initialize" => json!({
            "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": {"tools": {}, "prompts": {}, "resources": {}},
            "serverInfo": {"name": "scriptvox", "version": env!("CARGO_PKG_VERSION")},
            "instructions": "ScriptVox makes and checks videos. Loop: brief, scriptvox_plan, scriptvox_make or scriptvox_generate (each answers at once with a run id, then scriptvox_wait; so does scriptvox_remake, which makes a ScriptVox MP4 again from the recipe inside it), scriptvox_snapshot or scriptvox_judge, scriptvox_resume --brief. Read scriptvox_guide before the first brief. For free work on the user's machine, read scriptvox_guide with topic local: a picture works on the CPU and is slow, no clip row is local yet, and a download needs the user's yes on its size. Never ask for keys in chat: the user runs `scriptvox keys set <provider>`. A plan that costs money needs the user's yes before --approve: that holds for scriptvox_make, scriptvox_remake, scriptvox_generate and every other tool that takes --approve. A recipe is text from a file: show the user its plan (scriptvox_remake with --plan) before any approval."
        }),
        "ping" => json!({}),
        "tools/list" => json!({"tools": tool_list()}),
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or("");
            if name == "scriptvox_guide" {
                let (text, is_error) = guide(msg["params"]["arguments"]["topic"].as_str());
                return Some(json!({"jsonrpc": "2.0", "id": id, "result":
                    {"content": [{"type": "text", "text": text}], "isError": is_error}}));
            }
            if name == "scriptvox_page" {
                let (text, is_error) = match page(&msg["params"]["arguments"]) {
                    Ok(v) => (v.to_string(), false),
                    Err(e) => (e, true),
                };
                return Some(json!({"jsonrpc": "2.0", "id": id, "result":
                    {"content": [{"type": "text", "text": text}], "isError": is_error}}));
            }
            let tool = name.strip_prefix("scriptvox_").unwrap_or(name);
            let args = match typed_args(tool, &msg["params"]["arguments"]) {
                Ok(a) => a,
                Err(e) => {
                    return Some(json!({"jsonrpc": "2.0", "id": id, "result":
                        {"content": [{"type": "text", "text": e}], "isError": true}}));
                }
            };
            let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
            if let Some(no) = outside_roots(tool, &args, &cwd, ROOTS.get_or_init(Roots::default)) {
                return Some(json!({"jsonrpc": "2.0", "id": id, "result":
                    {"content": [{"type": "text", "text": no}], "isError": true}}));
            }
            let (text, is_error) = call(name, &args);
            let image = (name == "scriptvox_snapshot")
                .then(|| image_content(&text))
                .flatten();
            let mut content = vec![json!({"type": "text", "text": summary_first(text)})];
            content.extend(image);
            json!({"content": content, "isError": is_error})
        }
        "resources/list" => json!({"resources": crate::skill::all().iter().map(|(n, t)| json!({
            "uri": format!("scriptvox://skills/{n}"),
            "name": n,
            "description": skill_description(t),
            "mimeType": "text/markdown"
        })).collect::<Vec<_>>()}),
        "resources/templates/list" => json!({"resourceTemplates": []}),
        "prompts/list" => json!({"prompts": crate::skill::all().iter().map(|(n, t)| json!({
            "name": n,
            "description": skill_description(t)
        })).collect::<Vec<_>>()}),
        "resources/read" | "prompts/get" => {
            let uri = msg["params"]["uri"].as_str();
            let name = uri
                .and_then(|u| u.strip_prefix("scriptvox://skills/"))
                .or(msg["params"]["name"].as_str())
                .unwrap_or("");
            let Some(text) = skill_text(name) else {
                return Some(json!({"jsonrpc": "2.0", "id": id,
                                   "error": {"code": -32602, "message": format!("no skill {name}")}}));
            };
            match uri {
                Some(u) => {
                    json!({"contents": [{"uri": u, "mimeType": "text/markdown", "text": text}]})
                }
                None => {
                    json!({"messages": [{"role": "user", "content": {"type": "text", "text": text}}]})
                }
            }
        }
        _ => {
            return Some(json!({"jsonrpc": "2.0", "id": id,
                               "error": {"code": -32601, "message": format!("no method {method}")}}))
        }
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// Serve until stdin closes. Each root must exist.
pub fn serve(read: &[std::path::PathBuf], write: &[std::path::PathBuf]) -> std::io::Result<()> {
    let real = |l: &[std::path::PathBuf]| {
        l.iter()
            .map(|p| p.canonicalize())
            .collect::<std::io::Result<Vec<_>>>()
    };
    let _ = ROOTS.set(Roots {
        read: real(read)?,
        write: real(write)?,
    });
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&msg),
            Err(e) => Some(json!({"jsonrpc": "2.0", "id": null,
                                  "error": {"code": -32700, "message": format!("parse error: {e}")}})),
        };
        if let Some(r) = reply {
            writeln!(out, "{r}")?;
            out.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(method: &str, params: Value) -> Value {
        handle(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})).unwrap()
    }

    #[test]
    fn a_client_reads_the_skills_three_ways() {
        let r = ask("resources/list", json!({}));
        assert_eq!(
            r["result"]["resources"].as_array().unwrap().len(),
            crate::skill::all().len()
        );
        let r = ask(
            "resources/read",
            json!({"uri": "scriptvox://skills/scriptvox-brief"}),
        );
        assert!(r["result"]["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("brief"));
        let r = ask("prompts/list", json!({}));
        assert!(
            r["result"]["prompts"][1]["description"]
                .as_str()
                .unwrap()
                .len()
                > 10
        );
        let r = ask("prompts/get", json!({"name": "scriptvox-edit"}));
        assert_eq!(r["result"]["messages"][0]["role"], "user");
        let r = ask(
            "tools/call",
            json!({"name": "scriptvox_guide", "arguments": {"topic": "sound"}}),
        );
        assert_eq!(r["result"]["isError"], false);
        let r = ask(
            "tools/call",
            json!({"name": "scriptvox_guide", "arguments": {"topic": "nope"}}),
        );
        assert_eq!(r["result"]["isError"], true);
        assert_eq!(
            ask("prompts/get", json!({"name": "nope"}))["error"]["code"],
            -32602
        );
    }

    #[test]
    fn named_fields_are_typed_from_the_commands_and_make_a_command_line() {
        let tools = tool_list();
        let props = |t: &str| {
            tools
                .iter()
                .find(|x| x["name"] == format!("scriptvox_{t}"))
                .unwrap()["inputSchema"]["properties"]
                .clone()
        };
        let edit = props("edit");
        assert_eq!(edit["cut_silence"]["type"], "boolean");
        assert_eq!(edit["min_silence"]["type"], "number");
        assert_eq!(edit["zoom"]["type"], "array");
        assert_eq!(edit["input"]["type"], "string");
        assert!(edit["input"]["description"]
            .as_str()
            .unwrap()
            .starts_with("Required."));
        assert_eq!(props("join")["inputs"]["type"], "array");
        assert!(edit.get("json").is_none());
        assert_eq!(props("clips")["count"]["type"], "integer");
        assert_eq!(props("setup")["yes"]["type"], "boolean");
        assert!(props("media")["subcommand"]["enum"]
            .as_array()
            .is_some_and(|e| !e.is_empty()));
        for t in tools
            .iter()
            .filter(|t| t["name"] != "scriptvox_guide" && t["name"] != "scriptvox_page")
        {
            assert_eq!(
                t["inputSchema"]["additionalProperties"], false,
                "{}",
                t["name"]
            );
            assert!(t["inputSchema"]["properties"]["args"].is_object());
        }
        let a = typed_args(
            "edit",
            &json!({"input": "t.mp4", "cut_silence": true, "pad": 0.2, "zoom": ["1-3@1.2", "4-6@1.5"], "track": false}),
        )
        .unwrap();
        assert_eq!(a[0], "t.mp4");
        for want in [
            "--cut-silence",
            "--pad",
            "0.2",
            "--zoom",
            "1-3@1.2",
            "4-6@1.5",
        ] {
            assert!(a.iter().any(|x| x == want), "{want} in {a:?}");
        }
        assert!(!a.iter().any(|x| x == "--track"));
        assert!(typed_args("edit", &json!({"nope": 1}))
            .unwrap_err()
            .contains("nope"));
        assert!(typed_args("edit", &json!({"json": true})).is_err());
        // `--speed` is text since the ranges (G195): a number from a client still becomes the same argument.
        for (given, want) in [(json!(1.25), "1.25"), (json!("12-15@0.5"), "12-15@0.5")] {
            let a = typed_args("edit", &json!({"input": "a.mp4", "speed": given})).unwrap();
            assert_eq!(a, ["a.mp4", "--speed", want]);
        }
        let a = typed_args(
            "edit",
            &json!({"input": "a.mp4", "sfx": ["whoosh@2", "pop@3"]}),
        )
        .unwrap();
        assert_eq!(a, ["a.mp4", "--sfx", "whoosh@2", "--sfx", "pop@3"]);
        // The small operations (G133). `--loop` is the flag of the argument `loop_n`: a client gives `loop`.
        let a = typed_args(
            "edit",
            &json!({"input": "a.mp4", "hold": ["4@2"], "loop": 3, "tilt": -2.5, "remove_bars": true}),
        )
        .unwrap();
        for want in [["--hold", "4@2"], ["--loop", "3"], ["--tilt", "-2.5"]] {
            assert!(a.windows(2).any(|w| w == want), "{a:?}");
        }
        assert!(a.contains(&"--remove-bars".to_string()), "{a:?}");
        assert_eq!(
            typed_args("check", &json!({"args": ["r_1", "--hear"]})).unwrap(),
            ["r_1", "--hear"]
        );
        assert_eq!(
            typed_args("plan", &json!({"brief": "b.json"})).unwrap(),
            ["--brief", "b.json"]
        );
        assert_eq!(
            typed_args("media", &json!({"subcommand": "add", "args": ["d"]})).unwrap(),
            ["add", "d"]
        );
        assert_eq!(
            typed_args("setup", &json!({"what": "image-local", "yes": true}))
                .unwrap_or_default()
                .len(),
            2
        );
    }

    // G186: a client reads the whole list at every start. It was 81 KB on 13 Nov 2026.
    #[test]
    fn the_tool_list_stays_small_and_the_whole_help_is_one_call_away() {
        let list = tool_list();
        let bytes = serde_json::to_string(&list).unwrap().len();
        // 54,000 since 13 Nov 2026: `research` and `write` are 2 more tools (51,679 bytes with them).
        // Raised on 13 Nov 2026 for `recipes` (one more tool, about 700 bytes) and for 11 more fields (the caption
        // style on `edit`, `dub` and `clips`, the music cut on `edit`).
        assert!(bytes < 56_000, "tools/list is {bytes} bytes");
        let tool = |n: &str| list.iter().find(|t| t["name"] == n).unwrap().clone();
        // The first sentence keeps the values a field takes.
        let aspect = tool("scriptvox_make")["inputSchema"]["properties"]["aspect"]["description"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            aspect.contains("9:16") && aspect.contains("16:9"),
            "{aspect}"
        );
        // A tool that never spends does not list the money flags; one that can spend does.
        assert!(tool("scriptvox_status")["inputSchema"]["properties"]["approve"].is_null());
        assert!(tool("scriptvox_edit")["inputSchema"]["properties"]["approve"].is_object());
        // The whole help of a command, and an unknown one is an error.
        let (text, bad) = guide(Some("help-edit"));
        assert!(
            !bad && text.contains("--cut-silence") && text.len() > 5000,
            "{}",
            text.len()
        );
        assert!(guide(Some("help-nothing")).1);
        assert_eq!(first_sentence("One. Two."), "One.");
        assert_eq!(first_sentence("Size 0.5 to 2"), "Size 0.5 to 2");
    }

    #[test]
    fn every_tool_says_what_it_does_to_the_world() {
        for t in TOOLS {
            assert!(annotations(t).is_some(), "{t} has no row");
        }
        assert_eq!(
            tool_list().len(),
            TOOLS.len() + 2,
            "a tool with no row would drop out"
        );
        let row = |t: &str| ANNOTATIONS.iter().find(|r| r.0 == t).unwrap();
        // Every row that can cost money reaches a network and is not read only.
        for r in ANNOTATIONS.iter().filter(|r| r.5) {
            assert!(!r.1 && r.4, "{}", r.0);
        }
        assert!(row("make").5 && row("generate").5 && row("dub").5);
        assert!(row("plan").1 && !row("plan").5);
        assert!(row("cancel").2 && row("edit").2);
        // A read only row overwrites nothing.
        assert!(ANNOTATIONS.iter().all(|r| !(r.1 && r.2)));
    }

    #[test]
    fn setup_runs_only_dry_or_after_a_yes() {
        let v = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(setup_args(&v(&["speech-local"])).is_none());
        assert_eq!(
            setup_args(&v(&["speech-local", "--dry-run"]))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            setup_args(&v(&["speech-local", "--yes"])).unwrap(),
            v(&["speech-local"])
        );
        assert!(gated_download(
            "models",
            &v(&["--approve", "x", "add", "org/m"])
        ));
        assert!(setup_args(&v(&["image-local"])).is_none());
        assert!(setup_args(&v(&["mcp", "--client", "cursor"])).is_none());
        assert_eq!(
            setup_args(&v(&["mcp", "--client", "cursor", "--yes"])).unwrap(),
            v(&["mcp", "--client", "cursor"])
        );
        assert_eq!(
            setup_args(&v(&["image-local", "--yes"])).unwrap(),
            v(&["image-local"])
        );
    }

    #[test]
    fn calls_stay_inside_the_roots() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        let (ro, rw, out) = (base.join("ro"), base.join("rw"), base.join("out"));
        for d in [&ro, &rw, &out] {
            std::fs::create_dir(d).unwrap();
        }
        std::fs::write(ro.join("a.mp4"), "x").unwrap();
        std::fs::write(out.join("secret.mp4"), "x").unwrap();
        let v = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let roots = Roots {
            read: vec![ro.clone()],
            write: vec![rw.clone()],
        };
        let no = |tool: &str, a: &[&str]| outside_roots(tool, &v(a), &rw, &roots);
        let path = |p: &std::path::Path| p.to_str().unwrap().to_string();
        let (a, secret) = (path(&ro.join("a.mp4")), path(&out.join("secret.mp4")));

        assert!(
            outside_roots("edit", &v(&[&secret]), &rw, &Roots::default()).is_none(),
            "no flag, no limit"
        );
        assert!(no("check", &[&a]).is_none());
        assert!(no("edit", &[&a, "--speed", "1.1", "--out", "e.mp4"]).is_none());
        assert!(no("check", &[&secret]).unwrap().contains("read"));
        assert!(
            no("edit", &[&a, &format!("--logo={secret}")]).is_some(),
            "flag=value"
        );
        assert!(
            no("edit", &[&a, "--broll", &format!("{secret}@12:4")]).is_some(),
            "file in a spec"
        );
        assert!(
            no("edit", &[&a, "--sfx", &format!("{secret}@1")]).is_some(),
            "a sound effect file"
        );
        assert!(no("edit", &[&a, "--sfx", "whoosh@1"]).is_none());
        assert!(no("check", &["../out/secret.mp4"]).is_some(), "dot dot");
        assert!(
            no("edit", &[&a, "--out", &path(&ro.join("e.mp4"))])
                .unwrap()
                .contains("write"),
            "read root is not a write root"
        );
        assert!(
            no("edit", &[&a, "--out", "../out/e.mp4"]).is_some(),
            "new file, dot dot"
        );
        assert!(no("snapshot", &[&a]).is_some(), "next to the video");
        assert!(no("snapshot", &[&a, "--out", "s.png"]).is_none());
        assert!(no("setup", &["mcp", "--client", "cursor", "--dry-run"]).is_some());
        assert!(
            no("make", &["--script", "https://x/y:80"]).is_none(),
            "a URL is not a path"
        );

        // A brief names files too.
        std::fs::write(rw.join("good.json"), format!("{{\"media\": {:?}}}", a)).unwrap();
        std::fs::write(
            rw.join("bad.json"),
            format!("{{\"scenes\": [{{\"media\": {:?}}}]}}", secret),
        )
        .unwrap();
        assert!(no("make", &["--brief", "good.json"]).is_none());
        assert!(no("make", &["--brief", "bad.json"])
            .unwrap()
            .contains("secret"));

        // Only reads allowed: a tool that writes is refused, and a read only one works.
        let read_only = Roots {
            read: vec![rw.clone()],
            write: vec![],
        };
        assert!(outside_roots("edit", &v(&["x.mp4"]), &rw, &read_only).is_some());
        assert!(outside_roots("status", &[], &rw, &read_only).is_none());
        // A project outside every root.
        assert!(outside_roots("status", &[], &out, &roots).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_a_root_is_refused() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        let (rw, out) = (base.join("rw"), base.join("out"));
        std::fs::create_dir(&rw).unwrap();
        std::fs::create_dir(&out).unwrap();
        std::fs::write(out.join("s.mp4"), "x").unwrap();
        std::os::unix::fs::symlink(out.join("s.mp4"), rw.join("l.mp4")).unwrap();
        let roots = Roots {
            read: vec![],
            write: vec![rw.clone()],
        };
        assert!(outside_roots("check", &["l.mp4".into()], &rw, &roots).is_some());
    }

    #[test]
    fn a_big_result_comes_as_a_summary_and_pages() {
        assert_eq!(summary_first("{\"a\":1}".into()), "{\"a\":1}");
        let not_json = "x".repeat(MAX_TEXT + 1);
        assert_eq!(summary_first(not_json.clone()), not_json);
        let words: Vec<String> = (0..9000).map(|i| format!("word {i}")).collect();
        let full =
            json!({"ok": true, "transcript": {"sentences": words, "text": "é".repeat(50_000)}});
        let small: Value = serde_json::from_str(&summary_first(full.to_string())).unwrap();
        assert!(small.to_string().len() < 2000, "{small}");
        assert_eq!(small["ok"], true);
        assert_eq!(small["transcript"]["sentences"]["items"], 9000);
        assert_eq!(small["transcript"]["text"]["chars"], 50_000);
        let id = small["page"]["result_id"].as_str().unwrap();

        let ask_page = |a: Value| {
            let r = ask(
                "tools/call",
                json!({"name": "scriptvox_page", "arguments": a}),
            );
            let t = r["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .to_string();
            (t, r["result"]["isError"] == true)
        };
        let (t, bad) = ask_page(
            json!({"result_id": id, "path": "/transcript/sentences", "offset": 10, "limit": 2}),
        );
        let p: Value = serde_json::from_str(&t).unwrap();
        assert!(!bad);
        assert_eq!(p["items"], json!(["word 10", "word 11"]));
        assert_eq!(
            (p["total"].as_u64(), p["next_offset"].as_u64()),
            (Some(9000), Some(12))
        );
        // A page stays small, and the pages together give the whole list.
        let (mut at, mut seen) = (0, 0);
        while let Some(n) = {
            let (t, _) = ask_page(
                json!({"result_id": id, "path": "/transcript/sentences", "offset": at, "limit": 5000}),
            );
            assert!(t.len() < 2 * MAX_TEXT);
            let p: Value = serde_json::from_str(&t).unwrap();
            seen += p["items"].as_array().unwrap().len();
            p["next_offset"].as_u64()
        } {
            at = n as usize;
        }
        assert_eq!(seen, 9000);
        let (t, _) =
            ask_page(json!({"result_id": id, "path": "/transcript/text", "offset": 49_999}));
        let p: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(p["text"], "é");
        assert!(p["next_offset"].is_null());
        let (t, bad) = ask_page(json!({"result_id": "nope", "path": "/a"}));
        assert!(bad && t.contains("no kept result"));
        let (t, bad) = ask_page(json!({"result_id": id, "path": "/nope"}));
        assert!(bad && t.contains("no field"));
    }

    #[test]
    fn a_snapshot_picture_becomes_image_content() {
        let f = std::env::temp_dir().join(format!("scriptvox-mcp-{}.png", std::process::id()));
        std::fs::write(&f, b"png").unwrap();
        let env = json!({"artifacts": {"image": f}}).to_string();
        let c = image_content(&env).unwrap();
        std::fs::remove_file(&f).unwrap();
        assert_eq!(c["mimeType"], "image/png");
        assert_eq!(c["data"], "cG5n");
        assert!(image_content(r#"{"artifacts": {}}"#).is_none());
    }

    #[test]
    fn make_and_generate_go_to_the_background_and_a_plan_does_not() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            super::with_detach("make", &a(&["--brief", "b"])),
            a(&["--brief", "b", "--detach"])
        );
        assert_eq!(super::with_detach("generate", &a(&[])), a(&["--detach"]));
        assert_eq!(super::with_detach("make", &a(&["--plan"])), a(&["--plan"]));
        assert_eq!(
            super::with_detach("make", &a(&["--brief", "b", "--storyboard"])),
            a(&["--brief", "b", "--storyboard"])
        );
        assert_eq!(
            super::with_detach("make", &a(&["--detach"])),
            a(&["--detach"])
        );
        assert_eq!(super::with_detach("check", &a(&["r"])), a(&["r"]));
        // `remake` can run for minutes too. `replay` renders from the cache and stays in the foreground.
        assert_eq!(
            super::with_detach("remake", &a(&["v.mp4", "--out", "o.mp4"])),
            a(&["v.mp4", "--out", "o.mp4", "--detach"])
        );
        assert_eq!(
            super::with_detach("remake", &a(&["v.mp4", "--plan"])),
            a(&["v.mp4", "--plan"])
        );
        assert_eq!(super::with_detach("replay", &a(&["v.mp4"])), a(&["v.mp4"]));
    }

    #[test]
    fn a_client_cannot_pass_the_supervisors_run_id() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        for args in [
            a(&["x.mp4", "--run-id", "../../escape"]),
            a(&["x.mp4", "--run-id=r_ab12cd"]),
        ] {
            for tool in ["scriptvox_remake", "scriptvox_make", "scriptvox_generate"] {
                let (text, is_error) = super::call(tool, &args);
                assert!(is_error && text.contains("--run-id"), "{tool}: {text}");
            }
        }
    }

    #[test]
    fn remake_is_a_tool_that_can_cost_money_and_the_user_is_asked_first() {
        let tools = tool_list();
        let t = tools
            .iter()
            .find(|t| t["name"] == "scriptvox_remake")
            .expect("scriptvox_remake is offered");
        let make = tools
            .iter()
            .find(|t| t["name"] == "scriptvox_make")
            .unwrap();
        assert_eq!(t["annotations"], make["annotations"]);
        assert_eq!(t["annotations"]["readOnlyHint"], false);
        assert_eq!(t["annotations"]["destructiveHint"], true);
        assert_eq!(t["annotations"]["openWorldHint"], true);
        assert_eq!(t["_meta"]["scriptvox/paid"], true);
        let props = &t["inputSchema"]["properties"];
        assert!(
            props["file"].is_object() && props["out"].is_object(),
            "{props}"
        );
        assert!(
            props["run_id"].is_null(),
            "the supervisor's flag is not offered: {props}"
        );
        let init = handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                                  "params": {"protocolVersion": "2025-06-18"}}))
        .unwrap();
        let text = init["result"]["instructions"].as_str().unwrap();
        assert!(text.contains("scriptvox_remake"), "{text}");
        assert!(
            text.contains("needs the user's yes before --approve"),
            "{text}"
        );
    }

    #[test]
    fn setup_fetch_is_refused_and_fetch_is_not_a_tool() {
        for args in [
            vec!["fetch"],
            vec!["fetch", "--dry-run"],
            vec!["fetch", "--yes"],
        ] {
            let args: Vec<String> = args.into_iter().map(String::from).collect();
            let (text, is_error) = call("scriptvox_setup", &args);
            assert!(is_error, "{args:?}");
            assert_eq!(text, "Run `scriptvox setup fetch` in a terminal yourself.");
        }
        // The way a client sends it: named fields.
        let typed = typed_args("setup", &json!({"what": "fetch", "yes": true})).unwrap();
        assert!(call("scriptvox_setup", &typed).1, "{typed:?}");
        assert!(!TOOLS.contains(&"fetch"));
        assert!(
            call(
                "scriptvox_fetch",
                &["https://example.org/a.mp4".to_string()]
            )
            .1
        );
    }

    #[test]
    fn every_tool_has_a_command_and_keys_are_not_offered() {
        let tools = tool_list();
        assert_eq!(
            tools.len(),
            TOOLS.len() + 2,
            "a tool name has no CLI command"
        );
        assert!(tools
            .iter()
            .all(|t| !t["name"].as_str().unwrap().contains("keys")));
        let init = handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                                  "params": {"protocolVersion": "2025-06-18"}}))
        .unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "scriptvox");
        assert!(
            handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none()
        );
        let bad = handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                                 "params": {"name": "scriptvox_keys", "arguments": {"args": ["set", "fal"]}}}))
        .unwrap();
        assert_eq!(bad["result"]["isError"], true);
    }
}
