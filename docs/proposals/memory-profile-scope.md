# Proposal: scope the octos `memory/*` methods to a profile the client names

- **Status:** draft for the operator to submit upstream (octos). A36, 2026-10-03.
- **Blocks:** board 5 (`design/stage-a/phase4-new5/`), the Memory surface.

## Finding

All five memory methods resolve their profile from the **authenticated identity**, never from the request or the Session:

- `memory/overview` and `memory/entity` go through `memory_panel::my_memory` → `resolve_my_profile_id` (`auth_handlers.rs:3920`).
- `memory/search`, `memory/load` and `memory/ingest` go through `resolve_memory_profile_runtime` (`ui_protocol_transport.rs:~30098`) → `resolve_my_profile_id` → `ensure_session_profile_runtime`.

For the server's own token (`OCTOS_AUTH_TOKEN`, `AuthIdentity::Admin`), that profile is the fixed **`admin`** profile. It is auto-created with `enabled: false` and no LLM, so it never gets a runtime. Sessions run in the profile the client opens them with (OctosCode: `dsflash`), and the agent writes `MEMORY.md`, daily notes and bank pages into **that** profile's data dir.

I measured this on a private serve at a6ea8505 (a copy of live-gate data, a fresh token, no model turn):

| Call | Reply |
|---|---|
| `memory/overview {}` | `{"overview":{"ok":true,"long_term":"","today":"","recent":[],"entities":[],…,"refresh_enabled":true}}`, which is admin's empty memory |
| `memory/search {"query":"dentist"}` | `-32603` "No ProfileRuntime registered for profile 'admin'. Set up the profile with an API key in the dashboard." `{kind: runtime_unavailable}` |
| `memory/ingest {records:[doc]}` | same `runtime_unavailable` |
| `memory/load {"id":…}` | same `runtime_unavailable` |
| `auth/me {}` | `{"email":"unknown account","profile_id":"_main"}`, so the client cannot even learn that memory answered for `admin` |

The Skills family does not have this gap: `profile/skills/*` take `profile_id` and authorize it (`raw_profile_skill_profile_id`). octos main is unchanged here (3916c6a8, and dde76555 fetched 2026-10-03).

## Change

1. **Add `profile_id` to every memory method.** Add `profile_id: Option<String>` to `MemoryOverviewParams`, `MemoryEntityParams`, `MemorySearchParams`, `MemoryLoadParams` and `MemoryIngestParams` (`octos-core/src/ui_protocol.rs:3552-3680`).
   - When it is set, the server checks `is_authorized_for_profile(identity, profile_id)` and refuses a failure with `-32003` / 403 and `{kind: forbidden}`.
   - When it is absent, today's identity rule stays, so the change is additive.
   - Host scoping keeps precedence: a tenant subdomain fixes the profile, and a different `profile_id` is refused.
2. **Echo the answering profile.** Every memory result echoes `profile_id`, the profile that actually answered:
   - `MemoryOverviewResult {profile_id, overview}`;
   - the other results gain the same top-level field.

   A client can then name the scope truthfully and detect an older server that ignored the parameter (no `profile_id` in the reply).
3. **Gate it.** Put the change behind the existing `auxiliary.rest_to_ws.v1` gate. There is no new feature flag, because an unknown parameter is already ignored by older servers and the echo tells the client which behaviour it got.

## Tests

- `memory_overview_reads_the_named_profile`: two profiles with different `MEMORY.md`; the admin identity reads each by `profile_id`.
- `memory_methods_refuse_an_unauthorized_profile`: a user identity naming another user's profile gets `forbidden`.
- `memory_results_echo_the_profile`: each method's result carries the answering `profile_id`.
- `memory_search_uses_the_named_profile_runtime`: search on `dsflash` succeeds where the identity's own profile has no runtime.

## Client behaviour (built, A36b — the operator chose D2 A: Memory is for the Session's profile)

The native client already speaks the proposal (`crates/octoscode-module/src/screens/board3/memory.rs`):

- Every `memory/*` call names the Session's profile (`conv.profile()`, the same value Skills use): `{profile_id}` on
  `memory/overview`, `{profile_id, query, kinds?, limit: 20}` on `memory/search`, `{profile_id, id}` on `memory/load`,
  `{profile_id, name}` on `memory/entity`, `{profile_id, records: [...]}` on `memory/ingest`.
  - a6ea8505 accepts and ignores the parameter (re-probed on a private serve, 2026-10-03: the same replies as without
    it, no `-32602`), so sending it is safe today.
- A result is shown as the profile's memory only when it echoes that `profile_id`. The scope line reads
  "Server Profile: dsflash" (plain text, the muted ink — D1).
- An older server (no echo, today's octos): the dialog shows the refusal with bounded copy — "Couldn't read memory." /
  "This server reads memory for the account you signed in with, not for dsflash." / "Update octos to a version that
  reads memory per profile." — never the server's raw message (D1), never the account's memory presented as the
  profile's, and no "Add note" (a note must never land in memory the server cannot attribute). A search refused with
  `runtime_unavailable` reads the same way.
- An echo naming another profile: "The server answered for <p>, not for dsflash."
- Once the echo matches, `runtime_unavailable` means the profile itself has no runtime: "The server isn't running
  dsflash yet, so its memory isn't available." / "Add a model provider for this profile, then try again."

octos main dde76555 (fetched 2026-10-03) still resolves every `memory/*` call through `resolve_my_profile_id`
(`resolve_memory_profile_runtime`, `ui_protocol_transport.rs:32958`); the memory params and results are unchanged.
