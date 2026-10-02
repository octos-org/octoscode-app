//! A8 — the NEW-SESSION defaults, ported from the web's
//! `features/session-config/session-defaults.ts` (+ the creation-only apply in
//! `app/App.tsx:1913-1975` `openSessionWithDefaults` / `applyPermissionDefault`).
//!
//! The web's contract (`session-defaults.ts:4-9`): the permission mode, the
//! network policy and the sandbox are THIS CLIENT's preference, stored per
//! endpoint, and applied at CREATION only — the sandbox rides `session/open`,
//! the permission mode + network policy go out as ONE `permission/profile/set`
//! right after the created session opened. Re-opening an existing session
//! (resume, switch, a sidebar row) NEVER re-applies them, and a failed apply is
//! surfaced ("Couldn't apply the new-session permission default"), never
//! swallowed and never retried on a re-open.
//!
//! Natively the per-endpoint `localStorage` key is a file under the app's own
//! config dir through the recents [`Storage`] seam (`screens/recents.rs`), with
//! the web's key (`octoscode-web.session-defaults.v1:<endpoint>`, the endpoint
//! percent-encoded so the key is one safe path segment) and the web's stored
//! JSON shape (`version: 1`, camelCase field names) — a corrupt or unknown
//! value fails closed to "no defaults" exactly as `parse` does.
//!
//! Settings > Sandbox (A3's chrome, board 2 screen 9) edits these defaults:
//! "Workspace write" is the permission mode (on = `workspace_write`, off =
//! `read_only`), "Network access" the network policy (and the sandbox's
//! network access), "Sandbox" the sandbox switch.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::screens::recents::Storage;

/// The web's `DEFAULTS_PREFERENCES_KEY` (`session-defaults.ts:11`).
pub const DEFAULTS_PREFERENCES_KEY: &str = "octoscode-web.session-defaults.v1";
/// `MAX_READ_ALLOW_PATHS` (`:13`).
pub const MAX_READ_ALLOW_PATHS: usize = 16;
/// `MAX_PATH_LENGTH` (`:14`).
pub const MAX_PATH_LENGTH: usize = 4_096;
/// The sandbox feature the open's `sandbox` param needs
/// (octos-core `ui_protocol.rs:110` `UI_PROTOCOL_FEATURE_SESSION_SANDBOX_V1`).
pub const SANDBOX_FEATURE: &str = "session.sandbox.v1";
/// The web's surfaced failure (`App.tsx:1949`).
pub const APPLY_FAILED: &str = "Couldn't apply the new-session permission default";

/// `DefaultsPermissionMode` (`:16-17`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl PermissionMode {
    pub fn wire(self) -> &'static str {
        match self {
            PermissionMode::ReadOnly => "read_only",
            PermissionMode::WorkspaceWrite => "workspace_write",
            PermissionMode::DangerFullAccess => "danger_full_access",
        }
    }
    /// `MODES.includes` — an unknown mode is rejected, never guessed
    /// (`session-defaults.test.ts:46`).
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "read_only" => PermissionMode::ReadOnly,
            "workspace_write" => PermissionMode::WorkspaceWrite,
            "danger_full_access" => PermissionMode::DangerFullAccess,
            _ => return None,
        })
    }
    /// The Settings label (`SettingsDefaultsSection.tsx:27-31`).
    pub fn label(self) -> &'static str {
        match self {
            PermissionMode::ReadOnly => "Read only",
            PermissionMode::WorkspaceWrite => "Workspace write",
            PermissionMode::DangerFullAccess => "Full access",
        }
    }
}

/// `DefaultsNetworkPolicy` (`:19`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkPolicy {
    Deny,
    Allow,
}

impl NetworkPolicy {
    pub fn wire(self) -> &'static str {
        match self {
            NetworkPolicy::Deny => "deny",
            NetworkPolicy::Allow => "allow",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "deny" => NetworkPolicy::Deny,
            "allow" => NetworkPolicy::Allow,
            _ => return None,
        })
    }
}

/// `DefaultsSandbox` (`:22-26`) — the narrowed `session/open` sandbox params.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SandboxDefaults {
    pub enabled: bool,
    pub network_access: bool,
    pub read_allow_paths: Vec<String>,
}

/// `SessionDefaults` (`:28-32`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDefaults {
    pub permission_mode: PermissionMode,
    pub network: NetworkPolicy,
    pub sandbox: SandboxDefaults,
}

impl Default for SessionDefaults {
    /// The Settings fallback when nothing is stored
    /// (`SettingsDefaultsSection.tsx:41-45`): workspace write, network deny,
    /// sandbox off.
    fn default() -> Self {
        Self {
            permission_mode: PermissionMode::WorkspaceWrite,
            network: NetworkPolicy::Deny,
            sandbox: SandboxDefaults::default(),
        }
    }
}

/// `encodeURIComponent` — the endpoint becomes one safe key segment (the
/// recents cache does the same, `recents.rs` `storage_key`).
fn encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        let c = *byte as char;
        if c.is_ascii_alphanumeric() || "-_.!~*'()".contains(c) {
            out.push(c);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `scopedKey(endpoint)` (`:52-57`): endpoint-scoped, so one server's defaults
/// never bleed into another's (`session-defaults.test.ts:32`).
pub fn scoped_key(endpoint: &str) -> String {
    let origin = endpoint.trim();
    if origin.is_empty() {
        DEFAULTS_PREFERENCES_KEY.to_owned()
    } else {
        format!("{DEFAULTS_PREFERENCES_KEY}:{}", encode(origin))
    }
}

/// `sanitizePath` (`:59-63`).
fn sanitize_path(value: &Value) -> Option<String> {
    let s = value.as_str()?.trim();
    (!s.is_empty() && s.chars().count() <= MAX_PATH_LENGTH).then(|| s.to_owned())
}

/// `parse` (`:65-101`): `None` for anything missing, corrupt or unknown.
pub fn parse(raw: Option<&str>) -> Option<SessionDefaults> {
    let v: Value = serde_json::from_str(raw?).ok()?;
    let o = v.as_object()?;
    if o.get("version")?.as_u64()? != 1 {
        return None;
    }
    let permission_mode = PermissionMode::parse(o.get("permissionMode")?.as_str()?)?;
    let network = NetworkPolicy::parse(o.get("network")?.as_str()?)?;
    let sb = o.get("sandbox")?.as_object()?;
    let enabled = sb.get("enabled")?.as_bool()?;
    let network_access = sb.get("networkAccess")?.as_bool()?;
    let read_allow_paths = sb
        .get("readAllowPaths")?
        .as_array()?
        .iter()
        .take(MAX_READ_ALLOW_PATHS)
        .filter_map(sanitize_path)
        .collect();
    Some(SessionDefaults {
        permission_mode,
        network,
        sandbox: SandboxDefaults { enabled, network_access, read_allow_paths },
    })
}

/// The stored JSON (`StoredDefaults`, `:34-43`; bounded like `saveSessionDefaults`).
pub fn to_stored(value: &SessionDefaults) -> Value {
    let paths: Vec<String> = value
        .sandbox
        .read_allow_paths
        .iter()
        .take(MAX_READ_ALLOW_PATHS)
        .map(|p| p.trim().chars().take(MAX_PATH_LENGTH).collect::<String>())
        .filter(|p| !p.is_empty())
        .collect();
    json!({
        "version": 1,
        "permissionMode": value.permission_mode.wire(),
        "network": value.network.wire(),
        "sandbox": {
            "enabled": value.sandbox.enabled,
            "networkAccess": value.sandbox.network_access,
            "readAllowPaths": paths,
        }
    })
}

/// `loadSessionDefaults` (`:104-109`).
pub fn load(storage: &dyn Storage, endpoint: &str) -> Option<SessionDefaults> {
    parse(storage.get_item(&scoped_key(endpoint)).as_deref())
}

/// `saveSessionDefaults` (`:112-135`).
pub fn save(value: &SessionDefaults, storage: &dyn Storage, endpoint: &str) -> Result<(), String> {
    storage.set_item(&scoped_key(endpoint), &to_stored(value).to_string())
}

// ------------------------------------------------- the running app's copy

/// The defaults the running app edits and applies: the stored value (or the
/// fallback when none is stored yet) and whether anything is stored — the web
/// applies a permission default only when `defaults` is non-null
/// (`App.tsx:1936-1940`).
#[derive(Debug, Clone, Default)]
pub struct Live {
    pub value: SessionDefaults,
    pub stored: bool,
    /// The endpoint the value was loaded for (re-loaded when it changes).
    pub endpoint: String,
    /// The last save's failure, surfaced in Settings (never silent).
    pub save_error: Option<String>,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

/// The storage the defaults live in: the recents cache's file store in the
/// app (`$HOME/.octoscode/workspace-recents`, or `OCTOSCODE_RECENTS_DIR`), an
/// injected store in tests (unit tests default to memory, never a real home).
static STORAGE: Mutex<Option<Arc<dyn Storage>>> = Mutex::new(None);

/// Install the store (tests; the app keeps the recents file store).
pub fn set_storage(storage: Arc<dyn Storage>) {
    *STORAGE.lock().unwrap_or_else(|p| p.into_inner()) = Some(storage);
    reset_cache();
}

fn storage() -> Arc<dyn Storage> {
    let mut g = STORAGE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(s) = g.as_ref() {
        return s.clone();
    }
    if cfg!(test) {
        let m: Arc<dyn Storage> = Arc::new(crate::screens::recents::MemoryStore::new());
        *g = Some(m.clone());
        return m;
    }
    crate::screens::recents::store()
}

/// The defaults for the CONNECTED endpoint (loaded on first use and whenever
/// the endpoint changes).
pub fn current() -> Live {
    let endpoint = crate::screens::recents::endpoint();
    let mut g = LIVE.lock().unwrap_or_else(|p| p.into_inner());
    let stale = g.as_ref().map(|l| l.endpoint != endpoint).unwrap_or(true);
    if stale {
        let loaded = load(&*storage(), &endpoint);
        *g = Some(Live {
            stored: loaded.is_some(),
            value: loaded.unwrap_or_default(),
            endpoint: endpoint.clone(),
            save_error: None,
        });
    }
    g.clone().unwrap_or_default()
}

/// Edit the defaults and persist them for the connected endpoint
/// (`App.tsx:3538-3546` `onDefaultsChange` → `saveSessionDefaults`).
pub fn update(edit: impl FnOnce(&mut SessionDefaults)) -> Live {
    let mut live = current();
    edit(&mut live.value);
    // The web disables "Sandbox network access" unless the sandbox is on
    // (`SettingsDefaultsSection.tsx:103`); the network switch here drives
    // both the policy and the sandbox's network access.
    live.value.sandbox.network_access = live.value.network == NetworkPolicy::Allow;
    live.save_error = save(&live.value, &*storage(), &live.endpoint).err();
    if let Some(e) = &live.save_error {
        ::log::warn!("octoscode: new-session defaults not saved: {e}");
    }
    live.stored = live.save_error.is_none() || live.stored;
    *LIVE.lock().unwrap_or_else(|p| p.into_inner()) = Some(live.clone());
    live
}

/// Test seam: forget the loaded copy (the next [`current`] re-reads storage).
pub fn reset_cache() {
    *LIVE.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

// ------------------------------------------------- creation-only apply

/// The `session/open` sandbox param for a CREATED session
/// (`App.tsx:1922-1932`): present only when the defaults turn the sandbox on.
/// Natively it is also gated on the server advertising `session.sandbox.v1`
/// (fail closed: an unadvertised param is never sent; the pane's Sandbox
/// section says "Not supported by this server" for the same server).
pub fn open_sandbox(
    defaults: &SessionDefaults,
    advertised: bool,
) -> Option<octos_core::ui_protocol::SessionSandboxParams> {
    (defaults.sandbox.enabled && advertised).then(|| octos_core::ui_protocol::SessionSandboxParams {
        enabled: Some(true),
        network_access: Some(defaults.sandbox.network_access),
        read_allow_paths: defaults.sandbox.read_allow_paths.clone(),
    })
}

/// The ONE `permission/profile/set` after creation (`App.tsx:1956-1975`): the
/// CREATED session id, never the previously selected one.
pub fn permission_params(session_id: &str, defaults: &SessionDefaults) -> Value {
    json!({
        "session_id": session_id,
        "update": {
            "mode": defaults.permission_mode.wire(),
            "network": defaults.network.wire(),
        }
    })
}

/// The per-created-id marker (`appliedDefaultsForSession`, `App.tsx:649`):
/// `new_chat` arms one pending apply per FRESH id; the open reply for exactly
/// that id takes it (once). A re-open never arms one, so it can never apply.
#[derive(Default)]
pub struct Pending {
    armed: Mutex<HashMap<String, SessionDefaults>>,
    applied: Mutex<Vec<String>>,
}

impl Pending {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    /// Arm the creation-time apply for a freshly minted id.
    pub fn arm(&self, session_id: &str, defaults: SessionDefaults) {
        self.armed.lock().unwrap().insert(session_id.to_owned(), defaults);
    }
    /// Take the armed defaults for an opened id — at most once per id.
    pub fn take(&self, session_id: &str) -> Option<SessionDefaults> {
        let d = self.armed.lock().unwrap().remove(session_id)?;
        let mut applied = self.applied.lock().unwrap();
        if applied.iter().any(|s| s == session_id) {
            return None;
        }
        applied.push(session_id.to_owned());
        Some(d)
    }
    pub fn applied(&self) -> Vec<String> {
        self.applied.lock().unwrap().clone()
    }
}

/// The last creation-time apply failure, shown in the Session settings pane
/// (the web passes `permissionDefaultError` as the pane's notice,
/// `App.tsx:3270-3277`).
static APPLY_ERROR: Mutex<Option<String>> = Mutex::new(None);

pub fn note_apply_result(result: Result<(), String>) {
    let mut g = APPLY_ERROR.lock().unwrap_or_else(|p| p.into_inner());
    match result {
        Ok(()) => *g = None,
        Err(e) => {
            ::log::warn!("octoscode: {APPLY_FAILED}: {e}");
            *g = Some(APPLY_FAILED.to_owned());
        }
    }
}

pub fn apply_error() -> Option<String> {
    APPLY_ERROR.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::recents::MemoryStore;

    fn sample() -> SessionDefaults {
        SessionDefaults {
            permission_mode: PermissionMode::ReadOnly,
            network: NetworkPolicy::Allow,
            sandbox: SandboxDefaults {
                enabled: true,
                network_access: true,
                read_allow_paths: vec!["/home/user/notes".into()],
            },
        }
    }

    #[test]
    fn round_trips_through_an_endpoint_scoped_key_without_cross_origin_bleed() {
        // session-defaults.test.ts:26/:32
        let s = MemoryStore::new();
        save(&sample(), &s, "http://a.example:50190").unwrap();
        assert_eq!(load(&s, "http://a.example:50190"), Some(sample()));
        assert_eq!(load(&s, "http://b.example:50190"), None, "another endpoint sees nothing");
        assert!(scoped_key("http://a.example:50190").starts_with("octoscode-web.session-defaults.v1:http%3A%2F%2F"));
        assert!(!scoped_key("http://a.example").contains('/'), "one safe path segment");
    }

    #[test]
    fn fails_closed_on_corrupt_json_and_unknown_modes() {
        // :39 corrupt, :46 unknown permission mode
        assert_eq!(parse(Some("{not json")), None);
        assert_eq!(parse(Some(r#"{"version":2,"permissionMode":"read_only","network":"deny","sandbox":{"enabled":false,"networkAccess":false,"readAllowPaths":[]}}"#)), None);
        assert_eq!(parse(Some(r#"{"version":1,"permissionMode":"yolo","network":"deny","sandbox":{"enabled":false,"networkAccess":false,"readAllowPaths":[]}}"#)), None);
        assert_eq!(parse(Some(r#"{"version":1,"permissionMode":"read_only","network":"maybe","sandbox":{"enabled":false,"networkAccess":false,"readAllowPaths":[]}}"#)), None);
        assert_eq!(parse(Some(r#"{"version":1,"permissionMode":"read_only","network":"deny","sandbox":{"enabled":"yes","networkAccess":false,"readAllowPaths":[]}}"#)), None);
    }

    #[test]
    fn caps_read_allow_paths_to_a_bounded_list() {
        // :58
        let paths: Vec<String> = (0..40).map(|i| format!("/p/{i}")).chain([" ".into()]).collect();
        let raw = json!({"version":1,"permissionMode":"workspace_write","network":"deny",
            "sandbox":{"enabled":true,"networkAccess":false,"readAllowPaths":paths}})
        .to_string();
        let d = parse(Some(&raw)).unwrap();
        assert_eq!(d.sandbox.read_allow_paths.len(), MAX_READ_ALLOW_PATHS);
        let long = "x".repeat(MAX_PATH_LENGTH + 1);
        let raw = json!({"version":1,"permissionMode":"workspace_write","network":"deny",
            "sandbox":{"enabled":true,"networkAccess":false,"readAllowPaths":[long, "  /ok  "]}})
        .to_string();
        assert_eq!(parse(Some(&raw)).unwrap().sandbox.read_allow_paths, vec!["/ok".to_owned()]);
    }

    #[test]
    fn the_open_sandbox_rides_only_when_enabled_and_advertised() {
        let mut d = sample();
        let p = open_sandbox(&d, true).unwrap();
        assert_eq!((p.enabled, p.network_access), (Some(true), Some(true)));
        assert!(open_sandbox(&d, false).is_none(), "unadvertised: never sent");
        d.sandbox.enabled = false;
        assert!(open_sandbox(&d, true).is_none(), "sandbox off: no param (the web's default)");
    }

    #[test]
    fn a_created_id_applies_once_and_a_reopen_never_applies() {
        let p = Pending::new();
        p.arm("p:fresh", sample());
        assert!(p.take("p:other").is_none(), "a re-open of another id was never armed");
        assert_eq!(p.take("p:fresh"), Some(sample()));
        assert!(p.take("p:fresh").is_none(), "second open of the same id: never re-applied");
        p.arm("p:fresh", sample());
        assert!(p.take("p:fresh").is_none(), "the marker blocks a second apply for the same created id");
        assert_eq!(permission_params("p:fresh", &sample())["update"], json!({"mode":"read_only","network":"allow"}));
    }
}
