//! `peer/*` — sovereign peer sessions.
//!
//! Requests: `peer/prepare`, `peer/gather` (both AppUI extensions —
//! `packages/client/src/peer-protocol.ts:6-7`; `peer/dispatch` /
//! `peer/control` are a later card's external-driver pair, not this lane's).
//! Notifications: `peer/staged`, `peer/closed`
//! (`octos-core crates/octos-core/src/ui_protocol.rs:1329/1334` @ pin
//! `a6ea8505`).
//!
//! ## Wire parity
//! Params match what the web client actually SENDS:
//! - `peer/prepare` — `buildPeerPrepareParams` (`packages/client/src/peer-prepare.ts:8`)
//!   trims `brief` and keeps `n` / `title` / `names` / `cwd` / `worktree`
//!   only when present; `createPeerCommands` (`packages/client/src/peer-commands.ts:44`)
//!   then adds the captured `session_id` + `profile_id`.
//! - `peer/gather` — `packages/client/src/peer-commands.ts:66` sends
//!   `session_id`, `profile_id`, and `slugs` only when the caller filtered.
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::Store;
use serde::{Deserialize, Serialize};

use crate::registry::{NotificationHandler, Registry};
use crate::Method;

/// `peer/prepare` — stage a peer-agent spin-off (write the durable brief,
/// optionally fence a worktree). Server: `raw_peer_prepare`
/// (`crates/octos-cli/src/api/ui_protocol_transport.rs:14626`).
pub struct PeerPrepare;

/// Params exactly as the web sends them (`peer-prepare.ts:8` +
/// `peer-commands.ts:44`). Optionals are skipped when absent so the frame
/// matches the web byte-for-byte.
#[derive(Debug, Default, Clone, Serialize)]
pub struct PeerPrepareParams {
    /// The durable task contract (trimmed by the web before sending).
    pub brief: String,
    /// Fleet size (#1801 v2): stage N peers from ONE brief. Default 1, max 8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    /// Optional human title — seeds the slug.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Required-when-present peer NAMES: one per fleet member (`len == n`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<Vec<String>>,
    /// Create a git worktree (branch `peer/<slug>`) — the blast-radius fence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<bool>,
    /// Explicit workspace override. Defaults to the calling session's root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// The calling session — captured by the web command builder.
    pub session_id: String,
    /// The calling profile — captured by the web command builder.
    pub profile_id: String,
}

/// One staged fleet member — the web's `PeerFleetEntry`
/// (`packages/client/src/peer-protocol.ts:52`), the shape `parsePeerFleetEntry`
/// (`peer-results.ts:15`) validates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerFleetEntry {
    pub slug: String,
    pub topic: String,
    pub profile_id: String,
    pub cwd: String,
    pub brief_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_branch: Option<String>,
}

/// `peer/prepare` result: the scalar fields of the FIRST member plus the whole
/// `peers` fleet (server `raw_peer_prepare` tail; web `PeerPrepareResult`,
/// `peer-protocol.ts:59`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerPrepareResult {
    pub slug: String,
    pub topic: String,
    pub profile_id: String,
    pub cwd: String,
    pub brief_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_branch: Option<String>,
    /// Old scalar-only servers omit `peers`; default keeps them decodable.
    #[serde(default)]
    pub peers: Vec<PeerFleetEntry>,
}

impl Method for PeerPrepare {
    const NAME: &'static str = "peer/prepare";
    type Params = PeerPrepareParams;
    type Result = PeerPrepareResult;
}

/// `peer/gather` — read the profile's peer blackboard (brief + latest result
/// per staged peer). Read-only. Server: `raw_peer_gather`
/// (`crates/octos-cli/src/api/ui_protocol_transport.rs:16988`).
pub struct PeerGather;

/// Params as the web sends them (`peer-commands.ts:66`): the captured
/// `session_id` + `profile_id`, and `slugs` only when the caller filtered.
#[derive(Debug, Default, Clone, Serialize)]
pub struct PeerGatherParams {
    pub session_id: String,
    pub profile_id: String,
    /// Restrict to these slugs; omitted = every staged peer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slugs: Option<Vec<String>>,
}

/// One blackboard row — the web's `PeerGatherEntry`
/// (`packages/client/src/peer-protocol.ts:62`). The server also emits
/// execution-facet fields this lane does not model; serde ignores them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerGatherEntry {
    pub slug: String,
    pub topic: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub brief: String,
    pub brief_truncated: bool,
    #[serde(default)]
    pub result: Option<String>,
    pub result_truncated: bool,
    #[serde(default)]
    pub result_updated_unix: Option<i64>,
    pub has_worktree: bool,
    pub closed: bool,
}

/// `peer/gather` result: `{profile_id, peers[]}` (`TR:17039`; web
/// `PeerGatherResult`, `peer-protocol.ts:76`). Gather is PROFILE-wide — rows
/// assert no originating session or workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerGatherResult {
    pub profile_id: String,
    #[serde(default)]
    pub peers: Vec<PeerGatherEntry>,
}

impl Method for PeerGather {
    const NAME: &'static str = "peer/gather";
    type Params = PeerGatherParams;
    type Result = PeerGatherResult;
}

/// `peer/staged` — the model's `peer_handoff` tool staged a sovereign peer.
/// The client opens the staged session in the background.
pub struct PeerStagedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for PeerStagedHandler {
    const METHOD: &'static str = methods::PEER_STAGED;
    fn handle(&self, notification: &UiNotification) {
        let UiNotification::PeerStaged(event) = notification else {
            return;
        };
        self.store.domains.peer.observe_staged(peer_row_from_staged(event));
        self.store.note_seen(Self::METHOD);
    }
}

/// Project the decoded `peer/staged` event onto the store's roster row.
///
/// `session_id` is the ORIGINATING session (the routing key); `topic`
/// (`peer-<slug>`) is the staged peer's own session topic carried as a payload
/// field — see `PeerStagedEvent` docs (`ui_protocol.rs:6470-6499`).
fn peer_row_from_staged(event: &octos_core::ui_protocol::PeerStagedEvent) -> octoscode_store::domains::peer::Peer {
    octoscode_store::domains::peer::Peer {
        name: event.slug.clone(),
        closed: false,
        topic: Some(event.topic.clone()),
        profile_id: Some(event.profile_id.clone()),
        origin_session_id: Some(event.session_id.0.clone()),
        brief_path: Some(event.brief_path.clone()),
        cwd: Some(event.cwd.clone()),
        worktree_branch: event.worktree_branch.clone(),
        staged_at_ms: octoscode_store::domains::peer::now_ms(),
    }
}

/// `peer/closed` — the model's `peer_close` tool tore down the staged peer.
pub struct PeerClosedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for PeerClosedHandler {
    const METHOD: &'static str = methods::PEER_CLOSED;
    fn handle(&self, notification: &UiNotification) {
        let UiNotification::PeerClosed(event) = notification else {
            return;
        };
        self.store.domains.peer.mark_closed(&event.slug);
        self.store.note_seen(Self::METHOD);
    }
}

pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(PeerStagedHandler { store: store.clone() });
    reg.register(PeerClosedHandler { store });
}

// ---------------------------------------------------------------------------
// Card #13 §3: the external-driver pair (peer/control, peer/dispatch).
//
// These live in `protocol-ext-matrix.csv` (AppUI extensions, `native=absent`
// before this card), and the web issues them through its ONE generic request
// (`packages/client/src/external-driver-peer-control.ts:32-33` pins the wire
// names; the caller/response validation lives at `:565+`). octos-core declares
// NO types for them, and their request/response shapes are large and
// caller-validated on the web side, so we mirror that: the `Params`/`Result`
// stay `serde_json::Value` and the typed validation is the caller's job —
// exactly the transport's `Client::request` contract. That is a real
// production path (not a test-only stub): a caller can now issue either
// method through `Client::call::<PeerDispatch>(...)`.
// ---------------------------------------------------------------------------

/// `peer/dispatch` — hand work to a staged peer (external-driver extension).
pub struct PeerDispatch;

impl Method for PeerDispatch {
    const NAME: &'static str = "peer/dispatch";
    type Params = serde_json::Value;
    type Result = serde_json::Value;
}

/// `peer/control` — steer a running peer (external-driver extension).
pub struct PeerControl;

impl Method for PeerControl {
    const NAME: &'static str = "peer/control";
    type Params = serde_json::Value;
    type Result = serde_json::Value;
}
