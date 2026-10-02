//! A1 — the conversation rows on the production path: store -> display order
//! (`screen::timeline_rows_folded`) -> copies (`components::item_copies`) ->
//! the fluid lowering (`components::lower`), and the list template's hit
//! target.
use std::sync::{Arc, Mutex};

use octoscode_module::bindings::Ctx;
use octoscode_module::components::{self, ItemKind};
use octoscode_module::flow::FlowUi;
use octoscode_module::screen;
use octoscode_store::{EntryKind, Session, Store};

/// One settled turn: a prompt, three tool calls (the capture seed the
/// hidden-app checks use, `OCTOSCODE_SYNTHETIC_TOOLS`), an answer.
fn settled_turn_with_tools() -> Arc<Store> {
    let store = Arc::new(Store::new());
    store.set_sessions(vec![Session {
        id: "s1".into(),
        title: Some("t".into()),
        message_count: 2,
        updated_at: None,
        last_prompt: None,
        active_turn: false,
    }]);
    store.set_active(Some("s1".into()));
    let tl = &store.domains.session.timeline;
    tl.upsert_user_message("s1", "t1", "list the workspace", serde_json::json!({}));
    components::seed_tool_calls(&store, "s1", "t1");
    tl.append("s1", Some("t1".into()), EntryKind::ASSISTANT_TEXT, "Two entries.".into());
    tl.finalize_assistant("s1", "t1", "Two entries.");
    tl.close_turn("s1", "t1");
    store.domains.turn.started("t1");
    store.domains.turn.set_terminal("t1", "completed");
    store
}

/// The settled turn reads in the web's order — prompt, the "Worked for"
/// header, the three calls as ONE compact card (first / middle / last), the
/// answer, its actions — and each call is one header line: `title · target`
/// and `Done · N s`.
#[test]
fn a_settled_turn_lowers_to_one_compact_tool_card() {
    let store = settled_turn_with_tools();
    let ui = Arc::new(Mutex::new(FlowUi::default()));
    let rows = screen::timeline_rows_folded(&store, false, &[]);
    let kinds: Vec<ItemKind> = rows.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        vec![
            ItemKind::UserBubble,
            ItemKind::WorkedFor,
            ItemKind::ToolCell,
            ItemKind::ToolCell,
            ItemKind::ToolCell,
            ItemKind::AssistantProse,
            ItemKind::AnswerActions,
        ]
    );
    let want = [
        ("list_dir", ".", "vec4(5.0, 5.0, 1.0, 1.0)"),
        ("list_dir", ".octos", "vec4(1.0, 1.0, 1.0, 1.0)"),
        ("read_file", ".octos-workspace.toml", "vec4(1.0, 1.0, 5.0, 5.0)"),
    ];
    for (k, row) in rows.iter().filter(|r| r.kind == ItemKind::ToolCell).enumerate() {
        let copies = {
            let ctx = Ctx::new(&store, &ui);
            components::item_copies(row.kind, &ctx, row.index, row.turn.as_deref()).expect("tool copies")
        };
        let dsl = components::lower(row.kind, &k.to_string(), &copies).expect("tool row lowers");
        let (title, target, radii) = want[k];
        assert!(dsl.contains(&format!("text: {title:?}")), "{dsl}");
        assert!(dsl.contains(&format!("text: {target:?}")), "{dsl}");
        assert!(dsl.contains("text: \"Done · 0 s\""), "status on the header line: {dsl}");
        assert!(dsl.contains(radii), "card segment {k}: {dsl}");
        assert!(!dsl.contains("width: 371"), "no artboard-width card");
    }
    // The header counts the calls it folds.
    let copies = {
        let ctx = Ctx::new(&store, &ui);
        components::item_copies(ItemKind::WorkedFor, &ctx, 0, Some("t1")).expect("worked-for copies")
    };
    let dsl = components::lower(ItemKind::WorkedFor, "0", &copies).expect("worked-for lowers");
    assert!(dsl.contains("text: \"3 tool calls\""), "{dsl}");
}

/// The list template's `row_hit` must never size its row: a Button's own
/// content (an empty label line + the theme padding) made the hit 48 px on
/// the phone shell, taller than the 40 px tool rows it covers — the rows
/// grew, gaps split the tool card and each hit reached into the next row.
#[test]
fn the_row_hit_never_sizes_its_row() {
    let lib = include_str!("../src/lib.rs");
    let at = lib.find("row_hit := Button {").expect("the template's row hit");
    // The block ends with its last property (the walks hold `}` themselves).
    let block = &lib[at..at + lib[at..].find("border_color_2").expect("block end")];
    for prop in [
        "width: Fill height: Fill",
        "margin: 0 padding: 0",
        "label_walk: Walk{width: 0 height: 0}",
        "icon_walk: Walk{width: 0 height: 0}",
    ] {
        assert!(block.contains(prop), "row_hit must carry `{prop}`: {block}");
    }
}
