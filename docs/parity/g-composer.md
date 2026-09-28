# Parity map — group `g-composer` (board #1c)

One capability-per-row parity matrix for the 8 web feature dirs of group `g-composer`, cut from
the web oracle (`~/home/oa.noindex/src-web` @ `70a8a1c`) and the native base
(`~/home/oa.noindex/ref/OctoSense/apps/appcard/app` — the octos-app client inside OctoSense).

- Machine-readable: **`docs/parity/g-composer.csv`** — columns exactly
  `feature,group,capability,web_src,web_unit_tests,web_e2e_specs,protocol_methods,native_status,native_src,notes`
  (multi-values separated by `;`; paths prefixed `src-web/`, `appcard/`, `octosense/`).
- Dir order is **smallest first** (`docs/phase0/feature-sizes.csv` src_loc): async 78,
  transcript-export 252, research 399, markdown 617, onboarding 680, session-config 1178,
  fleet 1848, composer 2847.
- `native_status` = `exists` (reachable from a production entry point: `app/src/app/*.rs`,
  `app/src/backend/octos_ui.rs`, `crates/octos-app-transport`; an OctoSense system app or the
  shell is cited where it provides the behaviour), `partial` (says what is missing), or
  `missing`.

## Per-feature counts

| feature | rows | exists | partial | missing |
|---|---:|---:|---:|---:|
| async | 4 | 0 | 1 | 3 |
| transcript-export | 8 | 0 | 2 | 6 |
| research | 6 | 0 | 0 | 6 |
| markdown | 7 | 4 | 2 | 1 |
| onboarding | 6 | 0 | 0 | 6 |
| session-config | 10 | 0 | 1 | 9 |
| fleet | 9 | 0 | 1 | 8 |
| composer | 11 | 0 | 3 | 8 |
| **total** | **61** | **4** | **10** | **47** |

Reading of the totals: **markdown** is the one surface the native base already renders (it reuses
the aichat `streaming-markdown-kit` pipeline); everything else in this group is **missing or
partial** on the native side. The native app today is a single-session chat client
(`app/src/lib.rs`, `app/src/backend/octos_ui.rs`) with a per-message copy button and a tool/task
dock — it has no composer queue, no fleet/peer surface, no profile management and no session
settings pane.

## Top 5 largest gaps

1. **Composer turn control (queue / steer / interrupt-restore).** The web composer is a
   queue-backed controller (`composer/use-turn-controller.ts:191`, `turn-queue.ts:51`,
   `turn-steering.ts`) with FIFO queueing, native `turn/steer` (a core-contract **extension**
   method, `packages/client/src/steer.ts:8`), per-session interrupted-prompt restore, typed
   turn-collision disclosure (`turn-collision.ts:25`) and an OBSERVED-vs-owned turn lease. The
   native side can only `turn/start` and `turn/interrupt`
   (`crates/octos-app-transport/src/proto.rs:133,136`) — no queue, no steer, no restore, no
   collision typing. ~7 missing rows; the single most-used surface in the product.
2. **Session settings pane (`session-config`, 9/10 missing).** No pane, no model-section
   disposition mapping, no approval-policy selector, no sandbox readback, no browser-scoped
   new-session defaults, no status strip, no Show-thinking row, no holder banners. Its protocol
   hops (`permission/profile/set`, `profile/llm/select`, `session/status/read`) are all **absent
   natively** (`docs/protocol-matrix.csv`).
3. **Fleet / peer control (`fleet`, 8/9 missing).** The whole operator surface — status-word
   projection, goal grouping/ordering, inventory∪roster union, and the Start flow that acquires
   a driver seat then emits exactly one `peer/dispatch` — is missing. Methods
   `peer/dispatch`/`peer/control`/`peer/gather`/`session/driver/*` are core-contract
   **extensions** (`packages/client/src/external-driver-meta.ts:6-15`) and have no native caller.
   Native can only `turn/interrupt` and `approval/respond` (`proto.rs:136,139`).
4. **Onboarding / profile provisioning (`onboarding`, 6/6 missing).** The web derives an LLM
   route from the server catalog and runs create-profile → test-provider → upsert-provider →
   open-session (`onboarding/onboarding-submission.ts:27-134`). Native has **no**
   `profile/llm/*` or `profile/local/create` call at all (`login.rs:165-193` only takes a server
   URL + Profile ID), so a native user cannot provision a server from the app.
5. **Seat handover / driver seat (`composer-seat-handover.ts`, 1 missing row + composer context).**
   The §5.2 three-step (this-tab seat release-then-send; foreign holder → resume-chat, never a
   silent send; our-id lease with no proof → wait for expiry) is built on the
   `session/driver/*` extension methods and has no native equivalent. This is the gate that makes
   multi-client sessions safe, so its absence affects every other composer feature.

## Capabilities with NO test at all (web) — a spec should be written first

These rows have **neither** a unit test in the dir **nor** a Playwright spec exercising them:

| feature | capability | web_src |
|---|---|---|
| research | Remove a research lane by key | `src-web/packages/client/src/research.ts:190-206` |
| research | Latest-request-wins / generation guard for list+mutation | `src-web/apps/web/src/features/research/ResearchDialog.tsx:53-112` |
| onboarding | Credential redaction + latest-request-wins across prepare/submit | `src-web/apps/web/src/features/onboarding/use-onboarding.ts:61-183` |

Two more capabilities are covered **only by a Playwright spec, with no unit test in the dir**
(worth a unit test, but not "no test at all"): the `CopyConversationButton` component
(`transcript-export`, `CopyConversationButton.tsx:26-85`; e2e `copy-conversation.spec.ts:68`),
the markdown `CodeBlock` copy + lazy highlighting (`CodeBlock.tsx:27-121`; e2e
`final-reading.spec.ts:76,583`), and `composer/shortcut-suppression.ts:13-51` (e2e
`keyboard-parity.spec.ts:379`).

## Method notes

- Web protocol methods were traced from each feature through `packages/client/src/*` to the
  `CORE_UI_METHODS` constant (`packages/client/src/generated/core-contract.ts`) **and** to the
  AppUI **extension** method tables (`profile/sub_providers/*` → `research-methods.ts:1-5`;
  `profile/llm/*` → `onboarding-methods.ts:4-13`; `peer/*` + `session/driver/*` →
  `external-driver-meta.ts:6-15`; `turn/steer` → `steer.ts:8`; `tool/status/list` +
  `mcp/status/list` → `inventory-methods.ts:3-4`; `onboarding/workspace_list` →
  `onboarding-methods.ts:23`). Extension methods are listed in `protocol_methods` exactly like
  core methods even though they are not in `core-contract.ts`.
- Native citations point at the **consumer** path (`app/src/backend/octos_ui.rs`,
  `crates/octos-app-store/src/state.rs`, the UI that reads the state), **not** the deliberate
  no-op notification list at `octos_ui.rs:899-945`.
- No native `native_src` is claimed for a function reachable only from a test.
