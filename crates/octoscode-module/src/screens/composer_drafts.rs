//! A22 row 216 — per-record composer inputs: the thinking effort, the
//! reasoning visibility, the image draft, and the ORDERED restores of what
//! came back to the Session.
//!
//! Web oracle: `features/session/session-composer-drafts.ts:25-206`
//! (`SessionComposerDrafts`, "Browser-only input state. Queued prompts
//! capture it; selection never moves it"):
//! * `get(record)` (`:36-49`) — a record's draft starts ONCE: its effort is
//!   the open reply's `reasoning_effort`, reasoning shown; a later
//!   selection (a re-open) never resets it;
//! * `restoreUnsentTurn` / `restoreInterruptPrompt` (`:121-143`) — what
//!   comes back is PARKED on its OWN record, in order: an unsent turn whole
//!   (text, effort, media), an interrupted prompt as text;
//! * `consumeRestore` (`:148-166`) — the caller has checked the record's
//!   text is empty; a restore NEVER replaces newly selected images; a whole
//!   turn brings back its uploaded media (not uploaded again) and its effort;
//!   another record's restores are never taken;
//! * `retire` / `clear` (`:196-205`) — a retired record's inputs are gone
//!   and a late restore for it is ignored.
//!
//! Native homes for the three inputs (one composer, per-Session state): the
//! effort and the visibility are the store's per-Session thinking prefs
//! (`Sessions::thinking`, the strip / Thinking dialog / `turn/start` read
//! them), the images are the per-Session attachment draft
//! ([`crate::screens::media`]). This module adds the record lifecycle (the
//! once-only seed, retirement per connection) and the restore queue.
//!
//! A record is one Session under one connection (the A8 scope's authority
//! epoch + the Session id): a new connection starts every record afresh,
//! the web's "retired owners cannot enqueue or mutate a new Session's
//! inputs". The text drafts stay `screens::drafts`' (A8, principal-scoped);
//! nothing here persists.
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use octoscode_store::domains::composer::PromptTurn;

use crate::screens::media::{self, TurnMedia};

/// One thing that came back to a record (`ComposerRestore`, `:13`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restore {
    /// An interrupted turn's prompt (`restoreInterruptPrompt`).
    Text(String),
    /// A turn that never started, whole (`restoreUnsentTurn`).
    Turn { text: String, effort: Option<String>, media: Vec<TurnMedia> },
}

impl Restore {
    pub fn text(&self) -> &str {
        match self {
            Restore::Text(t) | Restore::Turn { text: t, .. } => t,
        }
    }
}

#[derive(Debug, Default)]
struct Record {
    restores: VecDeque<Restore>,
    retired: bool,
}

#[derive(Debug, Default)]
struct Records {
    /// The connection the records belong to; a new one retires them all.
    epoch: u64,
    by_session: HashMap<String, Record>,
}

static RECORDS: Mutex<Option<Records>> = Mutex::new(None);

fn with<R>(epoch: u64, f: impl FnOnce(&mut HashMap<String, Record>) -> R) -> R {
    let mut g = RECORDS.lock().unwrap_or_else(|p| p.into_inner());
    let recs = g.get_or_insert_with(Records::default);
    if recs.epoch != epoch {
        // A new connection: every earlier record is retired (`clear`, `:202-205`).
        recs.epoch = epoch;
        recs.by_session.clear();
    }
    f(&mut recs.by_session)
}

fn epoch_of(conv: &crate::flow::Conversation) -> u64 {
    conv.scope().authority_epoch
}

/// `get(record)` on the record's first open (`:36-49`): `true` exactly once
/// per record, when the caller seeds its effort from the open reply. A
/// re-open of a live record (a switch back, a reconnect) answers `false`.
pub fn seed_record(conv: &crate::flow::Conversation, session: &str) -> bool {
    with(epoch_of(conv), |recs| {
        if recs.contains_key(session) {
            false
        } else {
            recs.insert(session.to_owned(), Record::default());
            true
        }
    })
}

/// `restoreUnsentTurn(record, turn)` (`:129-137`): park the whole turn on
/// its own record (a retired record ignores it).
pub fn restore_unsent_turn(conv: &crate::flow::Conversation, session: &str, turn: &PromptTurn) {
    let media: Vec<TurnMedia> = turn
        .media
        .iter()
        .filter_map(|m| {
            Some(TurnMedia {
                path: m.get("path")?.as_str()?.to_owned(),
                mime: m.get("mime")?.as_str()?.to_owned(),
                size_bytes: m.get("size_bytes")?.as_u64()?,
            })
        })
        .collect();
    park(conv, session, Restore::Turn { text: turn.text.clone(), effort: turn.reasoning_effort.clone(), media });
}

/// `restoreInterruptPrompt(record, prompt)` (`:125-128`).
pub fn restore_interrupt_prompt(conv: &crate::flow::Conversation, session: &str, text: &str) {
    if !text.is_empty() {
        park(conv, session, Restore::Text(text.to_owned()));
    }
}

fn park(conv: &crate::flow::Conversation, session: &str, restore: Restore) {
    with(epoch_of(conv), |recs| {
        let rec = recs.entry(session.to_owned()).or_default();
        if rec.retired {
            return;
        }
        rec.restores.push_back(restore);
    });
}

/// `peekRestore(record)` (`:145-147`): the next restore, not drained.
pub fn peek_restore(conv: &crate::flow::Conversation, session: &str) -> Option<Restore> {
    with(epoch_of(conv), |recs| recs.get(session).and_then(|r| r.restores.front().cloned()))
}

/// How many restores wait on the record.
pub fn pending(conv: &crate::flow::Conversation, session: &str) -> usize {
    with(epoch_of(conv), |recs| recs.get(session).map(|r| r.restores.len()).unwrap_or(0))
}

/// `consumeRestore(record)` (`:148-166`) for the Session the composer
/// shows. The caller has checked the composer's text is empty. Returns the
/// text to put back, or `None`: nothing waits, the record is retired, or the
/// person selected new images since (a restore never replaces them). A
/// whole turn brings back its uploaded media (re-adopted, not uploaded
/// again) and its effort.
pub fn consume_restore(conv: &crate::flow::Conversation, session: &str) -> Option<String> {
    if conv.session_id() != session {
        return None; // only the composer's own Session (another record's are never taken)
    }
    let head = peek_restore(conv, session)?;
    let images = media::drafts_for_session(session);
    if images.as_ref().is_some_and(|d| !d.is_empty()) {
        return None;
    }
    if let Restore::Turn { effort, media: m, .. } = &head {
        if !m.is_empty() {
            let drafts = media::drafts_for_conv(conv);
            match drafts.restore_uploaded(m.clone()) {
                Ok(true) => {}
                Ok(false) => return None,
                Err(e) => {
                    // A handle of another Profile is never re-adopted; the
                    // text still comes back.
                    ::log::warn!("octoscode: returned images of {session} not restored: {e}");
                }
            }
        }
        conv.store
            .domains
            .session
            .set_thinking_effort(session, effort.as_deref().filter(|e| crate::screens::board3::thinking::EFFORTS.contains(e)).unwrap_or(""));
    }
    let taken = with(epoch_of(conv), |recs| {
        let rec = recs.get_mut(session)?;
        if rec.retired || rec.restores.front() != Some(&head) {
            return None;
        }
        rec.restores.pop_front()
    })?;
    Some(taken.text().to_owned())
}

/// `retire(record)` (`:196-201`): the record's restores are dropped, a late
/// restore for it is ignored, and its image draft is released.
pub fn retire(conv: &crate::flow::Conversation, session: &str) {
    with(epoch_of(conv), |recs| {
        let rec = recs.entry(session.to_owned()).or_default();
        rec.retired = true;
        rec.restores.clear();
    });
    if let Some(d) = media::drafts_for_session(session) {
        d.invalidate();
    }
}

/// Test seam.
pub fn reset() {
    *RECORDS.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_text_reads_both_kinds() {
        assert_eq!(Restore::Text("a".into()).text(), "a");
        assert_eq!(Restore::Turn { text: "b".into(), effort: None, media: vec![] }.text(), "b");
    }

    #[test]
    fn a_new_connection_retires_every_record() {
        reset();
        let seeded = with(1, |recs| {
            recs.insert("s".into(), Record::default());
            recs.len()
        });
        assert_eq!(seeded, 1);
        assert_eq!(with(2, |recs| recs.len()), 0, "epoch 2 starts afresh");
        reset();
    }
}
