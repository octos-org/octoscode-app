//! A30 — the peer dock in the sidebar (parity row 270; web
//! `features/peers/PeerDock.tsx`, `peer-row-view.ts`,
//! `features/shell/ProductSidebar.tsx:959-967`, `app/App.tsx:1088-1119`;
//! board 4 regions 6/7).
//!
//! Every test drives the SAME production path the dock's clicks run in the
//! app: `peer_dock::lower` (what the sidebar mounts, with the ids each
//! control was drawn for), `peer_dock::perform` (the routed tap or the
//! ⌥Y / ⌥N key) and `peer_dock::run` (the job the host spawns, through
//! `fleet_driver`'s one control chain) — against a fake server that serves
//! TWO dispatched peers, each blocked on its OWN approval (distinct ids,
//! sessions, operations and adopted turns), and that can push a replacement
//! approval on demand (`test/kick`).
//!
//! The safety bar (a wrong-target action is a safety bug): every dock action
//! carries the row and the pending approval id it was drawn for, sends
//! exactly ONE frame bound to THAT row's ids, and sends NOTHING once that
//! row's pending id changed.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::screens::board3::fleetview::Status;
use octoscode_module::screens::board3::host::{self as b3, Job as B3Job, Outcome as B3Outcome};
use octoscode_module::screens::peer_dock::{self as dock, Outcome, Seat};
use octoscode_module::screens::{fleet_driver, peers};
use octoscode_store::domains::peer::{Activity, Ack, Origin, Outcome as TurnOutcome, PeerRow, PeerSessionEvent, RequestKind, RowStatus};
use octoscode_store::Store;

const SESSION: &str = "dsflash:main";

// ------------------------------------------------------------- fixture

fn fixture_body(method: &str) -> Value {
    let path = format!(
        "{}/../octoscode-client/tests/fixtures/a10-fleet-driver-synthetic.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["dir"] == "in" && v["method"] == method)
        .map(|v| v["body"].clone())
        .unwrap_or_else(|| panic!("fixture has no {method}"))
}

fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_matches('-').to_owned()
}

/// The `n`-th approval id of a peer: approval ids are UUIDs on the wire
/// (`ApprovalId(Uuid)`, octos-core `ui_protocol.rs:629`) — a non-UUID id is
/// dropped by the decoder before it reaches any fold.
fn ap(slug: &str, n: u64) -> String {
    let tag: u64 = match slug {
        "alpha" => 0xa1,
        "beta" => 0xb2,
        _ => 0xff,
    };
    format!("01a0eb92-9444-7101-aa6f-{:012x}", (tag << 16) | n)
}

/// The pending approval the fake server announces for one peer session.
fn approval(session: &str, turn: &str, id: &str, command_line: &str) -> Value {
    let mut r = fixture_body("approval/requested");
    r["session_id"] = json!(session);
    r["turn_id"] = json!(turn);
    r["approval_id"] = json!(id);
    r["typed_details"] = json!({"kind": "command", "command": {"command_line": command_line}});
    r
}

// -------------------------------------------------------------- server

#[derive(Default)]
struct World {
    /// slug -> (adopted session, adopted turn, operation id).
    peers: Vec<(String, String, String, String)>,
    /// Notifications pushed after the next `test/kick` request.
    kick: Vec<(String, Value)>,
}

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    world: Arc<Mutex<World>>,
}

impl Server {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let world = Arc::new(Mutex::new(World::default()));
        let (log, w) = (seen.clone(), world.clone());
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (mut tx, mut rx) = ws.split();
            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let Some(id) = v.get("id").cloned() else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let p = v["params"].clone();
                log.lock().unwrap().push((method.clone(), p.clone()));
                let mut pushes: Vec<(String, Value)> = Vec::new();
                let reply: Value = match method.as_str() {
                    "session/open" => {
                        let mut opened = fixture_body("session/open");
                        let sid = p["session_id"].as_str().unwrap_or(SESSION).to_owned();
                        opened["session_id"] = json!(sid);
                        // A dispatched peer's background attach: its own
                        // frames follow (turn started, then ITS approval).
                        let peer = w.lock().unwrap().peers.iter().find(|x| x.1 == sid).cloned();
                        if let Some((slug, session, turn, _)) = peer {
                            let mut started = fixture_body("turn/started");
                            started["session_id"] = json!(session);
                            started["turn_id"] = json!(turn);
                            pushes.push(("turn/started".into(), started));
                            pushes.push((
                                "approval/requested".into(),
                                approval(&session, &turn, &ap(&slug, 1), &format!("cargo test -p {slug}")),
                            ));
                        }
                        json!({ "opened": opened })
                    }
                    "session/list" => json!({ "sessions": [] }),
                    "test/kick" => {
                        pushes.extend(std::mem::take(&mut w.lock().unwrap().kick));
                        json!({})
                    }
                    "session/driver/get" => fixture_body("session/driver/get"),
                    "profile/sub_providers/list" => fixture_body("profile/sub_providers/list"),
                    "session/driver/acquire" => {
                        let mut a = fixture_body("session/driver/acquire");
                        a["binding"]["driver_id"] = p["driver_id"].clone();
                        a
                    }
                    "peer/prepare" => {
                        let slug = slugify(p["title"].as_str().unwrap_or("peer"));
                        let peer = json!({
                            "slug": slug, "topic": format!("peer-{slug}"), "profile_id": "dsflash", "cwd": "<WORKSPACE>",
                            "brief_path": format!("<HOME>/.octos/profiles/dsflash/data/peers/{slug}/brief.md"),
                        });
                        let mut r = peer.clone();
                        r["peers"] = json!([peer]);
                        r
                    }
                    "peer/dispatch" => {
                        let mut wl = w.lock().unwrap();
                        let slug = p["dispatch"]["title"].as_str().unwrap_or("peer").to_owned();
                        let op = p["operation_id"].as_str().unwrap_or("").to_owned();
                        let n = wl.peers.len() + 1;
                        let session = format!("{SESSION}#peer-{slug}");
                        let turn = format!("00000000-0000-4000-8000-{:012x}", 0xd0 + n);
                        wl.peers.push((slug.clone(), session.clone(), turn.clone(), op.clone()));
                        let mut r = fixture_body("peer/dispatch");
                        r["operation_id"] = json!(op);
                        r["model_lane"] = p["model"].clone();
                        r["slug"] = json!(slug);
                        r["adopted_session_id"] = json!(session);
                        r["adopted_turn_id"] = json!(turn);
                        r
                    }
                    "peer/control" => {
                        let mut r = fixture_body("peer/control");
                        r["operation_id"] = p["operation_id"].clone();
                        r["target_operation_id"] = p["target_operation_id"].clone();
                        r["expected_turn_id"] = p["expected_turn_id"].clone();
                        let target = w
                            .lock()
                            .unwrap()
                            .peers
                            .iter()
                            .find(|x| x.3 == p["target_operation_id"].as_str().unwrap_or(""))
                            .cloned();
                        if let Some((slug, session, turn, _)) = target {
                            r["slug"] = json!(slug);
                            r["target_session_id"] = json!(session);
                            match p["command"]["kind"].as_str() {
                                Some("approval_respond") => {
                                    let mut d = fixture_body("approval/decided");
                                    d["session_id"] = json!(session);
                                    d["turn_id"] = json!(turn);
                                    d["approval_id"] = p["command"]["approval_id"].clone();
                                    d["decision"] = p["command"]["decision"].clone();
                                    pushes.push(("approval/decided".into(), d));
                                }
                                Some("interrupt") => {
                                    let mut e = fixture_body("turn/error");
                                    e["session_id"] = json!(session);
                                    e["turn_id"] = json!(turn);
                                    pushes.push(("turn/error".into(), e));
                                }
                                _ => {}
                            }
                        }
                        r
                    }
                    _ => json!({}),
                };
                let frame = json!({"jsonrpc": "2.0", "id": id, "result": reply});
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
                for (m, params) in pushes {
                    let n = json!({"jsonrpc": "2.0", "method": m, "params": params});
                    let _ = tx.send(Message::Text(n.to_string().into())).await;
                }
            }
        });
        Self { base_url: format!("http://{addr}"), seen, world }
    }

    fn sent(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    /// (slug, adopted session, adopted turn, operation id) of a dispatched peer.
    fn peer(&self, slug: &str) -> (String, String, String, String) {
        self.world.lock().unwrap().peers.iter().find(|x| x.0 == slug).cloned().expect("dispatched")
    }
}

async fn connect(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    let conv = Arc::new(conv);
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            peers::note_transport_event(&drv, &evt);
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(None).await.expect("session/open");
    for _ in 0..100 {
        if conv.store.domains.config.supported_methods().iter().any(|m| m == "peer/dispatch") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    conv
}

async fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    for _ in 0..300 {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

/// The board-3 state, the held seat and the dock's state are process
/// statics: one test at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn fresh() {
    b3::reset();
    fleet_driver::reset_seat();
    dock::reset();
    std::env::set_var("OCTOSCODE_DRIVER_ID_PATH", std::env::temp_dir().join("a30-dock-driver-id"));
}

const DESKTOP: Seat = Seat { compact: false, width: 260.0, room: 600.0 };
const PHONE: Seat = Seat { compact: true, width: 292.0, room: 446.0 };

/// Start one peer through the Fleet's production Start (acquire → prepare →
/// ONE dispatch → the background attach of its adopted session).
async fn start_peer(conv: &Conversation, brief: &str) {
    b3::input_changed("fleet.brief", brief);
    b3::perform("b3.fleet.lane", 0, &conv.store);
    let job = match b3::perform("b3.fleet.start", 0, &conv.store) {
        B3Outcome::Spawn(j @ B3Job::FleetStart { .. }) => j,
        other => panic!("expected a Start job, got {other:?}"),
    };
    b3::run(job, conv).await.expect("start");
}

/// Two dispatched peers (`alpha`, `beta`), each blocked on its OWN approval.
async fn two_waiting_peers() -> (Server, Arc<Conversation>) {
    let server = Server::start().await;
    let conv = connect(&server).await;
    let job = match b3::perform("b3.open.fleet", 0, &conv.store) {
        B3Outcome::Spawn(j) => j,
        other => panic!("{other:?}"),
    };
    b3::run(job, &conv).await.expect("the Fleet's opening reads");
    start_peer(&conv, "alpha").await;
    start_peer(&conv, "beta").await;
    let both = || {
        ["alpha", "beta"].iter().all(|s| {
            conv.store
                .domains
                .peer
                .row(&format!("{SESSION}#peer-{s}"))
                .is_some_and(|r| r.request_id.as_deref() == Some(ap(s, 1).as_str()))
        })
    };
    assert!(wait_until(both).await, "both peers wait on their own approval");
    (server, conv)
}

fn job_of(o: Outcome) -> dock::Job {
    match o {
        Outcome::Spawn(j) => j,
        other => panic!("expected a dock job, got {other:?}"),
    }
}

fn slot(key: &str) -> usize {
    dock::slot_of(key).unwrap_or_else(|| panic!("{key} is drawn"))
}

// ---------------------------------------------------------------- tests

/// THE safety test: Approve once on Peer `beta`'s row answers `beta`'s OWN
/// approval — exactly one `peer/control` bound to beta's id, operation and
/// adopted turn — and never `alpha`'s; Deny, Approve for session and Stop
/// on `alpha`'s row each send exactly one frame bound to alpha's ids.
#[tokio::test]
async fn each_dock_action_answers_only_the_row_it_was_drawn_for() {
    let _s = serial();
    fresh();
    let (server, conv) = two_waiting_peers().await;
    let lowered = dock::lower(&conv.store, peers::now_ms(), DESKTOP).expect("the dock shows");
    let (alpha, beta) = (format!("{SESSION}#peer-alpha"), format!("{SESSION}#peer-beta"));
    let (sa, sb) = (slot(&alpha), slot(&beta));
    assert_ne!(sa, sb);
    for (s, who) in [(sa, "alpha"), (sb, "beta")] {
        for a in [dock::ACTION_APPROVE, dock::ACTION_DENY, dock::ACTION_STOP, dock::ACTION_APPROVE_SESSION] {
            assert!(lowered.taps.iter().any(|(_, e)| e == &format!("{a}#{s}")), "{who}: {a} is drawn with its slot");
        }
    }

    // Approve once on beta's row.
    let job = job_of(dock::perform(dock::ACTION_APPROVE, sb, &conv.store, false));
    dock::run(job, &conv).await.expect("approve beta");
    let c = server.sent("peer/control");
    assert_eq!(c.len(), 1, "exactly one frame");
    let (_, _, beta_turn, beta_op) = server.peer("beta");
    assert_eq!(
        c[0]["command"],
        json!({"kind": "approval_respond", "approval_id": ap("beta", 1), "decision": "approve"}),
        "beta's OWN pending id, approve once (no session scope)"
    );
    assert_eq!(c[0]["target_operation_id"], json!(beta_op));
    assert_eq!(c[0]["expected_turn_id"], json!(beta_turn));
    assert!(
        !server.sent("peer/control").iter().any(|f| f["command"]["approval_id"] == json!(ap("alpha", 1))),
        "Approve on peer 2's row never answers peer 1's approval"
    );
    assert!(wait_until(|| conv.store.domains.peer.row(&beta).is_some_and(|r| r.request_id.is_none())).await);
    assert!(conv.store.domains.peer.row(&alpha).unwrap().request_id.as_deref() == Some(ap("alpha", 1).as_str()), "alpha still waits");

    // Deny on alpha's row (re-drawn after beta's decision).
    dock::lower(&conv.store, peers::now_ms(), DESKTOP).unwrap();
    let job = job_of(dock::perform(dock::ACTION_DENY, slot(&alpha), &conv.store, false));
    dock::run(job, &conv).await.expect("deny alpha");
    let c = server.sent("peer/control");
    let (_, _, alpha_turn, alpha_op) = server.peer("alpha");
    assert_eq!(c.len(), 2);
    assert_eq!(c[1]["command"], json!({"kind": "approval_respond", "approval_id": ap("alpha", 1), "decision": "deny"}));
    assert_eq!((c[1]["target_operation_id"].clone(), c[1]["expected_turn_id"].clone()), (json!(alpha_op), json!(alpha_turn)));

    // A new approval on alpha: Approve for session sends the session scope.
    server.world.lock().unwrap().kick =
        vec![("approval/requested".into(), approval(&alpha, &alpha_turn, &ap("alpha", 2), "cargo test -p alpha"))];
    conv.client().request("test/kick", json!({})).await.expect("kick");
    assert!(wait_until(|| conv.store.domains.peer.row(&alpha).is_some_and(|r| r.request_id.as_deref() == Some(ap("alpha", 2).as_str()))).await);
    dock::lower(&conv.store, peers::now_ms(), DESKTOP).unwrap();
    let job = job_of(dock::perform(dock::ACTION_APPROVE_SESSION, slot(&alpha), &conv.store, false));
    dock::run(job, &conv).await.expect("approve alpha for session");
    let c = server.sent("peer/control");
    assert_eq!(c.len(), 3);
    assert_eq!(
        c[2]["command"],
        json!({"kind": "approval_respond", "approval_id": ap("alpha", 2), "decision": "approve", "approval_scope": "session"})
    );

    // A third approval: Stop interrupts alpha's OWN operation + turn.
    server.world.lock().unwrap().kick =
        vec![("approval/requested".into(), approval(&alpha, &alpha_turn, &ap("alpha", 3), "rm -rf target"))];
    conv.client().request("test/kick", json!({})).await.expect("kick");
    assert!(wait_until(|| conv.store.domains.peer.row(&alpha).is_some_and(|r| r.request_id.as_deref() == Some(ap("alpha", 3).as_str()))).await);
    dock::lower(&conv.store, peers::now_ms(), DESKTOP).unwrap();
    let job = job_of(dock::perform(dock::ACTION_STOP, slot(&alpha), &conv.store, false));
    dock::run(job, &conv).await.expect("stop alpha");
    let c = server.sent("peer/control");
    assert_eq!(c.len(), 4, "one frame per action, never retried");
    assert_eq!(c[3]["command"], json!({"kind": "interrupt"}));
    assert_eq!((c[3]["target_operation_id"].clone(), c[3]["expected_turn_id"].clone()), (json!(alpha_op), json!(alpha_turn)));
    assert!(wait_until(|| dock::rows(&conv.store, peers::now_ms()).iter().any(|r| r.key == alpha && r.status == Status::Stopped)).await);
}

/// A STALE approval id sends nothing: the dock drew alpha's approval
/// `ap-alpha-1`, then the server replaced it (`ap-alpha-2`) — the click
/// drawn for the old id is refused at the tap, and a job built before the
/// change is refused before the wire. Zero `peer/control` frames.
#[tokio::test]
async fn a_stale_approval_id_sends_nothing() {
    let _s = serial();
    fresh();
    let (server, conv) = two_waiting_peers().await;
    let alpha = format!("{SESSION}#peer-alpha");
    let (_, _, alpha_turn, _) = server.peer("alpha");
    dock::lower(&conv.store, peers::now_ms(), DESKTOP).expect("the dock shows");
    let drawn = slot(&alpha);
    // A job built from the drawn ids BEFORE the change…
    let early = job_of(dock::perform(dock::ACTION_APPROVE, drawn, &conv.store, false));
    // …then the pending approval is replaced (decided elsewhere + a new one).
    let mut decided = fixture_body("approval/decided");
    decided["session_id"] = json!(alpha);
    decided["turn_id"] = json!(alpha_turn);
    decided["approval_id"] = json!(ap("alpha", 1));
    server.world.lock().unwrap().kick = vec![
        ("approval/decided".into(), decided),
        ("approval/requested".into(), approval(&alpha, &alpha_turn, &ap("alpha", 2), "git push --force")),
    ];
    conv.client().request("test/kick", json!({})).await.expect("kick");
    assert!(wait_until(|| conv.store.domains.peer.row(&alpha).is_some_and(|r| r.request_id.as_deref() == Some(ap("alpha", 2).as_str()))).await);
    // The tap still carrying the OLD slot is refused (no job)…
    assert!(matches!(dock::perform(dock::ACTION_APPROVE, drawn, &conv.store, false), Outcome::Refused(_)));
    assert!(matches!(dock::perform(dock::ACTION_STOP, drawn, &conv.store, false), Outcome::Refused(_)));
    // …and the early job is refused before the wire.
    assert!(dock::run(early, &conv).await.is_err(), "the pending id changed: refused");
    assert!(server.sent("peer/control").is_empty(), "a stale approval id sends NOTHING");
    // Re-drawn, the new approval has a NEW slot that answers ap-alpha-2.
    dock::lower(&conv.store, peers::now_ms(), DESKTOP).unwrap();
    let fresh_slot = slot(&alpha);
    assert_ne!(fresh_slot, drawn, "a new pending id is a new drawn control");
    let job = job_of(dock::perform(dock::ACTION_APPROVE, fresh_slot, &conv.store, false));
    dock::run(job, &conv).await.expect("approve the new one");
    let c = server.sent("peer/control");
    assert_eq!(c.len(), 1);
    assert_eq!(c[0]["command"]["approval_id"], json!(ap("alpha", 2)));
}

/// ⌥Y / ⌥N act on the FOCUSED row only (a row is focused by a tap on it or
/// on one of its controls); with no focused row they send nothing.
#[tokio::test]
async fn alt_y_and_alt_n_act_on_the_focused_row() {
    use makepad_widgets::KeyCode;
    let _s = serial();
    fresh();
    let (server, conv) = two_waiting_peers().await;
    let (alpha, beta) = (format!("{SESSION}#peer-alpha"), format!("{SESSION}#peer-beta"));
    dock::lower(&conv.store, peers::now_ms(), DESKTOP).unwrap();
    assert_eq!(dock::key_action(KeyCode::KeyY, false, true, false), Some(dock::ACTION_APPROVE));
    assert_eq!(dock::key_action(KeyCode::KeyN, false, true, false), Some(dock::ACTION_DENY));
    assert_eq!(dock::key_action(KeyCode::KeyY, false, false, false), None, "Alt is required");
    assert_eq!(dock::key_action(KeyCode::KeyY, true, true, false), None, "Ctrl+Alt stays inert");
    assert_eq!(dock::key_action(KeyCode::KeyY, false, true, true), None, "Cmd+Alt stays inert");
    assert_eq!(dock::focused_slot(), None, "nothing is focused yet: ⌥Y has no target");
    // Focus beta's row (a tap on it), then ⌥N denies beta's approval.
    assert!(matches!(dock::perform(dock::ACTION_FOCUS, slot(&beta), &conv.store, false), Outcome::Done));
    let lowered = dock::lower(&conv.store, peers::now_ms(), DESKTOP).unwrap();
    assert!(lowered.dsl.contains("pd_row_2_ring"), "the focused row (beta, drawn third after the inventory row and alpha) shows its ring");
    let focused = dock::focused_slot().expect("beta is focused");
    assert_eq!(focused, slot(&beta));
    let job = job_of(dock::perform(dock::ACTION_DENY, focused, &conv.store, false));
    dock::run(job, &conv).await.expect("deny beta");
    let c = server.sent("peer/control");
    assert_eq!(c.len(), 1);
    assert_eq!(c[0]["command"], json!({"kind": "approval_respond", "approval_id": ap("beta", 1), "decision": "deny"}));
    assert_eq!(conv.store.domains.peer.row(&alpha).unwrap().request_id.as_deref(), Some(ap("alpha", 1).as_str()));
}

/// The fold: expanded on a desktop, FOLDED on a phone (operator default);
/// ⌥P / the header's "Hide peers" / the pill toggle it; the collapsed dock is
/// ONE pill with the Fleet's words, and the hint names ⌥P.
#[tokio::test]
async fn the_dock_folds_to_one_pill_and_starts_folded_on_a_phone() {
    let _s = serial();
    fresh();
    let (_server, conv) = two_waiting_peers().await;
    assert!(!dock::folded(false), "a desktop starts expanded");
    assert!(dock::folded(true), "a phone starts folded");
    let phone = dock::lower(&conv.store, peers::now_ms(), PHONE).unwrap();
    assert!(phone.folded && phone.dsl.contains("pd_pill") && !phone.dsl.contains("pd_row_0_label"));
    // Inventory row (Requested) + alpha + beta: 3 peers, 2 waiting.
    assert!(phone.dsl.contains("\"3\"") && phone.dsl.contains("2 waiting") && phone.dsl.contains("0/3 finished"), "{}", phone.dsl);
    assert!(phone.dsl.contains("Show peers") && phone.dsl.contains("⌥P"));
    assert!(phone.taps.iter().any(|(_, e)| e == dock::ACTION_FOLD), "the pill expands");
    // ⌥P on the desktop folds; again unfolds.
    assert!(dock::toggle(false), "folded now");
    let folded = dock::lower(&conv.store, peers::now_ms(), DESKTOP).unwrap();
    assert!(folded.folded && folded.dsl.contains("pd_pill"));
    assert!(!dock::toggle(false));
    let open = dock::lower(&conv.store, peers::now_ms(), DESKTOP).unwrap();
    assert!(!open.folded && open.dsl.contains("Hide peers") && open.dsl.contains("PEERS"));
    // The header's Hide peers is the same toggle.
    assert!(open.taps.iter().any(|(id, e)| id == "pd_hide" && e == dock::ACTION_FOLD));
    assert!(matches!(dock::perform(dock::ACTION_FOLD, 0, &conv.store, false), Outcome::Done));
    assert!(dock::folded(false));
}

/// Hidden while there are no peers (the web renders nothing).
#[test]
fn the_dock_is_hidden_with_no_peers() {
    let _s = serial();
    fresh();
    let store = Store::new();
    store.set_active(Some(SESSION.into()));
    assert!(dock::lower(&store, 1_000, DESKTOP).is_none());
    assert!(dock::lower(&store, 1_000, PHONE).is_none());
}

fn row(identity: &str, status: RowStatus) -> PeerRow {
    let mut r = PeerRow::opening(identity, identity.rsplit("peer-").next().unwrap(), Origin::Dispatch, "00000000-0000-4000-8000-0000000000d9", 0);
    r.status = status;
    r.operation_id = Some(format!("op-{identity}"));
    r.model = Some("glm-4.6".into());
    r.accepted_at_ms = Some(0);
    r
}

/// The rows: the Fleet's labels ("Peer N · model", never the slug), the
/// Fleet's status words, the web's elapsed ("4m12s") and "↓ tokens"; the
/// pill is the Fleet-worded `formatPeerDockPill`.
#[test]
fn rows_use_the_fleet_words_the_web_elapsed_and_the_pill() {
    let _s = serial();
    fresh();
    let store = Store::new();
    store.set_active(Some(SESSION.into()));
    let p = &store.domains.peer;
    p.stage_row(row("m#peer-one", RowStatus::Started), false);
    p.stage_row(row("m#peer-two", RowStatus::Started), false);
    p.stage_row(row("m#peer-three", RowStatus::Started), false);
    p.observe_session_event("m#peer-one", &PeerSessionEvent::TurnStarted { turn_id: None }, 1);
    p.observe_session_event("m#peer-one", &PeerSessionEvent::Usage { output_tokens: 12_400 }, 1);
    p.observe_session_event(
        "m#peer-two",
        &PeerSessionEvent::AttentionRequested { request_id: Some("ap-2".into()), kind: Some(RequestKind::Approval), detail: None },
        1,
    );
    p.observe_session_event("m#peer-three", &PeerSessionEvent::Usage { output_tokens: 31_000 }, 1);
    p.observe_session_event("m#peer-three", &PeerSessionEvent::TurnTerminal { outcome: TurnOutcome::Finished, error: None }, 460_000);
    let now = 252_000; // 4m12s after acceptance
    let rows = dock::rows(&store, now);
    assert_eq!(rows.iter().map(|r| r.status).collect::<Vec<_>>(), [Status::Working, Status::WaitingApproval, Status::Finished]);
    assert_eq!(rows[0].label, "Peer 1 · glm-4.6");
    assert_eq!(peers::format_elapsed(rows[0].elapsed_ms), "4m12s");
    assert_eq!(peers::format_tokens(rows[0].tokens), "↓ 12.4k");
    assert_eq!(peers::format_elapsed(rows[2].elapsed_ms), "7m40s", "frozen at the terminal");
    assert_eq!(dock::pill(&rows), "3 · 1 working · ⚠ 1 waiting · 1/3 finished");
    let lowered = dock::lower(&store, now, DESKTOP).unwrap();
    for word in ["Peer 1 · glm-4.6", "Working", "Waiting for your approval", "Finished"] {
        assert!(lowered.dsl.contains(word), "{word}");
    }
    assert!(lowered.texts.iter().any(|(_, t)| t == "4m12s"), "elapsed is set in place: {:?}", lowered.texts);
    assert!(!lowered.dsl.contains("m#peer-"), "never the identity or slug");
    // A row without a REAL pending id grows no card (fail closed).
    assert!(!lowered.dsl.contains("pd_row_1_card"), "no approval detail/id → no actions");
}

/// zh: the dock's copy comes from the web's peer catalog (`peer-copy.ts`)
/// and the Fleet's (`fleet_copy.rs`), through `tr()`.
#[tokio::test]
async fn the_dock_speaks_chinese_from_the_catalogs() {
    let _s = serial();
    fresh();
    let (_server, conv) = two_waiting_peers().await;
    octoscode_module::i18n::set_language(octoscode_module::i18n::Lang::Zh);
    let zh = dock::lower(&conv.store, peers::now_ms(), DESKTOP).unwrap();
    octoscode_module::i18n::set_language(octoscode_module::i18n::Lang::En);
    for want in ["同侪", "隐藏同侪", "等待你的批准", "请求运行", "仅此一次批准", "拒绝", "停止", "本次会话内批准"] {
        assert!(zh.dsl.contains(want), "{want}");
    }
    assert!(!zh.dsl.contains("Hide peers") && !zh.dsl.contains("Approve once"), "no English left");
}

/// Out-of-order peer updates reconcile idempotently (outer/LESSONS.md): a
/// control receipt applied AFTER the turn's terminal never revives an
/// acknowledgment on the finished row, and a resolution for ANOTHER request
/// (a late or duplicate decided) never clears the pending one.
#[test]
fn out_of_order_peer_updates_never_revive_or_clear_the_wrong_request() {
    let p = octoscode_store::domains::peer::Peers::default();
    p.stage_row(row("m#peer-a", RowStatus::Started), false);
    let ev = |e: PeerSessionEvent| p.observe_session_event("m#peer-a", &e, 5);
    ev(PeerSessionEvent::TurnStarted { turn_id: None });
    ev(PeerSessionEvent::AttentionRequested { request_id: Some("ap-1".into()), kind: Some(RequestKind::Approval), detail: None });
    // The decided for ap-1 and a NEW request ap-2 arrive…
    ev(PeerSessionEvent::AttentionResolvedFor { request_id: "ap-1".into() });
    ev(PeerSessionEvent::AttentionRequested { request_id: Some("ap-2".into()), kind: Some(RequestKind::Approval), detail: None });
    // …then a late duplicate decided for ap-1: ap-2 must stay pending.
    ev(PeerSessionEvent::AttentionResolvedFor { request_id: "ap-1".into() });
    let r = p.row("m#peer-a").unwrap();
    assert_eq!((r.activity, r.request_id.as_deref()), (Activity::Blocked, Some("ap-2")), "a resolve for another request is ignored");
    ev(PeerSessionEvent::AttentionResolvedFor { request_id: "ap-2".into() });
    assert_eq!(p.row("m#peer-a").unwrap().activity, Activity::Live, "its own resolve restores the pre-block activity");
    // The terminal lands BEFORE the control's receipt continuation.
    ev(PeerSessionEvent::TurnTerminal { outcome: TurnOutcome::Stopped, error: None });
    ev(PeerSessionEvent::ControlAck { interrupt: true });
    let r = p.row("m#peer-a").unwrap();
    assert_eq!((r.activity, r.outcome, r.acknowledgment), (Activity::Done, Some(TurnOutcome::Stopped), None::<Ack>), "a terminal is final");
}

/// A stale press is SAID once ("Already handled" under the row whose card
/// went away) and the note leaves after its time; ⌥Y on a focused row that
/// was drawn with no pending approval is silent (nothing sent, no note).
#[test]
fn a_stale_refusal_is_said_once_then_leaves_and_a_cardless_chord_is_silent() {
    use octoscode_store::domains::peer::{ApprovalDetail, RequestDetail};
    let _s = serial();
    fresh();
    let store = Store::new();
    store.set_active(Some(SESSION.into()));
    let p = &store.domains.peer;
    p.stage_row(row("m#peer-one", RowStatus::Started), false);
    p.stage_row(row("m#peer-two", RowStatus::Started), false);
    p.observe_session_event("m#peer-one", &PeerSessionEvent::TurnStarted { turn_id: None }, 1);
    p.observe_session_event("m#peer-two", &PeerSessionEvent::TurnStarted { turn_id: None }, 1);
    let detail = RequestDetail::Approval(ApprovalDetail { tool_name: "shell".into(), target: Some("make".into()), ..Default::default() });
    p.observe_session_event(
        "m#peer-one",
        &PeerSessionEvent::AttentionRequested { request_id: Some("ap-1".into()), kind: Some(RequestKind::Approval), detail: Some(detail) },
        2,
    );
    let now = peers::now_ms();
    let lowered = dock::lower(&store, now, DESKTOP).unwrap();
    assert!(lowered.dsl.contains("pd_row_0_card") && lowered.dsl.contains("asks to run shell"));
    let (one, two) = (slot("m#peer-one"), slot("m#peer-two"));
    // The approval is answered elsewhere: the card's controls are stale.
    p.observe_session_event("m#peer-one", &PeerSessionEvent::AttentionResolvedFor { request_id: "ap-1".into() }, 3);
    assert!(matches!(dock::perform(dock::ACTION_APPROVE, one, &store, false), Outcome::Refused(_)));
    let said = dock::lower(&store, now, DESKTOP).unwrap();
    assert!(said.dsl.contains("pd_row_0_note") && said.dsl.contains("Already handled"), "said once");
    assert!(dock::note_showing(now));
    let later = dock::lower(&store, now + 9_000, DESKTOP).unwrap();
    assert!(!later.dsl.contains("pd_row_0_note"), "the note leaves after its time");
    assert!(!dock::note_showing(now + 9_000));
    // ⌥Y on the focused card-less row two: nothing to answer, nothing said.
    assert!(matches!(dock::perform(dock::ACTION_FOCUS, two, &store, false), Outcome::Done));
    assert_eq!(dock::focused_slot(), Some(two));
    assert!(matches!(dock::perform(dock::ACTION_APPROVE, two, &store, false), Outcome::Refused(_)));
    assert!(!dock::lower(&store, now, DESKTOP).unwrap().dsl.contains("pd_row_1_note"), "a card-less chord is silent");
}
