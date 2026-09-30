//! `peer` state: staged/closed peer sessions.
//!
//! Fed by the client domain's `peer/staged` + `peer/closed` handlers
//! (`crates/octoscode-client/src/domains/peer.rs`). Shaped for the peer
//! roster/dock the parity matrix describes: the slug (the peer's primary
//! address), its session topic, the ORIGINATING session, the durable brief
//! path, the fence branch, and whether the model has since closed it.
//!
//! Every field's source is `PeerStagedEvent`
//! (`octos-core crates/octos-core/src/ui_protocol.rs:6482` @ pin `a6ea8505`);
//! `peer/closed` (`:6514`) only flips [`Peer::closed`].
use std::collections::HashMap;
use std::sync::Mutex;

/// One peer, as `peer/staged` / `peer/closed` describe it.
///
/// `name` / `closed` lead deliberately: the roster row reads the address
/// (slug) and the closed flag first, and the open count is `!closed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    /// The directory slug reserved under the profile's `peers/` root — the
    /// peer's address (`PeerStagedEvent.slug`).
    pub name: String,
    /// Set by `peer/closed`; an open peer is not yet closed.
    pub closed: bool,
    /// The peer session's topic (`peer-<slug>`) — a payload field, NOT the
    /// routing key (routing stays on the originating `session_id`).
    pub topic: Option<String>,
    /// Profile the peer session runs under.
    pub profile_id: Option<String>,
    /// The ORIGINATING session — the conversation whose turn staged it.
    pub origin_session_id: Option<String>,
    /// Absolute path of the durable brief (`peers/<slug>/brief.md`).
    pub brief_path: Option<String>,
    /// Working directory of the peer session (worktree checkout when fenced).
    pub cwd: Option<String>,
    /// Fence branch (`peer/<slug>`) when a worktree was created.
    pub worktree_branch: Option<String>,
    /// When the roster first saw the peer (ms epoch) — the fleet row's
    /// elapsed segment (`formatElapsed`, peer-row-view.ts:127; #32c item 11).
    pub staged_at_ms: u64,
}

/// Wall-clock now in ms (falls back to 0 if the clock is before the epoch).
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Peer {
    /// A peer known only by its address — the legacy `stage` shape.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            closed: false,
            topic: None,
            profile_id: None,
            origin_session_id: None,
            brief_path: None,
            cwd: None,
            worktree_branch: None,
            staged_at_ms: now_ms(),
        }
    }
}

/// The peer domain.
#[derive(Debug, Default)]
pub struct Peers {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    peers: HashMap<String, Peer>,
}

impl Peers {
    /// Record a `peer/staged` fact, keyed by the slug.
    ///
    /// A replayed `peer/staged` (the event is durable, so reconnect replay
    /// redelivers it) must NOT reopen a peer the model has already closed:
    /// the existing `closed` flag is preserved across the update.
    pub fn observe_staged(&self, peer: Peer) {
        let mut i = self.inner.lock().unwrap();
        let closed = i.peers.get(&peer.name).map(|p| p.closed).unwrap_or(false);
        i.peers.insert(peer.name.clone(), Peer { closed, ..peer });
    }

    /// Record `peer/closed` for `name`; an unknown slug is recorded closed so
    /// the roster can still drop it.
    pub fn mark_closed(&self, name: &str) {
        let mut i = self.inner.lock().unwrap();
        match i.peers.get_mut(name) {
            Some(p) => p.closed = true,
            None => {
                i.peers
                    .insert(name.to_owned(), Peer { closed: true, ..Peer::named(name) });
            }
        }
    }

    /// One peer by its address.
    pub fn get(&self, name: &str) -> Option<Peer> {
        self.inner.lock().unwrap().peers.get(name).cloned()
    }

    /// Insert or replace the whole row (used by tests and direct callers).
    pub fn upsert(&self, peer: Peer) {
        self.inner.lock().unwrap().peers.insert(peer.name.clone(), peer);
    }

    /// Stage a peer by address alone (the first card's shape; kept).
    pub fn stage(&self, name: String) {
        self.observe_staged(Peer::named(name));
    }

    /// Close a peer by address (the first card's shape; kept).
    pub fn close(&self, name: &str) {
        self.mark_closed(name);
    }

    pub fn list(&self) -> Vec<Peer> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<Peer> = i.peers.values().cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    pub fn open_count(&self) -> usize {
        self.inner.lock().unwrap().peers.values().filter(|p| !p.closed).count()
    }
}
