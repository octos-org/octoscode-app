//! A8 — per-Session composer drafts, durable per authenticated principal.
//!
//! Web oracle: `features/session/durable-session-drafts.ts` (the durable
//! store: one scope per `[server origin, principal]`, the principal from REST
//! `/api/auth/me`; written per Session; an empty draft removes its entry; a
//! corrupt scope fails closed — read nothing, write nothing),
//! `session-draft-cache.ts` (the bound: 50 drafts / 512 KiB, the oldest
//! evicted first, a bad entry skipped rather than poisoning the batch) and
//! `session-composer-drafts.ts` (input state belongs to the Session it was
//! typed in: selection never moves it; a returned prompt goes back to its
//! OWN Session, in order).
//!
//! Natively there is ONE composer, so the per-Session drafts are a stash:
//! switching Sessions files the composer's text under the Session it was
//! typed in and puts the new Session's own draft back (empty for a new
//! chat); a send clears its Session's entry; a draft survives a restart once
//! the principal is known. The Session key is the web's
//! `workspaceSessionKey` (`[workspace, profile, session]` as JSON).
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::screens::recents::Storage;

/// `PREFIX` (`durable-session-drafts.ts:7`).
pub const PREFIX: &str = "octoscode-web.draft.v1:";
/// `MAX_DRAFTS` (`session-draft-cache.ts:1`).
pub const MAX_DRAFTS: usize = 50;
/// The cache's byte bound (`session-draft-cache.ts:28`).
pub const MAX_BYTES: usize = 524_288;
/// A key longer than this is not a Session key (`session-draft-cache.ts:22`).
pub const MAX_KEY: usize = 16_384;

/// `durableDraftScope(endpoint, principal)` (`:33-35`).
pub fn scope(origin: &str, principal: &str) -> String {
    format!("{PREFIX}{}:", json!([origin, principal]))
}

/// `workspaceSessionKey(workspace, profile, session)` (`active-session-key.ts:14-20`).
pub fn session_key(workspace: &str, profile: &str, session: &str) -> String {
    json!([workspace, profile, session]).to_string()
}

/// `parseSessionDrafts` (`session-draft-cache.ts:6-33`): newest-first, a bad
/// or duplicate entry skipped, the batch bounded by bytes.
pub fn parse_drafts(value: &Value) -> Vec<(String, String)> {
    let Some(arr) = value.as_array() else { return Vec::new() };
    let mut out: VecDeque<(String, String)> = VecDeque::new();
    let mut seen = std::collections::HashSet::new();
    let mut size = 0usize;
    for entry in arr.iter().rev() {
        let Some(pair) = entry.as_array().filter(|p| p.len() == 2) else { continue };
        let (Some(k), Some(t)) = (pair[0].as_str(), pair[1].as_str()) else { continue };
        if k.is_empty() || k.chars().count() > MAX_KEY || seen.contains(k) {
            continue;
        }
        let n = k.chars().count() + t.chars().count();
        if size + n > MAX_BYTES {
            continue;
        }
        size += n;
        seen.insert(k.to_owned());
        out.push_front((k.to_owned(), t.to_owned()));
    }
    out.into_iter().collect()
}

/// `SessionDraftCache` (`session-draft-cache.ts:35-80`): insertion-ordered,
/// the oldest evicted when a NEW key would exceed the bound.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraftCache {
    drafts: Vec<(String, String)>,
}

impl DraftCache {
    pub fn from(initial: Vec<(String, String)>) -> Self {
        Self { drafts: initial }
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.drafts.iter().find(|(k, _)| k == key).map(|(_, t)| t.as_str())
    }
    /// Set (empty = delete). Returns the evicted key, if one was dropped.
    pub fn set(&mut self, key: &str, text: &str) -> Option<String> {
        if text.is_empty() {
            self.drafts.retain(|(k, _)| k != key);
            return None;
        }
        if let Some(e) = self.drafts.iter_mut().find(|(k, _)| k == key) {
            e.1 = text.to_owned();
            return None;
        }
        let evicted = (self.drafts.len() >= MAX_DRAFTS).then(|| self.drafts.remove(0).0);
        self.drafts.push((key.to_owned(), text.to_owned()));
        evicted
    }
    pub fn snapshot(&self) -> Vec<(String, String)> {
        self.drafts.clone()
    }
    pub fn len(&self) -> usize {
        self.drafts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.drafts.is_empty()
    }
}

/// The scope's storage key: the scope itself, percent-encoded so a file
/// store keeps it as ONE path segment.
fn storage_key(scope: &str) -> String {
    let mut out = String::new();
    for b in scope.as_bytes() {
        let c = *b as char;
        if c.is_ascii_alphanumeric() || "-_.!~*'()".contains(c) {
            out.push(c);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The running app's draft state.
#[derive(Default)]
struct Drafts {
    /// The durable binding: (scope, storage, readable). Absent until the
    /// principal is known (drafts are then process-only, like the web's
    /// session-storage fallback).
    durable: Option<(String, Arc<dyn Storage>, bool)>,
    cache: DraftCache,
    /// The Session key the composer currently shows, and the text last filed.
    active: Option<String>,
    last: String,
    /// A launch's profile choice is opening: the NEXT switch MOVES the
    /// composer's text to the new Session instead of filing it under the old.
    carry: bool,
}

static DRAFTS: Mutex<Option<Drafts>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Drafts) -> R) -> R {
    let mut g = DRAFTS.lock().unwrap_or_else(|p| p.into_inner());
    f(g.get_or_insert_with(Drafts::default))
}

/// `loadDurableDrafts` for a scope: `None` when unreadable/corrupt (fail closed).
pub fn load(storage: &dyn Storage, scope: &str) -> Option<Vec<(String, String)>> {
    match storage.get_item(&storage_key(scope)) {
        None => Some(Vec::new()),
        Some(raw) => {
            let v: Value = serde_json::from_str(&raw).ok()?;
            let parsed = parse_drafts(&v);
            // `every(draft => parseSessionDrafts([draft]).length === 1)`: a
            // corrupt scope is not partially trusted.
            (parsed.len() == v.as_array().map(Vec::len).unwrap_or(usize::MAX)).then_some(parsed)
        }
    }
}

fn persist(d: &mut Drafts) -> bool {
    let Some((scope, storage, readable)) = &d.durable else { return true };
    if !*readable {
        return false; // a poisoned scope is never overwritten
    }
    let value = Value::Array(d.cache.snapshot().into_iter().map(|(k, t)| json!([k, t])).collect());
    storage.set_item(&storage_key(scope), &value.to_string()).is_ok()
}

/// Bind the durable scope once the principal is known (a new connection
/// re-binds). The scope's stored drafts join the cache; the composer's
/// Session gets its stored draft back when the composer is empty.
pub fn bind(scope_value: &str, storage: Arc<dyn Storage>) -> Option<String> {
    with(|d| {
        if d.durable.as_ref().is_some_and(|(s, _, _)| s == scope_value) {
            return None;
        }
        let loaded = load(&*storage, scope_value);
        let readable = loaded.is_some();
        if let Some(stored) = loaded {
            // The stored drafts first, this process's own edits on top.
            let mut merged = DraftCache::from(stored);
            for (k, t) in d.cache.snapshot() {
                merged.set(&k, &t);
            }
            d.cache = merged;
        }
        d.durable = Some((scope_value.to_owned(), storage, readable));
        persist(d);
        let key = d.active.clone()?;
        if d.last.is_empty() {
            let restored = d.cache.get(&key).unwrap_or("").to_owned();
            if !restored.is_empty() {
                d.last = restored.clone();
                return Some(restored);
            }
        }
        None
    })
}

/// Follow the composer: a Session switch files the old text and returns the
/// new Session's draft (the caller puts it in the composer); an edit in the
/// same Session is filed (`saveDurableDraft`; empty removes).
pub fn follow(active: Option<&str>, composer: &str) -> Option<String> {
    with(|d| {
        if d.active.as_deref() != active {
            // The first Session the composer is seen with: what the person
            // typed before it was known stays theirs (filed, not replaced).
            if d.active.is_none() && !composer.is_empty() {
                if let Some(k) = active {
                    d.cache.set(k, composer);
                }
                d.active = active.map(str::to_owned);
                d.last = composer.to_owned();
                persist(d);
                return None;
            }
            if std::mem::take(&mut d.carry) {
                // The transition committed: the text moves with it.
                if let Some(old) = d.active.clone() {
                    d.cache.set(&old, "");
                }
                if let Some(k) = active {
                    d.cache.set(k, composer);
                }
                d.active = active.map(str::to_owned);
                d.last = composer.to_owned();
                persist(d);
                return None;
            }
            if let Some(old) = d.active.clone() {
                if let Some(ev) = d.cache.set(&old, composer) {
                    ::log::info!("octoscode: draft for {ev} evicted (bound {MAX_DRAFTS})");
                }
            }
            d.active = active.map(str::to_owned);
            let next = active.and_then(|k| d.cache.get(k)).unwrap_or("").to_owned();
            d.last = next.clone();
            persist(d);
            return (next != composer).then_some(next);
        }
        if let (Some(key), true) = (active, d.last != composer) {
            d.cache.set(key, composer);
            d.last = composer.to_owned();
            persist(d);
        }
        None
    })
}

/// A prompt returned to a Session that is no longer in the composer (a send
/// that failed after a switch): it goes back to ITS Session, ahead of what
/// was typed there since (`session-composer-drafts.ts` restores, in order).
pub fn restore_for(key: &str, text: &str) {
    with(|d| {
        let merged = match d.cache.get(key).filter(|t| !t.trim().is_empty()) {
            Some(existing) => format!("{text}\n\n{existing}"),
            None => text.to_owned(),
        };
        d.cache.set(key, &merged);
        persist(d);
    });
}

/// A launch transition is opening: carry the composer's text to the Session
/// it commits (`product.spec.ts` "moves drafts only after a profile-choice
/// Session transition commits").
pub fn carry_next_switch() {
    with(|d| d.carry = true);
}

/// The transition failed: nothing moves.
pub fn cancel_carry() {
    with(|d| d.carry = false);
}

/// The stored draft of `key` (tests and the restore path).
pub fn get(key: &str) -> Option<String> {
    with(|d| d.cache.get(key).map(str::to_owned))
}

/// The composer's Session key for a conversation.
///
/// The profile part is the Session's OWN profile (a full id's prefix,
/// `<profile>:<channel>:<chat>` / `<profile>:main`; the connection's for a
/// bare legacy id), so a launch adopting another profile never re-keys the
/// Session still in the composer (`workspaceSessionKey` takes the Session
/// scope's profile, not the connection's).
pub fn key_of(conv: &crate::flow::Conversation, session: &str) -> String {
    let profile = session
        .split_once(':')
        .map(|(p, _)| p)
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| conv.profile());
    session_key(&conv.store.domains.session.workspace_root(session).unwrap_or_default(), &profile, session)
}

/// The web's principal: REST `GET /api/auth/me` with the connection's bearer
/// (`resolveDraftPrincipal`, `:10-31`): `user.id` (non-blank, <= 1024).
/// `None` = no durable drafts for this connection.
pub async fn resolve_principal(conv: &crate::flow::Conversation) -> Option<String> {
    let mut req = reqwest::Client::new().get(format!("{}/api/auth/me", conv.http_base()));
    if !conv.bearer().is_empty() {
        req = req.header("Authorization", format!("Bearer {}", conv.bearer().trim()));
    }
    let resp = tokio::time::timeout(std::time::Duration::from_secs(10), req.send()).await.ok()?.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: Value = resp.json().await.ok()?;
    let id = v.get("user")?.get("id")?.as_str()?;
    (!id.trim().is_empty() && id.len() <= 1_024).then(|| id.to_owned())
}

/// Bind the durable drafts for a fresh connection (spawned once per
/// connection by the host).
pub async fn bind_connection(conv: &crate::flow::Conversation) -> Option<String> {
    let principal = resolve_principal(conv).await?;
    let scope_value = scope(&conv.http_base(), &principal);
    bind(&scope_value, storage())
}

/// The authority epoch whose durable binding was already started (one REST
/// principal read per connection).
static BOUND_EPOCH: Mutex<Option<u64>> = Mutex::new(None);

/// Whether a connection with this authority epoch still needs its binding
/// started (marks it started).
pub fn needs_bind(epoch: u64) -> bool {
    let mut g = BOUND_EPOCH.lock().unwrap_or_else(|p| p.into_inner());
    if *g == Some(epoch) {
        return false;
    }
    *g = Some(epoch);
    true
}

/// The composer's Session key, once the Session's workspace is known (the
/// key is `[workspace, profile, session]`; before the open reply names the
/// workspace there is nothing to file under).
pub fn active_key(conv: &crate::flow::Conversation) -> Option<String> {
    let session = conv.store.active_session()?;
    conv.store.domains.session.workspace_root(&session)?;
    Some(key_of(conv, &session))
}

static STORAGE: Mutex<Option<Arc<dyn Storage>>> = Mutex::new(None);

/// Install the store (tests); the app uses the recents file store.
pub fn set_storage(s: Arc<dyn Storage>) {
    *STORAGE.lock().unwrap_or_else(|p| p.into_inner()) = Some(s);
}

fn storage() -> Arc<dyn Storage> {
    if let Some(s) = STORAGE.lock().unwrap_or_else(|p| p.into_inner()).clone() {
        return s;
    }
    crate::screens::recents::store()
}

/// Test seam.
pub fn reset() {
    *DRAFTS.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

// A21 failing-first stubs (main's behaviour).
pub fn on_new_connection(_conv: &crate::flow::Conversation) {}
pub fn forget(_scope: Option<(&str, &str)>) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::recents::MemoryStore;

    #[test]
    fn the_cache_bounds_and_evicts_the_oldest() {
        let mut c = DraftCache::default();
        for i in 0..MAX_DRAFTS {
            assert_eq!(c.set(&format!("k{i}"), "x"), None);
        }
        assert_eq!(c.set("new", "y").as_deref(), Some("k0"), "the oldest goes");
        assert_eq!(c.len(), MAX_DRAFTS);
        c.set("new", "");
        assert!(c.get("new").is_none(), "empty removes");
    }

    #[test]
    fn parse_skips_bad_entries_and_keeps_newest_duplicates() {
        let v = json!([["a", "1"], ["b", 2], ["", "x"], ["a", "2"], "junk"]);
        assert_eq!(parse_drafts(&v), vec![("a".to_owned(), "2".to_owned())]);
    }

    #[test]
    fn a_switch_files_the_old_draft_and_restores_the_new_sessions_own() {
        let _g = crate::screens::theme::test_lock();
        reset();
        assert_eq!(follow(Some("A"), ""), None);
        assert_eq!(follow(Some("A"), "half a thought"), None, "an edit is filed");
        assert_eq!(follow(Some("B"), "half a thought").as_deref(), Some(""), "a new Session starts empty");
        assert_eq!(get("A").as_deref(), Some("half a thought"));
        follow(Some("B"), "other");
        assert_eq!(follow(Some("A"), "other").as_deref(), Some("half a thought"), "A's own draft comes back");
        follow(Some("A"), "");
        assert_eq!(get("A"), None, "a send clears its Session's entry");
    }

    #[test]
    fn durable_drafts_survive_a_restart_per_scope_and_a_corrupt_scope_fails_closed() {
        let _g = crate::screens::theme::test_lock();
        reset();
        let store: Arc<dyn Storage> = Arc::new(MemoryStore::new());
        let s1 = scope("http://127.0.0.1:50190", "user-1");
        follow(Some("A"), "");
        bind(&s1, store.clone());
        follow(Some("A"), "unsent text");
        // "Restart": a fresh process state, the same scope.
        reset();
        follow(Some("A"), "");
        assert_eq!(bind(&s1, store.clone()).as_deref(), Some("unsent text"), "the draft comes back");
        // Another principal sees nothing.
        reset();
        follow(Some("A"), "");
        assert_eq!(bind(&scope("http://127.0.0.1:50190", "user-2"), store.clone()), None);
        // A corrupt scope is read as nothing and never overwritten.
        let mem = MemoryStore::new();
        mem.put_raw(&storage_key(&s1), "[[\"A\", 5]]");
        assert!(load(&mem, &s1).is_none());
    }
}
