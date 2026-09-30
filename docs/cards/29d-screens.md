# 29d — Stage C wiring: command palette (2.8), error screen (2.11), loading & reconnecting (2.12)

Branch `task/29d` (from origin/main @ 84162d5). Wires the three board-2 Stage B cards
(`design/stage-b/setup/cards/setup-08|11|12`) to the store + protocol through
`crates/octoscode-module/src/screens/palette.rs`. Report: `.peer/report-29d.md` (evidence: `tmp/29d-evidence/`).

## Contract (8.8 condition 2)

The cards name **binding ids** and **action ids**; the module owns the meaning:

| id | kind | meaning (web cite) |
|---|---|---|
| `palette.commands[/name/description/enabled]`, `palette.selected/count/query` | binding | the atlas slice /model /monitor /mode /compact /btw /resume; `enabled` = capability-gated (`registry.ts:52,679`, fail closed) |
| `palette.move` | action | ArrowUp/Down with wraparound (`CommandPalette.tsx:44-49`) |
| `palette.run` | action | route the command's native effect; unadvertised ⇒ named Unhandled |
| `palette.query.set` | action | the composer draft feeds the query box (UI-local) |
| `error.report`, `error.copy_state` | binding | redacted diagnostic (`FatalErrorBoundary.tsx:93-107`, 4000 cap) + copy state |
| `error.copy`, `error.copy_diagnostics` | action | copy the redacted report (the second id is the card's own control id, setup-11 service-actions.json) |
| `error.reload`, `connection.retry` | action | replay the production `Conversation::connect` handshake ("Reload app", `FatalErrorBoundary.tsx:71-74`) |
| `reconnect.banner/loading/retry` | binding | "Reconnecting… attempt N ·" (the module's own counter), "Loading session...", "Retry now" |

`/resume` is the one palette command with a native effect today (`session.refresh`, already on the router's production
path); the rest stay disabled until their domains land — fail closed, never a silent no-op.

## Mount (temporary until #28e)

`OCTOSCODE_SCREEN=palette|error|loading` mounts one card into the review column's `screen_splash` slot via the DESIGN
branch (the artifact Gate B rendered; these screens are fixed chrome, within the RULES 8.10 carve-out). Unset ⇒ the
screen is byte-identical to pre-#29d. `OCTOSCODE_ERROR_SEED` seeds a secrets-laden diagnostic so captures prove the
redaction boundary.

## Tests

`crates/octoscode-module/tests/f29d_screens.rs` — 5 tests: redaction+cap+copy_state; fail-closed gating; wraparound;
lowered live slots (via the DECLARED `palette.query.set` action); and `/resume` replayed over the committed
`live-gate-a6ea8505.jsonl` wire (the socket carries `session/list`; the store folds the recording's rows).
`f21_actions.rs` gained the `Effect::Screen` arm. Final `ctest`: 5/5 + 1/1, both ok.

## Headless evidence (entry item 3)

`examples/screens_probe` on the lane's port block 8371–8373 (`--remote`, hidden windows): `/snap`, `/g`, `/log?n=50`,
`/gq` per screen → `tmp/29d-evidence/{palette,error,loading}-{snap.json,shot.png,log.json,gq.json}` + app logs. Live
data in the captures: palette query box shows "/mo" (module draft), loading banner shows "Reconnecting… attempt 3"
(three driven retries). Clean exits, zero `[E]` lines.

Known environmental limits on the error capture (disclosed, not claimed ≥9): Inter-600 is not bundled in the module
(`resources/ux/` has 400/500/Mono) so the title falls back and clips, and the warning icon's asset host
(`http://127.0.0.1:8170/ux-images/...`) answers 501 — the holder process's cwd is `<HOME>/Octoscript-AppCard`,
which RULES forbid touching. Palette and loading self-score 9 against their Stage B v9 reviews.
