//! `turn` state: which turns are in flight, and the transcript folding.
//!
//! The folding itself lives in [`crate::timeline`] (it is shared shape, not
//! turn state); this domain owns *which* turn is current, so the notification
//! handlers and the module can ask without re-deriving it.
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// The turn domain: the turns this store has seen, per session.
#[derive(Debug, Default)]
pub struct Turns {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// Turns that started and have not completed/errored.
    in_flight: HashSet<String>,
    /// Every turn id seen, in arrival order (for a debug tile or a test).
    seen: Vec<String>,
    /// `turn/steer_dropped`: accepted-but-undrained steer inputs, per turn.
    /// Kept so the composer can hand the text back instead of losing it
    /// (`docs/parity/g-composer.csv` row 53).
    dropped_steers: Vec<DroppedSteer>,
    /// `thread/graph/get`: the last thread graph read, flattened to the
    /// fields the inspection surface shows (thread id, root seq, status and
    /// message count). Overwritten on each read.
    thread_rows: Vec<ThreadRow>,
    /// Card #13: `projection/envelope` folding state. The envelope stream is
    /// the client's canonical order (the web's `durable-session.ts:140-195`
    /// keeps a per-thread sequence and a max cursor), so the domain that owns
    /// the turn owns this too.
    envelopes: EnvelopeFold,
    /// A22 row 236 — every Session's turn terminals in ARRIVAL order, one
    /// entry per turn: the web reads a record's latest real terminal from its
    /// timeline's `terminal:<turn>` entries (`background-session-status.ts:
    /// 16-26`), so later generic activity never changes the answer.
    session_terminals: HashMap<String, Vec<(String, String)>>,
    /// A22 row 236 — the Session a live turn belongs to, when its
    /// `turn/started` named one: a background Session's turn is never the
    /// selected Session's live work. A turn with no recorded owner keeps the
    /// old meaning (any Session).
    owners: HashMap<String, String>,
}

/// `projection/envelope` ordering state (the web's per-thread sequence +
/// canonical cursor; `src-web/apps/web/src/features/session/durable-session.ts:140-195`).
#[derive(Debug, Default)]
pub struct EnvelopeFold {
    /// thread_id -> highest `seq` accepted.
    last_seq: HashMap<String, u64>,
    /// The canonical cursor: `(stream, seq)`, advanced to the max seen
    /// (`durable-session.ts:195`).
    cursor: Option<(String, u64)>,
    /// turn_id -> terminal outcome string (`completed` / `errored` /
    /// `interrupted` / `rate_limited`), set by a `turn_terminal` payload.
    terminals: HashMap<String, String>,
    /// Frames dropped because their `seq` was not strictly greater than the
    /// last accepted one for that thread (`durable-session.ts:185`).
    dropped: Vec<(String, u64)>,
    /// The last `progress/updated` metadata per session (card #13 §3).
    progress: HashMap<String, serde_json::Value>,
}

/// One flattened `ThreadGraphEntry` (`thread/graph/get`, UPCR-2026-010):
/// the thread's id, its root message seq, the open status string, and how
/// many messages the thread owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRow {
    pub thread_id: String,
    pub root_seq: u64,
    pub status: String,
    pub message_count: usize,
}

/// One `turn/steer_dropped` payload: the session/turn it belongs to, the
/// inputs the server could not drain (buffer order preserved), and the reason
/// (`"interrupted"` when the client interrupted the turn, else `"turn_ended"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedSteer {
    pub session_id: String,
    pub turn_id: String,
    pub inputs: Vec<String>,
    pub reason: String,
}

impl Turns {
    pub fn started(&self, turn_id: &str) {
        let mut i = self.inner.lock().unwrap();
        i.in_flight.insert(turn_id.to_owned());
        i.seen.push(turn_id.to_owned());
    }

    /// A22 row 236 — [`Turns::started`] for a turn whose Session is known.
    pub fn started_in(&self, session: &str, turn_id: &str) {
        self.started(turn_id);
        if !session.is_empty() {
            self.inner.lock().unwrap().owners.insert(turn_id.to_owned(), session.to_owned());
        }
    }

    pub fn ended(&self, turn_id: &str) {
        let mut i = self.inner.lock().unwrap();
        i.in_flight.remove(turn_id);
        i.owners.remove(turn_id);
    }

    /// A22 — the Session a live turn was started in (its `turn/started`).
    pub fn owner(&self, turn_id: &str) -> Option<String> {
        self.inner.lock().unwrap().owners.get(turn_id).cloned()
    }

    /// A22 — a live turn `session` owns (`turn/started` named it), if any.
    pub fn in_flight_owned_by(&self, session: &str) -> Option<String> {
        let i = self.inner.lock().unwrap();
        i.in_flight.iter().find(|t| i.owners.get(*t).is_some_and(|s| s == session)).cloned()
    }

    /// A22 row 236 — the live turns of `session`: those it owns, plus any
    /// whose Session was never named (the old global meaning).
    pub fn in_flight_in(&self, session: &str) -> usize {
        let i = self.inner.lock().unwrap();
        i.in_flight.iter().filter(|t| i.owners.get(*t).is_none_or(|s| s == session)).count()
    }

    /// Whether a turn is currently in flight.
    pub fn is_in_flight(&self, turn_id: &str) -> bool {
        self.inner.lock().unwrap().in_flight.contains(turn_id)
    }

    /// How many turns are in flight (across all sessions).
    pub fn in_flight_count(&self) -> usize {
        self.inner.lock().unwrap().in_flight.len()
    }

    /// Every turn id seen, in order.
    pub fn seen(&self) -> Vec<String> {
        self.inner.lock().unwrap().seen.clone()
    }

    /// Record a `turn/steer_dropped` payload. Appends, so a turn that drops
    /// twice (e.g. drained at interrupt, then again at end) keeps both, in
    /// arrival order — the UI restores them in that order.
    pub fn steer_dropped(
        &self,
        session_id: &str,
        turn_id: &str,
        inputs: Vec<String>,
        reason: &str,
    ) {
        self.inner.lock().unwrap().dropped_steers.push(DroppedSteer {
            session_id: session_id.to_owned(),
            turn_id: turn_id.to_owned(),
            inputs,
            reason: reason.to_owned(),
        });
    }

    /// Every dropped-steer payload seen, in arrival order.
    pub fn dropped_steers(&self) -> Vec<DroppedSteer> {
        self.inner.lock().unwrap().dropped_steers.clone()
    }

    /// Replace the thread graph with a fresh `thread/graph/get` read.
    pub fn set_threads(&self, rows: Vec<ThreadRow>) {
        self.inner.lock().unwrap().thread_rows = rows;
    }

    /// The last thread graph read (empty until one lands).
    pub fn threads(&self) -> Vec<ThreadRow> {
        self.inner.lock().unwrap().thread_rows.clone()
    }

    // ---- projection/envelope (card #13) ---------------------------------

    /// Accept an envelope frame for `thread_id` at `seq`. Returns `false` when
    /// it is stale (`seq <=` the last accepted for that thread) — the web
    /// drops those (`durable-session.ts:185`). On acceptance, advances the
    /// per-thread sequence and the canonical cursor to the max.
    pub fn accept_envelope(
        &self,
        thread_id: &str,
        seq: u64,
        cursor: Option<(&str, u64)>,
    ) -> bool {
        let mut i = self.inner.lock().unwrap();
        if let Some(&prev) = i.envelopes.last_seq.get(thread_id) {
            if seq <= prev {
                i.envelopes.dropped.push((thread_id.to_owned(), seq));
                return false;
            }
        }
        i.envelopes.last_seq.insert(thread_id.to_owned(), seq);
        if let Some((stream, cseq)) = cursor {
            let advance = match &i.envelopes.cursor {
                Some((_, cur)) => cseq > *cur,
                None => true,
            };
            if advance {
                i.envelopes.cursor = Some((stream.to_owned(), cseq));
            }
        }
        true
    }

    /// The canonical cursor as `(stream, seq)`.
    pub fn envelope_cursor(&self) -> Option<(String, u64)> {
        self.inner.lock().unwrap().envelopes.cursor.clone()
    }

    /// The last accepted `seq` for a thread.
    pub fn last_envelope_seq(&self, thread_id: &str) -> Option<u64> {
        self.inner.lock().unwrap().envelopes.last_seq.get(thread_id).copied()
    }

    /// Card #26 §2: fold a `session/hydrate` result's continuation checkpoints —
    /// reset the per-thread ordering window to `projection_thread_sequences`.
    ///
    /// The web's `commitHydrate` seeds `#threadSeq` from exactly this map
    /// (`src-web/apps/web/src/features/session/durable-session.ts:101-103`) so a
    /// resumed live stream continues from the authoritative snapshot rather than
    /// re-applying envelopes it already contains. Only the threads the server
    /// named are touched; an absent map leaves the window as-is.
    pub fn fold_hydrate(&self, thread_seqs: &std::collections::BTreeMap<String, u64>) {
        let mut i = self.inner.lock().unwrap();
        for (thread, seq) in thread_seqs {
            i.envelopes.last_seq.insert(thread.clone(), *seq);
        }
    }

    /// #P4g1 row 206: adopt an authoritative hydrate's cursor — the canonical
    /// `(stream, seq)` advances to the hydrate's checkpoint (the web's
    /// `commitHydrate` takes `#cursor = { ...result.cursor }`,
    /// `src-web/apps/web/src/features/session/durable-session.ts:84`). Max
    /// wins, so a late out-of-order hydrate cannot move the cursor backwards.
    pub fn adopt_hydrate_cursor(&self, stream: &str, seq: u64) {
        let mut i = self.inner.lock().unwrap();
        let advance = match &i.envelopes.cursor {
            Some((_, cur)) => seq > *cur,
            None => true,
        };
        if advance {
            i.envelopes.cursor = Some((stream.to_owned(), seq));
        }
    }

    /// Frames dropped for a non-increasing `seq`, in arrival order.
    pub fn dropped_envelopes(&self) -> Vec<(String, u64)> {
        self.inner.lock().unwrap().envelopes.dropped.clone()
    }

    /// Record a `turn_terminal` outcome for `turn_id`.
    pub fn set_terminal(&self, turn_id: &str, outcome: &str) {
        self.inner
            .lock()
            .unwrap()
            .envelopes
            .terminals
            .insert(turn_id.to_owned(), outcome.to_owned());
    }

    /// A22 row 236 — record `turn_id`'s terminal under its Session, in
    /// arrival order (a replay of the same terminal keeps its place and takes
    /// the newer outcome).
    pub fn note_session_terminal(&self, session: &str, turn_id: &str, outcome: &str) {
        let mut i = self.inner.lock().unwrap();
        let log = i.session_terminals.entry(session.to_owned()).or_default();
        match log.iter_mut().find(|(t, _)| t == turn_id) {
            Some(entry) => entry.1 = outcome.to_owned(),
            None => log.push((turn_id.to_owned(), outcome.to_owned())),
        }
    }

    /// A22 row 236 — the Session's latest terminal, whatever its outcome:
    /// `(turn, outcome)` (the selected row's rule, `SessionSidebar.tsx:65-97`).
    pub fn latest_terminal(&self, session: &str) -> Option<(String, String)> {
        self.inner.lock().unwrap().session_terminals.get(session).and_then(|l| l.last().cloned())
    }

    /// A22 row 236 — the Session's latest REAL terminal, a background
    /// record's rule (`backgroundSessionState`, background-session-status.ts:
    /// 16-24): an interrupted or rate-limited turn's terminal is `info`
    /// (timeline/model.ts `settleTimelineTurn`) and is passed over for the
    /// one before it.
    pub fn latest_real_terminal(&self, session: &str) -> Option<(String, String)> {
        self.inner
            .lock()
            .unwrap()
            .session_terminals
            .get(session)
            .and_then(|l| l.iter().rev().find(|(_, o)| o != "interrupted" && o != "rate_limited").cloned())
    }

    /// The recorded terminal outcome for `turn_id`, if any.
    pub fn terminal(&self, turn_id: &str) -> Option<String> {
        self.inner.lock().unwrap().envelopes.terminals.get(turn_id).cloned()
    }

    /// Record the last `progress/updated` metadata for a session (`None`
    /// clears it, e.g. at a turn boundary).
    pub fn set_progress(&self, session: &str, metadata: Option<serde_json::Value>) {
        let mut i = self.inner.lock().unwrap();
        match metadata {
            Some(m) => {
                i.envelopes.progress.insert(session.to_owned(), m);
            }
            None => {
                i.envelopes.progress.remove(session);
            }
        }
    }

    /// The last `progress/updated` metadata for a session, if any.
    pub fn progress(&self, session: &str) -> Option<serde_json::Value> {
        self.inner.lock().unwrap().envelopes.progress.get(session).cloned()
    }
}
