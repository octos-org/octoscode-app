//! P4h1 rows 304-307 — the recent-workspaces cache, ported field-for-field
//! from the web's `features/workspace/workspace-recents.ts`.
//!
//! ## What this is (and is not)
//!
//! The web's own doc comment (`workspace-recents.ts:10-17`) is the contract:
//! browser recents are a **tab-scoped workspace navigation cache, never a
//! session catalog**. So this cache stores only `{id, name, path,
//! lastOpenedAt}` and never a session id, title, prompt or protocol
//! projection — only Core can say which sessions are valid for the current
//! authenticated principal. Rows 304 (`loadRecentWorkspaces`), 305
//! (`rememberWorkspace`), 306 (`clearRecentWorkspaces`) and 307
//! (`workspaceName`) are the four exports.
//!
//! ## Storage
//!
//! The web stores one `localStorage` key per endpoint. Natively the same
//! per-endpoint isolation is a **file per endpoint** under the app's own
//! config dir, mirroring the crate's existing persistence precedent
//! (`screens/theme.rs:507` — `$HOME/.octoscode/display.json`, overridable with
//! an env var so tests stay hermetic). [`Store`] is the trait seam that keeps
//! the web's two failure behaviours reachable: a storage that throws on
//! `set_item` (`rememberWorkspace` still returns the new list — the catch at
//! `workspace-recents.ts:52-54`) and a storage that refuses to remove
//! (`clearRecentWorkspaces` reports `false` honestly — `:67-76`).
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

/// The web's `MAX_WORKSPACES` (`workspace-recents.ts:13`) — the bound the
/// loaded list is sliced to.
pub const MAX_WORKSPACES: usize = 20;

/// One remembered workspace — the web's `RecentWorkspace` interface
/// (`workspace-recents.ts:3-8`), field for field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentWorkspace {
    pub id: String,
    pub name: String,
    pub path: String,
    pub last_opened_at: u64,
}

impl RecentWorkspace {
    /// The web's `parseWorkspace` + `text` helpers (`:88-100`): every one of
    /// `id`/`name`/`path` must be a non-blank string and `lastOpenedAt` a
    /// number, or the row is DROPPED (not coerced). `None` = drop.
    fn parse(value: &serde_json::Value) -> Option<RecentWorkspace> {
        let object = value.as_object()?;
        let text = |key: &str| -> Option<String> {
            object
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let last_opened_at = object.get("lastOpenedAt")?.as_u64()?;
        Some(RecentWorkspace {
            id: text("id")?,
            name: text("name")?,
            path: text("path")?,
            last_opened_at,
        })
    }

    /// The JSON shape the web persists, keeping the **wire field names**
    /// (`lastOpenedAt`, camelCase) so a cache file stays readable against the
    /// web's own format.
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "path": self.path,
            "lastOpenedAt": self.last_opened_at,
        })
    }
}

/// The storage seam. The web takes a `Pick<Storage, "getItem" | "setItem" |
/// "removeItem">`; this is the same three operations, so the read-only and
/// throwing test stores are expressible.
pub trait Store_: Send + Sync {
    fn get_item(&self, key: &str) -> Option<String>;
    /// Errors are the web's caught `setItem` throw.
    fn set_item(&self, key: &str, value: &str) -> Result<(), String>;
    /// Errors are the web's caught `removeItem` throw.
    fn remove_item(&self, key: &str) -> Result<(), String>;
}

// The trait is named `Store_` in Rust and re-exported under the readable name.
pub use Store_ as Storage;

/// An in-memory store: the unit tests' `memoryStorage()`
/// (`workspace-recents.test.ts:8-15`).
#[derive(Default)]
pub struct MemoryStore {
    items: Mutex<BTreeMap<String, String>>,
    /// Set to make every `set_item`/`remove_item` throw — the test's
    /// read-only storage (`:19-27`).
    read_only: Mutex<bool>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Make writes fail the way a read-only browser storage does.
    pub fn set_read_only(&self, read_only: bool) {
        *self.read_only.lock().unwrap() = read_only;
    }

    /// Test/probe seam: write a raw value under a key (the malformed-storage
    /// and legacy-key tests need this).
    pub fn put_raw(&self, key: &str, value: &str) {
        self.items
            .lock()
            .unwrap()
            .insert(key.to_owned(), value.to_owned());
    }

    /// Test/probe seam: read a raw value (`None` = absent).
    pub fn peek(&self, key: &str) -> Option<String> {
        self.items.lock().unwrap().get(key).cloned()
    }
}

impl Storage for MemoryStore {
    fn get_item(&self, key: &str) -> Option<String> {
        self.items.lock().unwrap().get(key).cloned()
    }

    fn set_item(&self, key: &str, value: &str) -> Result<(), String> {
        if *self.read_only.lock().unwrap() {
            return Err("blocked".to_owned());
        }
        self.items
            .lock()
            .unwrap()
            .insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    fn remove_item(&self, key: &str) -> Result<(), String> {
        if *self.read_only.lock().unwrap() {
            return Err("blocked".to_owned());
        }
        self.items.lock().unwrap().remove(key);
        Ok(())
    }
}

/// The v2 key prefix (`workspace-recents.ts:11`).
pub const STORAGE_PREFIX: &str = "octoscode.product.workspace-recents.v2";
/// The v1 key prefix (`:12`) — the legacy cache that held session metadata
/// and is REMOVED on clear, never read (`workspace-recents.test.ts:73-84`).
pub const LEGACY_STORAGE_PREFIX: &str = "octoscode.product.workspace-recents.v1";

/// `storageKey(endpoint)` — the prefix plus the URL-encoded endpoint
/// (`:102-104`). `encodeURIComponent` leaves `A-Za-z0-9-_.!~*'()` alone and
/// percent-encodes everything else, which is what makes one endpoint's cache
/// never collide with another's (`workspace-recents.test.ts:63-66`).
pub fn storage_key(endpoint: &str) -> String {
    format!("{STORAGE_PREFIX}:{}", encode_uri_component(endpoint.trim()))
}

/// `legacyStorageKey(endpoint)` (`:106-108`).
pub fn legacy_storage_key(endpoint: &str) -> String {
    format!(
        "{LEGACY_STORAGE_PREFIX}:{}",
        encode_uri_component(endpoint.trim())
    )
}

/// `encodeURIComponent` (RFC 3986 unreserved set), the same encoding the web
/// puts in the key.
fn encode_uri_component(input: &str) -> String {
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

/// #P4h1 row 307 — `workspaceName` (`workspace-recents.ts:81-85`): strip
/// trailing separators, take the last non-empty segment, and fall back to the
/// ORIGINAL path when there is none (a bare `/` stays `/`).
pub fn workspace_name(path: &str) -> String {
    let normalized = path.trim_end_matches(['/', '\\']);
    normalized
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .next_back()
        .map(str::to_owned)
        .unwrap_or_else(|| path.to_owned())
}

/// #P4h1 row 304 — `loadRecentWorkspaces` (`:19-36`): read, parse, DROP
/// malformed rows, sort newest-first, slice to the bound. Any failure at all
/// (absent, not JSON, not an array) yields the empty list — never a throw.
pub fn load_recent_workspaces(storage: &dyn Storage, endpoint: &str) -> Vec<RecentWorkspace> {
    let Some(raw) = storage.get_item(&storage_key(endpoint)) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    let Some(array) = value.as_array() else {
        return Vec::new();
    };
    let mut rows: Vec<RecentWorkspace> = array.iter().filter_map(RecentWorkspace::parse).collect();
    // `sort((l, r) => r.lastOpenedAt - l.lastOpenedAt)` — a stable sort, so
    // two rows with the same timestamp keep their stored order.
    rows.sort_by(|left, right| right.last_opened_at.cmp(&left.last_opened_at));
    rows.truncate(MAX_WORKSPACES);
    rows
}

/// #P4h1 row 305 — `rememberWorkspace` (`:38-60`): the canonical path is the
/// id, the name is derived, an existing row for the same path is DEDUPED out
/// of its old position, the new row goes to the FRONT, and the list is sliced
/// to the bound. A blank path is not remembered (the loaded list is returned
/// unchanged). A storage that cannot save does NOT lose the navigation — the
/// new list is still returned.
pub fn remember_workspace(
    storage: &dyn Storage,
    endpoint: &str,
    path: &str,
    now: u64,
) -> Vec<RecentWorkspace> {
    let canonical_path = path.trim();
    if canonical_path.is_empty() {
        return load_recent_workspaces(storage, endpoint);
    }
    let current = load_recent_workspaces(storage, endpoint);
    let workspace = RecentWorkspace {
        id: canonical_path.to_owned(),
        name: workspace_name(canonical_path),
        path: canonical_path.to_owned(),
        last_opened_at: now,
    };
    let mut next = vec![workspace];
    next.extend(
        current
            .into_iter()
            .filter(|candidate| candidate.path != canonical_path),
    );
    next.truncate(MAX_WORKSPACES);
    // The web catches the throw and keeps going (`workspace-recents.ts:52-54`).
    let _ = write_recent_workspaces(storage, endpoint, &next);
    next
}

/// #P4h1 row 306 — `clearRecentWorkspaces` (`:65-79`): remove the v2 key AND
/// the legacy v1 key, and report the outcome HONESTLY — a key that is still
/// readable afterwards, or a remove that threw, makes this `false` even if the
/// other key went cleanly. The legacy key is why this matters: the v1 cache
/// held session metadata (`workspace-recents.test.ts:73-84`), so a silent
/// failure would leave it on disk.
pub fn clear_recent_workspaces(storage: &dyn Storage, endpoint: &str) -> bool {
    let mut cleared = true;
    for key in [storage_key(endpoint), legacy_storage_key(endpoint)] {
        if storage.remove_item(&key).is_err() || storage.get_item(&key).is_some() {
            cleared = false;
        }
    }
    cleared
}

/// Persist the list as the web's JSON array.
fn write_recent_workspaces(
    storage: &dyn Storage,
    endpoint: &str,
    rows: &[RecentWorkspace],
) -> Result<(), String> {
    let array: Vec<serde_json::Value> = rows.iter().map(RecentWorkspace::to_json).collect();
    let encoded = serde_json::to_string(&serde_json::Value::Array(array))
        .map_err(|e| e.to_string())?;
    storage.set_item(&storage_key(endpoint), &encoded)
}

/// ---- the file-backed store (production) -------------------------------------

/// The production store: one file per endpoint under the app config dir.
pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    /// The store the app uses. `OCTOSCODE_RECENTS_DIR` overrides the directory
    /// so tests (and the headless harness) never touch a real home.
    pub fn new() -> Self {
        let dir = match std::env::var("OCTOSCODE_RECENTS_DIR") {
            Ok(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".octoscode").join("workspace-recents")
            }
        };
        Self { dir }
    }

    /// A store rooted at an explicit directory (tests).
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// One endpoint's file. The key is already percent-encoded, so it is a
    /// safe single path segment.
    fn path_for(&self, key: &str) -> PathBuf {
        self.dir.join(key)
    }
}

impl Default for FileStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for FileStore {
    fn get_item(&self, key: &str) -> Option<String> {
        std::fs::read_to_string(self.path_for(key)).ok()
    }

    fn set_item(&self, key: &str, value: &str) -> Result<(), String> {
        if let Some(parent) = self.path_for(key).parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(self.path_for(key), value).map_err(|e| e.to_string())
    }

    fn remove_item(&self, key: &str) -> Result<(), String> {
        match std::fs::remove_file(self.path_for(key)) {
            Ok(()) => Ok(()),
            // An already-absent key IS removed — the web's `removeItem` does
            // not throw for a missing key, so this must not report failure.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// The endpoint this app is talking to — the cache's isolation key, and the
/// web's `connection.endpoint` (`App.tsx:663,1021,1031`). Natively the base
/// URL is the env var the connect path already reads (`lib.rs:1350`), with the
/// same default, so the picker and the transport can never disagree about
/// which deployment they are on.
pub fn endpoint() -> String {
    pick_endpoint(
        CONNECTED.read().ok().and_then(|c| c.clone()),
        std::env::var("OCTOS_BASE_URL").ok(),
    )
}

/// Pure core of [`endpoint`]: the connected server wins, then the env default,
/// then the built-in default.
fn pick_endpoint(connected: Option<String>, env: Option<String>) -> String {
    connected
        .or(env)
        .unwrap_or_else(|| "http://127.0.0.1:50082".into())
}

/// The stored form of a connected address ('/'-trimmed; `None` when blank).
fn normalize_endpoint(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/');
    (!url.is_empty()).then(|| url.to_owned())
}

/// The server the app actually connected to (Connect screen, pairing, or the
/// startup env). Until a connection succeeds, [`endpoint`] falls back to the
/// env default. Without this, Settings' "Octos server" row, the recent-workspace
/// key and Retry kept naming OCTOS_BASE_URL after the person connected (or
/// paired) somewhere else (found by #A2).
static CONNECTED: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

/// Record the endpoint of a successful connection (trailing '/' trimmed, so the
/// recent-workspace key matches however the address was typed).
pub fn set_connected_endpoint(url: &str) {
    if let Some(url) = normalize_endpoint(url) {
        if let Ok(mut c) = CONNECTED.write() {
            *c = Some(url);
        }
    }
}

/// The web's `Date.now()` argument to `rememberWorkspace` — milliseconds since
/// the Unix epoch. A clock that steps backwards still sorts correctly: the
/// bound and the dedupe are independent of it.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The store the running app reads and writes, installed once at startup.
static ACTIVE: Mutex<Option<Arc<dyn Storage>>> = Mutex::new(None);

/// Install the process-wide store (the app's startup path calls it with a
/// [`FileStore`]; tests inject a [`MemoryStore`]).
pub fn set_store(storage: Arc<dyn Storage>) {
    *ACTIVE.lock().unwrap() = Some(storage);
}

/// The startup hook: install the file store and purge the LEGACY v1 cache only.
///
/// This is the web's `App.tsx:1019-1023`, kept deliberately:
///
/// ```ts
/// // v1 persisted session ids/titles in localStorage. Remove that data rather
/// // than migrating it into the product: Core is the session authority.
/// clearRecentWorkspaces(browserStorage("localStorage"), connection.endpoint);
/// ```
///
/// The v1 cache held session ids, titles and prompts; this cache is a
/// navigation cache that must never carry them
/// (`workspace-recents.ts:10-17`), so the legacy file is DELETED rather than
/// read or migrated.
///
/// ## Why only the legacy key
///
/// The web's two callers of `clearRecentWorkspaces` are BOTH
/// identity-change/disconnect cleanups (`ConnectionGate.tsx:289-291,335-339`),
/// and the LIVE recents live in `sessionStorage` (tab-scoped) while the legacy
/// v1 blob lived in `localStorage` (durable). Natively there is one store, so
/// clearing both keys here would wipe the user's recents on every launch —
/// strictly worse than the web. So the startup purge removes ONLY the legacy
/// key, and the user-facing "forget this connection" purge is
/// [`forget_endpoint`], which clears both and is wired to the disconnect path.
///
/// Returns whether the purge reported success, so the caller logs an honest
/// failure instead of pretending (row 306's "report failure honestly").
pub fn init_persistence() -> bool {
    // Install the file store ONLY when nothing is installed. The startup seam
    // sits in `script_mod!`, which can be evaluated more than once (a script
    // reload re-runs the splice), and an unconditional install would replace
    // the active store on the second pass. A caller that WANTS to inject a
    // store does it through [`set_store`], which still replaces.
    let already = ACTIVE.lock().unwrap().is_some();
    if !already {
        set_store(Arc::new(FileStore::new()));
    }
    let key = legacy_storage_key(&endpoint());
    let storage = store();
    storage.remove_item(&key).is_ok() && storage.get_item(&key).is_none()
}

/// The `script_mod!` splice calls THIS, not [`init_persistence`] directly: a
/// bare `{ if .. ::log::warn!(..) }` block in `script_mod!` is SCRIPT tokens
/// to the makepad parser (`IfTest` / `EmitUnary` / `Operator(:)`) and fails to
/// parse — the app came up a 46-widget husk with 11 [E] DSL errors and no
/// screen_dock (fx1-evidence/walk-conv.log). Behind this fn the warn is plain
/// Rust and the splice rides the one `#(call)` shape both dialects accept
/// (the `theme::eval_roles` precedent). Same contract as [`init_persistence`]:
/// the bool is the purge's honest result.
pub fn init_persistence_logged() -> bool {
    let ok = init_persistence();
    if !ok {
        ::log::warn!("octoscode: workspace recents: legacy v1 cache could not be purged");
    }
    ok
}

/// The user-facing clear: BOTH keys for one endpoint, reporting honestly.
///
/// This is `ConnectionGate`'s disconnect/identity-change cleanup
/// (`ConnectionGate.tsx:335-339`) — the "forget this deployment" action. It is
/// a separate function from the startup purge precisely so the two are not
/// conflated: startup removes the legacy blob only, this removes the user's
/// navigation list too.
pub fn forget_endpoint(endpoint: &str) -> bool {
    clear_recent_workspaces(&*store(), endpoint)
}

/// The installed store, or a fresh [`FileStore`] when none was installed —
/// so a read before startup is still correct rather than empty.
pub fn store() -> Arc<dyn Storage> {
    ACTIVE
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| Arc::new(FileStore::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENDPOINT: &str = "https://octos.example";

    fn seeded() -> (MemoryStore, Vec<RecentWorkspace>) {
        let storage = MemoryStore::new();
        let rows = remember_workspace(&storage, ENDPOINT, "/old", 1);
        (storage, rows)
    }

    /// Row 307 — the web's `derives a human workspace name from host paths`
    /// (`workspace-recents.test.ts:68-71`), plus its fallbacks.
    #[test]
    fn workspace_name_derives_from_posix_and_windows_paths() {
        assert_eq!(workspace_name("/srv/projects/octoscode/"), "octoscode");
        assert_eq!(workspace_name("C:\\work\\octoscode"), "octoscode");
        // A path with no usable segment falls back to the ORIGINAL path.
        assert_eq!(workspace_name("/"), "/");
        assert_eq!(workspace_name(""), "");
    }

    /// Row 305 — dedupe moves the row to the front and refreshes its stamp.
    #[test]
    fn remember_dedupes_and_moves_to_front() {
        let storage = MemoryStore::new();
        remember_workspace(&storage, ENDPOINT, "/one", 1);
        remember_workspace(&storage, ENDPOINT, "/two", 2);
        let rows = remember_workspace(&storage, ENDPOINT, "/one", 3);
        assert_eq!(rows[0].path, "/one");
        assert_eq!(rows[0].last_opened_at, 3);
        assert_eq!(rows.len(), 2, "the /one row is deduped, not duplicated");
        assert_eq!(rows[1].path, "/two");
    }

    /// Row 305 — the web's blank-path arm (`:41-42`) is not a remember.
    #[test]
    fn a_blank_path_is_not_remembered() {
        let (storage, _) = seeded();
        let rows = remember_workspace(&storage, ENDPOINT, "   ", 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "/old");
    }

    /// Row 304 — the cache is bounded to 20, newest first.
    #[test]
    fn the_cache_is_bounded_and_newest_first() {
        let storage = MemoryStore::new();
        for i in 0..25 {
            remember_workspace(&storage, ENDPOINT, &format!("/p{i}"), i as u64);
        }
        let rows = load_recent_workspaces(&storage, ENDPOINT);
        assert_eq!(rows.len(), MAX_WORKSPACES);
        assert_eq!(rows[0].path, "/p24", "newest first");
        assert!(
            rows.iter().all(|r| r.last_opened_at >= 4),
            "the five oldest fell off the end"
        );
    }

    /// Row 304 — the web's "keeps deployments isolated and fails closed on
    /// malformed storage" (`workspace-recents.test.ts:61-67`).
    #[test]
    fn endpoints_are_isolated_and_malformed_storage_fails_closed() {
        let storage = MemoryStore::new();
        remember_workspace(&storage, "https://one.example", "/srv/one", 1);
        assert!(load_recent_workspaces(&storage, "https://two.example").is_empty());
        storage.put_raw(&storage_key("https://two.example"), "not-json");
        assert!(load_recent_workspaces(&storage, "https://two.example").is_empty());
    }

    /// Row 304 — a row missing any required field is DROPPED, and the rest of
    /// the list still loads (the web's `filter(workspace => workspace !== null)`).
    #[test]
    fn malformed_rows_are_dropped_not_coerced() {
        let storage = MemoryStore::new();
        remember_workspace(&storage, ENDPOINT, "/good", 1);
        let key = storage_key(ENDPOINT);
        let mut rows: Vec<serde_json::Value> = vec![serde_json::json!({
            "id": "/no-name", "path": "/no-name", "lastOpenedAt": 9
        })];
        rows.push(serde_json::json!({
            "id": "/blank", "name": "   ", "path": "/blank", "lastOpenedAt": 9
        }));
        rows.push(serde_json::json!({
            "id": "/no-stamp", "name": "n", "path": "/no-stamp"
        }));
        storage.put_raw(&key, &serde_json::to_string(&rows).unwrap());
        assert!(load_recent_workspaces(&storage, ENDPOINT).is_empty());

        // A non-array payload is not a list.
        storage.put_raw(&key, "{\"path\":\"/srv\"}");
        assert!(load_recent_workspaces(&storage, ENDPOINT).is_empty());
    }

    /// Rows 304/305/306 — the web's headline case
    /// ("keeps navigation in memory and reports failed cleanup when storage
    /// becomes read-only", `workspace-recents.test.ts:17-44`): a storage that
    /// cannot save still returns the new navigation, cannot clear reports
    /// `false`, and the older value is still readable afterwards.
    #[test]
    fn a_read_only_store_keeps_navigation_and_reports_failed_cleanup() {
        let (storage, _) = seeded();
        storage.set_read_only(true);
        let rows = remember_workspace(&storage, ENDPOINT, "/new", 2);
        assert_eq!(rows[0].path, "/new", "navigation is returned regardless");
        assert!(!clear_recent_workspaces(&storage, ENDPOINT), "honest failure");
        assert_eq!(
            load_recent_workspaces(&storage, ENDPOINT)[0].path,
            "/old",
            "the last saved value is untouched"
        );
        storage.set_read_only(false);
        assert!(clear_recent_workspaces(&storage, ENDPOINT));
        assert!(load_recent_workspaces(&storage, ENDPOINT).is_empty());
    }

    /// Row 306 — the web's legacy-removal case
    /// ("removes the legacy cache that contained session metadata",
    /// `workspace-recents.test.ts:73-84`).
    #[test]
    fn clear_removes_the_legacy_v1_key() {
        let (storage, _) = seeded();
        let legacy = legacy_storage_key(ENDPOINT);
        storage.put_raw(
            &legacy,
            r#"[{"path":"/srv/work","sessions":[{"id":"secret","title":"private"}]}]"#,
        );
        assert!(clear_recent_workspaces(&storage, ENDPOINT));
        assert!(storage.peek(&legacy).is_none(), "v1 is gone");
        assert!(storage.peek(&storage_key(ENDPOINT)).is_none(), "v2 is gone");
    }

    /// Row 306 — clearing an already-empty cache is success, not failure: the
    /// web's `removeItem` does not throw for a missing key (`:69-73`).
    #[test]
    fn clearing_an_empty_cache_reports_success() {
        let storage = MemoryStore::new();
        assert!(clear_recent_workspaces(&storage, ENDPOINT));
    }

    /// Rows 304-306 — the key encodes the endpoint, so no two deployments
    /// share a file (the web's isolation property, on the real store).
    #[test]
    fn the_file_store_isolates_endpoints_on_disk() {
        let dir = std::env::temp_dir().join(format!("octoscode-recents-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let storage = FileStore::at(&dir);
        remember_workspace(&storage, "https://one.example", "/srv/one", 1);
        assert_eq!(load_recent_workspaces(&storage, "https://one.example").len(), 1);
        assert!(load_recent_workspaces(&storage, "https://two.example").is_empty());
        assert!(clear_recent_workspaces(&storage, "https://one.example"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod connected_endpoint_tests {
    /// Integration fix (judge, from #A2's report): once a connection succeeds,
    /// `endpoint()` names THAT server, not the env default, so Settings, the
    /// recent-workspace key and Retry agree with it. Pure helpers only: the
    /// process-wide value is never mutated here (tests run in parallel).
    #[test]
    fn endpoint_follows_the_connected_server() {
        let connected = super::normalize_endpoint("http://127.0.0.1:50190/");
        assert_eq!(connected.as_deref(), Some("http://127.0.0.1:50190"));
        assert_eq!(super::normalize_endpoint("   "), None, "a blank address never replaces it");
        assert_eq!(
            super::pick_endpoint(connected, Some("http://127.0.0.1:50082".into())),
            "http://127.0.0.1:50190"
        );
        assert_eq!(super::pick_endpoint(None, Some("http://h:1".into())), "http://h:1");
        assert_eq!(super::pick_endpoint(None, None), "http://127.0.0.1:50082");
    }
}
