# publish: an export folder to your own channel or repo (13 Nov 2026)

Plan items P3.1 and P3.3 of `docs/plan/MILESTONE-PARITY.md`. Owner decision of 13 Nov 2026: "direct publish" is open again, as an opt-in part.

Why: the last step of a video was by hand. `export` makes one folder to publish from (`docs/specs/export.md`). `publish` sends that folder.

```
scriptvox publish setup youtube [--channel "<name>"]
scriptvox publish youtube <export folder | run id> [--privacy private|unlisted|public]
                          [--made-for-kids true|false] [--ai-content true|false] [--category <id>]
scriptvox publish hf <export folder | run id> --repo <owner>/<name> [--type dataset|space] [--public]
```

## 1. Rules for every platform

- Opt-in. No other command uploads. `publish` does nothing until the person connected an account.
- The input is an export folder: `video.mp4` and `metadata.json`. A run id is accepted when `scriptvox-out/<run>-export` exists. `publish` does not export by itself: the person can look at the folder first.
- Private by default. A wider audience only with its flag.
- An upload is an outward action, so the person decides each one. The first call sends nothing. It stops with exit 4, status `approval_needed`, and one `needs_input` item of kind `approval`. The item shows where the upload goes, the title, the audience and the file, and a `publish_id`.
- The item shows every field that goes to the platform: for YouTube also the description, the tags, the category and the two declarations. A description above 300 characters is shown as its first 300 characters, its length and a hash; all of it is in `metadata.json` of the folder.
- The `publish_id` is a hash of all that the item shows, and of the content of the file. A change of the title, the audience, the target or the file gives another id. So a yes is for exactly this upload.
- `--approve <publish_id>` starts the upload. The same rule as for money: an agent must not approve by its own decision. It shows the item to the person and waits for a yes.
- `--plan` sends nothing, also with a `--approve` that fits: the answer is the same stop. `--local-only` stops after the approval check with exit 2: an upload needs the network.
- A link in the folder is refused with exit 2, before the stop for approval, and nothing is sent. It is not followed and not left out in silence: a link can name a file outside the folder, for example a key file. `video.mp4` and `metadata.json` must be plain files too.
- What the person approved is what goes. Each file is hashed again right before it is sent, with the hash of the `publish_id`. A large file and the YouTube video are hashed and sent through one open handle. A file that changed: exit 1, the upload stops, no commit and no video is made, and the old approval does not fit the new file.
- `publish.json` in the export folder holds the result for each platform. A second call with the same id sends nothing and gives the same result, with `already_published: true`.
- Secrets never go in argv. They are never printed: `redact.rs` replaces every value of `keys.toml`. An error message holds the HTTP code and one word of the answer (letters and digits only), never the body.
- `publish` is not an MCP tool. An agent can run the command. The setup is for the person only.

`publish.json`:

```json
{
  "schema": "scriptvox.publish/v1",
  "youtube": {"publish_id": "pub_...", "video_id": "...", "url": "https://youtu.be/...", "privacy": "private",
              "channel": "...", "title": "...", "ai_content": true, "at": "2026-11-13T18:00:00Z"},
  "hf": {"publish_id": "pub_...", "repo": "owner/name", "type": "dataset", "private": true,
         "commit": "...", "url": "...", "files": 5, "at": "..."}
}
```

## 2. YouTube

### 2.1 Your own OAuth client

ScriptVox ships no client id and no client secret. The owner of the channel makes the client one time, in the Google Cloud console:

1. Make a project (or use one).
2. "APIs and services", "Library": turn on "YouTube Data API v3".
3. "OAuth consent screen" (Google Auth Platform): user type External, add your own Google account as a test user. With the status "Testing" Google ends a refresh token after 7 days; "In production" removes that limit.
4. "Credentials", "Create credentials", "OAuth client ID", application type "Desktop app". Copy the client id and the client secret.
5. In a terminal: `scriptvox publish setup youtube --channel "<the name of your channel>"`.

Read on Google's pages before a public upload: the rules for an API project that Google did not audit, and the daily upload limit of the API (`developers.google.com/youtube/v3/docs/videos/insert`). This spec does not repeat numbers that Google can change.

### 2.2 `publish setup youtube`

- Only a person, only in a terminal. With no TTY, or in agent mode: exit 10, `needs_input` of kind `secret`, and `next` is the one command `scriptvox publish setup youtube`. There is no `--stdin` and no variable that passes this check.
- It asks for the client id and the client secret with the input hidden (the input path of `keys set`).
- It opens a port on `127.0.0.1` and prints one address of `accounts.google.com` on stderr. The person opens it in a browser on the same machine, chooses the channel and says yes. The request has PKCE (S256) and a random `state`. The port takes only the answer with that `state`. It waits 5 minutes at most.
- Scope: `https://www.googleapis.com/auth/youtube.upload` and no other. With this scope ScriptVox can add a video. It cannot read, change or delete anything of the channel.
- It stores `youtube_client_id`, `youtube_client_secret` and `youtube_refresh_token` in `keys.toml` (user-only file). The access token is never stored.
- The channel name: Google gives no channel data with the upload scope. So the name in the approval text is the name that the person gave with `--channel` or at the question. It is stored in `publish.toml` in the config folder. It is a label, not a check: the video goes to the channel that the person chose in the browser.
- To remove the access: `myaccount.google.com/permissions`, or delete the client in the console.

### 2.3 `publish youtube`

| Field of the video | From |
|---|---|
| Title | `metadata.json` `platforms.youtube.title`, else `title`. 100 characters at most. |
| Description | `platforms.youtube.description` (with chapters and hashtags), else `description`. 5,000 characters at most. |
| Tags | `tags` (`export --tags a,b`), while they fit in 450 characters. |
| `status.privacyStatus` | `private`. `--privacy unlisted` or `--privacy public` only when given. |
| `status.selfDeclaredMadeForKids` | `false`. `--made-for-kids true` only when given. |
| `status.containsSyntheticMedia` | `true` when `ai_parts` of `metadata.json` has a row that made a picture, a clip, music, a sound or a lip sync (`video.gen`, `image.gen`, `music.gen`, `sfx.gen`, `lipsync`). A voice alone gives `false`. `--ai-content true|false` sets it by hand. Field name read on Google's page of the `videos` resource, 13 Nov 2026. |
| `snippet.categoryId` | `22` (People and Blogs). `--category <id>` for another. |

`<` and `>` are replaced by a space in the title and the description: YouTube refuses them.

Steps after the approval:

1. One call of `oauth2.googleapis.com/token` gives an access token from the refresh token.
2. `POST https://www.googleapis.com/upload/youtube/v3/videos?uploadType=resumable&part=snippet,status` with the fields above. The answer gives the session address.
3. The session address is stored before the first byte (in `keys.toml`, because who has it can send to that upload).
4. One `PUT` sends the file.
5. The answer has the video id. `publish.json` is written, then the session is removed.

Faults:

- No answer, or HTTP 500, 502, 503, 504: the command asks the session how many bytes it has (`Content-Range: bytes */<size>`, answer 308 with `Range`), and sends the rest. 3 tries in one command, with 1 s and 2 s between them. Then exit 5. The next run of the same command asks the session again and goes on from that byte.
- HTTP 401: one new access token, then the same step again.
- HTTP 404 on the session: the session ended. A new one starts from byte 0.
- A stop after the last byte and before `publish.json`: the session is still stored. The next run asks it, gets the same video, and writes the file. So no second video.
- The access token goes only to the host of the upload, and a session address on another host is refused. No redirect is followed.

### 2.4 Errors

| Cause | Exit | Code | Fix |
|---|---|---|---|
| No approval, or an approval for another upload | 4 | `SVX_APPROVAL_NEEDED` | Ask the person, then `--approve <publish_id>`. |
| No account connected | 10 | `SVX_NEEDS_INPUT` | The person runs `scriptvox publish setup youtube`. |
| Setup with no terminal, or by an agent | 10 | `SVX_NEEDS_INPUT` | The same command, in the person's terminal. |
| Google refuses the stored sign-in (HTTP 400 or 401 at the token address) | 10 | `SVX_NEEDS_INPUT` | The setup again. |
| Not an export folder; bad `--privacy`, `--category`, true/false value; no title | 2 | `SVX_USAGE` | Names the flag, or `scriptvox export <run_id>`. |
| The upload stopped 3 times | 5 | `SVX_PROVIDER_RETRYABLE` | The same command again with the same `--approve`. |
| YouTube refuses (quota, no channel, API off) | 1 | `SVX_ERROR` | The message has the HTTP code and the reason word. |

## 3. Hugging Face

`scriptvox publish hf <folder> --repo <owner>/<name> [--type dataset|space] [--public]`

- The token is the `hf` key (`scriptvox keys set hf`, or `HF_TOKEN`). It must be able to write. No token: exit 10 with that one command. The token goes only to `huggingface.co`. It does not go to the address that takes the bytes of a large file.
- What is sent: every file of the export folder, also in sub-folders (`hls/`). `publish.json` is not sent: it is the record of this machine.
- `--type space`: a static Space (sdk `static`, the free kind). When the folder has no `index.html`, one small page is added that plays `video.mp4` under the title. The approval item lists it.
- The approval item shows the repo, the type, the visibility and each file with its size. The `publish_id` covers the repo, the type, the visibility and the content of each file.
- Private by default. A repo that does not exist is made private; `--public` makes it public. A repo that exists and is public is refused without `--public` (exit 2, nothing is uploaded). A repo that exists and is private stays private also with `--public`, and the result says so.

Steps after the approval (paths read in the Hub's OpenAPI file, `huggingface.co/.well-known/openapi.json`, 13 Nov 2026):

1. `POST /api/repos/create` with `type`, `name`, `organization`, `private` (and `sdk` for a Space). HTTP 409: the repo exists, and `GET /api/<type>s/<repo>` gives its `private` field.
2. `POST /api/<type>s/<repo>/preupload/main` with the path, the size and the first 512 bytes of each file. The Hub says for each file `regular` or `lfs`.
3. Each `lfs` file: `POST /<type>s/<repo>.git/info/lfs/objects/batch` (the Git LFS batch API, transfer `basic`) with the SHA-256 and the size. An answer with an `upload` action gives a signed address: one `PUT` sends the file there, then the `verify` action when there is one. An answer with no action: the Hub has the bytes already, nothing is sent.
4. `POST /api/<type>s/<repo>/commit/main` as `application/x-ndjson`: a header line, a `file` line (base64) for each regular file, an `lfsFile` line (oid, size) for each large file.
5. `publish.json` gets the commit.

A fault (no answer, HTTP 408, 429, 5xx): exit 5. The same command again sends only the large files that did not arrive, then makes the one commit. HTTP 401 or 403 from the Hub: exit 10, the token cannot write there.

## 4. Limits and what is not done

- Written from memory and not checked on Google's pages on 13 Nov 2026: the names of the console steps in 2.1, the 7 days of a refresh token in "Testing", the 500 characters of tags, the refusal of `<` and `>`, and the wider scope of `thumbnails.set`. Checked on that day: the resumable upload protocol, the loopback OAuth flow with PKCE, the fields of `videos.insert`, and that `channels.list` does not list the upload scope.
- Through MCP `publish` is not offered. The `make` tool shows the new flag as `from_topic`.

- No real upload was run: there is no OAuth client of the owner yet. All tests use a server on loopback (`SCRIPTVOX_YOUTUBE_BASE`, only `http://127.0.0.1:<port>`). The OAuth step in a real browser was not run.
- The browser must be on the machine that runs the setup (loopback). A machine with no browser: run the setup with an SSH port forward, or copy `keys.toml` and `publish.toml` from a machine that has one. No device flow: it needs another client type, and its scope list was not checked here.
- One request sends the rest of the file. No parts of a fixed size.
- No cover upload (`thumbnails.set` needs a wider scope), no captions upload, no playlist, no schedule (`publishAt`).
- Closed 13 Nov 2026: the random values of the OAuth step (PKCE verifier, `state`) come from the OS random source on every platform (crate `getrandom`, MIT or Apache-2.0, already in the tree). No fallback: with no random source the sign in stops with an error. Before, Linux read `/dev/urandom` and Windows used the keys of the standard hasher.
- Hugging Face: no real call was run (no token was read). Step 1's `GET` and step 3 are not in the Hub's OpenAPI file; step 3 is the public Git LFS batch API. A file above 5 GB is refused (one plain request; no multipart, no Xet protocol). A second upload of a changed folder adds a commit and does not remove files that are gone from the folder.
- TikTok and Instagram (P3.2) are not here: each needs the platform's app review.
