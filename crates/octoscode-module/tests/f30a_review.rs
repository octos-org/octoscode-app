//! #30a — the board-3 review screens' gates:
//!
//! 1. **bindings coverage** — every one of the 36 declared ids resolves once
//!    a preview is folded (the #29c lesson: probe a FULL state; on an empty
//!    state `None` is correct — authored copy stays).
//! 2. **card-table agreement** — every copy id the table names exists in the
//!    authored `page.card`s.
//! 3. **the action table** — `diff.scope` cycles the pill; `review.start`
//!    maps the confirmed turn to `review/start` with the web's typed
//!    withholding reasons in precedence order (`native-review.ts:33-55`).
//! 4. **the preview fold** — a `diff/preview/get` result shaped by the
//!    octos-core types (`ui_protocol.rs:2700-2790`) folds into per-file
//!    +/− counts (`DiffReviewDialog.tsx:34-41`), 8 flattened lines with
//!    marks and gutter numbers.
//! 5. **replay** — the RECORDED r5-turn `review/start` traffic through a
//!    fake server: the request reaches the wire with the web's param shape
//!    (`history.ts:250`), and the recorded (null) reply is tolerated — the
//!    receipt folds only on `accepted: true`.

use std::sync::{Arc, Mutex};

use octoscode_module::bindings::{self, Ctx};
use octoscode_module::flow::FlowUi;
use octoscode_module::screens::review;
use octoscode_store::Store;

fn ctx() -> (Arc<Store>, Ctx<'static>) {
    let store: &'static Arc<Store> = Box::leak(Box::new(Arc::new(Store::new())));
    let ui: &'static Mutex<FlowUi> = Box::leak(Box::new(Mutex::new(FlowUi::default())));
    store.domains.session.set_active(Some("dsflash:main".into()));
    (store.clone(), Ctx::new(store, ui))
}

/// The recorded r5-turn wire shapes, cited in the asserts below.
const PREVIEW_JSON: &str = r##"{
  "status": "ready", "source": "pending_store",
  "preview": {"session_id": "dsflash:main", "preview_id": "01920000-0000-7000-8000-0000000000f1",
    "title": "Working tree",
    "files": [
      {"path": "crates/app/src/main.rs", "status": "modified", "hunks": [{"header": "@@",
        "lines": [
          {"kind": "context", "content": "fn main() {", "old_line": 1, "new_line": 1},
          {"kind": "removed", "content": "    run(old);", "old_line": 2},
          {"kind": "added", "content": "    run(new);", "new_line": 2},
          {"kind": "added", "content": "    check();", "new_line": 3}
        ]}]},
      {"path": "docs/b.md", "status": "added", "hunks": [{"header": "@@",
        "lines": [
          {"kind": "added", "content": "# b", "new_line": 1},
          {"kind": "context", "content": "", "new_line": 2}
        ]}]},
      {"path": "docs/c.md", "status": "deleted", "hunks": [{"header": "@@",
        "lines": [{"kind": "removed", "content": "gone", "old_line": 1}]}]},
      {"path": "docs/d.md", "status": "modified", "hunks": [{"header": "@@",
        "lines": [{"kind": "removed", "content": "x", "old_line": 9}]}]},
      {"path": "docs/e.md", "status": "modified", "hunks": [{"header": "@@",
        "lines": [{"kind": "added", "content": "y", "new_line": 9}]}]}
    ]}}"##;

fn seed_preview() {
    let v: serde_json::Value = serde_json::from_str(PREVIEW_JSON).unwrap();
    review::fold_preview(&v);
}

fn seed_run() {
    let mut ui = review::ui();
    ui.last_turn_id = Some("01920000-0000-7000-8000-0000000000a1".into());
    ui.preview_id = Some("01920000-0000-7000-8000-0000000000f1".into());
    ui.blocked = None;
    // The status slot resolves on the accepted receipt too
    // ("Reviewing · N specialists"); the coverage seed carries one.
    ui.agents = Some(3);
}

fn caps_for(store: &Store, caps: &[&str]) {
    store.set_capabilities(caps.iter().map(|c| c.to_string()).collect());
}

// ------------------------------------------------------- §1 bindings coverage

#[test]
fn every_declared_binding_resolves_on_a_folded_preview() {
    let _seq = review::test_lock();
    review::reset();
    let (store, ctx) = ctx();
    caps_for(&store, &["review/start", "review.start.v1"]);
    seed_preview();
    seed_run();
    for (id, _desc) in review::BINDINGS {
        let v = bindings::query(&ctx, id);
        assert!(v.is_some(), "binding {id} does not resolve");
    }
    // the action ids are owned here and never reach the conversation router
    for (a, _d) in review::ACTIONS {
        assert!(review::owns_action(a) && bindings::is_action(a), "{a} not owned");
    }
}

#[test]
fn an_empty_state_keeps_the_authored_copy() {
    let _seq = review::test_lock();
    review::reset();
    let (_store, ctx) = ctx();
    // no preview folded: the live slots yield nothing (authored stays)…
    for id in ["review.add", "review.del", "review.file1.path", "review.line0", "review.mark2"] {
        assert!(bindings::query(&ctx, id).is_none(), "{id} should stay authored");
    }
    // …except the chrome the web always renders.
    assert_eq!(bindings::query(&ctx, "review.scope").unwrap(),
               serde_json::json!("Last turn ▾"));
    assert_eq!(bindings::query(&ctx, "review.start_label").unwrap(),
               serde_json::json!("Start native review"));
}

// --------------------------------------------------- §2 card-table agreement

#[test]
fn every_copy_id_exists_in_its_authored_card() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../design/stage-b/autonomy/cards");
    for (copy_id, binding) in review::COPY_SLOTS {
        let card = if *binding == "review.status" || *binding == "review.start_label" {
            "autonomy-02"
        } else {
            "autonomy-01"
        };
        let src = std::fs::read_to_string(root.join(card).join("page.card"))
            .expect("the committed Stage-B card");
        let needle = format!("copy {copy_id} {{");
        assert!(src.contains(&needle), "{card}: copy slot `{copy_id}` missing");
    }
}

// ---------------------------------------------------- §3 the action table

#[test]
fn diff_scope_cycles_the_pill() {
    let _seq = review::test_lock();
    review::reset();
    let (_store, ctx) = ctx();
    assert_eq!(bindings::query(&ctx, "review.scope").unwrap(),
               serde_json::json!("Last turn ▾"));
    let e = review::resolve("diff.scope", 0, &ctx);
    assert!(matches!(e, review::Effect::ScopeCycle));
    assert_eq!(bindings::query(&ctx, "review.scope").unwrap(),
               serde_json::json!("Project ▾"));
    review::resolve("diff.scope", 0, &ctx);
    assert_eq!(bindings::query(&ctx, "review.scope").unwrap(),
               serde_json::json!("Last turn ▾"));
}

#[test]
fn start_maps_the_confirmed_turn_with_the_web_params() {
    let _seq = review::test_lock();
    review::reset();
    let (store, ctx) = ctx();
    caps_for(&store, &["review/start", "review.start.v1"]);
    seed_run();
    match review::resolve("review.start", 0, &ctx) {
        review::Effect::StartReview { session_id, turn_id } => {
            // history.ts:250 sends {session_id, turn_id, delivery: "inline"}.
            assert_eq!(session_id, "dsflash:main");
            assert_eq!(turn_id, "01920000-0000-7000-8000-0000000000a1");
        }
        other => panic!("expected StartReview, got {other:?}"),
    }
}

#[test]
fn start_is_blocked_with_the_typed_reasons_in_order() {
    let (store, ctx) = ctx();

    // capabilityUnsupported wins when the method OR feature is absent
    // (native-review.ts:18-24 gates on method AND feature).
    {
        let _seq = review::test_lock();
        review::reset();
        caps_for(&store, &["review/start"]); // feature missing
        seed_run();
        match review::resolve("review.start", 0, &ctx) {
            review::Effect::Blocked(r) => assert_eq!(
                r, "This server does not advertise native code review."),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }
    // sessionBusy: a live turn owns the single turn slot (resolve takes the
    // FlowUi lock internally, so the seed drops its guard first).
    {
        let _seq = review::test_lock();
        review::reset();
        caps_for(&store, &["review/start", "review.start.v1"]);
        seed_run();
        ctx.ui.lock().unwrap().begin_turn_now("live-1");
        match review::resolve("review.start", 0, &ctx) {
            review::Effect::Blocked(r) => assert!(r.starts_with(
                "Wait for this Session's active turn"),
                "sessionBusy reason, got {r}"),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }
    // sessionPendingInteraction: a pending approval must settle first.
    // Block 2's live turn is still active on this shared FlowUi — settle it
    // first, or the higher-precedence sessionBusy reason fires here instead.
    {
        let _seq = review::test_lock();
        review::reset();
        caps_for(&store, &["review/start", "review.start.v1"]);
        seed_run();
        ctx.ui.lock().unwrap().end_turn_for_test("live-1", true);
        ctx.ui.lock().unwrap().set_pending_for_test(true, false);
        match review::resolve("review.start", 0, &ctx) {
            review::Effect::Blocked(r) => assert!(r.starts_with(
                "Wait for this Session's pending questions and approvals"),
                "pending-interaction reason, got {r}"),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }
}

#[test]
fn unhandled_ids_stay_unhandled() {
    let (_store, ctx) = ctx();
    assert!(matches!(
        review::resolve("composer.submit", 0, &ctx),
        review::Effect::Unhandled(_)
    ));
}

// ------------------------------------------------------------- §4 the fold

#[test]
fn the_preview_folds_counts_lines_marks_numbers() {
    let _seq = review::test_lock();
    review::reset();
    seed_preview();
    let st = review::ui();
    // per-file +/− over the file's hunk lines (DiffReviewDialog.tsx:34-41)
    assert_eq!(st.files.len(), 5);
    assert_eq!(st.files[0].path, "crates/app/src/main.rs");
    assert_eq!(st.files[0].add, 2, "file 1: two added lines");
    assert_eq!(st.files[0].del, 1, "file 1: one removed line");
    assert_eq!(st.files[2].del, 1, "deleted file counts its removals");
    // header totals: +4 −3
    let a: u64 = st.files.iter().map(|f| f.add).sum();
    let d: u64 = st.files.iter().map(|f| f.del).sum();
    assert_eq!((a, d), (4, 3));
    // flattened lines, capped at the card's 8 slots
    assert!(st.lines.len() <= 8 && st.lines.len() >= 6);
    assert_eq!(st.lines[0].content, "fn main() {");
    assert_eq!(st.lines[1].mark(), "-");
    assert_eq!(st.lines[2].mark(), "+");
    assert_eq!(st.lines[2].num(), "2", "the gutter shows the new side");
    assert_eq!(st.lines[1].num(), "2", "…else the old side");
    assert_eq!(st.lines[0].mark(), "", "context lines carry no mark");
    assert_eq!(st.lines[3].mark(), "+", "lines[3] is the second added line");
}

#[test]
fn the_folded_values_reach_the_card_slots() {
    let _seq = review::test_lock();
    review::reset();
    let (_store, ctx) = ctx();
    seed_preview();
    assert_eq!(bindings::query(&ctx, "review.add").unwrap(), serde_json::json!("+4"));
    assert_eq!(bindings::query(&ctx, "review.del").unwrap(), serde_json::json!("-3"));
    assert_eq!(bindings::query(&ctx, "review.file1.path").unwrap(),
               serde_json::json!("crates/app/src/main.rs"));
    assert_eq!(bindings::query(&ctx, "review.file1.add").unwrap(), serde_json::json!("+2"));
    assert_eq!(bindings::query(&ctx, "review.line1").unwrap(), serde_json::json!("    run(old);"));
    assert_eq!(bindings::query(&ctx, "review.mark3").unwrap(), serde_json::json!("+"));
    assert_eq!(bindings::query(&ctx, "review.num4").unwrap(), serde_json::json!("1"));
}

// --------------------------------------------------------------- §5 replay

struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

fn load(path: &str) -> Vec<Frame> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
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

struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl ReplayServer {
    /// Serves the RECORDED reply body per method (r5-turn's review/start
    /// reply is `null` — the real recorded traffic), and logs every request.
    async fn start(canned: Vec<(String, serde_json::Value)>) -> Self {
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
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                let params = v.get("params").cloned().unwrap_or(serde_json::Value::Null);
                rx.lock().unwrap().push((method.clone(), params));
                let body = canned
                    .iter()
                    .find(|(m, _)| *m == method)
                    .map(|(_, b)| b.clone())
                    .unwrap_or(serde_json::Value::Null);
                let frame = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": body});
                let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { base_url: format!("http://{addr}"), received }
    }
}

#[tokio::test]
async fn replay_the_recorded_review_start_reaches_the_wire() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/r5-turn-a6ea8505.jsonl");
    let frames = load(fixture);
    let recorded = frames
        .iter()
        .find(|f| f.dir == "out" && f.method == "review/start")
        .expect("r5-turn records a review/start request")
        .body
        .clone();
    // The recording's params carry delivery "inline" (the web's constant).
    assert_eq!(recorded["delivery"], "inline");
    assert!(recorded["turn_id"].is_string(), "the recorded request names a turn");

    let reply = frames
        .iter()
        .find(|f| f.dir == "in" && f.method == "review/start")
        .map(|f| f.body.clone())
        .expect("r5-turn records the review/start reply");
    let server = ReplayServer::start(vec![("review/start".into(), reply)]).await;
    let (conv, mut _events) =
        octoscode_module::flow::Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
            .expect("connect");
    conv.open_workspace(None).await.expect("session/open");

    // The screen action maps to the recorded request shape…
    let turn = recorded["turn_id"].as_str().unwrap().to_owned();
    let effect = review::Effect::StartReview {
        session_id: conv.session_id(),
        turn_id: turn,
    };
    review::perform(effect, &conv).await.expect("review/start sends");

    // …and the server saw it.
    let seen = server.received.lock().unwrap().clone();
    let (_m, params) = seen
        .iter()
        .find(|(m, _)| m == "review/start")
        .expect("the wire carried review/start");
    assert_eq!(params["delivery"], "inline");
    assert_eq!(params["turn_id"], recorded["turn_id"]);
    // The recorded (null) reply is tolerated: nothing folds on accepted!=true.
    assert!(conv.store.domains.review.last_review().is_none());
}
