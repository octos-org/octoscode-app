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
    components::seed_tool_calls(&store, "s1", "t1", false);
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

/// No hit target in the list item's Fit template: a Fill Button there took
/// the phone shell's touch height (48 px Android / 44 px iOS over 40 px tool
/// rows) — the rows grew and gaps split the tool card. The clickable rows
/// carry their own hit over a FIXED-height header instead.
#[test]
fn the_clickable_rows_carry_their_own_fixed_height_hits() {
    let lib = include_str!("../src/lib.rs");
    let at = lib.find("TimelineItemTpl := View {").expect("the timeline template");
    let tpl = &lib[at..at + lib[at..].find("empty_state := View").expect("template end")];
    assert!(!tpl.contains(":= Button"), "no hit in the Fit template: {tpl}");

    let m = octoscode_module::conv_layout::Metrics::for_window(360.0, false);
    let t = octoscode_module::fluid::ToolView {
        title: "list_dir".into(),
        target: ".".into(),
        state: "done".into(),
        secs: Some(0),
        output: String::new(),
    };
    let tool = octoscode_module::fluid::tool_row("0", &t, octoscode_module::fluid::GroupPos::First, false, &m);
    let h = octoscode_module::fluid::scale(m.density).row_h;
    let head = tool.find(&format!("View{{width: Fill height: {h} flow: Overlay")).expect("a fixed-height header overlay");
    assert!(tool[head..].contains("tool_hit := Button{width: Fill height: Fill"), "{tool}");
    let worked = octoscode_module::fluid::worked_for("0", "Worked for 2s", 3, true, &m);
    let row = worked.find("View{width: Fill height: 28 flow: Overlay").expect("a 28 px overlay row");
    assert!(worked[row..].contains("worked_hit := Button{width: Fill height: Fill"), "{worked}");
}

/// The running seed: the last call still runs and the turn is live — the
/// rows end with the working row, no "Worked for" header yet, and the last
/// tool row says Running.
#[test]
fn a_live_turn_ends_with_the_working_row() {
    let store = Arc::new(Store::new());
    store.set_sessions(vec![Session {
        id: "s1".into(),
        title: Some("t".into()),
        message_count: 1,
        updated_at: None,
        last_prompt: None,
        active_turn: false,
    }]);
    store.set_active(Some("s1".into()));
    store.domains.session.timeline.upsert_user_message("s1", "t1", "go", serde_json::json!({}));
    components::seed_tool_calls(&store, "s1", "t1", true);
    let ui = Arc::new(Mutex::new(FlowUi::default()));
    let live = {
        let ctx = Ctx::new(&store, &ui);
        octoscode_module::bindings::query(&ctx, "turn.active").and_then(|v| v.as_bool()).unwrap_or(false)
    };
    assert!(live, "the seeded turn is live");
    let rows = screen::timeline_rows_folded(&store, live, &[]);
    let kinds: Vec<ItemKind> = rows.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        vec![ItemKind::UserBubble, ItemKind::ToolCell, ItemKind::ToolCell, ItemKind::ToolCell, ItemKind::WorkingRow]
    );
    let copies = {
        let ctx = Ctx::new(&store, &ui);
        components::item_copies(ItemKind::ToolCell, &ctx, 2, Some("t1")).expect("tool copies")
    };
    let dsl = components::lower(ItemKind::ToolCell, "2", &copies).expect("lowers");
    assert!(dsl.contains("text: \"Running\""), "{dsl}");
    assert!(dsl.contains("icon_spinner.svg"), "the running mark: {dsl}");
}

/// The Chinese seed lowers with the sans CJK face (never the calligraphic
/// WenKai as the first CJK member) in both the bubble and the answer.
#[test]
fn the_chinese_turn_lowers_with_the_sans_cjk_face() {
    let store = settled_turn_with_tools();
    components::seed_zh_turn(&store, "s1", "t2");
    let ui = Arc::new(Mutex::new(FlowUi::default()));
    let rows = screen::timeline_rows_folded(&store, false, &[]);
    let zh: Vec<_> = rows.iter().filter(|r| r.turn.as_deref() == Some("t2")).collect();
    assert_eq!(zh.len(), 7, "bubble, worked-for, 3 tools, answer, actions");
    for row in zh.iter().filter(|r| matches!(r.kind, ItemKind::UserBubble | ItemKind::AssistantProse)) {
        let copies = {
            let ctx = Ctx::new(&store, &ui);
            components::item_copies(row.kind, &ctx, row.index, row.turn.as_deref()).expect("copies")
        };
        let dsl = components::lower(row.kind, "9", &copies).expect("lowers");
        let noto = dsl.find("NotoSansSC").expect("the sans CJK member");
        if let Some(wenkai) = dsl.find("LXGWWenKai") {
            assert!(noto < wenkai, "WenKai only as the lazy rare-glyph fallback");
        }
        assert!(dsl.contains("工作区") || dsl.contains("请用中文回答"), "the Chinese text rides the row");
    }
}
