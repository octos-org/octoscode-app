//! #29b — Stage C wiring for board 2.4 Workspace picker, 2.5 Session settings,
//! 2.6 General settings (`design/stage-b/setup/cards/setup-{04,05,06}`).
//!
//! Same door rule as [`crate::bindings`] (8.8 condition 2): the cards see
//! binding ids and action ids only. This module owns the board-2 table:
//!
//! * [`query`] — store / screen-cache → the cards' data slots;
//! * [`resolve`] — an action id + item index → a pure [`Effect`];
//! * [`apply`] / [`spawn`] — the effect → protocol methods over the
//!   production client ([`Conversation::client`], the transport every live
//!   call takes), with the frame recorded in the flow's trace.
//!
//! The screen cache ([`WsState`], UI-local) holds the last read results —
//! the web keeps the same data in component state
//! (`workspace-browse-adapter.ts:21`, `SessionConfigPane.tsx:24-50`); the
//! protocol never pushes it.
//!
//! Behaviour citations (docs/parity-matrix.csv rows):
//! * 161/163 — recent picker + server cwd as the first entry
//!   (`server-working-directory.ts:2`, fallback "path not reported");
//! * 162/165 — explicit-path open via `session/open` cwd
//!   (`NewSessionWorkspacePicker.tsx:73`) and folder create via
//!   `onboarding/workspace_create`, name pre-validated exactly like the web's
//!   `validateWorkspaceFolderName` (`workspace-browse.ts:160-175`);
//! * 164/166 — `onboarding.workspace_browse.v1` is advertised-gated, fail
//!   closed (`workspace-browse-adapter.ts:25`);
//! * 100/101 — `profile/llm/select` and its six runtime dispositions, each
//!   mapped to the web's exact user message (`SessionConfigPane.tsx:63-90`);
//! * 102 — `permission/profile/set` with the picked segment's mode
//!   (`permissions-section.tsx:16,71`);
//! * 103 — sandbox read-only from the fixed-at-open runtime stamp, feature
//!   absent → "Not supported by this server" (`sandbox-section.tsx:4,33`).
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use serde_json::{json, Value};

use crate::bindings::Ctx;
use crate::flow::{Conversation, Direction};
use crate::screens::recents;

/// The board-2 data slots (setup cards 04/05/06).
pub const BINDINGS: &[(&str, &str)] = &[
    ("ws.folder", "text: the picker's folder field (env OCTOS_WORKSPACE_CWD, else the server root)"),
    ("ws.server_folder", "text: the row's fixed title 'Server folder' (atlas); '(path not reported)' appended when the server did not report one — server-working-directory.ts:2-23"),
    ("ws.server_folder_path", "text: the row's subtitle — the server's working directory with $HOME shown as '~' (server-working-directory.ts:2-23 projects label + path)"),
    ("ws.recent", "list: [{name,path}] the remembered workspaces (the last onboarding/workspace_list)"),
    ("ws.browse_visible", "bool: the Browse affordance gate — only when the session advertised onboarding.workspace_browse.v1 (fail closed, workspace-browse-adapter.ts:25)"),
    ("set.model", "text: the profile's current model (profile/llm/list primary)"),
    ("set.permission_mode", "text: the current permission profile mode (permission/profile/list)"),
    ("set.sandbox", "text: the one-run sandbox line from the fixed-at-open stamp, or 'Not supported by this server' (sandbox-section.tsx:4)"),
    ("set.conn_state", "text: the General-settings connection row (store)"),
    ("set.workspace", "text: the General-settings workspace row"),
    ("set.profile", "text: the active profile id"),
    ("set.save_notice", "text: the last model-select disposition notice (SessionConfigPane.tsx:63-90)"),
];

/// The board-2 action ids the setup cards emit.
pub const ACTIONS: &[(&str, &str)] = &[
    ("ws.refresh", "re-read onboarding/workspace_list + permission/profile/list + profile/llm/list"),
    ("ws.open", "session/open at the clicked row's path (index into ws.recent)"),
    ("ws.browse", "onboarding.workspace_browse.v1 at the current path (advertised-gated)"),
    ("ws.create_folder", "onboarding/workspace_create with the picker field's (draft) name under the server root"),
    ("set.model.select", "profile/llm/select for the cached primary route"),
    ("set.permission.set", "permission/profile/set with the clicked segment's mode (index into the cached profiles)"),
    ("set.permission.cycle", "#P4a1: the composer approval pill — cycle the mode via permission/profile/set"),
    ("set.diagnostics.copy", "copy the diagnostics (UI-local; the host owns the clipboard)"),
    // #40b — setup-11's own id for the same control; the #35b wired taps and
    // the click audit emit THIS string, so the alias must pass is_action or
    // the tap dies as Unhandled before resolve ever sees it (the audit's
    // last dead row was exactly that).
    ("error.copy_diagnostics", "alias of set.diagnostics.copy (setup-11's btn_diag)"),
    ("settings.close", "close the settings pane (UI-local until #28e mounts the overlay)"),
];

/// The action ids [`resolve`] routes.
pub const ROUTED: &[&str] = &[
    "ws.refresh",
    "ws.open",
    "ws.browse",
    "ws.create_folder",
    "set.model.select",
    "set.permission.set",
    "set.permission.cycle",
    "set.diagnostics.copy",
    "error.copy_diagnostics",
    "settings.close",
];

pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

pub fn is_routed(id: &str) -> bool {
    ROUTED.contains(&id)
}

/// Declared but not routed by this router (kept for the coverage contract).
pub fn unrouted() -> Vec<&'static str> {
    ACTIONS
        .iter()
        .map(|(a, _)| *a)
        .filter(|a| !ROUTED.contains(a))
        .collect()
}

/// The screen's UI-local cache: the last read results (the web keeps the same
/// in component state; the protocol never pushes it).
#[derive(Default)]
pub struct WsState {
    pub server_root: Option<String>,
    pub entries: Vec<Value>,
    pub model: Option<String>,
    pub family: Option<String>,
    pub route_id: Option<String>,
    pub permission_mode: Option<String>,
    pub profiles: Vec<Value>,
    pub sandbox: Option<String>,
    pub save_notice: Option<String>,
    pub profile_id: Option<String>,
}

fn state() -> MutexGuard<'static, WsState> {
    static STATE: OnceLock<Mutex<WsState>> = OnceLock::new();
    STATE
        .get_or_init(|| Mutex::new(WsState::default()))
        .lock()
        .unwrap()
}

/// A10 — the permission seat's menu read a profile list or applied a set:
/// the composer pill (`set.permission_mode`) follows the read-back.
pub fn note_permission_mode(mode: Option<&str>) {
    state().permission_mode = mode.map(str::to_owned);
    makepad_widgets::SignalToUI::set_ui_signal();
}

/// The concrete thing the module does for a routed board-2 action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Refresh,
    /// `session/open` carrying the picked workspace's path.
    Open(String),
    /// `onboarding.workspace_browse.v1` at `path` (already gated in resolve).
    Browse(String),
    /// `onboarding/workspace_create` under `parent`.
    CreateFolder { parent: String, name: String },
    SelectModel,
    /// `permission/profile/set` with the picked mode.
    SetPermission(String),
    CyclePermissionMode,
    CopyDiagnostics,
    Close,
    Unhandled(String),
}

/// Route one board-2 action. Pure — no transport, no window. Never panics: an
/// id with no target is [`Effect::Unhandled`] (logged by name, LESSONS 6).
pub fn resolve(action: &str, index: usize, ctx: &Ctx<'_>) -> Effect {
    let st = state();
    match action {
        "ws.refresh" => Effect::Refresh,
        "ws.open" => match st.entries.get(index).and_then(|e| e["path"].as_str()) {
            Some(path) => Effect::Open(path.to_owned()),
            None => Effect::Unhandled(format!("{action}[{index}]")),
        },
        // Parity 166: the affordance exists ONLY when the session advertised
        // the feature (fail closed, workspace-browse-adapter.ts:25).
        "ws.browse" => {
            if !ctx
                .store
                .capabilities()
                .iter()
                .any(|c| c == "onboarding.workspace_browse.v1")
            {
                return Effect::Unhandled("ws.browse[not-advertised]".to_owned());
            }
            match st.server_root.clone() {
                Some(root) => Effect::Browse(root),
                None => Effect::Unhandled("ws.browse[no-root]".to_owned()),
            }
        }
        "ws.create_folder" => {
            // The picker's field text rides the flow's draft (the only text
            // input the wired cards own until #28e mounts real containers).
            let name = ctx.ui.lock().unwrap().draft().trim().to_owned();
            match (st.server_root.clone(), name.is_empty()) {
                (Some(parent), false) => Effect::CreateFolder { parent, name },
                (parent, _) => Effect::Unhandled(format!("{action}[parent={parent:?}]")),
            }
        }
        "set.model.select" => Effect::SelectModel,
        "set.permission.set" => match st.profiles.get(index).and_then(|p| p["mode"].as_str()) {
            Some(mode) => Effect::SetPermission(mode.to_owned()),
            None => Effect::Unhandled(format!("{action}[{index}]")),
        },
        "set.permission.cycle" => Effect::CyclePermissionMode,
        "set.diagnostics.copy" | "error.copy_diagnostics" => Effect::CopyDiagnostics,
        "settings.close" => Effect::Close,
        other => Effect::Unhandled(other.to_owned()),
    }
}

/// Perform the effect against the production client (async; spawn from the UI
/// thread, await from tests/replays).
pub async fn apply(effect: Effect, conv: &Conversation) -> Result<(), String> {
    match effect {
        Effect::Refresh => refresh(conv).await,
        Effect::Open(cwd) => {
            // #P4h1 row 305: the web remembers a workspace at the OPEN, not at
            // a picker render (`App.tsx:1031` — `rememberWorkspace(storage,
            // connection.endpoint, cwd)` on the open path). Recording it here
            // is what makes the cache a production path rather than a helper
            // only tests call (RULES 3). It is a navigation cache, so it is
            // written AFTER the open succeeds — a failed open must not leave a
            // row the user never reached — and a write that fails never fails
            // the open (the web catches the storage throw, and
            // `remember_workspace` returns the new list either way).
            conv.open_workspace(Some(cwd.clone())).await?;
            recents::remember_workspace(
                &*recents::store(),
                &recents::endpoint(),
                &cwd,
                recents::now_ms(),
            );
            Ok(())
        }
        Effect::Browse(path) => {
            let result = conv
                .client()
                .request("onboarding.workspace_browse.v1", json!({"path": path}))
                .await
                .map_err(|e| e.to_string())?;
            conv.trace.record(
                Instant::now(),
                Direction::Out,
                "onboarding.workspace_browse.v1",
                None,
                Some(format!("entries={}", result["entries"].as_array().map(Vec::len).unwrap_or(0))),
            );
            Ok(())
        }
        Effect::CreateFolder { parent, name } => create_folder(conv, &parent, &name).await,
        Effect::SelectModel => select_model(conv).await,
        Effect::SetPermission(mode) => set_permission(conv, &mode).await,
        Effect::CyclePermissionMode => {
            // #P4a1 — the composer pill: cycle the web's two settings-app
            // modes; set_permission sends the same permission/profile/set the
            // web sends and stores the reply's `current` read-back.
            let next = next_permission_mode(state().permission_mode.as_deref());
            set_permission(conv, next).await
        }
        // UI-local: the host owns the clipboard; close belongs to #28e's
        // overlay container. No protocol method either way.
        Effect::CopyDiagnostics | Effect::Close => Ok(()),
        Effect::Unhandled(id) => {
            ::log::warn!("octoscode: unhandled screen action {id:?}");
            Ok(())
        }
    }
}

/// Spawn the effect on the module's runtime (the UI-thread entry).
pub fn spawn(effect: Effect, rt: &tokio::runtime::Runtime, conv: Arc<Conversation>) {
    rt.spawn(async move {
        if let Err(e) = apply(effect, &conv).await {
            ::log::warn!("octoscode: screen action: {e}");
        }
    });
}

/// The picker's read set: workspace list + permission profile + llm list.
async fn refresh(conv: &Conversation) -> Result<(), String> {
    let client = conv.client();
    let list = client
        .request("onboarding/workspace_list", json!({}))
        .await
        .map_err(|e| e.to_string())?;
    conv.trace.record(
        Instant::now(),
        Direction::Out,
        "onboarding/workspace_list",
        None,
        Some(format!("entries={}", list["entries"].as_array().map(Vec::len).unwrap_or(0))),
    );
    {
        let mut st = state();
        // server-working-directory.ts:2 — the canonical path is the first
        // entry's label source; "path not reported" when absent.
        st.server_root = list["canonical_path"].as_str().map(str::to_owned);
        st.entries = list["entries"].as_array().cloned().unwrap_or_default();
    }
    let perms = client
        .request(
            "permission/profile/list",
            json!({"session_id": conv.session_id()}),
        )
        .await
        .map_err(|e| e.to_string())?;
    conv.trace.record(
        Instant::now(),
        Direction::Out,
        "permission/profile/list",
        None,
        None,
    );
    {
        let mut st = state();
        st.permission_mode = perms["current"]["mode"].as_str().map(str::to_owned);
        st.profiles = perms["profiles"].as_array().cloned().unwrap_or_default();
    }
    // A10 — the same read feeds the store the permission seat reads (its
    // label is the web's trigger, "{mode} · {network}").
    if perms["session_id"].as_str() == Some(conv.session_id().as_str()) {
        let sel = |v: &Value| serde_json::from_value::<octoscode_store::domains::profile::PermissionProfileSelection>(v.clone()).ok();
        if let Some(current) = sel(&perms["current"]) {
            let profiles = perms["profiles"].as_array().map(|a| a.iter().filter_map(sel).collect()).unwrap_or_default();
            conv.store.domains.profile.set_permission(current, profiles);
        }
    }
    let profile = conv.profile().to_owned();
    let llms = client
        .request("profile/llm/list", json!({"profile_id": profile}))
        .await
        .map_err(|e| e.to_string())?;
    conv.trace.record(
        Instant::now(),
        Direction::Out,
        "profile/llm/list",
        None,
        None,
    );
    let mut st = state();
    st.model = llms["llm"]["primary"]["model_id"].as_str().map(str::to_owned);
    st.family = llms["llm"]["primary"]["family_id"].as_str().map(str::to_owned);
    st.route_id = llms["llm"]["primary"]["route_id"].as_str().map(str::to_owned);
    st.profile_id = Some(profile);
    Ok(())
}

/// `onboarding/workspace_create` with the web's exact pre-validation
/// (workspace-browse.ts:160-175) — never send a name the server would refuse.
async fn create_folder(conv: &Conversation, parent: &str, name: &str) -> Result<(), String> {
    let problem = if name.is_empty() {
        "empty"
    } else if name.contains('/') || name.contains('\\') {
        "separator"
    } else if name == "." || name == ".." {
        "relative"
    } else if name
        .chars()
        .any(|c| c.is_control() || c == '\u{7f}')
    {
        "control"
    } else if name.trim() != name {
        "surrounding_whitespace"
    } else {
        ""
    };
    if !problem.is_empty() {
        return Err(format!("folder name rejected before any request: {problem}"));
    }
    let result = conv
        .client()
        .request("onboarding/workspace_create", json!({"name": name, "parent": parent}))
        .await
        .map_err(|e| e.to_string())?;
    conv.trace.record(
        Instant::now(),
        Direction::Out,
        "onboarding/workspace_create",
        None,
        result["canonical_path"].as_str().map(str::to_owned),
    );
    Ok(())
}

/// `profile/llm/select`, then map `runtime_disposition` to the web's exact
/// user message (SessionConfigPane.tsx:63-90) and cache the sandbox line from
/// the fixed-at-open stamp (sandbox-section.tsx:23-80).
async fn select_model(conv: &Conversation) -> Result<(), String> {
    let (family, model, route_id, profile) = {
        let st = state();
        (
            st.family.clone(),
            st.model.clone(),
            st.route_id.clone(),
            st.profile_id.clone(),
        )
    };
    let Some(model) = model else {
        return Err("no model cached; run ws.refresh first".to_owned());
    };
    let result = conv
        .client()
        .request(
            "profile/llm/select",
            json!({
                "family_id": family,
                "model_id": model,
                "profile_id": profile,
                "route_id": route_id,
                "session_id": conv.session_id(),
            }),
        )
        .await
        .map_err(|e| e.to_string())?;
    conv.trace.record(
        Instant::now(),
        Direction::Out,
        "profile/llm/select",
        None,
        result["runtime_disposition"].as_str().map(str::to_owned),
    );
    let disposition = result["runtime_disposition"].as_str().unwrap_or("");
    let notice = match disposition {
        "reloaded" => format!(
            "Saved. Your next message uses {}",
            result["selected"]["model"].as_str().unwrap_or("the new model")
        ),
        "deferred" => "Saved. The model is not active yet".to_owned(),
        "restart_required" => format!(
            "Saved. The server keeps running {} until it restarts",
            result["runtime_policy_stamp"]["model"]
                .as_str()
                .unwrap_or("the previous model")
        ),
        "persisted_but_not_live" => format!(
            "Saved, but not usable right now: {}",
            result["runtime_error"]
                .as_str()
                .unwrap_or("the runtime could not start")
        ),
        "unchanged" => "Already selected".to_owned(),
        "refused" => format!(
            "Couldn't save: {}",
            result["reason"]
                .as_str()
                .unwrap_or("the server refused the change")
        ),
        other => {
            // Fail closed on an unknown disposition rather than guessing copy.
            return Err(format!("unknown runtime_disposition {other:?}"));
        }
    };
    let mut st = state();
    st.save_notice = Some(notice);
    if let Some(stamp) = result.get("runtime_policy_stamp") {
        st.sandbox = Some(format!(
            "Enabled · Network {} · {}",
            stamp["network"].as_str().unwrap_or("unknown"),
            stamp["filesystem_scope"].as_str().unwrap_or("unknown"),
        ));
    }
    Ok(())
}

/// #P4a1 — the pill's cycle rule, pure so it is testable: the web's
/// settings app offers two modes (on-request maps to `workspace_write`,
/// never-ask to `read_only` — permissions-section.tsx:17-18); a pill with no
/// known mode starts the cycle at read_only. Unknown values re-enter safely.
fn next_permission_mode(current: Option<&str>) -> &'static str {
    match current {
        Some("read_only") => "workspace_write",
        _ => "read_only",
    }
}

/// `permission/profile/set` with the picked mode; the reply's `current` is the
/// read-back (permissions-section.test.tsx:51 parity).
async fn set_permission(conv: &Conversation, mode: &str) -> Result<(), String> {
    let result = conv
        .client()
        .request(
            "permission/profile/set",
            json!({"session_id": conv.session_id(), "update": {"mode": mode}}),
        )
        .await
        .map_err(|e| e.to_string())?;
    conv.trace.record(
        Instant::now(),
        Direction::Out,
        "permission/profile/set",
        None,
        Some(mode.to_owned()),
    );
    state().permission_mode = result["current"]["mode"].as_str().map(str::to_owned);
    // #P4a1 — the read-back must REACH the pill: wake the UI thread so the
    // composer re-lowers with the fresh `set.permission_mode` binding (an
    // async arm on the tokio thread never repaints by itself).
    makepad_widgets::SignalToUI::set_ui_signal();
    Ok(())
}

/// Resolve a board-2 binding id. `None` when undeclared. Always JSON.
pub fn query(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    let store = ctx.store;
    let st = state();
    Some(match id {
        // #29b2: the title stays the atlas copy; only the SUBTITLE carries the
        // path (server-working-directory.ts:2-23 projects label + path).
        "ws.folder" => json!(
            std::env::var("OCTOS_WORKSPACE_CWD").ok().or_else(|| st.server_root.clone()).unwrap_or_default()
        ),
        "ws.server_folder" => json!(match st.server_root.as_deref() {
            Some(_) => "Server folder",
            None => "Server folder (path not reported)",
        }),
        "ws.server_folder_path" => json!(st
            .server_root
            .as_deref()
            .map(|p| abbreviate_home(p, home_dir()).unwrap_or_else(|| p.to_owned()))
            .unwrap_or_default()),
        // #29b2: the General-settings row shows the workspace NAME (the web's
        // display label renders workspace.name, NewSessionWorkspacePicker.tsx:242)
        // ellipsized so it keeps a gap before the row chevron.
        "set.workspace" => {
            let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();
            json!(st
                .server_root
                .as_deref()
                .or(cwd.as_deref())
                .map(workspace_display)
                .unwrap_or_default())
        }
        "ws.recent" => json!(st
            .entries
            .iter()
            .map(|e| json!({"name": e["name"], "path": e["path"]}))
            .collect::<Vec<Value>>()),
        "ws.browse_visible" => json!(store
            .capabilities()
            .iter()
            .any(|c| c == "onboarding.workspace_browse.v1")),
        "set.model" => json!(st.model),
        "set.permission_mode" => json!(st.permission_mode),
        // sandbox-section.tsx:4 — feature absent → the exact fallback copy.
        "set.sandbox" => json!(
            st.sandbox.clone().unwrap_or_else(|| "Not supported by this server".to_owned())
        ),
        "set.conn_state" => json!(store.connection()),
        "set.profile" => json!(st.profile_id),
        "set.save_notice" => json!(st.save_notice),
        _ => return None,
    })
}

/// The HOME dir for `~` abbreviation (test seam: None disables it).
fn home_dir() -> Option<String> {
    std::env::var("HOME").ok().filter(|h| !h.is_empty())
}

/// Show a leading `$HOME` as `~` (the entry's display rule). Pure: a path
/// outside $HOME passes through unchanged (Some), `home = None` disables.
fn abbreviate_home(path: &str, home: Option<String>) -> Option<String> {
    let home = home?;
    if path == home {
        return Some("~".to_owned());
    }
    let prefix = format!("{home}/");
    Some(
        path.strip_prefix(&prefix)
            .map(|rest| format!("~/{rest}"))
            .unwrap_or_else(|| path.to_owned()),
    )
}

/// The workspace's display NAME: the path's basename, ellipsized past 24
/// chars (the web renders `workspace.name`, NewSessionWorkspacePicker.tsx:242).
fn workspace_display(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if name.chars().count() > 24 {
        let head: String = name.chars().take(23).collect();
        format!("{head}…")
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviate_home_rewrites_only_the_home_prefix() {
        assert_eq!(
            abbreviate_home("/tmp/ws29b", Some("/tmp".into())).as_deref(),
            Some("~/ws29b")
        );
        assert_eq!(
            abbreviate_home("/tmp/ws29b", Some("/srv/demo-home".into())).as_deref(),
            Some("/tmp/ws29b"),
            "an unrelated prefix is untouched"
        );
        assert_eq!(abbreviate_home("/tmp", Some("/tmp".into())).as_deref(), Some("~"));
        assert_eq!(abbreviate_home("/tmp/ws29b", None), None);
    }

    #[test]
    fn workspace_display_is_the_basename_ellipsized() {
        assert_eq!(workspace_display("/tmp/ws29b"), "ws29b");
        assert_eq!(workspace_display("/tmp/ws29b/"), "ws29b");
        let long = format!("/tmp/{}", "a-very-long-workspace-name-over-24");
        let shown = workspace_display(&long);
        assert_eq!(shown.chars().count(), 24);
        assert!(shown.ends_with('…'), "{shown}");
    }

    #[test]
    fn the_pill_cycle_rule_matches_the_web() {
        // #P4a1 — no known mode starts the cycle at read_only; the two
        // settings-app modes flip onto each other; an unknown value
        // re-enters safely.
        assert_eq!(next_permission_mode(None), "read_only");
        assert_eq!(next_permission_mode(Some("read_only")), "workspace_write");
        assert_eq!(next_permission_mode(Some("workspace_write")), "read_only");
        assert_eq!(next_permission_mode(Some("on_request")), "read_only");
    }

    #[test]
    fn the_pill_action_is_declared_and_routes() {
        // #P4a1 — the cycle action sits in the wired tables (so
        // perform_action routes it) and resolves to its effect. Before this
        // card the id did not exist: resolve returned Unhandled.
        use crate::bindings::{self, Ctx};
        use crate::flow::FlowUi;
        use octoscode_store::Store;
        use std::sync::{Arc, Mutex};
        assert!(is_action("set.permission.cycle"));
        assert!(is_routed("set.permission.cycle"));
        let store = Arc::new(Store::new());
        let ui = Arc::new(Mutex::new(FlowUi::default()));
        let ctx = Ctx::new(&store, &ui);
        assert!(matches!(
            resolve("set.permission.cycle", 0, &ctx),
            Effect::CyclePermissionMode
        ));
        assert!(matches!(
            resolve("set.permission.nope", 0, &ctx),
            Effect::Unhandled(_)
        ));
    }
}
