# `tools/walk` — the Phase-4 walk-runner (card #19)

Turns the 234 rows of [`docs/walk-rows.csv`](../../docs/walk-rows.csv) (one per
web Playwright case) into scripted checks on the **native** app and records a
verdict for every row in [`docs/walk/results.csv`](results.csv).

No model runs. The backend is a recording-replay server (or a scripted fixture
server) that serves the committed real fixtures; the app runs hidden and is
driven through its own HTTP instrument.

**The official run** (A11) is one command — run.py's own checks for every
scriptable row PLUS every native click walk, in desktop and phone:

```sh
OCTOSCODE_APP_BIN=<host-bin> python3 tools/walk/run.py --full --port <your app port> \
    --scenario-port-base <8 free ports> --fixture-port <one per native walk>
```

It regenerates `results.csv` (one verdict per row) and `results-checks.csv`
(one line per check, with its evidence `file:line`). `--scenario-port-base`
moves run.py's own replay servers (default 8380.., one per scenario) into the
port block you were given; `--fixture-port` does the same for the native
walks' fixtures.

**Only that complete run regenerates the table; every narrower run MERGES
(A34).** `--only AREA[,AREA]`, `--limit N`, `--native-only --walks …`,
`--modes desktop`, `--no-native` re-decide the rows they touch exactly as a
full run would, and write a row only when a check that decides it ran (or its
app instance failed to start). Every other row — and its lines in
`results-checks.csv` — is copied byte for byte:

- `--only peer` runs every scriptable `peer` row (the limit counts that
  area's rows), including the env-group rows' instances (row 183 at 1280 and
  at 720); an instance that cannot start leaves its checks `not-run` for the
  row instead of dropping them. A native row whose walks did not run keeps
  its line (a full run decides it from the walks alone); a native-partial row
  takes the fresh run.py checks next to its walk checks from the table.
- `--native-only --walks W` re-decides the rows W maps; a row that another
  walk also maps keeps that walk's checks from the table.
- the parity relabel / demote passes touch only the re-decided rows.

Provenance: each row of `results.csv` records `run_sha` (the HOST build, read
from the `BUILT_FROM` stamp `outer/scripts/hostbuild.sh` writes next to the
binary, e.g. `octoscode-app@9c4fb794`; `WALK_RUN_SHA` overrides), `run_at`
(UTC, when the run started) and `run_scope` (`full`, `only=peer`,
`native-only walks=…`, …); each line of `results-checks.csv` its own
`run_sha` / `run_at` — a line carried from the table keeps its provenance, so
a merged row shows which of its checks are older. The rows of the official
`--full` run on 9c4fb794 were backfilled with `octoscode-app@9c4fb794`,
`2026-10-03T03:29:08Z` (the time of the commit that recorded them, e3073df0;
the run's own start was not recorded) and `full (recorded in e3073df0)`.

run.py's replay servers run with `--adopt-turn-ids`: each replayed turn plays
as the app's own `turn/start` id and prompt (a real server adopts the client's
turn id; A7's turn controller settles — and drains its queue past — only the
turn it dispatched). The composer area runs on the `two-turn` recording, whose
two turns both complete (the `conversation` recording's second turn is its
interrupted one).

---

## Native click walks (A11) — `tools/walk/native.py`

The subagents' click walks (`tools/walk/*_walk.py`, `tools/a1/conversation_walk.py`)
reach every control by a CLICK at its laid-out rect and assert the app's own
effect. run.py runs them through `tools/walk/native.py` and **re-points** the
walk rows they prove: those rows' verdicts come from the native checks, not
from run.py's area-matched Phase-3 checks.

**Picked up by convention, no registration.** Any `tools/walk/*.py` or
`tools/*/*.py` with a top-level literal `WALK = {...}` is a native walk (read
with `ast`, never imported; `*_walk.py` is the habit — A10's walks are
`a10_<area>.py` and take the aggregator's ports from `A10_PORT` /
`A10_REPLAY_PORT`). `python3 tools/walk/native.py` lists what it finds. The
literal says how to launch it and which rows it proves:

```python
WALK = {
    "name": "a2_board1", "modes": ["desktop", "phone"],
    "fixture": {"argv": ["{examples}/board1_serve", "{fport}"]},          # optional
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:8499"}, "ready": ["b1_connect_pair"]},
    #  ... or "app": "self" when the walk launches (and stops) its own app
    "runs": [{"argv": ["{mode}", "{out}"], "env": {"PORT": "{port}"}}],  # + "restart": app|fixture|both
    "needs": ["target/debug/examples/board1_serve"],                     # built when missing
    "rows": {87: ["p4-06_provider", "provider_saved"],                   # check-name substrings
             227: {"checks": {"phone": ["drawer"]}, "partial": "what the walk does NOT cover"}},
}
```

The full schema (placeholders, per-run `app_env` / `fixture_env` /
`fixture_args`) is in `tools/walk/native.py`'s docstring. A walk prints one
`PASS <name>` / `FAIL <name> — <detail>` line per check.

**Isolation (brief §8).** Every app the aggregator launches gets a fresh state
tree under `tmp/walk/native/<walk>-<mode>/state/` for drafts, credentials,
preferences, notifications, recents, show-thinking, downloads, display
preferences, the driver id and the Session pane's Advanced memory
(`tools/walk/walk_env.py`); a "self" walk inherits the same environment (A10's
`run_session` sets its own per-run tree). The phone is launched straight into OctosCode
(`--test-action page:0 --test-action launch-octoscode`, 360x780).

**A row's native verdict** is the AND of every mapped check in every mode a
walk mapping it ran; a mode in which no mapped check ran, a walk that could
not start, or a run that CRASHED (an uncaught Python traceback, even after
some checks passed) is a failing check — a crashed walk never reads green.

Iterate on one walk without re-running everything (merges into the committed
results: only the rows the walk maps are re-decided, and a row two walks map
keeps the other walk's checks — and the phone checks, with `--modes
desktop` — from the table):

```sh
python3 tools/walk/run.py --native-only --walks a11_offer --modes desktop --port <port>
```

Evidence: `docs/walk/evidence/native/<walk>-<mode>.log` (the walk's output,
machine paths scrubbed); a check's evidence cell points at its line.

A targeted run (`WALK_ONLY_ROWS=…` → `results-only*.csv`) can merge the last
native run's verdicts without walking again:
`--native-json tmp/walk/native/last.json` (every native run leaves it; the
flag is refused outside such a snapshot run). The official files are
`results.csv` / `results-checks.csv`; `results_live*.csv` is the last `--live`
run (real model turns), whose other rows are not maintained by it, and
`results-scenario*.csv` the last `WALK_SCENARIO=…` run (the negative
control's deliberately broken fixture, `negative-control.md`). Those
snapshot files stamp provenance on the rows their run selected.

---

## Run it

```sh
source .peer/env.sh

# 1) the native desktop app binary (see Prerequisites — external to this repo)
export OCTOSCODE_APP_BIN=<fork>/tmp/octosense-target/debug/octosense

# 2) run the first 30 scripted rows (all named areas), merged into the table
#    the scenario server is built for you on first run.
python3 tools/walk/run.py

# one area only (every row of it), merged into the table
python3 tools/walk/run.py --only conversation
# ... or its first 10 rows
python3 tools/walk/run.py --only conversation --limit 10

# a different app port (the block is 8370-8379)
python3 tools/walk/run.py --port 8371
```

The runner prints each check's result and a summary, then writes
`docs/walk/results.csv` (one verdict per row) **and**
`docs/walk/results-checks.csv` (one row per check that ran for a row) — the
whole table for `--full`, merged otherwise (the summary counts the rows this
run decided). Exit code is 0 iff none of the rows the run decided is `fail` or
`not-run`; **1** when a check failed, a check did not run, or a *selected* row
was blocked by a start failure; **2** when a documented prerequisite is
missing (the message names the command to run), `--only` names an unknown
area, or a partial run has no committed table to merge into.

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
   `<WORKSPACE>/p0-build/tmp/octosense-target/debug/octosense`
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
5. **Stops the app and the servers, then writes the verdicts**: all 234 rows
   for a complete `--full` run, otherwise merged into the committed table
   (only the rows it re-decided change).

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
| `not-walked` | (A11) the capabilities the parity matrix cites for this case are all built (A), but no click-walk check covers the case yet |
| `not-yet-implemented` | the native capability is missing; `reason` names the parity matrix's C capability citing this case — also (A11) when run.py's area-matched checks passed but a capability the matrix cites for the case is still C (generic checks cannot prove an unbuilt capability; the reason keeps them), and when every failing check of the row found its surface absent BY DESIGN (its reason starts `not built:`, e.g. Alt+P with no native peer dock) |
| `live-only` | needs state the replay fixtures cannot produce (a real model turn, browser-only state) |
| `blocked` | a selected row whose area failed to start (every app instance the row needs) |
| `not-run` | (A34) no check failed, but one the row needs did not run: one of its instances failed to start (row 183 at one width); the per-check line says `not-run` with the start failure |
| `skipped` | the web-only rows — operator-confirmation-pending |

The `depth` column: `specific` / `smoke` for run.py's own checks (#33a), and
(A11) `native` — the row's case is proven by native click-walk checks — or
`native-partial` — the native checks cover part of the case and `reason` says
which part they do not.

`run_sha` / `run_at` / `run_scope` (A34): the run that wrote the row — the HOST
build, the run's UTC start, what it covered (see "The official run" above).

### Per-check results (`docs/walk/results-checks.csv`)

Each check declares the rows it covers via `@check(..., rows=…)`: `run.ALL`, or
lowercase substrings matched against the row's `case` + `protocol_methods`. A row
is `pass` iff **every check mapped to it** passed, so one broken check fails only
the rows that actually use it — not its whole area (card #19c item 3). The
per-check CSV has columns `row_id, area, spec, case, check, status, evidence,
reason, run_sha, run_at`; a check is `pass`, `fail` or (A34) `not-run`.

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
