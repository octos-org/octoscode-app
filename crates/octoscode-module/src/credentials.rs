//! A1 — the connect screen's access token, remembered per server ORIGIN.
//!
//! The Connect card says "Stored for this server only" (board 4 frame 4,
//! `design/stage-b/setup/cards/setup-01`), but nothing stored it: the token
//! field was never even read, and a restart always came back empty.
//!
//! The web splits the two (`src-web/apps/web/src/features/connection/
//! preferences.ts:11-49`): the server address is durable (localStorage), the
//! token lives in tab-scoped sessionStorage — a browser tab is the web's unit
//! of "this session". A native app has no tab; its unit is the device, and
//! the card's own copy promises per-server storage. So:
//!
//! * the last server address is remembered (the web's durable endpoint);
//! * the token is remembered PER ORIGIN (`scheme://host[:port]`, the web's
//!   `connectionEndpointError` already refuses credentials in the address),
//!   in the app's data dir, one file per origin, mode 0600 in a 0700 dir;
//! * a token the server REFUSED is forgotten (it would otherwise prefill a
//!   known-bad credential forever);
//! * nothing here ever logs a token — only the origin.
//!
//! The directory: `OCTOSCODE_CREDENTIALS_DIR` (tests, the headless harness),
//! else the host's app data dir (Android's files dir, handed over through
//! `design::host_dir`), else `$HOME/.octoscode/credentials`.
use std::io::Write;
use std::path::{Path, PathBuf};

/// The credentials directory (see the module doc for the order).
pub fn dir() -> PathBuf {
    if let Ok(d) = std::env::var("OCTOSCODE_CREDENTIALS_DIR") {
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    if let Some(host) = crate::design::host_dir() {
        return host.join(".octoscode").join("credentials");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".octoscode").join("credentials")
}

/// The origin a token belongs to: lowercased `scheme://host[:port]`, default
/// ports dropped, any path/query ignored. `None` for an address that does not
/// parse as an http(s)/ws(s) URL.
pub fn origin(server: &str) -> Option<String> {
    let url = url::Url::parse(server.trim()).ok()?;
    let scheme = url.scheme().to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "ws" | "wss") {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    let port = url
        .port()
        .map(|p| format!(":{p}"))
        .unwrap_or_default();
    Some(format!("{scheme}://{host}{port}"))
}

/// One origin's token file: the origin hex-encoded, so any host (IPv6, ports)
/// is a single safe path segment.
fn token_file(dir: &Path, origin: &str) -> PathBuf {
    let hex: String = origin.bytes().map(|b| format!("{b:02x}")).collect();
    dir.join(format!("{hex}.token"))
}

fn last_server_file(dir: &Path) -> PathBuf {
    dir.join("last-server")
}

/// Write `contents` to `path` readable by the owner only (0600), inside a
/// 0700 directory. The mode is set at CREATE time, so the bytes never sit on
/// disk world-readable even for an instant.
fn write_private(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let tmp = path.with_extension("tmp");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)
}

/// Remember `token` for `server`'s origin (an empty token forgets it).
/// Logs the origin only.
pub fn remember_token_in(dir: &Path, server: &str, token: &str) -> Result<(), String> {
    let origin = origin(server).ok_or_else(|| format!("not a server origin: {server:?}"))?;
    let token = token.trim();
    if token.is_empty() {
        forget_token_in(dir, server);
        return Ok(());
    }
    write_private(&token_file(dir, &origin), token).map_err(|e| format!("store token for {origin}: {e}"))
}

/// The token remembered for `server`'s origin, if any.
pub fn token_for_in(dir: &Path, server: &str) -> Option<String> {
    let origin = origin(server)?;
    let t = std::fs::read_to_string(token_file(dir, &origin)).ok()?;
    let t = t.trim().to_owned();
    (!t.is_empty()).then_some(t)
}

/// Forget `server`'s token (a refused credential, or an emptied field).
pub fn forget_token_in(dir: &Path, server: &str) {
    if let Some(origin) = origin(server) {
        let _ = std::fs::remove_file(token_file(dir, &origin));
    }
}

/// Remember the last server address (the web's durable endpoint).
pub fn remember_server_in(dir: &Path, server: &str) -> Result<(), String> {
    write_private(&last_server_file(dir), server.trim()).map_err(|e| format!("store server: {e}"))
}

/// A9 — forget the remembered server address (Settings > Forget server).
pub fn forget_server_in(dir: &Path) {
    let _ = std::fs::remove_file(last_server_file(dir));
}

/// The last server address, if one was remembered.
pub fn last_server_in(dir: &Path) -> Option<String> {
    let s = std::fs::read_to_string(last_server_file(dir)).ok()?;
    let s = s.trim().to_owned();
    (!s.is_empty()).then_some(s)
}

// ---- the production store (the app's dir) ----------------------------------

pub fn remember_token(server: &str, token: &str) -> Result<(), String> {
    remember_token_in(&dir(), server, token)
}

pub fn token_for(server: &str) -> Option<String> {
    token_for_in(&dir(), server)
}

pub fn forget_token(server: &str) {
    forget_token_in(&dir(), server)
}

pub fn remember_server(server: &str) -> Result<(), String> {
    remember_server_in(&dir(), server)
}

pub fn last_server() -> Option<String> {
    last_server_in(&dir())
}

pub fn forget_server() {
    forget_server_in(&dir())
}

/// The connect screen's prefill at start: the last server and ITS token.
pub fn prefill() -> (Option<String>, Option<String>) {
    let server = last_server();
    let token = server.as_deref().and_then(token_for);
    (server, token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("octoscode-cred-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn the_origin_is_scheme_host_port_lowercased_without_default_ports() {
        assert_eq!(origin("http://127.0.0.1:50190").as_deref(), Some("http://127.0.0.1:50190"));
        assert_eq!(origin("HTTP://Example.COM/x?y").as_deref(), Some("http://example.com"));
        assert_eq!(origin("https://example.com:443/").as_deref(), Some("https://example.com"));
        assert_eq!(origin("ftp://x").as_deref(), None);
        assert_eq!(origin("not a url"), None);
    }

    /// The bug: "Stored for this server only" — but a restart came back with
    /// an empty token field. The token survives a fresh read of the store,
    /// is per origin, and the file is owner-only.
    #[test]
    fn a_token_is_remembered_per_origin_across_a_restart() {
        let d = tmp("restart");
        remember_server_in(&d, "http://127.0.0.1:50190").unwrap();
        remember_token_in(&d, "http://127.0.0.1:50190/", "sk-one").unwrap();
        remember_token_in(&d, "http://10.0.0.2:50190", "sk-two").unwrap();
        // "restart": nothing in memory, only the directory.
        assert_eq!(last_server_in(&d).as_deref(), Some("http://127.0.0.1:50190"));
        assert_eq!(token_for_in(&d, "http://127.0.0.1:50190").as_deref(), Some("sk-one"));
        assert_eq!(token_for_in(&d, "http://10.0.0.2:50190").as_deref(), Some("sk-two"));
        assert_eq!(token_for_in(&d, "http://10.0.0.3:50190"), None, "another server gets nothing");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let f = token_file(&d, "http://127.0.0.1:50190");
            let mode = std::fs::metadata(&f).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "the token file is owner-only");
            let dmode = std::fs::metadata(&d).unwrap().permissions().mode() & 0o777;
            assert_eq!(dmode, 0o700, "the directory is owner-only");
        }
        // Nothing but hex names on disk: the token never names a file.
        for e in std::fs::read_dir(&d).unwrap() {
            let name = e.unwrap().file_name().to_string_lossy().to_string();
            assert!(!name.contains("sk-"), "{name}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn forget_server_removes_the_address_and_keeps_other_origins_tokens() {
        let d = tmp("forget-server");
        remember_server_in(&d, "http://127.0.0.1:50190").unwrap();
        remember_token_in(&d, "http://127.0.0.1:50190", "sk-one").unwrap();
        remember_token_in(&d, "http://10.0.0.2:50190", "sk-two").unwrap();
        // Settings > Forget server: this origin's token and the address.
        forget_token_in(&d, "http://127.0.0.1:50190");
        forget_server_in(&d);
        assert_eq!(last_server_in(&d), None);
        assert_eq!(token_for_in(&d, "http://127.0.0.1:50190"), None);
        assert_eq!(token_for_in(&d, "http://10.0.0.2:50190").as_deref(), Some("sk-two"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_empty_or_refused_token_is_forgotten() {
        let d = tmp("forget");
        remember_token_in(&d, "http://127.0.0.1:50190", "sk-bad").unwrap();
        forget_token_in(&d, "http://127.0.0.1:50190");
        assert_eq!(token_for_in(&d, "http://127.0.0.1:50190"), None);
        remember_token_in(&d, "http://127.0.0.1:50190", "sk-x").unwrap();
        remember_token_in(&d, "http://127.0.0.1:50190", "  ").unwrap();
        assert_eq!(token_for_in(&d, "http://127.0.0.1:50190"), None, "an emptied field forgets");
        let _ = std::fs::remove_dir_all(&d);
    }
}
