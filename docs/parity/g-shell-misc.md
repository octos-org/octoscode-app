# Parity map — group `g-shell-misc`: features `activity`, `error`, `workspace`

Source: `src-web/apps/web/src/features/{activity,error,workspace}/`. Native target: `appcard/` = `ref/OctoSense/apps/appcard/app`
(octos-app client) — `app/src/*`, `crates/octos-app-transport`, `crates/octos-app-store`. Ordered smallest dir first.

## Summary per feature

**28 capabilities** total. native_status: **0 exists**, **8 partial**, **20 missing**.

| feature | LOC | capabilities | exists | partial | missing |
|---|---|---|---|---|---|
| error | 223 | 7 | 0 | 1 | 6 |
| activity | 322 | 9 | 0 | 2 | 7 |
| workspace | 572 | 12 | 0 | 5 | 7 |

## Top 5 largest gaps

1. **No native crash/error boundary at all (`error`, 6/7 missing)** — Web wraps the whole app in `FatalErrorBoundary` with a redacted diagnostic + reload/copy affordances and ~11 per-surface `SurfaceBoundary` mounts (`FatalErrorBoundary.tsx:13`, `SurfaceBoundary.tsx:16`;  `e2e/theme.spec.ts:150`, `e2e/surface-recovery.spec.ts:33`). Native has no boundary: a Rust panic aborts the process (`app/src/lib.rs:10748` calls this the "fatal class"); the only error surface is a partial `ToastQueue` (`crates/octos-app-store/src/toasts.rs:28`).

2. **No cross-session activity scan (`activity`, 7/9 missing)** — The whole feature reads `task/list` per confirmed session with bounded fan-out and a searchable model (`catalog.ts:12/23`, `workspace/model.ts:93`; `e2e/command-surface.spec.ts:180`). Native has **no `task/list` method** anywhere and only a single-session `TaskDock` projection (`app/src/app/task_dock.rs:117`).

3. **No `launch/resolve` profile choice (`workspace`)** — Web resolves a workspace launch and, on `cross_profile`, shows `LaunchDecisionPanel` (`LaunchDecisionPanel.tsx:19`, `launch-model.ts:3`; `e2e/product.spec.ts:1733/1753`). Native has no `launch/resolve`; it opens a pre-bound `SessionKey` (`app/src/lib.rs:8613`).

4. **No recent-workspaces navigation cache (`workspace`)** — Web persists a bounded (20) v2 recents cache with v1 migration, remember/dedupe and honest clear (`workspace-recents.ts:19/38/65`; `e2e/product.spec.ts:1260`). Native has no equivalent (localStorage/sidebar recents are absent).

5. **Task/activity is single-session only; per-session cwd is coarse** — Native `TaskDock`/`CodingScreen` project tasks and tool calls for the **current** session only (`task_dock.rs:117`, `coding.rs:747`), and the transport requests a per-session cwd but has no WS `session/list` consumer (`crates/octos-app-transport/src/lib.rs:108`; sidebar still uses REST `list_sessions`, `app/src/app/sessions.rs:121`).

## Capabilities with NO test at all in the web (9)

Verified by reading only; these need a spec written before they can be trusted as pinned:

- `error` — crash screen actions: reload app / copy diagnostics / report-crash link (`src-web/apps/web/src/features/error/FatalErrorBoundary.tsx:38`)
- `error` — unavailable-surface fallback (modal vs inline section, aria-labelledby) (`src-web/apps/web/src/features/error/SurfaceBoundary.tsx:44`)
- `error` — error toasts (transient, bounded queue) (`src-web/apps/web/src/features/error/SurfaceBoundary.tsx:63`)
- `activity` — activity: catalog auto-refresh while open (10s) + availability gating on task/list (`src-web/apps/web/src/features/activity/use-activity-catalog.ts:43`)
- `activity` — activity: blocked-switch warning before opening another session (`src-web/apps/web/src/features/activity/ActivityNavigator.tsx:97`)
- `workspace` — workspace: product state container (sessions/activity/token-cost/launch) (`src-web/apps/web/src/features/workspace/model.ts:21`)
- `workspace` — workspace: session display label from a catalog row (`src-web/apps/web/src/features/workspace/model.ts:71`)
- `workspace` — workspace: launch runtime state (idle/resolving/awaiting_choice/opening) (`src-web/apps/web/src/features/workspace/launch-model.ts:3`)
- `workspace` — workspace: request a per-session workspace cwd on open (`src-web/apps/web/src/features/workspace/use-workspace-product.ts:13`)

