# Doctor: HyperFrames idea (Apache-2.0)

Source: `packages/cli/src/commands/doctor.ts` (heygen-com/hyperframes, Apache-2.0). Idea taken: a list of named checks, each with a status, a detail and a hint. No code copied: ScriptVox's doctor is Rust.

ScriptVox additions: `fix` plus `fix_safe` on every check (an agent may run a safe fix, otherwise exit 10 with `kind:"dependency"`), the FFmpeg licence class from `configuration:`, a real 1-frame encoder probe, the WSL project filesystem and the state.db placement, a test fetch that tells TLS interception apart from a blocked host, `mode_ready`, and `--redact`.
