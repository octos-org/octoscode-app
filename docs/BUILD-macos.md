# OctosCode on macOS: run it on another Mac

OctosCode is the native octos client of this repository. It runs two ways, from the same module code
(`crates/octoscode-module`):

- **standalone**: its own app, `OctosCode.app` (crate `crates/octoscode-desktop`, binary `octoscode`). One window,
  titled OctosCode, nothing else. This is the one to give someone.
- **inside OctoSense**: a window of the OctoSense desktop (`--octosense` below), for the OctoSense integration.

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
| Network, the first time | GitHub (OctoSense, makepad, Octoscript, Octoscript-Makepad, octos) and crates.io | |

Then, in a clone of this repository:

```sh
git clone https://github.com/octos-org/octoscode-app.git
cd octoscode-app
tools/build-macos.sh              # the standalone app: target/debug/octoscode
tools/build-macos.sh --package    # + the self-contained OctosCode.app and its zip in target/macos-app/
tools/build-macos.sh --octosense  # + the OctoSense-hosted variant in .forks/octosense-host/
```

What it does (every step is a no-op when already done, so re-run it after a `git pull`):

1. Prepares the three forks the root `Cargo.toml` `[patch]`es, in `.forks/` (or `--work <dir>`, linked into `.forks/`):
   - `makepad-fork`: OctoSense-org/makepad at `6cf03859` + `patches/makepad/*.patch` (`tools/prepare-makepad-fork.sh`);
   - `octosense-fork`: OctoSense-org/OctoSense at `6e9bfd40` + `patches/octosense/0001-0003` (`tools/prepare-octosense-fork.sh`),
     for the transport crates;
   - `octoscript-makepad-fork`: Octoscript-Makepad at `6881fb6c` + `patches/octoscript-makepad/*` (`tools/prepare-octoscript-makepad-fork.sh`).
2. `cargo build -p octoscode-desktop` (`--release` for an optimized build).
3. `--package`: `tools/package-macos.sh` builds the `app-bundle` profile with makepad's packaged resource loading
   (`MAKEPAD=apple_bundle MAKEPAD_PACKAGE_DIR=makepad`), copies every crate's `resources/` into
   `OctosCode.app/Contents/Resources/makepad/`, writes the Info.plist (name OctosCode, id `org.octos.octoscode`),
   signs ad hoc and zips with `ditto`.
4. `--octosense`: a second OctoSense checkout, `.forks/octosense-host`, at the pin + 0001-0003 (0003 wires the module
   into the shell), its framework sources from OctoSense's own `tools/setup.py`, our makepad patches on them, the
   octoscode crates and `design/` vendored into `apps/`, then `cargo build -p octosense --features app-octoscode`.

Measured on this repository's build Mac (Apple Silicon, from an empty directory and an empty cargo cache):
see section 6.

## 3. Run it

```sh
target/debug/octoscode                      # or open target/macos-app/OctosCode.app
target/debug/octoscode --remote 8411        # + the instrument bridge (only when asked; harness/bcurl sends its token)
OCTOSENSE_WINDOW_SIZE=360x780 target/debug/octoscode   # the phone's shape
```

The OctoSense-hosted variant:

```sh
MAKEPAD_WM_TEST_APP=octoscode OCTOSCODE_DESIGN_DIR=$PWD/design .forks/octosense-host/target/debug/octosense --module octoscode
```

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

The server needs a profile with a model provider and its key (configured in octos; the app's Settings >
Providers can add one once connected). The app sends turns to that profile's model.

## 5. Troubleshooting

- **The first build is slow**: a cold build compiles about 280 crates for the standalone app (section 6 has the
  measured time); later builds take seconds. `--octosense` compiles the whole OctoSense desktop on top.
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

Filled in by the A33 clean-room run: see `docs/ux/a33/clean-room.md`.
