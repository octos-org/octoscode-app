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
use octoscode_module::l0_host;
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

/// The prompt the recorded trace's turn answers.
const USER_PROMPT: &str = "In one short paragraph: what does main.rs in this workspace print, and why?";

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

/// Lower one slot against a store — the module's own mount path
/// (`l0_host::slot_body`), so the test asserts what the UI shows.
fn lower(store: &Arc<Store>, slot: Slot) -> String {
    let ui = std::sync::Mutex::new(FlowUi::default());
    let ctx = bindings::Ctx::new(store, &ui);
    l0_host::slot_body(slot, &|id| bindings::query(&ctx, id))
        .unwrap_or_else(|e| panic!("slot {} must lower: {e}", slot.name()))
}

#[test]
fn the_thread_list_slot_lists_the_opened_session() {
    let frames = fixture();
    let store = replay_store(&frames);
    let body = lower(&store, Slot::ThreadList);

    let titles: Vec<String> = store.sessions().into_iter().map(|s| s.title.clone().unwrap_or_default()).collect();
    assert!(
        body.contains(SESSION) || store.sessions().iter().any(|s| s.id == SESSION),
        "the opened session {SESSION:?} must be listed by the thread-list card; \
         sessions={:?} body={body:?}",
        store.sessions().iter().map(|s| &s.id).collect::<Vec<_>>()
    );
    // It is a RENDER, not a text dump: the DSL the L0 runtime draws, with the
    // card's own node ids and the design kit's widget names.
    // A RENDER, not a text dump: the design kit's own widgets, with one
    // button per thread row.
    assert!(
        body.contains("KitButton") && body.contains("DesignNativeButton"),
        "the thread-list slot is the lowered card DSL, built from the kit; got {body:?}"
    );
    assert!(
        body.matches("DesignNativeButton").count() >= 5,
        "one control per thread row (5 rows + new chat); got {body:?}"
    );
    // The live row reaches the card. The fixture's session has no title (the
    // trace holds no `session/list` reply), so the card shows the session id,
    // which is what makes the row identifiable.
    assert!(
        body.contains(SESSION),
        "the live row reaches the card (its id when untitled); got {body:?}"
    );
    let _ = titles;
}

#[test]
fn the_conversation_slot_puts_the_user_entry_before_its_replies() {
    let frames = fixture();
    let store = replay_store(&frames);
    let body = lower(&store, Slot::Conversation);

    // It is the lowered card DSL, not a text dump.
    assert!(
        body.contains("DesignSurface") && body.contains(" := "),
        "the conversation slot is the lowered card DSL; got {body:?}"
    );
    // The user prompt and the answer both reach the card, in that order (the
    // live values are injected into the card's own `copy` entries, so the
    // rendered DSL carries them in the card's authored order).
    let user_at = body
        .find(USER_PROMPT)
        .unwrap_or_else(|| panic!("the card must carry the user prompt; got {body:?}"));
    let answer_at = body
        .find("`main.rs` prints a single line")
        .unwrap_or_else(|| panic!("the card must carry the answer; got {body:?}"));
    assert!(
        user_at < answer_at,
        "the user entry must precede its turn's replies (card #14 defect 1); got {body:?}"
    );
    // The answer prose the card shows is the live one, and it is not empty
    // (card #14 defect 2's "no (nearly) empty answer row", now stated as what
    // it is: the assistant text must carry real content).
    // The answer region carries the live answer, and that text is longer than a
    // stray marker (card #14 defect 2). The prose node is the one holding the
    // trace's answer, so assert on the content itself rather than a node name
    // (the renderer names nodes positionally).
    assert!(
        body.contains("`main.rs` prints a single line"),
        "the answer prose reaches the card; got {body:?}"
    );
    assert!(
        body.contains("DesignSurface"),
        "the conversation slot is built from the design kit; got {body:?}"
    );
}

#[test]
fn the_completed_answer_slot_shows_worked_for_and_the_answer() {
    let frames = fixture();
    let store = replay_store(&frames);
    let body = lower(&store, Slot::CompletedAnswer);

    assert!(
        body.contains("Worked for"),
        "the completed-answer card shows the `Worked for` row; got {body:?}"
    );
    assert!(
        body.contains("KitButton") && body.contains("DesignNativeButton"),
        "the `Worked for` row is the card's own kit button; got {body:?}"
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
        let body = lower(&store, *slot);
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
            kit_dir: Some("nope/kit".to_owned()),
        },
    })
    .expect_err("a missing artefact must produce a reason, not a panic");
    assert!(
        err.contains("nope/mapped.json"),
        "the reason names the artefact; got {err:?}"
    );
}
