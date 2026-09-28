//! Card #R1 — record the autonomy domain's real frames into a JSONL fixture.
//!
//! Drives the production [`Conversation`] (registry + store + trace) against a
//! real `octos serve` (`a6ea8505`, the `dsflash` profile) with
//! `OCTOSCODE_TRACE_FILE` set, exercises every read-only autonomy method plus
//! the create→delete mutation cycle, and flushes the trace. The resulting file
//! is committed as
//! `crates/octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl` and
//! replayed by `crates/octoscode-client/tests/r1_replay.rs`.
//!
//! **No model turns.** Reads are free; mutations run only against a throwaway
//! data dir (create then delete what is created).
//!
//! ```sh
//! OCTOS_BASE_URL=http://127.0.0.1:50170 \
//! OCTOS_BEARER=r1-dummy-token OCTOS_PROFILE_ID=dsflash \
//! OCTOSCODE_TRACE_FILE=$PWD/crates/octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl \
//! cargo run -p octoscode-module --example record_autonomy
//! ```
use std::time::Duration;

use octos_app_transport::TransportEvent;
use octoscode_client::domains::autonomy as au;
use octoscode_module::flow::Conversation;
use tokio::sync::mpsc::Receiver;

/// Drain pending transport events through the conversation for `ms`, so every
/// inbound frame is recorded by `Conversation::on_event` (the trace hook).
async fn drain(conv: &Conversation, rx: &mut Receiver<TransportEvent>, ms: u64) {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }
        match tokio::time::timeout(deadline - now, rx.recv()).await {
            Ok(Some(evt)) => {
                let _ = conv.on_event(evt);
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
}

#[tokio::main]
async fn main() {
    let base = std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50170".into());
    let bearer = std::env::var("OCTOS_BEARER").unwrap_or_default();
    let profile = std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into());

    let (conv, mut events) = Conversation::connect(&base, &bearer, &profile, None, None)
        .expect("connect");
    println!("[record] trace enabled: {}", conv.trace_enabled());

    conv.open_workspace(None).await.expect("session/open");
    drain(&conv, &mut events, 2500).await;
    let session_id = conv.session_id().to_owned();
    println!("[record] session: {session_id}");

    let list = || au::AutonomyListParams {
        session_id: Some(session_id.clone()),
        profile_id: Some(profile.clone()),
    };
    let goal_params = || au::GoalSessionParams {
        session_id: session_id.clone(),
        profile_id: Some(profile.clone()),
    };

    // ---- read-only reads (every one through the production Client path) ----
    macro_rules! read {
        ($label:expr, $call:expr) => {
            match $call.await {
                Ok(_) => println!("[record] {} ok", $label),
                Err(e) => println!("[record] {} err: {e}", $label),
            }
            drain(&conv, &mut events, 700).await;
        };
    }

    read!("agent/list", conv.client().call::<au::AgentList>(list()));
    read!("loop/list", conv.client().call::<au::LoopList>(list()));
    read!("monitor/list", conv.client().call::<au::MonitorList>(list()));
    read!("session/goal/get", conv.client().call::<au::GoalGet>(goal_params()));

    // ---- mutations: create then delete what we create --------------------
    let loop_id = match conv
        .client()
        .call::<au::LoopCreate>(au::LoopCreateParams {
            session_id: session_id.clone(),
            prompt: Some("r1 replay probe".to_owned()),
            command: None,
            interval_seconds: Some(3600),
            mode: Some("fixed_interval".to_owned()),
        })
        .await
    {
        Ok(r) => {
            println!("[record] loop/create -> {} ({})", r.loop_id, r.status);
            Some(r.loop_id)
        }
        Err(e) => {
            println!("[record] loop/create err: {e}");
            None
        }
    };
    drain(&conv, &mut events, 700).await;

    if let Some(id) = &loop_id {
        read!("loop/pause", conv.client().call::<au::LoopPause>(au::LoopControlParams::new(id)));
        read!("loop/resume", conv.client().call::<au::LoopResume>(au::LoopControlParams::new(id)));
        read!("loop/fire_now", conv.client().call::<au::LoopFireNow>(au::LoopControlParams::new(id)));
        read!("loop/delete", conv.client().call::<au::LoopDelete>(au::LoopControlParams::new(id)));
    }

    let monitor_id = match conv
        .client()
        .call::<au::MonitorCreate>(au::MonitorCreateParams {
            session_id: session_id.clone(),
            name: "r1 replay monitor".to_owned(),
            argv: vec!["./scripts/watch.sh".to_owned()],
            filter_regex: Some("ERROR".to_owned()),
            mode: Some("poll".to_owned()),
            interval_seconds: Some(3600),
            batch_ms: None,
            timeout_secs: None,
            persistent: None,
            max_events_per_hour: None,
            goal_id: None,
        })
        .await
    {
        Ok(r) => {
            println!("[record] monitor/create -> {} ({})", r.monitor_id, r.status);
            Some(r.monitor_id)
        }
        Err(e) => {
            println!("[record] monitor/create err: {e}");
            None
        }
    };
    drain(&conv, &mut events, 700).await;

    if let Some(id) = &monitor_id {
        read!("monitor/pause", conv.client().call::<au::MonitorPause>(au::MonitorControlParams::new(id)));
        read!("monitor/resume", conv.client().call::<au::MonitorResume>(au::MonitorControlParams::new(id)));
        read!("monitor/delete", conv.client().call::<au::MonitorDelete>(au::MonitorControlParams::new(id)));
    }

    // ---- session goal: set then clear ------------------------------------
    read!(
        "session/goal/set",
        conv.client().call::<au::GoalSet>(au::GoalSetParams {
            session_id: session_id.clone(),
            objective: "r1 replay probe".to_owned(),
            profile_id: Some(profile.clone()),
            status: None,
            token_budget: None,
            transition_actor: Some("user".to_owned()),
        })
    );
    read!("session/goal/clear", conv.client().call::<au::GoalClear>(goal_params()));

    // Let any trailing frames land, then flush the flow's own transitions.
    drain(&conv, &mut events, 800).await;
    conv.flush_trace();
    println!("[record] trace flushed");
}
