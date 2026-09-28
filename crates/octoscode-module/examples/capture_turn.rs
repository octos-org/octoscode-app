//! Card #13 §1 — capture ONE real turn into a JSONL fixture (no fake).
//!
//! Drives the production [`Conversation`] against a real `octos serve`
//! (`a6ea8505`, the `dsflash` profile) with `OCTOSCODE_TRACE_FILE` set, sends
//! **one** short prompt, drains to the terminal event, and flushes the trace.
//! The resulting file is committed as
//! `crates/octoscode-client/tests/fixtures/live-turn-a6ea8505.jsonl` and
//! replayed by `f13_replay.rs`. This is the ONE real turn the card authorises.
//!
//! ```sh
//! OCTOS_BASE_URL=http://127.0.0.1:50110 \
//! OCTOS_BEARER=$(cat tmp/f13-token) OCTOS_PROFILE_ID=dsflash \
//! OCTOSCODE_TRACE_FILE=$PWD/crates/octoscode-client/tests/fixtures/live-turn-a6ea8505.jsonl \
//! cargo run -p octoscode-module --example capture_turn -- tmp/f13-ws
//! ```
use std::time::Duration;

use octoscode_module::flow::{Conversation, FlowEvent};

const PROMPT: &str = "What does 2+3 equal? One word.";

#[tokio::main]
async fn main() {
    let cwd = std::env::args().nth(1);
    let base = std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50110".into());
    let bearer = std::env::var("OCTOS_BEARER").unwrap_or_default();
    let profile = std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into());

    let (conv, mut events) = Conversation::connect(&base, &bearer, &profile, cwd, None)
        .expect("connect");
    println!("[capture] trace enabled: {}", conv.trace_enabled());

    conv.open_workspace(None).await.expect("session/open");

    // Wait for the workspace to open (Live + SessionOpen).
    let mut opened = false;
    for _ in 0..100 {
        match tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(evt)) => {
                if let FlowEvent::WorkspaceOpened(id) = conv.on_event(evt) {
                    println!("[capture] workspace opened: {id}");
                    opened = true;
                    break;
                }
            }
            _ => break,
        }
    }
    assert!(opened, "session/open never completed");

    let turn = conv.start_turn(PROMPT).await.expect("turn/start");
    println!("[capture] turn/start -> {turn}");

    // Drain until the turn reaches a terminal event (or a generous timeout).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    let mut deltas = 0usize;
    let mut terminal = false;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), events.recv()).await {
            Ok(Some(evt)) => {
                match conv.on_event(evt) {
                    FlowEvent::Delta { .. } => deltas += 1,
                    FlowEvent::TurnEnded { turn_id, error } => {
                        println!("[capture] turn ended: {turn_id} error={error:?}");
                        terminal = true;
                        break;
                    }
                    _ => {}
                }
            }
            Ok(None) => break,
            Err(_) => {}
        }
    }
    println!("[capture] deltas={deltas} terminal={terminal}");

    // Let any trailing frames land, then flush the flow's own transitions.
    tokio::time::sleep(Duration::from_millis(800)).await;
    conv.flush_trace();
    println!("[capture] trace flushed");
}
