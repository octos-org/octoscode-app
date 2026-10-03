# octoscode-app

Native (Makepad + Octoscript) port of [octoscode-web](https://github.com/octos-org/octoscode-web),
packaged as an independent desktop app. It connects to `octos serve` over WebSocket using
`octos-ui/v1alpha1` (JSON-RPC 2.0). Its default build has no OctoSense shell, kernel,
configuration library, or module host dependency.

Build and run it on a Mac (the standalone OctosCode app, or a zip to copy to another Mac):
`tools/build-macos.sh [--package] [--octosense]`, see `docs/BUILD-macos.md`.

```sh
tools/build-macos.sh --package
open target/macos-app/OctosCode.app
```

Connect from the app, or pass `--server <url> --token-file <path> --workspace <directory>`
to the executable (after `open ... --args` for the app bundle). The app remembers the connection.
The optional `--octosense` build enables the `octosense-module` adapter for embedding in that host.

Drag the left sidebar’s right edge to resize it. The collapse button switches to the icon rail and restores the chosen width when expanded.
