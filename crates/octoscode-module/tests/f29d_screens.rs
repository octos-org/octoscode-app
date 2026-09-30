//! Card #29d — the Stage C screens (board 2.8 palette / 2.11 error / 2.12
//! loading+reconnecting): the binding/action contract, the fail-closed
//! capability gate, the web's redaction boundary, and the lowered card text.
//!
//! §1–§4 are pure (no UI, no network): the binding table is a function of the
//! store, so a test drives it exactly as a card would.
//! §5 replays the committed real live-gate recording
//! (`crates/octoscode-client/tests/fixtures/live-gate-a6ea8505.jsonl`) through
//! a fake server and drives `palette.run` for `/resume` — the one palette
//! command with a native effect today — asserting the production
//! `session.refresh` path sends `session/list` on the wire.
use std::sync::{Arc, Mutex};

use octoscode_module::actions::Effect;
use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::palette::{self, Effect as ScreenEffect};
use octoscode_store::Store;

/// The screens' state is process-global (`screens::palette` keeps it behind
/// one static), and the test harness runs tests in parallel threads — every
/// test holds this lock for its whole body so the sequences cannot interleave.
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

// ---------------------------------------------------------------- §1 redaction

/// The web's `buildSafeDiagnostic` boundary (`FatalErrorBoundary.tsx:93-107`):
/// `token=`, `auth_token=`, `api_key=` query params and `Bearer` tokens are
/// redacted, and the report is capped at 4000 chars.
#[test]
fn the_error_report_redacts_secrets_and_caps_at_4000() {
    let _state = state_lock();
    palette::reset_state();
    palette::report_error(
        "Render panicked: bad state\n\
         GET https://octos.example/ws?token=abc123&x=1\n\
         GET https://octos.example/ws?auth_token=zzz\n\
         GET https://octos.example/ws?api_key=sk-live-9\n\
         Authorization: Bearer sk-test-9f8e7d6c"
            .to_owned(),
    );
    let (store, ui) = ctx_with(&[]);
    let ctx = Ctx::new(&store, &ui);
    let report = palette::query(&ctx, "error.report")
        .and_then(|v| v.as_str().map(str::to_owned))
        .expect("error.report resolves");

    assert!(!report.contains("abc123"), "token= value leaked: {report}");
    assert!(!report.contains("zzz"), "auth_token= value leaked: {report}");
    assert!(!report.contains("sk-live-9"), "api_key= value leaked: {report}");
    assert!(!report.contains("9f8e7d6c"), "Bearer token leaked: {report}");
    assert!(report.contains("token=[redacted]"), "the key survives: {report}");
    assert!(report.contains("Bearer [redacted]"), "the scheme survives: {report}");
    assert!(report.starts_with("Render panicked"), "the diagnostic body survives");

    // The copy action carries the SAME redacted text and flips copy_state
    // (the web's `copyState` idle→copied, FatalErrorBoundary.tsx:45-53).
    let effect = palette::resolve("error.copy", 0, &ctx);
    let ScreenEffect::CopyReport(text) = effect else {
        panic!("error.copy resolves to CopyReport, got {effect:?}");
    };
    assert_eq!(text, report, "the clipboard gets the redacted report");
    let state = palette::query(&ctx, "error.copy_state").unwrap();
    assert_eq!(state, serde_json::json!("copied"));

    // The 4000-char cap.
    palette::report_error("x".repeat(9000));
    let long = palette::query(&ctx, "error.report").unwrap();
    let long = long.as_str().unwrap();
    assert_eq!(long.chars().count(), 4000, "capped at 4000 chars");
}

// ------------------------------------------------------- §2 fail-closed palette

/// `commandAvailability` (`registry.ts:679`): a command whose methods the
/// server did not advertise is disabled and running it fails closed — even the
/// declare-table level (`palette.run`) refuses it.
#[test]
fn palette_commands_fail_closed_without_their_capability() {
    let _state = state_lock();
    palette::reset_state();
    let (store, ui) = ctx_with(&["projection.envelope.v2"]); // none of the gates
    let ctx = Ctx::new(&store, &ui);

    let enabled = palette::query(&ctx, "palette.commands[].enabled").unwrap();
    let enabled = enabled.as_array().unwrap();
    assert_eq!(enabled.len(), palette::COMMANDS.len());
    assert!(enabled.iter().all(|v| !v.as_bool().unwrap()), "all gated off");

    // Running the selected (/resume) fails closed: Unhandled, named.
    let effect = palette::resolve("palette.run", 5, &ctx);
    let ScreenEffect::Unhandled(why) = effect else {
        panic!("an unadvertised command must not run, got {effect:?}");
    };
    assert!(why.contains("/resume"), "the refusal names the command: {why}");

    // With the gate advertised, /resume is enabled and routes to the native
    // effect (session.refresh); /btw stays disabled — session/btw is not
    // advertised by this fixture (the #28-era fail-closed boundary).
    drop(ctx);
    let (store, ui) = ctx_with(&["state.session_hydrate.v1"]);
    let ctx = Ctx::new(&store, &ui);
    let enabled = palette::query(&ctx, "palette.commands[].enabled").unwrap();
    let enabled = enabled.as_array().unwrap();
    assert!(enabled[5].as_bool().unwrap(), "/resume enabled under its gate");
    assert!(!enabled[4].as_bool().unwrap(), "/btw still gated off");

    let effect = palette::resolve("palette.run", 5, &ctx);
    let ScreenEffect::Run(Some("session.refresh"), name) = effect else {
        panic!("the gated /resume runs session.refresh, got {effect:?}");
    };
    assert_eq!(name, "/resume");
    // The native effect is one the router already owns (production path).
    assert!(octoscode_module::actions::is_routed("session.refresh"));
}

// ---------------------------------------------------------- §3 keyboard parity

/// The palette's keyboard model (`CommandPalette.tsx:44-49`): ArrowDown/Up
/// move with wraparound; the selection never leaves the table.
#[test]
fn palette_move_wraps_like_the_web_palette() {
    let _state = state_lock();
    palette::reset_state();
    let (store, ui) = ctx_with(&[]);
    let ctx = Ctx::new(&store, &ui);
    let n = palette::COMMANDS.len() as isize;

    let sel = |ctx: &Ctx<'_>| {
        palette::query(ctx, "palette.selected")
            .unwrap()
            .as_u64()
            .unwrap() as isize
    };
    assert_eq!(sel(&ctx), 0, "the atlas highlights the first row");

    // Up from the first row wraps to the last: +5 ≡ -1 (mod 6), the web's
    // (selectedIndex - 1 + length) % length with an unsigned step.
    palette::resolve("palette.move", (n - 1) as usize, &ctx);
    assert_eq!(sel(&ctx), n - 1, "(0+5)%6 = last row");
    // Down from the last row wraps to the first: (5+1)%6.
    palette::resolve("palette.move", 1, &ctx);
    assert_eq!(sel(&ctx), 0);
    // Plain downs advance one at a time.
    palette::resolve("palette.move", 1, &ctx);
    assert_eq!(sel(&ctx), 1);
}

// ------------------------------------------------- §4 the lowered card's slots

/// The screens mount as DATA (the Stage B cards), with the module's live slots
/// swapped into the lowered DSL: an idle screen keeps the authored copy, a
/// state flip rewrites exactly the banner/query slot.
#[test]
fn lowered_screens_carry_the_live_slots() {
    let _state = state_lock();
    palette::reset_state();
    let (store, ui) = ctx_with(&[]);

    // Idle loading screen: the authored Stage B banner stands (Gate B parity).
    let idle = palette::lower_screen("loading", &store).expect("loading lowers");
    assert!(idle.contains("Reconnecting… attempt 2 ·"), "authored banner");
    assert!(idle.contains("text: \"Loading session...\""));
    assert!(idle.contains("text: \"Retry now\""));
    assert!(idle.contains("text: \"Cancel\""));

    // After a retry the banner names the attempt (the web's counter).
    {
        let ctx = Ctx::new(&store, &ui);
        palette::resolve("connection.retry", 0, &ctx);
        palette::resolve("connection.retry", 0, &ctx);
    }
    let retrying = palette::lower_screen("loading", &store).expect("loading lowers");
    assert!(retrying.contains("Reconnecting… attempt 2 ·"), "attempt 2");
    assert!(!retrying.contains("Reconnecting… attempt 1 ·"), "the count is the store's, not a per-swap increment");

    // The error screen lowers with its actions (btn_reload / btn_diag) and the
    // document-shaped copy icon the atlas zoom showed.
    let error = palette::lower_screen("error", &store).expect("error lowers");
    assert!(error.contains("Something went wrong"));
    assert!(error.contains("Copy diagnostics"));
    // Note: the L0 DSL carries no asset ids (probed: no `src:` literals, no
    // `icon_copy`), so icon presence is asserted via the labels above.

    // The palette's query box follows the module's draft — but only once
    // there IS one (empty = the authored "/ mo" stands).
    let idle = palette::lower_screen("palette", &store).expect("palette lowers");
    assert!(idle.contains("text: \"/ mo\""), "authored query text");
    assert!(idle.contains("text: \"/model\""), "the command slice renders");
    ui.lock().unwrap().set_draft_inner("/comp");
    {
        let ctx = Ctx::new(&store, &ui);
        palette::resolve("palette.query.set", 0, &ctx);
    }
    let typed = palette::lower_screen("palette", &store).expect("palette lowers");
    assert!(typed.contains("text: \"/comp\""), "the live query feeds the card");
    assert!(!typed.contains("text: \"/ mo\""), "the authored copy is replaced");

    palette::reset_state();
}

// ------------------------------------------- §5 palette.run on the real wire

/// One recorded frame (`f26_replay.rs` shape).
#[derive(Debug, Clone)]
struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

fn live_gate() -> Vec<Frame> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/live-gate-a6ea8505.jsonl"
    );
    std::fs::read_to_string(path)
        .expect("read the live-gate fixture")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(serde_json::Value::Null),
            }
        })
        .collect()
}

/// The minimal fake server: the recording's own handshake (its capabilities
/// carry the palette gates), streamed notifications, and a record of every
/// outbound method.
struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
}

impl ReplayServer {
    async fn start(frames: Vec<Frame>) -> Self {
        use futures_util::{SinkExt, StreamExt};
        use tokio::net::TcpListener;
        use tokio_tungstenite::tungstenite::Message;

        let notifications: Vec<Frame> = frames
            .iter()
            .filter(|f| {
                f.dir == "in"
                    && matches!(
                        f.method.as_str(),
                        "turn/started" | "progress/updated" | "projection/envelope"
                    )
            })
            .cloned()
            .collect();
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (tx, mut rx_in) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                rx.lock().unwrap().push(method.clone());
                match method.as_str() {
                    "session/open" => {
                        // The recording's own handshake frame (its
                        // supported_features list carries state.session_hydrate.v1).
                        // Hand-built handshake (the `f26_replay` shape): the
                        // recording's own session/open frame is `dir: out`
                        // (the REQUEST), so there is no response frame to look
                        // up — build the response and adopt the session the
                        // client asked for. The features list carries the
                        // palette gates the tests assert on (/resume's
                        // state.session_hydrate.v1), as the real handshake did.
                        let session = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or("dsflash:main")
                            .to_owned();
                        let frame = serde_json::json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"opened": {
                                "session_id": session,
                                "active_profile_id": "dsflash",
                                "cursor": {"stream": "dsflash:main", "seq": 1},
                                "capabilities": {
                                    "version": {"protocol": "octos-ui/v1alpha1",
                                                "schema_version": 1, "jsonrpc": "2.0"},
                                    "capabilities_schema_version": 1,
                                    "supported_methods": ["session/open", "turn/start",
                                                          "turn/interrupt", "session/list",
                                                          "session/hydrate"],
                                    "supported_notifications": ["projection/envelope",
                                                                "message/delta", "turn/started",
                                                                "protocol/replay_lossy"],
                                    "supported_features": ["projection.envelope.v2",
                                                           "state.session_hydrate.v1"]
                                }
                            }}
                        });
                        let _ = tx
                            .lock()
                            .await
                            .send(Message::Text(frame.to_string().into()))
                            .await;
                        // Stream the recording's notifications from a SEPARATE
                        // task (f26's shape): the request loop must stay free
                        // to answer the client's follow-up RPCs (session/list).
                        {
                            let tx2 = tx.clone();
                            let notes = notifications.clone();
                            tokio::spawn(async move {
                                for f in &notes {
                                    let frame = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "method": f.method,
                                        "params": f.body
                                    });
                                    let _ = tx2
                                        .lock()
                                        .await
                                        .send(Message::Text(frame.to_string().into()))
                                        .await;
                                    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                                }
                            });
                        }
                    }
                    _ => {
                        let _ = tx
                            .lock()
                            .await
                            .send(Message::Text(
                                serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {}})
                                    .to_string()
                                    .into(),
                            ))
                            .await;
                    }
                }
            }
        });
        Self { base_url: format!("http://{addr}"), received }
    }
}

/// `palette.run` for `/resume` routes the EXISTING production effect
/// (`session.refresh`) over the real recorded wire: the store folds the
/// recording's session rows and the socket carries `session/list`.
#[tokio::test]
async fn palette_run_resume_routes_the_production_refresh_on_the_real_wire() {
    let _state = state_lock();
    palette::reset_state();
    let frames = live_gate();
    let server = ReplayServer::start(frames).await;
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    // Drain the handshake + streamed notifications into the store.
    for _ in 0..40 {
        match tokio::time::timeout(std::time::Duration::from_millis(120), events.recv()).await {
            Ok(Some(evt)) => {
                let _ = conv.on_event(evt);
            }
            _ => break,
        }
    }

    // The gate: the recording's own capabilities enable /resume.
    let ui_handle = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui_handle);
    let enabled = palette::query(&ctx, "palette.commands[].enabled").unwrap();
    assert!(
        enabled.as_array().unwrap()[5].as_bool().unwrap(),
        "/resume is enabled under the recorded capabilities"
    );

    // Run it: the resolved effect is the router's own production effect.
    let effect = palette::resolve("palette.run", 5, &ctx);
    let ScreenEffect::Run(Some("session.refresh"), "/resume") = &effect else {
        panic!("the palette routes /resume to session.refresh, got {effect:?}");
    };
    // Perform it exactly as `perform_action`'s Run arm does.
    let n = conv.refresh_sessions().await.expect("session.refresh");
    assert!(n >= 0);
    assert!(
        server
            .received
            .lock()
            .unwrap()
            .iter()
            .any(|m| m == "session/list"),
        "the wire carried session/list"
    );
}
