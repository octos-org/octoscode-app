# D9 spike — a minimal octoscode app in both shapes

Task #6. Decides D9 (app shape) with evidence: the *smallest* octoscode app built in **both** shapes inside
an APFS copy of OctoSense (`tmp/octosense`), each connecting to a **local `octos serve`**, opening a session,
showing it in the UI, and proven with the headless instrument.

**Verified here** = I ran it and the result is in `evidence/`. **Inferred** = reasoned from the source, not run.

## Environment

| Item | Value |
| --- | --- |
| OctoSense base | `6e9bfd4077cf8181878ac005ad586f908084a4bf` (ref clone → `tmp/octosense`) |
| octos for the spike | `tmp/octos` @ `a6ea8505170735f191a12bb47629a6728415d417` (OctoSense's pin), release-built |
| `octos --version` | `octos 2.0.3-rc.13 (a6ea8505 2026-09-27)` |
| serve | `octos serve --port 50081 --host 127.0.0.1 --solo --auth-token spike-dummy-token --data-dir tmp/octos-serve-home --instance-data-dir tmp/octos-serve-data` |
| ports | 8390 (shape i), 8391 (shape ii) — inside the 8390–8399 block |
| model turns | none (RULES: stop at session open) |

## Wire finding (both shapes depend on it)

A fresh `octos serve` has **no profile**, so `session/open` fails `profile_unresolved` until a profile
exists. The client onboards with `profile/local/create` first (local-solo only):

```
{"method":"profile/local/create","params":{"requested_id":"octoscode","name":"OctosCode","username":"octoscode"}}
-> {"result":{"profile_id":"octoscode",...,"runtime_mode":"solo"}}
{"method":"session/open","params":{"session_id":"octoscode:main","profile_id":"octoscode"}}
-> {"result":{"opened":{"session_id":"octoscode:main","active_profile_id":"octoscode",...}}}
```

`session/list` needs the `auxiliary.rest_to_ws.v1` feature in the `x-octos-ui-features` handshake header;
without it the server answers `-32004 method not supported`. **Verified** (`tmp/probe_ws.py`, `tmp/listprobe.py`).

## Shape (i) — native `AppModule`

Crate `apps/octoscode/module` (copy of the `apps/reference` pattern): depends on AppCard's
`octos-app-transport` / `octos-app-store` directly, spawns a `tokio` runtime, dials the serve over the UI
Protocol WebSocket, calls `session/open`, and renders one Label with the session id plus a Button that calls
`session/list` and shows the count. Registered behind feature `app-octoscode` per recipe (i) of
`docs/baseline.md` §5: root `Cargo.toml` members + `[workspace.dependencies]`, `crates/shell/Cargo.toml`
optional dep + feature, `linked_modules()` push, `desktop/Cargo.toml` feature forward.

Proof (`evidence/native/`): launch hidden `MAKEPAD_HIDE_WINDOWS=1 MAKEPAD_WM_TEST_APP=octoscode <bin>
--module octoscode --remote 8390`; `/snap` shows the labels; `/click` the button; `/snap` again; `/gq`.

| | |
| --- | --- |
| listening | `[makepad-remote] listening on 127.0.0.1:8390 pid=16813 app=octosense` |
| `/snap` | `session :: session: octoscode:main` · `status :: conn: Live   sessions listed: 0` |
| click | `{"ok":1,"f":14028}` |
| `/gq` | `{"quit":1}` |
| app log | `[octoscode] session: octoscode:main \| conn: Live` |

Note: with no model turns, `session/list` is genuinely empty (`{"sessions":[]}` — verified directly), so the
count is `0` both times. The button round-trip is what the click exercises; the count text is real.

## Shape (ii) — contained bundle + Rust host service

- `apps/octoscode/bundle/{manifest.json,main.splash}`: the script holds no socket/token/protocol code; it
  calls `host.request("octoscode.session_open")` / `("octoscode.session_list")` and renders the same Label +
  Button. Root is `HostedView{ … }` (the real bundle form; a `script_mod!{ Root{ Window{} } }` wrapper does
  **not** render — see blockers).
- `apps/octoscode/host-service`: crate `octosense-octoscode-service`, owns the same AppCard crates on a
  background `tokio` runtime (`ws::spawn`), answers the two requests, replies through `Replier::send`.
- Registered per recipe (ii): member in root `Cargo.toml`, workspace pin, `register_host_services()` call
  (`crates/shell/src/apps.rs`), `desktop/system-apps.json` listing, manifest `capabilities:["storage","octoscode"]`.

Proof (`evidence/script/`): launch hidden
`MAKEPAD_HIDE_WINDOWS=1 MAKEPAD_WM_TEST_APP=octoscode-script <bin> --remote 8391`.

| | |
| --- | --- |
| listening | `[makepad-remote] listening on 127.0.0.1:8391 pid=59334 app=octosense` |
| admission | `card: os.octoscode-script running under 2 capability(ies), 1 host(s), 67108864 bytes of storage, …` |
| `/snap` before | `session :: session: octoscode:host` · `status :: conn: session open` |
| click | `{"ok":1,"f":1617}` |
| `/snap` after | `status :: conn: sessions listed: 0` |
| `/gq` | `{"quit":1}` |
| serve log | `DEBUG appui context manager loaded for session open session=octoscode:logprobe ledger_status=Loaded` |

The click changes the status label `conn: session open` → `conn: sessions listed: 0`: the host-service round
trip is visible. **Verified.**

## Blockers found (and how each was cleared)

1. **`octoscode` is not a grantable capability.** `octosense-app-policy`'s `KNOWN_CAPABILITIES` is a closed
   list (`crates/app-policy/src/manifest.rs:15-17`, "a capability that is not here cannot be granted"); the
   first launch was refused:
   `card: Cannot open OctosCode (script): app os.octoscode-script requests unknown capability "octoscode"`.
   The `news` bundle documents the same wall (`apps/news/bundle/main.splash:24`, `TODO(App-Hub#18)`).
   **Cleared for the spike** by patching a local App Hub copy (`patch/app-hub.diff`, 3 added lines) and
   pointing the workspace at it; that is a spike-only change, not a proposal.
2. **A bundle's UI must be a `HostedView{ … }` root**, not a `script_mod!{ Root{ Window{ … } } }`. My first
   bundle compiled but rendered the raw source, and a top-level `let session` / `let status` collided with the
   widget ids (`splash: widget 'status' not found in tree`). Fixed by mirroring `apps/news/bundle` (no
   top-level bindings named like widgets; `start_timeout(0.05, || boot())`).
3. **`serve` needs `--solo`** for `profile/local/create`; without it every `session/open` is
   `profile_unresolved` (there is no profile yet).
4. **Desk default avoids the module**: a linked module boots only with `--module <id>` AND a boot trigger
   (`MAKEPAD_WM_TEST_APP=<id>`); `--module` alone sets hosting but opens nothing.

## Comparison

| Axis | Shape (i) native `AppModule` | Shape (ii) bundle + host service |
| --- | --- | --- |
| LOC | 283 (265 `.rs` + 18 `.toml`) | 336 (256 `.rs` + 28 `.toml` + 44 `.splash` + 8 `.json`) |
| Files touched outside the app dir | 5 (`Cargo.toml`, `crates/shell/Cargo.toml`, `crates/shell/src/apps.rs`, `desktop/Cargo.toml`) + `Cargo.lock` | 3 (`Cargo.toml`, `crates/shell/src/apps.rs`, `desktop/system-apps.json`) + `Cargo.lock` |
| Capability gate | none (trusted in-process module) | App Hub manifest `capabilities` — **closed list**; needs a policy change to add a new family |
| Async protocol stream → UI | direct: the module owns the transport's `mpsc::Receiver<TransportEvent>` on its own runtime and reads it in `handle_event` on `Event::Signal` (**verified**: `[octoscode] … conn: Live`) | indirect: the host service drains events into a `Mutex<Snapshot>`; the isolate only learns the **replies** to its requests. **No push/watch API exists** — the `host` bridge exposes only `request`, `has`, `capabilities` (`splash_host.rs`: `script_mod` registers `request`/`has`; no `subscribe`/`watch`/`emit`). A `message/delta`-style notification would need a new host→isolate event channel. |
| Design flow (`ref/OctoScript-App-Design-Flow`) | not covered — the flow produces **bundles**, not Rust modules (`AGENTS.md` "What this repository is / is not": the runtime is makepad/App Hub, flows produce checked bundles) | directly: `flows/script-app/FLOW.md` and `flows/image-to-card` (atlas→bundle) both target this shape; `tools/octo run/check/shot` drive `card-host` |
| Headless proof | one process, one port, `/snap` reads the module's own widgets | one process, one port, `/snap` reads the isolate's widgets (the `card` Splash) |
| Secret/token handling | in-process (trusted) | host-owned; the isolate never sees the token |
| Freshness/reload | rebuild the linked shell | swap the bundle; digest-admitted |

## Recommendation

**For the octoscode app: shape (ii), the contained bundle + host service** — with one prerequisite.

- It matches where the toolchain points: the design flow and `tools/octo` produce and check **bundles**, not
  Rust modules; shape (i) is outside that pipeline. A new app authored as a bundle ships without a shell
  rebuild and is admitted by digest.
- It keeps the token and the protocol in the host, which is the app-security model the shells already enforce.
- **Prerequisite (blocker 1):** the app needs a capability family App Hub does not know. Either reuse an
  existing family (if the app's real job maps onto `octos.session.*`/`matrix.*`) or add the family to
  `octosense-app-policy`'s `KNOWN_CAPABILITIES` upstream — the `news` bundle is already waiting on the same
  change (`TODO(App-Hub#18)`).
- **Caveat (the one real cost):** shape (ii) has **no host→isolate push channel** today. A live
  `message/delta` stream cannot reach a contained app's UI; only request replies can. If the octoscode app
  must render a streaming turn, either add a host→isolate event API (a `host.subscribe` in
  `splash_host.rs` + a matching dispatch in `appstore::services`) or start with shape (i) for that screen.

Which facts are **verified** (ran here): both shapes' launch/snap/click/quit evidence; the wire onboarding
flow; the closed-capability refusal; the missing push API (by reading `splash_host.rs`'s registered
`host` module — no subscribe/watch). **Inferred**: that the design flow covers only bundles (read from its
`AGENTS.md`/`flows/README.md`, not run through `tools/octo`).

## Acceptance

- `git apply --check patch/octosense.diff` against `6e9bfd4` → `RC=0` (**verified**).
- `lsof -iTCP -sTCP:LISTEN -P | grep -E ':839[0-9]'` → no output after cleanup (**verified**); serve stopped.
