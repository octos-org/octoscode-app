# Phase 3 evidence pack — main @ be44817 (card #38a, lane task/38a)

All numbers below are fresh from this branch (be44817), each with its command.
App binary: vendored rebuild of be44817 (`tmp/viz/21d/sync.sh` -> Finished; the
vendored workspace needed `octoscript-render` added to its `[patch]` section —
build scaffolding only, no app code changed). Captures: `docs/walk/evidence/phase3/`.

## 1. Suite numbers

| suite | command | result |
|---|---|---|
| unit+integration | `ctest --workspace --lib --tests` | **TOTAL passed=488 failed=0** (tmp/38a-ctest.log) |
| full walk | `python3 tools/walk/run.py --limit 999` | **rc=0, pass=115, fail=0 — specific=64 / smoke=51**, 35 distinct checks, 234 rows (committed b330142) |
| negative control | `WALK_SCENARIO=conversation=session --only conversation` | **rc=1 as required**: "a sent prompt streams an assistant answer row" + "the turn's timeline item kinds" FAIL (streaming timeout, no assistantprose) |

Walk split vs history: #33b 112 = 61+51 with 3 fails -> #34a/#37a/main now
**115 = 64+51 with 0 fails** (165/209/190 green since #34a). Zero FAIL rows ->
no new defects.md entries.

### Click audit (tools/walk/click_audit.py, fresh binary)

```
python3 tools/walk/click_audit.py --app-bin <vendored octosense> --out docs/walk/click-audit.csv
```

**SURFACE: 29 controls, 16 with an expected action: 2 respond, 8 dead,
0 dismissed-by-earlier-click, 6 shadowed-by-dock, 0 focus-only-bindings,
8 documented-unwired (+5 unmapped).**

Per surface (from docs/walk/click-audit.csv):

| surface | controls | respond | dead | shadowed | notes |
|---|---|---|---|---|---|
| dock-error | 8 | — | 2 (`error.reload`, `error.copy_diagnostics`) | 5 | host chrome under the visible dock = shadowed **by design** |
| dock-loading | 6 | — | — | 5 | same shadowing |
| dock-palette | 0 | — | — | — | no clickable controls |
| chrome-review | 8 | 1 (`send_hit` composer.submit) | 4 (`session.new`, `thread.open`, `review.toggle` ×2) | — | `review.toggle` click dead — see open items |
| chrome-settings | 7 | 1 (`send_hit`; `settings_disconnect` produced its log line) | 3 (`session.new`, `thread.open`, plus (unwired)) | — | |

## 2. Live gate (2 real turns on dsflash, streamed + interrupt verdict)

Env: `replay_serve 8380 --scenario conversation` (fixture `live-gate-a6ea8505`),
app = fresh vendored binary, the walk's env gates. Driven via the walk App +
instrument (`/snap`, `/g?raw=1`, HTTP `/log`).

- **Turn 1 — streamed**: `walk gate one` -> answer streamed to terminal
  (prose count rose, Working row cleared).
- **Turn 2 — interrupt**: `walk gate two` sent while turn 1 idle, Esc mid-turn ->
  verdict row **`Interrupted`** (`i0_workedfor_2 t='Interrupted'`).

Captures (each **viewed**, self-checked per the RULES):

| file | shows | verdict |
|---|---|---|
| `38a-connect.png` | first-run Connect (dead URL 8399, own instance): OctosCode title, Server + Token fields, Connect button, "or use local solo mode" link | good |
| `38a-turn.png` | finished turn: streamed answer prose, `Worked for 0s`, composer restored with placeholder | good |
| `38a-review.png` | review sheet open (`OCTOSCODE_CHROME=review` start-gate) over the conversation: title row, scope pill, close | good |
| `38a-settings.png` | settings drawer (`OCTOSCODE_CHROME=settings`, `settings_close` width>0 before capture): Session settings + Model / Permissions / Sandbox / Context / **General / Octos server / Disconnect** (#34a) | good |
| `38a-palette.png` | palette on `/` in an empty composer: all six commands (/model /monitor /mode /compact /btw /resume) with descriptions, move/run/esc footer | good |

**Review-title /snap numbers** (the #32i blank-capture redo):
`review_panel r=[370,76,530,603]`, `review_header r=[370,76,530,40]`,
title **`Review r=[374,80,68,29]`** — inside the panel with a 4 px top inset,
fully visible. `review_close r=[635,79,28,22]`, scope pill `r=[522,76,105,40]`.

Observed while capturing (matches known backlog rows, not new defects): the
window's right edge clips the timeline slightly (80b9f33 window-overflow row);
the finished turn's `Worked for 0s` row lingered in one capture (backlog row).

## 3. Known open items

From `outer/polish-backlog.md` (20 rows) + the click audit's dead rows. "Card"
= the card that owns/last touched it, from the backlog text.

| open item | source | card |
|---|---|---|
| user bubble keeps old height after #16e pitch fix (empty band under text) | backlog | #16e review |
| question-card 'Add a note' placeholder: no left inner padding | backlog | #18f |
| module: consume store resync_pending/take_resync -> session/hydrate (client/store done) | backlog | #22 (next app card) |
| headless RENDER proof of a >120-col code line fully reachable (lowering-only proof insufficient) | backlog §8.16 | #21g/#21e follow-up |
| timeline scrollbar draws a white bar over the bubble's right edge | backlog (g5) | — |
| 29c: provider card keeps 2-row height with 1 model | backlog | #29c |
| board 3.4/3.5 native text ~10% smaller than atlas | backlog | accepted (8.5) |
| 30d side question: bubble keeps 2-line height on 1-line question; '68%' label overlaps ring | backlog | #30d |
| 30e theme: dark pair only; 'system' resolves dark; in-memory only; components don't switch | backlog | #30e (Phase 3) |
| 30b monitors: paused monitor shows pause icon; commands ellipsize ~20 chars | backlog | #30b |
| 30c fleet: 0-peers full-height card; goal heading sample; meta line lost; 'Done' green; 3rd task overflows | backlog | #30c |
| 28e first-run: Connect card top-left over sidebar header, clips left edge | backlog | #28e (P3 first item) |
| live gate: board-4 window wider than ~900 display, right overflow | backlog | 80b9f33 |
| phone: 'OctosCode' title dark-on-dark (OnePlus 6T, a12769d) | backlog | — |
| timeline: 'Working · 0s' row lingers after the answer | backlog | seen again in 38a-turn.png |
| attachments: '68%' label wider than ring hole, white on light thumb (3 attempts) | backlog | #32b2 |
| verify #32b2 item 2 (bubble clears scrollbar) in the REAL live gate | backlog | this pack's gate run shows the bubble clear of the right edge except the known window overflow — final call outer's |
| **RESOLVED this pack**: #32i review-live capture BLANK -> replaced with a valid capture + /snap numbers | backlog | **#38a** |
| **RESOLVED this pack**: review title row clipped live -> title fully visible (top inset 4 px, numbers above) | backlog | **#38a** (verifies #32d caption-bar fix live) |
| **NEW (click audit)**: `review.toggle` click DEAD in the real app — both `review_toggle_hit` and `review_close` click to 'nothing' (audit chrome-review rows); the env start-gate and keyboard paths open the panel. Click wiring needed (LESSONS: wiring must be proven by a click) | click-audit.csv | next app card |
| **NEW (click audit)**: `error.reload` / `error.copy_diagnostics` dead on the error screen | click-audit.csv | next app card |
| **NEW (click audit)**: `session.new` / `thread.open` dead on chrome-review/chrome-settings surfaces (host chrome under a visible dock is shadowed **by design** on dock-*) | click-audit.csv | next app card |
| plus / mic hit targets unwired (documented reserved) | backlog | — |

## Commands (verbatim)
- `ctest --workspace --lib --tests` -> rc=0, TOTAL passed=488 failed=0
- `python3 tools/walk/run.py --limit 999` -> rc=0, pass=115 fail=0, specific=64 smoke=51
- `WALK_SCENARIO=conversation=session python3 tools/walk/run.py --only conversation --port 8370` -> rc=1 (expected FAILs)
- `python3 tools/walk/click_audit.py --app-bin <vendored> --out docs/walk/click-audit.csv` -> SURFACE 29: 2/8/6/8 (+5 unmapped)
- live gate session + captures: see §2 (replay_serve 8380 + harness/headless.sh 8370; connect via dead URL on 8371)
