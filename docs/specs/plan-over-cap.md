# A plan over its cap stops at plan time (G174)

`generate --plan --max-spend 0.01` on a $0.35 row said `ok: true` and offered `--approve`. The refusal came only at the reserve, after the agent approved.

- `plan::over_cap` is the one check. `generate` and `make` call it for `--plan` and for a run with no valid approval: none, or an id that is not this plan's. `plan` calls it through `make --plan`.
- Result: exit 4, `SVX_SPEND_CAP`, the cost in `metrics.cost_usd`, and `next` with the cap to give. It never offers `--approve`.
- `generate` also gives up to two rows of the same kind that fit the cap, cheapest first, each a `--plan` command with the same request and its price after `#`. A row is left out when `why_not` is set, when its price is unknown, or when it cannot take this request (mode, frame, length, a source length nobody measured). If none fits, `next` has `providers --ready --free`.
- `make` has no single row kind, so its `next` always has `providers --ready --free`.
- The plan id stays on the envelope. The cap is not part of it, in `generate` or `make`: a plan buys its own bill at the plan's price, which the reserve holds, so a larger cap buys nothing more, and a rerun with a raised cap must find the same job instead of paying again (audit money-11 and the repair group, 9 Nov).
- `make` with cutaways (`cutaways.rs`) calls it too, for `--plan` and for a run with no valid approval.
- A cap equal to the cost plans. A row of unknown price costs its cap, so it never stops here.
- The ledger's reserve is unchanged and still holds at run time.
