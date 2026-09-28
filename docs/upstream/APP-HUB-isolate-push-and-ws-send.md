# DRAFT for the operator to file — OctoSense-App-Hub (+ makepad script std): isolate push channel and WebSocket send

**Status:** draft, not filed. Filing is the operator's decision (supervisor 8.8 condition 3).

## Problem
A contained OctoScript app cannot carry a live, server-pushed UI (a coding agent's streaming turn, approvals, task
progress), for two independent reasons, both reproduced in spikes on 2026-09-28:

1. **No host→isolate push.** The isolate's `host` object exposes only `request`, `has`, `capabilities`
   (`makepad/widgets/src/splash_host.rs:243-335`). A host service can only answer requests, never notify.
   Evidence: `octoscode-app/docs/spike-d9/REPORT.md` (shape ii).
2. **No WebSocket send.** `net.web_socket` returns an id and delivers `on_opened/on_string/on_binary/on_closed`, but the
   script std exposes no send (`write_string not found on id`), although the network layer implements it
   (`platform/network/src/runtime.rs:55 ws_send`). `net.socket_stream` is refused for policed isolates. Evidence:
   `octoscode-app/docs/spike-oup-script/REPORT.md`.

## Ask (either suffices for push; both are small)
- **A. `host.subscribe(topic, fn(event))`** in `splash_host.rs`, plus a matching dispatch in App Hub's services, so a host
  service can push events to the isolate it serves (gated by the same capability family).
- **B. A send method on the `net.web_socket` handle** (`write_string`/`write`), routed to `NetworkBackend::ws_send`,
  under the existing per-isolate allowlist (~30 lines in `platform/script/std/src/net.rs`).

## Related (separate issue suggested)
- **Capability family for local agent clients.** `KNOWN_CAPABILITIES` is closed (`app-policy/src/manifest.rs:18`);
  `octoscode` (or an `octos.*` family) is not grantable. News waits on the same (`TODO(App-Hub#18)`).
- **Loopback grant for an app's own local service.** A manifest can't list `127.0.0.1:<port>`. Only the host can
  (`card-host/src/host.rs:275`). A supported, per-app way to reach its own local server would be needed for B.
- **Script state across the 1 Hz `sys.simsecs` re-eval** (`splash.rs:641`) resets module state (spike #7 finding 3).

## Why it matters
Without A or B, only read-mostly panels can be contained apps. octoscode-app therefore ships as a native `AppModule`
(its D9). With A (+ the capability family), its settings/fleet panels could move to contained bundles later.
