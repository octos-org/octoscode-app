# Headless native UI harness (card #5)

How every later UI lane proves its own work with Makepad's built-in instrument in
**hidden-window** mode. Start here; do not roll your own launcher.

- Script: [`harness/headless.sh`](../../harness/headless.sh) — `start | snap | click | type | shot | stop | status | ports`.
- Scripted tests: [`harness/ui-smoke/`](../../harness/ui-smoke) — the `makepad_test` template.
- Shared workspace: `<WORKSPACE>/native/` (see [Shared workspace](#shared-workspace)).
- Evidence produced by this card: [`docs/harness/evidence/`](evidence/) + [`docs/harness/index.json`](index.json).

Hidden means the native window is **never shown or focused**; the app's own HTTP
instrument (`/snap`, `/click`, `/t`, `/g`) works exactly the same. Never take an
OS/window/display screenshot (`screencapture`, `adb screencap`, …) — only the
app's own `/g` grab counts.

---

## 1. Quickstart

```sh
source .peer/env.sh
export OCTOSENSE_NATIVE_ROOT=<WORKSPACE>/native   # default anyway

# start an app hidden on YOUR port (see the port plan), then drive it:
bash harness/headless.sh start harness/ui-smoke/fixtures/notes/bundle 8301
bash harness/headless.sh snap  8301 Button      # find a control
bash harness/headless.sh click 8301 366 140     # centre of a rect from /snap
bash harness/headless.sh type  8301 "hello"     # only if the app has an input
bash harness/headless.sh snap  8301             # state changed?
bash harness/headless.sh shot  8301 out.png     # the app's own /g
bash harness/headless.sh stop  8301             # /gq, wait for exit, confirm the port is free
```

Verbatim output of exactly that flow (this card, from its herdr pane on :8301):

```text
$ bash harness/headless.sh start harness/ui-smoke/fixtures/notes/bundle 8301
[headless] launch: MAKEPAD_HIDE_WINDOWS=1 <WORKSPACE>/native/OctoSense-App-Hub/target/release/card-host --bundle harness/ui-smoke/fixtures/notes/bundle --app-data <WORKSPACE>/p0-harness/tmp/peer-tmp/octos-headless/app-data-8301 --allow-unsigned --remote 8301
[makepad-remote] listening on 127.0.0.1:8301 pid=82391 app=card-host grabs=<WORKSPACE>/p0-harness/tmp/peer-tmp/makepad-remote/card-host-82391
[headless] up: pid 82391  port 8301  log <WORKSPACE>/p0-harness/tmp/peer-tmp/octos-headless/port-8301.log

$ bash harness/headless.sh snap 8301 Button
{"s":[{"i":"card","ty":"Splash","r":[0,32,412,860],"w":0,...},{"i":"-","ty":"Button","r":[336,120,60,40],"w":0,"t":"Add"}]}
$ bash harness/headless.sh click 8301 172 140      # focus the entry (rect 16,120,312,40)
{"ok":1,"f":72}
$ bash harness/headless.sh type 8301 "hello harness"
{"ok":1,"f":75}
$ bash harness/headless.sh snap 8301 entry
{"s":[...,{"i":"entry","ty":"TextInput","r":[16,120,312,40],"w":0,"t":"hello harness","val":"hello harness"}]}
$ bash harness/headless.sh click 8301 366 140      # the Add button (rect 336,120,60,40)
{"ok":1,"f":84}
$ bash harness/headless.sh snap 8301               # the note is now in the rendered list
 ... "-","Label","r":[31,190,356,27],"t":"hello harness" ...
$ bash harness/headless.sh shot 8301 out.png
[headless] wrote out.png (42962 bytes)

$ bash harness/headless.sh stop 8301
gq: {"png":[".../card-host-82391/grab-w0-00001.png"],"quit":1}
[headless] pid 82391 exited after /gq
[headless] port 8301 free
```

Every line above is in [`evidence/octo-run/`](evidence/octo-run) (`start-verbatim.txt`,
`stop-verbatim.txt`, `actions-8301.log`, `snap-8301-{1..4}.json`,
`headless-flow-after-add.png`).

### Commands

| Command | Effect |
| --- | --- |
| `start <bundle\|binary> <port>` | Launch hidden + detached. A **directory with `manifest.json`** → App Hub `card-host`; an **executable** → launched directly with `--remote <port>` (extra args via `HEADLESS_ARGS`). Refuses a port that already answers. Prints the `[makepad-remote] listening on …` line (pid + endpoint). |
| `snap <port> [query]` | `GET /snap` (optionally `?q=`); the JSON of visible widgets, ready to click. |
| `click <port> <x> <y>` | `GET /click?x=&y=&wait=1` — replies after the resulting frame is drawn. |
| `type <port> <text>` | `GET /t?t=&wait=1`. |
| `shot <port> <out.png>` | `GET /g?raw=1` → PNG (verified PNG magic). |
| `stop <port>` | `GET /gq` (grab + quit), waits for the owned pid to exit, then confirms the port is free. |
| `status <port>` | Up/down and the `/s` identity, without changing state. |
| `ports` | Every port this harness started this session + live/gone. |

Useful env: `HEADLESS_EVIDENCE=<dir>` (copy snaps/shots/actions/stop into it),
`HEADLESS_NO_STAMP=1` (don't pass `--stamp` — for an immutable, pre-stamped bundle),
`OCTO_CARD_HOST=<path>`, `HEADLESS_TIMEOUT`, `HEADLESS_STOP_TIMEOUT`.
Run `bash harness/headless.sh` with no args for the usage banner.

### Two ways to launch, both proven

```sh
# a) tools/octo (from the design-flow repo) — stamps, resolves card-host for you:
python3 "$OCTOSCRIPT_FLOW/tools/octo" run <bundle> --port 8301 --hidden --detach
# b) this script / a bare binary — the pinned form the instrument documents:
MAKEPAD_HIDE_WINDOWS=1 <card-host> --remote 8301 --bundle <bundle> --app-data <dir> --allow-unsigned
```

`octo run` sets `MAKEPAD_REMOTE=<port>` + `MAKEPAD_HIDE_WINDOWS=1` and returns once
the first frame is drawn; `headless.sh` uses the documented `--remote <port>` form
and reads the `[makepad-remote] listening on` line. Both work; the script also
drives any other Makepad binary (the OctoSense shells).

---

## 2. Port plan — never collide

`8300 + 10*i`, ten ports per lane. **Use only your own block.** The proof for this
card used the harness block (`8301`).

| i | Block | Lane |
| --- | --- | --- |
| 0 | `8300–8309` | **harness / card #5** (proof port `8301`) |
| 1 | `8310–8319` | p0-oracle |
| 2 | `8320–8329` | p0-build |
| 3 | `8330–8339` | p0-map-b |
| 4 | `8340–8349` | p0-map-c |
| 5 | `8350–8359` | p0-map-e |
| 6 | `8360–8369` | p0-map-f |
| 7 | `8370–8379` | p0-proto |
| 8 | `8380–8389` | p0-proto2 |
| 9 | `8390–8399` | p0-build D9 spike (card #6) |
| — | `8490–8499` | outer loop (re-verification only) |
| 8 | `8380–8389` | next lane to land |
| 9 | `8390–8399` | manual / ad-hoc |

`headless.sh start` **refuses** a port that already answers (`/s` returns a pid) and
names the holder, so a wrong block fails loudly instead of driving someone else's
app. `makepad_test` never needs a fixed port: it launches its own app on an
**ephemeral** port and reads it from the listening line.

---

## 3. Evidence every UI card must attach

A card that changes or checks native UI attaches, for the run it claims:

1. **`/snap` JSON before and after** the input (`snap-<port>-<n>.json`) — the state
   change is the `t`/`val` diff between them.
2. **The actions you actually sent**, with their replies — `actions-<port>.log`
   (`click x=.. y=.. -> {"ok":1,"f":..}`, `type .. -> ...`). Coordinates are the
   centre of a rect from the **latest** `/snap`.
3. **The app's own `/g` PNG** (`shot` / `headless-flow-after-add.png`) — not an OS
   screenshot. Look at it before attaching it.
4. **`/log?n=50`** — the app's log lines around the run (`curl -s 127.0.0.1:<port>/log?n=50`).
5. **Exit confirmation** — the `/gq` reply (`"quit":1`) and `pid <n> exited`, from
   `stop-<port>.txt`; plus the ownership line from `start-<port>.log`.
6. **After cleanup**, nothing of yours is listening:
   `lsof -iTCP -sTCP:LISTEN -P | grep -E ':83[0-9][0-9]'` → empty (only your block could ever appear).

Set `HEADLESS_EVIDENCE=<card-evidence-dir>` and the script copies 1, 2, 5 into it as
it goes. Attach the directory (or its files) to your card.

Visual similarity (≥ 9/10) still needs a **vision-capable reviewer** — the harness
gives you geometry/text assertions from `/snap`; leave the score to the reviewer
your card names.

---

## 4. Scripted regression tests: `makepad_test`

Copy [`harness/ui-smoke/`](../../harness/ui-smoke) — a tiny crate whose one test
launches `card-host` on the committed fixture, clicks a control by selector and
waits for the list to change. It is the template: point it at your bundle, assert
widget state, and it builds/launches/drives/shuts down the app itself.

```sh
cd harness/ui-smoke
cargo test --release --test ui                            # serial
MAKEPAD_TEST_PARALLEL=1 cargo test --release --test ui    # several hidden apps at once
```

Run every `cargo test` through the host-wide slot wrapper so 20 lanes don't fight:

```sh
<HOME>/octoscode-app/outer/scripts/ctest --release --test ui
```

Verbatim (this card):

```text
running 2 tests
test add_note ... ok
test add_second_note ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 170.79s
```

Parallel run (`MAKEPAD_TEST_PARALLEL=1`) → `test result: ok. 2 passed; 0 failed; … finished in 1.22s`,
with two **distinct** owned apps in the artifacts (pid 65033 @ :63333 and pid 65034 @ :63332), both
`graceful: owned process exited after /gq or /quit`. Serialization (the default) is for timing-sensitive
assertions; opt into parallel only when each test gets its own `--app-data`.

**Template** — the two things that matter (full file: `harness/ui-smoke/tests/ui.rs`):

```rust
fn notes_app(test: &str) -> TestConfig {
    let mut c = TestConfig::new(card_host_dir, "octosense-card-host", test)?;
    c.bin_name = Some("card-host".into());            // the package builds one bin, named card-host
    c.app_args = vec![
        "--bundle".into(), bundle.into(),
        "--app-data".into(), per_test_state.into(),   // one jail PER TEST (parallel safety)
        "--allow-unsigned".into(),                    // add "--stamp" only for a mutable bundle
    ];
    Ok(c)
}

#[test]
fn add_note() {
    run_with_config(notes_app("add_note"), |app: TestApp| {
        app.locator(Selector::id("entry")).wait_visible().fill("hello harness");
        app.locator(Selector::widget_type("Button").text_exact("Add")).wait_visible().click();
        app.locator(Selector::widget_type("Label").text_exact("hello harness")).wait_visible();
    }).unwrap();
}
```

Notes that save time:

- `build_release_binary` runs `cargo build --release -p <package>` from the
  **card-host crate dir**; it reuses your inherited `CARGO_TARGET_DIR`. `TestConfig::new`
  takes the *package directory*, not the crate name.
- The macro form `#[makepad_test]` targets the current package and needs a
  `[dev-dependencies] makepad-test = { path = "<native>/makepad/libs/makepad_test" }`;
  `run_with_config` (used here) can point at any manifest.
- Failures write `failure.txt`, `logs.txt`, `widget-snapshot.json`, `widget-tree.txt`,
  `failure-screenshot.png` under `<card-host dir>/target/makepad_test/<package>/<test>/`.
- `Selector::id` matches a `name := …` you declared in the page; `Selector::widget_type`
  + `.text_exact(..)` matches a button's text, including widgets built inside `on_render`.

---

## 5. Hidden desktop shell (OctoSense)

The OctoSense desktop shell is any Makepad app: hidden + `--remote` works the same,
and its `/snap` exposes the shell's own widget tree.

```sh
MAKEPAD_HIDE_WINDOWS=1 MAKEPAD_REMOTE=8302 <octosense> &
# or:  bash harness/headless.sh start <octosense-binary> 8302
```

Verbatim (this card, the `p0-build` desktop binary, **read-only** — not rebuilt):

```text
[makepad-remote] listening on 127.0.0.1:8302 pid=76292 app=octosense grabs=.../octosense-76292
GET /s →  {"app":"octosense","pid":76292,"w":[{"i":0,"t":"OctoSense [remote]","sz":[1400,900],"px":[2800,1800],"dpi":2,"pos":[164,153]}]}
/snap →   13 widgets: main_window Window [0,0,1400,900] · bar/desktop_controls/shell_bar ShellBar [0,0,1400,32]
          · scene WmScene [0,32,1400,868] · wallpaper/bg_fill/bg_image Image [0,32,1400,868]
          · desk_row/desk WmDesk · shell_overlay · desktop_shelf DesktopShelf [411,812,578,78]
/g    →   png .../octosense-76292/grab-w0-00001.png sz [2800,1800]
/gq   →   {"png":[".../grab-w0-00003.png"],"quit":1}   → pid 76292 exited after 2 s, port 8302 free
log   →   wm: modules linked: ["rinx", "appcard", "apphub", "card"]
```

**Is the appcard module reachable in its UI?** *Partially verified.* The module is
**linked into the shell** (its own log line says so; source: `crates/shell/src/apps.rs:69`
pushes `octosense_appcard::APPCARD_MODULE`, registered at `apps.rs:644`; the shell opens it
via `client_of_module("appcard")` at `crates/shell/src/lib.rs:3890`). Its surfaces **exist in
the hidden widget tree** as the shell's app hosts (`desktop_shelf`, `shell_gallery_host`,
`gallery_holder`, `desk`). What this probe did **not** do is drive an appcard open by its
shelf/gallery icon and read the resulting card — so "reachable by clicking through the UI"
is **unverified**; see the source lines above for the launch path to follow next.
Evidence: `evidence/desktop-shell/` (`shell.log`, `shell-snap.json`, `shell-tree.txt`, `shell-grab.png`).

Limits: launching a shell without `MAKEPAD_HIDE_WINDOWS=1` shows a window — never do that
from a lane. The shell has no `--help` (passing it starts the app), so launch it only with the
hidden env var and `--remote`. A missing `OCTOS_APP_CORE_BIN` is expected and harmless
(no octos kernel configured).

---

## 6. Shared workspace — read it, or clone your own?

`<WORKSPACE>/native/` was built once by this card:

```sh
mkdir -p <WORKSPACE>/native
cp -c -R <ref>/OctoScript-App-Design-Flow <WORKSPACE>/native/   # APFS clone, ~2 s
cd <WORKSPACE>/native/OctoScript-App-Design-Flow
source .peer/env.sh && python3 tools/setup-native.py          # fetches the pinned closure
cp -c -R <ref>/OctoSense-App-Hub <WORKSPACE>/native/
cd <WORKSPACE>/native/OctoSense-App-Hub
cargo build --release -p octosense-card-host -p octosense-app-hub
```

Prepared revisions (`python3 tools/setup-native.py --check` → exit 0):

| Repo | Revision |
| --- | --- |
| `octoscript-makepad` | `99c1e5ee7925060ced59c66d6f8763c22217e4ad` (from `native-runtime.lock.json`) |
| `makepad` | `d0a9def5fc554a52955db21a2a5a196daccc8e3d` (from `runtime.json`) |
| `octoscript` | `68f6a9df55692b5d8ef8873a12721e279a3f40d6` (from `runtime.json`) |

**Is reading it shared-safe, or does each lane need its own clone?** Both work; the trade-off is:

- **Read + build with your own `CARGO_TARGET_DIR`** (what `.peer/env.sh` already sets). Each lane
  builds into its *own* target dir, so concurrent builds don't clobber each other. **This is what
  this card did, and it is safe.** The only shared writes are Cargo's
  `CARGO_HOME` (the shared registry cache) and each crate's `target/` — both already
  git-ignored. Confirmed: after building `hub` + `card-host` there, the App Hub checkout is
  **clean** (`git status --short` empty; `target/` ignored).
- **Clone your own APFS copy** if you need to *edit* the pinned sources, or to be fully
  isolated from a lane that dirties the shared tree. `cp -c -R` is ~2 s and copies nothing
  until written, so it is cheap; the one-time cost is per-lane disk as the clones diverge.

Never let `CARGO_TARGET_DIR` point at the shared tree's `target/` (two lanes would race).
`.peer/env.sh` already prevents that.

---

## 7. Known limits & cleanup

- **Hidden ≠ display-free.** It keeps Metal, native controls and platform WebViews; it
  still needs a live macOS GUI session. There is no headless software renderer here.
- **Not an Android/phone pass.** The pinned Makepad compiles `--remote` out on Android, so a
  desktop probe never counts as an Android pass — say "unverified on device".
- **No `timeout` on this host.** Waiting for a long job: one blocking call,
  `<HOME>/hl/waitfor '<pgrep pattern>' <logfile>`. Run builds under ~20 min in the
  foreground instead.
- **One service per port.** Two copies of the *same* bundle need distinct `--app-data`
  (`headless.sh` uses `$STATE/app-data-<port>` automatically; `makepad_test` uses one per test).
- **`--stamp` rewrites the manifest** (threads its `integrity.bundle_blake3`). For a committed,
  immutable fixture, pre-stamp it once with `hub stamp <bundle>` and run with `HEADLESS_NO_STAMP=1`
  / omit `--stamp`. `harness/ui-smoke/fixtures/notes/bundle` is pre-stamped.

**Cleanup — close only what you started:**

```sh
bash harness/headless.sh stop <your-port>          # /gq + wait for exit + confirm free
# never:  pkill card-host / pkill octosense / kill another lane's port
lsof -iTCP -sTCP:LISTEN -P | grep -E ':83[0-9][0-9]'   # after cleanup: nothing of yours
```

`headless.sh stop` refuses to touch a port it did not start (there is no pid in `runs.tsv`),
and a human-closed window is reported by the instrument as `{"err":"window N closed by user"}` —
that is not a crash; do not relaunch.

---

## 8. Machine-readable index

[`docs/harness/index.json`](index.json) records, per proof, the app, port, pid, revision pins,
the artifact files and their sha256 — so the outer loop can re-check this card's claims without
reading prose.
