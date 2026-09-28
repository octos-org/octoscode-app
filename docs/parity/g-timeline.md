# Parity map — group `g-timeline`

Seven feature dirs of `src-web/apps/web/src/features/` mapped to the octos-app native client
(`appcard/` = `ref/OctoSense/apps/appcard/app`). Machine-readable rows: `docs/parity/g-timeline.csv`
(columns: `feature,group,capability,web_src,web_unit_tests,web_e2e_specs,protocol_methods,native_status,native_src,notes`).

Method: capability = one user-visible behaviour, cited at its implementing web file:line; the unit
test / Playwright spec that pins it cited by test name; protocol methods traced from the feature through
`src-web/packages/client/src/*` to `CORE_UI_METHODS`; native status from the production entry points
(`appcard/app/src/app/*.rs`, `appcard/app/src/backend/octos_ui.rs`, `appcard/crates/octos-app-transport`),
plus any OctoSense system app / shell provider.

## Two evidence rules applied (from LESSONS.md "Parity-map pitfalls")

- **Native consumer arms, not the no-op list.** `octos_ui.rs:~899-951` is a deliberate no-op list
  ("drained into APP_STATE via `fold_into_store`") — citing it proves nothing. Every native
  notification claim here cites the **consumer** in `appcard/crates/octos-app-store/src/state.rs`
  (`apply_protocol`), e.g. `plan/updated` → `state.rs:515` (an explicit no-op → missing), or the UI
  that reads that state. This is why `PlanUpdated` is `missing` despite appearing in `octos_ui.rs`.
- **AppUI extension methods count too.** The web also calls RPC methods outside `core-contract.ts`
  (~36 of them). I checked whether the 7 dirs reach any of them: they do **not**
  (`grep -rnE "octoscode-client/(steer|history|context-state|peer-protocol|external-driver|inventory|onboarding|research|server|skill)" <7 dirs>` → empty). So every `protocol_methods`
  cell here is a `CORE_UI_METHODS` value; no extension method is dropped, there simply is none in scope.

## Per-feature summary

| feature | caps | exists | partial | missing |
|---|---:|---:|---:|---:|
| session-links | 6 | 0 | 0 | 6 |
| attention | 6 | 0 | 0 | 6 |
| inspection | 6 | 0 | 0 | 6 |
| resume | 6 | 0 | 1 | 5 |
| preferences | 6 | 0 | 0 | 6 |
| supervision | 8 | 0 | 3 | 5 |
| timeline | 12 | 0 | 5 | 7 |
| **total** | **50** | **0** | **9** | **41** |

`exists` (reachable from a production entry point) count is **0**: appcard has a partial *substrate*
for resume, supervision and timeline (transport methods + store reducers) but no feature dir reaches
full behavioural parity with any of the seven.

## Top 5 largest gaps

1. **timeline — reasoning/thinking transcript fold** (`Timeline`/`ThinkingDisclosure`, 12 caps, 7 missing).
   appcard deliberately does **not** surface reasoning in the transcript
   (`appcard/app/src/lib.rs:5394` "Reasoning/thinking is intentionally NOT surfaced"); `ReasoningDelta`
   is only accumulated (`appcard/crates/octos-app-store/src/state.rs:398`). The web renders a folded
   per-block disclosure with a duration/word summary. Also missing inline: tool-call disclosures
   (native uses a separate TaskDock pane, `appcard/app/src/app/task_dock.rs:117`), fold memory
   (`Expand all`/`Collapse all`), system notices in-transcript, in-transcript attachments.
2. **attention — desktop notifications + tab-count attention** (6 caps, all missing).
   The whole `AttentionTracker` (unread count, tab-title `(n)`, foreground terminal evidence) and the
   `Notification` consent model have no native counterpart. Native pushes `Warning`/`UserQuestion` to
   transient toasts instead (`appcard/crates/octos-app-store/src/state.rs:284,:404`).
3. **inspection — thread / turn / remembered-scope inspectors** (6 caps, all missing).
   `thread/graph/get`, `turn/state/get`, `approval/scopes/list` are absent from the native transport
   (`appcard/crates/octos-app-transport/src/lib.rs:125-165`); `approvals.rs` only *sends* a scope on
   respond (`appcard/app/src/app/approvals.rs:503`), it never reads remembered scopes.
4. **preferences — display palettes / language / vim mode** (6 caps, all missing).
   appcard has no user-facing display-settings surface: theme is card-intent based
   (`appcard/app/src/lib.rs:327` `detect_theme`) and locale is read from the environment only
   (`appcard/app/src/app/l0_card.rs:939` `publish_locale`), with no in-app language choice and no zh UI catalog.
   (Browser-only storage key `octoscode.web.display.v1`, `preferences/model.ts:1`.)
5. **session-links — saved conversation links** (6 caps, all missing).
   No conversation-link copy, no saved-link parse/URL build, no untrusted-bookmark confirm panel.
   Opening the link reaches `session/open` + `session/hydrate`, which the transport has
   (`appcard/crates/octos-app-transport/src/lib.rs:127,142`), but the browser surface does not exist.

## Capabilities with NO test at all in the web

These need a spec written before they can be used as acceptance evidence:

| feature | capability | web_src |
|---|---|---|
| attention | AttentionBridge loads after authentication and reports attention settings to the shell | `src-web/apps/web/src/features/attention/AttentionBridge.tsx:10` |

`AttentionBridge.tsx:10` is covered by neither a unit test in `attention/` nor a Playwright spec. Every
other capability has at least one of the two (see the CSV columns `web_unit_tests` / `web_e2e_specs`).

## Protocol methods reached (union)

`session/open`; `session/hydrate`; `session/list`; `thread/graph/get`; `turn/state/get`;
`approval/scopes/list`; `task/list`; `task/cancel`; `task/output/read`; `task/artifact/list`;
`task/artifact/read`; `session/status/read`; `plan/updated`; `task/updated`; `task/output/delta`;
`turn/started`; `turn/completed`; `turn/error`; `message/delta`; `message/reasoning_delta`;
`tool/started`; `tool/progress`; `tool/completed`; `projection/envelope`; `file/attached`; `warning`.
