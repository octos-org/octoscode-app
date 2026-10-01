//! #D1 — the workspace folder browser (atlas screens p4-08/09), one owner per
//! action id.
//!
//! The web's contract is `features/workspace-create/workspace-browse.ts` (plus
//! `packages/client/src/workspace-browse.ts` for the protocol side) and the spec
//! `e2e/workspace-browse.spec.ts`. Four properties this screen must keep, each
//! with its citation:
//!
//! 1. **The server's hiding and truncation are reported, never swallowed** —
//!    walk row 220 pins both notices: `"2 hidden folders aren't shown."`
//!    (`workspace-browse.spec.ts:49-51`) and `"Only the first 1 folders are
//!    shown."` (`:69-70`). They come from `workspaceListingNotices`
//!    (`workspace-browse.ts:241-259`), which pushes the truncated notice first
//!    and the hidden one only when `hiddenSkipped > 0`.
//! 2. **A refusal is bounded copy with a next step, never the server's prose** —
//!    walk row 221 pins `"Octos can't open that folder."` plus `"Pick a folder
//!    the Octos server is allowed to read."`, asserts the server's own string
//!    (`workspace_list`) is ABSENT, and asserts the folder we were standing in
//!    survives (`:90-99`).
//! 3. **A refusal is identity-checked and kind-whitelisted, not duck-typed** —
//!    `workspaceBrowseRefusal` (`packages/client/src/workspace-browse.ts:105`)
//!    returns a kind only for a real `OctosUiProtocolError` whose `data.kind`
//!    is in the list/create whitelists (`:63-80`); anything else stays an
//!    untyped failure. `banned_root` is "never rendered raw" (`:91`).
//! 4. **Picking a subfolder fills the path box without navigating** — walk row
//!    223: the path field is filled and the browser stays in its parent.
//!
//! The advertised-feature gate is the web's `supportsWorkspaceBrowse`
//! (`:126-135`): a client that does not see `onboarding.workspace_browse.v1`
//! hides every affordance and fails closed.

use serde_json::Value;

/// The feature that must be advertised before any affordance shows
/// (`workspace-browse.ts:126-135` — the gate is the CAPABILITY, never the method).
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

/// Which browser card is mounted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// `p4-08` — the listing, with the hidden count and the path box.
    #[default]
    Browser,
    /// `p4-09` — the same browser after the server refused a folder.
    Refused,
}

impl Screen {
    pub const ALL: [(Screen, &'static str); 2] =
        [(Screen::Browser, "p4-08"), (Screen::Refused, "p4-09")];

    /// The card directory under `design/stage-b/phase4/cards`.
    pub fn card_dir(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(s, _)| *s == self)
            .map(|(_, dir)| *dir)
            .expect("every screen has a card")
    }
}

/// A whitelisted refusal, kept as a KIND so no server string survives
/// (`workspace-browse.ts:105-120`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub kind: String,
    /// Only `*_root_escape` carries it; never rendered raw (`:91-92`).
    pub banned_root: Option<String>,
}

/// Whitelist ONE typed kind, identity first: only a value that arrived as a
/// typed protocol error is eligible, so a plain error carrying a matching
/// `data.kind` stays untyped (`workspace-browse.ts:105-120`).
pub fn refusal_from(typed: bool, kind: &str, banned_root: Option<String>) -> Option<Refusal> {
    if !typed {
        return None;
    }
    let known = LIST_REFUSAL_KINDS.contains(&kind)
        || CREATE_REFUSAL_KINDS.contains(&kind)
        || kind == PROFILE_LOCAL_UNSUPPORTED;
    if !known {
        return None;
    }
    // `banned_root` rides ONLY a root escape (`:91`); drop it otherwise so a
    // server cannot smuggle a path into the UI through the wrong field.
    let banned_root = if kind.ends_with("_root_escape") {
        banned_root
    } else {
        None
    };
    Some(Refusal {
        kind: kind.to_owned(),
        banned_root,
    })
}

/// The bounded copy for a refusal: a headline and a next step, never the
/// server's own prose. The permission-denied pair is the one the spec pins
/// verbatim (`workspace-browse.spec.ts:92-95`); the rest follow the same rule
/// (own words, own next step).
pub fn refusal_copy(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "workspace_list_permission_denied" | "workspace_create_permission_denied" => (
            "Octos can't open that folder.",
            "Pick a folder the Octos server is allowed to read.",
        ),
        "workspace_list_not_found" => (
            "That folder isn't there any more.",
            "Pick another folder from the list.",
        ),
        "workspace_list_not_a_directory" | "workspace_create_parent_not_a_directory" => (
            "That isn't a folder.",
            "Pick a folder from the list.",
        ),
        "workspace_list_invalid_path" => (
            "That path isn't one we can open.",
            "Pick a folder from the list, or type a path you can access.",
        ),
        "workspace_list_root_escape" | "workspace_create_root_escape" => (
            "Octos won't open that folder.",
            "Stay inside the folders Octos can read.",
        ),
        "workspace_create_invalid_name" => (
            "That folder name can't be used.",
            "Enter one name, without a slash.",
        ),
        "workspace_create_parent_not_found" => (
            "The folder to create inside is gone.",
            "Go back and pick another folder.",
        ),
        "workspace_create_exists_not_directory" => (
            "Something with that name is already there.",
            "Pick another name.",
        ),
        PROFILE_LOCAL_UNSUPPORTED => (
            "This server doesn't support that here.",
            "Use a folder the Octos server is allowed to read.",
        ),
        _ => (
            "Octos can't open that folder.",
            "Pick a folder the Octos server is allowed to read.",
        ),
    }
}

/// One listing row, as the browser shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
}

/// The browser's UI-local state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BrowserUi {
    /// The folder we are standing in. Its rows come from the last good listing.
    pub path: String,
    /// The rows, top to bottom.
    pub entries: Vec<Entry>,
    /// The path the operator typed in the path box (row 223 fills it).
    pub path_draft: String,
    /// The row selected in the list; `None` until one is picked.
    pub selected: Option<String>,
    /// How many folders the server skipped (`hidden_skipped`).
    pub hidden_skipped: u32,
    /// Whether the listing was cut short.
    pub truncated: bool,
    /// The refusal, as a KIND only.
    pub failure: Option<Refusal>,
    /// The new-folder name being typed (row 164).
    pub new_folder: String,
    /// Where we are.
    pub screen: Screen,
}

impl BrowserUi {
    /// Whether the "up" affordance is available — false at the root (row 220:
    /// "disables the way up at the root").
    pub fn can_go_up(&self) -> bool {
        self.path != "/" && !self.path.is_empty()
    }

    /// Whether any affordance shows at all: the advertised-feature gate, fail
    /// closed (`workspace-browse.ts:126-135`).
    pub fn browse_visible(advertised: bool) -> bool {
        advertised
    }

    /// The notices this listing owes the operator, in the web's order — the
    /// truncated notice first, then the hidden count only when it is non-zero
    /// (`workspace-browse.ts:241-259`). Row 220 pins both strings.
    pub fn notices(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.truncated {
            out.push(format!(
                "Only the first {} folders are shown.",
                self.entries.len()
            ));
        }
        if self.hidden_skipped > 0 {
            out.push(format!("{} hidden folders aren't shown.", self.hidden_skipped));
        }
        out
    }

    /// A refusal arrived: keep the last good listing and the current folder
    /// (row 221: "the current folder is unchanged") and show bounded copy.
    pub fn refuse(&mut self, r: Refusal) {
        // The folder we were standing in and its rows are deliberately NOT
        // cleared — that is the half of row 221 the spec pins.
        self.failure = Some(r);
        self.screen = Screen::Refused;
    }

    /// Pick a row: fill the path box and stay in this folder (row 223).
    pub fn pick(&mut self, index: usize) -> Option<String> {
        let entry = self.entries.get(index)?;
        self.selected = Some(entry.name.clone());
        self.path_draft = entry.path.clone();
        Some(entry.path.clone())
    }

    /// Go up one level. `None` at the root — the affordance is disabled there
    /// (row 220).
    pub fn up(&self) -> Option<String> {
        if !self.can_go_up() {
            return None;
        }
        let p = self.path.trim_end_matches('/');
        match p.rfind('/') {
            Some(0) => Some("/".to_owned()),
            Some(i) => Some(p[..i].to_owned()),
            None => Some("/".to_owned()),
        }
    }

    /// The new-folder name check, pre-validated before any request (row 164).
    /// The web's own message is pinned at `workspace-browse.spec.ts:116-117`.
    pub fn name_problem(&self) -> Option<&'static str> {
        let n = self.new_folder.trim();
        if n.is_empty() {
            return None;
        }
        if n.contains('/') || n.contains('\\') {
            return Some("A folder name can't contain a slash. Enter one name only.");
        }
        if n == "." || n == ".." {
            return Some("A folder name can't be '.' or '..'. Enter another name.");
        }
        None
    }
}

/// The action ids these two cards emit, with what each one means.
pub const ACTIONS: &[(&str, &str)] = &[
    ("browser.enter.0", "enter the first listed folder"),
    ("browser.enter.1", "enter the second listed folder"),
    ("browser.enter.2", "enter the third listed folder"),
    ("browser.enter.3", "enter the fourth listed folder"),
    ("browser.up", "go to the parent folder (disabled at the root, row 220)"),
    ("browser.path", "the path box's live text (row 223 fills it)"),
    ("browser.use", "start a session in the path box's folder"),
    ("browser.back", "step back out of a refused folder (p4-09)"),
    ("browser.create", "create a folder, pre-validated (row 164)"),
    ("browser.create_name", "the new-folder name field's live text"),
];

/// The ids [`resolve`] routes.
pub const ROUTED: &[&str] = &[
    "browser.enter.0",
    "browser.enter.1",
    "browser.enter.2",
    "browser.enter.3",
    "browser.up",
    "browser.path",
    "browser.use",
    "browser.back",
    "browser.create",
    "browser.create_name",
];

pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

pub fn is_routed(id: &str) -> bool {
    ROUTED.contains(&id)
}

/// Declared but unrouted (the coverage contract every screen set carries).
pub fn unrouted() -> Vec<&'static str> {
    ACTIONS
        .iter()
        .map(|(a, _)| *a)
        .filter(|a| !ROUTED.contains(a))
        .collect()
}

/// What an action means.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Live text for a field.
    Input { field: &'static str, value: String },
    /// Enter the folder at this row index.
    Enter(usize),
    /// Go up one level (`None` at the root).
    Up,
    /// Start a session in `path`.
    Use(String),
    /// Create a folder named by the field.
    Create(String),
    /// A whitelisted refusal the caller just received.
    Refused(Refusal),
    /// Unhandled id.
    Unhandled,
}

/// Route one action id to its effect.
pub fn resolve(id: &str, value: Option<&str>) -> Effect {
    match id {
        "browser.enter.0" => Effect::Enter(0),
        "browser.enter.1" => Effect::Enter(1),
        "browser.enter.2" => Effect::Enter(2),
        "browser.enter.3" => Effect::Enter(3),
        "browser.up" => Effect::Up,
        "browser.path" => Effect::Input {
            field: "browser.path",
            value: value.unwrap_or_default().to_owned(),
        },
        "browser.create_name" => Effect::Input {
            field: "browser.create_name",
            value: value.unwrap_or_default().to_owned(),
        },
        "browser.use" => Effect::Use(String::new()),
        "browser.create" => Effect::Create(String::new()),
        "browser.back" => Effect::Up,
        _ => Effect::Unhandled,
    }
}

/// Apply one effect to the browser's state. Returns the transport effect.
pub fn apply(ui: &mut BrowserUi, effect: Effect) -> Option<Effect> {
    match effect {
        Effect::Input { field, value } => {
            match field {
                "browser.path" => ui.path_draft = value,
                "browser.create_name" => ui.new_folder = value,
                _ => {}
            }
            None
        }
        Effect::Enter(i) => {
            // Row 223: picking a row fills the path box and does NOT navigate.
            ui.pick(i);
            None
        }
        Effect::Up => match ui.up() {
            Some(parent) => Some(Effect::Enter_(parent)),
            None => None, // disabled at the root (row 220)
        },
        Effect::Refused(r) => {
            ui.refuse(r);
            None
        }
        // The name is PRE-VALIDATED before the transport can be reached: an
        // invalid name never leaves this module (walk 164).
        Effect::Create(_) => {
            if ui.name_problem().is_some() {
                None
            } else {
                Some(Effect::Create(ui.new_folder.trim().to_owned()))
            }
        }
        transport @ Effect::Use(_) => Some(transport),
        Effect::Unhandled => None,
    }
}

impl Effect {
    /// "Navigate to this absolute path" — the transport half of `Up` and of
    /// entering a row when the caller really wants to descend.
    #[allow(non_snake_case)]
    pub fn Enter_(path: String) -> Effect {
        Effect::Use(path)
    }
}

/// The live copy overrides for one browser card.
pub fn copies(screen: Screen, ui: &BrowserUi) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |id: &str, v: &str| out.push((id.to_owned(), v.to_owned()));
    if !ui.path.is_empty() {
        push("t_crumbs", &ui.path);
    }
    for (i, e) in ui.entries.iter().enumerate().take(4) {
        push(&format!("t_row_{i}"), &e.name);
    }
    // The hidden count is the web's own notice string, formatted with the
    // server's number (row 220 / `workspace-browse.ts:252-256`).
    if ui.hidden_skipped > 0 {
        let n = format!("{} hidden folders aren't shown.", ui.hidden_skipped);
        push("t_hidden", &n);
    }
    if !ui.path_draft.is_empty() {
        push("browser_path", &ui.path_draft);
    }
    if screen == Screen::Refused {
        if let Some(r) = &ui.failure {
            let (head, next) = refusal_copy(&r.kind);
            push("t_cal1", head);
            // The next step is the card's second muted line.
            push("t_cal2", next);
        }
    }
    out
}

/// Lower one browser card to the module's DSL (the `connect::lower_screen`
/// chain, pointed at the phase4 board).
pub fn lower_screen(screen: Screen, ui: &BrowserUi) -> Result<String, String> {
    let dir = crate::design::dir("stage-b/phase4/cards").join(screen.card_dir());
    let card_src = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("read {}: {e}", dir.join("page.card").display()))?;
    let data: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("page.data.json"))
            .map_err(|e| format!("read page.data.json: {e}"))?,
    )
    .map_err(|e| format!("parse page.data.json: {e}"))?;
    let card_src = crate::l0_host::apply_copies(&card_src, &copies(screen, ui));
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &dir.join("kit"))
        .map_err(|e| format!("l0::prepare: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&tree))
        .map_err(|e| format!("to_makepad_ui: {e}"))?;
    // #35b item 1: the ONE card-tap wiring, keyed by the card DIRECTORY.
    Ok(super::taps::wire_card_events_dir(&dsl, &dir))
}

/// The bindings this screen projects.
pub fn query(ui: &BrowserUi, id: &str) -> Option<Value> {
    match id {
        "browser.path" => Some(Value::String(ui.path_draft.clone())),
        "browser.current" => Some(Value::String(ui.path.clone())),
        "browser.rows" => Some(Value::Array(
            ui.entries
                .iter()
                .map(|e| Value::String(e.name.clone()))
                .collect(),
        )),
        "browser.selected" => ui.selected.clone().map(Value::String),
        "browser.hidden_skipped" => Some(json_u32(ui.hidden_skipped)),
        "browser.truncated" => Some(Value::Bool(ui.truncated)),
        "browser.notices" => Some(Value::Array(
            ui.notices().into_iter().map(Value::String).collect(),
        )),
        "browser.can_go_up" => Some(Value::Bool(ui.can_go_up())),
        "browser.refusal" => ui.failure.as_ref().map(|r| Value::String(r.kind.clone())),
        "browser.name_problem" => ui.name_problem().map(|p| Value::String(p.to_owned())),
        _ => None,
    }
}

fn json_u32(n: u32) -> Value {
    Value::from(n)
}

/// The live browser state between taps (the web keeps it in reducer state).
/// One `OnceLock`, the shape `workspace.rs:117-123` established.
fn state() -> std::sync::MutexGuard<'static, BrowserUi> {
    static STATE: std::sync::OnceLock<std::sync::Mutex<BrowserUi>> =
        std::sync::OnceLock::new();
    STATE
        .get_or_init(|| std::sync::Mutex::new(BrowserUi::default()))
        .lock()
        .unwrap()
}

/// Replace the live browser state (the transport's listing result).
pub fn set(ui: BrowserUi) {
    *state() = ui;
}

/// Apply one action to the LIVE state and return the transport effect. The
/// production entry point; [`resolve`] + [`apply`] stay pure for the tests.
pub fn perform(id: &str, value: Option<&str>) -> Option<Effect> {
    apply(&mut state(), resolve(id, value))
}

/// A whitelisted refusal from the transport, routed to the live state: the
/// current folder and the last good listing survive (walk 221).
pub fn perform_refused(typed: bool, kind: &str, banned_root: Option<String>) {
    if let Some(r) = refusal_from(typed, kind, banned_root) {
        apply(&mut state(), Effect::Refused(r));
    }
}

/// The live browser card, for the production mount.
pub fn lower_mounted() -> Result<String, String> {
    let ui = state();
    lower_screen(ui.screen, &ui)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing() -> BrowserUi {
        BrowserUi {
            path: "/srv/fixture".into(),
            entries: vec![
                Entry { name: "Projects".into(), path: "/srv/fixture/Projects".into() },
                Entry { name: "archive".into(), path: "/srv/fixture/archive".into() },
            ],
            hidden_skipped: 2,
            ..Default::default()
        }
    }

    #[test]
    fn the_hidden_count_is_reported_not_swallowed() {
        // walk 220 pins "2 hidden folders aren't shown."
        let n = listing().notices();
        assert!(n.iter().any(|s| s == "2 hidden folders aren't shown."), "{n:?}");
    }

    #[test]
    fn truncation_is_reported_before_the_hidden_count() {
        // workspace-browse.ts:246-256 — truncated first, then hidden.
        let mut ui = listing();
        ui.truncated = true;
        ui.entries.truncate(1);
        let n = ui.notices();
        assert_eq!(n[0], "Only the first 1 folders are shown.");
        assert_eq!(n[1], "2 hidden folders aren't shown.");
    }

    #[test]
    fn no_hidden_count_means_no_notice() {
        let mut ui = listing();
        ui.hidden_skipped = 0;
        assert!(ui.notices().is_empty());
    }

    #[test]
    fn up_is_disabled_at_the_root() {
        // walk 220
        let mut ui = listing();
        assert!(ui.can_go_up());
        ui.path = "/".into();
        assert!(!ui.can_go_up());
        assert_eq!(ui.up(), None);
        // And the tap stays dead there rather than navigating nowhere.
        let mut ui2 = listing();
        ui2.path = "/".into();
        assert!(apply(&mut ui2, Effect::Up).is_none());
    }

    #[test]
    fn up_walks_one_level() {
        let mut ui = listing();
        assert_eq!(ui.up().as_deref(), Some("/srv"));
        ui.path = "/srv".into();
        assert_eq!(ui.up().as_deref(), Some("/"));
    }

    #[test]
    fn a_refusal_keeps_the_folder_we_were_standing_in() {
        // walk 221 — the current folder is unchanged.
        let mut ui = listing();
        let r = refusal_from(true, "workspace_list_permission_denied", None).expect("typed");
        apply(&mut ui, Effect::Refused(r));
        assert_eq!(ui.path, "/srv/fixture");
        assert_eq!(ui.entries.len(), 2);
        assert_eq!(ui.screen, Screen::Refused);
    }

    #[test]
    fn a_refusal_shows_bounded_copy_never_the_servers_prose() {
        // walk 221 pins both halves, and asserts `workspace_list` is absent.
        let (head, next) = refusal_copy("workspace_list_permission_denied");
        assert_eq!(head, "Octos can't open that folder.");
        assert_eq!(next, "Pick a folder the Octos server is allowed to read.");
        for k in LIST_REFUSAL_KINDS.iter().chain(CREATE_REFUSAL_KINDS) {
            let (h, n) = refusal_copy(k);
            assert!(!h.is_empty() && !n.is_empty(), "{k} needs a next step");
            assert!(!h.contains("workspace_list"), "{k} leaks the server kind");
            assert!(!h.contains("workspace_create"), "{k} leaks the server kind");
        }
    }

    #[test]
    fn an_untyped_error_with_a_matching_kind_is_not_a_refusal() {
        // workspace-browse.ts:105-110 — identity, not duck-typing.
        assert!(refusal_from(false, "workspace_list_permission_denied", None).is_none());
        assert!(refusal_from(true, "not_a_real_kind", None).is_none());
    }

    #[test]
    fn banned_root_rides_only_a_root_escape() {
        let r = refusal_from(true, "workspace_list_root_escape", Some("/etc".into()))
            .expect("typed");
        assert_eq!(r.banned_root.as_deref(), Some("/etc"));
        // A non-escape kind must NOT be able to smuggle a path through.
        let r2 = refusal_from(true, "workspace_list_permission_denied", Some("/etc".into()))
            .expect("typed");
        assert_eq!(r2.banned_root, None);
    }

    #[test]
    fn picking_a_row_fills_the_path_box_without_navigating() {
        // walk 223
        let mut ui = listing();
        let p = apply(&mut ui, Effect::Enter(0));
        assert!(p.is_none(), "picking must not navigate");
        assert_eq!(ui.path_draft, "/srv/fixture/Projects");
        assert_eq!(ui.path, "/srv/fixture", "the browser stays in its parent");
        assert_eq!(ui.selected.as_deref(), Some("Projects"));
    }

    #[test]
    fn a_new_folder_name_is_prevalidated_before_any_request() {
        // walk 164
        let mut ui = BrowserUi::default();
        ui.new_folder = "a/b".into();
        assert!(ui.name_problem().is_some());
        let t = apply(&mut ui, Effect::Create(String::new()));
        assert!(t.is_none(), "an invalid name must not reach the transport");
        ui.new_folder = "notes".into();
        assert!(ui.name_problem().is_none());
    }

    #[test]
    fn the_browse_gate_fails_closed() {
        // workspace-browse.ts:126-135
        assert!(!BrowserUi::browse_visible(false));
        assert!(BrowserUi::browse_visible(true));
    }

    #[test]
    fn every_declared_action_is_routed() {
        assert_eq!(unrouted(), Vec::<&str>::new());
    }
}
