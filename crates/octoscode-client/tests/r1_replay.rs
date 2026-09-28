//! Card #R1 — replay the **recorded real autonomy traffic** through the registry.
//!
//! Unlike `f1_autonomy.rs` (hand-written fakes), this test feeds
//! `tests/fixtures/r1-autonomy-a6ea8505.jsonl` — the frames a real `octos serve`
//! (a6ea8505, `dsflash`) actually sent for the autonomy surface (recorded by
//! `octoscode-module/examples/record_autonomy.rs`). Each inbound frame is
//! decoded by the transport's own contract decoder
//! ([`UiNotification::from_method_and_params`]) and dispatched through the real
//! [`Registry`] into an [`Store`], then the resulting state is asserted.
//!
//! The live gate taught the lesson this guards: the real server wraps turn /
//! message / tool updates in `projection/envelope`, which hand-written fakes
//! never did. This fixture carries the real shapes (e.g. `loop/updated` with a
//! `deleted: true` tombstone, `session/goal/*` with a monotonic `generation`).
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_client::Registry;
use octoscode_store::Store;
use serde_json::Value;

/// One recorded frame (the JSONL the client's `FrameTrace` writes).
struct Frame {
    dir: String,
    method: String,
    body: Value,
}

fn fixture() -> Vec<Frame> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/r1-autonomy-a6ea8505.jsonl"
    );
    let text = std::fs::read_to_string(path).expect("read the r1 autonomy fixture");
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

/// Is `method` an inbound *notification* (decodable by the transport), rather
/// than a connection-state line, a capability handshake or a lifecycle reply?
fn is_notification(f: &Frame) -> bool {
    f.dir == "in"
        && !f.method.starts_with("state:")
        && f.method != "capabilities"
        && f.method != "session/open"
}

#[test]
fn r1_autonomy_fixture_replays_through_the_real_registry_into_the_store() {
    let frames = fixture();
    assert!(frames.len() >= 20, "the fixture should carry a real recording");
    let session_id = "dsflash:main";

    let store = Arc::new(Store::new());
    let mut registry = Registry::new();
    octoscode_client::domains::register_all(&mut registry, store.clone());

    let mut decoded = 0usize;
    let mut undecodable: Vec<String> = Vec::new();
    let mut loop_seen_after_create = false;
    let mut loop_gone_after_delete = false;
    let mut fire_counter_after_fired: Option<u64> = None;
    let mut monitor_seen_after_create = false;
    let mut monitor_gone_after_delete = false;
    let mut goal_present_after_update = false;

    for f in &frames {
        if !is_notification(f) {
            continue;
        }
        let decoded_notification =
            match UiNotification::from_method_and_params(&f.method, f.body.clone()) {
                Ok(n) => {
                    decoded += 1;
                    n
                }
                Err(_) => {
                    undecodable.push(f.method.clone());
                    continue;
                }
            };
        let claimed = registry.dispatch(&decoded_notification);
        assert!(
            claimed || !f.method.contains('/') || f.method.starts_with("projection/"),
            "an unhandled notification hit the debug! arm: {}",
            f.method
        );

        // Milestones, in recording order.
        match f.method.as_str() {
            "loop/updated" => {
                let deleted = f.body.get("deleted").and_then(Value::as_bool).unwrap_or(false);
                if deleted {
                    loop_gone_after_delete = store.domains.autonomy.loop_record("loop_01").is_none();
                } else if store.domains.autonomy.loop_record("loop_01").is_some() {
                    loop_seen_after_create = true;
                }
            }
            "loop/fired" => {
                fire_counter_after_fired = store
                    .domains
                    .autonomy
                    .loop_record("loop_01")
                    .map(|l| l.fires);
            }
            "monitor/updated" => {
                let deleted = f.body.get("deleted").and_then(Value::as_bool).unwrap_or(false);
                if deleted {
                    monitor_gone_after_delete =
                        store.domains.autonomy.monitor("monitor_01").is_none();
                } else if store.domains.autonomy.monitor("monitor_01").is_some() {
                    monitor_seen_after_create = true;
                }
            }
            "session/goal/updated" => {
                goal_present_after_update = store.domains.autonomy.goal(session_id).is_some();
            }
            _ => {}
        }
    }

    // Every notification recorded from the real server decodes cleanly.
    assert_eq!(undecodable, Vec::<String>::new(), "undecodable inbound frames");

    // The real loop lifecycle: created (upserted), fired (counter bumped), then
    // deleted (tombstone removed it).
    assert!(loop_seen_after_create, "loop/updated upserted loop_01");
    assert_eq!(fire_counter_after_fired, Some(1), "loop/fired bumped the counter");
    assert!(loop_gone_after_delete, "loop/updated deleted:true removed loop_01");

    // The real monitor lifecycle: created, then deleted.
    assert!(monitor_seen_after_create, "monitor/updated upserted monitor_01");
    assert!(monitor_gone_after_delete, "monitor/updated deleted:true removed monitor_01");

    // The real goal lifecycle: set (generation 1), then cleared (generation 2),
    // and the #1959 generation guard admits the clear so the goal does not
    // resurrect.
    assert!(goal_present_after_update, "session/goal/updated installed the goal");
    assert!(
        store.domains.autonomy.goal(session_id).is_none(),
        "session/goal/cleared removed the goal"
    );
    assert_eq!(
        store.domains.autonomy.goal_generation(session_id),
        2,
        "the clear's generation was admitted (strictly greater than the update's)"
    );

    // Each autonomy handler actually ran for every recorded frame of its kind.
    assert_eq!(store.seen_count("loop/updated"), 4, "loop/updated frames handled");
    assert_eq!(store.seen_count("loop/fired"), 1, "loop/fired frame handled");
    assert_eq!(store.seen_count("monitor/updated"), 4, "monitor/updated frames handled");
    assert_eq!(store.seen_count("session/goal/updated"), 1, "goal/updated frame handled");
    assert_eq!(store.seen_count("session/goal/cleared"), 1, "goal/cleared frame handled");

    // R1 step 4: NO recorded live notification hit the registry's tolerated
    // `debug!` arm — every frame kind the real server sent is handled by some
    // domain (autonomy or another lane's). `unknown_methods()` is empty.
    assert_eq!(
        registry.unknown_methods(),
        Vec::<String>::new(),
        "a recorded live frame kind is unhandled (would hit the debug! arm)"
    );

    assert_eq!(store.domains.autonomy.agent_count(), 0, "no agents in a turn-free recording");
    assert!(decoded >= 6, "decoded {decoded} notifications from the fixture");
}
