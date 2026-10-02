//! A20 — parity row 247 (session:links-resume): "refuse a saved-link
//! candidate with a different workspace before requesting history".
//!
//! Web: `features/session/candidate-session.ts:230` (`validateCandidateWorkspace`
//! with `requireExactWorkspace`), `use-octos-session.ts:3068-3077` (a saved
//! link without a workspace is refused before any open), `App.tsx:1795-1830`
//! (`openSavedLink`); unit test `candidate-session.test.ts` "accepts the exact
//! saved workspace while fresh launches may canonicalize"; e2e
//! `session-links.spec.ts:254` "refuses a saved-link candidate with a
//! different workspace before requesting history".
//!
//! The real `Conversation` against a scripted fake AppUI server that logs
//! every request (the shared harness, `a20_common`). This file: the
//! matrix-cited resume card (main's API only — the failing-first proof).
//! The saved-link panel's tests are `a20_saved_link.rs`. Both paths:
//! * the matrix-cited resume card (`screens::sessions`, `Effect::ResumeConfirm`)
//!   — main's API only, so this file's first test is the failing-first proof;
//! * the saved-link panel (`screens::saved_link`, the board-3 dialog the app
//!   offers for `OCTOSCODE_SESSION_LINK`) — its "Open conversation" job, run by
//!   the board-3 host exactly as the click does (`host::perform` ->
//!   `host::run`).
mod a20_common;

use std::time::Duration;

use serde_json::json;

use a20_common::*;
use octoscode_module::bindings::Ctx;
use octoscode_module::screens::sessions::{self, Effect};

/// The matrix-cited path (`screens/sessions.rs` `Effect::ResumeConfirm`): a
/// candidate this app already confirmed in ANOTHER workspace than the one the
/// catalog was read for is refused before any `session/open` or
/// `session/hydrate`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_resume_candidate_confirmed_in_another_workspace_is_refused_before_any_open() {
    let _g = state_lock();
    sessions::reset_state();
    let server = Server::start(Script { catalog: true, ..Default::default() }).await;
    let (conv, mut events) = connect_in(&server, MAIN, REAL_WS).await;
    // The candidate lives in another workspace (an accepted open said so)…
    open_in(&conv, &mut events, XRAY, OTHER_WS).await;
    // …and the person is back in the main Session's workspace.
    open_in(&conv, &mut events, MAIN, REAL_WS).await;
    let (opens, reads) = (server.of("session/open", XRAY).len(), server.of("session/hydrate", XRAY).len());

    let ui = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui);
    let rows = sessions::query(&ctx, "resume.rows").unwrap();
    let index = rows.as_array().unwrap().iter().position(|r| r["id"] == json!(XRAY)).expect("the candidate is listed");
    sessions::resolve("resume.stage", index, &ctx);
    let effect = sessions::resolve("resume.confirm", 0, &ctx);
    assert!(matches!(&effect, Effect::ResumeConfirm(id) if id == XRAY), "{effect:?}");
    let refused = sessions::apply(effect, &conv).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    while let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(100), events.recv()).await {
        conv.on_event(evt);
    }
    // The wire first: nothing was opened and no history was requested for it.
    assert_eq!(
        server.of("session/open", XRAY).len(),
        opens,
        "no session/open for the refused candidate: {:?}",
        server.of("session/open", XRAY).last()
    );
    assert_eq!(server.of("session/hydrate", XRAY).len(), reads, "and no history request");
    assert!(refused.is_err(), "the candidate is refused: {refused:?}");
    assert_eq!(conv.store.active_session().as_deref(), Some(MAIN), "the Session on screen is untouched");
}
