//! A7 — the composer's turn admission state, per Session: the prompt queue
//! and the turn controller's bookkeeping, ported from the web oracle
//! (`apps/web/src/features/composer/turn-queue.ts:51-162` and
//! `use-turn-controller.ts:206-1453`).
//!
//! Pure state, no I/O: every transition here is a synchronous decision the
//! flow (`octoscode-module` `flow.rs`) performs and then executes on the wire
//! (`turn/start`, `turn/steer`, `turn/state/get`). The rules it keeps:
//!
//! * **FIFO** — one active turn per Session; later prompts queue and each
//!   becomes its own turn when the active one settles (`turn-queue.ts:51-60`).
//!   A queued prompt can be removed without interrupting server work.
//! * **Attempted ids are never replayed** (`use-turn-controller.ts:240-242`):
//!   a start whose outcome is unknown keeps its id and is never resent.
//! * **Unknown outcome** — a start that timed out or lost its transport is
//!   held (not rejected) until lifecycle evidence arrives: a terminal, server
//!   activity for the turn, or a `turn/state/get` (`:399-500`, `:1035-1060`).
//! * **Collisions** — a refused start that names the occupying turn settles
//!   OUR turn, adopts the occupier as the observed active turn (so pending
//!   prompts wait behind it) and hands our text back (`:431-468`).
//! * **Steering** — with steering enabled, a submission while an accepted
//!   turn runs steers into it instead of racing a second start; a refused
//!   steer is restored at the queue's front, in order (`:681-857`).
//! * **Ownership** — `dispatching` (our start is in flight), `local-owner`
//!   (our start was accepted), `observed` (a turn we did not start, e.g. one
//!   a hydrate or another client reported) (`:1310-1317`), each lease bound
//!   to the transport generation it was made under.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;

use serde_json::Value;

/// Who started a turn (`turn-queue.ts:8-18`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Origin {
    /// Written in this composer.
    #[default]
    Local,
    /// Reported by the server (a hydrate, another client's `turn/started`, a
    /// collision): watched and stoppable, never presented as ours.
    Adopted,
}

/// One admitted prompt (`PromptTurn`, `turn-queue.ts:3-27`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PromptTurn {
    pub turn_id: String,
    pub text: String,
    pub origin: Origin,
    /// The Session's reasoning selection captured AT ENQUEUE time.
    pub reasoning_effort: Option<String>,
    /// Uploaded handles owned by this turn (the `turn/start` `media` array).
    pub media: Vec<Value>,
}

impl PromptTurn {
    pub fn local(turn_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self { turn_id: turn_id.into(), text: text.into(), ..Default::default() }
    }

    pub fn adopted(turn_id: impl Into<String>) -> Self {
        Self { turn_id: turn_id.into(), origin: Origin::Adopted, ..Default::default() }
    }
}

/// A user-interrupted prompt held until its OWN turn's terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptRestore {
    pub session_id: String,
    pub turn_id: String,
    pub prompt: String,
}

/// `QueueTransition` (`turn-queue.ts:34-37`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueueTransition {
    pub settled: bool,
    pub next: Option<PromptTurn>,
}

/// `PromptTurnQueueSnapshot`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueueSnapshot {
    pub active: Option<PromptTurn>,
    pub pending: Vec<PromptTurn>,
}

/// `PromptTurnQueue` (`turn-queue.ts:51-153`), verbatim.
#[derive(Debug, Clone, Default)]
pub struct PromptTurnQueue {
    pending: VecDeque<PromptTurn>,
    interrupt_restores: Vec<InterruptRestore>,
    active: Option<PromptTurn>,
}

impl PromptTurnQueue {
    /// The first prompt starts now; later prompts queue.
    pub fn enqueue(&mut self, turn: PromptTurn) -> bool {
        if self.active.is_none() {
            self.active = Some(turn);
            return true;
        }
        self.pending.push_back(turn);
        false
    }

    /// Returned/rejected steering preceded later ordinary drafts.
    pub fn prepend_pending(&mut self, turns: Vec<PromptTurn>) {
        for t in turns.into_iter().rev() {
            self.pending.push_front(t);
        }
    }

    pub fn restore_active(&mut self, turn: PromptTurn, preserve_unsent_active: bool) -> bool {
        if let Some(active) = &self.active {
            if active.turn_id == turn.turn_id {
                return true;
            }
            if !preserve_unsent_active {
                return false;
            }
            let prev = self.active.take().expect("checked");
            self.pending.push_front(prev);
        }
        self.active = Some(turn);
        true
    }

    pub fn settle(&mut self, turn_id: &str) -> QueueTransition {
        if self.active.as_ref().map(|a| a.turn_id.as_str()) != Some(turn_id) {
            return QueueTransition::default();
        }
        self.active = self.pending.pop_front();
        QueueTransition { settled: true, next: self.active.clone() }
    }

    pub fn remove_pending(&mut self, turn_id: &str) -> bool {
        match self.pending.iter().position(|t| t.turn_id == turn_id) {
            Some(i) => {
                self.pending.remove(i);
                true
            }
            None => false,
        }
    }

    pub fn clear(&mut self) {
        self.active = None;
        self.pending.clear();
        self.interrupt_restores.clear();
    }

    /// One re-armable entry per session; empty prompts are ignored.
    pub fn stash_interrupt_prompt(&mut self, session_id: &str, turn_id: &str, prompt: &str) {
        if prompt.trim().is_empty() {
            return;
        }
        let entry = InterruptRestore {
            session_id: session_id.to_owned(),
            turn_id: turn_id.to_owned(),
            prompt: prompt.to_owned(),
        };
        match self.interrupt_restores.iter().position(|p| p.session_id == session_id) {
            Some(i) => self.interrupt_restores[i] = entry,
            None => self.interrupt_restores.push(entry),
        }
    }

    /// The restore armed for exactly this turn.
    pub fn take_interrupt_prompt(&mut self, turn_id: &str) -> Option<InterruptRestore> {
        let i = self.interrupt_restores.iter().position(|p| p.turn_id == turn_id)?;
        Some(self.interrupt_restores.remove(i))
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot { active: self.active.clone(), pending: self.pending.iter().cloned().collect() }
    }
}

/// `TurnRecoveryState.phase` (`use-turn-controller.ts:117-121`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryPhase {
    /// A `turn/state/get` is in flight.
    Checking,
    /// The server answered `unknown`.
    Unknown,
    /// The server does not advertise `turn/state/get`.
    Unavailable,
    /// The status check failed (its message).
    Error(String),
}

/// The active turn whose outcome is held for recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovery {
    pub turn_id: String,
    pub phase: RecoveryPhase,
}

/// The accepted owner's lifecycle (`acceptedOwner.state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerState {
    Running,
    Waiting,
    Completed,
    Failed,
}

/// `activeTurnOwnership` (`use-turn-controller.ts:1310-1317`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    None,
    /// Our `turn/start` is in flight.
    Dispatching,
    /// Our start was accepted on this transport.
    LocalOwner,
    /// A turn is active that this transport did not (provably) start.
    Observed,
}

/// The server lifecycle states (`TurnLifecycleState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Active,
    Interrupting,
    Completed,
    Errored,
    Interrupted,
    Unknown,
}

impl Lifecycle {
    pub fn parse(s: &str) -> Option<Lifecycle> {
        Some(match s {
            "active" => Lifecycle::Active,
            "interrupting" => Lifecycle::Interrupting,
            "completed" => Lifecycle::Completed,
            "errored" => Lifecycle::Errored,
            "interrupted" => Lifecycle::Interrupted,
            "unknown" => Lifecycle::Unknown,
            _ => return None,
        })
    }

    fn live(self) -> bool {
        matches!(self, Lifecycle::Active | Lifecycle::Interrupting)
    }
}

/// A system row the controller wants in the transcript (the web's
/// `addSystemMessage`): a stable key (one row per key), title, body, tone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub key: String,
    pub turn_id: String,
    pub title: String,
    pub body: String,
    /// `error` / `info` / `` (the web's tone).
    pub tone: &'static str,
}

/// A steer admitted while an accepted turn runs (`retainedSteers`).
#[derive(Debug, Clone, PartialEq)]
struct RetainedSteer {
    turn: PromptTurn,
    owner_turn_id: String,
    sent: bool,
    returned: bool,
}

/// What [`Composer::submit`] decided.
#[derive(Debug, Clone, PartialEq)]
pub enum Submit {
    /// Refused (recovery hold, blank text, duplicate id…); nothing changed.
    Refused,
    /// Admitted as the active turn: dispatch it now.
    StartNow(PromptTurn),
    /// Admitted behind the active turn (FIFO).
    Queued(PromptTurn),
    /// Admitted as a steer into `expected_turn_id` (the accepted owner).
    Steer { turn: PromptTurn, expected_turn_id: String },
}

/// How a `turn/start` ended, as the flow classified it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOutcome {
    Accepted,
    /// A transport failure / missing ACK: proves nothing (`:398-415`).
    Unconfirmed { timed_out: bool },
    /// A refused start naming the occupying turn (`turn-collision.ts`).
    Collision { occupier: String },
    /// Any other protocol refusal (its message).
    Rejected(String),
}

/// What the flow must do after a transition.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Effects {
    /// The next queue head to dispatch.
    pub start: Option<PromptTurn>,
    /// Transcript notices to add.
    pub notices: Vec<Notice>,
    /// Text to hand back to the owning Session's composer (a turn that never
    /// started: collision/rejection, or an interrupted turn's prompt).
    pub restore: Option<(String, String)>,
    /// Ask the server for this turn's lifecycle (`turn/state/get`).
    pub check_state: Option<String>,
}

#[derive(Debug, Default)]
struct SessionTurns {
    queue: PromptTurnQueue,
    attempted: HashSet<String>,
    /// `locallyStartedTurn`: (turn id, transport generation).
    dispatching: Option<(String, u64)>,
    /// `acceptedOwner`: (turn id, state, transport generation).
    accepted_owner: Option<(String, OwnerState, u64)>,
    interrupting: Option<String>,
    recovery: Option<Recovery>,
    /// Hydrate's server foreground turn: (id, interrupting).
    hydrated_active: Option<(String, bool)>,
    timed_out_start: Option<String>,
    steering_enabled: bool,
    steer_in_flight: bool,
    steer_unknown: bool,
    terminal_receipts: VecDeque<String>,
    steer_admitted: HashSet<String>,
    retained_steers: Vec<RetainedSteer>,
}

impl SessionTurns {
    fn note_terminal(&mut self, turn_id: &str) {
        if !self.terminal_receipts.iter().any(|t| t == turn_id) {
            self.terminal_receipts.push_back(turn_id.to_owned());
        }
        while self.terminal_receipts.len() > 256 {
            self.terminal_receipts.pop_front();
        }
    }

    fn has_terminal(&self, turn_id: &str) -> bool {
        self.terminal_receipts.iter().any(|t| t == turn_id)
    }
}

/// The composer domain: every Session's queue + controller state.
#[derive(Debug, Default)]
pub struct Composer {
    sessions: Mutex<HashMap<String, SessionTurns>>,
    /// The transport generation leases are bound to; bumped when the
    /// transport is suspended (`suspendTransport`, `:1388-1419`).
    generation: Mutex<u64>,
}

impl Composer {
    fn with<R>(&self, session: &str, f: impl FnOnce(&mut SessionTurns, u64) -> R) -> R {
        let generation = *self.generation.lock().unwrap();
        let mut map = self.sessions.lock().unwrap();
        let st = map.entry(session.to_owned()).or_default();
        f(st, generation)
    }

    fn read<R>(&self, session: &str, f: impl FnOnce(Option<&SessionTurns>, u64) -> R) -> R {
        let generation = *self.generation.lock().unwrap();
        let map = self.sessions.lock().unwrap();
        f(map.get(session), generation)
    }

    // ---------------------------------------------------------- reads

    pub fn snapshot(&self, session: &str) -> QueueSnapshot {
        self.read(session, |st, _| st.map(|s| s.queue.snapshot()).unwrap_or_default())
    }

    pub fn recovery(&self, session: &str) -> Option<Recovery> {
        self.read(session, |st, _| st.and_then(|s| s.recovery.clone()))
    }

    pub fn steering_enabled(&self, session: &str) -> bool {
        self.read(session, |st, _| st.is_some_and(|s| s.steering_enabled))
    }

    pub fn dispatching_turn(&self, session: &str) -> Option<String> {
        self.read(session, |st, _| st.and_then(|s| s.dispatching.as_ref().map(|(t, _)| t.clone())))
    }

    pub fn interrupting_turn(&self, session: &str) -> Option<String> {
        self.read(session, |st, _| st.and_then(|s| s.interrupting.clone()))
    }

    /// `activeTurnOwnership` with the lease checked against the CURRENT
    /// transport generation (`leaseMatchesCurrent`).
    pub fn ownership(&self, session: &str) -> Ownership {
        self.read(session, |st, generation| {
            let Some(st) = st else { return Ownership::None };
            if st.dispatching.as_ref().is_some_and(|(_, g)| *g == generation) {
                return Ownership::Dispatching;
            }
            if st.accepted_owner.as_ref().is_some_and(|(_, _, g)| *g == generation) {
                return Ownership::LocalOwner;
            }
            if st.queue.active.is_some() {
                Ownership::Observed
            } else {
                Ownership::None
            }
        })
    }

    /// The accepted owner turn (and its state) on the current transport —
    /// what a background handoff may take over (`backgroundHandoffTurn`).
    pub fn accepted_owner(&self, session: &str) -> Option<(String, OwnerState)> {
        self.read(session, |st, generation| {
            st.and_then(|s| s.accepted_owner.as_ref())
                .filter(|(_, _, g)| *g == generation)
                .map(|(t, s, _)| (t.clone(), *s))
        })
    }

    /// Can "Steer now" act: an accepted (attempted, not dispatching, not
    /// interrupting) active local turn and at least one queued prompt.
    pub fn can_steer_now(&self, session: &str) -> bool {
        self.read(session, |st, _| {
            let Some(st) = st else { return false };
            let Some(active) = &st.queue.active else { return false };
            !st.queue.pending.is_empty()
                && st.attempted.contains(&active.turn_id)
                && st.dispatching.is_none()
                && st.interrupting.is_none()
                && st.recovery.is_none()
                && !st.steer_in_flight
                && !st.steer_unknown
        })
    }

    pub fn generation(&self) -> u64 {
        *self.generation.lock().unwrap()
    }

    // ------------------------------------------------------ admission

    /// `enqueueTurn` (`:603-642`): refuse while a recovery holds, for blank
    /// text without media, or a reused id; otherwise FIFO.
    pub fn enqueue(&self, session: &str, turn: PromptTurn) -> Submit {
        self.with(session, |st, _| enqueue_inner(st, turn))
    }

    /// `submitTurn` (`:681-860`): with steering enabled and an accepted
    /// owner running (no pending prompts, nothing in flight), steer into it;
    /// otherwise enqueue.
    pub fn submit(&self, session: &str, turn: PromptTurn) -> Submit {
        self.with(session, |st, _| {
            let active = st.queue.active.clone();
            let steerable = st.steering_enabled
                && st.recovery.is_none()
                && active.as_ref().is_some_and(|a| st.attempted.contains(&a.turn_id))
                && st.dispatching.is_none()
                && st.queue.pending.is_empty()
                && st.interrupting.is_none()
                && !st.steer_in_flight
                && !st.steer_unknown
                && turn.media.is_empty()
                && turn.reasoning_effort.is_none();
            if !steerable {
                return enqueue_inner(st, turn);
            }
            let active = active.expect("steerable");
            if turn.text.trim().is_empty()
                || st.attempted.contains(&turn.turn_id)
                || st.steer_admitted.contains(&turn.turn_id)
            {
                return Submit::Refused;
            }
            let turn = PromptTurn { text: turn.text.trim().to_owned(), ..turn };
            admit_steer(st, turn, &active.turn_id)
        })
    }

    /// "Steer now" on the queued chip (conversation-08 board: `1 queued ·
    /// Steer now · ✕`): take the queue's head and steer it into the accepted
    /// active turn, the explicit form of the web's steering opt-in.
    pub fn steer_head(&self, session: &str) -> Submit {
        self.with(session, |st, _| {
            let Some(active) = st.queue.active.clone() else { return Submit::Refused };
            if st.queue.pending.is_empty()
                || !st.attempted.contains(&active.turn_id)
                || st.dispatching.is_some()
                || st.interrupting.is_some()
                || st.recovery.is_some()
                || st.steer_in_flight
                || st.steer_unknown
            {
                return Submit::Refused;
            }
            let head = st.queue.pending.pop_front().expect("non-empty");
            if !head.media.is_empty() {
                // Media cannot ride a steer: keep it queued.
                st.queue.pending.push_front(head);
                return Submit::Refused;
            }
            admit_steer(st, head, &active.turn_id)
        })
    }

    /// `cancelQueuedPrompt`: remove a not-yet-dispatched prompt; the active
    /// turn and the server are untouched.
    pub fn remove_pending(&self, session: &str, turn_id: &str) -> bool {
        self.with(session, |st, _| st.queue.remove_pending(turn_id))
    }

    /// `setSteeringEnabled` (`/steer [on|off]`, `intent.ts:96-108`).
    pub fn set_steering(&self, session: &str, enabled: bool) {
        self.with(session, |st, _| st.steering_enabled = enabled);
    }

    // ------------------------------------------------------- dispatch

    /// `startTurn`'s gate (`:248-262`): the queue head, not attempted, no
    /// recovery hold, no steer in flight. Marks it attempted + dispatching
    /// and returns the queue's owned copy to send.
    pub fn begin_dispatch(&self, session: &str, turn_id: &str) -> Option<PromptTurn> {
        self.with(session, |st, generation| {
            let head = st.queue.active.clone()?;
            if head.turn_id != turn_id
                || st.recovery.is_some()
                || st.steer_in_flight
                || st.steer_unknown
                || st.attempted.contains(turn_id)
                || head.origin == Origin::Adopted
            {
                return None;
            }
            st.attempted.insert(turn_id.to_owned());
            st.dispatching = Some((turn_id.to_owned(), generation));
            Some(head)
        })
    }

    /// Classify a finished `turn/start` (`:378-487`).
    pub fn finish_dispatch(&self, session: &str, turn_id: &str, outcome: StartOutcome) -> Effects {
        self.with(session, |st, generation| {
            let mut fx = Effects::default();
            // A stale completion (the queue moved on) changes nothing.
            if st.queue.active.as_ref().map(|a| a.turn_id.as_str()) != Some(turn_id) {
                return fx;
            }
            match outcome {
                StartOutcome::Accepted => {
                    accept_local_dispatch(st, turn_id, OwnerState::Running, generation);
                }
                StartOutcome::Unconfirmed { timed_out } => {
                    // Server activity may already have promoted the dispatch.
                    if st.dispatching.as_ref().map(|(t, _)| t.as_str()) != Some(turn_id) {
                        return fx;
                    }
                    st.timed_out_start = Some(turn_id.to_owned());
                    fx.notices.push(Notice {
                        key: format!("send-timeout:{turn_id}"),
                        turn_id: turn_id.to_owned(),
                        title: if timed_out { "Turn start timed out" } else { "Turn start unconfirmed" }.to_owned(),
                        body: "The server did not acknowledge the turn. It may still be running — do not resubmit; check its status after reconnecting.".to_owned(),
                        tone: "error",
                    });
                    // The turn is held with an unknown outcome: the recovery
                    // notice offers the lifecycle check.
                    st.recovery = Some(Recovery { turn_id: turn_id.to_owned(), phase: RecoveryPhase::Unknown });
                }
                StartOutcome::Collision { occupier } => {
                    let ours = st.queue.active.clone().expect("checked");
                    retire_local_dispatch(st, turn_id);
                    let transition = st.queue.settle(turn_id);
                    // The occupier's terminal may have beaten our refusal.
                    let occupied = !st.has_terminal(&occupier);
                    if occupied {
                        st.attempted.insert(occupier.clone());
                        st.queue.restore_active(PromptTurn::adopted(&occupier), true);
                    }
                    fx.restore = Some((session.to_owned(), ours.text.clone()));
                    fx.notices.push(Notice {
                        key: format!("send-busy:{turn_id}"),
                        turn_id: turn_id.to_owned(),
                        title: "Session busy".to_owned(),
                        body: "Another client was working in this session, so this message was not sent. It was kept for retry and will return when the composer is empty. Send it again when the running turn finishes, or Stop that turn to take over.".to_owned(),
                        tone: "info",
                    });
                    if !occupied {
                        fx.start = transition.next;
                    }
                }
                StartOutcome::Rejected(message) => {
                    let ours = st.queue.active.clone().expect("checked");
                    retire_local_dispatch(st, turn_id);
                    fx.restore = Some((session.to_owned(), ours.text.clone()));
                    fx.notices.push(Notice {
                        key: format!("send-error:{turn_id}"),
                        turn_id: turn_id.to_owned(),
                        title: "Turn rejected".to_owned(),
                        body: message,
                        tone: "error",
                    });
                    let more = settle_inner(st, session, turn_id, OwnerState::Failed);
                    merge(&mut fx, more);
                }
            }
            fx
        })
    }

    /// The transport changed while the seat gate ran (`:338-345`): the turn
    /// was never written, so it is NOT attempted and stays the queue head for
    /// the next ready drain (`resumePendingTurn`). False when the dispatch
    /// already moved on.
    pub fn cancel_dispatch(&self, session: &str, turn_id: &str) -> bool {
        self.with(session, |st, _| {
            if st.queue.active.as_ref().map(|a| a.turn_id.as_str()) != Some(turn_id) {
                return false;
            }
            retire_local_dispatch(st, turn_id);
            st.attempted.remove(turn_id);
            true
        })
    }

    /// A seat-gate refusal BEFORE any `turn/start` frame (`:300-328`): the
    /// turn never started, its text goes back, the notice names why.
    pub fn not_sent(&self, session: &str, turn_id: &str, message: &str) -> Effects {
        self.with(session, |st, _| {
            let mut fx = Effects::default();
            let Some(ours) = st.queue.active.clone().filter(|a| a.turn_id == turn_id) else { return fx };
            retire_local_dispatch(st, turn_id);
            fx.restore = Some((session.to_owned(), ours.text.clone()));
            fx.notices.push(Notice {
                key: format!("send-error:{turn_id}"),
                turn_id: turn_id.to_owned(),
                title: "Turn not sent".to_owned(),
                body: message.to_owned(),
                tone: "error",
            });
            let more = settle_inner(st, session, turn_id, OwnerState::Failed);
            merge(&mut fx, more);
            fx
        })
    }

    // ------------------------------------------------------ lifecycle

    /// A terminal for `turn_id` (`settleTurn`, `:520-560`): clears its
    /// recovery/hydrate/timeout marks, records the receipt, retires its
    /// leases, settles the queue (unless another hydrated foreground owns it)
    /// and returns the next head to start + an interrupted prompt to restore.
    pub fn settle(&self, session: &str, turn_id: &str, completed: bool) -> Effects {
        self.with(session, |st, _| {
            settle_inner(st, session, turn_id, if completed { OwnerState::Completed } else { OwnerState::Failed })
        })
    }

    /// Server activity for `turn_id` (a delta, a tool event, `turn/started`)
    /// proves an unacknowledged start was accepted (`confirmTurnAccepted`,
    /// `:1270-1289`); a `turn/started` for a turn this composer never queued
    /// is another client's: adopt it as observed (`adoptForeignTurn`,
    /// `:824-834`).
    pub fn observe_activity(&self, session: &str, turn_id: &str, started: bool) -> Effects {
        self.with(session, |st, generation| {
            let mut fx = Effects::default();
            let was_timed_out = st.timed_out_start.as_deref() == Some(turn_id);
            let accepted = accept_local_dispatch(st, turn_id, OwnerState::Running, generation);
            if accepted && was_timed_out {
                fx.notices.push(Notice {
                    key: format!("send-timeout:{turn_id}"),
                    turn_id: turn_id.to_owned(),
                    title: "Turn start timed out".to_owned(),
                    body: "The server had accepted the turn after all — no resubmission needed.".to_owned(),
                    tone: "info",
                });
            }
            if started
                && !st.attempted.contains(turn_id)
                && st.dispatching.as_ref().map(|(t, _)| t.as_str()) != Some(turn_id)
                && st.queue.active.is_none()
                && !st.queue.pending.iter().any(|t| t.turn_id == turn_id)
                && !st.has_terminal(turn_id)
            {
                st.attempted.insert(turn_id.to_owned());
                st.queue.restore_active(PromptTurn::adopted(turn_id), true);
            }
            fx
        })
    }

    /// The user pressed Stop on the active turn: stash its prompt for its
    /// OWN terminal (`interrupt`, `:877-905`). Returns false when nothing is
    /// interruptible yet (a start still in flight, a recovery hold).
    pub fn begin_interrupt(&self, session: &str, turn_id: &str) -> bool {
        self.with(session, |st, _| {
            if st.recovery.is_some() {
                return false;
            }
            if st.dispatching.as_ref().map(|(t, _)| t.as_str()) == Some(turn_id) {
                return false; // "Turn is still starting"
            }
            if st.interrupting.as_deref() == Some(turn_id) {
                return false;
            }
            if let Some(active) = st.queue.active.clone().filter(|a| a.turn_id == turn_id) {
                st.queue.stash_interrupt_prompt(session, turn_id, &active.text);
            }
            st.interrupting = Some(turn_id.to_owned());
            true
        })
    }

    /// An interrupt request failed: the turn is not interrupting.
    pub fn interrupt_failed(&self, session: &str, turn_id: &str) {
        self.with(session, |st, _| {
            if st.interrupting.as_deref() == Some(turn_id) {
                st.interrupting = None;
            }
        });
    }

    // ------------------------------------------------------- steering

    /// A steer request is about to go out (`markSent`, `:739-753`).
    pub fn steer_sent(&self, session: &str, turn_id: &str) -> bool {
        self.with(session, |st, _| {
            let Some(entry) = st.retained_steers.iter_mut().find(|e| e.turn.turn_id == turn_id) else {
                return false;
            };
            if entry.sent || st.interrupting.is_some() {
                return false;
            }
            entry.sent = true;
            true
        })
    }

    /// The steer's result (`:754-822`). `steered_into` = the receipt's
    /// `(turn_id, steered)`; `Err(protocol)` = a refusal (`true`) or a
    /// transport failure after sending (`false`).
    pub fn finish_steer(
        &self,
        session: &str,
        turn_id: &str,
        result: Result<(String, bool), bool>,
    ) -> Effects {
        self.with(session, |st, generation| {
            let mut fx = Effects::default();
            let Some(i) = st.retained_steers.iter().position(|e| e.turn.turn_id == turn_id) else {
                st.steer_in_flight = false;
                return fx;
            };
            let entry = st.retained_steers[i].clone();
            match result {
                Ok((receipt_turn, steered)) => {
                    if entry.returned {
                        // Already handed back by `turn/steer_dropped`.
                    } else if !steered {
                        // Core's no-active fallback STARTED this text as a new
                        // turn: adopt it without a second start.
                        st.retained_steers.remove(i);
                        st.attempted.insert(receipt_turn.clone());
                        if !st.has_terminal(&receipt_turn) {
                            if st.queue.active.as_ref().map(|a| a.turn_id.as_str()) == Some(entry.owner_turn_id.as_str()) {
                                st.queue.settle(&entry.owner_turn_id);
                            }
                            let adopted = PromptTurn { turn_id: receipt_turn.clone(), ..entry.turn.clone() };
                            st.queue.restore_active(adopted, true);
                            st.accepted_owner = Some((receipt_turn.clone(), OwnerState::Running, generation));
                        }
                        fx.notices.push(Notice {
                            key: format!("steer:{turn_id}"),
                            turn_id: receipt_turn,
                            title: "Native steering started a new turn".to_owned(),
                            body: "The prior turn ended before admission. Core started the submitted text as a new turn.".to_owned(),
                            tone: "",
                        });
                    } else {
                        fx.notices.push(Notice {
                            key: format!("steer:{turn_id}"),
                            turn_id: entry.owner_turn_id.clone(),
                            title: "Steering accepted".to_owned(),
                            body: entry.turn.text.clone(),
                            tone: "",
                        });
                    }
                }
                Err(protocol) => {
                    if !entry.returned && (!entry.sent || protocol) {
                        st.retained_steers.remove(i);
                        restage_steers(st, vec![entry.turn.clone()]);
                        fx.notices.push(Notice {
                            key: format!("steer:{turn_id}"),
                            turn_id: entry.owner_turn_id.clone(),
                            title: "Steering queued".to_owned(),
                            body: "Steering was not admitted. The text remains ahead of later pending prompts.".to_owned(),
                            tone: "",
                        });
                    } else if !entry.returned {
                        st.steer_unknown = true;
                        fx.notices.push(Notice {
                            key: format!("steer:{turn_id}"),
                            turn_id: entry.owner_turn_id.clone(),
                            title: "Steering outcome unknown".to_owned(),
                            body: format!(
                                "The request may have been consumed or started a new turn. It will not be resent automatically. Recover the Session before continuing. Submitted text: {}",
                                entry.turn.text
                            ),
                            tone: "error",
                        });
                    }
                }
            }
            st.steer_in_flight = false;
            if !st.steer_unknown && st.has_terminal(&entry.owner_turn_id) {
                let owner = entry.owner_turn_id.clone();
                st.retained_steers.retain(|e| e.owner_turn_id != owner);
            }
            // A turn may have been restaged at the head: start it.
            if let Some(head) = st.queue.active.clone() {
                if !st.attempted.contains(&head.turn_id) && head.origin == Origin::Local {
                    fx.start = Some(head);
                }
            }
            fx
        })
    }

    /// `turn/steer_dropped` (`observeSteerDropped`, `:860-899`): accepted but
    /// undrained steer inputs come back, in order, ahead of later prompts.
    pub fn steer_dropped(&self, session: &str, turn_id: &str, inputs: &[String]) -> Effects {
        self.with(session, |st, _| {
            let mut fx = Effects::default();
            let mut returned = Vec::new();
            for text in inputs {
                if let Some(i) = st
                    .retained_steers
                    .iter()
                    .position(|e| e.sent && !e.returned && e.owner_turn_id == turn_id && &e.turn.text == text)
                {
                    let entry = st.retained_steers.remove(i);
                    fx.notices.push(Notice {
                        key: format!("steer:{}", entry.turn.turn_id),
                        turn_id: turn_id.to_owned(),
                        title: "Steering returned to queue".to_owned(),
                        body: entry.turn.text.clone(),
                        tone: "",
                    });
                    returned.push(entry.turn);
                }
            }
            if !returned.is_empty() {
                st.steer_unknown = false;
            }
            restage_steers(st, returned);
            fx
        })
    }

    // ------------------------------------------------------- recovery

    /// `retryTurnRecovery` (`:930-988`): the active turn to check, or the
    /// `unavailable` phase when the server cannot answer.
    pub fn begin_recovery_check(&self, session: &str, can_get_state: bool) -> Option<String> {
        self.with(session, |st, _| {
            let turn = st.queue.active.as_ref()?.turn_id.clone();
            if matches!(st.recovery.as_ref().map(|r| &r.phase), Some(RecoveryPhase::Checking)) {
                return None;
            }
            if !can_get_state {
                st.recovery = Some(Recovery { turn_id: turn, phase: RecoveryPhase::Unavailable });
                return None;
            }
            st.recovery = Some(Recovery { turn_id: turn.clone(), phase: RecoveryPhase::Checking });
            Some(turn)
        })
    }

    /// The `turn/state/get` answer (`:955-975`, `applyRecoveredState`
    /// `:1015-1050`). `Err` = the check failed.
    pub fn finish_recovery_check(&self, session: &str, turn_id: &str, state: Result<Lifecycle, String>) -> Effects {
        self.with(session, |st, generation| {
            let mut fx = Effects::default();
            if st.queue.active.as_ref().map(|a| a.turn_id.as_str()) != Some(turn_id) {
                return fx;
            }
            match state {
                Err(message) => {
                    st.recovery = Some(Recovery { turn_id: turn_id.to_owned(), phase: RecoveryPhase::Error(message) });
                }
                Ok(Lifecycle::Unknown) => {
                    st.recovery = Some(Recovery { turn_id: turn_id.to_owned(), phase: RecoveryPhase::Unknown });
                }
                Ok(s) => {
                    st.recovery = None;
                    let owner_state = match s {
                        Lifecycle::Completed => OwnerState::Completed,
                        Lifecycle::Active | Lifecycle::Interrupting => OwnerState::Running,
                        _ => OwnerState::Failed,
                    };
                    accept_local_dispatch(st, turn_id, owner_state, generation);
                    st.interrupting = (s == Lifecycle::Interrupting).then(|| turn_id.to_owned());
                    if !s.live() {
                        let more = settle_inner(st, session, turn_id, owner_state);
                        merge(&mut fx, more);
                    }
                }
            }
            fx
        })
    }

    /// "Continue without it" (`continueWithoutTurn`, `:990-1013`): release
    /// the local wait without claiming a terminal or resending; queued
    /// prompts send next.
    pub fn continue_without(&self, session: &str) -> Effects {
        self.with(session, |st, _| {
            let mut fx = Effects::default();
            let Some(turn_id) = st.queue.active.as_ref().map(|a| a.turn_id.clone()) else { return fx };
            match &st.recovery {
                Some(r) if r.turn_id == turn_id && r.phase != RecoveryPhase::Checking => {}
                _ => return fx,
            }
            st.recovery = None;
            st.timed_out_start = None;
            retire_local_dispatch(st, &turn_id);
            if st.accepted_owner.as_ref().is_some_and(|(t, _, _)| *t == turn_id) {
                st.accepted_owner = None;
            }
            // No server terminal was observed: never restore the prompt.
            st.queue.take_interrupt_prompt(&turn_id);
            fx.notices.push(Notice {
                key: format!("turn-unresolved:{turn_id}"),
                turn_id: turn_id.clone(),
                title: "Response outcome unknown".to_owned(),
                body: "Continued without confirming this response. No stop request or resubmission was sent.".to_owned(),
                tone: "",
            });
            let transition = st.queue.settle(&turn_id);
            st.interrupting = None;
            fx.start = transition.next;
            fx
        })
    }

    // ------------------------------------------------- hydrate/transport

    /// `reconcileFromHydrate` (`:1052-1268`) over a canonical hydrate's
    /// `turns` (id, state). Without `preserve`, transport ownership is
    /// retired; with it, only leases of the current generation survive.
    pub fn reconcile_hydrate(&self, session: &str, turns: &[(String, Lifecycle)], preserve: bool) -> Effects {
        self.with(session, |st, generation| {
            let mut fx = Effects::default();
            st.steer_unknown = false;
            st.recovery = None;
            if !preserve {
                st.dispatching = None;
                st.accepted_owner = None;
            } else {
                if st.dispatching.as_ref().is_some_and(|(_, g)| *g != generation) {
                    st.dispatching = None;
                }
                if st.accepted_owner.as_ref().is_some_and(|(_, _, g)| *g != generation) {
                    st.accepted_owner = None;
                }
            }
            let server_active = turns.iter().find(|(_, s)| s.live()).cloned();
            st.hydrated_active = server_active
                .as_ref()
                .map(|(id, s)| (id.clone(), *s == Lifecycle::Interrupting));
            st.interrupting = server_active
                .as_ref()
                .filter(|(_, s)| *s == Lifecycle::Interrupting)
                .map(|(id, _)| id.clone());
            let active = st.queue.active.clone();
            if let Some((sid, _)) = &server_active {
                let ours_unsent = active.as_ref().map_or(true, |a| !st.attempted.contains(&a.turn_id));
                if active.as_ref().map(|a| &a.turn_id) != Some(sid) && ours_unsent {
                    // A hydrated active turn is OBSERVED, never owned.
                    st.attempted.insert(sid.clone());
                    st.queue.restore_active(PromptTurn::adopted(sid), true);
                    return fx;
                }
            }
            let Some(active) = active else { return fx };
            if !st.attempted.contains(&active.turn_id) {
                // A head that never reached Core: let the ready drain send it.
                fx.start = Some(active);
                return fx;
            }
            let server_turn = turns.iter().find(|(id, _)| *id == active.turn_id).cloned();
            if let Some((_, s)) = &server_turn {
                if *s != Lifecycle::Unknown {
                    let state = match s {
                        Lifecycle::Completed => OwnerState::Completed,
                        Lifecycle::Active | Lifecycle::Interrupting => OwnerState::Running,
                        _ => OwnerState::Failed,
                    };
                    accept_local_dispatch(st, &active.turn_id, state, generation);
                    if !s.live() {
                        if let Some(owner) = st.accepted_owner.as_mut().filter(|o| o.0 == active.turn_id) {
                            owner.1 = state;
                        }
                        let transition = st.queue.settle(&active.turn_id);
                        st.note_terminal(&active.turn_id);
                        if let Some((sid, _)) = &server_active {
                            if *sid != active.turn_id {
                                st.attempted.insert(sid.clone());
                                st.queue.restore_active(PromptTurn::adopted(sid), true);
                                return fx;
                            }
                        }
                        fx.start = transition.next;
                        return fx;
                    }
                    return fx;
                }
            }
            // Absent or unknown: hydrate cannot prove it never ran.
            fx.check_state = Some(active.turn_id.clone());
            fx
        })
    }

    /// `suspendTransport` (`:1388-1419`): the transport changed. Leases of
    /// the old generation stop counting; unsent steers go back to the queue,
    /// sent ones are reported unknown; nothing is replayed.
    pub fn suspend_transport(&self) -> Vec<(String, Effects)> {
        {
            let mut g = self.generation.lock().unwrap();
            *g += 1;
        }
        let mut out = Vec::new();
        let mut map = self.sessions.lock().unwrap();
        for (session, st) in map.iter_mut() {
            let mut fx = Effects::default();
            let unsent: Vec<PromptTurn> =
                st.retained_steers.iter().filter(|e| !e.sent).map(|e| e.turn.clone()).collect();
            for e in st.retained_steers.iter().filter(|e| e.sent && !e.returned) {
                fx.notices.push(Notice {
                    key: format!("steer:{}", e.turn.turn_id),
                    turn_id: e.owner_turn_id.clone(),
                    title: "Steering outcome unknown".to_owned(),
                    body: format!(
                        "Transport changed before consumption could be confirmed. This text will not be resent automatically: {}",
                        e.turn.text
                    ),
                    tone: "error",
                });
            }
            st.retained_steers.clear();
            st.steer_in_flight = false;
            st.steer_unknown = false;
            restage_steers(st, unsent);
            st.dispatching = None;
            st.accepted_owner = None;
            st.interrupting = None;
            if !fx.notices.is_empty() {
                out.push((session.clone(), fx));
            }
        }
        out
    }

    /// Clear one Session's queue entirely (a New chat replaces it).
    pub fn reset(&self, session: &str) {
        self.sessions.lock().unwrap().remove(session);
    }
}

fn enqueue_inner(st: &mut SessionTurns, turn: PromptTurn) -> Submit {
    if st.recovery.is_some()
        || turn.turn_id.trim().is_empty()
        || (turn.text.trim().is_empty() && turn.media.is_empty())
        || st.attempted.contains(&turn.turn_id)
        || st.steer_admitted.contains(&turn.turn_id)
        || st.queue.active.as_ref().is_some_and(|a| a.turn_id == turn.turn_id)
        || st.queue.pending.iter().any(|p| p.turn_id == turn.turn_id)
    {
        return Submit::Refused;
    }
    let turn = PromptTurn { text: turn.text.trim().to_owned(), ..turn };
    if st.queue.enqueue(turn.clone()) {
        Submit::StartNow(turn)
    } else {
        Submit::Queued(turn)
    }
}

fn admit_steer(st: &mut SessionTurns, turn: PromptTurn, owner: &str) -> Submit {
    st.steer_admitted.insert(turn.turn_id.clone());
    st.retained_steers.push(RetainedSteer {
        turn: turn.clone(),
        owner_turn_id: owner.to_owned(),
        sent: false,
        returned: false,
    });
    st.steer_in_flight = true;
    Submit::Steer { turn, expected_turn_id: owner.to_owned() }
}

/// `restageSteers` (`:662-671`): returned/refused steering goes back AHEAD
/// of later prompts, in order.
fn restage_steers(st: &mut SessionTurns, turns: Vec<PromptTurn>) {
    if turns.is_empty() {
        return;
    }
    let active_attempted = st.queue.active.as_ref().is_some_and(|a| st.attempted.contains(&a.turn_id));
    if !active_attempted {
        let mut it = turns.into_iter();
        let first = it.next().expect("non-empty");
        let had_active = st.queue.active.is_some();
        st.queue.restore_active(first, had_active);
        st.queue.prepend_pending(it.collect());
    } else {
        st.queue.prepend_pending(turns);
    }
}

fn accept_local_dispatch(st: &mut SessionTurns, turn_id: &str, state: OwnerState, generation: u64) -> bool {
    match &st.dispatching {
        Some((t, g)) if t == turn_id && *g == generation => {}
        _ => return false,
    }
    if st.recovery.as_ref().is_some_and(|r| r.turn_id == turn_id) {
        st.recovery = None;
    }
    if st.timed_out_start.as_deref() == Some(turn_id) {
        st.timed_out_start = None;
    }
    st.accepted_owner = Some((turn_id.to_owned(), state, generation));
    st.dispatching = None;
    true
}

fn retire_local_dispatch(st: &mut SessionTurns, turn_id: &str) -> bool {
    match &st.dispatching {
        Some((t, _)) if t == turn_id => {
            st.dispatching = None;
            true
        }
        _ => false,
    }
}

fn settle_inner(st: &mut SessionTurns, session: &str, turn_id: &str, outcome: OwnerState) -> Effects {
    let mut fx = Effects::default();
    if st.recovery.as_ref().is_some_and(|r| r.turn_id == turn_id) {
        st.recovery = None;
    }
    if st.hydrated_active.as_ref().is_some_and(|(t, _)| t == turn_id) {
        st.hydrated_active = None;
    }
    if st.timed_out_start.as_deref() == Some(turn_id) {
        st.timed_out_start = None;
    }
    st.note_terminal(turn_id);
    if !st.steer_in_flight {
        st.retained_steers.retain(|e| e.owner_turn_id != turn_id);
    }
    if let Some(owner) = st.accepted_owner.as_mut().filter(|o| o.0 == turn_id) {
        owner.1 = outcome;
    }
    retire_local_dispatch(st, turn_id);
    // Another hydrated foreground owns the session: never advance the local
    // FIFO over it (`releaseQueueTurn`, `:562-590`).
    if let Some((other, interrupting)) = st.hydrated_active.clone() {
        if other != turn_id && st.queue.active.as_ref().map(|a| a.turn_id.as_str()) == Some(turn_id) {
            st.queue.settle(turn_id);
            st.attempted.insert(other.clone());
            st.queue.restore_active(PromptTurn::adopted(&other), true);
            st.interrupting = interrupting.then_some(other);
            if let Some(r) = st.queue.take_interrupt_prompt(turn_id) {
                fx.restore = Some((r.session_id, r.prompt));
            }
            return fx;
        }
    }
    let transition = st.queue.settle(turn_id);
    if !transition.settled {
        return fx;
    }
    if st.interrupting.as_deref() == Some(turn_id) {
        st.interrupting = None;
    }
    if let Some(r) = st.queue.take_interrupt_prompt(turn_id) {
        fx.restore = Some((r.session_id, r.prompt));
    } else {
        let _ = session;
    }
    fx.start = transition.next;
    fx
}

fn merge(fx: &mut Effects, more: Effects) {
    if more.start.is_some() {
        fx.start = more.start;
    }
    fx.notices.extend(more.notices);
    if more.restore.is_some() && fx.restore.is_none() {
        fx.restore = more.restore;
    }
    if more.check_state.is_some() {
        fx.check_state = more.check_state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: &str) -> PromptTurn {
        PromptTurn::local(id, id)
    }

    // ---- turn-queue.test.ts:47 / :63 / :81 / :97
    #[test]
    fn the_queue_is_fifo_and_removes_only_pending_prompts() {
        let mut q = PromptTurnQueue::default();
        assert!(q.enqueue(t("one")));
        assert!(!q.enqueue(t("two")));
        assert!(!q.enqueue(t("three")));
        assert_eq!(q.snapshot().pending.iter().map(|p| p.turn_id.as_str()).collect::<Vec<_>>(), ["two", "three"]);
        assert!(!q.remove_pending("one"), "the active turn is not pending");
        assert!(!q.remove_pending("missing"));
        assert!(q.remove_pending("two"));
        assert!(!q.remove_pending("two"));
        assert_eq!(q.settle("stale"), QueueTransition::default(), "a stale terminal changes nothing");
        assert_eq!(q.settle("one").next.map(|n| n.turn_id), Some("three".into()));
        assert_eq!(q.settle("one"), QueueTransition::default(), "duplicate terminal");
        let mut q = PromptTurnQueue::default();
        assert!(q.restore_active(t("server-turn"), false));
        assert!(!q.restore_active(t("other-turn"), false));
        assert_eq!(q.snapshot().active.unwrap().turn_id, "server-turn");
    }

    // ---- turn-queue.test.ts:104 / :118 / :131 (interrupt restores)
    #[test]
    fn interrupt_restores_are_keyed_by_turn_one_per_session() {
        let mut q = PromptTurnQueue::default();
        q.stash_interrupt_prompt("a", "turn-a1", "first");
        q.stash_interrupt_prompt("b", "turn-b", "keep b");
        q.stash_interrupt_prompt("a", "turn-a2", "second");
        assert_eq!(q.take_interrupt_prompt("turn-a1"), None);
        assert_eq!(q.take_interrupt_prompt("turn-a2").unwrap().prompt, "second");
        assert_eq!(q.take_interrupt_prompt("turn-b").unwrap().prompt, "keep b");
        q.stash_interrupt_prompt("a", "turn-a", "   ");
        assert_eq!(q.take_interrupt_prompt("turn-a"), None, "an empty prompt is never stashed");
    }

    // ---- use-turn-controller.test.ts:38 / :88: the first prompt starts,
    // ---- later ones queue; a queued prompt is cancelled without touching
    // ---- the server; the next starts when the active one settles.
    #[test]
    fn first_starts_later_queue_and_settle_starts_the_next() {
        let c = Composer::default();
        assert_eq!(c.enqueue("s", t("one")), Submit::StartNow(t("one")));
        assert!(matches!(c.enqueue("s", t("two")), Submit::Queued(_)));
        assert!(matches!(c.enqueue("s", t("three")), Submit::Queued(_)));
        assert_eq!(c.begin_dispatch("s", "one"), Some(t("one")));
        assert_eq!(c.begin_dispatch("s", "one"), None, "an attempted id is never sent twice");
        assert_eq!(c.ownership("s"), Ownership::Dispatching);
        assert_eq!(c.finish_dispatch("s", "one", StartOutcome::Accepted), Effects::default());
        assert_eq!(c.ownership("s"), Ownership::LocalOwner);
        assert!(c.remove_pending("s", "two"), "cancel a queued prompt");
        let fx = c.settle("s", "one", true);
        assert_eq!(fx.start, Some(t("three")));
        assert_eq!(c.snapshot("s").active.unwrap().turn_id, "three");
        assert!(c.snapshot("s").pending.is_empty());
        // Blank text and reused ids are refused.
        assert_eq!(c.enqueue("s", PromptTurn::local("x", "   ")), Submit::Refused);
        assert_eq!(c.enqueue("s", t("one")), Submit::Refused);
    }

    // ---- use-turn-controller.ts:431-468 (turn-collision.test.ts semantics)
    #[test]
    fn a_collision_hands_the_text_back_and_waits_behind_the_occupier() {
        let c = Composer::default();
        c.enqueue("s", t("mine"));
        c.enqueue("s", t("later"));
        c.begin_dispatch("s", "mine");
        let occupier = "3f1a9c52-4d1b-4c2e-8f6a-0b7d21e9c4aa".to_owned();
        let fx = c.finish_dispatch("s", "mine", StartOutcome::Collision { occupier: occupier.clone() });
        assert_eq!(fx.restore, Some(("s".into(), "mine".into())), "the text is not thrown away");
        assert_eq!(fx.notices[0].title, "Session busy");
        assert_eq!(fx.start, None, "pending prompts wait behind the occupier");
        let snap = c.snapshot("s");
        assert_eq!(snap.active.as_ref().unwrap().turn_id, occupier);
        assert_eq!(snap.active.unwrap().origin, Origin::Adopted);
        assert_eq!(c.ownership("s"), Ownership::Observed);
        // The occupier's terminal releases the FIFO.
        let fx = c.settle("s", &occupier, true);
        assert_eq!(fx.start.map(|t| t.turn_id), Some("later".into()));
    }

    #[test]
    fn a_collision_whose_occupier_already_finished_starts_the_next() {
        let c = Composer::default();
        let occupier = "3f1a9c52-4d1b-4c2e-8f6a-0b7d21e9c4aa".to_owned();
        c.settle("s", &occupier, true); // its terminal beat our refusal
        c.enqueue("s", t("mine"));
        c.enqueue("s", t("later"));
        c.begin_dispatch("s", "mine");
        let fx = c.finish_dispatch("s", "mine", StartOutcome::Collision { occupier });
        assert_eq!(fx.start.map(|t| t.turn_id), Some("later".into()));
    }

    // ---- use-turn-controller.test.ts:226 / :399 / :426 (unknown outcome)
    #[test]
    fn a_start_timeout_is_held_unknown_never_rejected_or_resent() {
        let c = Composer::default();
        c.enqueue("s", t("one"));
        c.enqueue("s", t("two"));
        c.begin_dispatch("s", "one");
        let fx = c.finish_dispatch("s", "one", StartOutcome::Unconfirmed { timed_out: true });
        assert_eq!(fx.notices[0].title, "Turn start timed out");
        assert_eq!(fx.start, None, "the FIFO does not advance over an unknown outcome");
        assert_eq!(c.recovery("s").unwrap().phase, RecoveryPhase::Unknown);
        // New admission is held while recovery is unresolved.
        assert_eq!(c.enqueue("s", t("three")), Submit::Refused);
        // An explicit status retry recovers.
        assert_eq!(c.begin_recovery_check("s", true), Some("one".into()));
        assert_eq!(c.recovery("s").unwrap().phase, RecoveryPhase::Checking);
        let fx = c.finish_recovery_check("s", "one", Ok(Lifecycle::Completed));
        assert_eq!(fx.start.map(|t| t.turn_id), Some("two".into()));
        assert_eq!(c.recovery("s"), None);
        assert_eq!(c.begin_dispatch("s", "one"), None, "never resent");
    }

    #[test]
    fn continue_without_it_releases_the_wait_without_a_terminal_or_resend() {
        let c = Composer::default();
        c.enqueue("s", t("one"));
        c.enqueue("s", t("two"));
        c.begin_dispatch("s", "one");
        c.finish_dispatch("s", "one", StartOutcome::Unconfirmed { timed_out: false });
        assert_eq!(c.begin_recovery_check("s", true), Some("one".into()));
        c.finish_recovery_check("s", "one", Ok(Lifecycle::Unknown));
        let fx = c.continue_without("s");
        assert_eq!(fx.notices[0].title, "Response outcome unknown");
        assert_eq!(fx.start.map(|t| t.turn_id), Some("two".into()), "queued messages send next");
        assert_eq!(fx.restore, None, "the prompt is not restored without a terminal");
        // Unavailable: the server has no lifecycle lookup.
        let c = Composer::default();
        c.enqueue("s", t("one"));
        c.begin_dispatch("s", "one");
        c.finish_dispatch("s", "one", StartOutcome::Unconfirmed { timed_out: true });
        assert_eq!(c.begin_recovery_check("s", false), None);
        assert_eq!(c.recovery("s").unwrap().phase, RecoveryPhase::Unavailable);
    }

    #[test]
    fn server_activity_proves_an_unacknowledged_start() {
        let c = Composer::default();
        c.enqueue("s", t("one"));
        c.begin_dispatch("s", "one");
        c.finish_dispatch("s", "one", StartOutcome::Unconfirmed { timed_out: true });
        let fx = c.observe_activity("s", "one", false);
        assert_eq!(fx.notices[0].body, "The server had accepted the turn after all — no resubmission needed.");
        assert_eq!(c.recovery("s"), None);
        assert_eq!(c.ownership("s"), Ownership::LocalOwner);
    }

    // ---- turn-steering.test.ts:95 / :107 / :141 / :160
    #[test]
    fn steering_defaults_off_and_steers_into_the_accepted_owner_when_on() {
        let c = Composer::default();
        c.enqueue("s", t("owner"));
        assert!(matches!(c.submit("s", t("early")), Submit::Queued(_)), "queues before ACK");
        c.remove_pending("s", "early");
        c.begin_dispatch("s", "owner");
        assert!(matches!(c.submit("s", t("x")), Submit::Queued(_)), "steering is opt-in");
        c.remove_pending("s", "x");
        c.finish_dispatch("s", "owner", StartOutcome::Accepted);
        c.set_steering("s", true);
        match c.submit("s", t("steer me")) {
            Submit::Steer { expected_turn_id, turn } => {
                assert_eq!(expected_turn_id, "owner");
                assert_eq!(turn.text, "steer me");
            }
            other => panic!("expected a steer, got {other:?}"),
        }
        assert!(c.steer_sent("s", "steer me"));
        let fx = c.finish_steer("s", "steer me", Ok(("owner".into(), true)));
        assert_eq!(fx.notices[0].title, "Steering accepted");
        assert_eq!(c.snapshot("s").active.unwrap().turn_id, "owner", "no second turn");
    }

    #[test]
    fn a_rejected_steer_is_restored_ahead_of_later_prompts_in_order() {
        let c = Composer::default();
        c.enqueue("s", t("owner"));
        c.begin_dispatch("s", "owner");
        c.finish_dispatch("s", "owner", StartOutcome::Accepted);
        c.enqueue("s", t("q1"));
        c.enqueue("s", t("q2"));
        // "Steer now" takes the head.
        assert!(c.can_steer_now("s"));
        assert!(matches!(c.steer_head("s"), Submit::Steer { .. }));
        assert_eq!(c.snapshot("s").pending.iter().map(|p| p.turn_id.as_str()).collect::<Vec<_>>(), ["q2"]);
        c.steer_sent("s", "q1");
        let fx = c.finish_steer("s", "q1", Err(true));
        assert_eq!(fx.notices[0].title, "Steering queued");
        assert_eq!(
            c.snapshot("s").pending.iter().map(|p| p.turn_id.as_str()).collect::<Vec<_>>(),
            ["q1", "q2"],
            "restored in order, ahead of later prompts"
        );
        // A steer returned by turn/steer_dropped comes back too.
        assert!(matches!(c.steer_head("s"), Submit::Steer { .. }));
        c.steer_sent("s", "q1");
        let fx = c.steer_dropped("s", "owner", &["q1".to_owned()]);
        assert_eq!(fx.notices[0].title, "Steering returned to queue");
        assert_eq!(c.snapshot("s").pending[0].turn_id, "q1");
    }

    #[test]
    fn a_not_steered_receipt_adopts_cores_new_turn_without_a_second_start() {
        let c = Composer::default();
        c.enqueue("s", t("owner"));
        c.begin_dispatch("s", "owner");
        c.finish_dispatch("s", "owner", StartOutcome::Accepted);
        c.enqueue("s", t("q1"));
        c.steer_head("s");
        c.steer_sent("s", "q1");
        let fx = c.finish_steer("s", "q1", Ok(("new-turn".into(), false)));
        assert_eq!(fx.notices[0].title, "Native steering started a new turn");
        assert_eq!(c.snapshot("s").active.unwrap().turn_id, "new-turn");
        assert_eq!(fx.start, None);
        assert_eq!(c.ownership("s"), Ownership::LocalOwner);
    }

    // ---- use-turn-controller.test.ts:911 / :925 / :1038 (durable ownership)
    #[test]
    fn a_hydrated_active_turn_is_observed_never_owned() {
        let c = Composer::default();
        let fx = c.reconcile_hydrate("s", &[("server-turn".into(), Lifecycle::Active)], false);
        assert_eq!(fx, Effects::default());
        assert_eq!(c.ownership("s"), Ownership::Observed);
        assert_eq!(c.snapshot("s").active.unwrap().origin, Origin::Adopted);
        // A local prompt queues behind it instead of racing a second start.
        assert!(matches!(c.enqueue("s", t("mine")), Submit::Queued(_)));
        assert_eq!(c.begin_dispatch("s", "server-turn"), None, "an adopted turn is never dispatched");
        let fx = c.settle("s", "server-turn", true);
        assert_eq!(fx.start.map(|t| t.turn_id), Some("mine".into()));
    }

    #[test]
    fn the_accepted_owner_survives_a_preserving_hydrate_but_not_a_transport_change() {
        let c = Composer::default();
        c.enqueue("s", t("owner"));
        c.begin_dispatch("s", "owner");
        c.finish_dispatch("s", "owner", StartOutcome::Accepted);
        c.reconcile_hydrate("s", &[("owner".into(), Lifecycle::Active)], true);
        assert_eq!(c.ownership("s"), Ownership::LocalOwner, "same transport: still ours");
        assert!(c.suspend_transport().is_empty());
        assert_eq!(c.ownership("s"), Ownership::Observed, "a new transport only observes it");
        // An interrupting server turn keeps Stop de-duplicated after hydrate.
        c.reconcile_hydrate("s", &[("owner".into(), Lifecycle::Interrupting)], true);
        assert_eq!(c.interrupting_turn("s"), Some("owner".into()));
        assert!(!c.begin_interrupt("s", "owner"), "no second interrupt");
        // A hydrate proving the turn completed settles it.
        let fx = c.reconcile_hydrate("s", &[("owner".into(), Lifecycle::Completed)], true);
        assert_eq!(fx.start, None);
        assert_eq!(c.snapshot("s").active, None);
    }

    #[test]
    fn a_hydrate_that_omits_our_attempted_turn_asks_for_its_state() {
        let c = Composer::default();
        c.enqueue("s", t("owner"));
        c.begin_dispatch("s", "owner");
        c.finish_dispatch("s", "owner", StartOutcome::Accepted);
        let fx = c.reconcile_hydrate("s", &[], true);
        assert_eq!(fx.check_state, Some("owner".into()), "absence is not proof it never ran");
    }

    #[test]
    fn another_clients_turn_started_is_adopted_as_observed() {
        let c = Composer::default();
        c.observe_activity("s", "foreign", true);
        assert_eq!(c.ownership("s"), Ownership::Observed);
        assert_eq!(c.snapshot("s").active.unwrap().origin, Origin::Adopted);
        // Our own dispatch is never adopted as foreign.
        let c = Composer::default();
        c.enqueue("s", t("mine"));
        c.begin_dispatch("s", "mine");
        c.observe_activity("s", "mine", true);
        assert_eq!(c.ownership("s"), Ownership::LocalOwner);
    }

    #[test]
    fn an_interrupted_prompt_returns_on_its_own_terminal_only() {
        let c = Composer::default();
        c.enqueue("s", t("one"));
        c.begin_dispatch("s", "one");
        c.finish_dispatch("s", "one", StartOutcome::Accepted);
        assert!(c.begin_interrupt("s", "one"));
        assert_eq!(c.settle("s", "other", false).restore, None);
        assert_eq!(c.settle("s", "one", false).restore, Some(("s".into(), "one".into())));
    }
}
