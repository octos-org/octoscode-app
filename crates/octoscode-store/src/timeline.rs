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

/// One row of `SessionHydrateResult.messages`
/// (octos-core `ui_protocol.rs:2865`), reduced to what the rebuild needs —
/// the store stays decoupled from octos-core types.
pub struct HydratedRow<'a> {
    pub seq: u64,
    pub role: &'a str,
    pub content: &'a str,
    /// Owned: the caller derives it from a typed id (`TurnId(pub Uuid)`), so
    /// the string must be materialised somewhere anyway.
    pub turn_id: Option<String>,
    /// Captured reasoning for the row; becomes its own REASONING entry.
    pub reasoning: Option<&'a str>,
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

    /// A6 — [`Timeline::append_delta`] for a TIMED block (reasoning): the
    /// entry records when it was first seen (`data.started_ms`) and the latest
    /// streamed time (`data.ended_ms`), the web's `appendText`
    /// (`timeline/model.ts:669-678`: `startedAtMs: existing?.startedAtMs ??
    /// Date.now()`, `endedAtMs: Date.now()`), so the folded header can say
    /// `12 s · 340 words` (`folds.ts:53-91`). `now_ms` is the caller's clock
    /// (Unix ms) so a replay test is deterministic.
    pub fn append_delta_timed(
        &self,
        session: &str,
        turn_id: Option<&str>,
        kind: EntryKind,
        text: &str,
        now_ms: u64,
    ) -> u64 {
        let id = self.append_delta(session, turn_id, kind, text);
        let mut map = self.inner.lock().unwrap();
        if let Some(e) = map
            .get_mut(session)
            .and_then(|es| es.iter_mut().find(|e| e.id == id))
        {
            // A finalized (hydrated) block keeps its own record untouched.
            if !e.finalized {
                if !e.data.is_object() {
                    e.data = serde_json::json!({});
                }
                if let Some(obj) = e.data.as_object_mut() {
                    obj.entry("started_ms").or_insert(serde_json::json!(now_ms));
                    obj.insert("ended_ms".to_owned(), serde_json::json!(now_ms));
                }
            }
        }
        id
    }

    /// A6 — a system notice with a DETERMINISTIC id (the web's
    /// `addSystemMessage` upsert, `timeline/entry-model.ts:70-78` +
    /// `upsert` `:112-124`): the same `notice_id` updates ONE row in place
    /// instead of appending a duplicate — a replayed terminal, or a turn's
    /// `turn/error` AND its `turn_terminal` (both name `terminal:<turn>`).
    /// The id rides `data.notice_id`. Returns the row's entry id.
    pub fn upsert_notice(
        &self,
        session: &str,
        turn_id: Option<String>,
        notice_id: &str,
        text: String,
        data: serde_json::Value,
    ) -> u64 {
        let mut data = if data.is_object() { data } else { serde_json::json!({}) };
        if let Some(obj) = data.as_object_mut() {
            obj.insert("notice_id".to_owned(), serde_json::json!(notice_id));
        }
        let mut map = self.inner.lock().unwrap();
        let entries = map.entry(session.to_owned()).or_default();
        if let Some(e) = entries.iter_mut().find(|e| {
            e.kind == EntryKind::SYSTEM_NOTICE
                && e.data.get("notice_id").and_then(|v| v.as_str()) == Some(notice_id)
        }) {
            e.text = text;
            e.data = data;
            if turn_id.is_some() {
                e.turn_id = turn_id;
            }
            return e.id;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut e = TimelineEntry::new(id, turn_id, EntryKind::SYSTEM_NOTICE);
        e.text = text;
        e.data = data;
        entries.push(e);
        id
    }

    /// A6 — the next free ordinal notice id `<prefix>:<n>` (the web's
    /// `nextNoticeId`, `entry-model.ts:101-110`): `n` starts at the session's
    /// entry count and skips ids already taken, so two same-millisecond
    /// warnings keep two rows and the id never depends on the wall clock.
    pub fn next_notice_id(&self, session: &str, prefix: &str) -> String {
        let map = self.inner.lock().unwrap();
        let entries = map.get(session).map(Vec::as_slice).unwrap_or(&[]);
        let taken = |id: &str| {
            entries
                .iter()
                .any(|e| e.data.get("notice_id").and_then(|v| v.as_str()) == Some(id))
        };
        let mut ordinal = entries.len();
        loop {
            let id = format!("{prefix}:{ordinal}");
            if !taken(&id) {
                return id;
            }
            ordinal += 1;
        }
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
    /// Card #P4b2 (canonical hydrate recovery): rebuild the transcript from
    /// `SessionHydrateResult.messages` — the web's `restoreCanonicalHydrate`
    /// (`timeline/canonical-hydrate.ts:31`) rules, reduced to what the native
    /// store keeps:
    /// * **event order** — rows land in `seq` order regardless of arrival;
    /// * **durable bodies win, ambiguous identity never deletes** — existing
    ///   entries are never overwritten or removed; a rebuild only appends;
    /// * **idempotent** — each rebuilt row carries `data.hydreate_id`
    ///   `hydrate:seq:<seq>`; re-hydrating the same snapshot adds nothing.
    ///   Identity is `seq` (unique per stream cursor): the web's
    ///   `message_id ?? client_message_id ?? thread:seq` collapses to the
    ///   same uniqueness (recorded deviation);
    /// * rebuilt user/assistant rows are `finalized` — the durable receipt
    ///   absorbs no further `append_delta` text (#21i semantics);
    /// * captured `reasoning` becomes its own REASONING entry immediately
    ///   before the row (the folded-disclosure source).
    /// Returns the number of entries added.
    pub fn fold_hydrated_messages(&self, session: &str, rows: &[HydratedRow]) -> usize {
        let mut ordered: Vec<&HydratedRow> = rows.iter().collect();
        ordered.sort_by_key(|r| r.seq);
        let mut i = self.inner.lock().unwrap();
        let entries = i.entry(session.to_owned()).or_default();
        let mut seen: Vec<String> = entries
            .iter()
            .filter_map(|e| {
                e.data.get("hydrate_id").and_then(|v| v.as_str().map(str::to_owned))
            })
            .collect();
        let mut added = 0usize;
        for row in ordered {
            let hid = format!("hydrate:seq:{}", row.seq);
            let rhid = format!("hydrate:reasoning:seq:{}", row.seq);
            if seen.iter().any(|s| s == &hid) || seen.iter().any(|s| s == &rhid) {
                continue;
            }
            let turn = row.turn_id.clone();
            let mut push = |entries: &mut Vec<TimelineEntry>,
                            turn: Option<String>,
                            kind: EntryKind,
                            text: &str,
                            hid: String,
                            finalized: bool| {
                let id = self.next_id.fetch_add(1, Ordering::Relaxed);
                let mut e = TimelineEntry::new(id, turn, kind);
                e.text = text.to_owned();
                e.data = serde_json::json!({ "hydrate_id": hid });
                e.finalized = finalized;
                entries.push(e);
            };
            if let Some(reasoning) = row.reasoning {
                push(
                    entries,
                    turn.clone(),
                    EntryKind::REASONING,
                    reasoning,
                    rhid.clone(),
                    true,
                );
                seen.push(rhid.clone());
                added += 1;
            }
            let (kind, finalized) = match row.role {
                "user" => (EntryKind::USER_MESSAGE, true),
                "assistant" => (EntryKind::ASSISTANT_TEXT, true),
                "reasoning" => (EntryKind::REASONING, true),
                _ => (EntryKind::SYSTEM_NOTICE, false),
            };
            push(entries, turn, kind, row.content, hid.clone(), finalized);
            seen.push(hid);
            added += 1;
        }
        added
    }

    pub fn of_kind(&self, session: &str, kind: EntryKind) -> Vec<TimelineEntry> {
        self.entries(session).into_iter().filter(|e| e.kind == kind).collect()
    }
}


#[cfg(test)]
mod a6_tests {
    use super::*;

    #[test]
    fn a_notice_id_upserts_one_row_and_ordinals_never_collide() {
        let tl = Timeline::default();
        let a = tl.upsert_notice("s", Some("t1".into()), "terminal:t1", "x".into(), serde_json::json!({"code": "e"}));
        let b = tl.upsert_notice("s", Some("t1".into()), "terminal:t1", "y".into(), serde_json::json!({"code": "e2"}));
        assert_eq!(a, b, "the same id is one row");
        assert_eq!(tl.len("s"), 1);
        assert_eq!(tl.entries("s")[0].text, "y");
        // Same-millisecond warnings keep two rows (web model.test.ts:1611).
        let w1 = tl.next_notice_id("s", "warning");
        tl.upsert_notice("s", None, &w1, "w".into(), serde_json::json!({}));
        let w2 = tl.next_notice_id("s", "warning");
        assert_ne!(w1, w2);
        tl.upsert_notice("s", None, &w2, "w".into(), serde_json::json!({}));
        assert_eq!(tl.len("s"), 3);
        // Deterministic: the id is a function of the transcript, not a clock.
        let other = Timeline::default();
        other.upsert_notice("s", Some("t1".into()), "terminal:t1", "x".into(), serde_json::json!({}));
        assert_eq!(other.next_notice_id("s", "warning"), w1);
    }

    #[test]
    fn a_timed_block_records_first_seen_and_latest_stream_time() {
        let tl = Timeline::default();
        let id = tl.append_delta_timed("s", Some("t"), EntryKind::REASONING, "Weighing ", 1_000);
        tl.append_delta_timed("s", Some("t"), EntryKind::REASONING, "two options", 13_400);
        let e = tl.entries("s").into_iter().find(|e| e.id == id).unwrap();
        assert_eq!(e.text, "Weighing two options");
        assert_eq!(e.data["started_ms"], serde_json::json!(1_000));
        assert_eq!(e.data["ended_ms"], serde_json::json!(13_400));
    }
}

#[cfg(test)]
mod p4b2_tests {
    use super::*;

    fn rows() -> Vec<HydratedRow<'static>> {
        vec![
            HydratedRow {
                seq: 11,
                role: "assistant",
                content: "because five",
                turn_id: Some("t1".to_owned()),
                reasoning: Some("counting to five"),
            },
            HydratedRow {
                seq: 10,
                role: "user",
                content: "why 5?",
                turn_id: Some("t1".to_owned()),
                reasoning: None,
            },
        ]
    }

    #[test]
    fn rebuilds_in_event_order_with_durable_bodies() {
        let tl = Timeline::default();
        let added = tl.fold_hydrated_messages("s1", &rows());
        assert_eq!(added, 3, "reasoning + user + assistant");
        let es = tl.entries("s1");
        let kinds: Vec<EntryKind> = es.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![EntryKind::USER_MESSAGE, EntryKind::REASONING, EntryKind::ASSISTANT_TEXT],
            "seq order (user 10 before assistant 11), reasoning right before its answer"
        );
        assert_eq!(es[0].text, "why 5?");
        assert_eq!(es[2].text, "because five");
        assert!(es[0].finalized && es[2].finalized, "durable receipts absorb no further deltas");
    }

    #[test]
    fn rehydrate_is_idempotent_and_never_deletes() {
        let tl = Timeline::default();
        // A LIVE durable entry the server did not send back: must survive.
        tl.append_delta("s1", Some("t-live"), EntryKind::ASSISTANT_TEXT, "live partial");
        let before = tl.len("s1");
        assert_eq!(tl.fold_hydrated_messages("s1", &rows()), 3);
        // The exact same snapshot again: adds nothing, deletes nothing.
        assert_eq!(tl.fold_hydrated_messages("s1", &rows()), 0, "idempotent");
        let es = tl.entries("s1");
        assert_eq!(es[0].text, "live partial", "the live entry survives, untouched");
        assert_eq!(tl.len("s1"), before + 3);
    }

    #[test]
    fn unknown_roles_lands_as_system_notice() {
        let tl = Timeline::default();
        let rows = vec![HydratedRow {
            seq: 1,
            role: "moderator",
            content: "session pinned",
            turn_id: None,
            reasoning: None,
        }];
        assert_eq!(tl.fold_hydrated_messages("s1", &rows), 1);
        assert_eq!(tl.entries("s1")[0].kind, EntryKind::SYSTEM_NOTICE);
    }
}
