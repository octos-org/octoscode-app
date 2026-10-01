//! P4d4 — the production-path replay tests for the two control surfaces,
//! mirroring fp4f1's replay server: a fake WS server serves the RECORDED
//! real-gate frames, the screens' functions run through the production
//! `Conversation` client, and the server's received-method log proves the wire
//! traffic (for `peer/control` and the `turn/start` `media` field).
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use octoscode_module::flow::Conversation;
use octoscode_module::screens::media::{
    self, AttachmentDraftStore, AttachmentScope, DraftStatus, LocalFile, TurnMedia,
};
use octoscode_module::screens::peers::{self, PeerActivity, PeerRowState};
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

/// The replay server: canned replies per method, `{}` for the rest, and a log of
/// every method + params the (production) client sent.
struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<(String, Value)>>>,
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
                let params = v.get("params").cloned().unwrap_or(Value::Null);
                rx.lock().unwrap().push((method.clone(), params));
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
        self.received.lock().unwrap().iter().any(|(m, _)| m == method)
    }

    fn params_of(&self, method: &str) -> Value {
        self.received
            .lock()
            .unwrap()
            .iter()
            .find(|(m, _)| m == method)
            .map(|(_, p)| p.clone())
            .unwrap_or(Value::Null)
    }

    fn count(&self, method: &str) -> usize {
        self.received.lock().unwrap().iter().filter(|(m, _)| m == method).count()
    }
}

const R6: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r6-peer-a6ea8505.jsonl"
);
const LIVE_TURN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/live-turn-a6ea8505.jsonl"
);

/// The recorded `turn/start` OUT frame's own prompt text — the exact text the
/// production send path must reproduce.
fn recorded_prompt() -> String {
    load(LIVE_TURN)
        .into_iter()
        .find(|f| f.dir == "out" && f.method == "turn/start")
        .and_then(|f| f.body["input"][0]["text"].as_str().map(str::to_owned))
        .expect("the recording has a turn/start OUT frame with text")
}

/// Open a conversation on a session id UNIQUE to this test, so the process-wide
/// attachment draft authority of one test can never be observed by another.
async fn connect(server: &ReplayServer) -> (Conversation, Arc<Store>) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let session = format!("dsflash:p4d4-{}", SEQ.fetch_add(1, Ordering::SeqCst));
    let (conv, mut _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    conv.open_workspace_as(&session, None).await.expect("session/open");
    let store = conv.store.clone();
    (conv, store)
}

/// The scope for THIS conversation's authority, exactly as `media::current_scope`
/// derives it. Anything else is (correctly) refused as another authority.
fn scope_for(conv: &Conversation) -> AttachmentScope {
    AttachmentScope {
        authority_key: conv.session_id(),
        session_id: conv.session_id(),
        profile_id: conv.profile(),
    }
}

fn file(name: &str, bytes: u64) -> LocalFile {
    LocalFile {
        name: name.to_owned(),
        bytes,
        mime: "image/png".into(),
        content: Arc::new(vec![0u8; bytes as usize]),
    }
}

fn b64url(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(T[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(T[n as usize & 63] as char);
        }
    }
    out
}

/// A valid uploaded handle: `up/<base64url("dsflash/uploads/x.png")>/opaque`.
/// The decoded path is RELATIVE and starts with the Profile — the web refuses
/// an empty component, so a leading "/" is invalid (packages/client/src/media.ts:57-64).
fn handle(name: &str) -> String {
    format!("up/{}/opaque", b64url(format!("dsflash/uploads/{name}").as_bytes()))
}

fn ready_media(name: &str, bytes: u64) -> TurnMedia {
    TurnMedia { path: handle(name), mime: "image/png".into(), size_bytes: bytes }
}

// ------------------------------------------------------------------ media

/// Row 2: the accepted batch rides `turn/start` as `media`, and a text-only
/// turn omits the field ENTIRELY (octos-core `ui_protocol.rs:2044-2045`) — so
/// the text-only path stays byte-identical to the recorded frame.
#[tokio::test]
async fn the_accepted_batch_rides_turn_start_and_an_empty_batch_is_omitted() {
    // 1. a text-only turn: the recorded shape, with NO media field. This runs
    //    against a scope with no draft, which is the whole point: an EMPTY
    //    draft must not block a text-only send.
    let server = ReplayServer::start(vec![]).await;
    let (conv, store) = connect(&server).await;
    media::clear_draft_for_test(&conv);
    media::perform(&conv, "media.submit", &store, Some(&recorded_prompt()))
        .await
        .expect("a text-only send through the production client");
    assert!(server.saw("turn/start"), "the wire carried the turn");
    let sent = server.params_of("turn/start");
    assert_eq!(sent["input"][0]["text"], recorded_prompt(), "the recorded prompt text");
    assert!(
        sent.get("media").is_none(),
        "an empty batch omits media entirely, matching the recorded frame: {sent}"
    );

    // 2. with a ready attachment, the same field IS present and carries the
    //    accepted batch, consumed exactly once.
    let server = ReplayServer::start(vec![]).await;
    let (conv, store) = connect(&server).await;
    let scope = scope_for(&conv);
    let drafts = Arc::new(AttachmentDraftStore::new(scope.clone(), true).expect("scope"));
    drafts.select_files(vec![file("shot.png", 10)]).expect("select");
    let id = drafts.entries()[0].id.clone();
    drafts.begin_upload(&id).expect("upload starts");
    assert!(drafts.complete_upload(&id, &ready_media("shot.png", 10)).expect("receipt"));
    // Put the prepared draft on the conversation's authority so `perform`
    // consumes THIS batch through the accepted boundary.
    media::seed_draft_for_test(&conv, &scope, &drafts);
    media::perform(&conv, "media.submit", &store, Some("with a shot"))
        .await
        .expect("an attachment send through the production client");
    let sent = server.params_of("turn/start");
    let media_field = sent["media"].as_array().expect("media is present");
    assert_eq!(media_field.len(), 1, "the accepted batch is carried");
    assert_eq!(media_field[0]["path"], handle("shot.png"));
    assert_eq!(media_field[0]["mime"], "image/png");
    assert_eq!(media_field[0]["size_bytes"], 10);
    // consumed: a second send carries no media
    media::perform(&conv, "media.submit", &store, Some("second"))
        .await
        .expect("a second send");
    let again = server.params_of("turn/start");
    let last = server.received.lock().unwrap().last().unwrap().1.clone();
    assert!(
        last.get("media").is_none(),
        "the draft is consumed exactly once at the accepted boundary: {last}"
    );
    let _ = again;
}

/// Row 1 through the production surface: an unready entry refuses the turn and
/// NOTHING reaches the wire (the draft is kept for the person to finish).
#[tokio::test]
async fn an_unready_attachment_refuses_the_turn_and_keeps_the_draft() {
    let server = ReplayServer::start(vec![]).await;
    let (conv, store) = connect(&server).await;
    let scope = scope_for(&conv);
    let drafts = Arc::new(AttachmentDraftStore::new(scope.clone(), true).expect("scope"));
    // selected but never uploaded
    drafts.select_files(vec![file("shot.png", 10)]).expect("select");
    media::seed_draft_for_test(&conv, &scope, &drafts);
    let err = media::perform(&conv, "media.submit", &store, Some("too early"))
        .await
        .expect_err("an unready attachment refuses the turn");
    assert_eq!(err, "Upload or remove every selected image before sending this turn.");
    assert!(
        !server.saw("turn/start"),
        "a refused enqueue never issues a turn"
    );
    assert_eq!(drafts.len(), 1, "the draft is kept intact");
    assert_eq!(drafts.entries()[0].status, DraftStatus::Selected);
}

// ------------------------------------------------------------------ peers

/// Rows 4+5: `peer/control` through the production client, addressed to the one
/// row the action names, with the availability gate refusing a row that has no
/// affordance BEFORE the wire.
#[tokio::test]
async fn peer_control_is_addressed_to_the_named_row_through_the_production_client() {
    let server = ReplayServer::start(vec![(
        "peer/control".to_owned(),
        json!({"ok": true, "acknowledged": "steer"}),
    )])
    .await;
    let (conv, store) = connect(&server).await;

    // Stage the recorded peer and give it a live, ADDRESSABLE row.
    let staged = recorded(R6, "peer/prepare");
    let slug = staged["peers"][0]["slug"]
        .as_str()
        .or_else(|| staged["slug"].as_str())
        .unwrap_or("r6-smoke")
        .to_owned();
    let mut peer = octoscode_store::domains::peer::Peer::named(&slug);
    peer.origin_session_id = Some("dsflash:main".into());
    store.domains.peer.observe_staged(peer);
    // The axis is EVENT-driven: a staged row alone is `idle` and has no
    // operation id, so `steer` is correctly refused. Fold a turn-started event
    // (the web's live-run arm) to make the row addressable.
    peers::fold_axis_for_test(&store, &slug, &peers::PeerSessionEvent::TurnStarted {
        turn_id: "01a0f814-5d33-70d5-8121-1da0934df3c5".into(),
    });

    let steer = peers::perform(&conv, "peer.steer", &store, Some(&slug))
        .await
        .expect("steer through the production client");
    assert!(server.saw("peer/control"), "the wire carried the control");
    assert_eq!(steer, "Sent", "the web's control-ack copy");
    let sent = server.params_of("peer/control");
    assert_eq!(sent["slug"], slug, "addressed to the ONE named row");
    assert_eq!(sent["control"], "steer");
    assert_eq!(sent["session_id"], "dsflash:main", "the ORIGINATING session routes");

    // A row that is not addressable has NO affordance, so the control is
    // refused before the wire (fail closed).
    let before = server.count("peer/control");
    let err = peers::perform(&conv, "peer.stop", &store, Some("no-such-peer"))
        .await
        .expect_err("an unknown row is refused");
    assert_eq!(err, "No peer named \"no-such-peer\".");
    assert_eq!(
        server.count("peer/control"),
        before,
        "an unknown row never touches the wire"
    );
}

/// The roster read: the collapsed tallies the ambient dock renders, from the
/// store's real rows.
#[tokio::test]
async fn the_roster_read_reports_the_collapsed_tallies() {
    let server = ReplayServer::start(vec![]).await;
    let (conv, store) = connect(&server).await;
    for (slug, closed) in [("alpha", false), ("beta", false), ("gamma", true)] {
        let mut p = octoscode_store::domains::peer::Peer::named(slug);
        p.closed = closed;
        store.domains.peer.observe_staged(p);
    }
    let summary = peers::perform(&conv, "peer.roster", &store, None)
        .await
        .expect("the roster read");
    // 3 rows, all idle (no axis events have folded), 0 landed.
    assert_eq!(summary, "3 peers — 0 live, 0 blocked, 0 done, 3 idle; 0/3 landed");
    assert!(!server.saw("peer/control"), "a roster read is not a control");
}

/// The axis projection is the store's, so a folded event is visible in the
/// roster's own counts.
#[tokio::test]
async fn a_folded_axis_event_is_visible_in_the_roster_counts() {
    let store = Store::new();
    for slug in ["live-one", "done-one", "blocked-one"] {
        store.domains.peer.observe_staged(octoscode_store::domains::peer::Peer::named(slug));
    }
    // The pure axis fold, over states keyed by the peer's address.
    let mut live = PeerRowState { activity: PeerActivity::Live, ..Default::default() };
    let done = PeerRowState {
        activity: PeerActivity::Done,
        finished_at_ms: Some(5),
        ..Default::default()
    };
    let mut blocked = PeerRowState {
        activity: PeerActivity::Blocked,
        request_kind: Some(peers::AttentionRequestKind::Approval),
        request_id: Some("r1".into()),
        ..Default::default()
    };
    // the freeze: an ack never moves the axis
    live.apply(&peers::PeerSessionEvent::ControlAck, 9);
    assert_eq!(live.activity, PeerActivity::Live);
    blocked.apply(&peers::PeerSessionEvent::AttentionResolved, 10);
    assert_eq!(blocked.activity, PeerActivity::Idle, "resolving returns it to idle");
    let entries = peers::roster(
        &store,
        &[
            ("live-one".into(), live),
            ("done-one".into(), done),
            ("blocked-one".into(), PeerRowState { activity: PeerActivity::Blocked, request_kind: Some(peers::AttentionRequestKind::Approval), request_id: Some("r1".into()), ..Default::default() }),
        ],
    );
    let counts = peers::summarize_roster(&entries);
    assert_eq!((counts.total, counts.live, counts.done, counts.blocked, counts.idle), (3, 1, 1, 1, 0));
    assert_eq!(peers::fleet_landed(&entries), (1, 3));
}
