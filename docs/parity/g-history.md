# Parity map — group `g-history` (board #1i)

One capability-per-row parity matrix for the 2 web feature dirs of group `g-history`, cut from the
web oracle (`~/home/oa.noindex/src-web` @ `70a8a1c`) and the native base
(`~/home/oa.noindex/ref/OctoSense/apps/appcard/app` — the octos-app client inside OctoSense).

- Machine-readable: **`docs/parity/g-history.csv`** — columns exactly
  `feature,group,capability,web_src,web_unit_tests,web_e2e_specs,protocol_methods,native_status,native_src,notes`
  (multi-values separated by `;`; paths prefixed `src-web/`, `appcard/`, `octosense/`).
- Dir order is **smallest first** (`docs/phase0/feature-sizes.csv` src_loc): history 894, connection 1340.
- `native_status` = `exists` (reachable from a production entry point: `app/src/app/*.rs`,
  `app/src/backend/octos_ui.rs`, `crates/octos-app-transport`; an OctoSense system app or the shell is
  cited where it provides the behaviour), `partial` (says what is missing), or `missing`.

## Per-feature counts

| feature | rows | exists | partial | missing |
|---|---:|---:|---:|---:|
| history | 11 | 0 | 1 | 10 |
| connection | 13 | 0 | 4 | 9 |
| **total** | **24** | **0** | **5** | **19** |

Reading of the totals: **nothing in this group exists whole natively.** The only `partial`s are the
shared transport primitives the native app already owns — `session/hydrate` (native folds a hydrate
reply to refill the chat, `appcard/app/src/backend/octos_ui.rs:332`), `session/open`
(`appcard/app/crates/octos-app-transport/src/proto.rs:123`), and the login/keychain server-config
store (`appcard/app/src/app/login.rs:248-317`, `crates/octos-app-store/src/keychain.rs:96-186`).
No conversation-history surface (undo/rewind/fork/checkpoints) and no pairing path exist at all.

## Top 5 largest gaps

1. **Conversation history mutations (`history`, 10/11 missing).** The whole checkpoint surface —
   `checkpoints.ts:15` thread grouping, `checkpoints.ts:59` stale-index recompute,
   `history-binding.ts:53` per-mode capability gating, `history-binding.ts:97` blocked-reason lease,
   `history-binding.ts:141` canonical hydrate read, `history-binding.ts:156` mutation lease, and the
   `history-coordinator.ts` undo/rewind/fork flows — has **no native counterpart**. The protocol hops
   `session/rollback`, `session/fork`, `snapshot/list`, `snapshot/restore` are all **absent natively**
   (`docs/protocol-matrix.csv`). Native calls `session/hydrate` only to refill the chat.
2. **Pairing link (`connection`, 4 rows missing).** The one-use-code flow — read from the URL, strip
   before first render, exchange via one unauthenticated `POST <origin>/pair/claim`
   (`pairing.ts:165-205`), the loopback rule (`pairing.ts:137-163`), and discovery via `GET
   /pair/info` (`pairing.ts:207-243`) — has **zero** native presence (0 `pair` hits outside
   unrelated `chunks(2)` loops). Native uses an email OTP flow instead
   (`crates/octos-app-store/src/auth.rs:9-17`).
3. **Connect-failure classification + copy (`connection`, 2 rows).** §5.1's
   unreachable / rejected-token / origin-not-allowed classifier with its exact messages and the
   rejected-token focus-and-keep-field behaviour (`connect-failure.ts:16-72`,
   `ConnectionPanel.tsx:76-90`) is missing; native surfaces the raw server error only.
4. **Tab-scoped draft/credential envelope (`connection`, 3 rows).** The web splits durable origin
   (localStorage) from tab-scoped token + session metadata + principal-bound composer drafts with
   51-entry capacity (`preferences.ts:11-49,80-143,224-283`). Native stores the token in the OS
   keychain per host+profile (`keychain.rs:96,163`) — arguably stronger for the credential, but there
   is no per-tab principal envelope, no draft cache, and no Forget confirm dialog
   (`LeaveConnectionDialog.tsx`).
5. **Endpoint validation before connect (`connection`, 1 row).** `validation.ts:2-17` validates the
   scheme/URL and refuses credentials/query/fragment **before** opening a socket or remembering an
   address. Native's `login.rs` takes a free-text server URL (and a `base_url|profile_id|token`
   provisioning string, `login.rs:321-328`) with no such validation.

## Capabilities with NO test at all (web) — a spec should be written first

These rows have **neither** a unit test in the dir **nor** a Playwright spec exercising them:

*(none — every capability in this group is pinned by at least one web unit test or Playwright spec.)*

Three capabilities are covered **only by a Playwright spec, with no unit test in the dir** (worth a
unit test, but not "no test at all"):

| feature | capability | web_src | e2e |
|---|---|---|---|
| history | History dialog (modal, pickers, consequence copy) | `HistoryDialog.tsx:20-262` | `command-surface.spec.ts:269,368,426` |
| connection | Connect screen (origin + token, live validation) | `ConnectionPanel.tsx:50-374` | `onboarding.spec.ts:18,47` |
| connection | Leave Connection confirm dialog | `LeaveConnectionDialog.tsx:1-63` | `connection-storage.spec.ts:201,267` |

## Method notes

- Web protocol methods were traced from each feature through `packages/client/src/*` to the
  `CORE_UI_METHODS` constant (`packages/client/src/generated/core-contract.ts`) **and** to the AppUI
  **extension** table: `snapshot/list` + `snapshot/restore` (`packages/client/src/history.ts:12-15`).
  Both are listed in `protocol_methods` exactly like core methods.
- Native citations point at the **consumer** path (`octos_ui.rs:332` HydrateSession,
  `lib.rs:8547` folding the reply, `proto.rs:123/156`), **not** the deliberate no-op notification list
  at `octos_ui.rs:899-945`.
- No native `native_src` is claimed for a function reachable only from a test.
