# Parity matrix — group `g-connection` (final scope: `approval`)

Card #1d, branch `task/1d`. Per the 2026-09-28 2nd addendum on
`.octos/OUTER_LOOP_REVIEW.md`, this group was reduced to the **approval** dir only;
`product-settings`/`shell` → #1h (p0-map-b), `connection`/`history` → #1i (p0-map-c),
`activity`/`error`/`workspace` → #1j (p0-map-f). Machine-readable companion:
`docs/parity/g-connection.csv` (7 rows, written with `csv.DictWriter`).

## Feature summary

| feature | capabilities | exists | partial | missing | spec-needed (no web test at all) |
|---|---|---|---|---|---|
| approval | 7 | 4 | 2 | 1 | 1 |

Per-capability: takeover card (`exists`), decide w/ scope (`exists`), typed command
details (`exists`), lifecycle notifications (`exists`), Y/N/S/D keyboard shortcuts
(`partial`), diff review render (`partial`), Alt+A reveal (`missing`).

## Top gaps (ranked; only 3 gap rows exist in the final scope, so fewer than 5)

1. **Alt+A reveal of the pending approval panel — `missing`.** Web:
   `src-web/apps/web/src/app/App.tsx:1051-1062`, pinned by
   `src-web/e2e/keyboard-parity.spec.ts:335` ("Alt+A focuses the pending approval panel
   and announces when none waits"). Native: no hit for the `show-approval` shortcut in
   `appcard/app/src/app/approvals.rs` / `app/src/app/mod.rs`.
2. **Y/N/S/D approval shortcuts with modifier + IME guard — `partial`.** Web:
   `src-web/apps/web/src/features/approval/ApprovalPanel.tsx:33-56`; e2e
   `src-web/e2e/final-input.spec.ts:28`. Native has button-driven decisions only
   (`appcard/app/src/app/approvals.rs:86-91`); no key-equivalent handlers.
3. **Deep diff review from an approval — `partial`.** Transport method is wired
   (`appcard/crates/octos-app-transport/src/proto.rs:142` `diff/preview/get`), but the
   UI stops at summary/op labels: `TODO(W05.diff)` at
   `appcard/app/src/app/approvals.rs:430-432` (Deep DiffView is M3 work). Web renders a
   full review surface above the approval (`src-web/e2e/modal-stack.spec.ts:23`,
   `src-web/e2e/diff-review.spec.ts:82`).

## Capabilities with no web test at all

- **approval lifecycle notifications** (`approval/decided`, `approval/auto_resolved`,
  `approval/cancelled` → toast + list updates). Web src:
  `src-web/packages/client/src/interaction.ts:82-84`. No `*.test.*` in the approval dir
  and no e2e spec found (grep over `src-web/e2e/` + `e2e-live/` for `auto_resolved`:
  no hits). A web spec must be written first; the native side is already the stronger
  half (`appcard/crates/octos-app-store/src/state.rs:342-354` auto-resolve toast,
  `:369` decided, `:380` cancelled).

## Honesty notes

- Every native citation is a **consumer** arm in `octos-app-store/src/state.rs` or UI
  code reading `APP_STATE` — never the deliberate no-op list in
  `app/src/backend/octos_ui.rs:~899-945` (LESSONS 2026-09-28).
- No AppUI extension methods were needed: all approval rows use core-contract methods
  (`approval/requested`, `approval/respond`, `approval/decided`,
  `approval/auto_resolved`, `approval/cancelled`, `diff/preview/get`).
- `web_unit_tests` is empty on all 7 rows: the approval dir contains no
  `*.test.tsx` files (only `ApprovalPanel.tsx`), matching
  `docs/phase0/feature-sizes.csv` (`approval,1,0,143`). Coverage is e2e-only.
