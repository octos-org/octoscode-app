//! A29 — the `/btw` aside, ONE per Session (parity row 6), ported from the
//! web's `LazyBtwController` (`apps/web/src/features/btw/lazy-btw-controller.ts`)
//! and its per-record pool (`features/session/use-octos-session.ts:1257`,
//! `:2625`, `:3503-3522`).
//!
//! Pure state, no I/O: the flow (`octoscode-module` `flow_btw.rs`) admits an
//! ask here, performs `session/btw` with the [`Ticket`] it got back, and
//! settles the reply here. The rules:
//!
//! * **Ownership.** An aside belongs to the Session that asked: the ticket
//!   captures that Session's id at admission (`lazy-btw-controller.ts:89-95`),
//!   so a reply that arrives after the person switched away lands in the
//!   asking Session only — never in the one on screen.
//! * **Admission is synchronous and typed** (`:85-105`): an empty question,
//!   no live connection, an unadvertised method, or an aside still answering
//!   in that Session are refused before anything is sent, so a refused
//!   command keeps its draft.
//! * **Idempotent settling.** A reply settles only the request that is still
//!   answering under its own id (`#owns`, `:150-155`): a dismissed aside, a
//!   newer ask, or an aside the connection already staled ignore it. A
//!   terminal state is never revived (outer/LESSONS.md: replies and
//!   notifications apply on different tasks).
//! * **Stale.** A connection or authority change while answering fails the
//!   aside at once with the stale copy (the runtime subscription, `:74-78`);
//!   a reply under an older connection epoch settles stale too (`:163-177`).
//! * **Dismiss** removes the aside, answering or not (`:110-115`);
//!   **clear_settled** removes it only when it is no longer answering — the
//!   web calls it once an ordinary prompt is admitted (`:122-126`).
//! * **Collapsed** (board 4 region 5b, the operator's 2026-10-02 decision) is
//!   UI-local and per aside; a new ask starts expanded.
use std::collections::HashMap;
use std::sync::Mutex;

/// Why an aside failed (`FAILED` / `STALE`, `lazy-btw-controller.ts:52-54`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The call failed or its reply was invalid while the Session was still
    /// current: "The aside could not be answered. Try again."
    Failed,
    /// The Session's connection or authority changed before the answer:
    /// "The Session connection changed before the aside completed. Ask again
    /// when it is ready."
    Stale,
}

/// `BtwAsideSnapshot.state` (`lazy-btw-controller.ts:24-31`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsideState {
    Answering,
    Answered { answer: String, model: Option<String> },
    Failed(Failure),
}

/// One Session's aside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aside {
    /// The request this aside shows (`requestId`).
    pub request_id: u64,
    /// The trimmed question.
    pub question: String,
    pub state: AsideState,
    /// Folded to its one-row form.
    pub collapsed: bool,
}

/// What a sidebar row marks for a Session that holds an aside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Answering,
    Answered,
    Failed,
}

impl Aside {
    pub fn mark(&self) -> Mark {
        match self.state {
            AsideState::Answering => Mark::Answering,
            AsideState::Answered { .. } => Mark::Answered,
            AsideState::Failed(_) => Mark::Failed,
        }
    }
}

/// The admitted request: everything the `session/btw` call and its settle
/// need, captured at admission (`Operation`, `lazy-btw-controller.ts:44-49`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    /// The Session that asked — the call's `session_id`, and the only
    /// Session the reply may land in.
    pub session: String,
    pub request_id: u64,
    pub question: String,
    /// The connection epoch the ask was admitted under.
    pub epoch: u64,
}

/// `BtwAdmission` (`lazy-btw-controller.ts:33-34`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    Accepted(Ticket),
    Empty,
    Busy,
    Unavailable,
    Stale,
}

/// The facts the admission gate reads at ask time: the method is advertised
/// (`supportsMethod`, `btw.ts:66`) and the Session's connection is live and
/// healthy (`#current`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gate {
    pub advertised: bool,
    pub connected: bool,
}

/// The `session/btw` outcome the flow observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// A valid reply (`parseSessionBtwResult` admitted it).
    Answer { answer: String, model: Option<String> },
    /// A transport error, a timeout, an RPC error or an invalid reply.
    Error,
}

/// What a settle did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    Answered,
    Failed(Failure),
    /// The reply no longer owns an answering aside (why: `dismissed`,
    /// `superseded`, `settled`) — dropped, never shown.
    Ignored(&'static str),
}

#[derive(Debug, Default)]
struct Inner {
    by_session: HashMap<String, Aside>,
    next_request: u64,
    /// Bumped on every connection or authority change.
    epoch: u64,
}

/// Every Session's aside.
#[derive(Debug, Default)]
pub struct Asides {
    inner: Mutex<Inner>,
}

impl Asides {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `ask` (`lazy-btw-controller.ts:85-105`): admit `question` for
    /// `session`, replacing a settled aside there. Synchronous; nothing is
    /// sent unless this returns [`Admission::Accepted`].
    pub fn ask(&self, session: &str, question: &str, gate: Gate) -> Admission {
        let question = question.trim();
        if question.is_empty() {
            return Admission::Empty;
        }
        if session.trim().is_empty() || !gate.connected {
            return Admission::Stale;
        }
        if !gate.advertised {
            return Admission::Unavailable;
        }
        let mut s = self.lock();
        if s.by_session.get(session).is_some_and(|a| a.state == AsideState::Answering) {
            return Admission::Busy;
        }
        s.next_request += 1;
        let ticket = Ticket {
            session: session.to_owned(),
            request_id: s.next_request,
            question: question.to_owned(),
            epoch: s.epoch,
        };
        s.by_session.insert(
            session.to_owned(),
            Aside {
                request_id: ticket.request_id,
                question: ticket.question.clone(),
                state: AsideState::Answering,
                collapsed: false,
            },
        );
        Admission::Accepted(ticket)
    }

    /// Settle `ticket` with its reply (`#run` / `#fail`,
    /// `lazy-btw-controller.ts:157-208`). Idempotent: only the request that
    /// still answers under its own id settles; `connected` and the epoch are
    /// the `#current` check at reply time.
    pub fn settle(&self, ticket: &Ticket, reply: Reply, connected: bool) -> Settled {
        let mut s = self.lock();
        let epoch = s.epoch;
        let Some(aside) = s.by_session.get_mut(&ticket.session) else {
            return Settled::Ignored("dismissed");
        };
        if aside.request_id != ticket.request_id {
            return Settled::Ignored("superseded");
        }
        if aside.state != AsideState::Answering {
            return Settled::Ignored("settled");
        }
        if epoch != ticket.epoch || !connected {
            aside.state = AsideState::Failed(Failure::Stale);
            return Settled::Failed(Failure::Stale);
        }
        match reply {
            Reply::Answer { answer, model } => {
                aside.state = AsideState::Answered { answer, model };
                Settled::Answered
            }
            Reply::Error => {
                aside.state = AsideState::Failed(Failure::Failed);
                Settled::Failed(Failure::Failed)
            }
        }
    }

    /// The connection or authority changed (a drop, a re-dial): every aside
    /// still answering fails stale at once, and a reply admitted under the
    /// old epoch can no longer answer. Returns how many asides went stale.
    pub fn link_changed(&self) -> usize {
        let mut s = self.lock();
        s.epoch += 1;
        let mut n = 0;
        for aside in s.by_session.values_mut() {
            if aside.state == AsideState::Answering {
                aside.state = AsideState::Failed(Failure::Stale);
                n += 1;
            }
        }
        n
    }

    /// `dismiss` (`lazy-btw-controller.ts:110-115`): the Session's aside goes,
    /// answering or not; its late reply then has nothing to land in.
    pub fn dismiss(&self, session: &str) -> bool {
        self.lock().by_session.remove(session).is_some()
    }

    /// `clearSettled` (`lazy-btw-controller.ts:122-126`): after an ordinary
    /// prompt was admitted in `session`, a settled aside goes; an answering
    /// one stays.
    pub fn clear_settled(&self, session: &str) -> bool {
        let mut s = self.lock();
        if s.by_session.get(session).is_some_and(|a| a.state != AsideState::Answering) {
            s.by_session.remove(session);
            return true;
        }
        false
    }

    /// Fold or unfold the Session's aside; the new `collapsed` value.
    pub fn toggle_collapsed(&self, session: &str) -> Option<bool> {
        let mut s = self.lock();
        let aside = s.by_session.get_mut(session)?;
        aside.collapsed = !aside.collapsed;
        Some(aside.collapsed)
    }

    /// The Session's aside, if it holds one.
    pub fn get(&self, session: &str) -> Option<Aside> {
        self.lock().by_session.get(session).cloned()
    }

    /// The sidebar mark of the Session's aside, if it holds one.
    pub fn mark(&self, session: &str) -> Option<Mark> {
        self.lock().by_session.get(session).map(Aside::mark)
    }

    /// The current connection epoch.
    pub fn epoch(&self) -> u64 {
        self.lock().epoch
    }

    /// How many Sessions hold an aside.
    pub fn count(&self) -> usize {
        self.lock().by_session.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIVE: Gate = Gate { advertised: true, connected: true };

    fn accepted(a: Admission) -> Ticket {
        match a {
            Admission::Accepted(t) => t,
            other => panic!("expected Accepted, got {other:?}"),
        }
    }

    fn answer(text: &str) -> Reply {
        Reply::Answer { answer: text.to_owned(), model: Some("deepseek-v4-flash".to_owned()) }
    }

    #[test]
    fn admission_is_typed_and_synchronous() {
        let a = Asides::default();
        assert_eq!(a.ask("p:x", "   ", LIVE), Admission::Empty);
        assert_eq!(a.ask("", "why?", LIVE), Admission::Stale);
        assert_eq!(a.ask("p:x", "why?", Gate { advertised: true, connected: false }), Admission::Stale);
        assert_eq!(a.ask("p:x", "why?", Gate { advertised: false, connected: true }), Admission::Unavailable);
        assert_eq!(a.count(), 0, "nothing refused leaves state behind");
        let t = accepted(a.ask("p:x", "  why?  ", LIVE));
        assert_eq!((t.session.as_str(), t.question.as_str()), ("p:x", "why?"), "trimmed, owned by the asker");
        assert_eq!(a.ask("p:x", "again?", LIVE), Admission::Busy, "one answering aside per Session");
        // Another Session asks independently.
        let u = accepted(a.ask("p:y", "other?", LIVE));
        assert_ne!(t.request_id, u.request_id);
    }

    #[test]
    fn a_reply_lands_only_in_the_session_that_asked() {
        let a = Asides::default();
        let t = accepted(a.ask("p:x", "why?", LIVE));
        assert_eq!(a.settle(&t, answer("because"), true), Settled::Answered);
        assert_eq!(
            a.get("p:x").map(|x| x.state),
            Some(AsideState::Answered { answer: "because".into(), model: Some("deepseek-v4-flash".into()) })
        );
        assert_eq!(a.get("p:y"), None, "never in another Session");
        assert_eq!(a.mark("p:x"), Some(Mark::Answered));
        assert_eq!(a.mark("p:y"), None);
    }

    #[test]
    fn settling_is_idempotent_against_every_ordering() {
        let a = Asides::default();
        // Dismissed, then the reply: hidden.
        let t = accepted(a.ask("p:x", "q1", LIVE));
        assert!(a.dismiss("p:x"));
        assert_eq!(a.settle(&t, answer("late"), true), Settled::Ignored("dismissed"));
        assert_eq!(a.get("p:x"), None);
        // A newer ask, then the OLD reply: superseded.
        let t1 = accepted(a.ask("p:x", "q1", LIVE));
        assert!(a.dismiss("p:x"));
        let t2 = accepted(a.ask("p:x", "q2", LIVE));
        assert_eq!(a.settle(&t1, answer("old"), true), Settled::Ignored("superseded"));
        assert_eq!(a.get("p:x").unwrap().state, AsideState::Answering);
        // The same reply twice: the second is ignored.
        assert_eq!(a.settle(&t2, answer("new"), true), Settled::Answered);
        assert_eq!(a.settle(&t2, Reply::Error, true), Settled::Ignored("settled"));
        assert!(matches!(a.get("p:x").unwrap().state, AsideState::Answered { .. }));
    }

    #[test]
    fn a_connection_change_stales_answering_asides_and_nothing_revives_them() {
        let a = Asides::default();
        let tx = accepted(a.ask("p:x", "q", LIVE));
        let ty = accepted(a.ask("p:y", "q", LIVE));
        assert_eq!(a.settle(&ty, answer("done"), true), Settled::Answered);
        assert_eq!(a.link_changed(), 1, "only the answering aside goes stale");
        assert_eq!(a.get("p:x").unwrap().state, AsideState::Failed(Failure::Stale));
        assert!(matches!(a.get("p:y").unwrap().state, AsideState::Answered { .. }), "a settled aside stays");
        // The late reply cannot revive the stale aside.
        assert_eq!(a.settle(&tx, answer("late"), true), Settled::Ignored("settled"));
        assert_eq!(a.get("p:x").unwrap().state, AsideState::Failed(Failure::Stale));
        assert_eq!(a.mark("p:x"), Some(Mark::Failed));
    }

    #[test]
    fn a_reply_under_an_older_epoch_or_offline_settles_stale() {
        let a = Asides::default();
        let t = accepted(a.ask("p:x", "q", LIVE));
        // The epoch moved without an answering aside seeing it (e.g. the
        // change was applied on another task between admission and reply).
        {
            let mut s = a.lock();
            s.epoch += 1;
        }
        assert_eq!(a.settle(&t, answer("x"), true), Settled::Failed(Failure::Stale));
        let t = accepted(a.ask("p:z", "q", LIVE));
        assert_eq!(a.settle(&t, answer("x"), false), Settled::Failed(Failure::Stale), "not current: stale");
        let t = accepted(a.ask("p:w", "q", LIVE));
        assert_eq!(a.settle(&t, Reply::Error, true), Settled::Failed(Failure::Failed), "current: failed");
    }

    #[test]
    fn clear_settled_keeps_an_answering_aside_and_dismiss_drops_any() {
        let a = Asides::default();
        let t = accepted(a.ask("p:x", "q", LIVE));
        assert!(!a.clear_settled("p:x"), "answering stays");
        assert_eq!(a.settle(&t, answer("a"), true), Settled::Answered);
        assert!(a.clear_settled("p:x"));
        assert_eq!(a.get("p:x"), None);
        assert!(!a.clear_settled("p:x"));
        let _ = accepted(a.ask("p:x", "q", LIVE));
        assert!(a.dismiss("p:x"), "dismiss hides an answering aside too");
        assert!(!a.dismiss("p:x"));
    }

    #[test]
    fn collapse_is_per_aside_and_a_new_ask_starts_expanded() {
        let a = Asides::default();
        assert_eq!(a.toggle_collapsed("p:x"), None);
        let t = accepted(a.ask("p:x", "q", LIVE));
        assert_eq!(a.toggle_collapsed("p:x"), Some(true));
        assert!(a.get("p:x").unwrap().collapsed);
        assert_eq!(a.settle(&t, answer("a"), true), Settled::Answered);
        assert!(a.get("p:x").unwrap().collapsed, "settling keeps the fold");
        let _ = accepted(a.ask("p:x", "q2", LIVE));
        assert!(!a.get("p:x").unwrap().collapsed);
    }
}
