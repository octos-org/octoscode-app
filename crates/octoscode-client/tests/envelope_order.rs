//! A turn's envelopes in the order octos writes them on a connection that did
//! not negotiate `projection.envelope.v2` (OctoSense's private stdio pipe,
//! octos 056173e8): the terminal takes a direct lane and overtakes the turn's
//! last deltas, and the persisted row is sequenced among those deltas (the
//! commit races the delta queue). The shapes are a real turn's, scrubbed.
//! Folded through the production registry, the answer is whole, said once,
//! and the turn ends after it.
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent;
use octoscode_client::Registry;
use octoscode_store::{EntryKind, Store};
use serde_json::{json, Value};

const SESSION: &str = "_main:api:code-0001";
const THREAD: &str = "01a10dab-thread";
const SEGMENT: &str = "01a10dab-thread:assistant:iteration:1";

fn envelope(seq: u64, payload: Value) -> AppUiBackendEvent {
    let body = json!({
        "session_id": SESSION,
        "thread_id": THREAD,
        "turn_id": THREAD,
        "seq": seq,
        "cursor": {"seq": 100 + seq, "stream": format!("{SESSION}\u{0}~cwd-1")},
        "payload": payload,
    });
    AppUiBackendEvent::from_method_and_params("projection/envelope", body).expect("an envelope")
}

fn delta(seq: u64, text: &str) -> AppUiBackendEvent {
    envelope(seq, json!({"type": "assistant_delta", "data": {"assistant_segment_id": SEGMENT, "text": text}}))
}

fn persisted(seq: u64, text: &str) -> AppUiBackendEvent {
    envelope(
        seq,
        json!({"type": "assistant_persisted", "data": {
            "assistant_segment_id": SEGMENT,
            "meta": {"message_id": "m1", "persisted_at": "2026-10-05T20:05:09.013215Z"},
            "text": text,
        }}),
    )
}

fn terminal(seq: u64) -> AppUiBackendEvent {
    envelope(
        seq,
        json!({"type": "turn_terminal", "data": {
            "outcome": "completed",
            "token_usage": {"cache_read_tokens": 0, "input_tokens": 1, "output_tokens": 1, "reasoning_tokens": 0},
        }}),
    )
}

fn wired() -> (Registry, Arc<Store>) {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    octoscode_client::domains::register_all(&mut reg, store.clone());
    (reg, store)
}

fn answers(store: &Store) -> Vec<String> {
    store
        .domains
        .session
        .timeline
        .entries(SESSION)
        .into_iter()
        .filter(|e| e.kind == EntryKind::ASSISTANT_TEXT)
        .map(|e| e.text)
        .collect()
}

#[test]
fn a_terminal_written_ahead_of_its_turns_last_deltas_lands_after_them() {
    let (mut reg, store) = wired();
    for frame in [delta(1, "`gre"), delta(2, "et`"), terminal(6), delta(3, " returns"), persisted(4, "`greet` returns x."), delta(5, " x.")] {
        reg.dispatch(&frame);
    }
    assert_eq!(answers(&store), ["`greet` returns x."], "one answer, whole, said once");
    assert_eq!(store.domains.turn.terminal(THREAD).as_deref(), Some("completed"), "and then the turn ended");
    // A replay of what was applied changes nothing.
    reg.dispatch(&delta(3, " returns"));
    assert_eq!(answers(&store), ["`greet` returns x."]);
}

#[test]
fn until_the_gap_fills_the_answer_and_the_turn_stay_as_they_were() {
    let (mut reg, store) = wired();
    reg.dispatch(&delta(1, "a"));
    reg.dispatch(&terminal(3));
    assert_eq!(answers(&store), ["a"]);
    assert_eq!(store.domains.turn.terminal(THREAD), None, "held: the turn has not ended yet");
    reg.dispatch(&delta(2, "b"));
    assert_eq!(answers(&store), ["ab"]);
    assert_eq!(store.domains.turn.terminal(THREAD).as_deref(), Some("completed"));
}

#[tokio::test(start_paused = true)]
async fn a_gap_that_never_fills_is_accepted_after_the_wait() {
    let (mut reg, store) = wired();
    reg.dispatch(&delta(1, "a"));
    reg.dispatch(&delta(3, "c"));
    reg.dispatch(&terminal(4));
    assert_eq!(answers(&store), ["a"]);
    tokio::time::sleep(octoscode_client::domains::turn::GAP_WAIT + std::time::Duration::from_millis(10)).await;
    assert_eq!(answers(&store), ["ac"], "what was held, in order");
    assert_eq!(store.domains.turn.terminal(THREAD).as_deref(), Some("completed"));
}
