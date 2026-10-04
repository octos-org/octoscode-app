//! A36 — Skills and the MCP status are reachable by a CLICK in Settings, not
//! only by typing `/skills` or `/mcp`.
//!
//! Settings gains a **Capabilities** section (after Model) whose rows open the
//! EXISTING surfaces: Skills -> the A5/A10/A31 Skills dialog
//! (`dialog.open.skills`), MCP servers -> the board-3 runtime inventory on its
//! MCP tab (`b3.open.mcp`). Each row shows only when the server advertises the
//! method its surface reads (`profile/skills/list`, `mcp/status/list`), like
//! Settings > Model's "All models" row (`profile/llm/list`).
//!
//! The MCP view states the truth about management: octos has no UI-protocol
//! method to add, remove or configure an MCP server (servers come from the
//! server's config), so the view says servers are configured on the
//! server and offers no add/remove.
//!
//! The click itself (the laid-out row hit -> the action) is proven by the walk
//! `tools/walk/a36_capabilities.py` on desktop and phone.
use std::sync::Mutex;

use octoscode_module::chrome::Section;
use octoscode_module::screens::board3::{host, inventory};
use octoscode_module::screens::{dialog, settings};
use octoscode_store::Store;

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    host::reset();
    settings::reset_state();
    g
}

fn store_with(methods: &[&str]) -> Store {
    let store = Store::new();
    store.set_connection("Live".into(), true);
    store.domains.config.set_supported_methods(methods.iter().map(|m| m.to_string()).collect());
    store
}

#[test]
fn settings_has_a_capabilities_section_after_model() {
    let _g = lock();
    let ids: Vec<&str> = Section::ALL.iter().map(|s| s.id()).collect();
    let model = ids.iter().position(|i| *i == "model").expect("model section");
    assert_eq!(ids.get(model + 1), Some(&"capabilities"), "Capabilities follows Model: {ids:?}");
    assert_eq!(Section::from_id("capabilities"), Some(Section::Capabilities));
    assert_eq!(Section::Capabilities.title(), "Capabilities");
    // Its nav cell / rail chip route through the one owner of settings ids.
    assert!(settings::owns("settings.section.capabilities"));
    assert_eq!(settings::apply_ui("settings.section.capabilities"), settings::UiEffect::Open);
    assert_eq!(settings::snapshot().section, Section::Capabilities);
}

#[test]
fn capability_rows_show_only_what_the_server_advertises() {
    let _g = lock();
    assert!(settings::capability_rows(&store_with(&[])).is_empty(), "nothing advertised: no row");
    assert_eq!(settings::capability_rows(&store_with(&["profile/skills/list"])), vec!["skills"]);
    assert_eq!(
        settings::capability_rows(&store_with(&["mcp/status/list", "profile/skills/list"])),
        vec!["skills", "mcp"],
        "the rows keep their order"
    );
    // Every row names its hit id and the action its click performs.
    let rows: Vec<(&str, &str, &str)> =
        settings::CAPABILITY_ROWS.iter().map(|r| (r.id, r.hit, r.action)).collect();
    assert_eq!(
        rows,
        vec![
            ("skills", "set_cap_skills", "dialog.open.skills"),
            ("mcp", "set_cap_mcp", "b3.open.mcp"),
            // A36b — board 5's third row.
            ("memory", "set_cap_memory", "b3.open.memory"),
        ]
    );
}

#[test]
fn the_skills_row_opens_the_existing_skills_dialog() {
    let _g = lock();
    let row = settings::CAPABILITY_ROWS.iter().find(|r| r.id == "skills").expect("skills row");
    // One owner: the dialog host (lib.rs routes `dialog::is_action` first).
    assert!(dialog::is_action(row.action));
    assert!(!host::routes(row.action));
    assert_eq!(dialog::resolve(row.action), dialog::Effect::Open(dialog::Dialog::Skills));
}

#[test]
fn the_mcp_row_opens_the_inventory_on_its_mcp_tab() {
    let _g = lock();
    let store = store_with(&["mcp/status/list", "tool/status/list"]);
    let row = settings::CAPABILITY_ROWS.iter().find(|r| r.id == "mcp").expect("mcp row");
    assert!(host::routes(row.action), "a board-3 id");
    assert!(!dialog::is_action(row.action));
    // Even when the dialog was last left on Tools, the row lands on MCP.
    host::state().inv.tab = inventory::Tab::Tools;
    assert_eq!(host::perform(row.action, 0, &store), host::Outcome::Spawn(host::Job::InventoryLoad));
    assert_eq!(host::open_dialog(), Some(host::Dialog::Inventory));
    assert_eq!(host::state().inv.tab, inventory::Tab::Mcp);
}

/// The live octos reply (private serve, a6ea8505): no servers, all counts 0.
fn live_empty_mcp() -> serde_json::Value {
    serde_json::json!({"profile_id": "dsflash", "session_id": "dsflash:a36probe", "servers": [],
                       "summary": {"connected": 0, "connecting": 0, "failed": 0, "disabled": 0}})
}

#[test]
fn the_mcp_view_says_servers_are_configured_on_the_server_and_offers_no_management() {
    let _g = lock();
    let store = store_with(&["mcp/status/list", "tool/status/list"]);
    host::perform("b3.open.mcp", 0, &store);
    {
        let mut st = host::state();
        st.inv.loading = false;
        st.inv.servers = inventory::parse_mcp(&live_empty_mcp(), "dsflash:a36probe", "dsflash");
        assert!(st.inv.servers.is_some(), "the live reply parses");
    }
    let lowered = host::lower_open(&store).expect("the inventory lowers");
    let dsl = &lowered.dsl;
    assert!(
        dsl.contains(inventory::MCP_MANAGED_ON_SERVER),
        "the MCP view says where servers are managed"
    );
    assert!(dsl.contains("b3_inv_mcp_note"), "the note has an id the walk can read");
    for fake in ["Add server", "Remove server", "Edit server"] {
        assert!(!dsl.contains(fake), "no fake management control: {fake}");
    }
    // The copy reads in Chinese too (the native supplement).
    assert_ne!(octoscode_module::i18n::zh_for(inventory::MCP_MANAGED_ON_SERVER), None);
}

#[test]
fn mcp_and_tools_tabs_show_only_their_own_inventory() {
    let _g = lock();
    let store = store_with(&["mcp/status/list", "tool/status/list"]);
    host::perform("b3.open.mcp", 0, &store);
    {
        let mut st = host::state();
        st.inv.loading = false;
        st.inv.servers = inventory::parse_mcp(&serde_json::json!({
            "profile_id":"dsflash", "session_id":"dsflash:a36probe",
            "servers":[{"id":"server-1","display_name":"Test MCP","transport":"stdio","status":"connected","tool_count":0,"tools":[]}],
            "summary":{"connected":1,"connecting":0,"failed":0,"disabled":0}
        }), "dsflash:a36probe", "dsflash");
        st.inv.tools = Some(("default".into(), vec![inventory::ToolRow {
            name:"builtin-demo".into(), category:"runtime".into(),
            status:"enabled".into(), policy:"allow".into(), aliases:vec![], backend:None, detail:None,
        }]));
    }
    let mcp = host::lower_open(&store).unwrap();
    assert!(mcp.dsl.contains("Test MCP"));
    assert!(!mcp.dsl.contains("builtin-demo"));
    assert!(!mcp.dsl.contains("b3_inv_count"));
    host::state().inv.tab = inventory::Tab::Tools;
    let tools = host::lower_open(&store).unwrap();
    assert!(tools.dsl.contains("builtin-demo"));
    assert!(!tools.dsl.contains("Test MCP"));
    assert!(!tools.dsl.contains("b3_inv_summary"));
}
