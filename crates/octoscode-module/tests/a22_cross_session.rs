//! A22 — the cross-session Stop defect (A20's finding, the row-250 class: an
//! action applied to another Session than the one it was pressed in), on the
//! production path: the real WS transport, the real `Conversation` and the
//! lib.rs event loop, and the host's OWN turn dispatch —
//! `actions::resolve("turn.interrupt")` (the Stop button, the Escape key and
//! `/stop` all resolve that id) performed by `actions::perform_turn` (what
//! lib.rs spawns).
//!
//! The fake Core runs long streamed turns and interrupts the turn a
//! `turn/interrupt` NAMES, whichever Session runs it — the behaviour A20
//! observed (octos a6ea8505's own `decide_interrupt`,
//! `ui_protocol_transport.rs:25708-25723`, declines a pair whose turn is not
//! the named Session's; this test pins the client either way: no frame for
//! another Session's turn may leave the app).
//!
//! The web: a Stop is the SELECTED record's (`use-turn-controller.ts:860-930`
//! interrupt on the record's own queue head; only the selected record's
//! controller is the composer's, `use-octos-session.ts:2774-2800`).
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::actions;
use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, History};

const PROFILE: &str = "a22";
const CWD: &str = "/home/user/a22-ws";

/// One streamed turn on the fake Core.
struct Running {
    session: String,
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
struct World {
    seen: Vec<(String, Value)>,
    running: HashMap<String, Running>,
    /// Sessions whose `session/hydrate` is held (never answered).
    held: HashSet<String>,
    cursor: u64,
}

struct Core {
    base: String,
    world: Arc<Mutex<World>>,
    push: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>>,
}

fn note(method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string()
}

fn opened(session: &str, cwd: &str) -> Value {
    json!({"opened": {
        "session_id": session, "active_profile_id": PROFILE, "workspace_root": cwd,
        "cursor": {"stream": session, "seq": 1},
        "capabilities": {
            "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
            "capabilities_schema_version": 2,
            "supported_methods": ["session/open", "session/hydrate", "session/list", "turn/start", "turn/interrupt", "turn/steer",
                                  "turn/state/get"],
            "supported_notifications": ["turn/started", "projection/envelope"],
            "supported_features": ["state.session_hydrate.v1", "projection.envelope.v2", "session.workspace_cwd.v1",
                                   "event.turn_steer_dropped.v1"]
        }
    }})
}

impl Core {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let world: Arc<Mutex<World>> = Arc::new(Mutex::new(World { cursor: 10, ..Default::default() }));
        let push: Arc<Mutex<Option<mpsc::UnboundedSender<String>>>> = Arc::new(Mutex::new(None));
        let (w2, p2) = (world.clone(), push.clone());
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { continue };
                let (mut sink, mut source) = ws.split();
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                *p2.lock().unwrap() = Some(tx.clone());
                tokio::spawn(async move {
                    while let Some(frame) = rx.recv().await {
                        if sink.send(Message::Text(frame.into())).await.is_err() {
                            break;
                        }
                    }
                });
                let world = w2.clone();
                tokio::spawn(async move {
                    while let Some(Ok(msg)) = source.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        let p = v["params"].clone();
                        let session = p["session_id"].as_str().unwrap_or("").to_owned();
                        world.lock().unwrap().seen.push((method.clone(), p.clone()));
                        let ok = |r: Value| json!({"jsonrpc": "2.0", "id": v["id"], "result": r}).to_string();
                        match method.as_str() {
                            "session/open" => {
                                let _ = tx.send(ok(opened(&session, p["cwd"].as_str().unwrap_or(CWD))));
                            }
                            "session/hydrate" => {
                                if world.lock().unwrap().held.contains(&session) {
                                    continue; // never answered
                                }
                                let _ = tx.send(ok(json!({"session_id": session, "cursor": {"stream": session, "seq": 1}, "messages": []})));
                            }
                            "session/list" => {
                                let _ = tx.send(ok(json!({"sessions": [], "workspace_root": CWD, "profile_id": PROFILE})));
                            }
                            "turn/start" => {
                                let turn = p["turn_id"].as_str().unwrap_or("").to_owned();
                                if p["input"][0]["text"].as_str().unwrap_or("").contains("[hold]") {
                                    continue; // a lost acknowledgement: never answered
                                }
                                let _ = tx.send(ok(json!({"accepted": true})));
                                Core::stream(world.clone(), tx.clone(), session, turn, 60);
                            }
                            "turn/state/get" => {
                                let turn = p["turn_id"].as_str().unwrap_or("").to_owned();
                                let _ = tx.send(ok(json!({"session_id": session, "turn_id": turn, "state": "completed"})));
                            }
                            "turn/steer" => {
                                // Accepted only into the named Session's own
                                // running turn.
                                let expected = p["expected_turn_id"].as_str().unwrap_or("").to_owned();
                                let own = world.lock().unwrap().running.get(&expected).is_some_and(|r| r.session == session);
                                let frame = if own {
                                    ok(json!({"turn_id": expected, "steered": true}))
                                } else {
                                    json!({"jsonrpc": "2.0", "id": v["id"], "error": {"code": -32000, "message": "no such running turn"}}).to_string()
                                };
                                let _ = tx.send(frame);
                            }
                            "turn/interrupt" => {
                                // Interrupt the turn the frame NAMES, wherever
                                // it runs (what A20 observed).
                                let turn = p["turn_id"].as_str().unwrap_or("").to_owned();
                                let hit = world.lock().unwrap().running.get(&turn).map(|r| r.stop.store(true, Ordering::SeqCst)).is_some();
                                let _ = tx.send(ok(json!({"interrupted": hit})));
                            }
                            _ => {
                                let _ = tx.send(ok(json!({})));
                            }
                        }
                    }
                });
            }
        });
        Self { base, world, push }
    }

    /// One turn streamed on `session`: `turn/started`, the prompt, `ticks`
    /// deltas 50 ms apart, then the terminal — `interrupted` as soon as an
    /// interrupt named it.
    fn stream(world: Arc<Mutex<World>>, tx: mpsc::UnboundedSender<String>, session: String, turn: String, ticks: u64) {
        let stop = Arc::new(AtomicBool::new(false));
        world.lock().unwrap().running.insert(turn.clone(), Running { session: session.clone(), stop: stop.clone() });
        tokio::spawn(async move {
            let env = |world: &Arc<Mutex<World>>, seq: u64, payload: Value| {
                let mut w = world.lock().unwrap();
                w.cursor += 1;
                json!({"session_id": session, "thread_id": turn, "turn_id": turn, "seq": seq,
                       "cursor": {"stream": session, "seq": w.cursor}, "payload": payload})
            };
            let _ = tx.send(note("turn/started", json!({"session_id": session, "turn_id": turn, "timestamp": "2026-10-02T09:00:00Z"})));
            let _ = tx.send(note("projection/envelope", env(&world, 1, json!({"type": "user_message", "data": {"text": "prompt"}}))));
            let seg = format!("{turn}:assistant:iteration:1");
            let mut seq = 2;
            let mut outcome = "completed";
            for k in 0..ticks {
                tokio::time::sleep(Duration::from_millis(50)).await;
                if stop.load(Ordering::SeqCst) {
                    outcome = "interrupted";
                    break;
                }
                let e = env(&world, seq, json!({"type": "assistant_delta", "data": {"text": format!("{k} "), "assistant_segment_id": seg}}));
                let _ = tx.send(note("projection/envelope", e));
                seq += 1;
            }
            let e = env(&world, seq, json!({"type": "turn_terminal", "data": {"outcome": outcome}}));
            let _ = tx.send(note("projection/envelope", e));
            world.lock().unwrap().running.remove(&turn);
        });
    }

    /// Another client starts a turn in `session` (the server pushes it).
    fn foreign_turn(&self, session: &str, turn: &str, ticks: u64) {
        let tx = self.push.lock().unwrap().clone().expect("a socket");
        Core::stream(self.world.clone(), tx, session.to_owned(), turn.to_owned(), ticks);
    }

    /// The server pushes a notification (the socket's writer).
    fn notify(&self, method: &str, params: Value) {
        if let Some(tx) = self.push.lock().unwrap().as_ref() {
            let _ = tx.send(note(method, params));
        }
    }

    fn hold_history(&self, session: &str) {
        self.world.lock().unwrap().held.insert(session.to_owned());
    }

    fn params_of(&self, method: &str) -> Vec<Value> {
        self.world.lock().unwrap().seen.iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    fn is_running(&self, turn: &str) -> bool {
        self.world.lock().unwrap().running.contains_key(turn)
    }

    fn running_in(&self, session: &str) -> Vec<String> {
        self.world.lock().unwrap().running.iter().filter(|(_, r)| r.session == session).map(|(t, _)| t.clone()).collect()
    }
}

async fn until(what: &str, f: impl Fn() -> bool) {
    for _ in 0..500 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for: {what}");
}

async fn launch(core: &Core) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&core.base, "dummy", PROFILE, Some(CWD.to_owned()), None).expect("connect");
    let conv = Arc::new(conv);
    conv.attach();
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(Some(CWD.to_owned())).await.expect("session/open");
    let s = conv.session_id();
    until("the startup Session settles", || conv.store.is_live() && conv.history(&s) == History::Ready).await;
    conv
}

/// A sidebar click (lib.rs `thread.open`): `open_session` with its folder,
/// then the Session's history settles.
async fn open(conv: &Arc<Conversation>, id: &str) {
    conv.open_session(id, Some(CWD.to_owned())).await.expect("session/open");
    until(&format!("{id} settles"), || {
        conv.store.active_session().as_deref() == Some(id) && conv.history(id) == History::Ready
    })
    .await;
}

/// A prompt sent from the composer (Enter), its turn accepted and streaming.
async fn run_long_turn(conv: &Arc<Conversation>, core: &Core, text: &str) -> String {
    let before = core.params_of("turn/start").len();
    conv.set_draft(text);
    conv.submit_draft().await.expect("submit");
    until("the turn was sent", || core.params_of("turn/start").len() > before).await;
    let turn = core.params_of("turn/start")[before]["turn_id"].as_str().unwrap().to_owned();
    until("the turn is live", || conv.store.domains.turn.is_in_flight(&turn) && core.is_running(&turn)).await;
    turn
}

/// The host's Stop: the button, Escape and `/stop` all resolve
/// `turn.interrupt` against the window's state and lib.rs spawns
/// `actions::perform_turn` with the result.
fn press_stop(conv: &Arc<Conversation>) -> actions::Effect {
    let ui = conv.ui();
    let ctx = Ctx::new(&conv.store, &ui);
    actions::resolve("turn.interrupt", 0, &ctx)
}

async fn perform(conv: &Arc<Conversation>, effect: actions::Effect) {
    let _ = actions::perform_turn(effect, conv).await;
}

fn interrupts_naming(core: &Core, turn: &str) -> Vec<Value> {
    core.params_of("turn/interrupt").into_iter().filter(|p| p["turn_id"] == json!(turn)).collect()
}

fn quit(conv: &Conversation) {
    let _ = conv.command_sender().try_send(octos_app_transport::OutboundCommand::Disconnect);
}

/// With a turn running in Session X, the person switches to Session Y and
/// presses Stop: X's turn is NOT interrupted (no `turn/interrupt` naming it
/// leaves the app) and it streams to its end. The same holds for a Session
/// the app opened but left before its history answered, when another client
/// starts a turn in it. A Stop pressed in X just before the switch still
/// stops X — under X's own Session. And Stop in X stops X.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_pressed_in_y_never_interrupts_another_sessions_turn() {
    let core = Core::start().await;
    let conv = launch(&core).await;
    let x = conv.session_id();
    let (y, z) = ("a22:api:y", "a22:api:z");

    // 1. A long turn in X; the person switches to Y and presses Stop.
    let tx = run_long_turn(&conv, &core, "a long job in X").await;
    open(&conv, y).await;
    let effect = press_stop(&conv);
    perform(&conv, effect).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(interrupts_naming(&core, &tx).is_empty(), "Stop in Y sent an interrupt for X's turn: {:?}", core.params_of("turn/interrupt"));
    until("X's turn streams to its end", || !core.is_running(&tx)).await;
    assert_eq!(conv.store.domains.turn.terminal(&tx).as_deref(), Some("completed"), "X's turn finished, not interrupted");

    // 2. A Session opened and left before its history answered (Z): another
    //    client starts a turn there while Y is on screen; Stop in Y.
    core.hold_history(z);
    conv.open_session(z, Some(CWD.to_owned())).await.expect("open Z");
    open(&conv, y).await;
    let tz = "01920000-0000-7000-8000-0000000002f1";
    core.foreign_turn(z, tz, 40);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let effect = press_stop(&conv);
    perform(&conv, effect).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(interrupts_naming(&core, tz).is_empty(), "Stop in Y sent an interrupt for Z's turn: {:?}", core.params_of("turn/interrupt"));
    until("Z's turn streams to its end", || !core.is_running(tz)).await;

    // 3. Stop pressed in X, the person switches to Y before it leaves: the
    //    interrupt is X's — X's Session, X's turn — and X stops.
    open(&conv, &x).await;
    let tx2 = run_long_turn(&conv, &core, "another long job in X").await;
    let effect = press_stop(&conv);
    open(&conv, y).await;
    perform(&conv, effect).await;
    until("the Stop pressed in X reached the wire", || !interrupts_naming(&core, &tx2).is_empty()).await;
    let sent = interrupts_naming(&core, &tx2);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0]["session_id"], json!(x), "the Stop pressed in X names X's Session: {}", sent[0]);
    until("X's second turn stopped", || !core.is_running(&tx2)).await;

    // 4. Positive control: Stop in X stops X.
    open(&conv, &x).await;
    let tx3 = run_long_turn(&conv, &core, "a third long job in X").await;
    let effect = press_stop(&conv);
    perform(&conv, effect).await;
    until("Stop in X reached the wire", || !interrupts_naming(&core, &tx3).is_empty()).await;
    assert_eq!(interrupts_naming(&core, &tx3)[0]["session_id"], json!(x));
    until("X's third turn stopped", || !core.is_running(&tx3)).await;
    until("its terminal landed", || conv.store.domains.turn.terminal(&tx3).as_deref() == Some("interrupted")).await;

    // Nothing ever named a turn of another Session than its own.
    for p in core.params_of("turn/interrupt") {
        let turn = p["turn_id"].as_str().unwrap_or_default();
        let owner = [&tx, tz, &tx2, &tx3].iter().position(|t| *t == turn).map(|i| [x.as_str(), z, x.as_str(), x.as_str()][i]);
        assert_eq!(owner, p["session_id"].as_str(), "an interrupt for another Session's turn: {p}");
    }
    assert!(core.running_in(&x).is_empty());
    quit(&conv);
}

fn steers(core: &Core) -> Vec<Value> {
    core.params_of("turn/steer")
}

/// The steer controls (A22 audit): the composer's steer action, a stale
/// steer naming another Session's turn, and the queued chip's "Steer now" —
/// each carries the Session it was issued in and is checked against THAT
/// Session's live turn before it is sent.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_steer_reaches_only_the_live_turn_of_the_session_it_was_issued_in() {
    let core = Core::start().await;
    let conv = launch(&core).await;
    let x = conv.session_id();
    let y = "a22:api:y";

    // A long turn in X; a prompt queued behind it (steering off: it queues).
    let tx = run_long_turn(&conv, &core, "a long job in X").await;
    conv.set_draft("queued for X");
    conv.submit_draft().await.expect("queue");
    until("X's prompt is queued", || conv.store.domains.composer.snapshot(&x).pending.len() == 1).await;
    until("X's turn is accepted", || conv.store.domains.composer.can_steer_now(&x)).await;

    // In Y, the steer action has no live turn to steer: nothing is sent.
    open(&conv, y).await;
    conv.set_draft("steer into whatever runs");
    let effect = {
        let ui = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui);
        actions::resolve("turn.steer", 0, &ctx)
    };
    assert_eq!(effect, actions::Effect::Unhandled("turn.steer".into()), "Y has no live turn");
    perform(&conv, effect).await;
    // A steer naming X's turn under Y is refused before the wire.
    let r = conv.steer_in(y, &tx, "stale").await.expect("no error");
    assert!(r.is_null(), "not sent: {r}");
    assert!(steers(&core).is_empty(), "no turn/steer left the app: {:?}", steers(&core));

    // X's chip "Steer now", tapped in X and run after the window moved to Y:
    // it steers X's head into X's turn, under X.
    assert!(conv.steer_queued_head_in(&x).await, "admitted as a steer of X's queue");
    until("the steer reached the wire", || !steers(&core).is_empty()).await;
    let sent = steers(&core);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!((sent[0]["session_id"].clone(), sent[0]["expected_turn_id"].clone()), (json!(x), json!(tx)), "{}", sent[0]);
    assert_eq!(sent[0]["input"][0]["text"], json!("queued for X"));

    // Positive control: the steer action in X steers X's own turn.
    open(&conv, &x).await;
    conv.set_draft("steer X");
    let effect = {
        let ui = conv.ui();
        let ctx = Ctx::new(&conv.store, &ui);
        actions::resolve("turn.steer", 0, &ctx)
    };
    perform(&conv, effect).await;
    until("X's own steer reached the wire", || steers(&core).len() == 2).await;
    assert_eq!((steers(&core)[1]["session_id"].clone(), steers(&core)[1]["expected_turn_id"].clone()), (json!(x), json!(tx)));
    quit(&conv);
}

/// The queued chip's ✕ and the recovery notice's "Check status" / "Continue
/// without it" (A22 audit): tapped in X and run after the window moved to Y,
/// each acts on X's own queue / held turn — never on Y's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_queue_and_recovery_controls_act_on_the_session_they_were_tapped_in() {
    // A lost acknowledgement is held for recovery after 1.5 s here.
    std::env::set_var("OCTOSCODE_TURN_START_TIMEOUT_MS", "1500");
    let core = Core::start().await;
    let conv = launch(&core).await;
    let x = conv.session_id();
    let y = "a22:api:y";

    // ✕: a prompt queued in X behind a long turn.
    let _tx = run_long_turn(&conv, &core, "a long job in X").await;
    conv.set_draft("remove me");
    conv.submit_draft().await.expect("queue");
    until("X's prompt is queued", || conv.store.domains.composer.snapshot(&x).pending.len() == 1).await;
    let head = conv.store.domains.composer.snapshot(&x).pending[0].turn_id.clone();
    open(&conv, y).await;
    assert!(conv.remove_queued_in(&x, &head), "X's chip removes X's queued prompt");
    assert!(conv.store.domains.composer.snapshot(&x).pending.is_empty());
    until("X's long turn ends", || conv.store.domains.composer.snapshot(&x).active.is_none()).await;
    assert!(!core.params_of("turn/start").iter().any(|p| p["turn_id"] == json!(head)), "the removed prompt was never sent");

    // Check status: a turn in X whose start was never acknowledged.
    open(&conv, &x).await;
    conv.set_draft("[hold] lost ack");
    conv.submit_draft().await.expect("submit");
    until("X holds the turn for recovery", || conv.store.domains.composer.recovery(&x).is_some()).await;
    let held = conv.store.domains.composer.recovery(&x).unwrap().turn_id;
    conv.set_draft("waits behind the held turn");
    // (Admission refuses while a turn is held: the text stays; nothing queues.)
    open(&conv, y).await;
    conv.check_turn_state_in(&x).await;
    let asked = core.params_of("turn/state/get");
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!((asked[0]["session_id"].clone(), asked[0]["turn_id"].clone()), (json!(x), json!(held)), "{}", asked[0]);
    until("X's held turn settled from the lookup", || conv.store.domains.composer.recovery(&x).is_none()).await;
    assert!(conv.store.domains.composer.recovery(y).is_none());

    // Continue without it: another held turn in X, released from Y's window.
    open(&conv, &x).await;
    conv.set_draft("[hold] lost again");
    conv.submit_draft().await.expect("submit");
    until("X holds the second turn", || conv.store.domains.composer.recovery(&x).is_some()).await;
    open(&conv, y).await;
    conv.continue_without_turn_in(&x);
    assert!(conv.store.domains.composer.recovery(&x).is_none(), "X's own hold is released");
    assert!(conv.store.domains.composer.snapshot(&x).active.is_none());
    quit(&conv);
}

/// The approval keys (A22 audit, the row-250 class): an approval pending in
/// X never turns Y/S/N into a decision while Y is on screen — nothing is
/// answered there under Y's id — and in X the keys answer X's own approval
/// under X. The shape is the recorded r5 `approval/requested`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_approval_keys_answer_only_the_approval_of_the_session_on_screen() {
    use octoscode_module::screens::keys::{self, KeyAction};
    let core = Core::start().await;
    let conv = launch(&core).await;
    let x = conv.session_id();
    let y = "a22:api:y";
    let approval = "01a0e773-f844-7d50-b171-b38d159f02aa";
    let tx = run_long_turn(&conv, &core, "a job in X that asks").await;
    open(&conv, y).await;
    core.notify("approval/requested", json!({
        "approval_id": approval, "approval_kind": "command", "body": "printf a22", "risk": "low",
        "session_id": x, "title": "A22 approval", "tool_name": "shell", "turn_id": tx,
        "typed_details": {"command": {"argv": ["printf", "a22"], "command_line": "printf a22",
                                      "tool_call_id": "a22-approval-1"}, "kind": "command"}
    }));
    until("X's approval is pending", || conv.store.domains.approval.detail(approval).is_some()).await;
    // In Y: the keys see no approval of Y — no decision, nothing sent.
    let in_y = conv.store.active_session().unwrap();
    assert_eq!(in_y, y);
    assert_eq!(keys::oldest_pending_id_in(&conv.store, &in_y), None, "Y shows no approval");
    assert_eq!(keys::key_decision(&conv.store, &in_y, &KeyAction::ApprovalApproveRequest), None);
    assert!(!conv.ui().lock().unwrap().approval_pending(), "Y's window is not waiting on X's approval");
    // Back in X: the key answers X's own approval, under X.
    open(&conv, &x).await;
    let body = keys::key_decision(&conv.store, &x, &KeyAction::ApprovalApproveRequest).expect("X's approval");
    assert_eq!((body["approval_id"].clone(), body["session_id"].clone()), (json!(approval), json!(x)), "{body}");
    conv.client().request("approval/respond", body).await.expect("sent");
    let sent = core.params_of("approval/respond");
    assert_eq!(sent.len(), 1, "exactly the one decision made in X: {sent:?}");
    assert_eq!(sent[0]["session_id"], json!(x));
    quit(&conv);
}
