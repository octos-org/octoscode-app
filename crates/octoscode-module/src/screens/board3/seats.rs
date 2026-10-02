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

/// The model seat's label (`ModelControl` trigger): the selected model's
/// name (title, else id), else the select prompt.
pub fn model_seat_label(store: &Store) -> String {
    store
        .domains
        .profile
        .llm_models()
        .into_iter()
        .find(|m| m.selected)
        .map(|m| if m.title.is_empty() { m.model } else { m.title })
        .unwrap_or_else(|| MODEL_SELECT.to_owned())
}

/// The permission seat's label (`PermissionControl` trigger).
pub fn permission_seat_label(store: &Store) -> Option<String> {
    permission_options(store).into_iter().next().map(|o| o.name())
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

pub fn permission_locked(st: &SeatsState, store: &Store) -> bool {
    st.turn_busy || st.perm_busy || !has(store, "permission/profile/set")
}

pub fn model_locked(st: &SeatsState, store: &Store) -> bool {
    st.turn_busy || st.models_busy || !has(store, "profile/llm/select")
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
                PermIntent::None => Outcome::Done,
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

fn model_from(v: &Value) -> Option<ProfileLlmModel> {
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    Some(ProfileLlmModel {
        model: text("model").filter(|s| !s.is_empty())?,
        provider: text("provider").filter(|s| !s.is_empty())?,
        title: text("title").unwrap_or_default(),
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
        if st.models_busy || st.turn_busy || !target.available {
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
    d.view("b3_root", "width: Fill height: Fill flow: Overlay");
    d.view("b3_backdrop_box", "width: Fill height: Fill flow: Overlay");
    d.tap("b3_backdrop_hit", "b3.close");
    d.close();
    // Above the seat (`bottom: calc(100% + 8px)`), left- or right-aligned to
    // it; without a measured seat, above the composer's bottom-left/right.
    let a = anchor.unwrap_or(Anchor { x: 16.0, y: frame.avail_h - 96.0, w: 120.0, h: 30.0 });
    let bottom = (frame.avail_h - a.y + 8.0).max(8.0).floor();
    let (align_x, left, right_pad) = if right {
        (1.0, 16.0, (frame.avail_w - (a.x + a.w)).max(16.0).floor())
    } else {
        (0.0, a.x.max(16.0).floor(), 16.0)
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
        12.0,
        Some(tok::HAIRLINE),
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

fn status_line(d: &mut Dsl, id: &str, text: &str) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit padding: Inset{left: 10 right: 10 top: 10 bottom: 10}");
    d.text(id, text, &Txt::new(13.0, Face::Regular, tok::FAINT).w(W::Fill).wrap());
    d.close();
}

fn error_line(d: &mut Dsl, id: &str, message: &str, retry: &str) {
    d.surface(
        id,
        "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 0.0 y: 0.0} padding: Inset{left: 8 right: 8 top: 7 bottom: 7} margin: Inset{bottom: 4}",
        tok::RED_BG,
        8.0,
        None,
    );
    d.text(&format!("{id}_text"), message, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
    let b = d.anon();
    d.view(&b, "width: Fit height: 28 flow: Overlay align: Align{x: 0.5 y: 0.5} padding: Inset{left: 4 right: 4}");
    d.text(&format!("{id}_retry_label"), "Retry", &Txt::new(12.0, Face::Semibold, tok::RED));
    d.tap(&format!("{id}_retry"), retry);
    d.close();
    d.close();
}

/// One menu option (`.option`: min-height 42, 6/8 padding, radius 10, the
/// icon / copy / check columns).
#[allow(clippy::too_many_arguments)]
fn option_row(
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
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 0.0 y: 0.5} padding: Inset{left: 8 right: 8 top: 6 bottom: 6}");
    if let Some(file) = icon {
        let ic = d.anon();
        d.view(&ic, "width: 18 height: 30 flow: Overlay align: Align{x: 0.5 y: 0.5}");
        d.icon(&format!("{id}_icon"), file, 16.0, tok::MUTED);
        d.close();
    }
    let copy = d.anon();
    d.view(&copy, "width: Fill height: Fit flow: Down spacing: 1");
    let ink = if enabled { tok::TEXT } else { tok::FAINT };
    d.text(&format!("{id}_name"), &ui::fit_w(name, copy_w, 14.0, Face::Medium), &Txt::new(14.0, Face::Medium, ink).w(W::Fill));
    if let Some(desc) = description {
        d.text(&format!("{id}_desc"), &ui::fit_w(desc, copy_w, 12.0, Face::Regular), &Txt::new(12.0, Face::Regular, tok::FAINT).w(W::Fill));
    }
    d.close();
    let ck = d.anon();
    d.view(&ck, "width: 18 height: 30 flow: Overlay align: Align{x: 0.5 y: 0.5}");
    if checked {
        d.icon(&format!("{id}_check"), "b3_check.svg", 16.0, tok::TEXT);
    }
    d.close();
    d.close();
    if enabled {
        d.tap(id, event);
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
        .map(|o| ui::text_w(&o.name(), 14.0, Face::Medium) + 18.0 + 18.0 + 16.0 + 16.0 + 8.0 + 24.0)
        .fold(260.0_f64, f64::max);
    let width = menu_width(frame, natural);
    let copy_w = width - 8.0 - 16.0 - 18.0 - 18.0 - 16.0;
    popover_open(d, frame, st.perm_anchor, false, width);
    d.text("b3_title", PERMISSION_MENU, &Txt::new(12.0, Face::Medium, tok::FAINT).w(W::Fill));
    let locked = permission_locked(st, store);
    let available = permission_seat(store);
    if !available {
        status_line(d, "b3_perm_unavailable", PERMISSION_UNAVAILABLE);
    } else if st.perm_loading && options.is_empty() {
        status_line(d, "b3_perm_loading", PERMISSION_LOADING);
    }
    if let Some(e) = &st.perm_error {
        error_line(d, "b3_perm_error", e, "b3.perm.retry");
    }
    let selected = options.first().map(|o| o.id.clone());
    for (i, o) in options.iter().enumerate() {
        let icon = if o.dangerous { "b3_shield_danger.svg" } else { "b3_shield.svg" };
        let enabled = !locked && available && !(st.perm_loading && options.is_empty());
        option_row(
            d,
            &format!("b3_perm_opt_{i}"),
            Some(icon),
            &o.name(),
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
    d.text("b3_title", RISK_TITLE, &Txt::new(18.0, Face::Semibold, tok::TEXT).w(W::Fill));
    d.close();
    d.gap(W::Fill, 10.0);
    d.text("b3_risk_description", RISK_DESCRIPTION, &Txt::new(14.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
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
        d.text(&format!("{id}_k"), k, &Txt::new(13.0, Face::Regular, tok::FAINT).w(W::Px(118.0)));
        d.text(&format!("{id}_v"), v, &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Fill));
        d.close();
    }
    d.close();
    d.gap(W::Fill, 16.0);
    // The acknowledgement (`<input type=checkbox>` + label): one press target.
    d.view("b3_risk_ack_box", "width: Fill height: Fit flow: Overlay");
    let ack = d.anon();
    d.view(&ack, "width: Fill height: Fit flow: Right spacing: 10 align: Align{x: 0.0 y: 0.0} padding: Inset{top: 4 bottom: 4}");
    d.icon("b3_risk_ack_icon", if st.acknowledged { "b3_box_on.svg" } else { "b3_box_off.svg" }, 18.0, tok::RED);
    d.text("b3_risk_ack_label", RISK_ACK, &Txt::new(13.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.close();
    if !locked {
        d.tap("b3_risk_ack", "b3.perm.ack");
    }
    d.close();
    if !st.acknowledged {
        d.gap(W::Fill, 8.0);
        d.text("b3_risk_hint", RISK_HINT, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    d.gap(W::Fill, 20.0);
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 1.0 y: 0.5}");
    d.button("b3_risk_cancel", "Cancel", "b3.perm.cancel", Btn::Outline, W::Fit, 36.0);
    danger_button(d, "b3_risk_confirm", RISK_CONFIRM, "b3.perm.confirm", st.acknowledged && !locked);
    d.close();
    ui::shell_close(d);
}

/// `.dangerAction`: the error fill, or — inert — the neutral fill (an inert
/// danger action must not read as armed).
fn danger_button(d: &mut Dsl, id: &str, label: &str, event: &str, armed: bool) {
    let w = (ui::text_w(label, 14.0, Face::Medium) + 32.0).ceil();
    let (fill, ink) = if armed { (tok::RED, tok::WHITE) } else { (tok::DISABLED_BG, tok::FAINT) };
    d.surface(
        &format!("{id}_box"),
        &format!("width: {w} height: 36 flow: Overlay align: Align{{x: 0.5 y: 0.5}}"),
        fill,
        18.0,
        None,
    );
    d.text(&format!("{id}_label"), label, &Txt::new(14.0, Face::Medium, ink));
    if armed {
        d.tap(id, event);
    }
    d.close();
}

/// The model menu (`ModelMenu`): the catalog by provider; unusable entries
/// stay visible, disabled, with their reason; the selected one checked;
/// under it the Session's notice board (Saving…, an external change, the
/// latest disposition notice).
pub fn build_models(d: &mut Dsl, st: &SeatsState, frame: &Frame, store: &Store) {
    let models = store.domains.profile.llm_models();
    let groups = model_groups(&models);
    let natural = models
        .iter()
        .map(|m| ui::text_w(if m.title.is_empty() { &m.model } else { &m.title }, 14.0, Face::Medium) + 18.0 + 16.0 + 16.0 + 8.0 + 24.0)
        .fold(260.0_f64, f64::max);
    let width = menu_width(frame, natural);
    let copy_w = width - 8.0 - 16.0 - 18.0 - 8.0;
    popover_open(d, frame, st.model_anchor, true, width);
    d.text("b3_title", MODEL_MENU, &Txt::new(12.0, Face::Medium, tok::FAINT).w(W::Fill));
    let available = model_seat(store);
    let locked = model_locked(st, store);
    if !available {
        status_line(d, "b3_model_unavailable", MODEL_UNAVAILABLE);
    } else if st.models_loading && models.is_empty() {
        status_line(d, "b3_model_loading", MODEL_LOADING);
    }
    if let Some(e) = &st.models_error {
        error_line(d, "b3_model_error", e, "b3.model.retry");
    }
    let body_h = (frame.avail_h - 32.0 - 180.0).clamp(160.0, 380.0);
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
        for (i, m) in rows {
            let name = if m.title.is_empty() { m.model.clone() } else { m.title.clone() };
            let description = if !m.available {
                Some(MODEL_UNAVAILABLE_REASON.to_owned())
            } else if !m.title.is_empty() && m.title != m.model {
                Some(m.model.clone())
            } else {
                None
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
            d.text("b3_model_saving", "Saving…", &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill));
        }
        if board.external_change {
            d.text("b3_model_external", EXTERNAL_CHANGE, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        }
        if let Some(n) = board.latest() {
            let ink = if n.kind == Disposition::Refused { tok::RED } else { tok::TEXT };
            d.text("b3_model_notice", &n.message, &Txt::new(12.5, Face::Regular, ink).w(W::Fill).wrap());
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
        assert_eq!(perform(&mut st, "b3.perm.choose", 1, &s), Outcome::Done, "locked while a turn runs");
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
        assert_eq!(model_seat_label(&s), "GLM-5.2");
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
