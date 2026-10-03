# OctosCode on macOS: run it on another Mac

OctosCode is the native octos client of this repository. It runs two ways, from the same UI code
(`crates/octoscode-module`):

- **standalone**: its own app, `OctosCode.app` (crate `crates/octoscode-desktop`, binary `octoscode`). One window,
  titled OctosCode. Its WebSocket transport lives in this repository; the app does not link the OctoSense
  kernel, configuration library, shell, or module host. This is the one to give someone.
- **inside OctoSense**: an optional adapter, enabled by the `octosense-module` feature (`--octosense` below).

Either way it is a client: it needs an **octos server** (`octos serve`) to talk to (section 4).

## 1. Quick path: copy the app

On the build Mac, `tools/build-macos.sh --package` (section 2) writes
`target/macos-app/OctosCode-macos-arm64.zip` (about 60 MB; the app is about 115 MB unzipped).

On the other Mac:

1. Copy the zip over (AirDrop, USB, scp) and double-click it, or `ditto -x -k OctosCode-macos-arm64.zip ~/Applications`.
   Move `OctosCode.app` to `/Applications` or `~/Applications` if you like.
2. The first launch: the app is ad-hoc signed, not notarized, so macOS refuses a plain double-click.
   - macOS 14 and older: right-click `OctosCode.app` > **Open** > **Open**.
   - macOS 15 and newer: double-click once (it is refused), then System Settings > Privacy & Security >
     **Open Anyway** next to "OctosCode was blocked".
   - Or, from Terminal, clear the download quarantine once: `xattr -dr com.apple.quarantine /Applications/OctosCode.app`.
3. Connect (section 4).

Requirements of the copied app: an Apple Silicon Mac (the zip is `arm64`; an Intel Mac builds from source,
section 2), macOS 11 or newer. Nothing else: the app carries its fonts, icons, design files and makepad's
resources. It writes its state under `~/.octoscode` (saved server, credentials, drafts, preferences, and a cache
of its design files) and nothing else outside itself.

## 2. From source on a fresh Mac

Prerequisites:

| what | how | checked by the script |
|---|---|---|
| Xcode Command Line Tools (clang, git, make, codesign) | `xcode-select --install` | yes |
| Rust, stable toolchain (built here with 1.95.0) | install rustup from https://rustup.rs, then `rustup default stable` | yes |
| Python 3 | ships with the Command Line Tools | yes |
| Disk | about 8 GB free for the standalone app; about 25 GB more for `--octosense` | warns below 8 GB |
| Network, the first time | GitHub (makepad, Octoscript, Octoscript-Makepad, octos; OctoSense only with `--octosense`) and crates.io | |

Then, in a clone of this repository:

```sh
git clone https://github.com/octos-org/octoscode-app.git
cd octoscode-app
tools/build-macos.sh              # the standalone app, optimized: target/release/octoscode
tools/build-macos.sh --package    # + the self-contained OctosCode.app and its zip in target/macos-app/
tools/build-macos.sh --octosense  # + the OctoSense-hosted variant (optimized too)
tools/build-macos.sh --debug      # development only: debug builds in target/debug/ (a slower UI)
```

What it does (every step is a no-op when already done, so re-run it after a `git pull`):

1. Prepares the two renderer forks the root `Cargo.toml` `[patch]`es, in `.forks/` (or `--work <dir>`, linked into `.forks/`):
   - `makepad-fork`: OctoSense-org/makepad at `6cf03859` + `patches/makepad/*.patch` (`tools/prepare-makepad-fork.sh`);
   - `octoscript-makepad-fork`: Octoscript-Makepad at `6881fb6c` + `patches/octoscript-makepad/*` (`tools/prepare-octoscript-makepad-fork.sh`).
2. `cargo build --release -p octoscode-desktop`: the optimized app. `--debug` builds the dev profile instead
   (`target/debug/octoscode`), for development only: the root `Cargo.toml` optimizes its dependencies, but the
   UI is still slower than a release build. Measured with the Makepad instrument on the standalone app
   (`tools/walk/a35_browser_latency_walk.py`): a fully unoptimized build took 0.3-0.7 s to open and navigate the
   folder browser, an optimized one well under 0.1 s. `--release` is still accepted; it is the default.
3. `--package`: `tools/package-macos.sh` builds the `app-bundle` profile with makepad's packaged resource loading
   (`MAKEPAD=apple_bundle MAKEPAD_PACKAGE_DIR=makepad`), copies every crate's `resources/` into
   `OctosCode.app/Contents/Resources/makepad/`, writes the Info.plist (name OctosCode, id `org.octos.octoscode`),
   signs ad hoc and zips with `ditto`.
4. `--octosense`: an optional OctoSense checkout, `.forks/octosense-host`, at the pin + 0001-0003 (0003 wires the module
   into the shell), its framework sources from OctoSense's own `tools/setup.py`, our makepad patches on them, the
   octoscode crates and `design/` vendored into `apps/`, then
   `cargo build -p octosense --features app-octoscode,octoscode-module/octosense-module`.
   Both the standalone app and the OctoSense host are optimized unless `--debug` is given.
   The packaged `.app` always uses the optimized `app-bundle` profile.

From a fresh clone with an empty cargo cache it took 10 minutes and 10 GB here (4.5 minutes and about 6 GB without
`--octosense`); section 6 has the breakdown. To repeat that check on any Mac without touching your own setup:
`tools/check-fresh-clone-macos.sh <an empty directory>` (a fresh HOME and cargo home inside it, a fresh clone, then
`tools/build-macos.sh --package --octosense`; log in `<dir>/fresh-clone.log`).

## 3. Run it

```sh
target/release/octoscode                    # or open target/macos-app/OctosCode.app
target/release/octoscode --remote 8411      # + the instrument bridge (only when asked; harness/bcurl sends its token)
OCTOSCODE_WINDOW_SIZE=360x780 target/release/octoscode   # the phone's shape
```

A `--debug` build is `target/debug/octoscode`.

The OctoSense-hosted variant, built with `--octosense`:

```sh
MAKEPAD_WM_TEST_APP=octoscode OCTOSCODE_DESIGN_DIR=$PWD/design .forks/octosense-host/target/release/octosense --module octoscode
```

Use `target/debug/octosense` instead after a `--debug` build.

## 4. Connect it to an octos server

The first window is the **Connect to Octos** card:

- **Server**: the server's address, e.g. `http://127.0.0.1:50190`, or `http://<the server's IP>:<port>` on your network.
- **Access token**: the server's token (the value its `OCTOS_AUTH_TOKEN` was started with). It is stored for that
  server only, under `~/.octoscode/credentials`.
- **Connect**. Or **Pair with a link instead** with a pairing link from the server, or **Use local solo server** for an
  `octos serve --solo` on the same Mac.

Launched from Terminal, the same values can come from the environment (nothing is written to the screen):

```sh
OCTOS_BASE_URL=http://127.0.0.1:50190 OCTOS_BEARER=<token> OCTOS_PROFILE_ID=<profile> \
  /Applications/OctosCode.app/Contents/MacOS/octoscode
```

No server yet? octos is one Rust binary. On the same or another Mac, at the protocol revision this app pins:

```sh
cargo install --git https://github.com/octos-org/octos --rev a6ea8505170735f191a12bb47629a6728415d417 --features api octos-cli
export OCTOS_AUTH_TOKEN=$(openssl rand -hex 24)      # keep it; the app asks for it
octos serve --port 50190                              # --host 0.0.0.0 to accept other machines
```

The server needs a profile with a model provider and its key (configured in octos, or from the app once connected:
Settings > Model providers > Add provider). The app sends turns to that profile's model.

## 5. Troubleshooting

- **The first build is slow**: a cold build compiles about 280 crates for the standalone app (section 6 has the
  measured time); later builds take seconds. `--octosense` compiles the whole OctoSense desktop on top.
- **The app feels slow** (typing, the folder browser, the sidebar): check that it is an optimized build —
  `target/release/octoscode`, the packaged `.app`, or the zip. A `--debug` build is for development only.
- **Disk**: the cargo target directory holds most of it (section 6). `cargo clean` frees it; the forks are in
  `.forks/` (about 1 GB), the shared download cache in `~/.cargo`.
- **"the Xcode Command Line Tools are missing"**: `xcode-select --install`, then re-run.
- **A fork refuses to update** ("uncommitted changes"): the script never discards local edits in `.forks/`; move them
  away, or re-run the named `tools/prepare-*-fork.sh` with `--force`.
- **`.forks/<name> is a link`**: a fork shared with other checkouts is never modified by this script; remove the
  link or pass `--work <dir>`.
- **The app opens but shows no text or icons**: the bundle was built without `tools/package-macos.sh` (a plain
  `cargo build` binary reads makepad's resources from the build machine's cargo checkouts). Use the zip from
  `--package`.
- **"OctosCode is damaged" / it will not open**: the quarantine of a downloaded copy; see section 1, step 2.
- **Notifications**: macOS asks once, the first time OctosCode wants to show one (only the `.app` can post them).

## 6. Measured (A33, clean room)

A fresh clone under another user's HOME with an empty cargo cache, Apple Silicon, 6 build jobs,
`tools/build-macos.sh --package --octosense` (details and the two live checks: `docs/ux/a33/clean-room.md`):

| | time | disk |
|---|---|---|
| clone | 6 s | 2.3 GB (the repo's own files) |
| the three forks | 1.5 min | 0.9 GB |
| the standalone app (debug) | 1 min | |
| `--package` (release build, bundle, zip) | 2 min | `target/` 2.0 GB in all |
| `--octosense` | 5.5 min | 3.8 GB |
| cargo download cache | | 1.2 GB |
| **in all** | **10 min** (4.5 min without `--octosense`) | **10 GB** (about 6 GB without `--octosense`) |

The bundle is 116 MB (zip 61 MB). A Mac building with all its cores is faster; a later build after a `git pull`
takes seconds to a minute.

A33 measured the standalone step when its default was a debug build. Since A35b the default is the release
build (the `--package` row's kind of build), and the dev profile optimizes its dependencies: both compile longer
on a cold cache than that debug build did, in exchange for an app whose UI is not slowed down by its build.
