# Parity map — group `g-settings`

The two `src-web/apps/web/src/features/` dirs **product-settings** and **shell** mapped to the
octos-app native client (`appcard/` = `ref/OctoSense/apps/appcard/app`). Machine-readable rows:
`docs/parity/g-settings.csv` (schema identical to `g-timeline.csv`).

Method: capability = one user-visible behaviour cited at its implementing web file:line; the unit
test / Playwright spec that pins it cited by test name; protocol methods traced from the feature
through `src-web/packages/client/src/*`; native status from the production entry points
(`appcard/app/src/app/*.rs`, `appcard/app/src/backend/octos_ui.rs`, `appcard/crates/octos-app-transport`),
plus any OctoSense system app / shell provider.

## Two evidence rules applied (LESSONS.md "Parity-map pitfalls")

- **Native consumer arms, not the no-op list.** `appcard/app/src/backend/octos_ui.rs:~899-951` is a
  deliberate no-op list; every native notification claim here cites the consumer in
  `appcard/crates/octos-app-store/src/state.rs` (`apply_protocol`) or the UI that reads the state
  (e.g. the session list path `octos_ui.rs:544 hydrate_from_ws_value` → `sessions.rs:97`).
- **AppUI extension methods count.** Unlike `g-timeline`, **these two dirs do reach extension methods**:
  `profile/llm/*` (`onboarding-methods.ts:6-12`), `profile/skills/*` (`skill-methods.ts:2-3`),
  `server/shutdown` (`server-methods.ts:7`), `peer/control` (`external-driver-meta.ts:12`). They are in
  `protocol_methods` exactly like core methods. Only `permission/profile/list` and `session/*` are core.
- CSV written exclusively with `csv.DictWriter`.

## Per-feature summary

| feature | caps | exists | partial | missing |
|---|---:|---:|---:|---:|
| shell | 11 | 0 | 2 | 9 |
| product-settings | 12 | 0 | 1 | 11 |
| **total** | **23** | **0** | **3** | **20** |

`exists` = 0: appcard has a flat session list and a sign-out, but reaches full parity with neither dir.

## Top 5 largest gaps

1. **product-settings — model provider management** (12 caps, 11 missing). The whole
   read/catalog/test/upsert/delete/fetch_models provider workflow (`ModelManagementSection.tsx:1003`)
   is absent, as is the projection and the Profile-mutation lease. All six methods are AppUI
   extensions (`onboarding-methods.ts:6-12`).
2. **shell — workspace tree + grouped/flat views** (11 caps, 9 missing). appcard's sidebar is one
   flat list (`appcard/app/src/app/sessions.rs:217` `sessions_for_sidebar`); the web groups by
   workspace with expand/collapse, add-workspace, view/order modes, and search.
3. **shell — sidebar footer entries** (Fleet entry, theme toggle System/Light/Dark, Settings). Native's
   `settings_button` is a DSL declaration with **no handler** (`appcard/app/src/lib.rs:3988`); theme is
   not a user toggle natively (card-intent only, `appcard/app/src/lib.rs:327`).
4. **product-settings — General settings pane**. There is no native settings surface at all (the
   connection/disconnect/forget/diagnostics/conversation-link/stop-server rows have no counterpart);
   native only wipes the keychain on sign-out (`appcard/app/src/lib.rs:4010`).
5. **shell — per-session status projection + relative time**. Native renders a binary streaming dot
   (`appcard/app/src/app/sessions.rs:239`) where the web projects running/waiting/completed/failed/
   interrupted labels plus relative-time strings (`shell/relative-time.ts:1`).

## Capabilities with NO test at all in the web

None — every capability row has at least one of `web_unit_tests` / `web_e2e_specs`.

## Protocol methods reached (union)

Core: `session/list`; `session/open`; `session/hydrate`; `permission/profile/list`.
AppUI extensions: `profile/llm/list`; `profile/llm/catalog`; `profile/llm/test`; `profile/llm/upsert`;
`profile/llm/delete`; `profile/llm/fetch_models`; `profile/skills/list`; `profile/skills/registry/search`;
`server/shutdown`; `peer/control`.
