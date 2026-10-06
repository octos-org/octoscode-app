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

The composer runtime status refreshes when the selected model, provider, route,
or foreground turn changes. The runtime label comes from a checked server status
reply, not from the saved selection. Older replies cannot overwrite a newer
selection's status; an absent runtime clears the old label. The session settings
pane receives the same current status for its session.

## Verification

- `restart_requires_an_explicit_server_hint`
- `the_restart_notice_requires_the_server_hint`
- `the_model_groups_are_the_webs`
- `the_model_seat_groups_selects_and_keeps_the_notice_board` sends a real client
  selection to the fake OUP server while the UI marks the turn busy.
- `composer_runtime_refreshes_after_model_and_route_selection`
- `composer_runtime_rejects_status_from_before_the_switch`
- `composer_runtime_waits_for_live_status_and_refreshes_when_turn_finishes`
- `the_pane_shows_the_restart_truth_of_the_runtime_and_the_last_answer` checks
  explicit restart guidance and rejects an inferred warning when runtime and
  saved model differ after a successful reload, over the real client transport.
