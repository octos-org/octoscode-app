//! Card #23 — replay the **recorded real dsflash/a6ea8505 traffic** for the
//! conversation domains (turn + message/delta + reasoning, tool, plan,
//! approval, user question, steer) through the production client path.
//!
//! The fixture is `tests/fixtures/r23-conversation-a6ea8505.jsonl`: 11 real
//! model turns against `octos serve` a6ea8505 with the `dsflash` profile,
//! recorded by `capture23*.rs` on this lane's own port **50160**. It is
//! hermetic — no machine path, no credential (guarded by
//! `fixtures_hermetic.rs` and re-checked below).
//!
//! Every test reads the fixture's OWN recorded values (never the environment),
//! per LESSONS "Replay tests must be hermetic".
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_client::domains;
use octoscode_client::registry::Registry;
use octoscode_store::{EntryKind, Store};
use serde_json::Value;

/// One recorded frame (the shape `FrameTrace` writes).
struct Frame {
    dir: String,
    method: String,
    body: Value,
}

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/r23-conversation-a6ea8505.jsonl")
}

fn load_fixture() -> Vec<Frame> {
    let text = std::fs::read_to_string(fixture_path()).expect("read the r23 fixture");
    text.lines()
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

/// Replay every `in` notification frame through the **real** `register_all`
/// registry into a fresh store; return the store and the unhandled methods.
fn replay_into_store(frames: &[Frame]) -> (Arc<Store>, Vec<String>) {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store.clone());
    let mut unhandled = Vec::new();
    for f in frames.iter().filter(|f| f.dir == "in") {
        // Non-notification in-frames (state:/capabilities/result rows the
        // capture writes for the handshake) do not decode here.
        let Ok(n) = UiNotification::from_method_and_params(&f.method, f.body.clone()) else {
            continue;
        };
        if !reg.dispatch(&n) {
            unhandled.push(f.method.clone());
        }
    }
    (store, unhandled)
}

fn session_of(frames: &[Frame]) -> String {
    frames
        .iter()
        .find(|f| f.dir == "out" && f.method == "session/open")
        .and_then(|f| f.body["session_id"].as_str().map(str::to_owned))
        .expect("the fixture carries our outbound session/open")
}

/// The `turn_id` of the recorded turn with the given terminal `outcome`.
fn turn_id_for_outcome(frames: &[Frame], outcome: &str) -> String {
    frames
        .iter()
        .find(|f| {
            f.dir == "in"
                && f.method == "projection/envelope"
                && f.body["payload"]["data"]["outcome"] == outcome
        })
        .and_then(|f| f.body["turn_id"].as_str().map(str::to_owned))
        .unwrap_or_else(|| panic!("no recorded turn_terminal with outcome {outcome}"))
}

// ---------------------------------------------------------- hermeticity

#[test]
fn r23_fixture_is_hermetic_and_secret_free() {
    let text = std::fs::read_to_string(fixture_path()).expect("read the r23 fixture");
    for bad in ["/Users/", "/var/folders/", "/home/runner/"] {
        assert!(!text.contains(bad), "machine path `{bad}` leaked into the fixture");
    }
    let lowered = text.to_ascii_lowercase();
    assert!(!lowered.contains("bearer "), "no bearer header may be recorded");
    for (i, w) in text
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .enumerate()
    {
        assert!(
            !(w.starts_with("sk-") && w.len() >= 20),
            "key-shaped token at word {i}"
        );
    }
    // The card's acceptance is a raw `grep -ciE "sk-|bearer [a-z0-9]{20}"` that
    // must print 0. Prove the file satisfies the exact regex — the only way a
    // `sk-` could appear is inside recorded prose (e.g. "ask-user"), which the
    // recorder must avoid.
    assert_eq!(
        regex_sk_bearer_count(&text),
        0,
        "the card's `sk-|bearer <token>` grep must be 0"
    );
}

/// Count lines matching the card's regex (`sk-` OR `bearer` + 20 alnum), case
/// -insensitive — the same thing `grep -ciE` counts.
fn regex_sk_bearer_count(text: &str) -> usize {
    text.lines()
        .filter(|l| {
            let low = l.to_ascii_lowercase();
            if low.contains("sk-") {
                return true;
            }
            low.split("bearer ").skip(1).any(|rest| {
                rest.chars().take_while(|c| c.is_ascii_alphanumeric()).count() >= 20
            })
        })
        .count()
}

// --------------------------------------------------- real-recording shape

#[test]
fn the_fixture_is_a_real_recording_of_these_domains() {
    let frames = load_fixture();
    let methods: Vec<&str> = frames.iter().map(|f| f.method.as_str()).collect();
    // The domains this card owns, every one of which the recording must carry.
    for required in [
        "turn/start",
        "turn/started",
        "turn/steer",
        "turn/interrupt",
        "turn/steer_dropped",
        "projection/envelope",
        "progress/updated",
        "approval/requested",
        "approval/respond",
        "approval/decided",
        "plan/updated",
        "user_question/requested",
        "user_question/respond",
        "permission/profile/list",
        "permission/profile/set",
        "thread/graph/get",
        "turn/state/get",
    ] {
        assert!(
            methods.contains(&required),
            "the recording must exercise {required}"
        );
    }
    // Every turn in the recording reached a terminal (no dangling turn).
    let terminals = frames
        .iter()
        .filter(|f| f.method == "projection/envelope")
        .filter(|f| f.body["payload"]["type"] == "turn_terminal")
        .count();
    let started = methods.iter().filter(|m| **m == "turn/started").count();
    assert_eq!(terminals, started, "every started turn must have a terminal");
    assert!(started >= 10, "the card recorded real turns, found {started}");
}

#[test]
fn every_recorded_notification_is_claimed_by_some_domain() {
    let frames = load_fixture();
    let (_store, unhandled) = replay_into_store(&frames);
    assert!(
        unhandled.is_empty(),
        "recorded notifications no domain claims (would hit the debug! arm): {unhandled:?}"
    );
}

// ------------------------------------------------- turn + reasoning + text

#[test]
fn replayed_turns_and_reasoning_fold_into_the_store() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);
    let session = session_of(&frames);

    // Every recorded turn is known; all reached a terminal, so none is left
    // in flight.
    let seen = store.domains.turn.seen();
    assert!(seen.len() >= 10, "all recorded turns are tracked: {seen:?}");
    assert_eq!(store.domains.turn.in_flight_count(), 0, "all turns terminated");

    // The recorded terminals survive with their real outcomes — this is the
    // `projection/envelope` `turn_terminal` fold, the bug #13 found.
    let completed = turn_id_for_outcome(&frames, "completed");
    assert_eq!(
        store.domains.turn.terminal(&completed).as_deref(),
        Some("completed")
    );
    let interrupted = turn_id_for_outcome(&frames, "interrupted");
    assert_eq!(
        store.domains.turn.terminal(&interrupted).as_deref(),
        Some("interrupted"),
        "the interrupted turn's outcome must survive"
    );

    // Reasoning is its OWN entry kind (never merged into the answer text).
    let reasoning = store.domains.session.timeline.of_kind(&session, EntryKind::REASONING);
    assert!(!reasoning.is_empty(), "the visible thinking must fold in");
    assert!(
        reasoning.iter().any(|e| !e.text.is_empty()),
        "reasoning entries carry the text"
    );

    // The assistant answer text folded, and a known recorded phrase is present.
    let text = store.domains.session.timeline.assistant_text(&session);
    assert!(!text.trim().is_empty(), "the answer text must fold in");
    assert!(
        text.contains("blue") || text.contains("Blue"),
        "the recorded question answer ('blue') must be in the transcript; got {text:?}"
    );

    // The user's own prompts became user.message entries. The server emits a
    // `user_message` payload for SOME turns only (7 of the 11 recorded here,
    // read from the fixture — never an assumed count), and `upsert_user_message`
    // keeps exactly one row per turn, so the store count must MATCH the fixture.
    let recorded_user_turns: std::collections::BTreeSet<String> = frames
        .iter()
        .filter(|f| f.method == "projection/envelope")
        .filter(|f| f.body["payload"]["type"] == "user_message")
        .filter_map(|f| f.body["turn_id"].as_str().map(str::to_owned))
        .collect();
    assert!(!recorded_user_turns.is_empty(), "the recording carries user prompts");
    let users = store.domains.session.timeline.of_kind(&session, EntryKind::USER_MESSAGE);
    assert_eq!(
        users.len(),
        recorded_user_turns.len(),
        "one user.message row per recorded prompt (upsert dedups by turn): {users:?}"
    );
    assert!(
        users.iter().all(|e| !e.text.trim().is_empty()),
        "every user.message row carries its prompt text"
    );
}

#[test]
fn the_envelope_ordering_advanced_the_canonical_cursor() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);
    let session = session_of(&frames);
    let (stream, seq) = store
        .domains
        .turn
        .envelope_cursor()
        .expect("the envelopes advanced the canonical cursor");
    assert_eq!(stream, session, "the cursor stream is the session");
    assert!(seq > 0, "the cursor advanced past the first seq");

    // The recorded per-thread sequences are monotonic, so NOTHING is dropped.
    assert!(
        store.domains.turn.dropped_envelopes().is_empty(),
        "a real monotonic stream drops no envelope: {:?}",
        store.domains.turn.dropped_envelopes()
    );

    // Every recorded thread carries its own last accepted seq.
    let threads: std::collections::BTreeSet<String> = frames
        .iter()
        .filter(|f| f.method == "projection/envelope")
        .filter_map(|f| f.body["thread_id"].as_str().map(str::to_owned))
        .collect();
    assert!(threads.len() >= 10, "one thread per recorded turn");
    for t in &threads {
        assert!(
            store.domains.turn.last_envelope_seq(t).is_some(),
            "thread {t} accepted at least one envelope"
        );
    }
}

// --------------------------------------------------------------------- tool

#[test]
fn replayed_tool_payloads_drive_the_tool_domain() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);
    let calls = store.domains.tool.calls();
    assert!(!calls.is_empty(), "the recorded tool calls must land");

    // The `bash` tool the model ran is present and closed with its real output.
    let bash = calls
        .iter()
        .find(|c| c.name == "bash")
        .unwrap_or_else(|| panic!("the recorded `bash` call must land; got {calls:?}"));
    assert_eq!(bash.status, "done", "tool_end `complete` maps to `done`: {bash:?}");
    assert!(
        bash.output_preview
            .as_deref()
            .is_some_and(|o| o.contains("r23-tool-ok")),
        "the recorded bash output must survive: {bash:?}"
    );
    assert!(bash.duration_ms.is_some(), "the recorded duration must survive");

    // The user-question tool call is recorded too.
    assert!(
        calls.iter().any(|c| c.name == "ask_user_question"),
        "the recorded ask_user_question call must land: {calls:?}"
    );
}

// ----------------------------------------------------------------- plan

#[test]
fn replayed_plan_updated_replaces_the_session_plan() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);
    let session = session_of(&frames);

    // The recorded plan's own values (hermetic: read from the fixture).
    let recorded = frames
        .iter()
        .find(|f| f.method == "plan/updated")
        .expect("the fixture carries plan/updated");
    let plan = store
        .domains
        .task
        .plan(&session)
        .expect("the recorded plan landed");
    assert_eq!(plan.items.len(), recorded.body["plan"]["items"].as_array().unwrap().len());
    assert_eq!(plan.items[0].title, "Add the --version flag");
    assert_eq!(plan.items[0].status, "pending");
    assert_eq!(
        plan.turn_id.as_deref(),
        recorded.body["turn_id"].as_str(),
        "the authoring turn is kept for clearing"
    );
    assert_eq!(plan.updated_at_ms, recorded.body["plan"]["updated_at_ms"].as_i64().unwrap());
}

// -------------------------------------------------------------- approvals

#[test]
fn replayed_approval_lifecycle_settles_the_store_rows() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);

    // Both recorded approvals are known and DECIDED (approve then deny).
    let pending = store.domains.approval.pending();
    assert_eq!(pending.len(), 2, "both approvals are tracked: {pending:?}");
    for row in &pending {
        assert!(row.decided, "the recorded approval/decided settled {row:?}");
        assert!(!row.cancelled, "no approval/cancelled was recorded");
        assert!(!row.auto_resolved, "a person decided it, not a policy");
    }
    // The requested row kept the tool that prompted it (the sheet's target).
    assert!(
        pending.iter().all(|r| r.target.as_deref() == Some("bash")),
        "the recorded approval names the bash tool: {pending:?}"
    );

    // The two decisions in the recording are one approve and one deny.
    let decisions: Vec<&str> = frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == "approval/decided")
        .filter_map(|f| f.body["decision"].as_str())
        .collect();
    assert!(decisions.contains(&"approve") && decisions.contains(&"deny"));
}

#[test]
fn replayed_user_question_is_outstanding_for_the_sheet() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);

    let recorded = frames
        .iter()
        .find(|f| f.method == "user_question/requested")
        .expect("the fixture carries user_question/requested");
    let q = store
        .domains
        .approval
        .question()
        .expect("the recorded question is outstanding");
    assert_eq!(
        q.question_id,
        recorded.body["question_id"].as_str().unwrap(),
        "the question keeps its exact id for user_question/respond"
    );
    assert_eq!(q.turn_id, recorded.body["turn_id"].as_str().unwrap());
    assert_eq!(q.title, recorded.body["title"].as_str().unwrap());
    assert!(
        q.questions.is_array() && !q.questions.as_array().unwrap().is_empty(),
        "the structured questions survive for the sheet"
    );
}

// -------------------------------------------------------- steer + progress

#[test]
fn replayed_leftover_steer_is_recorded_for_the_composer() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);

    let recorded = frames
        .iter()
        .find(|f| f.method == "turn/steer_dropped")
        .expect("the fixture carries turn/steer_dropped");
    let dropped = store.domains.turn.dropped_steers();
    assert_eq!(dropped.len(), 1, "one leftover steer was returned: {dropped:?}");
    assert_eq!(
        dropped[0].reason,
        recorded.body["reason"].as_str().unwrap(),
        "the reason (interrupted) survives"
    );
    assert_eq!(
        dropped[0].inputs,
        recorded.body["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        "the returned input text is preserved so the composer can restore it"
    );
    assert_eq!(
        dropped[0].turn_id,
        recorded.body["turn_id"].as_str().unwrap()
    );
}

#[test]
fn replayed_progress_lands_in_the_store() {
    let frames = load_fixture();
    let (store, _) = replay_into_store(&frames);
    let session = session_of(&frames);

    // `progress/updated` metadata is kept per session (the last one wins).
    assert!(
        store.domains.turn.progress(&session).is_some(),
        "the recorded progress/updated metadata must land"
    );

    // `context/normalization_reported` is folded by the session domain.
    assert!(store.seen_count("context/normalization_reported") > 0);
}

/// `thread/graph/get` is a **request/response** method (it has a `Method` impl
/// but no notification handler): its result rides the transport's oneshot, so
/// it never reaches the registry and no store setter is driven by it. Assert
/// the recording really carries the live round trip, like R5 does.
#[test]
fn replayed_thread_graph_read_only_round_trip_is_recorded() {
    let frames = load_fixture();
    assert!(
        frames
            .iter()
            .any(|f| f.dir == "out" && f.method == "thread/graph/get"),
        "our outbound thread/graph/get was really sent"
    );
    let answered = frames.iter().any(|f| {
        f.dir == "in"
            && (f.method == "thread/graph/get" || f.method.starts_with("error:thread/graph/get"))
    });
    assert!(answered, "the server answered thread/graph/get live");
    // The recorded result really lists this session's threads. The read was a
    // pre-turn probe, so it lists only the threads that existed THEN — assert
    // the shape and the fixture's own recorded count, never an assumed one.
    let results: Vec<&Frame> = frames
        .iter()
        .filter(|f| f.dir == "in" && f.method == "thread/graph/get")
        .collect();
    assert!(!results.is_empty(), "a recorded thread/graph/get result");
    let first = results[0];
    let threads = first.body["threads"]
        .as_array()
        .expect("the result carries `threads`");
    assert!(!threads.is_empty(), "the pre-turn probe lists the open threads");
    assert_eq!(first.body["session_id"], session_of(&frames));
    // Every row carries the flattened fields the store's `ThreadRow` models.
    for row in threads {
        assert!(row["thread_id"].is_string(), "row has a thread_id: {row}");
        assert!(row["root_seq"].is_number(), "row has a root_seq: {row}");
        assert!(row["status"].is_string(), "row has a status: {row}");
    }
}
