//! A7 — composer draft recovery (`e2e/draft-recovery.spec.ts:21`: "reload
//! restores exact unsent text without dispatching it, and sent drafts stay
//! cleared"; the web keeps per-Session drafts in browser storage,
//! `session/session-composer-drafts.ts`).
//!
//! Natively the unsent text of each Session is kept in
//! `$HOME/.octoscode/composer-drafts.json` (`OCTOSCODE_DRAFTS_FILE`
//! overrides the path), written on every edit and cleared when the prompt is
//! admitted. A restart restores the text into the composer when that Session
//! opens again — never dispatched. A write that fails is reported (the web
//! warns "Draft changes could not be saved on this device", `App.tsx:2835`).
//!
//! ## A21 — the tab's drafts, bound to ONE connection identity (row 196)
//! This file is the native twin of the web's TAB drafts
//! (`preferences.ts:224-283`: `composerDrafts` + `draftPrincipal` inside the
//! tab connection envelope, `:59-70`), so it follows their rules:
//! - the drafts belong to the identity that saved them — the server origin
//!   plus its token (`matchesIdentity`, `:371-384`); they restore only for
//!   that identity ([`load`]) and another identity starts with none: a
//!   connection with another token or server replaces the envelope
//!   (`saveConnectionPreferences`, `:118-135`; [`attach`]);
//! - an unscoped file (the previous A7 format, keyed by Session id only)
//!   restores for nobody;
//! - the identity is kept as a SHA-256 digest, never the token, and the file
//!   is owner-only (0600) like the credentials;
//! - bounded like the web's `SessionDraftCache` (`session-draft-cache.ts`):
//!   50 drafts, the oldest evicted by a NEW 51st; 512 Ki chars; a draft that
//!   does not fit is reported unsaved and never truncated
//!   (`preferences.test.ts:72`);
//! - it remembers the identity's confirmed principal for an offline Forget
//!   (`loadDraftPrincipal` / `rememberDraftPrincipal`, `:244-264`);
//! - Forget removes it (`clearConnectionPreferences`, `:143-159`).
//!
//! The principal-scoped DURABLE drafts (the web's localStorage, restored for
//! the same user after REST auth, a rotated token included) are A8's
//! `screens::drafts`.
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// `MAX_DRAFTS` (`session-draft-cache.ts:1`).
pub const MAX_DRAFTS: usize = 50;
/// The batch bound (`session-draft-cache.ts:27`, key + text characters).
pub const MAX_CHARS: usize = 524_288;
/// A Session key longer than this is not one (`session-draft-cache.ts:21`).
const MAX_KEY: usize = 16_384;
const VERSION: u64 = 2;

/// The drafts file.
pub fn path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("OCTOSCODE_DRAFTS_FILE") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".octoscode/composer-drafts.json"))
}

/// Whether the last write reached the disk (`App.tsx:2835`'s warning).
static SAVED: Mutex<bool> = Mutex::new(true);

pub fn last_write_ok() -> bool {
    *SAVED.lock().unwrap_or_else(|p| p.into_inner())
}

fn set_saved(ok: bool) {
    *SAVED.lock().unwrap_or_else(|p| p.into_inner()) = ok;
}

/// The identity this process's composer is bound to — its digest and its
/// server origin — set at every new connection ([`attach`]). `None` before
/// any connection.
static IDENTITY: Mutex<Option<(String, String)>> = Mutex::new(None);

fn current_identity() -> Option<(String, String)> {
    IDENTITY.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// The identity of a connection: a digest of its server origin and token
/// (the web compares the two, `matchesIdentity`). The token never leaves.
pub fn identity_of(server: &str, token: &str) -> String {
    let origin = crate::credentials::origin(server).unwrap_or_else(|| server.trim().to_owned());
    let mut h = Sha256::new();
    h.update(b"octoscode-tab-identity/v1\n");
    h.update(origin.as_bytes());
    h.update(b"\n");
    h.update(token.trim().as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The file's envelope: one identity, its principal, its drafts (oldest
/// first, the web's tuple list).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Tab {
    identity: String,
    origin: String,
    principal: Option<String>,
    drafts: Vec<(String, String)>,
}

/// The stored envelope, or `None` for a missing, corrupt or unscoped
/// (previous-format) file — none of which restores anything.
fn read_tab() -> Option<Tab> {
    let p = path()?;
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?;
    if v.get("version").and_then(Value::as_u64) != Some(VERSION) {
        return None;
    }
    let identity = v.get("identity")?.as_str()?.to_owned();
    let drafts = v
        .get("drafts")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|e| {
                    let pair = e.as_array().filter(|p| p.len() == 2)?;
                    Some((pair[0].as_str()?.to_owned(), pair[1].as_str()?.to_owned()))
                })
                .filter(|(k, _)| !k.is_empty() && k.chars().count() <= MAX_KEY)
                .collect()
        })
        .unwrap_or_default();
    Some(Tab {
        identity,
        origin: v.get("origin").and_then(Value::as_str).unwrap_or_default().to_owned(),
        principal: v
            .get("principal")
            .and_then(Value::as_str)
            .filter(|p| !p.trim().is_empty() && p.len() <= 1_024)
            .map(str::to_owned),
        drafts,
    })
}

fn write_tab(tab: &Tab) -> bool {
    let Some(p) = path() else { return false };
    let mut v = json!({
        "version": VERSION,
        "identity": tab.identity,
        "origin": tab.origin,
        "drafts": tab.drafts.iter().map(|(k, t)| json!([k, t])).collect::<Vec<_>>(),
    });
    if let Some(pr) = &tab.principal {
        v["principal"] = json!(pr);
    }
    match crate::credentials::write_private(&p, &v.to_string()) {
        Ok(()) => true,
        Err(e) => {
            ::log::warn!("octoscode: the composer draft could not be saved on this device ({e})");
            false
        }
    }
}

/// The envelope of the attached identity (a fresh one when the file belongs
/// to another identity or is unreadable).
fn tab_for((identity, origin): &(String, String)) -> Tab {
    read_tab()
        .filter(|t| &t.identity == identity)
        .unwrap_or_else(|| Tab { identity: identity.clone(), origin: origin.clone(), ..Tab::default() })
}

/// A21 — a new connection: bind the composer's drafts to its identity. The
/// file of another identity is replaced (its drafts and principal go, as the
/// web's tab envelope does on any identity change, `preferences.ts:118-135`);
/// the same identity keeps its own.
pub fn attach(server: &str, token: &str) {
    let identity = identity_of(server, token);
    let origin = crate::credentials::origin(server).unwrap_or_default();
    *IDENTITY.lock().unwrap_or_else(|p| p.into_inner()) = Some((identity.clone(), origin.clone()));
    let exists = path().is_some_and(|p| p.exists());
    match read_tab() {
        Some(t) if t.identity == identity => {}
        _ if !exists => {}
        stale => {
            // Another identity's drafts (or an unscoped file): not this
            // connection's to restore — dropped with that identity.
            let dropped = stale.map(|t| t.drafts.len()).unwrap_or(0);
            if write_tab(&Tab { identity, origin, ..Tab::default() }) {
                makepad_widgets::log!("[octoscode] drafts: a new connection identity — {dropped} tab draft(s) of the previous one dropped");
            }
        }
    }
}

/// Save (or, for blank text, clear) `session`'s unsent text under the
/// attached identity (`saveComposerDrafts`, `preferences.ts:266-281`: no
/// identity, no write). Bounded (see the module doc).
pub fn save(session: &str, text: &str) {
    if session.is_empty() {
        return;
    }
    let Some(identity) = current_identity() else { return };
    let mut tab = tab_for(&identity);
    let blank = text.trim().is_empty();
    let pos = tab.drafts.iter().position(|(k, _)| k == session);
    let changed = match (blank, pos) {
        (true, Some(i)) => {
            tab.drafts.remove(i);
            true
        }
        (true, None) => false,
        (false, Some(i)) if tab.drafts[i].1 == text => false,
        (false, Some(i)) => {
            tab.drafts[i].1 = text.to_owned();
            true
        }
        (false, None) => {
            // `SessionDraftCache.set`: a NEW key at capacity evicts the oldest.
            if tab.drafts.len() >= MAX_DRAFTS {
                let (evicted, _) = tab.drafts.remove(0);
                ::log::info!("octoscode: tab draft for {evicted} evicted (bound {MAX_DRAFTS})");
            }
            tab.drafts.push((session.to_owned(), text.to_owned()));
            true
        }
    };
    if !changed {
        return;
    }
    let size: usize = tab.drafts.iter().map(|(k, t)| k.chars().count() + t.chars().count()).sum();
    if size > MAX_CHARS {
        // `parseSessionDrafts` would drop it: report unsaved, never truncate.
        ::log::warn!("octoscode: the composer draft is too large to save on this device");
        set_saved(false);
        return;
    }
    set_saved(write_tab(&tab));
}

/// The saved unsent text of `session` — only for the identity that saved it
/// (`loadComposerDrafts`, `preferences.ts:234-242`).
pub fn load(session: &str) -> Option<String> {
    let (identity, _) = current_identity()?;
    read_tab()
        .filter(|t| t.identity == identity)?
        .drafts
        .into_iter()
        .find(|(k, _)| k == session)
        .map(|(_, t)| t)
        .filter(|t| !t.trim().is_empty())
}

/// `rememberDraftPrincipal` (`preferences.ts:255-264`): the attached
/// identity's confirmed principal, for a Forget made while offline.
pub fn remember_principal(principal: &str) {
    let Some(identity) = current_identity() else { return };
    let mut tab = tab_for(&identity);
    if tab.principal.as_deref() == Some(principal) {
        return;
    }
    tab.principal = Some(principal.to_owned());
    write_tab(&tab);
}

/// The envelope's `(origin, principal)` — whatever identity it holds (Forget
/// clears the one envelope there is; `loadDraftPrincipal`, `:244-253`).
pub fn principal_scope() -> Option<(String, String)> {
    let t = read_tab()?;
    Some((t.origin, t.principal?)).filter(|(o, _)| !o.is_empty())
}

/// Forget (`clearConnectionPreferences`): the file goes.
pub fn forget() {
    if let Some(p) = path() {
        if p.exists() {
            let _ = std::fs::remove_file(&p);
            makepad_widgets::log!("[octoscode] drafts: the tab drafts were forgotten");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draft_survives_a_restart_and_a_sent_one_stays_cleared() {
        let dir = std::env::temp_dir().join(format!("a7-drafts-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("drafts.json"));
        // A21: the drafts belong to a connection identity.
        attach("http://127.0.0.1:50190", "tok");
        save("s1", "half-written prompt\nsecond line");
        save("s2", "other");
        // "Restart": the same identity attached again.
        attach("http://127.0.0.1:50190", "tok");
        assert_eq!(load("s1").as_deref(), Some("half-written prompt\nsecond line"), "exact text");
        assert!(last_write_ok());
        save("s1", "");
        assert_eq!(load("s1"), None, "an admitted prompt clears its draft");
        assert_eq!(load("s2").as_deref(), Some("other"), "other Sessions keep theirs");
        save("s2", "   ");
        assert_eq!(load("s2"), None);
        std::env::remove_var("OCTOSCODE_DRAFTS_FILE");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_identity_is_a_digest_of_origin_and_token() {
        let a = identity_of("http://127.0.0.1:50190/", "tok");
        assert_eq!(a, identity_of("HTTP://127.0.0.1:50190", " tok "), "the origin and trimmed token");
        assert_ne!(a, identity_of("http://127.0.0.1:50190", "tok2"));
        assert_ne!(a, identity_of("http://127.0.0.1:50191", "tok"));
        assert_eq!(a.len(), 64);
        assert!(!a.contains("tok"));
    }
}
