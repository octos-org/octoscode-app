# Behaviour specs — group `spec-b` (board #9b)

Behaviour specs for the 12 web capabilities the parity matrix lists with **no web unit test and no
Playwright spec**. Written from the **web source** (`~/src-web` @ `70a8a1c`), not
from memory. No native card may claim these until a spec exists (supervisor 8.8 condition 5).

Machine-readable twin: `docs/specs/spec-b.csv` (one row per `### SPEC-` block below).

Each block carries: id · capability · web source (`file:line`) · Given/When/Then scenarios covering
the happy path and every error/edge branch found in code (including fail-closed behaviour) · exact
user-visible strings (quoted, with `file:line`) · protocol methods/fields · a **native test hint**
(fixture server vs none; the `/snap` assertion that proves it).

---

## skills

### SPEC-skills-1

**Capability.** Profile skills surface: list a Profile's installed skills, search the registry,
install a repo (explicit, scoped mutation) and remove a skill by name — **and** surface the
background skill-action job snapshot (`skill/action/job/updated`).

**Web source.** `src-web/apps/web/src/features/skills/SkillsDialog.tsx:21` (component;
`SkillsDialog.tsx:16-18` the `Mutation` union; `:94-107` `refresh`; `:110-134` `confirmMutation`),
`src-web/packages/client/src/skills.ts:139-205`, `skill-methods.ts:1-6`.

**Protocol methods.** `profile/skills/list`, `profile/skills/registry/search`,
`profile/skills/install`, `profile/skills/remove`. Notification (see gap): `skill/action/job/updated`.

**Scenarios.**

- **Happy path — list.** *Given* a confirmed Profile and a server advertising `profile/skills/list`,
  *When* the dialog mounts, *Then* it calls `list()` (`skills.ts:140-148`) and renders one row per
  `InstalledSkill` (`SkillsDialog.tsx:187-215`); a `version: null` row shows the fallback and the row
  shows `toolCount` + `tools`.
- **Happy path — search.** *Given* `profile/skills/registry/search` advertised, *When* the operator
  submits a non-empty query, *Then* `search(query)` runs (`skills.ts:149-159`) and packages render;
  a blank query throws `"Invalid registry query"` (`skills.ts:151`) and publishes nothing.
- **Happy path — install.** *Given* `profile/skills/install` advertised, *When* the operator confirms
  an install of `repo`/`branch`, *Then* `install(repo, branch || undefined)` runs with `force: false`
  (`skills.ts:160-186`) and a notice renders with installed/skipped/dependencies lists.
- **Happy path — remove.** *Given* `profile/skills/remove` advertised, *When* `Remove <name>` is
  clicked and confirmed, *Then* `remove(name)` runs (`skills.ts:188-204`) and a notice renders.
- **Fail-closed — unadvertised.** *Given* the server does not advertise a method, *Then* the
  `available(...)` gate (`SkillsDialog.tsx:51`) hides that control and `commands` throws
  `"<method> is not advertised by this server"` (`skills.ts:136-137`); **no inventory is fabricated**
  (the dialog renders no "No skills installed" row before a load — see
  `SkillsDialog.test.tsx:29`).
- **Fail-closed — wrong profile / bad receipt.** *Given* a reply whose `profile_id` is not the
  confirmed Profile, or `ok !== true`, or `removed !== skillName`, *Then* the parse returns null and a
  bounded error is thrown (`skills.ts:170-180`, `:195-203`).
- **Mutation lock.** *Given* known Profile work is running (`profileBusy`), *Then* the mutation is
  refused (`SkillsDialog.tsx:60`) and `"Skill changes are paused while known work is running in this
  Profile."` is shown (`SkillsDialog.tsx:170-176`).
- **Latest-request-wins.** *Given* a request is in flight and the authority (`client`/`profileId`)
  changes, *Then* the stale result must not publish (`SkillsDialog.tsx:62-63`, `:99-106`).

**Exact strings** (with `file:line`). `"Profile skills"` (`:145`), `"Refresh skills"` (`:147`),
`"Close skills"` (`:150`), `"Server Profile:"` (`:154`), `"Skills are shared by this Profile, not
installed in your browser. Installation may download executable tools and dependencies. Review and
trust the source first."` (`:158-160`), `"Skill changes are paused while known work is running in
this Profile."` (`:172-174`), `"Waiting for the server…"` (`:179`), `"Installed skills"` (`:184`),
`"No skills installed in this Profile."` (`:188`), `"Version not reported"` (`:194`), `"tools"`
(`:194`), `"Remove "` (`:209`), `"Skill registry"` (`:219`), `"Registry query"` (`:236`),
`` `Server installed: ${result.installed.join(", ") || "none"}. Skipped: ${result.skipped.join(", ")
|| "none"}. Dependencies: ${result.dependenciesInstalled.join(", ") || "none"}.` `` (`:118-120`),
`` `Removed ${choice.name} from server Profile ${profileId}.` `` (`:126`), `"Could not confirm the
server change. It may have been applied; refresh the Profile before reviewing another attempt."`
(`:79`), `"Skill request failed"` (`:82`).

**Native test hint.** Needs a **fixture server**: `apps/web/scripts/mock-ui-server.mjs` (or the
spec's own `page.routeWebSocket`) answering `profile/skills/list` (+ `install`/`remove` where tested).
Assert the dialog title `Profile skills` and one row per served skill; assert the unadvertised case
hides `Review installation` / `Search registry` (mirrors `SkillsDialog.test.tsx:29-31`).

**⚠ Citation does not resolve to consumed behaviour.** The board cites
`SkillsDialog.tsx:21` for "the background skill-action job snapshot (SkillActionJobUpdated)".
`SkillsDialog.tsx:21` is the component's **doc comment** (`"Profile mutations are explicit, scoped
and never presented as browser installs."`) — it is *not* a `SkillActionJobUpdated` consumer.
`git grep "SkillActionJob\|skill/action/job"` over the whole tracked tree matches **only**
`src-web/packages/client/src/generated/core-contract.ts:123` (the constant) and `:310` (the
notification registry list). **There is no web consumer of `skill/action/job/updated` in the source
tree.** `docs/parity-matrix.csv:15` already records this ("web-only notification with no native
consumer"). A job-snapshot scenario therefore **cannot be written from the web source** and is left
as this documented gap. `profile/skills/*` (list/search/install/remove) *is* fully specified above.

---

## review

### SPEC-review-1

**Capability.** Permission profile: read the coding permission profile (list) and apply an explicit
update from the review/coding-safety surface.

**Web source.** `src-web/apps/web/src/features/review/use-coding-safety.ts:95` (`configureCapabilities`
capability gate; `:117-159` `refreshPermission`; `:161-259` `updatePermission`),
`src-web/packages/client/src/client.ts:571-590`, `coding.ts:17-42`, `types.ts:100-131`.

**Protocol methods/fields.** `permission/profile/list` (`PermissionProfileListParams { session_id }`
→ `PermissionProfileListResult { session_id, current, profiles }`), `permission/profile/set`
(`PermissionProfileSetParams { session_id, update, runtime_mode? }` →
`PermissionProfileSetResult { session_id, current, applied }`). `update` =
`PermissionProfileUpdate { mode?, network?, approval_policy? }` (`types.ts:106-110`).

**Scenarios.**

- **Availability gate.** *Given* capabilities, *When* `configureCapabilities` runs, *Then*
  `available = supportsMethod(PERMISSION_PROFILE_LIST)` and `editable = available &&
  supportsMethod(PERMISSION_PROFILE_SET)` (`use-coding-safety.ts:95-113`); any in-flight read/mutation
  is invalidated synchronously (`:88-93`).
- **Read (happy).** *Given* `available` and no in-flight read, *When* `refreshPermission` runs,
  *Then* `permission/profile/list` is called and `{ loading: false, result }` is published
  (`:117-158`), guarded by `RequestAuthorityGate.isCurrent(request, client, sessionId)`.
- **Update (happy).** *Given* `available && editable`, *When* `updatePermission(update)` runs and the
  server replies `{ session_id, current, applied: true }`, *Then* the read requests are invalidated
  (`:213-216`), the result is merged into `current` (`:232-243`), a post-write refresh runs, and
  `onPermissionApplied(client)` fires (`:244-247`).
- **Edge — not advertised.** *Given* the requested `{mode, network}` is not in
  `permission.result.profiles`, *Then* `updatePermission` sets the error and returns **without** any
  RPC (`:206-211`).
- **Fail-closed — foreign session.** *Given* a reply whose `session_id !== sessionId`, *Then*
  `"permission/profile/set returned another session"` is thrown (`:225-226`).
- **Fail-closed — not applied.** *Given* `result.applied === false`, *Then*
  `"The server did not apply the permission change"` is thrown (`:227-228`).
- **Superseded.** *Given* a mutation in flight and a newer mutation/read, *Then* only the latest
  publishes (`permissionMutationRef`, `:215-216`, `:222`).

**Exact strings.** `"The requested permission profile was not advertised for this session"`
(`:207-208`), `"permission/profile/set returned another session"` (`:225`), `"The server did not
apply the permission change"` (`:227`).

**Native test hint.** Needs a **fixture server** serving `permission/profile/list` +
`permission/profile/set` (the review surface is otherwise pure state). Assert the runtime state
exposes `available/editable/loading/busy/result/error` and that a `applied:false` reply lands as an
error, not a saved profile — the `/snap` proves the permission row shows the refusal.

---

## research

### SPEC-research-1

**Capability.** Remove a research lane by key.

**Web source.** `src-web/packages/client/src/research.ts:190-206`.

**Protocol methods/fields.** `profile/sub_providers/remove`, params `{ profile_id, key }`; parses
`ResearchLaneMutation { profileId, lanes, applied, restart_required }` (`research.ts:78-95`).

**Scenarios.**

- **Happy.** *Given* `profile/sub_providers/remove` advertised, *When* `remove(key)` runs with a
  non-empty key, *Then* the RPC is sent and the parsed mutation is returned (`:190-205`).
- **Edge — blank key.** *Given* `key` is blank, *Then* `"A lane key is required"` is thrown before any
  RPC (`:191`).
- **Fail-closed — unadvertised.** *Given* the method is not advertised, *Then* `available(...)` throws
  `"<method> is not advertised by this server"` (`:134-138`).
- **Fail-closed — receipt still contains the lane.** *Given* `applied === true` **and** the returned
  `lanes` still contains `key`, *Then* the removal is treated as unconfirmed and
  `"Research removal receipt still contains the requested lane; refresh before retrying"` is thrown
  (`:197-202`).
- **Fail-closed — malformed receipt.** *Given* a reply failing `parseResearchLaneMutation` (bad
  `profile_id`, non-boolean `applied`/`restart_required`), *Then* `"Invalid or wrong-profile research
  mutation receipt; refresh before retrying"` is thrown (`:134-141`).

**Exact strings.** `"A lane key is required"` (`:191`), `"Research removal receipt still contains the
requested lane; refresh before retrying"` (`:199-200`), `"A confirmed Profile is required"`
(`:132`), `"Invalid or wrong-profile research mutation receipt; refresh before retrying"` (`:139`).

**Native test hint.** Needs a **fixture server** serving `profile/sub_providers/remove`. Drive the
removal and assert the receipt rule: a reply that still lists the key must surface the bounded error,
**not** a success notice (the `/snap` proves no "Removed" line renders).

### SPEC-research-2

**Capability.** Latest-request-wins / generation guard: a superseded list or mutation cannot publish
its result.

**Web source.** `src-web/apps/web/src/features/research/ResearchDialog.tsx:53-98` (`generation`,
`pending`, `blocked` refs and `run`; `:105-112` mount effect). Note: this uses an **inline generation
ref**, *not* `features/async`.

**Protocol methods.** `profile/sub_providers/list`, `profile/sub_providers/upsert`,
`profile/sub_providers/remove`.

**Scenarios.**

- **Superseded read.** *Given* a `list()` in flight, *When* the authority (`client`/`profileId`)
  changes (mount effect bumps `generation`), *Then* the in-flight result must not call `setData`
  (`:68-69`, `:103`, `:105-112`).
- **Superseded mutation.** *Given* a confirm in flight, *When* `generation` advances before the reply,
  *Then* `recordMutation` must not run and no notice publishes (`:65-69`, `:132-134`).
- **Re-entrancy.** *Given* a request already pending, *When* another `run` is requested, *Then* it
  returns immediately without a second RPC (`:66`); a mutation is additionally refused while
  `blocked.current` (`:66`, `:76`).
- **Finish ownership.** *Given* the request is superseded mid-flight, *Then* the `finally` must still
  clear the local `busy`/`pending` it installed while publishing nothing (`:91-98`).
- **Error mapping.** *Given* a failed mutation, *Then* the bounded string `"Could not confirm the
  server change. It may have been applied; refresh before a new attempt and re-enter any
  credential."` is shown (sliced to 512, `:81-90`); a failed read shows `cause.message` or
  `"Research request failed"`.

**Exact strings.** `"Could not confirm the server change. It may have been applied; refresh before a
new attempt and re-enter any credential."` (`:85`), `"Research request failed"` (`:88`).

**Native test hint.** **Fixture server**, with a **deferred** reply for `profile/sub_providers/list`:
open the surface, switch the confirmed authority before releasing the reply, then release it. Assert
the superseded result never renders — the `/snap` proves the row list did not change.

---

## onboarding

### SPEC-onboarding-1

**Capability.** Credential redaction and latest-request-wins: the API key is never echoed back in an
error, and a superseded prepare/submit cannot publish its result.

**Web source.** `src-web/apps/web/src/features/onboarding/use-onboarding.ts:61-71` (`RequestGate`
+ `reset`), `:73-124` (`prepare`), `:126-172` (`submit`), `:173-183` (`redactSecret`).

**Protocol methods.** `profile/local/create`, `profile/llm/catalog`, `profile/llm/test`,
`profile/llm/upsert` (`onboarding-methods.ts:4-13`).

**Scenarios.**

- **Redaction.** *Given* an error message that contains the submitted API key, *When* the error is
  published, *Then* every occurrence of the key is replaced with `[redacted]` and the result is
  sliced to 1000 chars (`:173-183`); the raw key must never appear in `state.error`.
- **Superseded prepare.** *Given* `prepare()` in flight, *When* `reset()` runs or the client changes,
  *Then* the stale catalog must not publish (`:76`, `:96-101`, `:110-115`).
- **Superseded submit.** *Given* `submit()` in flight, *When* the authority changes, *Then* the stale
  result must not publish (`:143-147`, `:155-163`).
- **Re-entrancy.** *Given* `submissionActiveRef.current` is true, *When* `submit` is called again,
  *Then* it returns immediately without a second RPC (`:135`).
- **Happy path.** *Given* a client, `state.supported`, a catalog and `phase === "ready"`, *When*
  `submit` runs, *Then* it delegates to `submitOnboarding` (`:145-155`) which creates/tests/saves and
  calls `onConfigured` (`onboarding-submission.ts:59-134`).

**Exact strings.** `"[redacted]"` (`:178`). (Also relevant to the flow, owned by
`onboarding-submission.ts`: `"Profile ID and profile name are required."` / `"Profile ID, profile
name, and API key are required."` at `:42-46`, `"The provider test did not pass."` at `:104`.)

**Native test hint.** **No server round-trip needed** for the redaction scenario — it is pure
(`redactSecret`). Drive it headlessly by submitting a key, forcing a rejection whose message embeds
the key, and asserting the published error contains `[redacted]` and not the key: the `/snap` proves
the error line contains no credential. The generation-guard scenario needs a deferred
`profile/llm/catalog` fixture.

---

## approval

### SPEC-approval-1

**Capability.** Lifecycle notifications decided / auto_resolved / cancelled update UI (toast on
auto-resolve).

**Web source.** `src-web/packages/client/src/interaction.ts:77-92` (`approvalResolutionId`).
Consumers: `src-web/apps/web/src/features/session/session-interaction-ledger.ts:291`,
`src-web/apps/web/src/features/session/session-peer-coordinator.ts:241`.

**Protocol notifications.** `approval/decided`, `approval/auto_resolved`, `approval/cancelled`;
param field `approval_id: string`.

**Scenarios.**

- **Happy — resolve by id.** *Given* a notification whose `method` is one of the three and whose
  `params.approval_id` is a string, *When* `approvalResolutionId(notification)` runs, *Then* it
  returns that `approval_id` (`interaction.ts:78-92`).
- **Edge — other method.** *Given* a notification of any other method, *Then* it returns `null`
  (`:80-87`).
- **Edge — missing/typed id.** *Given* the method matches but `params` is not a record or
  `approval_id` is absent/non-string, *Then* it returns `null` (`:88-91`).
- **Consumer — ledger.** *Given* a resolution id, *Then* the session interaction ledger matches the
  pending approval and settles it, while a terminal from another session cannot consume it
  (`session-interaction-ledger.ts:291`).
- **Consumer — peer coordinator.** *Given* a peer-owned pending approval, *Then* the coordinator
  clears it on the resolution frame (`session-peer-coordinator.ts:241`).

**Exact strings.** The three method strings: `"approval/decided"`, `"approval/auto_resolved"`,
`"approval/cancelled"` (`core-contract.ts:105-107`). The param field `approval_id`.

**⚠ "toast on auto-resolve" is not evidenced in the source.** The board's parenthetical says
`auto_resolved` raises a *toast*. `interaction.ts:82-84` only classifies the id; the two consumers
above settle ledger/coordinator state. `git grep -il toast` over the whole tracked tree returns
**nothing** — there is no toast component in the web source. So the spec pins the **classification +
settlement** behaviour (above) and records the toast claim as an unverified gap; `auto_resolved`
is handled **identically** to `decided`/`cancelled` (one shared branch, `:80-86`), with no distinct
toast path.

**Native test hint.** **Fixture server** emitting `approval/requested` then each of
`approval/decided|auto_resolved|cancelled` with the same `approval_id`. Assert the pending approval
row clears for all three — the `/snap` proves no approval card remains. (Note the native side already
consumes these three: `docs/protocol-matrix.csv`.)

---

## inventory

### SPEC-inventory-1

**Capability.** Read-only runtime tools inventory (name/category/status/policy/aliases/backend).

**Web source.** `src-web/apps/web/src/features/inventory/InventoryDialog.tsx:41` (the `tools()` call;
`:36-65` `refresh`; `:146-178` tools render), `src-web/packages/client/src/inventory.ts:53-95`,
`inventory-methods.ts:3`.

**Protocol methods/fields.** `tool/status/list`; `RuntimeTools { sessionId, profileId, policyId,
tools: RuntimeTool[] }`, `RuntimeTool { name, category, status, policy, aliases, backendTool?,
detail? }`.

**Scenarios.**

- **Happy.** *Given* the dialog open with `mode: "tools"`, *When* it mounts, *Then* `commands.tools()`
  runs and the parsed `RuntimeTools` renders (`:47-49`, `:146-178`); the header line shows
  `"{n} tools reported · Policy {policyId}"` (`:150-153`).
- **Edge — backend differs from name.** *Given* `tool.backendTool` present and `!== tool.name`, *Then*
  a `"Backend:"` line renders (`:158-163`); otherwise it is omitted.
- **Edge — aliases.** *Given* non-empty `aliases`, *Then* an `"Aliases:"` line renders the joined list
  (`:164-168`).
- **Edge — empty result.** *Given* no matching tools, *Then* `"No matching tools."` renders
  (`:178`).
- **Fail-closed — wrong session/profile.** *Given* a reply whose `session_id`/`profile_id` differ,
  *Then* `parseRuntimeTools` returns null and the client throws a bounded error
  (`inventory.ts:58-66`); the dialog shows `error` via `role="alert"` (`:141`).
- **Superseded.** *Given* a refresh in flight, *When* `mode`/authority changes, *Then* the stale
  result must not publish (`:37`, `:46`, `:49`, `:66-73`).

**Exact strings.** `"Runtime tools"` (`:111`), `"Refresh inventory"` (`:114`), `"Read-only status
reported by the server. This runtime does not provide tool or MCP configuration editing."`
(`:125-127`), `"Search runtime inventory"` (`:131`), `"Search names, status, or tools…"` (`:134`),
`"Loading runtime inventory…"` (`:136`), `"tools reported · Policy"` (`:152`), `"Backend:"` (`:160`),
`"Aliases:"` (`:166`), `"No matching tools."` (`:178`).

**Native test hint.** **Fixture server** serving `tool/status/list` with two tools (one with a
distinct `backend_tool`, one with aliases). Assert names/status/category/policy render and the
`Backend:`/`Aliases:` lines appear only when present — `/snap` on the tool rows.

### SPEC-inventory-2

**Capability.** MCP server status inventory (id/transport/status/toolCount/summary).

**Web source.** `src-web/apps/web/src/features/inventory/InventoryDialog.tsx:51` (the `mcp()` call;
`:180-201` mcp render), `src-web/packages/client/src/inventory.ts:96-150` (`parseRuntimeMcp`),
`inventory-methods.ts:4`.

**Protocol methods/fields.** `mcp/status/list`; `RuntimeMcp { sessionId, profileId, servers:
RuntimeMcpServer[], summary: { connected, connecting, failed, disabled } }`, `RuntimeMcpServer { id,
displayName?, transport?, status, toolCount, tools, error? }`.

**Scenarios.**

- **Happy.** *Given* `mode: "mcp"`, *When* the dialog mounts, *Then* `commands.mcp()` runs and the
  parsed servers + summary render (`:50-52`, `:180-201`).
- **Summary line.** *Then* `"{connected} connected · {connecting} connecting · {failed} failed ·
  {disabled} disabled"` renders (`:183-190`).
- **Edge — per-server.** *Given* a server with `transport`/`tools`/`error`, *Then* the transport and
  tool list render, and `error` renders via `role="alert"` (`:192-198`).
- **Edge — empty.** *Given* `servers.length === 0`, *Then* `"No MCP servers reported by this
  runtime."`; *given* servers exist but none match, *Then* `"No matching servers."` (`:199-201`).
- **Fail-closed.** *Given* a reply whose `session_id`/`profile_id` differ or a malformed server row,
  *Then* the parse returns null and a bounded error is thrown (`inventory.ts`), shown in the alert.

**Exact strings.** `"MCP server status"` (`:111`), `"connected ·"` / `"connecting ·"` / `"failed ·"` /
`"disabled"` (`:184-189`), `"tools"` (`:192`), `"No matching servers."` (`:200`), `"No MCP servers
reported by this runtime."` (`:200`).

**Native test hint.** **Fixture server** serving `mcp/status/list` with a connected + a failed server.
Assert the summary counts and the per-server status/toolCount render — `/snap` on the server rows.

### SPEC-inventory-3

**Capability.** Client-side search filter over tools and servers.

**Web source.** `src-web/apps/web/src/features/inventory/InventoryDialog.tsx:74-101`.

**Protocol methods.** None (pure, client-side over already-loaded `tools`/`mcp`).

**Scenarios.**

- **Happy — tools.** *Given* loaded tools, *When* `query` is typed, *Then* the row is kept when the
  lowercased join of `name, category, status, policy, detail, ...aliases` includes the lowercased
  trimmed query (`:74-88`).
- **Happy — servers.** *Given* loaded servers, *When* `query` is typed, *Then* the row is kept when
  the lowercased join of `id, displayName, status, transport, ...tools` includes the query (`:89-101`).
- **Edge — no match.** *Given* no row matches, *Then* `"No matching tools."` / `"No matching
  servers."` renders.
- **Edge — blank query.** *Given* `query` is empty/whitespace, *Then* all rows are kept (empty search
  matches every join).
- **Case-insensitivity.** *Given* a mixed-case query, *Then* matching is case-insensitive
  (`toLocaleLowerCase()` on both sides).

**Exact strings.** `"Search names, status, or tools…"` (`:134`), `"No matching tools."` (`:178`),
`"No matching servers."` (`:200`).

**Native test hint.** **No server round-trip needed beyond the initial list** — load a fixed fixture
inventory, type a query, and assert filtered rows. `/snap` before/after the keystroke proves the row
count changed and no request was issued for the filter.

---

## error

### SPEC-error-1

**Capability.** Crash screen actions: reload app / copy diagnostics / report-crash link.

**Web source.** `src-web/apps/web/src/features/error/FatalErrorBoundary.tsx:38` (`FatalCrashScreen`;
`:44-51` copy; `:53-90` render; `:93-108` `buildSafeDiagnostic`/`redactSecrets`).

**Protocol methods.** None (a render-failure boundary; no RPC).

**Scenarios.**

- **Happy — actions.** *Given* the boundary caught a render error and `report` is set, *When* the
  crash screen renders, *Then* `"Reload app"` calls `window.location.reload()` (`:66-72`) and
  `"Copy diagnostics"` writes `report` to the clipboard (`:44-51`, `:73-83`).
- **Copy outcome.** *Given* a clipboard write resolves, *Then* the label becomes `"Copied"`; *given*
  it rejects, *Then* `"Copy failed"` (`:78-82`).
- **Report link.** *Then* `"Report this crash ↗"` links to
  `https://github.com/octos-org/octoscode-web/issues/new` (`:85-87`).
- **Redaction.** *Given* an error whose text embeds `token=…`/`auth_token=…`/`api_key=…`/`Bearer …`,
  *Then* `buildSafeDiagnostic` redacts each and slices to 4000 chars (`:93-108`).
- **Accessibility.** *Then* the shell is `role="alert"` and the `<pre>` carries
  `aria-label="Redacted crash diagnostics"` (`:54`, `:64`).

**Exact strings.** `"Octoscode Web stopped rendering"` (`:57`), `"Client view unavailable"` (`:58`),
`"The client could not recover this view. Closing its connection may have stopped running work. Octos
keeps persisted history; unsent drafts and queued messages may be lost when you reload."` (`:60-62`),
`"Redacted crash diagnostics"` (`:64`), `"Reload app"` (`:71`), `"Copy diagnostics"` (`:82`),
`"Copied"` (`:79`), `"Copy failed"` (`:81`), `"Report this crash ↗"` (`:86`); redaction tokens
`[redacted]` (`:105-107`).

**Native test hint.** **No server round-trip needed.** Drive `buildSafeDiagnostic` with an error
embedding `token=secret-value` + `Bearer second-secret` and assert both are redacted (mirrors
`FatalErrorBoundary.test.tsx:5-15`). For the actions, render the crash screen and assert the two
buttons + link exist; `/snap` proves the diagnostics `<pre>` shows `[redacted]` and not the secret.

### SPEC-error-2

**Capability.** Unavailable-surface fallback (modal vs inline section, `aria-labelledby`).

**Web source.** `src-web/apps/web/src/features/error/SurfaceBoundary.tsx:44` (`UnavailableSurface`;
`:16-42` boundary; `:91-112` modal-vs-section).

**Protocol methods.** None.

**Scenarios.**

- **Happy — boundary catches.** *Given* a child view throws, *Then* `getDerivedStateFromError` sets
  `failed` and `UnavailableSurface` renders instead, so the session owner above the boundary is not
  unmounted (`:15`, `:22-27`).
- **Failure copy.** *Then* `"{name} unavailable"` renders as the heading, with the two paragraphs
  (`:57-72`) — and the second warns a reload may discard drafts/queued messages.
- **Modal vs inline.** *Given* `onDismiss` is provided, *Then* the fallback is a `ModalSurface`
  labelled by `titleId` with `initialFocusRef` on the close button and `onEscape`/`closeOnBackdrop`
  (`:91-107`); *given* no `onDismiss`, *Then* it is an inline `<section aria-labelledby={titleId}>`
  (`:108-112`).
- **Loading variant.** *Given* `onDismiss` and a pending `Suspense`, *Then* the fallback shows
  `"Loading {name}…"`, the provided `fallback` body, and a `"Cancel"` button (`:29-36`, `:52`,
  `:57-60`, `:76-78`).
- **Actions.** *Then* non-loading shows `actions` (if any) and `"Reload app"`
  (`window.location.reload()`) (`:80-87`).

**Exact strings.** `"Loading ${name.toLowerCase()}…"` / `"${name} unavailable"` (`:58`),
`"This view could not be displayed. Other parts of the app remain available."` (`:64-67`),
`"Reload the page to try again. Reloading may stop running work and discard drafts and queued
messages."` (`:68-71`), `"Cancel"` (`:77`), `"Close"` (`:77`), `"Reload app"` (`:84`). Default
`name = "View"` (`:45`); default rendering label `"View"` → `"View unavailable"`.

**Native test hint.** **No server round-trip needed.** Render `SurfaceBoundary` with a throwing child
and `onDismiss` present vs absent; assert modal vs inline and the `aria-labelledby` linkage.
`/snap` proves the heading text and that the surrounding session chrome is still present. Existing
e2e coverage of the failed-chunk flavour: `e2e/surface-recovery.spec.ts:33`.

### SPEC-error-3

**Capability.** Error toasts (transient, bounded queue).

**Web source (as cited).** `src-web/apps/web/src/features/error/SurfaceBoundary.tsx:63`.

**⚠ This capability has no implementation in the web source.** The board cites
`SurfaceBoundary.tsx:63`; that line is the inline `<div role="alert">` inside `UnavailableSurface`
(`:63-72`) — a **persistent inline alert**, not a transient toast. `git grep -il toast` over the
whole tracked tree (`apps/**`, `packages/**`, excluding `node_modules`) returns **nothing**: there is
no toast component, no toast queue, and no "bounded queue" of transient messages anywhere in the web
source. `docs/parity-matrix.csv:8` still lists the row (native `partial` on
`appcard/crates/octos-app-store/src/toasts.rs:28` + `state.rs:383`), but the **web** side the spec
must be written from does not contain the behaviour.

**Protocol methods.** None evidenced (no toast producer found).

**Scenarios (what the cited anchor actually does — the honest nearest behaviour).**

- **Persistent inline alert.** *Given* a surface failed, *Then* `"This view could not be displayed…"`
  is rendered inside `<div role="alert">` (`:63-72`) and **stays** until dismissed/reloaded — it does
  **not** auto-expire. There is no queue, no cap, and no timeout.
- **Gap (unwritten).** *Given* the board's "transient, bounded queue" description, *Then* **no web
  scenario can be written** — the behaviour is absent from the source. Recorded so no native card
  claims it from a web spec that does not exist.

**Exact strings.** `"This view could not be displayed. Other parts of the app remain available."`
(`:64-67`), `"Reload the page to try again. Reloading may stop running work and discard drafts and
queued messages."` (`:68-71`).

**Native test hint.** For the *inline alert* that does exist: **no server round-trip needed** — a
throwing child proves it, with `/snap` on the alert text. For the *toast queue*: **cannot be driven
from the web source** (no producer/consumer exists); if a native toast queue is wanted it must be
specified against the **native** source (`toasts.rs:28`) under a native card, not this web spec.

---

## Coverage note

- 12 `### SPEC-` blocks, matching the 12 rows in `docs/specs/spec-b.csv`.
- Every block names a web source with `file:line`.
- Two board citations do **not** resolve to consumed web behaviour and are recorded as explicit gaps
  (not invented): `SPEC-skills-1` (no `skill/action/job/updated` consumer) and `SPEC-error-3` (no
  toast queue in the web source). `SPEC-approval-1` additionally notes the "toast on auto-resolve"
  parenthetical is unevidenced.
