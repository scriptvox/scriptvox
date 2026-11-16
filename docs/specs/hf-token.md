# Hugging Face token (G021, 7 Nov 2026)

A model behind an Agree button stops a download with a bare error today. `scriptvox setup` must say which page to open.

- `scriptvox keys set hf`, or `HF_TOKEN`. It lives with the other keys and is redacted the same way.
- `provision::install` sends the token only to `https://huggingface.co/`. A redirect to the file host drops it (ureq's default).
- A 401 or 403 from that host gets no retry. Every `setup` gives exit 10, `needs_input` with field `hf`, kind `secret`, the model page in `url`, and the step: click Agree, then run again. `doctor::setup_failure` is the one place for this.
- No pin is gated today. The two local setups stay tokenless. This serves the next gated row.

## Measured, 7 Nov 2026

| Request | Result |
|---|---|
| Gated file (FLUX.1-dev), no token | 401, `x-error-code: GatedRepo` |
| Same file, a made-up token | 401, `x-error-code: GatedRepo` |
| Same model, its README | 200: the gate covers some files only |
| Public file, a made-up token | 307, same as no token: a bad token does not break public pins. Real `setup` got the file through the redirect, and the checksum stopped it as planned |
| `setup speech-local` with the gated file as a pin (lock override) | exit 10, the page, "click Agree"; with a token the text adds "the token may be wrong"; no token value in the output |

**Not measured: a real token with no Agree.** No real token is on this machine. Forum posts say 403. The code treats 401 and 403 the same, so either answer gives the same message. A real gated download with a good token did not run either.
