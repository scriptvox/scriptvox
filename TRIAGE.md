# Triage

1. Ask for `scriptvox doctor --redact --json` output (it hides paths and never shows keys).
2. Ask for the failing command with `--json`, and `.scriptvox/runs/<id>/events.jsonl`.
3. Label: `bug`, `arabic`, `windows`, `provider`, `docs`, `good first issue`.
4. A provider break (Edge, Pexels, Pixabay, ElevenLabs) is a data change in `registry/` when possible, not code.
5. Never paste user keys or private media into issues.
