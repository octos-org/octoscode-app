//! octoscode-store — the session store every later lane writes into.
//!
//! **The public API here is ours.** It wraps `octos-app-store`'s session
//! concepts behind octoscode's own types so a fan-out lane can add state
//! without reaching into a dependency's internals.
//!
//! ## Fan-out-safe by construction (card #10)
//!
//! The store was one file with one shared `TimelineEntry` enum; eight domain
//! lanes would all have edited it. It is now **one file per domain**
//! ([`domains`]), exactly like `octoscode-client`. A lane that adds state
//! edits its own domain file and (at most) one field in [`domains::State`].
//!
//! - [`Store`] holds the [`domains::State`] plus the two things no domain owns:
//!   [`connection::ConnectionState`] and [`diagnostics::Diagnostics`].
//! - The transcript lives in [`timeline`]: entries carry an open
//!   [`timeline::EntryKind`] **tag** (not a shared enum) with the parity
//!   matrix's twelve kinds declared, so a domain can declare a new kind in its
//!   own file — see that module's docs for why.
//!
//! Notifications reach this store **only through the client's registry**:
//! no other code path writes to it.
pub mod connection;
pub mod diagnostics;
pub mod domains;
pub mod timeline;

pub use connection::ConnectionState;
pub use diagnostics::Diagnostics;
pub use domains::session::Session;
pub use timeline::{EntryKind, TimelineEntry};

/// The session store. Cheap to share: wrap in an `Arc`.
///
/// Each domain is independently locked, so a lane's hot path (a `message/delta`
/// fold into the transcript) does not serialise against an unrelated read.
#[derive(Debug, Default)]
pub struct Store {
    /// The protocol domains, one struct each.
    pub domains: domains::State,
    /// The transport's connection state.
    pub connection: ConnectionState,
    /// What has been seen, by method.
    pub diagnostics: Diagnostics,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- connection (moved from the old flat API) --------------------------

    /// Record a connection transition (display text + liveness).
    pub fn set_connection(&self, display: String, live: bool) {
        self.connection.set(display, live);
    }

    /// The connection state as display text (e.g. `"Live"`).
    pub fn connection(&self) -> String {
        self.connection.display()
    }

    /// Whether the connection is `Live`.
    pub fn is_live(&self) -> bool {
        self.connection.is_live()
    }

    // ---- capabilities (config domain) -------------------------------------

    pub fn set_capabilities(&self, accepted: Vec<String>) {
        self.domains.config.set_capabilities(accepted);
    }

    pub fn capabilities(&self) -> Vec<String> {
        self.domains.config.capabilities()
    }

    // ---- sessions (session domain) ---------------------------------------

    pub fn set_sessions(&self, sessions: Vec<Session>) {
        self.domains.session.set_list(sessions);
    }

    pub fn sessions(&self) -> Vec<Session> {
        self.domains.session.list()
    }

    /// The session count — what the module tile shows.
    pub fn session_count(&self) -> usize {
        self.domains.session.count()
    }

    pub fn set_active(&self, id: Option<String>) {
        self.domains.session.set_active(id);
    }

    /// The server confirmed a session OPEN — seed it into the list (#34b's
    /// replay half: a static fixture's `session/list` reply never names the
    /// freshly-minted id, so without the seed the active id would dangle).
    /// Web: `session/opened` seeds the tab-known registry
    /// (`known-session-registry.ts`).
    pub fn note_session_opened(&self, id: &str, title: Option<String>) {
        self.domains.session.note_opened(id, title);
    }

    pub fn active_session(&self) -> Option<String> {
        self.domains.session.active()
    }

    // ---- transcript (timeline) -------------------------------------------

    /// The concatenated assistant text for a session — the live reply.
    pub fn live_text(&self, session: &str) -> String {
        self.domains.session.timeline.assistant_text(session)
    }

    // ---- diagnostics ------------------------------------------------------

    /// Record that a notification method was seen (any handler).
    pub fn note_seen(&self, method: &str) {
        self.diagnostics.note(method);
    }

    pub fn seen_count(&self, method: &str) -> usize {
        self.diagnostics.count(method)
    }

    // ---- card #22 §2: the no-silent-drops guard ---------------------------

    /// Card #22 §2: record that `method` reached neither a handler nor an
    /// explicit ignore entry (a silent drop).
    pub fn note_unhandled(&self, method: &str) {
        self.diagnostics.note_unhandled(method);
    }

    /// Card #22 §2: how many times `method` arrived unhandled.
    pub fn unhandled_count(&self, method: &str) -> usize {
        self.diagnostics.unhandled_count(method)
    }

    /// Card #22 §2: every unhandled method, sorted. Empty in a healthy run.
    pub fn unhandled_methods(&self) -> Vec<String> {
        self.diagnostics.unhandled_methods()
    }

    /// Card #22 §2: whether anything arrived unhandled.
    pub fn has_unhandled(&self) -> bool {
        self.diagnostics.has_unhandled()
    }

    /// Card #22 §2: record one `projection/envelope` payload `type` folded.
    pub fn note_payload_type(&self, kind: &str) {
        self.diagnostics.note_payload_type(kind);
    }

    /// Card #22 §2: every `projection/envelope` payload `type` folded, sorted.
    pub fn payload_types(&self) -> Vec<String> {
        self.diagnostics.payload_types()
    }

    /// A one-line summary for a tile: connection + session count.
    pub fn summary(&self) -> String {
        format!(
            "conn: {}   sessions: {}",
            self.connection(),
            self.session_count()
        )
    }
}
