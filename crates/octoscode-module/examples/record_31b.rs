//! Entry #31b — record the four no-fixture notifications into a real-traffic
//! fixture (the entry authorises ≤3 dsflash model turns; this recorder spends
//! at most 2 and one zero-turn probe).
//!
//! Provocation plan (each honestly reported in `.peer/report-31b.md`):
//! 1. `monitor/expired` — NO model turn: `monitor/create` with
//!    `timeout_secs: 2`, drain 8s; #1977 fires on timeout.
//! 2. `message/reasoning_delta` — ONE turn with a reasoning prompt; whether
//!    the model emits thinking deltas is model-dependent (reported as-is).
//! 3. `approval/cancelled` — ONE turn asking for an OUT-OF-WORKSPACE write
//!    (escalation → `approval/requested`), then `turn/interrupt` mid-flight;
//!    the server cancels the pending approval (`turn_interrupted`).
//! 4. `approval/auto_resolved` — NOT provoked: it needs a pre-persisted
//!    matching scope policy plus a second matching request; the turn budget
//!    was spent on the two above. Reported as not provoked.
//!
//! ```sh
//! OCTOS_BASE_URL=http://127.0.0.1:50140 OCTOS_BEARER=r31b-dummy \
//! OCTOS_PROFILE_ID=dsflash OCTOSCODE_TRACE_FILE=$PWD/tmp/31b-raw.jsonl \
//! cargo run -p octoscode-module --example record_31b -- tmp/31b-ws
//! ```
use std::time::Duration;

use octos_app_transport::TransportEvent;
use octoscode_client::domains::autonomy as au;
use octoscode_module::flow::{Conversation, FlowEvent};
use tokio::sync::mpsc::Receiver;

const REASONING_PROMPT: &str =
    "Answer with one short sentence: is 17 prime? Think it through briefly before answering.";
const ESCALATION_PROMPT: &str =
    "Run exactly this one shell command and nothing else: echo 31b > /tmp/31b-probe.txt \
     (that path is outside the workspace, so it needs approval).";

async fn drain(conv: &Conversation, rx: &mut Receiver<TransportEvent>, ms: u64) -> usize {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    let mut n = 0usize;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(deadline - tokio::time::Instant::now(), rx.recv()).await {
            Ok(Some(evt)) => {
                let _ = conv.on_event(evt);
                n += 1;
            }
            _ => break,
        }
    }
    n
}

#[tokio::main]
async fn main() {
    let cwd = std::env::args().nth(1);
    let base = std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50140".into());
    let bearer = std::env::var("OCTOS_BEARER").unwrap_or_default();
    let profile = std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into());

    let (conv, mut events) =
        Conversation::connect(&base, &bearer, &profile, cwd, None).expect("connect");
    println!("[r31b] trace enabled: {}", conv.trace_enabled());

    conv.open_workspace(None).await.expect("session/open");
    let mut opened = false;
    for _ in 0..100 {
        match tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(evt)) => {
                if let FlowEvent::WorkspaceOpened(id) = conv.on_event(evt) {
                    println!("[r31b] workspace opened: {id}");
                    opened = true;
                    break;
                }
            }
            _ => break,
        }
    }
    assert!(opened, "session/open never completed");
    let session = conv
        .store
        .active_session()
        .unwrap_or_else(|| "dsflash:main".into());

    // ---- 1. monitor/expired: zero model turns -------------------------------
    let created = conv
        .client()
        .call::<au::MonitorCreate>(au::MonitorCreateParams {
            session_id: session.clone(),
            name: "r31b expiry probe".to_owned(),
            argv: vec!["/bin/echo".to_owned(), "waiting".to_owned()],
            filter_regex: None,
            mode: Some("poll".to_owned()),
            interval_seconds: Some(3600),
            batch_ms: None,
            timeout_secs: Some(2), // expires ~2s in (#1977)
            persistent: Some(false),
            goal_id: None,
            max_events_per_hour: None,
        })
        .await;
    match created {
        Ok(r) => println!("[r31b] monitor/create -> {} ({})", r.monitor_id, r.status),
        Err(e) => println!("[r31b] monitor/create err: {e}"),
    }
    let n = drain(&conv, &mut events, 9_000).await;
    println!("[r31b] expiry drain: {n} frames");

    // ---- 2. reasoning_delta: one real turn ----------------------------------
    let t1 = conv.start_turn(REASONING_PROMPT).await.expect("turn/start 1");
    println!("[r31b] turn1 -> {t1}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), events.recv()).await {
            Ok(Some(evt)) => {
                if let FlowEvent::TurnEnded { error, .. } = conv.on_event(evt) {
                    println!("[r31b] turn1 ended error={error:?}");
                    break;
                }
            }
            _ => break,
        }
    }
    drain(&conv, &mut events, 1_500).await;

    // ---- 3. approval/cancelled: escalate, then interrupt --------------------
    let t2 = conv.start_turn(ESCALATION_PROMPT).await.expect("turn/start 2");
    println!("[r31b] turn2 -> {t2}");
    // Give the model time to request the out-of-workspace write.
    drain(&conv, &mut events, 12_000).await;
    match conv.interrupt(&t2).await {
        Ok(_) => println!("[r31b] turn2 interrupted mid-flight"),
        Err(e) => println!("[r31b] interrupt err: {e}"),
    }
    let n = drain(&conv, &mut events, 6_000).await;
    println!("[r31b] post-interrupt drain: {n} frames");

    tokio::time::sleep(Duration::from_millis(800)).await;
    conv.flush_trace();
    println!("[r31b] trace flushed (2 model turns spent; monitor probe was free)");
}
