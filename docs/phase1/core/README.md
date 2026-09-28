# Phase 1 core — client + dispatch registry + session store + module skeleton

The shape every later fan-out lane builds inside. One card, before any fan-out:
**one file per domain**, and no lane ever edits a shared dispatch/store file.

## The layers

```
octoscode-module   AppModule: owns the transport on a tokio runtime, drains
                   TransportEvents into the store, renders conn + count.
                   Exposes store data to cards ONLY through bindings.rs.
      │
      ├── octoscode-client   Method trait + Client::call (typed) / Client::request
      │                      (generic) + Registry (notification routing).
      │        │
      │        └── domains/  ONE FILE PER DOMAIN. Each exports its Method impls
      │                      and a register() for its notifications.
      │
      └── octoscode-store    THE public session-store API: sessions, per-session
                             timeline, connection state, capabilities. Public API is
                             ours; it may wrap octos-app-store internally.

octos-app-transport (git, D10)   one generic OutboundCommand::Request (D10a)
octos-core          (git)        protocol types + method-name constants
```

`octoscode-client/src/domains/mod.rs::register_all` is the **only** shared line
per domain: it calls each domain's `register()` once.

## How to add a method in your domain (≤ 10 lines — the fan-out recipe)

1. In **your** file `crates/octoscode-client/src/domains/<domain>.rs`, add a
   unit struct implementing `Method`:
   ```rust
   pub struct MyThing;
   #[derive(Serialize)] pub struct MyThingParams { pub id: String }
   #[derive(Deserialize)] pub struct MyThingResult { pub ok: bool }
   impl Method for MyThing {
       const NAME: &'static str = methods::MY_THING;   // the exact wire name
       type Params = MyThingParams;
       type Result = MyThingResult;
   }
   ```
2. Call it: `client.call::<MyThing>(params).await?`.
3. (Optional) re-export it from `mod.rs` next to the others.

That is all. No shared file changes, no enum to extend.

## How to add a notification in your domain

1. Add a handler in your file:
   ```rust
   pub struct MyNotifHandler { pub store: Arc<Store> }
   impl NotificationHandler for MyNotifHandler {
       const METHOD: &'static str = methods::MY_NOTIFICATION;
       fn handle(&self, n: &UiNotification) { /* match n, write the store */ }
   }
   ```
2. Add **one line** to your `register()`:
   `reg.register(MyNotifHandler { store });`

An unregistered method is not an error: `Registry::dispatch` logs
`debug!("unhandled notification method '…'")` by name and continues (RULES #6,
8.8 condition 7). Unknown **requests** from the server get a JSON-RPC "method
not found" reply and the connection continues (transport-side).

## How to add state in your domain (≤ 10 lines — the store recipe)

The store is split the same way the client is (**card #10**): one file per
domain, so a lane owns one file here too.

1. Add fields/methods to **your** domain in
   `crates/octoscode-store/src/domains/<domain>.rs` (each is its own struct with
   its own `Mutex`, constructed by `Default`):
   ```rust
   impl MyDomain {
       pub fn set_thing(&self, v: String) { self.inner.lock().unwrap().thing = v; }
       pub fn thing(&self) -> Option<String> { self.inner.lock().unwrap().thing.clone() }
   }
   ```
2. Read/write it from **your** client domain's handler as
   `store.domains.<domain>.<method>(…)`.
3. A new transcript kind is a `const` in **your** file — never an edit to a
   shared enum:
   `const MY_KIND: EntryKind = EntryKind::new("my.domain.thing");`
   then `store.domains.session.timeline.append(session, turn, MY_KIND, text)`.

A new domain is `pub mod` + one field in `domains::State` (the single
construction site). No lane edits another lane's file.

## Implemented in this card

| Method | Where | Note |
| --- | --- | --- |
| `session/list` | `domains/session.rs` | via `Client::call`; `cwd: None` (legacy listing) |
| `config/capabilities/list` | `domains/config.rs` | result is an **object** (`supported_methods`, `supported_notifications`), not a list |
| `tool/status/list` | `domains/tool.rs` | AppUI **extension** method (`packages/client/src/inventory-methods.ts:3`) |
| `session/open` | (typed transport) | carries the replay cursor bracket — deliberately not a `Method` |

Notifications implemented: `session/opened`-class (`session/open`),
`message/delta`, `turn/started`, `turn/completed`, `turn/error`,
`tool/started|progress|completed`, `approval/requested`, `task/updated`,
`task/output/delta`.

## Stub domains → the matrix rows they own

Each stub is a real file with a header comment naming its methods; its fan-out
lane fills it in.

| File | Methods it owns (from `docs/protocol-matrix.csv` + `-ext`) |
| --- | --- |
| `session.rs` | `session/*` (31 rows incl. `session/goal/*`, `session/driver/*`, `session/wake/*`) — `session/list` done |
| `turn.rs` | `turn/start`, `turn/steer`, `turn/interrupt`, `turn/state/get` + the streaming notifications |
| `tool.rs` | `tool/status/list` (done) + `tool/*` notifications |
| `approval.rs` | `approval/respond`, `approval/scopes/list` + 4 approval notifications |
| `review.rs` | `review/start` |
| `task.rs` | `task/*` (8 rows) |
| `autonomy.rs` | `agent/*`, `loop/*`, `monitor/*`, `session/goal/*`, `background/activity` (M15) |
| `peer.rs` | `peer/*` (6 rows) |
| `profile.rs` | `profile/*` (15 rows) + `onboarding/*` (2) |
| `media.rs` | `visual/*`, `voice/*`, `content/*`, `file/attached`, `smart_home/*` |
| `config.rs` | `config/capabilities/list` (done) + the misc singletons (`cron/*`, `diff/preview/get`, `memory/*`, `router/*`, `snapshot/*`, `user_question/*`, `mcp/status/list`, …) |

## Tests

- **Unit** (`crates/octoscode-client/tests/client_core.rs`, no socket): registry
  routing; the tolerated-unknown arm; `Client::call` round-trip against a fake
  transport; RPC error and decode error both carrying the method name; a closed
  transport surfacing as `ClientError::Transport`; a delta reaching the store.
- **Live** (`crates/octoscode-client/tests/live_serve.rs`, `#[ignore]`): against
  a real `octos serve a6ea8505` — `profile/local/create` → `session/open` →
  `session/list` → `config/capabilities/list` → `tool/status/list`, every call
  through the production `Client` path. **No model turns.**
- **Headless** (`docs/phase1/core/evidence/`): the module in the desktop shell
  (hidden, `--remote`), showing `conn: Live` + the session count from the store.

## Dependency route (D10a)

`octos-app-transport` / `octos-app-store` come from **git @ pinned rev**
(D10). Our one required change — the generic `Request` — is a
`[patch]` to the local fork branch, recreated idempotently by
`tools/prepare-octosense-fork.sh`. **Drop the `[patch]` when the upstream PR
lands** (`docs/upstream/octosense-generic-request-PR.md`).
