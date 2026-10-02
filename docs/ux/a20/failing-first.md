# A20 failing-first runs (the new production-path tests against main's crate sources, main b1fe23fb)

Method: `git checkout main -- crates/*/src` (the new test files kept), `cargo test -p octoscode-module --test <file>`,
then `git checkout HEAD -- crates`.

## Row 250 — `crates/octoscode-module/tests/a20_interaction_origin.rs` (main's APIs only): 0/5 on main, 5/5 on the branch

```
test switching_sessions_never_shows_or_answers_another_sessions_approval ... FAILED
test a_restore_admits_only_the_askers_well_formed_negotiated_entries ... FAILED
test each_session_keeps_and_answers_only_its_own_question ... FAILED
test a_response_on_a_replaced_socket_fails_closed_until_restored ... FAILED
test a_foreign_topic_request_never_takes_its_base_session_over ... FAILED
thread 'switching_sessions_never_shows_or_answers_another_sessions_approval' panicked at crates/octoscode-module/tests/a20_interaction_origin.rs:403:5:
assertion `left == right` failed: Y's keyboard has nothing to answer
  left: Some("01a0e773-f844-7d50-b171-b38d159f20a1")
 right: None
thread 'a_restore_admits_only_the_askers_well_formed_negotiated_entries' panicked at crates/octoscode-module/tests/a20_interaction_origin.rs:509:5:
the well-formed parked approval of X is restored, attributed to X: []
thread 'each_session_keeps_and_answers_only_its_own_question' panicked at crates/octoscode-module/tests/a20_interaction_origin.rs:440:5:
assertion `left == right` failed: X still waits on ITS question
  left: Idle
 right: Waiting
thread 'a_response_on_a_replaced_socket_fails_closed_until_restored' panicked at crates/octoscode-module/tests/a20_interaction_origin.rs:566:5:
a stale record is never answered
thread 'a_foreign_topic_request_never_takes_its_base_session_over' panicked at crates/octoscode-module/tests/a20_interaction_origin.rs:479:5:
assertion `left == right` failed: X never shows its topic child's request
  left: Some(Approval("01a0e773-f844-7d50-b171-b38d159f20a1"))
 right: None
test result: FAILED. 0 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.49s
```

The same bug in the real app (main's build, `tools/walk/a20_interaction_walk.py desktop`, `before/`): pressing
Y / S / N in Session Y sent `approval/respond {session_id: a20:api:yankee, approval_id: <X's>}` three times
(`before/main-desktop-wire.jsonl`), and the sidebar marked Y waiting instead of X (`before/main-desktop-03-sidebar.png`).

## Row 247 — `crates/octoscode-module/tests/a20_resume_workspace.rs` (main's APIs only): FAILED on main, passes on the branch

```
test a_resume_candidate_confirmed_in_another_workspace_is_refused_before_any_open ... FAILED
thread 'a_resume_candidate_confirmed_in_another_workspace_is_refused_before_any_open' panicked at crates/octoscode-module/tests/a20_resume_workspace.rs:61:5:
assertion `left == right` failed: no session/open for the refused candidate: Some(Object {"profile_id": String("a20"), "session_id": String("a20:api:xray")})
  left: 2
 right: 1
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
```

(main opened the candidate folder-less — no `cwd` at all.) `crates/octoscode-module/tests/a20_saved_link.rs`
exercises the new saved-link panel; on main it does not compile (the surface does not exist):

```
error[E0432]: unresolved import `octoscode_module::screens::saved_link`
error[E0599]: no variant or associated item named `SavedLinkOpen` found for enum `octoscode_module::screens::board3::host::Job` in the current scope
```
