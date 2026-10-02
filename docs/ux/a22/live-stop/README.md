# A22 — the cross-session Stop, LIVE

The defect A20 found: with a turn running in Session X, the person switches to
Session Y and presses Stop — and X's turn was interrupted. Fixed in `5e687a51`
(a Stop acts only on the live turn of the Session it was pressed in; the
failing-first test is `b47d451d`), with the other turn-scoped actions audited
(`3f0c5743` steer, `6720648f` queue + recovery controls, `179c62bd` approval
keys — superseded at the merge by A20's production path for the same class —
and `b1c4269b` Take over).

This folder is the LIVE proof on a real octos server, plus the failing-first
records.

## Setup (nothing of the operator's)

- A PRIVATE `octos serve` (the p0-build release binary) on `127.0.0.1:50224`
  with `--solo` and its own `--data-dir` (a fresh copy of the live-gate data:
  the dsflash profile), `--instance-data-dir` and `--cwd` under the
  worktree's `tmp/`. Never the operator's `:50190`, never the operator's apps.
- A random token in a mode-600 file, exported ONLY in the serve's
  environment as `OCTOS_AUTH_TOKEN`; the app got it as `OCTOS_BEARER`. It was
  never printed, logged or saved (see "Secrets and paths").
- The app: the hidden headless OctosCode host built from
  `agent-afb5d60af030eace4@fd0d2307` (the A22 branch with main `998d5d0f`
  merged; the makepad bridge's per-launch token, D10c, sent by
  `tools/walk/bridgeauth.py`) on port 8494, no visible window, its own state
  dirs, its frame trace in `OCTOSCODE_TRACE_FILE`.
- The dsflash profile for the model turns: 2 model turns in this run (9
  across all of A22's live runs — the earlier ones debugged the walk itself —
  under the cap of 15).
- Driver: `tools/walk/a22_live_stop.py` (every step a CLICK, a key or typed
  text through the instrument; every check reads the app's snap or its own
  frame trace).

## What was done and what happened (21/21 checks, `checks.txt`)

1. X is the startup Session, fresh (its empty state shows). A long prompt
   ("Write the whole numbers from 1 to 1200 as English words, one per line,
   and nothing else.") streams in it (Stop shows). `01-x-streaming.png`
2. CLICK **New chat**: Session Y is on screen while X keeps streaming. Y
   shows Send and no Stop control (Y has no live turn); X's sidebar row shows
   the running dot. `02-y-while-x-streams.png`
3. "Press Stop" in Y — at once, while X still streams: Escape, then the
   `/stop` command. `03-y-after-stop.png`
   - X had no terminal yet when Stop was pressed (it was live);
   - wire: NO `turn/interrupt` left the app at all.
4. X streamed to its end (10 243 deltas): its terminal is `completed`, never
   `interrupted`; no `turn/interrupt` ever named X's turn; X's row read done
   (completed in the background) while Y was on screen.
5. CLICK X's row: exactly one `session/open` (X's); X's whole answer is
   there, down to its last line ("… one thousand one hundred and ninety-nine
   | one thousand two hundred"). `04-x-finished.png`
6. Positive control: a second long prompt in X (Roman numerals), CLICK the
   Stop control: `turn/interrupt {session_id: X, turn_id: that turn}` and the
   turn ended `interrupted` (its prompt came back to the composer).
   `05-x-stopped.png`

`ids.json` names X (`dsflash:main`), Y and both turns.

## Wire evidence (`trace-excerpt.jsonl`, from the app's `OCTOSCODE_TRACE_FILE`)

Every `session/open`, `turn/start` and `turn/interrupt` the app sent and
every `turn/started` and `turn_terminal` it received, with wall times
(relative to the first open):

| t (s) | dir | frame | Session | turn |
|---|---|---|---|---|
| 0.0 | out | `session/open` | X | |
| 2.5 | out / in | `turn/start` / `turn/started` | X | turn 1 |
| 6.8 | out | `session/open` | Y (New chat) | |
| — | | Stop pressed in Y (Escape, `/stop`): no frame | | |
| 48.3 | in | `turn_terminal` **completed** | X | turn 1 |
| 49.4 | out | `session/open` (back to X) | X | |
| 55.3 | out / in | `turn/start` / `turn/started` | X | turn 2 |
| 59.6 | out | `turn/interrupt` (the run's only one) | X | turn 2 |
| 59.6 | in | `turn_terminal` **interrupted** | X | turn 2 |

`trace-deltas.txt` counts the streamed deltas per turn (turn 1: 10 243,
turn 2: 205).

## Captures

`01`–`05` `.png` with their `.snap.json` (the widget tree with rects). The
header's folder line is a machine path and is painted over in each PNG; the
snaps went through `tools/walk/snapsafe.scrub`, and machine paths are
rewritten (`<live>`, `<home>`, `<tmp>`).

## Failing-first records

- `failing-first-head.log` — the committed test
  `stop_pressed_in_y_never_interrupts_another_sessions_turn`
  (`crates/octoscode-module/tests/a22_cross_session.rs`) at `1699016a`, before
  the fix: RED ("Stop in Y sent an interrupt for Z's turn").
- `failing-first-main.log` — the same scenario against main `b1fe23fb` (the
  coordinator's exact case: a turn in X, switch to Y, Stop): RED ("Stop in Y
  sent an interrupt for X's turn").

## Secrets and paths

Every file in this folder, this README and the driver script were grepped for
the token (whole, first 8, last 8) and for machine paths before the commit:
0 hits. The serve's data and instance dirs and the token file were deleted
after the run.

## Re-run

Start a private serve the brief's way (`octos serve --port <port> --host
127.0.0.1 --solo --data-dir D/data --instance-data-dir D/inst --cwd D/ws`, D a
fresh dir under `tmp/`, D/data a copy of the live-gate data,
`OCTOS_AUTH_TOKEN` only in its environment from the mode-600 file
`D/token`), then:

    LIVE_DIR=D python3 tools/walk/a22_live_stop.py <host-bin> 8494 http://127.0.0.1:<port> <outdir>
