//! #D1t / #A2 — board 1 talks to the server through the PRODUCTION path: the
//! action a click routes (`board1::route`, what lib.rs's `perform_board1`
//! calls) returns the `Work`, and `board1::execute` — the exact future
//! `board1::spawn` runs on the module's runtime — performs it through the
//! typed client. Nothing here calls the client directly.
//!
//! Traffic:
//! - provider: `crates/octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl`,
//!   recorded from a real `octos serve` a6ea8505 — the `profile/llm/test` 401
//!   (line 10) and the `profile/llm/upsert` `applied: true` (line 12). The
//!   passing test reply is the server's documented `LlmTestResult` shape
//!   (`profile.rs` `LlmTestResult`), synthetic because the recording ran the
//!   failing key only; `profile_id` is rewritten to the session's own.
//! - browser: `onboarding/workspace_list|create` replies in the server's
//!   documented `WorkspaceListResult`/`WorkspaceCreateResult` shapes
//!   (`profile.rs:92-126`), and the typed refusal as a JSON-RPC error whose
//!   `data.kind` is a whitelisted kind (the recording never browsed).
//! - pairing: `crates/octoscode-client/tests/fixtures/pairing-a6ea8505.jsonl`,
//!   recorded from a real local `octos serve --web-url`.
mod replay {
    use std::sync::{Arc, Mutex};

    use futures_util::{SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;

    use octoscode_module::flow::Conversation;
    use octoscode_module::screens::{board1, browser, pairing, provider};

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn recorded(line: usize) -> serde_json::Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl"
        );
        let text = std::fs::read_to_string(path).expect("read the r29a fixture");
        let v: serde_json::Value = serde_json::from_str(text.lines().nth(line - 1).unwrap()).unwrap();
        v["body"].clone()
    }

    type Seen = Arc<Mutex<Vec<(String, serde_json::Value)>>>;

    /// A UI-protocol server answering by method (and params), recording what
    /// reached the wire.
    struct Server {
        url: String,
        seen: Seen,
    }

    impl Server {
        fn sent(&self, method: &str) -> Vec<serde_json::Value> {
            self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
        }
    }

    fn answer(method: &str, p: &serde_json::Value) -> Result<serde_json::Value, serde_json::Value> {
        let listing = |path: &str, names: &[&str], hidden: u64| {
            let parent = path.rsplit_once('/').map(|(p, _)| if p.is_empty() { "/" } else { p }).unwrap_or("/");
            serde_json::json!({
                "canonical_path": path, "parent_path": parent, "writable": true,
                "entries": names.iter().map(|n| serde_json::json!({"name": n, "path": format!("{path}/{n}"), "writable": true})).collect::<Vec<_>>(),
                "truncated": false, "hidden_skipped": hidden,
            })
        };
        match method {
            // The open reply's shape is the one board1_serve answers the app
            // with (the server's `UiProtocolCapabilities`).
            "session/open" => Ok(serde_json::json!({"opened": {
                "session_id": p["session_id"], "active_profile_id": "octoscode",
                "workspace_root": p["cwd"].as_str().unwrap_or("/home/user/code"),
                "cursor": {"stream": "t", "seq": 1},
                "capabilities": {
                    "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                    "capabilities_schema_version": 2,
                    "supported_methods": ["session/open", "session/list", "profile/llm/list", "profile/llm/catalog",
                        "profile/llm/test", "profile/llm/upsert", "onboarding/workspace_list", "onboarding/workspace_create"],
                    "supported_notifications": ["turn/started", "turn/completed"],
                    "supported_features": [browser::BROWSE_FEATURE],
                }
            }})),
            "session/list" => Ok(serde_json::json!({"sessions": []})),
            "profile/llm/list" => Ok(serde_json::json!({
                "profile_id": "octoscode",
                "primary": {"family_id": "deepseek", "model_id": "deepseek-v4-flash",
                    "route": {"route_id": "deepseek", "label": "Official API", "api_key_env": "DEEPSEEK_API_KEY", "api_type": "openai"},
                    "has_api_key": true, "selected": true, "available": true},
                "fallbacks": []
            })),
            "profile/llm/catalog" => Ok(serde_json::json!({"families": {"deepseek": {"env": "DEEPSEEK_API_KEY", "models": [
                {"id": "deepseek-v4-flash"}, {"id": "deepseek-v4"}, {"id": "deepseek-chat"}]}}})),
            "profile/llm/test" => {
                if p["api_key"].as_str().unwrap_or("").starts_with("sk-good") {
                    Ok(serde_json::json!({"profile_id": "octoscode", "applied": true, "message": "ok"}))
                } else {
                    let mut b = recorded(10); // the REAL a6ea8505 401
                    b["profile_id"] = serde_json::json!("octoscode");
                    Ok(b)
                }
            }
            "profile/llm/upsert" => {
                let mut b = recorded(12); // the REAL applied: true
                b["profile_id"] = serde_json::json!("octoscode");
                Ok(b)
            }
            "onboarding/workspace_list" => match p["path"].as_str() {
                None | Some("/home/user/code") => Ok(listing("/home/user/code", &["octos", "octoscode-app", "notes", "scratch"], 3)),
                Some("/home/user") => Ok(listing("/home/user", &["code"], 0)),
                Some("/home/user/code/octoscode-app") => Ok(listing("/home/user/code/octoscode-app", &["crates", "design"], 0)),
                Some("/home/user/code/fresh") => Ok(listing("/home/user/code/fresh", &[], 0)),
                Some("/private") => Err(serde_json::json!({"code": -32602,
                    "message": "workspace_list: permission denied at /private (EACCES)",
                    "data": {"kind": "workspace_list_permission_denied"}})),
                Some(_) => Err(serde_json::json!({"code": -32602, "message": "workspace_list: not found",
                    "data": {"kind": "workspace_list_not_found"}})),
            },
            "onboarding/workspace_create" => Ok(serde_json::json!({
                "canonical_path": format!("{}/{}", p["parent"].as_str().unwrap_or(""), p["name"].as_str().unwrap_or("")),
                "created": true})),
            _ => Ok(serde_json::json!({})),
        }
    }

    async fn server() -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let seen: Seen = Arc::default();
        let rec = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let rec = rec.clone();
                tokio::spawn(async move {
                    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
                    let (mut tx, mut rx) = ws.split();
                    while let Some(Ok(Message::Text(text))) = rx.next().await {
                        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        let params = v.get("params").cloned().unwrap_or(serde_json::Value::Null);
                        rec.lock().unwrap().push((method.clone(), params.clone()));
                        let frame = match answer(&method, &params) {
                            Ok(r) => serde_json::json!({"jsonrpc": "2.0", "id": v["id"], "result": r}),
                            Err(e) => serde_json::json!({"jsonrpc": "2.0", "id": v["id"], "error": e}),
                        };
                        let _ = tx.send(Message::Text(frame.to_string().into())).await;
                    }
                });
            }
        });
        Server { url: format!("ws://{addr}"), seen }
    }

    async fn connected(s: &Server) -> Arc<Conversation> {
        let (conv, mut rx) = Conversation::connect(&s.url, "fixture", "octoscode", None, None).expect("connects");
        let conv = Arc::new(conv);
        let d = conv.clone();
        tokio::spawn(async move {
            while let Some(e) = rx.recv().await {
                let _ = d.on_event(e);
            }
        });
        conv.open_workspace(None).await.expect("session/open");
        // The open reply's capabilities fold into the store asynchronously.
        for _ in 0..100 {
            if conv.store.capabilities().iter().any(|c| c == browser::BROWSE_FEATURE) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        conv
    }

    /// Route one action the way a click does and run its work to completion.
    async fn click(action: &str, value: Option<&str>, conv: &Arc<Conversation>) -> Vec<board1::Work> {
        let work = board1::route(action, value);
        for w in work.clone() {
            if !matches!(w, board1::Work::Connect { .. } | board1::Work::LeaveToForm { .. } | board1::Work::Scan | board1::Work::Forget) {
                let _ = board1::execute(w, Some(conv.clone())).await;
            }
        }
        work
    }

    // ------------------------------------------------------------- provider
    #[tokio::test]
    async fn the_provider_editor_seeds_rejects_keeps_the_draft_then_saves() {
        let _s = serial();
        let s = server().await;
        let conv = connected(&s).await;
        board1::close_all();

        // Settings -> Edit provider: the editor is seeded from the profile.
        click("b1.open.provider", None, &conv).await;
        {
            let p = provider::state();
            assert_eq!((p.family.as_str(), p.route.as_str(), p.route_label.as_str()), ("deepseek", "deepseek", "Official API"));
            assert_eq!(p.models, ["deepseek-v4-flash", "deepseek-v4", "deepseek-chat"]);
            assert!(p.key_stored, "has_api_key -> a blank masked key");
        }
        assert!(!s.sent("profile/llm/list").is_empty() && !s.sent("profile/llm/catalog").is_empty());

        // A typed key + Save: ONE test with the recorded 401 -> p4-07, no upsert.
        board1::route("provider.key", Some("sk-rejected-123"));
        click("provider.save", None, &conv).await;
        assert!(s.sent("profile/llm/upsert").is_empty(), "a failed test must not upsert");
        {
            let p = provider::state();
            assert_eq!(p.screen, provider::Screen::Rejected);
            assert_eq!(p.failure_line(), "The provider rejected this key (401).");
            // Walk 88: the whole draft survives — family, route, model, key.
            assert_eq!((p.family.as_str(), p.route.as_str(), p.default_model.as_deref()), ("deepseek", "deepseek", Some("deepseek-v4-flash")));
            assert_eq!(p.key, "sk-rejected-123");
            assert!(!p.error.as_deref().unwrap_or("").contains("sk-rejected-123"), "the kept error is redacted");
        }
        // The key is never page text: it lives only in the password field.
        board1::mark_dirty();
        let dsl = board1::view(990.0, 603.0).unwrap();
        let mut seen_in_field = 0;
        for (i, _) in dsl.match_indices("sk-rejected-123") {
            let named = dsl[..i].rfind(":= ").expect("inside a named widget");
            let line = dsl[..named].rfind('\n').map(|n| n + 1).unwrap_or(0);
            let end = i + dsl[i..].find("draw_cursor").expect("a text input");
            let field = &dsl[line..end];
            assert!(
                field.starts_with("b1_prov_key := TextInputFlat") && field.contains("is_password: true"),
                "the key leaked outside its password field: {field}"
            );
            seen_in_field += 1;
        }
        assert_eq!(seen_in_field, 1, "the draft key lives in exactly one place: its masked field");
        assert!(dsl.contains("Your draft is kept."));

        // Try again with a good key: test then upsert, with ONE param shape.
        board1::route("provider.key", Some("sk-good-key-1"));
        click("provider.retry", None, &conv).await;
        let tests = s.sent("profile/llm/test");
        let ups = s.sent("profile/llm/upsert");
        assert_eq!((tests.len(), ups.len()), (2, 1), "retry = test then upsert");
        assert_eq!(tests[1]["selection"], ups[0]["selection"], "Test and Save cannot drift");
        assert_eq!(ups[0]["set_primary"], true);
        assert!(tests[1].get("set_primary").is_none());
        assert_eq!(ups[0]["profile_id"], "octoscode", "the session's own profile");
        assert_eq!(ups[0]["selection"]["route"]["label"], "Official API");
        assert!(ups[0]["selection"]["route"].get("base_url").is_none(), "the official endpoint is not sent back");
        assert_ne!(board1::top(), Some(board1::Surface::Provider), "a saved editor closes");
    }

    // --------------------------------------------------- picker + browser
    #[tokio::test]
    async fn the_picker_opens_the_browser_which_lists_refuses_creates_and_starts_a_session() {
        let _s = serial();
        let s = server().await;
        let conv = connected(&s).await;
        board1::close_all();
        browser::set(browser::BrowserUi::default());
        // The recents cache must never touch a real home from a test.
        octoscode_module::screens::recents::set_store(Arc::new(octoscode_module::screens::recents::MemoryStore::new()));
        board1::note_context(&board1::Context { capabilities: conv.store.capabilities(), ..Default::default() });

        // + Add workspace (A3's sidebar `workspace.add`; lib.rs answers it
        // with `b1.open.add`): the picker reads the server's working
        // directory, and the browser over it lists that folder with the
        // server's hidden count (walk 220).
        click("b1.open.add", None, &conv).await;
        assert_eq!(board1::picker().server_root.as_deref(), Some("/home/user/code"));
        assert!(board1::picker().browse_advertised);
        assert_eq!(board1::top(), Some(board1::Surface::Browser));
        {
            let b = browser::state();
            assert_eq!(b.path, "/home/user/code");
            assert_eq!(b.entries.len(), 4);
            assert_eq!(b.notices(), vec!["3 hidden by the server".to_owned()]);
        }
        // Pick a row: fills the path box, NO request (walk 223).
        let lists = s.sent("onboarding/workspace_list").len();
        click("browser.enter.1", None, &conv).await;
        assert_eq!(s.sent("onboarding/workspace_list").len(), lists, "picking must not navigate");
        assert_eq!(browser::state().path_draft, "/home/user/code/octoscode-app");
        // A second tap opens it (drill in).
        click("browser.enter.1", None, &conv).await;
        assert_eq!(browser::state().path, "/home/user/code/octoscode-app");
        // The way back up is the listing's parent.
        click("browser.up", None, &conv).await;
        assert_eq!(browser::state().path, "/home/user/code");

        // A refused folder: bounded copy, never the server's prose, the folder
        // we stood in survives (walk 221).
        board1::route("browser.path", Some("/private"));
        click("browser.go", None, &conv).await;
        {
            let b = browser::state();
            assert_eq!(b.screen, browser::Screen::Refused);
            assert_eq!(b.failure.as_ref().unwrap().kind, "workspace_list_permission_denied");
            assert_eq!(b.path, "/home/user/code", "the last good folder is kept");
            assert_eq!(b.entries.len(), 4, "with its rows");
        }
        board1::mark_dirty();
        let dsl = board1::view(990.0, 603.0).unwrap();
        assert!(dsl.contains("The server won’t list this folder."));
        assert!(!dsl.contains("EACCES") && !dsl.contains("permission denied at"), "no server prose");
        let lists = s.sent("onboarding/workspace_list").len();
        click("browser.back", None, &conv).await;
        assert_eq!(s.sent("onboarding/workspace_list").len(), lists, "Back needs no request");

        // New folder: an invalid name never reaches the wire; a valid one is
        // created and the browser MOVES INTO it.
        // A breadcrumb lists that ancestor; its row drills back in.
        click("browser.crumb.2", None, &conv).await; // "/" > home > user
        assert_eq!(browser::state().path, "/home/user");
        click("browser.enter.0", None, &conv).await;
        click("browser.enter.0", None, &conv).await;
        assert_eq!(browser::state().path, "/home/user/code");
        click("browser.newfolder", None, &conv).await;
        board1::route("browser.create_name", Some("a/b"));
        click("browser.create", None, &conv).await;
        assert!(s.sent("onboarding/workspace_create").is_empty(), "pre-validated before any request");
        board1::route("browser.create_name", Some("fresh"));
        click("browser.create", None, &conv).await;
        let created = s.sent("onboarding/workspace_create");
        assert_eq!(created.len(), 1);
        assert_eq!((created[0]["parent"].as_str(), created[0]["name"].as_str()), (Some("/home/user/code"), Some("fresh")));
        assert_eq!(browser::state().path, "/home/user/code/fresh", "creating moves into the folder");

        // Use this folder: a new session opens there (walk 222).
        click("browser.use", None, &conv).await;
        let opens = s.sent("session/open");
        assert_eq!(opens.last().unwrap()["cwd"], "/home/user/code/fresh");
        assert!(!board1::is_open(), "the picker closes once the session opened");
        // And the browser reopens at the chosen folder next time (walk 223),
        // and the picker lists it first among the recents.
        assert_eq!(browser::state().chosen.as_deref(), Some("/home/user/code/fresh"));
        assert_eq!(board1::picker().recents.first().map(|(_, p)| p.as_str()), Some("/home/user/code/fresh"));
    }

    // -------------------------------------------------------------- pairing
    async fn pair_server() -> u16 {
        // Replays the RECORDED a6ea8505 answers: the printed code once, then
        // pair_code_unknown.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../octoscode-client/tests/fixtures/pairing-a6ea8505.jsonl");
        let rows: Vec<serde_json::Value> = std::fs::read_to_string(path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        let pick = |part: &str| rows.iter().find(|r| r["note"].as_str().unwrap_or("").contains(part)).unwrap().clone();
        let (ok, used) = (pick("claims the token once"), pick("single use"));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let claimed = Arc::new(Mutex::new(false));
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else { return };
                let (ok, used, claimed) = (ok.clone(), used.clone(), claimed.clone());
                tokio::spawn(async move {
                    // The head, then exactly content-length bytes of body.
                    let (mut buf, mut chunk) = (Vec::new(), [0u8; 4096]);
                    let (head, mut body) = loop {
                        let n = stream.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            break (String::from_utf8_lossy(&buf[..i]).to_string(), buf[i + 4..].to_vec());
                        }
                    };
                    let len = head
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length").then(|| v.trim().parse().ok())?
                        })
                        .unwrap_or(0usize);
                    while body.len() < len {
                        let n = stream.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        body.extend_from_slice(&chunk[..n]);
                    }
                    let req = String::from_utf8_lossy(&body).to_string();
                    let r = if req.contains("3QK7ZP2M") && !std::mem::replace(&mut *claimed.lock().unwrap(), true) { &ok } else { &used };
                    let body = r["body"].to_string().replace("<redacted>", "paired-token-1");
                    let status = r["status"].as_u64().unwrap();
                    let resp = format!("HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
                    let _ = stream.write_all(resp.as_bytes()).await;
                });
            }
        });
        port
    }

    #[tokio::test]
    async fn pairing_claims_once_hands_the_token_to_the_connect_path_and_refuses_a_spent_link() {
        let _s = serial();
        let port = pair_server().await;
        board1::close_all();
        let _ = board1::take_pending();
        board1::route("b1.open.pairing", None);
        let link = format!("http://app.invalid/?octos=http://127.0.0.1:{port}&pair=3qk7zp2m");
        board1::route("pair.paste", Some(&link));
        for w in board1::route("pair.submit", None) {
            board1::execute(w, None).await.unwrap();
        }
        // The claim's answer is handed to the host as ONE Connect with the
        // token (the recording's server_origin echo is a loopback origin).
        let pending = board1::take_pending();
        match pending.as_slice() {
            [board1::Work::Connect { server, token }] => {
                assert_eq!(token, "paired-token-1");
                assert_eq!(server, "http://127.0.0.1:8422");
            }
            other => panic!("one Connect expected: {other:?}"),
        }
        assert!(pairing::state().link_draft.is_empty(), "the one-use code left the field");
        // The connection goes live: the pairing dialog closes, p4-05 remembers.
        board1::note_context(&board1::Context { live: true, ..Default::default() });
        assert!(!board1::is_open());
        assert!(pairing::state().paired.is_some());

        // The same link again: the recorded pair_code_unknown -> p4-03.
        board1::route("b1.open.pairing", None);
        board1::route("pair.paste", Some(&link));
        for w in board1::route("pair.submit", None) {
            board1::execute(w, None).await.unwrap();
        }
        assert!(board1::take_pending().is_empty());
        assert_eq!(pairing::state().screen, pairing::Screen::LinkProblem);
        assert_eq!(pairing::state().server, format!("http://127.0.0.1:{port}"), "the origin is prefilled");

        // A link to another computer is refused WITHOUT a request (walk 111).
        board1::route("pair.back", None);
        board1::route("pair.paste", Some("?octos=http://192.168.1.20:50190&pair=3QK7ZP2M"));
        assert!(board1::route("pair.submit", None).is_empty(), "no exchange may start");
        assert_eq!(pairing::state().server, "", "the refused address is not prefilled");
        board1::close_all();
    }
}
