//! A7 — the turn controller over the store's composer domain
//! (`octoscode_store::domains::composer`): the web's
//! `createQueueBackedTurnController` (`use-turn-controller.ts`) on the
//! native [`Conversation`].
//!
//! Admission (FIFO / steer), the ONE `turn/start` per admitted turn and its
//! classified outcome, the steer round trip, lifecycle recovery, and the
//! transport/hydrate reconcile. A child module of `flow`, so it reads the
//! conversation's private plumbing without widening its API.
use std::sync::Arc;
use std::time::Duration;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::TurnId;
use octoscode_client::ClientError;
use octoscode_store::domains::composer::{Effects, Lifecycle, PromptTurn, StartOutcome, Submit};

use super::{Conversation, Direction};
use crate::chrome::native_driver_id;
use crate::seat::{self, Plan};

impl Conversation {
    /// Attach the shared handle, so a terminal can start the next queued
    /// prompt (`settleTurn` -> `startTurn(next)`) on the runtime.
    pub fn attach(self: &Arc<Self>) {
        *self.weak_self.lock().unwrap() = Arc::downgrade(self);
    }

    /// The shared handle [`Conversation::attach`] set (`None` before it).
    pub fn shared(&self) -> Option<Arc<Self>> {
        self.weak_self.lock().unwrap().upgrade()
    }

    /// The `turn/start` acknowledgement budget (the web client's default
    /// `requestTimeoutMs: 30_000`, `packages/client/src/client.ts:199`);
    /// `OCTOSCODE_TURN_START_TIMEOUT_MS` overrides it for a fixture walk.
    pub fn start_timeout() -> Duration {
        let ms = std::env::var("OCTOSCODE_TURN_START_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30_000);
        Duration::from_millis(ms)
    }

    /// The composer's submission (`submitTurn` / `enqueuePrompt`): a fresh
    /// turn id, the Session's reasoning effort captured NOW, the attachment
    /// batch; then start it, queue it behind the active turn, or steer it.
    pub async fn submit_prompt(
        &self,
        text: String,
        media: Vec<crate::screens::media::TurnMedia>,
    ) -> Result<String, ClientError> {
        let session = self.session_id();
        let turn = PromptTurn {
            turn_id: TurnId::new().0.to_string(),
            text,
            reasoning_effort: crate::screens::board3::thinking::effort_param(&self.store, &session),
            media: media.iter().map(|m| m.to_value()).collect(),
            ..Default::default()
        };
        let admitted = self.store.domains.composer.submit(&session, turn);
        if !matches!(admitted, Submit::Refused) {
            // The composer clears when the prompt is admitted (queued or
            // started); the saved draft goes with it (`draft-recovery.spec.ts`:
            // "sent drafts stay cleared").
            self.ui.lock().unwrap().set_draft_inner(String::new());
            crate::drafts::save(&session, "");
        }
        makepad_widgets::SignalToUI::set_ui_signal();
        match admitted {
            Submit::Refused => {
                makepad_widgets::log!("[octoscode] submit refused by the turn controller (draft kept)");
                Ok(String::new())
            }
            Submit::StartNow(turn) => self.dispatch_turn(turn).await,
            Submit::Queued(turn) => {
                makepad_widgets::log!(
                    "[octoscode] prompt queued: {} ({} pending)",
                    turn.turn_id,
                    self.store.domains.composer.snapshot(&session).pending.len()
                );
                Ok(turn.turn_id)
            }
            Submit::Steer { turn, expected_turn_id } => {
                self.run_steer_in(&session, turn, expected_turn_id).await;
                Ok(String::new())
            }
        }
    }

    /// `enqueueTurn` (`use-turn-controller.ts`; `features/peers/gather.ts:
    /// 124-131`): a turn the app composes (the peer gather's synthesis) —
    /// FIFO behind the active turn, never steered, the composer's draft
    /// untouched — dispatched like any prompt, so it crosses the SAME seat
    /// gate (a held seat is handed back before its `turn/start`). `Ok("")`
    /// = the controller refused it (blocked).
    pub async fn enqueue_turn(&self, text: String) -> Result<String, ClientError> {
        let session = self.session_id();
        let turn = PromptTurn {
            turn_id: TurnId::new().0.to_string(),
            text,
            reasoning_effort: crate::screens::board3::thinking::effort_param(&self.store, &session),
            ..Default::default()
        };
        let admitted = self.store.domains.composer.enqueue(&session, turn);
        makepad_widgets::SignalToUI::set_ui_signal();
        match admitted {
            Submit::StartNow(turn) => self.dispatch_turn(turn).await,
            Submit::Queued(turn) => Ok(turn.turn_id),
            Submit::Refused | Submit::Steer { .. } => Ok(String::new()),
        }
    }

    /// `startTurn` (`use-turn-controller.ts:248-487`): the ONE `turn/start`
    /// for an admitted queue head, then its classified outcome — accepted,
    /// unconfirmed (held for recovery, never resent), a collision (the text
    /// comes back, the occupier is observed) or a rejection.
    pub async fn dispatch_turn(&self, turn: PromptTurn) -> Result<String, ClientError> {
        let session = self.session_id();
        self.dispatch_turn_in(&session, turn).await
    }

    /// A22 row 236 — `startTurn` for the queue of `session`, which may be a
    /// BACKGROUND record: its FIFO advances where it lives (a terminal
    /// advances a background queue while another Session is selected,
    /// `use-octos-session.ts:2711-2713`). Only the selected Session's turn
    /// becomes the foreground's live turn.
    pub async fn dispatch_turn_in(&self, session: &str, turn: PromptTurn) -> Result<String, ClientError> {
        let session = session.to_owned();
        let foreground = |me: &Self| me.session_id() == session;
        let Some(turn) = self.store.domains.composer.begin_dispatch(&session, &turn.turn_id) else {
            return Ok(String::new());
        };
        let turn_id = turn.turn_id.clone();
        let mut params = serde_json::json!({
            "session_id": session,
            "turn_id": turn_id,
            "input": [{"kind": "text", "text": turn.text}],
        });
        if let Some(effort) = &turn.reasoning_effort {
            params["reasoning_effort"] = serde_json::Value::String(effort.clone());
        }
        if !turn.media.is_empty() {
            params["media"] = serde_json::Value::Array(turn.media.clone());
        }
        self.trace
            .record(self.started, Direction::Out, "turn/start", None, Some(format!("turn={turn_id}")));
        // The optimistic row and the live turn, from the moment of dispatch
        // (`acceptLocalDispatch` / `addOptimisticUser`).
        self.store.domains.session.timeline.upsert_user_message(
            &session,
            &turn_id,
            &turn.text,
            serde_json::json!({"optimistic": true}),
        );
        if foreground(self) {
            self.ui.lock().unwrap().begin_turn_in(&session, &turn_id);
        }
        // A8 — a turn this client dispatched is its OWN: the strip shows its
        // live step, never "Another client is working in this session"
        // (`origin === "adopted"` is the web's other-client test).
        crate::flow::note_own_turn(&turn_id);
        makepad_widgets::SignalToUI::set_ui_signal();
        // §5.2: the prompt crosses the driver seam FIRST — the seat handover
        // (or its refusal) completes before any turn/start frame is written.
        let generation = self.store.domains.composer.generation();
        if let Err(message) = self.seat_gate(&session).await {
            self.ui.lock().unwrap().abandon_turn(&turn_id);
            let bounded = seat::bounded_turn_admission_error(&message);
            makepad_widgets::log!("[octoscode] turn/start {turn_id}: not sent ({bounded})");
            let fx = self.store.domains.composer.not_sent(&session, &turn_id, &bounded);
            self.apply_effects(&session, fx);
            return Ok(String::new());
        }
        if self.store.domains.composer.generation() != generation {
            // The transport changed during the handback: nothing was written,
            // so the head waits for the ready drain (never a blind resend).
            self.ui.lock().unwrap().abandon_turn(&turn_id);
            self.store.domains.composer.cancel_dispatch(&session, &turn_id);
            makepad_widgets::log!("[octoscode] turn/start {turn_id}: held (transport changed during the handback)");
            return Ok(String::new());
        }
        let reply = tokio::time::timeout(Self::start_timeout(), self.client.request("turn/start", params)).await;
        let outcome = match &reply {
            Err(_) => StartOutcome::Unconfirmed { timed_out: true },
            Ok(Ok(_)) => StartOutcome::Accepted,
            Ok(Err(e)) => match octoscode_client::domains::turn::turn_collision_from(e) {
                Some(occupier) => StartOutcome::Collision { occupier },
                None => match e {
                    // §6: a seat refusal is bounded to the human message.
                    ClientError::Rpc { error, .. } => {
                        StartOutcome::Rejected(seat::bounded_turn_admission_error(&error.message))
                    }
                    // A transport failure is not proof of a rejection.
                    _ => StartOutcome::Unconfirmed { timed_out: false },
                },
            },
        };
        makepad_widgets::log!("[octoscode] turn/start {turn_id}: {outcome:?}");
        if matches!(outcome, StartOutcome::Collision { .. } | StartOutcome::Rejected(_)) {
            // The turn never started: no live row, no working indicator.
            self.ui.lock().unwrap().abandon_turn(&turn_id);
        }
        let fx = self.store.domains.composer.finish_dispatch(&session, &turn_id, outcome.clone());
        self.apply_effects(&session, fx);
        if let Ok(Err(ClientError::Rpc { error, .. })) = &reply {
            if seat::is_external_master_held(&error.message) {
                // The server says another app holds the seat: re-read the
                // record so the held banner (and its Take over) appears.
                self.refresh_seat(&session).await;
            }
        }
        match (outcome, reply) {
            (StartOutcome::Rejected(_), Ok(Err(e))) => Err(e),
            (StartOutcome::Collision { .. }, _) => Ok(String::new()),
            _ => Ok(turn_id),
        }
    }

    /// §5.2 send gate (`releaseControlSeatForUserTurn`): plan the ONE seat
    /// handover. A proven own seat is released (`next: internal`) and then
    /// the turn may go; a foreign / parked holder or an unproven own lease
    /// refuses with no frame (`Err` = the message, bounded by the caller).
    /// A refusal is decided on a fresh read of the record.
    async fn seat_gate(&self, session: &str) -> Result<(), String> {
        seat::set_status(session, None);
        let own = seat::proof(session);
        let me = native_driver_id();
        let mut plan = seat::plan(own.as_ref(), seat::observed(session).as_ref(), &me, seat::now_ms());
        if matches!(plan, Plan::WaitForExpiry(_) | Plan::ResumeChat { .. }) {
            self.refresh_seat(session).await;
            plan = seat::plan(own.as_ref(), seat::observed(session).as_ref(), &me, seat::now_ms());
        }
        match plan {
            Plan::Send => Ok(()),
            Plan::ReleaseThenSend(proof) => {
                seat::set_status(session, Some((seat::HANDING_BACK_CONTROL_STATUS, false)));
                makepad_widgets::SignalToUI::set_ui_signal();
                let released = self.hand_back(session, &proof).await;
                seat::set_status(session, None);
                makepad_widgets::SignalToUI::set_ui_signal();
                if released {
                    // `refreshDriverInventory` before the send: the console's
                    // next CAS and the pane's disclosure follow the handback.
                    if crate::screens::fleet_driver::peer_control_admitted(&self.store) {
                        let _ = crate::screens::fleet_driver::load_inventory(self).await;
                        crate::screens::board3::session_pane::mirror_inventory(&self.store);
                    }
                    Ok(())
                } else {
                    Err(seat::RELEASE_FAILED_MESSAGE.to_owned())
                }
            }
            other => Err(seat::refusal(&other).unwrap_or_default()),
        }
    }

    /// One `session/driver/release {next: "internal"}` with the kept proof.
    /// A lost or refused reply is reconciled from the OBSERVED record (a
    /// timeout may mean the release landed) — never inferred from a kept id.
    async fn hand_back(&self, session: &str, proof: &seat::Proof) -> bool {
        let reply = self.client.request("session/driver/release", seat::release_params(session, proof)).await;
        let mut released = matches!(&reply, Ok(v) if seat::release_confirmed(v));
        if !released {
            if let Err(e) = &reply {
                // A typed stale fence: the proof is dead — the held seat is
                // dropped, never retried with it (`settleControlState`).
                let kind = octoscode_client::domains::external_driver::typed_refusal(e);
                crate::screens::fleet_driver::note_refusal(session, kind.as_deref());
            }
            self.refresh_seat(session).await;
            released = seat::observed(session).is_some_and(|d| !d.external);
        }
        if released {
            seat::drop_proof(session);
            seat::observe(session, Some(seat::Disclosure { external: false, binding: None }));
            crate::chrome::set_held(session, None);
            // `parkControlSeat(record)`: the console's seat is handed back too.
            crate::screens::fleet_driver::handed_back(session);
        }
        makepad_widgets::log!(
            "[octoscode] seat: hand back {session}: {}",
            if released { "released (internal)" } else { "refused, nothing sent" }
        );
        released
    }

    /// Read the Session's driver record (`session/driver/get`, when the
    /// server advertises it): the held banner's foreign holder (board 12,
    /// `chrome::foreign_holder`) and the seat plan's observation.
    pub async fn refresh_seat(&self, session: &str) {
        let advertised = self
            .store
            .domains
            .config
            .supported_methods()
            .iter()
            .any(|m| m == "session/driver/get");
        if !advertised {
            return;
        }
        let params = serde_json::json!({ "session_id": session });
        match self.client.request("session/driver/get", params).await {
            Ok(v) => {
                let held = crate::chrome::foreign_holder(&v, &native_driver_id());
                makepad_widgets::log!("[octoscode] driver/get {session}: held={held:?}");
                crate::chrome::set_held(session, held);
                seat::observe(session, seat::parse_disclosure(&v));
            }
            Err(e) => makepad_widgets::log!("[octoscode] driver/get {session}: {e}"),
        }
        makepad_widgets::SignalToUI::set_ui_signal();
    }

    /// The held banner's Take over — the web's Resume chat (`resumeChatSend`,
    /// §5.2 case 3): acquire on the OBSERVED revision → release(`internal`)
    /// with that one proof → send the composer's draft ONCE through the
    /// ordinary submit. Nothing is sent on any refused step; the draft stays.
    pub async fn resume_chat(&self) {
        let session = self.session_id();
        self.resume_chat_in(&session).await
    }

    /// A22 audit — Take over for `session`, the Session whose banner was
    /// tapped (the window may have moved on before this runs): the seat
    /// steps act on THAT Session, and the composer's draft is sent only
    /// while that Session is still on screen — else the composer holds
    /// another Session's draft, which never goes into this one.
    pub async fn resume_chat_in(&self, session: &str) {
        let session = session.to_owned();
        let revision = |s: &str| {
            seat::observed(s)
                .filter(|d| d.external)
                .and_then(|d| d.binding)
                .map(|b| b.revision)
        };
        if revision(&session).is_none() {
            self.refresh_seat(&session).await;
        }
        let Some(expected) = revision(&session) else {
            makepad_widgets::log!("[octoscode] seat: resume chat: nothing holds {session}");
            return;
        };
        seat::set_status(&session, Some((seat::RESUMING_CHAT_STATUS, false)));
        makepad_widgets::SignalToUI::set_ui_signal();
        // Board 12's acquire (`chrome::take_over_params`: our driver id, a
        // 60 s lease), CAS on the revision this record was OBSERVED at.
        crate::screens::fleet_driver::acquiring_driver_id(); // persisted before a lease is taken under it
        let mut params = crate::chrome::take_over_params(&session);
        params["expected_revision"] = serde_json::Value::from(expected);
        let proof = match self.client.request("session/driver/acquire", params).await {
            Ok(v) => seat::parse_acquire(&v, &native_driver_id()),
            Err(e) => {
                makepad_widgets::log!("[octoscode] seat: acquire refused: {e}");
                None
            }
        };
        let Some(proof) = proof else {
            seat::set_status(&session, Some((seat::RELEASE_FAILED_MESSAGE, true)));
            self.refresh_seat(&session).await;
            return;
        };
        seat::keep_proof(&session, proof.clone());
        crate::chrome::set_held(&session, None);
        if !self.hand_back(&session, &proof).await {
            seat::set_status(&session, Some((seat::RELEASE_FAILED_MESSAGE, true)));
            makepad_widgets::SignalToUI::set_ui_signal();
            return;
        }
        seat::set_status(&session, None);
        makepad_widgets::SignalToUI::set_ui_signal();
        if self.session_id() != session {
            makepad_widgets::log!(
                "[octoscode] seat: took over {session}; the window moved on — its draft is not sent from another Session"
            );
            return;
        }
        if self.ui.lock().unwrap().draft().trim().is_empty() {
            return;
        }
        if let Err(e) = self.submit_draft().await {
            makepad_widgets::log!("[octoscode] seat: resume chat send: {e}");
        }
    }

    /// The `turn/steer` round trip for an admitted steer (`submitTurn`,
    /// `use-turn-controller.ts:731-822`): `{session_id, expected_turn_id,
    /// input}`; the receipt's `{turn_id, steered}` decides.
    pub async fn run_steer(&self, turn: PromptTurn, expected: String) {
        let session = self.session_id();
        self.run_steer_in(&session, turn, expected).await
    }

    /// A22 — the steer round trip for `session`, the Session the steer was
    /// admitted in (the window may have moved on). `expected` must still be
    /// a live turn OF that Session; else nothing is sent and the steer goes
    /// back to its queue, in order (a refused steer).
    pub async fn run_steer_in(&self, session: &str, turn: PromptTurn, expected: String) {
        let session = session.to_owned();
        if !self.is_live_turn_of(&session, &expected) {
            makepad_widgets::log!("[octoscode] turn/steer not sent: {expected} is not a live turn of {session}");
            let fx = self.store.domains.composer.finish_steer(&session, &turn.turn_id, Err(true));
            self.apply_effects(&session, fx);
            return;
        }
        if !self.store.domains.composer.steer_sent(&session, &turn.turn_id) {
            let fx = self.store.domains.composer.finish_steer(&session, &turn.turn_id, Err(true));
            self.apply_effects(&session, fx);
            return;
        }
        let params = serde_json::json!({
            "session_id": session,
            "expected_turn_id": expected,
            "input": [{"kind": "text", "text": turn.text}],
        });
        self.trace
            .record(self.started, Direction::Out, "turn/steer", None, Some(format!("into={expected}")));
        let reply = tokio::time::timeout(Self::start_timeout(), self.client.request("turn/steer", params)).await;
        let result = match reply {
            Ok(Ok(v)) => {
                let receipt = v.get("turn_id").and_then(|t| t.as_str()).map(str::to_owned);
                let steered = v.get("steered").and_then(|b| b.as_bool());
                match (receipt, steered) {
                    // A steered receipt must name the captured turn.
                    (Some(t), Some(true)) if t == expected => Ok((t, true)),
                    (Some(t), Some(false)) => Ok((t, false)),
                    // An invalid receipt after sending: the outcome is unknown.
                    _ => Err(false),
                }
            }
            Ok(Err(ClientError::Rpc { .. })) => Err(true),
            _ => Err(false),
        };
        makepad_widgets::log!("[octoscode] turn/steer {}: {result:?}", turn.turn_id);
        let fx = self.store.domains.composer.finish_steer(&session, &turn.turn_id, result);
        self.apply_effects(&session, fx);
    }

    /// The queued chip's "Steer now" (conversation-08: `1 queued · Steer now
    /// · ✕`): steer the queue head into the accepted active turn.
    pub async fn steer_queued_head(&self) -> bool {
        let session = self.session_id();
        self.steer_queued_head_in(&session).await
    }

    /// A22 — the queued chip's "Steer now" for `session`, the Session whose
    /// chip was tapped (the window may have moved on since).
    pub async fn steer_queued_head_in(&self, session: &str) -> bool {
        let session = session.to_owned();
        if !self.can_steer() {
            makepad_widgets::log!("[octoscode] steer now: turn/steer is not offered by this server");
            return false;
        }
        match self.store.domains.composer.steer_head(&session) {
            Submit::Steer { turn, expected_turn_id } => {
                makepad_widgets::SignalToUI::set_ui_signal();
                self.run_steer_in(&session, turn, expected_turn_id).await;
                true
            }
            _ => false,
        }
    }

    /// Whether this server offers safe steering (`App.tsx:2849-2858`: the
    /// `turn/steer` method AND `event.turn_steer_dropped.v1`).
    pub fn can_steer(&self) -> bool {
        let methods = self.store.domains.config.supported_methods();
        let features = self.store.domains.config.supported_features();
        methods.iter().any(|m| m == "turn/steer") && features.iter().any(|f| f == "event.turn_steer_dropped.v1")
    }

    /// `/steer [on|off]`: flip / set the Session's steering opt-in and say
    /// what it means here (`App.tsx:2849-2858`'s field note copy).
    pub(super) fn steer_command(&self, args: &str) -> String {
        let session = self.session_id();
        let choice = args.trim().to_ascii_lowercase();
        let value = match choice.as_str() {
            "" => Some(!self.store.domains.composer.steering_enabled(&session)),
            "on" | "true" | "enable" | "enabled" => Some(true),
            "off" | "false" | "disable" | "disabled" => Some(false),
            _ => None,
        };
        let text = match value {
            None => "Unsupported command: /steer — Use /steer [on | off]. Nothing was sent to the model.".to_owned(),
            Some(on) => {
                self.store.domains.composer.set_steering(&session, on);
                self.ui.lock().unwrap().set_draft_inner(String::new());
                crate::drafts::save(&session, "");
                match (on, self.can_steer()) {
                    (false, _) => {
                        "Steering off. Mid-turn text is queued and sent in order after the current response."
                            .to_owned()
                    }
                    (true, true) => "Steering enabled for this Session. Eligible mid-turn text is sent to the active turn; other inputs remain queued.".to_owned(),
                    (true, false) => {
                        "Steering enabled, but safe steering is unavailable on this server. Inputs remain queued."
                            .to_owned()
                    }
                }
            }
        };
        self.store.domains.session.timeline.append(
            &session,
            Some(crate::screens::palette::next_receipt_turn()),
            crate::screens::palette::REPORT_KIND,
            text.clone(),
        );
        makepad_widgets::SignalToUI::set_ui_signal();
        text
    }

    /// `cancelQueuedPrompt`: remove a not-yet-dispatched prompt; the active
    /// turn and the server are untouched.
    pub fn remove_queued(&self, turn_id: &str) -> bool {
        let session = self.session_id();
        self.remove_queued_in(&session, turn_id)
    }

    /// A22 — the queued chip's ✕ for `session`, the Session whose chip was
    /// tapped: removes its own pending prompt only.
    pub fn remove_queued_in(&self, session: &str, turn_id: &str) -> bool {
        let removed = self.store.domains.composer.remove_pending(session, turn_id);
        makepad_widgets::SignalToUI::set_ui_signal();
        removed
    }

    /// "Check status" (`retryTurnRecovery`, `:930-988`): `turn/state/get`
    /// for the held turn, the answer validated against the session and turn.
    pub async fn check_turn_state(&self) {
        let session = self.session_id();
        self.check_turn_state_in(&session).await
    }

    /// A22 — "Check status" for `session`, the Session whose recovery notice
    /// was tapped (its own held turn; the window may have moved on).
    pub async fn check_turn_state_in(&self, session: &str) {
        let session = session.to_owned();
        let advertised = self
            .store
            .domains
            .config
            .supported_methods()
            .iter()
            .any(|m| m == "turn/state/get");
        let Some(turn_id) = self.store.domains.composer.begin_recovery_check(&session, advertised) else {
            makepad_widgets::SignalToUI::set_ui_signal();
            return;
        };
        makepad_widgets::SignalToUI::set_ui_signal();
        let params = serde_json::json!({"session_id": session, "turn_id": turn_id});
        let state = match tokio::time::timeout(Self::start_timeout(), self.client.request("turn/state/get", params)).await
        {
            Ok(Ok(v)) => {
                let same = v.get("session_id").and_then(|x| x.as_str()) == Some(session.as_str())
                    && v.get("turn_id").and_then(|x| x.as_str()) == Some(turn_id.as_str());
                match v.get("state").and_then(|x| x.as_str()).and_then(Lifecycle::parse) {
                    Some(st) if same => Ok(st),
                    _ => Err("The server returned status for a different or invalid turn.".to_owned()),
                }
            }
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err("The status check timed out.".to_owned()),
        };
        makepad_widgets::log!("[octoscode] turn/state/get {turn_id}: {state:?}");
        if let Ok(st) = &state {
            if !matches!(st, Lifecycle::Active | Lifecycle::Interrupting | Lifecycle::Unknown) {
                // The lookup proved a terminal: the live row settles too.
                self.ui.lock().unwrap().end_turn(&turn_id, *st == Lifecycle::Completed);
                self.store.domains.turn.set_terminal(
                    &turn_id,
                    match st {
                        Lifecycle::Completed => "completed",
                        Lifecycle::Interrupted => "interrupted",
                        _ => "errored",
                    },
                );
            }
        }
        let fx = self.store.domains.composer.finish_recovery_check(&session, &turn_id, state);
        self.apply_effects(&session, fx);
    }

    /// "Continue without it" (`continueWithoutTurn`): release the local wait;
    /// nothing is stopped or resent; queued prompts send next.
    pub fn continue_without_turn(&self) {
        let session = self.session_id();
        self.continue_without_turn_in(&session)
    }

    /// A22 — "Continue without it" for `session`, the Session whose recovery
    /// notice was tapped: its own held turn is released, its own queue sends.
    pub fn continue_without_turn_in(&self, session: &str) {
        let session = session.to_owned();
        let held = self.store.domains.composer.snapshot(&session).active.map(|a| a.turn_id);
        let fx = self.store.domains.composer.continue_without(&session);
        if let Some(t) = held {
            self.ui.lock().unwrap().abandon_turn(&t);
        }
        self.apply_effects(&session, fx);
    }

    /// Apply a controller transition: the notices into the transcript (one
    /// row per key), text handed back to its composer, the next head started,
    /// a lifecycle check asked.
    pub fn apply_effects(&self, session: &str, fx: Effects) {
        for n in fx.notices {
            self.store.domains.session.timeline.upsert_notice(
                session,
                Some(n.turn_id.clone()),
                &n.key,
                &n.title,
                &n.body,
                n.tone,
            );
        }
        // A22 row 216 — what comes back is parked on its OWN record, in
        // order: a turn that never started WHOLE (text, effort, images), an
        // interrupted prompt as text (`restoreUnsentTurn` /
        // `restoreInterruptPrompt`, session-composer-drafts.ts:121-143).
        match (fx.returned, fx.restore) {
            (Some((owner, turn)), _) => {
                crate::screens::composer_drafts::restore_unsent_turn(self, &owner, &turn);
                self.restore_text(&owner);
            }
            (None, Some((owner, text))) => {
                crate::screens::composer_drafts::restore_interrupt_prompt(self, &owner, &text);
                self.restore_text(&owner);
            }
            (None, None) => {}
        }
        if let Some(turn) = fx.start {
            self.spawn_dispatch(session, turn);
        }
        if fx.check_state.is_some() {
            self.spawn_check(session);
        }
        makepad_widgets::SignalToUI::set_ui_signal();
    }

    /// `onTurnNotSentRestore` / the interrupt restore: the text returns to the
    /// owning Session's composer when it is empty, else it waits there
    /// (`restoreUnsentTurn`: "kept for retry and will return when the composer
    /// is empty").
    fn restore_text(&self, session: &str) {
        // A22 row 216 — the composer of the owning Session takes it back
        // now when it is empty (`consumeRestore`: never over new images);
        // else it waits on the record and the host's composer sync takes it
        // when the composer empties (lib.rs `sync_composer_extras`).
        // The composer lock is held across the take, so a keystroke cannot
        // land between the check and the restore (`consume_restore` never
        // takes it — the host's sync holds it the same way).
        let mut ui = self.ui.lock().unwrap();
        if self.session_id() != session || !ui.draft().trim().is_empty() {
            return;
        }
        if let Some(text) = crate::screens::composer_drafts::consume_restore(self, session) {
            ui.set_draft_inner(text);
        }
    }

    fn spawn_dispatch(&self, session: &str, turn: PromptTurn) {
        let me = self.weak_self.lock().unwrap().upgrade();
        let session = session.to_owned();
        match (me, tokio::runtime::Handle::try_current()) {
            (Some(me), Ok(handle)) => {
                handle.spawn(async move {
                    if let Err(e) = me.dispatch_turn_in(&session, turn).await {
                        makepad_widgets::log!("[octoscode] queued turn: {e}");
                    }
                });
            }
            _ => self.pending_starts.lock().unwrap().push((session, turn)),
        }
    }

    fn spawn_check(&self, session: &str) {
        let me = self.weak_self.lock().unwrap().upgrade();
        let session = session.to_owned();
        if let (Some(me), Ok(handle)) = (me, tokio::runtime::Handle::try_current()) {
            handle.spawn(async move { me.check_turn_state_in(&session).await });
        }
    }

    /// Start whatever a transition queued while no shared handle was
    /// attached (a test drives this; production spawns directly).
    pub async fn pump(&self) {
        let pending: Vec<_> = std::mem::take(&mut *self.pending_starts.lock().unwrap());
        for (session, turn) in pending {
            let _ = self.dispatch_turn_in(&session, turn).await;
        }
    }

    /// The controller's view of one notification for the Session it names:
    /// server activity proves an unacknowledged start (and adopts another
    /// client's `turn/started`), a terminal settles the FIFO, returned
    /// steering goes back to the queue.
    pub(super) fn composer_observe(&self, n: &UiNotification) {
        use octos_core::ui_protocol::{PayloadV2, TurnTerminalOutcome};
        let active = self.session_id();
        let pick = |sid: &str| if sid.is_empty() { active.clone() } else { sid.to_owned() };
        let composer = &self.store.domains.composer;
        let (session, fx) = match n {
            UiNotification::TurnStarted(e) => {
                let s = pick(&e.session_id.0);
                let fx = composer.observe_activity(&s, &e.turn_id.0.to_string(), true);
                (s, fx)
            }
            UiNotification::MessageDelta(e) => {
                let s = pick(&e.session_id.0);
                let fx = composer.observe_activity(&s, &e.turn_id.0.to_string(), false);
                (s, fx)
            }
            UiNotification::TurnCompleted(e) => {
                let s = pick(&e.session_id.0);
                let fx = composer.settle(&s, &e.turn_id.0.to_string(), true);
                (s, fx)
            }
            UiNotification::TurnError(e) => {
                let s = pick(&e.session_id.0);
                let fx = composer.settle(&s, &e.turn_id.0.to_string(), false);
                (s, fx)
            }
            UiNotification::TurnSteerDropped(e) => {
                let s = pick(&e.session_id.0);
                let fx = composer.steer_dropped(&s, &e.turn_id.0.to_string(), &e.inputs);
                (s, fx)
            }
            UiNotification::EnvelopeV2(frame) => {
                let s = pick(&frame.session_id.0);
                let turn_id = frame.envelope.turn_id.clone();
                let fx = match &frame.envelope.payload {
                    PayloadV2::TurnTerminal { outcome, .. } => {
                        composer.settle(&s, &turn_id, matches!(outcome, TurnTerminalOutcome::Completed))
                    }
                    PayloadV2::AssistantDelta { .. } | PayloadV2::ToolStart { .. } | PayloadV2::ToolEnd { .. } => {
                        composer.observe_activity(&s, &turn_id, false)
                    }
                    _ => return,
                };
                (s, fx)
            }
            _ => return,
        };
        self.apply_effects(&session, fx);
    }

    /// A transport drop suspends the generation (leases stop counting,
    /// unsent steers go back to the queue); the next Live reconciles the
    /// bound Session from a canonical hydrate WITH turns
    /// (`reconcileFromHydrate`): a hydrated active turn is observed, a turn
    /// the hydrate proves finished settles, an absent one is checked.
    pub(super) fn note_connection(&self, live: bool, dropped: bool) {
        if !live && !dropped {
            // Handshaking / replaying: neither a drop nor live yet.
            return;
        }
        let was = std::mem::replace(&mut *self.was_live.lock().unwrap(), live);
        if was && dropped {
            *self.transport_suspended.lock().unwrap() = true;
            for (session, fx) in self.store.domains.composer.suspend_transport() {
                self.apply_effects(&session, fx);
            }
            return;
        }
        if live && !was && std::mem::replace(&mut *self.transport_suspended.lock().unwrap(), false) {
            let session = self.session_id();
            if self.store.domains.composer.snapshot(&session).active.is_none() {
                return;
            }
            let me = self.weak_self.lock().unwrap().upgrade();
            if let (Some(me), Ok(handle)) = (me, tokio::runtime::Handle::try_current()) {
                handle.spawn(async move { me.reconcile_from_hydrate(true).await });
            }
        }
    }

    /// `session/hydrate {include: [messages, turns]}` for the bound Session,
    /// then the controller's reconcile.
    pub async fn reconcile_from_hydrate(&self, preserve: bool) {
        let session = self.session_id();
        let params = serde_json::json!({"session_id": session, "include": ["messages", "turns"]});
        match self.client.request("session/hydrate", params).await {
            Ok(v) => {
                if v.get("session_id").and_then(|x| x.as_str()) != Some(session.as_str()) {
                    makepad_widgets::log!("[octoscode] reconcile: hydrate answered another Session");
                    return;
                }
                let turns = hydrated_turns(&v);
                makepad_widgets::log!("[octoscode] reconcile: {} hydrated turns", turns.len());
                let fx = self.store.domains.composer.reconcile_hydrate(&session, &turns, preserve);
                self.apply_effects(&session, fx);
            }
            Err(e) => makepad_widgets::log!("[octoscode] reconcile hydrate: {e}"),
        }
    }
}

/// `(turn_id, state)` from a hydrate reply's `turns` (`HydratedTurn`).
pub fn hydrated_turns(v: &serde_json::Value) -> Vec<(String, Lifecycle)> {
    v.get("turns")
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    let id = t.get("turn_id")?.as_str()?.to_owned();
                    let st = Lifecycle::parse(t.get("state")?.as_str()?)?;
                    Some((id, st))
                })
                .collect()
        })
        .unwrap_or_default()
}
