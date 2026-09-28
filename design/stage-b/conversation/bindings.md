# Stage B — binding list (first batch: the 5 live-gate components)

Every binding id the 5 cards declare, its type, and the store query or client action it needs.
Machine-readable twin: [`bindings.json`](bindings.json). **Rule 8.8 condition 2:** a card binds only to
a declared data id / action id — it never sees a Rust type. Card **#12** wires these in
`crates/octoscode-module/src/bindings.rs` (one `query` arm per data id, one `action` arm per action id).

Scenes in this batch: **1 THREAD LIST**, **3 STREAMING TURN**, **4 TOOL CELLS**, **8 COMPOSER STATES**,
**9 COMPLETED ANSWER**. Scenes 5/6 rows are marked *out of batch* (declared for completeness only).

## Data bindings (card reads → JSON)

| Binding id | Type | Card / node | Store query (card #12) | Notes |
| --- | --- | --- | --- | --- |
| `threads` | list | `conversation-01` / `thread_list` | `Store::sessions()` → `{id,title,message_count,active_turn}[]` | The reference's 5 rows are exactly `session/list`'s `SessionInfo` shape. |
| `threads[].title` | text | `conversation-01` / `thread_1_title … thread_5_title` | `Store::sessions()[i].title` | Copy: *Fix steer queue drop on reconnect*, *Add session fork*, *Review PR #2566*, *Bump octos-core to a6ea8505*, *Why is hydrate slow?* |
| `threads[].active` | bool | `conversation-01` / `thread_1` (highlight) | `Store::active_session() == sessions[i].id` | Drives the selected row's highlight. |
| `threads.active` | text | `conversation-01` | `Store::active_session()` | The active session id (used by the actions below). |
| `turn.active` | bool | `conversation-03`, `conversation-08` | `Store::sessions()[active].active_turn` | Chooses STOP (live) vs send (idle) composer button. |
| `timeline.entries` | list | `conversation-03`, `conversation-09` | `Store::timeline(active)` → `TimelineEntry[]` | `TextDelta{text,turn_id}` \| `TurnStarted` \| `TurnEnded`. |
| `timeline.entries[].text` | text | `conversation-03` / `assistant_1..4`; `conversation-09` / `answer_lead`, `bullet_*` | `Store::timeline(active)[i].text` | |
| `timeline.entries[].kind` | text | `conversation-03` | `Store::timeline(active)[i].kind` | Picks user bubble vs activity row vs tool row. |
| `turn.activity` | text | `conversation-03` / `activity` ("Working · 12s") | `Store::live_text(active)` + elapsed | "12s" is elapsed turn time, not a store field. |
| `composer.draft` | text | `conversation-03`,`conversation-08` / `composer_input`, `active_draft` | session draft | TextInput value; its `behavior.event='changed'` updates it. |
| `composer.placeholder` | text | `conversation-08` / `idle_placeholder` | static copy ("Ask Octos anything") | Empty-state copy, not store data. |
| `tools` | list | `conversation-04` / `tool_rows` (`tool_1..3`) | `Store::seen` / `tool/*` notifications → `{name,summary,status}[]` | `tool/started`, `tool/progress`, `tool/completed`. |
| `tools[].summary` | text | `conversation-04` / `tool_*_summary` | tool summary text | |
| `tools[].status` | text | `conversation-04` / row status | `tool/completed` vs `tool/progress` | Right-hand ✓ / spinner. |
| `tools[].expanded` | bool | `conversation-04` / `tool_3_output` | UI-local disclosure toggle | Not store data. |
| `tool.output` | list | `conversation-04` / `out_running`,`out_test`,`out_result` | tool output lines | |
| `answer.worked_for` | text | `conversation-09` / `worked_label` ("Worked for 3m 4s ›") | turn duration | From `turn/started` → `turn/completed`. |
| `answer.timestamp` | text | `conversation-09` / `answer_timestamp` ("Sep 28, 9:41 PM") | `turn.completed.at` | |

## Action bindings (card sends → host performs)

| Action id | Card / node | Protocol method (card #12) | Params |
| --- | --- | --- | --- |
| `thread.open` | `conversation-01` / `thread_1..5` | `session/open` | `{session_id: row.id}` |
| `thread.new` | `conversation-01` / `new_chat` | `session/open` | `{new: true}` |
| `turn.interrupt` | `conversation-03`,`conversation-08` / `composer_stop` | `turn/interrupt` | `{session: threads.active}` |
| `composer.submit` | `conversation-08` / send button | `turn/start` | `{session, text: composer.draft}` |
| `composer.steer` | `conversation-08` / `queued_row` ("1 queued · Steer now") | `turn/steer` | `{session, text: composer.draft}` |
| `tool.toggle` | `conversation-04` / `tool_1..3` | UI-local expand | — |
| `turn.expand` | `conversation-09` / `worked_row` | UI-local expand | — |
| `session.refresh` | `conversation-01` | `session/list` | already declared in `bindings.rs` |

## Out of batch (declared only)

| Binding id | Type | Scene | Store query |
| --- | --- | --- | --- |
| `approval.pending` | bool | 5 (INLINE APPROVAL, not in this batch) | `approval/requested` seen |
| `question.pending` | bool | 6 (USER QUESTION, not in this batch) | `user_question/requested` seen |

## Grounding

- Existing bindings to extend: `conn.state`, `conn.live`, `session.count`, `session.list`, `session.active`,
  `caps.count`, `summary` (`crates/octoscode-module/src/bindings.rs:17-25`); existing action `session.refresh`.
- Store accessors available today: `Store::{sessions,session_count,active_session,timeline,live_text,connection,is_live,capabilities,summary}`
  (`crates/octoscode-store/src/lib.rs:64-190`).
- Protocol methods (octos `4231669`, `crates/octos-core/src/ui_protocol.rs` + web `packages/client/src`):
  `session/open`, `session/fork`, `session/list`, `session/messages_page`, `turn/start`, `turn/interrupt`,
  `turn/steer`, `approval/respond`, `user_question/respond`, `queue/state`, `plan/updated`, `tool/status/list`,
  `profile/llm/list`, `profile/llm/select`, `review/start`.

## Unverified / inferred

- The JSON row shape for `tools` is inferred from the `tool/*` notifications, not read from a live store call.
- Elapsed-time fields repaint from `turn/started` → `turn/completed` timestamps; there is no single store field for them.
- Cards never see a Rust type: ids marked UI-local need no store arm.
