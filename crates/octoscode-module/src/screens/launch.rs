//! A8 — the WORKSPACE LAUNCH: what happens between "start a session in this
//! folder" and the `session/open`, ported from the web's
//! `features/workspace/launch-model.ts` (the runtime state
//! `idle | resolving | awaiting_choice | opening`),
//! `features/session/launch-transition.ts` (the lease taken BEFORE
//! `launch/resolve`; an older resolver is rejected),
//! `use-octos-session.ts:3010-3060` `resolveInitialLaunch` and
//! `features/workspace/LaunchDecisionPanel.tsx` (the decision panel).
//!
//! When the server advertises `launch/resolve` AND `session.workspace_cwd.v1`
//! a workspace launch asks it first: `resume` / `activate` open at once with
//! the resolved profile; `cross_profile` waits for the person to choose which
//! profile owns the new Session; `no_profile` offers to create the local
//! profile first (the web routes it to its onboarding panel). Without the
//! advertisement the launch opens directly (the web's fallback). The composer
//! text typed while a choice was pending moves to the new Session only once
//! its open COMMITS (`product.spec.ts` "moves drafts only after a
//! profile-choice Session transition commits").
use std::sync::Mutex;

use serde_json::json;

use octoscode_store::Store;

use super::board3::host::Outcome;
use super::board3::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};
use crate::i18n::{tr, tr1};

/// `LaunchRuntimeState.phase` (`launch-model.ts:3-7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Resolving,
    AwaitingChoice,
    Opening,
}

/// `LaunchResolveResult` (octos-core `ui_protocol.rs:3321-3332`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// `resume` | `activate` | `cross_profile` | `no_profile`.
    pub decision: String,
    pub resolved_profile: Option<String>,
    pub existing_profiles: Vec<String>,
}

/// The launch runtime (`LaunchRuntimeState`) plus the transition lease.
#[derive(Debug, Clone, Default)]
pub struct LaunchState {
    pub phase: Phase,
    pub cwd: Option<String>,
    pub decision: Option<Decision>,
    pub error: Option<String>,
    /// The lease generation: `begin` takes a new one; a resolver or an
    /// opening holding an older lease is stale (`launch-transition.ts`).
    pub lease: u64,
}

static STATE: Mutex<LaunchState> = Mutex::new(LaunchState { phase: Phase::Idle, cwd: None, decision: None, error: None, lease: 0 });

fn lock() -> std::sync::MutexGuard<'static, LaunchState> {
    STATE.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn snapshot() -> LaunchState {
    lock().clone()
}

/// Test seam.
pub fn reset() {
    *lock() = LaunchState::default();
    crate::screens::onboarding::reset();
}

/// Own the transition BEFORE `launch/resolve` (`launch-transition.ts:25-60`):
/// the new lease retires every older one. A17 — a new launch resets the
/// onboarding (`use-octos-session.ts:3265` `onboardingController.reset()`),
/// so a previous panel's late reply can never publish into this one.
pub fn begin(cwd: &str) -> u64 {
    let lease = {
        let mut st = lock();
        st.lease += 1;
        st.phase = Phase::Resolving;
        st.cwd = Some(cwd.to_owned());
        st.decision = None;
        st.error = None;
        st.lease
    };
    crate::screens::onboarding::reset();
    lease
}

pub fn is_current(lease: u64) -> bool {
    lock().lease == lease
}

/// Whether the server takes the launch probe.
pub fn advertised(store: &Store) -> bool {
    store.domains.config.supported_methods().iter().any(|m| m == "launch/resolve")
        && store.capabilities().iter().any(|f| f == "session.workspace_cwd.v1")
}

/// What a launch came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launched {
    /// The new Session opened (its id).
    Opened(String),
    /// The person has to choose (the decision panel is open).
    AwaitingChoice,
    /// A newer launch owns the transition: this one stopped, silently.
    Stale,
    Failed(String),
}

fn parse_decision(v: &serde_json::Value) -> Option<Decision> {
    Some(Decision {
        decision: v.get("decision")?.as_str()?.to_owned(),
        resolved_profile: v.get("resolved_profile").and_then(|p| p.as_str()).map(str::to_owned),
        existing_profiles: v
            .get("existing_profiles")
            .and_then(|e| e.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
    })
}

/// `launch/resolve`'s params: `{cwd}`, plus `profile_id` only when the
/// connection carries one (`use-octos-session.ts:3033-3036`).
pub fn resolve_params(cwd: &str, profile: &str) -> serde_json::Value {
    let mut p = json!({"cwd": cwd});
    if !profile.trim().is_empty() {
        p["profile_id"] = json!(profile.trim());
    }
    p
}

/// Open the new Session in `cwd` under `profile` (the connection adopts the
/// chosen profile first when it differs).
async fn open_as(conv: &crate::flow::Conversation, cwd: &str, profile: Option<&str>, lease: u64) -> Launched {
    if !is_current(lease) {
        return Launched::Stale;
    }
    lock().phase = Phase::Opening;
    if let Some(p) = profile.filter(|p| *p != conv.profile()) {
        conv.adopt_profile(p.to_owned());
    }
    match conv.new_chat(Some(cwd.to_owned())).await {
        Ok(id) => {
            let mut st = lock();
            if st.lease == lease {
                *st = LaunchState { lease, ..Default::default() };
            }
            Launched::Opened(id)
        }
        Err(e) => {
            crate::screens::drafts::cancel_carry();
            let mut st = lock();
            if st.lease == lease {
                // A failed candidate returns to the server's choice when there
                // was one (`restoreLaunchChoice`), else to idle with the error.
                st.phase = if st.decision.is_some() { Phase::AwaitingChoice } else { Phase::Idle };
                st.error = Some("The new coding session could not be opened.".into());
            }
            Launched::Failed(e)
        }
    }
}

/// Start a session in `cwd`: resolve first when advertised
/// (`resolveInitialLaunch`), else open directly.
pub async fn create(conv: &crate::flow::Conversation, cwd: String) -> Launched {
    let lease = begin(&cwd);
    if !advertised(&conv.store) {
        return open_as(conv, &cwd, None, lease).await;
    }
    // A19 — the profile id rides only when the connection carries one
    // (`use-octos-session.ts:3033-3036`: `...(config.profileId ? {profile_id}
    // : {})`): a fresh connection has none (`connection-bootstrap.ts:21`), so
    // Core's answer decides — `no_profile` on a server with no profile yet.
    let reply = conv.client().request("launch/resolve", resolve_params(&cwd, &conv.profile())).await;
    // A19 — the decision on the protocol trace too (OCTOSCODE_TRACE_FILE; a
    // generic reply is not traced inbound, and this one carries no secret).
    match &reply {
        Ok(v) => conv.client().trace().inbound("launch/resolve", v),
        Err(e) => conv.client().trace().inbound("launch/resolve", &json!({"error": e.to_string()})),
    }
    // A newer launch took the transition while this one resolved.
    if !is_current(lease) {
        return Launched::Stale;
    }
    let decision = match reply.ok().as_ref().and_then(parse_decision) {
        Some(d) => d,
        None => {
            let mut st = lock();
            st.phase = Phase::Idle;
            st.error = Some("The server could not resolve this workspace.".into());
            return Launched::Failed("launch/resolve".into());
        }
    };
    match decision.decision.as_str() {
        // `resume` always, and `activate` for a freshly minted id, open at
        // once with the resolved profile.
        "resume" | "activate" => {
            let profile = decision.resolved_profile.clone();
            open_as(conv, &cwd, profile.as_deref(), lease).await
        }
        "cross_profile" | "no_profile" => {
            // A17 — `no_profile` is the web's onboarding decision
            // (`use-octos-session.ts:3051-3058`: the decision is recorded,
            // then `onboardingController.prepare()`); the panel's loading
            // state is set BEFORE the dialog opens, so it never flashes an
            // empty one. The catalog read runs on its own task.
            let onboarding = (decision.decision == "no_profile")
                .then(|| crate::screens::onboarding::prepare_begin(&conv.store, conv.scope().authority_epoch));
            {
                let mut st = lock();
                st.phase = Phase::AwaitingChoice;
                st.decision = Some(decision);
            }
            super::board3::host::open(super::board3::host::Dialog::Launch);
            super::board3::host::wake();
            if let (Some(Some(token)), Ok(handle)) = (onboarding, tokio::runtime::Handle::try_current()) {
                let client = conv.client().clone();
                handle.spawn(async move {
                    let r = crate::screens::onboarding::fetch_catalog(&client, token).await;
                    makepad_widgets::log!("[octoscode] onboarding prepare: {}", r.unwrap_or_else(|e| e));
                });
            }
            Launched::AwaitingChoice
        }
        other => {
            let mut st = lock();
            st.phase = Phase::Idle;
            st.error = Some(format!("Unknown launch decision: {other}"));
            Launched::Failed(other.to_owned())
        }
    }
}

/// `chooseLaunchProfile(profile)`: the panel's choice opens the Session; the
/// composer's text follows it once the open commits.
pub async fn choose(conv: &crate::flow::Conversation, profile: String) -> Launched {
    let (lease, cwd) = {
        let st = lock();
        (st.lease, st.cwd.clone())
    };
    let Some(cwd) = cwd else { return Launched::Failed("no pending launch".into()) };
    crate::screens::drafts::carry_next_switch();
    let r = open_as(conv, &cwd, Some(&profile), lease).await;
    if matches!(r, Launched::Opened(_)) {
        super::board3::host::close();
        super::board3::host::wake();
    }
    r
}

/// `no_profile`: create the local profile (`profile/local/create`, the
/// onboarding's first step), then open with it.
pub async fn create_profile_and_open(conv: &crate::flow::Conversation) -> Launched {
    let (lease, cwd) = {
        let st = lock();
        (st.lease, st.cwd.clone())
    };
    let Some(cwd) = cwd else { return Launched::Failed("no pending launch".into()) };
    lock().phase = Phase::Opening;
    match conv.create_profile().await {
        Ok(id) => {
            crate::screens::drafts::carry_next_switch();
            let r = open_as(conv, &cwd, Some(&id), lease).await;
            if matches!(r, Launched::Opened(_)) {
                super::board3::host::close();
                super::board3::host::wake();
            }
            r
        }
        Err(e) => {
            let mut st = lock();
            st.phase = Phase::AwaitingChoice;
            st.error = Some("The profile could not be created.".into());
            Launched::Failed(e.to_string())
        }
    }
}

/// A17 — the onboarding's `onConfigured` (`use-octos-session.ts:2665-2692`):
/// once the provider is tested and saved, open the canonical coding Session
/// in the launch's folder under the profile Core assigned
/// (`launchProfileConfig(config, profileId)`), the composer text following it
/// once the open commits. A launch that is no longer pending is "the
/// connection changed"; a failed open is the web's own refusal (the panel
/// keeps the created profile, so a retry repeats only test, save and open).
pub async fn open_onboarded(conv: &crate::flow::Conversation, profile: String) -> Result<(), String> {
    let (lease, cwd, awaiting) = {
        let st = lock();
        (st.lease, st.cwd.clone(), st.phase == Phase::AwaitingChoice)
    };
    let Some(cwd) = cwd.filter(|_| awaiting) else {
        return Err("The server connection changed during onboarding.".into());
    };
    crate::screens::drafts::carry_next_switch();
    match open_as(conv, &cwd, Some(&profile), lease).await {
        Launched::Opened(_) => {
            crate::screens::onboarding::reset();
            super::board3::host::close();
            super::board3::host::wake();
            Ok(())
        }
        // A newer launch owns the transition: this one stops, silently.
        Launched::Stale => Ok(()),
        _ => Err("The new coding session could not be opened.".into()),
    }
}

// ---------------------------------------------------- A19 connect-time launch

/// A19 — what a new connection carries and opens first. The web decides the
/// same two things at its start: whether a remembered tab connection is
/// restored (`connection-bootstrap.ts:44-48`, `use-octos-session.ts:2956-3008`)
/// and, for a launch, which profile id `launch/resolve` gets
/// (`use-octos-session.ts:3010-3060`). It never invents a profile id and
/// never creates a profile outside its onboarding panel
/// (`onboarding-submission.ts:59` is the web's only `profile/local/create`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// `OCTOS_PROFILE_ID` — a DEV/TEST override only (the walks, the live
    /// gate, the replay harnesses). The web has no such setting (its only
    /// build-time default is the endpoint, `connection-bootstrap.ts:11-15`),
    /// and the product never sets it: the profile is used as given and
    /// `<profile>:main` opens at the startup workspace, as before.
    /// `OCTOS_CREATE_PROFILE` (test-only too, used by no script) still mints
    /// `<profile>-<pid>` first, on this path only.
    Explicit(String),
    /// A profile the person just created in the connect screen's onboarding
    /// (`screens::connect::run_onboarding`, the web's `onConfigured`,
    /// `onboarding-submission.ts:126`): its id comes from the server; its
    /// `<profile>:main` opens and is remembered.
    Created(String),
    /// The remembered open for this server, restored directly — no
    /// `launch/resolve` (`use-octos-session.ts:2978-3001`).
    Restore(super::remembered::Remembered),
    /// The one-time migration from the previous build
    /// (`super::remembered` module doc), its profile not resolved yet.
    Migrate,
    /// A19b — the migration with the previous build's profile resolved
    /// BEFORE the socket ([`resolve_migration`]), so the connection carries it
    /// as the previous build's did: Core resolves a Session id that does not
    /// embed its profile (`<profile>:main`) from the connection's profile
    /// header, and without it answers that Session's history "unknown".
    MigrateAs(String),
    /// Nothing known: NO profile id (`connection-bootstrap.ts:21`), so
    /// `launch/resolve`'s answer decides.
    Fresh,
}

impl Start {
    /// The profile id the connection carries (`""` = none).
    pub fn profile(&self) -> String {
        match self {
            Start::Explicit(p) | Start::Created(p) | Start::MigrateAs(p) => p.clone(),
            Start::Restore(r) => r.profile_id.clone(),
            Start::Migrate | Start::Fresh => String::new(),
        }
    }

    /// Whether this connection's opens are remembered: not for the explicit
    /// override (a harness's profile is not the person's).
    pub fn remembers(&self) -> bool {
        !matches!(self, Start::Explicit(_))
    }
}

/// The dev/test override, when set.
pub fn explicit_profile() -> Option<String> {
    std::env::var("OCTOS_PROFILE_ID").ok().map(|p| p.trim().to_owned()).filter(|p| !p.is_empty())
}

/// A19 — the plan for a new connection to `server` (consumes the one-time
/// migration when it applies).
pub fn plan(server: &str) -> Start {
    if let Some(p) = explicit_profile() {
        return Start::Explicit(p);
    }
    if let Some(r) = super::remembered::load(server) {
        return Start::Restore(r);
    }
    if super::remembered::legacy_candidate(server) {
        return Start::Migrate;
    }
    Start::Fresh
}

/// A19b — resolve the migration's profile BEFORE the connection is made (the
/// previous build discovered it before its socket too): `Migrate` becomes
/// `MigrateAs(profile)`, or `Fresh` when the server has no profile to
/// migrate. Every other plan is returned unchanged.
pub async fn resolve_migration(server: &str, start: Start) -> Start {
    if start != Start::Migrate {
        return start;
    }
    match crate::flow::Conversation::discover_solo_profile(server).await {
        Some(profile) => {
            makepad_widgets::log!("[octoscode] migration: the previous build's profile is {profile}");
            Start::MigrateAs(profile)
        }
        None => {
            makepad_widgets::log!("[octoscode] migration: no previous profile on this server — a fresh launch");
            Start::Fresh
        }
    }
}

/// The profile a re-dial to `server` carries (the retry path): the plan's,
/// without consuming the migration.
pub fn planned_profile(server: &str) -> String {
    explicit_profile()
        .or_else(|| super::remembered::load(server).map(|r| r.profile_id))
        .unwrap_or_default()
}

/// What the connect-time launch came to (logged; the tests assert it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Started {
    /// The explicit profile's open was sent (its Session id).
    Explicit(String),
    /// The remembered Session was restored (its id).
    Restored(String),
    /// The previous build's landing was reopened (its id).
    Migrated(String),
    /// The launch decision: a Session opened, or a panel waits for the person.
    Launched(Launched),
    /// No folder to resolve: the workspace picker is open (the web's hero,
    /// `App.tsx:2492-2526`, when no Session is open).
    Picker,
    Failed(String),
}

/// How long a restore waits for the server's answer before it lets the
/// transport carry on alone (the open stays queued; a late accept still lands).
const OPEN_WAIT: std::time::Duration = std::time::Duration::from_secs(20);

/// The web's authenticate (`active-session-runtime.ts:482-532`, the read at
/// `:507`): the server's capability object BEFORE any Session, so the launch
/// probe's gate (`advertised`) and the onboarding's (`onboarding::supported`)
/// read the server's own answer.
pub async fn read_capabilities(conv: &crate::flow::Conversation) -> Result<usize, String> {
    use octoscode_client::domains::config::{CapabilitiesList, CapabilitiesListParams};
    let caps = conv
        .client()
        .call::<CapabilitiesList>(CapabilitiesListParams {})
        .await
        .map_err(|e| format!("config/capabilities/list: {e}"))?
        .capabilities;
    let n = caps.supported_methods.len();
    // On the protocol trace: the counts and the features (no secret).
    conv.client().trace().inbound(
        "config/capabilities/list",
        &json!({"supported_methods": n, "supported_features": caps.supported_features}),
    );
    conv.store.domains.config.set_supported_methods(caps.supported_methods);
    conv.store.domains.config.set_supported_features(caps.supported_features.clone());
    conv.store.set_capabilities(caps.supported_features);
    Ok(n)
}

/// The server's own working directory (`onboarding/workspace_list` with no
/// path) — the picker's first entry (parity row 163), when the server offers it.
async fn server_working_directory(conv: &crate::flow::Conversation) -> Option<String> {
    use octoscode_client::domains::profile::{WorkspaceList, WorkspaceListParams};
    let offered = conv.store.domains.config.supported_methods().iter().any(|m| m == "onboarding/workspace_list");
    if !offered {
        return None;
    }
    conv.client()
        .call::<WorkspaceList>(WorkspaceListParams { path: None })
        .await
        .ok()
        .map(|l| l.canonical_path)
        .filter(|p| !p.trim().is_empty())
}

/// A19 — the connect-time launch for `start`, on a connection just made
/// (its event drain already running).
///
/// * `Explicit` — the dev/test override: `<profile>:main` at `cwd`.
/// * `Restore` — the remembered Session, opened directly; a refusal clears it
///   and launches fresh (the web's `restoreRejected`, `App.tsx:947-974`).
/// * `Migrate` — once: the previous build's landing (`remembered` doc).
/// * `Fresh` — the web's launch with NO profile id for the startup
///   workspace: `cwd` (`OCTOS_WORKSPACE_CWD`), else the server's working
///   directory, else the picker. The native has no hero step at connect (it
///   always opened its startup workspace); the decision is Core's, exactly
///   as `resolveInitialLaunch` takes it: `resume`/`activate` open the Session,
///   `cross_profile` waits on the panel, `no_profile` shows the onboarding.
pub async fn startup(conv: &std::sync::Arc<crate::flow::Conversation>, start: Start, cwd: Option<String>) -> Started {
    let cwd = cwd.filter(|c| !c.trim().is_empty());
    match start {
        Start::Explicit(_) => {
            if std::env::var_os("OCTOS_CREATE_PROFILE").is_some() {
                match conv.create_profile().await {
                    Ok(id) => {
                        conv.adopt_profile(id.clone());
                        makepad_widgets::log!("[octoscode] profile ready: {id} (OCTOS_CREATE_PROFILE, test-only)");
                    }
                    Err(e) => makepad_widgets::log!("[octoscode] profile/local/create failed: {e}"),
                }
            }
            match conv.open_workspace(cwd).await {
                Ok(id) => Started::Explicit(id),
                Err(e) => Started::Failed(e),
            }
        }
        Start::Created(_) => match conv.open_workspace(cwd).await {
            Ok(id) => Started::Explicit(id),
            Err(e) => Started::Failed(e),
        },
        Start::Restore(r) => {
            let outcome = conv.watch_next_open();
            if let Err(e) = conv.open_session(&r.session_id, Some(r.cwd.clone())).await {
                return Started::Failed(e);
            }
            match tokio::time::timeout(OPEN_WAIT, outcome).await {
                Ok(Ok(Ok(id))) => Started::Restored(id),
                Ok(Ok(Err(reason))) => {
                    makepad_widgets::log!("[octoscode] the remembered Session was refused ({reason}): a fresh launch");
                    super::remembered::forget(&conv.http_base());
                    conv.adopt_profile(String::new());
                    fresh(conv, cwd).await
                }
                _ => Started::Failed("the restore was not answered yet".into()),
            }
        }
        Start::Migrate => migrate(conv, cwd).await,
        Start::MigrateAs(profile) => migrate_as(conv, profile, cwd).await,
        Start::Fresh => fresh(conv, cwd).await,
    }
}

/// A19b — where a Session lives: its id, the workspace whose project store
/// holds it, and how many messages that store reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHome {
    pub session_id: String,
    pub root: String,
    pub messages: usize,
}

/// A19b — find a Session of `profile` and the folder its history is recorded
/// in, the web's way: the per-workspace catalog read `session/list {cwd,
/// profile_id}` (A15; web `workspace-session-catalog.ts:193-208`), trusting
/// only a listing the server ATTESTS (`workspace_root` present and
/// `profile_id` echoed, octos-core `SessionListResult`), over the workspaces
/// this client knows for the server: `extra` (the launch folder, a reported
/// root), the recents remembered for this server (the web's catalog source,
/// `App.tsx:661-666` + `:757`), the server's own working directory (the
/// picker's first entry, row 163) and that directory's folders (the
/// browser's first level, row 164).
///
/// `want` = a Session id: that Session wherever it is recorded. `None`: the
/// profile's legacy main Session (`<profile>:main`, what the previous build
/// reopened) when it has history, else its most recently updated Session
/// with history. Read-only; `None` when the server offers no scoped catalog.
pub async fn find_session_home(
    conv: &crate::flow::Conversation,
    profile: &str,
    want: Option<&str>,
    extra: &[String],
) -> Option<SessionHome> {
    let config = &conv.store.domains.config;
    let offered = config.supported_methods().iter().any(|m| m == "session/list")
        && config.supported_features().iter().any(|f| f == "session.workspace_cwd.v1");
    if !offered || profile.trim().is_empty() {
        return None;
    }
    let mut candidates: Vec<String> = extra.iter().filter(|c| !c.trim().is_empty()).cloned().collect();
    candidates.extend(
        super::recents::load_recent_workspaces(&*super::recents::store(), &super::recents::endpoint())
            .into_iter()
            .map(|r| r.path),
    );
    if config.supported_methods().iter().any(|m| m == "onboarding/workspace_list") {
        if let Ok(v) = conv.client().request("onboarding/workspace_list", json!({"path": null})).await {
            if let Some(root) = v.get("canonical_path").and_then(|p| p.as_str()) {
                candidates.push(root.to_owned());
            }
            let folders = v.get("entries").and_then(|e| e.as_array()).cloned().unwrap_or_default();
            candidates.extend(
                folders
                    .iter()
                    .filter_map(|e| e.get("path").and_then(|p| p.as_str()).map(str::to_owned))
                    .take(MAX_HOME_FOLDERS),
            );
        }
    }
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.trim_end_matches('/').to_owned()));
    let legacy_main = format!("{profile}:main");
    // (updated_at, home) of the most recent Session with history.
    let mut recent: Option<(String, SessionHome)> = None;
    let mut main_home: Option<SessionHome> = None;
    for cwd in candidates {
        let Ok(v) = conv.client().request("session/list", json!({"cwd": cwd, "profile_id": profile})).await else {
            continue;
        };
        // Only a listing the server attests as this profile's project store.
        let Some(root) = v.get("workspace_root").and_then(|r| r.as_str()).filter(|r| !r.trim().is_empty()) else {
            continue;
        };
        if v.get("profile_id").and_then(|p| p.as_str()) != Some(profile) {
            continue;
        }
        for row in v.get("sessions").and_then(|s| s.as_array()).into_iter().flatten() {
            let Some(id) = row.get("id").and_then(|i| i.as_str()) else { continue };
            let messages = row.get("message_count").and_then(|m| m.as_u64()).unwrap_or(0) as usize;
            let home = SessionHome { session_id: id.to_owned(), root: root.to_owned(), messages };
            if want == Some(id) {
                return Some(home);
            }
            if want.is_some() || messages == 0 {
                continue;
            }
            if id == legacy_main && main_home.is_none() {
                main_home = Some(home.clone());
            }
            let updated = row.get("updated_at").and_then(|u| u.as_str()).unwrap_or("").to_owned();
            if recent.as_ref().map_or(true, |(u, _)| updated > *u) {
                recent = Some((updated, home));
            }
        }
    }
    if want.is_some() {
        return None;
    }
    main_home.or(recent.map(|(_, h)| h))
}

/// The server working directory's folders probed for a Session's store.
const MAX_HOME_FOLDERS: usize = 40;

/// The one-time migration. The previous build connected with the profile the
/// solo login ranks first (`Conversation::discover_solo_profile`, commit
/// 04c49631) and reopened `<profile>:main`. A19b — the Session it showed is
/// reopened IN the folder its history is recorded in, found the web's way
/// ([`find_session_home`]), so the very first launch after the upgrade shows
/// the whole history: the web never opens an existing Session folder-less
/// (`App.tsx:1778-1783`, `cwd: target.workspaceRoot`), and Core reads a
/// folder-less open from another store (octos-cli `runtime/cache.rs:379-384`
/// keys a Session's runtime by its store root). Nothing is created; the
/// accepted open is remembered, so every later launch restores it.
/// No profile to migrate (a fresh server, no solo login): a fresh launch. A
/// profile with no Session holding history: the web's launch, carrying that
/// profile (nothing to restore, the same profile kept). The caller resolves
/// the profile before the socket when it can (`MigrateAs`, the connection
/// then carries it); an unresolved `Migrate` resolves it here.
async fn migrate(conv: &std::sync::Arc<crate::flow::Conversation>, cwd: Option<String>) -> Started {
    let Some(profile) = crate::flow::Conversation::discover_solo_profile(&conv.http_base()).await else {
        makepad_widgets::log!("[octoscode] migration: no previous profile on this server — a fresh launch");
        return fresh(conv, cwd).await;
    };
    migrate_as(conv, profile, cwd).await
}

/// The migration for a resolved `profile` (see [`migrate`]).
async fn migrate_as(conv: &std::sync::Arc<crate::flow::Conversation>, profile: String, cwd: Option<String>) -> Started {
    conv.adopt_profile(profile.clone());
    match read_capabilities(conv).await {
        Ok(n) => makepad_widgets::log!("[octoscode] connect: {n} methods advertised"),
        Err(e) => return Started::Failed(e),
    }
    let extra: Vec<String> = cwd.iter().cloned().collect();
    let Some(home) = find_session_home(conv, &profile, None, &extra).await else {
        makepad_widgets::log!(
            "[octoscode] migration: no {profile} Session with history in the known workspaces — the launch, carrying {profile}"
        );
        return launch_in_folder(conv, cwd).await;
    };
    makepad_widgets::log!(
        "[octoscode] migration: reopening {} in {} ({} messages)",
        home.session_id,
        home.root,
        home.messages
    );
    let outcome = conv.watch_next_open();
    if let Err(e) = conv.open_session(&home.session_id, Some(home.root.clone())).await {
        return Started::Failed(e);
    }
    match tokio::time::timeout(OPEN_WAIT, outcome).await {
        Ok(Ok(Ok(id))) => Started::Migrated(id),
        Ok(Ok(Err(reason))) => {
            makepad_widgets::log!("[octoscode] migration refused ({reason}): the launch, carrying {profile}");
            launch_in_folder(conv, cwd).await
        }
        _ => Started::Failed("the migration's open was not answered yet".into()),
    }
}

/// The fresh launch (see [`startup`]).
async fn fresh(conv: &std::sync::Arc<crate::flow::Conversation>, cwd: Option<String>) -> Started {
    match read_capabilities(conv).await {
        Ok(n) => makepad_widgets::log!("[octoscode] connect: {n} methods advertised"),
        Err(e) => return Started::Failed(e),
    }
    launch_in_folder(conv, cwd).await
}

/// The launch decision for the startup folder (capabilities already read).
async fn launch_in_folder(conv: &std::sync::Arc<crate::flow::Conversation>, cwd: Option<String>) -> Started {
    let folder = match cwd {
        Some(c) => Some(c),
        None => server_working_directory(conv).await,
    };
    match folder {
        Some(f) => {
            let profile = conv.profile();
            makepad_widgets::log!(
                "[octoscode] launch at connect: {f} ({})",
                if profile.is_empty() { "no profile id".to_owned() } else { format!("profile {profile}") }
            );
            Started::Launched(create(conv, f).await)
        }
        // On a phone inside OctoSense the app works only in a folder the
        // kernel keeps for each session (the shell allows no other), so
        // there is nothing to pick.
        None if conv.is_hosted() && cfg!(any(target_os = "android", target_os = "ios", target_env = "ohos")) => {
            makepad_widgets::log!("[octoscode] launch at connect: a session in the kernel's own folder");
            match conv.new_chat(None).await {
                Ok(id) => Started::Launched(Launched::Opened(id)),
                Err(e) => Started::Failed(e),
            }
        }
        None => {
            makepad_widgets::log!("[octoscode] launch at connect: no folder reported — the workspace picker");
            for work in super::board1::route("b1.open.picker", None) {
                let _ = super::board1::execute(work, Some(conv.clone())).await;
            }
            Started::Picker
        }
    }
}

/// A19 — the launch panel owns the first-run frame: the connection is up
/// with no Session yet (a fresh server's onboarding, a cross-profile choice)
/// — the web shows its panel in the shell then (`App.tsx:2470-2490`), never
/// the Connect form, so the Connect card stays hidden behind it.
pub fn holds_first_run(store: &Store) -> bool {
    !store.keeps_shell()
        && super::board3::host::open_dialog() == Some(super::board3::host::Dialog::Launch)
        && lock().phase != Phase::Idle
}

/// `cancelLaunch`: the pending launch is dropped (its lease retired), and the
/// onboarding with it (`use-octos-session.ts:3334-3342`).
pub fn cancel() {
    {
        let mut st = lock();
        let lease = st.lease + 1;
        *st = LaunchState { lease, ..Default::default() };
    }
    crate::screens::onboarding::reset();
}

// ------------------------------------------------------------------ actions

/// Route one `b3.launch.*` action (the panel's controls).
pub fn perform(action: &str, index: usize) -> Outcome {
    let st = snapshot();
    let opening = st.phase == Phase::Opening;
    match action {
        "b3.launch.cancel" => {
            if opening {
                return Outcome::Done;
            }
            cancel();
            Outcome::Close
        }
        "b3.launch.choose" => {
            if opening || st.phase != Phase::AwaitingChoice {
                return Outcome::Done;
            }
            let Some(d) = &st.decision else { return Outcome::Done };
            let choices = choices(d);
            match choices.get(index) {
                Some(p) => Outcome::Spawn(super::board3::host::Job::LaunchChoose(p.clone())),
                None => Outcome::Done,
            }
        }
        "b3.launch.create_profile" => {
            if opening || st.phase != Phase::AwaitingChoice {
                return Outcome::Done;
            }
            Outcome::Spawn(super::board3::host::Job::LaunchCreateProfile)
        }
        _ => Outcome::Unrouted,
    }
}

/// The panel's profile buttons: the resolved profile first, then each
/// existing one (`LaunchDecisionPanel.tsx:84-111`).
pub fn choices(d: &Decision) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(p) = &d.resolved_profile {
        out.push(p.clone());
    }
    for p in &d.existing_profiles {
        if !out.contains(p) {
            out.push(p.clone());
        }
    }
    out
}

// --------------------------------------------------------------------- view

/// A17 — whether the pending decision is the server's `no_profile` (the
/// onboarding panel's decision).
pub fn is_no_profile(st: &LaunchState) -> bool {
    st.decision.as_ref().is_some_and(|d| d.decision == "no_profile")
}

fn choice_button(d: &mut Dsl, id: &str, title: &str, sub: &str, event: Option<&str>) {
    d.surface(
        &format!("{id}_box"),
        "width: Fill height: Fit flow: Overlay",
        if event.is_some() { tok::SURFACE } else { tok::SURFACE2 },
        12.0,
        Some(tok::HAIRLINE),
    );
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 3 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}");
    d.text(&format!("{id}_title"), title, &Txt::new(14.0, Face::Medium, if event.is_some() { tok::TEXT } else { tok::DISABLED_INK }).w(W::Fill).wrap());
    d.text(&format!("{id}_sub"), sub, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.close();
    if let Some(ev) = event {
        d.tap(id, ev);
    }
    d.close();
}

/// The decision panel (`LaunchDecisionPanel.tsx:52-128`), in the board-3
/// dialog style.
pub fn build(d: &mut Dsl, frame: &Frame) {
    let st = snapshot();
    // A17 — `no_profile` renders the onboarding panel in the decision's place
    // (`LaunchDecisionPanel.tsx:33-48`).
    if is_no_profile(&st) {
        crate::screens::onboarding::build(d, frame);
        return;
    }
    let width = frame.dialog_w(520.0);
    let opening = st.phase == Phase::Opening;
    ui::shell_open(d, frame, width);
    d.text("b3_launch_eyebrow", tr("Workspace launch"), &Txt::new(11.5, Face::Medium, tok::MUTED));
    let no_profile = st.decision.as_ref().is_some_and(|x| x.decision == "no_profile");
    let title = tr(if no_profile { "Set up a profile for this workspace" } else { "Choose this workspace’s profile" });
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5}");
    d.text("b3_title", title, &ui::title().w(W::Fill).wrap());
    ui::close_glyph(d, "b3.launch.cancel");
    d.close();
    d.gap(W::Fill, 6.0);
    let body = tr(if no_profile {
        "This server has no profile yet. Create the local profile, then the Session opens in this folder."
    } else {
        "This folder is known to more than one profile. Choose which profile should own the new Session."
    });
    d.text("b3_launch_body", body, &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Fill).wrap());
    d.gap(W::Fill, 8.0);
    // The folder and the choices share the body (one right edge: the body
    // keeps the scroll bar's gutter).
    ui::body_open(d, frame, width, 170.0);
    let list = d.anon();
    d.view(&list, "width: Fill height: Fit flow: Down spacing: 8");
    if let Some(cwd) = &st.cwd {
        // The web's `<code>` folder line: mono text on the grey well (a
        // label, not a field — nothing here is edited or copied).
        d.surface(
            "b3_launch_cwd_field",
            "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 10 right: 10 top: 7 bottom: 7}",
            tok::SURFACE2,
            8.0,
            Some(tok::HAIRLINE),
        );
        d.text("b3_launch_cwd", cwd, &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill).wrap());
        d.close();
    }
    if let Some(e) = &st.error {
        d.text("b3_launch_error", tr(e), &Txt::new(12.0, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
    }
    d.gap(W::Fill, 4.0);
    if no_profile {
        let ev = (!opening).then_some("b3.launch.create_profile");
        choice_button(d, "b3_launch_create", tr("Create the local profile"), tr("Then start a coding Session in this folder"), ev);
    } else if let Some(dec) = &st.decision {
        for (i, p) in choices(dec).iter().enumerate() {
            let ev = format!("b3.launch.choose#{i}");
            let (title, sub) = if Some(p) == dec.resolved_profile.as_ref() {
                (tr1("Start {value0} here", p), tr("Create this profile’s coding conversation in the folder"))
            } else {
                (format!("{} {p}", tr("Start new session with")), tr("Use this existing profile for a new Session"))
            };
            choice_button(d, &format!("b3_launch_choice_{i}"), &title, sub, (!opening).then_some(ev.as_str()));
        }
    }
    d.close();
    ui::body_close(d);
    d.gap(W::Fill, 12.0);
    let foot = d.anon();
    d.view(&foot, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    d.text(
        "b3_launch_status",
        tr(if opening { "Opening durable session…" } else { "Server decision" }),
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill),
    );
    d.button("b3_launch_cancel", tr("Cancel"), "b3.launch.cancel", if opening { Btn::Disabled } else { Btn::Outline }, W::Fit, 34.0);
    d.close();
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_lease_retires_an_older_resolver() {
        let _g = crate::screens::theme::test_lock();
        reset();
        let a = begin("/home/user/a");
        assert_eq!(snapshot().phase, Phase::Resolving);
        let b = begin("/home/user/b");
        assert!(!is_current(a) && is_current(b), "the older resolver is stale");
        cancel();
        assert!(!is_current(b), "a cancel retires the lease too");
        assert_eq!(snapshot().phase, Phase::Idle);
    }

    #[test]
    fn the_panel_offers_the_resolved_profile_first_then_the_existing_ones() {
        let d = Decision { decision: "cross_profile".into(), resolved_profile: Some("a8".into()), existing_profiles: vec!["glm".into(), "a8".into()] };
        assert_eq!(choices(&d), vec!["a8".to_owned(), "glm".to_owned()]);
    }

    #[test]
    fn the_panel_lowers_with_its_choices_and_is_inert_while_opening() {
        let _g = crate::screens::theme::test_lock();
        reset();
        begin("/home/user/octos");
        {
            let mut st = lock();
            st.phase = Phase::AwaitingChoice;
            st.decision = Some(Decision { decision: "cross_profile".into(), resolved_profile: Some("a8".into()), existing_profiles: vec!["glm".into()] });
        }
        let mut d = Dsl::new();
        build(&mut d, &Frame::DESKTOP);
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        for t in ["Choose this workspace’s profile", "Start a8 here", "Start new session with glm", "Server decision"] {
            assert!(dsl.contains(t), "{t}");
        }
        let taps = crate::screens::taps::wired_taps(&dsl);
        assert!(taps.iter().any(|(_, e)| e == "b3.launch.choose#1"));
        lock().phase = Phase::Opening;
        let mut d = Dsl::new();
        build(&mut d, &Frame::DESKTOP);
        let dsl = d.finish();
        assert!(dsl.contains("Opening durable session…"));
        assert!(!crate::screens::taps::wired_taps(&dsl).iter().any(|(_, e)| e.starts_with("b3.launch.choose")), "no choice while opening");
        assert_eq!(perform("b3.launch.cancel", 0), Outcome::Done, "cancel is inert while opening");
    }
}
