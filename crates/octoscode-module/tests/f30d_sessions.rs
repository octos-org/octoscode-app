//! Card #30d — board 3.8 Resume / 3.9 Attachments / 3.10 Side question: the
//! screens table's contract, the one-owner rule, and the wire.
//!
//! §1 stages + confirms a resume on REAL recorded-traffic shape: the fake
//! server answers `session/list` with the recording's row grammar
//! (`SessionListRow`: id/title/message_count/updated_at/last_prompt/
//! active_turn — `domains/session.rs:38-57`) and the confirmed open leaves as
//! `session/open` (resume-binding.ts:227: never starts a turn).
//! §2 asks the aside on the wire (`session/btw`, btw.ts:77) — with the
//! advertisement present; the empty-draft refusal and the dismiss follow.
//! §3 proves the fail-closed gate (btw.ts:66): without the advertisement
//! nothing leaves the process.
//! §4 pins the one-owner rule: the six screen ids live ONLY here.
//! §5/§6: the attachment draft + the lowered live slots.

use std::sync::{Arc, Mutex};

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::sessions::{self, Effect as SessEffect};
use octoscode_store::Store;

/// The screens' state is process-global; the harness runs tests in parallel
/// threads — hold this across each test body (the f29d pattern).
static STATE_LOCK: Mutex<()> = Mutex::new(());

fn state_lock() -> std::sync::MutexGuard<'static, ()> {
    STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn ctx_with(caps: &[&str]) -> (Arc<Store>, Arc<Mutex<FlowUi>>) {
    let store = Arc::new(Store::new());
    store.set_connection("Live".into(), true);
    store.set_capabilities(caps.iter().map(|s| s.to_string()).collect());
    let ui = Arc::new(Mutex::new(FlowUi::default()));
    (store, ui)
}

/// The recorded traffic's row grammar (the fields `into_sessions` folds).
fn row(id: &str, title: &str, turns: usize, when: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id, "title": title, "message_count": turns,
        "updated_at": when, "last_prompt": null, "active_turn": false
    })
}

/// The minimal fake server (the f26/f29d shape): a hand-built handshake
/// (`session/open` has no response frame to look up in the recordings — the
/// recorded frame is the REQUEST), the recording's `session/list` reply
/// grammar, and a record of every outbound method.
struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
    btw_advertised: bool,
}

impl ReplayServer {
    /// The default (well-formed) `SessionBtwResult` the recording-derived
    /// shape carries. #P4e2a: `start_with_btw` swaps it for a hostile reply
    /// to prove the admission rule (btw.ts:36-51).
    fn default_btw_result() -> serde_json::Value {
        serde_json::json!({
            "session_id": "dsflash:main",
            "answer": "It's a metric that increments on reconnect drops.",
            "model": null
        })
    }

    async fn start(btw_advertised: bool) -> Self {
        Self::start_with_btw(btw_advertised, Self::default_btw_result()).await
    }

    async fn start_with_btw(btw_advertised: bool, btw_result: serde_json::Value) -> Self {
        use futures_util::{SinkExt, StreamExt};
        use tokio::net::TcpListener;
        use tokio_tungstenite::tungstenite::Message;

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (tx, mut rx_in) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
            let mut features = vec![
                "projection.envelope.v2".to_owned(),
                "state.session_hydrate.v1".to_owned(),
            ];
            if btw_advertised {
                features.push("session/btw".to_owned());
            }
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                rx.lock().unwrap().push(method.clone());
                let frame = match method.as_str() {
                    "session/open" => {
                        let session = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or("dsflash:main")
                            .to_owned();
                        serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"opened": {
                                "session_id": session,
                                "active_profile_id": "dsflash",
                                "cursor": {"stream": "dsflash:main", "seq": 1},
                                "capabilities": {
                                    "version": {"protocol": "octos-ui/v1alpha1",
                                                "schema_version": 1, "jsonrpc": "2.0"},
                                    "capabilities_schema_version": 1,
                                    "supported_methods": ["session/open", "session/list",
                                                          "session/btw"],
                                    "supported_notifications": ["projection/envelope"],
                                    "supported_features": features
                                }
                            }}
                        })
                    }
                    "session/list" => {
                        // The recording's grammar (r22-live / r3-session rows).
                        serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"sessions": [
                                row("dsflash:fork", "Add session fork", 14, "2h ago"),
                                row("dsflash:steer", "Fix steer queue drop on reconnect", 23, "1d ago"),
                                row("dsflash:pr", "Review PR #2566", 17, "3d ago")
                            ]}
                        })
                    }
                    "session/btw" => {
                        // The recording has no session/btw frame (no fixture
                        // carries one — #30d recon), so the reply follows the
                        // octos-core shape (`SessionBtwResult`: session_id,
                        // answer, model? — ui_protocol.rs:3106).
                        serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": btw_result.clone()
                        })
                    }
                    _ => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {}}),
                };
                let _ = tx
                    .lock()
                    .await
                    .send(Message::Text(frame.to_string().into()))
                    .await;
            }
        });
        Self { base_url: format!("http://{addr}"), received, btw_advertised }
    }

    fn saw(&self, method: &str) -> bool {
        self.received.lock().unwrap().iter().any(|m| m == method)
    }

    /// How many times `method` left the process (open_workspace sends
    /// session/open during connect — assertions must count, not boolean).
    fn count(&self, method: &str) -> usize {
        self.received.lock().unwrap().iter().filter(|m| *m == method).count()
    }
}

async fn connect_and_refresh(server: &ReplayServer) -> Conversation {
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    for _ in 0..40 {
        match tokio::time::timeout(std::time::Duration::from_millis(120), events.recv()).await {
            Ok(Some(evt)) => {
                let _ = conv.on_event(evt);
            }
            _ => break,
        }
    }
    conv
}

// ------------------------------------------- §1 resume: stage, then confirm

#[tokio::test]
async fn resume_stages_never_opens_and_confirm_opens_on_the_wire() {
    let _state = state_lock();
    sessions::reset_state();
    let server = ReplayServer::start(false).await;
    let conv = connect_and_refresh(&server).await;

    // The candidates folded from the recording's grammar.
    let ui_handle = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui_handle);
    let titles = sessions::query(&ctx, "resume.rows[].title").unwrap();
    let titles = titles.as_array().unwrap();
    // #34b: the list fold MERGES, so the reply's three recorded rows come
    // first and the seeded open session (dsflash:main, known to the tab)
    // is retained after them — the #39a live defect was exactly this row
    // being dropped by the old replace.
    assert_eq!(titles.len(), 4, "3 recorded rows + the retained open session");
    assert_eq!(titles[0], serde_json::json!("Add session fork"));

    // Staging NEVER opens (resume-binding.ts:152): connect's own session/open
    // already flew (open_workspace), so count — no NEW open may appear.
    let opens_before = server.count("session/open");
    let effect = sessions::resolve("resume.stage", 0, &ctx);
    assert!(matches!(effect, SessEffect::ResumeStage(0)), "got {effect:?}");
    assert_eq!(
        server.count("session/open"),
        opens_before,
        "a bare row must not open"
    );
    let confirm = sessions::query(&ctx, "resume.confirm").unwrap();
    assert_eq!(
        confirm,
        serde_json::json!("Resume \"Add session fork\"?"),
        "the confirm echoes the row's exact title"
    );

    // Confirm → the production open path (session/open on the wire, no turn).
    let effect = sessions::resolve("resume.confirm", 0, &ctx);
    let SessEffect::ResumeConfirm(id) = &effect else {
        panic!("confirm resolves to ResumeConfirm, got {effect:?}");
    };
    assert_eq!(id, "dsflash:fork");
    sessions::apply(effect, &conv).await.expect("the confirmed open");
    assert!(
        server.count("session/open") > opens_before,
        "the confirmed resume opens"
    );
    // The dialog cleared (UI-local) and the confirm line is empty again.
    let confirm = sessions::query(&ctx, "resume.confirm").unwrap();
    assert_eq!(confirm, serde_json::json!(""));

    // An out-of-range stage names the row: Unhandled, never a panic.
    let effect = sessions::resolve("resume.stage", 9, &ctx);
    let SessEffect::Unhandled(why) = effect else {
        panic!("an out-of-range row is Unhandled, got {effect:?}");
    };
    assert!(why.contains("resume.stage[9]"), "{why}");
}

// ------------------------------------------------- §2 the aside on the wire

#[tokio::test]
async fn aside_ask_sends_session_btw_and_the_answer_lands() {
    let _state = state_lock();
    sessions::reset_state();
    let server = ReplayServer::start(true).await; // advertised
    let conv = connect_and_refresh(&server).await;

    let ui_handle = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui_handle);
    ui_handle.lock().unwrap().set_draft_inner("What does steer_dropped mean?");

    let effect = sessions::resolve("aside.ask", 0, &ctx);
    let SessEffect::AsideAsk(question) = &effect else {
        panic!("the advertised ask resolves to AsideAsk, got {effect:?}");
    };
    assert_eq!(question, "What does steer_dropped mean?");
    sessions::apply(effect, &conv).await.expect("the aside call");
    assert!(server.saw("session/btw"), "the wire carried session/btw");

    let answer = sessions::query(&ctx, "aside.answer").unwrap();
    assert_eq!(
        answer,
        serde_json::json!("It's a metric that increments on reconnect drops.")
    );
    let state = sessions::query(&ctx, "aside.state").unwrap();
    assert_eq!(state, serde_json::json!("answered"));

    // Dismiss: the panel hides and a late answer has nowhere to land
    // (lazy-btw-controller.ts:110).
    let effect = sessions::resolve("aside.dismiss", 0, &ctx);
    assert!(matches!(effect, SessEffect::AsideDismiss));
    let state = sessions::query(&ctx, "aside.state").unwrap();
    assert_eq!(state, serde_json::json!("hidden"));
    let answer = sessions::query(&ctx, "aside.answer").unwrap();
    assert_eq!(answer, serde_json::json!(""));

    // An empty draft never consumes anything (lazy-btw-controller.ts:85).
    ui_handle.lock().unwrap().set_draft_inner("   ");
    let effect = sessions::resolve("aside.ask", 0, &ctx);
    let SessEffect::Unhandled(why) = effect else {
        panic!("an empty draft is Unhandled, got {effect:?}");
    };
    assert!(why.contains("empty"), "{why}");
}

/// #P4e2a — the aside's reply VALIDATION (btw.ts:36-51 `parseSessionBtwResult`,
/// the rule btw.ts:81-86 enforces). A reply is admitted only when it is a
/// record carrying THIS Session's id and a non-blank string `answer`; anything
/// else is `BtwProtocolError("Invalid or wrong-Session aside result")` and must
/// NEVER reach the panel as an answer. Exercised through the PRODUCTION path
/// (`resolve` → `apply` → `Conversation::client`), with the hostile reply
/// served by the same fake server. RED before the #P4e2a change: the old arm
/// read `v["answer"].as_str().unwrap_or_default()`, so a wrong-Session reply
/// rendered as `answered` with an EMPTY answer.
#[tokio::test]
async fn a_wrong_session_or_blank_aside_reply_is_rejected_as_a_protocol_error() {
    let _state = state_lock();

    // The pure admission rule, field by field, against the oracle.
    let ok = ReplayServer::default_btw_result();
    assert_eq!(
        sessions::parse_aside_result(&ok, "dsflash:main"),
        Ok("It's a metric that increments on reconnect drops.".to_owned())
    );
    // btw.ts:40 — the session_id must be the one the question went to.
    let foreign = serde_json::json!({
        "session_id": "dsflash:other", "answer": "hello", "model": null
    });
    assert!(sessions::parse_aside_result(&foreign, "dsflash:main").is_err());
    // btw.ts:42-43 — the answer must be a non-blank STRING.
    for blank in ["", "   "] {
        let v = serde_json::json!({"session_id": "dsflash:main", "answer": blank});
        assert!(
            sessions::parse_aside_result(&v, "dsflash:main").is_err(),
            "a blank answer {blank:?} is a protocol error"
        );
    }
    // A missing answer is not a valid string either.
    let no_answer = serde_json::json!({"session_id": "dsflash:main"});
    assert!(sessions::parse_aside_result(&no_answer, "dsflash:main").is_err());
    // btw.ts:43-45 — model is OPTIONAL, but a present one must be non-blank.
    let blank_model =
        serde_json::json!({"session_id": "dsflash:main", "answer": "hi", "model": "  "});
    assert!(sessions::parse_aside_result(&blank_model, "dsflash:main").is_err());
    let null_model =
        serde_json::json!({"session_id": "dsflash:main", "answer": "hi", "model": null});
    assert!(sessions::parse_aside_result(&null_model, "dsflash:main").is_ok());

    // …and the same rule end-to-end, on the wire, for each hostile shape.
    for (label, reply) in [
        (
            "wrong-Session",
            serde_json::json!({"session_id": "dsflash:other", "answer": "leaked", "model": null}),
        ),
        (
            "blank-answer",
            serde_json::json!({"session_id": "dsflash:main", "answer": "   ", "model": null}),
        ),
    ] {
        sessions::reset_state();
        let server = ReplayServer::start_with_btw(true, reply).await;
        let conv = connect_and_refresh(&server).await;
        let ui_handle = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui_handle);
        ui_handle.lock().unwrap().set_draft_inner("why?");

        let effect = sessions::resolve("aside.ask", 0, &ctx);
        let err = sessions::apply(effect, &conv)
            .await
            .expect_err(&format!("a {label} reply must be a protocol error"));
        assert!(
            err.contains("Invalid or wrong-Session aside result"),
            "{label}: {err}"
        );
        // The panel never shows it: the aside reports unavailable, and NO
        // answer is projected (the web raises before it can render one).
        assert_eq!(
            sessions::query(&ctx, "aside.state").unwrap(),
            serde_json::json!("unavailable"),
            "{label}: the panel reports unavailable (btw.ts:85)"
        );
        assert_eq!(
            sessions::query(&ctx, "aside.answer").unwrap(),
            serde_json::json!(""),
            "{label}: no answer is projected"
        );
    }
}

// ------------------------------------------------------ §3 fail closed

#[tokio::test]
async fn the_aside_fails_closed_without_the_advertisement() {
    let _state = state_lock();
    sessions::reset_state();
    let server = ReplayServer::start(false).await; // NOT advertised
    let conv = connect_and_refresh(&server).await;

    let ui_handle = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui_handle);
    ui_handle.lock().unwrap().set_draft_inner("why?");

    let effect = sessions::resolve("aside.ask", 0, &ctx);
    let SessEffect::Unhandled(why) = effect else {
        panic!("an unadvertised ask fails closed, got {effect:?}");
    };
    assert!(why.contains("not-advertised"), "{why}");
    let state = sessions::query(&ctx, "aside.state").unwrap();
    assert_eq!(state, serde_json::json!("unavailable"), "btw.ts:66");
    assert!(!server.saw("session/btw"), "nothing left the process");
}

// --------------------------------------------- §4 the one-owner rule (#29d3)

#[test]
fn board3_action_ids_are_owned_by_the_screen_not_the_conversation_tables() {
    for (id, _) in sessions::ACTIONS {
        assert!(sessions::is_action(id), "{id} must be owned here");
        assert!(
            !octoscode_module::bindings::ACTIONS
                .iter()
                .any(|(a, _)| *a == *id),
            "{id} leaked into the conversation bindings::ACTIONS"
        );
        assert!(
            !octoscode_module::actions::is_routed(id),
            "{id} leaked into the conversation router's ROUTED"
        );
    }
    // The conversation tables keep their pinned shapes (#29d3).
    assert_eq!(octoscode_module::bindings::ACTIONS.len(), 9);
    assert_eq!(octoscode_module::actions::unrouted(), vec!["answer.expand"]);
}

// ----------------------------------------------- §5 the attachment draft

#[test]
fn attachment_draft_counts_and_removes_like_the_web_draft() {
    let _state = state_lock();
    sessions::reset_state();
    sessions::seed_attachments(vec![("a.png", 1_258_291), ("b.png", 1_258_291)]);
    let (store, ui) = ctx_with(&[]);
    let ctx = Ctx::new(&store, &ui);

    // attachment-drafts.ts:6 — max 4 images / 20 MiB, the count line is a slot.
    let count = sessions::query(&ctx, "att.count").unwrap();
    assert_eq!(count, serde_json::json!("2 of 4 images • 20 MB max"));
    let sizes = sessions::query(&ctx, "att.sizes").unwrap();
    assert_eq!(sizes.as_array().unwrap().len(), 2);

    // Remove the first (the card's attachment.remove.1 control, index 0).
    let effect = sessions::resolve("attachment.remove", 0, &ctx);
    assert!(matches!(effect, SessEffect::AttachmentRemove(0)));
    let count = sessions::query(&ctx, "att.count").unwrap();
    assert_eq!(count, serde_json::json!("1 of 4 images • 20 MB max"));

    // Out of range names the control: Unhandled, never a panic.
    let effect = sessions::resolve("attachment.remove", 5, &ctx);
    let SessEffect::Unhandled(why) = effect else {
        panic!("an out-of-range remove is Unhandled, got {effect:?}");
    };
    assert!(why.contains("attachment.remove[5]"), "{why}");
    sessions::reset_state();
}

// ----------------------------------------------- §6 the lowered live slots

#[test]
fn lowered_screens_carry_the_live_slots() {
    let _state = state_lock();
    sessions::reset_state();
    let (store, ui) = ctx_with(&[]);

    // Idle resume: the store carries the four authored candidates, so the
    // per-item surgery keeps all four rows and the swaps are no-ops — the
    // render matches the Stage B card (an EMPTY store would cut the rows:
    // per-item, never the design's count).
    store.set_sessions(vec![
        octoscode_store::Session {
            id: "dsflash:fork".into(),
            title: Some("Add session fork".into()),
            message_count: 14,
            updated_at: Some("2h ago".into()),
            last_prompt: None,
            active_turn: false,
        },
        octoscode_store::Session {
            id: "dsflash:steer".into(),
            title: Some("Fix steer queue drop on reconnect".into()),
            message_count: 23,
            updated_at: Some("1d ago".into()),
            last_prompt: None,
            active_turn: false,
        },
        octoscode_store::Session {
            id: "dsflash:pr".into(),
            title: Some("Review PR #2566".into()),
            message_count: 17,
            updated_at: Some("3d ago".into()),
            last_prompt: None,
            active_turn: false,
        },
        octoscode_store::Session {
            id: "dsflash:hydrate".into(),
            title: Some("Why is hydrate slow?".into()),
            message_count: 9,
            updated_at: Some("4d ago".into()),
            last_prompt: None,
            active_turn: false,
        },
    ]);
    let idle = sessions::lower_screen("resume", &store).expect("resume lowers");
    assert!(idle.contains("text: \"Add session fork\""), "authored row 1");

    // Live rows: the store's sessions swap in (title + meta), the confirm line
    // appears once something is staged.
    store.set_sessions(vec![octoscode_store::Session {
        id: "dsflash:live".into(),
        title: Some("Live row".into()),
        message_count: 7,
        updated_at: Some("5m ago".into()),
        last_prompt: None,
        active_turn: false,
    }]);
    {
        let ctx = Ctx::new(&store, &ui);
        sessions::resolve("resume.stage", 0, &ctx);
    }
    let live = sessions::lower_screen("resume", &store).expect("resume lowers");
    assert!(live.contains("text: \"Live row\""), "the live title swapped in");
    assert!(live.contains("text: \"dsflash • 5m ago • 7 turns\""), "the live meta grammar (host = the id's profile prefix)");
    assert!(live.contains("text: \"Resume \\\"Live row\\\"?\""), "the staged confirm");
    // #34b: the fold merges, so the tab-known rows the reply omits are
    // RETAINED (the web's catalog only adds — App.tsx:747-756); the old
    // negative (authored copy replaced) encoded the replace that dropped a
    // running session on the real gate.
    assert!(live.contains("text: \"Add session fork\""), "the merge retains the tab-known rows");

    // Attachments: the count slot follows the draft.
    sessions::seed_attachments(vec![("a.png", 1_258_291)]);
    let live = sessions::lower_screen("attachments", &store).expect("attachments lowers");
    assert!(live.contains("text: \"1 of 4 images • 20 MB max\""));
    assert!(!live.contains("text: \"2 of 4 images"), "the authored count replaced");

    // The aside: question + answer swap in only when seeded.
    sessions::seed_aside("steer queue?", "a metric on reconnect drops.");
    let live = sessions::lower_screen("aside", &store).expect("aside lowers");
    assert!(live.contains("text: \"steer queue?\""));
    assert!(live.contains("a metric on reconnect drops."));
    assert!(!live.contains("Why is the steer queue dropping"), "the authored question replaced");
    sessions::reset_state();
}
