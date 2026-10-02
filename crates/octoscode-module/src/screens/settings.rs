//! #D2b / A3 — board 2, settings half (screens 6-12): the ONE owner of every
//! settings action id — the ids the seven Stage B cards declare
//! (`design/stage-b/settings/cards/settings-06..12/service-actions.json`,
//! authored by lane D2b) AND the native Settings chrome (`crate::chrome`),
//! which dispatches the SAME ids, so a card tap and a chrome click mean the
//! same thing.
//!
//! ## RPC-mapped actions (every value cited)
//!
//! - `server.stop.confirm` → `server/shutdown`, params `{}` — the typed
//!   `octoscode_client::domains::config::ServerShutdown` (config.rs:208-228;
//!   web `server-methods.ts:7`). ADVERTISED-gated like the web's row
//!   (`canStopServer={supportsMethod(session.capabilities,
//!   APPUI_SERVER_METHODS.SHUTDOWN)}`, App.tsx:3454; the row also needs
//!   `connected`, GeneralSettingsContent.tsx:129-130).
//! - `perm_ask|perm_workspace|perm_full` → `permission/profile/set` with the
//!   web's update shape `{session_id, update:{...}}` (App.tsx:1966-1972).
//!   Axes: mode read_only|workspace_write|danger_full_access
//!   (session-defaults.ts:16-17), network deny|allow, approval_policy
//!   ask|never (permissions-section.tsx:27). Presets: "Ask for approval" =
//!   workspace_write+deny+ask, "Auto-approve in workspace" =
//!   workspace_write+deny+never, "Full access" = danger_full_access+allow+never.
//!
//! ## UI-local by design (the web keeps these client-side too)
//!
//! - the stop CONFIRM GATE: `server.stop.request` only raises the pending
//!   flag; `server.stop.cancel` clears it with no RPC; only the dialog's
//!   `server.stop.confirm` sends (GeneralSettingsContent.tsx:123-147).
//! - the sandbox toggles: the NEW-CHAT defaults (session-defaults.ts: a
//!   client preference applied at creation only); the open session's sandbox
//!   is fixed (sandbox-section.tsx:2).
//! - the thinking segments (client-local `onShowThinkingChange`,
//!   SessionConfigPane.tsx:167-169) — written to the store's per-session
//!   thinking prefs, the seam board 3 reads.
//! - desktop notifications (desktop-notifications.ts: a client preference).
//! - section navigation, `settings.defaults.open`, `settings.advanced`.
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use octoscode_store::Store;

use crate::flow::Conversation;

/// The seven Stage B cards this table owns ids for (id → title; the design
/// record under `design/stage-b/settings/cards/`).
pub const CARDS: &[(&str, &str)] = &[
    ("settings-06", "Settings general"),
    ("settings-07", "Stop server confirm"),
    ("settings-08", "Settings permissions"),
    ("settings-09", "Settings sandbox"),
    ("settings-10", "New-chat defaults"),
    ("settings-11", "Settings model thinking"),
    ("settings-12", "Held by another client"),
];

pub fn is_screen(id: &str) -> bool {
    CARDS.iter().any(|(c, _)| *c == id)
}

fn cards_root() -> std::path::PathBuf {
    crate::design::dir("stage-b/settings/cards")
}

/// Lower one settings card to Splash DSL with its taps wired (the design
/// record's dev mount, `OCTOSCODE_SCREEN=settings-0N`). The PRODUCT Settings
/// is the native chrome (`crate::chrome::OcSettingsPanel`).
pub fn lower(screen_id: &str) -> Result<String, String> {
    let dir = cards_root().join(screen_id);
    let card_text = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("octoscode: {screen_id}/page.card: {e}"))?;
    let data_text =
        std::fs::read_to_string(dir.join("page.data.json")).unwrap_or_else(|_| "{}".into());
    let data: Value = serde_json::from_str(&data_text)
        .map_err(|e| format!("octoscode: {screen_id}/page.data.json: {e}"))?;
    let prepared = octoscript_makepad::l0::prepare(&card_text, &data, &dir.join("kit"))?;
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&prepared.tree))?;
    Ok(crate::screens::taps::wire_card_events_dir(&dsl, &dir))
}

// ------------------------------------------------------------------ actions

/// Every id this table owns: the cards' ids (verbatim from their
/// service-actions.json) plus the native chrome's section/toggle ids.
pub const ACTIONS: &[&str] = &[
    "server.stop.request",
    "server.stop.confirm",
    "server.stop.cancel",
    "perm_ask.select",
    "perm_workspace.select",
    "perm_full.select",
    "settings.advanced",
    "settings.defaults.open",
    "sandbox_write.toggle",
    "sandbox_network.toggle",
    "sandbox_read_outside.toggle",
    "settings.thinking.off",
    "settings.thinking.on",
    "settings.thinking.high",
    "session.take_over",
    "notifications_toggle.toggle",
    // Native chrome: open/close and the section nav.
    "settings.panel.open",
    "settings.panel.close",
    "settings.section.general",
    "settings.section.permissions",
    "settings.section.model",
    "settings.section.sandbox",
    "settings.section.connection",
    "settings.section.preferences",
    "settings.section.about",
    // The Model row: select the next configured model (profile/llm/select).
    "settings.model.next",
];

/// The one-owner rule.
pub fn owns(action: &str) -> bool {
    ACTIONS.contains(&action)
}

/// The web renders the Stop row ONLY when the session's capabilities advertise
/// `server/shutdown` (App.tsx:3454) AND the connection is up
/// (GeneralSettingsContent.tsx:129-130). The native half reads the SAME
/// production fold: the session/open reply's `capabilities.supported_methods`
/// (`set_supported_methods`, flow.rs — #P4g1 row 217).
pub fn can_stop_server(store: &Store) -> bool {
    store.is_live()
        && store
            .domains
            .config
            .supported_methods()
            .iter()
            .any(|m| m == "server/shutdown")
}

/// The PURE `(action, store) → (method, params)` table — RPC-mapped ids only.
/// UI-local ids return None (the caller applies them locally). The confirm is
/// advertised-gated: unmapped when the server does not advertise
/// `server/shutdown` (an unadvertised confirm must not send).
pub fn action_params(action: &str, store: &Store) -> Option<(String, Value)> {
    let session = store.active_session().unwrap_or_default();
    match action {
        // config.rs:208-228 — params {} exactly as the web sends.
        "server.stop.confirm" if can_stop_server(store) => {
            Some(("server/shutdown".to_owned(), json!({})))
        }
        "perm_ask.select" => Some((
            "permission/profile/set".to_owned(),
            json!({"session_id": session,
                   "update": {"mode": "workspace_write", "network": "deny",
                              "approval_policy": "ask"}}),
        )),
        "perm_workspace.select" => Some((
            "permission/profile/set".to_owned(),
            json!({"session_id": session,
                   "update": {"mode": "workspace_write", "network": "deny",
                              "approval_policy": "never"}}),
        )),
        "perm_full.select" => Some((
            "permission/profile/set".to_owned(),
            json!({"session_id": session,
                   "update": {"mode": "danger_full_access", "network": "allow",
                              "approval_policy": "never"}}),
        )),
        // The web's model select (`profile/llm/select`, the shape
        // screens::workspace::select_model sends): the NEXT available
        // configured model after the selected one; none when there is no
        // other model to pick.
        "settings.model.next" => next_model(store).map(|m| {
            (
                "profile/llm/select".to_owned(),
                json!({
                    "family_id": m.family,
                    "model_id": m.model,
                    "profile_id": store.domains.profile.current(),
                    "route_id": m.route,
                    "session_id": session,
                }),
            )
        }),
        _ => None,
    }
}

/// The configured model after the selected one (wrapping), skipping
/// unavailable models; `None` when there is no OTHER available model.
pub fn next_model(store: &Store) -> Option<octoscode_store::domains::profile::ProfileLlmModel> {
    let models = store.domains.profile.llm_models();
    let cur = models.iter().position(|m| m.selected).unwrap_or(0);
    (1..models.len())
        .map(|k| &models[(cur + k) % models.len()])
        .find(|m| m.available && !m.selected)
        .cloned()
}

/// After a confirmed `profile/llm/select`, mark the chosen model selected in
/// the store's picker list (the next `profile/llm/list` refresh confirms it).
pub fn mark_model_selected(store: &Store, model: &str) {
    let models = store
        .domains
        .profile
        .llm_models()
        .into_iter()
        .map(|mut m| {
            m.selected = m.model == model;
            m
        })
        .collect();
    store.domains.profile.set_llm_models(models);
}

/// The RPC half — the models::perform shape (the production client).
pub async fn perform(conv: &Conversation, action: &str, store: &Store) -> Result<Value, String> {
    let Some((method, params)) = action_params(action, store) else {
        return Err(format!("screens/settings: no protocol mapping for {action:?}"));
    };
    let session = params.get("session_id").and_then(|s| s.as_str()).map(str::to_owned);
    let v = conv
        .client()
        .request(&method, params)
        .await
        .map_err(|e| format!("{method}: {e}"))?;
    // A15 — a preset's read-back is the Session's selection from now on (the
    // seat, the strip and the readback show what the server applied), the
    // seat's own `updatePermission` rule (`board3::seats::set_permission`):
    // only a reply naming this Session.
    if method == "permission/profile/set" {
        let echoed = v.get("session_id").and_then(|s| s.as_str()).is_some_and(|s| Some(s) == session.as_deref());
        let current = v.get("current").and_then(|c| {
            serde_json::from_value::<octoscode_store::domains::profile::PermissionProfileSelection>(c.clone()).ok()
        });
        if let (true, Some(sel)) = (echoed, current) {
            store.domains.profile.set_permission_current(sel);
        }
    }
    Ok(v)
}

/// Bookkeeping after a confirmed send (the caller calls this on Ok).
pub fn note_sent(action: &str) {
    let mut st = state().lock().unwrap();
    match action {
        "server.stop.confirm" => {
            st.shutdowns += 1;
            st.stop_pending = false;
            st.stop_busy = false;
            st.stop_failed = false;
        }
        "perm_ask.select" => st.permission = Some(Preset::Ask),
        "perm_workspace.select" => st.permission = Some(Preset::Workspace),
        "perm_full.select" => st.permission = Some(Preset::Full),
        _ => {}
    }
    st.saving = None;
}

/// A failed send: the confirm stays open with the web's error copy
/// (GeneralSettingsContent.tsx:354-359); a preset reports the failure.
pub fn note_failed(action: &str, error: &str) {
    let mut st = state().lock().unwrap();
    if action == "server.stop.confirm" {
        st.stop_busy = false;
        st.stop_failed = true;
    }
    st.saving = None;
    st.last_error = Some(error.to_owned());
}

// --------------------------------------------------------------- UI-local half

/// The permission presets (board 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Ask,
    Workspace,
    Full,
}

impl Preset {
    // A15: the readback line under the presets is the server's report
    // (`permission_readback`), no longer a preset's static claim ("ask before
    // shell, write and network" — octos asks only before risky commands).
    pub fn label(self) -> &'static str {
        match self {
            Preset::Ask => "Ask for approval",
            Preset::Workspace => "Auto-approve in workspace",
            Preset::Full => "Full access",
        }
    }
}

/// The thinking setting (board 11's Off | On | High).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thinking {
    Off,
    On,
    High,
}

impl Thinking {
    pub fn label(self) -> &'static str {
        match self {
            Thinking::Off => "Off",
            Thinking::On => "On",
            Thinking::High => "High",
        }
    }
}

/// The new-chat defaults as Settings > Sandbox's three switches (board 9).
/// A8: a VIEW of the persisted `screens::session_defaults` (the web's
/// `SessionDefaults`, session-defaults.ts:24-35 — stored per endpoint,
/// applied at creation only): "Workspace write" = the permission mode
/// (`workspace_write`; off = `read_only`), "Network access" = the network
/// policy (and the sandbox's network access), "Sandbox" = `sandbox.enabled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxDefaults {
    /// `permissionMode` is not `read_only`.
    pub workspace_write: bool,
    /// `network: allow` / `sandbox.networkAccess`.
    pub network: bool,
    /// `sandbox.enabled` (the third switch, the web's "Sandbox" checkbox,
    /// `SettingsDefaultsSection.tsx:88-100`). Field name kept for the chrome.
    pub read_outside: bool,
}

impl Default for SandboxDefaults {
    fn default() -> Self {
        // The web's fallback (SettingsDefaultsSection.tsx:41-45):
        // workspace_write, network deny, sandbox off.
        Self::of(&crate::screens::session_defaults::SessionDefaults::default())
    }
}

impl SandboxDefaults {
    pub fn of(d: &crate::screens::session_defaults::SessionDefaults) -> Self {
        use crate::screens::session_defaults::{NetworkPolicy, PermissionMode};
        Self {
            workspace_write: d.permission_mode != PermissionMode::ReadOnly,
            network: d.network == NetworkPolicy::Allow,
            read_outside: d.sandbox.enabled,
        }
    }
}

/// The Settings surface's UI state.
#[derive(Debug, Clone)]
pub struct SettingsState {
    pub section: crate::chrome::Section,
    /// `server.stop.request` raised it; only the dialog's confirm/cancel clear it.
    pub stop_pending: bool,
    /// The confirm was sent and is awaiting the server.
    pub stop_busy: bool,
    /// The last shutdown was not confirmed (the dialog shows the error).
    pub stop_failed: bool,
    /// Confirmed `server/shutdown` sends (the cancel test's zero).
    pub shutdowns: usize,
    /// The last preset this client set (the readback when the server's stamp
    /// does not carry the approval policy — the web's "as set here").
    pub permission: Option<Preset>,
    /// A preset send in flight.
    pub saving: Option<Preset>,
    pub last_error: Option<String>,
    pub notifications: bool,
    pub sandbox: SandboxDefaults,
    pub last_ui_action: Option<String>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            section: crate::chrome::Section::General,
            stop_pending: false,
            stop_busy: false,
            stop_failed: false,
            shutdowns: 0,
            permission: None,
            saving: None,
            last_error: None,
            // A7 — consent (`desktop-notifications.ts:27-44`): OFF until the
            // Settings action is used, whatever the OS would allow; the
            // explicit opt-in is remembered across restarts.
            notifications: notification_consent(),
            sandbox: SandboxDefaults::default(),
            last_ui_action: None,
        }
    }
}

/// A7 — where the desktop-notification opt-in is kept (the web's
/// `octoscode-web:desktop-notifications` preference):
/// `$HOME/.octoscode/desktop-notifications.json`, overridable with
/// `OCTOSCODE_NOTIFICATIONS_FILE`.
pub fn notification_pref_path() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("OCTOSCODE_NOTIFICATIONS_FILE") {
        if !p.is_empty() {
            return Some(p.into());
        }
    }
    std::env::var("HOME")
        .ok()
        .map(|h| std::path::Path::new(&h).join(".octoscode/desktop-notifications.json"))
}

/// A7 — the remembered opt-in; absent / unreadable = OFF (no notification —
/// so no OS permission prompt — until the Settings toggle is used).
pub fn notification_consent() -> bool {
    notification_pref_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("enabled").and_then(|b| b.as_bool()))
        .unwrap_or(false)
}

/// A7 — persist the explicit Settings choice. A blocked write keeps the
/// choice for this run only (the web: "This preference can stay in memory
/// when browser storage is blocked").
pub fn save_notification_consent(enabled: bool) {
    let Some(p) = notification_pref_path() else { return };
    let ok = p.parent().map(std::fs::create_dir_all).transpose().is_ok()
        && std::fs::write(&p, json!({ "enabled": enabled }).to_string()).is_ok();
    if !ok {
        ::log::warn!("octoscode: the desktop-notification preference stays in memory (storage blocked)");
    }
}

fn state() -> &'static Mutex<SettingsState> {
    static S: OnceLock<Mutex<SettingsState>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(SettingsState::default()))
}

/// A copy of the state (the chrome reads it once per sync). The sandbox
/// switches are read from the persisted new-session defaults.
pub fn snapshot() -> SettingsState {
    let mut s = state().lock().unwrap().clone();
    s.sandbox = SandboxDefaults::of(&crate::screens::session_defaults::current().value);
    s
}

/// Test seam (also forgets the new-session defaults: unit tests run on a
/// fresh memory store).
pub fn reset_state() {
    *state().lock().unwrap() = SettingsState::default();
    if cfg!(test) {
        crate::screens::session_defaults::set_storage(std::sync::Arc::new(crate::screens::recents::MemoryStore::new()));
    }
}

/// What a UI-local action asks the host to do besides the state change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEffect {
    None,
    /// Open the Settings surface (at the state's section).
    Open,
    /// Close the Settings surface.
    Close,
    /// Send the RPC for this id (the caller spawns [`perform`]).
    Send,
    /// Write the thinking choice to the active session's prefs.
    Thinking(Thinking),
    /// "Take over": claim the session from the other client.
    TakeOver,
}

/// Apply a UI-local action (or mark an RPC one as sending) and say what the
/// host must do. The confirm gate lives here: `server.stop.confirm` is only
/// honoured while a request is pending.
pub fn apply_ui(action: &str) -> UiEffect {
    use crate::chrome::Section;
    let mut st = state().lock().unwrap();
    st.last_ui_action = Some(action.to_owned());
    let section = |st: &mut SettingsState, s: Section| {
        st.section = s;
        UiEffect::Open
    };
    match action {
        "server.stop.request" => {
            st.stop_pending = true;
            st.stop_failed = false;
            UiEffect::None
        }
        "server.stop.cancel" => {
            st.stop_pending = false;
            st.stop_busy = false;
            st.stop_failed = false;
            UiEffect::None
        }
        "server.stop.confirm" => {
            if !st.stop_pending || st.stop_busy {
                // No dialog, no shutdown: the gate is stateful.
                return UiEffect::None;
            }
            st.stop_busy = true;
            UiEffect::Send
        }
        "perm_ask.select" | "perm_workspace.select" | "perm_full.select" => {
            st.saving = Some(match action {
                "perm_ask.select" => Preset::Ask,
                "perm_workspace.select" => Preset::Workspace,
                _ => Preset::Full,
            });
            UiEffect::Send
        }
        "settings.model.next" => UiEffect::Send,
        // A8 — the switches edit the persisted new-session defaults
        // (`App.tsx:3538-3546` onDefaultsChange -> saveSessionDefaults).
        "sandbox_write.toggle" => {
            use crate::screens::session_defaults::PermissionMode as M;
            let live = crate::screens::session_defaults::update(|d| {
                d.permission_mode = if d.permission_mode == M::ReadOnly { M::WorkspaceWrite } else { M::ReadOnly };
            });
            st.sandbox = SandboxDefaults::of(&live.value);
            UiEffect::None
        }
        "sandbox_network.toggle" => {
            use crate::screens::session_defaults::NetworkPolicy as N;
            let live = crate::screens::session_defaults::update(|d| {
                d.network = if d.network == N::Allow { N::Deny } else { N::Allow };
            });
            st.sandbox = SandboxDefaults::of(&live.value);
            UiEffect::None
        }
        "sandbox_read_outside.toggle" => {
            let live = crate::screens::session_defaults::update(|d| d.sandbox.enabled = !d.sandbox.enabled);
            st.sandbox = SandboxDefaults::of(&live.value);
            UiEffect::None
        }
        "notifications_toggle.toggle" => {
            st.notifications = !st.notifications;
            // A7 — the explicit opt-in (or opt-out) is the ONLY consent.
            save_notification_consent(st.notifications);
            UiEffect::None
        }
        "settings.thinking.off" => UiEffect::Thinking(Thinking::Off),
        "settings.thinking.on" => UiEffect::Thinking(Thinking::On),
        "settings.thinking.high" => UiEffect::Thinking(Thinking::High),
        "settings.panel.open" => UiEffect::Open,
        "settings.panel.close" => {
            st.stop_pending = false;
            UiEffect::Close
        }
        // The board-10 strip's "Change": the new-chat defaults live in the
        // Sandbox section (and Permissions for the approval preset).
        "settings.defaults.open" => section(&mut st, Section::Sandbox),
        // "Advanced…" under the presets: the sandbox/network detail.
        "settings.advanced" => section(&mut st, Section::Sandbox),
        "session.take_over" => UiEffect::TakeOver,
        other => match other.strip_prefix("settings.section.").and_then(Section::from_id) {
            Some(s) => section(&mut st, s),
            None => UiEffect::None,
        },
    }
}

/// The thinking choice the store's prefs encode (board 11): reasoning hidden
/// = Off; shown with a `high`/`max` effort = High; shown otherwise = On.
pub fn thinking_of(store: &Store) -> Thinking {
    let sid = store.active_session().unwrap_or_default();
    let p = store.domains.session.thinking(&sid);
    if !p.show_reasoning {
        Thinking::Off
    } else if matches!(p.effort.as_str(), "high" | "max") {
        Thinking::High
    } else {
        Thinking::On
    }
}

/// Write a thinking choice to the active session's prefs (the seam board 3's
/// thinking card writes, `screens::board3::perform`).
pub fn set_thinking(store: &Store, t: Thinking) {
    let sid = store.active_session().unwrap_or_default();
    match t {
        Thinking::Off => store.domains.session.set_show_reasoning(&sid, false),
        Thinking::On => {
            store.domains.session.set_show_reasoning(&sid, true);
            store.domains.session.set_thinking_effort(&sid, "medium");
        }
        Thinking::High => {
            store.domains.session.set_show_reasoning(&sid, true);
            store.domains.session.set_thinking_effort(&sid, "high");
        }
    }
}

/// The preset the radios show — the one the SERVER's selection for the
/// active Session matches, or none.
///
/// A15: this used to fall back to "Ask for approval" for anything that was
/// not full access, so a Session the server runs as Write · Network allowed
/// (octos' default for a local Session: `effective_session_permission_state`,
/// `ui_protocol_transport.rs:1681-1726`) read as the Ask preset (Write ·
/// Network blocked · approval on request) — and the header promised it. The
/// server's list reports mode + network (`permission/profile/list`); the
/// approval policy rides its status stamp (`session/status/read`
/// `runtime_policy_stamp.approval_policy`, the web's readback,
/// `App.tsx:3228-3234`); a preset this client set and the server still
/// reports wins for the approval (the web's "as set here").
pub fn preset_of(store: &Store) -> Option<Preset> {
    use octoscode_store::domains::profile::{PermissionNetworkPolicy as N, PermissionProfileMode as M};
    let mine = snapshot().permission;
    let Some(sel) = store.domains.profile.permission() else {
        return mine;
    };
    match (sel.mode, sel.network) {
        (M::DangerFullAccess, N::Allow) => Some(Preset::Full),
        (M::WorkspaceWrite, N::Deny) => Some(match approval_of(store, mine) {
            Some(Approval::Never) => Preset::Workspace,
            _ => Preset::Ask,
        }),
        _ => None,
    }
}

/// The approval policy the server applies to the active Session (octos'
/// two values, `parse_permission_approval_policy`, `:10700-10719`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    /// `on-request`: the server asks when its command policy wants a human
    /// (octos `SafePolicy` ask patterns: `sudo`, `rm -rf`,
    /// `git push --force`, `git reset --hard`, `octos-agent/src/policy.rs`);
    /// workspace writes and other commands run without a prompt.
    OnRequest,
    /// `never`: nothing asks — a command that would ask fails instead
    /// (`coding_tools.rs` "approval_policy is never"); full access forces it.
    Never,
}

/// The active Session's approval policy: a preset this client set that the
/// server's selection still matches, else the status stamp's
/// (`board3::strip::load_status` records it), else octos' rule for the mode
/// (`permission_selection_policy_fields`: never with full access, else
/// on-request).
pub fn approval_of(store: &Store, mine: Option<Preset>) -> Option<Approval> {
    use octoscode_store::domains::profile::{PermissionNetworkPolicy as N, PermissionProfileMode as M};
    let sel = store.domains.profile.permission()?;
    if sel.mode == M::DangerFullAccess {
        return Some(Approval::Never);
    }
    let mine_matches = |p: Preset| match p {
        Preset::Ask => sel.mode == M::WorkspaceWrite && sel.network == N::Deny,
        Preset::Workspace => sel.mode == M::WorkspaceWrite && sel.network == N::Deny,
        Preset::Full => false,
    };
    if let Some(p) = mine.filter(|p| mine_matches(*p)) {
        return Some(if p == Preset::Workspace { Approval::Never } else { Approval::OnRequest });
    }
    let session = store.active_session().unwrap_or_default();
    match crate::screens::board3::strip::stamp_approval(&session).as_deref() {
        Some("never") => Some(Approval::Never),
        _ => Some(Approval::OnRequest),
    }
}

/// The web's permission option name (`permissionName`,
/// `SessionControlBar.tsx:254-256`; labels `permission-projection.ts:53-66`):
/// "Write · Network allowed" — the composer seat's words.
pub fn permission_name(
    mode: octoscode_store::domains::profile::PermissionProfileMode,
    network: octoscode_store::domains::profile::PermissionNetworkPolicy,
) -> String {
    use octoscode_store::domains::profile::{PermissionNetworkPolicy as N, PermissionProfileMode as M};
    let m = match mode {
        M::ReadOnly => "Read",
        M::WorkspaceWrite => "Write",
        M::DangerFullAccess => "Full access",
    };
    let n = if network == N::Allow { "Network allowed" } else { "Network blocked" };
    format!("{m} · {n}")
}

/// Settings > Permissions' readback: what the server reports for the active
/// Session — its selection in the seat's words and its approval policy
/// (the web's "Approval policy: <stamp>", `permissions-section.tsx:54-62`).
pub fn permission_readback(store: &Store) -> String {
    let Some(sel) = store.domains.profile.permission() else {
        return "Server: permissions not reported yet".to_owned();
    };
    let approval = match approval_of(store, snapshot().permission) {
        Some(Approval::Never) => "never asks",
        _ => "asks on request",
    };
    format!("Server: {} · {approval}", permission_name(sel.mode, sel.network))
}

/// The current model's display name: the profile's selected configured model
/// (`profile/llm/list`, ProfileLlmModel.selected), else the first available.
pub fn model_of(store: &Store) -> String {
    let models = store.domains.profile.llm_models();
    models
        .iter()
        .find(|m| m.selected)
        .or_else(|| models.iter().find(|m| m.available))
        .map(|m| if m.title.trim().is_empty() { m.model.clone() } else { m.title.clone() })
        .unwrap_or_else(|| "Default model".to_owned())
}

/// The board-10 strip: "New chat defaults · <approval> · <permissions> ·
/// <model> · Thinking: <On>" — what a New chat ACTUALLY gets.
///
/// A15 (live smoke: the line said "Ask for approval · Workspace write" while
/// the Session ran Write · Network allowed and wrote files with no card):
/// * the permissions are the stored new-session defaults `Conversation::
///   new_chat` applies — ONE `permission/profile/set {mode, network}` right
///   after the created Session opens (the web, `App.tsx:1913-1975`), in the
///   seat's words; with nothing stored nothing is applied and the server's
///   own default holds (octos: Write · Network allowed for a local Session),
///   so the line says so instead of showing the Settings fallback;
/// * the approval is the policy the server derives for that mode — no client
///   (web or native) sends one at creation: never with full access, else on
///   request (`permission_selection_policy_fields`). It was the ACTIVE
///   Session's Settings preset, with "Ask" for anything not full access;
/// * the model is the profile's selected model (a new Session runs it).
///
/// The words are short so A13's phone cut (`chrome::fit_segments`: whole
/// segments, then "…", in 256 px) still keeps the second segment ("Server
/// defaults" / "Asks on request"), not just "New chat defaults · …".
pub fn defaults_line(store: &Store) -> String {
    let live = crate::screens::session_defaults::current();
    let mut parts = vec!["New chat defaults".to_owned()];
    parts.extend(new_chat_permissions(&live));
    parts.push(model_of(store));
    parts.push(format!("Thinking: {}", thinking_of(store).label()));
    parts.join(" · ")
}

/// [`defaults_line`]'s permission facts for the new-session defaults:
/// `[approval, permissions]` when stored, `["Server defaults"]`
/// when nothing is stored (nothing is applied at creation).
pub fn new_chat_permissions(live: &crate::screens::session_defaults::Live) -> Vec<String> {
    use crate::screens::session_defaults::{NetworkPolicy, PermissionMode};
    use octoscode_store::domains::profile::{PermissionNetworkPolicy as N, PermissionProfileMode as M};
    if !live.stored {
        return vec!["Server defaults".to_owned()];
    }
    let d = &live.value;
    let mode = match d.permission_mode {
        PermissionMode::ReadOnly => M::ReadOnly,
        PermissionMode::WorkspaceWrite => M::WorkspaceWrite,
        PermissionMode::DangerFullAccess => M::DangerFullAccess,
    };
    let network = if d.network == NetworkPolicy::Allow { N::Allow } else { N::Deny };
    let approval = if mode == M::DangerFullAccess { "Never asks" } else { "Asks on request" };
    vec![approval.to_owned(), permission_name(mode, network)]
}

/// The settings state, readable through the same `set.*` surface the
/// workspace card exposes (the f29b pattern).
pub fn query(id: &str) -> Option<Value> {
    let st = state().lock().unwrap();
    match id {
        "set.stop_pending" => Some(json!(st.stop_pending)),
        "set.shutdowns" => Some(json!(st.shutdowns)),
        "set.permission_mode" => Some(json!(st.permission.map(|p| p.label()))),
        "set.last_ui_action" => Some(json!(st.last_ui_action)),
        "set.section" => Some(json!(st.section.id())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        crate::screens::theme::test_lock()
    }

    // ---- A7: desktop-notifications.test.ts:22 "does not prompt or notify
    // ---- without explicit opt-in even with existing permission".
    #[test]
    fn notifications_stay_off_until_the_settings_toggle_opts_in() {
        let _g = lock();
        let dir = std::env::temp_dir().join(format!("a7-consent-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("desktop-notifications.json");
        let _ = std::fs::remove_file(&file);
        std::env::set_var("OCTOSCODE_NOTIFICATIONS_FILE", &file);
        reset_state();
        assert!(!snapshot().notifications, "no opt-in yet: no notice can fire, so no OS prompt");
        apply_ui("notifications_toggle.toggle");
        assert!(snapshot().notifications);
        reset_state();
        assert!(snapshot().notifications, "the explicit opt-in survives a restart");
        apply_ui("notifications_toggle.toggle");
        reset_state();
        assert!(!snapshot().notifications, "and so does the opt-out");
        std::env::remove_var("OCTOSCODE_NOTIFICATIONS_FILE");
        reset_state();
    }

    #[test]
    fn the_confirm_is_gated_by_a_pending_request() {
        let _g = lock();
        reset_state();
        // No request: the confirm does nothing (no Send).
        assert_eq!(apply_ui("server.stop.confirm"), UiEffect::None);
        assert_eq!(apply_ui("server.stop.request"), UiEffect::None);
        assert!(snapshot().stop_pending);
        assert_eq!(apply_ui("server.stop.confirm"), UiEffect::Send);
        // A second confirm while busy is ignored.
        assert_eq!(apply_ui("server.stop.confirm"), UiEffect::None);
        note_sent("server.stop.confirm");
        assert_eq!(snapshot().shutdowns, 1);
        assert!(!snapshot().stop_pending);
    }

    #[test]
    fn a_cancelled_stop_never_sends() {
        let _g = lock();
        reset_state();
        apply_ui("server.stop.request");
        assert_eq!(apply_ui("server.stop.cancel"), UiEffect::None);
        assert!(!snapshot().stop_pending);
        assert_eq!(apply_ui("server.stop.confirm"), UiEffect::None, "cancel closed the gate");
        assert_eq!(snapshot().shutdowns, 0);
    }

    #[test]
    fn a_failed_stop_keeps_the_dialog_with_the_error() {
        let _g = lock();
        reset_state();
        apply_ui("server.stop.request");
        apply_ui("server.stop.confirm");
        note_failed("server.stop.confirm", "server/shutdown: timeout");
        let st = snapshot();
        assert!(st.stop_pending && st.stop_failed && !st.stop_busy);
    }

    #[test]
    fn sections_navigate_and_the_strip_opens_the_defaults() {
        use crate::chrome::Section;
        let _g = lock();
        reset_state();
        for s in Section::ALL {
            assert_eq!(apply_ui(&format!("settings.section.{}", s.id())), UiEffect::Open);
            assert_eq!(snapshot().section, s);
            assert!(owns(&format!("settings.section.{}", s.id())));
        }
        apply_ui("settings.defaults.open");
        assert_eq!(snapshot().section, Section::Sandbox);
    }

    #[test]
    fn the_rpc_ids_lower_to_the_cited_wire_shapes() {
        let store = Store::new();
        store.set_connection("Live".into(), true);
        assert_eq!(action_params("server.stop.confirm", &store), None, "unadvertised");
        store
            .domains
            .config
            .set_supported_methods(vec!["server/shutdown".to_owned()]);
        let (m, p) = action_params("server.stop.confirm", &store).unwrap();
        assert_eq!((m.as_str(), p), ("server/shutdown", json!({})));
        let (m, p) = action_params("perm_full.select", &store).unwrap();
        assert_eq!(m, "permission/profile/set");
        assert_eq!(
            p["update"],
            json!({"mode": "danger_full_access", "network": "allow", "approval_policy": "never"})
        );
        // Offline: the Stop row is not offered (connected gate).
        store.set_connection("Offline".into(), false);
        assert!(!can_stop_server(&store));
    }

    #[test]
    fn thinking_round_trips_through_the_store_prefs() {
        let store = Store::new();
        store.set_active(Some("s1".into()));
        for t in [Thinking::Off, Thinking::On, Thinking::High] {
            set_thinking(&store, t);
            assert_eq!(thinking_of(&store), t);
        }
    }

    #[test]
    fn sandbox_toggles_flip_the_new_chat_defaults() {
        let _g = lock();
        reset_state();
        assert!(snapshot().sandbox.workspace_write);
        apply_ui("sandbox_network.toggle");
        apply_ui("sandbox_write.toggle");
        let st = snapshot();
        assert!(st.sandbox.network && !st.sandbox.workspace_write);
        let store = Store::new();
        // A15: the line names what New chat applies, in the seat's words.
        assert_eq!(
            defaults_line(&store),
            "New chat defaults · Asks on request · Read · Network allowed · Default model · Thinking: On"
        );
    }

    /// A15 — the live smoke: nothing stored, so New chat applies nothing and
    /// the server's own default holds; the line must not claim the Settings
    /// fallback ("Ask for approval · Workspace write") as applied.
    #[test]
    fn with_nothing_stored_the_line_names_the_servers_defaults() {
        let _g = lock();
        reset_state();
        let store = Store::new();
        assert!(!crate::screens::session_defaults::current().stored);
        let line = defaults_line(&store);
        assert_eq!(line, "New chat defaults · Server defaults · Default model · Thinking: On");
        assert!(!line.contains("Ask for approval"));
        // Full access stored: the server asks for nothing.
        crate::screens::session_defaults::update(|d| {
            d.permission_mode = crate::screens::session_defaults::PermissionMode::DangerFullAccess;
            d.network = crate::screens::session_defaults::NetworkPolicy::Allow;
        });
        assert_eq!(
            defaults_line(&store),
            "New chat defaults · Never asks · Full access · Network allowed · Default model · Thinking: On"
        );
        reset_state();
    }

    /// A15 — on a phone (A13's one-line cut, 360 - 104 px) the line keeps its
    /// first fact after "New chat defaults", never only the label.
    #[test]
    fn the_phone_cut_keeps_the_first_fact() {
        let _g = lock();
        reset_state();
        let store = Store::new();
        let phone = |s: &Store| crate::chrome::fit_segments(&defaults_line(s), 360.0 - 104.0);
        assert_eq!(phone(&store), "New chat defaults · Server defaults · …");
        crate::screens::session_defaults::update(|d| {
            d.permission_mode = crate::screens::session_defaults::PermissionMode::WorkspaceWrite;
        });
        assert_eq!(phone(&store), "New chat defaults · Asks on request · …");
        reset_state();
    }

    /// A15 — the radios and the readback follow the SERVER: a Session it runs
    /// as Write · Network allowed matches no preset (it read as "Ask for
    /// approval" before), and the readback says what the server reports.
    #[test]
    fn the_permission_radios_follow_the_servers_selection() {
        use octoscode_store::domains::profile::{
            PermissionNetworkPolicy as N, PermissionProfileMode as M, PermissionProfileSelection as Sel,
        };
        let _g = lock();
        reset_state();
        let store = Store::new();
        store.set_active(Some("s1".into()));
        assert_eq!(preset_of(&store), None, "nothing reported, nothing set here");
        assert_eq!(permission_readback(&store), "Server: permissions not reported yet");
        store.domains.profile.set_permission_current(Sel { mode: M::WorkspaceWrite, network: N::Allow });
        assert_eq!(preset_of(&store), None, "Write · Network allowed is none of the presets");
        assert_eq!(permission_readback(&store), "Server: Write · Network allowed · asks on request");
        store.domains.profile.set_permission_current(Sel { mode: M::WorkspaceWrite, network: N::Deny });
        assert_eq!(preset_of(&store), Some(Preset::Ask));
        // This client set "Auto-approve in workspace" (approval never): the
        // server's list carries no approval policy, the preset set here does.
        apply_ui("perm_workspace.select");
        note_sent("perm_workspace.select");
        assert_eq!(preset_of(&store), Some(Preset::Workspace));
        assert_eq!(permission_readback(&store), "Server: Write · Network blocked · never asks");
        store.domains.profile.set_permission_current(Sel { mode: M::DangerFullAccess, network: N::Allow });
        assert_eq!(preset_of(&store), Some(Preset::Full));
        assert_eq!(permission_readback(&store), "Server: Full access · Network allowed · never asks");
        reset_state();
    }
}
