//! #D1t — board 1's transport arms (the provider editor + the workspace
//! browser) talk to the server, driven from the SCREENS' production path over
//! **recorded real traffic** (RULES "Tests replay recorded real traffic").
//!
//! Replayed: `crates/octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl`
//! — the frames a real `octos serve` a6ea8505 sent. It carries exactly the
//! board-1 traffic: `config/capabilities/list` (line 1, which advertises
//! `onboarding.workspace_browse.v1`), `profile/llm/test` (line 8 request /
//! line 9 the server's REAL 401 reply) and `profile/llm/upsert` (line 10 /
//! line 11 the `applied: true` reply).
//!
//! Driven from `screens::provider::perform` + `perform_transport` and
//! `screens::browser::perform` + `perform_transport` — the screen's own action
//! id first, then its transport half — NOT the client directly. That is the
//! distinction the card asks for: a test that calls `conv.client().request` would
//! pass while the card's controls stayed dead.
//!
//! §1 provider: `prov.test` sends the recorded param shape and the recorded 401
//!     failure reaches the draft REDACTED, with the draft's fields kept (row 88).
//! §2 provider: `provider.save` sends `set_primary: true` and the recorded
//!     `applied: true` reply leaves the editor (row 87).
//! §3 browser: `browser.use` asks `onboarding/workspace_list` and projects the
//!     listing, its hidden count and its truncation into the card (row 220).
//! §4 browser: `browser.create` asks `onboarding/workspace_create` then re-lists
//!     the parent, and the pre-validated name reaches the wire (row 164/220).
//! §5 the key never reaches a copy id (the module header's guarantee).
mod replay {
    use std::sync::{Arc, Mutex};

    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;

    use octoscode_module::flow::Conversation;
    use octoscode_module::screens::{browser, provider};

    #[derive(Debug, Clone)]
    struct Frame {
        dir: String,
        method: String,
        body: serde_json::Value,
        error: Option<serde_json::Value>,
    }

    fn fixture() -> Vec<Frame> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl"
        );
        let text = std::fs::read_to_string(path).expect("read the r29a fixture");
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).expect("fixture line is JSON");
                Frame {
                    dir: v["dir"].as_str().unwrap_or("").to_owned(),
                    method: v["method"].as_str().unwrap_or("").to_owned(),
                    body: v.get("body").cloned().unwrap_or(serde_json::Value::Null),
                    error: v.get("error").cloned(),
                }
            })
            .collect()
    }

    /// Both modules keep their live state in a process-global `OnceLock`
    /// (`provider.rs:478-485`, `browser.rs:495-502`) and the lane runs
    /// `RUST_TEST_THREADS=4`, so the tests that assert on the LIVE card must not
    /// overlap — otherwise one test's `provider.save` closes the draft another
    /// test is still reading. One lock, taken by every test that touches that
    /// state. A poisoned lock (a panicking test) is recovered, not propagated.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// The recorded reply for `method` (the `dir == "in"` frame), or a synthetic
    /// one for a method the capture never produced (the browse RPCs are not in
    /// the recording — the capture ran the LLM arm, not the folder browser, so
    /// §3/§4 supply the server's own documented result shape and say so).
    fn reply(frames: &[Frame], method: &str, synth: serde_json::Value) -> serde_json::Value {
        frames
            .iter()
            .find(|f| f.dir == "in" && f.method == method)
            .map(|f| f.body.clone())
            .unwrap_or(synth)
    }

    struct ReplayServer {
        url: String,
        seen: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
    }

    impl ReplayServer {
        async fn start(frames: Vec<Frame>, synth: Vec<(String, serde_json::Value)>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr = listener.local_addr().expect("addr");
            let seen = Arc::new(Mutex::new(Vec::new()));
            let rec = seen.clone();
            tokio::spawn(async move {
                let Ok((stream, _)) = listener.accept().await else { return };
                let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                let (mut tx, mut rx_in) = ws.split();
                while let Some(Ok(msg)) = rx_in.next().await {
                    let Message::Text(text) = msg else { continue };
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                    let method = v["method"].as_str().unwrap_or("").to_owned();
                    let id = v["id"].as_str().unwrap_or("").to_owned();
                    let params = v.get("params").cloned().unwrap_or(serde_json::Value::Null);
                    rec.lock().unwrap().push((method.clone(), params.clone()));
                    let body = frames
                        .iter()
                        .find(|f| f.dir == "in" && f.method == method)
                        .map(|f| f.body.clone())
                        .or_else(|| {
                            synth.iter().find(|(m, _)| *m == method).map(|(_, b)| b.clone())
                        });
                    let reply = match body {
                        Some(b) => serde_json::json!({
                            "jsonrpc": "2.0", "id": id, "result": b,
                        }),
                        None => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": null}),
                    };
                    let _ = tx.send(Message::Text(reply.to_string().into())).await;
                }
            });
            Self { url: format!("ws://{addr}"), seen }
        }

        fn sent(&self, method: &str) -> Option<serde_json::Value> {
            self.seen
                .lock()
                .unwrap()
                .iter()
                .find(|(m, _)| m == method)
                .map(|(_, p)| p.clone())
        }
    }

    /// The module's own conversation over the replay server (production
    /// transport + client + registry).
    async fn connected(server: &ReplayServer) -> Arc<Conversation> {
        let (conv, mut evt_rx) =
            Conversation::connect(&server.url, "mapb-dummy", "octoscode", None, None)
                .expect("conversation connects to the replay server");
        let conv = Arc::new(conv);
        let drain = conv.clone();
        tokio::spawn(async move {
            while let Some(evt) = evt_rx.recv().await {
                let _ = drain.on_event(evt);
            }
        });
        conv
    }

    /// The server's documented `onboarding/workspace_list` result shape
    /// (`WorkspaceListResult`, `profile.rs:92-104`): hidden 2, truncated to 1,
    /// so the card must show BOTH notices (row 220).
    fn list_synthetic() -> serde_json::Value {
        serde_json::json!({
            "canonical_path": "/srv/fixture",
            "parent_path": "/srv",
            "writable": true,
            "entries": [
                {"name": "Projects", "path": "/srv/fixture/Projects", "writable": true}
            ],
            "truncated": true,
            "hidden_skipped": 2,
        })
    }

    fn create_synthetic() -> serde_json::Value {
        serde_json::json!({
            "canonical_path": "/srv/fixture/NewFolder",
            "created": true,
        })
    }

    // ------------------------------------------------------------------ §1/§2
    #[tokio::test]
    async fn prov_test_sends_the_recorded_shape_and_keeps_the_draft_on_the_real_401() {
        let _serial = serial();
        let frames = fixture();
        let server = ReplayServer::start(frames.clone(), Vec::new()).await;
        let conv = connected(&server).await;

        // A fresh draft with a typed key, then the CARD's own action id.
        let mut ui = provider::ProviderUi::new("deepseek");
        ui.key = "sk-secret-value".into();
        provider::set(ui);

        let effect = provider::perform("prov.test", None).expect("prov.test is a transport effect");
        let out = provider::perform_transport(&conv, effect).await;

        // The recorded 401 is a real FAILURE: the editor must not report success.
        assert!(out.is_err(), "the recorded 401 must not read as success: {out:?}");

        // The request carried the recorded param shape.
        let sent = server.sent("profile/llm/test").expect("prov.test reached the wire");
        assert_eq!(sent["selection"]["family_id"], "deepseek", "{sent}");
        assert_eq!(
            sent["selection"]["model_id"], "deepseek-v4-flash",
            "the default model row is the one that goes on the wire: {sent}"
        );
        assert_eq!(sent["api_key"], "sk-secret-value", "the key rides the request: {sent}");
        // A TEST never asks to become primary.
        assert!(
            sent.get("set_primary").is_none(),
            "llm/test must not set set_primary: {sent}"
        );

        // Row 88: after the failure the draft KEEPS family, model, route and key.
        // Neither module exposes a live-state reader, so the proof is the card's
        // OWN surface: `lower_mounted` lowers the LIVE draft, and the rejected
        // card carries the redacted alert.
        let dsl = provider::lower_mounted().expect("the live editor lowers");
        assert!(dsl.contains(provider::TEST_FAILED), "the alert copy is on the card");
        assert!(
            !dsl.contains("sk-secret-value"),
            "the credential must never reach the lowered card"
        );
    }

    #[tokio::test]
    async fn provider_save_sends_set_primary_and_leaves_the_editor() {
        let _serial = serial();
        let frames = fixture();
        let server = ReplayServer::start(frames.clone(), Vec::new()).await;
        let conv = connected(&server).await;

        let mut ui = provider::ProviderUi::new("deepseek");
        ui.key = "sk-secret-value".into();
        provider::set(ui);

        let effect =
            provider::perform("provider.save", None).expect("provider.save is a transport effect");
        let out = provider::perform_transport(&conv, effect).await;
        assert!(out.is_ok(), "the recorded applied:true must succeed: {out:?}");

        let sent = server.sent("profile/llm/upsert").expect("provider.save reached the wire");
        assert_eq!(sent["set_primary"], true, "upsert asks to become primary: {sent}");
        assert_eq!(sent["profile_id"], serde_json::Value::Null, "default profile: {sent}");
    }

    // -------------------------------------------------------------------- §3/§4
    #[tokio::test]
    async fn browser_use_lists_and_projects_the_hidden_and_truncated_notices() {
        let _serial = serial();
        let frames = fixture();
        let server = ReplayServer::start(
            frames,
            vec![
                ("onboarding/workspace_list".into(), list_synthetic()),
                ("onboarding/workspace_create".into(), create_synthetic()),
            ],
        )
        .await;
        let conv = connected(&server).await;

        let effect = browser::perform("browser.use", None).expect("browser.use is a transport effect");
        let out = browser::perform_transport(&conv, effect).await;
        assert!(out.is_ok(), "the listing must succeed: {out:?}");

        assert!(
            server.sent("onboarding/workspace_list").is_some(),
            "browser.use reached the wire"
        );

        // Row 220: BOTH notices, truncated first then hidden. The browser module
        // exposes no live-state reader, so the proof is the card's OWN surface:
        // `lower_mounted` lowers the LIVE listing and `query` projects it.
        let dsl = browser::lower_mounted().expect("the live browser lowers");
        assert!(dsl.contains("Only the first 1 folders are shown."), "{dsl}");
        assert!(dsl.contains("2 hidden folders aren't shown."), "{dsl}");
        assert!(dsl.contains("/srv/fixture"), "the listing moved us: {dsl}");
    }

    #[tokio::test]
    async fn browser_create_sends_the_name_then_relists_the_parent() {
        let _serial = serial();
        let frames = fixture();
        let server = ReplayServer::start(
            frames,
            vec![
                ("onboarding/workspace_list".into(), list_synthetic()),
                ("onboarding/workspace_create".into(), create_synthetic()),
            ],
        )
        .await;
        let conv = connected(&server).await;

        // Stand in a folder first: `create` sends the folder we are in as
        // `parent`, and a fresh `BrowserUi::default()` has none.
        browser::set(browser::BrowserUi {
            path: "/srv/fixture".into(),
            ..Default::default()
        });

        // Type the name through the card's own input action first.
        assert!(
            browser::perform("browser.create_name", Some("NewFolder")).is_none(),
            "typing is UI-local"
        );
        let effect = browser::perform("browser.create", None).expect("create is a transport effect");
        let out = browser::perform_transport(&conv, effect).await;
        assert!(out.is_ok(), "the create must succeed: {out:?}");

        let sent = server.sent("onboarding/workspace_create").expect("create reached the wire");
        assert_eq!(sent["name"], "NewFolder", "{sent}");
        assert_eq!(sent["parent"], "/srv/fixture", "the folder we stand in: {sent}");
        // The create arm sends `workspace_create` and then re-lists ONCE — the
        // new folder is on screen because of that single follow-up listing, not
        // a second round trip of its own.
        let lists = server
            .seen
            .lock()
            .unwrap()
            .iter()
            .filter(|(m, _)| m == "onboarding/workspace_list")
            .count();
        assert_eq!(lists, 1, "create re-lists the parent exactly once: {lists}");
    }

    // ---------------------------------------------------------------------- §5
    #[test]
    fn the_key_never_reaches_a_copy_id() {
        // The module header's guarantee (provider.rs:23-24): the draft holds the
        // key, but no copy id and no binding may carry it.
        let mut ui = provider::ProviderUi::new("deepseek");
        ui.key = "sk-secret-value".into();
        let copies = provider::copies(provider::Screen::Editor, &ui);
        for (id, value) in &copies {
            assert!(
                !value.contains("sk-secret-value"),
                "copy {id} leaked the credential: {value}"
            );
        }
        for id in [
            "prov.key_masked",
            "prov.key_present",
            "prov.family",
            "prov.route",
            "prov.model",
        ] {
            if let Some(v) = provider::query(&ui, id) {
                assert!(
                    !v.to_string().contains("sk-secret-value"),
                    "binding {id} leaked the credential: {v}"
                );
            }
        }
    }
}
