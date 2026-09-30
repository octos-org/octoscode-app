//! Entry #31b — the six "missing" notifications: replay tests through the
//! REAL pipeline (octos-core serde decode → `Registry::dispatch` → the
//! existing domain handlers → store), with the screen bindings asserted.
//!
//! Supervisor string-check correction (verified this session): the handlers
//! were never missing — they match on typed variants + `methods::*` constants,
//! so the wire-literal grep missed them. What was missing is exactly what
//! these tests pin: the decode leg and the store/binding state per
//! notification.
//!
//! Fixture status per the entry's "record where no fixture has it":
//! - `context/compaction_started` / `context/compaction_completed`: REAL
//!   recorded frames in `r3-session-a6ea8505.jsonl` (the only two of the six
//!   with delivered frames anywhere);
//! - `message/reasoning_delta`, `approval/auto_resolved`,
//!   `approval/cancelled`, `monitor/expired`: NO fixture carries a delivered
//!   frame (the strings in other fixtures are capabilities advertisements) —
//!   their bodies here are hand-written to the octos-core @ a6ea8505 event
//!   structs (`ui_protocol.rs:5181,5483,5553,6091`), decoded through the same
//!   `#[serde(tag = "kind")]` enum the transport decodes
//!   (`ui_protocol.rs:6579-6582`). Recording attempts are reported in
//!   `.peer/report-31b.md`.

use std::sync::{Arc, Mutex};

use octoscode_client::Registry;
use octoscode_module::bindings::{self, Ctx};
use octoscode_module::flow::FlowUi;
use octoscode_store::Store;

/// `octos_core::app_ui::AppUiBackendEvent` — a TYPE ALIAS for
/// `UiNotification` (`octos-core/src/app_ui.rs:31`), which is what the
/// handlers match on.
use octos_core::app_ui::AppUiBackendEvent as UiNotification;

const R3_SESSION: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r3-session-a6ea8505.jsonl"
);

/// The first recorded IN body for `method` in the r3-session recording.
fn recorded(method: &str) -> serde_json::Value {
    for line in std::fs::read_to_string(R3_SESSION).unwrap().lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["dir"].as_str() == Some("in") && v["method"].as_str() == Some(method) {
            return v.get("body").cloned().unwrap_or(serde_json::Value::Null);
        }
    }
    panic!("r3-session carries no recorded {method} frame")
}

/// The wire notification object: the recorded params PLUS the `kind` tag the
/// enum is internally tagged with (`#[serde(tag = "kind",
/// rename_all = "snake_case")]`, `ui_protocol.rs:6579-6582`) — the transport's
/// decode shape. The tag is the VARIANT's snake_case (`ApprovalAutoResolved`
/// → `approval_auto_resolved`), not the method's last segment.
fn wire(method: &str, mut body: serde_json::Value) -> UiNotification {
    let kind = match method {
        "message/reasoning_delta" => "reasoning_delta",
        "approval/auto_resolved" => "approval_auto_resolved",
        "approval/cancelled" => "approval_cancelled",
        "context/compaction_started" => "context_compaction_started",
        "context/compaction_completed" => "context_compaction_completed",
        "monitor/expired" => "monitor_expired",
        other => panic!("unmapped method {other}"),
    };
    let obj = body.as_object_mut().expect("notification body is an object");
    obj.insert("kind".to_owned(), serde_json::json!(kind));
    serde_json::from_value::<UiNotification>(body).expect("the wire body decodes")
}

/// A registry with the four owning domains registered (turn, approval,
/// session, autonomy — the one-owner rule; each domain's `register` is the
/// production registration path).
fn registry(store: &Arc<Store>) -> Registry {
    let mut reg = Registry::new();
    octoscode_client::domains::turn::register(&mut reg, store.clone());
    octoscode_client::domains::approval::register(&mut reg, store.clone());
    octoscode_client::domains::session::register(&mut reg, store.clone());
    octoscode_client::domains::autonomy::register(&mut reg, store.clone());
    reg
}

// ------------------------------------------------- 1. the recorded compaction pair

#[test]
fn the_recorded_compaction_pair_folds_through_the_real_pipeline() {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:api:main".into()));
    let mut reg = registry(&store);

    // STARTED — the recorded frame: trigger + threshold ride the detail.
    let started = wire("context/compaction_started", recorded("context/compaction_started"));
    assert!(reg.dispatch(&started), "a handler owns context/compaction_started");
    let l = store
        .domains
        .session
        .context("dsflash:api:main")
        .expect("the lifecycle is stored");
    assert_eq!(l.kind, "compaction_started");
    assert_eq!(l.state["token_estimate"], 1);
    let d = l.detail.as_ref().expect("the started detail rides the record");
    assert_eq!(d["trigger"], "appui_manual_compact");
    assert_eq!(d["threshold_tokens"], 734003);

    // COMPLETED — the recorded frame: the compaction record rides the detail
    // (this one is a FAILED pass: "no closed semantic prefix is safe to
    // compact" — exactly what the Context panel's last-compaction line shows,
    // web `ContextPanel.tsx:135-143`).
    let done = wire("context/compaction_completed", recorded("context/compaction_completed"));
    assert!(reg.dispatch(&done));
    let l = store.domains.session.context("dsflash:api:main").unwrap();
    assert_eq!(l.kind, "compaction_completed");
    let d = l.detail.as_ref().expect("the compaction record rides the detail");
    assert_eq!(d["status"], "failed");
    assert_eq!(d["checkpoint_id"], "ctxchk_000001_1790588953825");

    // The #31b screen binding exposes the lifecycle for the Context panel.
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    let v = bindings::query(&ctx, "context.lifecycle").expect("context.lifecycle resolves");
    assert_eq!(v["kind"], "compaction_completed");
    assert_eq!(v["detail"]["trigger"], "appui_manual_compact");
}

// --------------------------------------------------- 2. reasoning_delta

#[test]
fn reasoning_delta_appends_a_reasoning_entry_the_binding_exposes() {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    let mut reg = registry(&store);

    // `ReasoningDeltaEvent` (`ui_protocol.rs:5181-5189`): session, turn, text.
    let body = serde_json::json!({
        "session_id": "dsflash:main",
        "turn_id": "01920000-0000-7000-8000-0000000000f4",
        "text": "weighing the two approaches",
    });
    let n = wire("message/reasoning_delta", body);
    assert!(reg.dispatch(&n), "a handler owns message/reasoning_delta");

    // The fold: a `timeline.append_delta(EntryKind::REASONING)` entry
    // (`turn.rs:463-479`) — the store's kind tag is `assistant.reasoning`
    // (`octoscode-store/src/timeline.rs:49-50`).
    let entries = store.domains.session.timeline.entries("dsflash:main");
    let reasoning: Vec<_> = entries
        .iter()
        .filter(|e| e.kind.tag() == "assistant.reasoning")
        .collect();
    assert_eq!(reasoning.len(), 1, "one reasoning entry");
    assert_eq!(reasoning[0].text, "weighing the two approaches");

    // And the timeline binding carries it with its kind tag (the data the
    // web renders at `timeline/model.ts:488,525`).
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    let v = bindings::query(&ctx, "timeline.entries").expect("timeline.entries resolves");
    assert_eq!(v[0]["kind"], "assistant.reasoning");
    assert_eq!(v[0]["text"], "weighing the two approaches");
}

// --------------------------------------------------- 3. the approval pair

/// `ApprovalId` is a Uuid newtype (`ui_protocol.rs:629`): the wire body must
/// carry real UUIDs, and the store rows use the same id strings.
const AUTO_ID: &str = "01920000-0000-7000-8000-0000000000aa";
const CANCEL_ID: &str = "01920000-0000-7000-8000-0000000000bb";

#[test]
fn the_approval_lifecycle_notifications_drive_the_card_states() {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    let mut reg = registry(&store);

    // Two pending rows (as `approval/requested` would leave them).
    let mk = |id: &str| octoscode_store::domains::approval::PendingApproval {
        id: id.to_owned(),
        target: Some("shell".to_owned()),
        decided: false,
        auto_resolved: false,
        cancelled: false,
    };
    store.domains.approval.push(mk(AUTO_ID));
    store.domains.approval.push(mk(CANCEL_ID));

    // AUTO_RESOLVED — a scope policy decided it (`ui_protocol.rs:5483-5496`).
    let auto = wire(
        "approval/auto_resolved",
        serde_json::json!({
            "session_id": "dsflash:main",
            "approval_id": AUTO_ID,
            "turn_id": "01920000-0000-7000-8000-0000000000f4",
            "tool_name": "shell",
            "scope": "workspace-write",
            "scope_match": "exact",
            "decision": "approve",
        }),
    );
    assert!(reg.dispatch(&auto), "a handler owns approval/auto_resolved");

    // CANCELLED — the server cancelled it before anyone could respond
    // (`ui_protocol.rs:5553-5563`).
    let cancel = wire(
        "approval/cancelled",
        serde_json::json!({
            "session_id": "dsflash:main",
            "approval_id": CANCEL_ID,
            "turn_id": "01920000-0000-7000-8000-0000000000f4",
            "reason": "turn_interrupted",
        }),
    );
    assert!(reg.dispatch(&cancel), "a handler owns approval/cancelled");

    // The store states (`domains/approval.rs:99-123` settle/cancel) …
    let rows = store.domains.approval.pending();
    let by_id = |id: &str| rows.iter().find(|a| a.id == id).unwrap();
    assert!(by_id(AUTO_ID).decided && by_id(AUTO_ID).auto_resolved);
    assert!(by_id(CANCEL_ID).cancelled && !by_id(CANCEL_ID).decided);

    // … drive the #31b `approval.rows` binding (the card states the web's
    // blocked wait closes on, `session-peer-coordinator.ts:240`).
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    let v = bindings::query(&ctx, "approval.rows").expect("approval.rows resolves");
    assert_eq!(v[0]["id"], AUTO_ID);
    assert_eq!(v[0]["auto_resolved"], true);
    assert_eq!(v[1]["id"], CANCEL_ID);
    assert_eq!(v[1]["cancelled"], true);
}

// --------------------------------------------------- 4. monitor/expired

#[test]
fn monitor_expired_marks_the_row_expired() {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    let mut reg = registry(&store);

    // A live monitor row (as monitor/create → monitor/updated would leave it).
    store.domains.autonomy.upsert_monitor(
        octoscode_store::domains::autonomy::MonitorRecord {
            monitor_id: "mon-x".into(),
            session_id: "dsflash:main".into(),
            profile_id: None,
            name: "watch".into(),
            mode: "poll".into(),
            status: "active".into(),
            pause_reason: None,
            fires_used: 0,
            last_fired_at_ms: None,
            expires_at_ms: Some(1),
            updated_at_ms: 0,
        },
    );

    // `MonitorExpiredEvent` (`ui_protocol.rs:6091-6102`, #1977); the
    // `monitor` record is OPTIONAL (`rename = "monitor"`) — the timeout-only
    // shape must still mark the row.
    let n = wire(
        "monitor/expired",
        serde_json::json!({
            "session_id": "dsflash:main",
            "monitor_id": "mon-x",
            "status": "expired",
            "reason": "timeout",
        }),
    );
    assert!(reg.dispatch(&n), "a handler owns monitor/expired");

    let m = store
        .domains
        .autonomy
        .monitors()
        .into_iter()
        .find(|m| m.monitor_id == "mon-x")
        .expect("the row survives");
    assert_eq!(m.status, "expired", "MonitorExpiredHandler marks it (autonomy.rs:745-765)");
    assert_eq!(m.pause_reason.as_deref(), Some("timeout"));
}
