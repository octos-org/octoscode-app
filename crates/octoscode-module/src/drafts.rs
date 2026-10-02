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
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

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

fn read_all() -> BTreeMap<String, String> {
    let Some(p) = path() else { return BTreeMap::new() };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save (or, for blank text, clear) `session`'s unsent text.
pub fn save(session: &str, text: &str) {
    if session.is_empty() {
        return;
    }
    let Some(p) = path() else { return };
    let mut all = read_all();
    let changed = if text.trim().is_empty() {
        all.remove(session).is_some()
    } else if all.get(session).map(String::as_str) != Some(text) {
        all.insert(session.to_owned(), text.to_owned());
        true
    } else {
        false
    };
    if !changed {
        return;
    }
    let ok = (|| -> std::io::Result<()> {
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&all).unwrap_or_default())?;
        std::fs::rename(&tmp, &p)
    })()
    .is_ok();
    if !ok {
        ::log::warn!("octoscode: the composer draft could not be saved on this device");
    }
    *SAVED.lock().unwrap_or_else(|p| p.into_inner()) = ok;
}

/// The saved unsent text of `session`, if any.
pub fn load(session: &str) -> Option<String> {
    read_all().remove(session).filter(|t| !t.trim().is_empty())
}

// A21 failing-first stubs (main's behaviour: no identity scope).
pub const MAX_DRAFTS: usize = 50;
pub fn attach(_server: &str, _token: &str) {}
pub fn remember_principal(_principal: &str) {}
pub fn principal_scope() -> Option<(String, String)> {
    None
}
pub fn forget() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draft_survives_a_restart_and_a_sent_one_stays_cleared() {
        let dir = std::env::temp_dir().join(format!("a7-drafts-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("drafts.json"));
        save("s1", "half-written prompt\nsecond line");
        save("s2", "other");
        assert_eq!(load("s1").as_deref(), Some("half-written prompt\nsecond line"), "exact text");
        assert!(last_write_ok());
        save("s1", "");
        assert_eq!(load("s1"), None, "an admitted prompt clears its draft");
        assert_eq!(load("s2").as_deref(), Some("other"), "other Sessions keep theirs");
        save("s2", "   ");
        assert_eq!(load("s2"), None);
        std::env::remove_var("OCTOSCODE_DRAFTS_FILE");
    }
}
