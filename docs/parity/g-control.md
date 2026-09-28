# Parity map — group `g-control`

Scope: 8 web feature dirs in `src-web/apps/web/src/features/` — `commands control inventory media peers
questions reasoning workspace-create`. Companion machine-readable data: `docs/parity/g-control.csv`
(43 rows / 8 features). Path prefixes: `src-web/` = `/Users/yuechen/home/oa.noindex/src-web/`,
`appcard/` = `ref/OctoSense/apps/appcard/app/`, `octosense/` = `ref/OctoSense/`.

Method: one row per user-visible behaviour, cited `file:line`; unit tests from `*.test.ts[x]` in the dir,
Playwright specs from `docs/phase0/web-e2e-specs.txt`; protocol methods traced to `CORE_UI_METHODS`
(`packages/client/src/generated/core-contract.ts`, octos @ 4231669) or the AppUI extension modules
(`packages/client/src/*-methods.ts`, `peer-protocol.ts`, `external-driver-meta.ts`); native status checked
against `appcard/app/src/app/*.rs`, `appcard/app/src/backend/octos_ui.rs`,
`appcard/app/crates/octos-app-transport`, plus OctoSense system apps and `octosense/crates/shell`.

## Per-feature summary

| feature | capabilities | exists | partial | missing | caps with no test |
|---|---:|---:|---:|---:|---:|
| reasoning | 4 | 0 | 2 | 2 | 0 |
| inventory | 3 | 0 | 0 | 3 | 3 |
| questions | 7 | 0 | 1 | 6 | 0 |
| media | 4 | 0 | 0 | 4 | 0 |
| commands | 7 | 0 | 1 | 6 | 0 |
| workspace-create | 6 | 0 | 3 | 3 | 0 |
| control | 6 | 0 | 0 | 6 | 0 |
| peers | 6 | 0 | 0 | 6 | 0 |
| **total** | **43** | **0** | **7** | **36** | **3** |

No capability in this group reaches `exists` (a native path reachable from a production entry point).
The seven `partial` rows are the only places the native client touches the same behaviour at all.

## Top 5 largest gaps

1. **`peers` — no peer surface at all (6/6 missing).** The web runs a full authority-fenced peer
   lifecycle (`features/peers/peer-manager.ts:50`), blackboard gather/synthesis (`gather.ts:13`), lazy
   authority loading (`lazy-peer-manager.ts:32`), roster counts and a 4-state activity axis
   (`peer-roster.ts:40`). Native decodes the events and throws them away: `PeerStaged`/`PeerClosed` are
   explicit no-ops at `appcard/app/crates/octos-app-store/src/state.rs:511` ("octos-app has no peer pane").
2. **`control` — no Fleet / peer-control surface (6/6 missing).** `Fleet Start = acquire(CAS) → proof →
   dispatch once` (`features/control/fleet-actions.ts:58`), peer control/dispatch commands
   (`peer-control-commands.ts:75`, `peer-dispatch-commands.ts:84`), the lane picker sourced only from
   advertised lanes (`peer-lane-source.ts:46`) and both roster panels have no native counterpart
   (`peer/dispatch`, `peer/control`, `session/driver/acquire` are unused in appcard).
3. **`media` — image attachments are silently dropped (4/4 missing).** The web stages up to 4 images /
   20 MiB per turn and consumes them only at the accepted queue-enqueue boundary
   (`features/media/attachment-drafts.ts:6,19`). The native turn always sends `media: Vec::new()`
   (`appcard/app/src/backend/octos_ui.rs:1064`), so an app turn can never carry an image.
4. **`commands` — no native command surface (1 partial / 6 missing).** The web ships a 47-entry
   capability-gated command registry (`features/commands/registry.ts:87`), a parser
   (`registry.ts:658`), keyboard-parity shortcuts (`registry.ts:611`) and a palette
   (`CommandPalette.tsx:14`). Native has none; only the task dock's pagination is noted as an open gap
   (`appcard/app/src/app/task_dock.rs:254`).
5. **`questions` — a blocked agent becomes a toast (1 partial / 6 missing).** The web renders a
   full takeover card with single/multi-select, free text, arrow-key nav and a consequence-labelled
   submit (`features/questions/UserQuestionPanel.tsx:70`). Native's consumer arm only pushes
   `"Agent asks: {title}"` as a toast (`appcard/app/crates/octos-app-store/src/state.rs:404`); there is
   no answer-composition UI and no `user_question/respond` send path.

## Capabilities with NO test at all in the web (need a spec written first)

All three are in `inventory` — the whole dir is untested (0 test files):

- Read-only runtime tools inventory — `src-web/apps/web/src/features/inventory/InventoryDialog.tsx:41`
- MCP server status inventory — `src-web/apps/web/src/features/inventory/InventoryDialog.tsx:51`
- Client-side search filter over tools and servers — `src-web/apps/web/src/features/inventory/InventoryDialog.tsx:74`

Every other row has at least one unit test; 26 of 43 rows additionally name a Playwright spec.

## Notes on `partial` rows

- `reasoning` effort is binary in native: `set_thinking` → `reasoning_effort: High` only
  (`appcard/app/src/backend/octos_ui.rs:1069,1079`); no `low`/`medium`/`max`, no Profile-default clear.
- `reasoning` visibility: native folds `ReasoningDelta` into `ephemeral.thinking_text`
  (`state.rs:398`) but deliberately never surfaces it (`appcard/app/src/lib.rs:5394`).
- `workspace-create`: native resolves a workspace cwd at `session/open`
  (`octos_ui.rs:975,986`) but has no picker, create form, first-entry projection or folder browser.
- `commands`: only the cold local-report shape has any echo (`task_dock.rs:254`), and that is an open
  gap note, not an implementation.
