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
//! saved-link panel; the matrix-cited resume card is `a20_resume_workspace.rs`.
//! Both paths:
//! * the matrix-cited resume card (`screens::sessions`, `Effect::ResumeConfirm`)
//!   — main's API only, so this file's first test is the failing-first proof;
//! * the saved-link panel (`screens::saved_link`, the board-3 dialog the app
//!   offers for `OCTOSCODE_SESSION_LINK`) — its "Open conversation" job, run by
//!   the board-3 host exactly as the click does (`host::perform` ->
//!   `host::run`).
mod a20_common;

use std::time::Duration;

use a20_common::*;
use serde_json::json;

/// The saved-link panel: a link whose workspace the server resolves to
/// another folder is refused BEFORE the open (the catalog read attests the
/// canonical root; nothing is opened, no history is read), the refusal is on
/// the panel and the link stays; the exact workspace then opens.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_saved_link_whose_workspace_differs_is_refused_before_open_or_history() {
    use octoscode_module::screens::board3::host::{self, Job, Outcome};
    use octoscode_module::screens::saved_link;
    let _g = state_lock();
    saved_link::reset();
    let server = Server::start(Script {
        catalog: true,
        attest: vec![(LINK_WS.to_owned(), REAL_WS.to_owned())],
        ..Default::default()
    })
    .await;
    let (conv, events) = connect_in(&server, MAIN, REAL_WS).await;
    drain(&conv, events);

    // The link names a workspace the server resolves elsewhere (a moved or
    // symlinked folder).
    saved_link::set_link(&format!(r#"["{LINK_WS}","{PROFILE}","{XRAY}"]"#), &server.base);
    assert_eq!(host::perform("b3.link.open", 0, &conv.store), Outcome::Spawn(Job::SavedLinkOpen));
    let refused = host::run(Job::SavedLinkOpen, &conv).await;
    assert!(refused.is_err(), "{refused:?}");
    assert!(
        server.seen.lock().unwrap().iter().any(|(m, p)| m == "session/list" && p["cwd"] == json!(LINK_WS)),
        "the precondition read the server's attestation of the link's workspace"
    );
    assert!(server.of("session/open", XRAY).is_empty(), "no session/open for the refused link");
    assert!(server.of("session/hydrate", XRAY).is_empty(), "and no history request");
    let st = saved_link::snapshot();
    assert!(st.reference.is_some(), "the link stays");
    let e = st.error.expect("the panel says why");
    assert!(e.lead.contains("different folder") && e.detail.contains(REAL_WS), "{e:?}");
    assert_eq!(conv.store.active_session().as_deref(), Some(MAIN));

    // The exact workspace opens (and only then is history read).
    saved_link::set_link(&format!(r#"["{REAL_WS}","{PROFILE}","{XRAY}"]"#), &server.base);
    assert_eq!(host::perform("b3.link.open", 0, &conv.store), Outcome::Spawn(Job::SavedLinkOpen));
    host::run(Job::SavedLinkOpen, &conv).await.expect("the exact saved workspace opens");
    assert_eq!(server.of("session/open", XRAY).len(), 1);
    assert_eq!(server.of("session/open", XRAY)[0]["cwd"], json!(REAL_WS), "opened in the link's own workspace");
    for _ in 0..40 {
        if !server.of("session/hydrate", XRAY).is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(!server.of("session/hydrate", XRAY).is_empty(), "history is read for the accepted open");
    assert_eq!(saved_link::snapshot().raw, None, "opened: the link is consumed");
    assert_eq!(conv.store.active_session().as_deref(), Some(XRAY));
}

/// A link naming a Session this app already confirmed elsewhere is refused
/// before the open even when the server offers no catalog attestation; and
/// when only the open can tell (the server answers another root), the open
/// reply refuses it before any history read and the previous Session returns.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_known_or_server_confirmed_foreign_workspace_never_reaches_history() {
    use octoscode_module::screens::board3::host::{self, Job};
    use octoscode_module::screens::saved_link;
    let _g = state_lock();
    saved_link::reset();
    // No catalog: nothing attests a workspace before the open.
    let server = Server::start(Script::default()).await;
    let (conv, events) = connect_in(&server, MAIN, REAL_WS).await;
    drain(&conv, events);
    conv.open_session(XRAY, Some(OTHER_WS.to_owned())).await.expect("open");
    for _ in 0..40 {
        if conv.store.domains.session.workspace_root(XRAY).as_deref() == Some(OTHER_WS) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    conv.open_session(MAIN, Some(REAL_WS.to_owned())).await.expect("open");
    tokio::time::sleep(Duration::from_millis(300)).await;
    let opens = server.of("session/open", XRAY).len();
    let reads = server.of("session/hydrate", XRAY).len();

    // Known elsewhere: refused before any open.
    saved_link::set_link(&format!(r#"["{LINK_WS}","{PROFILE}","{XRAY}"]"#), &server.base);
    host::perform("b3.link.open", 0, &conv.store);
    assert!(host::run(Job::SavedLinkOpen, &conv).await.is_err());
    assert_eq!(server.of("session/open", XRAY).len(), opens, "no session/open");
    assert_eq!(server.of("session/hydrate", XRAY).len(), reads, "no history");
    assert!(saved_link::snapshot().error.unwrap().detail.contains(OTHER_WS));

    // Only the open can tell: the server answers another root for a Session
    // this app never confirmed — refused before any history read.
    let fresh = "a20:api:fresh";
    server.script.lock().unwrap().open_root = Some((fresh.to_owned(), OTHER_WS.to_owned()));
    saved_link::set_link(&format!(r#"["{LINK_WS}","{PROFILE}","{fresh}"]"#), &server.base);
    host::perform("b3.link.open", 0, &conv.store);
    let r = host::run(Job::SavedLinkOpen, &conv).await;
    assert_eq!(r, Err(saved_link::DIFFERENT_WORKSPACE.to_owned()), "the web's own copy");
    assert_eq!(server.of("session/open", fresh).len(), 1, "the one open the server answered elsewhere");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(server.of("session/hydrate", fresh).is_empty(), "no history read for it");
    assert_eq!(conv.store.active_session().as_deref(), Some(MAIN), "the previous Session is back");
}
