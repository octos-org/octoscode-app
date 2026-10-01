//! P4a3 — the production-path replay tests for the research-lane mutations
//! and the transcript export, mirroring f29c's replay server: a fake WS
//! server serves the RECORDED real-gate frames (r2-profile for
//! `profile/sub_providers/*`, r43a-recovery for `session/hydrate`), the
//! screens' functions run through the production `Conversation` client, and
//! the server's received-method log proves the wire traffic.
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use octoscode_client::domains::profile::SubProviderParams;
use octoscode_module::screens::{research, transcript};
use octoscode_module::flow::Conversation;
use octoscode_store::Store;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

struct Frame {
    dir: String,
    method: String,
    body: Value,
}

fn load(path: &str) -> Vec<Frame> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(Value::Null),
            }
        })
        .collect()
}

fn recorded(path: &str, method: &str) -> Value {
    load(path)
        .iter()
        .find(|f| f.dir == "in" && f.method == method)
        .map(|f| f.body.clone())
        .unwrap_or_else(|| panic!("{method} is in the recording {path}"))
}

/// f29c's replay server: canned replies per method, `{}` for the rest, and a
/// log of every method the (production) client sent.
struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
}

impl ReplayServer {
    async fn start(canned: Vec<(String, Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (tx, mut rx_in) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                rx.lock().unwrap().push(method.clone());
                let body = canned
                    .iter()
                    .find(|(m, _)| *m == method)
                    .map(|(_, b)| b.clone())
                    .unwrap_or(json!({}));
                let frame = json!({"jsonrpc": "2.0", "id": id, "result": body});
                let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { base_url: format!("http://{addr}"), received }
    }

    fn saw(&self, method: &str) -> bool {
        self.received.lock().unwrap().iter().any(|m| m == method)
    }
}

fn lane(key: &str) -> SubProviderParams {
    SubProviderParams {
        key: key.to_owned(),
        provider: "zhipu".to_owned(),
        model: Some("glm-4-flash".to_owned()),
        api_key_env: None,
        base_url: None,
        description: Some("P4a3 replay lane".to_owned()),
        default_context_window: None,
        max_output_tokens: None,
        api_type: None,
    }
}

fn advertised(store: &Store) {
    store.domains.config.set_supported_methods(vec![
        "profile/sub_providers/list".into(),
        "profile/sub_providers/upsert".into(),
        "profile/sub_providers/remove".into(),
    ]);
}

const R2: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r2-profile-a6ea8505.jsonl"
);
const R43A: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r43a-recovery-a6ea8505.jsonl"
);

#[tokio::test]
async fn upsert_remove_send_the_recorded_shape_and_fold_the_receipt() {
    let upsert_reply = recorded(R2, "profile/sub_providers/upsert");
    let remove_reply = recorded(R2, "profile/sub_providers/remove");
    let server = ReplayServer::start(vec![
        ("profile/sub_providers/upsert".to_owned(), upsert_reply.clone()),
        ("profile/sub_providers/remove".to_owned(), remove_reply.clone()),
    ])
    .await;
    let (conv, mut _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
            .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let store = conv.store.clone();
    advertised(&store);

    // Upsert WITH a dispatch-only credential: the server sees the method and
    // the receipt's lanes fold; the notice matches the receipt's flags.
    let notice = research::upsert_lane(conv.client(), &store, lane("r2-lane"), Some("sk-dispatch-only".into()))
        .await
        .expect("upsert through the production client");
    assert!(server.saw("profile/sub_providers/upsert"), "the wire carried the mutation");
    let applied = upsert_reply["applied"].as_bool().unwrap_or(false);
    let restart = upsert_reply["restart_required"].as_bool().unwrap_or(false);
    assert_eq!(notice, research::mutation_notice(applied, restart), "verbatim web notice");
    let folded = store.domains.profile.sub_providers();
    assert_eq!(folded.len(), upsert_reply["sub_providers"].as_array().map(Vec::len).unwrap_or(0));
    // The credential never reached the store (dispatch-only).
    assert!(!format!("{folded:?}").contains("sk-dispatch-only"));

    // Remove by key: same gate path, receipt folds over the lanes.
    let notice = research::remove_lane(conv.client(), &store, "r2-lane")
        .await
        .expect("remove through the production client");
    assert!(server.saw("profile/sub_providers/remove"), "the wire carried the removal");
    let applied = remove_reply["applied"].as_bool().unwrap_or(false);
    let restart = remove_reply["restart_required"].as_bool().unwrap_or(false);
    assert_eq!(notice, research::mutation_notice(applied, restart));
}

#[tokio::test]
async fn mutations_refuse_off_the_recorded_gates_without_touching_the_wire() {
    let server = ReplayServer::start(vec![(
        "profile/sub_providers/upsert".to_owned(),
        recorded(R2, "profile/sub_providers/upsert"),
    )])
    .await;
    let (conv, mut _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
            .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let store = conv.store.clone();

    // Nothing advertised -> refused BEFORE the wire.
    let err = research::upsert_lane(conv.client(), &store, lane("x"), None)
        .await
        .expect_err("unadvertised method is refused");
    assert!(err.contains("does not advertise"), "{err}");
    assert!(!server.saw("profile/sub_providers/upsert"));

    // Advertised but Profile work running -> locked BEFORE the wire.
    advertised(&store);
    store.domains.profile.set_profile_busy(true);
    let err = research::remove_lane(conv.client(), &store, "x")
        .await
        .expect_err("busy Profile locks the mutation");
    assert!(err.contains("locked"), "{err}");
    assert!(!server.saw("profile/sub_providers/remove"));
}

#[tokio::test]
async fn transcript_copy_reads_the_recorded_hydrate_and_refuses_foreign() {
    let hydrate = recorded(R43A, "session/hydrate");
    let session = hydrate["session_id"].as_str().expect("recorded session id").to_owned();

    // The REAL reply: the copy succeeds with the chat-only markdown.
    let server = ReplayServer::start(vec![("session/hydrate".to_owned(), hydrate.clone())]).await;
    let (conv, mut _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
            .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let outcome = transcript::copy_conversation(conv.client(), &session, Some("/tmp/43a-ws"), None)
        .await
        .expect("hydrate read");
    let transcript::CopyOutcome::Copied(markdown) = outcome else {
        panic!("the recorded history copies");
    };
    assert!(server.saw("session/hydrate"));
    assert!(markdown.starts_with("# 43a-ws"), "leaf-name header, got: {}", &markdown[..40]);
    assert!(markdown.contains(&format!("- Session: `{session}`")));
    assert!(markdown.contains("## User"), "chat text only");
    // Tool rows are dropped BY ROLE — assert on the recorded tool row's
    // content (the word "tool" itself appears in legitimate chat text).
    assert!(
        !markdown.contains("Successfully wrote 1 lines"),
        "tool rows are excluded from the transcript"
    );

    // A history with ANOTHER session_id is an error, never a copy.
    let mut foreign = hydrate.clone();
    foreign["session_id"] = json!("other:session");
    let server = ReplayServer::start(vec![("session/hydrate".to_owned(), foreign)]).await;
    let (conv, mut _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
            .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let outcome = transcript::copy_conversation(conv.client(), &session, None, None).await.expect("read");
    assert!(matches!(outcome, transcript::CopyOutcome::Foreign), "session mismatch refuses the copy");

    // An empty conversation returns Empty (the caller writes nothing).
    let mut empty = hydrate.clone();
    empty["messages"] = json!([]);
    let server = ReplayServer::start(vec![("session/hydrate".to_owned(), empty)]).await;
    let (conv, mut _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
            .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let outcome = transcript::copy_conversation(conv.client(), &session, None, None).await.expect("read");
    assert!(matches!(outcome, transcript::CopyOutcome::Empty), "empty writes nothing");
}
