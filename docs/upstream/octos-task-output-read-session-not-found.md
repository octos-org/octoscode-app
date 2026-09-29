# octos: `task/output/read` returns "session not found" for a session `task/list` just served

**Seen on:** octos a6ea8505, `octos serve --solo`, dsflash profile (octoscode-app #24b, 2026-09-28).
**Evidence:** `crates/octoscode-client/tests/fixtures/c24b-subagent-a6ea8505.jsonl` in octoscode-app (hermetic, real frames).

## Steps
1. Open a session and run one turn that spawns a background sub-agent (`agent/updated` with `backend_kind=spawn_child_session`,
   `role=background_task`; `task/updated` with `tool_call_id=spawn-subagent-0`).
2. `task/list {session_id}` → returns the task row.
3. `task/output/read {session_id, task_id}` using the same `session_id` → **error `-32602 "session not found"`**.

## Expected
`task/output/read` returns the task's output for any task that `task/list` returned for that session.

## Suspected cause (unverified)
The two handlers resolve the session store from different data dirs (e.g. `--data-dir` vs `--instance-data-dir`). The reporter lane
did not read octos-cli source to confirm. Please check where `task/output/read` looks up sessions compared with `task/list`.
