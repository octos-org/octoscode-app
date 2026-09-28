# Behaviour specs — group `spec-a`

Specs for the 11 web capabilities that the parity matrix lists with **no unit test and no Playwright
spec**. Written from the web source (cited `file:line`), never from memory. No native card may claim
these until a spec exists (supervisor 8.8 condition 5).

Machine-readable copy: `docs/specs/spec-a.csv`
(`spec_id,feature,capability,web_src,scenarios,strings,methods,native_test_hint`).

Conventions: **G** = Given, **W** = When, **T** = Then. Strings are quoted verbatim with `file:line`.
"native test hint" says how the scenario can be driven headlessly (fixture server vs none; which `/snap`
assertion proves it). `harness/headless.sh` + `GET /snap` are the driver (see `docs/harness/GUIDE.md`).

---

### SPEC-session-1 — Session runtime scope key

- **Capability:** the durable identity of a Session is `(endpoint, workspaceRoot, profileId, sessionId,
  authorityEpoch)`, serialized as a trimmed JSON array.
- **Web source:** `src-web/apps/web/src/features/session/session-scope.ts:18`.
- **Scenarios**
  - G a scope object; W `sessionRuntimeScopeKey(scope)`; T a `JSON.stringify` of the 5-tuple with each
    string `.trim()`ed and the epoch verbatim (`session-scope.ts:21`).
  - G a field with surrounding whitespace; W `key()`; T whitespace is trimmed, so padded and unpadded
    scopes collide (`session-scope.ts:22`).
  - G a segment containing a `::` delimiter; W `key()`; T **no** collision — a JSON array is used, not a
    `::` join (`session-scope.ts:19-21`).
  - G two scopes differing only in `authorityEpoch`; W `key()` each; T different keys
    (`session-scope.ts:26`).
- **Strings:** "`::` delimiter can collide when any segment contains the delimiter; a JSON array cannot"
  (`session-scope.ts:19-20`); "NEVER credentials" (`session-scope.ts:6`).
- **Methods:** none.
- **Native test hint:** no fixture server — unit-test the pure ported function (assert the JSON 5-tuple
  with trimming and the epoch). Not UI; no `/snap`.

---

### SPEC-session-2 — Connection input contract

- **Capability:** the exact fields a Connect submits to the runtime:
  `endpoint, token, sessionId, profileId, cwd`.
- **Web source:** `src-web/apps/web/src/features/session/connection-lifecycle.ts:1`.
- **Scenarios**
  - G a `SessionConnectionInput`; W it is passed to `runtime.authenticate(input)`; T the runtime receives
    all five fields, with `token` treated as a credential that must never reach a snapshot
    (`lazy-server-runtime.ts:67-69` copies the input but publishes only the empty snapshot).
  - G `token` present; W a snapshot is read; T no credential appears in the snapshot
    (`lazy-server-runtime.ts:68`).
- **Strings:** "Credentials stay only in this pending call and the existing runtime, never snapshots."
  (`lazy-server-runtime.ts:68`).
- **Methods:** none.
- **Native test hint:** no fixture server. Type-level + unit test: assert the ported struct has exactly
  these 5 fields and that the credential field is not serialized into the published state.

---

### SPEC-session-3 — useServerConnection controller

- **Capability:** a stable `LazyServerRuntime` whose admission/connection state is the authority, with
  React only subscribing; the runtime disconnects on unmount.
- **Web source:** `src-web/apps/web/src/features/session/use-server-connection.ts:26`.
- **Scenarios**
  - G a hook mount; W it renders; T it returns `{ runtime, snapshot }` and the **same** runtime instance
    survives re-renders (`runtimeRef`, `use-server-connection.ts:31-38`).
  - G the client factory still loading; W `authenticate` is called; T the snapshot goes `connecting`
    (`lazy-server-runtime.ts:71`) **before** the factory resolves.
  - G unmount; W cleanup runs; T `runtime.disconnect()` is invoked (`use-server-connection.ts:49`).
  - G the latest `onEvent` changes between renders; W an event fires; T the newest sink receives it
    (`eventSinkRef.current`, `:29-30,46`).
- **Strings:** "React is only a subscriber; ActiveSessionRuntime remains the authority."
  (`use-server-connection.ts:25`).
- **Methods:** none.
- **Native test hint:** no fixture server. Port the controller as a non-React object and unit-test:
  same-instance across re-render, `connecting` published before the factory resolves, disconnect on
  teardown, latest-sink event delivery.

---

### SPEC-session-4 — Sidebar catalog rows

- **Capability:** one row per server-attested Session for a workspace (title / lastPrompt / updatedAt /
  activeTurn), newest first, open-on-click and delete; unattested rows stay tab-only.
- **Web source:** `src-web/apps/web/src/features/session/workspace-session-catalog.ts:141`.
- **Scenarios**
  - G capabilities advertise `session/list` **and** feature `session.workspace_cwd.v1`; W
    `supportsWorkspaceSessionCatalog()`; T `true` (`:56-63`); missing either T `false`.
  - G a `session/list` result with a truthy `workspace_root` and `result.profile_id === profileId`; W
    `refresh()`; T state `loaded` with `catalogSessionsFromList` rows (`:199-208`).
  - G a falsy `workspace_root` **or** a profile mismatch; W `refresh()`; T state `unscoped` (no rows
    projected, `:209`).
  - G an id failing `isFullSessionForProfile(entry.id, profileId)`; W `catalogSessionsFromList()`; T that
    row is dropped (`:73`).
  - G rows with `updatedAt`; W `catalogSessionsFromList()`; T sorted newest-first, missing/invalid
    `updatedAt` sorts last (`:87-91`).
  - G a tab-opened ref absent from the catalog; W `mergeWorkspaceSessionRows()`; T it is appended with
    `title: null`, `updatedAt = lastOpenedAt`, recency `= max(catalog, lastOpened)` (`:107-135`).
  - G a prior `loaded`/`error` state and a refresh that throws; W `refresh()`; T status `error` keeps the
    last attested rows; a first-ever failure stays `unscoped` (`:210-222`).
  - G `dispose()`; W `refresh()` after; T no publish (disposed guard, `:178,:228-232`).
- **Strings:** status literals `"unscoped" | "loaded" | "error"` (`:46`).
- **Methods:** `session/list` (params `{ cwd, profile_id }`); response fields `workspace_root`,
  `profile_id`, `sessions[]` with `id, title, last_prompt, updated_at, active_turn`.
- **Native test hint:** fixture server advertising `session/list` + `session.workspace_cwd.v1` returning
  one attested workspace. Drive the session list pane and assert via `/snap`: exactly one row label per
  attested id, newest first, a delete button per non-active row. Then make the fixture reply with a
  foreign `workspace_root` and assert `/snap` shows the tab-only fallback (no server rows).

---

### SPEC-activity-1 — Activity catalog auto-refresh + availability gating

- **Capability:** while Activity is open, read `task/list` for the confirmed session ids (batched by 4)
  and auto-refresh every 10 s; gate availability on the `task/list` method.
- **Web source:** `src-web/apps/web/src/features/activity/use-activity-catalog.ts:43`.
- **Scenarios**
  - G `task/list` not advertised; W the hook runs; T `activityAvailable=false` and the effect returns
    early (no reads) (`:37,47-50`).
  - G `task/list` advertised, `open=true`, client set; W the effect runs; T `activityLoading=true` then a
    full refresh, and a `setTimeout` schedules the next refresh at **10000 ms** (`:53,67`).
  - G `open=false` or `client` null; W the effect runs; T no refresh and the timer is cleared on cleanup
    (`:47-50,71-73`).
  - G a wrong-session reply (`response.session_id !== sessionId`); W `readActivityCatalog()`; T that
    session is counted unavailable, not projected (`:28-32`).
  - G more than 4 ids; W `readActivityCatalog()`; T reads are issued in batches of 4 via `Promise.all`
    (`:23-25`).
  - G some sessions fail; W refresh completes; T error text `"<n> Session task snapshots unavailable"`
    with `n = unavailableSessions.length` (`:63-65`).
  - G duplicate ids; W `readActivityCatalog()`; T collapsed (`[...new Set]`, `:18`).
- **Strings:** `"<n> Session task snapshots unavailable"` (`use-activity-catalog.ts:64`);
  `"Read-only · refreshes every 10 seconds"` (`ActivityNavigator.tsx:177`).
- **Methods:** `task/list` (params `{ session_id }`); `TaskListResult.session_id` must echo the request;
  `TaskListResult.tasks` (`TaskListEntry[]`).
- **Native test hint:** fixture server advertising `task/list` returning N tasks for a confirmed session.
  Open Activity and assert `/snap` shows the task rows + the `"Read-only · refreshes every 10 seconds"`
  footer; advance >10 s and assert a second `task/list` request was made (fixture request counter). With
  `task/list` NOT advertised, assert the `"This server does not advertise task snapshots."` string.

---

### SPEC-activity-2 — Blocked-switch warning

- **Capability:** while a workspace transition blocks switching, opening another Session is prevented and
  a warning is shown; the row action is disabled.
- **Web source:** `src-web/apps/web/src/features/activity/ActivityNavigator.tsx:97`.
- **Scenarios**
  - G `switchBlocked=true`; W the navigator renders; T a `role=status` paragraph with the warning text is
    present (`:97-101`) and every **non-current** row action is disabled (`:131`).
  - G `switchBlocked=true`; W a non-current row action is clicked; T it cannot fire, so no
    `onOpenSession` (`:131,142-146`).
  - G the row IS the active session and `inspectAvailable=false`; W it renders; T disabled with the title
    `"Task details are unavailable on this connection"` (`:131-135`).
  - G `switchBlocked=false` and a non-current row; W its action is clicked; T `onOpenSession(sessionId)`
    then `onClose()` (`:142-146`).
  - G the current-session row and `inspectAvailable`; W clicked; T `onInspectCurrentTask(taskId)` then
    `onClose()` (`:143-145`).
- **Strings:** `"Finish the workspace transition before opening another session."` (`:99`);
  `"Task details are unavailable on this connection"` (`:134`); button labels `"Inspect"`/`"Open session"`
  (`:148`); aria-labels `"Inspect {task}"` / `"Open {session}"` (`:139-140`).
- **Methods:** none (pure presentation over `WorkspaceProductState`).
- **Native test hint:** no fixture server. Mount Activity with `switchBlocked=true` and assert via `/snap`
  that the warning paragraph text is present and the non-current row action is disabled (a `/click` has no
  effect). With `switchBlocked=false`, `/click` the row action and assert a session-open request was issued.

---

### SPEC-workspace-1 — Workspace product state container

- **Capability:** one reducer-owned state (`sessions/files/tokenCost/activity/launch` bookkeeping) with
  fail-closed capability flags.
- **Web source:** `src-web/apps/web/src/features/workspace/model.ts:21`.
- **Scenarios**
  - G the initial state; W `EMPTY_WORKSPACE_PRODUCT` is used; T every capability flag is false and every
    collection empty (`model.ts:40-56`).
  - G `configureCapabilities(caps)`; W called; T `sessionsAvailable=false` and `filesAvailable=false`
    **always** (regardless of advertised methods), `deleteAvailable = supportsMethod(caps, session/delete)`,
    and all activity/token state cleared (`:60-79`).
  - G `observeTokenCost(update)` for the same `sessionId`; W called twice with sparse fields; T
    `mergeTokenCost` keeps prior defined fields and overlays only defined new ones (`:58-69`).
  - G an update for a **different** `sessionId`; W observed; T the old token cost is replaced wholesale
    (`:62`).
  - G `setError(msg)`; W called; T `state.error = msg` only (`:159-160`).
- **Strings:** (none user-visible in the reducer itself).
- **Methods:** `session/delete` (capability gate for `deleteAvailable`).
- **Native test hint:** no fixture server. Port the state container as a plain struct and unit-test:
  `EMPTY` defaults; `configureCapabilities` forcing sessions/files false while gating delete on
  `session/delete`; sparse token-cost merge within a session and wholesale replace across sessions.

---

### SPEC-workspace-2 — Session display label

- **Capability:** a catalog/known row's label is `title.trim() || last_prompt.trim() || id`.
- **Web source:** `src-web/apps/web/src/features/workspace/model.ts:71`.
- **Scenarios**
  - G `title='  Fix the bug  '`; W `sessionLabel()`; T `"Fix the bug"` (trimmed, `model.ts:72`).
  - G `title` empty/whitespace but `last_prompt` set; W `sessionLabel()`; T the trimmed `last_prompt`
    (`:72`).
  - G both empty; W `sessionLabel()`; T `session.id` (raw, never blank, `:72`).
  - G an activity row whose label map lacks the session but a catalog row exists; W
    `buildWorkspaceActivityModel()`; T the label comes from `sessionLabel(session)` (`:104-106`).
  - G a label present in `activitySessionLabels`; W `buildWorkspaceActivityModel()`; T that label wins
    over the catalog title (`:105`).
- **Strings:** (none beyond the label itself).
- **Methods:** none.
- **Native test hint:** no fixture server. Unit-test `sessionLabel` precedence (title → last_prompt → id)
  with empty/whitespace, plus the label-map override inside `buildWorkspaceActivityModel`.

---

### SPEC-workspace-3 — Launch runtime state

- **Capability:** phase ∈ `{idle, resolving, awaiting_choice, opening}` with the requested `cwd` and the
  resolve decision.
- **Web source:** `src-web/apps/web/src/features/workspace/launch-model.ts:3`.
- **Scenarios**
  - G a fresh state; W `EMPTY_LAUNCH_RUNTIME` is used; T `phase='idle'`, `cwd=null`, `decision=null`
    (`launch-model.ts:9-13`).
  - G a resolution is in flight; W state advances; T `phase='resolving'` with `cwd` set.
  - G the server returns choices; W resolved; T `phase='awaiting_choice'` with `decision =
    LaunchResolveResult`.
  - G a choice is confirmed; W opening begins; T `phase='opening'` (the temporary phase that returns to the
    exact candidate on failure — `session/launch-transition.ts:7-10`).
  - G a failed candidate; W recovery; T the phase returns to the prior candidate rather than inventing
    state (`launch-transition.ts:7-10`).
- **Strings:** (none).
- **Methods:** `LaunchResolveResult` (type).
- **Native test hint:** no fixture server. Unit-test the ported enum/struct: only the four phases exist;
  `EMPTY` is idle / cwd null / decision null; transitions set `cwd` and `decision` as described.

---

### SPEC-workspace-4 — Per-session workspace cwd on open

- **Capability:** the connection config carries a `cwd`, and requesting history for a cwd is **refused**
  unless a real server-owned catalog exists.
- **Web source:** `src-web/apps/web/src/features/workspace/use-workspace-product.ts:13`.
- **Scenarios**
  - G the connection config; W read; T it carries a `cwd` string (`:17-19`).
  - G `listWorkspaceSessions('  ')`; W called; T throws `"The Octos server connection is not ready."`
    (trimmed-empty, `:142-145`).
  - G no client; W `listWorkspaceSessions('/w')`; T the same error (`:143-145`).
  - G a client and a non-empty cwd; W `listWorkspaceSessions('/w')`; T throws `"Session history is not
    available for this workspace."` — the rc.9 list result has no scope proof (`:146-149`).
  - G `refresh()`; W called; T `sessionsAvailable=false`, `sessions=[]`, `error=null` (never the legacy
    list, `:82-94`).
  - G `deleteSession(currentSessionId)`; W called; T no-op (guarded, `:98-108`).
  - G `deleteSession(other)` with `session/delete` advertised; W it resolves; T the row is removed and
    `deletingSessionId` cleared (`:115-124`).
  - G `deleteSession(other)` failing; W it rejects; T `error=errorMessage(reason)` and `deletingSessionId`
    cleared (`:125-136`).
- **Strings:** `"The Octos server connection is not ready."` (`:144`); `"Session history is not available
  for this workspace."` (`:149`).
- **Methods:** `session/delete`; **`session/list` is explicitly NOT called** (`refresh` sets
  `sessionsAvailable=false`, `:60-63,82-94`).
- **Native test hint:** fixture server advertising `session/delete`. Unit-test `listWorkspaceSessions`
  refusal (empty cwd, no client, and the always-throw), the refresh fail-closed (no sessions), and
  `deleteSession` no-op on the active session / optimistic remove on success / `errorMessage` on failure.

---

### SPEC-attention-1 — AttentionBridge (settings bridge)

- **Capability:** mounted only after authentication, subscribes to attention, reports settings to the
  shell, and clears them on unmount.
- **Web source:** `src-web/apps/web/src/features/attention/AttentionBridge.tsx:10`.
- **Scenarios**
  - G unauthenticated; W the shell renders; T no `AttentionBridge` mount (`App.tsx:2236` guards on
    `session.authenticated`).
  - G authenticated; W mounted; T it renders `null` and calls `onSettingsChange(settings)` after each
    settings change (`AttentionBridge.tsx:14-17`).
  - G mounted and settings change; W the effect runs; T the shell's `setAttentionSettings` receives the
    new `AttentionSettings` (`App.tsx:2249`).
  - G unmount; W cleanup; T `onSettingsChange(null)` (`AttentionBridge.tsx:18`; clears `App.tsx`
    `attentionSettings`).
  - G a `SurfaceBoundary` around it; W the bridge throws; T the fallback is `null`, so the shell is
    unaffected (`App.tsx:2237`).
- **Strings:** the toggle row it feeds reads `"Desktop notifications"` (`GeneralSettingsContent.tsx:220`).
- **Methods:** none (the bridge consumes `AttentionSettings`; the notification consent itself is the
  desktop-notifications unit, out of this spec's scope).
- **Native test hint:** fixture server (auth fixture). Drive login then assert via `/snap`: after
  authentication the desktop-notifications toggle is present in Settings; before authentication it is
  absent. `/click` the toggle and assert the OS-notification permission request is gated behind explicit
  opt-in.

---

## Branches I could not fully explain

- **`SPEC-attention-1` methods.** The cited `AttentionBridge.tsx` itself issues no RPC call — it is a pure
  bridge over `useAttention`; the notification machinery and any settings persistence live in
  `attention/desktop-notifications.ts` (browser `Notification`/`localStorage`), which is not a protocol
  method. Recorded as "none" rather than inventing a method.
- **`SPEC-workspace-4` `listWorkspaceSessions`.** The function always throws by deliberate design
  (`:146-149`); there is no reachable success path in the pinned tree, so the spec asserts the refusal
  rather than a happy path that does not exist in code.
