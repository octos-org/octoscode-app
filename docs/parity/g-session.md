# Parity map — group `g-session`: feature `session`

Source: `src-web/apps/web/src/features/session/` (27 src, 40 unit-test files, 11.5k LOC, 40 tests per `docs/phase0/feature-sizes.csv`).
Native target: `appcard/` = `ref/OctoSense/apps/appcard/app` (octos-app client) — `app/src/*`, `crates/octos-app-transport`, `crates/octos-app-store`.

## Summary per feature

`session` — **59 capabilities** mapped. native_status: **3 exists**, **24 partial**, **32 missing**.

| group | capabilities | exists | partial | missing |
|---|---|---|---|---|
| session:store-hydrate | 22 | 0 | 10 | 12 |
| session:list-sidebar | 19 | 2 | 10 | 7 |
| session:links-resume | 18 | 1 | 4 | 13 |

## Top 5 largest gaps

1. **No external-driver / peer-control channel at all** — The whole `session:links-resume` protocol surface — `session/driver/get`, `peer/dispatch`, `peer/control`, `session/driver/{acquire,renew,release}` — has **13 missing** rows and **zero** native code. Web has a full typed decoder + disclosure walk (`external-driver.ts`, `driver-discovery.ts`, `use-octos-session.ts:724/911`); native has no `peer/*` or `session/driver/*` string anywhere (`appcard/crates/octos-app-transport/src/proto.rs:110-165` covers only open/hydrate/list/turn/approval/diff/task-output).

2. **No per-record Session isolation / background-turn retention** — `session:store-hydrate` is **12 missing, 10 partial, 0 exists**. Web runs one record per scope with isolated FIFOs, a background-turn manager (MAX 8 transports), attention feed and recovery surfacing (`session-record-manager.ts:223`, `background-turn-manager.ts:127`). Native `AppState` holds a single `current_session` pointer (`crates/octos-app-store/src/state.rs:41`) and one `SessionMap`.

3. **Sidebar is REST-backed, not `session/list` over WS** — Native `hydrate_from_ws_value` exists (`app/src/app/sessions.rs:97`) but the sidebar hydrates through `RestClient::list_sessions` (`app/src/app/sessions.rs:121`), a REST route the server retired. Web reads the catalog through `session/list {cwd, profile_id}` with stale-while-revalidate (`workspace-session-catalog.ts:154/177`).

4. **No durable unsent-draft persistence** — Web persists per-principal drafts in `localStorage` with a bounded cache and eviction (`durable-session-drafts.ts:37`, `session-draft-cache.ts:35`); native has none.

5. **No user-question (ask) surface** — Web has a full question ledger + `user_question/respond` (`session-interaction-ledger.ts:400`). Native folds `user_question/requested` into an **info toast only** (`crates/octos-app-store/src/state.rs:404`) and has no `user_question/respond` method.

## Capabilities with NO test at all in the web (4)

These need a spec written before they can be trusted as pinned (all are verified by reading only):

- `session:store-hydrate` — session runtime scope key (endpoint/workspace/profile/session/authority epoch) (`src-web/apps/web/src/features/session/session-scope.ts:18`)
- `session:store-hydrate` — connection input: endpoint/token/sessionId/profileId/cwd (`src-web/apps/web/src/features/session/connection-lifecycle.ts:1`)
- `session:store-hydrate` — useServerConnection controller (`src-web/apps/web/src/features/session/use-server-connection.ts:26`)
- `session:list-sidebar` — sidebar renders one row per Session (click to select, x to delete) (`src-web/apps/web/src/features/session/workspace-session-catalog.ts:141`)

