//! A20 — the shared fake AppUI server + helpers of the row-247 tests
//! (`a20_resume_workspace.rs`, `a20_saved_link.rs`).
#![allow(dead_code)]
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octos_app_transport::TransportEvent;
use octoscode_module::flow::Conversation;

pub const PROFILE: &str = "a20";
pub const MAIN: &str = "a20:main";
pub const XRAY: &str = "a20:api:xray";
pub const REAL_WS: &str = "/home/user/a20-real-ws";
pub const OTHER_WS: &str = "/home/user/a20-other-ws";
pub const LINK_WS: &str = "/home/user/a20-link-ws";

#[derive(Default)]
pub struct Script {
    /// `session/list {cwd}` attests this canonical root for a cwd.
    pub attest: Vec<(String, String)>,
    /// The open reply's `workspace_root` for one Session regardless of the
    /// requested cwd (a server that resolves its folder elsewhere).
    pub open_root: Option<(String, String)>,
    /// Advertise the scoped catalog (`session.workspace_cwd.v1`).
    pub catalog: bool,
}

#[derive(Clone)]
pub struct Server {
    pub base: String,
    pub seen: Arc<Mutex<Vec<(String, Value)>>>,
    pub script: Arc<Mutex<Script>>,
}

impl Server {
    pub async fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let srv = Server {
            base: format!("http://{}", listener.local_addr().unwrap()),
            seen: Arc::new(Mutex::new(Vec::new())),
            script: Arc::new(Mutex::new(script)),
        };
        let s2 = srv.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let s = s2.clone();
                tokio::spawn(async move {
                    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                    let (mut tx, mut rx) = ws.split();
                    while let Some(Ok(Message::Text(text))) = rx.next().await {
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        let p = v["params"].clone();
                        s.seen.lock().unwrap().push((method.clone(), p.clone()));
                        let frame = json!({"jsonrpc": "2.0", "id": v["id"].clone(), "result": s.answer(&method, &p)});
                        if tx.send(Message::Text(frame.to_string().into())).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        srv
    }

    pub fn answer(&self, method: &str, p: &Value) -> Value {
        let script = self.script.lock().unwrap();
        let session = p["session_id"].as_str().unwrap_or(MAIN).to_owned();
        let mut features = vec!["state.session_hydrate.v1"];
        if script.catalog {
            features.push("session.workspace_cwd.v1");
        }
        match method {
            "session/open" => {
                let root = script
                    .open_root
                    .clone()
                    .filter(|(s, _)| *s == session)
                    .map(|(_, r)| r)
                    .or_else(|| p["cwd"].as_str().map(str::to_owned))
                    .unwrap_or_else(|| REAL_WS.to_owned());
                json!({"opened": {
                    "session_id": session, "active_profile_id": PROFILE, "workspace_root": root,
                    "cursor": {"stream": session, "seq": 1},
                    "capabilities": {
                        "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                        "capabilities_schema_version": 2,
                        "supported_methods": ["session/open", "session/hydrate", "session/list"],
                        "supported_notifications": [],
                        "supported_features": features
                    }
                }})
            }
            "session/hydrate" => json!({"session_id": session, "cursor": {"stream": session, "seq": 2}, "messages": []}),
            "session/list" => {
                let rows = json!([
                    {"id": XRAY, "title": "Saved conversation", "message_count": 2, "updated_at": "2026-10-02T09:00:00Z"},
                    {"id": MAIN, "title": "Main", "message_count": 2, "updated_at": "2026-10-02T08:00:00Z"}
                ]);
                match p["cwd"].as_str() {
                    Some(cwd) if script.catalog => {
                        let root = script.attest.iter().find(|(c, _)| c == cwd).map(|(_, r)| r.clone()).unwrap_or_else(|| cwd.to_owned());
                        json!({"sessions": rows, "workspace_root": root, "profile_id": p["profile_id"]})
                    }
                    _ => json!({"sessions": rows}),
                }
            }
            _ => json!({}),
        }
    }

    pub fn of(&self, method: &str, session: &str) -> Vec<Value> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|(m, p)| m == method && p["session_id"] == json!(session))
            .map(|(_, p)| p.clone())
            .collect()
    }
}

pub type Events = tokio::sync::mpsc::Receiver<TransportEvent>;

pub async fn fold_until(conv: &Conversation, events: &mut Events, secs: u64, done: impl Fn(&Conversation) -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while tokio::time::Instant::now() < deadline {
        if done(conv) {
            return true;
        }
        if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(50), events.recv()).await {
            conv.on_event(evt);
        }
    }
    done(conv)
}

/// A drain that keeps folding transport events in the background (the app's
/// own drain task), so awaited opens see their replies.
pub fn drain(conv: &Arc<Conversation>, mut events: Events) {
    let c = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            c.on_event(evt);
        }
    });
}

/// Connect, open `session` in `cwd`, wait for the reply to be adopted.
pub async fn connect_in(server: &Server, session: &str, cwd: &str) -> (Arc<Conversation>, Events) {
    let (conv, mut events) = Conversation::connect(&server.base, "dummy", PROFILE, None, None).expect("connect");
    let conv = Arc::new(conv);
    conv.attach();
    let opener = {
        let (c, id, cwd) = (conv.clone(), session.to_owned(), cwd.to_owned());
        tokio::spawn(async move { c.open_session(&id, Some(cwd)).await })
    };
    assert!(
        fold_until(&conv, &mut events, 10, |c| c.store.is_live() && c.store.domains.session.workspace_root(session).is_some()).await,
        "{session} opened"
    );
    opener.await.unwrap().expect("open");
    (conv, events)
}

pub async fn open_in(conv: &Arc<Conversation>, events: &mut Events, session: &str, cwd: &str) {
    let opener = {
        let (c, id, cwd) = (conv.clone(), session.to_owned(), cwd.to_owned());
        tokio::spawn(async move { c.open_session(&id, Some(cwd)).await })
    };
    assert!(
        fold_until(conv, events, 10, |c| {
            c.store.active_session().as_deref() == Some(session)
                && c.store.domains.session.workspace_root(session).as_deref() == Some(cwd)
        })
        .await,
        "{session} opened in {cwd}"
    );
    opener.await.unwrap().expect("open");
}

pub fn state_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|p| p.into_inner())
}
