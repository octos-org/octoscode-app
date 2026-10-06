//! #D1/#A2 — the workspace folder browser (atlas board 1 screens p4-08/09),
//! one owner per action id.
//!
//! The web's contract is `features/workspace-create/workspace-browse.ts`
//! (client half), `WorkspaceFolderBrowser.tsx` (the view) and the spec
//! `e2e/workspace-browse.spec.ts`. The properties this screen keeps:
//!
//! 1. **What the server hid or cut is reported, never swallowed** — walk 220:
//!    the listing's `hidden_skipped` and `truncated` become notices
//!    (`workspaceListingNotices`, `workspace-browse.ts:241-259`, truncated
//!    first); the board words the hidden one "3 hidden by the server".
//! 2. **A refusal is bounded copy with a next step, never the server's prose**
//!    — walk 221; and the folder we stood in survives (the last good listing
//!    is kept, `workspaceBrowseReducer` "failed", `:316-318`).
//! 3. **A refusal is identity-checked and kind-whitelisted** — only a typed
//!    protocol error (`ClientError::Rpc`) whose `data.kind` is in the list /
//!    create whitelists becomes a kind (`workspaceBrowseRefusal`,
//!    `packages/client/src/workspace-browse.ts:105`); anything else is
//!    `unknown`. `banned_root` rides only a root escape and is never rendered.
//! 4. **Picking a subfolder fills the path box without navigating** — walk
//!    223; the browser then reopens at the chosen folder.
//! 5. **The affordance exists only when advertised** — the board-1 picker
//!    shows Browse/New folder only for `onboarding.workspace_browse.v1`
//!    (`supportsWorkspaceBrowse`, `:126-135`), fail closed.
use octoscode_client::domains::profile::{
    WorkspaceCreate, WorkspaceCreateParams, WorkspaceList, WorkspaceListParams, WorkspaceListResult,
};
use serde_json::Value;

use super::board1::{Layout, Ui};
use crate::i18n::{tr, tr1};
use super::board1_kit::{self as kit, Field, Text};

/// The most rows one listing shows (the server's own page cap is what
/// `truncated` reports; this only bounds the view).
pub const MAX_ROWS: usize = 40;

/// The advertised feature (`workspace-browse.ts:126-135`).
pub const BROWSE_FEATURE: &str = "onboarding.workspace_browse.v1";

/// The two browser cards (design/stage-b/phase4/cards).
pub const CARDS: &[(&str, &str)] = &[("browser", "p4-08"), ("refused", "p4-09")];

/// `workspace_list_*` refusals (`workspace-browse.ts:63-70`).
pub const LIST_REFUSAL_KINDS: &[&str] = &[
    "workspace_list_invalid_path",
    "workspace_list_not_found",
    "workspace_list_not_a_directory",
    "workspace_list_permission_denied",
    "workspace_list_root_escape",
];

/// `workspace_create_*` refusals (`workspace-browse.ts:72-80`).
pub const CREATE_REFUSAL_KINDS: &[&str] = &[
    "workspace_create_invalid_name",
    "workspace_create_parent_not_found",
    "workspace_create_parent_not_a_directory",
    "workspace_create_permission_denied",
    "workspace_create_root_escape",
    "workspace_create_exists_not_directory",
];

/// The kind both whitelists share (`workspace-browse.ts:79`).
pub const PROFILE_LOCAL_UNSUPPORTED: &str = "profile_local_unsupported";

/// The client-owned bucket: a transport failure or an unknown refusal.
pub const UNKNOWN: &str = "unknown";

/// `WORKSPACE_BROWSE_MAX_ASCENT` (`workspace-browse.ts:13`).
pub const MAX_ASCENT: usize = 32;

/// Which browser card is showing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// `p4-08` — the listing, the hidden count and the path box.
    #[default]
    Browser,
    /// `p4-09` — the server refused the folder we tried to open.
    Refused,
}

impl Screen {
    pub const ALL: [(Screen, &'static str); 2] = [(Screen::Browser, "p4-08"), (Screen::Refused, "p4-09")];

    pub fn card_dir(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(s, _)| *s == self)
            .map(|(_, dir)| *dir)
            .expect("every screen has a card")
    }
}

/// A whitelisted refusal, kept as a KIND so no server string survives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub kind: String,
    /// Only `*_root_escape` carries it; never rendered (`:91-92`).
    pub banned_root: Option<String>,
}

/// Whitelist ONE typed kind, identity first: only a value that arrived as a
/// typed protocol error is eligible (`workspace-browse.ts:105-120`).
pub fn refusal_from(typed: bool, kind: &str, banned_root: Option<String>) -> Option<Refusal> {
    if !typed {
        return None;
    }
    let known = LIST_REFUSAL_KINDS.contains(&kind) || CREATE_REFUSAL_KINDS.contains(&kind) || kind == PROFILE_LOCAL_UNSUPPORTED;
    if !known {
        return None;
    }
    let banned_root = if kind.ends_with("_root_escape") { banned_root } else { None };
    Some(Refusal { kind: kind.to_owned(), banned_root })
}

/// The classification a failed list/create earns: a whitelisted kind from a
/// typed protocol error (`data.kind`), else [`UNKNOWN`] — never duck-typed
/// from the error's text.
pub fn classify(e: &octoscode_client::ClientError) -> Refusal {
    if let octoscode_client::ClientError::Rpc { error, .. } = e {
        let data = error.data.as_ref();
        let kind = data.and_then(|d| d.get("kind")).and_then(|k| k.as_str()).unwrap_or("");
        let banned = data
            .and_then(|d| d.get("banned_root"))
            .and_then(|k| k.as_str())
            .map(str::to_owned);
        if let Some(r) = refusal_from(true, kind, banned) {
            return r;
        }
    }
    Refusal { kind: UNKNOWN.to_owned(), banned_root: None }
}

/// The bounded copy for a refusal: a headline and a next step, never the
/// server's own prose. The list-permission pair is the board's own (atlas
/// screen 9); the rest are the web's table (`workspace-browse.ts:79-120`).
pub fn refusal_copy(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "workspace_list_permission_denied" => (
            "The server won’t list this folder.",
            "Pick another folder or type a path you can access.",
        ),
        "workspace_list_invalid_path" => (
            "That path can’t be browsed.",
            "Browse from the server's working directory instead.",
        ),
        "workspace_list_not_found" => (
            "That folder is no longer on the server.",
            "Go up one level and pick a folder that still exists.",
        ),
        "workspace_list_not_a_directory" => ("That path is a file, not a folder.", "Go up one level and pick a folder."),
        "workspace_list_root_escape" => (
            "That folder is outside the area Octos may browse.",
            "Pick a folder inside your own projects instead.",
        ),
        "workspace_create_invalid_name" => (
            "The server rejected that folder name.",
            "Use a single name without slashes, up to 255 bytes.",
        ),
        "workspace_create_parent_not_found" => (
            "The folder you’re creating in is no longer on the server.",
            "Go up one level and try again.",
        ),
        "workspace_create_parent_not_a_directory" => (
            "The place you’re creating in is a file, not a folder.",
            "Go up one level and pick a folder.",
        ),
        "workspace_create_permission_denied" => (
            "Octos can’t create a folder here.",
            "Pick a folder the Octos server is allowed to write to.",
        ),
        "workspace_create_root_escape" => (
            "That location is outside the area Octos may write to.",
            "Create the folder inside your own projects instead.",
        ),
        "workspace_create_exists_not_directory" => ("A file of that name is already here.", "Choose a different folder name."),
        PROFILE_LOCAL_UNSUPPORTED => (
            "This server doesn’t offer folder browsing.",
            "Type the workspace path instead.",
        ),
        _ => (
            "Couldn’t reach the server's folders.",
            "Try again, or type the workspace path instead.",
        ),
    }
}

/// `validateWorkspaceFolderName` (`workspace-browse.ts:160-175`): one path
/// component, no separators, not `.`/`..`, no control characters, no
/// surrounding whitespace, 1..=255 bytes. The copy is the web's own.
pub fn validate_folder_name(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("Enter a name for the new folder.");
    }
    if name.contains('/') || name.contains('\\') {
        return Some("A folder name can’t contain a slash. Enter one name only.");
    }
    if name == "." || name == ".." {
        return Some("Enter a folder name other than . or ..");
    }
    if name.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f) {
        return Some("A folder name can’t contain control characters. Use plain text.");
    }
    if name != name.trim() {
        return Some("A folder name can’t start or end with a space. Trim it.");
    }
    if name.len() > 255 {
        return Some("That folder name is too long. Use up to 255 bytes.");
    }
    None
}

/// `parentWorkspacePath` (`workspace-browse.ts:185-196`): a client-side hint
/// only — a listing's own `parent_path` is authoritative for the way up.
pub fn parent_path(path: &str) -> Option<String> {
    let t = path.trim();
    if t.is_empty() || t == "/" || t == "~" {
        return None;
    }
    let n = t.trim_end_matches('/');
    if n.is_empty() || n == "~" {
        return None;
    }
    match n.rfind('/') {
        None => None,
        Some(0) => Some("/".to_owned()),
        Some(i) => Some(n[..i].to_owned()),
    }
}

/// One listing row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
}

/// The browser's UI-local state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BrowserUi {
    /// The folder we are standing in (the last good listing's canonical path).
    pub path: String,
    /// The last good listing's rows, top to bottom.
    pub entries: Vec<Entry>,
    /// The listing's `parent_path` (`None` at the root).
    pub parent: Option<String>,
    /// The listing says a folder can be created here.
    pub writable: bool,
    /// The path box's text (walk 223 fills it).
    pub path_draft: String,
    /// The row picked in the list.
    pub selected: Option<String>,
    pub hidden_skipped: u32,
    pub truncated: bool,
    /// The refusal, as a KIND only.
    pub failure: Option<Refusal>,
    /// The path the refused request asked for (p4-09's breadcrumb).
    pub attempted: Option<String>,
    /// The New-folder field is open, and its draft + pre-validation.
    pub new_folder_open: bool,
    pub new_folder: String,
    pub name_problem: Option<&'static str>,
    /// A request is in flight.
    pub loading: bool,
    /// The folder chosen last ("Use this folder"): the browser reopens here.
    pub chosen: Option<String>,
    pub screen: Screen,
}

impl BrowserUi {
    /// Whether the way up exists (row 220: "disables the way up at the root").
    pub fn can_go_up(&self) -> bool {
        self.parent.is_some() || (self.path != "/" && !self.path.is_empty() && parent_path(&self.path).is_some())
    }

    /// The advertised-feature gate, fail closed.
    pub fn browse_visible(advertised: bool) -> bool {
        advertised
    }

    /// The notices this listing owes the operator: truncated first, then the
    /// hidden count (`workspace-browse.ts:241-259`; the board's wording).
    pub fn notices(&self) -> Vec<String> {
        let mut out = Vec::new();
        // A35b: the count is what the view SHOWS. The server cuts a listing
        // at its page (`truncated`), and the view holds [`MAX_ROWS`] of it:
        // a 150-folder listing showed 40 rows and said nothing, so the rest
        // looked absent (reachable only by typing their path).
        let shown = self.entries.len().min(MAX_ROWS);
        if self.truncated || self.entries.len() > MAX_ROWS {
            out.push(crate::i18n::tr1("Only the first {value0} folders are shown.", &shown.to_string()));
        }
        if self.hidden_skipped > 0 {
            out.push(crate::i18n::tr1("{value0} hidden by the server", &self.hidden_skipped.to_string()));
        }
        out
    }

    /// A refusal arrived: keep the folder we were standing in and its rows
    /// (walk 221: "the current folder is unchanged").
    pub fn refuse(&mut self, r: Refusal, attempted: Option<String>) {
        self.loading = false;
        self.failure = Some(r);
        if let Some(a) = &attempted {
            self.path_draft = a.clone();
        }
        self.attempted = attempted;
        self.screen = Screen::Refused;
    }

    /// Pick a row: fill the path box and stay in this folder (walk 223).
    pub fn pick(&mut self, index: usize) -> Option<String> {
        let entry = self.entries.get(index)?;
        self.selected = Some(entry.name.clone());
        self.path_draft = entry.path.clone();
        Some(entry.path.clone())
    }

    /// The way up (`None` at the root).
    pub fn up(&self) -> Option<String> {
        if let Some(p) = &self.parent {
            return Some(p.clone());
        }
        if self.path == "/" || self.path.is_empty() {
            return None;
        }
        parent_path(&self.path)
    }

    /// Fold a good listing in (the reducer's "listed").
    pub fn listed(&mut self, l: &WorkspaceListResult) {
        self.path = l.canonical_path.clone();
        self.entries = l.entries.iter().map(|e| Entry { name: e.name.clone(), path: e.path.clone() }).collect();
        self.parent = l.parent_path.clone();
        self.writable = l.writable;
        self.truncated = l.truncated;
        // The server counts in u64 (`profile.rs:104`); saturate, never wrap.
        self.hidden_skipped = l.hidden_skipped.min(u32::MAX as u64) as u32;
        self.path_draft = l.canonical_path.clone();
        self.selected = None;
        self.failure = None;
        self.attempted = None;
        self.loading = false;
        self.new_folder_open = false;
        self.new_folder.clear();
        self.name_problem = None;
        self.screen = Screen::Browser;
    }

    /// The breadcrumb of `path`: `(label, absolute path)` from the root.
    pub fn crumbs(path: &str) -> Vec<(String, String)> {
        let mut out = vec![("/".to_owned(), "/".to_owned())];
        let mut acc = String::new();
        for seg in path.split('/').filter(|s| !s.is_empty()) {
            acc.push('/');
            acc.push_str(seg);
            out.push((seg.to_owned(), acc.clone()));
        }
        out
    }
}

/// The action ids the two cards emit.
pub const ACTIONS: &[(&str, &str)] = &[
    ("browser.enter.0", "pick the 1st folder (a second tap opens it)"),
    ("browser.enter.1", "pick the 2nd folder (a second tap opens it)"),
    ("browser.enter.2", "pick the 3rd folder (a second tap opens it)"),
    ("browser.enter.3", "pick the 4th folder (a second tap opens it)"),
    ("browser.enter.4", "pick the 5th folder"),
    ("browser.enter.5", "pick the 6th folder"),
    ("browser.enter.6", "pick the 7th folder"),
    ("browser.enter.7", "pick the 8th folder"),
    ("browser.crumb.0", "open the breadcrumb's root"),
    ("browser.crumb.1", "open the breadcrumb's 1st folder"),
    ("browser.crumb.2", "open the breadcrumb's 2nd folder"),
    ("browser.crumb.3", "open the breadcrumb's 3rd folder"),
    ("browser.crumb.4", "open the breadcrumb's 4th folder"),
    ("browser.crumb.5", "open the breadcrumb's 5th folder"),
    ("browser.up", "go to the parent folder (disabled at the root, walk 220)"),
    ("browser.path", "the path box's live text"),
    ("browser.go", "open the folder typed in the path box (Return)"),
    ("browser.use", "use the path box's folder: start a session there"),
    ("browser.back", "leave a refused folder for the last good one (p4-09)"),
    ("browser.newfolder", "open the New folder field"),
    ("browser.newfolder.cancel", "close the New folder field"),
    ("browser.create_name", "the New folder name's live text"),
    ("browser.create", "create the folder (name pre-validated, walk 222)"),
    ("browser.close", "the back chevron: leave the browser"),
];

pub const ROUTED: &[&str] = &[
    "browser.enter.0",
    "browser.enter.1",
    "browser.enter.2",
    "browser.enter.3",
    "browser.enter.4",
    "browser.enter.5",
    "browser.enter.6",
    "browser.enter.7",
    "browser.crumb.0",
    "browser.crumb.1",
    "browser.crumb.2",
    "browser.crumb.3",
    "browser.crumb.4",
    "browser.crumb.5",
    "browser.up",
    "browser.path",
    "browser.go",
    "browser.use",
    "browser.back",
    "browser.newfolder",
    "browser.newfolder.cancel",
    "browser.create_name",
    "browser.create",
    "browser.close",
];

/// A numbered family id (`browser.enter.<n>` / `browser.crumb.<n>`): the row
/// rides IN the id, so a listing longer than the table still routes.
fn numbered(id: &str) -> Option<usize> {
    id.strip_prefix("browser.enter.")
        .or_else(|| id.strip_prefix("browser.crumb."))
        .and_then(|n| n.parse().ok())
}

pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id) || numbered(id).is_some()
}

pub fn is_routed(id: &str) -> bool {
    ROUTED.contains(&id) || numbered(id).is_some()
}

pub fn unrouted() -> Vec<&'static str> {
    ACTIONS.iter().map(|(a, _)| *a).filter(|a| !ROUTED.contains(a)).collect()
}

/// What an action means.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Input { field: &'static str, value: String },
    /// A row tap (select; a second tap on the selected row opens it).
    Enter(usize),
    /// A breadcrumb segment.
    Crumb(usize),
    Up,
    /// List this folder (`None` = the server's own working directory).
    List(Option<String>),
    /// Use this folder (start a session there).
    Use(String),
    /// Create a folder named by the field, inside `parent`.
    Create { parent: String, name: String },
    /// Leave a refused folder for the last good one.
    Back,
    NewFolder(bool),
    /// The browser closes (back chevron).
    Close,
    Refused(Refusal),
    Unhandled,
}

/// Route one action id to its effect (the row/crumb ride IN the id).
pub fn resolve(id: &str, value: Option<&str>) -> Effect {
    if let Some(n) = id.strip_prefix("browser.enter.").and_then(|n| n.parse().ok()) {
        return Effect::Enter(n);
    }
    if let Some(n) = id.strip_prefix("browser.crumb.").and_then(|n| n.parse().ok()) {
        return Effect::Crumb(n);
    }
    let v = || value.unwrap_or_default().to_owned();
    match id {
        "browser.up" => Effect::Up,
        "browser.path" => Effect::Input { field: "browser.path", value: v() },
        "browser.create_name" => Effect::Input { field: "browser.create_name", value: v() },
        "browser.go" => Effect::List(None),
        "browser.use" => Effect::Use(String::new()),
        "browser.back" => Effect::Back,
        "browser.newfolder" => Effect::NewFolder(true),
        "browser.newfolder.cancel" => Effect::NewFolder(false),
        "browser.create" => Effect::Create { parent: String::new(), name: String::new() },
        "browser.close" => Effect::Close,
        _ => Effect::Unhandled,
    }
}

/// Apply one effect to the browser's state. Returns the transport effect.
pub fn apply(ui: &mut BrowserUi, effect: Effect) -> Option<Effect> {
    match effect {
        Effect::Input { field, value } => {
            match field {
                "browser.path" => ui.path_draft = value,
                "browser.create_name" => {
                    ui.new_folder = value;
                    ui.name_problem = None;
                }
                _ => {}
            }
            None
        }
        Effect::Enter(i) => {
            let name = ui.entries.get(i)?.name.clone();
            if ui.selected.as_deref() == Some(name.as_str()) {
                // A second tap on the picked row opens it (drill in, walk 220).
                let path = ui.entries[i].path.clone();
                ui.loading = true;
                return Some(Effect::List(Some(path)));
            }
            // Walk 223: picking fills the path box and does NOT navigate.
            ui.pick(i);
            None
        }
        Effect::Crumb(i) => {
            let base = ui.attempted.clone().unwrap_or_else(|| ui.path.clone());
            let crumbs = BrowserUi::crumbs(&base);
            let (_, path) = crumbs.get(i)?.clone();
            ui.loading = true;
            Some(Effect::List(Some(path)))
        }
        Effect::Up => {
            let parent = ui.up()?; // disabled at the root (walk 220)
            ui.loading = true;
            Some(Effect::List(Some(parent)))
        }
        Effect::List(None) => {
            let typed = ui.path_draft.trim().to_owned();
            ui.loading = true;
            Some(Effect::List(if typed.is_empty() { None } else { Some(typed) }))
        }
        Effect::List(Some(p)) => {
            ui.loading = true;
            Some(Effect::List(Some(p)))
        }
        Effect::Use(_) => {
            let path = if ui.path_draft.trim().is_empty() { ui.path.clone() } else { ui.path_draft.trim().to_owned() };
            if path.is_empty() {
                return None;
            }
            ui.chosen = Some(path.clone());
            Some(Effect::Use(path))
        }
        Effect::Back => {
            // The last good listing is still here (walk 221) — no request.
            ui.failure = None;
            ui.attempted = None;
            ui.path_draft = ui.path.clone();
            ui.screen = Screen::Browser;
            None
        }
        Effect::NewFolder(open) => {
            ui.new_folder_open = open;
            ui.new_folder.clear();
            ui.name_problem = None;
            None
        }
        // The name is PRE-VALIDATED before the transport can be reached: an
        // invalid name never leaves this module (`workspace-browse.ts:160`).
        Effect::Create { .. } => {
            if let Some(p) = validate_folder_name(&ui.new_folder) {
                ui.name_problem = Some(p);
                return None;
            }
            if ui.path.is_empty() {
                return None;
            }
            ui.loading = true;
            Some(Effect::Create { parent: ui.path.clone(), name: ui.new_folder.clone() })
        }
        Effect::Close => Some(Effect::Close),
        Effect::Refused(r) => {
            ui.refuse(r, None);
            None
        }
        Effect::Unhandled => None,
    }
}

// --------------------------------------------------------------------- views

/// A text's estimated width at `px` logical pixels: Inter runs ~0.56 em per
/// character (`board1::title_px`'s estimate); 0.6 keeps a margin for wide
/// glyphs and the SemiBold face.
fn est_w(text: &str, px: f64) -> f64 {
    text.chars().count() as f64 * 0.6 * px
}

/// The breadcrumb's text size at this layout (the phone's type scale).
fn crumb_px(l: &Layout) -> f64 {
    15.0 * if l.phone { 1.08 } else { 1.0 }
}

/// How many trailing segments of `crumbs` the breadcrumb shows: at most 4
/// (with the root and, when it skips some, "…"), and A35b: only as many as
/// fit the card — a phone showed "/ › … › user › work › monorepo › pa", cut
/// at the screen's edge (the breadcrumb never wraps).
fn crumbs_shown(crumbs: &[(String, String)], l: &Layout) -> usize {
    let n = crumbs.len();
    if n <= 1 {
        return 0;
    }
    let px = crumb_px(l);
    // A link pads 6 + 6; a separator is the 12 px chevron and 2 + 2 spacing.
    let link = |label: &str| est_w(label, px) + 12.0;
    const SEP: f64 = 16.0;
    let width = |k: usize| {
        let mut w = link("/");
        if k + 1 < n {
            w += SEP + est_w("\u{2026}", px);
        }
        w + crumbs[n - k..].iter().map(|(label, _)| SEP + link(label)).sum::<f64>()
    };
    (1..=(n - 1).min(4)).rev().find(|&k| width(k) <= l.content_w).unwrap_or(1)
}

fn crumb_row(ui: &BrowserUi, l: &Layout, v: &mut Ui) -> String {
    let base = ui.attempted.clone().unwrap_or_else(|| ui.path.clone());
    let crumbs = BrowserUi::crumbs(&base);
    // A deep path keeps its root and its last segments (the breadcrumb never
    // wraps): "/ › … › user › code".
    let skip = crumbs.len().saturating_sub(crumbs_shown(&crumbs, l));
    let mut out = String::from("View { width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 2\n");
    for (i, (label, _)) in crumbs.iter().enumerate() {
        if i > 0 && i < skip {
            if i == 1 {
                out.push_str(&kit::svg("", "b1_chevron_right.svg", 12.0));
                out.push_str(&Text::new("", "\u{2026}").px(15.0).color(kit::MUTED).dsl());
            }
            continue;
        }
        if i > 0 {
            out.push_str(&kit::svg("", "b1_chevron_right.svg", 12.0));
        }
        let id = format!("b1_br_crumb_{i}");
        out.push_str(&kit::link(&id, label, kit::MUTED, 15.0, 400));
        v.button(&id, &format!("browser.crumb.{i}"));
    }
    out.push_str("}\n");
    out
}

/// The native view of the browser (p4-08, or p4-09 after a refusal).
pub fn view(ui: &BrowserUi, l: &Layout) -> Ui {
    let mut v = Ui::default();
    v.header(l, "b1_br_back", "browser.close", tr("Choose workspace folder"));
    v.push(kit::gap(if l.phone { 10.0 } else { 6.0 }));
    let crumbs = crumb_row(ui, l, &mut v);
    v.push(crumbs);
    v.push(kit::gap(if l.phone { 10.0 } else { 8.0 }));
    match ui.screen {
        Screen::Browser => listing_view(ui, l, &mut v),
        Screen::Refused => refused_view(ui, l, &mut v),
    }
    v
}

fn listing_view(ui: &BrowserUi, l: &Layout, v: &mut Ui) {
    let row_h = if l.phone { 56 } else { 44 };
    let glyph = if l.phone { 24.0 } else { 22.0 };
    let rows: Vec<String> = ui
        .entries
        .iter()
        .take(MAX_ROWS)
        .enumerate()
        .map(|(i, e)| {
            let selected = ui.selected.as_deref() == Some(e.name.as_str());
            let id = format!("b1_br_row_{i}");
            v.button(&id, &format!("browser.enter.{i}"));
            let bg = if selected {
                format!("SolidView {{ width: Fill height: Fill draw_bg.color: {} }}\n", kit::SELECTED)
            } else {
                String::new()
            };
            format!(
                "View {{ width: Fill height: {row_h} flow: Overlay\n{bg}View {{ width: Fill height: Fill flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 16 right: 12}} spacing: 14\n{}{}}}\n{}}}\n",
                kit::svg("", "b1_folder.svg", glyph),
                Text::new(&format!("b1_br_row_t{i}"), &e.name).px(15.0).fill().one_line().dsl(),
                kit::hit(&id, !selected)
            )
        })
        .collect();
    if rows.is_empty() {
        let empty = if ui.loading { "Loading folders\u{2026}" } else { "No subfolders here." };
        v.push(Text::new("b1_br_empty", tr(empty)).px(14.0).color(kit::MUTED).fill().dsl());
    } else {
        // The card never outgrows the window: rows past what fits scroll
        // inside the list (the fixed chrome around it is ~302 px in a desktop
        // dialog, ~330 px on a phone sheet) — A35b: plus each line the
        // notice wraps onto beside "New folder" (two notices wrap on a phone:
        // the second line pushed "Use this folder" under the screen's edge).
        let notice_lines = {
            let text = ui.notices().join(" ");
            let link = if ui.writable { est_w(tr("New folder"), crumb_px(l)) + 12.0 } else { 0.0 };
            let avail = (l.content_w - 6.0 - link).max(1.0);
            (est_w(&text, crumb_px(l)) / avail).ceil().max(1.0)
        };
        let fixed = if l.phone { 330.0 } else { 302.0 } + (notice_lines - 1.0) * 20.0;
        let fits = (((l.h - fixed) / row_h as f64).floor() as usize).max(3);
        let max_h = (rows.len() > fits).then(|| fits as f64 * (row_h as f64 + 1.0));
        v.push(kit::list_card_scroll("b1_br_list", &rows, max_h));
    }
    let notices = ui.notices();
    v.push(kit::gap(8.0));
    v.push(format!(
        "View {{ width: Fill height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 6}}\n{}{}}}\n",
        Text::new("b1_br_notice", &notices.join(" ")).px(15.0).color(kit::MUTED).fill().dsl(),
        if ui.writable && !ui.new_folder_open {
            kit::link("b1_br_newfolder", tr("New folder"), kit::BLUE, 15.0, 500)
        } else {
            String::new()
        }
    ));
    if ui.writable && !ui.new_folder_open {
        v.button("b1_br_newfolder", "browser.newfolder");
    }
    if ui.new_folder_open {
        v.push(kit::gap(8.0));
        v.push(Field::new("b1_br_newname", &ui.new_folder).label(tr("New folder name")).error(ui.name_problem.is_some()).dsl());
        v.input("b1_br_newname", "browser.create_name");
        v.returns("b1_br_newname", "browser.create");
        if let Some(p) = ui.name_problem {
            v.push(kit::gap(6.0));
            v.push(Text::new("b1_br_name_problem", tr(p)).px(13.0).color(kit::RED).fill().dsl());
        }
        v.push(kit::gap(10.0));
        v.push(format!(
            "View {{ width: Fill height: Fit flow: Right spacing: 12\n{}{}}}\n",
            kit::pill_outline("b1_br_newcancel", tr("Cancel"), "Fill"),
            kit::pill_primary("b1_br_create", tr("Create folder"), "Fill")
        ));
        v.button("b1_br_newcancel", "browser.newfolder.cancel");
        v.button("b1_br_create", "browser.create");
    }
    v.spacer(l, 14.0, 22.0);
    v.push(Field::new("b1_br_path", &ui.path_draft).placeholder("/home/user/code").dsl());
    v.input("b1_br_path", "browser.path");
    v.returns("b1_br_path", "browser.go");
    v.push(kit::gap(if l.phone { 12.0 } else { 10.0 }));
    v.push(kit::pill_primary("b1_br_use", tr("Use this folder"), "Fill"));
    v.button("b1_br_use", "browser.use");
    if l.phone {
        v.push(kit::gap(36.0));
    }
}

/// The refusal's way back — "Back to <the folder we stood in>" — and its
/// pill's width: the board's two-thirds pill when the label fits it, the
/// full width when only that holds the path, and the folder's own name when
/// even that does not. A35b: a deep path ("Back to
/// /home/user/work/frontend/node_modules") overflowed the fixed pill and was
/// cut off at both ends, on the desktop and the phone alike.
fn back_label(path: &str, l: &Layout) -> (String, f64) {
    let path = if path.is_empty() { "/" } else { path };
    // The pill's 15 px SemiBold label, centred: keep 12 px clear each side.
    let px = crumb_px(l);
    let fits = |label: &str, w: f64| est_w(label, px) + 24.0 <= w;
    let full = tr1("Back to {value0}", path);
    let label = if fits(&full, l.content_w) {
        full
    } else {
        let name = path.trim_end_matches('/').rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("/");
        tr1("Back to {value0}", name)
    };
    let narrow = (l.content_w * 0.66).round().max(180.0);
    let w = if fits(&label, narrow) { narrow } else { l.content_w };
    (label, w)
}

fn refused_view(ui: &BrowserUi, l: &Layout, v: &mut Ui) {
    let kind = ui.failure.as_ref().map(|r| r.kind.as_str()).unwrap_or(UNKNOWN);
    let (head, next) = refusal_copy(kind);
    v.push(kit::gap(if l.phone { 14.0 } else { 4.0 }));
    v.push(kit::callout(false, true, head, Some(next)));
    v.push(kit::gap(if l.phone { 52.0 } else { 20.0 }));
    let (back_to, w) = back_label(&ui.path, l);
    v.push(format!(
        "View {{ width: Fill height: Fit align: Align{{x: 0.5 y: 0.0}}\n{}}}\n",
        kit::pill_outline("b1_br_backto", &back_to, &format!("{w}"))
    ));
    v.button("b1_br_backto", "browser.back");
    v.push(kit::gap(if l.phone { 96.0 } else { 20.0 }));
    v.push(Field::new("b1_br_path", &ui.path_draft).placeholder("/home/user/code").dsl());
    v.input("b1_br_path", "browser.path");
    v.returns("b1_br_path", "browser.go");
}

/// The bindings this screen projects.
pub fn query(ui: &BrowserUi, id: &str) -> Option<Value> {
    match id {
        "browser.path" => Some(Value::String(ui.path_draft.clone())),
        "browser.current" => Some(Value::String(ui.path.clone())),
        "browser.rows" => Some(Value::Array(ui.entries.iter().map(|e| Value::String(e.name.clone())).collect())),
        "browser.selected" => ui.selected.clone().map(Value::String),
        "browser.hidden_skipped" => Some(Value::from(ui.hidden_skipped)),
        "browser.truncated" => Some(Value::Bool(ui.truncated)),
        "browser.notices" => Some(Value::Array(ui.notices().into_iter().map(Value::String).collect())),
        "browser.can_go_up" => Some(Value::Bool(ui.can_go_up())),
        "browser.refusal" => ui.failure.as_ref().map(|r| Value::String(r.kind.clone())),
        "browser.name_problem" => ui.name_problem.map(|p| Value::String(p.to_owned())),
        _ => None,
    }
}

/// The live copy overrides for one Stage-B browser card (the design artifact).
pub fn copies(screen: Screen, ui: &BrowserUi) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |id: &str, v: &str| out.push((id.to_owned(), v.to_owned()));
    if !ui.path.is_empty() {
        push("t_crumbs_text", &ui.path);
    }
    for (i, e) in ui.entries.iter().enumerate().take(4) {
        push(&format!("t_row_{i}_text"), &e.name);
    }
    if !ui.notices().is_empty() {
        push("t_hidden_text", &ui.notices().join(" "));
    }
    if !ui.path_draft.is_empty() {
        push("browser_path_text", &ui.path_draft);
    }
    if screen == Screen::Refused {
        if let Some(r) = &ui.failure {
            let (head, next) = refusal_copy(&r.kind);
            push("t_cal1_text", head);
            push("t_cal2_text", next);
        }
    }
    out
}

/// Lower one Stage-B browser card — the accepted design artifact; the app
/// mounts the native [`view`] (board1.rs explains why).
pub fn lower_screen(screen: Screen, ui: &BrowserUi) -> Result<String, String> {
    let dir = crate::design::dir("stage-b/phase4/cards").join(screen.card_dir());
    let card_src = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("read {}: {e}", dir.join("page.card").display()))?;
    let data: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("page.data.json")).map_err(|e| format!("read page.data.json: {e}"))?,
    )
    .map_err(|e| format!("parse page.data.json: {e}"))?;
    let card_src = crate::l0_host::apply_copies(&card_src, &copies(screen, ui));
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &dir.join("kit"))
        .map_err(|e| format!("l0::prepare: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&tree))
        .map_err(|e| format!("to_makepad_ui: {e}"))?;
    Ok(super::taps::wire_card_events_dir(&dsl, &dir))
}

// ------------------------------------------------------------- live state

pub fn state() -> std::sync::MutexGuard<'static, BrowserUi> {
    static STATE: std::sync::OnceLock<std::sync::Mutex<BrowserUi>> = std::sync::OnceLock::new();
    STATE
        .get_or_init(|| std::sync::Mutex::new(BrowserUi::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn set(ui: BrowserUi) {
    *state() = ui;
}

/// Apply one action to the LIVE state and return the transport effect.
pub fn perform(id: &str, value: Option<&str>) -> Option<Effect> {
    apply(&mut state(), resolve(id, value))
}

/// `onboarding/workspace_list` through the TYPED client, folded into the
/// live state. On open (`resolve_ancestor`) a typed path that is invalid /
/// missing / not a folder walks up to its nearest listable ancestor, at most
/// [`MAX_ASCENT`] steps (`WorkspaceFolderBrowser.tsx` `load`,
/// `workspaceBrowseRetryPath`, `workspace-browse.ts:208-222`). `Ok` carries
/// the listed folder's canonical path (A35b: a listing with no path names
/// the server's working directory).
pub async fn list(conv: &crate::flow::Conversation, path: Option<String>, resolve_ancestor: bool) -> Result<String, String> {
    let request = begin_request();
    let mut candidate = path;
    for _ in 0..=MAX_ASCENT {
        let asked = std::time::Instant::now();
        let answer = if conv.is_hosted() {
            local::list(candidate.as_deref())
        } else {
            conv.client().call::<WorkspaceList>(WorkspaceListParams { path: candidate.clone() }).await
        };
        if crate::perf::enabled() {
            // A35b: the server's part of a navigation, on the app's clock
            // (the round trip; never the path, which names the server's disk).
            makepad_widgets::log!(
                "[octoscode] perf: workspace_list answered in {:.1} ms ({})",
                asked.elapsed().as_secs_f64() * 1000.0,
                match &answer {
                    Ok(l) => format!("{} entries", l.entries.len()),
                    Err(_) => "refused".to_owned(),
                }
            );
        }
        match answer {
            Ok(listing) => {
                if is_latest(request) {
                    state().listed(&listing);
                }
                return Ok(listing.canonical_path);
            }
            Err(e) => {
                let refusal = classify(&e);
                let retry = resolve_ancestor
                    && matches!(
                        refusal.kind.as_str(),
                        "workspace_list_invalid_path" | "workspace_list_not_found" | "workspace_list_not_a_directory"
                    );
                match (retry, candidate.as_deref()) {
                    (true, Some(c)) => {
                        candidate = parent_path(c);
                        continue;
                    }
                    _ => {
                        if is_latest(request) {
                            state().refuse(refusal.clone(), candidate.clone());
                        }
                        return Err(format!("onboarding/workspace_list: {}", refusal.kind));
                    }
                }
            }
        }
    }
    if is_latest(request) {
        state().refuse(Refusal { kind: UNKNOWN.to_owned(), banned_root: None }, None);
    }
    Err("onboarding/workspace_list: no listable ancestor".to_owned())
}

/// Latest-request-wins for the listing (the web's browse state machine drops
/// a superseded answer): every `list` takes a ticket, and only the newest
/// ticket may fold its answer in. Measured: a breadcrumb's listing that
/// resolved after a typed path's refusal overwrote the refusal.
static LATEST_LIST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn begin_request() -> u64 {
    LATEST_LIST.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
}

fn is_latest(request: u64) -> bool {
    LATEST_LIST.load(std::sync::atomic::Ordering::SeqCst) == request
}

/// `onboarding/workspace_create` then MOVE INTO the new folder (the web's
/// `submitNewFolder`, `WorkspaceFolderBrowser.tsx`: "creating a folder MOVES
/// INTO it"). `created: false` is an idempotent success.
pub async fn create(conv: &crate::flow::Conversation, parent: String, name: String) -> Result<(), String> {
    let created = if conv.is_hosted() {
        local::create(&parent, &name)
    } else {
        conv.client().call::<WorkspaceCreate>(WorkspaceCreateParams { parent: parent.clone(), name }).await
    };
    match created {
        Ok(created) => list(conv, Some(created.canonical_path), false).await.map(|_| ()),
        Err(e) => {
            let refusal = classify(&e);
            let mut ui = state();
            ui.loading = false;
            ui.new_folder_open = true;
            // A create refusal is shown inline under the name field.
            ui.name_problem = Some(refusal_copy(&refusal.kind).0);
            Err(format!("onboarding/workspace_create: {}", refusal.kind))
        }
    }
}

/// Inside OctoSense the module runs in the shell's process and lists the
/// person's folders itself: the shell's port offers no `onboarding/*`
/// (octos browses folders only for a solo server), and whether a session may
/// work in a folder is the shell's to decide when the session opens there.
/// The answers and refusals have the server's shapes, so the browser is the
/// same either way.
pub mod local {
    use std::path::{Path, PathBuf};

    use octos_core::ui_protocol::RpcError;
    use octoscode_client::domains::profile::{WorkspaceCreateResult, WorkspaceFolderEntry, WorkspaceListResult};
    use octoscode_client::ClientError;

    /// The most folders one listing names (`truncated` beyond).
    pub const PAGE: usize = 500;

    fn refused(method: &str, kind: &str, message: String) -> ClientError {
        ClientError::Rpc {
            method: method.to_owned(),
            error: RpcError::new(-32602, message).with_data(serde_json::json!({ "kind": kind })),
        }
    }

    fn home() -> Option<PathBuf> {
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).filter(|h| !h.is_empty()).map(PathBuf::from)
    }

    /// `path` (`~` expanded) resolved to a folder, or the refusal octos
    /// would give.
    fn folder(method: &str, path: &str) -> Result<PathBuf, ClientError> {
        let path = match (path.strip_prefix('~'), home()) {
            (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => home.join(rest.trim_start_matches('/')),
            _ => PathBuf::from(path),
        };
        if !path.is_absolute() {
            return Err(refused(method, "workspace_list_invalid_path", format!("{} is not an absolute path", path.display())));
        }
        let canonical = std::fs::canonicalize(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => refused(method, "workspace_list_not_found", e.to_string()),
            std::io::ErrorKind::PermissionDenied => refused(method, "workspace_list_permission_denied", e.to_string()),
            _ => refused(method, "workspace_list_invalid_path", e.to_string()),
        })?;
        if !canonical.is_dir() {
            return Err(refused(method, "workspace_list_not_a_directory", format!("{} is not a folder", canonical.display())));
        }
        Ok(canonical)
    }

    fn writable(path: &Path) -> bool {
        std::fs::metadata(path).is_ok_and(|m| !m.permissions().readonly())
    }

    /// `onboarding/workspace_list`, answered here: the folders in `path`
    /// (the person's home without one), hidden ones counted, not listed.
    pub fn list(path: Option<&str>) -> Result<WorkspaceListResult, ClientError> {
        const METHOD: &str = "onboarding/workspace_list";
        let start = match path.filter(|p| !p.trim().is_empty()) {
            Some(path) => path.to_owned(),
            None => home().map(|h| h.to_string_lossy().into_owned()).unwrap_or_else(|| "/".to_owned()),
        };
        let canonical = folder(METHOD, &start)?;
        let read = std::fs::read_dir(&canonical).map_err(|e| refused(METHOD, "workspace_list_permission_denied", e.to_string()))?;
        let mut entries = Vec::new();
        let mut hidden_skipped = 0u64;
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if name.starts_with('.') {
                hidden_skipped += 1;
                continue;
            }
            let path = std::fs::canonicalize(&path).unwrap_or(path);
            entries.push(WorkspaceFolderEntry { name, writable: writable(&path), path: path.to_string_lossy().into_owned() });
        }
        entries.sort_by_key(|e| e.name.to_lowercase());
        let truncated = entries.len() > PAGE;
        entries.truncate(PAGE);
        Ok(WorkspaceListResult {
            parent_path: canonical.parent().map(|p| p.to_string_lossy().into_owned()),
            writable: writable(&canonical),
            canonical_path: canonical.to_string_lossy().into_owned(),
            entries,
            truncated,
            hidden_skipped,
        })
    }

    /// `onboarding/workspace_create`, answered here: the folder `name` in
    /// `parent` (`created: false` when it was there already).
    pub fn create(parent: &str, name: &str) -> Result<WorkspaceCreateResult, ClientError> {
        const METHOD: &str = "onboarding/workspace_create";
        let name = name.trim();
        if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\']) {
            return Err(refused(METHOD, "workspace_create_invalid_name", format!("{name:?} is not a folder name")));
        }
        let dir = folder(METHOD, parent)?.join(name);
        if dir.is_dir() {
            return Ok(WorkspaceCreateResult { canonical_path: dir.to_string_lossy().into_owned(), created: false });
        }
        std::fs::create_dir(&dir).map_err(|e| refused(METHOD, "workspace_create_failed", e.to_string()))?;
        Ok(WorkspaceCreateResult { canonical_path: std::fs::canonicalize(&dir).unwrap_or(dir).to_string_lossy().into_owned(), created: true })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_local_listing_names_folders_counts_hidden_ones_and_refuses_as_octos_does() {
            let root = std::env::temp_dir().join(format!("octoscode-local-browse-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            for dir in ["b", "A", ".hidden"] {
                std::fs::create_dir_all(root.join(dir)).unwrap();
            }
            std::fs::write(root.join("file.txt"), "x").unwrap();
            let listed = list(Some(&root.to_string_lossy())).unwrap();
            assert_eq!(listed.entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["A", "b"], "folders only, sorted");
            assert_eq!(listed.hidden_skipped, 1);
            assert!(listed.parent_path.is_some());
            let kind = |e: ClientError| match e {
                ClientError::Rpc { error, .. } => error.data.unwrap()["kind"].as_str().unwrap().to_owned(),
                other => panic!("{other}"),
            };
            assert_eq!(kind(list(Some(&root.join("missing").to_string_lossy())).unwrap_err()), "workspace_list_not_found");
            assert_eq!(kind(list(Some(&root.join("file.txt").to_string_lossy())).unwrap_err()), "workspace_list_not_a_directory");
            assert_eq!(kind(list(Some("relative/path")).unwrap_err()), "workspace_list_invalid_path");
            let made = create(&root.to_string_lossy(), "new").unwrap();
            assert!(made.created && root.join("new").is_dir());
            assert!(!create(&root.to_string_lossy(), "new").unwrap().created, "idempotent");
            assert!(create(&root.to_string_lossy(), "../escape").is_err());
            let _ = std::fs::remove_dir_all(&root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_newest_listing_may_fold_its_answer_in() {
        let first = begin_request();
        let second = begin_request();
        assert!(!is_latest(first), "a superseded answer is dropped");
        assert!(is_latest(second));
    }

    fn listing() -> BrowserUi {
        BrowserUi {
            path: "/home/user/code".into(),
            entries: vec![
                Entry { name: "octos".into(), path: "/home/user/code/octos".into() },
                Entry { name: "octoscode-app".into(), path: "/home/user/code/octoscode-app".into() },
            ],
            parent: Some("/home/user".into()),
            hidden_skipped: 3,
            ..Default::default()
        }
    }

    #[test]
    fn the_hidden_count_is_reported_not_swallowed() {
        assert_eq!(listing().notices(), vec!["3 hidden by the server".to_owned()]);
    }

    #[test]
    fn truncation_is_reported_before_the_hidden_count() {
        let mut ui = listing();
        ui.truncated = true;
        let n = ui.notices();
        assert_eq!(n[0], "Only the first 2 folders are shown.");
        assert_eq!(n[1], "3 hidden by the server");
    }

    #[test]
    fn a_listing_longer_than_the_view_says_how_many_rows_it_shows() {
        // A35b: 150 folders, none cut by the server, MAX_ROWS shown.
        let mut ui = listing();
        ui.entries = (0..150)
            .map(|i| Entry { name: format!("pkg-{i:03}"), path: format!("/home/user/code/pkg-{i:03}") })
            .collect();
        ui.hidden_skipped = 0;
        assert_eq!(ui.notices(), vec![format!("Only the first {MAX_ROWS} folders are shown.")]);
        // The server's own cut of 500 says the same: the view shows MAX_ROWS.
        ui.truncated = true;
        assert_eq!(ui.notices(), vec![format!("Only the first {MAX_ROWS} folders are shown.")]);
    }

    /// A35b — a deep path's breadcrumb and the refusal's way back fit the
    /// card (measured: the phone's breadcrumb was cut at the screen's edge,
    /// and "Back to /home/user/work/frontend/node_modules" overflowed its
    /// pill at both ends on the desktop and the phone).
    #[test]
    fn a_deep_path_fits_the_breadcrumb_and_the_way_back() {
        use crate::screens::board1::Surface;
        let desk = Layout::of(Surface::Browser, 990.0, 603.0);
        let phone = Layout::of(Surface::Browser, 360.0, 780.0);
        let deep = BrowserUi::crumbs("/home/user/work/monorepo/packages");
        assert_eq!(crumbs_shown(&deep, &desk), 4, "the desktop keeps four segments");
        let k = crumbs_shown(&deep, &phone);
        assert!((1..4).contains(&k), "the phone keeps fewer: {k}");
        let shown: f64 = deep[deep.len() - k..].iter().map(|(s, _)| est_w(s, crumb_px(&phone)) + 28.0).sum::<f64>()
            + est_w("/\u{2026}", crumb_px(&phone))
            + 44.0;
        assert!(shown <= phone.content_w, "{shown} > {}", phone.content_w);
        // A short path keeps all of it.
        assert_eq!(crumbs_shown(&BrowserUi::crumbs("/home/user/code"), &phone), 3);

        let long = "/home/user/work/frontend/node_modules";
        let (label, w) = back_label(long, &desk);
        assert_eq!(label, format!("Back to {long}"), "the desktop holds the whole path");
        assert!(w <= desk.content_w && est_w(&label, crumb_px(&desk)) + 24.0 <= w);
        let (label, w) = back_label(long, &phone);
        assert_eq!(label, "Back to node_modules", "the phone names the folder");
        assert!(w <= phone.content_w && est_w(&label, crumb_px(&phone)) + 24.0 <= w);
        // The board's own short path keeps the two-thirds pill.
        let (label, w) = back_label("/home/user/code", &desk);
        assert_eq!((label.as_str(), w), ("Back to /home/user/code", (desk.content_w * 0.66).round()));
    }

    #[test]
    fn up_is_disabled_at_the_root() {
        let mut ui = listing();
        assert_eq!(apply(&mut ui, Effect::Up), Some(Effect::List(Some("/home/user".into()))));
        let mut root = BrowserUi { path: "/".into(), ..Default::default() };
        assert!(!root.can_go_up());
        assert_eq!(apply(&mut root, Effect::Up), None);
    }

    #[test]
    fn picking_a_row_fills_the_path_box_and_a_second_tap_opens_it() {
        // walk 223, then walk 220's drill-in.
        let mut ui = listing();
        assert_eq!(apply(&mut ui, Effect::Enter(1)), None, "picking must not navigate");
        assert_eq!(ui.path_draft, "/home/user/code/octoscode-app");
        assert_eq!(ui.path, "/home/user/code", "the browser stays in its parent");
        assert_eq!(
            apply(&mut ui, Effect::Enter(1)),
            Some(Effect::List(Some("/home/user/code/octoscode-app".into())))
        );
    }

    #[test]
    fn a_refusal_keeps_the_folder_we_were_standing_in() {
        // walk 221
        let mut ui = listing();
        let r = refusal_from(true, "workspace_list_permission_denied", None).expect("typed");
        ui.refuse(r, Some("/private".into()));
        assert_eq!(ui.path, "/home/user/code");
        assert_eq!(ui.entries.len(), 2);
        assert_eq!(ui.screen, Screen::Refused);
        assert_eq!(apply(&mut ui, Effect::Back), None, "back needs no request");
        assert_eq!(ui.screen, Screen::Browser);
        assert_eq!(ui.path_draft, "/home/user/code");
    }

    #[test]
    fn a_refusal_shows_bounded_copy_never_the_servers_prose() {
        for k in LIST_REFUSAL_KINDS.iter().chain(CREATE_REFUSAL_KINDS).chain([&PROFILE_LOCAL_UNSUPPORTED, &UNKNOWN]) {
            let (h, n) = refusal_copy(k);
            assert!(!h.is_empty() && !n.is_empty(), "{k} needs a next step");
            assert!(!h.contains("workspace_") && !n.contains("workspace_"), "{k} leaks the kind");
        }
    }

    #[test]
    fn an_untyped_error_with_a_matching_kind_is_not_a_refusal() {
        assert!(refusal_from(false, "workspace_list_permission_denied", None).is_none());
        assert!(refusal_from(true, "not_a_real_kind", None).is_none());
        let e = octoscode_client::ClientError::Transport { method: "x".into(), reason: "\"kind\":\"workspace_list_permission_denied\"".into() };
        assert_eq!(classify(&e).kind, UNKNOWN, "never duck-typed from text");
    }

    #[test]
    fn a_typed_rpc_refusal_is_read_from_its_data_kind() {
        let e = octoscode_client::ClientError::Rpc {
            method: "onboarding/workspace_list".into(),
            error: octos_core::ui_protocol::RpcError {
                code: -32602,
                message: "workspace_list: permission denied at /private".into(),
                data: Some(serde_json::json!({"kind": "workspace_list_permission_denied", "banned_root": "/private"})),
            },
        };
        let r = classify(&e);
        assert_eq!(r.kind, "workspace_list_permission_denied");
        assert_eq!(r.banned_root, None, "banned_root rides only a root escape");
    }

    #[test]
    fn a_new_folder_name_is_prevalidated_before_any_request() {
        let mut ui = listing();
        ui.new_folder = "a/b".into();
        assert_eq!(apply(&mut ui, Effect::Create { parent: String::new(), name: String::new() }), None);
        assert!(ui.name_problem.is_some());
        for bad in ["", ".", "..", " x", "x\u{7}"] {
            assert!(validate_folder_name(bad).is_some(), "{bad:?}");
        }
        ui.new_folder = "notes".into();
        assert_eq!(
            apply(&mut ui, Effect::Create { parent: String::new(), name: String::new() }),
            Some(Effect::Create { parent: "/home/user/code".into(), name: "notes".into() })
        );
    }

    #[test]
    fn the_browse_gate_fails_closed() {
        assert!(!BrowserUi::browse_visible(false));
        assert!(BrowserUi::browse_visible(true));
    }

    #[test]
    fn crumbs_walk_from_the_root() {
        let c = BrowserUi::crumbs("/home/user/code");
        assert_eq!(c.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), ["/", "home", "user", "code"]);
        assert_eq!(c[2].1, "/home/user");
    }

    #[test]
    fn every_declared_action_is_routed() {
        assert_eq!(unrouted(), Vec::<&str>::new());
    }
}
