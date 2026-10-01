//! P4h1 — the production-path replay tests for the workspace rows this lane
//! flipped: the sparse token-cost merge (row 300), the session display label
//! (row 301) and the recent-workspaces cache reaching the OPEN path (row 305).
//!
//! Every assertion below runs through the same entry points the app uses:
//! `models::note_transport_event` (the `progress/updated` fold wired at
//! `lib.rs:1453`), `models::query_binding` (the context panel's slots), the
//! `screen::thread_rows` / `screens::sessions::query` row projections, and
//! `screens::workspace::apply(Effect::Open(..))` against the production
//! `Conversation` client. RULES 3: a function only a test calls counts as
//! missing, so each rule is proven where the app reaches it.
//!
//! The frames are RECORDED real `octos serve` traffic (a6ea8505) — the
//! live-gate capture. `progress/updated` is the token-cost carrier
//! (`workspace-events.ts:6-10`); every recorded one is DENSE (all five fields),
//! which is exactly why the sparse-merge gap survived upstream: the merge only
//! differs on a sparse frame, so the test pairs the recorded dense frame with a
//! hand-built SPARSE one over the same `session_id`.
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screen;
use octoscode_module::screens::{models, recents, sessions, workspace};
use octos_app_transport::TransportEvent;
use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_store::Session;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

// ---- the recorded traffic ----------------------------------------------------

const LIVE_GATE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/live-gate-a6ea8505.jsonl"
);
/// The r3-session capture, for the recorded `session/open` + `session/list`
/// reply shapes.
const R3_SESSION: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r3-session-a6ea8505.jsonl"
);

/// The session the live-gate recording's turn ran under.
const SESSION: &str = "dsflash:main";

struct Frame {
    dir: String,
    method: String,
    body: Value,
}

fn load(path: &str) -> Vec<Frame> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(Value::Null),
            }
        })
        .collect()
}

/// The FIRST recorded IN body for `method` (an RPC reply is `res:<method>`).
fn recorded_in(path: &str, method: &str) -> Value {
    load(path)
        .iter()
        .find(|f| f.dir == "in" && (f.method == method || f.method == format!("res:{method}")))
        .map(|f| f.body.clone())
        .unwrap_or_else(|| panic!("{method} is in the recording {path}"))
}

/// The recorded `progress/updated` frame that actually carries a token cost.
///
/// NOT just the first frame with that method: the live-gate capture has 9
/// `progress/updated` IN frames and only ONE is a `token_cost_update` — the
/// rest are `thinking` / `agent_progress` / `response` / `stream_end`. The
/// production fold ignores those (it gates on `metadata.kind`), so the token
/// cost here is the frame the fold would actually consume.
fn recorded_token_cost() -> Value {
    load(LIVE_GATE)
        .into_iter()
        .find(|f| {
            f.dir == "in"
                && f.method == "progress/updated"
                && f.body["metadata"]["kind"] == "token_cost_update"
        })
        .map(|f| f.body)
        .unwrap_or_else(|| panic!("the live-gate recording carries no token_cost_update frame"))
}

/// Decode a recorded frame into the transport event the screen folds.
///
/// This is the SAME decode the transport performs, so the test cannot drift
/// from what the socket really delivers.
fn event(method: &str, body: Value) -> TransportEvent {
    let payload = UiNotification::from_method_and_params(method, body)
        .unwrap_or_else(|e| panic!("{method} did not decode: {e:?}"));
    TransportEvent::DurableNotification {
        payload,
        cursor: None,
    }
}

// ---- the replay server (f29c/fp4a3's shape) ---------------------------------

struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
}

impl ReplayServer {
    async fn start(canned: Vec<(String, Value)>) -> Self {
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
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                rx.lock().unwrap().push(method.clone());
                let body = canned
                    .iter()
                    .find(|(m, _)| *m == method)
                    .map(|(_, b)| b.clone())
                    .unwrap_or(json!({}));
                let frame = json!({"jsonrpc": "2.0", "id": id, "result": body});
                let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self {
            base_url: format!("http://{addr}"),
            received,
        }
    }

    fn saw(&self, method: &str) -> bool {
        self.received.lock().unwrap().iter().any(|m| m == method)
    }
}

fn store_with_sessions(rows: Vec<Session>) -> Arc<octoscode_store::Store> {
    let store = Arc::new(octoscode_store::Store::new());
    store.set_sessions(rows);
    store
}

fn session(id: &str, title: Option<&str>, last_prompt: Option<&str>) -> Session {
    Session {
        id: id.to_owned(),
        title: title.map(str::to_owned),
        message_count: 0,
        updated_at: None,
        last_prompt: last_prompt.map(str::to_owned),
        active_turn: false,
    }
}

/// The token-cost projection is ONE process-global (the production fold owns
/// it; there is no per-test isolation), so the three tests that reset or write
/// it must not run CONCURRENTLY — `RUST_TEST_THREADS=4` would let a sibling's
/// `reset_token_cost()`, or its write for another session, stomp this test's
/// projection between a fold and its assertion.
///
/// Poisoning is recovered from deliberately: one failing test must not cascade
/// into the other two reporting a spurious global-state failure.
static TOKEN_COST_GUARD: Mutex<()> = Mutex::new(());

fn token_cost_guard() -> std::sync::MutexGuard<'static, ()> {
    TOKEN_COST_GUARD.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- row 300: the sparse token-cost merge ------------------------------------

/// Row 300 through the PRODUCTION fold: the recorded dense `progress/updated`
/// frame seeds the projection, then a SPARSE frame for the same session must
/// not erase the fields it did not carry.
///
/// The recorded frame's own field set is asserted first, so if a future capture
/// ever DOES contain a sparse frame this test documents which case it covers.
#[test]
fn a_sparse_token_cost_frame_preserves_the_fields_it_does_not_carry() {
    let _guard = token_cost_guard();
    models::reset_token_cost();
    let recorded = recorded_token_cost();
    let cost = &recorded["metadata"]["token_cost"];
    assert_eq!(
        cost["model"], "deepseek-v4-flash",
        "the recorded frame is the live-gate token cost"
    );
    let recorded_fields = cost.as_object().expect("token_cost is an object").len();
    assert_eq!(
        recorded_fields, 5,
        "every RECORDED frame is dense — the sparse case is the one the recording cannot show"
    );

    // 1. The recorded frame, folded by the production entry.
    models::note_transport_event(&event("progress/updated", recorded.clone()));
    let merged = models::token_cost(SESSION).expect("the fold wrote the projection");
    assert_eq!(merged.session_id, SESSION);
    assert_eq!(merged.fields["session_cost"], cost["session_cost"]);

    // 2. A SPARSE frame for the SAME session: only the window arrives.
    let sparse = json!({
        "session_id": SESSION,
        "turn_id": recorded["turn_id"],
        "metadata": {
            "kind": "token_cost_update",
            "token_cost": {"context_window": 1048576}
        }
    });
    models::note_transport_event(&event("progress/updated", sparse));
    let merged = models::token_cost(SESSION).expect("still one projection");
    assert_eq!(
        merged.fields["context_window"], 1048576,
        "the sparse frame's own field lands"
    );
    for carried in ["model", "session_cost", "input_tokens", "output_tokens"] {
        assert_eq!(
            merged.fields[carried], cost[carried],
            "a sparse frame must not erase {carried} (web mergeTokenCost, model.ts:58)"
        );
    }
}

/// The other arm: an update for a DIFFERENT session REPLACES the projection
/// (`if (!current || current.sessionId !== next.sessionId) return next`).
#[test]
fn a_token_cost_update_for_another_session_replaces_the_projection() {
    let _guard = token_cost_guard();
    models::reset_token_cost();
    let recorded = recorded_token_cost();
    models::note_transport_event(&event("progress/updated", recorded.clone()));
    let other = json!({
        "session_id": "dsflash:other",
        "metadata": {"kind": "token_cost_update", "token_cost": {"context_window": 200_000}}
    });
    models::note_transport_event(&event("progress/updated", other));
    let merged = models::token_cost("dsflash:other").expect("the newer session owns it");
    assert_eq!(merged.fields["context_window"], 200_000);
    assert!(
        models::token_cost(SESSION).is_none(),
        "the other session's update replaced rather than merged"
    );
}

/// The merged projection is READ by the context panel — this is the production
/// reader that makes the merge a product path rather than a test-only helper
/// (RULES 3). The panel's window comes from the projection now.
#[test]
fn the_context_panel_reads_the_merged_projection() {
    let _guard = token_cost_guard();
    models::reset_token_cost();
    let store = store_with_sessions(vec![session(SESSION, Some("T"), None)]);
    store.set_active(Some(SESSION.to_owned()));
    // The panel composes TWO legs (web ContextPanel.tsx:96-104): the
    // lifecycle's `token_estimate` and the token-cost window. `pct_line`
    // needs both, so the lifecycle is seeded here as f29c's `seed` does.
    store.domains.session.set_context(
        SESSION,
        octoscode_store::domains::session::ContextLifecycle {
            kind: "context/normalization_reported".into(),
            state: json!({"token_estimate": 262144, "session_id": SESSION}),
            detail: None,
        },
    );
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);

    let recorded = recorded_token_cost();
    models::note_transport_event(&event("progress/updated", recorded.clone()));
    let window = recorded["metadata"]["token_cost"]["context_window"]
        .as_u64()
        .expect("the recorded window");

    // The window leg: "262144 of 1048576 tokens" (the recorded 1_048_576).
    let usage = models::query_binding(&ctx, "context.usage").expect("the usage slot");
    let usage = usage.as_str().expect("a string");
    assert!(
        usage.contains(&window.to_string()),
        "the panel's window comes from the merged projection, got {usage}"
    );

    // And the derived percentage, which needs BOTH legs — the proof the
    // projection is genuinely read by the panel and not stored beside it.
    let pct = models::query_binding(&ctx, "context.pct").expect("the pct slot");
    let pct = pct.as_str().expect("a string");
    assert_ne!(pct, "—", "the panel composed a percentage, got {pct}");
    assert_eq!(pct, &format!("{}%", 262144 * 100 / window), "got {pct}");

    // A SPARSE frame still leaves the panel whole: the merge is what makes
    // the second fold safe for a panel that has already rendered.
    models::note_transport_event(&event(
        "progress/updated",
        json!({
            "session_id": SESSION,
            "metadata": {"kind": "token_cost_update",
                         "token_cost": {"context_window": window}}
        }),
    ));
    let after = models::query_binding(&ctx, "context.pct").expect("still resolves");
    assert_eq!(after.as_str(), Some(pct), "a sparse frame does not blank the panel");
}

// ---- row 301: the session display label --------------------------------------

/// Row 301 through BOTH production row projections: the sidebar
/// (`screen::thread_rows`, the display label) and the resume picker
/// (`screens::sessions::query`, the catalog label). The two web rules differ
/// ONLY in their fallback — the display label falls through to the id, the
/// catalog row stays null — and this asserts that difference is preserved.
#[test]
fn the_session_label_falls_back_title_then_prompt_then_id() {
    let store = store_with_sessions(vec![
        session("p:titled", Some("Fix the parser"), Some("ignored prompt")),
        session("p:prompt", Some("   "), Some("why does steer_dropped mean?")),
        session("p:bare", None, None),
    ]);
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);

    // The sidebar projection: always a non-empty label.
    let titles: Vec<String> = screen::thread_rows(&store)
        .into_iter()
        .map(|r| r.title)
        .collect();
    assert_eq!(titles[0], "Fix the parser", "the title wins over the prompt");
    assert_eq!(
        titles[1], "why does steer_dropped mean?",
        "a whitespace-only title falls through to the prompt"
    );
    assert_eq!(titles[2], "p:bare", "and with neither, the id");

    // The resume picker projection: the web's catalog row has NO id fallback
    // (`workspace-session-catalog.ts:120` — `title: entry.title ??
    // entry.lastPrompt`), so the untitled row stays null.
    let picker = sessions::query(&ctx, "resume.rows[].title").expect("the picker slot");
    let rows = picker.as_array().expect("a list of titles");
    assert_eq!(rows[0], json!("Fix the parser"));
    assert_eq!(rows[1], json!("why does steer_dropped mean?"));
    assert_eq!(rows[2], Value::Null, "the catalog row stays null, not the id");
}

// ---- row 305: the cache on the OPEN path -------------------------------------

/// Row 305 through the production open: `screens::workspace::apply` against a
/// real `Conversation`, so the recents write is reached the way the picker
/// reaches it — not by calling the cache directly.
#[tokio::test]
async fn opening_a_workspace_remembers_it_on_the_production_path() {
    let mut open_reply = recorded_in(R3_SESSION, "session/open");
    open_reply["workspace_root"] = json!("/srv/work/octoscode");
    let server = ReplayServer::start(vec![
        ("session/open".to_owned(), open_reply),
        ("session/list".to_owned(), json!({"sessions": []})),
    ])
    .await;
    let (conv, mut _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");

    // The cache under test is an in-memory one, so this never touches a home.
    let cache = Arc::new(recents::MemoryStore::new());
    recents::set_store(cache.clone());

    workspace::apply(workspace::Effect::Open("/srv/work/octoscode".to_owned()), &conv)
        .await
        .expect("the open through the production client");
    assert!(server.saw("session/open"), "the wire carried session/open");

    let stored = recents::load_recent_workspaces(&*cache, &recents::endpoint());
    assert_eq!(stored.len(), 1, "the open remembered the workspace");
    assert_eq!(stored[0].path, "/srv/work/octoscode");
    assert_eq!(
        stored[0].name, "octoscode",
        "the name is DERIVED from the host path (row 307)"
    );

    // Re-opening the same path dedupes and moves it to the front (row 305).
    workspace::apply(workspace::Effect::Open("/srv/other".to_owned()), &conv)
        .await
        .expect("second open");
    workspace::apply(workspace::Effect::Open("/srv/work/octoscode".to_owned()), &conv)
        .await
        .expect("re-open");
    let stored = recents::load_recent_workspaces(&*cache, &recents::endpoint());
    assert_eq!(stored.len(), 2, "the re-open did not duplicate the row");
    assert_eq!(stored[0].path, "/srv/work/octoscode", "moved to the front");
}
