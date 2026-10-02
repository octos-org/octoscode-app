//! A19 — the remembered connection: the Profile, the Session and the
//! workspace this app last opened on a server, so the next launch goes
//! straight back to them instead of guessing a profile.
//!
//! ## The web
//! The web keeps exactly these three facts in its tab connection state
//! (`src-web/apps/web/src/features/connection/preferences.ts:59-70`
//! `TabConnectionPreferences {sessionId, profileId, cwd}`, bounded at
//! `:23-29`), taken from every committed open (`app/App.tsx:974-992`:
//! `opened.session_id`, `opened.active_profile_id`, `opened.workspace_root`)
//! and saved with the connection (`preferences.ts:99-141`). A fresh
//! connection carries none of them (`connection-bootstrap.ts:17-23`,
//! `profileId: ""`). A reload restores the remembered Session DIRECTLY —
//! `autoStartKind` "restore" (`connection-bootstrap.ts:44-48`),
//! `session.restore(connection)` (`App.tsx:814-845`), `beginConnection(input,
//! true)` -> `openCandidateSession(restoreTarget)` only when `sessionId` and
//! `cwd` are both known (`use-octos-session.ts:2956-2961`, `:2978-3001`), with the
//! remembered `profile_id` on the open (`active-session-runtime.ts:1021-1025`)
//! and no `launch/resolve`. A refused restore clears them
//! (`App.tsx:947-974`); a new identity — another endpoint or another token —
//! starts with none (`ConnectionGate.tsx:266-307`), and Forget clears them
//! (`ConnectionGate.tsx:319-356`).
//!
//! ## Natively
//! A native app has no tab: its unit is the device (see `credentials.rs`),
//! so the facts are kept per server ORIGIN in one small JSON file —
//! `OCTOSCODE_CONNECTION_FILE` (tests, the headless harness, `iso-env.sh`),
//! else the host's app data dir (`design::host_dir`), else
//! `$HOME/.octoscode/connection-v1.json`. It holds no secret (the token is
//! `credentials.rs`'s, per origin) and is written 0600 like it anyway.
//!
//! ## The one-time migration (native only — the web has no legacy state)
//! The previous build never remembered a profile: every connect re-derived
//! one (`OCTOS_PROFILE_ID`, else the solo login's ranked profile list —
//! `Conversation::discover_solo_profile` — else the built-in `octoscode` plus
//! an auto-created `octoscode-<pid>`) and reopened `<profile>:main`. A device
//! that used a server before the upgrade has its address in A1's
//! `last-server` (read when the process starts, before any connect rewrites
//! it) and that origin is not yet in this file's `seen` list (every origin
//! this build has planned a connection to); [`legacy_candidate`] names exactly
//! that server, once, so its first connect after the upgrade can land where
//! the previous build landed (`screens::launch::startup`). Per origin, so a
//! run against another server can never use it up or cancel it.
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The web's bounds (`preferences.ts:23-29`).
const MAX_SESSION_ID: usize = 1_024;
const MAX_PROFILE_ID: usize = 512;
const MAX_CWD: usize = 4_096;

/// One server's remembered open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remembered {
    pub profile_id: String,
    pub session_id: String,
    pub cwd: String,
}

impl Remembered {
    /// The web restores only a complete target (`use-octos-session.ts:2978-2981`:
    /// `config.sessionId && config.cwd`), within its bounds.
    pub fn is_restorable(&self) -> bool {
        let ok = |s: &str, max: usize| !s.trim().is_empty() && s.len() <= max;
        ok(&self.profile_id, MAX_PROFILE_ID) && ok(&self.session_id, MAX_SESSION_ID) && ok(&self.cwd, MAX_CWD)
    }
}

/// The file (see the module doc for the order).
pub fn path() -> PathBuf {
    if let Ok(p) = std::env::var("OCTOSCODE_CONNECTION_FILE") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Some(host) = crate::design::host_dir() {
        return host.join(".octoscode").join("connection-v1.json");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".octoscode").join("connection-v1.json")
}

/// The file's two parts: the remembered opens by origin, and every origin
/// this build has planned a connection to (the migration's "already done").
#[derive(Default)]
struct File {
    servers: serde_json::Map<String, serde_json::Value>,
    seen: Vec<String>,
}

fn read_all(path: &Path) -> File {
    let v = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .filter(|v| v.get("version").and_then(|n| n.as_u64()) == Some(1));
    let Some(v) = v else { return File::default() };
    File {
        servers: v.get("servers").and_then(|s| s.as_object()).cloned().unwrap_or_default(),
        seen: v
            .get("seen")
            .and_then(|s| s.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
    }
}

fn write_all(path: &Path, f: File) -> Result<(), String> {
    let body = serde_json::json!({"version": 1, "servers": f.servers, "seen": f.seen}).to_string();
    crate::credentials::write_private(path, &body).map_err(|e| format!("remember connection: {e}"))
}

/// What `server`'s origin remembers, if it is a complete restore target.
pub fn load_in(path: &Path, server: &str) -> Option<Remembered> {
    let origin = crate::credentials::origin(server)?;
    let v = read_all(path).servers.remove(&origin)?;
    let field = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_owned();
    let r = Remembered { profile_id: field("profile_id"), session_id: field("session_id"), cwd: field("cwd") };
    r.is_restorable().then_some(r)
}

/// Remember `r` for `server`'s origin (an incomplete target is not kept).
pub fn remember_in(path: &Path, server: &str, r: &Remembered) -> Result<(), String> {
    let origin = crate::credentials::origin(server).ok_or_else(|| format!("not a server origin: {server:?}"))?;
    if !r.is_restorable() {
        return Ok(());
    }
    let mut f = read_all(path);
    let next = serde_json::json!({"profile_id": r.profile_id, "session_id": r.session_id, "cwd": r.cwd});
    if f.servers.get(&origin) == Some(&next) && f.seen.contains(&origin) {
        return Ok(());
    }
    if !f.seen.contains(&origin) {
        f.seen.push(origin.clone());
    }
    f.servers.insert(origin, next);
    write_all(path, f)
}

/// Forget `server`'s origin (a refused restore, a new identity, Forget). It
/// stays `seen`: the migration is never offered again for it.
pub fn forget_in(path: &Path, server: &str) {
    let Some(origin) = crate::credentials::origin(server) else { return };
    let mut f = read_all(path);
    if f.servers.remove(&origin).is_some() {
        let _ = write_all(path, f);
    }
}

/// Note that this build planned a connection to `server` (the migration is
/// then never offered for it). Idempotent; writes only when new.
pub fn mark_seen_in(path: &Path, server: &str) -> Result<(), String> {
    let Some(origin) = crate::credentials::origin(server) else { return Ok(()) };
    let mut f = read_all(path);
    if f.seen.contains(&origin) {
        return Ok(());
    }
    f.seen.push(origin);
    write_all(path, f)
}

/// Whether this build already planned a connection to `server`.
pub fn seen_in(path: &Path, server: &str) -> bool {
    crate::credentials::origin(server).is_some_and(|o| read_all(path).seen.contains(&o))
}

// ---- the production store ---------------------------------------------------

pub fn load(server: &str) -> Option<Remembered> {
    load_in(&path(), server)
}

pub fn forget(server: &str) {
    forget_in(&path(), server);
    makepad_widgets::log!(
        "[octoscode] remembered connection forgotten for {}",
        crate::credentials::origin(server).unwrap_or_default()
    );
}

/// A Connect with `token` for `server`: another identity — this origin's
/// stored token differs from the one being used — starts with nothing
/// remembered (the web resets `sessionId`/`profileId`/`cwd` when the endpoint
/// or the token changes, `ConnectionGate.tsx:266-307`). Compared in memory,
/// never logged; returns whether the remembered open was dropped.
pub fn on_connect_identity(server: &str, token: &str) -> bool {
    // A21 — the tab envelope holds ONE endpoint's restore hints
    // (`preferences.ts:59-70`, replaced whole on an identity change,
    // `:118-135`): another origin's remembered open goes with its identity.
    keep_only_in(&path(), server);
    if crate::credentials::token_for(server).unwrap_or_default() == token.trim() {
        return false;
    }
    let had = load(server).is_some();
    if had {
        forget(server);
    }
    had
}

/// A21 — drop every remembered open except `server`'s origin (`seen` stays:
/// the migration marker is not a restore hint). Writes only when one went.
pub fn keep_only_in(path: &Path, server: &str) {
    let keep = crate::credentials::origin(server);
    let mut f = read_all(path);
    let before = f.servers.len();
    f.servers.retain(|origin, _| Some(origin) == keep.as_ref());
    if f.servers.len() != before {
        let _ = write_all(path, f);
    }
}

/// A21 — Forget clears the whole tab envelope's restore hints
/// (`ConnectionGate.tsx:319-356`, `clearConnectionPreferences`), every
/// origin's (`seen` stays).
pub fn forget_all() {
    let p = path();
    let mut f = read_all(&p);
    if !f.servers.is_empty() {
        f.servers.clear();
        let _ = write_all(&p, f);
    }
}

/// The open reply's three facts (`App.tsx:974-992`), kept for `server`.
pub fn note_opened(server: &str, profile: &str, session: &str, cwd: Option<&str>) {
    let r = Remembered {
        profile_id: profile.trim().to_owned(),
        session_id: session.trim().to_owned(),
        cwd: cwd.unwrap_or("").trim().to_owned(),
    };
    match remember_in(&path(), server, &r) {
        Ok(()) if r.is_restorable() => makepad_widgets::log!(
            "[octoscode] remembered for {}: profile={} session={}",
            crate::credentials::origin(server).unwrap_or_default(),
            r.profile_id,
            r.session_id
        ),
        Ok(()) => {}
        Err(e) => makepad_widgets::log!("[octoscode] {e}"),
    }
}

// ---- the migration marker ---------------------------------------------------

/// `Some(Some(origin))` = A1's `last-server` when this process started (the
/// previous build's server); `Some(None)` = none. Taken once, before any
/// connect rewrites `last-server`.
static LEGACY: Mutex<Option<Option<String>>> = Mutex::new(None);

/// The pure rule: `server` is the previous build's server, this build has not
/// planned a connection to it yet, and nothing is remembered for it.
pub fn legacy_origin_in(path: &Path, last_server: Option<&str>, server: &str) -> Option<String> {
    let origin = crate::credentials::origin(server)?;
    let last = last_server.and_then(crate::credentials::origin)?;
    (last == origin && !seen_in(path, server) && load_in(path, server).is_none()).then_some(origin)
}

/// Record the previous build's server for this process (lib.rs `start`, first).
pub fn note_process_start() {
    let mut g = LEGACY.lock().unwrap_or_else(|p| p.into_inner());
    if g.is_none() {
        *g = Some(crate::credentials::last_server().and_then(|s| crate::credentials::origin(&s)));
    }
}

/// Whether this connect to `server` is its one-time migration. Every origin
/// planned here is marked seen, so it is true at most once per origin.
pub fn legacy_candidate(server: &str) -> bool {
    let last = LEGACY.lock().unwrap_or_else(|p| p.into_inner()).clone().flatten();
    let hit = legacy_origin_in(&path(), last.as_deref(), server).is_some();
    if hit {
        makepad_widgets::log!(
            "[octoscode] connection memory: the first connect after the upgrade to {} (the one-time migration)",
            crate::credentials::origin(server).unwrap_or_default()
        );
    }
    mark_seen(server);
    hit
}

/// [`mark_seen_in`] on the production file.
pub fn mark_seen(server: &str) {
    if let Err(e) = mark_seen_in(&path(), server) {
        makepad_widgets::log!("[octoscode] {e}");
    }
}

/// Test seam: set (or clear) the process's previous-build server.
pub fn set_legacy_for_test(origin: Option<&str>) {
    *LEGACY.lock().unwrap_or_else(|p| p.into_inner()) = Some(origin.map(str::to_owned));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("octoscode-remembered-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("connection-v1.json")
    }

    fn r(p: &str, s: &str, c: &str) -> Remembered {
        Remembered { profile_id: p.into(), session_id: s.into(), cwd: c.into() }
    }

    /// The web's tab state, per origin: what one server remembers is not
    /// another's, and it survives a fresh read (a relaunch).
    #[test]
    fn an_open_is_remembered_per_origin_across_a_restart() {
        let f = tmp("restart");
        remember_in(&f, "http://127.0.0.1:50190/", &r("dsflash", "dsflash:main", "/home/user/ws")).unwrap();
        remember_in(&f, "http://10.0.0.2:50190", &r("coding-2", "coding-2:api:web-1", "/srv/work")).unwrap();
        assert_eq!(load_in(&f, "http://127.0.0.1:50190"), Some(r("dsflash", "dsflash:main", "/home/user/ws")));
        assert_eq!(load_in(&f, "http://10.0.0.2:50190").unwrap().profile_id, "coding-2");
        assert_eq!(load_in(&f, "http://10.0.0.3:50190"), None, "another server remembers nothing");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&f).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        forget_in(&f, "http://127.0.0.1:50190");
        assert_eq!(load_in(&f, "http://127.0.0.1:50190"), None);
        assert!(load_in(&f, "http://10.0.0.2:50190").is_some(), "forget is per origin");
        let _ = std::fs::remove_dir_all(f.parent().unwrap());
    }

    /// `use-octos-session.ts:2978-2981`: no restore without a session id AND a
    /// workspace; the web's bounds apply.
    #[test]
    fn an_incomplete_target_is_never_kept() {
        let f = tmp("incomplete");
        remember_in(&f, "http://127.0.0.1:1", &r("dsflash", "dsflash:main", "")).unwrap();
        remember_in(&f, "http://127.0.0.1:1", &r("", "x:main", "/w")).unwrap();
        remember_in(&f, "http://127.0.0.1:1", &r("p", "p:main", &"/w".repeat(3000))).unwrap();
        assert_eq!(load_in(&f, "http://127.0.0.1:1"), None);
        assert!(!f.exists(), "nothing was written");
        let _ = std::fs::remove_dir_all(f.parent().unwrap());
    }

    /// The migration names the previous build's server only until this build
    /// has planned a connection to it — per origin, so another server's run
    /// can neither use it up nor cancel it.
    #[test]
    fn the_legacy_candidate_is_the_previous_server_until_this_build_used_it() {
        let f = tmp("legacy");
        let s = "http://127.0.0.1:50190";
        assert_eq!(legacy_origin_in(&f, Some("http://127.0.0.1:50190/x"), s).as_deref(), Some(s));
        assert_eq!(legacy_origin_in(&f, None, s), None, "a fresh install has no previous server");
        assert_eq!(legacy_origin_in(&f, Some(s), "http://127.0.0.1:50082"), None, "only that server");
        // Another server's run (a fixture, the dead default) leaves it alone.
        mark_seen_in(&f, "http://127.0.0.1:8422").unwrap();
        remember_in(&f, "http://127.0.0.1:8433", &r("a", "a:main", "/w")).unwrap();
        assert_eq!(legacy_origin_in(&f, Some(s), s).as_deref(), Some(s));
        // Once this build planned a connection to it, never again.
        mark_seen_in(&f, s).unwrap();
        assert_eq!(legacy_origin_in(&f, Some(s), s), None);
        assert!(seen_in(&f, s) && !seen_in(&f, "http://127.0.0.1:1"));
        let _ = std::fs::remove_dir_all(f.parent().unwrap());
    }
}
