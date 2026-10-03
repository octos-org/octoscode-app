# Native extras (no web counterpart)

Capabilities the native OctosCode app has that the TypeScript web client does not, so they have no row in
`docs/parity-matrix.csv` (whose rows are the web's capabilities). Each entry names the native code, the tests, the
click walk and the UX evidence, the way a parity row's evidence does.

| Extra | Since | Native | Tests | Click walk | UX evidence |
|---|---|---|---|---|---|
| **Settings > Capabilities** — one Settings section (after Model) whose rows open Skills, the MCP status and Memory by a click; each row only when the server advertises the method its surface reads (`profile/skills/list`, `mcp/status/list`, `memory/overview`), a note when none is. (The Skills dialog itself is the web's: parity rows 9-15, and row 284 for the web's Settings entry, `ProfileExtensionsDialog`; the section, its MCP row and its Memory row are native.) | A36 / A36b (board 5 frames 1, 3; operator D3) | `crates/octoscode-module/src/screens/settings.rs` `CAPABILITY_ROWS` / `capability_rows`; `crates/octoscode-module/src/chrome.rs` `sec_capabilities`, `set_nav_capabilities`, `set_rail_capabilities`, `set_cap_{skills,mcp,memory}` and their visibility in `sync` | `crates/octoscode-module/tests/a36_capabilities.rs` (5), `tests/a36_memory.rs` `capabilities_offers_memory_only_when_the_server_advertises_it` | `tools/walk/a36_capabilities.py` (main / none / dark / zh, desktop + phone) | `docs/ux/a36/capabilities/`, `docs/ux-scores.csv` area `a36` |
| **The MCP view's management note** — "MCP servers are configured on the server. This app shows the status the server reports and can't add or remove servers." (octos has no MCP management method; `mcp/status/list` answers a hard-coded empty list, `docs/proposals/mcp-management.md`) | A36 | `crates/octoscode-module/src/screens/board3/inventory.rs` `MCP_MANAGED_ON_SERVER`, `servers_section` (`b3_inv_mcp_note`) | `tests/a36_capabilities.rs` `the_mcp_view_says_servers_are_configured_on_the_server_and_offers_no_management` | `tools/walk/a36_capabilities.py` main | `docs/ux/a36/capabilities/*/main/03-mcp-from-settings-*.png` |
| **Memory** (board 5, signed off D1 yes / D2 A / D4) — the Session profile's memory: the overview (long-term memory, today, the last week's notes, the entity bank, the staging line), search with the kind filter, a hit's record (untrusted content as data with its callout), an entity page, the full long-term memory with the server's truncation notice, Add note (one untrusted `doc:octoscode:` record) with its receipt; the empty, loading and refused states; the app theme; zh. The web has no memory UI. | A36b | `crates/octoscode-module/src/screens/board3/memory.rs` (+ `board3/host.rs` `Dialog::Memory`, the `Job::Memory*` jobs) | `crates/octoscode-module/tests/a36_memory.rs` (20) | `tools/walk/a36_memory.py` (main, truncated, recent, empty, loading, today, notrunning, dark, zh; desktop + phone); live: `tools/walk/a36_live.py` | `design/stage-b/phase4-new5/` (the Stage B cards), `docs/ux/a36/memory/`, `docs/ux/a36/live/`, `docs/ux-scores.csv` area `a36` |

## Memory and the server (why the live server refuses)

octos a6ea8505 and main dde76555 resolve every `memory/*` call to the signed-in identity's profile (`admin` for the
server token), not to the Session's profile, and ignore a `profile_id` parameter. The app is built for the Session's
profile (operator decision D2 A): it names the profile on every call and shows a reply as that profile's memory only
when the reply says it answered for it — the echo the upstream proposal adds (`docs/proposals/memory-profile-scope.md`).
Until octos takes the proposal, the live server's memory is refused honestly (problem + next step, never the raw
error, no Add note). The proposal's world is exercised against `replay_serve --scenario memory` (octos' own reply
shapes + the echo; `crates/octoscode-client/tests/fixtures/a36-memory-proposal-synthetic.jsonl`), today's world against
the RECORDED replies of a private serve (`a36-memory-a6ea8505.jsonl`) and live (`tools/walk/a36_live.py`).
