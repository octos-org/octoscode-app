# `tools/walk` — the Phase-4 walk-runner (card #19)

Turns the 234 rows of [`docs/walk-rows.csv`](../../docs/walk-rows.csv) (one per
web Playwright case) into scripted checks on the **native** app and records a
verdict for every row in [`docs/walk/results.csv`](results.csv).

No model runs. The backend is a recording-replay server that serves the
committed real fixtures; the app runs hidden and is driven through its own HTTP
instrument.

---

## Run it

```sh
source .peer/env.sh

# 1) the native desktop app binary (see Prerequisites — external to this repo)
export OCTOSCODE_APP_BIN=<fork>/tmp/octosense-target/debug/octosense

# 2) run the first 30 scripted rows (all named areas)
#    the scenario server is built for you on first run.
python3 tools/walk/run.py

# one area only, a smaller batch
python3 tools/walk/run.py --only conversation --limit 10

# a different app port (the block is 8370-8379)
python3 tools/walk/run.py --port 8371
```

The runner prints each check's result and a summary, then writes
`docs/walk/results.csv` (one verdict per row) **and**
`docs/walk/results-checks.csv` (one row per check that ran for a row). Exit code
is 0 iff no row is `fail` and none is `not-run`; **1** when a check failed, a row
was not run, or a *selected* row was blocked by a start failure; **2** when a
documented prerequisite is missing (the message names the command to run).

---

## Prerequisites

The runner needs two things. It **builds the first itself** and **checks the
second once, failing fast** with an explicit recipe if it is absent — so a run
never fails with a mystery `0 pass` result.

1. **The scenario server** — ours, and built automatically on first run when
   `target/debug/examples/replay_serve` is missing:

   ```sh
   cargo build -p octoscode-module --example replay_serve
   ```

   Pass `--no-build` to skip the auto-build and be told to run that yourself.

2. **The native desktop `octosense` binary** — **external to this repo.** It is
   the OctoSense desktop shell with the `octoscode` module linked, built from the
   OctoSense fork (this repo's `[patch]` already points `octos-app-transport` /
   `octos-app-store` at that fork). Point `OCTOSCODE_APP_BIN` at an existing
   build, or make one:

   ```sh
   tools/prepare-octosense-fork.sh            # recreate the [patch] fork (idempotent)
   cd <fork> && python3 tools/setup.py        # framework sources (`.sources/`)
   cd <fork> && CARGO_TARGET_DIR=$PWD/tmp/octosense-target \
       cargo build --features app-appcard -p octosense
   export OCTOSCODE_APP_BIN=<fork>/tmp/octosense-target/debug/octosense
   ```

   A prebuilt copy may already exist read-only at
   `/Users/yuechen/home/oa.noindex/p0-build/tmp/octosense-target/debug/octosense`
   (set `OCTOSCODE_APP_BIN` to that instead of rebuilding). The desktop *crate*
   directory (`<fork>/desktop`) is derived from the binary's path; override it
   with `OCTOSCODE_SHELL_CWD` if your layout differs. Building it takes ~12 min.

Environment knobs:

| var | meaning | default |
|---|---|---|
| `OCTOSCODE_APP_BIN` | the native desktop binary to launch (required) | `p0-build/tmp/octosense-target/debug/octosense` |
| `OCTOSCODE_SHELL_CWD` | the desktop crate dir the app runs from | derived from `OCTOSCODE_APP_BIN` |

---

## What it does, step by step

1. **Selects the rows** (`select_targets`): the first 30 rows in the areas the
   card names — conversation, threads, composer, recovery — taken **round-robin
   by area** so no area is crowded out. `real-turn` rows and web-only rows are
   excluded from selection (they get `blocked` / `skipped` verdicts instead).
2. **Starts one backend per area** (`replay_serve.rs --scenario <name>`) on the
   backend ports 8380-8385.
3. **Launches the app hidden** via [`harness/headless.sh`](../../harness/headless.sh)
   on port 8370 (this card's block), pointed at that scenario's backend.
4. **Drives real input** — `/click`, `/t` — and **asserts from `/snap`**. A
   failure writes `/g` + the snap JSON into `docs/walk/evidence/`.
5. **Writes a verdict for all 234 rows**, then stops the app and the servers.

---

## Scenario server — `crates/octoscode-module/examples/replay_serve.rs`

`--scenario <name>` picks which committed recording under
`crates/octoscode-client/tests/fixtures/` is served:

| scenario | fixture | exercises |
|---|---|---|
| `conversation` (default) | `live-gate-a6ea8505` | one full streamed turn |
| `approval` | `r5-turn-a6ea8505` | `approval/requested` + decisions |
| `task` | `r4-task-a6ea8505` | `task/updated`, task artifacts |
| `autonomy` | `r1-autonomy-a6ea8505` | monitors, loops, session goals |
| `peer` | `r6-peer-a6ea8505` | `peer/gather`, `peer/prepare` |
| `session` | `r3-session-a6ea8505` | the context-compaction lifecycle |

### Adding a scenario from a recorded fixture

1. Record real frames (see card #R-series): run `octos serve` with
   `OCTOSCODE_TRACE_FILE=<file>` set and exercise the methods.
2. Scrub machine paths to placeholders (`<WORKSPACE>`, `<TMP>`, `<HOME>`) — the
   hygiene test `crates/octoscode-client/tests/fixtures_hermetic.rs` fails the
   suite otherwise.
3. Add one arm to `scenario_fixture(name)` in `replay_serve.rs`.
4. Point the runner at it: add an entry to `SCENARIO_PORTS` and `AREA_SCENARIO`.

### Why the server rewrites the session id

A recording carries the session id of the profile it was captured under
(`dsflash:main`, `octoscode49213:main`, …). The app opens `{OCTOS_PROFILE_ID}:main`.
If they differ, every replayed frame lands in a session the app does *not* have
active and **nothing renders**. The server therefore rewrites the recorded id to
the one the app actually requested in `session/open`, across every served frame
(including `cursor.stream`).

---

## Verdicts (`docs/walk/results.csv`)

| status | meaning |
|---|---|
| `pass` | every check **mapped to this row** passed (≥1 ran) |
| `fail` | ≥1 mapped check failed; `/g` + `/snap` evidence is recorded |
| `not-yet-implemented` | the native capability is missing; `reason` names it (from `docs/parity-matrix.csv`) |
| `blocked` | the 3 `real-turn` rows — need an outer-loop live model run |
| `skipped` | the 31 web-only rows — operator-confirmation-pending |

### Per-check results (`docs/walk/results-checks.csv`)

Each check declares the rows it covers via `@check(..., rows=…)`: `run.ALL`, or
lowercase substrings matched against the row's `case` + `protocol_methods`. A row
is `pass` iff **every check mapped to it** passed, so one broken check fails only
the rows that actually use it — not its whole area (card #19c item 3). The
per-check CSV has columns `row_id, area, spec, case, check, status, evidence,
reason`; per-row check counts currently range 1–7.

## The composer input checks are deterministic (card #19c)

makepad `/t` sends `Input::Text { replace_last: false }`
(`native/makepad/platform/src/remote.rs:1328-1332`) — it **inserts at the caret**,
it never replaces the field, and a click on a *populated* field may place the
caret or select according to position. Asserting exact equality after typing into
a non-empty field is therefore non-deterministic. The composer checks instead:
`focus_composer()` clicks the field's **right edge** (caret → end), then
`clear_composer()` backspaces it empty and **waits** for the empty/placeholder
state, then `type_into_composer()` types and **waits** until the draft equals the
text. Waiting is always on the observable `/snap` condition — never a fixed sleep.

### Which areas are scripted, and why `approval` is not

The card lists approval among the areas "that exercise what exists today". It
does not: the built design batch is `conversation-01/03/04/08/09`, and
`design/bindings.json:40` records that the inline-approval scene
(`conversation-05`) "is NOT in this batch's 5 mapped components". The delivery of
`approval/requested` is a *store* fact reachable only from a binding, not a
widget on `/snap` — so a row cannot honestly be marked `pass` on it. Approval
rows are therefore `not-yet-implemented`, naming the missing card. When the
approval card lands, flip `AREA_SCRIPTABLE["approval"]` to `True` and add its
checks.

---

## Evidence

`docs/walk/evidence/` holds the renders and snaps the runner captured
(`19-launch-module.png`, `19-conversation.png`, and `area-*.png` / `area-*.snap.json`
for any failing area). `docs/walk/server-*.log` are the replay servers' logs.
