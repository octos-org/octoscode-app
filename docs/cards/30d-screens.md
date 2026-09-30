# 30d — Stage C wiring: board 3.8 Resume, 3.9 Attachments, 3.10 Side question

Branch `task/30d` (from origin/main @ `7294e5c`). Wires the three board-3 Stage B cards
(`design/stage-b/autonomy/cards/autonomy-{08,09,10}`) to the store + protocol through
`crates/octoscode-module/src/screens/sessions.rs`. Report: `.peer/report-30d.md` (evidence: `tmp/30d-evidence/`).

## Contract (8.8 condition 2, one-owner)

The cards name **binding ids** and **action ids**; this module owns the meaning. The six action ids live ONLY in
`sessions::ACTIONS` (the #29d3 rule) and route through the `sessions::is_action` arm in `perform_action`:

| id | kind | meaning (web cite) |
|---|---|---|
| `resume.rows[/title/meta]`, `resume.confirm`, `resume.pending` | binding | the store's first four sessions as candidates, listed WITHOUT creating or selecting (`resume-binding.ts:180`, parity 331); the confirm echoes the staged row's exact title (`resume-binding.ts:152`, parity 332) |
| `resume.stage` | action | stage the clicked row into the confirm dialog — a bare row NEVER opens |
| `resume.confirm` | action | `session/open` (+ the session/list refresh) via the production `Conversation::open_session`; never starts a turn (`resume-binding.ts:227`, parity 333); consumes the staged row |
| `resume.cancel` | action | hide the dialog without opening |
| `att.count`, `att.sizes` | binding | the draft line "<n> of 4 images • 20 MB max" (`attachment-drafts.ts:6`, parity 150) |
| `attachment.remove` | action | drop a draft attachment (UI-local, `attachment-drafts.ts:16`, parity 151) |
| `aside.header/question/answer/state` | binding | the aside panel (`registry.ts:186`, parity btw-2; `BtwAsidePanel.tsx:22`, btw-4); state hidden/asking/answered/unavailable (`lazy-btw-controller.ts:85`) |
| `aside.ask` | action | `session/btw` with the composer draft as the question (`btw.ts:77`); empty draft refused without consuming (`lazy-btw-controller.ts:85`); **fails closed without the advertisement** (`btw.ts:66`, btw-7) |
| `aside.dismiss` | action | hide the aside; a late answer stays hidden (`lazy-btw-controller.ts:110`, btw-5) |

Protocol effects go through [`Conversation::client`] — the same transport every live call takes — via the
`workspace::spawn` shape. The fail-closed gate mirrors `workspace.rs`'s advertised-gate (`store.capabilities()`).

## Tests

`crates/octoscode-module/tests/f30d_sessions.rs` — 6 tests: §1 resume stage/confirm on the recorded `session/list` row
grammar (staging never opens; confirm opens exactly once — count-based, connect's own `session/open` excluded); §2 the
aside on the wire + dismiss + empty-draft refusal; §3 fail-closed without the advertisement; §4 the one-owner rule
(every screen id absent from both conversation tables, pinned shapes `ACTIONS.len()==9`, `unrouted()==["answer.expand"]`);
§5 the attachment draft; §6 the lowered live slots. Final: `test result: ok. 6 passed; 0 failed`.

No new fixture was recorded (turns spent: **0**): resume rides the recordings' own `session/list` grammar
(r22-live/r3-session carry the frames); `session/btw` has no committed fixture on either side, so the fake server
answers with the octos-core `SessionBtwResult` shape (`ui_protocol.rs:3106`) — no model turns involved.

## Headless evidence (entry item 3)

`screens_probe` on the lane's block 8371–8373 (`--remote`, hidden windows, kit fonts in-crate #29a2, SVGs served
in-process on 8374): `/snap`, `/g`, `/log?n=50`, `/gq` per screen → `tmp/30d-evidence/{resume,attachments,aside}-*`.
Clean exits, `0` `[E]` lines. Live data in the captures: resume shows the store's rows ("Live row"/"Fix steer queue
drop…" with the derived `dsflash • 5m ago • 7 turns` meta grammar) + the staged confirm `Resume "Live row"?`;
attachments shows "1 of 4 images • 20 MB max"; aside shows the seeded question + answer.

Self-scores (pre-filter; the outer loop makes the final call), each against its latest Stage B review PNG:
**resume 9** (`autonomy-08-review-z1`), **attachments 9** (`autonomy-09-review-w1`; the 68% ring's white-on-dark style
and dark thumbnails match the Stage B render), **aside 9** (`autonomy-10-review-w1`).

## Pre-ACK suite (entry item 4)

`git fetch origin && git merge origin/main` → "Already up to date" (main still `7294e5c`, the branch point).
`ctest --workspace --lib --tests` → `exit=0`, `test result: ok` ×59, `FAILED` ×0, **total passed=400 failed=0**
(log `tmp/ws-30d.log`).
