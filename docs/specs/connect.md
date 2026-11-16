# Connect your own image service (section 6 row 25, E5, first runner)

`scriptvox connect <name> --url <base> [--key-name n] [--model id]... [--dry-run]` adds rows for an OpenAI-compatible image service. Custom endpoints were speech only.

- **URL.** https anywhere, http only on loopback. A user part before `@` is refused. The check is shared with the speech rows, which had the `@` hole.
- **Keys.** `--key-name` is refused, before any call, when it names a built-in key or a key that a row with another base uses. A row in generators.toml with `base` cannot use a built-in key other than `selfhost`.
- **List.** `GET <base>/v1/models`, sent with the key when it is set. 401 or 403 stops with the next step: a key name, then `keys set`. Image models are found by the words in their ids, and `--model` names any other.
- **Rows.** Gateway `openai-images`, one per model, name given or `<name>-<id>`. Terms not checked and price unknown, as row 17: `generate` needs `--max-spend`, and `providers test` runs the same path. A plan with an unknown-price row is still refused.
- **Admission.** `Generator::admission` is the one check that test, generate and make call. A whole plan passes no spend cap, so a row with no known price stops there, as before.
- **Runner.** `POST <base>/v1/images/generations` with model and prompt, and optional size, n and response_format. A 400 or 422 that names an optional field drops it, then the call repeats, at most once per field. A 400 is not a charge. Results: `b64_json` or `url` (https, or the service's own loopback host), and the bytes must be a picture. It takes the same reservation and submit marker as a queue row; a lost answer is exit 6 and never repeats by itself.
- **MCP.** `scriptvox_connect` in the same commit.

## ComfyUI (second runner, G047, 9 Nov 2026)

`scriptvox connect <name> --url <base> --workflow graph.json [--map role=node.input]... [--key-name n] [--dry-run]`. ComfyUI stays a separate program; ScriptVox only speaks HTTP to it.

- **Graph.** API format only (Export (API) in ComfyUI), at most 1 MB. The editor format is refused with the export step.
- **Roles.** `prompt`: the text node that reaches a sampler or guider on its positive side, followed back through conditioning links; never the negative side. `seed`: every numeric `seed` or `noise_seed`. `width`, `height`: the `Empty...` latent node. `length`: a numeric `length` on a video graph. `output`: the one `SaveImage`, `SaveVideo` or `VHS_VideoCombine`. A missing or ambiguous prompt or output is exit 2 and names `--map`. `--map` wins over what is found and must point at a literal input, not a link. Nothing else in the graph changes.
- **Checked at connect.** `GET /object_info`: every node class of the graph must be on the server, or exit 2 names the missing ones.
- **Row.** One row, gateway `comfyui`, `image.gen` or `video.gen` by the output node. Graph and roles in `<config>/comfy/<row>.json`. A missing file is a `dependency` in `why_not`, so `plan` and `generate` say it before a call.
- **Admission.** The same as the first runner: `Generator::admission`, unknown price, `--max-spend`, the same reservation and submit marker (`run_connected`). A 4xx on `/prompt` clears them. Lost answer, failed execution or timeout is exit 6 and never repeats by itself: a `--url` may be a paid cloud.
- **Request.** `POST /prompt`, poll `GET /history/<id>` every second up to the wait limit, `GET /view` for the one file (redirects off, at most 512 MB). A name or folder that could leave ComfyUI's `output` or `temp` folder is refused. The file must be a picture, or an mp4 on a video row. No seed given: a new one each call.
- **Size.** The graph's long side stays, the other comes from the aspect, both multiples of 16. Frames: 8n+1 from the seconds and the output node's `fps` or `frame_rate` (default 16).
- **URL.** The same rule as the first runner: http on loopback only. A ComfyUI on another machine goes through https or an SSH tunnel.
- **MCP.** `scriptvox_connect` takes `workflow` and `map`: its schema comes from the clap arguments.

- **Picture in.** A video graph with one `LoadImage`, or `--map image=<node id>.<input>`, gets `i2v`, so `generate --image` works. The picture goes to `POST /upload/image` under a name from its hash, before the graph is queued. A failed upload costs nothing and clears the reservation. With no `--image` the graph keeps the file it names. An image row does not take a picture.

Not in this slice: more than one output, an image graph with a picture in.

Ideas and small helpers from a crate under MIT OR Apache-2.0, used as Apache-2.0: `docs/provenance/comfyui-bridge.md`. Tests: `src/comfy.rs` (roles, length, refusals, unsafe paths) and `tests/connect.rs` (fake server). Real run: the binary against a local fake server, image and video rows, the posted graphs read. Not run: a real ComfyUI.

### Refused graphs and choices (10 Nov 2026)

- **A refused graph.** ComfyUI answers a bad graph on `POST /prompt` with HTTP 400, an `error` and `node_errors`. When the answer has `node_errors`, the message names each node (id and class), the input and the server's words, and `node_errors` in the JSON result has one entry for each: `node`, `class`, `input`, `type`, `message`. The fix fits the cause. A value that is not in the server's list: pick one of the server's values or install the model file on the server. A required input with no value: set it in the graph or with `--map`. Any other body gives the first 400 characters, as before. At most 8 nodes and 4 errors for each are read, and control characters are removed. `providers test` gives the same result. `make` gives the message and the fix, not the JSON list.
- **Where the input name comes from.** `extra_info.input_name` when the answer has it. If not, the input of that node in the sent graph that the message names. If neither, the entry has no input.
- **Choices checked at connect.** `/object_info` gives, for each node class, `input.required` and `input.optional`. A slot that starts with a list of texts is a choice. Each text value in the graph in such a slot must be in the list. If one is not, `connect` (also with `--dry-run`) stops with exit 2 before a row is written. It names the node, class, input and value, and up to 5 nearest values of the server. A link to another node is not checked. The picture role is not checked, because a run uploads its own file there.
- **What is not checked.** A slot that starts with a type name (`STRING`) is not a choice. A slot with any other shape, a `COMBO` slot, or no slot is skipped and counted. The result has `choices` with `checked` and `skipped`.
- **Sources.** Documentation pages only, no ComfyUI source. docs.comfy.org says that `/prompt` gives `error` and `node_errors` when validation fails, and that a choice input is a list of texts. The shape inside `node_errors` (`errors` with `type` and `message`, `class_type`) and the `[[...]]` slot in `/object_info` come from one public guide (runflow.io). `extra_info.input_name`, `details` and the type `required_input_missing` are in no page that was read: the code accepts them and does not need them.
- **Tests.** `tests/connect.rs` with the fake server: a refusal of the documented shape, a body of another shape, a model name the server does not list (no row, no POST), values that are all in the lists, a link in a choice slot. `src/comfy.rs`: slot shapes and answers that are not read. Each new test in `tests/connect.rs` failed before the change. Run: `just ci`. Not run: a real ComfyUI, so the real answer of a server was never seen.

## A service from a data file (third runner, row 25, 10 Nov 2026)

`scriptvox connect <name> --url <base> --spec file.json [--key-name n] [--dry-run]`. For a video or image API that queues jobs and is not OpenAI-shaped. One row, gateway `data-file`, the file kept in `<config>/spec/<row>.json`. The file is data. No code, no conditions.

```json
{ "kind": "video.gen",
  "auth": { "kind": "header", "name": "x-api-key" },
  "submit": { "path": "/v1/videos", "job_id_at": "/id",
    "body": { "model": "m1", "prompt": "{prompt}", "ratio": "{aspect}", "seconds": "{seconds}", "image": "{image}" } },
  "poll": { "path": "/v1/videos/{job_id}", "status_at": "/status", "result_at": "/output/url", "result_as": "url",
    "states": { "queued": "running", "working": "running", "done": "succeeded", "error": "failed" } } }
```

- **Auth.** `bearer` (the default), `header` with a name, or `none`. The key comes from `--key-name` only, never from the file, and goes only in that one header. A name the transport owns is refused.
- **Holes.** Only in the string leaves of `body`: `{prompt}`, `{aspect}`, `{seconds}`, `{seed}`, `{image}`. `{job_id}` only in the poll path, exactly once. Any other hole stops at `connect`. The body needs `{prompt}`. A leaf that is only `{seconds}` or `{seed}` stays a number. A leaf whose value this request does not have is left out, so a graph with `{image}` also runs with no picture. Filling is one pass, so a prompt cannot add a field or fill a hole.
- **`{image}`** gives the row `i2v`: the picture as a data URI.
- **Paths.** Start with `/`, on `base` only. No host, no `..`, no `//`. The poll address is built from the file, never from a URL in a reply. No redirects.
- **States.** Every word goes to `running`, `succeeded` or `failed`. A word the file does not list stops the run with exit 6, and is never guessed. At most 16.
- **Result.** A JSON pointer, as `url` (https or the service's own host, no key sent) or `base64`. It must be an mp4 or a picture.
- **Limits.** File at most 32 KB. Unknown fields are refused, by name. Replies at most 32 MB. Polls every 2 s up to the wait limit.
- **Admission.** The same as the other two runners. Unknown price, `--max-spend`, reservation and submit marker. A 4xx on submit clears them. A lost answer, a failed job or a timeout is exit 6 and never repeats by itself.

Not in this slice: a request/response flow, static headers, cancel, signing, multipart. Idea and limits from an Apache-2.0 design: `docs/provenance/data-file-adapter.md`. Tests: `src/data_row.rs`, `tests/connect.rs` (fake queue service). Not run: a real service.

Tests: `tests/connect.rs` (loopback service with and without a key, refused field, base64 and URL results, http refusal). Real run: the binary against a local fake server, scratch config, picture written. Not run: a real image service.

## A result URL on a machine inside a network is refused (10 Nov 2026, found by a review)

A connected service gives back a URL for its picture or clip. ScriptVox downloads it with no key. Before, any `https://` URL was taken, so a service could make the user's machine fetch from a host inside the user's own network. Now a result URL must be on a host the row lists, or be a public `https://` address: `http_rows::local_host` refuses an address in a private, loopback, link-local or shared range (IPv4 and IPv6), a name with no dot, and names under `.local`, `.localhost`, `.internal`, `.lan` and `.home.arpa`. A public file host that the row does not list still works, because such hosts cannot be listed in advance. Unit test in `http_rows.rs` (3 public and 16 local forms). Second pass on 11 Nov 2026, after the same review looked again:
- Forms a resolver can read as an address are local too: `127.1`, `0x7f.0.0.1`, `2130706433`, a last label of digits, and a name with one dot at its end.
- A public name is resolved, and it is refused when any of its addresses is private. The download resolves the name again, so a name that changes its answer between the two is not caught.
- The result download follows no redirect (`download_no_redirect`): a 3xx answer is an error, and the next host gets no request. Unit test `a_result_download_follows_no_redirect`; the address test now has 4 public and 24 local forms.
