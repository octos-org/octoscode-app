# Phase 3 evidence pack — main @ be44817 (card #38a, lane task/38a)

## Closing check @ 3e9fb24 (card #40a, task/40a) — the supervisor's final numbers

Every number fresh on `task/40a` (main @ 3e9fb24), each with its command. App
binary: vendored rebuild of 3e9fb24 (`tmp/viz/21d/sync.sh` -> Finished 29.68s
after restoring the vendored manifest's `reqwest` workspace dep — build
scaffolding only). Captures: `docs/walk/evidence/phase3/live/40a-live-*.png`,
each VIEWED.

### Suites
| suite | command | result |
|---|---|---|
| unit+integration | `ctest --workspace --lib --tests` | **TOTAL passed=514 failed=0** (tmp/40a-ctest.log) |
| full walk (replay) | `python3 tools/walk/run.py --limit 999` | **rc=0, pass=115, fail=0 — specific=64 / smoke=51**, 35 distinct checks (results.csv committed 55afa52) |
| negative control | `WALK_SCENARIO=conversation=session --only conversation` | **rc=1 as required** (streaming/kinds checks FAIL) |
| walk --live | `--live --limit 999 --port 8370` (OCTOS_LIVE_TOKEN_FILE) | **rc=0, scripted rows 2, specific=2 / smoke=0** — row 1 `streamed+terminal=1.9s prose=2 bubbles_laid_out=2`; row 2 `background turn terminal, prose rows=2` (results_live.csv) |
| click audit | `python3 tools/walk/click_audit.py --app-bin <vendored> --out docs/walk/click-audit.csv` | **SURFACE 35 controls, 18 with an expected action: 8 respond / 1 dead / 9 shadowed-by-dock / 10 documented-unwired (+7 unmapped)** — #35c's button routing moved 8/1/9/10 from #38a's 2/8/6/8 |

Walk honesty note: the FIRST full run on 3e9fb24 was 108/7 — all seven runner-side,
none an app defect: the two live checks ran against replay fixtures (no mirror
filter), conv_fits snapped before `now` laid out, and v_badge/v_fold asserted the
authored `+62 −5`/`unmodified` copy that #36c replaced with computed totals (the
walk fixtures carry no diff receipt; with-receipt rendering is f36c's 6 passing
unit tests). Fixed in 21b64e8 (replay-mode mirror filter, now-poll, empty-state
contract); a `diffpreview` replay scenario was tried and reverted (r30a lacks an
open-result frame — the loader panics).

### A REAL live session WITHOUT OCTOS_PROFILE_ID (#32h self-discovery)
App started with NO `OCTOS_PROFILE_ID` (only the remote bridge env), driven through
the UI, captures in phase3/live/ (all viewed; /snap numbers for every assertion):
- **Connect via the UI**: first-run Connect screen; the app discovered the gate's
  profile itself and reached `conn: Live   sessions: 1` 1 s after clicking
  Connect (40a-live-1-connect.png). Fresh UI-connect instances mint
  `octoscode-<n>:main` session ids (three instances: 4436/4861/62…); the walk's
  `--live` app resolved the gate's `dsflash` profile and listed its sessions.
- **New chat** (40a-live-2-newchat.png): BOTH sessions listed —
  `dsflash:main` AND the fresh `dsflash:01a0f5d3…` (#34b's merge fold live).
- **Turn 1 streamed** (40a-live-3-turn1.png): "what does main.rs print? just the
  number" -> terminal, prose rows=2.
- **Turn 2 interrupted** (40a-live-4-turn2.png): Esc mid-turn -> verdict rows
  `Interrupted` (turn 1's row reads `Worked for 5s ›`).
- **Back to the first thread** (40a-live-5-threadback.png): the original row
  found by title and re-selected — both sessions still listed; the NEW session
  re-selected afterwards replays its turns (prose=2; #34b: "re-selecting the
  first shows its turn").
- **Review** (40a-live-6-review.png): panel `r=[370,76,530,603]`, title `Review
  r=[374,80,68,29]` fully visible. HONESTY: the in-flow toggle click did NOT
  open the panel (the hit target had rect [0,0,0,0] in that state — the click
  audit's 1 remaining dead row); the capture uses the `OCTOSCODE_CHROME=review`
  start gate on a fresh session, so the body shows the designed EMPTY state (no
  diff receipt folded — the #36c rows+diff rendering is proven by f36c's unit
  tests, not re-proven live here).
- **Settings** (40a-live-8-settings.png): `OCTOSCODE_CHROME=settings`; Session
  settings + Model / Permissions / Sandbox / Context / General + Octos server /
  Disconnect all laid out (`settings_close` zero-width noted — the close glyph
  paints, the hit slot does not). NEW observation: the Disconnect label is
  CLIPPED at the window's right edge at 900 wide — the #38c right inset covered
  the conversation column only; the settings drawer needs the same treatment
  (open-items table).
- **Palette** (40a-live-7-palette.png): opened by `/` in the live session —
  search `r=[220,207,544,34]`, all six commands with descriptions
  (/model /monitor /resume /btw /mode /compact) + move/run/esc footer.

### Open items (regenerated from outer/polish-backlog.md, OPEN section only)
| open item | card/owner |
|---|---|
| user-bubble keeps old height after the #16e pitch fix (empty band under text) | #16e review |
| 29c: provider card keeps 2-row height with 1 model | #29c |
| board 3.4/3.5 loops+monitors native text ~10% smaller than atlas (accepted 8.5) | accepted |
| 30e theme: dark pair only; 'system' resolves dark; in-memory; components don't switch | #30e (Phase 3) |
| 30b monitors: paused shows pause icon; commands ellipsize ~20 chars | #30b |
| 30c fleet: 0-peers full-height card; sample goal heading; meta line lost; 'Done' green; 3rd task overflows | #30c |
| 30a/28e: first-run Connect card top-left over the sidebar header, clips left edge (P3 first item) | #28e |
| whole-app live gate (80b9f33): window overflow right — RESIDUAL: the settings drawer still clips Disconnect at 900 wide (#38c fixed the conversation column only) | next app card |
| final live check: #32i review capture BLANK — RESOLVED (#38a replaced it; this run re-proves title [374,80,68,29]) | done |
| NEW (this run): the review toggle hit target did not lay out in the live flow (rect 0,0,0,0 — the click audit's 1 remaining dead row) | next app card |
| NEW (this run): settings drawer clips `Disconnect` at the right edge at 900 wide | next app card |

All numbers above are fresh from this branch (be44817), each with its command. App
binary: vendored rebuild of be44817 (`tmp/viz/21d/sync.sh` -> Finished; the
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

## Real live run (#38b, main @ 0591982) — dsflash, NOT replay

Setup: the outer loop's REAL live gate — `octos serve` already listening on
127.0.0.1:50190 (a6ea8505, dsflash), native app hidden on :8490 with the
live-gate env (`OCTOS_BASE_URL=…50190`, `$G/.token`, `OCTOS_PROFILE_ID=dsflash`,
`OCTOS_WORKSPACE_CWD=$G/ws`), binary = fresh vendored build of 0591982.
Driven with the walk App + instrument (0.4 s /snap polling; timestamps wall-clock).

| check | result |
|---|---|
| turn 1 streams | **yes** — `In one short paragraph: what does main.rs … print, and why?` streamed a real paragraph (main.rs prints `5`; `println!("{}", add(2, 3))`), terminal 2.5 s after send |
| "Working" -> "Worked for Ns" (#32i recheck, timed) | **replaced within one poll of the last delta (0.0 s ± 0.4 s poll)** — row reads `Worked for 1s ›` matching the real ~1 s duration. NO linger in the live run; #38a's "Worked for 0s" was the REPLAY fixture's ~0 s turns, not an app defect -> nothing filed |
| turn 2 interrupt verdict | **`Interrupted`** — `walk 38b second prompt: list the top-level files here`, list_dir tool cell done, Esc -> terminal 1.5 s after Esc, verdict row `Interrupted` |

Captures (both VIEWED): `docs/walk/evidence/phase3/live/38b-live-turn1.png`
(streamed answer + `Worked for 1s ›` + restored composer with approval pill /
v4-flash selector), `38b-live-turn2.png` (second prompt bubble + list_dir done
cell + `Interrupted` row). Both also show the known 80b9f33 window-overflow
right-edge clip — backlog row, not a new defect.
