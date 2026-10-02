//! #D2a / A3 — board 2, sidebar half (screens 1-5): the ONE owner of the
//! product sidebar's action ids AND the projection the native sidebar draws.
//!
//! Screens (design/stage-a/phase4-new2/atlas.png, operator-approved board 2):
//! 1 grouped tree, 2 per-session statuses, 3 search, 4 a collapsed workspace
//! with its overflow menu, 5 the compact (phone) drawer.
//!
//! The web reference is `features/shell/ProductSidebar.tsx` (+ its CSS): the
//! tree groups sessions by workspace (`workspace-session-catalog.ts`), each
//! row carries a status slot (`StatusDot`, `ProductSidebar.tsx:1316-1347`)
//! and a relative time (`relative-time.ts`), search is fail-closed, and below
//! 760 px the sidebar becomes a drawer (`use-compact-layout.ts`,
//! `NavigationSurface.tsx`).
//!
//! ## Action ids (ONE owner: `lib.rs::perform_action` routes these here first)
//!
//! The 13 ids the five Stage B cards declare (`phase4n2-01..05`
//! `service-actions.json`) plus the three the native chrome adds
//! (`drawer.open`, `sidebar.sort`, `workspace.rename.cancel`):
//!
//!   new_chat, session.open,
//!   workspace.toggle, workspace.add, workspace.rename, workspace.remove,
//!   workspace.menu, workspace.new_chat_here,
//!   sidebar.mode.grouped, sidebar.mode.flat, sidebar.sort,
//!   search.focus, search.clear, drawer.open, drawer.close,
//!   workspace.rename.cancel, sidebar.collapse, sidebar.expand, search.open
//!
//! Workspace-scoped ids address a GROUP by its index in the last projection
//! ([`project`] records the order), the same item-index contract the native
//! chrome uses for `thread.open`. `session.open` carries the session's index
//! in the STORE's list, so it routes through the router's own `thread.open`.
//!
//! Everything here is UI-local except `session.open` / `new_chat` /
//! `workspace.new_chat_here`, which the caller performs through the
//! conversation router (the production path the web's session list uses).

use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use octoscode_store::Store;

use crate::bindings::Ctx;
use crate::i18n::{tr, tr1, tr_with};

/// The action ids the sidebar owns. ONE OWNER: these never reach the
/// conversation router — `lib.rs` routes them through [`resolve`] first.
pub const ACTIONS: &[(&str, &str)] = &[
    ("new_chat", "mint a fresh session and open it (the sidebar's New chat row)"),
    ("session.open", "open the session the clicked row names (its store index)"),
    ("workspace.toggle", "expand/collapse a workspace group's session rows"),
    ("workspace.add", "add a workspace (opens the folder browser)"),
    ("workspace.rename", "start renaming a workspace (the overflow menu)"),
    ("workspace.remove", "remove a workspace from the sidebar"),
    ("workspace.menu", "open/close a workspace row's overflow menu"),
    ("workspace.new_chat_here", "new session in the clicked workspace"),
    ("sidebar.mode.grouped", "the grouped (by workspace) view mode"),
    ("sidebar.mode.flat", "the flat (All) view mode"),
    ("search.focus", "focus the sidebar's Search chats field"),
    ("search.clear", "clear the search query and restore every row"),
    ("drawer.close", "close the compact/mobile navigation drawer"),
    // Native chrome additions (no Stage B card declares these):
    ("drawer.open", "open the compact/mobile navigation drawer"),
    ("sidebar.sort", "cycle the session order (Recent / Oldest)"),
    ("workspace.rename.cancel", "abandon an in-progress workspace rename"),
    // The web's collapsed rail (`.collapsed`, 56 px; ProductSidebar.tsx:532
    // "Collapse sidebar" / "Expand sidebar").
    ("sidebar.collapse", "collapse the sidebar to its 56 px icon rail"),
    ("sidebar.expand", "expand the sidebar from its rail"),
    ("search.open", "expand the sidebar and focus Search chats (the rail's search)"),
];

/// Whether `id` is one of this screen's action ids.
pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

// ---------------------------------------------------------------- screen state

/// How the sidebar groups its rows (the board's "By workspace | All").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Sessions nested under their workspace.
    Grouped,
    /// One flat list.
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

/// The session order (the board's "Recent ▾"; the web's three order modes,
/// `ProductSidebarOrderMode`, ProductSidebar.tsx:39 and :1125-1142).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Newest first (the web's `updated`).
    Recent,
    /// Oldest first (the web's `oldest`, "Least recently opened").
    Oldest,
    /// The server's own order (the web's `manual`, "Fixed order").
    Fixed,
}

impl Sort {
    pub fn label(self) -> &'static str {
        match self {
            Sort::Recent => "Recent",
            Sort::Oldest => "Oldest",
            Sort::Fixed => "Fixed",
        }
    }
}

/// Sidebar-local UI state (values the protocol never carries).
#[derive(Debug, Clone)]
pub struct SidebarUi {
    pub mode: Mode,
    pub sort: Sort,
    /// The search query (the field's live text).
    pub query: String,
    /// Whether the search field holds focus (`search.focus`).
    pub search_focused: bool,
    /// The compact/mobile drawer's open flag.
    pub drawer_open: bool,
    /// Collapsed workspace groups, by workspace key.
    pub collapsed: BTreeSet<String>,
    /// Which workspace's overflow menu is open (its key).
    pub menu_for: Option<String>,
    /// Which workspace is being renamed (its key).
    pub renaming: Option<String>,
    /// Display-name overrides set by Rename, by workspace key.
    pub names: HashMap<String, String>,
    /// Workspaces removed from the sidebar, by key.
    pub hidden: BTreeSet<String>,
    /// The group keys in the order the LAST projection drew them — the
    /// index contract every workspace-scoped action resolves through.
    pub last_groups: Vec<GroupKey>,
    /// A folder-browser request raised by `workspace.add` (the host consumes
    /// it with [`take_add_request`]).
    pub add_requested: bool,
    /// The sidebar is collapsed to its icon rail (desktop only).
    pub rail: bool,
    /// The search field should take key focus (the host consumes it).
    pub focus_search: bool,
}

/// A projected group's identity: its key and the path a new session there
/// opens with (`None` when the workspace path is unknown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupKey {
    pub key: String,
    pub label: String,
    pub path: Option<String>,
}

impl Default for SidebarUi {
    fn default() -> Self {
        Self {
            mode: Mode::Grouped,
            sort: Sort::Recent,
            query: String::new(),
            search_focused: false,
            drawer_open: false,
            collapsed: BTreeSet::new(),
            menu_for: None,
            renaming: None,
            names: HashMap::new(),
            hidden: BTreeSet::new(),
            last_groups: Vec::new(),
            add_requested: false,
            rail: false,
            focus_search: false,
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

/// A copy of the whole UI state (the chrome reads it once per frame).
pub fn snapshot() -> SidebarUi {
    sidebar().lock().unwrap().clone()
}

/// The current view mode.
pub fn mode() -> Mode {
    sidebar().lock().unwrap().mode
}

/// The drawer's open flag — the compact layout's only navigation surface.
pub fn drawer_open() -> bool {
    sidebar().lock().unwrap().drawer_open
}

/// Open or close the drawer directly (the host's Escape / backdrop paths).
pub fn set_drawer_open(open: bool) {
    let mut s = sidebar().lock().unwrap();
    s.drawer_open = open;
    if !open {
        s.menu_for = None;
    }
}

/// The live search query.
pub fn query() -> String {
    sidebar().lock().unwrap().query.clone()
}

/// The search field's live text (the TextInput's `changed` action). Returns
/// whether the query actually changed.
pub fn set_query(text: &str) -> bool {
    let mut s = sidebar().lock().unwrap();
    if s.query == text {
        return false;
    }
    s.query = text.to_owned();
    s.search_focused = true;
    true
}

/// Commit an in-progress rename with the field's text. A blank name clears
/// the override (the workspace shows its folder name again).
pub fn rename_commit(text: &str) -> Option<String> {
    let mut s = sidebar().lock().unwrap();
    let key = s.renaming.take()?;
    let name = text.trim();
    if name.is_empty() {
        s.names.remove(&key);
    } else {
        s.names.insert(key.clone(), name.to_owned());
    }
    Some(key)
}

/// The key of the workspace being renamed, if any.
pub fn renaming() -> Option<String> {
    sidebar().lock().unwrap().renaming.clone()
}

/// The open overflow menu's group (key + its index in the last projection).
pub fn menu_for() -> Option<(usize, String)> {
    let s = sidebar().lock().unwrap();
    let key = s.menu_for.clone()?;
    let i = s.last_groups.iter().position(|g| g.key == key)?;
    Some((i, key))
}

/// Close the overflow menu (a click outside it).
pub fn close_menu() {
    sidebar().lock().unwrap().menu_for = None;
}

/// Consume a pending search-focus request (the host focuses the field).
pub fn take_focus_search() -> bool {
    std::mem::take(&mut sidebar().lock().unwrap().focus_search)
}

/// Consume a pending `workspace.add` request (the host opens the browser).
pub fn take_add_request() -> bool {
    std::mem::take(&mut sidebar().lock().unwrap().add_requested)
}

// ------------------------------------------------------------ the projection

/// A session's work state — the web's `BackgroundSessionState`
/// (`background-session-status.ts:4-5`), with the board's labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Running,
    Waiting,
    Done,
    Failed,
}

impl Status {
    /// The accessible label (`defaultStatusLabel`, ProductSidebar.tsx:1336-1347,
    /// with the board's "Done" for completed).
    pub fn label(self) -> &'static str {
        match self {
            Status::Idle => "Idle",
            Status::Running => "Running",
            Status::Waiting => "Waiting for input",
            Status::Done => "Done",
            Status::Failed => "Failed",
        }
    }
}

/// One drawn row of the sidebar tree, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// A workspace header: `group` indexes [`Projection::groups`].
    Group {
        group: usize,
        label: String,
        count: usize,
        expanded: bool,
        menu_open: bool,
    },
    /// A session row. `store_index` is the session's index in the store's
    /// list (the `thread.open` contract). `matched` is the CHAR range of the
    /// search hit inside `title`, when a query is active.
    Session {
        store_index: usize,
        title: String,
        time: String,
        status: Status,
        selected: bool,
        matched: Option<(usize, usize)>,
        /// A29 — the Session holds a `/btw` aside and is NOT the one on
        /// screen (the operator's 2026-10-02 decision: a small marker on its
        /// row while you are on another Session).
        aside: Option<octoscode_store::domains::btw::Mark>,
    },
    /// A grey empty-state line.
    Note(String),
    /// The "Clear search" link.
    ClearSearch,
    /// The hairline between groups while searching (boards 3/4).
    Divider,
}

/// The whole drawn sidebar tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Projection {
    pub groups: Vec<GroupKey>,
    pub rows: Vec<Row>,
}

/// One session as the projection sees it.
struct Item {
    store_index: usize,
    id: String,
    title: String,
    updated_ms: Option<u64>,
    group: String,
}

/// The current time, ms since the epoch (the relative-time clock).
pub fn now_ms() -> u64 {
    crate::screens::recents::now_ms()
}

/// Project the store + the sidebar UI state onto the drawn tree, and record
/// the group order for the index-addressed workspace actions.
pub fn project(store: &Store, now: u64) -> Projection {
    project_recorded(store, now, &recent_paths())
}

/// [`project`] with the remembered workspaces passed in (hermetic tests).
pub fn project_recorded(store: &Store, now: u64, recents: &[String]) -> Projection {
    let ui = sidebar().lock().unwrap().clone();
    let proj = project_with(store, &ui, now, recents);
    sidebar().lock().unwrap().last_groups = proj.groups.clone();
    proj
}

/// The remembered workspaces (the web's recents), newest first.
fn recent_paths() -> Vec<String> {
    let storage = crate::screens::recents::store();
    crate::screens::recents::load_recent_workspaces(
        &*storage,
        &crate::screens::recents::endpoint(),
    )
    .into_iter()
    .map(|w| w.path)
    .collect()
}

/// The PURE projection (tests drive it with an explicit UI state and clock).
pub fn project_with(store: &Store, ui: &SidebarUi, now: u64, recents: &[String]) -> Projection {
    let sessions = store.sessions();
    let active = store.active_session();
    let fallback_root = active
        .as_deref()
        .and_then(|a| store.domains.session.workspace_root(a))
        .or_else(|| std::env::var("OCTOS_WORKSPACE_CWD").ok().filter(|s| !s.trim().is_empty()));

    let items: Vec<Item> = sessions
        .iter()
        .enumerate()
        .map(|(i, s)| Item {
            store_index: i,
            id: s.id.clone(),
            title: title_of(s),
            updated_ms: s.updated_at.as_deref().and_then(parse_rfc3339_ms),
            group: store
                .domains
                .session
                .workspace_root(&s.id)
                .or_else(|| fallback_root.clone())
                .unwrap_or_default(),
        })
        .filter(|it| !ui.hidden.contains(&it.group))
        .collect();

    // Groups: every workspace a session lives in, plus the remembered ones.
    let mut keys: Vec<String> = Vec::new();
    for it in &items {
        if !keys.contains(&it.group) {
            keys.push(it.group.clone());
        }
    }
    for path in recents {
        if !keys.contains(path) && !ui.hidden.contains(path) {
            keys.push(path.clone());
        }
    }
    // Recent: the group with the newest session first; groups with no dated
    // session keep their first-seen order after the dated ones.
    let newest = |key: &str| {
        items
            .iter()
            .filter(|it| it.group == key)
            .filter_map(|it| it.updated_ms)
            .max()
    };
    let mut keyed: Vec<(usize, Option<u64>, String)> =
        keys.into_iter().enumerate().map(|(i, k)| (i, newest(&k), k)).collect();
    keyed.sort_by(|a, b| match (a.1, b.1) {
        _ if ui.sort == Sort::Fixed => a.0.cmp(&b.0),
        (Some(x), Some(y)) => match ui.sort {
            Sort::Recent => y.cmp(&x),
            Sort::Oldest => x.cmp(&y),
            Sort::Fixed => std::cmp::Ordering::Equal,
        },
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.0.cmp(&b.0),
    });
    let groups: Vec<GroupKey> = keyed
        .into_iter()
        .map(|(_, _, key)| GroupKey {
            label: ui
                .names
                .get(&key)
                .cloned()
                .unwrap_or_else(|| group_label(&key)),
            path: (!key.is_empty()).then(|| key.clone()),
            key,
        })
        .collect();

    let status_of = |it: &Item| session_status(store, &it.id, active.as_deref());
    let query = ui.query.trim().to_owned();
    let row_of = |it: &Item, matched: Option<(usize, usize)>| Row::Session {
        store_index: it.store_index,
        title: it.title.clone(),
        time: it
            .updated_ms
            .map(|t| relative_label(t, now))
            .unwrap_or_default(),
        status: status_of(it),
        selected: active.as_deref() == Some(it.id.as_str()),
        matched,
        aside: (active.as_deref() != Some(it.id.as_str()))
            .then(|| store.domains.btw.mark(&it.id))
            .flatten(),
    };
    fn ordered_by(sort: Sort, mut v: Vec<&Item>) -> Vec<&Item> {
        v.sort_by(|a, b| match (a.updated_ms, b.updated_ms) {
            _ if sort == Sort::Fixed => a.store_index.cmp(&b.store_index),
            (Some(x), Some(y)) => match sort {
                Sort::Recent => y.cmp(&x),
                Sort::Oldest => x.cmp(&y),
                Sort::Fixed => std::cmp::Ordering::Equal,
            },
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.store_index.cmp(&b.store_index),
        });
        v
    }

    let mut rows = Vec::new();
    match ui.mode {
        Mode::Flat => {
            let all = ordered_by(ui.sort, items.iter().collect());
            if query.is_empty() {
                rows.extend(all.into_iter().map(|it| row_of(it, None)));
                if rows.is_empty() {
                    rows.push(Row::Note(tr("No chats yet.").to_owned()));
                }
            } else {
                let hits: Vec<Row> = all
                    .into_iter()
                    .filter_map(|it| find_ci(&it.title, &query).map(|m| row_of(it, Some(m))))
                    .collect();
                if hits.is_empty() {
                    rows.push(Row::Note(tr1("No chats match \u{201c}{value0}\u{201d}", &query)));
                    rows.push(Row::ClearSearch);
                } else {
                    rows.extend(hits);
                }
            }
        }
        Mode::Grouped => {
            let mut any_miss = false;
            for (gi, g) in groups.iter().enumerate() {
                let members = ordered_by(ui.sort, items.iter().filter(|it| it.group == g.key).collect());
                let expanded = !ui.collapsed.contains(&g.key);
                if !query.is_empty() && gi > 0 {
                    rows.push(Row::Divider);
                }
                rows.push(Row::Group {
                    group: gi,
                    label: g.label.clone(),
                    count: members.len(),
                    expanded: expanded || !query.is_empty(),
                    menu_open: ui.menu_for.as_deref() == Some(g.key.as_str()),
                });
                if query.is_empty() {
                    if !expanded {
                        continue;
                    }
                    if members.is_empty() {
                        rows.push(Row::Note(tr("No chats yet.").to_owned()));
                    }
                    rows.extend(members.into_iter().map(|it| row_of(it, None)));
                } else {
                    let hits: Vec<Row> = members
                        .into_iter()
                        .filter_map(|it| {
                            find_ci(&it.title, &query).map(|m| row_of(it, Some(m)))
                        })
                        .collect();
                    if hits.is_empty() {
                        any_miss = true;
                        rows.push(Row::Note(tr_with(
                            "No chats in {value0} match \u{201c}{value1}\u{201d}",
                            &[("value0", &g.label), ("value1", &query)],
                        )));
                    } else {
                        rows.extend(hits);
                    }
                }
            }
            if groups.is_empty() {
                rows.push(Row::Note(if query.is_empty() {
                    tr("No chats yet.").to_owned()
                } else {
                    tr1("No chats match \u{201c}{value0}\u{201d}", &query)
                }));
                any_miss = !query.is_empty();
            }
            if any_miss {
                rows.push(Row::ClearSearch);
            }
        }
    }
    Projection { groups, rows }
}

/// The row title: the web's sidebar projection keeps title, then last prompt
/// (`workspace-session-catalog.ts:120`); a session with neither is a "New
/// chat" (`ProductSidebar.tsx:1223`, the board's copy).
fn title_of(s: &octoscode_store::Session) -> String {
    s.label_stem().unwrap_or_else(|| tr("New chat").to_owned())
}

/// A group's display label: the workspace folder name (`workspaceName`,
/// `workspace-recents.ts:81-85`), or "Sessions" when the path is unknown.
fn group_label(key: &str) -> String {
    if key.is_empty() {
        tr("Sessions").to_owned()
    } else {
        crate::screens::recents::workspace_name(key)
    }
}

/// The per-session status, the web's precedence
/// (`backgroundSessionState`, background-session-status.ts:8-27):
/// waiting > running > the newest settled turn (completed / failed) > idle.
pub fn session_status(store: &Store, id: &str, active: Option<&str>) -> Status {
    let is_active = active == Some(id);
    // A20 (parity row 250): Waiting is THIS Session's own interaction — an
    // approval or question whose recorded origin is `id`, selected or not
    // (a blocked background Session surfaces without selection,
    // `session-record-manager.ts:351-357`). Before, ANY pending approval made
    // the SELECTED row read Waiting (Session X's wait shown as Y's), and X's
    // own row did not.
    if store.domains.approval.waiting(id) {
        return Status::Waiting;
    }
    let listed_running = store
        .sessions()
        .iter()
        .any(|s| s.id == id && s.active_turn);
    if listed_running || (is_active && store.domains.turn.in_flight_count() > 0) {
        return Status::Running;
    }
    // The newest TERMINAL turn of this session's timeline decides
    // (`terminal = timeline.findLast(latestTurnOutcome)`,
    // SessionSidebar.tsx:65-98): completed -> completed, anything else ->
    // failed (an interrupted turn reads "Stopped", `terminal_label`).
    let entries = store.domains.session.timeline.entries(id);
    let mut seen: Vec<String> = Vec::new();
    for e in entries.iter().rev() {
        let Some(turn) = e.turn_id.as_deref() else { continue };
        if seen.iter().any(|t| t == turn) {
            continue;
        }
        seen.push(turn.to_owned());
        match store.domains.turn.terminal(turn).as_deref() {
            Some("completed") => return Status::Done,
            Some(_) => return Status::Failed,
            None => {}
        }
    }
    Status::Idle
}

/// Case-insensitive substring search, returning the CHAR range of the first
/// hit in `hay` (so the chrome can split the title around the highlight).
pub fn find_ci(hay: &str, needle: &str) -> Option<(usize, usize)> {
    let fold = |c: char| c.to_lowercase().next().unwrap_or(c);
    let h: Vec<char> = hay.chars().map(fold).collect();
    let n: Vec<char> = needle.chars().map(fold).collect();
    if n.is_empty() || n.len() > h.len() {
        return None;
    }
    (0..=h.len() - n.len())
        .find(|&i| h[i..i + n.len()] == n[..])
        .map(|i| (i, i + n.len()))
}

/// Split `text` into (before, hit, after) by a CHAR range.
pub fn split_chars(text: &str, range: (usize, usize)) -> (String, String, String) {
    let chars: Vec<char> = text.chars().collect();
    let (a, b) = (range.0.min(chars.len()), range.1.min(chars.len()));
    (
        chars[..a].iter().collect(),
        chars[a..b].iter().collect(),
        chars[b..].iter().collect(),
    )
}

/// The web's `formatRelativeTime` (relative-time.ts:1-17), bucket for
/// bucket: now / Nm / Nh / Nd / a short date. (The board's "Yesterday" is the
/// image's copy; the native label follows the web, "1d".)
pub fn relative_label(then_ms: u64, now_ms: u64) -> String {
    let elapsed = now_ms.saturating_sub(then_ms);
    const MIN: u64 = 60_000;
    const HOUR: u64 = 3_600_000;
    const DAY: u64 = 86_400_000;
    if elapsed < MIN {
        "now".to_owned()
    } else if elapsed < HOUR {
        format!("{}m", elapsed / MIN)
    } else if elapsed < DAY {
        format!("{}h", elapsed / HOUR)
    } else if elapsed < 7 * DAY {
        format!("{}d", elapsed / DAY)
    } else {
        short_date(then_ms)
    }
}

/// "Oct 1" for a timestamp (UTC civil date; no locale data in the app).
fn short_date(ms: u64) -> String {
    let days = (ms / 86_400_000) as i64;
    let (_, m, d) = civil_from_days(days);
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{} {}", MONTHS[(m - 1) as usize], d)
}

/// Parse an RFC 3339 timestamp (`2026-10-01T15:39:00Z`, `…+02:00`, optional
/// fraction) to ms since the epoch. `None` for anything else — the web's
/// `Date.parse` NaN, which shows no time.
pub fn parse_rfc3339_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !(b[10] == b'T' || b[10] == b' ') {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let mut rest = &s[19..];
    let mut frac_ms = 0i64;
    if let Some(r) = rest.strip_prefix('.') {
        let digits: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
        rest = &r[digits.len()..];
        let ms = format!("{:0<3}", &digits[..digits.len().min(3)]);
        frac_ms = ms.parse().unwrap_or(0);
    }
    let offset_s = if rest.is_empty() || rest == "Z" || rest == "z" {
        0
    } else {
        let sign = match rest.as_bytes()[0] {
            b'+' => 1,
            b'-' => -1,
            _ => return None,
        };
        let oh = rest.get(1..3)?.parse::<i64>().ok()?;
        let om = rest.get(4..6)?.parse::<i64>().ok()?;
        sign * (oh * 3600 + om * 60)
    };
    let days = days_from_civil(y, mo, d);
    let secs = days * 86_400 + h * 3600 + mi * 60 + se - offset_s;
    (secs >= 0).then(|| (secs * 1000 + frac_ms) as u64)
}

/// Format ms since the epoch as RFC 3339 UTC (`2026-10-01T15:39:00Z`) — the
/// shape `session/list` carries in `updated_at` (seeds and tests).
pub fn rfc3339_from_ms(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant's days-from-civil (proleptic Gregorian, UTC).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The inverse of [`days_from_civil`].
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ------------------------------------------------------------------ bindings

/// Resolve one of this module's binding ids (JSON only — a card never receives
/// a Rust type).
pub fn query_binding(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    let s = sidebar().lock().unwrap().clone();
    Some(match id {
        "sidebar.mode" => json!(s.mode.id()),
        "sidebar.sort" => json!(s.sort.label()),
        "sidebar.query" => json!(s.query),
        "sidebar.search_focused" => json!(s.search_focused),
        "sidebar.drawer_open" => json!(s.drawer_open),
        "sidebar.menu_for" => match &s.menu_for {
            Some(w) => json!(w),
            None => Value::Null,
        },
        "sidebar.groups" => {
            let p = project_with(ctx.store, &s, now_ms(), &[]);
            json!(p.groups.iter().map(|g| g.label.clone()).collect::<Vec<_>>())
        }
        "sidebar.search.count" => {
            let p = project_with(ctx.store, &s, now_ms(), &[]);
            json!(if s.query.trim().is_empty() {
                0
            } else {
                p.rows
                    .iter()
                    .filter(|r| matches!(r, Row::Session { .. }))
                    .count()
            })
        }
        _ => return None,
    })
}

// ------------------------------------------------------------------- actions

/// What a sidebar action means. UI-local effects are already applied when
/// [`resolve`] returns; the protocol ones are performed by the caller, which
/// owns the runtime + conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// `session.open` — open the session at the store `index`.
    OpenSession { index: usize },
    /// `new_chat` — mint a fresh session (the router's `session.new`).
    NewChat,
    /// `workspace.new_chat_here` — a fresh session in a workspace. `path` is
    /// the cwd the session opens with (`None` = the server's default).
    NewChatInWorkspace { workspace: String, path: Option<String> },
    /// `workspace.add` — open the folder browser (the host docks it).
    AddWorkspace,
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
pub fn resolve(action: &str, index: usize, ctx: &Ctx<'_>) -> Effect {
    let _ = ctx;
    let mut s = sidebar().lock().unwrap();
    let group = s.last_groups.get(index).cloned();
    match action {
        "session.open" => {
            // Opening from the drawer closes it (the web's compact surface
            // dismisses on select).
            s.drawer_open = false;
            s.menu_for = None;
            Effect::OpenSession { index }
        }
        "new_chat" => {
            s.drawer_open = false;
            s.menu_for = None;
            Effect::NewChat
        }
        "workspace.new_chat_here" => {
            s.menu_for = None;
            s.drawer_open = false;
            match group {
                Some(g) => Effect::NewChatInWorkspace { workspace: g.label, path: g.path },
                None => Effect::Unhandled(format!("{action}[{index}]")),
            }
        }
        "workspace.toggle" => match group {
            Some(g) => {
                if !s.collapsed.remove(&g.key) {
                    s.collapsed.insert(g.key);
                }
                s.menu_for = None;
                Effect::Applied
            }
            None => Effect::Unhandled(format!("{action}[{index}]")),
        },
        "workspace.add" => {
            s.menu_for = None;
            s.add_requested = true;
            Effect::AddWorkspace
        }
        "workspace.rename" => match group {
            Some(g) => {
                s.menu_for = None;
                s.renaming = Some(g.key);
                Effect::Applied
            }
            None => Effect::Unhandled(format!("{action}[{index}]")),
        },
        "workspace.rename.cancel" => {
            s.renaming = None;
            Effect::Applied
        }
        "workspace.remove" => match group {
            Some(g) => {
                s.hidden.insert(g.key.clone());
                s.collapsed.remove(&g.key);
                if s.menu_for.as_deref() == Some(g.key.as_str()) {
                    s.menu_for = None;
                }
                Effect::Applied
            }
            None => Effect::Unhandled(format!("{action}[{index}]")),
        },
        "workspace.menu" => match group {
            Some(g) => {
                s.menu_for = match &s.menu_for {
                    // Re-clicking the same row closes it.
                    Some(cur) if *cur == g.key => None,
                    _ => Some(g.key),
                };
                Effect::Applied
            }
            None => Effect::Unhandled(format!("{action}[{index}]")),
        },
        "sidebar.mode.grouped" => {
            s.mode = Mode::Grouped;
            Effect::Applied
        }
        "sidebar.mode.flat" => {
            s.mode = Mode::Flat;
            s.menu_for = None;
            Effect::Applied
        }
        "sidebar.sort" => {
            s.sort = match s.sort {
                Sort::Recent => Sort::Oldest,
                Sort::Oldest => Sort::Fixed,
                Sort::Fixed => Sort::Recent,
            };
            Effect::Applied
        }
        "search.focus" => {
            s.search_focused = true;
            Effect::Applied
        }
        "search.clear" => {
            // Fail-closed on clear too: an empty query restores EVERY row.
            s.query.clear();
            s.search_focused = false;
            Effect::Applied
        }
        "drawer.open" => {
            s.drawer_open = true;
            Effect::Applied
        }
        "sidebar.collapse" => {
            s.rail = true;
            s.menu_for = None;
            Effect::Applied
        }
        "sidebar.expand" => {
            s.rail = false;
            Effect::Applied
        }
        "search.open" => {
            // The rail's magnifier: expand, then focus Search chats.
            s.rail = false;
            s.search_focused = true;
            s.focus_search = true;
            Effect::Applied
        }
        "drawer.close" => {
            s.drawer_open = false;
            s.search_focused = false;
            s.menu_for = None;
            Effect::Applied
        }
        other => Effect::Unhandled(other.to_owned()),
    }
}

// --------------------------------------------------------------- the mount

/// `OCTOSCODE_SCREEN` names -> the board-2 card that renders it. Screens 1-5
/// are `phase4n2-01..05`; 6-12 are the settings cards (`screens::settings`).
/// These Stage B cards are the design record; the product sidebar is the
/// native chrome (`crate::chrome`), which needs no env to be reached.
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
    // `page.design.splash`); these five are FIXED chrome, so measured
    // coordinates are inside the RULES 8.10 carve-out.
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&prepared.tree))?;
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
    use octoscode_store::Session;
    use std::sync::Arc;

    const NOW: u64 = 1_790_000_000_000;

    fn session(id: &str, title: &str, ago_min: u64) -> Session {
        Session {
            id: id.into(),
            title: Some(title.into()),
            message_count: 1,
            updated_at: Some(iso(NOW - ago_min * 60_000)),
            last_prompt: None,
            active_turn: false,
        }
    }

    fn iso(ms: u64) -> String {
        let secs = (ms / 1000) as i64;
        let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
        let rem = secs.rem_euclid(86_400);
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
            rem / 3600,
            (rem % 3600) / 60,
            rem % 60
        )
    }

    /// The board-2 fixture: two workspaces, five threads.
    fn board_store() -> Arc<Store> {
        let store = Arc::new(Store::new());
        store.set_sessions(vec![
            session("s1", "Fix steer queue drop on reconnect", 2),
            session("s2", "Add session fork", 60),
            session("s3", "Review PR #2566", 26 * 60),
            session("s4", "Bump octos-core to a6ea8505", 120),
            session("s5", "Why is hydrate slow?", 27 * 60),
        ]);
        for id in ["s1", "s2", "s3"] {
            store.domains.session.set_workspace_root(id, "/home/user/src/octos");
        }
        for id in ["s4", "s5"] {
            store.domains.session.set_workspace_root(id, "/home/user/src/octoscode-app");
        }
        store.set_active(Some("s1".into()));
        store
    }

    fn rows_text(p: &Projection) -> Vec<String> {
        p.rows
            .iter()
            .map(|r| match r {
                Row::Group { label, count, expanded, .. } => {
                    format!("[{}{label} {count}]", if *expanded { "v " } else { "> " })
                }
                Row::Session { title, time, status, .. } => format!("{title} | {time} | {status:?}"),
                Row::Note(t) => format!("note: {t}"),
                Row::ClearSearch => "clear".into(),
                Row::Divider => "---".into(),
            })
            .collect()
    }

    // -- the one-owner table is the load-bearing contract ---------------------

    #[test]
    fn every_card_event_has_exactly_one_owner_here() {
        // The 13 ids the five board-2 cards declare, plus the three native
        // chrome additions. Nothing may reach the conversation router un-owned.
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
        const CHROME: &[&str] = &[
            "drawer.open",
            "sidebar.sort",
            "workspace.rename.cancel",
            "sidebar.collapse",
            "sidebar.expand",
            "search.open",
        ];
        for id in CHROME {
            assert!(is_action(id), "{id} (native chrome) must be owned here");
        }
        assert_eq!(ACTIONS.len(), CARD_EVENTS.len() + CHROME.len(), "no undeclared owners");
    }

    #[test]
    fn a_foreign_id_is_unhandled_not_silently_applied() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        let ctx = test_ctx();
        assert!(resolve("thread.open", 0, &ctx).is_unhandled());
        assert!(resolve("composer.submit", 0, &ctx).is_unhandled());
    }

    // -- the projection: board screens 1-4 --------------------------------

    #[test]
    fn grouped_mode_projects_the_board_1_tree() {
        let store = board_store();
        let ui = SidebarUi::default();
        let p = project_with(&store, &ui, NOW, &[]);
        assert_eq!(
            rows_text(&p),
            vec![
                "[v octos 3]",
                "Fix steer queue drop on reconnect | 2m | Idle",
                "Add session fork | 1h | Idle",
                "Review PR #2566 | 1d | Idle",
                "[v octoscode-app 2]",
                "Bump octos-core to a6ea8505 | 2h | Idle",
                "Why is hydrate slow? | 1d | Idle",
            ]
        );
        // The active session's row is the selected one.
        assert!(matches!(&p.rows[1], Row::Session { selected: true, store_index: 0, .. }));
    }

    #[test]
    fn flat_mode_lists_every_session_newest_first() {
        let store = board_store();
        let ui = SidebarUi { mode: Mode::Flat, ..Default::default() };
        let p = project_with(&store, &ui, NOW, &[]);
        let titles: Vec<String> = rows_text(&p);
        assert_eq!(titles.len(), 5);
        assert!(titles[0].starts_with("Fix steer queue"));
        assert!(titles[4].starts_with("Why is hydrate slow?"));
        // Oldest flips the order; Fixed keeps the server's (store) order.
        let ui = SidebarUi { mode: Mode::Flat, sort: Sort::Oldest, ..Default::default() };
        let p = project_with(&store, &ui, NOW, &[]);
        assert!(rows_text(&p)[0].starts_with("Why is hydrate slow?"));
        let ui = SidebarUi { mode: Mode::Flat, sort: Sort::Fixed, ..Default::default() };
        let p = project_with(&store, &ui, NOW, &[]);
        let fixed: Vec<String> = rows_text(&p).iter().map(|r| r.split(" |").next().unwrap().to_owned()).collect();
        assert_eq!(fixed[2], "Review PR #2566", "store order, not recency");
    }

    #[test]
    fn statuses_follow_the_web_precedence() {
        use octoscode_store::domains::approval::PendingQuestion;
        use octoscode_store::timeline::EntryKind;
        let store = board_store();
        // running: the listed active_turn flag
        let mut list = store.sessions();
        list[0].active_turn = true;
        store.set_sessions(list);
        // waiting: an outstanding user question for s2
        store.domains.approval.set_question(PendingQuestion {
            question_id: "q1".into(),
            session_id: "s2".into(),
            turn_id: "t2".into(),
            title: "Which branch?".into(),
            body: String::new(),
            questions: serde_json::Value::Null,
            ..Default::default()
        });
        // done / failed: the newest terminal turn of the session's timeline
        let tl = &store.domains.session.timeline;
        tl.append("s3", Some("t3".into()), EntryKind::ASSISTANT_TEXT, "ok".into());
        store.domains.turn.set_terminal("t3", "completed");
        tl.append("s4", Some("t4a".into()), EntryKind::ASSISTANT_TEXT, "a".into());
        store.domains.turn.set_terminal("t4a", "completed");
        tl.append("s4", Some("t4b".into()), EntryKind::ASSISTANT_TEXT, "b".into());
        store.domains.turn.set_terminal("t4b", "errored");
        let active = store.active_session();
        let st = |id: &str| session_status(&store, id, active.as_deref());
        assert_eq!(st("s1"), Status::Running);
        assert_eq!(st("s2"), Status::Waiting);
        assert_eq!(st("s3"), Status::Done);
        assert_eq!(st("s4"), Status::Failed, "the NEWEST terminal decides");
        assert_eq!(st("s5"), Status::Idle);
        // An interrupted newest turn is a failed outcome ("Stopped").
        tl.append("s3", Some("t3b".into()), EntryKind::ASSISTANT_TEXT, "x".into());
        store.domains.turn.set_terminal("t3b", "interrupted");
        assert_eq!(st("s3"), Status::Failed);
    }

    #[test]
    fn search_is_fail_closed_and_names_the_empty_group() {
        let store = board_store();
        let ui = SidebarUi { query: "hydrate".into(), ..Default::default() };
        let p = project_with(&store, &ui, NOW, &[]);
        assert_eq!(
            rows_text(&p),
            vec![
                "[v octos 3]",
                "note: No chats in octos match \u{201c}hydrate\u{201d}",
                "---",
                "[v octoscode-app 2]",
                "Why is hydrate slow? | 1d | Idle",
                "clear",
            ]
        );
        // The hit carries the highlight's char range.
        let hit = p.rows.iter().find_map(|r| match r {
            Row::Session { matched, title, .. } => Some((title.clone(), *matched)),
            _ => None,
        });
        let (title, m) = hit.expect("one hit");
        assert_eq!(split_chars(&title, m.unwrap()).1, "hydrate");
        // A query that matches nothing yields NOTHING, never the full list.
        let ui = SidebarUi { query: "zzz".into(), mode: Mode::Flat, ..Default::default() };
        let p = project_with(&store, &ui, NOW, &[]);
        assert_eq!(rows_text(&p), vec!["note: No chats match \u{201c}zzz\u{201d}", "clear"]);
    }

    #[test]
    fn search_is_case_insensitive_and_cjk_safe() {
        assert_eq!(find_ci("Why is HYDRATE slow?", "hydrate"), Some((7, 14)));
        assert_eq!(find_ci("修复 重连 队列", "重连"), Some((3, 5)));
        assert_eq!(split_chars("修复 重连 队列", (3, 5)).1, "重连");
        assert_eq!(find_ci("abc", ""), None);
    }

    #[test]
    fn a_collapsed_group_keeps_its_count_and_hides_its_rows() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        let store = board_store();
        let ctx_store = store.clone();
        let p = project_recorded(&ctx_store, NOW, &[]);
        assert_eq!(p.groups.len(), 2);
        let ctx = ctx_for(&store);
        resolve("workspace.toggle", 0, &ctx);
        let p = project_recorded(&store, NOW, &[]);
        assert_eq!(rows_text(&p)[0], "[> octos 3]");
        assert_eq!(rows_text(&p)[1], "[v octoscode-app 2]");
        // Toggle again restores it.
        resolve("workspace.toggle", 0, &ctx);
        let p = project_recorded(&store, NOW, &[]);
        assert_eq!(rows_text(&p)[0], "[v octos 3]");
    }

    #[test]
    fn the_overflow_menu_rename_and_remove_address_the_projected_group() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        let store = board_store();
        let ctx = ctx_for(&store);
        project_recorded(&store, NOW, &[]);
        resolve("workspace.menu", 1, &ctx);
        assert_eq!(menu_for().map(|m| m.0), Some(1));
        resolve("workspace.menu", 1, &ctx);
        assert_eq!(menu_for(), None, "a second click closes it");
        // Rename: start, commit, and the label changes.
        resolve("workspace.rename", 1, &ctx);
        assert!(renaming().is_some());
        rename_commit("app");
        let p = project_recorded(&store, NOW, &[]);
        assert_eq!(p.groups[1].label, "app");
        // Remove hides the group AND its sessions.
        resolve("workspace.remove", 1, &ctx);
        let p = project_recorded(&store, NOW, &[]);
        assert_eq!(p.groups.len(), 1);
        assert!(!rows_text(&p).iter().any(|r| r.contains("hydrate")));
    }

    #[test]
    fn new_chat_here_carries_the_workspace_path() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        let store = board_store();
        let ctx = ctx_for(&store);
        project_recorded(&store, NOW, &[]);
        assert_eq!(
            resolve("workspace.new_chat_here", 1, &ctx),
            Effect::NewChatInWorkspace {
                workspace: "octoscode-app".into(),
                path: Some("/home/user/src/octoscode-app".into()),
            }
        );
        // A stale index is Unhandled, never a panic or a wrong workspace.
        assert!(resolve("workspace.new_chat_here", 9, &ctx).is_unhandled());
    }

    #[test]
    fn session_open_routes_with_the_store_index_and_closes_the_drawer() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        set_drawer_open(true);
        let store = board_store();
        let ctx = ctx_for(&store);
        assert_eq!(resolve("session.open", 3, &ctx), Effect::OpenSession { index: 3 });
        assert!(!drawer_open(), "selecting from the drawer dismisses it");
    }

    #[test]
    fn the_drawer_opens_and_closes_and_takes_the_search_focus_with_it() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        let store = board_store();
        let ctx = ctx_for(&store);
        resolve("drawer.open", 0, &ctx);
        assert!(drawer_open());
        resolve("search.focus", 0, &ctx);
        resolve("drawer.close", 0, &ctx);
        assert!(!drawer_open());
        assert!(!snapshot().search_focused);
    }

    #[test]
    fn the_rail_collapses_and_its_search_expands_and_focuses() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        let store = board_store();
        let ctx = ctx_for(&store);
        resolve("sidebar.collapse", 0, &ctx);
        assert!(snapshot().rail);
        resolve("search.open", 0, &ctx);
        let s = snapshot();
        assert!(!s.rail && s.search_focused);
        assert!(take_focus_search(), "the host gets one focus request");
        assert!(!take_focus_search());
        resolve("sidebar.collapse", 0, &ctx);
        resolve("sidebar.expand", 0, &ctx);
        assert!(!snapshot().rail);
    }

    #[test]
    fn clear_restores_every_row() {
        let _g = crate::screens::theme::test_lock();
        reset_state();
        let store = board_store();
        let ctx = ctx_for(&store);
        assert!(set_query("hydrate"));
        assert!(!set_query("hydrate"), "an unchanged query is not a change");
        resolve("search.clear", 0, &ctx);
        assert_eq!(query(), "");
        let p = project_recorded(&store, NOW, &[]);
        assert_eq!(p.rows.iter().filter(|r| matches!(r, Row::Session { .. })).count(), 5);
    }

    #[test]
    fn relative_labels_match_the_web_buckets() {
        assert_eq!(relative_label(NOW - 30_000, NOW), "now");
        assert_eq!(relative_label(NOW - 2 * 60_000, NOW), "2m");
        assert_eq!(relative_label(NOW - 60 * 60_000, NOW), "1h");
        assert_eq!(relative_label(NOW - 26 * 3_600_000, NOW), "1d");
        assert_eq!(relative_label(NOW - 3 * 86_400_000, NOW), "3d");
        assert_eq!(parse_rfc3339_ms("2026-10-01T15:39:00Z"), Some(1_790_869_140_000));
        assert_eq!(
            parse_rfc3339_ms("2026-10-01T17:39:00.250+02:00"),
            Some(1_790_869_140_250)
        );
        assert_eq!(parse_rfc3339_ms("yesterday"), None);
        assert_eq!(short_date(1_790_869_140_000), "Oct 1");
    }

    #[test]
    fn untitled_sessions_read_new_chat_and_unknown_roots_group_as_sessions() {
        let store = Arc::new(Store::new());
        store.set_sessions(vec![Session {
            id: "x".into(),
            title: None,
            message_count: 0,
            updated_at: None,
            last_prompt: Some("  ".into()),
            active_turn: false,
        }]);
        let p = project_with(&store, &SidebarUi::default(), NOW, &[]);
        assert_eq!(rows_text(&p), vec!["[v Sessions 1]", "New chat |  | Idle"]);
    }

    #[test]
    fn remembered_workspaces_show_as_empty_groups() {
        let store = board_store();
        let p = project_with(&store, &SidebarUi::default(), NOW, &["/home/user/src/web".into()]);
        let text = rows_text(&p);
        assert_eq!(text[text.len() - 2], "[v web 0]");
        assert_eq!(text[text.len() - 1], "note: No chats yet.");
    }

    // -- test helpers ---------------------------------------------------------

    fn test_ctx() -> Ctx<'static> {
        let store = Arc::new(Store::new());
        ctx_for(&store)
    }

    fn ctx_for(store: &Arc<Store>) -> Ctx<'static> {
        let ui = Arc::new(Mutex::new(crate::flow::FlowUi::default()));
        Ctx::new(Box::leak(Box::new(store.clone())), Box::leak(Box::new(ui)))
    }
}
