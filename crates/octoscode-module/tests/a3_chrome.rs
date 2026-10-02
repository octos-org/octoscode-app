//! A3 — the board-2 chrome's production paths, at the wire.
//!
//! A scripted fake WS server (the f12 pattern) records every request's method
//! AND params. The tests drive the SAME one-owner tables the native chrome's
//! clicks route through (`lib.rs::perform_action` → `screens::settings` /
//! `screens::sidebar` → the production client), then assert what reached the
//! wire:
//!
//! 1. The Stop-server gate: a request + cancel round sends NOTHING; only a
//!    confirmed request sends `server/shutdown` with params `{}` — and only
//!    when the server advertised it (GeneralSettingsContent.tsx:129-130).
//! 2. A permission preset sends `permission/profile/set` with the web's
//!    update shape for the active session (App.tsx:1966-1972).
//! 3. The model row selects the NEXT configured model (`profile/llm/select`).
//! 4. Every chrome action id has exactly ONE owner across the module's tables.
//! 5. The sidebar's `session.open` routes the clicked row's STORE index
//!    through the router's own `thread.open`.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::{settings, sidebar};

/// A fake server that records `(method, params)` and advertises `methods`.
struct FakeServer {
    base_url: String,
    received: Arc<Mutex<Vec<(String, Value)>>>,
}

impl FakeServer {
    async fn start(methods: Vec<&'static str>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (mut tx, mut rx_in) = ws.split();
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].clone();
                rx.lock().unwrap().push((method.clone(), v["params"].clone()));
                let result = match method.as_str() {
                    "session/open" => {
                        let session = v["params"]["session_id"].as_str().unwrap_or("a3:main");
                        json!({"opened": {
                            "session_id": session,
                            "active_profile_id": "a3",
                            "workspace_root": "/home/user/src/octos",
                            "cursor": {"stream": "a3", "seq": 1},
                            "capabilities": {
                                "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                                "capabilities_schema_version": 1,
                                "supported_methods": methods,
                                "supported_notifications": ["turn/started"],
                                "supported_features": []
                            }
                        }})
                    }
                    "profile/llm/select" => json!({"runtime_disposition": "reloaded"}),
                    _ => json!({}),
                };
                let frame = json!({"jsonrpc": "2.0", "id": id, "result": result});
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { base_url, received }
    }

    fn methods(&self) -> Vec<String> {
        self.received.lock().unwrap().iter().map(|(m, _)| m.clone()).collect()
    }

    fn params_of(&self, method: &str) -> Option<Value> {
        self.received
            .lock()
            .unwrap()
            .iter()
            .find(|(m, _)| m == method)
            .map(|(_, p)| p.clone())
    }
}

/// Connect + open, then fold events until the open reply (which carries the
/// advertised methods, flow.rs `set_supported_methods`) lands in the store.
async fn connected(server: &FakeServer) -> Conversation {
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "a3", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    // Fold until the open reply landed AND the connection reported Live (the
    // Live state can arrive after the open reply).
    let mut opened = false;
    for _ in 0..60 {
        if opened && conv.store.is_live() {
            break;
        }
        match tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(evt)) => {
                if matches!(conv.on_event(evt), FlowEvent::WorkspaceOpened(_)) {
                    opened = true;
                }
            }
            _ => break,
        }
    }
    conv
}

/// The settings/sidebar tables are process-global: serialise these tests.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|p| p.into_inner())
}

#[tokio::test]
async fn a_cancelled_stop_never_reaches_the_wire_and_a_confirmed_one_does() {
    let _g = lock();
    settings::reset_state();
    let server = FakeServer::start(vec!["session/open", "server/shutdown"]).await;
    let conv = connected(&server).await;
    let store = conv.store.clone();
    assert!(settings::can_stop_server(&store), "advertised + live: the Stop row shows");

    // The General row raises the dialog; Cancel closes it — no RPC.
    assert_eq!(settings::apply_ui("server.stop.request"), settings::UiEffect::None);
    assert_eq!(settings::apply_ui("server.stop.cancel"), settings::UiEffect::None);
    // A confirm with no open dialog is refused by the gate.
    assert_eq!(settings::apply_ui("server.stop.confirm"), settings::UiEffect::None);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !server.methods().iter().any(|m| m == "server/shutdown"),
        "a cancelled stop must not send: {:?}",
        server.methods()
    );

    // Request -> confirm: exactly the web's call, params {} (config.rs:208-228).
    settings::apply_ui("server.stop.request");
    assert_eq!(settings::apply_ui("server.stop.confirm"), settings::UiEffect::Send);
    settings::perform(&conv, "server.stop.confirm", &store)
        .await
        .expect("server/shutdown answered");
    settings::note_sent("server.stop.confirm");
    assert_eq!(server.params_of("server/shutdown"), Some(json!({})));
    assert!(!settings::snapshot().stop_pending, "the dialog closed on success");
}

#[tokio::test]
async fn an_unadvertised_stop_is_never_offered_nor_sent() {
    let _g = lock();
    settings::reset_state();
    let server = FakeServer::start(vec!["session/open"]).await;
    let conv = connected(&server).await;
    let store = conv.store.clone();
    assert!(!settings::can_stop_server(&store), "no server/shutdown: no Stop row");
    assert!(settings::action_params("server.stop.confirm", &store).is_none());
    let err = settings::perform(&conv, "server.stop.confirm", &store).await;
    assert!(err.is_err(), "an unmapped confirm must not send");
    assert!(!server.methods().iter().any(|m| m == "server/shutdown"));
}

#[tokio::test]
async fn a_permission_preset_sends_the_webs_update_for_the_active_session() {
    let _g = lock();
    settings::reset_state();
    let server = FakeServer::start(vec!["session/open", "permission/profile/set"]).await;
    let conv = connected(&server).await;
    let store = conv.store.clone();
    assert_eq!(settings::apply_ui("perm_full.select"), settings::UiEffect::Send);
    settings::perform(&conv, "perm_full.select", &store).await.expect("set");
    settings::note_sent("perm_full.select");
    let params = server.params_of("permission/profile/set").expect("sent");
    assert_eq!(params["session_id"], json!(store.active_session().unwrap()));
    assert_eq!(
        params["update"],
        json!({"mode": "danger_full_access", "network": "allow", "approval_policy": "never"})
    );
    assert_eq!(settings::preset_of(&store), Some(settings::Preset::Full), "the radios follow the save");
}

#[tokio::test]
async fn the_model_row_selects_the_next_configured_model() {
    use octoscode_store::domains::profile::ProfileLlmModel;
    let _g = lock();
    settings::reset_state();
    let server = FakeServer::start(vec!["session/open", "profile/llm/select"]).await;
    let conv = connected(&server).await;
    let store = conv.store.clone();
    let model = |m: &str, selected: bool| ProfileLlmModel {
        model: m.into(),
        provider: "deepseek".into(),
        title: m.into(),
        family: Some("deepseek".into()),
        route: Some("r1".into()),
        selected,
        available: true,
    };
    store
        .domains
        .profile
        .set_llm_models(vec![model("v4-flash", true), model("v4-pro", false)]);
    assert_eq!(settings::model_of(&store), "v4-flash");
    settings::perform(&conv, "settings.model.next", &store).await.expect("select");
    let params = server.params_of("profile/llm/select").expect("sent");
    assert_eq!(params["model_id"], json!("v4-pro"));
    assert_eq!(params["family_id"], json!("deepseek"));
    settings::mark_model_selected(&store, "v4-pro");
    assert_eq!(settings::model_of(&store), "v4-pro");
}

#[test]
fn every_chrome_id_has_exactly_one_owner() {
    use octoscode_module::screens::*;
    // Every table the dispatcher consults BEFORE (or instead of) ours: a
    // collision would make the chrome's click land in the wrong owner — the
    // `settings.close` vs `workspace` collision found by the A3 click walk.
    let others: Vec<(&str, fn(&str) -> bool)> = vec![
        ("autonomy", autonomy::is_action),
        ("fleet", fleet::is_action),
        ("workspace", workspace::is_action),
        ("review", review::is_action),
        ("connect", connect::is_action),
        ("sessions", sessions::is_action),
        ("theme", theme::is_action),
        ("research", research::owns),
        ("board3", board3::owns),
        ("history", history::owns),
        ("media", media::owns),
        ("peers", peers::owns),
        ("transcript", transcript::owns),
        ("models", models::owns),
        ("provider", provider::is_action),
        ("browser", browser::is_action),
        ("pairing", pairing::is_action),
    ];
    let ours: Vec<&str> = sidebar::ACTIONS
        .iter()
        .map(|(a, _)| *a)
        .chain(settings::ACTIONS.iter().copied())
        .collect();
    for id in &ours {
        let claimed: Vec<&str> = others.iter().filter(|(_, f)| f(id)).map(|(n, _)| *n).collect();
        assert!(claimed.is_empty(), "{id} is also claimed by {claimed:?}");
        assert!(
            !(sidebar::is_action(id) && settings::owns(id)),
            "{id} is claimed by both chrome tables"
        );
    }
}

#[test]
fn a_sidebar_row_click_opens_the_rows_store_session() {
    use octoscode_module::actions;
    use octoscode_module::bindings::Ctx;
    use octoscode_module::flow::FlowUi;
    use octoscode_store::{Session, Store};
    let _g = lock();
    sidebar::reset_state();
    let store = Arc::new(Store::new());
    let row = |id: &str, t: &str| Session {
        id: id.into(),
        title: Some(t.into()),
        message_count: 1,
        updated_at: Some("2026-10-01T15:39:00Z".into()),
        last_prompt: None,
        active_turn: false,
    };
    store.set_sessions(vec![row("s0", "zeta"), row("s1", "alpha"), row("s2", "beta")]);
    let ui = Arc::new(Mutex::new(FlowUi::default()));
    let ctx = Ctx::new(&store, &ui);
    // The drawn tree carries each row's STORE index; the sidebar routes it…
    let proj = sidebar::project_recorded(&store, sidebar::now_ms(), &[]);
    let idx = proj
        .rows
        .iter()
        .find_map(|r| match r {
            sidebar::Row::Session { title, store_index, .. } if title == "beta" => Some(*store_index),
            _ => None,
        })
        .expect("beta drawn");
    assert_eq!(
        sidebar::resolve("session.open", idx, &ctx),
        sidebar::Effect::OpenSession { index: idx }
    );
    // …and the router's own `thread.open` opens exactly that session.
    assert_eq!(actions::resolve("thread.open", idx, &ctx), actions::Effect::Open("s2".into()));
}

#[test]
fn the_held_banner_classifies_the_driver_record_like_the_web() {
    use octoscode_module::chrome;
    // seat-holder.ts:18-29: internal -> none; external + our id -> self (no
    // banner); external + another id -> foreign (banner).
    assert_eq!(chrome::foreign_holder(&json!({"mode": "internal"}), "me"), None);
    assert_eq!(
        chrome::foreign_holder(&json!({"mode": "external", "binding": {"driver_id": "me", "revision": 3}}), "me"),
        None
    );
    let held = chrome::foreign_holder(
        &json!({"mode": "external", "binding": {"driver_id": "octosense-remote", "revision": 7}}),
        "me",
    )
    .expect("foreign");
    assert_eq!(held.revision, 7);
    chrome::set_held("s9", Some(held));
    let p = chrome::take_over_params("s9");
    assert_eq!(p["session_id"], json!("s9"));
    assert_eq!(p["expected_revision"], json!(7), "CAS on the reported revision");
    assert_eq!(p["lease_seconds"], json!(60));
    chrome::set_held("s9", None);
}
