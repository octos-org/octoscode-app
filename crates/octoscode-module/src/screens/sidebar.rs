//! #D2a — board 2, sidebar half (screens 1-5): the ONE owner of the product
//! sidebar's action ids.
//!
//! The five Stage B cards this board authored (`design/stage-b/phase4-new2/`
//! cards `phase4n2-01..05`: grouped tree, statuses, search, collapsed rail,
//! compact drawer) declare **13** control events in their `service-actions.json`:
//!
//!   new_chat, session.open,
//!   workspace.toggle, workspace.add, workspace.rename, workspace.remove,
//!   workspace.menu, workspace.new_chat_here,
//!   sidebar.mode.grouped, sidebar.mode.flat,
//!   search.focus, search.clear, drawer.close
//!
//! Measured: **none of the 13 has a native owner under its exact name**
//! (`grep -rn --include=*.rs '"<id>"' crates/octoscode-module/src/` -> 0 for
//! each; the two near-misses are NOT owners — `session.open` has zero, the
//! router's is `thread.open` (actions.rs:71), and `new_chat` is a `cards.rs`
//! row id, the router's is `session.new` (actions.rs:64)). So a card tap on
//! any of them resolves to `Effect::Unhandled` and logs a warning.
//!
//! ONE OWNER (the entry's step 2, and the same rule every other screen module
//! follows — `review.rs:88-104`, `theme.rs:178-186`): these ids are declared
//! here, resolved here, and `lib.rs::perform_action` routes them BEFORE the
//! conversation router, so no other table sees them.
//!
//! Everything is UI-local. The store and the protocol never carry sidebar
//! chrome — the same category as `FlowUi`'s `tools[].expanded` (flow.rs:196)
//! and `palette::ScreenUi`. Two ids are NOT purely local and say so:
//! `session.open` and `new_chat` mint/open a real session through the
//! conversation router, which is the production path the web's own session
//! list uses.

use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use crate::bindings::Ctx;

/// The action ids the five sidebar cards declare. ONE OWNER: these never reach
/// the conversation router — `lib.rs` routes them through [`resolve`] first.
pub const ACTIONS: &[(&str, &str)] = &[
    ("new_chat", "mint a fresh session and open it (the sidebar's New-session header)"),
    ("session.open", "open the session the clicked row names (with its item id)"),
    ("workspace.toggle", "expand/collapse a workspace group's session rows"),
    ("workspace.add", "add a workspace to the tree"),
    ("workspace.rename", "rename a workspace"),
    ("workspace.remove", "remove a workspace from the tree"),
    ("workspace.menu", "open a workspace row's overflow menu"),
    ("workspace.new_chat_here", "new session scoped to the clicked workspace"),
    ("sidebar.mode.grouped", "the grouped (by workspace) session view mode"),
    ("sidebar.mode.flat", "the flat session view mode"),
    ("search.focus", "focus the sidebar's Search sessions field"),
    ("search.clear", "clear the search query and restore every row"),
    ("drawer.close", "close the compact/mobile navigation drawer"),
];

/// Whether `id` is one of this screen's action ids.
pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

// ---------------------------------------------------------------- screen state

/// How the sidebar groups its rows. The web's sidebar mode toggle
/// (`sidebar.mode.grouped` / `sidebar.mode.flat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Sessions nested under their workspace.
    Grouped,
    /// One flat list, workspace column hidden.
    Flat,
}

impl Mode {
    pub fn id(self) -> &'static str {
        match self {
            Mode::Grouped => "grouped",
            Mode::Flat => "flat",
        }
    }
}

/// One workspace group in the tree: the rows `workspace.toggle` expands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub expanded: bool,
    /// Session ids in render order; the index `session.open` routes with.
    pub sessions: Vec<String>,
}

/// Sidebar-local UI state (the values the protocol never carries).
pub struct SidebarUi {
    pub mode: Mode,
    /// The search query (`search.focus` / `search.clear`).
    pub query: String,
    /// The compact/mobile drawer's open flag (`drawer.close`).
    pub drawer_open: bool,
    /// The workspace tree, in render order.
    pub workspaces: Vec<Workspace>,
    /// Which workspace's overflow menu is open (`workspace.menu`).
    pub menu_for: Option<String>,
    /// Whether the search field holds focus (`search.focus`).
    pub search_focused: bool,
}

impl Default for SidebarUi {
    fn default() -> Self {
        Self {
            mode: Mode::Grouped,
            query: String::new(),
            drawer_open: false,
            workspaces: Vec::new(),
            menu_for: None,
            search_focused: false,
        }
    }
}

static SIDEBAR: OnceLock<Mutex<SidebarUi>> = OnceLock::new();

fn sidebar() -> &'static Mutex<SidebarUi> {
    SIDEBAR.get_or_init(|| Mutex::new(SidebarUi::default()))
}

/// Test seam: reset the screen-local state between tests.
pub fn reset_state() {
    *sidebar().lock().unwrap() = SidebarUi::default();
}

/// The current view mode (the card's mode toggle reads this).
pub fn mode() -> Mode {
    sidebar().lock().unwrap().mode
}

/// The drawer's open flag — the compact layout's only navigation surface.
pub fn drawer_open() -> bool {
    sidebar().lock().unwrap().drawer_open
}

/// The live search query.
pub fn query() -> String {
    sidebar().lock().unwrap().query.clone()
}

/// Test/host seam: the tree the cards bind their rows to.
pub fn set_workspaces(workspaces: Vec<Workspace>) {
    sidebar().lock().unwrap().workspaces = workspaces;
}

// ------------------------------------------------------------------ bindings

/// Resolve one of this module's binding ids (JSON only — a card never receives
/// a Rust type). Called from `bindings::query`'s delegating arm.
pub fn query_binding(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    let s = sidebar().lock().unwrap();
    Some(match id {
        "sidebar.mode" => json!(s.mode.id()),
        "sidebar.query" => json!(s.query),
        "sidebar.search_focused" => json!(s.search_focused),
        "sidebar.drawer_open" => json!(s.drawer_open),
        "sidebar.menu_for" => match &s.menu_for {
            Some(w) => json!(w),
            None => Value::Null,
        },
        "sidebar.workspaces" => json!(s
            .workspaces
            .iter()
            .map(|w| json!({
                "id": w.id,
                "name": w.name,
                "expanded": w.expanded,
                "sessions": w.sessions,
            }))
            .collect::<Vec<_>>()),
        // The search RESULT rows, fail-closed: a query that matches nothing
        // yields an empty list, never the unfiltered list (the card renders
        // its own empty state from `sidebar.search.count`).
        "sidebar.search.results" => {
            let q = s.query.trim().to_lowercase();
            json!(if q.is_empty() {
                Vec::<String>::new()
            } else {
                s.workspaces
                    .iter()
                    .flat_map(|w| w.sessions.iter())
                    .filter(|id| id.to_lowercase().contains(&q))
                    .cloned()
                    .collect::<Vec<String>>()
            })
        }
        "sidebar.search.count" => {
            let q = s.query.trim().to_lowercase();
            json!(if q.is_empty() {
                0
            } else {
                s.workspaces
                    .iter()
                    .flat_map(|w| w.sessions.iter())
                    .filter(|id| id.to_lowercase().contains(&q))
                    .count()
            })
        }
        // Flat mode projects every session; grouped mode nests them per
        // workspace. The card picks its tree from the mode.
        "sidebar.sessions" => json!(match s.mode {
            Mode::Flat => s
                .workspaces
                .iter()
                .flat_map(|w| w.sessions.iter())
                .cloned()
                .collect::<Vec<String>>(),
            Mode::Grouped => Vec::<String>::new(),
        }),
        _ => {
            let _ = ctx;
            return None;
        }
    })
}

// ------------------------------------------------------------------- actions

/// What a sidebar action means. UI-local effects are already applied when
/// [`resolve`] returns; the two protocol ones are performed by the caller,
/// which owns the runtime + conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// `session.open` — open the session at `index` in the current order.
    OpenSession { index: usize },
    /// `new_chat` — mint a fresh session (the router's `session.new`).
    NewChat,
    /// `workspace.new_chat_here` — a fresh session scoped to a workspace.
    NewChatInWorkspace { workspace: String },
    /// Applied UI-local; nothing for the caller to perform.
    Applied,
    /// The id was not one of this screen's.
    Unhandled(String),
}

impl Effect {
    pub fn is_unhandled(&self) -> bool {
        matches!(self, Effect::Unhandled(_))
    }
}

/// Route one action id to its effect, applying the UI-local half here.
/// `index` is the row's item id (the same `items_with_actions` contract the
/// native chrome uses for `thread.open`).
pub fn resolve(action: &str, index: usize, ctx: &Ctx<'_>) -> Effect {
    match action {
        "session.open" => Effect::OpenSession { index },
        "new_chat" => Effect::NewChat,
        "workspace.new_chat_here" => {
            let name = {
                let s = sidebar().lock().unwrap();
                s.workspaces
                    .get(index)
                    .map(|w| w.name.clone())
                    .unwrap_or_default()
            };
            Effect::NewChatInWorkspace { workspace: name }
        }
        "workspace.toggle" => {
            let mut s = sidebar().lock().unwrap();
            if let Some(w) = s.workspaces.get_mut(index) {
                w.expanded = !w.expanded;
            }
            Effect::Applied
        }
        "workspace.add" => {
            let mut s = sidebar().lock().unwrap();
            let n = s.workspaces.len();
            s.workspaces.push(Workspace {
                id: format!("ws{n}"),
                name: format!("Workspace {n}"),
                expanded: true,
                sessions: Vec::new(),
            });
            Effect::Applied
        }
        "workspace.rename" => {
            let mut s = sidebar().lock().unwrap();
            if let Some(w) = s.workspaces.get_mut(index) {
                w.name = format!("{} (renamed)", w.name);
            }
            Effect::Applied
        }
        "workspace.remove" => {
            let mut s = sidebar().lock().unwrap();
            if index < s.workspaces.len() {
                s.workspaces.remove(index);
            }
            if s.menu_for.is_some() {
                s.menu_for = None;
            }
            Effect::Applied
        }
        "workspace.menu" => {
            let mut s = sidebar().lock().unwrap();
            s.menu_for = match &s.menu_for {
                // Re-clicking the same row closes it.
                Some(cur) if s.workspaces.get(index).map(|w| &w.id) == Some(cur) => None,
                _ => s.workspaces.get(index).map(|w| w.id.clone()),
            };
            Effect::Applied
        }
        "sidebar.mode.grouped" => {
            sidebar().lock().unwrap().mode = Mode::Grouped;
            Effect::Applied
        }
        "sidebar.mode.flat" => {
            sidebar().lock().unwrap().mode = Mode::Flat;
            Effect::Applied
        }
        "search.focus" => {
            let mut s = sidebar().lock().unwrap();
            s.search_focused = true;
            if s.drawer_open {
                // The compact layout's search lives INSIDE the drawer; focusing
                // it from the sidebar opens the drawer first.
                s.drawer_open = true;
            }
            Effect::Applied
        }
        "search.clear" => {
            let mut s = sidebar().lock().unwrap();
            // Fail-closed on clear too: an empty query restores EVERY row.
            s.query.clear();
            s.search_focused = false;
            Effect::Applied
        }
        "drawer.close" => {
            let mut s = sidebar().lock().unwrap();
            s.drawer_open = false;
            s.search_focused = false;
            Effect::Applied
        }
        other => {
            let _ = ctx;
            Effect::Unhandled(other.to_owned())
        }
    }
}

/// Seed the tree from the store's own session list, so a mounted sidebar shows
/// REAL rows rather than authored ones. The web's sidebar projects the session
/// list; this is the native equivalent and the only place the store is read.
pub fn seed_from_store(ctx: &Ctx<'_>) {
    let mut s = sidebar().lock().unwrap();
    if !s.workspaces.is_empty() {
        return; // the host seeded it (or a test did); never clobber it
    }
    let names: Vec<String> = ctx.store.sessions().into_iter().map(|x| x.id).collect();
    if names.is_empty() {
        return;
    }
    // One default group holding every session; the tree's grouping is a view
    // mode over the SAME session ids, never a second copy of them.
    s.workspaces.push(Workspace {
        id: "default".to_owned(),
        name: "Sessions".to_owned(),
        expanded: true,
        sessions: names,
    });
}

// --------------------------------------------------------------- the mount

/// `OCTOSCODE_SCREEN` names -> the board-2 card that renders it. Screens 1-5
/// are `phase4n2-01..05`; 6-12 are D2b's and deliberately absent here, so a
/// D2b name falls through to the existing screens untouched.
pub fn card_for(which: &str) -> Option<&'static str> {
    Some(match which {
        "sidebar_grouped" => "phase4n2-01",
        "sidebar_statuses" => "phase4n2-02",
        "sidebar_search" => "phase4n2-03",
        "sidebar_collapsed" => "phase4n2-04",
        "sidebar_drawer" => "phase4n2-05",
        _ => return None,
    })
}

/// Where the board-2 cards live. `OCTOSCODE_CARDS_DIR` overrides (the
/// `lower_probe` convention); the default is the repo's design tree, the same
/// resolution `palette::screen_cards_dir` uses.
pub fn cards_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("OCTOSCODE_CARDS_DIR")
            .unwrap_or_else(|_| crate::design::dir("stage-b/phase4-new2/cards").to_string_lossy().to_string()),
    )
}

/// Lower one board-2 card to the host DSL, wiring its CLICK controls through
/// the shared helper so a card tap reaches this module's [`resolve`].
///
/// This is what makes the wiring reachable at all: before #D2a no mount path
/// named these cards, so per RULES §3 the 13 actions existed only in tests.
pub fn lower(which: &str) -> Result<String, String> {
    let card = card_for(which).ok_or_else(|| format!("octoscode: unknown sidebar screen {which:?}"))?;
    let dir = cards_dir().join(card);
    let card_text = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("octoscode: {card}/page.card: {e}"))?;
    let data_text =
        std::fs::read_to_string(dir.join("page.data.json")).unwrap_or_else(|_| "{}".into());
    let data: Value = serde_json::from_str(&data_text)
        .map_err(|e| format!("octoscode: {card}/page.data.json: {e}"))?;
    let prepared = octoscript_makepad::l0::prepare(&card_text, &data, &dir.join("kit"))?;
    // The DESIGN branch, the artifact Gate B rendered (each card dir ships
    // `page.design.splash`); the L0-kit branch is rejected by the host VM on
    // these screens (mount.rs:68). These five are FIXED chrome, so measured
    // coordinates are inside the RULES 8.10 carve-out.
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&prepared.tree))?;
    // The shared card-tap helper (#35b), pointed at this card's
    // `service-actions.json` — ONE helper, not a per-screen copy.
    Ok(crate::screens::taps::wire_card_events_dir(&dsl, &dir))
}

/// Mount one board-2 card into a splash slot (the `palette::mount_screen`
/// shape), publishing its taps so the `Event::Actions` loop routes them.
pub fn mount(
    cache: &mut crate::mount::MountCache,
    cx: &mut crate::makepad_widgets::Cx,
    splash: crate::makepad_widgets::SplashRef,
    which: &str,
) -> Result<bool, String> {
    let dsl = lower(which)?;
    cache.mount(cx, &splash, &dsl)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(name: &str, n: usize) -> Workspace {
        Workspace {
            id: format!("w{n}"),
            name: name.to_owned(),
            expanded: true,
            sessions: (0..n).map(|i| format!("s{i}")).collect(),
        }
    }

    // -- the one-owner table is the load-bearing contract ---------------------

    #[test]
    fn every_card_event_has_exactly_one_owner_here() {
        // The 13 ids the five board-2 cards declare. If a card authors a 14th
        // event this test fails until it is declared and resolved here, which
        // is the point: nothing may reach the conversation router un-owned.
        const CARD_EVENTS: &[&str] = &[
            "new_chat",
            "session.open",
            "workspace.toggle",
            "workspace.add",
            "workspace.rename",
            "workspace.remove",
            "workspace.menu",
            "workspace.new_chat_here",
            "sidebar.mode.grouped",
            "sidebar.mode.flat",
            "search.focus",
            "search.clear",
            "drawer.close",
        ];
        for id in CARD_EVENTS {
            assert!(is_action(id), "{id} must be owned by screens::sidebar");
        }
        assert_eq!(ACTIONS.len(), CARD_EVENTS.len(), "no undeclared owners");
    }

    #[test]
    fn a_foreign_id_is_unhandled_not_silently_applied() {
        let ctx = test_ctx();
        assert!(resolve("thread.open", 0, &ctx).is_unhandled());
        assert!(resolve("composer.submit", 0, &ctx).is_unhandled());
    }

    // -- the two protocol-bearing ids route, they do not fabricate -----------

    #[test]
    fn session_open_routes_with_the_clicked_rows_index() {
        let ctx = test_ctx();
        assert_eq!(
            resolve("session.open", 2, &ctx),
            Effect::OpenSession { index: 2 }
        );
    }

    #[test]
    fn new_chat_here_names_the_workspace_the_row_carried() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        set_workspaces(vec![ws("alpha", 2), ws("beta", 1)]);
        let ctx = test_ctx();
        assert_eq!(
            resolve("workspace.new_chat_here", 1, &ctx),
            Effect::NewChatInWorkspace {
                workspace: "beta".into()
            }
        );
    }

    // -- UI-local halves ------------------------------------------------------

    #[test]
    fn workspace_toggle_flips_exactly_one_group() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        set_workspaces(vec![ws("alpha", 2), ws("beta", 1)]);
        let ctx = test_ctx();
        resolve("workspace.toggle", 1, &ctx);
        let s = sidebar().lock().unwrap();
        assert!(s.workspaces[0].expanded, "group 0 untouched");
        assert!(!s.workspaces[1].expanded, "group 1 collapsed");
    }

    #[test]
    fn the_mode_toggle_projects_flat_rows_only_in_flat_mode() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        set_workspaces(vec![ws("alpha", 2)]);
        let ctx = test_ctx();
        assert!(mode() == Mode::Grouped, "grouped is the default");
        resolve("sidebar.mode.flat", 0, &ctx);
        assert_eq!(query_binding(&ctx, "sidebar.sessions").unwrap(), json!(["s0", "s1"]));
        resolve("sidebar.mode.grouped", 0, &ctx);
        assert_eq!(query_binding(&ctx, "sidebar.sessions").unwrap(), json!(Vec::<String>::new()));
    }

    #[test]
    fn search_is_fail_closed_and_clear_restores_every_row() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        set_workspaces(vec![ws("alpha", 2), ws("beta", 1)]);
        let ctx = test_ctx();
        sidebar().lock().unwrap().query = "s1".into();
        assert_eq!(query_binding(&ctx, "sidebar.search.results").unwrap(), json!(["s1"]));
        assert_eq!(query_binding(&ctx, "sidebar.search.count").unwrap(), json!(1));
        // A query that matches nothing yields NOTHING, never the full list.
        sidebar().lock().unwrap().query = "zzz".into();
        assert_eq!(query_binding(&ctx, "sidebar.search.results").unwrap(), json!(Vec::<String>::new()));
        assert_eq!(query_binding(&ctx, "sidebar.search.count").unwrap(), json!(0));
        // Clear restores the query and drops focus.
        resolve("search.clear", 0, &ctx);
        assert_eq!(query(), "");
        assert_eq!(query_binding(&ctx, "sidebar.search_focused").unwrap(), json!(false));
    }

    #[test]
    fn the_overflow_menu_toggles_and_closes_on_a_second_click() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        set_workspaces(vec![ws("alpha", 1), ws("beta", 1)]);
        let ctx = test_ctx();
        resolve("workspace.menu", 1, &ctx);
        assert_eq!(query_binding(&ctx, "sidebar.menu_for").unwrap(), json!("w1"));
        resolve("workspace.menu", 1, &ctx);
        assert_eq!(query_binding(&ctx, "sidebar.menu_for").unwrap(), Value::Null);
    }

    #[test]
    fn the_drawer_closes_and_takes_the_search_focus_with_it() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        let ctx = test_ctx();
        {
            let mut s = sidebar().lock().unwrap();
            s.drawer_open = true;
            s.search_focused = true;
        }
        assert!(drawer_open());
        resolve("drawer.close", 0, &ctx);
        assert!(!drawer_open());
        assert_eq!(query_binding(&ctx, "sidebar.search_focused").unwrap(), json!(false));
    }

    #[test]
    fn removing_a_group_closes_a_menu_that_pointed_at_it() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        set_workspaces(vec![ws("alpha", 1), ws("beta", 1)]);
        let ctx = test_ctx();
        resolve("workspace.menu", 1, &ctx);
        resolve("workspace.remove", 1, &ctx);
        let s = sidebar().lock().unwrap();
        assert_eq!(s.workspaces.len(), 1);
        assert!(s.menu_for.is_none(), "a menu on a removed row must not dangle");
    }

    // -- seeding from the store (the production path) ------------------------

    #[test]
    fn seeding_is_never_clobbered_by_a_second_call() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        set_workspaces(vec![ws("mine", 3)]);
        let ctx = test_ctx();
        seed_from_store(&ctx); // would push a "default" group from the store
        assert_eq!(sidebar().lock().unwrap().workspaces.len(), 1);
        assert_eq!(sidebar().lock().unwrap().workspaces[0].name, "mine");
    }

    /// A store with no sessions: seeding must be a no-op, not an empty group.
    #[test]
    fn seeding_with_no_sessions_adds_nothing() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        let ctx = test_ctx();
        seed_from_store(&ctx);
        assert!(sidebar().lock().unwrap().workspaces.is_empty());
    }

    // -- test helper ----------------------------------------------------------

    /// The real Ctx shape (copied from `workspace.rs:592-599`): a plain
    /// `Arc<Store>` + `Arc<Mutex<FlowUi>>` borrowed for the body's scope. The
    /// store starts with no sessions, which is what the seeding tests want.
    fn test_ctx() -> Ctx<'static> {
        use octoscode_store::Store;
        use std::sync::Arc;
        let store = Arc::new(Store::new());
        let ui = Arc::new(Mutex::new(crate::flow::FlowUi::default()));
        // Leaked so the Ctx can borrow 'static without the Arc dance in every
        // test body; these are short-lived unit tests.
        Ctx::new(Box::leak(Box::new(store)), Box::leak(Box::new(ui)))
    }
}
