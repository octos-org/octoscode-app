//! Card #22 — the no-silent-drops guard.
//!
//! Every inbound `UiNotification` kind must reach a handler **or** an explicit
//! `ignored(reason)` entry. Anything else is a silent drop: it bumps a per-kind
//! `unhandled` counter and logs once per kind at `warn`. This file proves the
//! guard on (a) the core kind list, (b) every recorded real-traffic fixture in
//! the repo, and (c) the committed ignore list.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use octos_core::ui_protocol::methods;
use octoscode_client::domains;
use octoscode_client::Registry;
use octoscode_store::Store;
use serde_json::Value;

/// The core notification kind names (`UI_PROTOCOL_NOTIFICATION_METHODS`).
fn core_kinds() -> &'static [&'static str] {
    octos_core::ui_protocol::UI_PROTOCOL_NOTIFICATION_METHODS
}

/// A real, wired registry + its store.
fn wired() -> (Registry, Arc<Store>) {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store.clone());
    (reg, store)
}

fn decode(method: &str, params: Value) -> octos_core::app_ui::AppUiBackendEvent {
    octos_core::app_ui::AppUiBackendEvent::from_method_and_params(method, params)
        .unwrap_or_else(|e| panic!("{method} must decode: {e:?}"))
}

/// Every `.jsonl` fixture under `crates/*/tests/fixtures/`.
fn all_fixtures() -> Vec<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    // .../crates/octoscode-client -> repo root
    let repo = manifest.parent().and_then(|p| p.parent()).expect("repo root");
    let mut out = Vec::new();
    let crates = repo.join("crates");
    for entry in std::fs::read_dir(&crates).expect("crates dir") {
        let Ok(entry) = entry else { continue };
        let dir = entry.path().join("tests/fixtures");
        if !dir.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&dir).expect("fixtures dir") {
            let Ok(f) = f else { continue };
            let p = f.path();
            if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

// ------------------------------------------------------------ the guard (§2)

#[test]
fn every_core_notification_kind_is_handled_or_explicitly_ignored() {
    // The load-bearing assertion: the core kind list has NO silent drop. Each
    // method is either owned by a handler or carries an explicit ignore entry.
    let (reg, _store) = wired();
    let mut silent: Vec<&str> = Vec::new();
    for m in core_kinds() {
        if !reg.handles(m) && !reg.is_ignored(m) {
            silent.push(m);
        }
    }
    assert!(
        silent.is_empty(),
        "core notification kinds with no handler and no ignore entry: {silent:?}"
    );
}

#[test]
fn an_ignored_kind_is_resolved_and_never_counted_as_unhandled() {
    let (mut reg, store) = wired();
    // `queue/state` is explicitly ignored. Decode a real frame for it.
    let n = decode(
        methods::QUEUE_STATE,
        serde_json::json!({"session_id": "c:c1", "pending_count": 0}),
    );
    assert!(reg.dispatch(&n), "an ignored kind is resolved (not a drop)");
    assert!(
        store.unhandled_methods().is_empty(),
        "an ignored kind must not land in the unhandled counter"
    );
    assert!(reg.ignore_reason(methods::QUEUE_STATE).is_some());
}

#[test]
fn an_unhandled_kind_is_counted_and_warned_once_per_kind() {
    let (mut reg, store) = wired();
    // `peer/staged` IS handled; pick a method that is neither handled nor
    // ignored to exercise the drop path. `session/hydrate` is a *command*, so it
    // is absent from both tables — a faithful stand-in for a future kind.
    let n = decode(
        methods::SESSION_EVENT,
        serde_json::json!({
            "session_id": "c:c1",
            "kind": "legacy",
            "payload": {}
        }),
    );
    // Prove the premise: session/event is handled, so build the drop with a
    // registry that does NOT own it.
    let mut bare = Registry::new();
    bare.set_store(store.clone());
    assert!(!bare.dispatch(&n), "session/event reaches no handler here");
    assert_eq!(store.unhandled_count(methods::SESSION_EVENT), 1);
    // A second arrival counts again but warns only once (per kind).
    assert!(!bare.dispatch(&n));
    assert_eq!(store.unhandled_count(methods::SESSION_EVENT), 2);
    assert_eq!(
        bare.warned_kinds(),
        vec![methods::SESSION_EVENT],
        "warn fires once per kind, not once per frame"
    );
    assert_eq!(store.unhandled_methods(), vec![methods::SESSION_EVENT.to_string()]);
    // The wired registry, by contrast, claims it.
    assert!(reg.dispatch(&n));
}

// ------------------------------------------------ the repo-wide fixture sweep

#[test]
fn every_fixture_replays_with_zero_unhandled_kinds() {
    let fixtures = all_fixtures();
    assert!(fixtures.len() >= 8, "expected the recorded fixtures, got {}", fixtures.len());

    let mut report: Vec<(String, usize, Vec<String>)> = Vec::new();
    let mut payload_types_seen = std::collections::BTreeSet::new();
    let mut all_payload_types = std::collections::BTreeMap::new();

    for path in &fixtures {
        let text = std::fs::read_to_string(path).expect("read fixture");
        let store = Arc::new(Store::new());
        let mut reg = Registry::new();
        domains::register_all(&mut reg, store.clone());

        let mut dispatched = 0;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(line).expect("fixture line is JSON");
            if v["dir"].as_str() != Some("in") {
                continue;
            }
            let method = v["method"].as_str().unwrap_or("");
            let body = v.get("body").cloned().unwrap_or(Value::Null);
            // Only frames that decode as a `UiNotification` are in scope: the
            // guard covers notification *kinds*, and a result/error/state frame
            // is not one. (`from_method_and_params` is the exact decoder the
            // transport uses.)
            let Ok(n) = octos_core::app_ui::AppUiBackendEvent::from_method_and_params(method, body)
            else {
                continue;
            };
            reg.dispatch(&n);
            dispatched += 1;
        }

        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let unhandled = store.unhandled_methods();
        report.push((name, dispatched, unhandled.clone()));
        for t in store.payload_types() {
            payload_types_seen.insert(t.clone());
            *all_payload_types.entry(t).or_insert(0usize) += 1;
        }
        assert!(
            unhandled.is_empty(),
            "{}: notification kinds reached neither a handler nor an ignore entry: {unhandled:?}",
            path.display()
        );
    }

    for (name, n, _) in &report {
        eprintln!("[card22] {name}: {n} notification frames dispatched, 0 unhandled");
    }
    eprintln!("[card22] projection/envelope payload types folded: {payload_types_seen:?}");
    assert!(
        !payload_types_seen.is_empty(),
        "the fixtures must carry projection/envelope payloads to fold"
    );
}

// -------------------------------------------------- the ignore list is a file

#[test]
fn the_committed_ignored_csv_matches_the_registry() {
    let (reg, _store) = wired();
    let expected = reg.ignored_methods();

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.parent().and_then(|p| p.parent()).expect("repo root");
    let csv_path = repo.join("docs/cards/22-ignored.csv");
    let text = std::fs::read_to_string(&csv_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", csv_path.display()));

    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("method,reason"),
        "the CSV header must be exactly `method,reason`"
    );
    // Parse simply (no commas in our reasons; assert that).
    let mut from_csv: Vec<(String, String)> = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (m, r) = line.split_once(',').expect("`method,reason` row");
        assert!(!r.contains(','), "reason for {m} must not contain a comma");
        from_csv.push((m.to_owned(), r.to_owned()));
    }

    let from_reg: Vec<(String, String)> = expected
        .iter()
        .map(|(m, r)| ((*m).to_owned(), (*r).to_owned()))
        .collect();
    assert_eq!(
        from_csv, from_reg,
        "docs/cards/22-ignored.csv must list exactly the registry's ignored kinds, sorted"
    );
}

// ------------------------------------------------------- §1 replay_lossy

#[test]
fn replay_lossy_marks_the_session_lossy_and_raises_a_resync() {
    // The web's `observe` sets `phase="lossy"` and returns `{kind:"recover"}`
    // (durable-session.ts:127-135), which the runtime turns into a
    // `session/hydrate` (active-session-runtime.ts:1256).
    let (mut reg, store) = wired();
    let n = decode(
        methods::REPLAY_LOSSY,
        serde_json::json!({
            "session_id": "c:c1", "dropped_count": 3,
            "last_durable_cursor": {"stream": "main", "seq": 9}
        }),
    );
    assert!(reg.dispatch(&n), "protocol/replay_lossy is handled");

    let recovery = store.domains.config.recovery("c:c1");
    assert_eq!(recovery.phase, octoscode_store::domains::config::LossyPhase::Lossy);
    assert_eq!(recovery.detail, "3 durable events dropped");
    assert!(recovery.resync_pending, "a resync (hydrate) is owed");
    assert!(store.domains.config.resync_pending("c:c1"));

    // The loss itself is still recorded (the pre-#22 projection).
    let loss = store.domains.config.replay_loss("c:c1").expect("recorded");
    assert_eq!(loss.dropped_count, 3);

    // Consuming the resync clears the flag exactly once.
    assert!(store.domains.config.take_resync("c:c1"));
    assert!(!store.domains.config.take_resync("c:c1"));

    // An authoritative hydrate completion resets the session to healthy.
    store.domains.config.mark_recovered("c:c1");
    let after = store.domains.config.recovery("c:c1");
    assert_eq!(after.phase, octoscode_store::domains::config::LossyPhase::Healthy);
    assert!(!after.resync_pending);
}

#[test]
fn the_resync_method_is_session_hydrate() {
    // Card #22 §1 names the resync target: the module issues this `Method`.
    use octoscode_client::Method;
    use octoscode_client::domains::session::SessionHydrate;
    assert_eq!(<SessionHydrate as Method>::NAME, methods::SESSION_HYDRATE);
    assert_eq!(<SessionHydrate as Method>::NAME, "session/hydrate");
}
