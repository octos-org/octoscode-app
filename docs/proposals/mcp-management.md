# Proposal: MCP server management over the octos UI protocol

- **Status:** draft for the operator to submit upstream (octos). A36, 2026-10-03.
- **Scope:** octos `octos-cli` UI protocol (`api/ui_protocol_transport.rs`, `api/coding_tool_contract.rs`), the profile config, and the web client package.
- **Why:** The operator asked to manage skills, memories and MCP servers from the app. Skills can be managed over the protocol and memory has methods. **MCP servers cannot be managed at all**, and their status is not reported either.

## 1. What exists today

All citations are at the pinned octos `a6ea8505`. octos `main` (3916c6a8, 7 commits later) has the same code in every place cited.

| What | Where | Behaviour |
|---|---|---|
| Server config | profile `config.mcp_servers` (`profiles.rs:201`); serve/gateway `config.json` `mcp_servers` (`config.rs:110`); plugin manifests (`runtime/profile.rs:687`) | `Vec<McpServerConfig>`: `command`, `args`, `env`, `url`, `headers`, `oauth`, `scopes`, `concurrency_class` (`octos-agent/src/mcp.rs:53-90`). There is **no id field**; `display_name()` is the command or the URL. |
| Start | `runtime/profile.rs:1177-1182` | `McpClient::start(&config.mcp_servers)` at profile bootstrap, then `register_tools`. A failure is only a `warn!`; no status is kept. |
| OAuth | `octos mcp login|logout <url>` (`commands/mcp.rs`) | CLI only. The tokens go to the OS keyring. |
| Status method | `mcp/status/list` (`ui_protocol_transport.rs:273`, dispatch `:19399-19430`) | **Always answers `servers: []`**. `mcp_status_list_result` passes `servers: &[]` (`:11152-11161`), and the runtime policy stamp passes `mcp_servers: &[]` (`:11229`). |
| Status shape | `McpServerStatusView` (`coding_tool_contract.rs:211-219`) | `{id, display_name, transport, status, tool_count, tools, error?}` + `summary {connected, connecting, failed, disabled}` (`:641-648`, `:897-950`). |
| Write methods | none | No `mcp/*` method adds, edits, removes, enables or tests a server. |

Measured on a private serve (a copy of live-gate data, no model turn):

```
mcp/status/list {session_id, profile_id: "dsflash"}
  -> {"profile_id":"dsflash","session_id":"dsflash:a36probe","servers":[],"summary":{"connected":0,"connecting":0,"failed":0,"disabled":0}}
```

So a client cannot tell "no MCP servers configured" from "servers configured but not reported". The native app's MCP view says so honestly. It shows what the server reports, states that servers are configured on the server, and offers no add or remove.

## 2. Proposed methods

There are two steps. Step A is a bug fix and can ship alone.

### Step A: report real status (no new method)

- Keep the started `McpClient` per profile runtime, with one record per configured server: `{id, display_name, transport, status, tools, error, started_at}`.
- `status` takes the existing constants: `connecting`, then `connected`, or `failed` with `error` set; `disabled` when the config disables the server.
- `mcp/status/list` and the policy stamp's `mcp_servers` read that record instead of `&[]`.
- A server needs a **stable id**. Add an optional `id` to `McpServerConfig`. When it is absent, derive one from the URL host or the command's file name, plus a short hash of the full config. Clients key rows and writes on this id.

### Step B: management (behind a new UI feature `mcp.config.v1`)

The methods are gated strictly like `auxiliary.rest_to_ws.v1`: no negotiation means `method_not_supported`. Every method takes `profile_id`, which is authorized the way `profile/skills/*` are (`raw_profile_skill_profile_id`). The `config` objects mirror `McpServerConfig`. **Secret values are write-only:** `env` and `headers` values never come back; the server returns `{"name": {"set": true}}` for each one.

| Method | Params | Result |
|---|---|---|
| `mcp/server/list` | `{profile_id}` | `{profile_id, servers:[{id, source:"profile"|"server"|"plugin", editable, config:{command?, args, url?, env_keys:[…], header_keys:[…], oauth, scopes, concurrency_class?, enabled}, status:<McpServerStatusView>}]}`. `source` says where the entry lives; only `profile` entries are `editable`. |
| `mcp/server/test` | `{profile_id, config}` | `{ok, transport, tools:[name], error?}`. Starts the server, runs the MCP `initialize` and lists its tools, then shuts it down without saving. The 30 s handshake timeout of `mcp.rs:43` applies. |
| `mcp/server/upsert` | `{profile_id, id?, config, secrets?:{env?:{k:v}, headers?:{k:v}}, confirm:true}` | `{profile_id, server:<as in list>, restarted:true}`. A missing `id` adds a new server; an existing `id` replaces it. A secret key that is left out keeps its stored value. |
| `mcp/server/set_enabled` | `{profile_id, id, enabled}` | `{profile_id, id, enabled, status}` |
| `mcp/server/remove` | `{profile_id, id, confirm:true}` | `{profile_id, removed:id}` |
| `mcp/server/login` | `{profile_id, id}` | `{authorization_url}` for an `oauth` server. This is the WS twin of `octos mcp login`. The tokens still land in the server's keyring. The client opens the URL and the server's loopback redirect completes the flow. |
| notification `mcp/server/updated` | — | `{profile_id, server:<status view>}` on every status change, so status views stay live without polling. |

**Mutation semantics.** These mirror `profile/skills/install` (`:13137-13168`):

- one per-profile mutation lock (`profile_skill_mutation_locks` or its MCP twin) held across the whole sequence;
- the change is persisted to the profile config;
- the profile runtime is rebuilt (`rebuild_profile_runtime_after_skill_mutation`), so the MCP clients restart and the tools re-register;
- sessions are evicted, so the next turn sees the new tools.

## 3. Permission model

- **Who may write.** An identity authorized for the profile: `is_authorized_for_profile`, meaning the admin token, an admin user, the profile owner or its parent.
  - Session-ingress credentials are refused, as for every profile write (`validate_session_ingress_command_scope`).
  - `mcp/status/list` and `mcp/server/list` stay readable by anyone who can read the profile.
- **A stdio server runs a command on the server host.** That makes adding one equivalent to installing an executable skill. So:
  1. writes require `confirm: true` (the client must have shown a confirmation naming the command and the profile, like the Skills install confirm);
  2. stdio writes are refused unless the serve opts in (`mcp.allow_client_stdio = true`, default **false** on hosted/fleet deployments, true for `--solo` local);
  3. `command` must resolve to an absolute path or a name on the server's `PATH`. Shell strings are never accepted (args stay an array).
- **HTTP servers** keep the existing SSRF guard (`SsrfDnsResolver`, `mcp.rs:130-160`): private, loopback and link-local targets are refused, as today.
- **Secrets.** Never echoed back, never logged, and stored where the profile's other secrets are (the `env_vars` / keyring path). `mcp/server/list` shows only the key names.
- **Audit.** Each write appends `{at, identity, profile_id, id, action}` to the profile's audit log. The server never prompts its own agent for approval of a config write; the human confirmation is the client's.

## 4. What the clients would need

- **Native (OctosCode).**
  - Settings › Capabilities › MCP servers opens the inventory's MCP tab today.
  - With `mcp.config.v1` it gains per-row Edit, Disable and Remove (each asking first), an "Add server" form (transport choice; command and args, or URL; env and header keys with masked values; OAuth with a "Sign in" button), and "Test" before saving.
  - Rows go live through `mcp/server/updated`.
- **Web.**
  - `packages/client/src/mcp-methods.ts` with the six methods and the notification type, `InventoryDialog` grows the same actions, and `core-contract.ts` gains the feature constant.
  - The `profile-mutation-leases.ts` lease that the Skills dialog uses also covers MCP writes, so two tabs cannot race a rebuild.

## 5. Tests the octos change should carry

- `mcp_status_list_reports_configured_servers`: a profile with one stdio test server answers `connected` with its tools, and a broken command answers `failed` with `error`.
- Each write method is refused with `method_not_supported` when `mcp.config.v1` was not negotiated, and refused for a session-ingress credential.
- Upsert without `confirm` is refused, and stdio upsert is refused when the serve has not opted in.
- A secret value never appears in any result or in the trace log.
- Upsert rebuilds the runtime once, and a concurrent second upsert waits on the lock.
