//! Card #14 — the live-gate defects, replayed from the **recorded real trace**.
//!
//! The fixture is the outer loop's own gate run
//! (`docs/phase1/live-gate/trace.jsonl` → copied here): 266 frames from a real
//! `octos serve` a6ea8505 (solo, `dsflash`), driven through the native app by
//! real input. This card's defects were all found on those frames.
//!
//! Each defect gets ONE test. Per the 8.10 supervision rule they were written
//! and run **failing first** (committed before the fix); the report lists the
//! failing sha and the fix sha for each.
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_client::domains;
use octoscode_client::registry::Registry;
use octoscode_store::{EntryKind, Store};

/// One recorded frame.
struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

fn fixture() -> Vec<Frame> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/live-gate-a6ea8505.jsonl"
    );
    let text = std::fs::read_to_string(path).expect("read the live-gate fixture");
    text.lines()
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

/// The gate's first turn (156 frames).
const TURN_1: &str = "01a0e75b-dfb8-708a-a7ce-5d29c534f2f6";
/// The gate's session.
const SESSION: &str = "dsflash:main";

/// Replay every recorded **in**bound notification through the real decoder +
/// registry, exactly as the transport would, and hand back the store.
fn replay(frames: &[Frame]) -> Arc<Store> {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store.clone());
    for f in frames {
        if f.dir != "in" {
            continue;
        }
        // Skip only the non-notifications: connection-state transitions and
        // the `capabilities` negotiation event. `session/open` IS a
        // notification (it carries the opened session) and must be dispatched.
        if matches!(f.method.as_str(), "capabilities") || f.method.starts_with("state:") {
            continue;
        }
        match UiNotification::from_method_and_params(&f.method, f.body.clone()) {
            Ok(n) => {
                reg.dispatch(&n);
            }
            Err(e) => panic!("fixture frame {} did not decode: {e:?}", f.method),
        }
    }
    store
}

/// The entry kinds of one turn, in stored order — the timeline as rendered.
fn turn_kinds(store: &Store, turn: &str) -> Vec<(usize, String, String)> {
    store
        .domains
        .session
        .timeline
        .entries(SESSION)
        .into_iter()
        .enumerate()
        .filter(|(_, e)| e.turn_id.as_deref() == Some(turn))
        .map(|(i, e)| (i, e.kind.tag().to_owned(), e.text.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// Defect 1 — the turn's `user.message` must precede its assistant entries.
//
// The real server sends the user_message at `seq 154`, AFTER 153
// reasoning/assistant delta frames (see the fixture). The web splices the
// canonical user row in before that turn's first reply
// (`src-web/apps/web/src/features/timeline/model.ts:1019-1043`, `upsertUser`),
// and never leaves a duplicate when an optimistic row already exists.
// ---------------------------------------------------------------------------
#[test]
fn defect1_user_entry_precedes_the_turns_assistant_entries() {
    let frames = fixture();
    let store = replay(&frames);

    let entries = turn_kinds(&store, TURN_1);
    let user_at = entries
        .iter()
        .position(|(_, kind, _)| kind == EntryKind::USER_MESSAGE.tag())
        .expect("the replay produced the turn's user.message entry");
    let first_reply_at = entries
        .iter()
        .position(|(_, kind, _)| {
            kind == EntryKind::ASSISTANT_TEXT.tag() || kind == EntryKind::REASONING.tag()
        })
        .expect("the replay produced the turn's assistant entries");

    assert!(
        user_at < first_reply_at,
        "the user.message entry (index {user_at}) must precede the turn's first \
         assistant/reasoning entry (index {first_reply_at}); got {:#?}",
        entries.iter().map(|(_, k, t)| (k.as_str(), &t[..t.len().min(24)])).collect::<Vec<_>>()
    );
    // Exactly one user row for the turn (no optimistic + persisted duplicate).
    let users = entries
        .iter()
        .filter(|(_, kind, _)| kind == EntryKind::USER_MESSAGE.tag())
        .count();
    assert_eq!(users, 1, "exactly one user.message entry for the turn");
}

// ---------------------------------------------------------------------------
// Defect 2 — no stray/empty `assistant.text` row.
//
// The gate's timeline carried a bare `[assistant.text] ` row. The fixture pins
// the mechanism: turn 2 (`01a0e75b-f89c-…`) emits `turn/started` and then only
// `reasoning_delta` frames — no `assistant_delta`, no `assistant_persisted`
// (its `turn_terminal` outcome is `interrupted`). So the row can only come from
// creating the entry eagerly at `turn/started` and never filling it.
//
// The web creates NO row on `turn/started` — a row is born on the first
// `appendText` (`src-web/apps/web/src/features/timeline/model.ts:648-678`),
// i.e. from a delta or `assistant_persisted` (`:493-521`). So the fold must
// not leave an empty assistant row behind.
// ---------------------------------------------------------------------------
#[test]
fn defect2_no_stray_or_empty_assistant_entry() {
    let frames = fixture();
    let store = replay(&frames);

    let strays: Vec<_> = store
        .domains
        .session
        .timeline
        .entries(SESSION)
        .into_iter()
        .filter(|e| {
            e.kind == EntryKind::ASSISTANT_TEXT
                && (e.text.trim().is_empty() || e.text.trim() == ".")
        })
        .collect();
    assert!(
        strays.is_empty(),
        "no empty (or lone-'.') `assistant.text` entry may be left behind; got \
         {} stray row(s) on turns {:?}",
        strays.len(),
        strays
            .iter()
            .map(|e| e.turn_id.clone().unwrap_or_default())
            .collect::<Vec<_>>()
    );
    // The answered turn's text is the canonical answer, not a truncated tail.
    let answer = store.live_text(SESSION);
    assert!(
        answer.contains("`main.rs` prints a single line"),
        "the turn's assistant text is the canonical answer; got {answer:?}"
    );
}

// ---------------------------------------------------------------------------
// Defect 3 — the opened session must appear in the session list.
//
// The gate showed `sessions: 0` after a successful open (`s3-completed.json`),
// and the trace holds no `session/list` reply at all — so relying on the
// catalog reply alone leaves the sidebar empty. The web treats the opened
// session as known immediately (`session/opened` seeds the tab-known registry;
// `workspace-session-catalog.ts` only augments it), so a session the server has
// confirmed open must be listed.
// ---------------------------------------------------------------------------
#[test]
fn defect3_the_opened_session_is_listed() {
    let frames = fixture();
    let store = replay(&frames);

    let ids: Vec<String> = store.sessions().into_iter().map(|s| s.id).collect();
    assert!(
        ids.iter().any(|id| id == SESSION),
        "the opened session {SESSION:?} must appear in the session list; got {ids:?}"
    );
    assert!(
        store.session_count() >= 1,
        "the session count must be > 0 after a successful open (the gate showed \
         `sessions: 0`); got {}",
        store.session_count()
    );
}
