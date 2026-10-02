//! A8 — the SESSION SETTINGS pane (session-config rows), the native form of
//! the web's `features/session-config/SessionConfigPane.tsx` (+ its sections
//! `model-section.tsx`, `permissions-section.tsx`, `sandbox-section.tsx`, the
//! Show thinking row `SessionConfigPane.tsx:340-368`, the holder banners
//! `:217-255` and the Advanced body `App.tsx:3307-3350`).
//!
//! It opens from the session strip above the composer (`App.tsx:3139-3156`:
//! the strip's click opens THIS pane, not app Settings) and from the palette's
//! "Session settings" command. No approved board draws the pane, so it is
//! built in the board-3 dialog style (`board3/ui.rs`: dimmed backdrop, centred
//! white card, 20 px padding, a phone sheet below 520 px) with the web's
//! section order and copy:
//!
//! 1. holder banner — "Another app is using this session" + Resume chat, or
//!    the own-hold copy "A peer you started is using this session";
//! 2. Model — the profile's SAVED model vs the session RUNTIME model, the
//!    running response's model only while a turn runs, the configured models
//!    as a picker (`profile/llm/select`, each `runtime_disposition` mapped to
//!    its exact message, `SessionConfigPane.tsx:52-90`);
//! 3. Permissions — the next-message timing disclosure, the server's presets
//!    (`permission/profile/list`; Full access asks first), the approval-policy
//!    readback from `session/status/read` (+ "not verified (as set here)"),
//!    the On request / Never ask selector, Saving / Saved / Failed + Retry;
//! 4. Sandbox — the effective sandbox read-only with the fixed-at-open note,
//!    or "Not supported by this server" (feature `session.sandbox.v1`);
//! 5. Show thinking — the persisted preference;
//! 6. Advanced (collapsed by default, remembered) — who controls the session
//!    and the external-driver disclosure (`screens::driver_discovery`), then
//!    the web's `advancedChildren` (A10): the control seat and the peer
//!    controller console (`fleet_console::session_controls` — Acquire seat /
//!    Release seat, the four seat commands, Dispatch), gated on the record's
//!    control readiness (`fleet_driver::control_ready`).
use serde_json::{json, Value};

use octoscode_store::Store;

use super::host::{Job, Outcome};
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Seg, Txt, W};
use crate::screens::driver_discovery::{self as dd, Inventory};

pub const STATUS_METHOD: &str = "session/status/read";
pub const PERM_LIST: &str = "permission/profile/list";
pub const PERM_SET: &str = "permission/profile/set";
pub const LLM_LIST: &str = "profile/llm/list";
pub const LLM_SELECT: &str = "profile/llm/select";

/// The facts the pane reads from `session/status/read`
/// (`session-status-result.ts:5-60`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusFacts {
    /// `model.title ?? model.model` (`App.tsx:2000`).
    pub model: Option<String>,
    /// `approval_policy` — top level, else the runtime policy stamp's
    /// (`permissions-section.tsx:2-8`: "the session-addressed
    /// `session/status/read` runtime stamp").
    pub approval_policy: Option<String>,
    /// `sandbox ?? sandbox_mode` (`App.tsx:3238-3250`).
    pub sandbox: Option<String>,
    /// The stamp's `network` ("allowed" / "blocked").
    pub network: Option<String>,
    pub read_paths: Option<Vec<String>>,
}

/// Parse a status read for `session` (the identity check,
/// `session-status-result.ts:6`): another session's reply is `None`.
pub fn parse_status(v: &Value, session: &str) -> Option<StatusFacts> {
    if v.get("session_id")?.as_str()? != session {
        return None;
    }
    let s = |o: &Value, k: &str| o.get(k).and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(str::to_owned);
    let stamp = v.get("runtime_policy_stamp").cloned().unwrap_or(Value::Null);
    let model = v.get("model").and_then(|m| s(m, "title").or_else(|| s(m, "model")));
    Some(StatusFacts {
        model,
        approval_policy: s(v, "approval_policy").or_else(|| s(&stamp, "approval_policy")),
        sandbox: s(v, "sandbox").or_else(|| s(v, "sandbox_mode")).or_else(|| s(&stamp, "sandbox_mode")),
        network: s(v, "network").or_else(|| s(&stamp, "network")),
        read_paths: v
            .get("read_allow_paths")
            .or_else(|| stamp.get("read_allow_paths"))
            .and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|p| p.as_str().map(str::to_owned)).collect()),
    })
}

/// One configured model (`profile/llm/list`): the web's flat `models` rows,
/// or the live server's `primary` + `fallbacks` (r2-profile line 12). The
/// route keeps its wire ID (what `profile/llm/select` needs) and its label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRow {
    pub title: String,
    pub model_id: String,
    pub family_id: String,
    pub route_id: Option<String>,
    pub route_label: Option<String>,
    pub selected: bool,
    pub available: bool,
}

fn model_row(v: &Value, selected_default: bool) -> Option<ModelRow> {
    let str_of = |k: &str| v.get(k).and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(str::to_owned);
    let model_id = str_of("model_id").or_else(|| str_of("model"))?;
    let family_id = str_of("family_id").or_else(|| str_of("family")).or_else(|| str_of("provider"))?;
    let route = v.get("route");
    let route_id = str_of("route_id").or_else(|| route.and_then(|r| r.get("route_id")).and_then(|x| x.as_str()).map(str::to_owned));
    let route_label = route
        .and_then(|r| r.get("label").and_then(|x| x.as_str()).or_else(|| r.as_str()))
        .map(str::to_owned);
    Some(ModelRow {
        title: str_of("title").unwrap_or_else(|| model_id.clone()),
        model_id,
        family_id,
        route_id,
        route_label,
        selected: v.get("selected").and_then(|x| x.as_bool()).unwrap_or(selected_default),
        available: v.get("available").and_then(|x| x.as_bool()).unwrap_or(true),
    })
}

/// `profile/llm/list` -> the picker rows (primary first).
pub fn parse_models(v: &Value) -> Vec<ModelRow> {
    if let Some(rows) = v.get("models").and_then(|m| m.as_array()) {
        return rows.iter().filter_map(|r| model_row(r, false)).collect();
    }
    let llm = v.get("llm").unwrap_or(v);
    let mut out: Vec<ModelRow> = Vec::new();
    for (val, sel) in std::iter::once((llm.get("primary"), true))
        .chain(llm.get("fallbacks").and_then(|f| f.as_array()).into_iter().flatten().map(|f| (Some(f), false)))
    {
        if let Some(row) = val.and_then(|p| model_row(p, sel)) {
            if !out.iter().any(|o| o.model_id == row.model_id && o.route_id == row.route_id) {
                out.push(row);
            }
        }
    }
    out
}

/// The six `runtime_disposition`s (+ the legacy `persisted`) mapped to the
/// web's exact message (`dispositionNotice`, `SessionConfigPane.tsx:52-90`;
/// `parseRuntimeDisposition`, `model-notices.ts:38-66`). `running` is the
/// model the session keeps serving (the pane's runtime model).
pub fn disposition_notice(result: &Value, saved: &str, running: Option<&str>) -> String {
    let text = |k: &str| result.get(k).and_then(|x| x.as_str()).filter(|x| !x.is_empty());
    match result.get("runtime_disposition").and_then(|d| d.as_str()) {
        None if result.get("applied").and_then(|a| a.as_bool()) == Some(true) => "Saved".into(),
        None => "Couldn't save: the server refused the change".into(),
        Some("reloaded") => format!("Saved. Your next message uses {saved}"),
        Some("deferred") => match text("condition") {
            Some(c) => format!("Saved. The model is not active yet ({c})"),
            None => "Saved. The model is not active yet".into(),
        },
        Some("restart_required") => format!(
            "Saved. The server keeps running {} until it restarts",
            running
                .or_else(|| result.pointer("/runtime_policy_stamp/model").and_then(|m| m.as_str()))
                .unwrap_or("the previous model")
        ),
        Some("persisted_but_not_live") => format!(
            "Saved, but not usable right now: {}",
            text("runtime_error").unwrap_or("the runtime could not start")
        ),
        Some("unchanged") => "Already selected".into(),
        Some(_) => format!("Couldn't save: {}", text("reason").unwrap_or("the server refused the change")),
    }
}

/// One permission preset (`permissionOptions`, `permission-projection.ts:31-50`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermOption {
    pub mode: String,
    pub network: String,
}

impl PermOption {
    pub fn id(&self) -> String {
        format!("{}:{}", self.mode, self.network)
    }
    /// `permissionName` (`SessionControlBar.tsx:254-256`).
    pub fn label(&self) -> String {
        let mode = match self.mode.as_str() {
            "read_only" => "Read",
            "workspace_write" => "Write",
            _ => "Full access",
        };
        let net = if self.network == "allow" { "Network allowed" } else { "Network blocked" };
        format!("{mode} · {net}")
    }
    pub fn dangerous(&self) -> bool {
        self.mode == "danger_full_access"
    }
}

fn wire_mode(m: octoscode_store::domains::profile::PermissionProfileMode) -> &'static str {
    use octoscode_store::domains::profile::PermissionProfileMode as M;
    match m {
        M::ReadOnly => "read_only",
        M::WorkspaceWrite => "workspace_write",
        M::DangerFullAccess => "danger_full_access",
    }
}

fn wire_net(n: octoscode_store::domains::profile::PermissionNetworkPolicy) -> &'static str {
    use octoscode_store::domains::profile::PermissionNetworkPolicy as N;
    match n {
        N::Allow => "allow",
        N::Deny => "deny",
    }
}

/// The presets the server offers, in ITS order, with the current selection
/// prepended only when it is not one of them (the web's `[current,
/// ...profiles]`, `permission-projection.ts:31-50`, kept stable for a radio
/// list: a pick must not reshuffle the rows under the pointer).
pub fn perm_options(store: &Store) -> Vec<PermOption> {
    let to = |sel: octoscode_store::domains::profile::PermissionProfileSelection| PermOption {
        mode: wire_mode(sel.mode).into(),
        network: wire_net(sel.network).into(),
    };
    let mut out: Vec<PermOption> = Vec::new();
    for o in store.domains.profile.permission_profiles().into_iter().map(to) {
        if !out.contains(&o) {
            out.push(o);
        }
    }
    if let Some(cur) = store.domains.profile.permission().map(to) {
        if !out.contains(&cur) {
            out.insert(0, cur);
        }
    }
    out
}

/// The current selection's option id.
pub fn perm_selected(store: &Store) -> Option<String> {
    store
        .domains
        .profile
        .permission()
        .map(|s| format!("{}:{}", wire_mode(s.mode), wire_net(s.network)))
}

/// The Permissions section's save feedback (`PermissionSaveState`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveState {
    Saving,
    Saved,
    Failed(String),
}

/// What Retry re-sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermIntent {
    Preset(PermOption),
    Approval(String),
}

impl PermIntent {
    pub fn params(&self, session: &str) -> Value {
        match self {
            PermIntent::Preset(o) => json!({"session_id": session, "update": {"mode": o.mode, "network": o.network}}),
            PermIntent::Approval(p) => json!({"session_id": session, "update": {"approval_policy": p}}),
        }
    }
}

/// The pane's UI state (the values the protocol never carries).
#[derive(Debug, Clone, Default)]
pub struct PaneState {
    /// The session the pane was opened for (the replies are checked against it).
    pub session: String,
    pub loading: bool,
    pub ticket: u64,
    pub status: Option<StatusFacts>,
    pub models: Vec<ModelRow>,
    /// The selection the last list showed (case 23's comparison).
    pub last_seen_model: Option<String>,
    pub external_change: bool,
    pub model_saving: bool,
    pub model_notice: Option<String>,
    pub perm_save: Option<SaveState>,
    pub perm_retry: Option<PermIntent>,
    /// The approval policy THIS client last set (the readback when the
    /// server's stamp does not carry one: "not verified (as set here)").
    pub approval_set_here: Option<String>,
    /// A Full access preset waiting for its risk confirmation.
    pub confirm: Option<PermOption>,
    pub risk_ack: bool,
    pub advanced_open: bool,
    pub driver: Inventory,
    pub resume_busy: bool,
    pub resume_notice: Option<String>,
}

/// The Advanced section's remembered open state (`advancedOpen`, "collapsed
/// by default, remembered per browser"): a file beside the show-thinking
/// preference, `OCTOSCODE_PANE_ADVANCED_FILE` overriding it for tests.
fn advanced_pref_path() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("OCTOSCODE_PANE_ADVANCED_FILE") {
        return Some(p.into());
    }
    std::env::var("HOME").ok().map(|h| std::path::Path::new(&h).join(".octoscode/session-pane-advanced.json"))
}

pub fn load_advanced_pref() -> bool {
    advanced_pref_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|s| s.trim() == "true")
}

fn save_advanced_pref(open: bool) {
    if let Some(p) = advanced_pref_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, if open { "true" } else { "false" });
    }
}

/// Opening: reset the transient feedback and ask for the reads (the load
/// binds the pane to the session it reads).
pub fn on_open(st: &mut PaneState) -> Outcome {
    st.loading = true;
    st.confirm = None;
    st.risk_ack = false;
    st.perm_save = None;
    st.resume_notice = None;
    st.external_change = false;
    st.advanced_open = load_advanced_pref();
    if st.driver == Inventory::Unavailable || matches!(st.driver, Inventory::Error(_)) {
        st.driver = Inventory::Loading;
    }
    Outcome::Spawn(Job::PaneLoad)
}

fn advertised(store: &Store, method: &str) -> bool {
    store.domains.config.supported_methods().iter().any(|m| m == method)
}

/// The holder the strip and the banner name (A3's foreign-holder read,
/// `chrome::held_by_other`, or the pane's own walk).
pub fn foreign_held(st: &PaneState, store: &Store) -> bool {
    crate::chrome::held_by_other(store).is_some()
        || st
            .driver
            .disclosure()
            .and_then(|d| (d.mode == dd::Mode::External).then_some(d.binding.as_ref()))
            .flatten()
            .is_some_and(|b| b.driver_id != crate::chrome::native_driver_id())
}

/// Our own peer holds the seat (`seatHolderKind` SELF, `seat-holder.ts:18-29`).
pub fn own_held(st: &PaneState) -> bool {
    st.driver
        .disclosure()
        .and_then(|d| (d.mode == dd::Mode::External).then_some(d.binding.as_ref()))
        .flatten()
        .is_some_and(|b| b.driver_id == crate::chrome::native_driver_id())
}

/// A10 — after a seat change re-walked the store's inventory
/// (`fleet_driver::load_inventory`), the pane's disclosure follows it (the
/// web's ONE `driverInventory` per record) without a second walk.
pub fn mirror_inventory(store: &Store) {
    use octoscode_store::domains::peer::FleetInventory;
    let inv = store.domains.peer.inventory();
    let mut st = super::host::state();
    let bound = if st.pane.session.is_empty() { store.active_session().unwrap_or_default() } else { st.pane.session.clone() };
    let next = match inv {
        Some(FleetInventory::Complete { session_id, snapshot, observed_revision, operations, disclosure, .. }) if session_id == bound => {
            let mode = if disclosure.mode == "external" { dd::Mode::External } else { dd::Mode::Internal };
            let recovery = match disclosure.recovery.as_str() {
                "interrupted" => dd::Recovery::Interrupted,
                "recovery_required" => dd::Recovery::RecoveryRequired,
                _ => dd::Recovery::None,
            };
            Inventory::Complete {
                snapshot,
                observed_revision,
                operations: operations
                    .into_iter()
                    .map(|o| dd::Operation {
                        operation_id: o.operation_id,
                        slug: o.slug,
                        lifecycle: o.lifecycle,
                        adopted_session_id: o.adopted_session_id,
                        model: o.model,
                        model_lane: o.model_lane,
                        accepted_at_ms: o.accepted_at_ms,
                    })
                    .collect(),
                disclosure: dd::Disclosure {
                    mode,
                    recovery,
                    binding: disclosure.binding.map(|(driver_id, epoch, revision, lease_expires_at_ms)| dd::Binding {
                        driver_id,
                        epoch,
                        revision,
                        lease_expires_at_ms,
                    }),
                },
            }
        }
        Some(FleetInventory::Error { session_id, reason }) if session_id == bound => {
            match dd::REFUSAL_KINDS.iter().find(|k| reason.ends_with(*k)) {
                Some(k) => Inventory::Error(dd::ErrorReason::Refused(*k)),
                None => Inventory::Error(dd::ErrorReason::Unknown),
            }
        }
        _ => return,
    };
    st.pane.driver = next;
}

/// Route one `b3.sc.*` action.
pub fn perform(st: &mut PaneState, action: &str, index: usize, store: &Store) -> Outcome {
    match action {
        "b3.sc.model" => {
            if st.model_saving {
                return Outcome::Done;
            }
            match st.models.get(index) {
                Some(m) if m.available => {
                    st.model_saving = true;
                    st.model_notice = None;
                    Outcome::Spawn(Job::PaneModel(index))
                }
                _ => Outcome::Done,
            }
        }
        "b3.sc.perm" => {
            if matches!(st.perm_save, Some(SaveState::Saving)) {
                return Outcome::Done;
            }
            let opts = perm_options(store);
            let Some(o) = opts.get(index).cloned() else { return Outcome::Done };
            if perm_selected(store).as_deref() == Some(o.id().as_str()) {
                return Outcome::Done; // already the current preset: nothing to send
            }
            if o.dangerous() {
                // `permissionSelectionIntent`: Full access confirms first.
                st.confirm = Some(o);
                st.risk_ack = false;
                return Outcome::Done;
            }
            st.perm_save = Some(SaveState::Saving);
            st.perm_retry = Some(PermIntent::Preset(o.clone()));
            Outcome::Spawn(Job::PanePerm(PermIntent::Preset(o)))
        }
        "b3.sc.risk.ack" => {
            st.risk_ack = !st.risk_ack;
            Outcome::Done
        }
        "b3.sc.risk.cancel" => {
            st.confirm = None;
            st.risk_ack = false;
            Outcome::Done
        }
        "b3.sc.risk.confirm" => {
            // The confirm button is disabled until the box is ticked; a stray
            // route without the acknowledgement sends nothing (fail closed).
            let (Some(o), true) = (st.confirm.clone(), st.risk_ack) else { return Outcome::Done };
            st.confirm = None;
            st.risk_ack = false;
            st.perm_save = Some(SaveState::Saving);
            st.perm_retry = Some(PermIntent::Preset(o.clone()));
            Outcome::Spawn(Job::PanePerm(PermIntent::Preset(o)))
        }
        "b3.sc.approval.on-request" | "b3.sc.approval.never" => {
            if matches!(st.perm_save, Some(SaveState::Saving)) {
                return Outcome::Done;
            }
            let policy = action.trim_start_matches("b3.sc.approval.").to_owned();
            let intent = PermIntent::Approval(policy);
            st.perm_save = Some(SaveState::Saving);
            st.perm_retry = Some(intent.clone());
            Outcome::Spawn(Job::PanePerm(intent))
        }
        "b3.sc.perm.retry" => match st.perm_retry.clone() {
            Some(intent) if !matches!(st.perm_save, Some(SaveState::Saving)) => {
                st.perm_save = Some(SaveState::Saving);
                Outcome::Spawn(Job::PanePerm(intent))
            }
            _ => Outcome::Done,
        },
        "b3.sc.thinking" => {
            // `onShowThinkingChange` (`App.tsx:3302-3306`): the Session's
            // visibility AND the persisted preference.
            let session = store.active_session().unwrap_or_default();
            let next = !store.domains.session.thinking(&session).show_reasoning;
            store.domains.session.set_show_reasoning(&session, next);
            store.domains.session.set_thinking_default_on(&session, next);
            super::thinking::save_pref(next);
            Outcome::Done
        }
        "b3.sc.advanced" => {
            st.advanced_open = !st.advanced_open;
            save_advanced_pref(st.advanced_open);
            Outcome::Done
        }
        // "New session with…" (`App.tsx:3250-3253`): the new-session
        // defaults live in Settings (here: Settings > Sandbox).
        "b3.sc.newsession" => Outcome::Action("settings.defaults.open".into()),
        "b3.sc.resume_chat" => {
            if st.resume_busy || !foreign_held(st, store) {
                return Outcome::Done;
            }
            st.resume_busy = true;
            st.resume_notice = None;
            Outcome::Spawn(Job::PaneResumeChat)
        }
        _ => Outcome::Unrouted,
    }
}

// --------------------------------------------------------------- transport

/// The pane's reads: the status stamp, the permission presets, the model
/// list and the driver disclosure — each through the production client,
/// each advertised-gated, each checked against the session it was asked for.
pub async fn load(conv: &crate::flow::Conversation) -> Result<String, String> {
    let store = &conv.store;
    let session = conv.session_id();
    // The runtime scope the reads belong to (`session-scope.ts`): a reply
    // that lands after the endpoint, workspace, profile, session or
    // authority epoch moved is dropped, never folded into the pane.
    let scope = conv.scope_key();
    let ticket = {
        let mut st = super::host::state();
        st.pane.ticket += 1;
        st.pane.session = session.clone();
        st.pane.loading = true;
        st.pane.ticket
    };
    let mut notes = Vec::new();
    let status = if advertised(store, STATUS_METHOD) {
        match conv
            .client()
            .request(STATUS_METHOD, json!({"session_id": session, "profile_id": conv.profile()}))
            .await
        {
            Ok(v) => parse_status(&v, &session),
            Err(e) => {
                notes.push(format!("{STATUS_METHOD}: {e}"));
                None
            }
        }
    } else {
        None
    };
    if advertised(store, PERM_LIST) {
        use octoscode_client::domains::profile::PermissionProfileList;
        match conv
            .client()
            .call::<PermissionProfileList>(octos_core::ui_protocol::PermissionProfileListParams {
                session_id: octos_core::SessionKey(session.clone()),
            })
            .await
        {
            Ok(r) if r.session_id.0 == session => fold_permission(store, &serde_json::to_value(&r).unwrap_or(Value::Null)),
            Ok(_) => notes.push(format!("{PERM_LIST}: another session's reply dropped")),
            Err(e) => notes.push(format!("{PERM_LIST}: {e}")),
        }
    }
    let models = if advertised(store, LLM_LIST) {
        match conv
            .client()
            .request(LLM_LIST, json!({"session_id": session, "profile_id": conv.profile()}))
            .await
        {
            Ok(v) => Some(parse_models(&v)),
            Err(e) => {
                notes.push(format!("{LLM_LIST}: {e}"));
                None
            }
        }
    } else {
        None
    };
    let driver = dd::walk(conv).await;
    // A10 — the Advanced children's facts (the web reads them per record):
    // the walked inventory the seat's readiness and CAS revision come from,
    // and the profile's lanes for the console's picker. Only when the server
    // advertises `peer/control` (+ `external_driver_v1`): otherwise there is
    // no seat and nothing is read.
    if crate::screens::fleet_driver::peer_control_admitted(store) {
        let _ = crate::screens::fleet_driver::load_inventory(conv).await;
        if crate::screens::fleet_driver::control_advertised(store) {
            let _ = super::fleetview::load_lanes(conv).await;
        }
    }
    let mut st = super::host::state();
    if st.pane.ticket != ticket || conv.scope_key() != scope {
        return Ok("stale pane read dropped".into());
    }
    let p = &mut st.pane;
    p.loading = false;
    p.status = status;
    if let Some(models) = models {
        // Case 23 (`use-model-selection.ts:162-190`): a refresh that shows a
        // different selection than the last one seen, which this pane did
        // not make, was changed by another tab or app.
        let now = models.iter().find(|m| m.selected).map(|m| m.model_id.clone());
        p.external_change = matches!((&p.last_seen_model, &now), (Some(a), Some(b)) if a != b);
        if now.is_some() {
            p.last_seen_model = now;
        }
        p.models = models;
    }
    p.driver = driver;
    Ok(format!(
        "status={} models={} perms={} driver={} {}",
        p.status.is_some(),
        p.models.len(),
        perm_options(store).len(),
        dd::controller_label(&p.driver),
        notes.join("; ")
    ))
}

/// A `permission/profile/list|set` reply -> the store's selection.
fn fold_permission(store: &Store, v: &Value) {
    use octoscode_store::domains::profile::PermissionProfileSelection as Sel;
    let sel = |x: &Value| serde_json::from_value::<Sel>(x.clone()).ok();
    let Some(current) = v.get("current").and_then(sel) else { return };
    let profiles: Vec<Sel> = v
        .get("profiles")
        .and_then(|p| p.as_array())
        .map(|a| a.iter().filter_map(sel).collect())
        .unwrap_or_default();
    if profiles.is_empty() {
        store.domains.profile.set_permission_current(current);
    } else {
        store.domains.profile.set_permission(current, profiles);
    }
}

/// `profile/llm/select` for picker row `index`, then the disposition notice.
pub async fn select_model(conv: &crate::flow::Conversation, index: usize) -> Result<String, String> {
    let session = conv.session_id();
    let (row, running) = {
        let st = super::host::state();
        (st.pane.models.get(index).cloned(), st.pane.status.as_ref().and_then(|s| s.model.clone()))
    };
    let Some(row) = row else {
        super::host::state().pane.model_saving = false;
        return Err("no such model row".into());
    };
    let params = json!({
        "family_id": row.family_id,
        "model_id": row.model_id,
        "profile_id": conv.profile(),
        "route_id": row.route_id,
        "session_id": session,
    });
    let result = conv.client().request(LLM_SELECT, params).await;
    let mut st = super::host::state();
    st.pane.model_saving = false;
    let notice = match &result {
        Ok(v) => disposition_notice(v, &row.title, running.as_deref()),
        Err(octoscode_client::ClientError::Rpc { error, .. }) => format!("Couldn't save: {}", error.message),
        Err(_) => "Couldn't save: the server refused the change".into(),
    };
    st.pane.model_notice = Some(notice.clone());
    if let Ok(v) = &result {
        let saved = v.get("applied").and_then(|a| a.as_bool()) == Some(true)
            || matches!(
                v.get("runtime_disposition").and_then(|d| d.as_str()),
                Some("reloaded" | "deferred" | "restart_required" | "persisted_but_not_live" | "unchanged")
            );
        if saved {
            for m in st.pane.models.iter_mut() {
                m.selected = m.model_id == row.model_id && m.route_id == row.route_id;
            }
            st.pane.last_seen_model = Some(row.model_id.clone());
            st.pane.external_change = false;
            drop(st);
            crate::screens::settings::mark_model_selected(&conv.store, &row.model_id);
        }
    }
    Ok(notice)
}

/// `permission/profile/set` (a preset or the approval policy), then re-read
/// the status stamp (`permissions-section.tsx:4`: "re-read when the pane opens
/// and after each save").
pub async fn set_permission(conv: &crate::flow::Conversation, intent: PermIntent) -> Result<String, String> {
    let session = conv.session_id();
    let result = conv.client().request(PERM_SET, intent.params(&session)).await;
    match result {
        Ok(v) => {
            if v.get("session_id").and_then(|s| s.as_str()) != Some(session.as_str()) {
                super::host::state().pane.perm_save = Some(SaveState::Failed("the reply named another session".into()));
                return Err("permission/profile/set: another session's reply".into());
            }
            fold_permission(&conv.store, &v);
            let status = if advertised(&conv.store, STATUS_METHOD) {
                conv.client()
                    .request(STATUS_METHOD, json!({"session_id": session, "profile_id": conv.profile()}))
                    .await
                    .ok()
                    .and_then(|v| parse_status(&v, &session))
            } else {
                None
            };
            let mut st = super::host::state();
            if let PermIntent::Approval(p) = &intent {
                st.pane.approval_set_here = Some(p.clone());
            }
            if status.is_some() {
                st.pane.status = status;
            }
            st.pane.perm_save = Some(SaveState::Saved);
            Ok("saved".into())
        }
        Err(e) => {
            let msg = match &e {
                octoscode_client::ClientError::Rpc { error, .. } => error.message.clone(),
                other => other.to_string(),
            };
            super::host::state().pane.perm_save = Some(SaveState::Failed(msg.clone()));
            Err(format!("{PERM_SET}: {msg}"))
        }
    }
}

/// Resume chat (`App.tsx:1723-1756` + `composer-seat-handover.ts:136-174`
/// plan 2): acquire the seat at the OBSERVED revision, release it straight
/// back to `internal`, then send the composer's prompt ONCE. Any refusal
/// sends nothing ("Couldn't resume chat — nothing was sent"). The control
/// token lives only inside this function and is never logged.
pub async fn resume_chat(conv: &crate::flow::Conversation) -> Result<String, String> {
    let session = conv.session_id();
    let fail = |why: &str| {
        let mut st = super::host::state();
        st.pane.resume_busy = false;
        st.pane.resume_notice = Some("Couldn't resume chat — nothing was sent".into());
        Err::<String, String>(format!("resume chat: {why}"))
    };
    let revision = {
        let st = super::host::state();
        st.pane.driver.disclosure().and_then(|d| d.binding.as_ref()).map(|b| b.revision)
    };
    crate::screens::fleet_driver::acquiring_driver_id(); // persisted before a lease is taken under it
    let mut acquire = crate::chrome::take_over_params(&session);
    if let Some(r) = revision {
        acquire["expected_revision"] = json!(r);
    }
    let reply = match conv.client().request("session/driver/acquire", acquire).await {
        Ok(v) => v,
        Err(_) => return fail("acquire refused"),
    };
    let token = reply.get("control_token").and_then(|t| t.as_str()).filter(|t| !t.is_empty());
    let binding = reply.get("binding");
    let me = crate::chrome::native_driver_id();
    let ours = binding.and_then(|b| b.get("driver_id")).and_then(|d| d.as_str()) == Some(me.as_str());
    let (Some(token), true) = (token, ours) else { return fail("acquire receipt malformed") };
    let epoch = binding.and_then(|b| b.get("epoch")).and_then(|e| e.as_u64()).unwrap_or(0);
    let rev = binding.and_then(|b| b.get("revision")).and_then(|e| e.as_u64()).unwrap_or(0);
    let release = json!({
        "session_id": session,
        "driver_id": me,
        "epoch": epoch,
        "control_token": token,
        "expected_revision": rev,
        "next": "internal",
    });
    match conv.client().request("session/driver/release", release).await {
        Ok(v) if v.get("mode").and_then(|m| m.as_str()) == Some("internal") => {}
        _ => return fail("release refused"),
    }
    crate::chrome::set_held(&session, None);
    {
        let mut st = super::host::state();
        st.pane.driver = Inventory::Loading;
    }
    let sent = conv.submit_draft().await.map_err(|e| e.to_string());
    let driver = dd::walk(conv).await;
    let mut st = super::host::state();
    st.pane.resume_busy = false;
    st.pane.driver = driver;
    match sent {
        Ok(turn) if !turn.is_empty() => Ok(format!("resumed; prompt sent once ({turn})")),
        Ok(_) => Ok("resumed; no prompt to send".into()),
        Err(e) => {
            st.pane.resume_notice = Some("Couldn't resume chat — nothing was sent".into());
            Err(e)
        }
    }
}

// -------------------------------------------------------------------- view

fn kv(d: &mut Dsl, id: &str, label: &str, value: &str, inner_w: f64) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{top: 2 bottom: 2}");
    let lw = ui::text_w(label, 12.0, Face::Regular) + 2.0;
    d.text(&format!("{id}_label"), label, &Txt::new(12.0, Face::Regular, tok::MUTED));
    d.gap(W::Fill, 1.0);
    let budget = (inner_w - lw - 16.0).max(60.0);
    let shown = ui::fit_w(value, budget, 12.0, Face::Medium);
    d.text(&format!("{id}_value"), &shown, &Txt::new(12.0, Face::Medium, tok::TEXT));
    d.close();
}

fn radio(d: &mut Dsl, id: &str, on: bool) {
    d.surface(
        &format!("{id}_radio"),
        "width: 18 height: 18 flow: Overlay align: Align{x: 0.5 y: 0.5}",
        tok::SURFACE,
        9.0,
        Some(if on { tok::BLUE } else { "#c7c7ccff" }),
    );
    if on {
        let dot = d.anon();
        d.surface(&dot, "width: 9 height: 9", tok::BLUE, 4.5, None);
        d.close();
    }
    d.close();
}

/// A selectable row: radio, title, an optional muted second line.
fn choice_row(d: &mut Dsl, id: &str, title: &str, sub: Option<&str>, on: bool, event: Option<&str>, inner_w: f64) {
    d.view(&format!("{id}_box"), "width: Fill height: Fit flow: Overlay");
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{top: 8 bottom: 8}");
    radio(d, id, on);
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 2");
    let budget = (inner_w - 28.0).max(80.0);
    d.text(
        &format!("{id}_title"),
        &ui::fit_w(title, budget, 13.0, Face::Regular),
        &Txt::new(13.0, if on { Face::Medium } else { Face::Regular }, if event.is_some() || on { tok::TEXT } else { tok::FAINT }),
    );
    if let Some(s) = sub {
        d.text(&format!("{id}_sub"), &ui::fit_w(s, budget, 12.0, Face::Regular), &Txt::new(12.0, Face::Regular, tok::MUTED));
    }
    d.close();
    d.close();
    if let Some(ev) = event {
        d.tap(id, ev);
    }
    d.close();
}

fn help(d: &mut Dsl, id: &str, text: &str) {
    d.text(id, text, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
}

fn status_line(d: &mut Dsl, id: &str, text: &str, color: &'static str) {
    d.text(id, text, &Txt::new(12.0, Face::Regular, color).w(W::Fill).wrap());
}

pub fn build(d: &mut Dsl, st: &PaneState, fleet: &mut super::fleetview::FleetState, frame: &Frame, store: &Store) {
    let width = frame.dialog_w(560.0);
    let pad = ui::dialog_pad(frame, width);
    // The card's content width: dialog - padding - the scroll gutter - the
    // section card's own 14 px insets.
    let inner_w = width - 2.0 * pad - 10.0 - 28.0;
    let session = store.active_session().unwrap_or_default();
    ui::shell_open(d, frame, width);
    ui::header(d, "Session settings", "b3.close");
    let scope = if st.session.is_empty() { session.clone() } else { st.session.clone() };
    d.text("b3_sc_scope", &ui::fit_w(&scope, width - 2.0 * pad, 11.5, Face::Mono), &Txt::new(11.5, Face::Mono, tok::MUTED).w(W::Fill));
    d.gap(W::Fill, 12.0);
    ui::body_open(d, frame, width, 64.0);
    let body = d.anon();
    d.view(&body, "width: Fill height: Fit flow: Down spacing: 12");

    // 1. Holder banners (`SessionConfigPane.tsx:217-255`): where the
    // operator lands when they open the pane.
    let foreign = foreign_held(st, store);
    if foreign {
        d.surface(
            "b3_sc_holder",
            "width: Fill height: Fit flow: Down spacing: 8 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}",
            tok::AMBER_BG,
            12.0,
            Some(tok::AMBER_LINE),
        );
        d.text("b3_sc_holder_head", "Another app is using this session", &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill));
        if let Some(n) = &st.resume_notice {
            status_line(d, "b3_sc_holder_notice", n, tok::RED);
        }
        let (label, kind) = if st.resume_busy { ("Resuming chat…", Btn::Disabled) } else { ("Resume chat", Btn::Outline) };
        d.button("b3_sc_resume", label, "b3.sc.resume_chat", kind, W::Fit, 32.0);
        d.close();
    } else if own_held(st) {
        d.surface(
            "b3_sc_ownhold",
            "width: Fill height: Fit flow: Down spacing: 4 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}",
            tok::SURFACE2,
            12.0,
            Some(tok::HAIRLINE),
        );
        d.text("b3_sc_ownhold_head", "A peer you started is using this session", &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill));
        help(d, "b3_sc_ownhold_body", "It keeps running while you chat. Chat sends hand control back first.");
        d.close();
    }

    // 2. Model (`model-section.tsx:36-94`).
    ui::card_open(d, "b3_sc_model", 6.0);
    ui::section_title(d, "b3_sc_model_title", "Model");
    help(d, "b3_sc_model_help", "Changing the model changes the shared profile, not just this session.");
    let saved = st
        .models
        .iter()
        .find(|m| m.selected)
        .map(|m| m.title.clone())
        .or_else(|| store.domains.profile.llm_models().into_iter().find(|m| m.selected).map(|m| m.title))
        .unwrap_or_else(|| "(no model selected)".into());
    kv(d, "b3_sc_saved", "Saved for this profile:", &saved, inner_w);
    let runtime = st.status.as_ref().and_then(|s| s.model.clone());
    if let Some(rt) = &runtime {
        kv(d, "b3_sc_runtime", "Session runtime", rt, inner_w);
    }
    if let (Some(rt), true) = (&runtime, store.domains.turn.in_flight_count() > 0) {
        kv(d, "b3_sc_turn_model", "This response is using:", rt, inner_w);
    }
    if !st.models.is_empty() {
        d.gap(W::Fill, 2.0);
        d.hairline();
        for (i, m) in st.models.iter().enumerate() {
            let sub = match (&m.route_label, m.available) {
                (_, false) => format!("{} · unavailable", m.family_id),
                (Some(r), _) => format!("{} · {r}", m.family_id),
                (None, _) => m.family_id.clone(),
            };
            let event = (!st.model_saving && m.available && !m.selected).then(|| format!("b3.sc.model#{i}"));
            choice_row(d, &format!("b3_sc_model_{i}"), &m.title, Some(&sub), m.selected, event.as_deref(), inner_w);
        }
    } else if st.loading {
        status_line(d, "b3_sc_models_loading", "Loading models…", tok::MUTED);
    } else if !advertised(store, LLM_LIST) {
        status_line(d, "b3_sc_models_none", "Not supported by this server", tok::MUTED);
    }
    if st.model_saving {
        status_line(d, "b3_sc_model_saving", "Saving…", tok::MUTED);
    }
    if st.external_change {
        status_line(d, "b3_sc_model_external", "The selection changed in another tab or app", tok::AMBER);
    }
    // The creation-time default's failure is the pane's notice too
    // (`App.tsx:3270-3277`).
    if let Some(n) = st.model_notice.clone().or_else(crate::screens::session_defaults::apply_error) {
        let color = if n.starts_with("Couldn't") { tok::RED } else { tok::GREEN };
        status_line(d, "b3_sc_model_notice", &n, color);
    }
    d.close();

    // 3. Permissions (`permissions-section.tsx:32-97`).
    ui::card_open(d, "b3_sc_perm", 6.0);
    ui::section_title(d, "b3_sc_perm_title", "Permissions");
    help(
        d,
        "b3_sc_perm_help",
        "Applies from your next message. A response that is already running keeps the permissions it started with.",
    );
    let opts = perm_options(store);
    if !advertised(store, PERM_SET) || opts.is_empty() {
        status_line(d, "b3_sc_perm_none", if st.loading && advertised(store, PERM_SET) { "Loading access…" } else { "Not supported by this server" }, tok::MUTED);
    } else {
        d.hairline();
        let selected = perm_selected(store);
        let saving = matches!(st.perm_save, Some(SaveState::Saving));
        for (i, o) in opts.iter().enumerate() {
            let on = selected.as_deref() == Some(o.id().as_str());
            let sub = o.dangerous().then_some("Asks before it is applied");
            let event = (!saving && !on).then(|| format!("b3.sc.perm#{i}"));
            choice_row(d, &format!("b3_sc_perm_{i}"), &o.label(), sub, on, event.as_deref(), inner_w);
        }
        if let Some(o) = &st.confirm {
            risk_confirm(d, o, st.risk_ack);
        }
    }
    // The readback (`:56-66`): the stamp's value, else what this client set
    // (not verified); no line at all when nothing is known.
    let readback = st.status.as_ref().and_then(|s| s.approval_policy.clone());
    let has_readback = readback.is_some() || st.approval_set_here.is_some();
    match (readback, &st.approval_set_here) {
        (Some(p), _) => kv(d, "b3_sc_policy", "Approval policy:", &p, inner_w),
        (None, Some(p)) => {
            kv(d, "b3_sc_policy", "Approval policy:", p, inner_w);
            status_line(d, "b3_sc_policy_unverified", "Current approval policy not verified (as set here)", tok::AMBER);
        }
        (None, None) => {}
    }
    if advertised(store, PERM_SET) {
        let current = st
            .status
            .as_ref()
            .and_then(|s| s.approval_policy.clone())
            .or_else(|| st.approval_set_here.clone())
            .map(|p| if p == "never" { 1 } else { 0 })
            .unwrap_or(usize::MAX);
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Down spacing: 6 padding: Inset{top: 4}");
        // The readback line above already names the control.
        if !has_readback {
            d.text("b3_sc_policy_label", "Approval policy", &Txt::new(12.0, Face::Medium, tok::MUTED));
        }
        let opts: Vec<(&str, String)> = vec![
            ("On request", "b3.sc.approval.on-request".into()),
            ("Never ask", "b3.sc.approval.never".into()),
        ];
        d.segmented("b3_sc_policy_seg", &opts, current, W::Fill, Seg::Tab);
        d.close();
    }
    match &st.perm_save {
        Some(SaveState::Saving) => status_line(d, "b3_sc_perm_state", "Saving…", tok::MUTED),
        Some(SaveState::Saved) => status_line(d, "b3_sc_perm_state", "Saved", tok::GREEN),
        Some(SaveState::Failed(m)) => {
            let row = d.anon();
            d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
            let text = format!("Failed: {m}");
            d.text("b3_sc_perm_state", &ui::fit_w(&text, inner_w - 60.0, 12.0, Face::Regular), &Txt::new(12.0, Face::Regular, tok::RED));
            d.link("b3_sc_perm_retry", "Retry", Some("b3.sc.perm.retry"), 12.5);
            d.close();
        }
        None => {}
    }
    d.close();

    // 4. Sandbox (`sandbox-section.tsx:23-80`).
    ui::card_open(d, "b3_sc_sandbox", 6.0);
    ui::section_title(d, "b3_sc_sandbox_title", "Sandbox");
    let supported = store.capabilities().iter().any(|f| f == crate::screens::session_defaults::SANDBOX_FEATURE);
    if !supported {
        status_line(d, "b3_sc_sandbox_none", "Not supported by this server", tok::MUTED);
    } else {
        let s = st.status.clone().unwrap_or_default();
        let enabled = s.sandbox.as_deref().is_some_and(|v| v != "off" && v != "danger-full-access" && v != "none");
        kv(d, "b3_sc_sb_on", "Sandbox:", if s.sandbox.is_some() { if enabled { "on" } else { "off" } } else { "not reported" }, inner_w);
        let net = match s.network.as_deref() {
            Some("allowed" | "allow" | "enabled") => "allowed",
            Some(_) => "blocked",
            None => "not reported",
        };
        kv(d, "b3_sc_sb_net", "Network:", net, inner_w);
        let paths = match &s.read_paths {
            None => "not reported".to_owned(),
            Some(p) if p.is_empty() => "(none)".to_owned(),
            Some(p) => p.join(", "),
        };
        kv(d, "b3_sc_sb_paths", "Read paths:", &paths, inner_w);
    }
    help(d, "b3_sc_sandbox_note", "Set when the session opens — start a new session to change it");
    d.link("b3_sc_newsession", "New session with…", Some("b3.sc.newsession"), 12.5);
    d.close();

    // 5. Show thinking (`SessionConfigPane.tsx:340-368`).
    ui::card_open(d, "b3_sc_thinking", 0.0);
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 12");
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 3");
    ui::section_title(d, "b3_sc_thinking_title", "Show thinking");
    help(d, "b3_sc_thinking_help", "Thinking appears in this Session's transcript, collapsed by default.");
    d.close();
    d.toggle("b3_sc_thinking_toggle", store.domains.session.thinking(&session).show_reasoning, "b3.sc.thinking");
    d.close();
    d.close();

    // 6. Advanced (`:369-417`): collapsed by default, remembered.
    d.view("b3_sc_adv_box", "width: Fill height: 36 flow: Overlay");
    let row = d.anon();
    d.view(&row, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 2}");
    d.icon(
        "b3_sc_adv_chev",
        if st.advanced_open { "b3_chevron_down_dark.svg" } else { "b3_chevron_right_dark.svg" },
        14.0,
        tok::TEXT,
    );
    d.text("b3_sc_adv_label", "Advanced", &Txt::new(13.0, Face::Semibold, tok::TEXT));
    d.close();
    d.tap("b3_sc_adv", "b3.sc.advanced");
    d.close();
    if st.advanced_open {
        advanced(d, st, store, foreign, inner_w);
        // A10 — `advancedChildren`: the control seat + the controller
        // console, inside the same card width.
        super::fleet_console::session_controls(d, fleet, store, inner_w);
    }
    d.close();
    ui::body_close(d);
    ui::shell_close(d);
}

/// The Full access risk confirmation (`PERMISSION_RISK_COPY`, `App.tsx:3914-3925`).
fn risk_confirm(d: &mut Dsl, o: &PermOption, ack: bool) {
    d.surface(
        "b3_sc_risk",
        "width: Fill height: Fit flow: Down spacing: 8 padding: Inset{left: 12 right: 12 top: 12 bottom: 12}",
        tok::RED_BG,
        10.0,
        Some("#f5c2c7ff"),
    );
    d.text("b3_sc_risk_title", "Enable full access?", &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill));
    help(
        d,
        "b3_sc_risk_body",
        "Octos can read and modify files outside the workspace and use the network without the normal sandbox boundary.",
    );
    let net = if o.network == "allow" { "Network allowed" } else { "Network blocked" };
    d.text("b3_sc_risk_facts", &format!("Filesystem access: Full access · Network access: {net}"), &Txt::new(12.0, Face::Medium, tok::TEXT).w(W::Fill).wrap());
    // The web's acknowledgement is a checkbox ("Tick the box above…"): the
    // whole row is its 28+ px tap target.
    d.view("b3_sc_risk_ack_box", "width: Fill height: Fit flow: Overlay");
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{top: 6 bottom: 6}");
    d.surface(
        "b3_sc_risk_ack_check",
        "width: 18 height: 18 flow: Overlay align: Align{x: 0.5 y: 0.5}",
        if ack { tok::BLUE } else { tok::SURFACE },
        4.0,
        Some(if ack { tok::BLUE } else { "#aeaeb2ff" }),
    );
    if ack {
        d.icon("b3_sc_risk_ack_tick", "b3_check_white.svg", 12.0, tok::WHITE);
    }
    d.close();
    d.text(
        "b3_sc_risk_ack_label",
        "I understand that this session can make unrestricted changes.",
        &Txt::new(12.0, Face::Regular, tok::TEXT).w(W::Fill).wrap(),
    );
    d.close();
    d.tap("b3_sc_risk_ack", "b3.sc.risk.ack");
    d.close();
    if !ack {
        help(d, "b3_sc_risk_hint", "Tick the box above to enable this button.");
    }
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5} spacing: 8");
    d.button("b3_sc_risk_cancel", "Cancel", "b3.sc.risk.cancel", Btn::Outline, W::Fit, 32.0);
    d.button(
        "b3_sc_risk_confirm",
        "Enable full access",
        "b3.sc.risk.confirm",
        if ack { Btn::Primary } else { Btn::Disabled },
        W::Fit,
        32.0,
    );
    d.close();
    d.close();
}

fn advanced(d: &mut Dsl, st: &PaneState, store: &Store, foreign: bool, inner_w: f64) {
    ui::card_open(d, "b3_sc_adv_body", 6.0);
    d.text("b3_sc_who_label", "Who controls this session", &Txt::new(12.0, Face::Medium, tok::MUTED));
    d.text("b3_sc_who", dd::controller_label(&st.driver), &Txt::new(13.0, Face::Medium, tok::TEXT));
    if foreign {
        status_line(d, "b3_sc_adv_foreign", "Another app is using this session", tok::AMBER);
    }
    match &st.driver {
        Inventory::Loading => status_line(d, "b3_sc_driver_loading", "Reading the controller…", tok::MUTED),
        Inventory::Unavailable => {
            let why = if store.active_session().is_none() { "No session is open" } else { "Not reported by this server" };
            status_line(d, "b3_sc_driver_none", why, tok::MUTED);
        }
        Inventory::Error(reason) => status_line(d, "b3_sc_driver_error", dd::error_label(reason), tok::RED),
        Inventory::Complete { operations, disclosure, .. } => {
            d.hairline();
            d.text("b3_sc_driver_mode", dd::mode_label(disclosure), &Txt::new(13.0, Face::Semibold, tok::TEXT));
            kv(d, "b3_sc_driver_recovery", "Recovery", dd::recovery_label(disclosure), inner_w);
            if let Some(b) = &disclosure.binding {
                kv(d, "b3_sc_driver_id", "Driver", &b.driver_id, inner_w);
                kv(d, "b3_sc_driver_epoch", "Epoch", &b.epoch.to_string(), inner_w);
                kv(d, "b3_sc_driver_rev", "Revision", &b.revision.to_string(), inner_w);
                let lease = dd::lease_label(b.lease_expires_at_ms);
                kv(d, "b3_sc_driver_lease", "Lease", lease.trim_start_matches("Lease expires "), inner_w);
            }
            if !operations.is_empty() {
                d.hairline();
                d.text("b3_sc_ops_title", &format!("Peers ({})", operations.len()), &Txt::new(12.0, Face::Medium, tok::MUTED));
                for (i, op) in operations.iter().enumerate().take(8) {
                    kv(d, &format!("b3_sc_op_{i}"), &op.slug, &format!("{} · {}", op.lifecycle, op.model), inner_w);
                }
            }
        }
    }
    d.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_runtime_disposition_to_its_exact_message() {
        // SessionConfigPane.test.tsx:276
        let r = |d: &str| json!({"applied": true, "runtime_disposition": d});
        assert_eq!(disposition_notice(&r("reloaded"), "GLM-5", None), "Saved. Your next message uses GLM-5");
        assert_eq!(disposition_notice(&r("deferred"), "x", None), "Saved. The model is not active yet");
        assert_eq!(
            disposition_notice(&json!({"runtime_disposition": "deferred", "condition": "after the turn"}), "x", None),
            "Saved. The model is not active yet (after the turn)"
        );
        assert_eq!(disposition_notice(&r("restart_required"), "x", Some("v4-flash")), "Saved. The server keeps running v4-flash until it restarts");
        assert_eq!(
            disposition_notice(&json!({"runtime_disposition": "restart_required", "runtime_policy_stamp": {"model": "deepseek-v4-flash"}}), "x", None),
            "Saved. The server keeps running deepseek-v4-flash until it restarts"
        );
        assert_eq!(
            disposition_notice(&json!({"runtime_disposition": "persisted_but_not_live", "runtime_error": "no key"}), "x", None),
            "Saved, but not usable right now: no key"
        );
        assert_eq!(disposition_notice(&r("unchanged"), "x", None), "Already selected");
        assert_eq!(
            disposition_notice(&json!({"runtime_disposition": "refused", "reason": "locked"}), "x", None),
            "Couldn't save: locked"
        );
        // :307 "a refused save is never Saved"
        assert!(disposition_notice(&json!({"runtime_disposition": "bogus"}), "x", None).starts_with("Couldn't save"));
        assert!(disposition_notice(&json!({"applied": false}), "x", None).starts_with("Couldn't save"));
    }

    #[test]
    fn the_status_read_is_checked_and_the_stamp_carries_the_policy() {
        // The recorded r3-session status read (line 8).
        let v = json!({"session_id": "s", "model": {"model": "deepseek-v4-flash", "provider": "deepseek"},
            "sandbox": "workspace-write", "runtime_policy_stamp": {"approval_policy": "on-request", "network": "allowed"}});
        let f = parse_status(&v, "s").unwrap();
        assert_eq!(f.model.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(f.approval_policy.as_deref(), Some("on-request"));
        assert_eq!(f.sandbox.as_deref(), Some("workspace-write"));
        assert_eq!(f.network.as_deref(), Some("allowed"));
        assert!(parse_status(&v, "other").is_none(), "another session's read is dropped");
    }

    #[test]
    fn the_live_model_list_keeps_route_ids() {
        // r2-profile line 12 (primary + fallbacks, no `models` key).
        let v = json!({"llm": {"primary": {"model_id": "deepseek-v4-flash", "family_id": "deepseek", "route_id": "deepseek",
            "route": {"label": "Official API", "route_id": "deepseek"}, "selected": true, "available": true},
            "fallbacks": [{"model_id": "glm-5", "family_id": "zhipu", "route": {"route_id": "z1"}, "selected": false, "available": true}]}});
        let rows = parse_models(&v);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].route_id.as_deref(), rows[0].route_label.as_deref(), rows[0].selected), (Some("deepseek"), Some("Official API"), true));
        assert_eq!(rows[1].route_id.as_deref(), Some("z1"));
    }

    #[test]
    fn full_access_asks_first_and_never_sends_without_the_acknowledgement() {
        use octoscode_store::domains::profile::{PermissionNetworkPolicy as N, PermissionProfileMode as M, PermissionProfileSelection as S};
        let store = Store::new();
        store.set_active(Some("s".into()));
        store.domains.profile.set_permission(
            S { mode: M::WorkspaceWrite, network: N::Allow },
            vec![S { mode: M::ReadOnly, network: N::Deny }, S { mode: M::DangerFullAccess, network: N::Allow }],
        );
        let opts = perm_options(&store);
        assert_eq!(opts.iter().map(PermOption::label).collect::<Vec<_>>(), ["Write · Network allowed", "Read · Network blocked", "Full access · Network allowed"]);
        let mut st = PaneState::default();
        assert_eq!(perform(&mut st, "b3.sc.perm", 0, &store), Outcome::Done, "the current preset sends nothing");
        assert_eq!(perform(&mut st, "b3.sc.perm", 2, &store), Outcome::Done, "full access confirms first");
        assert!(st.confirm.is_some());
        assert_eq!(perform(&mut st, "b3.sc.risk.confirm", 0, &store), Outcome::Done, "no ack: nothing sent");
        perform(&mut st, "b3.sc.risk.ack", 0, &store);
        assert!(matches!(perform(&mut st, "b3.sc.risk.confirm", 0, &store), Outcome::Spawn(Job::PanePerm(_))));
        assert_eq!(st.perm_save, Some(SaveState::Saving));
        assert_eq!(perform(&mut st, "b3.sc.perm", 1, &store), Outcome::Done, "a second save while saving is refused");
    }

    /// The pane's DSL evaluates in the app VM (the mount path), loading and
    /// loaded, desktop and phone frames.
    #[test]
    fn the_pane_evaluates_in_the_app_vm() {
        use makepad_widgets::*;
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(makepad_widgets::script_mod);
        cx.with_vm(octoscript_widgets::design::script_mod);
        cx.with_vm(octoscript_widgets::kit::script_mod);
        let store = Store::new();
        store.set_active(Some("s".into()));
        store.domains.config.set_supported_methods(vec![PERM_SET.into(), LLM_LIST.into(), STATUS_METHOD.into()]);
        let loaded = PaneState {
            status: Some(StatusFacts { model: Some("deepseek-v4-flash".into()), approval_policy: Some("on-request".into()), ..Default::default() }),
            models: parse_models(&json!({"llm": {"primary": {"model_id": "m", "family_id": "f", "route_id": "r", "selected": true, "available": true}, "fallbacks": []}})),
            advanced_open: true,
            ..Default::default()
        };
        for st in [PaneState { loading: true, ..Default::default() }, loaded] {
            for frame in [Frame::DESKTOP, Frame { avail_w: 360.0, avail_h: 700.0 }] {
                let mut d = Dsl::new();
                build(&mut d, &st, &mut Default::default(), &frame, &store);
                let dsl = d.finish();
                assert!(
                    crate::mount::eval_component(&mut cx, makepad_widgets::MAIN_SPLASH_VM_ID, &dsl).is_ok(),
                    "the pane DSL evaluates"
                );
            }
        }
        let mut d = Dsl::new();
        super::super::thinking::build(&mut d, &Frame::DESKTOP, &store);
        assert!(crate::mount::eval_component(&mut cx, makepad_widgets::MAIN_SPLASH_VM_ID, &d.finish()).is_ok());
        // The inspector's link field (A13: one line, shortened in the
        // middle; Copy writes the whole link).
        let insp = super::super::inspector::InspState {
            link: "octoscode://session?s=%5B%22%2Fhome%2Fuser%2Focts%22%2C%22a8%22%2C%22a8%3Amain%22%5D".into(),
            copied: true,
            ..Default::default()
        };
        let mut d = Dsl::new();
        super::super::inspector::build(&mut d, &insp, &Frame::DESKTOP, &store);
        let dsl = d.finish();
        assert!(dsl.contains("b3_insp_link_value_field := DesignSurface") && dsl.contains("b3_insp_link_value := Label"));
        assert!(crate::mount::eval_component(&mut cx, makepad_widgets::MAIN_SPLASH_VM_ID, &dsl).is_ok(), "the link field evaluates");
    }

    #[test]
    fn the_pane_lowers_balanced_with_every_section_and_its_taps() {
        let store = Store::new();
        store.set_active(Some("s".into()));
        store.domains.config.set_supported_methods(vec![PERM_SET.into(), LLM_LIST.into()]);
        let st = PaneState {
            models: vec![ModelRow { title: "glm-5".into(), model_id: "glm-5".into(), family_id: "zhipu".into(), route_id: None, route_label: None, selected: false, available: true }],
            advanced_open: true,
            ..Default::default()
        };
        let mut d = Dsl::new();
        build(&mut d, &st, &mut Default::default(), &Frame::DESKTOP, &store);
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        let taps = crate::screens::taps::wired_taps(&dsl);
        for ev in ["b3.close", "b3.sc.model#0", "b3.sc.thinking", "b3.sc.advanced", "b3.sc.newsession", "b3.sc.approval.never"] {
            assert!(taps.iter().any(|(_, e)| e == ev), "{ev} wired");
        }
        for text in ["Session settings", "Saved for this profile:", "Applies from your next message.", "Not supported by this server", "Show thinking", "Who controls this session"] {
            assert!(dsl.contains(text), "{text}");
        }
    }

    /// `model-section.tsx:76-81`: "This response is using:" only while a turn
    /// runs; the saved default and the session runtime always.
    #[test]
    fn the_running_response_stamp_shows_only_while_a_turn_runs() {
        let store = Store::new();
        store.set_connection("Live".into(), true);
        store.set_active(Some("s".into()));
        let st = PaneState {
            status: Some(StatusFacts { model: Some("deepseek-v4-flash".into()), ..Default::default() }),
            ..Default::default()
        };
        let lower = |store: &Store| {
            let mut d = Dsl::new();
            build(&mut d, &st, &mut Default::default(), &Frame::DESKTOP, store);
            d.finish()
        };
        let idle = lower(&store);
        assert!(idle.contains("Session runtime") && idle.contains("deepseek-v4-flash"));
        assert!(!idle.contains("This response is using:"), "no stamp while idle");
        store.domains.turn.started("t1");
        assert!(lower(&store).contains("This response is using:"), "the stamp while the turn runs");
    }
}
