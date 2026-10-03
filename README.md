# OctosCode

English | [简体中文](README.zh-CN.md)

OctosCode is a native desktop client for [Octos](https://github.com/octos-org/octos),
built with Makepad and Octoscript. It provides a desktop interface for coding conversations,
workspaces, model providers, and session permissions, alongside
[octoscode-web](https://github.com/octos-org/octoscode-web).

The standalone app runs independently of OctoSense. Its default build has no OctoSense
shell, kernel, configuration library, or module host dependency. It connects to a
separately running `octos serve` over WebSocket using `octos-ui/v1alpha1` (JSON-RPC 2.0).

## Try the release candidate

Open the **[v0.1.0-rc.1 release page](https://github.com/octos-org/octoscode-app/releases/tag/v0.1.0-rc.1)**
for downloads and platform availability. This is a prerelease for testing.

| Platform | Package |
|---|---|
| macOS, Apple Silicon | `OctosCode-macos-arm64.zip` |
| Linux, x86_64 | `OctosCode-linux-x86_64.tar.gz` — packaging and validation pending |
| Windows, x86_64 | `OctosCode-windows-x86_64.zip` — availability pending; may be deferred from this RC |

On macOS, unzip the download, move `OctosCode.app` to Applications, and launch it.
The bundle includes its UI resources; it does not require Rust, a source checkout,
or OctoSense on the machine running it. It is ad-hoc signed and not notarized. If macOS
blocks it, use **System Settings → Privacy & Security → Open Anyway** after attempting
to open the copy downloaded from this repository.

Follow the **[quickstart](docs/QUICKSTART.md)** to start an Octos server, connect the app,
and configure a model provider. The desktop download does not include the server.

## Build on macOS

With Xcode Command Line Tools, stable Rust, and Python 3 installed:

```sh
git clone https://github.com/octos-org/octoscode-app.git
cd octoscode-app
tools/build-macos.sh --release --package
open target/macos-app/OctosCode.app
```

The script prepares the pinned renderer forks and builds the app. `--release` optimizes
the executable; `--package` always builds the bundle with the optimized `app-bundle`
profile. Use optimized builds for everyday use and performance testing. Without either
flag, the script produces a debug executable.

An optional OctoSense adapter is available with `tools/build-macos.sh --octosense --release`.
The `--release` flag applies to both the standalone executable and the OctoSense host.
See the [macOS build guide](docs/BUILD-macos.md) for prerequisites, output paths,
host launch commands, and troubleshooting.

## Using the app

- Connect using the server address and access token, a fresh pairing link, or **Use local solo server**.
- Configure a provider in **Settings → Model providers**, then choose a workspace and session.
- Drag the left sidebar's right edge to resize it. Collapse it to an icon rail and expand it to restore its width.
- Connection settings, credentials, drafts, and preferences are saved under `~/.octoscode`.

For scripted launches, the executable accepts `--server <url> --token-file <path> --workspace <directory>`;
see the [quickstart](docs/QUICKSTART.md#launch-with-an-existing-token-file).
