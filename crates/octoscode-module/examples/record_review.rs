//! #30a — record the review surface's real frames into a JSONL fixture.
//!
//! The `record_autonomy` pattern ([`crate::examples`], card #R1): drive the
//! production [`Conversation`] against a real `octos serve` (a6ea8505, the
//! `dsflash` profile) with `OCTOSCODE_TRACE_FILE` set, exercise the screen's
//! unrecorded method, flush the trace, scrub machine paths.
//!
//! `review/start` is already recorded (r5-turn: request + its null reply).
//! This recorder covers **`diff/preview/get`** — a read (ZERO model turns;
//! RULES forbid any spend) — so the screen's preview fetch replays committed
//! real traffic like its action does.
//!
//! ```sh
//! OCTOS_BASE_URL=http://127.0.0.1:50160 OCTOS_BEARER=30a-lane \
//! OCTOS_PROFILE_ID=dsflash OCTOS_WORKSPACE_CWD=$PWD \
//! OCTOSCODE_TRACE_FILE=$PWD/tmp/r30a-diffpreview.jsonl \
//! cargo run -p octoscode-module --example record_review
//! ```
use octoscode_module::flow::Conversation;

#[tokio::main]
async fn main() {
    let base = std::env::var("OCTOS_BASE_URL").expect("OCTOS_BASE_URL");
    let bearer = std::env::var("OCTOS_BEARER").unwrap_or_default();
    let profile = std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "dsflash".into());
    let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();

    let (conv, mut events) =
        Conversation::connect(&base, &bearer, &profile, cwd, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    // Drain the open's inbound frames into the trace.
    for _ in 0..20 {
        match tokio::time::timeout(std::time::Duration::from_millis(150), events.recv()).await {
            Ok(Some(evt)) => {
                let _ = conv.on_event(evt);
            }
            _ => break,
        }
    }

    let session = conv.session_id();
    // The octos-core param type is {session_id, preview_id}
    // (ui_protocol.rs:2483-2486); PreviewId is a uuid v7. A wire-shaped id
    // literal (no uuid dep needed): the server answers from its pending
    // store — on a serve with no live turn the recorded reply IS the truth
    // this fixture pins (status/source shape or the typed error), never a
    // fabrication.
    let preview_id = "01920000-0000-7000-8000-0000000000f1";
    let client = conv.client();
    let outcome = client
        .request(
            "diff/preview/get",
            serde_json::json!({"session_id": session, "preview_id": preview_id}),
        )
        .await;
    match &outcome {
        Ok(v) => println!("diff/preview/get -> {}", serde_json::to_string(v).unwrap_or_default()),
        Err(e) => println!("diff/preview/get -> error: {e}"),
    }

    conv.flush_trace();
    println!(
        "frames recorded: {}",
        conv.trace_enabled().then(|| "see OCTOSCODE_TRACE_FILE").unwrap_or("trace OFF")
    );
}
