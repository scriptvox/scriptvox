//! `connect` against a loopback OpenAI-compatible service. Nothing leaves this machine.
mod common;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

type Log = Arc<Mutex<Vec<(String, String, String)>>>;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest-of-a-picture";

/// (line, auth header, body) of each request are in the log. The service wants `secret` when `key` is true.
fn service(key: bool) -> (String, Log) {
    let log: Log = Arc::new(Mutex::new(vec![]));
    let seen = log.clone();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let me = base.clone();
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let (mut auth, mut len) = (String::new(), 0usize);
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                if h.trim().is_empty() {
                    break;
                }
                let low = h.to_ascii_lowercase();
                if low.starts_with("authorization:") {
                    auth = h["authorization:".len()..].trim().to_string();
                } else if let Some(v) = low.strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
            }
            let mut b = vec![0; len];
            r.read_exact(&mut b).unwrap();
            let body = String::from_utf8_lossy(&b).to_string();
            let path = line.split(' ').nth(1).unwrap_or("").to_string();
            seen.lock()
                .unwrap()
                .push((line.trim().to_string(), auth.clone(), body.clone()));
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(PNG);
            let (code, out): (u16, Vec<u8>) = if key && auth != "Bearer secret" {
                (401, b"{}".to_vec())
            } else if path == "/v1/models" {
                (
                    200,
                    json!({"data": [{"id": "flux.1-dev"}, {"id": "url-image"}, {"id": "gpt-4o"}]})
                        .to_string()
                        .into_bytes(),
                )
            } else if path == "/v1/audio/speech" {
                let mut wav = b"RIFF\0\0\0\0WAVE".to_vec();
                wav.resize(64, 0);
                (200, wav)
            } else if path == "/result.png" {
                (200, PNG.to_vec())
            } else if body.contains("response_format") {
                (400, br#"{"error":{"message":"Unknown parameter: 'response_format'.","param":"response_format"}}"#.to_vec())
            } else if body.contains("url-image") {
                (
                    200,
                    json!({"data": [{"url": format!("{me}/result.png")}]})
                        .to_string()
                        .into_bytes(),
                )
            } else {
                (
                    200,
                    json!({"data": [{"b64_json": b64}]})
                        .to_string()
                        .into_bytes(),
                )
            };
            let _ = write!(
                s,
                "HTTP/1.1 {code} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                out.len()
            );
            let _ = s.write_all(&out);
        }
    });
    (base, log)
}

fn runner(d: &std::path::Path) -> impl Fn(&[&str], Option<&str>) -> (i32, Value) + '_ {
    move |args, stdin| {
        let mut c = common::bin();
        c.current_dir(d)
            .env("SCRIPTVOX_CONFIG", d.join("cfg"))
            .env("SCRIPTVOX_DATA", d.join("data"))
            .arg("--json")
            .args(args);
        if let Some(i) = stdin {
            c.write_stdin(i);
        }
        let o = c.output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
        )
    }
}

#[test]
fn a_service_with_no_key_makes_a_picture_and_refused_fields_are_dropped() {
    let d = tempfile::tempdir().unwrap();
    let run = runner(d.path());
    let (base, log) = service(false);
    let (code, v) = run(&["connect", "mine", "--url", &format!("{base}/v1")], None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["added"],
        json!(["mine-flux.1-dev", "mine-url-image"]),
        "only the image models: {v}"
    );

    // Unknown price: the same admission as row 17.
    let (code, v) = run(
        &["generate", "--model", "mine-flux.1-dev", "--prompt", "a"],
        None,
    );
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("--max-spend"),
        "{v}"
    );
    let (code, v) = run(
        &[
            "generate",
            "--model",
            "mine-flux.1-dev",
            "--prompt",
            "a",
            "--max-spend",
            "1",
            "--plan",
        ],
        None,
    );
    assert_eq!(code, 0, "{v}");
    let plan = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(
        &[
            "generate",
            "--model",
            "mine-flux.1-dev",
            "--prompt",
            "a",
            "--max-spend",
            "1",
            "--approve",
            &plan,
        ],
        None,
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        std::fs::read(v["artifacts"]["image"].as_str().unwrap()).unwrap(),
        PNG
    );
    assert_eq!(
        v["metrics"]["cost_usd"], 1.0,
        "the cap is what is recorded: {v}"
    );
    let posts: Vec<_> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.0.starts_with("POST"))
        .cloned()
        .collect();
    assert_eq!(posts.len(), 2, "one refused field, one retry: {posts:?}");
    assert!(
        posts[0].2.contains("response_format") && !posts[1].2.contains("response_format"),
        "{posts:?}"
    );

    // A URL result, through `providers test` and the same run path.
    let (code, v) = run(
        &[
            "providers",
            "test",
            "mine-url-image",
            "--max-spend",
            "1",
            "--plan",
        ],
        None,
    );
    assert_eq!(code, 0, "{v}");
    let plan = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(
        &[
            "providers",
            "test",
            "mine-url-image",
            "--max-spend",
            "1",
            "--approve",
            &plan,
        ],
        None,
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        std::fs::read(v["artifacts"]["image"].as_str().unwrap()).unwrap(),
        PNG
    );
}

#[test]
fn a_service_with_a_key_stops_with_the_key_step_then_lists_with_the_key() {
    let d = tempfile::tempdir().unwrap();
    let run = runner(d.path());
    let (base, log) = service(true);
    let (code, v) = run(&["connect", "mine", "--url", &base], None);
    assert_eq!(code, 10, "no key name yet: {v}");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("--key-name"),
        "{v}"
    );

    let (code, v) = run(
        &[
            "connect",
            "mine",
            "--url",
            &base,
            "--key-name",
            "my-key",
            "--model",
            "flux.1-dev",
        ],
        None,
    );
    assert_eq!(code, 10, "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .contains("keys set my-key"),
        "{v}"
    );

    let (code, v) = run(&["keys", "set", "my-key", "--stdin"], Some("secret\n"));
    assert_eq!(code, 0, "{v}");
    let (code, v) = run(
        &[
            "connect",
            "mine",
            "--url",
            &base,
            "--key-name",
            "my-key",
            "--model",
            "flux.1-dev",
        ],
        None,
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["added"], json!(["mine"]), "{v}");
    assert_eq!(log.lock().unwrap().last().unwrap().1, "Bearer secret");
}

#[test]
fn a_key_never_crosses_a_network_in_clear_text() {
    let d = tempfile::tempdir().unwrap();
    let run = runner(d.path());
    for url in ["http://example.com", "http://127.0.0.1:80@example.com"] {
        let (code, v) = run(&["connect", "mine", "--url", url], None);
        assert_eq!(code, 2, "{url}: {v}");
        assert!(v["error"]["fix"].as_str().unwrap().contains("https"), "{v}");
    }
}

/// A loopback ComfyUI: the graph is queued, the first history poll says it runs, the next that it is done.
fn comfy(missing: bool, clip: Vec<u8>) -> (String, Log) {
    let mut info = bare();
    if missing {
        info.as_object_mut().unwrap().remove("KSampler");
    }
    comfy_with(info, None, clip)
}

/// The node classes with no detail: nothing in a graph can be checked against it.
fn bare() -> Value {
    json!({"KSampler": {}, "CheckpointLoaderSimple": {}, "EmptyLatentImage": {}, "CLIPTextEncode": {}, "SaveImage": {}, "LoadImage": {}, "SaveVideo": {}, "WanImageToVideo": {}})
}

/// The same with this node catalogue. With `refuse`, a queued graph gets HTTP 400 and that body.
fn comfy_with(info: Value, refuse: Option<String>, clip: Vec<u8>) -> (String, Log) {
    let log: Log = Arc::new(Mutex::new(vec![]));
    let seen = log.clone();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    std::thread::spawn(move || {
        let mut polls = 0;
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let mut len = 0usize;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                if h.trim().is_empty() {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
            }
            let mut b = vec![0; len];
            r.read_exact(&mut b).unwrap();
            let path = line.split(' ').nth(1).unwrap_or("").to_string();
            let body = String::from_utf8_lossy(&b).to_string();
            seen.lock()
                .unwrap()
                .push((line.trim().to_string(), String::new(), body));
            let mut code = 200;
            let out: Vec<u8> = if path == "/object_info" {
                info.to_string().into_bytes()
            } else if let (true, Some(body)) = (path == "/prompt", &refuse) {
                code = 400;
                body.clone().into_bytes()
            } else if path == "/upload/image" {
                let upload = "scriptvox-up.png";
                json!({"name": upload, "subfolder": "", "type": "input"})
                    .to_string()
                    .into_bytes()
            } else if path == "/prompt" {
                json!({"prompt_id": "job-1"}).to_string().into_bytes()
            } else if path == "/history/job-1" {
                polls += 1;
                if polls == 1 {
                    b"{}".to_vec()
                } else {
                    json!({"job-1": {"status": {"completed": true, "status_str": "success"},
                        "outputs": {"9": {
                            "videos": [{"filename": "out.mp4", "subfolder": "", "type": "output"}],
                            "images": [{"filename": "out.png", "subfolder": "", "type": "output"}]}}}})
                    .to_string()
                    .into_bytes()
                }
            } else if path.starts_with("/view?filename=out.png&type=output") {
                PNG.to_vec()
            } else if path.starts_with("/view?filename=out.mp4&type=output") {
                clip.clone()
            } else {
                b"{}".to_vec()
            };
            let _ = write!(
                s,
                "HTTP/1.1 {code} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                out.len()
            );
            let _ = s.write_all(&out);
        }
    });
    (base, log)
}

const SD_GRAPH: &str = r#"{
  "3": {"class_type": "KSampler", "inputs": {"seed": 1, "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0], "latent_image": ["5", 0]}},
  "4": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": "a.safetensors"}},
  "5": {"class_type": "EmptyLatentImage", "inputs": {"width": 1024, "height": 1024, "batch_size": 1}},
  "6": {"class_type": "CLIPTextEncode", "inputs": {"text": "old", "clip": ["4", 1]}},
  "7": {"class_type": "CLIPTextEncode", "inputs": {"text": "bad hands", "clip": ["4", 1]}},
  "9": {"class_type": "SaveImage", "inputs": {"images": ["3", 0], "filename_prefix": "x"}}
}"#;

#[test]
fn a_comfy_graph_runs_by_role_on_the_same_admission_path() {
    let d = tempfile::tempdir().unwrap();
    let run = runner(d.path());
    let wf = d.path().join("wf.json");
    std::fs::write(&wf, SD_GRAPH).unwrap();
    let wf = wf.to_str().unwrap();

    let (bad, _) = comfy(true, vec![]);
    let (code, v) = run(&["connect", "cf", "--url", &bad, "--workflow", wf], None);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("KSampler"),
        "{v}"
    );

    let (base, log) = comfy(false, vec![]);
    let (code, v) = run(&["connect", "cf", "--url", &base, "--workflow", wf], None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["kind"], "image.gen", "{v}");
    assert_eq!(v["roles"]["prompt"]["node"], "6", "{v}");

    // The same admission as every row with no known price.
    let (code, v) = run(&["generate", "--model", "cf", "--prompt", "a"], None);
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["fix"].as_str().unwrap().contains("--max-spend"),
        "{v}"
    );

    let args = [
        "generate",
        "--model",
        "cf",
        "--prompt",
        "rain",
        "--aspect",
        "16:9",
        "--seed",
        "7",
        "--max-spend",
        "1",
    ];
    let (code, v) = run(&[&args[..], &["--plan"]].concat(), None);
    assert_eq!(code, 0, "{v}");
    let plan = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(&[&args[..], &["--approve", &plan]].concat(), None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        std::fs::read(v["artifacts"]["image"].as_str().unwrap()).unwrap(),
        PNG
    );

    let posted = log
        .lock()
        .unwrap()
        .iter()
        .find(|e| e.0.starts_with("POST /prompt"))
        .cloned()
        .unwrap();
    let g: Value = serde_json::from_str(&posted.2).unwrap();
    assert_eq!(g["prompt"]["6"]["inputs"]["text"], "rain");
    assert_eq!(g["prompt"]["7"]["inputs"]["text"], "bad hands");
    assert_eq!(g["prompt"]["3"]["inputs"]["seed"], 7);
    assert_eq!(g["prompt"]["5"]["inputs"]["height"], 576);
}

#[test]
fn a_comfy_video_graph_uploads_the_first_picture_before_the_graph() {
    let Some(clip) = real_clip() else { return };
    let d = tempfile::tempdir().unwrap();
    let run = runner(d.path());
    let mut g: Value = serde_json::from_str(SD_GRAPH).unwrap();
    g["9"]["class_type"] = json!("SaveVideo");
    g["20"] = json!({"class_type": "LoadImage", "inputs": {"image": "old.png"}});
    let wf = d.path().join("wf.json");
    std::fs::write(&wf, g.to_string()).unwrap();
    std::fs::write(d.path().join("pic.png"), PNG).unwrap();
    let (base, log) = comfy(false, clip);
    let wf = wf.to_str().unwrap();
    let (code, v) = run(&["connect", "cv", "--url", &base, "--workflow", wf], None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["kind"], "video.gen", "{v}");
    assert_eq!(v["roles"]["image"]["node"], "20", "{v}");
    let args = [
        "generate",
        "--model",
        "cv",
        "--prompt",
        "rain",
        "--image",
        "pic.png",
        "--aspect",
        "16:9",
        "--max-spend",
        "1",
    ];
    let (code, v) = run(&[&args[..], &["--plan"]].concat(), None);
    assert_eq!(code, 0, "{v}");
    let plan = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(&[&args[..], &["--approve", &plan]].concat(), None);
    assert_eq!(code, 0, "{v}");
    let seen = log.lock().unwrap().clone();
    let at = |p: &str| seen.iter().position(|e| e.0.starts_with(p)).unwrap();
    assert!(at("POST /upload/image") < at("POST /prompt"), "{seen:?}");
    assert!(seen[at("POST /upload/image")].2.contains("PNG"), "{seen:?}");
    let posted: Value = serde_json::from_str(&seen[at("POST /prompt")].2).unwrap();
    assert_eq!(
        posted["prompt"]["20"]["inputs"]["image"],
        "scriptvox-up.png"
    );
}

/// A node catalogue with choice lists, as a server gives it: a list of the allowed values first in the slot.
fn catalogue() -> Value {
    json!({
        "KSampler": {"input": {
            "required": {"seed": ["INT", {"default": 0}], "model": ["MODEL"], "positive": ["CONDITIONING"],
                "sampler_name": [["euler", "euler_ancestral", "heun"], {}]},
            "optional": {"scheduler": [["normal", "karras"]]}}},
        "CheckpointLoaderSimple": {"input": {"required": {"ckpt_name": [[
            "sd_xl_base_1.0.safetensors", "sd_xl_refiner_1.0.safetensors", "dreamshaper_8.safetensors",
            "flux1-dev.safetensors", "v1-5-pruned.safetensors", "wan2.1.safetensors"]]}}},
        "EmptyLatentImage": {"input": {"required": {"width": ["INT", {}], "height": ["INT", {}]}}},
        "CLIPTextEncode": {"input": {"required": {"text": ["STRING", {"multiline": true}], "clip": ["CLIP"]}}},
        // A slot shape this tool does not know.
        "SaveImage": {"input": {"required": {"images": ["IMAGE"], "filename_prefix": {"odd": 1}}}},
        "LoadImage": {"input": {"required": {"image": [["on-server.png"]]}}}
    })
}

/// SD_GRAPH with a checkpoint, sampler and scheduler named as text.
fn choice_graph(ckpt: &str) -> Value {
    let mut g: Value = serde_json::from_str(SD_GRAPH).unwrap();
    g["4"]["inputs"]["ckpt_name"] = json!(ckpt);
    g["3"]["inputs"]["sampler_name"] = json!("euler");
    g["3"]["inputs"]["scheduler"] = json!("karras");
    g
}

#[test]
fn a_choice_the_server_does_not_have_stops_at_connect() {
    let d = tempfile::tempdir().unwrap();
    let run = runner(d.path());
    let wf = d.path().join("wf.json");
    std::fs::write(&wf, choice_graph("sd_xl_base.safetensors").to_string()).unwrap();
    let wf = wf.to_str().unwrap();
    let (base, log) = comfy_with(catalogue(), None, vec![]);
    for extra in [&["--dry-run"][..], &[]] {
        let args = [&["connect", "cf", "--url", &base, "--workflow", wf], extra].concat();
        let (code, v) = run(&args, None);
        assert_eq!(code, 2, "{v}");
        let why = v["error"]["message"].as_str().unwrap();
        for word in [
            "node 4",
            "CheckpointLoaderSimple",
            "ckpt_name",
            "sd_xl_base.safetensors",
        ] {
            assert!(why.contains(word), "{word}: {v}");
        }
        let fix = v["error"]["fix"].as_str().unwrap();
        assert!(fix.contains("sd_xl_base_1.0.safetensors"), "{v}");
        assert_eq!(fix.matches(".safetensors").count(), 5, "5 nearest: {v}");
    }
    assert!(
        !d.path().join("cfg/comfy/cf.json").exists(),
        "no row is kept"
    );
    assert_eq!(posts(&log), 0, "nothing is queued: {log:?}");
}

#[test]
fn choices_the_server_has_connect_and_links_are_not_checked() {
    let d = tempfile::tempdir().unwrap();
    let run = runner(d.path());
    let (base, _) = comfy_with(catalogue(), None, vec![]);
    let wf = d.path().join("wf.json");
    let wf_s = wf.to_str().unwrap();

    std::fs::write(&wf, choice_graph("sd_xl_base_1.0.safetensors").to_string()).unwrap();
    let (code, v) = run(
        &[
            "connect",
            "ok",
            "--url",
            &base,
            "--workflow",
            wf_s,
            "--dry-run",
        ],
        None,
    );
    assert_eq!(code, 0, "{v}");
    // Checkpoint, sampler, scheduler. The two prompt texts are not choices; the odd slot is counted.
    assert_eq!(v["choices"], json!({"checked": 3, "skipped": 1}), "{v}");
    let (code, v) = run(&["connect", "ok", "--url", &base, "--workflow", wf_s], None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["choices"], json!({"checked": 3, "skipped": 1}), "{v}");

    // A link in a choice slot is not a value. The picture a run uploads is not checked either.
    let mut g = choice_graph("sd_xl_base_1.0.safetensors");
    g["3"]["inputs"]["sampler_name"] = json!(["4", 0]);
    g["20"] = json!({"class_type": "LoadImage", "inputs": {"image": "on-my-disk.png"}});
    std::fs::write(&wf, g.to_string()).unwrap();
    let (code, v) = run(&["connect", "ln", "--url", &base, "--workflow", wf_s], None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["choices"], json!({"checked": 2, "skipped": 1}), "{v}");
}

#[test]
fn a_refused_graph_names_the_node_the_input_and_the_fix() {
    const SECRET: &str = "sk-test-secret-12345";
    let d = tempfile::tempdir().unwrap();
    let envs = [("MYCOMFY_API_KEY", SECRET)];
    let wf = d.path().join("wf.json");
    std::fs::write(&wf, SD_GRAPH).unwrap();
    let wf = wf.to_str().unwrap();
    let generate = |row: &str| {
        let args = [
            "generate",
            "--model",
            row,
            "--prompt",
            "rain",
            "--max-spend",
            "1",
        ];
        let (code, v) = run_env(d.path(), &envs, &[&args[..], &["--plan"]].concat());
        assert_eq!(code, 0, "{v}");
        let plan = v["plan_id"].as_str().unwrap().to_string();
        run_env(
            d.path(),
            &envs,
            &[&args[..], &["--approve", &plan]].concat(),
        )
    };

    // The refusal as the server gives it. Node 4 has the public shape; node 3 names its input apart.
    let refusal = json!({
        "error": {"type": "prompt_outputs_failed_validation",
            "message": format!("Prompt outputs failed validation {SECRET}")},
        "node_errors": {
            "4": {"errors": [{"type": "value_not_in_list",
                    "message": "Value not in list: ckpt_name: 'a.safetensors'"}],
                "class_type": "CheckpointLoaderSimple"},
            "3": {"errors": [{"type": "required_input_missing", "message": "Required input is missing",
                    "details": "sampler_name", "extra_info": {"input_name": "sampler_name"}}],
                "class_type": "KSampler"}}});
    let (base, log) = comfy_with(bare(), Some(refusal.to_string()), vec![]);
    let (code, v) = run_env(
        d.path(),
        &envs,
        &[
            "connect",
            "cf",
            "--url",
            &base,
            "--workflow",
            wf,
            "--key-name",
            "mycomfy",
        ],
    );
    assert_eq!(code, 0, "{v}");
    let (code, v) = generate("cf");
    assert_eq!(code, 1, "{v}");
    let why = v["error"]["message"].as_str().unwrap();
    for word in [
        "node 4 (CheckpointLoaderSimple)",
        "ckpt_name",
        "Value not in list",
        "node 3 (KSampler)",
        "sampler_name",
        "Required input is missing",
    ] {
        assert!(why.contains(word), "{word}: {v}");
    }
    let fix = v["error"]["fix"].as_str().unwrap();
    assert!(fix.contains("ckpt_name") && fix.contains("install"), "{v}");
    assert!(fix.contains("sampler_name") && fix.contains("--map"), "{v}");
    assert_eq!(
        v["node_errors"],
        json!([
            {"node": "3", "class": "KSampler", "input": "sampler_name", "type": "required_input_missing",
                "message": "Required input is missing: sampler_name"},
            {"node": "4", "class": "CheckpointLoaderSimple", "input": "ckpt_name", "type": "value_not_in_list",
                "message": "Value not in list: ckpt_name: 'a.safetensors'"}]),
        "{v}"
    );
    assert!(!v.to_string().contains(SECRET), "the key is not shown: {v}");
    let seen = log.lock().unwrap().clone();
    assert!(
        seen.iter().any(|e| e.0.starts_with("POST /prompt")),
        "{seen:?}"
    );

    // A body of another shape: the first of its text, as before.
    let (other, _) = comfy_with(bare(), Some(r#"{"detail": "nope"}"#.into()), vec![]);
    let (code, v) = run_env(
        d.path(),
        &envs,
        &["connect", "raw", "--url", &other, "--workflow", wf],
    );
    assert_eq!(code, 0, "{v}");
    let (code, v) = generate("raw");
    assert_eq!(code, 1, "{v}");
    let why = v["error"]["message"].as_str().unwrap();
    assert!(why.contains(r#"HTTP 400 {"detail": "nope"}"#), "{v}");
    assert!(
        v["error"]["fix"]
            .as_str()
            .unwrap()
            .starts_with("Fix the graph in ComfyUI"),
        "{v}"
    );
    assert!(v["node_errors"].is_null(), "{v}");
}

/// A real short clip, so a fake service's result passes the same gates a real one does.
fn real_clip() -> Option<Vec<u8>> {
    let ff = common::ready()?;
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("c.mp4");
    let st = std::process::Command::new(ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=1280x720:r=24:d=5",
        ])
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-y"])
        .arg(&out)
        .status()
        .unwrap();
    assert!(st.success());
    Some(std::fs::read(out).unwrap())
}

/// A loopback queue service: the job is queued, the first poll says it works, the next that it is done.
fn queue(clip: Vec<u8>) -> (String, Log) {
    let log: Log = Arc::new(Mutex::new(vec![]));
    let seen = log.clone();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let me = base.clone();
    std::thread::spawn(move || {
        let mut polls = 0;
        for mut s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let (mut len, mut key) = (0usize, String::new());
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                if h.trim().is_empty() {
                    break;
                }
                let low = h.to_ascii_lowercase();
                if let Some(v) = low.strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
                if let Some(v) = low.strip_prefix("x-api-key:") {
                    key = v.trim().to_string();
                }
            }
            let mut b = vec![0; len];
            r.read_exact(&mut b).unwrap();
            let path = line.split(' ').nth(1).unwrap_or("").to_string();
            seen.lock().unwrap().push((
                line.trim().to_string(),
                key,
                String::from_utf8_lossy(&b).to_string(),
            ));
            let out: Vec<u8> = match path.as_str() {
                "/v1/videos" => json!({"id": "job1"}).to_string().into_bytes(),
                "/v1/videos/job1" => {
                    polls += 1;
                    if polls == 1 {
                        json!({"status": "working"}).to_string().into_bytes()
                    } else {
                        json!({"status": "done", "output": {"url": format!("{me}/f.mp4")}})
                            .to_string()
                            .into_bytes()
                    }
                }
                _ => clip.clone(),
            };
            let _ = write!(
                s,
                "HTTP/1.1 200 X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                out.len()
            );
            let _ = s.write_all(&out);
        }
    });
    (base, log)
}

#[test]
fn a_data_file_row_runs_the_queue_with_the_key_in_its_one_header() {
    let Some(clip) = real_clip() else { return };
    let d = tempfile::tempdir().unwrap();
    let run = runner(d.path());
    let spec = json!({"kind": "video.gen", "auth": {"kind": "header", "name": "x-api-key"},
        "submit": {"path": "/v1/videos", "job_id_at": "/id",
            "body": {"prompt": "{prompt}", "len": "{seconds}", "image": "{image}"}},
        "poll": {"path": "/v1/videos/{job_id}", "status_at": "/status", "result_at": "/output/url", "result_as": "url",
            "states": {"working": "running", "done": "succeeded"}}});
    std::fs::write(d.path().join("s.json"), spec.to_string()).unwrap();
    let (base, log) = queue(clip);
    let add = [
        "connect",
        "dv",
        "--url",
        &base,
        "--spec",
        "s.json",
        "--key-name",
        "dvk",
    ];
    let (code, v) = run(&add, None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["takes_image"], true, "{v}");
    let (code, _) = run(&["keys", "set", "dvk", "--stdin"], Some("sekret\n"));
    assert_eq!(code, 0);
    let args = [
        "generate",
        "--model",
        "dv",
        "--prompt",
        "rain",
        "--aspect",
        "16:9",
        "--max-spend",
        "1",
    ];
    let (code, v) = run(&[&args[..], &["--plan"]].concat(), None);
    assert_eq!(code, 0, "{v}");
    let plan = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run(&[&args[..], &["--approve", &plan]].concat(), None);
    assert_eq!(code, 0, "{v}");
    assert!(v["artifacts"]["video"].is_string(), "{v}");
    let seen = log.lock().unwrap().clone();
    let post = seen
        .iter()
        .find(|e| e.0.starts_with("POST /v1/videos"))
        .unwrap();
    assert_eq!(post.1, "sekret", "the key is in the one header: {seen:?}");
    let body: Value = serde_json::from_str(&post.2).unwrap();
    assert_eq!(body["prompt"], "rain");
    assert!(body.get("image").is_none(), "no picture, no field: {body}");
    assert!(
        seen.iter()
            .filter(|e| e.0.contains("/f.mp4"))
            .all(|e| e.1.is_empty()),
        "the result host gets no key: {seen:?}"
    );
}

/// `scriptvox --json <args>` with these extra environment variables.
fn run_env(d: &std::path::Path, envs: &[(&str, &str)], args: &[&str]) -> (i32, Value) {
    let mut c = common::bin();
    c.current_dir(d)
        .env("SCRIPTVOX_CONFIG", d.join("cfg"))
        .env("SCRIPTVOX_DATA", d.join("data"))
        .env_remove("FAL_KEY")
        .arg("--json")
        .args(args);
    for (k, v) in envs {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null),
    )
}

fn posts(log: &Log) -> usize {
    log.lock()
        .unwrap()
        .iter()
        .filter(|e| e.0.starts_with("POST"))
        .count()
}

/// Finding 1: a built-in key, or a key that a row with another base uses, never goes to a new `--url`.
#[test]
fn connect_never_sends_a_key_that_belongs_to_another_service() {
    let d = tempfile::tempdir().unwrap();
    let (base, log) = service(false);
    let env = [("FAL_KEY", "fal-secret"), ("MINE_API_KEY", "mine-secret")];
    for extra in [&[][..], &["--dry-run"][..]] {
        let args = [
            &["connect", "x", "--url", &base, "--key-name", "fal"],
            extra,
        ]
        .concat();
        let (code, v) = run_env(d.path(), &env, &args);
        assert_eq!(code, 2, "{v}");
    }
    assert!(log.lock().unwrap().is_empty(), "{:?}", log.lock().unwrap());

    let (code, v) = run_env(
        d.path(),
        &env,
        &[
            "connect",
            "a",
            "--url",
            &base,
            "--key-name",
            "mine",
            "--model",
            "flux.1-dev",
        ],
    );
    assert_eq!(code, 0, "{v}");
    // The same key on the same base is fine; on another base it is not.
    let (code, v) = run_env(
        d.path(),
        &env,
        &[
            "connect",
            "a2",
            "--url",
            &base,
            "--key-name",
            "mine",
            "--model",
            "url-image",
        ],
    );
    assert_eq!(code, 0, "{v}");
    let (other, log2) = service(false);
    let (code, v) = run_env(
        d.path(),
        &env,
        &[
            "connect",
            "b",
            "--url",
            &other,
            "--key-name",
            "mine",
            "--model",
            "flux.1-dev",
        ],
    );
    assert_eq!(code, 2, "{v}");
    assert!(log2.lock().unwrap().is_empty());
}

/// Finding 2: `providers add` cannot point a built-in key at a new host, by a merge or by text in a value.
#[test]
fn providers_add_cannot_send_a_built_in_key_to_a_new_base() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("cfg/generators.toml");
    let add = |a: &[&str]| run_env(d.path(), &[], &[&["providers", "add"], a].concat());
    let (code, v) = add(&[
        "kling-2.6-pro",
        "--gateway",
        "selfhost",
        "--kind",
        "video.gen",
        "--t2v",
        "x",
        "--base",
        "https://evil.example",
    ]);
    assert_eq!(code, 2, "{v}");
    let (code, v) = add(&[
        "y",
        "--gateway",
        "selfhost",
        "--kind",
        "video.gen",
        "--t2v",
        "x",
        "--base",
        "https://evil.example\"\nkey = \"fal",
    ]);
    assert_eq!(code, 2, "{v}");
    let (code, v) = add(&[
        "z",
        "--gateway",
        "fal",
        "--kind",
        "video.gen",
        "--t2v",
        "fal-ai/y\"\nkey = \"pexels",
    ]);
    assert_eq!(code, 2, "{v}");
    assert!(!std::fs::read_to_string(&file)
        .unwrap_or_default()
        .contains("evil"));
}

/// Finding 3: a cap that is not a positive number is no cap.
#[test]
fn an_unknown_price_needs_a_real_cap() {
    let d = tempfile::tempdir().unwrap();
    let (base, log) = service(false);
    let (code, v) = run_env(
        d.path(),
        &[],
        &["connect", "m", "--url", &base, "--model", "flux.1-dev"],
    );
    assert_eq!(code, 0, "{v}");
    for cap in ["0", "-5", "NaN", "inf"] {
        let flag = format!("--max-spend={cap}");
        let (code, v) = run_env(
            d.path(),
            &[],
            &["generate", "--model", "m", "--prompt", "a", &flag],
        );
        assert_ne!(code, 0, "{cap}: {v}");
    }
    assert_eq!(posts(&log), 0);
}

/// Finding 4: a speech row of unknown price is not picked on its own, so no audio leaves.
#[test]
fn an_unknown_price_speech_row_is_not_picked() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let st = std::process::Command::new(&ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x240:d=2",
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=2",
            "-shortest",
            "-y",
        ])
        .arg(d.path().join("a.mp4"))
        .status()
        .unwrap();
    assert!(st.success());
    let (base, log) = service(false);
    let env = [
        ("FAL_KEY", "k"),
        ("SCRIPTVOX_FAL_BASE", base.as_str()),
        ("SCRIPTVOX_FFMPEG", ff.as_str()),
    ];
    let (code, v) = run_env(
        d.path(),
        &env,
        &[
            "providers",
            "add",
            "x",
            "--gateway",
            "fal",
            "--kind",
            "stt",
            "--t2v",
            "fal-ai/y",
        ],
    );
    assert_eq!(code, 0, "{v}");
    let (code, v) = run_env(
        d.path(),
        &env,
        &["edit", "a.mp4", "--captions", "karaoke", "--out", "b.mp4"],
    );
    assert_ne!(code, 0, "{v}");
    assert!(log.lock().unwrap().is_empty(), "{:?}", log.lock().unwrap());
}

/// Finding 5: a connected row with a real price is held to the cap.
#[test]
fn a_priced_connected_row_is_held_to_the_cap() {
    let d = tempfile::tempdir().unwrap();
    let (base, log) = service(false);
    std::fs::create_dir_all(d.path().join("cfg")).unwrap();
    std::fs::write(
        d.path().join("cfg/generators.toml"),
        format!("[[generator]]\nname = \"p\"\ngateway = \"openai-images\"\nkind = \"image.gen\"\nkey = \"none\"\nt2v = \"flux.1-dev\"\nbase = \"{base}\"\noutput = \"inline\"\nhosts = [\"127.0.0.1\"]\nprice = \"exact\"\nusd_each = 0.05\nprice_source = \"https://example.org\"\nprice_checked = \"2026-11-09\"\n"),
    )
    .unwrap();
    let a = [
        "generate",
        "--model",
        "p",
        "--prompt",
        "a",
        "--max-spend",
        "0.01",
    ];
    // The plan already says no (G174). The approved run below shows the reserve still holds on its own.
    let (code, v) = run_env(d.path(), &[], &[&a[..], &["--plan"]].concat());
    assert_eq!(code, 4, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run_env(d.path(), &[], &[&a[..], &["--approve", &id]].concat());
    assert_ne!(code, 0, "{v}");
    assert_eq!(posts(&log), 0, "{v}");
}

/// Finding 6: an approval covers the cap, the base and the key it was given for.
#[test]
fn an_approval_does_not_cover_a_bigger_cap_or_another_base() {
    let d = tempfile::tempdir().unwrap();
    let (base, log) = service(false);
    let (code, v) = run_env(
        d.path(),
        &[],
        &["connect", "m", "--url", &base, "--model", "flux.1-dev"],
    );
    assert_eq!(code, 0, "{v}");
    let a = |cap: &'static str| {
        vec![
            "generate",
            "--model",
            "m",
            "--prompt",
            "a",
            "--max-spend",
            cap,
        ]
    };
    let (_, v) = run_env(d.path(), &[], &[&a("1")[..], &["--plan"]].concat());
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run_env(
        d.path(),
        &[],
        &[&a("500")[..], &["--approve", &id]].concat(),
    );
    assert_ne!(code, 0, "{v}");
    assert_eq!(posts(&log), 0, "{v}");
    let f = d.path().join("cfg/generators.toml");
    let text = std::fs::read_to_string(&f).unwrap();
    let (other, log2) = service(false);
    std::fs::write(&f, text.replace(&base, &other)).unwrap();
    let (code, v) = run_env(d.path(), &[], &[&a("1")[..], &["--approve", &id]].concat());
    assert_ne!(code, 0, "{v}");
    assert_eq!(posts(&log2), 0, "{v}");
}

/// Finding 7: a row name that is a path is refused, before the name reaches a folder.
#[test]
fn a_model_name_cannot_be_a_path() {
    let d = tempfile::tempdir().unwrap();
    let body = br#"{"first_stage_model.a":{},"cond_stage_model.transformer.b":{}}"#;
    let mut f = (body.len() as u64).to_le_bytes().to_vec();
    f.extend(body);
    std::fs::write(d.path().join("m.safetensors"), f).unwrap();
    let (code, v) = run_env(
        d.path(),
        &[],
        &[
            "models",
            "add",
            "./m.safetensors",
            "--name",
            "../../../../tmp/x",
            "--dry-run",
        ],
    );
    assert_eq!(code, 2, "{v}");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("../"),
        "{v}"
    );
}

/// Finding 9b: an unknown-price row does not win the free pick of a cutaway.
#[test]
fn a_cutaway_picks_a_row_with_a_known_price() {
    let Some(ff) = common::ready() else { return };
    let d = tempfile::tempdir().unwrap();
    let st = std::process::Command::new(&ff)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=360x640:d=6:r=30",
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=6",
            "-shortest",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(d.path().join("talk.mp4"))
        .status()
        .unwrap();
    assert!(st.success());
    let (base, _log) = service(false);
    let env = [("FAL_KEY", "k"), ("SCRIPTVOX_FFMPEG", ff.as_str())];
    let (code, v) = run_env(
        d.path(),
        &env,
        &["connect", "m", "--url", &base, "--model", "flux.1-dev"],
    );
    assert_eq!(code, 0, "{v}");
    let b = json!({"base_video": "talk.mp4", "cutaways": [{"at": 1.0, "seconds": 2.0, "prompt": "a map", "still": true}]});
    std::fs::write(d.path().join("b.json"), b.to_string()).unwrap();
    let (code, v) = run_env(d.path(), &env, &["make", "--brief", "b.json", "--plan"]);
    assert_eq!(code, 0, "{v}");
}

/// Finding 9c: a speech row of unknown price records what it may have cost.
#[test]
fn an_unknown_price_voice_row_records_its_cap() {
    let d = tempfile::tempdir().unwrap();
    let (base, _log) = service(false);
    let (code, v) = run_env(
        d.path(),
        &[],
        &[
            "providers",
            "add",
            "sp",
            "--gateway",
            "openai-speech",
            "--kind",
            "tts",
            "--t2v",
            "tts-1",
            "--base",
            &base,
        ],
    );
    assert_eq!(code, 0, "{v}");
    let a = [
        "generate",
        "--model",
        "sp",
        "--prompt",
        "hello",
        "--max-spend",
        "1",
    ];
    let (code, v) = run_env(d.path(), &[], &[&a[..], &["--plan"]].concat());
    assert_eq!(code, 0, "{v}");
    let id = v["plan_id"].as_str().unwrap().to_string();
    let (code, v) = run_env(d.path(), &[], &[&a[..], &["--approve", &id]].concat());
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["metrics"]["cost_usd"], 1.0, "{v}");
}
