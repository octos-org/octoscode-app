//! A6 — the conversation pane's SURFACES: the approval and user-question
//! takeovers (Gate-B `conversation-05` / `-06`), the plan checklist card and
//! the Trajectory tab with its task detail (`conversation-10`, the web's
//! supervision feature), and the transcript's fold-all / view-state rules.
//!
//! One owner for every `cv.*` action id ([`routes`]); the host (`lib.rs`)
//! mounts the four lowerings into their slots — the takeover and the plan
//! card in the composer dock, the Trajectory pane in the conversation
//! column, the task detail in its own top dock — and routes their wired taps
//! (`taps::wired_taps`, the shared #FX1 path) back here. Transport work runs
//! through [`run`] on the host's runtime, always the production client.
pub mod folds;
pub mod host;
pub mod plan;
pub mod takeover;
pub mod trajectory;
pub mod view;

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use octoscode_store::Store;
use serde_json::json;

use crate::conv_layout::{Density, Metrics};
use crate::flow::FlowUi;
use crate::screens::board3::ui::{Dsl, Frame};

/// The conversation pane's view (`conversationTab`, `App.tsx:654`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Chat,
    Trajectory,
}

/// Which takeover owns the composer now (`App.tsx:2759-2826`: an approval
/// first, then a question).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Takeover {
    Approval(String),
    Question(String),
}

/// All the surfaces' UI state (what the protocol never carries).
#[derive(Debug, Clone, Default)]
pub struct State {
    pub tab: Tab,
    pub frame: Option<Frame>,
    pub approval: takeover::ApprovalUi,
    pub question: takeover::QuestionUi,
    pub plan: plan::PlanUi,
    pub view: view::ViewState,
    /// (session, entry count) the fold memory was last pruned against.
    pub folds_seen: Option<(String, usize)>,
}

/// Whether the active session's transcript changed shape since the last
/// prune (a session switch, or entries added/removed): the cheap trigger for
/// `folds::prune`.
pub fn folds_changed(store: &Store) -> bool {
    let Some(session) = store.active_session() else { return false };
    let n = store.domains.session.timeline.len(&session);
    let mut st = state();
    let now = Some((session, n));
    if st.folds_seen == now {
        return false;
    }
    st.folds_seen = now;
    true
}

/// A row the person just opened (timeline item `item_id`): the next draws
/// reveal its grown body ([`view::reveal_delta`]).
pub fn reveal_item(item_id: usize) {
    state().view.reveal = Some((format!("item:{item_id}"), 3));
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static TRAJ: OnceLock<Mutex<trajectory::TrajState>> = OnceLock::new();

/// The surfaces' state behind its one lock.
pub fn state() -> MutexGuard<'static, State> {
    STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// The Trajectory's state (its own lock: the loaders hold it across no
/// await, but they run on the runtime while the UI reads the main state).
pub fn traj() -> &'static Mutex<trajectory::TrajState> {
    TRAJ.get_or_init(|| Mutex::new(trajectory::TrajState::default()))
}

/// Test seam.
pub fn reset() {
    *state() = State::default();
    *traj().lock().unwrap_or_else(|p| p.into_inner()) = trajectory::TrajState::default();
}

/// Wake the UI thread (a job folded something a surface shows).
pub fn wake() {
    makepad_widgets::SignalToUI::set_ui_signal();
}

/// The host reports the module's laid-out size (the dialog frame).
pub fn set_frame(w: f64, h: f64) {
    if w > 0.0 && h > 0.0 {
        state().frame = Some(Frame { avail_w: w, avail_h: h });
    }
}

fn frame() -> Frame {
    state().frame.unwrap_or(Frame::DESKTOP)
}

/// Whether `action` is one of these surfaces' ids (one owner).
pub fn routes(action: &str) -> bool {
    action.starts_with("cv.")
}

// ------------------------------------------------------------- the takeover

/// The takeover the ACTIVE session shows, if any. Approvals win
/// (`App.tsx:2759`); a question needs the advertised method and feature
/// (`session-interaction-ledger.ts:254-262`) and a parseable payload. A20
/// (parity row 250): only interactions whose recorded origin IS the Session
/// on screen — another Session's approval or question never takes it over,
/// and each Session keeps its own question (one global slot let a second
/// Session's question replace the first's).
pub fn takeover(store: &Store) -> Option<Takeover> {
    let session = store.active_session()?;
    if let Some((p, _)) = store.domains.approval.showing(&session) {
        return Some(Takeover::Approval(p.id));
    }
    let q = store.domains.approval.question_for(&session)?;
    if !question_supported(store) {
        return None;
    }
    takeover::parse_questions(&q.questions).filter(|qs| !qs.is_empty())?;
    Some(Takeover::Question(q.question_id))
}

/// A20 — the active Session's question (the takeover's, the keys', the
/// submit's: one source).
fn active_question(store: &Store) -> Option<octoscode_store::domains::approval::PendingQuestion> {
    let session = store.active_session()?;
    store.domains.approval.question_for(&session)
}

/// A20 — whether the connection can carry a response now (the web's
/// `authority.ready`: connected, not recovering).
fn ready(store: &Store) -> bool {
    store.is_live() && store.outage().is_none()
}

/// `supportsMethod(USER_QUESTION_RESPOND) && supportsFeature(USER_QUESTION_V1)`.
pub fn question_supported(store: &Store) -> bool {
    store.domains.config.supported_methods().iter().any(|m| m == "user_question/respond")
        && store.domains.config.has_capability("user_question.v1")
}

/// The takeover card's look from the live conversation geometry.
pub fn look(m: &Metrics) -> takeover::Look {
    let f = frame();
    takeover::Look {
        phone: m.density == Density::Phone,
        width: m.composer_w,
        max_h: (f.avail_h - 120.0).clamp(240.0, 620.0),
    }
}

/// What the host mounts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lowered {
    pub dsl: String,
    pub taps: Vec<(String, String)>,
    pub inputs: Vec<(String, String)>,
}

impl Lowered {
    fn from(d: Dsl) -> Self {
        let taps = d.taps.clone();
        let inputs = d.inputs.clone();
        Lowered { dsl: crate::screens::theme::retint_dsl(&d.finish()), taps, inputs }
    }
}

/// Lower the active session's takeover card (`None` = the composer shows).
pub fn lower_takeover(store: &Store, m: &Metrics) -> Option<Lowered> {
    let t = takeover(store)?;
    let session = store.active_session()?;
    let look = look(m);
    let mut d = Dsl::new();
    match t {
        Takeover::Approval(_) => {
            let (p, a) = store.domains.approval.showing(&session)?;
            let ui = state().approval.clone();
            takeover::approval_card(&mut d, &p, &a, &ui, &look);
        }
        Takeover::Question(_) => {
            let q = store.domains.approval.question_for(&session)?;
            let qs = takeover::parse_questions(&q.questions)?;
            let mut s = state();
            s.question.bind(&q, qs.len());
            takeover::question_card(&mut d, &q, &qs, &s.question, &look);
            let lowered = Lowered::from(d);
            // Typing never remounts (the card lowers the LAST-MOUNTED text).
            // When anything else changed — a selection, sending, an error,
            // the focus ring — the card remounts anyway, so it carries the
            // typed draft: a failed send keeps the text and the selections
            // (`final-input.spec.ts`: "long questions retain free text and
            // selections after a failed response").
            let draft: Vec<String> = s.question.answers.iter().map(|a| a.free_text.clone()).collect();
            let remounts = s.question.last_dsl.as_deref().is_some_and(|last| last != lowered.dsl);
            if remounts && s.question.free_snap != draft {
                s.question.free_snap = draft;
                let mut d2 = Dsl::new();
                takeover::question_card(&mut d2, &q, &qs, &s.question, &look);
                let carried = Lowered::from(d2);
                s.question.last_dsl = Some(carried.dsl.clone());
                return Some(carried);
            }
            s.question.last_dsl = Some(lowered.dsl.clone());
            return Some(lowered);
        }
    }
    Some(Lowered::from(d))
}

/// The live (post-mount) visibility the typed answers drive without a
/// remount: the question's live/disabled submit pair and its reason line.
pub fn live_visibility(store: &Store) -> Vec<(String, bool)> {
    match takeover(store) {
        Some(Takeover::Question(_)) => {
            let st = state();
            let blocked = takeover::submit_blocked_reason(st.question.busy, &st.question.answers);
            vec![
                ("cv_q_submit_box".into(), blocked.is_none()),
                ("cv_q_submit_off_box".into(), blocked.is_some()),
                ("cv_q_reason".into(), blocked.is_some()),
            ]
        }
        _ => Vec::new(),
    }
}

/// The reason line's live text (it changes with the draft, no remount).
pub fn live_reason(store: &Store) -> Option<String> {
    match takeover(store) {
        Some(Takeover::Question(_)) => {
            let st = state();
            takeover::submit_blocked_reason(st.question.busy, &st.question.answers).map(str::to_owned)
        }
        _ => None,
    }
}

// --------------------------------------------------------------- the plan

/// Lower the plan card (`None` = no card: no feature, no plan, an empty
/// checklist, or a takeover owns the composer, `App.tsx:2657-2667`).
pub fn lower_plan(store: &Store, m: &Metrics) -> Option<String> {
    if takeover(store).is_some() || tab() == Tab::Trajectory {
        return None;
    }
    let plan = plan::visible_plan(store)?;
    let ui = state().plan.clone();
    let mut d = Dsl::new();
    plan::card(&mut d, &plan, &ui, m.composer_w, m.density == Density::Phone, crate::screens::board3::ui::now_ms() as i64);
    Some(crate::screens::theme::retint_dsl(&d.finish()))
}

// --------------------------------------------------------- the trajectory

pub fn tab() -> Tab {
    state().tab
}

/// Whether the header shows the Chat | Trajectory tabs.
pub fn tabs_available(store: &Store) -> bool {
    store.is_live() && store.active_session().is_some() && trajectory::Avail::of(store).any()
}

/// The pane the column shows: the Trajectory only while available (the
/// web falls back to Chat when the surfaces disappear, `App.tsx:992-1006`).
pub fn showing_trajectory(store: &Store) -> bool {
    if tab() != Tab::Trajectory {
        return false;
    }
    if !tabs_available(store) {
        state().tab = Tab::Chat;
        return false;
    }
    true
}

pub fn lower_trajectory(store: &Store, pane_w: f64, phone: bool) -> Option<Lowered> {
    if !showing_trajectory(store) {
        return None;
    }
    let st = traj().lock().unwrap_or_else(|p| p.into_inner()).clone();
    let mut d = Dsl::new();
    trajectory::pane(&mut d, store, &st, pane_w, phone);
    Some(Lowered::from(d))
}

pub fn detail_open() -> bool {
    traj().lock().unwrap_or_else(|p| p.into_inner()).detail.active
}

pub fn lower_detail(store: &Store) -> Option<Lowered> {
    let st = traj().lock().unwrap_or_else(|p| p.into_inner()).clone();
    if !st.detail.active {
        return None;
    }
    let mut d = Dsl::new();
    trajectory::detail_dialog(&mut d, store, &st, &frame());
    Some(Lowered::from(d))
}

/// A session switch refreshes the Trajectory once for the new session.
pub fn trajectory_refresh_needed(store: &Store) -> bool {
    let Some(session) = store.active_session() else { return false };
    showing_trajectory(store)
        && traj().lock().unwrap_or_else(|p| p.into_inner()).refreshed_for.as_deref() != Some(session.as_str())
}

/// Feed a live notification to the open task detail (`observeNotification`,
/// `use-supervision.ts:477-525`: `task/output/delta` for the session).
pub fn observe(n: &octos_core::app_ui::AppUiBackendEvent, active_session: Option<&str>) {
    if let octos_core::app_ui::AppUiBackendEvent::TaskOutputDelta(e) = n {
        if Some(e.session_id.0.as_str()) != active_session {
            return;
        }
        let mut st = traj().lock().unwrap_or_else(|p| p.into_inner());
        if trajectory::append_delta(&mut st.detail, &e.task_id.0.to_string(), e.cursor.offset, &e.text) {
            drop(st);
            wake();
        }
    }
}

// ------------------------------------------------------------------ jobs

/// Transport work an action asks for (run by the host on its runtime).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    /// `approval/respond` (`ApprovalPanel.tsx:100-125`).
    Approve { approval_id: String, session_id: String, decision: String, scope: String },
    /// `user_question/respond` (`answers.ts:44-55` -> `client.ts:558`).
    Answer { question_id: String, session_id: String, answers: String },
    /// `task/list` + `session/status/read`.
    Refresh,
    /// `task/output/read` + `task/artifact/list` for one task.
    OpenTask(String),
    MoreOutput,
    CancelTask(String),
    ReadArtifact(usize),
    MoreArtifact,
}

/// What an action asks of the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Spawn(Job),
    /// Route another (non-surface) id through the host router.
    Action(String),
    /// Open the diff review for this preview id (the `D` path,
    /// `ApprovalPanel.tsx:45` / `:93-99`).
    ReviewDiff(String),
    Unrouted,
}

/// Route one `cv.*` action. `index` is the `#<row>` the tap path decoded.
pub fn perform(action: &str, index: usize, store: &Store, ui: &Arc<Mutex<FlowUi>>) -> Outcome {
    let session = store.active_session().unwrap_or_default();
    match action {
        "cv.noop" => Outcome::Done,
        // ---- the approval card
        "cv.approval.once" | "cv.approval.session" | "cv.approval.deny" => {
            let Some((p, detail)) = store.domains.approval.showing(&session) else { return Outcome::Done };
            let mut st = state();
            if st.approval.busy.is_some() {
                return Outcome::Done; // `busy` disables every decision
            }
            st.view.focus_inside = true;
            // A20 — the `resolve` preflight (`session-interaction-ledger.ts:
            // 400-414`): the record's own owner must be the Session this
            // client drives, over a ready connection, on the generation that
            // observed it — else the card says so and NOTHING is sent.
            let owner = detail.owner();
            if let Err(stale) = store.domains.approval.authorize(
                octoscode_store::domains::approval::InteractionKind::Approval,
                &owner,
                &p.id,
                Some(session.as_str()),
                ready(store),
            ) {
                st.approval.error = Some((p.id, stale.to_owned()));
                return Outcome::Done;
            }
            st.approval.busy = Some(p.id.clone());
            st.approval.error = None;
            let (decision, scope) = match action {
                "cv.approval.once" => ("approve", "request"),
                "cv.approval.session" => ("approve", "session"),
                _ => ("deny", "request"),
            };
            Outcome::Spawn(Job::Approve {
                approval_id: p.id,
                // The record's exact owning SessionKey (`:439`), never
                // whichever Session happens to be selected later.
                session_id: owner,
                decision: decision.into(),
                scope: scope.into(),
            })
        }
        "cv.approval.diff" => match store.domains.approval.showing(&session).and_then(|(p, _)| p.preview_id) {
            Some(id) => Outcome::ReviewDiff(id),
            None => Outcome::Done,
        },
        // ---- the question card
        "cv.q.opt" => {
            let Some(q) = active_question(store) else { return Outcome::Done };
            let Some(qs) = takeover::parse_questions(&q.questions) else { return Outcome::Done };
            let (qi, oi) = (index / 100, index % 100);
            let (Some(question), mut st) = (qs.get(qi), state()) else { return Outcome::Done };
            st.question.bind(&q, qs.len());
            if st.question.busy {
                return Outcome::Done;
            }
            let Some((label, _)) = question.options.get(oi) else { return Outcome::Done };
            let next = takeover::toggle_option(question, &st.question.answers[qi], label);
            st.question.answers[qi] = next;
            // The mounted free text follows the draft on this remount.
            st.question.free_snap = st.question.answers.iter().map(|a| a.free_text.clone()).collect();
            st.question.focus = (qi, oi);
            st.view.focus_inside = true;
            Outcome::Done
        }
        "cv.q.submit" => submit_question(store),
        "cv.q.stop" => Outcome::Action(crate::bindings::ACTION_INTERRUPT.to_owned()),
        // ---- the plan card
        "cv.plan.toggle" => {
            let mut st = state();
            st.plan.collapsed = !st.plan.collapsed;
            Outcome::Done
        }
        // ---- the tabs + the Trajectory
        "cv.tab.chat" => {
            state().tab = Tab::Chat;
            Outcome::Done
        }
        "cv.tab.trajectory" => {
            if !tabs_available(store) {
                return Outcome::Done;
            }
            state().tab = Tab::Trajectory;
            if trajectory_refresh_needed(store) {
                Outcome::Spawn(Job::Refresh)
            } else {
                Outcome::Done
            }
        }
        "cv.traj.refresh" => Outcome::Spawn(Job::Refresh),
        "cv.task.open" | "cv.task.cancel" => {
            let rows = store.domains.task.session_rows(&session);
            let Some(t) = rows.get(index) else { return Outcome::Done };
            if action == "cv.task.open" {
                Outcome::Spawn(Job::OpenTask(t.id.clone()))
            } else {
                Outcome::Spawn(Job::CancelTask(t.id.clone()))
            }
        }
        "cv.detail.close" => {
            // `closeTaskDetail` invalidates the in-flight reads (`:251`).
            traj().lock().unwrap_or_else(|p| p.into_inner()).detail = trajectory::Detail::default();
            Outcome::Done
        }
        "cv.detail.more" => Outcome::Spawn(Job::MoreOutput),
        "cv.art.read" => Outcome::Spawn(Job::ReadArtifact(index)),
        "cv.art.more" => Outcome::Spawn(Job::MoreArtifact),
        // ---- the transcript's fold bar
        "cv.fold.expand_all" => {
            folds::expand_all(store, ui);
            Outcome::Done
        }
        "cv.fold.collapse_all" => {
            folds::collapse_all(store, ui);
            Outcome::Done
        }
        _ => Outcome::Unrouted,
    }
}

/// The question's submit (`UserQuestionPanel.tsx:53-57`): only a complete,
/// idle draft goes out; Enter anywhere in the card lands here too.
pub fn submit_question(store: &Store) -> Outcome {
    let Some(q) = active_question(store) else { return Outcome::Done };
    let Some(qs) = takeover::parse_questions(&q.questions) else { return Outcome::Done };
    let mut st = state();
    st.question.bind(&q, qs.len());
    if st.question.busy || !takeover::answers_complete(&st.question.answers) {
        return Outcome::Done;
    }
    st.view.focus_inside = true;
    // A20 — the same preflight as an approval (`:400-414`): a stale record
    // keeps every selection and the typed text, says why, sends nothing.
    let owner = q.owner();
    if let Err(stale) = store.domains.approval.authorize(
        octoscode_store::domains::approval::InteractionKind::Question,
        &owner,
        &q.question_id,
        store.active_session().as_deref(),
        ready(store),
    ) {
        st.question.error = Some(stale.to_owned());
        return Outcome::Done;
    }
    st.question.busy = true;
    st.question.error = None;
    Outcome::Spawn(Job::Answer {
        question_id: q.question_id.clone(),
        // The record's exact owning SessionKey (`:455`).
        session_id: owner,
        answers: takeover::to_wire_answers(&st.question.answers).to_string(),
    })
}

/// A text input changed (`cv.q.other#<question>`): the draft only — the
/// mounted input keeps its own text, so no remount follows a keystroke.
pub fn input_changed(key: &str, text: &str) {
    let (base, row) = crate::screens::taps::split_row(key);
    if base == "cv.q.other" {
        let qi = row.unwrap_or(0);
        let mut st = state();
        if let Some(a) = st.question.answers.get_mut(qi) {
            a.free_text = text.to_owned();
        }
        st.view.focus_inside = true;
    }
}

/// A job could not run (no live conversation): fail closed, visibly.
pub fn job_unavailable(job: &Job) {
    let msg = "Not connected to the server.".to_owned();
    match job {
        Job::Approve { approval_id, .. } => {
            let mut st = state();
            st.approval.busy = None;
            st.approval.error = Some((approval_id.clone(), msg));
        }
        Job::Answer { .. } => {
            let mut st = state();
            st.question.busy = false;
            st.question.error = Some(msg);
        }
        _ => {
            let mut t = traj().lock().unwrap_or_else(|p| p.into_inner());
            t.loading = false;
            t.error = Some(msg);
        }
    }
}

fn readable(e: &octoscode_client::ClientError) -> String {
    match e {
        octoscode_client::ClientError::Rpc { error, .. } => error.message.clone(),
        other => other.to_string(),
    }
}

/// Run a job through the production client.
pub async fn run(job: Job, conv: &crate::flow::Conversation) -> Result<String, String> {
    match job {
        Job::Approve { approval_id, session_id, decision, scope } => {
            use octoscode_store::domains::approval::InteractionKind;
            // A20 — the preflight again, at the moment of sending (the web's
            // `resolve`, `:400-414`): the conversation must still drive the
            // record's owner, ready, on the record's generation.
            let driving = conv.session_id();
            let generation = match conv.store.domains.approval.authorize(
                InteractionKind::Approval,
                &session_id,
                &approval_id,
                Some(driving.as_str()).filter(|d| conv.store.active_session().as_deref() == Some(*d)),
                ready(&conv.store),
            ) {
                Ok(g) => g,
                Err(stale) => {
                    let mut st = state();
                    st.approval.busy = None;
                    st.approval.error = Some((approval_id, stale.to_owned()));
                    return Err(stale.to_owned());
                }
            };
            // ApprovalPanel.tsx:100-125 -> session-interaction-ledger.ts:432-447:
            // the generation-checked owning session, the scope, no note.
            let r = conv
                .client()
                .request(
                    "approval/respond",
                    json!({
                        "session_id": session_id,
                        "approval_id": approval_id,
                        "decision": decision,
                        "approval_scope": scope,
                    }),
                )
                .await;
            let outcome = match r {
                Ok(v) if v.get("accepted").and_then(|a| a.as_bool()) != Some(true) => {
                    Err("The server rejected the response".to_owned())
                }
                Ok(v) if v.get("approval_id").and_then(|a| a.as_str()) != Some(approval_id.as_str()) => {
                    Err("The server responded for another interaction".to_owned())
                }
                Ok(_) => Ok(()),
                Err(e) => Err(readable(&e)),
            };
            // A20 — `isCurrent()` (`:417-427`): a reply for a record that a
            // restore re-armed, a newer request superseded or a session switch
            // retired settles nothing and reports nothing.
            if !conv.store.domains.approval.is_current(InteractionKind::Approval, &session_id, &approval_id, generation) {
                state().approval.busy = None;
                return Ok(format!("{decision}/{scope}: the record changed meanwhile — nothing settled"));
            }
            let mut st = state();
            st.approval.busy = None;
            match outcome {
                Ok(()) => {
                    // Accepted: the record is done (`:465`); the durable
                    // `approval/decided` settles the same row again, harmlessly.
                    conv.store.domains.approval.decide(&approval_id);
                    st.approval.error = None;
                    Ok(format!("{decision}/{scope} accepted"))
                }
                Err(e) => {
                    st.approval.error = Some((approval_id, e.clone()));
                    Err(e)
                }
            }
        }
        Job::Answer { question_id, session_id, answers } => {
            use octoscode_store::domains::approval::InteractionKind;
            let driving = conv.session_id();
            let generation = match conv.store.domains.approval.authorize(
                InteractionKind::Question,
                &session_id,
                &question_id,
                Some(driving.as_str()).filter(|d| conv.store.active_session().as_deref() == Some(*d)),
                ready(&conv.store),
            ) {
                Ok(g) => g,
                Err(stale) => {
                    let mut st = state();
                    st.question.busy = false;
                    st.question.error = Some(stale.to_owned());
                    return Err(stale.to_owned());
                }
            };
            let answers: serde_json::Value = serde_json::from_str(&answers).unwrap_or(json!([]));
            let r = conv
                .client()
                .request(
                    "user_question/respond",
                    json!({ "session_id": session_id, "question_id": question_id, "answers": answers }),
                )
                .await;
            let outcome = match r {
                Ok(v) if v.get("accepted").and_then(|a| a.as_bool()) != Some(true) => {
                    Err("The server rejected the response".to_owned())
                }
                Ok(v) if v.get("question_id").and_then(|a| a.as_str()) != Some(question_id.as_str()) => {
                    Err("The server responded for another interaction".to_owned())
                }
                Ok(_) => Ok(()),
                Err(e) => Err(readable(&e)),
            };
            if !conv.store.domains.approval.is_current(InteractionKind::Question, &session_id, &question_id, generation) {
                state().question.busy = false;
                return Ok("the question changed meanwhile — nothing settled".into());
            }
            let mut st = state();
            st.question.busy = false;
            match outcome {
                Ok(()) => {
                    conv.store.domains.approval.clear_question_if(&question_id);
                    st.question.error = None;
                    Ok("answer accepted".into())
                }
                Err(e) => {
                    // A failed response keeps every selection and the typed
                    // text (`final-input.spec.ts`: "retain free text and
                    // selections after a failed response").
                    st.question.error = Some(e.clone());
                    Err(e)
                }
            }
        }
        Job::Refresh => trajectory::refresh(conv, traj()).await,
        Job::OpenTask(id) => trajectory::open_task(conv, traj(), id).await,
        Job::MoreOutput => trajectory::load_more(conv, traj()).await,
        Job::CancelTask(id) => trajectory::cancel(conv, traj(), id).await,
        Job::ReadArtifact(i) => trajectory::read_artifact(conv, traj(), i).await,
        Job::MoreArtifact => trajectory::more_artifact(conv, traj()).await,
    }
}

// ------------------------------------------------------------- keyboard

/// What a key does to the surfaces (the host performs it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Not ours: the key falls through to the shell's resolver.
    Pass,
    /// Swallowed (no composer submit behind a takeover).
    Swallow,
    /// Perform this `cv.*` action.
    Action(String, usize),
}

/// One key while a takeover shows. `text_focus` = one of the card's text
/// inputs holds key focus (its keys are typing, `UserQuestionPanel.tsx:
/// 107-121` moves only between checkboxes). Modifier chords never act
/// (`ApprovalPanel.tsx:36-43`); a key the IME consumed for a composition
/// never reaches the app (the platform's `MacosImeKeyboard::end_key_down`
/// forwards a Return that commits a candidate as consumed), which is the
/// native form of the web's `isComposing || keyCode === 229` guard.
pub fn key(store: &Store, key: &str, shift: bool, ctrl: bool, alt: bool, logo: bool, text_focus: bool) -> KeyOutcome {
    let Some(t) = takeover(store) else { return KeyOutcome::Pass };
    if ctrl || alt || logo {
        return KeyOutcome::Pass;
    }
    match t {
        Takeover::Approval(_) => {
            if text_focus {
                return KeyOutcome::Pass;
            }
            match key {
                "y" if !shift => KeyOutcome::Action("cv.approval.once".into(), 0),
                "s" if !shift => KeyOutcome::Action("cv.approval.session".into(), 0),
                "n" if !shift => KeyOutcome::Action("cv.approval.deny".into(), 0),
                "d" if !shift => KeyOutcome::Action("cv.approval.diff".into(), 0),
                // Return behind the card must never send the hidden draft.
                "Enter" => KeyOutcome::Swallow,
                _ => KeyOutcome::Pass,
            }
        }
        Takeover::Question(_) => {
            // Enter ANYWHERE in the card sends the answer (`:59-68`) —
            // inputs included; Shift+Enter too (the web ignores modifiers
            // except the composition guard).
            if key == "Enter" {
                return KeyOutcome::Action("cv.q.submit".into(), 0);
            }
            if text_focus {
                return KeyOutcome::Pass;
            }
            let Some(q) = active_question(store) else { return KeyOutcome::Pass };
            let Some(qs) = takeover::parse_questions(&q.questions) else { return KeyOutcome::Pass };
            let mut st = state();
            st.question.bind(&q, qs.len());
            let (qi, oi) = st.question.focus;
            let Some(question) = qs.get(qi) else { return KeyOutcome::Pass };
            if let Some(delta) = takeover::arrow_delta(key) {
                let next = takeover::next_option_index(oi, delta, question.options.len());
                st.question.focus = (qi, next);
                st.question.focus_visible = true;
                st.view.focus_inside = true;
                // A radio group selects as it moves (the browser's native
                // radio arrows); a checkbox group only moves focus (`:107-121`).
                if !question.multi_select {
                    return KeyOutcome::Action("cv.q.opt".into(), qi * 100 + next);
                }
                return KeyOutcome::Swallow;
            }
            if key == "Space" {
                st.question.focus_visible = true;
                return KeyOutcome::Action("cv.q.opt".into(), qi * 100 + oi);
            }
            KeyOutcome::Pass
        }
    }
}

/// The focus-restore decision for this sync (`focus-restore.ts`): returns
/// true once when the takeover just disappeared while holding the keyboard.
pub fn take_focus_restore(showing: bool) -> bool {
    let mut st = state();
    let restore = view::restore_focus(st.view.takeover_was, showing, st.view.focus_inside);
    if !showing {
        st.view.focus_inside = false;
    }
    st.view.takeover_was = showing;
    restore
}

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_store::domains::approval::{ApprovalDetail, PendingQuestion};

    fn live_store() -> Arc<Store> {
        let s = Arc::new(Store::new());
        s.set_active(Some("s1".into()));
        s.set_connection("Live".into(), true);
        s.domains.config.set_supported_methods(vec!["user_question/respond".into(), "task/list".into()]);
        s.set_capabilities(vec!["user_question.v1".into(), "plan.todos.v1".into()]);
        s
    }

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static L: Mutex<()> = Mutex::new(());
        let g = L.lock().unwrap_or_else(|p| p.into_inner());
        reset();
        g
    }

    #[test]
    fn an_approval_takes_over_before_a_question_and_only_in_its_session() {
        let _g = lock();
        let s = live_store();
        s.domains.approval.set_question(PendingQuestion {
            question_id: "q1".into(),
            session_id: "s1".into(),
            turn_id: "t1".into(),
            title: "Pick".into(),
            body: String::new(),
            questions: json!([{"header": "H", "question": "Pick", "options": [{"label": "A", "description": ""}]}]),
            ..Default::default()
        });
        assert_eq!(takeover(&s), Some(Takeover::Question("q1".into())));
        s.domains.approval.request("a1", None);
        s.domains.approval.set_detail("a1", ApprovalDetail { session_id: "s2".into(), ..Default::default() });
        assert_eq!(takeover(&s), Some(Takeover::Question("q1".into())), "another session's approval waits there");
        s.domains.approval.request("a2", None);
        s.domains.approval.set_detail("a2", ApprovalDetail { session_id: "s1".into(), ..Default::default() });
        assert_eq!(takeover(&s), Some(Takeover::Approval("a2".into())));
    }

    #[test]
    fn approval_keys_are_bare_keys_and_return_never_sends_the_hidden_draft() {
        let _g = lock();
        let s = live_store();
        s.domains.approval.request("a1", None);
        s.domains.approval.set_detail("a1", ApprovalDetail { session_id: "s1".into(), ..Default::default() });
        assert_eq!(key(&s, "y", false, false, false, false, false), KeyOutcome::Action("cv.approval.once".into(), 0));
        assert_eq!(key(&s, "s", false, false, false, false, false), KeyOutcome::Action("cv.approval.session".into(), 0));
        assert_eq!(key(&s, "n", false, false, false, false, false), KeyOutcome::Action("cv.approval.deny".into(), 0));
        for (c, a, l) in [(true, false, false), (false, true, false), (false, false, true)] {
            assert_eq!(key(&s, "y", false, c, a, l, false), KeyOutcome::Pass, "a chord never decides");
        }
        assert_eq!(key(&s, "y", true, false, false, false, false), KeyOutcome::Pass, "Shift+Y is not Y");
        assert_eq!(key(&s, "Enter", false, false, false, false, false), KeyOutcome::Swallow);
        // A decision is one in flight at a time.
        let ui = Arc::new(Mutex::new(FlowUi::default()));
        assert!(matches!(perform("cv.approval.once", 0, &s, &ui), Outcome::Spawn(Job::Approve { .. })));
        assert_eq!(perform("cv.approval.deny", 0, &s, &ui), Outcome::Done, "busy: no second decision");
    }

    #[test]
    fn the_question_card_moves_selects_and_submits_only_when_complete() {
        let _g = lock();
        let s = live_store();
        s.domains.approval.set_question(PendingQuestion {
            question_id: "q1".into(),
            session_id: "s1".into(),
            turn_id: "t1".into(),
            title: "Pick".into(),
            body: String::new(),
            questions: json!([
                {"header": "Color", "question": "Which?", "multi_select": false, "allow_free_text": true,
                 "options": [{"label": "Blue", "description": ""}, {"label": "Red", "description": ""}]},
                {"header": "Extras", "question": "Any?", "multi_select": true, "allow_free_text": false,
                 "options": [{"label": "Tests", "description": ""}, {"label": "Docs", "description": ""}]}
            ]),
            ..Default::default()
        });
        let ui = Arc::new(Mutex::new(FlowUi::default()));
        assert_eq!(submit_question(&s), Outcome::Done, "incomplete: nothing goes out");
        // ArrowDown in the radio group selects as it moves (wrapping).
        assert_eq!(key(&s, "ArrowDown", false, false, false, false, false), KeyOutcome::Action("cv.q.opt".into(), 1));
        perform("cv.q.opt", 1, &s, &ui);
        assert_eq!(state().question.answers[0].selected, vec!["Red"]);
        // Into the checkbox group by a click, then arrows only MOVE focus.
        perform("cv.q.opt", 100, &s, &ui);
        assert_eq!(key(&s, "ArrowUp", false, false, false, false, false), KeyOutcome::Swallow);
        assert_eq!(state().question.focus, (1, 1));
        assert_eq!(key(&s, "Space", false, false, false, false, false), KeyOutcome::Action("cv.q.opt".into(), 101));
        perform("cv.q.opt", 101, &s, &ui);
        assert_eq!(state().question.answers[1].selected, vec!["Tests", "Docs"]);
        // Typing never moves focus; Enter anywhere submits.
        assert_eq!(key(&s, "ArrowDown", false, false, false, false, true), KeyOutcome::Pass);
        assert_eq!(key(&s, "Enter", false, false, false, false, true), KeyOutcome::Action("cv.q.submit".into(), 0));
        match submit_question(&s) {
            Outcome::Spawn(Job::Answer { answers, .. }) => assert_eq!(
                answers,
                json!([{"selected_labels": ["Red"]}, {"selected_labels": ["Tests", "Docs"]}]).to_string()
            ),
            other => panic!("{other:?}"),
        }
        assert_eq!(submit_question(&s), Outcome::Done, "busy: one send at a time");
    }

    /// Every surface's DSL, in every state, EVALUATES in the app VM (the
    /// mount path's `eval_component`): a property a widget lacks fails the
    /// whole mount at runtime ("pop_stack_value on empty stack"), so this
    /// catches it before a launch — desktop and phone.
    #[test]
    fn every_surface_evaluates_in_the_app_vm() {
        use crate::screens::board3::ui::Frame;
        use makepad_widgets::*;
        let _g = lock();
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(makepad_widgets::script_mod);
        cx.with_vm(octoscript_widgets::design::script_mod);
        cx.with_vm(octoscript_widgets::kit::script_mod);
        let vm = makepad_widgets::widget_async::MAIN_SPLASH_VM_ID;
        let mut eval = |name: &str, dsl: &str| {
            assert!(!dsl.is_empty(), "{name} lowered nothing");
            assert_eq!(dsl.matches('{').count(), dsl.matches('}').count(), "{name} is balanced");
            assert!(crate::mount::eval_component(&mut cx, vm, dsl).is_ok(), "{name} must evaluate:\n{dsl}");
        };
        let p = octoscode_store::domains::approval::PendingApproval {
            id: "a1".into(),
            target: None,
            decided: false,
            auto_resolved: false,
            cancelled: false,
            preview_id: Some("p".into()),
        };
        let a = ApprovalDetail {
            session_id: "s1".into(),
            turn_id: "t".into(),
            tool_name: "shell".into(),
            title: "Run this command?".into(),
            body: "Push the fix branch so CI can run".into(),
            kind: Some("command".into()),
            risk: Some("medium".into()),
            command: Some("git push origin feat/steer-queue-with-a-long-branch-name-that-wraps".into()),
            ..Default::default()
        };
        let q = PendingQuestion {
            question_id: "q1".into(),
            session_id: "s1".into(),
            turn_id: "t".into(),
            title: "Where should queued steers be persisted?".into(),
            body: String::new(),
            questions: json!([
                {"header": "Store", "question": "Where?", "multi_select": false, "allow_free_text": true,
                 "options": [{"label": "In the session ledger (recommended)", "description": "Durable"}, {"label": "In memory only", "description": ""}]},
                {"header": "Extras", "question": "Any?", "multi_select": true, "allow_free_text": false,
                 "options": [{"label": "Tests", "description": "add"}, {"label": "Docs", "description": ""}]}
            ]),
            ..Default::default()
        };
        let qs = takeover::parse_questions(&q.questions).unwrap();
        for phone in [false, true] {
            let look = takeover::Look { phone, width: if phone { 336.0 } else { 661.0 }, max_h: 480.0 };
            for (busy, err) in [(None, None), (Some("a1".to_owned()), None), (None, Some(("a1".to_owned(), "denied".to_owned())))] {
                let mut d = Dsl::new();
                takeover::approval_card(&mut d, &p, &a, &takeover::ApprovalUi { busy, error: err }, &look);
                eval("approval", &d.finish());
            }
            let mut st = takeover::QuestionUi::default();
            st.bind(&q, qs.len());
            st.focus_visible = true;
            st.answers[0].selected = vec!["In memory only".into()];
            st.error = Some("The server rejected the response".into());
            let mut d = Dsl::new();
            takeover::question_card(&mut d, &q, &qs, &st, &look);
            eval("question", &d.finish());
            let plan = octoscode_store::domains::task::Plan {
                items: vec![
                    octoscode_store::domains::task::PlanItem { id: "1".into(), title: "Reproduce".into(), status: "completed".into(), priority: Some("P1".into()) },
                    octoscode_store::domains::task::PlanItem { id: "2".into(), title: "Fix".into(), status: "in_progress".into(), priority: None },
                ],
                title: None,
                updated_at_ms: 0,
                turn_id: None,
            };
            for collapsed in [false, true] {
                let mut d = Dsl::new();
                plan::card(&mut d, &plan, &plan::PlanUi { collapsed }, look.width, phone, 0);
                eval("plan", &d.finish());
            }
            // The Trajectory with every section and a cancelling row, and
            // the detail dialog loading / with output and a read artifact.
            let s = live_store();
            s.domains.config.set_supported_methods(vec![
                "task/list".into(), "task/output/read".into(), "task/cancel".into(),
                "task/artifact/list".into(), "task/artifact/read".into(), "session/status/read".into(),
            ]);
            s.set_capabilities(vec!["plan.todos.v1".into(), "harness.task_artifacts.v1".into()]);
            s.domains.task.set_plan("s1", plan.clone());
            let mut rows = Vec::new();
            for (i, state) in ["running", "cancelling", "failed"].iter().enumerate() {
                let mut t = octoscode_store::domains::task::TaskSnapshot::from_list_row(
                    format!("t{i}"), "c24b-probe".into(), (*state).into(), (*state).into(),
                    None, None, None, None, 1, vec![], Some("exit 1".into()), None,
                );
                t.phase = Some("verify".into());
                rows.push(t);
            }
            s.domains.task.replace_session_rows("s1", rows);
            let mut ts = trajectory::TrajState {
                error: Some("task/list returned another session".into()),
                status: Some(("s1".into(), trajectory::RuntimeStatus { model: Some("deepseek-v4-flash".into()), permission: None, health: Some("ok".into()) })),
                ..Default::default()
            };
            let mut d = Dsl::new();
            trajectory::pane(&mut d, &s, &ts, if phone { 360.0 } else { 709.0 }, phone);
            eval("trajectory", &d.finish());
            let frame = if phone { Frame { avail_w: 360.0, avail_h: 780.0 } } else { Frame::DESKTOP };
            ts.detail = trajectory::Detail { active: true, task_id: Some("t0".into()), loading: true, ..Default::default() };
            let mut d = Dsl::new();
            trajectory::detail_dialog(&mut d, &s, &ts, &frame);
            eval("detail loading", &d.finish());
            ts.detail.loading = false;
            ts.detail.text = "line one\nline two".into();
            ts.detail.output = Some(trajectory::OutputPage { next_offset: 17, total_bytes: 2048, complete: false, source: "runtime_projection".into() });
            let art = trajectory::Artifact { id: "a".into(), title: "report.md".into(), kind: "report".into(), status: "ready".into(), path: None, content: None };
            ts.detail.artifacts = Some(vec![art.clone()]);
            ts.detail.selected = Some(trajectory::ArtifactPage { artifact: art, content: "# r".into(), has_more: true, next_offset: Some(3) });
            ts.detail.error = Some(trajectory::GAP_ERROR.into());
            let mut d = Dsl::new();
            trajectory::detail_dialog(&mut d, &s, &ts, &frame);
            eval("detail", &d.finish());
        }
        // The transcript rows this area owns (A4's rows + A6's notices): the
        // fold bar, a thinking block folded and open, every notice shape and
        // a delivered file. (A blank-id `rule("")` in the notice row failed
        // here — the row had never evaluated in the app.)
        use crate::screens::board3::rows::{self, TRow};
        use octoscode_store::timeline::EntryKind;
        let s = live_store();
        let tl = &s.domains.session.timeline;
        tl.upsert_user_message("s1", "t1", "hi", json!({}));
        let r = tl.append_delta_timed("s1", Some("t1"), EntryKind::REASONING, "Weighing the retry path\nthen the queue", 1_000);
        tl.append_delta_timed("s1", Some("t1"), EntryKind::REASONING, " order", 13_000);
        let n1 = tl.upsert_notice_data("s1", Some("t1".into()), "terminal:t1", "errored: boom".into(), json!({"outcome": "errored", "code": "e", "message": "boom"}));
        let n2 = tl.upsert_notice_data("s1", None, "warning:3", "w".into(), json!({"code": "provider_busy", "message": "Retry later"}));
        let n3 = tl.upsert_notice_data("s1", Some("t1".into()), "approval:a", "Auto-approved".into(), json!({"title": "Auto-approved", "message": "bash · matched the session scope"}));
        let f = tl.append_data("s1", Some("t1".into()), EntryKind::ATTACHMENT, "out/report.pdf".into(), json!({"path": "out/report.pdf", "size_bytes": 2048}));
        let mut rows_to_eval = vec![TRow::FoldBar, TRow::Thinking(r), TRow::Notice(n1), TRow::Notice(n2), TRow::Notice(n3), TRow::File(f)];
        s.domains.session.set_thinking_expanded("s1", vec![format!("r{r}")]);
        rows_to_eval.push(TRow::Thinking(r));
        for row in rows_to_eval {
            let dsl = rows::lower(&row, &s);
            eval(&format!("{row:?}"), &dsl);
        }
    }

    /// Debug aid: evaluate every DSL dumped by `OCTOSCODE_SURFACES_DUMP`
    /// (run with `A6_EVAL_DUMP=<dir> cargo test … -- --nocapture`).
    #[test]
    fn dumped_surfaces_evaluate() {
        let Ok(dir) = std::env::var("A6_EVAL_DUMP") else { return };
        use makepad_widgets::*;
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(makepad_widgets::script_mod);
        cx.with_vm(octoscript_widgets::design::script_mod);
        cx.with_vm(octoscript_widgets::kit::script_mod);
        let vm = makepad_widgets::widget_async::MAIN_SPLASH_VM_ID;
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
        names.sort();
        for p in names {
            let dsl = std::fs::read_to_string(&p).unwrap();
            if dsl.is_empty() {
                continue;
            }
            eprintln!("== eval {}", p.display());
            let r = crate::mount::eval_component(&mut cx, vm, &dsl);
            eprintln!("   -> {}", if r.is_ok() { "ok" } else { "ERR" });
        }
    }

    #[test]
    fn focus_returns_to_the_composer_only_off_a_removed_card_that_held_it() {
        let _g = lock();
        assert!(!take_focus_restore(true));
        state().view.focus_inside = true;
        assert!(!take_focus_restore(true), "still showing");
        assert!(take_focus_restore(false), "removed while holding focus");
        assert!(!take_focus_restore(false), "once");
    }
}
