# Parity map — group `g-autonomy2`

Scope: 1 web feature dir in `src-web/apps/web/src/features/` — `autonomy` (12 src, 7 test files, 2959 LOC).
Companion machine-readable data: `docs/parity/g-autonomy2.csv` (26 rows / 1 feature).
Path prefixes: `src-web/` = `/Users/yuechen/home/oa.noindex/src-web/`,
`appcard/` = `ref/OctoSense/apps/appcard/app/`, `octosense/` = `ref/OctoSense/`.

Method: one row per user-visible behaviour, cited `file:line`; unit tests from `*.test.ts[x]` in the dir;
Playwright specs from `docs/phase0/web-e2e-specs.txt`; protocol methods traced through
`packages/client/src/autonomy.ts` to `CORE_UI_METHODS` + `AUTONOMY_NOTIFICATION_METHODS`; native status read
from `appcard/app` (+ OctoSense `apps/*`, `crates/shell`), citing the **consumer** arm in
`crates/octos-app-store/src/state.rs`, never the `octos_ui.rs:~899-945` no-op list.

## Per-feature summary

| feature | capabilities | exists | partial | missing | caps with no test |
|---|---:|---:|---:|---:|---:|
| autonomy | 26 | 0 | 0 | 26 | 0 |

**Every capability in this dir is `missing` in the native octos-app client.** There is no goal, loop,
monitor or agent *state*, no read/write RPC, and no UI in `appcard`. The transport decodes the eight
`AUTONOMY_NOTIFICATION_METHODS` and the store's consumer arms for all of them are **explicit no-ops**
(`appcard/app/crates/octos-app-store/src/state.rs:450-466`), commented "no app surface". `docs/protocol-matrix.csv`
independently records the same: every `session/goal/*`, `loop/*`, `monitor/*`, `agent/*` **request** is
`absent` in native, and every matching notification is `decoded-only`.

## Top 5 largest gaps

1. **Session goal lifecycle (5 rows missing).** The web reads/sets/clears a goal and drives the pinned
   two-step operator transition (`get` → `set` with `transition_actor:"user"`) — `features/autonomy/AutonomyPanel.tsx:138,174,194`,
   `store.ts:475,503,532`. Native has no goal at all: `session/goal/get|set|clear` are `absent`, and the
   `session/goal/updated` / `cleared` events land on no-op arms (`state.rs:456,457`).
2. **Recurring loops (2 rows missing).** The web lists loops with pause/resume/delete/fire-now and creates
   maintenance / self-paced / fixed-interval loops with native interval syntax — `AutonomyPanel.tsx:235`,
   `loop-creation.ts:68`, `store.ts:591,625`. Native: all six `loop/*` requests `absent`; `loop/updated` &
   `loop/fired` no-op (`state.rs:458,459`).
3. **Zero-token monitors (3 rows missing).** The web lists/creates/controls monitors (pause/resume/delete,
   argv as an explicit JSON array, optional filter regex, flood pause reason) — `AutonomyPanel.tsx:335,439`,
   `model.ts:215`, `store.ts:669,699`. Native: all five `monitor/*` requests `absent`; the three monitor
   events are no-ops (`state.rs:464,465,466`).
4. **Agent roster & detail (7 rows missing).** The web lists agents and reads status/output/artifacts,
   with interrupt/close controls and a single detail viewer that rejects superseded reads —
   `AgentPanel.tsx:41,53,65,80,91,293`, `store.ts:743,774,792,810,833`. Native: all seven `agent/*`
   requests `absent`; `agent/updated|output/delta|artifact/updated` are no-ops (`state.rs:450,451,452`).
5. **Parallel-agent spawn (1 row missing).** The web composes the pinned TUI prompt and admits it only as
   an idle-only ordinary turn — `agent-spawn.ts:2`, `agent-spawn-admission.ts:10`. Native has no spawn
   path; the admission requires `turn/start` **and** `agent/list` (the latter `absent`).

The remaining 8 rows are the cross-cutting store/host invariants the native app has no counterpart for at
all: capability gating (`coding.*` method+feature), authority epoch, subscribe-before-refresh binding,
foreign-session isolation, refresh revision guard, create upsert, capability withdrawal fail-closed,
error authorization, and session binding (`store.ts:214,358,880`, `binding.ts:42`, `model.ts:101,243`).

## Capabilities with NO test at all in the web

**None.** All 26 rows name at least one unit test in the dir (`*.test.ts[x]`); 16 also name a
`e2e/native-workflows.spec.ts` case. Nothing in this dir needs a spec written first.

## Notes on citations

- Native consumer cites are the `apply_protocol` arms in `crates/octos-app-store/src/state.rs`
  (`450-452` agents, `456-457` goals, `458-459` loops, `464-466` monitors) — all explicit no-ops,
  matching `docs/protocol-matrix.csv` (`decoded-only`).
- No row cites `appcard/app/src/backend/octos_ui.rs:~899-945` (the deliberate drain list), per
  `.peer/LESSONS.md` §"Parity-map pitfalls".
- Protocol method strings are taken from `CORE_UI_METHODS` / the feature's own constant list, including
  the `coding.*.v1` gate features (`packages/client/src/autonomy.ts:98-116`).
