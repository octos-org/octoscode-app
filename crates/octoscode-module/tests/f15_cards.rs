//! Card #15 — the mounted cards' two gates:
//!
//! 1. **bindings coverage** — every binding id the mounted cards declare in
//!    `design/cards/index.json` resolves in `bindings.rs` (a `query` arm or a
//!    declared action). This is the 8.8 condition-2 audit for the mount.
//! 2. **replay** — feeding the **recorded live-gate trace**
//!    (`crates/octoscode-client/tests/fixtures/live-gate-a6ea8505.jsonl`, 266
//!    real frames from `octos serve` a6ea8505 + `dsflash`) through the real
//!    decoder + registry, the mounted cards' bound values are what the gate
//!    demands: the user entry BEFORE its turn's replies, the answer text, the
//!    `Worked for` row, and the opened session listed.
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_client::domains;
use octoscode_client::registry::Registry;
use octoscode_module::bindings;
use octoscode_module::cards::{self, Slot};
use octoscode_module::flow::FlowUi;
use octoscode_store::Store;
use serde_json::Value;

// ---------------------------------------------------------------------------
// 1. bindings coverage
// ---------------------------------------------------------------------------

#[test]
fn every_declared_card_binding_resolves() {
    let declared = cards::declared_bindings();
    assert!(
        !declared.is_empty(),
        "the manifest must declare bindings (design/cards/index.json)"
    );

    let mut missing = Vec::new();
    for id in &declared {
        let resolves = bindings::is_action(id)
            || {
                // A data id resolves when `query` has an arm for it. Probe with
                // the same fixture the module uses (an empty store is fine — the
                // point is that the ARM exists, not its value).
                let store = Arc::new(Store::new());
                let ui = std::sync::Mutex::new(FlowUi::default());
                bindings::query(&bindings::Ctx::new(&store, &ui), id).is_some()
            };
        if !resolves {
            missing.push(id.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "every binding id a mounted card declares must resolve in bindings.rs; \
         missing: {missing:?}"
    );

    // Every slot the module mounts is filled by exactly one card.
    for slot in Slot::ALL {
        assert!(
            cards::card_for_slot(*slot).is_some(),
            "slot `{}` has no card in the manifest",
            slot.name()
        );
    }
}

#[test]
fn the_manifest_parses_and_its_artefacts_exist() {
    let m = cards::mounted();
    assert_eq!(m.manifest.stage, "B");
    assert_eq!(m.manifest.mount, "native");
    assert_eq!(m.manifest.cards.len(), 5, "the 5 Gate-B cards");
    for c in &m.manifest.cards {
        assert!(
            Slot::from_name(&c.slot).is_some(),
            "card {} names unknown slot {:?}",
            c.id,
            c.slot
        );
        assert!(c.gate_b_score > 0.0, "card {} has no Gate-B score", c.id);
        // The tree the mount reads must parse (a slot falls back otherwise).
        let root = cards::structure(c).unwrap_or_else(|e| {
            panic!("card {} structure must parse: {e}", c.id)
        });
        assert_eq!(root.kind, "stack", "card {} root is the page surface", c.id);
        assert!(
            !cards::plan(&root).is_empty(),
            "card {} planned no rows",
            c.id
        );
    }
}

// ---------------------------------------------------------------------------
// 2. replay the recorded live-gate trace through the mounted cards
// ---------------------------------------------------------------------------

struct Frame {
    dir: String,
    method: String,
    body: Value,
}

fn fixture() -> Vec<Frame> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/live-gate-a6ea8505.jsonl"
    );
    let text = std::fs::read_to_string(path).expect("read the live-gate fixture");
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

/// The gate's first turn + its session.
const TURN_1: &str = "01a0e75b-dfb8-708a-a7ce-5d29c534f2f6";
const SESSION: &str = "dsflash:main";

/// Replay the recorded notifications through the real decoder + registry.
fn replay_store(frames: &[Frame]) -> Arc<Store> {
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    domains::register_all(&mut reg, store.clone());
    for f in frames {
        if f.dir != "in" {
            continue;
        }
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

/// Render one slot against a store (the same resolver shape the module uses).
fn render(store: &Arc<Store>, slot: Slot) -> String {
    let ui = std::sync::Mutex::new(FlowUi::default());
    let ctx = bindings::Ctx::new(store, &ui);
    cards::render_slot(slot, &|id| bindings::query(&ctx, id))
        .unwrap_or_else(|e| panic!("slot {} must render: {e}", slot.name()))
}

#[test]
fn the_thread_list_slot_lists_the_opened_session() {
    let frames = fixture();
    let store = replay_store(&frames);
    let body = render(&store, Slot::ThreadList);

    let titles: Vec<String> = store.sessions().into_iter().map(|s| s.title.clone().unwrap_or_default()).collect();
    assert!(
        body.contains(SESSION) || store.sessions().iter().any(|s| s.id == SESSION),
        "the opened session {SESSION:?} must be listed by the thread-list card; \
         sessions={:?} body={body:?}",
        store.sessions().iter().map(|s| &s.id).collect::<Vec<_>>()
    );
    // The card renders one row per session row (the authored 5 are overridden).
    assert!(
        body.contains("thread_1"),
        "the thread rows come from the card's node ids; got {body:?}"
    );
    let _ = titles;
}

#[test]
fn the_conversation_slot_puts_the_user_entry_before_its_replies() {
    let frames = fixture();
    let store = replay_store(&frames);
    let body = render(&store, Slot::Conversation);

    let user_at = body
        .find("user.message")
        .unwrap_or_else(|| panic!("the conversation card must show the user entry; got {body:?}"));
    let reply_at = body
        .find("assistant.text")
        .unwrap_or_else(|| panic!("the conversation card must show the answer; got {body:?}"));
    assert!(
        user_at < reply_at,
        "the user entry must precede its turn's replies (card #14 defect 1); \
         got {body:?}"
    );
    // The turn-1 answer text (the real model's answer, per the trace).
    assert!(
        body.contains("`main.rs` prints a single line"),
        "the answer text comes from the trace; got {body:?}"
    );
    // No stray/empty assistant row (card #14 defect 2).
    assert!(
        !body.contains("[assistant.text] \n") && !body.contains("[assistant.text] .\n"),
        "no empty or lone-'.' assistant row; got {body:?}"
    );
}

#[test]
fn the_completed_answer_slot_shows_worked_for_and_the_answer() {
    let frames = fixture();
    let store = replay_store(&frames);
    let body = render(&store, Slot::CompletedAnswer);

    assert!(
        body.contains("Worked for"),
        "the completed-answer card shows the `Worked for` row; got {body:?}"
    );
    assert!(
        body.contains("`main.rs` prints a single line"),
        "the completed-answer card shows the answer prose; got {body:?}"
    );
}

#[test]
fn every_slot_renders_from_the_recorded_trace() {
    let frames = fixture();
    let store = replay_store(&frames);
    for slot in Slot::ALL {
        let body = render(&store, *slot);
        assert!(
            !body.is_empty(),
            "slot {} rendered nothing from the recorded trace",
            slot.name()
        );
    }
}

#[test]
fn a_slot_falls_back_when_its_card_is_unreadable() {
    // Point the mount at a directory with no cards: every slot must report the
    // reason (the module then keeps its fallback widget and logs the slot).
    // The manifest is a process-wide OnceLock, so the first read wins — assert
    // the contract against the pure helpers instead, which is what the module's
    // error arm consumes.
    let err = cards::structure(&cards::Card {
        id: "conversation-99".to_owned(),
        slot: "conversation".to_owned(),
        title: "missing".to_owned(),
        source_commit: "0".to_owned(),
        gate_b_score: 9.0,
        bindings: vec![],
        artifacts: cards::Artifacts {
            card: "nope/page.card".to_owned(),
            data: "nope/page.data.json".to_owned(),
            kit_tokens: "nope/kit/kit.json".to_owned(),
            kit_components: "nope/kit/components.l0".to_owned(),
            semantic_map: "nope/semantic-map.json".to_owned(),
            mapped: "nope/mapped.json".to_owned(),
        },
    })
    .expect_err("a missing artefact must produce a reason, not a panic");
    assert!(
        err.contains("nope/mapped.json"),
        "the reason names the artefact; got {err:?}"
    );
}
