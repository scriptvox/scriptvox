# Contributing

1. Install Rust (stable), `just` and `uv`.
2. Run `git config core.hooksPath hooks` once. The hooks check the DCO sign-off and scan for keys.
3. Run `just ci` before each commit. It must be green.
4. Sign off each commit: `git commit -s`. This is the Developer Certificate of Origin (https://developercertificate.org).
5. You may study other projects for ideas. Do not copy code, prompts or text from AGPL, GPL or source-available projects. Read `docs/specs/idea-reuse.md`.

All contributions are Apache-2.0.
