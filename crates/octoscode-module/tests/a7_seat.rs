//! A7 — the driver-seat handover before ONE send, at the wire (production
//! path: `Conversation::submit_draft` → the turn controller's seat gate,
//! `Conversation::resume_chat` = the held banner's Take over, and
//! `Conversation::refresh_seat` = the shell's `session/driver/get` probe).
//!
//! A scripted fake server keeps one driver record (mode, binding, proof) and
//! answers `session/driver/get|acquire|release` and `turn/start` the way the
//! Core does: `turn/start` is refused `ExternalMasterHeld` while the Session
//! is external, an acquire is a CAS on the revision, a release must carry the
//! acquire's proof. Ports `composer-seat-handover.test.ts` (:101 release
//! before the one send, :119 a foreign holder is never a silent send, :143 an
//! unproven own lease waits, :296/:310 the gate's wire order and the kept
//! draft) and the §5.2 Resume chat sequence (`resumeChatSend`).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::{chrome, seat};

const PROFILE: &str = "a7";
const ME: &str = chrome::NATIVE_DRIVER_ID;

enum Reply {
    Ok(Value),
    Err(i64, String),
}

type Script = Arc<dyn Fn(&str, &Value) -> Reply + Send + Sync>;

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
    async fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { continue };
                let (mut sink, mut source) = ws.split();
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                tokio::spawn(async move {
                    while let Some(frame) = rx.recv().await {
                        if sink.send(Message::Text(frame.into())).await.is_err() {
                            break;
                        }
                    }
                });
                let (seen, script) = (seen2.clone(), script.clone());
                tokio::spawn(async move {
                    while let Some(Ok(msg)) = source.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        seen.lock().unwrap().push((method.clone(), v["params"].clone()));
                        let frame = match script(&method, &v["params"]) {
                            Reply::Ok(result) => json!({"jsonrpc": "2.0", "id": v["id"], "result": result}),
                            Reply::Err(code, message) => {
                                json!({"jsonrpc": "2.0", "id": v["id"], "error": {"code": code, "message": message}})
                            }
                        };
                        let _ = tx.send(frame.to_string());
                    }
                });
            }
        });
        Self { base_url, seen }
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    /// The seat-relevant frames, in wire order.
    fn seat_order(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .map(|(m, _)| m.clone())
            .filter(|m| m.starts_with("session/driver/") && m != "session/driver/get" || m == "turn/start")
            .collect()
    }
}

/// The server's one driver record (`session/driver/get`'s disclosure plus
/// the live proof).
struct Driver {
    external: bool,
    driver: String,
    epoch: u64,
    revision: u64,
    lease: u64,
    token: Option<String>,
    /// Refuse this many releases (a §6 row-8 refusal).
    release_fails: usize,
}

impl Driver {
    fn foreign_parked() -> Self {
        Self { external: true, driver: "octos-tui".into(), epoch: 2, revision: 7, lease: 0, token: None, release_fails: 0 }
    }

    fn disclosure(&self) -> Value {
        if !self.external {
            return json!({"mode": "internal", "recovery": "none", "binding": null});
        }
        json!({"mode": "external", "recovery": "none", "binding": {
            "driver_id": self.driver, "epoch": self.epoch, "revision": self.revision,
            "lease_expires_at_ms": self.lease}})
    }
}

fn open_reply(params: &Value) -> Value {
    let session = params["session_id"].as_str().unwrap_or("a7:main");
    json!({"opened": {
        "session_id": session,
        "active_profile_id": PROFILE,
        "workspace_root": "/home/user/src/octos",
        "cursor": {"stream": session, "seq": 1},
        "capabilities": {
            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
            "capabilities_schema_version": 1,
            "supported_methods": ["session/open", "session/list", "session/hydrate", "turn/start",
                "turn/interrupt", "session/driver/get", "session/driver/acquire", "session/driver/release"],
            "supported_notifications": ["turn/started", "turn/completed", "message/delta"],
            "supported_features": []
        }
    }})
}

fn seat_server(state: Arc<Mutex<Driver>>) -> Script {
    Arc::new(move |m: &str, p: &Value| {
        let mut d = state.lock().unwrap();
        match m {
            "session/open" => Reply::Ok(open_reply(p)),
            "session/list" => Reply::Ok(json!({"sessions": []})),
            "session/driver/get" => Reply::Ok(d.disclosure()),
            "session/driver/acquire" => {
                if p["expected_revision"].as_u64() != Some(d.revision) {
                    return Reply::Err(-32010, "driver_revision_conflict".into());
                }
                let who = p["driver_id"].as_str().unwrap_or_default().to_owned();
                if d.external && d.lease > seat::now_ms() && d.driver != who {
                    return Reply::Err(-32012, "driver_busy".into());
                }
                d.external = true;
                d.driver = who;
                d.epoch += 1;
                d.revision += 1;
                d.lease = seat::now_ms() + 60_000;
                let token = format!("tok-{}", d.epoch);
                d.token = Some(token.clone());
                Reply::Ok(json!({"control_token": token, "recovery": "none", "binding": {
                    "driver_id": d.driver, "epoch": d.epoch, "revision": d.revision,
                    "lease_expires_at_ms": d.lease}}))
            }
            "session/driver/release" => {
                if d.release_fails > 0 {
                    d.release_fails -= 1;
                    return Reply::Err(-32011, "driver_release_refused".into());
                }
                let proven = d.token.as_deref() == p["control_token"].as_str()
                    && p["epoch"].as_u64() == Some(d.epoch)
                    && p["expected_revision"].as_u64() == Some(d.revision)
                    && p["driver_id"].as_str() == Some(d.driver.as_str());
                if !proven || p["next"] != "internal" {
                    return Reply::Err(-32013, "driver_fence_stale".into());
                }
                d.external = false;
                d.revision += 1;
                d.lease = 0;
                d.token = None;
                Reply::Ok(json!({"mode": "internal", "recovery": "none"}))
            }
            "turn/start" if d.external => {
                Reply::Err(-32003, "turn admission refused for this session: ExternalMasterHeld".into())
            }
            "turn/start" => Reply::Ok(json!({"accepted": true})),
            _ => Reply::Ok(json!({})),
        }
    })
}

async fn connected(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", PROFILE, None, None).expect("connect");
    let conv = Arc::new(conv);
    conv.attach();
    conv.open_workspace(None).await.expect("session/open");
    let mut opened = false;
    for _ in 0..80 {
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
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    // A fresh Session: no proof, no observation, no holder.
    let session = conv.session_id();
    seat::drop_proof(&session);
    seat::observe(&session, None);
    seat::set_status(&session, None);
    chrome::set_held(&session, None);
    conv
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    std::env::set_var("OCTOSCODE_TURN_START_TIMEOUT_MS", "1500");
    std::env::remove_var("OCTOSCODE_HELD_BY");
    let dir = std::env::temp_dir().join(format!("a7-seat-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("drafts.json"));
    g
}

async fn submit(conv: &Arc<Conversation>, text: &str) {
    conv.set_draft(text);
    let _ = conv.submit_draft().await;
}

fn notices(conv: &Conversation) -> Vec<(String, String)> {
    let session = conv.session_id();
    conv.store
        .domains
        .session
        .timeline
        .entries(&session)
        .into_iter()
        .filter(|e| e.kind == octoscode_store::EntryKind::SYSTEM_NOTICE)
        .map(|e| (e.data["title"].as_str().unwrap_or("").to_owned(), e.text.clone()))
        .collect()
}

fn draft(conv: &Conversation) -> String {
    conv.ui().lock().unwrap().draft()
}

// :119 — a foreign holder + send: never a silent send, the draft stays.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_foreign_holder_refuses_the_send_with_no_frame_and_keeps_the_draft() {
    let _g = lock();
    let state = Arc::new(Mutex::new(Driver::foreign_parked()));
    let server = Server::start(seat_server(state.clone())).await;
    let conv = connected(&server).await;
    let session = conv.session_id();

    // The shell's probe: the held banner names the holder.
    conv.refresh_seat(&session).await;
    assert_eq!(chrome::held_by_other(&conv.store).as_deref(), Some("octos-tui"));

    submit(&conv, "keep me").await;
    assert!(server.seat_order().is_empty(), "no acquire, no release, no turn/start: {:?}", server.seat_order());
    let n = notices(&conv);
    assert!(
        n.iter().any(|(t, b)| t == "Turn not sent" && b == seat::FOREIGN_SEAT_HOLDER_MESSAGE),
        "the bounded human message: {n:?}"
    );
    assert_eq!(draft(&conv), "keep me", "the text is back in the composer");
}

// §5.2 case 3 — Take over (Resume chat): acquire on the observed revision →
// release(next: internal) with that proof → the draft is sent ONCE.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn take_over_acquires_releases_internal_then_sends_the_draft_once() {
    let _g = lock();
    let state = Arc::new(Mutex::new(Driver::foreign_parked()));
    let server = Server::start(seat_server(state.clone())).await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    conv.refresh_seat(&session).await;

    conv.set_draft("ship it");
    conv.resume_chat().await;

    assert_eq!(
        server.seat_order(),
        ["session/driver/acquire", "session/driver/release", "turn/start"],
        "acquire, then the handback, then exactly one send"
    );
    let acquire = &server.params_of("session/driver/acquire")[0];
    assert_eq!(acquire["expected_revision"], 7, "CAS on the OBSERVED revision");
    assert_eq!(acquire["driver_id"], ME);
    assert_eq!(acquire["lease_seconds"], 60);
    let release = &server.params_of("session/driver/release")[0];
    assert_eq!(release["next"], "internal", "never parked with external");
    assert_eq!(release["control_token"], "tok-3", "the proof from that acquire");
    assert_eq!(release["expected_revision"], 8);
    let start = &server.params_of("turn/start")[0];
    assert_eq!(start["input"][0]["text"], "ship it");
    assert!(chrome::held_by_other(&conv.store).is_none(), "the banner is gone");
    assert!(seat::proof(&session).is_none(), "the proof was spent on the handback");
    assert_eq!(seat::status(&session), None);
    assert_eq!(draft(&conv), "", "the sent draft clears");
}

// :101/:296/:310 — a refused handback sends nothing; the kept proof then
// releases BEFORE the one turn/start of the next send.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_handback_sends_nothing_and_the_kept_seat_releases_before_the_next_send() {
    let _g = lock();
    let state = Arc::new(Mutex::new(Driver { release_fails: 1, ..Driver::foreign_parked() }));
    let server = Server::start(seat_server(state.clone())).await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    conv.refresh_seat(&session).await;

    conv.set_draft("retry me");
    conv.resume_chat().await;
    assert_eq!(server.seat_order(), ["session/driver/acquire", "session/driver/release"]);
    assert!(server.params_of("turn/start").is_empty(), "a refused release sends no turn/start");
    assert_eq!(
        seat::status(&session),
        Some((seat::RELEASE_FAILED_MESSAGE.to_owned(), true)),
        "the §6 row-8 message"
    );
    assert!(seat::proof(&session).is_some(), "this app now holds the seat, with proof");
    assert_eq!(draft(&conv), "retry me", "the draft is kept");

    // The next send hands the seat back first, then sends once.
    submit(&conv, "retry me").await;
    assert_eq!(
        server.seat_order(),
        ["session/driver/acquire", "session/driver/release", "session/driver/release", "turn/start"]
    );
    assert_eq!(server.params_of("session/driver/release")[1]["next"], "internal");
    assert_eq!(server.params_of("turn/start").len(), 1);
    assert!(seat::proof(&session).is_none());
    assert_eq!(seat::status(&session), None, "the status line clears");
}

// :143 — a LIVE lease under our own id with no proof (a lost acquire reply):
// nothing may be sent with the unproven binding.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unproven_live_lease_under_our_id_waits_for_its_expiry() {
    let _g = lock();
    let lease = seat::now_ms() + 120_000;
    let state = Arc::new(Mutex::new(Driver {
        external: true,
        driver: ME.into(),
        epoch: 5,
        revision: 4,
        lease,
        token: Some("lost".into()),
        release_fails: 0,
    }));
    let server = Server::start(seat_server(state.clone())).await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    conv.refresh_seat(&session).await;
    assert!(chrome::held_by_other(&conv.store).is_none(), "our own id is no foreign banner");

    submit(&conv, "wait for it").await;
    assert!(server.seat_order().is_empty(), "no release, no renew, no turn/start: {:?}", server.seat_order());
    let n = notices(&conv);
    assert!(n.iter().any(|(_, b)| b == seat::WAIT_FOR_EXPIRY_MESSAGE), "{n:?}");
    assert_eq!(draft(&conv), "wait for it");
}

// §6 row 1 — the server's own ExternalMasterHeld refusal (the app had not
// seen the holder yet) is bounded, keeps the text and reveals the holder.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_refusal_is_bounded_and_reveals_the_holder() {
    let _g = lock();
    let state = Arc::new(Mutex::new(Driver::foreign_parked()));
    let server = Server::start(seat_server(state.clone())).await;
    let conv = connected(&server).await;

    // No probe yet: the plan is a plain send, and the Core refuses it.
    submit(&conv, "hello").await;
    assert_eq!(server.params_of("turn/start").len(), 1);
    let n = notices(&conv);
    assert!(n.iter().any(|(_, b)| b == seat::FOREIGN_SEAT_HOLDER_MESSAGE), "bounded: {n:?}");
    assert!(!n.iter().any(|(_, b)| b.contains("ExternalMasterHeld")), "no protocol words: {n:?}");
    assert_eq!(draft(&conv), "hello");
    assert_eq!(chrome::held_by_other(&conv.store).as_deref(), Some("octos-tui"), "the banner appears");
}
