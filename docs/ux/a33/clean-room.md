# A33 clean room: "another Mac" on the build Mac

Decision D10f (`docs/decisions/D10f-standalone-desktop-app.md`). Two proofs, both against a PRIVATE `octos serve`
(a copy of the live gate's data dir, profile dsflash, a fresh random token in the serve's environment only, the app's
`OCTOS_BEARER` only), one dsflash turn each, recorded by `tools/walk/a33_other_mac_live.py`.

## The other Mac

- A fresh user HOME, `other-mac/home` under the work root (written `<OTHER_HOME>` below), with an EMPTY cargo home in it
  (`CARGO_HOME=<OTHER_HOME>/.cargo`), the system `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin`, so `/usr/bin/python3`) plus only
  rustup's proxies; the stable toolchain 1.95.0 through `RUSTUP_HOME` (a prerequisite, not part of the build).
- A fresh `git clone` of this branch's committed tree into `<OTHER_HOME>/src/octoscode-app` (commit `ee193375`, 8696
  tracked files; the repo is never pushed by an agent, so the clone reads the branch locally: the same tree a pull
  delivers). Script: the A33 lane's clean-room script, tracked since as `tools/check-fresh-clone-macos.sh <empty dir>`; log: `other-mac/clean-room.log`.
- One command: `tools/build-macos.sh --package --octosense`. It exited 0.

## Times (Apple Silicon, `CARGO_BUILD_JOBS=6`, empty cargo home: every crate downloaded)

| step | time |
|---|---|
| `git clone` | 6 s |
| 1. the three forks from GitHub (makepad shallow, OctoSense, Octoscript-Makepad) + every patch | 94 s |
| 2. the standalone app, `cargo build -p octoscode-desktop` (debug) | 63 s |
| 2b. `OctosCode.app`: the `app-bundle` (release) build 1 m 50 s, resources, Info.plist, ad-hoc signature, zip | 2 m |
| 3. the OctoSense-hosted variant: OctoSense's `setup.py` (`.sources`), our makepad patches, the vendored crates, `cargo build -p octosense --features app-octoscode` (5 m 25 s) | 5 m 26 s |
| **clone to done** | **596 s (just under 10 min)** |

Without `--octosense` (the forks, the app and its bundle) it is about 4.5 min; a Mac building with all its cores is
faster.

## Disk

| what | size |
|---|---|
| the clone's own files (`.git` 1.2 GB, `docs/` 661 MB, `design/` 364 MB) | 2.3 GB |
| `target/` (debug 1.1 GB + the bundle's release build 767 MB) | 2.0 GB |
| the forks: makepad 431 MB, OctoSense (transport) 397 MB, Octoscript-Makepad 93 MB | 0.9 GB |
| `--octosense`: `.forks/octosense-host` (its `target/` 2.6 GB, `.sources` 530 MB) | 3.8 GB |
| the cargo download cache (`~/.cargo`) | 1.2 GB |
| **total** | **10 GB** (the standalone app alone: about 6 GB) |

The bundle: `OctosCode.app` 116 MB (binary 51 MB, resources 65 MB: makepad_widgets 61.7 MB, octoscode_module 5.4 MB);
the zip `OctosCode-macos-arm64.zip` 61 MB.

## Proof 1: the from-source build streams a turn (`live-from-source/`)

`<OTHER_HOME>/src/octoscode-app/target/debug/octoscode`, HOME = the other user's: **10/10 checks**
(`live-from-source/checks.txt`): the module mounted in the standalone host; the design root and the seven kit faces
materialized under that HOME (`design embed: 7 font face(s)`, `app.log`); no resource failed to load; connected; the
turn started (Stop seen), completed, and the answer rendered ("2 + 3 = 5."); no saved file carries the token.
Captures: `from-source-01-connected.png`, `-02-streaming.png`, `-03-answered.png` (their header's workspace-path line is
redacted: it named the serve's scratch dir under the build machine's home; the later runs use a `/tmp` workspace).

## Proof 2: the zipped app, unzipped into Downloads, with every build tree unreadable (`live-unzipped-app/`)

`OctosCode-macos-arm64.zip` copied to `<OTHER_HOME>/Downloads` and unzipped there with `ditto -x -k` (what a double-click
does); `codesign --verify --strict` passes. The other user's `~/.octoscode` from proof 1 moved away first (a first
launch). For the whole run the walk RENAMES the clone (its `design/`, `crates/*/resources`, `.forks/` with makepad's
resources, `target/`) and the cargo home away (`--hide`), so nothing the binary was compiled from can be read.
**13/13 checks** (`live-unzipped-app/checks.txt`): the trees unreachable; the module mounted; "design: packaged
build — the embedded design tree and faces only"; design root and faces under the other user's HOME; no resource
failed to load (makepad's own fonts and icons from `Contents/Resources/makepad/`, the module's icons by
`file_resource` through `packaged-file-resource.patch`); connected; the turn streamed (trace: `turn/started`, a text
stream, `stream_end`), completed, and rendered "2 + 3 = 5."; no saved file carries the token. Notifications are
available from the bundle (`attention: os Authorization { status: NotDetermined }`; the bare binary of proof 1 says
"unavailable — this process does not run from an .app bundle").
Captures: `unzipped-app-01-connected.png`, `unzipped-app-02-answered.png`.

- The first attempt (`live-unzipped-app-attempt1/`, 11/12) streamed and rendered the same answer, but its walk polled
  for the Stop state after the one-second answer had already finished ("the turn starts (Stop shows)" FAIL); the walk
  now also accepts the trace's `turn/started` and checks the stream itself. Live turns used: 3 (the cap).
- Both proof-2 runs show "Permissions not reported" and no history read: the walk's `/tmp` workspace is a symlink,
  the app asked for `/tmp/a33-ws-…` and the server answered `/private/tmp/a33-ws-…`, and the module compares the two
  byte for byte (`flow.rs` "the server opened another workspace", pre-existing; the turn itself is unaffected). Proof 1,
  with a workspace path that is not a symlink, shows the full seat. A finding for the module, not the host.

## The other checks (replay server, no model)

- The unzipped clean-room bundle with the trees hidden, Connect driven by CLICKS (`unzipped-trees-hidden-connect.png`,
  `unzipped-trees-hidden-connected.png`): Connect card, server typed and read back, Connect clicked, composer shown.
- The standalone dev build: `standalone-connect-desktop.png` / `standalone-connected-desktop.png` (1280x800, window
  "OctosCode [remote]"; the menu bar name comes from the Info.plist, OctosCode); `standalone-connect-phone.png` /
  `standalone-connected-phone.png` (`OCTOSENSE_WINDOW_SIZE=360x780`: the sidebar folds into the menu, as in the
  phone shell). Bridge guard: a plain `curl /s` gets `403 {"err":"token"}`, a browser `Origin` header 403, `harness/bcurl` 200.
- The OctoSense-hosted variant the clean room built (`octosense-hosted-clean-room.png`): the OctoSense desktop with
  the OctosCode window, `wm: module instance octoscode.1 … in isolate SplashVmId(1)`, connected to the replay server.
