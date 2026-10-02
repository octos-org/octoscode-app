//! A10 — the autonomy dialogs' remaining production paths, against
//! recorded + faithful traffic: the loop creation form's three cadences
//! (Maintenance / Self-paced / Fixed interval, `LoopCreationControls.tsx`),
//! the Monitors dialog's create form (`AutonomyPanel.tsx:431-482`), the
//! family alert line that shows only while the op stays authorized and
//! clears on success, and the close-time suspend (an in-flight refresh never
//! publishes after the dialog closed; the notification fold stays bound).
//!
//! Replies: the r1-autonomy open (capabilities) and loop/list, the c24b
//! monitor/create reply, and `a10-autonomy-faithful.jsonl` for what no
//! recording carries (a maintenance create, a refused monitor create).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::{autonomy as au, dialog};

struct Frame {
    dir: String,
    method: String,
    body: Value,
}

fn fixture(name: &str) -> Vec<Frame> {
    let path = format!("{}/../octoscode-client/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
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

fn recorded(frames: &[Frame], method: &str) -> Vec<Value> {
    frames.iter().filter(|f| f.dir == "in" && f.method == method).map(|f| f.body.clone()).collect()
}

fn recorded_open() -> Value {
    recorded(&fixture("r1-autonomy-a6ea8505.jsonl"), "session/open")
        .into_iter()
        .find(|b| b.get("active_profile_id").is_some())
        .expect("r1 recorded the open result")
}

/// A fake WS server: per-method replies IN ORDER (the last repeating), a
/// `{"__error__": …}` body answered as a JSON-RPC error, an optional
/// per-method delay, and a log of every (method, params).
struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
    async fn start(canned: Vec<(String, Value)>, delays: Vec<(&'static str, u64)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let mut queues: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for (m, b) in canned {
            queues.entry(m).or_default().push(b);
        }
        let delays: BTreeMap<&str, u64> = delays.into_iter().collect();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (tx, mut rx) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
            let mut pos: BTreeMap<String, usize> = BTreeMap::new();
            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                log.lock().unwrap().push((method.clone(), v["params"].clone()));
                let body = match queues.get(&method) {
                    Some(list) => {
                        let k = pos.entry(method.clone()).or_insert(0);
                        let b = list[(*k).min(list.len() - 1)].clone();
                        *k += 1;
                        b
                    }
                    None => json!({}),
                };
                let frame = match body.get("__error__") {
                    Some(err) => json!({"jsonrpc": "2.0", "id": id, "error": err}),
                    None => json!({"jsonrpc": "2.0", "id": id, "result": body}),
                };
                let delay = delays.get(method.as_str()).copied().unwrap_or(0);
                let tx = tx.clone();
                tokio::spawn(async move {
                    if delay > 0 {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                    let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
                });
            }
        });
        Self { base_url: format!("http://{addr}"), seen }
    }

    fn sent_all(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }
}

async fn connect(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    let conv = Arc::new(conv);
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(None).await.expect("session/open");
    for _ in 0..50 {
        if conv.store.domains.autonomy.bound_session().is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    conv.store.domains.session.set_active(Some(conv.session_id()));
    conv
}

/// The dialog/autonomy caches are process statics: one test at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn canned() -> Vec<(String, Value)> {
    let faithful = fixture("a10-autonomy-faithful.jsonl");
    let mut v: Vec<(String, Value)> = vec![("session/open".into(), json!({"opened": recorded_open()}))];
    for f in faithful.iter().filter(|f| f.dir == "in") {
        v.push((f.method.clone(), f.body.clone()));
    }
    v
}

/// The loop form's cadences, end to end: Maintenance sends the web's exact
/// body (`prompt: ""`, `mode: "maintenance"`), its reply folds into the
/// Loops dialog; Self-paced / Fixed interval go out with their own bodies;
/// every refusal stays on the form and sends nothing.
#[tokio::test]
async fn the_loop_form_sends_each_cadence_with_the_webs_body() {
    let _s = serial();
    au::reset_state();
    let server = Server::start(canned(), vec![]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    let ui = Mutex::new(FlowUi::default());
    // The seedless form is Maintenance with an optional prompt.
    let form = dialog::form_for("loop.create", "").expect("loop form");
    assert_eq!(form.mode.as_deref(), Some("maintenance"));
    let effect = dialog::form_effect(&form, &[String::new()]).expect("an effect");
    assert!(au::action_advertised("loop.create", &store), "r1 advertises loop/create");
    au::apply(effect, &conv).await.expect("maintenance create");
    let sent = server.sent_all("loop/create");
    assert_eq!(sent[0], json!({"prompt": "", "session_id": conv.session_id(), "mode": "maintenance"}));
    let st = au::state_snapshot();
    assert!(st.loops.iter().any(|l| l["mode"] == "maintenance"), "the reply folds: {:?}", st.loops);
    dialog::open(dialog::Dialog::Loops);
    let ctx = Ctx::new(&store, &ui);
    let m = dialog::lower(dialog::Dialog::Loops, &ctx, 990.0, 603.0).expect("loops");
    assert!(m.dsl.contains("run maintenance checks"), "the server's maintenance prompt is the row");
    // Self-paced and fixed: their bodies (the r1-recorded fixed shape).
    let sp = dialog::loop_form("self_paced", "", "");
    au::apply(dialog::form_effect(&sp, &["Summarize drift".into()]).unwrap(), &conv).await.ok();
    let fx = dialog::loop_form("fixed", "", "");
    au::apply(dialog::form_effect(&fx, &["Run CI smoke".into(), "15m".into()]).unwrap(), &conv).await.ok();
    let sent = server.sent_all("loop/create");
    assert_eq!(sent[1], json!({"prompt": "Summarize drift", "session_id": conv.session_id(), "mode": "self_paced"}));
    assert_eq!(
        sent[2],
        json!({"prompt": "Run CI smoke", "session_id": conv.session_id(), "mode": "fixed_interval", "interval_seconds": 900})
    );
    // Refusals: no frame.
    for (mode, values) in [("self_paced", vec![" ".to_owned()]), ("fixed", vec!["x".into(), "90".into()]), ("fixed", vec!["x".into(), "59s".into()])] {
        let f = dialog::loop_form(mode, "", "");
        match dialog::form_effect(&f, &values).unwrap() {
            au::Effect::Unhandled(why) => assert!(dialog::notice_for_refusal(&why).is_some(), "{why} has copy"),
            other => panic!("{mode} {values:?} must be refused, got {other:?}"),
        }
    }
    assert_eq!(server.sent_all("loop/create").len(), 3, "refusals send nothing");
    dialog::close();
}

/// "+ New monitor": a refused create (the server's monitor_invalid_spec for
/// an uncompilable regex) becomes the Monitors dialog's alert line; the
/// corrected create succeeds, clears the alert and the monitor row appears.
/// The body is the web's (`{session_id, name, argv, filter_regex?, mode}`).
#[tokio::test]
async fn the_monitor_form_creates_and_a_refusal_is_the_dialogs_alert() {
    let _s = serial();
    au::reset_state();
    let server = Server::start(canned(), vec![]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    let ui = Mutex::new(FlowUi::default());
    dialog::open(dialog::Dialog::Monitors);
    let ctx = Ctx::new(&store, &ui);
    // The create control is drawn (monitor/create is advertised) and wired.
    let m = dialog::lower(dialog::Dialog::Monitors, &ctx, 990.0, 603.0).expect("monitors");
    assert!(m.taps.iter().any(|(_, e)| e == "dialog.form.monitor.create"), "{:?}", m.taps);
    assert!(m.dsl.contains("+ New monitor"));
    let form = dialog::form_for("monitor.create", "").expect("monitor form");
    assert_eq!(form.fields.len(), 3);
    // A non-array command never leaves the module.
    assert_eq!(
        dialog::form_effect(&form, &["watch-build".into(), "./scripts/watch.sh".into(), String::new()]),
        Some(au::Effect::Unhandled("monitor.create[argv]".into()))
    );
    assert_eq!(
        dialog::form_effect(&form, &["watch-build".into(), "[\"\"]".into(), String::new()]),
        Some(au::Effect::Unhandled("monitor.create[argv]".into())),
        "empty argv entries are refused like the web"
    );
    let bad = dialog::form_effect(&form, &["watch-build".into(), "[\"./scripts/watch.sh\", \"--verbose\"]".into(), "(".into()]).unwrap();
    assert!(au::apply(bad, &conv).await.is_err(), "the server refuses");
    let alert = store.domains.autonomy.error(au::FAMILY_MONITORS).expect("recorded while authorized");
    assert!(alert.contains("filter_regex does not compile"), "{alert}");
    let m = dialog::lower(dialog::Dialog::Monitors, &ctx, 990.0, 603.0).unwrap();
    assert!(m.dsl.contains("filter_regex does not compile"), "the alert line renders");
    let good = dialog::form_effect(&form, &["watch-build".into(), "[\"./scripts/watch.sh\", \"--verbose\"]".into(), "ERROR.*".into()]).unwrap();
    au::apply(good, &conv).await.expect("created");
    let sent = server.sent_all("monitor/create");
    assert_eq!(
        sent[1],
        json!({"session_id": conv.session_id(), "name": "watch-build", "argv": ["./scripts/watch.sh", "--verbose"],
               "filter_regex": "ERROR.*", "mode": "poll"})
    );
    assert!(store.domains.autonomy.error(au::FAMILY_MONITORS).is_none(), "success clears the alert");
    let st = au::state_snapshot();
    assert!(st.monitors.iter().any(|m| m["monitor_id"] == "monitor_03"), "the created row folds");
    let m = dialog::lower(dialog::Dialog::Monitors, &ctx, 990.0, 603.0).unwrap();
    assert!(!m.dsl.contains("filter_regex does not compile") && m.dsl.contains("./scripts/watch.sh"), "the row (its command may end in an ellipsis)");
    dialog::close();
}

/// The close-time suspend and the authorized-only alert: a loops refresh
/// still in flight when the dialog closes never publishes (its revision was
/// superseded); a failure that lands after the authority moved on is not
/// recorded; a reopen's refresh applies.
#[tokio::test]
async fn a_closed_dialogs_late_reply_never_publishes_and_a_stale_error_is_dropped() {
    let _s = serial();
    au::reset_state();
    let r1 = fixture("r1-autonomy-a6ea8505.jsonl");
    let first_loop = recorded(&r1, "loop/updated").remove(0)["loop"].clone();
    let mut canned = canned();
    canned.push(("loop/list".into(), json!({"loops": [first_loop]})));
    canned.push(("monitor/list".into(), json!({"monitors": []})));
    let server = Server::start(canned, vec![("loop/list", 300), ("monitor/create", 300)]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    // Open: the refresh is dispatched; the user closes before it answers.
    let c2 = conv.clone();
    let refresh = tokio::spawn(async move { au::apply(au::Effect::RefreshLists, &c2).await });
    tokio::time::sleep(Duration::from_millis(60)).await;
    au::suspend(&store);
    refresh.await.unwrap().expect("the read itself succeeds");
    assert!(au::state_snapshot().loops.is_empty(), "the closed dialog's late reply did not publish");
    // Reopen: a fresh refresh applies.
    au::apply(au::Effect::RefreshLists, &conv).await.expect("refresh");
    assert_eq!(au::state_snapshot().loops.len(), 1, "the reopened dialog's refresh applies");
    // A failure that answers after the authority moved on is dropped.
    let c3 = conv.clone();
    let form = dialog::form_for("monitor.create", "").unwrap();
    let bad = dialog::form_effect(&form, &["w".into(), "[\"x\"]".into(), "(".into()]).unwrap();
    let create = tokio::spawn(async move { au::apply(bad, &c3).await });
    tokio::time::sleep(Duration::from_millis(60)).await;
    store.domains.autonomy.bind_identity("client-2:retired-socket");
    assert!(create.await.unwrap().is_err());
    assert!(store.domains.autonomy.error(au::FAMILY_MONITORS).is_none(), "a superseded epoch records nothing");
}
