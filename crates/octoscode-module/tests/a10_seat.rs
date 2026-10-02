//! A10 — the peer-control SEAT and the record's control readiness, at the
//! wire (the session pane's Advanced children, the web's
//! `SessionControlBar` inside `SessionConfigPane`'s Advanced:
//! `App.tsx:3341-3349`).
//!
//! Ports `control-readiness.ts:10-26` / `session-record-manager-control-
//! readiness.test.ts` (the fail-closed projection: `peer/control` +
//! `external_driver_v1` + a KNOWN inventory, a COLD internal session
//! included), `peer-control-seat.test.ts` ("seats from a COLD internal
//! inventory once a capability is acquired"), the seat's lifecycle in
//! `use-octos-session.ts:1744-2095` (opt-in Acquire seat → CAS acquire; the
//! 45 s renew cadence; a typed `driver_fence_stale` drops the seat and keeps
//! the §6 label; Release seat parks it; a chat send hands it back first) and
//! `e2e/peer-control.spec.ts` (no seat panel and zero frames when
//! `peer/control` or the feature is unadvertised; ONE `peer/control` per
//! command; an accepted receipt renders its slug and "Newly applied").
//!
//! Every step drives the SAME functions the pane's clicks run
//! (`board3::host::perform` + the job `board3::host::run`), the composer's
//! production submit, and the transport drain `lib.rs` runs. The fake server
//! keeps ONE driver record the way the Core does (CAS acquire, fenced
//! renew / release / control, `turn/start` refused `ExternalMasterHeld` while
//! the session is external); its replies use the faithful fleet fixture's
//! bodies (`a10-fleet-driver-synthetic.jsonl`) where one exists.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::board3::host::{self, Dialog, Job, Outcome};
use octoscode_module::screens::fleet_driver;
use octoscode_module::seat;
use octoscode_store::domains::peer::{Disclosure, FleetInventory};

const ME: &str = "octoscode-native:00000000-0000-4000-8000-0000000000a1";
const TURN: &str = "01a0faec-fc98-73f9-b110-9f404d26812a";
const PENDING: &str = "synthetic-pending-op";

fn fixture(method: &str) -> Value {
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
        .unwrap_or_else(|| panic!("the fleet fixture has no {method}"))
}

// -------------------------------------------------------------- server

#[derive(Clone, Copy, PartialEq, Eq)]
enum Caps {
    Full,
    NoMethod,
    NoFeature,
}

/// The server's ONE driver record + its knobs.
struct Record {
    caps: Caps,
    external: bool,
    driver: String,
    epoch: u64,
    revision: u64,
    lease: u64,
    token: Option<String>,
    /// Revoke the lease (another app acquired) after this many renews.
    revoke_after_renews: Option<usize>,
    renews: usize,
    controls: Vec<String>,
}

impl Record {
    /// A COLD session: internal, never bound (revision 0).
    fn cold(caps: Caps) -> Self {
        Self {
            caps,
            external: false,
            driver: String::new(),
            epoch: 0,
            revision: 0,
            lease: 0,
            token: None,
            revoke_after_renews: None,
            renews: 0,
            controls: Vec::new(),
        }
    }

    fn binding(&self) -> Value {
        json!({"driver_id": self.driver, "epoch": self.epoch, "revision": self.revision, "lease_expires_at_ms": self.lease})
    }

    fn disclosure(&self) -> Value {
        if self.external {
            json!({"mode": "external", "recovery": "none", "binding": self.binding()})
        } else {
            json!({"mode": "internal", "recovery": "none", "binding": null})
        }
    }

    fn fenced(&self, p: &Value) -> bool {
        self.token.as_deref().is_some_and(|t| p["control_token"] == t)
            && p["driver_id"] == self.driver.as_str()
            && p["epoch"].as_u64() == Some(self.epoch)
    }
}

fn refusal(kind: &str) -> Value {
    json!({"code": -32602, "message": "driver operation refused (fixture text never shown)", "data": {"kind": kind}})
}

fn open_reply(caps: Caps, p: &Value) -> Value {
    let mut opened = fixture("session/open");
    let c = &mut opened["capabilities"];
    let methods = c["supported_methods"].as_array_mut().unwrap();
    for m in ["session/driver/renew", "session/driver/release", "session/driver/acquire", "session/driver/get", "session/list"] {
        if !methods.iter().any(|x| x == m) {
            methods.push(json!(m));
        }
    }
    if caps == Caps::NoMethod {
        methods.retain(|x| x != "peer/control");
    }
    if caps == Caps::NoFeature {
        c["supported_features"].as_array_mut().unwrap().retain(|x| x != "external_driver_v1");
    }
    if let Some(s) = p["session_id"].as_str() {
        opened["session_id"] = json!(s);
    }
    json!({ "opened": opened })
}

fn answer(rec: &mut Record, method: &str, p: &Value) -> Result<Value, Value> {
    let now = seat::now_ms();
    match method {
        "session/open" => Ok(open_reply(rec.caps, p)),
        "session/list" => Ok(json!({"sessions": []})),
        "profile/sub_providers/list" => Ok(fixture("profile/sub_providers/list")),
        "session/driver/get" => {
            let mut v = rec.disclosure();
            if p.get("operations").is_some() {
                v["operations"] = json!({
                    "items": [], "snapshot": format!("seat-snapshot-{}", rec.revision),
                    "observed_revision": rec.revision.to_string(), "complete": true, "next_cursor": null,
                });
            }
            Ok(v)
        }
        "session/driver/acquire" => {
            if p["expected_revision"].as_u64() != Some(rec.revision) {
                return Err(refusal("driver_revision_conflict"));
            }
            rec.external = true;
            rec.driver = p["driver_id"].as_str().unwrap_or_default().to_owned();
            rec.epoch += 1;
            rec.revision += 1;
            rec.lease = now + p["lease_seconds"].as_u64().unwrap_or(120) * 1000;
            rec.renews = 0;
            let token = format!("tok-{}", rec.epoch);
            rec.token = Some(token.clone());
            Ok(json!({"control_token": token, "recovery": "none", "binding": rec.binding(), "pending_work": [PENDING]}))
        }
        "session/driver/renew" => {
            if rec.revoke_after_renews.is_some_and(|n| rec.renews >= n) {
                // Another app took the seat: our proof is dead.
                rec.driver = "octos-tui".into();
                rec.epoch += 1;
                rec.revision += 1;
                rec.token = Some("theirs".into());
            }
            if !rec.fenced(p) {
                return Err(refusal("driver_fence_stale"));
            }
            rec.renews += 1;
            rec.lease = now + p["lease_seconds"].as_u64().unwrap_or(120) * 1000;
            Ok(json!({"lease_expires_at_ms": rec.lease}))
        }
        "session/driver/release" => {
            if !rec.fenced(p) || p["expected_revision"].as_u64() != Some(rec.revision) {
                return Err(refusal("driver_fence_stale"));
            }
            rec.revision += 1;
            rec.token = None;
            rec.lease = 0;
            if p["next"] == "internal" {
                rec.external = false;
                Ok(json!({"mode": "internal", "recovery": "none", "binding": null}))
            } else {
                Ok(json!({"mode": "external", "recovery": "none", "binding": rec.binding()}))
            }
        }
        "peer/control" => {
            if !rec.fenced(p) {
                return Err(refusal("driver_fence_stale"));
            }
            let op = p["operation_id"].as_str().unwrap_or_default().to_owned();
            let duplicate = rec.controls.contains(&op);
            rec.controls.push(op);
            let mut r = fixture("peer/control");
            r["operation_id"] = p["operation_id"].clone();
            r["target_operation_id"] = p["target_operation_id"].clone();
            r["expected_turn_id"] = p["expected_turn_id"].clone();
            r["duplicate"] = json!(duplicate);
            Ok(r)
        }
        "turn/start" if rec.external => {
            Err(json!({"code": -32003, "message": "turn admission refused for this session: ExternalMasterHeld"}))
        }
        "turn/start" => Ok(json!({"accepted": true})),
        _ => Ok(json!({})),
    }
}

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    record: Arc<Mutex<Record>>,
}

impl Server {
    async fn start(record: Record) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::new(Mutex::new(record));
        let (seen2, rec2) = (seen.clone(), record.clone());
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
                let (seen, rec) = (seen2.clone(), rec2.clone());
                tokio::spawn(async move {
                    while let Some(Ok(msg)) = source.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        seen.lock().unwrap().push((method.clone(), v["params"].clone()));
                        let frame = match answer(&mut rec.lock().unwrap(), &method, &v["params"]) {
                            Ok(result) => json!({"jsonrpc": "2.0", "id": v["id"], "result": result}),
                            Err(error) => json!({"jsonrpc": "2.0", "id": v["id"], "error": error}),
                        };
                        let _ = tx.send(frame.to_string());
                    }
                });
            }
        });
        Self { base_url, seen, record }
    }

    fn sent(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }

    /// The seat-relevant frames, in wire order.
    fn seat_order(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .map(|(m, _)| m.clone())
            .filter(|m| matches!(m.as_str(), "session/driver/acquire" | "session/driver/release" | "turn/start"))
            .collect()
    }
}

// ------------------------------------------------------------ harness

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    let dir = std::env::temp_dir().join(format!("a10-seat-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    // The Advanced section open (the pane's remembered state) — never the
    // operator's home.
    let adv = dir.join("pane-advanced.json");
    std::fs::write(&adv, "true").unwrap();
    std::env::set_var("OCTOSCODE_PANE_ADVANCED_FILE", &adv);
    std::env::set_var("OCTOSCODE_DRAFTS_FILE", dir.join("drafts.json"));
    std::env::set_var("OCTOSCODE_SHOW_THINKING_FILE", dir.join("show-thinking.json"));
    std::env::set_var("OCTOSCODE_RECENTS_DIR", &dir);
    std::env::set_var("OCTOSCODE_DRIVER_ID", ME);
    std::env::set_var("OCTOSCODE_TURN_START_TIMEOUT_MS", "1500");
    std::env::set_var("OCTOSCODE_SEAT_RENEW_MS", "60000");
    std::env::remove_var("OCTOSCODE_HELD_BY");
    host::reset();
    fleet_driver::reset_seat();
    g
}

async fn connected(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
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
    let session = conv.session_id();
    seat::drop_proof(&session);
    seat::observe(&session, None);
    seat::set_status(&session, None);
    conv
}

fn spawn_of(o: Outcome) -> Job {
    match o {
        Outcome::Spawn(j) => j,
        other => panic!("expected a job, got {other:?}"),
    }
}

/// The strip's click → the pane, its reads run (the A8 entry).
async fn open_pane(conv: &Conversation) {
    let job = spawn_of(host::perform("b3.strip.settings", 0, &conv.store));
    assert_eq!(job, Job::PaneLoad);
    assert_eq!(host::open_dialog(), Some(Dialog::SessionPane));
    host::run(job, conv).await.expect("pane load");
}

fn dsl(conv: &Conversation) -> String {
    host::lower_open(&conv.store).expect("the pane is open").dsl
}

/// ONE click through the host: the tap's job, run to completion.
async fn click(conv: &Conversation, action: &str, index: usize) {
    let job = spawn_of(host::perform(action, index, &conv.store));
    let _ = host::run(job, conv).await;
}

async fn wait_until(mut f: impl FnMut() -> bool, ms: u64) -> bool {
    for _ in 0..(ms / 10) {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    f()
}

fn complete(session: &str, mode: &str, binding: Option<(String, u64, u64, u64)>) -> FleetInventory {
    FleetInventory::Complete {
        session_id: session.to_owned(),
        snapshot: "s-1".into(),
        observed_revision: "3".into(),
        operations: Vec::new(),
        disclosure: Disclosure { mode: mode.into(), recovery: "none".into(), binding },
        completed_at_ms: 1,
    }
}

// --------------------------------------------------------------- tests

/// `deriveControlReadiness`: ready only with `peer/control` AND
/// `external_driver_v1` AND a KNOWN inventory of the active session — any
/// mode, bound or not (a COLD internal session can still seat).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn control_readiness_is_the_fail_closed_projection() {
    let _g = lock();
    // No capabilities yet (nothing opened): unavailable.
    let (idle, _ev) = Conversation::connect("http://127.0.0.1:9", "dummy", "dsflash", None, None).expect("connect");
    assert!(!fleet_driver::control_ready(&idle.store), "unknown caps");

    let full = Server::start(Record::cold(Caps::Full)).await;
    let conv = connected(&full).await;
    let s = conv.session_id();
    let peer = &conv.store.domains.peer;
    assert!(!fleet_driver::control_ready(&conv.store), "no walk yet");
    peer.set_inventory(Some(FleetInventory::Loading));
    assert!(!fleet_driver::control_ready(&conv.store), "a loading walk");
    peer.set_inventory(Some(FleetInventory::Error { session_id: s.clone(), reason: "x".into() }));
    assert!(!fleet_driver::control_ready(&conv.store), "a failed walk");
    peer.set_inventory(Some(complete("another:main", "internal", None)));
    assert!(!fleet_driver::control_ready(&conv.store), "another session's walk");
    peer.set_inventory(Some(complete(&s, "internal", None)));
    assert!(fleet_driver::control_ready(&conv.store), "a COLD internal session is ready");
    peer.set_inventory(Some(complete(&s, "external", None)));
    assert!(fleet_driver::control_ready(&conv.store), "external, no binding");
    peer.set_inventory(Some(complete(&s, "external", Some(("drv-1".into(), 7, 12, 1_700_000_000_000)))));
    assert!(fleet_driver::control_ready(&conv.store), "an observed (retained / parked) binding");

    for caps in [Caps::NoMethod, Caps::NoFeature] {
        let server = Server::start(Record::cold(caps)).await;
        let conv = connected(&server).await;
        let s = conv.session_id();
        conv.store.domains.peer.set_inventory(Some(complete(&s, "internal", None)));
        assert!(!fleet_driver::control_ready(&conv.store), "a missing half of the pair is unavailable");
    }
}

/// e2e `no seat panel when peer/control is unadvertised (no-method)` /
/// `(no-feature)`: the pane's Advanced mounts no seat and no console, the
/// acquire route sends nothing, and no `peer/control` frame ever leaves.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_seat_and_zero_frames_when_peer_control_or_the_feature_is_unadvertised() {
    let _g = lock();
    for caps in [Caps::NoMethod, Caps::NoFeature] {
        host::reset();
        fleet_driver::reset_seat();
        let server = Server::start(Record::cold(caps)).await;
        let conv = connected(&server).await;
        open_pane(&conv).await;
        let d = dsl(&conv);
        assert!(d.contains("Who controls this session"), "the Advanced section is open");
        for id in ["b3_fleet_console", "b3_fleet_seat", "b3_fleet_console_acquire", "b3_fleet_console_release"] {
            assert!(!d.contains(id), "no {id} without the pair");
        }
        assert_eq!(host::perform("b3.fleet.console.acquire", 0, &conv.store), Outcome::Done, "the route is inert");
        assert!(server.sent("session/driver/acquire").is_empty() && server.sent("peer/control").is_empty());
    }
}

/// `peer-control-seat.test.ts` "seats from a COLD internal inventory once a
/// capability is acquired" + the lifecycle: opt-in Acquire seat → ONE CAS
/// acquire at revision 0 under OUR stable driver id → held (Release seat
/// live, the proof shared with the composer, the disclosure re-walked) →
/// renewed on the cadence with the held fence → Release seat → ONE release
/// `next: external` → parked, renewals stop, Acquire seat offered again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cold_session_seats_on_acquire_renews_while_held_and_parks_on_release() {
    let _g = lock();
    std::env::set_var("OCTOSCODE_SEAT_RENEW_MS", "80");
    let server = Server::start(Record::cold(Caps::Full)).await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    open_pane(&conv).await;
    assert!(fleet_driver::control_ready(&conv.store), "caps + a known (cold, internal) inventory");
    let d = dsl(&conv);
    assert!(d.contains("b3_fleet_console_acquire") && d.contains("Acquire seat"), "P2q: the seat is offered, never taken");
    assert!(server.sent("session/driver/acquire").is_empty(), "nothing auto-acquires");
    assert!(!d.contains("b3_fleet_seat_title"), "no seat panel without a held seat");

    let gets = server.sent("session/driver/get").len();
    click(&conv, "b3.fleet.console.acquire", 0).await;
    let acquire = server.sent("session/driver/acquire");
    assert_eq!(acquire.len(), 1, "ONE acquire");
    assert_eq!(
        acquire[0],
        json!({"session_id": session, "driver_id": ME, "expected_revision": 0, "lease_seconds": 120}),
        "CAS on the cold revision 0, our stable id, the 120 s lease"
    );
    assert!(fleet_driver::seat_held(&session));
    assert!(seat::proof(&session).is_some(), "the composer's handover holds the SAME proof");
    assert!(server.sent("session/driver/get").len() > gets, "refreshControlInventory after the acquire");
    let d = dsl(&conv);
    assert!(!d.contains("b3_fleet_console_acquire"), "no Acquire while held");
    assert!(d.contains("A peer you started is using this session"), "the re-walked disclosure names this app (SELF)");
    let observed = host::state().pane.driver.disclosure().and_then(|d| d.binding.clone()).map(|b| b.driver_id);
    assert_eq!(observed.as_deref(), Some(ME), "the pane's disclosure follows the re-walk: our driver");

    // The cadence: the held fence, the same bounded lease.
    assert!(wait_until(|| server.sent("session/driver/renew").len() >= 2, 3000).await, "renewed on the cadence");
    let renew = &server.sent("session/driver/renew")[0];
    assert_eq!(renew["driver_id"], ME);
    assert_eq!(renew["epoch"], 1);
    assert_eq!(renew["control_token"], "tok-1");
    assert_eq!(renew["lease_seconds"], 120);
    assert!(fleet_driver::seat_held(&session), "a renewed seat stays held");

    click(&conv, "b3.fleet.console.release", 0).await;
    let release = server.sent("session/driver/release");
    assert_eq!(release.len(), 1, "ONE release");
    assert_eq!(release[0]["next"], "external", "Release seat PARKS the binding");
    assert_eq!(release[0]["expected_revision"], 1, "the acquire's binding revision");
    assert!(!fleet_driver::seat_held(&session) && fleet_driver::seat_parked(&session));
    assert!(seat::proof(&session).is_none(), "the spent proof is gone from the composer too");
    let renews = server.sent("session/driver/renew").len();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(server.sent("session/driver/renew").len(), renews, "no renew after the release");
    let d = dsl(&conv);
    assert!(d.contains("b3_fleet_console_acquire"), "Acquire seat is offered again");

    // The explicit re-acquire: ONE acquire on the re-walked revision.
    click(&conv, "b3.fleet.console.acquire", 0).await;
    assert_eq!(server.sent("session/driver/acquire").len(), 2);
    assert_eq!(server.sent("session/driver/acquire")[1]["expected_revision"], 2, "the revision the release moved");
    assert!(fleet_driver::seat_held(&session));
}

/// P2e: a typed `driver_fence_stale` (here from a renew, after another app
/// took the seat) drops the seat WITHOUT a frame and keeps the bounded
/// label; the renewals stop; Acquire seat is offered again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stale_renew_drops_the_seat_and_keeps_the_label() {
    let _g = lock();
    std::env::set_var("OCTOSCODE_SEAT_RENEW_MS", "80");
    let server = Server::start(Record { revoke_after_renews: Some(1), ..Record::cold(Caps::Full) }).await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    open_pane(&conv).await;
    click(&conv, "b3.fleet.console.acquire", 0).await;
    assert!(fleet_driver::seat_held(&session));
    assert!(wait_until(|| !fleet_driver::seat_held(&session), 3000).await, "the stale renew drops the seat");
    assert!(fleet_driver::seat_expired(&session));
    assert!(seat::proof(&session).is_none(), "the dead proof is never used for a handover");
    let releases = server.sent("session/driver/release").len();
    assert_eq!(releases, 0, "dropped without a frame");
    let renews = server.sent("session/driver/renew").len();
    assert_eq!(renews, 2, "one good renew, then the stale one");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(server.sent("session/driver/renew").len(), renews, "never retried with the dead fence");
    let d = dsl(&conv);
    assert!(d.contains("Your control of this session expired"), "the §6 label stays");
    assert!(!d.contains("fixture text never shown"), "never the server's copy");
    assert!(d.contains("b3_fleet_console_acquire"), "the console re-offers Acquire");
}

/// `releaseControlSeatForUserTurn`: a chat send while THIS app holds the
/// seat hands it back (`next: internal`, the seat's own proof) BEFORE its
/// ONE `turn/start`; the console then offers Acquire seat (parked).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_chat_send_hands_the_held_seat_back_before_its_one_turn() {
    let _g = lock();
    let server = Server::start(Record::cold(Caps::Full)).await;
    let conv = connected(&server).await;
    let session = conv.session_id();
    open_pane(&conv).await;
    click(&conv, "b3.fleet.console.acquire", 0).await;
    assert!(fleet_driver::seat_held(&session));

    conv.set_draft("hello from the composer");
    let _ = conv.submit_draft().await;
    assert_eq!(
        server.seat_order(),
        ["session/driver/acquire", "session/driver/release", "turn/start"],
        "the handback precedes the one send"
    );
    let release = &server.sent("session/driver/release")[0];
    assert_eq!(release["next"], "internal", "a chat handback, never parked");
    assert_eq!(release["control_token"], "tok-1", "the seat's own proof");
    assert_eq!(release["driver_id"], ME);
    assert_eq!(server.sent("turn/start").len(), 1);
    assert!(!fleet_driver::seat_held(&session) && fleet_driver::seat_parked(&session), "parkControlSeat(record)");
    assert!(seat::proof(&session).is_none());
    assert!(dsl(&conv).contains("b3_fleet_console_acquire"), "Acquire seat is offered again");
}

/// e2e `each of the four commands emits exactly ONE peer/control frame` +
/// `an accepted receipt renders its slug and duplicate:false`: the seat
/// panel mounts on a READY record whose seat this app holds and whose target
/// exists (the acquire's pending work + the master's live turn).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_seat_commands_send_one_frame_each_and_render_the_receipt() {
    let _g = lock();
    let server = Server::start(Record::cold(Caps::Full)).await;
    let conv = connected(&server).await;
    open_pane(&conv).await;
    click(&conv, "b3.fleet.console.acquire", 0).await;
    assert!(!dsl(&conv).contains("b3_fleet_seat_title"), "no live turn: no target, no seat panel");
    host::set_live_turn(Some(TURN.into()));
    let d = dsl(&conv);
    assert!(d.contains("b3_fleet_seat_title") && d.contains("Peer control"), "the seat panel mounts");
    for i in 0..4 {
        let before = server.sent("peer/control").len();
        click(&conv, "b3.fleet.console.seat", i).await;
        assert_eq!(server.sent("peer/control").len(), before + 1, "command {i}: exactly ONE frame");
    }
    let frames = server.sent("peer/control");
    let kinds: Vec<&str> = frames.iter().map(|f| f["command"]["kind"].as_str().unwrap_or("")).collect();
    assert_eq!(kinds, ["approval_respond", "question_respond", "steer", "interrupt"]);
    for f in &frames {
        assert_eq!(f["target_operation_id"], PENDING, "the held acquire's pending work");
        assert_eq!(f["expected_turn_id"], TURN, "the master's live turn");
        assert_eq!(f["driver_id"], ME);
        assert_eq!(f["control_token"], "tok-1");
    }
    let d = dsl(&conv);
    assert!(d.contains("b3_fleet_seat_worker_v") && d.contains("fleet-review"), "the receipt's slug");
    assert!(d.contains("Newly applied"), "duplicate:false");
    assert_eq!(server.record.lock().unwrap().controls.len(), 4, "four distinct control operation ids");
}
