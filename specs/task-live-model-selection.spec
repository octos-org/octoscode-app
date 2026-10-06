spec: task
name: "Allow next-turn model selection without inferred restarts"
tags: [model, session, native]
---

## Contract

The composer permits changing the profile model during an active response;
the selection applies to the next turn. A pending model save, an unavailable
model, or a missing server capability still disables selection. Permission
controls retain their existing turn lock.

Only an explicit `restart_required` flag or disposition produces a restart
warning. A saved profile default differing from a cached runtime or an active
response's model does not prove a restart is necessary. Explicit hints remain
authoritative even when model strings match (for example, a route change).

## Verification

- `restart_requires_an_explicit_server_hint`
- `the_restart_notice_requires_the_server_hint`
- `the_model_groups_are_the_webs`
- `the_model_seat_groups_selects_and_keeps_the_notice_board` sends a real client
  selection to the fake OUP server while the UI marks the turn busy.
