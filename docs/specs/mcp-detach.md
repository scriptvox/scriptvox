# MCP: make and generate go to the background (E7, section 6 row 5, 9 Nov 2026)

Before: `scriptvox_make` and `scriptvox_generate` ran the command and waited for it. The server handles one call at a time, so a long call blocked every other tool with no limit, and a client that timed out had no id to find the work.

- The server adds `--detach` to `make` and `generate`, unless the call is a `--plan`. The answer is `status: running`, the `run_id` and `next: scriptvox wait <id>`.
- `generate` gains `--detach`. It reuses the supervisor of `make`, and the run id is the plan's own, so a second call finds the same job and does not pay twice. A second start while the first is alive answers `running`.
- A paid plan with no approval, a missing key or service, and a usage error are answered at once by the plain command (exit 4, 10, 2). No job starts. The spend cap and approval do not change.
- `cancel` and `wait` work as for `make`.
- `make --detach` reports a stop for input at `wait`, as it always has. Since 10 Nov 2026 a paid plan with no approval, or one over its cap, stops at the start with exit 4 and no run, as the plain command does (test `make_detach_with_no_approval_stops_at_once_and_starts_no_run`); before, the MCP tool answered `running` and only `wait` said `approval_needed`.
- Windows: `make --detach` runs in the foreground with a warning, and `generate --detach` runs in the foreground (no supervisor there in v0.1), so the MCP call still blocks on Windows.

Not done: MCP progress notifications (later, per section 6).

Tests: `mcp::tests::make_and_generate_go_to_the_background_and_a_plan_does_not`, `generate::generate_detach_returns_the_run_id_and_approval_still_stops_it`.

10 Nov 2026: `remake` goes to the background too, see `remake-detach.md`.
