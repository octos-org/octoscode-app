//! #29a — replay the **recorded real onboarding traffic**
//! (`crates/octoscode-client/tests/fixtures/r29a-onboarding-a6ea8505.jsonl`,
//! the frames a real `octos serve` a6ea8505 sent my capture on :50140)
//! through the module's own path, and assert what the board-2 screens do
//! with it. No model is called; the fixture is replayed (RULES "Tests replay
//! recorded real traffic").
//!
//! What is asserted, per the entry:
//! - the `create_profile` action's protocol run (`screens::connect::
//!   run_onboarding` on the production client over the module's own
//!   `Conversation`): the recorded catalog parses, the selection derives
//!   (deepseek / deepseek-v4-flash / DEEPSEEK_API_KEY / Official API),
//!   `profile/local/create` is sent with the requested id, the recorded
//!   `profile/llm/test` failure (the server's real 401 body — `applied:
//!   false`) propagates as the panel's error **verbatim**, never rewritten;
//! - the lowered cards' text: the recorded failure lands on 2.2's
//!   `t_error_text` / 2.3's error slot through [`copies`]/[`lower_screen`],
//!   and the binding table projects it.
//!
//! The recorded `session/open` rpc error (profile `octoscode` is not
//! configured on a fresh capture serve) is replayed as-is: the sequence
//! must not depend on it, exactly as the capture run did not.
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::connect::{
    self, ConnectUi, FailureKind, Provider, Screen,
};

/// One recorded frame.
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

fn recorded(frames: &[Frame], method: &str) -> Frame {
    frames
        .iter()
        .find(|f| f.dir == "in" && f.method == method)
        .cloned()
        .unwrap_or_else(|| panic!("the fixture carries a recorded {method} reply"))
}

/// A fake WS server that answers each method with the RECORDED real body
/// (`dir == "in"` frames); every other request gets `null`. The replies go
/// back with the request's own id, rpc-error frames as `error` responses.
struct ReplayServer {
    url: String,
    seen: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl ReplayServer {
    async fn start(frames: Vec<Frame>) -> Self {
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
                let reply = match frames.iter().find(|f| f.dir == "in" && f.method == method) {
                    Some(f) if f.error.is_some() => serde_json::json!({
                        "jsonrpc": "2.0", "id": id, "error": f.error.clone(),
                    }),
                    Some(f) => serde_json::json!({
                        "jsonrpc": "2.0", "id": id, "result": f.body.clone(),
                    }),
                    None => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": null}),
                };
                let _ = tx.send(Message::Text(reply.to_string().into())).await;
            }
        });
        Self {
            url: format!("ws://{addr}"),
            seen,
        }
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

/// The module's own conversation over the replay server (the production
/// transport + client + registry path).
async fn connected(server: &ReplayServer) -> Arc<Conversation> {
    // `Conversation::connect` is the module's own synchronous constructor
    // (it spawns the transport internally) — no await.
    let (conv, mut evt_rx) = Conversation::connect(
        &server.url,
        "mapb-dummy",
        "octoscode",
        None,
        None,
    )
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

#[tokio::test]
async fn the_recorded_run_creates_the_profile_and_propagates_the_real_test_failure() {
    let frames = fixture();
    let server = ReplayServer::start(frames.clone()).await;
    let conv = connected(&server).await;

    // The action's run, exactly as `perform_screen_action("create_profile")`
    // performs it (catalog = None → the run fetches it itself).
    let err = connect::run_onboarding(
        conv.client(),
        "octos-dev",
        "octos-dev",
        "octoscode-web-keyless-probe",
        Provider::DeepSeek,
        None,
    )
    .await
    .expect_err("the recorded test reply is the server's real 401");

    // The verbatim recorded verdict — never a rewritten diagnosis.
    assert!(
        err.contains("authentication failed") && err.contains("HTTP 401"),
        "the panel error must be the recorded body, got: {err}"
    );

    // The wire sequence the run actually sent (the production path).
    let create = server.sent("profile/local/create").expect("local/create sent");
    assert_eq!(create["requested_id"], "octos-dev");
    assert_eq!(create["name"], "octos-dev");
    let test = server.sent("profile/llm/test").expect("llm/test sent");
    assert_eq!(test["profile_id"], "octos-dev");
    assert_eq!(test["selection"]["family_id"], "deepseek");
    assert_eq!(test["selection"]["model_id"], "deepseek-v4-flash");
    assert_eq!(test["selection"]["route"]["api_key_env"], "DEEPSEEK_API_KEY");
    assert_eq!(test["selection"]["route"]["label"], "Official API");
    // Test failed → the run must have stopped before the upsert (the web's
    // `submitOnboarding` bails at `onboarding-submission.ts:97-105`).
    assert!(server.sent("profile/llm/upsert").is_none(), "no upsert after a failed test");
    // The create reply's server-assigned id is what the run carried forward.
    let recorded_create = recorded(&frames, "profile/local/create");
    assert_eq!(recorded_create.body["profile_id"], "octos-dev");
}

// #P4a2 — the keyless NEGOTIATION hits the wire: with a keyless family
// (empty env) and an empty key input, the run sends the web's exact
// non-empty probe value as the test key (octos#2123 via
// onboarding-submission.ts:51-53 `wireApiKey = apiKey ||
// KEYLESS_CORE_PROBE`; the native mirror is screens/connect.rs
// KEYLESS_PROBE behind the same requires_key gate).
#[tokio::test]
async fn an_empty_key_on_a_keyless_family_negotiates_the_probe_value_on_the_wire() {
    let frames = fixture();
    let server = ReplayServer::start(frames.clone()).await;
    let conv = connected(&server).await;

    // The panel's prepare hands the run its ALREADY-FETCHED catalog
    // (use-onboarding.ts:91); the recorded one is mutated to the keyless
    // shape (env = ""), exactly the field selectionFromCatalog reads.
    let mut catalog: octoscode_client::domains::profile::LlmCatalogResult =
        serde_json::from_value(recorded(&frames, "profile/llm/catalog").body)
            .expect("the recorded catalog parses");
    catalog
        .families
        .iter_mut()
        .find(|f| f.id == "deepseek")
        .expect("the recorded catalog advertises deepseek")
        .env = String::new();

    // The run with an EMPTY key: the replayed test verdict still fails the
    // run (the fixture's recorded 401) — the WIRE is what this asserts.
    let _ = connect::run_onboarding(
        conv.client(),
        "octos-dev",
        "octos-dev",
        "",
        Provider::DeepSeek,
        Some(catalog),
    )
    .await;

    let test = server.sent("profile/llm/test").expect("llm/test sent");
    assert_eq!(
        test["api_key"], "octoscode-web-keyless-probe",
        "an empty key must negotiate to the web's exact probe value on the wire"
    );
    assert_eq!(
        test["selection"]["route"]["api_key_env"], "",
        "the keyless family keeps its empty env (the web's selectionFromCatalog)"
    );
}

#[tokio::test]
async fn the_recorded_catalog_parses_and_the_radio_derives_the_official_route() {
    let frames = fixture();
    let catalog_body = recorded(&frames, "profile/llm/catalog").body.clone();
    let catalog: octoscode_client::domains::profile::LlmCatalogResult =
        serde_json::from_value(catalog_body).expect("the recorded catalog parses");
    assert!(
        catalog.families.iter().any(|f| f.id == "deepseek"),
        "the recorded catalog advertises the deepseek radio"
    );
    let sel = connect::selection_from_catalog(&catalog, Provider::DeepSeek).expect("derives");
    assert_eq!(sel.family_id, "deepseek");
    assert_eq!(sel.model_id, "deepseek-v4-flash");
    assert_eq!(sel.route.api_key_env.as_deref(), Some("DEEPSEEK_API_KEY"));
}

#[test]
fn the_recorded_failure_lands_on_the_lowered_cards_and_the_bindings() {
    let raw = recorded(&fixture(), "profile/llm/test");
    let recorded_error = raw.body["error"]
        .as_str()
        .expect("the recorded test reply carries its real error text")
        .to_owned();

    // As `perform_screen_action` records it: the run's Err onto the state.
    let mut ui = ConnectUi::default();
    ui.onboarding_error = Some(recorded_error.clone());
    ui.note_connect_error("The server refused this token", "9:41 PM");

    // 2.3: the panel's error slot carries the verbatim recorded body.
    let dsl = connect::lower_screen(Screen::Onboarding, &ui).expect("onboarding lowers");
    assert!(
        dsl.contains(recorded_error.trim_matches('"').get(0..40).unwrap_or("authentication")),
        "the lowered onboarding card must carry the recorded error"
    );
    // 2.2: the §5.1 copy + the timestamp row.
    let dsl = connect::lower_screen(Screen::ConnectFailed, &ui).expect("connect-failed lowers");
    assert!(dsl.contains("The server refused this token"));
    assert!(dsl.contains("Last tried 9:41 PM"));
    // The binding projection for the outer loop's re-check.
    let f = connect::query(&ui, "connect.failure").unwrap();
    assert_eq!(f["kind"], "rejected-token");
    assert_eq!(f["focus_token"], true);
    assert_eq!(
        connect::query(&ui, "onboarding.error").unwrap(),
        serde_json::json!(recorded_error)
    );
    // The recorded failure is a provider verdict, NOT a §5.1 class: the
    // onboarding screen shows it raw, the connect screen stays unclassified
    // for it (an unrelated error is never force-diagnosed).
    assert!(connect::failure_for(&recorded_error, "http://x").is_none());
    assert_eq!(
        connect::failure_for("The server refused this token", "http://x")
            .unwrap()
            .kind,
        FailureKind::RejectedToken
    );
}
