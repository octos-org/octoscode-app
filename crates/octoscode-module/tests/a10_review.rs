//! A10 — the native code review dialog as the web's `NativeReviewDialog`
//! (row: "distinguish a native server-owned review workflow from a diff
//! preview, rendering instructions inertly and never fabricating a prompt"),
//! through the production path: `dialog::lower(Dialog::Review)` (the web's
//! paragraphs + the "Review instructions (optional)" field), the
//! `review.start` resolve the dialog's Start routes, and
//! `screens::review::perform` against a WebSocket server answering
//! `review/start` with the octos-core `ReviewStartResult` shape (the
//! request's own turn echoed — no recording carries an accepted review: r5's
//! only `review/start` was refused, see f30a_review.rs).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::{dialog, review};

fn recorded_open() -> Value {
    let path = format!("{}/../octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .expect("r1")
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|f| f["dir"] == "in" && f["method"] == "session/open" && f["body"].get("active_profile_id").is_some())
        .map(|f| f["body"].clone())
        .expect("r1 open")
}

/// `review/start` answers by `mode`: the faithful receipt (the request's
/// turn echoed), a refusal (`accepted:false`), or another turn.
struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
    async fn start(modes: Vec<&'static str>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let open = recorded_open();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (mut tx, mut rx) = ws.split();
            let mut k = 0usize;
            let mut pos: BTreeMap<String, usize> = BTreeMap::new();
            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                log.lock().unwrap().push((method.clone(), v["params"].clone()));
                *pos.entry(method.clone()).or_insert(0) += 1;
                let p = &v["params"];
                let result = match method.as_str() {
                    "session/open" => json!({"opened": open}),
                    "review/start" => {
                        let mode = modes[k.min(modes.len() - 1)];
                        k += 1;
                        let turn = match mode {
                            "other-turn" => json!("01920000-0000-7000-8000-00000000ffff"),
                            _ => p["turn_id"].clone(),
                        };
                        json!({
                            "session_id": p["session_id"], "turn_id": turn, "accepted": mode != "refused",
                            "workflow": "code_review", "backend": "native", "agent_count": 3,
                        })
                    }
                    _ => json!({}),
                };
                let frame = json!({"jsonrpc": "2.0", "id": id, "result": result});
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { base_url: format!("http://{addr}"), seen }
    }

    fn sent(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }
}

async fn connect(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    let conv = Arc::new(conv);
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(None).await.expect("session/open");
    conv.store.domains.session.set_active(Some(conv.session_id()));
    conv.store.set_connection("Live".into(), true);
    conv
}

fn notes(conv: &Conversation, turn: &str) -> Vec<String> {
    conv.store
        .domains
        .session
        .timeline
        .entries(&conv.session_id())
        .into_iter()
        .filter(|e| e.turn_id.as_deref() == Some(turn))
        .map(|e| e.text)
        .collect()
}

/// Start with typed instructions (markup included): the dialog distinguishes
/// the workflow from a diff preview and offers the optional field; the
/// instructions ride `prompt` verbatim, trimmed; a fresh turn per start; the
/// Session records the request as text; the receipt folds and the dialog
/// closes; the review's own turn ending frees it.
#[tokio::test]
async fn a_native_review_is_a_new_turn_with_inert_instructions() {
    let _seq = review::test_lock();
    review::reset();
    let server = Server::start(vec!["ok", "ok"]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    let ui = Mutex::new(FlowUi::default());
    dialog::open(dialog::Dialog::Review);
    {
        let ctx = Ctx::new(&store, &ui);
        let m = dialog::lower(dialog::Dialog::Review, &ctx, 990.0, 603.0).expect("review");
        for want in [
            "Ready to review the current project changes.",
            dialog::REVIEW_NOT_A_PREVIEW,
            dialog::REVIEW_RESULTS_HERE,
            "Review instructions (optional)",
            "Leave empty to review the current project changes.",
            "Start native review",
        ] {
            assert!(m.dsl.contains(want), "the dialog shows {want:?}");
        }
        assert!(m.dsl.contains(dialog::REVIEW_PROMPT_INPUT), "the field is a real input");
        let events: Vec<&str> = m.taps.iter().map(|(_, e)| e.as_str()).collect();
        assert!(events.contains(&"review.start"), "{events:?}");
    }
    // Typed instructions: verbatim, trimmed, never interpreted.
    const TYPED: &str = "  <script>alert(1)</script> check the **parser**  ";
    review::set_prompt(TYPED);
    let effect = {
        let ctx = Ctx::new(&store, &ui);
        review::resolve("review.start", 0, &ctx)
    };
    let review::Effect::StartReview { turn_id, prompt, .. } = &effect else { panic!("admitted: {effect:?}") };
    let turn = turn_id.clone();
    assert_eq!(prompt.as_deref(), Some(TYPED.trim()));
    review::apply(&effect);
    assert_eq!(dialog::current(), None, "an admitted review closes the dialog");
    review::perform(effect, &conv).await.expect("accepted");
    let sent = server.sent("review/start");
    assert_eq!(
        sent[0],
        json!({"session_id": conv.session_id(), "turn_id": turn, "delivery": "inline", "prompt": TYPED.trim()}),
        "history.ts startReview: {{session_id, turn_id, delivery:'inline', prompt?}}"
    );
    assert_eq!(notes(&conv, &turn), [format!("Native code review: {}", TYPED.trim())], "recorded as text");
    assert_eq!(review::ui().agents, Some(3));
    assert_eq!(store.domains.review.last_review().map(|r| r.turn_id), Some(turn.clone()));
    // The review's own turn ends: a new one may start.
    review::note_envelope(&json!({"turn_id": turn, "payload": {"type": "turn_terminal", "data": {"outcome": "completed"}}}));
    assert_eq!(review::ui().agents, None);
    // Empty instructions: no prompt key, the web's own line, a new turn id.
    review::set_prompt("   ");
    let effect = {
        let ctx = Ctx::new(&store, &ui);
        review::resolve("review.start", 0, &ctx)
    };
    let review::Effect::StartReview { turn_id: second, prompt: None, .. } = &effect else { panic!("{effect:?}") };
    let second = second.clone();
    assert_ne!(second, turn);
    review::perform(effect, &conv).await.expect("accepted");
    let sent = server.sent("review/start");
    assert!(sent[1].get("prompt").is_none(), "no fabricated prompt: {}", sent[1]);
    assert_eq!(notes(&conv, &second), ["Native code review: Review requested for current project changes."]);
}

/// The receipt is checked like the web's turn controller: a refusal and a
/// receipt for another turn both fail visibly on the Session.
#[tokio::test]
async fn refused_or_mismatched_receipts_fail_on_the_session() {
    let _seq = review::test_lock();
    review::reset();
    let server = Server::start(vec!["refused", "other-turn"]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    let ui = Mutex::new(FlowUi::default());
    for want in ["The server did not accept native review.", "Native review returned another turn or workflow."] {
        let effect = {
            let ctx = Ctx::new(&store, &ui);
            review::resolve("review.start", 0, &ctx)
        };
        let review::Effect::StartReview { turn_id, .. } = &effect else { panic!("{effect:?}") };
        let turn = turn_id.clone();
        let err = review::perform(effect, &conv).await.expect_err("fails");
        assert!(err.ends_with(want), "{err}");
        assert_eq!(notes(&conv, &turn)[1], format!("Native code review not started: {want}"));
    }
    assert!(store.domains.review.last_review().is_none());
    assert_eq!(review::ui().agents, None);
}
