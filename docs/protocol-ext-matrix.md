# Extension-method matrix — the web's RPC calls outside `core-contract.ts` (card #2c)

Companion machine-readable file: `docs/protocol-ext-matrix.csv` (34 rows).
Card #2 (`docs/protocol-matrix.csv`) covers the 125 `CORE_UI_METHODS` in
`packages/client/src/generated/core-contract.ts` (contract rev `4231669`). This
card covers the **AppUI extension** methods the web client calls that live
OUTSIDE that generated contract — handwritten constants and helpers.

Sources: web oracle `<WORKSPACE>/src-web`
(octoscode-web @ `70a8a1c`); protocol pin `<WORKSPACE>/src-octos`
(`a6ea8505`, the rev OctoSense pins — see "Pin" below); native
`<WORKSPACE>/ref/OctoSense/apps/appcard/app`.

## Counts

- **34** extension methods, all `kind=request`.
- `server_status`: **28 `defined`**, **8 `absent-in-pin`** (allowed set).
- `native_status`: **34 `absent`** (allowed set) — the native app currently
  emits none of these extension methods.

## How the list was built (seed: complete the outer loop's list yourself)

Search used (non-test sources only, then reconciled against the card's list):

1. **Literal method strings.** Every `"<ns>/<path>"`-shaped string literal under
   `packages/client/src` + `apps/web/src`, minus the 125 core methods, the 46
   `CORE_UI_SERVER_METHODS`, and the 49 `CORE_UI_NOTIFICATION_METHODS`
   (`comm -23` against the union of the three generated lists). This yields the
   method literals but also non-method noise (MIME types, `shiki/core`, …).
2. **Constant-map authoritative pass.** For each candidate, the defining
   constant map: `APPUI_INVENTORY_METHODS`, `APPUI_CONTEXT_METHODS`,
   `APPUI_SNAPSHOT_METHODS`, `APPUI_SERVER_METHODS`,
   `APPUI_ONBOARDING_METHODS`, `APPUI_WORKSPACE_BROWSE_METHODS`,
   `APPUI_RESEARCH_METHODS`, `APPUI_SKILL_METHODS`, `PEER_METHODS`,
   `EXTERNAL_DRIVER_METHODS`, plus the locals `TURN_STEER_METHOD`
   (`steer.ts:8`) and the two locals in `external-driver-peer-control.ts`
   (`:32` `peer/dispatch`, `:33` `peer/control`).
3. **Call-site trace.** Each method traced from its constant to the
   `request(...)`/`validatedRequest(...)` production call site (the `rpc.ts`
   `createRequest` / `client.ts` `request` path).

**Reconciliation vs the card's "about 36".** No method the card names is
missing; no method outside it was added. The list is **34, not 36**: the card's
"~36" is approximate, and the two extra it implies are **`peer/model/set`** and
**`peer/context/open`|`close`** — those ARE `defined` in octos (`APPUI_EXTRA_METHODS`)
but the **web never calls them**, so they are correctly excluded (0 call sites).
They are listed below as octos-only for completeness, not as matrix rows.

## `absent-in-pin` — these block parity until octos serves them (8)

The web calls these, but **neither** the pinned `a6ea8505` **nor** the contract
rev `4231669` defines them anywhere in the octos tree
(`git grep -c '<substring>' a6ea8505` → 0 and `4231669` → 0, whole tree):

| method | web defines it at | feature gate (web) |
|---|---|---|
| `peer/dispatch` | `packages/client/src/external-driver-meta.ts:11` | `external_driver_v1` |
| `peer/control` | `packages/client/src/external-driver-meta.ts:12` | `external_driver_v1` |
| `session/driver/get` | `packages/client/src/external-driver-meta.ts:7` | `external_driver_v1` |
| `session/driver/acquire` | `packages/client/src/external-driver-meta.ts:8` | `external_driver_v1` |
| `session/driver/renew` | `packages/client/src/external-driver-meta.ts:9` | `external_driver_v1` |
| `session/driver/release` | `packages/client/src/external-driver-meta.ts:10` | `external_driver_v1` |
| `session/wake/claim` | `packages/client/src/external-driver-meta.ts:13` | `external_driver_v1` |
| `session/wake/ack` | `packages/client/src/external-driver-meta.ts:14` | `external_driver_v1` |

Note the web's own source comments pin these to octos lines that do not exist at
the pinned rev: `external-driver-meta.ts:3-4` cites
`ui_protocol_transport.rs:19218-19236` and `peers/mod.rs:3451` for the refusal
kinds, and `external-driver-wake-control.ts:11` cites "ui_protocol.rs 1249 /
1253". At `a6ea8505`, `ui_protocol_transport.rs:19218` is inside
`handle_raw_appui_rpc`'s autonomy dispatch (unrelated), and `ui_protocol.rs` has
no driver/wake constants. **The web client targets a newer octos than the pin
carries** (the `external_driver_v1` family, ~#2541+). Until the pin moves, these
8 methods cannot be exercised end-to-end against the pinned server.

## Web-used methods not `handled` natively (all 34)

Every one of the 34 is `native_status=absent`: the native app card
(`ref/OctoSense/apps/appcard/app`) neither emits nor handles any extension
method. Evidence per method in the CSV `native_src`/`notes`; the native
transport's only method set is `methods::{SESSION_OPEN, TURN_START,
TURN_INTERRUPT, APPROVAL_RESPOND, DIFF_PREVIEW_GET, TASK_OUTPUT_READ,
SESSION_LIST, SESSION_HYDRATE, MESSAGE_DELTA, TOOL_STARTED, TURN_COMPLETED}`
(`crates/octos-app-transport/src/proto.rs:123-190`), all core methods.
`grep -rF '"<method>"'` over the native tree returns **0** for all 34.

**Priority for the port** (by web call-site count / user-visible surface):
`peer/dispatch` (33 prod sites), `peer/control` (25), `session/driver/get` (21),
`profile/llm/list` (9), `profile/llm/select` (6), `session/driver/{renew,release}`
(5 each), `profile/sub_providers/list` (4), `peer/prepare` (4), `turn/steer` (3),
`session/wake/{claim,ack}` (3 each), `snapshot/{list,restore}` (2 each),
`onboarding/workspace_*` (2 each). Full counts in the CSV `web_call_sites`.

## Pin

Native pins octos at `a6ea8505` (`apps/appcard/app/Cargo.toml:85-86`,
`octos-core` + `octos-cli`). The web's generated contract records revision
`4231669` (`packages/client/src/generated/core-contract.ts:3-6`), and `4231669`
**is an ancestor** of `a6ea8505` (`git merge-base --is-ancestor 4231669 a6ea8505`
→ true). So `a6ea8505` is the newer of the two — and it still lacks the
`external_driver_v1` family, i.e. that family landed after `a6ea8505`.

## Octos-only extensions the web does NOT call (out of matrix scope)

Defined in `APPUI_EXTRA_METHODS` but with 0 web call sites (`grep -rF '"<m>"'`
over `packages/client/src apps/web/src` → 0): `peer/model/set`,
`peer/context/open`, `peer/context/close`, `onboarding/workspace_probe`,
`voice/admit`, `voice/commit_admission`, `skill/action/list`,
`skill/action/invoke`, `skill/action/job/list`, `skill/action/job/read`,
`auth/status`, `auth/send_code`, `auth/verify`, `auth/me`, `auth/logout`,
`client_hello`. (The web's `client_hello`/auth flow rides the REST/auth path, not
these RPC names — verified 0 literal-string hits.)

## Verification commands

```
# row count + statuses (Acceptance #1)
python3 -c "import csv;r=list(csv.DictReader(open('docs/protocol-ext-matrix.csv')));print(len(r));print(sorted({x['server_status'] for x in r}),sorted({x['native_status'] for x in r}))"
34
['absent-in-pin', 'defined'] ['absent']

# no CSV method appears in core-contract.ts (Acceptance #2)
python3 - <<'EOF'
import csv
core=set()
import re
for l in open('<WORKSPACE>/src-web/packages/client/src/generated/core-contract.ts'):
    core|=set(re.findall(r'"([a-z][a-z0-9_]*/[a-z0-9_./-]+)"',l))
rows=[r['method'] for r in csv.DictReader(open('docs/protocol-ext-matrix.csv'))]
hit=[m for m in rows if m in core]
print('overlap:',hit)
EOF
overlap: []
```
