# octoscode-app

Native (Makepad + Octoscript) port of [octoscode-web](https://github.com/octos-org/octoscode-web), packaged as a
desktop binary and an OctoSense `AppModule`. It speaks `octos-ui/v1alpha1` (JSON-RPC 2.0) to `octos serve`:
WebSocket on desktop, a spawned `serve --stdio` on Android. Protocol target: octos `4231669` (the web client's pin).

Status: Phase 0 (ground truth). See `docs/phase0/`.

Build and run it on a Mac (the standalone OctosCode app, or a zip to copy to another Mac):
`tools/build-macos.sh [--package] [--octosense]`, see `docs/BUILD-macos.md`.

Drag the left sidebar’s right edge to resize it. The collapse button switches to the icon rail and restores the chosen width when expanded.
