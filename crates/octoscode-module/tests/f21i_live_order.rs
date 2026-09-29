//! Card #21i — the prose row must carry the FULL canonical paragraph under the
//! real (interleaved) frame order, not just the post-receipt tail.
//!
//! ## The defect
//!
//! `live-gate/evidence/g3-completed.png` showed only
//! `` ` and is formatted into the `{}` placeholder.`` (its `/snap` reports that
//! string as the Markdown's own `body`, so the *text* was wrong, not the layout).
//!
//! The real server interleaves: on the live `trace.jsonl` the
//! `assistant_persisted` receipt landed at frame 94 of 105, and the 11
//! `assistant_delta`s after it joined to EXACTLY that 45-char tail. The store
//! appended those deltas to the receipt — and because `finalize_assistant` had
//! closed the entry, they opened a SECOND assistant entry. `screen::timeline_rows`
//! renders the turn's LAST assistant entry (`.next_back()`), so the row showed
//! only the tail.
//!
//! The web is explicit that the receipt wins
//! (`apps/web/src/features/timeline/model.ts:655-663`, "Receipt finality wins over
//! delivery order"): the persisted body is the segment's canonical text and later
//! deltas are dropped. The store fix lives in
//! `octoscode-store/src/timeline.rs`; a store-level test pins the fold, and this
//! test pins the **row the screen renders** and the text its binding resolves.
//!
//! ## Scope / limit
//!
//! This asserts the *text* path end to end (store fold → the screen's row pick →
//! the component's own binding → the lowered DSL the app mounts). It does not open
//! a window; the pixel-level half is the live capture named in the report.
use std::sync::Arc;

use octoscode_module::bindings::Ctx;
use octoscode_module::components::{self, ItemKind};
use octoscode_module::screen;
use octoscode_store::{EntryKind, Session, Store};

/// The live order + strings, verbatim from
/// `live-gate/evidence/trace.jsonl` (turn `01a0ed1d-9d4d-7343-9aff-c5d5e6d20ccc`).
const PRE: &str = "`main.rs` prints a single line, `5` — `main` calls `println!(\"{}\", add(2, 3))`, and the helper `add(a: i32, b: i32) -> i32 { a + b }` returns the sum of its arguments, so `2 + 3` evaluates to `5`";
const PERSISTED: &str = "`main.rs` prints a single line, `5` — `main` calls `println!(\"{}\", add(2, 3))`, and the helper `add(a: i32, b: i32) -> i32 { a + b }` returns the sum of its arguments, so `2 + 3` evaluates to `5` and is formatted into the `{}` placeholder.";
const LATE: &str = "` and is formatted into the `{}` placeholder.";

const SESSION: &str = "dsflash:main";
const TURN: &str = "01a0ed1d-9d4d-7343-9aff-c5d5e6d20ccc";

/// A store holding the live gate's turn in the live arrival order.
fn store_live_order() -> Arc<Store> {
    let store = Arc::new(Store::new());
    store.set_active(Some(SESSION.into()));
    store.set_sessions(vec![Session {
        id: SESSION.into(),
        title: Some("dsflash:main".into()),
        message_count: 1,
        updated_at: None,
        last_prompt: None,
        active_turn: false,
    }]);
    let tl = &store.domains.session.timeline;
    tl.upsert_user_message(
        SESSION,
        TURN,
        "In one short paragraph: what does main.rs in this workspace print, and why?",
        serde_json::json!({}),
    );
    // The receipt lands MID-stream; more deltas follow it (the real interleaving).
    tl.append_delta(SESSION, Some(TURN), EntryKind::ASSISTANT_TEXT, PRE);
    tl.finalize_assistant(SESSION, TURN, PERSISTED);
    tl.append_delta(SESSION, Some(TURN), EntryKind::ASSISTANT_TEXT, LATE);
    store
}

/// The assistant-prose row's own binding resolves to the full paragraph — so
/// every word the card lists is painted, and the tail alone is not enough.
#[test]
fn item_i_the_prose_row_binding_carries_the_whole_paragraph() {
    let store = store_live_order();

    // The screen's row list must select the assistant row (`.next_back()` of the
    // turn's assistant entries) — this is the pick that used to land on the tail.
    let rows = screen::timeline_rows(&store, false);
    let prose = rows
        .iter()
        .find(|r| r.kind == ItemKind::AssistantProse)
        .expect("the turn renders an assistant-prose row");

    // Resolve that row's copies through the component's own binding (the path the
    // app's cache uses), then lower it exactly as the app mounts it.
    let ui = screen::flow_ui();
    let copies = {
        let ctx = Ctx::new(&store, &ui);
        components::item_copies(ItemKind::AssistantProse, &ctx, prose.index)
            .expect("the prose row's binding resolves")
    };
    let text = copies
        .iter()
        .find(|(copy, _)| copy == "answer_md_text")
        .map(|(_, v)| v.clone())
        .expect("the prose component binds `answer_md_text`");

    assert_eq!(
        text, PERSISTED,
        "the row must carry the canonical paragraph, not the post-receipt tail"
    );
    assert_ne!(
        text, LATE,
        "the row must never degrade to the 45-char post-receipt tail"
    );

    // Census: every word the card names survives, in order.
    for want in [
        "main.rs",
        "prints",
        "single",
        "line",
        "calls",
        "println!",
        "add(2, 3)",
        "helper",
        "returns",
        "arguments",
        "evaluates",
        "formatted",
        "placeholder.",
    ] {
        assert!(
            text.contains(want),
            "the paragraph must still paint {want:?}; got:\n{text}"
        );
    }

    let dsl = components::lower(ItemKind::AssistantProse, "0", &copies)
        .expect("the prose lowers");
    // The ledger escapes the value into its own `en:` string (`l0_host::set_copy`),
    // so compare against the escaped form exactly as the app writes it.
    let escaped = PERSISTED
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    assert!(
        dsl.contains(&escaped),
        "the lowered DSL the app mounts must carry the whole paragraph; got:\n{dsl}"
    );
}
