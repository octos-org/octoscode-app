//! The timeline: one append-only transcript per session.
//!
//! ## Why a `kind` tag and not an enum (the fan-out decision)
//!
//! Eight domain lanes will write timeline entries. The card's goal is that
//! **no lane ever edits a shared file**. Two shapes could do that:
//!
//! 1. A shared `enum EntryKind { … }` with every kind pre-declared now.
//!    Every kind exists from day one, so no lane edits it — *until* the
//!    parity matrix grows a kind the enum does not name, and then a lane must
//!    edit a shared enum after all.
//! 2. A **newtype tag** ([`EntryKind`], a `&'static str`) with the twelve
//!    parity-matrix kinds pre-declared as `const`s here, and room for a domain
//!    to declare *its own* kind in *its own file*:
//!    `pub const MY_KIND: EntryKind = EntryKind::new("my.kind");`
//!
//! We chose (2): the twelve known kinds are declared now (as the card asks),
//! and a new kind is a one-line `const` in the owning domain's file, never an
//! edit here. It is the same guarantee the client's per-domain files give,
//! applied to the transcript.
//!
//! The entry also carries an open `data: serde_json::Value` slot, so a domain
//! can attach structured detail (a tool's args, an approval's id) without
//! changing the entry struct.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// A timeline entry's kind. A stable tag, not an enum — see the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntryKind(&'static str);

impl EntryKind {
    /// Declare a kind. Use a dotted, lowercase tag, e.g. `"assistant.text"`.
    pub const fn new(tag: &'static str) -> Self {
        Self(tag)
    }

    /// The wire-ish tag.
    pub const fn tag(self) -> &'static str {
        self.0
    }

    // --- the twelve kinds the parity matrix names (declared once, now) ------
    /// The person's message.
    pub const USER_MESSAGE: EntryKind = EntryKind::new("user.message");
    /// Streamed assistant text (`message/delta`) — folds into one entry.
    pub const ASSISTANT_TEXT: EntryKind = EntryKind::new("assistant.text");
    /// Assistant reasoning/thinking (`message/reasoning_delta`).
    pub const REASONING: EntryKind = EntryKind::new("assistant.reasoning");
    /// A tool call, rendered as a folded header.
    pub const TOOL_CALL: EntryKind = EntryKind::new("tool.call");
    /// An approval request/decision.
    pub const APPROVAL: EntryKind = EntryKind::new("approval");
    /// A question the server asks the person.
    pub const USER_QUESTION: EntryKind = EntryKind::new("user_question");
    /// Files a turn edited.
    pub const EDITED_FILES: EntryKind = EntryKind::new("edited_files");
    /// A system notice (deterministic id, readable error).
    pub const SYSTEM_NOTICE: EntryKind = EntryKind::new("system.notice");
    /// A plan update.
    pub const PLAN: EntryKind = EntryKind::new("plan");
    /// A goal update.
    pub const GOAL: EntryKind = EntryKind::new("goal");
    /// A reference to a diff (the body is fetched separately).
    pub const DIFF_REF: EntryKind = EntryKind::new("diff.ref");
    /// A delivered file / attachment.
    pub const ATTACHMENT: EntryKind = EntryKind::new("attachment");

    /// Every kind declared here, in the matrix's order — for the exhaustiveness
    /// test that keeps this list honest.
    pub const DECLARED: &'static [EntryKind] = &[
        EntryKind::USER_MESSAGE,
        EntryKind::ASSISTANT_TEXT,
        EntryKind::REASONING,
        EntryKind::TOOL_CALL,
        EntryKind::APPROVAL,
        EntryKind::USER_QUESTION,
        EntryKind::EDITED_FILES,
        EntryKind::SYSTEM_NOTICE,
        EntryKind::PLAN,
        EntryKind::GOAL,
        EntryKind::DIFF_REF,
        EntryKind::ATTACHMENT,
    ];
}

/// One entry in a session's transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    /// Monotonic, per-store id — stable within a run.
    pub id: u64,
    /// The turn this belongs to, when it belongs to one.
    pub turn_id: Option<String>,
    pub kind: EntryKind,
    /// The collectable/rendered text (`""` when the entry is not textual).
    pub text: String,
    /// Optional structured detail; a domain decides its own shape.
    pub data: serde_json::Value,
    /// A folded entry stops absorbing `append_delta` text once closed
    /// (a turn boundary closes the assistant entry it belongs to).
    pub closed: bool,
    /// Card #21i: the entry holds its **canonical** body (a finalized
    /// `assistant_persisted`) rather than a streamed prefix. A finalized entry
    /// absorbs no further `append_delta` text: the web drops later deltas
    /// ("receipt finality wins over delivery order", `timeline/model.ts:655-663`).
    /// Distinct from `closed`: `close_turn` closes without canonicalizing.
    pub finalized: bool,
}

impl TimelineEntry {
    fn new(id: u64, turn_id: Option<String>, kind: EntryKind) -> Self {
        Self {
            id,
            turn_id,
            kind,
            text: String::new(),
            data: serde_json::Value::Null,
            closed: false,
            finalized: false,
        }
    }
}

/// The transcript store: `session id -> entries`, in arrival order.
#[derive(Debug, Default)]
pub struct Timeline {
    inner: Mutex<HashMap<String, Vec<TimelineEntry>>>,
    next_id: AtomicU64,
}

impl Timeline {
    /// Append a new entry of `kind` and return its id.
    pub fn append(&self, session: &str, turn_id: Option<String>, kind: EntryKind, text: String) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut e = TimelineEntry::new(id, turn_id, kind);
        e.text = text;
        self.inner
            .lock()
            .unwrap()
            .entry(session.to_owned())
            .or_default()
            .push(e);
        id
    }

    /// Append with a structured payload.
    pub fn append_data(
        &self,
        session: &str,
        turn_id: Option<String>,
        kind: EntryKind,
        text: String,
        data: serde_json::Value,
    ) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut e = TimelineEntry::new(id, turn_id, kind);
        e.text = text;
        e.data = data;
        self.inner
            .lock()
            .unwrap()
            .entry(session.to_owned())
            .or_default()
            .push(e);
        id
    }

    /// Fold streamed `text` into the last OPEN entry of `(session, turn_id,
    /// kind)`, appending one when there is none. This is how `message/delta`
    /// becomes **one** assistant entry instead of thousands.
    ///
    /// Card #21i: a `finalized` entry is a closed receipt, not an open stream —
    /// once the canonical `assistant_persisted` has landed, later deltas are
    /// **dropped**, not appended (the web: "receipt finality wins over delivery
    /// order", `timeline/model.ts:655-663`). The real server interleaves them
    /// (live `trace.jsonl`: `persisted` at frame 94, 11 more deltas after), and
    /// appending those 45 chars OVER the canonical 239 gave `finalize`'s entry a
    /// tail that only held the post-receipt fragment — what the screen showed.
    pub fn append_delta(&self, session: &str, turn_id: Option<&str>, kind: EntryKind, text: &str) -> u64 {
        let mut map = self.inner.lock().unwrap();
        let entries = map.entry(session.to_owned()).or_default();
        if let Some(last) = entries.iter_mut().rev().find(|e| {
            e.kind == kind && e.turn_id.as_deref() == turn_id && !e.closed && !e.finalized
        }) {
            last.text.push_str(text);
            return last.id;
        }
        // The turn's assistant entry already holds its canonical body? Then a
        // late delta is a duplicate tail — keep the receipt, drop the delta.
        // (`closed` is not consulted: the web drops a delta whenever the turn's
        // entry is complete OR the turn has a terminal, `timeline/model.ts:487-491`
        // + `:655-663`, so a post-terminal delta cannot resurrect a tail row.)
        if let Some(fin) = entries
            .iter()
            .rev()
            .find(|e| e.kind == kind && e.turn_id.as_deref() == turn_id && e.finalized)
        {
            return fin.id;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut e = TimelineEntry::new(id, turn_id.map(str::to_owned), kind);
        e.text = text.to_owned();
        entries.push(e);
        id
    }

    /// Fold a **finalized** assistant message (`assistant_persisted`).
    ///
    /// Card #21i: the web's fold makes the persisted body the segment's
    /// **canonical** content — its `upsert` writes `body: textOf(data)` and
    /// `status: "complete"` (`timeline/model.ts:502-520`), and `appendText`
    /// drops later deltas once the entry is complete (`:655-663`).
    ///
    /// So: put the persisted body INTO the turn's assistant entry (replacing a
    /// streamed prefix, which may be partial or, when the receipt raced ahead of
    /// the stream, only a tail), and mark it `finalized`. `closed` stays false
    /// so a genuinely new segment in the same turn still opens its own entry
    /// (multi-segment turns — exactly what `assistant_segment_id` is for).
    pub fn finalize_assistant(&self, session: &str, turn_id: &str, text: &str) {
        let mut map = self.inner.lock().unwrap();
        let entries = map.entry(session.to_owned()).or_default();
        if let Some(last) = entries
            .iter_mut()
            .rev()
            .find(|e| {
                e.kind == EntryKind::ASSISTANT_TEXT
                    && e.turn_id.as_deref() == Some(turn_id)
                    && !e.closed
            })
        {
            last.text = text.to_owned();
            last.finalized = true;
            return;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut e = TimelineEntry::new(id, Some(turn_id.to_owned()), EntryKind::ASSISTANT_TEXT);
        e.text = text.to_owned();
        e.finalized = true;
        entries.push(e);
    }

    /// Insert (or update in place) a **user message**, ordered BEFORE its
    /// turn's first activity entry.
    ///
    /// The web's `upsertUser`
    /// (`src-web/apps/web/src/features/timeline/model.ts:1019-1043`) exists
    /// because the canonical `user_message` can arrive *after* streaming
    /// replies: the real server sends it at `seq 154`, behind 153 delta frames
    /// (`docs/phase1/live-gate/trace.jsonl`). So arrival order is not display
    /// order. This also dedups: a turn keeps exactly one user row, so an
    /// optimistic row and the persisted copy never both render.
    pub fn upsert_user_message(
        &self,
        session: &str,
        turn_id: &str,
        text: &str,
        data: serde_json::Value,
    ) -> u64 {
        let mut map = self.inner.lock().unwrap();
        let entries = map.entry(session.to_owned()).or_default();

        // Existing user row for this turn? Update it in place (dedup) and reuse
        // its id; else make a fresh row.
        let id = match entries.iter_mut().find(|e| {
            e.kind == EntryKind::USER_MESSAGE && e.turn_id.as_deref() == Some(turn_id)
        }) {
            Some(e) => {
                e.text = text.to_owned();
                e.data = data;
                e.id
            }
            None => {
                let id = self.next_id.fetch_add(1, Ordering::Relaxed);
                let mut e =
                    TimelineEntry::new(id, Some(turn_id.to_owned()), EntryKind::USER_MESSAGE);
                e.text = text.to_owned();
                e.data = data;
                entries.push(e);
                id
            }
        };

        // Move the row so it precedes the turn's first activity entry — but
        // only then (the web: "Only the first observed question may need moving
        // before its replies"). `reasoning`/`assistant`/`tool` are the activity.
        let user_at = entries.iter().position(|e| e.id == id);
        let first_reply = entries.iter().position(|e| {
            e.turn_id.as_deref() == Some(turn_id)
                && matches!(
                    e.kind.tag(),
                    "assistant.reasoning" | "assistant.text" | "tool.call"
                )
        });
        if let (Some(user_at), Some(reply_at)) = (user_at, first_reply) {
            if reply_at < user_at {
                let entry = entries.remove(user_at);
                entries.insert(reply_at, entry);
            }
        }
        id
    }

    /// Close every open entry of a turn (a turn boundary stops folding).
    pub fn close_turn(&self, session: &str, turn_id: &str) {
        let mut map = self.inner.lock().unwrap();
        if let Some(entries) = map.get_mut(session) {
            for e in entries.iter_mut().filter(|e| e.turn_id.as_deref() == Some(turn_id)) {
                e.closed = true;
            }
        }
    }

    /// A copy of a session's entries, in arrival order.
    pub fn entries(&self, session: &str) -> Vec<TimelineEntry> {
        self.inner
            .lock()
            .unwrap()
            .get(session)
            .cloned()
            .unwrap_or_default()
    }

    /// How many entries a session has.
    pub fn len(&self, session: &str) -> usize {
        self.inner.lock().unwrap().get(session).map(Vec::len).unwrap_or(0)
    }

    /// The concatenated assistant text — the live reply.
    pub fn assistant_text(&self, session: &str) -> String {
        self.entries(session)
            .into_iter()
            .filter(|e| e.kind == EntryKind::ASSISTANT_TEXT)
            .map(|e| e.text)
            .collect()
    }

    /// The entries of one kind, in order.
    pub fn of_kind(&self, session: &str, kind: EntryKind) -> Vec<TimelineEntry> {
        self.entries(session).into_iter().filter(|e| e.kind == kind).collect()
    }
}
