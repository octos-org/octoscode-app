//! Card #P4g2 — the `session:list-sidebar` rows through the PRODUCTION path
//! (`Conversation::connect` → transport → `Conversation::on_event` /
//! `refresh_sessions` → store → `screens::sessions::query("resume.rows…")`).
//! The fake below only plays the SERVER side of the wire (the f26 pattern);
//! every client-side behaviour asserted is the real production code.
//!
//! Rows covered:
//! - **227**: a failed `session/list` refresh keeps the last attested rows
//!   (web `workspace-session-catalog.ts:209-219` — `error` keeps
//!   `previous.sessions`; `unscoped` before the first attestation).
//! - **230**: retained peers merge into the known rows WITHOUT stealing
//!   focus (web `retained-session-catalog.ts:7-22` — appended with
//!   `lastOpenedAt: 0`, i.e. after the session rows; no open is issued).
//! - **231**: a row's title falls back to its last prompt (web
//!   `workspace-session-catalog.ts:124` `title: entry.title ?? entry.lastPrompt`).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::Conversation;
use octoscode_module::screens::sessions;

/// A fake WS server (the f26 / fp4g1 pattern) with per-request `session/list`
/// replies: `list_replies` are served IN ORDER — a reply whose `__error__`
/// key is set goes out as a JSON-RPC ERROR (the failing refresh of row 227).
/// Answers `session/open` with a minimal `opened`, `{}` to other methods,
/// and streams `stream` as RAW notification frames after the open.
struct ListServer {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
}

impl ListServer {
    async fn start(
        list_replies: Vec<serde_json::Value>,
        stream: Vec<(String, serde_json::Value)>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();

        tokio::spawn(async move {
            let Ok((sock, _)) = listener.accept().await else {
                return;
            };
            let Ok(ws) = tokio_tungstenite::accept_async(sock).await else {
                return;
            };
            let (tx, mut rx_in) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
            let mut list_idx = 0usize;

            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                rx.lock().unwrap().push(method.clone());

                match method.as_str() {
                    "session/open" => {
                        let session = v["params"]["session_id"]
                            .as_str()
                            .unwrap_or("dsflash:main")
                            .to_owned();
                        let opened = serde_json::json!({
                            "session_id": session,
                            "active_profile_id": "dsflash",
                            "cursor": {"stream": "dsflash:main", "seq": 1},
                            "capabilities": {
                                "version": {"protocol": "octos-ui/v1alpha1",
                                            "schema_version": 1, "jsonrpc": "2.0"},
                                "capabilities_schema_version": 1,
                                "supported_methods": ["session/open", "session/hydrate",
                                                      "turn/start", "session/list"],
                                "supported_notifications": ["projection/envelope",
                                                            "peer/staged", "peer/closed"],
                                "supported_features": ["approval.typed.v1",
                                                       "state.session_hydrate.v1",
                                                       "projection.envelope.v2"]
                            }
                        });
                        let frame =
                            serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {"opened": opened}});
                        let _ =
                            tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                        let tx2 = tx.clone();
                        let frames = stream.clone();
                        tokio::spawn(async move {
                            for (m, params) in frames {
                                let frame =
                                    serde_json::json!({"jsonrpc": "2.0", "method": m, "params": params});
                                let _ = tx2
                                    .lock()
                                    .await
                                    .send(Message::Text(frame.to_string().into()))
                                    .await;
                                tokio::time::sleep(Duration::from_millis(60)).await;
                            }
                        });
                    }
                    "session/list" => {
                        let reply = list_replies
                            .get(list_idx)
                            .cloned()
                            .unwrap_or(serde_json::json!({"sessions": []}));
                        list_idx += 1;
                        let frame = if reply.get("__error__").is_some() {
                            serde_json::json!({
                                "jsonrpc": "2.0", "id": id,
                                "error": {"code": -32000, "message": "list failed"}
                            })
                        } else {
                            serde_json::json!({"jsonrpc": "2.0", "id": id, "result": reply})
                        };
                        let _ =
                            tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                    }
                    _ => {
                        let _ = tx
                            .lock()
                            .await
                            .send(Message::Text(
                                serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {}})
                                    .to_string()
                                    .into(),
                            ))
                            .await;
                    }
                }
            }
        });

        Self {
            base_url: format!("http://{addr}"),
            received,
        }
    }

    fn count(&self, method: &str) -> usize {
        self.received.lock().unwrap().iter().filter(|m| *m == method).count()
    }
}

fn peer_staged(session: &str, slug: &str) -> serde_json::Value {
    serde_json::json!({
        "session_id": session,
        "topic": format!("peer-{slug}"),
        "slug": slug,
        "brief": "b",
        "brief_path": format!("/tmp/p/{slug}/brief.md"),
        "cwd": format!("/tmp/p/{slug}"),
        "profile_id": "dsflash"
    })
}

// ---- 227: a failed refresh keeps the last attested rows --------------------

#[tokio::test]
async fn a_failed_refresh_keeps_the_last_attested_rows() {
    // First list attests two rows; every later list FAILS (JSON-RPC error).
    // The web's contract (workspace-session-catalog.ts:209-219): once the
    // server has attested a listing, an error keeps the previous rows.
    let server = ListServer::start(
        vec![
            serde_json::json!({"sessions": [
                {"id": "dsflash:main", "title": "First attested", "message_count": 3,
                 "updated_at": "2h ago", "last_prompt": null, "active_turn": false},
                {"id": "dsflash:pr", "title": "Review PR #2566", "message_count": 17,
                 "updated_at": "3d ago", "last_prompt": null, "active_turn": false}
            ]}),
            serde_json::json!({"__error__": true}),
        ],
        vec![],
    )
    .await;
    let (conv, _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");

    // The attesting refresh folds its rows through the production path.
    let n = conv.refresh_sessions().await.expect("first refresh");
    assert_eq!(n, 2, "the attested rows folded");
    let attested = conv.store.sessions();
    assert_eq!(attested.len(), 2);
    assert_eq!(attested[0].title.as_deref(), Some("First attested"));

    // The failing refresh must not wipe them.
    let err = conv
        .refresh_sessions()
        .await
        .expect_err("the second list fails");
    let _ = format!("{err:?}");
    let kept = conv.store.sessions();
    assert_eq!(
        kept.len(),
        2,
        "a failed refresh keeps the last attested rows: {kept:?}"
    );
    assert_eq!(kept[0].title.as_deref(), Some("First attested"));
    assert_eq!(kept[1].id, "dsflash:pr");
}

// ---- 230: retained peers merge into the rows without stealing focus --------

#[tokio::test]
async fn retained_peers_merge_into_the_rows_without_stealing_focus() {
    // A session row is attested; a peer is then staged for THIS session (the
    // production peer/staged path). The peer must appear in the sidebar rows
    // AFTER the session rows (lastOpenedAt: 0, retained-session-catalog.ts:18)
    // with NO session/open issued for it, and drop out again on peer/closed.
    let server = ListServer::start(
        vec![serde_json::json!({"sessions": [
            {"id": "dsflash:main", "title": "Main chat", "message_count": 5,
             "updated_at": "1h ago", "last_prompt": null, "active_turn": false}
        ]})],
        vec![
            ("peer/staged".to_owned(), peer_staged("dsflash:main", "ada")),
            (
                "peer/closed".to_owned(),
                serde_json::json!({
                    "session_id": "dsflash:main",
                    "topic": "peer-ada",
                    "slug": "ada",
                    "profile_id": "dsflash"
                }),
            ),
        ],
    )
    .await;
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    conv.refresh_sessions().await.expect("attest one row");
    let opens_before = server.count("session/open");

    // Drain until the staged peer is observable, snapshot the rows, then keep
    // draining until the close landed.
    let rows_with_peer;
    loop {
        match tokio::time::timeout(Duration::from_millis(120), events.recv()).await {
            Ok(Some(evt)) => {
                conv.on_event(evt);
            }
            _ => panic!("the peer events never arrived"),
        }
        let ui = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui);
        let rows = sessions::query(&ctx, "resume.rows").unwrap();
        let arr = rows.as_array().unwrap();
        if arr.iter().any(|r| r["id"] == "peer-ada") {
            rows_with_peer = arr.clone();
            break;
        }
    }
    while conv.store.domains.peer.get("ada").map(|p| p.closed) != Some(true) {
        match tokio::time::timeout(Duration::from_millis(120), events.recv()).await {
            Ok(Some(evt)) => {
                conv.on_event(evt);
            }
            _ => panic!("peer/closed never arrived"),
        }
    }

    // While staged: session row FIRST, peer row appended AFTER it (no focus
    // steal) — and exactly the attested session row, no duplicates.
    assert_eq!(rows_with_peer.len(), 2, "{rows_with_peer:?}");
    assert_eq!(rows_with_peer[0]["id"], "dsflash:main");
    assert_eq!(rows_with_peer[1]["id"], "peer-ada");
    assert_eq!(rows_with_peer[1]["title"], "ada");
    // No open flew for the retained peer (no focus steal on the wire).
    assert_eq!(
        server.count("session/open"),
        opens_before,
        "a retained peer must not issue session/open"
    );

    // After the close the peer row is gone; the session row stays.
    let ui = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui);
    let rows = sessions::query(&ctx, "resume.rows").unwrap();
    let arr = rows.as_array().unwrap();
    assert_eq!(arr.len(), 1, "the closed peer drops out: {arr:?}");
    assert_eq!(arr[0]["id"], "dsflash:main");
}

// ---- 231: a row's title falls back to its last prompt -----------------------

#[tokio::test]
async fn row_titles_fall_back_to_the_last_prompt() {
    // workspace-session-catalog.ts:124: `title: entry.title ?? entry.lastPrompt`.
    let server = ListServer::start(
        vec![serde_json::json!({"sessions": [
            {"id": "dsflash:main", "title": "Titled", "message_count": 1,
             "updated_at": null, "last_prompt": null, "active_turn": false},
            {"id": "dsflash:pr", "title": null, "message_count": 9,
             "updated_at": null, "last_prompt": "Review the diff line by line",
             "active_turn": false},
            {"id": "dsflash:mt", "title": "  ", "message_count": 2,
             "updated_at": null, "last_prompt": null, "active_turn": false}
        ]})],
        vec![],
    )
    .await;
    let (conv, _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.refresh_sessions().await.expect("attest rows");

    let ui = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui);
    let titles = sessions::query(&ctx, "resume.rows[].title").unwrap();
    assert_eq!(
        titles,
        serde_json::json!([
            "Titled",
            "Review the diff line by line",
            null
        ]),
        "title falls back to the row's last prompt; both-absent stays null"
    );
}
