# OctosCode quickstart

English | [简体中文](QUICKSTART.zh-CN.md) · [README](../README.md)

OctosCode is a standalone desktop **client**. You also need an Octos server, either on
the same machine or at an address supplied by its administrator. OctoSense is optional.

## 1. Install the app

Open the [v0.1.0-rc.1 release page](https://github.com/octos-org/octoscode-app/releases/tag/v0.1.0-rc.1)
for the macOS ARM64 and Linux x86_64 downloads. The Windows build succeeds, but its
public release is deferred until native startup and persistent data directories are validated.

On an Apple Silicon Mac:

1. Download `OctosCode-macos-arm64.zip`.
2. Unzip it and move `OctosCode.app` to `/Applications` or `~/Applications`.
3. Open the app. This prerelease is ad-hoc signed, not notarized. If macOS blocks it,
   go to **System Settings → Privacy & Security → Open Anyway** after attempting to
   open the copy downloaded from this repository.

On Linux, download `OctosCode-linux-x86_64.tar.gz` for the
Ubuntu 22.04 or newer x86_64 target, extract it, and launch from a graphical desktop:

```sh
tar -xzf OctosCode-linux-x86_64.tar.gz
cd OctosCode
./octoscode
```

Keep the `makepad/` directory beside the executable. Windows packages produced by CI
are development artifacts for now: several persistence paths still rely on `HOME`,
which may be absent when launching from Explorer. They are not part of this RC download.

The download contains the app and its resources. It does not start a server, and it does
not require a source checkout, Rust, or OctoSense to run.

## 2. Start or locate a server

If you already have a running server, skip to step 3. For a local setup, install the
Octos CLI with stable Rust and the native build tools for your system (Xcode Command
Line Tools on macOS). This command uses the server revision pinned by the client's
protocol dependency:

```sh
cargo install --git https://github.com/octos-org/octos \
  --rev a6ea8505170735f191a12bb47629a6728415d417 --features api octos-cli
octos serve --solo --host 127.0.0.1 --port 50190
```

Keep that terminal running. The `api` feature includes the WebSocket endpoint; no
separate WebSocket switch is needed. `--solo` enables the local single-user login used
below. Keep this mode on loopback, without a reverse proxy.

For a server managed by someone else, obtain its URL and an access token or a fresh
pairing link. See the [Octos repository](https://github.com/octos-org/octos) for server configuration.

## 3. Connect and send a first message

1. In **Connect to Octos**, enter the server address. For the command above, use
   `http://127.0.0.1:50190` and choose **Use local solo server**.
2. For a token-protected server, enter its access token and click **Connect** instead.
   **Pair with a link instead** accepts a fresh pairing link from the server.
3. Complete provider setup if prompted. You can also open **Settings → Model providers**
   to add the provider, API key, and model available to your account.
4. Select a workspace and create or open a session, then send a message. A remote
   server works with directories on the server's machine.

Drag the sidebar's right edge to resize it. Its collapse button switches to an icon rail;
expanding restores the chosen width. The app remembers connections and preferences
under `~/.octoscode`.

### Launch with an existing token file

If you already have a file containing the server's access token, you can launch the app
without placing the token value in your command line. Quit an existing app instance
before using these launch arguments. For the macOS bundle:

```sh
open /Applications/OctosCode.app --args \
  --server http://127.0.0.1:50190 \
  --token-file /path/to/server.token \
  --workspace /path/to/project
```

Replace the paths and address with your own. `--token-file` is read on the client
machine and saves the token for that server. `--workspace` selects the working directory
on the server. The `octoscode` executable accepts the same three flags directly.

## 4. Build an optimized version on macOS

Prerequisites: macOS, Xcode Command Line Tools (`xcode-select --install`), stable Rust,
Python 3, Git, and network access to GitHub and crates.io.

```sh
git clone https://github.com/octos-org/octoscode-app.git
cd octoscode-app
git checkout v0.1.0-rc.1
tools/build-macos.sh --release --package
open target/macos-app/OctosCode.app
```

The outputs are `target/release/octoscode`, `target/macos-app/OctosCode.app`, and
`target/macos-app/OctosCode-macos-arm64.zip` on Apple Silicon. If `CARGO_TARGET_DIR` is
set, those output paths use that directory instead of `target`.

The bundle always uses the optimized `app-bundle` profile. On this RC tag the separate
executable needs `--release` to be optimized; on the default branch the optimized build is
the default and `--debug` opts into a debug build, which is substantially slower and meant
for development only. Omit the `git checkout` command to build the default branch instead
of this RC.

To build the optional OctoSense host as well:

```sh
tools/build-macos.sh --octosense --release
MAKEPAD_WM_TEST_APP=octoscode OCTOSCODE_DESIGN_DIR="$PWD/design" \
  .forks/octosense-host/target/release/octosense --module octoscode
```

Here `--release` applies to both executables (the default on the default branch). This optional host is not included in the
standalone download. See the [macOS build guide](BUILD-macos.md) for the full build workflow.

### Native Linux and Windows packaging

The [desktop build workflow](../.github/workflows/desktop-packages.yml) lists the native
dependencies for Ubuntu 22.04 and the Windows build environment. After installing those
dependencies, Rust, Python 3, and Git, run the following in a clone (use Git Bash on Windows):

```sh
bash tools/prepare-makepad-fork.sh
bash tools/prepare-octoscript-makepad-fork.sh
python3 tools/package-desktop.py
```

On Windows, use `python` if that is your Python 3 command. The script builds for the
current platform with the optimized `app-bundle` profile and writes the portable archive
and its SHA-256 checksum to `target/desktop-packages/`. It does not cross-compile.

## Troubleshooting

| Symptom | Check |
|---|---|
| Cannot connect | Keep `octos serve` running and confirm its host and port match the app. |
| Token rejected | Use the current token for that server, or the local solo button when the server was started with `--solo`. |
| Pairing link already used | Generate a fresh link on the server; used links cannot be reused. |
| Connected, but model calls fail | Check the selected provider, model, and API key in Settings. Server access tokens and provider API keys are separate credentials. |
| Sidebar, typing or the folder browser feels slow | Use the release bundle or an optimized executable (`tools/build-macos.sh` builds one by default; `--release` on the RC tag). A `--debug` build is substantially slower. |
| Missing fonts or icons after copying a binary | Copy the packaged `.app` or release ZIP; a bare source-build executable is not the portable bundle. |
| macOS blocks launch | Follow the per-app approval in step 1 for the download from this repository. |

## Session history, memory and skills in current source builds

Open **Session history** in the sidebar (or `/sessions`) to browse the server's
Profile history and known projects. Each row names its Profile and workspace;
**Load more** pages through older sessions. Add an older workspace if it has not
been opened on this client. The catalog reports unavailable paths and never
assumes it has scanned the server's entire filesystem.

**Settings → Capabilities → Skills** shows instruction skills backed by `SKILL.md`,
grouped by the sources actually loaded for the current session. Tools and MCP
servers have separate inventories; loading a tool plugin does not make it a skill.
Global/project groups may be empty when the server does not load instruction
skills from those layers. The separate Installed list manages
Profile packages; global or built-in entries cannot be removed from that list.

**Memory** shows the current session's authorized storage scope. Ordinary
sessions share persistent memory within a Profile, across projects. App-owned
namespaces are isolated and require the owning app's credential. Changing sessions
refreshes resource panels and rejects late replies from the previous session.
Viewing a memory record does not increment model retrieval statistics.

These additions require a matching server advertising `session/history/list`,
`memory.session_scope.v1` and `skills.effective_catalog.v1`. They are source-build
features and are not included in the older RC download linked above.
