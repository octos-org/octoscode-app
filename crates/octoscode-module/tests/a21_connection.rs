//! A21 — the connection's storage rules on the production paths, the web's way.
//!
//! * Row 199 (`connection-bootstrap.ts:11-48`): the pre-connection bootstrap —
//!   the default endpoint (`VITE_OCTOS_DEFAULT_ENDPOINT` or the page origin),
//!   and the auto-start decision: only a connection this app already had
//!   restores itself (`autoStartKind` "restore"); otherwise the person presses
//!   Connect. `bootstrap::boot` is what `OctoscodeView::start` runs.
//! * Row 196 (`preferences.ts:224-283`, `durable-session-drafts.ts`,
//!   `ConnectionGate.tsx:266-356`): unsent drafts restore only for the
//!   matching identity/principal, an identity change never carries them over,
//!   Forget clears the confirmed principal's drafts, the tab drafts are bounded.
//! * Row 195 (`preferences.ts:11-49`, `:80-143`, `remembered-token.ts`): only
//!   the origin is durable; the token and the restore hints belong to ONE
//!   connection identity (switching servers drops the previous one's), and the
//!   former per-origin device memory is purged at start.
//!
//! Every test runs on its own temp stores (brief §8: never ~/.octoscode).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::credentials;
use octoscode_module::flow::Conversation;
use octoscode_module::screens::bootstrap::{self, AutoStart, Inputs};
use octoscode_module::screens::recents::{MemoryStore, Storage};
use octoscode_module::screens::{a9_settings, drafts as session_drafts, remembered};

const PROFILE: &str = "a8";
const CWD: &str = "/home/user/octos";

/// A fake AppUI server: the WebSocket answers `session/open` (the workspace
/// named, so a Session key exists) and the drafts' REST principal read
/// (`/api/auth/me`, `durable-session-drafts.ts:10-31`) names the user BEHIND
/// THE TOKEN: `alice.1` and `alice.2` are one user (a rotated token), `bob.1`
/// another; no token is a 401.
struct FakeServer {
    base_url: String,
    sockets: Arc<Mutex<usize>>,
}

impl FakeServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let sockets = Arc::new(Mutex::new(0usize));
        let s2 = sockets.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let sockets = s2.clone();
                tokio::spawn(async move {
                    let mut head = [0u8; 2048];
                    let n = stream.peek(&mut head).await.unwrap_or(0);
                    let text = String::from_utf8_lossy(&head[..n]).to_string();
                    if text.starts_with("GET /api/auth/me") {
                        let mut stream = stream;
                        let mut buf = vec![0u8; 2048];
                        let _ = stream.read(&mut buf).await;
                        let token = text
                            .lines()
                            .find_map(|l| {
                                let (k, v) = l.split_once(':')?;
                                (k.trim().eq_ignore_ascii_case("authorization")).then(|| v.trim().to_owned())
                            })
                            .and_then(|v| v.strip_prefix("Bearer ").map(str::to_owned))
                            .unwrap_or_default();
                        let resp = if token.is_empty() {
                            "HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_owned()
                        } else {
                            let user = token.split('.').next().unwrap_or_default();
                            let body = json!({"user": {"id": user}}).to_string();
                            format!(
                                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                                body.len()
                            )
                        };
                        let _ = stream.write_all(resp.as_bytes()).await;
                        let _ = stream.shutdown().await;
                        return;
                    }
                    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                    *sockets.lock().unwrap() += 1;
                    let (mut tx, mut rx) = ws.split();
                    while let Some(Ok(msg)) = rx.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        let p = v["params"].clone();
                        let session = p["session_id"].as_str().unwrap_or("a8:main").to_owned();
                        let result = match method.as_str() {
                            "session/open" => json!({"opened": {
                                "session_id": session, "active_profile_id": PROFILE, "workspace_root": CWD,
                                "cursor": {"stream": session, "seq": 1},
                                "capabilities": {
                                    "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                                    "capabilities_schema_version": 2,
                                    "supported_methods": ["session/open", "session/hydrate", "session/list"],
                                    "supported_notifications": [],
                                    "supported_features": ["state.session_hydrate.v1"]
                                }
                            }}),
                            "session/hydrate" => json!({"session_id": session, "cursor": {"stream": session, "seq": 1}, "messages": []}),
                            "session/list" => json!({"sessions": []}),
                            _ => json!({}),
                        };
                        let frame = json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": result}).to_string();
                        if tx.send(Message::Text(frame.into())).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        Self { base_url, sockets }
    }
}

/// The process-global stores (env paths, the drafts' process state) are
/// shared: one test at a time, each on fresh temp stores.
struct Env {
    _g: std::sync::MutexGuard<'static, ()>,
    dir: std::path::PathBuf,
    store: Arc<MemoryStore>,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn env(name: &str) -> Env {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    let dir = std::env::temp_dir().join(format!("a21-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("OCTOSCODE_CREDENTIALS_DIR", dir.join("cred"));
    std::env::set_var("OCTOSCODE_CONNECTION_FILE", dir.join("connection-v1.json"));
    std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("composer-drafts.json"));
    std::env::set_var("OCTOSCODE_RECENTS_DIR", dir.join("recents"));
    for k in ["OCTOS_BASE_URL", "OCTOS_BEARER", "OCTOS_PROFILE_ID", "OCTOS_PAIRING_LINK"] {
        std::env::remove_var(k);
    }
    session_drafts::reset();
    let store = Arc::new(MemoryStore::new());
    session_drafts::set_storage(store.clone());
    Env { _g: g, dir, store }
}

/// A connection as `token`: the production new-connection hook runs first
/// (lib.rs `sync_labels`, at the first frame of a new authority epoch), then
/// its Session opens (the events folded until it is live).
async fn connect(server: &FakeServer, token: &str) -> (Conversation, tokio::sync::mpsc::Receiver<octos_app_transport::TransportEvent>) {
    let (conv, mut events) = Conversation::connect(&server.base_url, token, PROFILE, None, None).expect("connect");
    session_drafts::on_new_connection(&conv);
    conv.open_workspace(None).await.expect("session/open");
    let mut opened = false;
    for _ in 0..80 {
        if opened && conv.store.is_live() {
            break;
        }
        match tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(evt)) => {
                if matches!(conv.on_event(evt), octoscode_module::flow::FlowEvent::WorkspaceOpened(_)) {
                    opened = true;
                }
            }
            _ => break,
        }
    }
    (conv, events)
}

/// The composer as the person types into it (both stores save every edit:
/// lib.rs `drafts::save` + `screens::drafts::follow`).
fn type_draft(conv: &Conversation, key: &str, text: &str) {
    conv.set_draft(text);
    octoscode_module::drafts::save(&conv.session_id(), text);
    session_drafts::follow(Some(key), text);
}

fn scope_text(store: &MemoryStore, origin: &str, principal: &str) -> String {
    let scope = session_drafts::scope(origin, principal);
    let enc: String = scope
        .bytes()
        .map(|b| {
            let c = b as char;
            if c.is_ascii_alphanumeric() || "-_.!~*'()".contains(c) {
                c.to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    store.get_item(&enc).unwrap_or_default()
}

// ------------------------------------------------------------------ row 196

/// `ConnectionGate.tsx:319-330` forgetConnection ->
/// `clearDurableDrafts(durableDraftScope(endpoint, principal))`, and the tab's
/// drafts go with `clearConnectionPreferences` (`preferences.test.ts:28`):
/// after Forget the SAME principal reconnecting gets nothing back.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forget_clears_the_confirmed_principals_drafts_and_the_tab_drafts() {
    let e = env("forget");
    let server = FakeServer::start().await;
    let (conv, _ev) = connect(&server, "alice.1").await;
    assert_eq!(session_drafts::bind_connection(&conv).await, None);
    let key = session_drafts::active_key(&conv).expect("the open named the workspace");
    session_drafts::follow(Some(&key), "");
    type_draft(&conv, &key, "alice's unsent words");
    assert!(scope_text(&e.store, &server.base_url, "alice").contains("alice's unsent words"), "saved per principal");

    // Settings > Forget server (a9_host::a9_leave_now -> a9_settings::forget_saved).
    a9_settings::forget_saved(&server.base_url);

    assert!(
        !scope_text(&e.store, &server.base_url, "alice").contains("alice's unsent words"),
        "Forget clears the confirmed principal's durable drafts"
    );
    assert_eq!(
        octoscode_module::drafts::load(&conv.session_id()),
        None,
        "Forget clears the tab drafts (clearConnectionPreferences)"
    );
    // The same principal reconnects: nothing comes back.
    let (conv2, _ev2) = connect(&server, "alice.1").await;
    session_drafts::follow(session_drafts::active_key(&conv2).as_deref(), "");
    assert_eq!(session_drafts::bind_connection(&conv2).await, None, "nothing restored after Forget");
    assert_eq!(session_drafts::get(&key), None);
    assert_eq!(octoscode_module::drafts::load(&conv2.session_id()), None);
}

/// `ConnectionGate.tsx:266-300` changeConnection: another identity resets the
/// drafts (`resetIdentity`: the cache cleared, the scope unbound) — a token
/// for ANOTHER principal never sees, or inherits, the previous one's drafts;
/// a rotated token for the SAME principal gets them back
/// (`connection-storage.spec.ts:63`, `deployment.md:123-129`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_token_change_never_carries_drafts_to_another_principal_and_the_same_principal_gets_them_back() {
    let e = env("identity");
    let server = FakeServer::start().await;
    let (alice, _ea) = connect(&server, "alice.1").await;
    session_drafts::bind_connection(&alice).await;
    let key = session_drafts::active_key(&alice).unwrap();
    session_drafts::follow(Some(&key), "");
    type_draft(&alice, &key, "alice's unsent words");

    // Disconnect; Connect with bob's token: a new connection, a new composer.
    let (bob, _eb) = connect(&server, "bob.1").await;
    session_drafts::follow(session_drafts::active_key(&bob).as_deref(), "");
    assert_eq!(session_drafts::bind_connection(&bob).await, None, "bob gets nothing of alice's");
    assert_eq!(session_drafts::get(&key), None, "not in bob's process cache either");
    assert!(
        !scope_text(&e.store, &server.base_url, "bob").contains("alice's unsent words"),
        "alice's drafts never merged into bob's durable scope"
    );
    assert_eq!(octoscode_module::drafts::load(&bob.session_id()), None, "the tab drafts belong to alice's identity");
    assert!(scope_text(&e.store, &server.base_url, "alice").contains("alice's unsent words"), "alice's own scope keeps them");

    // A rotated token for alice: the same principal, its drafts come back.
    let (alice2, _e2) = connect(&server, "alice.2").await;
    session_drafts::follow(session_drafts::active_key(&alice2).as_deref(), "");
    assert_eq!(
        session_drafts::bind_connection(&alice2).await.as_deref(),
        Some("alice's unsent words"),
        "the same principal's draft is restored (never sent)"
    );
}

/// `preferences.test.ts:28` "keeps unsent drafts only in the matching tab
/// identity and clears them on token change or Forget": the native tab drafts
/// (A7's file) restore only for the identity that saved them — an unscoped
/// file restores for nobody — and hold no token.
#[test]
fn the_tab_drafts_restore_only_for_the_matching_identity_and_never_from_an_unscoped_file() {
    let e = env("tab");
    let file = e.dir.join("composer-drafts.json");
    // A7's previous, unscoped file (keyed by Session id only).
    std::fs::write(&file, r#"{"a8:main": "words from another identity"}"#).unwrap();
    assert_eq!(octoscode_module::drafts::load("a8:main"), None, "an unscoped draft restores for nobody");

    let server = "http://127.0.0.1:50190";
    octoscode_module::drafts::attach(server, "first-secret");
    assert_eq!(octoscode_module::drafts::load("a8:main"), None, "not even for the first identity attached");
    octoscode_module::drafts::save("scoped-session", "  私有草稿\n保留空白  ");
    assert_eq!(octoscode_module::drafts::load("scoped-session").as_deref(), Some("  私有草稿\n保留空白  "), "exact text");
    let raw = std::fs::read_to_string(&file).unwrap();
    assert!(!raw.contains("first-secret"), "the file never holds the token");

    // Another token on the same server: another identity — nothing, and the
    // previous identity's drafts are gone with its envelope.
    octoscode_module::drafts::attach(server, "another-secret");
    assert_eq!(octoscode_module::drafts::load("scoped-session"), None);
    octoscode_module::drafts::attach(server, "first-secret");
    assert_eq!(octoscode_module::drafts::load("scoped-session"), None, "cleared on token change");
    // Another server: another identity too.
    octoscode_module::drafts::save("scoped-session", "kept");
    octoscode_module::drafts::attach("https://other.example", "first-secret");
    assert_eq!(octoscode_module::drafts::load("scoped-session"), None);
    // Forget clears them.
    octoscode_module::drafts::attach(server, "first-secret");
    octoscode_module::drafts::save("scoped-session", "kept");
    octoscode_module::drafts::forget();
    assert_eq!(octoscode_module::drafts::load("scoped-session"), None, "cleared on Forget");
    #[cfg(unix)]
    {
        octoscode_module::drafts::save("scoped-session", "kept");
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "owner-only, like the credentials");
    }
}

/// `session-draft-cache.ts:51-69` (+ `connection-storage.spec.ts:298` "a 51st
/// unsent draft evicts the oldest, stays saved"): the tab drafts are bounded
/// at 50; a NEW 51st evicts the oldest; an edit of a kept one evicts nothing;
/// an oversized draft is reported unsaved and never truncated (`:72`).
#[test]
fn the_tab_drafts_are_bounded_and_the_51st_evicts_the_oldest() {
    let _e = env("bound");
    octoscode_module::drafts::attach("http://127.0.0.1:50190", "tok");
    for i in 0..octoscode_module::drafts::MAX_DRAFTS {
        octoscode_module::drafts::save(&format!("s{i}"), &format!("draft {i}"));
    }
    assert_eq!(octoscode_module::drafts::load("s0").as_deref(), Some("draft 0"));
    octoscode_module::drafts::save("s3", "draft 3, edited");
    assert_eq!(octoscode_module::drafts::load("s0").as_deref(), Some("draft 0"), "an edit evicts nothing");
    octoscode_module::drafts::save("s50", "the 51st");
    assert_eq!(octoscode_module::drafts::load("s0"), None, "the oldest went");
    assert_eq!(octoscode_module::drafts::load("s1").as_deref(), Some("draft 1"));
    assert_eq!(octoscode_module::drafts::load("s50").as_deref(), Some("the 51st"), "the new one stays saved");
    assert!(octoscode_module::drafts::last_write_ok());
    let huge = "x".repeat(524_289);
    octoscode_module::drafts::save("s51", &huge);
    assert!(!octoscode_module::drafts::last_write_ok(), "an oversized draft is reported unsaved");
    assert_eq!(octoscode_module::drafts::load("s51"), None, "and never stored truncated");
    assert_eq!(octoscode_module::drafts::load("s50").as_deref(), Some("the 51st"), "the others are untouched");
}

// ------------------------------------------------------------------ row 199

/// `connection-bootstrap.ts:11-15` + `:44-48`: a first launch with nothing
/// remembered shows the Connect card on the default endpoint
/// (`VITE_OCTOS_DEFAULT_ENDPOINT` -> OCTOS_BASE_URL, else the built-in one)
/// and starts NOTHING — the person presses Connect.
#[test]
fn a_first_launch_with_nothing_remembered_prefills_the_default_endpoint_and_never_dials() {
    let _e = env("first");
    let b = bootstrap::boot();
    assert_eq!(b.auto, AutoStart::None, "no unattended dial");
    assert_eq!(b.server, "http://127.0.0.1:50190", "the built-in default endpoint");
    assert_eq!(b.token, None, "the initial draft has no token");
    std::env::set_var("OCTOS_BASE_URL", "http://127.0.0.1:8499");
    let b = bootstrap::boot();
    std::env::remove_var("OCTOS_BASE_URL");
    assert_eq!(b.auto, AutoStart::None, "the build's default endpoint is a prefill, never a dial");
    assert_eq!(b.server, "http://127.0.0.1:8499");
}

/// `autoStartKind` "restore" only for a connection this app already had
/// (`App.tsx:848-852` sets it once authenticated): a remembered server with
/// its token restores; Disconnect stops that (`ConnectionGate.tsx:308-318`,
/// the server and token kept for the card); a new Connect is "restore" again
/// only once it authenticates; Forget leaves nothing to restore.
#[test]
fn a_remembered_server_restores_itself_until_a_disconnect_or_forget() {
    let _e = env("restore");
    let x = "http://127.0.0.1:50190";
    credentials::remember_server(x).unwrap();
    credentials::remember_token(x, "tok-x").unwrap();
    // The previous build's state (no marker yet): it dialed this server at
    // every launch, so the upgrade restores it (A19's migration rides on it).
    let b = bootstrap::boot();
    assert_eq!(b.auto, AutoStart::Restore { server: x.into(), token: "tok-x".into() });

    bootstrap::note_disconnect();
    let b = bootstrap::boot();
    assert_eq!(b.auto, AutoStart::None, "a Disconnect stops the unattended restore");
    assert_eq!((b.server.as_str(), b.token.as_deref()), (x, Some("tok-x")), "the card keeps the server and token");

    // Connect again: not a restore target until the server accepted it.
    bootstrap::adopt_identity(x, "tok-x");
    assert_eq!(bootstrap::boot().auto, AutoStart::None);
    bootstrap::note_authenticated(x);
    assert_eq!(bootstrap::boot().auto, AutoStart::Restore { server: x.into(), token: "tok-x".into() });

    a9_settings::forget_saved(x);
    let b = bootstrap::boot();
    assert_eq!(b.auto, AutoStart::None, "nothing remembered after Forget");
    assert_eq!((b.server.as_str(), b.token.as_deref()), ("http://127.0.0.1:50190", None), "back to the initial draft");

    // A tokenless server whose Session was remembered (A19) restores too.
    let y = "http://127.0.0.1:8433";
    credentials::remember_server(y).unwrap();
    remembered::note_opened(y, "a8", "a8:main", Some(CWD));
    assert_eq!(bootstrap::boot().auto, AutoStart::Restore { server: y.into(), token: String::new() });
}

/// The decision's other inputs (`connection-bootstrap.ts:44-46`): a pairing
/// link drives its own connect, nothing races it; the HARNESS start (no web
/// equivalent — the web's suites press Connect or reload a connected tab) is
/// taken only when a harness hands over a credential or a profile.
#[test]
fn a_pairing_link_wins_and_the_harness_start_needs_a_harness_credential_or_profile() {
    let remembered = Inputs {
        remembered: Some("http://127.0.0.1:50190".into()),
        token: Some("tok".into()),
        ..Default::default()
    };
    assert_eq!(bootstrap::decide(&Inputs { pairing_link: true, ..remembered.clone() }), AutoStart::None);
    let base = Some("http://127.0.0.1:8480".to_owned());
    assert_eq!(
        bootstrap::decide(&Inputs { env_base: base.clone(), env_bearer: Some("walk-dummy-token".into()), ..Default::default() }),
        AutoStart::Harness { server: "http://127.0.0.1:8480".into(), token: "walk-dummy-token".into() }
    );
    assert_eq!(
        bootstrap::decide(&Inputs { env_base: base.clone(), env_profile: true, ..Default::default() }),
        AutoStart::Harness { server: "http://127.0.0.1:8480".into(), token: String::new() }
    );
    assert_eq!(bootstrap::decide(&Inputs { env_base: base, ..Default::default() }), AutoStart::None, "a default endpoint alone never dials");
    assert_eq!(bootstrap::decide(&Inputs::default()), AutoStart::None);
    // The token never reaches a Debug line.
    let r = AutoStart::Restore { server: "http://127.0.0.1:50190".into(), token: "secret-token".into() };
    assert!(!format!("{r:?}").contains("secret-token"));
}

/// The A11 discovery offer and the restore never race: a socket opens only
/// for the restore (or a Connect), and only once.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_restore_dials_its_remembered_server_with_its_token() {
    let _e = env("dial");
    let server = FakeServer::start().await;
    credentials::remember_server(&server.base_url).unwrap();
    credentials::remember_token(&server.base_url, "alice.1").unwrap();
    let b = bootstrap::boot();
    let AutoStart::Restore { server: s, token } = b.auto else { panic!("expected a restore, got {:?}", b.auto) };
    assert_eq!(s, server.base_url);
    // The dial `start` makes for it (Conversation::connect is the transport's).
    let (conv, _ev) = Conversation::connect(&s, &token, "", None, None).expect("connect");
    for _ in 0..50 {
        if *server.sockets.lock().unwrap() > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(*server.sockets.lock().unwrap(), 1, "one socket, to the remembered server");
    drop(conv);
}

// ------------------------------------------------------------------ row 195

/// `preferences.ts:118-135` + `ConnectionGate.tsx:266-307`: ONE connection
/// identity at a time — switching to another server drops the previous
/// server's token and restore hints (the tab envelope is replaced, and
/// `clearRememberedTokens` runs); only the new origin is durable.
#[test]
fn one_identity_at_a_time_switching_servers_drops_the_previous_token_and_restore_hints() {
    let _e = env("one");
    let (x, y) = ("http://127.0.0.1:50190", "http://127.0.0.1:8433");
    // Connected to X (the production Connect path's three writes).
    remembered::on_connect_identity(x, "tok-x");
    credentials::remember_server(x).unwrap();
    credentials::remember_token(x, "tok-x").unwrap();
    remembered::note_opened(x, "a8", "a8:main", Some(CWD));
    // Then Connect to Y.
    remembered::on_connect_identity(y, "tok-y");
    credentials::remember_server(y).unwrap();
    credentials::remember_token(y, "tok-y").unwrap();
    assert_eq!(credentials::token_for(y).as_deref(), Some("tok-y"));
    assert_eq!(credentials::last_server().as_deref(), Some(y), "the durable origin is the new one");
    assert_eq!(credentials::token_for(x), None, "X's token went with its identity");
    assert_eq!(remembered::load(x), None, "X's restore hints went with it");
}

/// `remembered-token.ts:1-15` + `ConnectionGate.tsx:204-206`: the former
/// device-memory feature (a saved token per origin) is purged at every start;
/// only the current connection's own token outlives it (a relaunch is the
/// web's refresh of the same tab, `architecture.md:150-157`). Forget then
/// leaves no token for any origin (`ConnectionGate.tsx:319-356`).
#[test]
fn the_former_device_memory_is_purged_at_start_and_forget_leaves_no_token() {
    let e = env("purge");
    let (x, z) = ("http://127.0.0.1:50190", "http://127.0.0.1:8422");
    let cred = e.dir.join("cred");
    credentials::remember_server_in(&cred, x).unwrap();
    credentials::remember_token_in(&cred, x, "tok-x").unwrap();
    credentials::remember_token_in(&cred, z, "tok-z").unwrap();
    let b = bootstrap::boot();
    assert_eq!(credentials::token_for(z), None, "another origin's saved token is purged at start");
    assert_eq!(credentials::token_for(x).as_deref(), Some("tok-x"), "the connection's own token stays");
    assert_eq!(b.token.as_deref(), Some("tok-x"));
    a9_settings::forget_saved(x);
    let tokens: Vec<_> = std::fs::read_dir(&cred)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".token")).collect())
        .unwrap_or_default();
    assert!(tokens.is_empty(), "no token for any origin after Forget: {tokens:?}");
    assert_eq!(credentials::last_server(), None);
}
