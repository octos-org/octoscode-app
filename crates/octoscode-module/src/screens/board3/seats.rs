//! A10 — the composer's two control seats (web `SessionControlBar.tsx`):
//! the permission seat (left, the approval pill) opens the permission menu,
//! the model seat (right, the model label) opens the model menu. Each menu
//! is the web's popover above its seat (`.menu { bottom: calc(100% + 8px) }`,
//! permission left-aligned, model right-aligned), dismissed by an outside
//! press or Escape; a dangerous permission preset goes through the web's
//! separate confirmation (`PermissionRiskDialog`), never a direct select.
//!
//! Web oracle: `features/product-controls/{SessionControlBar.tsx,
//! selection-policy.ts}`, `features/shell/{permission,model}-projection.ts`,
//! `features/review/use-coding-safety.ts` (the permission read/update),
//! `features/models/{use-model-selection.ts, model-notices.ts}` (the select
//! and its per-Session notice board — `octoscode_store::domains::models`).
//! No Stage-A board draws the menus: built with the native dialog kit
//! (`board3/ui.rs`), the web component as the reference.
use octoscode_store::domains::models::{self as notices, Disposition, Event, Identity, MessageInput};
use octoscode_store::domains::profile::{
    PermissionNetworkPolicy, PermissionProfileMode, PermissionProfileSelection, ProfileLlmModel,
};
use octoscode_store::Store;
use serde_json::{json, Value};

use super::host::{Job, Outcome};
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};
use crate::i18n::tr;

// ------------------------------------------------------------------ copy

/// `PERMISSION_LABELS` (`App.tsx:3896-3903`).
pub const PERMISSION_MENU: &str = "Permission";
pub const PERMISSION_LOADING: &str = "Loading access…";
pub const PERMISSION_UNAVAILABLE: &str = "Permission unavailable";
pub const PERMISSION_EMPTY: &str = "No permission presets are available.";
/// `MODEL_LABELS` (`App.tsx:3905-3912`).
pub const MODEL_MENU: &str = "Model";
pub const MODEL_LOADING: &str = "Loading models…";
pub const MODEL_UNAVAILABLE: &str = "Model unavailable";
pub const MODEL_SELECT: &str = "Select a model";
pub const MODEL_EMPTY: &str = "No models are configured for this profile.";
/// `modelGroups` (`model-projection.ts:44`).
pub const MODEL_UNAVAILABLE_REASON: &str = "This configured model is unavailable.";
pub const EXTERNAL_CHANGE: &str = "The selection changed in another tab or app";
/// `PERMISSION_RISK_COPY` (`App.tsx:3914-3925`).
pub const RISK_TITLE: &str = "Enable full access?";
pub const RISK_DESCRIPTION: &str = "Octos can read and modify files outside the workspace and use the network \
                                    without the normal sandbox boundary.";
pub const RISK_ACCESS: &str = "Filesystem access";
pub const RISK_NETWORK: &str = "Network access";
pub const RISK_ACK: &str = "I understand that this session can make unrestricted changes.";
pub const RISK_HINT: &str = "Tick the box above to enable this button.";
pub const RISK_CONFIRM: &str = "Enable full access";

// ------------------------------------------------------------ projection

/// One permission preset (`SessionPermissionOption`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermOption {
    pub id: String,
    pub mode: PermissionProfileMode,
    pub network: PermissionNetworkPolicy,
    pub mode_label: &'static str,
    pub network_label: &'static str,
    pub dangerous: bool,
}

impl PermOption {
    /// `permissionName`: "{modeLabel} · {networkLabel}".
    pub fn name(&self) -> String {
        format!("{} · {}", self.mode_label, self.network_label)
    }

    /// A24 — [`Self::name`] in the current language (what the seat and the
    /// menu show; `name` stays the web's English for logic and tests).
    pub fn display_name(&self) -> String {
        format!("{} · {}", tr(self.mode_label), tr(self.network_label))
    }
}

pub fn mode_wire(m: PermissionProfileMode) -> &'static str {
    match m {
        PermissionProfileMode::ReadOnly => "read_only",
        PermissionProfileMode::WorkspaceWrite => "workspace_write",
        PermissionProfileMode::DangerFullAccess => "danger_full_access",
    }
}

pub fn network_wire(n: PermissionNetworkPolicy) -> &'static str {
    match n {
        PermissionNetworkPolicy::Allow => "allow",
        PermissionNetworkPolicy::Deny => "deny",
    }
}

/// `permissionOptions` (`permission-projection.ts:30-52`): the current
/// selection first, then every other advertised profile; full access is
/// the dangerous preset.
pub fn permission_options(store: &Store) -> Vec<PermOption> {
    let Some(current) = store.domains.profile.permission() else { return Vec::new() };
    let mut sel = vec![current];
    sel.extend(store.domains.profile.permission_profiles().into_iter().filter(|p| *p != current));
    sel.into_iter()
        .map(|s| PermOption {
            id: format!("{}:{}", mode_wire(s.mode), network_wire(s.network)),
            mode: s.mode,
            network: s.network,
            mode_label: match s.mode {
                PermissionProfileMode::ReadOnly => "Read",
                PermissionProfileMode::WorkspaceWrite => "Write",
                PermissionProfileMode::DangerFullAccess => "Full access",
            },
            network_label: match s.network {
                PermissionNetworkPolicy::Allow => "Network allowed",
                PermissionNetworkPolicy::Deny => "Network blocked",
            },
            dangerous: s.mode == PermissionProfileMode::DangerFullAccess,
        })
        .collect()
}

/// `permissionSelectionIntent` (`selection-policy.ts:20-28`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermIntent {
    None,
    Confirm(PermOption),
    Select(PermOption),
}

pub fn permission_intent(option: &PermOption, selected: Option<&str>, locked: bool) -> PermIntent {
    if locked || selected == Some(option.id.as_str()) {
        PermIntent::None
    } else if option.dangerous {
        PermIntent::Confirm(option.clone())
    } else {
        PermIntent::Select(option.clone())
    }
}

/// `modelOptionId`.
pub fn model_option_id(m: &ProfileLlmModel) -> String {
    format!("{}:{}", m.model, m.route.as_deref().unwrap_or("default"))
}

/// `providerLabel`: "zai-coding" -> "Zai Coding".
pub fn provider_label(provider: &str) -> String {
    if crate::desk::active() {
        return crate::screens::models::family_label(provider);
    }
    provider
        .split(['-', '_'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `modelGroups` (`model-projection.ts:24-48`): by provider, in list order;
/// each row's index into the store's list rides along for the select.
pub fn model_groups(models: &[ProfileLlmModel]) -> Vec<(String, String, Vec<(usize, ProfileLlmModel)>)> {
    let mut groups: Vec<(String, String, Vec<(usize, ProfileLlmModel)>)> = Vec::new();
    for (i, m) in models.iter().enumerate() {
        match groups.iter_mut().find(|g| g.0 == m.provider) {
            Some(g) => g.2.push((i, m.clone())),
            None => groups.push((m.provider.clone(), provider_label(&m.provider), vec![(i, m.clone())])),
        }
    }
    groups
}

/// `profileDefaultNeedsRestart` (`product-projection.ts:3-19`) over the
/// Profile's configured list: the Profile default is the `selected` row.
pub fn profile_default_needs_restart(runtime: Option<(&str, &str)>, models: &[ProfileLlmModel], server_hint: bool) -> bool {
    needs_restart(runtime, models.iter().find(|m| m.selected).map(|m| (m.model.as_str(), m.provider.as_str())), server_hint)
}

/// Only the server can require a restart. A cached status or an active
/// response may still name the previous model after a successful reload.
/// Equal model strings also cannot disprove an explicit route-change hint.
pub fn needs_restart(_runtime: Option<(&str, &str)>, _profile_default: Option<(&str, &str)>, server_hint: bool) -> bool {
    server_hint
}

/// The server's restart hint a selection answer leaves (`restartHint`,
/// `use-model-selection.ts:236-242,297`): a saved selection answering
/// `restart_required` lights it; any other answer — a refusal or a failed
/// request (`None`) included — puts it out. Both selection paths (this menu
/// and the Session settings pane's picker) record it per Session in
/// `PaneState::restart_hints`, the one place the restart truth reads.
pub fn answer_needs_restart(answer: Option<&Value>) -> bool {
    answer.is_some_and(|v| {
        v.get("restart_required").and_then(|b| b.as_bool()) == Some(true)
            || v.get("runtime_disposition").and_then(|d| d.as_str()) == Some("restart_required")
    })
}

fn note_restart_hint(session: &str, lit: bool) {
    super::host::state().pane.restart_hints.insert(session.to_owned(), lit);
}

/// The restart notice (`ModelsSettingsContent.tsx:99-107`), verbatim.
pub fn restart_notice(profile_default: Option<&str>, runtime: Option<&str>) -> String {
    format!(
        "Profile default is {}. This Octos process is still serving {}. Restart Octos to apply the new default.",
        profile_default.filter(|s| !s.trim().is_empty()).unwrap_or("saved"),
        runtime.filter(|s| !s.trim().is_empty()).unwrap_or("its current runtime model"),
    )
}

/// The model seat's label (`ModelControl` trigger): the selected model's
/// name (title, else id; on a Mac the name people read, never the id), else
/// the select prompt.
pub fn model_seat_label(store: &Store) -> String {
    store
        .domains
        .profile
        .llm_models()
        .into_iter()
        .find(|m| m.selected)
        .map(|m| {
            if crate::desk::active() {
                crate::desk::model_title(&m.title, &m.model)
            } else if m.title.is_empty() {
                m.model
            } else {
                m.title
            }
        })
        .unwrap_or_else(|| tr(MODEL_SELECT).to_owned())
}

/// The permission seat's label (`PermissionControl` trigger).
pub fn permission_seat_label(store: &Store) -> Option<String> {
    permission_options(store).into_iter().next().map(|o| o.display_name())
}

/// A10 — the composer's Stop control (web `TurnStopButton`): what the round
/// send/stop control shows and does for the Session's foreground turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopState {
    /// No turn: the send arrow submits.
    Idle,
    /// A local `turn/start` Core has not accepted yet: status, not an
    /// interruptible turn — the control is inert ("Starting…").
    Starting,
    /// An accepted, interruptible turn: Stop sends ONE `turn/interrupt`.
    Stop,
    /// The interrupt is in flight: the control locks ("Stopping…").
    Stopping,
    /// The server does not offer `turn/interrupt`: no Stop control (a typed
    /// draft can still be queued with the send arrow).
    Unavailable,
}

impl StopState {
    /// The status word the transcript's activity row shows instead of the
    /// elapsed time (`App.tsx:2633` `turnStarting ? "Starting…"`; the web's
    /// Stop pill reads "Stopping…").
    pub fn word(self) -> Option<&'static str> {
        match self {
            StopState::Starting => Some("Starting…"),
            StopState::Stopping => Some("Stopping…"),
            _ => None,
        }
    }
}

/// `TurnStopButton`'s projection from the composer store's turn controller
/// (A7: `dispatching_turn` / `interrupting_turn`), the flow's live turn and
/// the advertised `turn/interrupt`.
pub fn stop_state(store: &Store, active_turn: Option<&str>) -> StopState {
    let session = session_of(store);
    let composer = &store.domains.composer;
    let active = composer.snapshot(&session).active.map(|a| a.turn_id).or_else(|| active_turn.map(str::to_owned));
    let Some(active) = active else { return StopState::Idle };
    if composer.dispatching_turn(&session).as_deref() == Some(active.as_str()) {
        return StopState::Starting;
    }
    if !has(store, "turn/interrupt") {
        return StopState::Unavailable;
    }
    if composer.interrupting_turn(&session).as_deref() == Some(active.as_str()) {
        return StopState::Stopping;
    }
    StopState::Stop
}

fn identity(m: &ProfileLlmModel) -> Identity {
    Identity { model: m.model.clone(), provider: m.provider.clone(), route: m.route.clone() }
}

// ----------------------------------------------------------------- state

/// Where a seat sits in the module view (logical px), for the popover.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Anchor {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Default)]
pub struct SeatsState {
    // permission/profile/* (`PermissionRuntimeState`)
    pub perm_loading: bool,
    pub perm_busy: bool,
    pub perm_error: Option<String>,
    /// The dangerous preset awaiting its confirmation, and the box.
    pub pending: Option<PermOption>,
    pub acknowledged: bool,
    /// Request authority: a newer read/write retires an older reply.
    pub perm_gen: u64,
    // profile/llm/* (`ModelSelectionRuntimeState`)
    pub models_loading: bool,
    pub models_busy: bool,
    pub models_error: Option<String>,
    pub models_gen: u64,
    /// A turn runs (the host reports it): `runtimeMutationBlocked`.
    pub turn_busy: bool,
    pub perm_anchor: Option<Anchor>,
    pub model_anchor: Option<Anchor>,
    /// Settings' Model button, while its menu is open (a Mac): the menu opens
    /// under it instead of over the composer's seat.
    pub settings_anchor: Option<Anchor>,
    /// The button an opening from Settings hands to `host::open`.
    pub settings_next: Option<Anchor>,
    /// The Session whose permission profile the seat already asked for (the
    /// web reads it once per opened Session, `refreshPermission`).
    pub read_for: Option<String>,
}

fn has(store: &Store, m: &str) -> bool {
    store.domains.config.supported_methods().iter().any(|x| x == m)
}

/// The seats' capability gates (`SessionControlBar`: a missing capability
/// removes its seat instead of exposing a dead control).
pub fn permission_seat(store: &Store) -> bool {
    store.is_live() && has(store, "permission/profile/list")
}

pub fn model_seat(store: &Store) -> bool {
    store.is_live() && has(store, "profile/llm/list")
}

/// A Mac's composer keeps its shape while a lost connection re-dials: a seat
/// the server offered stays, dimmed under its veil, and takes no click
/// (`OctoscodeView::sync_seats`). (permission, model)
pub fn seats_held(store: &Store) -> (bool, bool) {
    let held = crate::desk::active() && !store.is_live() && store.outage().is_some();
    (held && has(store, "permission/profile/list"), held && has(store, "profile/llm/list"))
}

pub fn permission_locked(st: &SeatsState, store: &Store) -> bool {
    st.turn_busy || st.perm_busy || !has(store, "permission/profile/set")
}

pub fn model_locked(st: &SeatsState, store: &Store) -> bool {
    st.models_busy || !has(store, "profile/llm/select")
}

fn session_of(store: &Store) -> String {
    store.active_session().unwrap_or_default()
}

// ---------------------------------------------------------------- routing

pub fn on_open_permission(st: &mut SeatsState) -> Outcome {
    st.pending = None;
    st.acknowledged = false;
    Outcome::Spawn(Job::PermissionLoad)
}

pub fn on_open_models(_st: &mut SeatsState) -> Outcome {
    Outcome::Spawn(Job::ModelsLoad)
}

/// Route one `b3.perm.*` / `b3.model.*` action.
pub fn perform(st: &mut SeatsState, action: &str, index: usize, store: &Store) -> Outcome {
    match action {
        "b3.perm.retry" if !st.perm_loading => Outcome::Spawn(Job::PermissionLoad),
        "b3.perm.choose" => {
            let options = permission_options(store);
            let Some(option) = options.get(index) else { return Outcome::Done };
            let selected = options.first().map(|o| o.id.clone());
            match permission_intent(option, selected.as_deref(), permission_locked(st, store)) {
                // `choose` closes the menu first; the selected preset (or a
                // locked menu) then does nothing else.
                PermIntent::None => Outcome::Close,
                PermIntent::Confirm(o) => {
                    st.acknowledged = false;
                    st.pending = Some(o);
                    Outcome::Done
                }
                PermIntent::Select(o) => Outcome::Spawn(Job::PermissionSet(mode_wire(o.mode), network_wire(o.network))),
            }
        }
        "b3.perm.ack" if !permission_locked(st, store) => {
            st.acknowledged = !st.acknowledged;
            Outcome::Done
        }
        "b3.perm.cancel" => {
            st.pending = None;
            st.acknowledged = false;
            Outcome::Done
        }
        "b3.perm.confirm" => {
            if !st.acknowledged || permission_locked(st, store) {
                return Outcome::Done;
            }
            let Some(o) = st.pending.take() else { return Outcome::Done };
            st.acknowledged = false;
            Outcome::Spawn(Job::PermissionSet(mode_wire(o.mode), network_wire(o.network)))
        }
        "b3.model.retry" if !st.models_loading => Outcome::Spawn(Job::ModelsLoad),
        "b3.model.choose" => {
            // `modelSelectionIntent`: only a new, currently usable entry.
            let models = store.domains.profile.llm_models();
            match models.get(index) {
                Some(m) if !model_locked(st, store) && m.available && !m.selected => {
                    Outcome::Spawn(Job::ModelSelect(index))
                }
                _ => Outcome::Done,
            }
        }
        a if a.starts_with("b3.perm.") || a.starts_with("b3.model.") => Outcome::Done,
        _ => Outcome::Unrouted,
    }
}

pub fn note_turn_busy(st: &mut SeatsState, busy: bool) {
    st.turn_busy = busy;
}

// -------------------------------------------------------------- transport

fn with<R>(f: impl FnOnce(&mut SeatsState) -> R) -> R {
    f(&mut super::host::state().seats)
}

fn selection_of(v: &Value) -> Option<PermissionProfileSelection> {
    serde_json::from_value(json!({ "mode": v.get("mode")?, "network": v.get("network")? })).ok()
}

/// The composer pill's read-back (`set.permission_mode`).
fn note_mode(sel: &PermissionProfileSelection) {
    crate::screens::workspace::note_permission_mode(Some(mode_wire(sel.mode)));
}

/// `refreshPermission` — `permission/profile/list {session_id}`; the reply
/// must echo the Session.
pub async fn load_permission(conv: &crate::flow::Conversation) -> Result<String, String> {
    let store = &conv.store;
    let session = session_of(store);
    let ticket = with(|st| {
        if st.perm_busy {
            return None;
        }
        st.perm_gen += 1;
        st.perm_loading = true;
        st.perm_error = None;
        Some(st.perm_gen)
    });
    let Some(ticket) = ticket else { return Ok("refused (a permission change is in flight)".into()) };
    let r = conv.client().request("permission/profile/list", json!({ "session_id": session })).await;
    let outcome = match r {
        Ok(v) if v.get("session_id").and_then(|s| s.as_str()) != Some(session.as_str()) => {
            Err("permission/profile/list returned another session".to_owned())
        }
        Ok(v) => match v.get("current").and_then(selection_of) {
            Some(current) => {
                let profiles: Vec<PermissionProfileSelection> = v
                    .get("profiles")
                    .and_then(|p| p.as_array())
                    .map(|a| a.iter().filter_map(selection_of).collect())
                    .unwrap_or_default();
                Ok((current, profiles))
            }
            None => Err("permission/profile/list returned no current profile".to_owned()),
        },
        Err(e) => Err(crate::screens::dialog::display_error(&e.to_string())),
    };
    let current = with(|st| st.perm_gen == ticket);
    if !current {
        return Ok("superseded".into());
    }
    match outcome {
        Ok((cur, profiles)) => {
            store.domains.profile.set_permission(cur, profiles);
            note_mode(&cur);
            with(|st| st.perm_loading = false);
            Ok(format!("{} preset(s)", permission_options(store).len()))
        }
        Err(e) => {
            with(|st| {
                st.perm_loading = false;
                st.perm_error = Some(e.clone());
            });
            Err(e)
        }
    }
}

/// `updatePermission` — `permission/profile/set {session_id, update}`: only
/// an advertised profile, the reply must echo the Session and apply; the
/// read-back becomes current, then the list is re-read.
pub async fn set_permission(conv: &crate::flow::Conversation, mode: &str, network: &str) -> Result<String, String> {
    let store = &conv.store;
    let session = session_of(store);
    let next: Option<PermissionProfileSelection> = selection_of(&json!({ "mode": mode, "network": network }));
    let advertised = next.is_some_and(|n| {
        store.domains.profile.permission() == Some(n) || store.domains.profile.permission_profiles().contains(&n)
    });
    if !advertised {
        let e = "The requested permission profile was not advertised for this session".to_owned();
        with(|st| st.perm_error = Some(e.clone()));
        return Err(e);
    }
    let ticket = with(|st| {
        if st.perm_busy || st.turn_busy {
            return None;
        }
        st.perm_gen += 1; // a mutation supersedes any older read
        st.perm_busy = true;
        st.perm_loading = false;
        st.perm_error = None;
        Some(st.perm_gen)
    });
    let Some(ticket) = ticket else { return Ok("refused (locked)".into()) };
    makepad_widgets::SignalToUI::set_ui_signal();
    let r = conv
        .client()
        .request("permission/profile/set", json!({ "session_id": session, "update": { "mode": mode, "network": network } }))
        .await;
    if with(|st| st.perm_gen != ticket) {
        return Ok("superseded".into());
    }
    let outcome = match r {
        Ok(v) if v.get("session_id").and_then(|s| s.as_str()) != Some(session.as_str()) => {
            Err("permission/profile/set returned another session".to_owned())
        }
        Ok(v) if v.get("applied") != Some(&Value::Bool(true)) => {
            Err("The server did not apply the permission change".to_owned())
        }
        Ok(v) => v.get("current").and_then(selection_of).ok_or_else(|| "permission/profile/set returned no current profile".to_owned()),
        Err(e) => Err(crate::screens::dialog::display_error(&e.to_string())),
    };
    match outcome {
        Ok(cur) => {
            store.domains.profile.set_permission_current(cur);
            note_mode(&cur);
            with(|st| st.perm_busy = false);
            let _ = load_permission(conv).await;
            // The menu closes on a choice (`closeMenu()`); the seat shows the
            // read-back.
            super::host::close_if(super::host::Dialog::Permission);
            Ok(format!("{mode}/{network} applied"))
        }
        Err(e) => {
            with(|st| {
                st.perm_busy = false;
                st.perm_error = Some(e.clone());
            });
            Err(e)
        }
    }
}

pub(crate) fn model_from(v: &Value) -> Option<ProfileLlmModel> {
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    let model = text("model").filter(|s| !s.is_empty())?;
    // A Mac names the model, never its id or its route and id.
    let title = text("title").unwrap_or_default();
    let title = if crate::desk::active() { crate::desk::model_title(&title, &model) } else { title };
    Some(ProfileLlmModel {
        model,
        provider: text("provider").filter(|s| !s.is_empty())?,
        title,
        family: text("family"),
        route: text("route"),
        selected: v.get("selected").and_then(|b| b.as_bool())?,
        available: v.get("available").and_then(|b| b.as_bool())?,
    })
}

fn now_ms() -> u64 {
    super::ui::now_ms()
}

/// `refresh` — `profile/llm/list {session_id, profile_id}` (the session
/// picker list, `parseProfileLlmListResult`): echoed Session, at most 500
/// rows, each a whole model; then the board's external-change check.
pub async fn load_models(conv: &crate::flow::Conversation) -> Result<String, String> {
    let store = &conv.store;
    let session = session_of(store);
    let ticket = with(|st| {
        if st.models_busy {
            return None;
        }
        st.models_gen += 1;
        st.models_loading = true;
        st.models_error = None;
        Some(st.models_gen)
    });
    let Some(ticket) = ticket else { return Ok("refused (a selection is in flight)".into()) };
    let mut params = json!({ "session_id": session });
    if let Some(p) = store.domains.profile.current() {
        params["profile_id"] = json!(p);
    }
    let r = conv.client().request("profile/llm/list", params).await;
    if with(|st| st.models_gen != ticket) {
        return Ok("superseded".into());
    }
    let outcome = match r {
        Ok(v) if v.get("session_id").and_then(|s| s.as_str()) != Some(session.as_str()) => {
            Err("profile/llm/list returned another session".to_owned())
        }
        Ok(v) => match v.get("models").and_then(|m| m.as_array()) {
            Some(rows) if rows.len() <= 500 => {
                let models: Option<Vec<ProfileLlmModel>> = rows.iter().map(model_from).collect();
                models.ok_or_else(|| "Invalid profile/llm/list result".to_owned())
            }
            _ => Err("Invalid profile/llm/list result".to_owned()),
        },
        Err(e) => Err(crate::screens::dialog::display_error(&e.to_string())),
    };
    match outcome {
        Ok(models) => {
            store.domains.profile.set_llm_models(models.clone());
            observe_list(store, &session, &models);
            with(|st| st.models_loading = false);
            Ok(format!("{} model(s)", models.len()))
        }
        Err(e) => {
            with(|st| {
                st.models_loading = false;
                st.models_error = Some(e.clone());
            });
            Err(e)
        }
    }
}

/// `observeListResult`: case 23 (another actor changed the selection).
fn observe_list(store: &Store, session: &str, models: &[ProfileLlmModel]) {
    let n = &store.domains.models;
    let last_seen = n.last_seen(session);
    n.apply(
        session,
        Event::ListRefreshed { models: models.iter().map(|m| (identity(m), m.selected)).collect(), last_seen, at_ms: now_ms() },
    );
    if let Some(sel) = models.iter().find(|m| m.selected) {
        n.set_last_seen(session, Some(identity(sel)));
    }
}

/// `select` — `profile/llm/select {session_id, profile_id?, family_id,
/// model_id, route_id?}` for a listed, available entry; the reply must echo
/// the Session and apply (else a refusal); its runtime disposition goes on
/// the Session's notice board; then the list is re-read.
pub async fn select_model(conv: &crate::flow::Conversation, index: usize) -> Result<String, String> {
    let store = &conv.store;
    let session = session_of(store);
    let Some(target) = store.domains.profile.llm_models().get(index).cloned() else {
        return Ok("refused (no such model)".into());
    };
    let ticket = with(|st| {
        if st.models_busy || !target.available {
            return None;
        }
        st.models_gen += 1; // a selection retires an older list read
        st.models_busy = true;
        st.models_loading = false;
        st.models_error = None;
        Some(st.models_gen)
    });
    let Some(ticket) = ticket else { return Ok("refused (locked or unavailable)".into()) };
    store.domains.models.apply(&session, Event::Saving(true));
    makepad_widgets::SignalToUI::set_ui_signal();
    let mut params = json!({
        "session_id": session,
        "family_id": target.family.clone().unwrap_or_else(|| target.provider.clone()),
        "model_id": target.model,
    });
    if let Some(p) = store.domains.profile.current() {
        params["profile_id"] = json!(p);
    }
    if let Some(route) = &target.route {
        params["route_id"] = json!(route);
    }
    let r = conv.client().request("profile/llm/select", params).await;
    if with(|st| st.models_gen != ticket) {
        store.domains.models.apply(&session, Event::Saving(false));
        return Ok("superseded".into());
    }
    let outcome: Result<(Value, ProfileLlmModel), String> = match r {
        Ok(v) => match (v.get("session_id").and_then(|s| s.as_str()), v.get("applied"), v.get("selected").and_then(model_from)) {
            (Some(s), Some(Value::Bool(true)), Some(sel)) if s == session => Ok((v.clone(), sel)),
            (_, Some(Value::Bool(_)), Some(_)) => Err("The server did not apply the model selection".to_owned()),
            _ => Err("Invalid profile/llm/select result".to_owned()),
        },
        Err(e) => Err(crate::screens::dialog::display_error(&e.to_string())),
    };
    store.domains.models.apply(&session, Event::Saving(false));
    with(|st| st.models_busy = false);
    note_restart_hint(&session, answer_needs_restart(outcome.as_ref().ok().map(|(raw, _)| raw)));
    match outcome {
        Ok((raw, selected)) => {
            let rows = store
                .domains
                .profile
                .llm_models()
                .into_iter()
                .map(|mut m| {
                    m.selected = m.model == selected.model && m.provider == selected.provider && m.route == selected.route;
                    m
                })
                .collect();
            store.domains.profile.set_llm_models(rows);
            let parsed = notices::parse_runtime_disposition(&raw);
            let (disposition, runtime_error, condition) = match parsed {
                Some(p) => (p.disposition, p.runtime_error, p.condition),
                None => (Disposition::Persisted, None, None),
            };
            // The model the boot snapshot keeps serving (`restart_required`).
            let running = raw.pointer("/runtime_policy_stamp/model").and_then(|m| m.as_str()).map(str::to_owned);
            let board = store.domains.models.apply(
                &session,
                Event::Disposition {
                    disposition,
                    saved_model: Some(identity(&selected)),
                    input: MessageInput { condition, running_model: running, runtime_error, ..Default::default() },
                    at_ms: now_ms(),
                },
            );
            store.domains.models.set_last_seen(&session, Some(identity(&selected)));
            // One app, one last-seen selection (`lastSeenSelectionRef`): the
            // Session settings pane must not read this app's own choice as
            // another tab's change (case 23).
            super::host::state().pane.last_seen_model = Some(selected.model.clone());
            let _ = load_models(conv).await;
            Ok(board.latest().map(|n| n.message.clone()).unwrap_or_else(|| "Already selected".into()))
        }
        Err(reason) => {
            let board = store.domains.models.apply(
                &session,
                Event::Disposition {
                    disposition: Disposition::Refused,
                    saved_model: Some(identity(&target)),
                    input: MessageInput { reason: Some(reason.clone()), ..Default::default() },
                    at_ms: now_ms(),
                },
            );
            with(|st| st.models_error = Some(reason.clone()));
            Err(board.latest().map(|n| n.message.clone()).unwrap_or(reason))
        }
    }
}

// ------------------------------------------------------------------- view

/// The popover root: a transparent full-frame press target (the web's
/// outside-press dismiss) and the menu card above its seat.
fn popover_open(d: &mut Dsl, frame: &Frame, anchor: Option<Anchor>, right: bool, width: f64) {
    ui::no_sheet_done();
    d.view("b3_root", "width: Fill height: Fill flow: Overlay");
    d.view("b3_backdrop_box", "width: Fill height: Fill flow: Overlay");
    d.tap("b3_backdrop_hit", "b3.close");
    d.close();
    // Above the seat (`bottom: calc(100% + 8px)`), left- or right-aligned to
    // it; without a measured seat, above the composer's bottom-left/right.
    let a = anchor.unwrap_or(Anchor { x: 16.0, y: frame.avail_h - 96.0, w: 120.0, h: 30.0 });
    let bottom = (frame.avail_h - a.y + 8.0).max(8.0).floor();
    // Aligned to the seat's edge, but always inside the frame's 16 px margin
    // (a menu wider than the room beside its seat slides inward).
    let room = (frame.avail_w - 16.0 - width).max(16.0);
    let (align_x, left, right_pad) = if right {
        (1.0, 16.0, (frame.avail_w - (a.x + a.w)).clamp(16.0, room).floor())
    } else {
        (0.0, a.x.clamp(16.0, room).floor(), 16.0)
    };
    d.view(
        "b3_anchor",
        &format!(
            "width: Fill height: Fill flow: Overlay align: Align{{x: {align_x} y: 1.0}} padding: Inset{{left: {left} right: {right_pad} top: 16 bottom: {bottom}}}"
        ),
    );
    d.surface(
        "b3_dialog",
        &format!("width: {} height: Fit flow: Down padding: Inset{{left: 4 right: 4 top: 4 bottom: 4}}", width.floor()),
        tok::SURFACE,
        crate::desk::by(12.0, 8.0),
        Some(tok::HAIRLINE),
    );
}

/// Where a menu row's words start, from the menu's edge: the menu's 4, the row's
/// 6, the tick's 18 and the 4 after it.
const MENU_WORDS: f64 = 32.0;
/// Where a seat's words start, from its edge.
const SEAT_WORDS: f64 = 8.0;

/// A Mac's pop-up button opens its menu over itself (E7 2): the ticked row
/// (`ticked`: its middle, from the menu's top) over the button and its words
/// over the button's. Where that would run past the window's foot (`est_h`:
/// the menu's height as its rows add up), the menu moves up until it fits,
/// still over its button; 16 points inside the frame's top and sides, 8 above
/// its foot (the composer's seats sit 19 over it: a ticked last row fits on its seat).
fn popup_open(d: &mut Dsl, frame: &Frame, anchor: Option<Anchor>, width: f64, ticked: Option<f64>, est_h: f64) {
    ui::no_sheet_done();
    d.view("b3_root", "width: Fill height: Fill flow: Overlay");
    d.view("b3_backdrop_box", "width: Fill height: Fill flow: Overlay");
    d.tap("b3_backdrop_hit", "b3.close");
    d.close();
    let a = anchor.unwrap_or(Anchor { x: 16.0, y: frame.avail_h - 96.0, w: 120.0, h: 30.0 });
    let left = (a.x + SEAT_WORDS - MENU_WORDS).clamp(16.0, (frame.avail_w - 16.0 - width).max(16.0)).floor();
    let top = ticked.map_or(a.y, |t| a.y + a.h / 2.0 - t).max(16.0).floor();
    let place = if top + est_h <= frame.avail_h - 8.0 {
        format!("align: Align{{x: 0.0 y: 0.0}} padding: Inset{{left: {left} right: 16 top: {top} bottom: 8}}")
    } else {
        format!("align: Align{{x: 0.0 y: 1.0}} padding: Inset{{left: {left} right: 16 top: 16 bottom: 8}}")
    };
    d.view("b3_anchor", &format!("width: Fill height: Fill flow: Overlay {place}"));
    d.menu_surface(
        "b3_dialog",
        &format!("width: {} height: Fit flow: Down padding: Inset{{left: 4 right: 4 top: 4 bottom: 4}}", width.floor()),
        8.0,
    );
}

/// Under a pop-up button (Settings' Model on a Mac): the menu's top 4 points under
/// it, its trailing edge on the button's, inside the frame's 16-point margin.
fn popover_under(d: &mut Dsl, frame: &Frame, a: Anchor, width: f64) {
    ui::no_sheet_done();
    d.view("b3_root", "width: Fill height: Fill flow: Overlay");
    d.view("b3_backdrop_box", "width: Fill height: Fill flow: Overlay");
    d.tap("b3_backdrop_hit", "b3.close");
    d.close();
    let room = (frame.avail_w - 16.0 - width).max(16.0);
    let right_pad = (frame.avail_w - (a.x + a.w)).clamp(16.0, room).floor();
    let top = (a.y + a.h + 4.0).floor();
    d.view(
        "b3_anchor",
        &format!("width: Fill height: Fill flow: Overlay align: Align{{x: 1.0 y: 0.0}} padding: Inset{{left: 16 right: {right_pad} top: {top} bottom: 16}}"),
    );
    d.menu_surface(
        "b3_dialog",
        &format!("width: {} height: Fit flow: Down padding: Inset{{left: 4 right: 4 top: 4 bottom: 4}}", width.floor()),
        crate::desk::by(12.0, 8.0),
    );
}

fn popover_close(d: &mut Dsl) {
    d.close(); // b3_dialog
    d.close(); // b3_anchor
    d.close(); // b3_root
}

/// `min(260px, 100vw - 32px)` .. `min(420px, 100vw - 32px)`.
fn menu_width(frame: &Frame, natural: f64) -> f64 {
    let max = (frame.avail_w - 32.0).min(420.0);
    natural.clamp((frame.avail_w - 32.0).min(260.0), max)
}

/// The menu's label (`aria-label={labels.menu}`), drawn as the group
/// titles are (12 px tertiary, the options' 8 px inset).
fn menu_title(d: &mut Dsl, text: &str) {
    // A Mac's pop-up menu opens on its choices.
    if crate::desk::active() {
        d.unseen("b3_title", tr(text));
        return;
    }
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit padding: Inset{left: 8 right: 8 top: 4 bottom: 2}");
    d.text("b3_title", tr(text), &Txt::new(12.0, Face::Medium, tok::FAINT).w(W::Fill));
    d.close();
}

fn status_line(d: &mut Dsl, id: &str, text: &str) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit padding: Inset{left: 10 right: 10 top: 10 bottom: 10}");
    d.text(id, tr(text), &Txt::new(13.0, Face::Regular, tok::FAINT).w(W::Fill).wrap());
    d.close();
}

fn error_line(d: &mut Dsl, id: &str, message: &str, retry: &str) {
    // A Mac's menu says what happened, not which method returned what.
    let message = if crate::desk::active() && (ui::is_protocol_error(message) || message.contains("returned another session")) {
        tr("Couldn't load this list.")
    } else {
        message
    };
    if crate::desk::active() {
        d.view(id, "width: Fill height: Fit flow: Down spacing: 6 align: Align{x: 0.5 y: 0.0} padding: Inset{left: 8 right: 8 top: 8 bottom: 8}");
        d.text(&format!("{id}_text"), message, &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Fill).wrap().centre());
        d.button_ids(&format!("{id}_retry_box"), &format!("{id}_retry_label"), &format!("{id}_retry"), tr("Try again"), retry, Btn::Outline, W::Fit, 28.0);
        d.close();
        return;
    }
    d.surface(
        id,
        "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 0.0 y: 0.0} padding: Inset{left: 8 right: 8 top: 7 bottom: 7} margin: Inset{bottom: 4}",
        tok::RED_BG,
        8.0,
        None,
    );
    d.text(&format!("{id}_text"), message, &Txt::new(12.0, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
    let b = d.anon();
    // A24: an explicit width, >= 28 px: "重试" is two 12 px glyphs, so a Fit
    // box measured 24 px wide in Chinese (under the 28 px hit floor).
    let retry_w = (ui::text_w(tr("Retry"), 12.0, Face::Semibold) + 8.0).max(28.0).ceil();
    // No padding: the tap fills the whole box (the label is centred).
    d.view(&b, &format!("width: {retry_w} height: 28 flow: Overlay align: Align{{x: 0.5 y: 0.5}}"));
    d.text(&format!("{id}_retry_label"), tr("Retry"), &Txt::new(12.0, Face::Semibold, tok::RED_TEXT));
    d.tap(&format!("{id}_retry"), retry);
    d.close();
    d.close();
}

/// One menu option (`.option`: min-height 42, 6/8 padding, radius 10, the
/// icon / copy / check columns).
#[allow(clippy::too_many_arguments)]
pub(super) fn option_row(
    d: &mut Dsl,
    id: &str,
    icon: Option<&str>,
    name: &str,
    description: Option<&str>,
    checked: bool,
    enabled: bool,
    event: &str,
    copy_w: f64,
) {
    d.view(&format!("{id}_box"), "width: Fill height: Fit flow: Overlay");
    // A Mac's menu row: one line is 28 points, and its marks don't set the height.
    let (pad, cell) = crate::desk::by(("top: 6 bottom: 6", 30.0), ("top: 5 bottom: 5", 18.0));
    let row = d.anon();
    d.view(&row, &format!("width: Fill height: Fit flow: Right spacing: {} align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: {} right: 8 {pad}}}", crate::desk::by(8, 4), crate::desk::by(8, 6)));
    // A Mac's menu keeps its check at the leading edge, before the name.
    let desk = crate::desk::active();
    if desk {
        check_cell(d, id, cell, checked);
    }
    if let Some(file) = icon.filter(|_| !desk) {
        let ic = d.anon();
        d.view(&ic, &format!("width: 18 height: {cell} flow: Overlay align: Align{{x: 0.5 y: 0.5}}"));
        d.icon(&format!("{id}_icon"), file, 16.0, tok::MUTED);
        d.close();
    }
    let copy = d.anon();
    d.view(&copy, "width: Fill height: Fit flow: Down spacing: 1");
    let ink = if enabled || (checked && crate::desk::active()) {
        tok::TEXT
    } else if crate::desk::active() {
        // On the menu's material (`menu_surface`), not a sheet's page.
        crate::desk::disabled_menu_ink()
    } else {
        tok::DISABLED_INK
    };
    let name_face = crate::desk::by(Face::Medium, Face::Regular);
    d.text(&format!("{id}_name"), &ui::fit_w(name, copy_w, 14.0, Face::Medium), &Txt::new(14.0, name_face, ink).w(W::Fill));
    if let Some(desc) = description {
        d.text(&format!("{id}_desc"), &ui::fit_w(desc, copy_w, 12.0, Face::Regular), &Txt::new(12.0, Face::Regular, tok::FAINT).w(W::Fill));
    }
    d.close();
    if !desk {
        check_cell(d, id, cell, checked);
    }
    d.close();
    if enabled {
        d.tap(id, event);
    }
    d.close();
}

/// A menu row's check column.
fn check_cell(d: &mut Dsl, id: &str, cell: f64, checked: bool) {
    let ck = d.anon();
    d.view(&ck, &format!("width: 18 height: {cell} flow: Overlay align: Align{{x: 0.5 y: 0.5}}"));
    if checked {
        d.icon(&format!("{id}_check"), "b3_check.svg", crate::desk::by(16.0, 14.0), tok::TEXT);
    }
    d.close();
}

/// The permission menu (`PermissionMenu`), or the full-access confirmation
/// (`PermissionRiskDialog`) while a dangerous preset waits for it.
pub fn build_permission(d: &mut Dsl, st: &SeatsState, frame: &Frame, store: &Store) {
    if let Some(o) = &st.pending {
        build_risk(d, st, frame, store, o);
        return;
    }
    let options = permission_options(store);
    let natural = options
        .iter()
        .map(|o| ui::text_w(&o.display_name(), 14.0, Face::Medium) + 18.0 + 18.0 + 16.0 + 16.0 + 8.0 + 24.0)
        .fold(260.0_f64, f64::max);
    let width = menu_width(frame, natural);
    let copy_w = width - 8.0 - 16.0 - 18.0 - 18.0 - 16.0;
    let locked = permission_locked(st, store);
    let available = permission_seat(store);
    if crate::desk::active() {
        // The current preset is the last row (`order`, below): the seat sits at
        // the window's foot and a menu can't run on past it, so the ticked row
        // opens over the seat as a Mac's pop-up's does, unless a line about the
        // list comes first.
        let lines = !available || (st.perm_loading && options.is_empty()) || st.perm_error.is_some();
        let ticked = (!lines && !options.is_empty()).then(|| 4.0 + 28.0 * (options.len() - 1) as f64 + 14.0);
        let est_h = 8.0 + 28.0 * options.len() as f64 + (if lines || options.is_empty() { 72.0 } else { 0.0 });
        popup_open(d, frame, st.perm_anchor, width, ticked, est_h);
    } else {
        popover_open(d, frame, st.perm_anchor, false, width);
    }
    menu_title(d, PERMISSION_MENU);
    if !available {
        status_line(d, "b3_perm_unavailable", PERMISSION_UNAVAILABLE);
    } else if st.perm_loading && options.is_empty() {
        status_line(d, "b3_perm_loading", PERMISSION_LOADING);
    }
    if let Some(e) = &st.perm_error {
        error_line(d, "b3_perm_error", e, "b3.perm.retry");
    }
    let selected = options.first().map(|o| o.id.clone());
    // A Mac's rows: the others in their order, then the current one (its ids
    // and choices keep their index).
    let order: Vec<usize> = if crate::desk::active() && !options.is_empty() {
        (1..options.len()).chain(0..1).collect()
    } else {
        (0..options.len()).collect()
    };
    for i in order {
        let o = &options[i];
        let icon = if o.dangerous { "b3_shield_danger.svg" } else { "b3_shield.svg" };
        let enabled = !locked && available && !(st.perm_loading && options.is_empty());
        option_row(
            d,
            &format!("b3_perm_opt_{i}"),
            Some(icon),
            &o.display_name(),
            None,
            selected.as_deref() == Some(o.id.as_str()),
            enabled,
            &format!("b3.perm.choose#{i}"),
            copy_w,
        );
    }
    if available && !st.perm_loading && options.is_empty() && st.perm_error.is_none() {
        status_line(d, "b3_perm_empty", PERMISSION_EMPTY);
    }
    popover_close(d);
}

/// `PermissionRiskDialog`: a separate blocking surface (the kit's centred
/// card): the warning shield and title, the description, the summary, the
/// acknowledgement box, the hint while it is unticked, Cancel / Enable full
/// access (inert until ticked).
fn build_risk(d: &mut Dsl, st: &SeatsState, frame: &Frame, store: &Store, o: &PermOption) {
    let width = frame.dialog_w(460.0);
    let locked = permission_locked(st, store);
    ui::shell_open(d, frame, width);
    let head = d.anon();
    d.view(&head, "width: Fill height: 30 flow: Right spacing: 10 align: Align{x: 0.0 y: 0.5}");
    d.icon("b3_risk_icon", "b3_shield_danger.svg", 20.0, tok::RED);
    d.text("b3_title", tr(RISK_TITLE), &Txt::new(18.0, Face::Semibold, tok::TEXT).w(W::Fill));
    d.close();
    d.gap(W::Fill, 10.0);
    d.text("b3_risk_description", tr(RISK_DESCRIPTION), &Txt::new(14.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.gap(W::Fill, 16.0);
    d.surface(
        "b3_risk_summary",
        "width: Fill height: Fit flow: Down spacing: 8 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}",
        tok::SURFACE2,
        12.0,
        None,
    );
    for (id, k, v) in [("b3_risk_access", RISK_ACCESS, o.mode_label), ("b3_risk_network", RISK_NETWORK, o.network_label)] {
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Right spacing: 8");
        d.text(&format!("{id}_k"), tr(k), &Txt::new(13.0, Face::Regular, tok::FAINT).w(W::Px(118.0)));
        d.text(&format!("{id}_v"), tr(v), &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Fill));
        d.close();
    }
    d.close();
    d.gap(W::Fill, 16.0);
    // The acknowledgement (`<input type=checkbox>` + label): one press target.
    d.view("b3_risk_ack_box", "width: Fill height: Fit flow: Overlay");
    let ack = d.anon();
    d.view(&ack, "width: Fill height: Fit flow: Right spacing: 10 align: Align{x: 0.0 y: 0.0} padding: Inset{top: 7 bottom: 7}");
    d.icon("b3_risk_ack_icon", if st.acknowledged { "b3_box_on.svg" } else { "b3_box_off.svg" }, 18.0, tok::RED);
    d.text("b3_risk_ack_label", tr(RISK_ACK), &Txt::new(13.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.close();
    if !locked {
        d.tap("b3_risk_ack", "b3.perm.ack");
    }
    d.close();
    if !st.acknowledged {
        d.gap(W::Fill, 8.0);
        d.text("b3_risk_hint", tr(RISK_HINT), &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    d.gap(W::Fill, 20.0);
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 1.0 y: 0.5}");
    d.button("b3_risk_cancel", tr("Cancel"), "b3.perm.cancel", Btn::Outline, W::Fit, 36.0);
    danger_button(d, "b3_risk_confirm", tr(RISK_CONFIRM), "b3.perm.confirm", st.acknowledged && !locked);
    d.close();
    ui::shell_close(d);
}

/// `.dangerAction`: the error fill, or — inert — the neutral fill (an inert
/// danger action must not read as armed).
fn danger_button(d: &mut Dsl, id: &str, label: &str, event: &str, armed: bool) {
    let w = (ui::text_w(label, 14.0, Face::Medium) + 32.0).ceil();
    let (fill, ink) = if armed { (tok::RED, tok::WHITE) } else { (tok::DISABLED_BG, tok::DISABLED_INK) };
    d.surface(
        &format!("{id}_box"),
        &format!("width: {w} height: {} flow: Overlay align: Align{{x: 0.5 y: 0.5}}", crate::desk::by(36, 28)),
        fill,
        crate::desk::by(18.0, 6.0),
        None,
    );
    d.text(&format!("{id}_label"), label, &Txt::new(14.0, Face::Medium, ink));
    if armed {
        d.tap(id, event);
    }
    d.close();
}

/// A Mac's notice names the model (the menu's name for it, else its id in words),
/// never its id.
fn desk_notice(n: &notices::Notice, models: &[ProfileLlmModel]) -> String {
    let name = |id: &str| {
        models
            .iter()
            .find(|m| m.model == id)
            .map(|m| crate::desk::model_title(&m.title, &m.model))
            .unwrap_or_else(|| crate::desk::model_word(id))
    };
    match (n.kind, n.model.as_ref(), n.running_model.as_deref()) {
        (Disposition::Reloaded, Some(m), _) => crate::i18n::tr1("Saved. Your next message uses {value0}", &name(&m.model)),
        (Disposition::RestartRequired, _, Some(r)) => crate::i18n::tr1("Saved. The server keeps running {value0} until it restarts", &name(r)),
        _ => n.message.clone(),
    }
}

/// A Mac's model menu as its rows add up: where they end and the middle of the
/// ticked row (a group's title 23 points, a row 28, 41 with a line under it, 4
/// between groups, the menu's 4 over the first).
fn desk_rows_end(groups: &[(String, String, Vec<(usize, ProfileLlmModel)>)]) -> (f64, Option<f64>) {
    let (mut y, mut ticked) = (4.0, None);
    for (g, (_, _, rows)) in groups.iter().enumerate() {
        y += (if g > 0 { 4.0 } else { 0.0 }) + 23.0;
        for (_, m) in rows {
            let h = if desk_model_line(m, rows).is_some() { 41.0 } else { 28.0 };
            if m.selected {
                ticked = Some(y + h / 2.0);
            }
            y += h;
        }
    }
    (y, ticked)
}

/// A Mac's model row's line under its name: why it can't be used, or which
/// route it takes when two rows read alike (the rows' own logic, below).
fn desk_model_line(m: &ProfileLlmModel, rows: &[(usize, ProfileLlmModel)]) -> Option<String> {
    let name_of = |m: &ProfileLlmModel| crate::desk::model_title(&m.title, &m.model);
    let name = name_of(m);
    let description = (!m.available).then(|| tr(MODEL_UNAVAILABLE_REASON).to_owned());
    let twin = rows.iter().filter(|(_, o)| name_of(o) == name).count() > 1;
    match (twin, m.route.as_deref(), description) {
        (true, Some(route), Some(d)) if m.available => Some(format!("{d} · {route}")),
        (true, Some(route), None) => Some(crate::screens::models::family_label(route)),
        (_, _, d) => d,
    }
}

/// The model menu (`ModelMenu`): the catalog by provider; unusable entries
/// stay visible, disabled, with their reason; the selected one checked;
/// under it the Session's notice board (Saving…, an external change, the
/// latest disposition notice).
pub fn build_models(d: &mut Dsl, st: &SeatsState, frame: &Frame, store: &Store) {
    let models = store.domains.profile.llm_models();
    let mut groups = model_groups(&models);
    // `width: max-content` (260..420): the widest name or description.
    let natural = models
        .iter()
        .map(|m| {
            let name = ui::text_w(if m.title.is_empty() { &m.model } else { &m.title }, 14.0, Face::Medium);
            let desc = if !m.available {
                ui::text_w(tr(MODEL_UNAVAILABLE_REASON), 12.0, Face::Regular)
            } else if !m.title.is_empty() && m.title != m.model {
                ui::text_w(&m.model, 12.0, Face::Regular)
            } else {
                0.0
            };
            name.max(desc) + 18.0 + 16.0 + 16.0 + 8.0 + 24.0
        })
        .fold(260.0_f64, f64::max);
    let width = menu_width(frame, natural);
    let copy_w = width - 8.0 - 16.0 - 18.0 - 8.0;
    let available = model_seat(store);
    let locked = model_locked(st, store);
    let desk = crate::desk::active();
    // The rows' room: under a phone's Settings button, what is left under it
    // less the notice's; a Mac's menu opens over its button anywhere.
    let body_h = match st.settings_anchor {
        Some(a) if !desk => (frame.avail_h - (a.y + a.h + 4.0) - 16.0 - 96.0).clamp(120.0, 380.0),
        _ => (frame.avail_h - 32.0 - 180.0).clamp(160.0, 380.0),
    };
    if desk {
        // The composer's seat sits at the window's foot and a menu can't run on
        // past it: there the ticked model's group comes last, and its row last
        // in it, so the row opens over the seat as a Mac's pop-up's does (E7 2).
        // Under Settings' Model button the list keeps its order, ticked row
        // over the button, as onboarding's.
        if st.settings_anchor.is_none() && desk_rows_end(&groups).0 - 4.0 <= body_h {
            if let Some(g) = groups.iter().position(|(_, _, rows)| rows.iter().any(|(_, m)| m.selected)) {
                let mut group = groups.remove(g);
                if let Some(r) = group.2.iter().position(|(_, m)| m.selected) {
                    let row = group.2.remove(r);
                    group.2.push(row);
                }
                groups.push(group);
            }
        }
        let lines = !available || (st.models_loading && models.is_empty()) || st.models_error.is_some();
        let (y, ticked) = desk_rows_end(&groups);
        let ticked = ticked.filter(|t| !lines && *t + 14.0 <= 4.0 + body_h);
        let board = store.domains.models.board(&session_of(store));
        let notice = if board.saving || board.external_change || board.latest().is_some() { 72.0 } else { 0.0 };
        let est_h = 8.0 + (y - 4.0).min(body_h) + notice + (if lines || models.is_empty() { 72.0 } else { 0.0 });
        popup_open(d, frame, st.settings_anchor.or(st.model_anchor), width, ticked, est_h);
    } else {
        match st.settings_anchor {
            Some(a) => popover_under(d, frame, a, width),
            None => popover_open(d, frame, st.model_anchor, true, width),
        }
    }
    menu_title(d, MODEL_MENU);
    if !available {
        status_line(d, "b3_model_unavailable", MODEL_UNAVAILABLE);
    } else if st.models_loading && models.is_empty() {
        status_line(d, "b3_model_loading", MODEL_LOADING);
    }
    if let Some(e) = &st.models_error {
        error_line(d, "b3_model_error", e, "b3.model.retry");
    }
    d.open(
        "b3_scroll",
        "ScrollYView",
        &format!("width: Fill height: Fit max_height: {} flow: Down", body_h.floor()),
    );
    for (g, (_, name, rows)) in groups.iter().enumerate() {
        let gt = d.anon();
        d.view(&gt, "width: Fill height: Fit padding: Inset{left: 8 right: 8 top: 5 bottom: 3}");
        d.text(&format!("b3_model_group_{g}"), name, &Txt::new(12.0, Face::Medium, tok::FAINT).w(W::Fill));
        d.close();
        let name_of = |m: &ProfileLlmModel| {
            if desk {
                crate::desk::model_title(&m.title, &m.model)
            } else if m.title.is_empty() {
                m.model.clone()
            } else {
                m.title.clone()
            }
        };
        for (i, m) in rows {
            let name = name_of(m);
            let description = if !m.available {
                Some(tr(MODEL_UNAVAILABLE_REASON).to_owned())
            } else if !crate::desk::active() && !m.title.is_empty() && m.title != m.model {
                Some(m.model.clone())
            } else {
                None
            };
            // Two routes of one model read alike in the web's menu; the
            // route tells them apart (the select already names it).
            let twin = rows.iter().filter(|(_, o)| name_of(o) == name).count() > 1;
            let description = match (twin, m.route.as_deref(), description) {
                (true, Some(route), Some(d)) if m.available => Some(format!("{d} · {route}")),
                (true, Some(route), None) if crate::desk::active() => Some(crate::screens::models::family_label(route)),
        (true, Some(route), None) => Some(route.to_owned()),
                (_, _, d) => d,
            };
            option_row(
                d,
                &format!("b3_model_opt_{i}"),
                None,
                &name,
                description.as_deref(),
                m.selected,
                available && !locked && m.available,
                &format!("b3.model.choose#{i}"),
                copy_w,
            );
        }
        if g + 1 < groups.len() {
            d.gap(W::Fill, 4.0);
        }
    }
    d.close();
    if available && !st.models_loading && models.is_empty() && st.models_error.is_none() {
        status_line(d, "b3_model_empty", MODEL_EMPTY);
    }
    // The Session's notice board (`ModelSection`: Saving… / external change
    // / the outstanding disposition notice).
    let board = store.domains.models.board(&session_of(store));
    if board.saving || board.external_change || board.latest().is_some() {
        d.rule("b3_model_rule", "width: Fill height: 1 margin: Inset{top: 4 bottom: 2}", tok::HAIRLINE);
        let col = d.anon();
        d.view(&col, "width: Fill height: Fit flow: Down spacing: 4 padding: Inset{left: 10 right: 10 top: 6 bottom: 8}");
        if board.saving {
            d.text("b3_model_saving", tr("Saving…"), &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill));
        }
        if board.external_change {
            d.text("b3_model_external", tr(EXTERNAL_CHANGE), &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        }
        if let Some(n) = board.latest() {
            let ink = if n.kind == Disposition::Refused { tok::RED_TEXT } else { tok::TEXT };
            let message = if crate::desk::active() { desk_notice(n, &models) } else { n.message.clone() };
            d.text("b3_model_notice", &message, &Txt::new(12.5, Face::Regular, ink).w(W::Fill).wrap());
        }
        d.close();
    }
    popover_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let s = Store::new();
        s.domains.config.set_supported_methods(
            ["permission/profile/list", "permission/profile/set", "profile/llm/list", "profile/llm/select"]
                .iter()
                .map(|m| m.to_string())
                .collect(),
        );
        s.domains.profile.set_permission(
            PermissionProfileSelection { mode: PermissionProfileMode::WorkspaceWrite, network: PermissionNetworkPolicy::Allow },
            vec![
                PermissionProfileSelection { mode: PermissionProfileMode::ReadOnly, network: PermissionNetworkPolicy::Deny },
                PermissionProfileSelection { mode: PermissionProfileMode::WorkspaceWrite, network: PermissionNetworkPolicy::Deny },
                PermissionProfileSelection { mode: PermissionProfileMode::DangerFullAccess, network: PermissionNetworkPolicy::Allow },
            ],
        );
        s
    }

    /// `TurnStopButton` over the composer store's real transitions: Starting
    /// (not interruptible) -> Stop -> Stopping (locked); no interrupt
    /// capability -> no Stop; Starting is status even then.
    #[test]
    fn the_stop_control_follows_the_turn_controller() {
        use octoscode_store::domains::composer::{PromptTurn, StartOutcome};
        let s = store();
        s.domains.session.set_active(Some("dsflash:main".into()));
        let mut methods = s.domains.config.supported_methods();
        methods.push("turn/interrupt".into());
        s.domains.config.set_supported_methods(methods.clone());
        let c = &s.domains.composer;
        assert_eq!(stop_state(&s, None), StopState::Idle);
        c.submit("dsflash:main", PromptTurn::local("t-1", "fix the parser"));
        assert!(c.begin_dispatch("dsflash:main", "t-1").is_some());
        assert_eq!(stop_state(&s, Some("t-1")), StopState::Starting);
        assert_eq!(StopState::Starting.word(), Some("Starting…"));
        c.finish_dispatch("dsflash:main", "t-1", StartOutcome::Accepted);
        assert_eq!(stop_state(&s, Some("t-1")), StopState::Stop);
        assert!(c.begin_interrupt("dsflash:main", "t-1"));
        assert_eq!(stop_state(&s, Some("t-1")), StopState::Stopping);
        assert!(!c.begin_interrupt("dsflash:main", "t-1"), "locked: one interrupt");
        c.interrupt_failed("dsflash:main", "t-1");
        assert_eq!(stop_state(&s, Some("t-1")), StopState::Stop, "a failed interrupt unlocks");
        // The server offers no interrupt: no Stop control.
        s.domains.config.set_supported_methods(methods.into_iter().filter(|m| m != "turn/interrupt").collect());
        assert_eq!(stop_state(&s, Some("t-1")), StopState::Unavailable);
    }

    #[test]
    fn the_permission_projection_is_the_webs() {
        let s = store();
        let o = permission_options(&s);
        let names: Vec<String> = o.iter().map(|o| o.name()).collect();
        assert_eq!(
            names,
            ["Write · Network allowed", "Read · Network blocked", "Write · Network blocked", "Full access · Network allowed"],
            "the current selection first, then the other profiles"
        );
        assert_eq!(o[0].id, "workspace_write:allow");
        assert!(o[3].dangerous && !o[1].dangerous);
        // The selection policy: dangerous -> confirm, standard -> select,
        // the selected one or a locked menu -> nothing.
        assert!(matches!(permission_intent(&o[3], Some(&o[0].id), false), PermIntent::Confirm(_)));
        assert!(matches!(permission_intent(&o[1], Some(&o[0].id), false), PermIntent::Select(_)));
        assert_eq!(permission_intent(&o[0], Some(&o[0].id), false), PermIntent::None);
        assert_eq!(permission_intent(&o[1], Some(&o[0].id), true), PermIntent::None);
    }

    #[test]
    fn a_dangerous_preset_needs_the_box_and_the_confirm() {
        let s = store();
        let mut st = SeatsState::default();
        assert_eq!(perform(&mut st, "b3.perm.choose", 3, &s), Outcome::Done, "no direct select");
        assert!(st.pending.is_some());
        assert_eq!(perform(&mut st, "b3.perm.confirm", 0, &s), Outcome::Done, "inert until ticked");
        perform(&mut st, "b3.perm.ack", 0, &s);
        assert_eq!(perform(&mut st, "b3.perm.confirm", 0, &s), Outcome::Spawn(Job::PermissionSet("danger_full_access", "allow")));
        assert!(st.pending.is_none());
        assert_eq!(perform(&mut st, "b3.perm.choose", 1, &s), Outcome::Spawn(Job::PermissionSet("read_only", "deny")));
        st.turn_busy = true;
        assert_eq!(perform(&mut st, "b3.perm.choose", 1, &s), Outcome::Close, "locked while a turn runs: nothing but the close");
    }

    #[test]
    fn the_model_groups_are_the_webs() {
        let m = |p: &str, model: &str, title: &str, sel: bool, avail: bool| ProfileLlmModel {
            model: model.into(),
            provider: p.into(),
            title: title.into(),
            family: None,
            route: None,
            selected: sel,
            available: avail,
        };
        let rows = vec![m("zai-coding", "glm-5.2", "GLM-5.2", true, true), m("deepseek", "v4", "", false, true), m("zai-coding", "glm-old", "", false, false)];
        let g = model_groups(&rows);
        assert_eq!(g.iter().map(|g| g.1.as_str()).collect::<Vec<_>>(), ["Zai Coding", "Deepseek"]);
        assert_eq!(g[0].2.iter().map(|(i, _)| *i).collect::<Vec<_>>(), [0, 2]);
        assert_eq!(model_option_id(&rows[0]), "glm-5.2:default");
        let s = store();
        s.domains.profile.set_llm_models(rows);
        let mut st = SeatsState::default();
        assert_eq!(perform(&mut st, "b3.model.choose", 0, &s), Outcome::Done, "already selected");
        assert_eq!(perform(&mut st, "b3.model.choose", 2, &s), Outcome::Done, "unavailable");
        assert_eq!(perform(&mut st, "b3.model.choose", 1, &s), Outcome::Spawn(Job::ModelSelect(1)));
        st.turn_busy = true;
        assert_eq!(perform(&mut st, "b3.model.choose", 1, &s), Outcome::Spawn(Job::ModelSelect(1)), "a running turn does not block selection for the next turn");
        assert_eq!(model_seat_label(&s), "GLM-5.2");
    }

    /// Runtime/default mismatches can be normal next-turn transitions.
    #[test]
    fn restart_requires_an_explicit_server_hint() {
        let glm = ProfileLlmModel {
            model: "glm-5.2".into(),
            provider: "zai".into(),
            title: "GLM 5.2".into(),
            family: None,
            route: None,
            selected: true,
            available: true,
        };
        let models = vec![glm];
        assert!(!profile_default_needs_restart(Some(("deepseek-v4", "deepseek")), &models, false), "an old turn or cached status does not require a restart");
        assert!(profile_default_needs_restart(Some(("glm-5.2", "zai")), &models, true), "equal strings never disprove the hint");
        assert!(profile_default_needs_restart(None, &models, true), "no runtime: the hint decides");
        assert!(!profile_default_needs_restart(Some(("glm-5.2", "zai")), &models, false), "the runtime IS the default");
        assert!(!profile_default_needs_restart(None, &models, false));
        assert!(!profile_default_needs_restart(Some(("x", "y")), &[], false), "no Profile default: the hint decides");
        assert_eq!(
            restart_notice(Some("GLM 5.2"), Some("deepseek-v4")),
            "Profile default is GLM 5.2. This Octos process is still serving deepseek-v4. Restart Octos to apply the new default."
        );
        assert_eq!(
            restart_notice(None, None),
            "Profile default is saved. This Octos process is still serving its current runtime model. Restart Octos to apply the new default."
        );
        // The hint a selection answer leaves.
        assert!(answer_needs_restart(Some(&json!({"applied": true, "restart_required": true}))));
        assert!(answer_needs_restart(Some(&json!({"applied": true, "runtime_disposition": "restart_required"}))));
        assert!(!answer_needs_restart(Some(&json!({"applied": true, "restart_required": false, "runtime_disposition": "reloaded"}))));
        assert!(!answer_needs_restart(Some(&json!({"applied": false}))), "a refusal puts it out");
        assert!(!answer_needs_restart(None), "a failed request puts it out");
    }

    #[test]
    fn the_menus_lower_balanced() {
        let s = store();
        let st = SeatsState::default();
        for frame in [Frame::DESKTOP, Frame { avail_w: 360.0, avail_h: 776.0 }] {
            let mut d = Dsl::new();
            build_permission(&mut d, &st, &frame, &s);
            let dsl = d.finish();
            assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
            assert!(dsl.contains("Full access · Network allowed"));
            let mut d = Dsl::new();
            build_models(&mut d, &st, &frame, &s);
            let dsl = d.finish();
            assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
            let mut st2 = SeatsState::default();
            st2.pending = permission_options(&s).pop();
            let mut d = Dsl::new();
            build_permission(&mut d, &st2, &frame, &s);
            let dsl = d.finish();
            assert!(dsl.contains(RISK_TITLE) && dsl.contains(RISK_HINT));
            let taps: Vec<String> = crate::screens::taps::wired_taps(&dsl).into_iter().map(|(_, e)| e).collect();
            assert!(!taps.iter().any(|t| t == "b3.perm.confirm"), "inert until ticked: {taps:?}");
        }
    }
}
