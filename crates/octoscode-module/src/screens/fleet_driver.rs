//! A10 — the external-driver control chains behind the Fleet (web
//! `features/control/*`, `app/fleet-start-sequencer.ts`,
//! `session/use-octos-session.ts` seat + row-control seams), over the typed
//! leaf in `octoscode_client::domains::external_driver`.
//!
//! * the **driver id** is stable per install (`stablePeerDriverId`:
//!   `octoscode-native:<uuid>`, minted once, persisted beside the display
//!   preferences; fail-open to an in-memory id);
//! * the **inventory walk** (`session/driver/get` with operations pages)
//!   lands in the store ([`load_inventory`]); readiness = advertised + a
//!   COMPLETE walk (`deriveControlReadiness`);
//! * **Start** = acquire (CAS on the walked revision) → `peer/prepare` →
//!   EXACTLY ONE `peer/dispatch` with the Start's operation id → the
//!   server-ADOPTED row → a background `session/open` of the adopted session
//!   so its frames reach the roster ([`start`]); a retry reuses the SAME id;
//! * **row control** sends EXACTLY ONE `peer/control` frame per activation
//!   with the held fence, the row's accepted operation id and ADOPTED turn,
//!   never retried ([`row_control`]);
//! * the seat itself: acquire / release(next external) for the console
//!   ([`acquire_seat`], [`release_seat`]).
use std::sync::Mutex;
use std::time::Duration;

use octoscode_client::domains::external_driver as xd;
use octoscode_client::domains::peer::{PeerPrepare, PeerPrepareParams};
use octoscode_store::domains::peer::{
    Disclosure, FleetInventory, InventoryOp, Origin, PeerSessionEvent, RequestKind, RowControl,
};
use octoscode_store::Store;

use crate::flow::Conversation;
use crate::screens::peers::{self, RowAction};

/// The dispatch receipt deadline (§6 "no receipt within 15 s").
pub const RECEIPT_TIMEOUT: Duration = Duration::from_secs(15);

// ------------------------------------------------------------- driver id

static DRIVER_ID: Mutex<Option<String>> = Mutex::new(None);

fn driver_id_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("OCTOSCODE_DRIVER_ID_PATH") {
        return p.into();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".octoscode").join("driver-id")
}

/// `stablePeerDriverId` (`use-octos-session.ts:525-540`): the persisted id
/// when it is well-formed, else a fresh `octoscode-native:<uuid>` (persisting
/// is best-effort; a blocked store still yields a usable id this run).
pub fn driver_id() -> String {
    let mut g = DRIVER_ID.lock().unwrap();
    if let Some(id) = g.as_ref() {
        return id.clone();
    }
    const PREFIX: &str = "octoscode-native:";
    let path = driver_id_path();
    let stored = std::fs::read_to_string(&path).ok().map(|s| s.trim().to_owned()).filter(|s| {
        s.strip_prefix(PREFIX)
            .is_some_and(|u| octoscode_client::protocol_id::is_protocol_uuid(&serde_json::Value::String(u.to_owned())))
    });
    let id = stored.unwrap_or_else(|| {
        let minted = format!("{PREFIX}{}", xd::new_operation_id());
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, &minted);
        minted
    });
    *g = Some(id.clone());
    id
}

// ----------------------------------------------------------------- scope

/// The captured driver scope of the active master session.
pub fn scope(conv: &Conversation) -> xd::DriverScope {
    scope_of(&conv.store, &conv.session_id(), &conv.profile())
}

pub fn scope_of(store: &Store, session_id: &str, profile_id: &str) -> xd::DriverScope {
    xd::DriverScope {
        session_id: session_id.to_owned(),
        profile_id: profile_id.to_owned(),
        methods: store.domains.config.supported_methods(),
        features: store.capabilities(),
    }
}

/// `peerControlAdmitted && peerDispatchAdmitted` — the advertised half of
/// "remote control of peers".
pub fn control_advertised(store: &Store) -> bool {
    let (m, f) = (store.domains.config.supported_methods(), store.capabilities());
    xd::admitted(&m, &f, xd::PEER_CONTROL) && xd::admitted(&m, &f, xd::PEER_DISPATCH)
}

/// `deriveControlReadiness` (`control-readiness.ts:10-26`): advertised AND a
/// COMPLETE inventory walk for the active session.
pub fn control_ready(store: &Store) -> bool {
    control_advertised(store) && complete_inventory(store).is_some()
}

/// The walked inventory, when complete for the ACTIVE session.
pub fn complete_inventory(store: &Store) -> Option<FleetInventory> {
    let active = store.active_session()?;
    match store.domains.peer.inventory() {
        Some(inv @ FleetInventory::Complete { .. }) => {
            let FleetInventory::Complete { session_id, .. } = &inv else { return None };
            (session_id == &active).then_some(inv)
        }
        _ => None,
    }
}

/// The disclosure of the active session's complete walk.
pub fn disclosure(store: &Store) -> Option<Disclosure> {
    match complete_inventory(store)? {
        FleetInventory::Complete { disclosure, .. } => Some(disclosure),
        _ => None,
    }
}

// ------------------------------------------------------------- the seat

struct Seat {
    session_id: String,
    view: xd::AcquireView,
}

static SEAT: Mutex<Option<Seat>> = Mutex::new(None);
/// The session whose seat the operator PARKED (`seatReleased`): it stays
/// released until an explicit "Acquire seat".
static PARKED: Mutex<Option<String>> = Mutex::new(None);

/// Whether this app holds the seat for `session`.
pub fn seat_held(session: &str) -> bool {
    SEAT.lock().unwrap().as_ref().is_some_and(|s| s.session_id == session)
}

/// Whether the operator parked the seat for `session`.
pub fn seat_parked(session: &str) -> bool {
    PARKED.lock().unwrap().as_deref() == Some(session)
}

/// The held binding (`peerControlBindingFor`: the HELD acquire's own binding
/// wins; the caller falls back to the observed disclosure).
pub fn held_binding(session: &str) -> Option<xd::DriverBinding> {
    SEAT.lock().unwrap().as_ref().filter(|s| s.session_id == session).map(|s| s.view.binding.clone())
}

/// The held acquire's pending work (the control seat's target source).
pub fn held_pending_work(session: &str) -> Vec<String> {
    SEAT.lock()
        .unwrap()
        .as_ref()
        .filter(|s| s.session_id == session)
        .map(|s| s.view.pending_work.clone())
        .unwrap_or_default()
}

fn held_fence(session: &str) -> Option<xd::ControlFence> {
    SEAT.lock().unwrap().as_ref().filter(|s| s.session_id == session).map(|s| s.view.fence.clone())
}

/// Test seam: drop the held seat.
pub fn reset_seat() {
    *SEAT.lock().unwrap() = None;
    *PARKED.lock().unwrap() = None;
}

/// ONE `session/driver/acquire` with CAS on the walked revision
/// (`peerControlAcquireInput`). No complete inventory → no CAS basis → no
/// frame (`buildFleetStartAcquire` returns null).
pub async fn acquire_seat(conv: &Conversation) -> Result<(), xd::DriverError> {
    let store = &conv.store;
    let Some(FleetInventory::Complete { disclosure, .. }) = complete_inventory(store) else {
        return Err(xd::DriverError::InvalidArgs(xd::SESSION_DRIVER_ACQUIRE));
    };
    let revision = disclosure.binding.as_ref().map(|b| b.2).unwrap_or(0);
    let scope = scope(conv);
    let view = xd::acquire(conv.client(), &scope, &driver_id(), revision).await?;
    *SEAT.lock().unwrap() = Some(Seat { session_id: scope.session_id.clone(), view });
    *PARKED.lock().unwrap() = None;
    Ok(())
}

/// ONE `session/driver/release {next: "external"}` (`planPeerSeatRelease`):
/// the seat is PARKED until an explicit acquire.
pub async fn release_seat(conv: &Conversation) -> Result<(), xd::DriverError> {
    let scope = scope(conv);
    let (fence, revision) = {
        let g = SEAT.lock().unwrap();
        let Some(s) = g.as_ref().filter(|s| s.session_id == scope.session_id) else {
            return Err(xd::DriverError::InvalidArgs(xd::SESSION_DRIVER_RELEASE));
        };
        (s.view.fence.clone(), s.view.binding.revision)
    };
    xd::release(conv.client(), &scope, &fence, revision, xd::DriverMode::External).await?;
    *SEAT.lock().unwrap() = None;
    *PARKED.lock().unwrap() = Some(scope.session_id);
    Ok(())
}

/// ONE `session/driver/renew` of the held lease (the 45 s cadence's tick).
pub async fn renew_seat(conv: &Conversation) -> Result<u64, xd::DriverError> {
    let scope = scope(conv);
    let fence = held_fence(&scope.session_id).ok_or(xd::DriverError::InvalidArgs(xd::SESSION_DRIVER_RENEW))?;
    let lease = xd::renew(conv.client(), &scope, &fence).await?;
    if let Some(s) = SEAT.lock().unwrap().as_mut() {
        s.view.binding.lease_expires_at_ms = lease;
    }
    Ok(lease)
}

// -------------------------------------------------------------- inventory

fn op_of(row: &xd::OperationRow) -> InventoryOp {
    InventoryOp {
        operation_id: row.operation_id.clone(),
        slug: row.acceptance.slug.clone(),
        lifecycle: row.lifecycle.clone(),
        adopted_session_id: row.acceptance.adopted_session_id.clone(),
        adopted_turn_id: row.acceptance.adopted_turn_id.clone(),
        workspace_root: row.acceptance.workspace_root.clone(),
        model: row.acceptance.model.clone(),
        model_lane: row.acceptance.model_lane.clone(),
        goal_id: row.acceptance.scoped_goal.as_ref().map(|g| g.goal_id.clone()),
        accepted_at_ms: row.acceptance.accepted_at_ms,
    }
}

/// Walk the active session's driver inventory into the store
/// (`walkDriverInventoryChain`). Unadvertised → nothing is read (the walk
/// never starts; readiness stays unavailable).
pub async fn load_inventory(conv: &Conversation) -> Result<String, String> {
    let store = &conv.store;
    let scope = scope(conv);
    if !xd::driver_get_available(&scope.methods, &scope.features) {
        return Ok("driver inventory not advertised".into());
    }
    store.domains.peer.set_inventory(Some(FleetInventory::Loading));
    match xd::walk_inventory(conv.client(), &scope).await {
        Ok(inv) => {
            let n = inv.operations.len();
            store.domains.peer.set_inventory(Some(FleetInventory::Complete {
                session_id: scope.session_id.clone(),
                snapshot: inv.snapshot.clone(),
                observed_revision: inv.observed_revision.clone(),
                operations: inv.operations.iter().map(op_of).collect(),
                disclosure: Disclosure {
                    mode: inv.mode.as_str().to_owned(),
                    recovery: inv.recovery.as_str().to_owned(),
                    binding: inv
                        .binding
                        .as_ref()
                        .map(|b| (b.driver_id.clone(), b.epoch, b.revision, b.lease_expires_at_ms)),
                },
                completed_at_ms: peers::now_ms(),
            }));
            Ok(format!("{n} operation(s), revision {}", inv.observed_revision))
        }
        Err(e) => {
            store.domains.peer.set_inventory(Some(FleetInventory::Error {
                session_id: scope.session_id.clone(),
                reason: e.to_string(),
            }));
            Err(e.to_string())
        }
    }
}

// ----------------------------------------------------------------- labels

/// `peerDispatchRefusalLabel` (`peer-dispatch-commands.ts:58-70`).
pub fn dispatch_refusal_label(kind: &str) -> &'static str {
    match kind {
        "driver_scope_mismatch" => "This session can't be controlled from here",
        "driver_fence_stale" => "Your control of this session expired",
        "driver_revision_conflict" => "This session changed hands; refresh and try again",
        "driver_busy_handover" => "This session is changing hands right now",
        "driver_operation_conflict" => "A different request already used this id — nothing was sent",
        "driver_model_unavailable" => "That model is not configured on this server",
        _ => "Couldn't start that peer.",
    }
}

/// `peerControlRefusalLabel` (`peer-control-commands.ts:156-169`).
pub fn control_refusal_label(kind: &str) -> &'static str {
    match kind {
        "driver_scope_mismatch" => "This session can't be controlled from here",
        "driver_fence_stale" => "Your control of this session expired",
        "driver_revision_conflict" => "This session changed hands; refresh and try again",
        "driver_busy_handover" => "This session is changing hands right now",
        "driver_operation_conflict" => "A different request already used this id — nothing was sent",
        "interaction_recovery_required" => "This session needs recovery on the server",
        "driver_model_unavailable" => "That model is not configured on this server",
        _ => "That action was refused.",
    }
}

// ------------------------------------------------------------------ Start

/// What ONE Start (or its same-id retry) settled as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOutcome {
    /// The receipt adopted a row: its slug + the accepted operation id.
    Accepted { slug: String, operation_id: String, duplicate: bool },
    /// A typed refusal (bounded kind); the brief stays for retry.
    Refused { kind: String },
    /// No receipt (timeout / kind-less failure): the request MAY have run —
    /// Retry resends the SAME id.
    Unknown,
}

/// The staged identity a Start carries across its same-id retry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Staged {
    pub identity: String,
    pub slug: String,
    pub brief_path: String,
}

/// The Start chain (`fleet-start-sequencer.ts` + `dispatchStagedPeer` +
/// `#openByDispatch`): acquire when no seat is held → `peer/prepare` (only on
/// the first attempt; a retry reuses `staged`) → EXACTLY ONE `peer/dispatch`
/// carrying `operation_id` → adopt the row → attach the adopted session.
/// `lanes` are the advertised keys: a lane outside them is refused
/// `driver_model_unavailable` with ZERO frames (`choosePeerLane`).
pub async fn start(
    conv: &Conversation,
    operation_id: &str,
    lane: &str,
    lanes: &[String],
    brief: &str,
    staged: &mut Option<Staged>,
) -> StartOutcome {
    start_with(conv, operation_id, lane, lanes, brief, None, true, staged).await
}

/// The shared staging chain: the Fleet's Start (`implicit_acquire`, the
/// title derived from the brief) and the console's Dispatch (a HELD seat
/// only, the operator's own title).
#[allow(clippy::too_many_arguments)]
pub async fn start_with(
    conv: &Conversation,
    operation_id: &str,
    lane: &str,
    lanes: &[String],
    brief: &str,
    title: Option<&str>,
    implicit_acquire: bool,
    staged: &mut Option<Staged>,
) -> StartOutcome {
    if !lanes.iter().any(|l| l == lane) || lane.trim().is_empty() {
        return StartOutcome::Refused { kind: "driver_model_unavailable".into() };
    }
    let store = conv.store.clone();
    let scope = scope(conv);
    // 1. Start is the ONLY implicit acquisition (§4.3); the console never
    // acquires on its own.
    if !seat_held(&scope.session_id) {
        if !implicit_acquire {
            return StartOutcome::Refused { kind: "driver_fence_stale".into() };
        }
        if let Err(e) = acquire_seat(conv).await {
            return match e.refusal_kind() {
                Some(k) => StartOutcome::Refused { kind: k.to_owned() },
                None => StartOutcome::Refused { kind: "driver_fence_stale".into() },
            };
        }
    }
    let Some(fence) = held_fence(&scope.session_id) else {
        return StartOutcome::Refused { kind: "driver_fence_stale".into() };
    };
    // 2. Stage ONCE (the retry keeps the prepared identity).
    if staged.is_none() {
        let derived: String = brief.lines().next().unwrap_or("").trim().chars().take(60).collect();
        let title: String = title.map(str::to_owned).unwrap_or(derived);
        let prepared = conv
            .client()
            .call::<PeerPrepare>(PeerPrepareParams {
                brief: brief.trim().to_owned(),
                n: None,
                title: (!title.is_empty()).then_some(title),
                names: None,
                worktree: None,
                cwd: None,
                session_id: scope.session_id.clone(),
                profile_id: scope.profile_id.clone(),
            })
            .await;
        let p = match prepared {
            Ok(p) => p,
            Err(e) => {
                ::log::warn!("octoscode: fleet start: prepare failed: {e}");
                return StartOutcome::Unknown;
            }
        };
        let identity = peers::identity_for_topic(&p.profile_id, &p.topic);
        let mut row = octoscode_store::domains::peer::PeerRow::opening(
            &identity,
            &p.slug,
            Origin::Dispatch,
            &xd::new_operation_id(),
            peers::now_ms(),
        );
        row.profile_id = p.profile_id.clone();
        row.cwd = p.cwd.clone();
        row.brief_path = p.brief_path.clone();
        row.brief = brief.trim().to_owned();
        row.operation_id = None;
        store.domains.peer.stage_row(row, true);
        *staged = Some(Staged { identity, slug: p.slug.clone(), brief_path: p.brief_path.clone() });
    }
    let st = staged.clone().unwrap_or_default();
    makepad_widgets::SignalToUI::set_ui_signal();
    // 3. EXACTLY ONE dispatch.
    let request = xd::DispatchRequest {
        operation_id: operation_id.to_owned(),
        model: lane.to_owned(),
        target: xd::DispatchTarget::NewBrief { brief: brief.trim().to_owned(), title: Some(st.slug.clone()), worktree: None },
        kickoff_text: Some(peers::kickoff_prompt(brief.trim(), &st.brief_path)),
        goal_id: None,
        task_id: None,
    };
    let sent = tokio::time::timeout(RECEIPT_TIMEOUT, xd::dispatch(conv.client(), &scope, &fence, &request)).await;
    let receipt = match sent {
        Err(_) => return StartOutcome::Unknown,
        Ok(Err(e)) => {
            return match e.refusal_kind() {
                Some(k) => {
                    store.domains.peer.mark_not_started(&st.identity, false, dispatch_refusal_label(k));
                    StartOutcome::Refused { kind: k.to_owned() }
                }
                None => {
                    store.domains.peer.mark_not_started(
                        &st.identity,
                        true,
                        "Peer dispatch could not be confirmed. Inspect the peer session; do not automatically retry.",
                    );
                    StartOutcome::Unknown
                }
            };
        }
        Ok(Ok(r)) => r,
    };
    // 4. The SERVER-adopted identity keys the row.
    store.domains.peer.mark_started(
        &st.identity,
        &receipt.adopted_session_id,
        &receipt.slug,
        Some(&receipt.operation_id),
        Some(&receipt.adopted_turn_id),
        Some(&receipt.model),
        peers::now_ms(),
    );
    store.domains.peer.update_row(&receipt.adopted_session_id, |r| {
        r.accepted_at_ms = Some(receipt.accepted_at_ms);
        r.goal_id = receipt.scoped_goal.as_ref().map(|g| g.goal_id.clone());
        r.cwd = receipt.workspace_root.clone();
    });
    // 5. Attach the adopted session (the web's `adoptOnRecord`) so its own
    // frames reach the roster. Best-effort: the row is already accepted.
    let attach = conv
        .client()
        .request(
            "session/open",
            serde_json::json!({
                "session_id": receipt.adopted_session_id,
                "profile_id": scope.profile_id,
                "cwd": receipt.workspace_root,
            }),
        )
        .await;
    if let Err(e) = attach {
        ::log::warn!("octoscode: fleet start: the adopted session did not attach: {e}");
    }
    makepad_widgets::SignalToUI::set_ui_signal();
    StartOutcome::Accepted { slug: receipt.slug, operation_id: receipt.operation_id, duplicate: receipt.duplicate }
}

/// A model-staged peer's background open (`session-peer-coordinator.ts`
/// `#open`): when remote control is READY it stages through `peer/dispatch`
/// with the operator's last lane choice (`peerLaneChoiceRef`; none chosen →
/// the typed `driver_model_unavailable` refusal, zero frames); otherwise it
/// opens the peer session and queues ONE kickoff turn.
pub async fn open_staged(conv: &Conversation, req: peers::OpenRequest) {
    let store = conv.store.clone();
    if !control_ready(&store) {
        if let Err(e) = peers::open_peer(conv, &req).await {
            ::log::warn!("octoscode: peer {} open: {e}", req.identity);
        }
        return;
    }
    let (lane, keys) = {
        let st = crate::screens::board3::host::state();
        (st.fleet.chosen_lane().unwrap_or_default(), st.fleet.lane_keys())
    };
    let scope = scope(conv);
    if !keys.contains(&lane) {
        store.domains.peer.mark_not_started(&req.identity, false, dispatch_refusal_label("driver_model_unavailable"));
        return;
    }
    let Some(fence) = held_fence(&scope.session_id) else {
        store.domains.peer.mark_not_started(&req.identity, false, dispatch_refusal_label("driver_fence_stale"));
        return;
    };
    let brief = store.domains.peer.row(&req.identity).map(|r| r.brief).unwrap_or_default();
    let request = xd::DispatchRequest {
        operation_id: xd::new_operation_id(),
        model: lane,
        target: xd::DispatchTarget::NewBrief { brief, title: Some(req.slug.clone()), worktree: None },
        kickoff_text: Some(req.prompt.clone()),
        goal_id: None,
        task_id: None,
    };
    match xd::dispatch(conv.client(), &scope, &fence, &request).await {
        Ok(receipt) => {
            store.domains.peer.mark_started(
                &req.identity,
                &receipt.adopted_session_id,
                &receipt.slug,
                Some(&receipt.operation_id),
                Some(&receipt.adopted_turn_id),
                Some(&receipt.model),
                peers::now_ms(),
            );
            let _ = conv
                .client()
                .request(
                    "session/open",
                    serde_json::json!({
                        "session_id": receipt.adopted_session_id,
                        "profile_id": scope.profile_id,
                        "cwd": receipt.workspace_root,
                    }),
                )
                .await;
        }
        Err(e) => match e.refusal_kind() {
            Some(k) => {
                store.domains.peer.mark_not_started(&req.identity, false, dispatch_refusal_label(k));
            }
            None => {
                store.domains.peer.mark_not_started(
                    &req.identity,
                    true,
                    "Peer dispatch could not be confirmed. Inspect the peer session; do not automatically retry.",
                );
            }
        },
    }
}

// ------------------------------------------------------------ row control

/// Build ONE product row command from the row's REAL attention facts
/// (`buildRowControlCommand`, `peer-row-command.ts:105-150`); `None` = fail
/// closed (no frame).
pub fn row_command(row: &octoscode_store::domains::peer::PeerRow, action: RowAction, text: &str) -> Option<xd::ControlCommand> {
    let has = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.is_empty());
    match action {
        RowAction::Approve | RowAction::ApproveSession | RowAction::Deny => {
            if row.request_kind != Some(RequestKind::Approval) || !has(&row.request_id) {
                return None;
            }
            Some(xd::ControlCommand::ApprovalRespond {
                approval_id: row.request_id.clone()?,
                approve: action != RowAction::Deny,
                approval_scope: (action == RowAction::ApproveSession).then(|| "session".to_owned()),
            })
        }
        RowAction::Answer => {
            if row.request_kind != Some(RequestKind::Question) || !has(&row.request_id) || text.trim().is_empty() {
                return None;
            }
            Some(xd::ControlCommand::QuestionRespond {
                question_id: row.request_id.clone()?,
                answers: vec![xd::QuestionAnswer { selected_labels: None, free_text: Some(text.trim().to_owned()) }],
            })
        }
        RowAction::Steer => {
            if !has(&row.operation_id) || row.turn_id.is_empty() || text.trim().is_empty() {
                return None;
            }
            Some(xd::ControlCommand::Steer { text: text.trim().to_owned() })
        }
        RowAction::Stop => {
            if !has(&row.operation_id) || row.turn_id.is_empty() {
                return None;
            }
            Some(xd::ControlCommand::Interrupt)
        }
    }
}

/// EXACTLY ONE `peer/control` frame for one row activation (never retried).
/// `Ok(ack copy)` — "Sent" / "Stop requested" (an ACKNOWLEDGMENT, never an
/// outcome); `Err(bounded copy)` — a refusal label or a fail-closed reason
/// (no frame).
pub async fn row_control(conv: &Conversation, identity: &str, action: RowAction, text: &str) -> Result<String, String> {
    let store = &conv.store;
    let scope = scope(conv);
    let row = store.domains.peer.row(identity).ok_or_else(|| "This peer is no longer in the roster.".to_owned())?;
    let Some(fence) = held_fence(&scope.session_id) else {
        return Err("Take control of this session to do this".to_owned());
    };
    let command = row_command(&row, action, text).ok_or_else(|| "That action is not available right now.".to_owned())?;
    let target = xd::ControlTarget::for_row(row.operation_id.as_deref().unwrap_or(""), &row.turn_id)
        .ok_or_else(|| "That action is not available right now.".to_owned())?;
    store.domains.peer.update_row(identity, |r| r.control = Some(RowControl::Sending));
    makepad_widgets::SignalToUI::set_ui_signal();
    let result = xd::control(conv.client(), &scope, &fence, &target, &command).await;
    match result {
        Ok(receipt) => {
            let interrupt = matches!(command, xd::ControlCommand::Interrupt);
            store.domains.peer.update_row(identity, |r| r.control = Some(RowControl::Receipt { duplicate: receipt.duplicate }));
            store
                .domains
                .peer
                .observe_session_event(identity, &PeerSessionEvent::ControlAck { interrupt }, peers::now_ms());
            Ok(if interrupt { "Stop requested".to_owned() } else { "Sent".to_owned() })
        }
        Err(e) => {
            let kind = e.refusal_kind().unwrap_or("peer_control_refused").to_owned();
            let label = control_refusal_label(&kind).to_owned();
            store.domains.peer.update_row(identity, |r| r.control = Some(RowControl::Refused { kind }));
            Err(label)
        }
    }
}

/// The CONSOLE's legacy row command (`buildRowControlCommand(action,
/// steerText)` with no attention: the frozen console vocabulary, its
/// encoder-valid synthetic approval id).
pub fn console_command(action: &str, steer_text: &str) -> Option<xd::ControlCommand> {
    Some(match action {
        "approve" | "deny" => xd::ControlCommand::ApprovalRespond {
            approval_id: "synthetic-approval".into(),
            approve: action == "approve",
            approval_scope: None,
        },
        "steer" => {
            if steer_text.trim().is_empty() {
                return None;
            }
            xd::ControlCommand::Steer { text: steer_text.trim().to_owned() }
        }
        "interrupt" => xd::ControlCommand::Interrupt,
        _ => return None,
    })
}

/// ONE console row control (the Advanced roster): the row's accepted
/// operation id + adopted turn, the frozen console command.
pub async fn console_row_control(conv: &Conversation, identity: &str, action: &str, steer_text: &str) -> Result<String, String> {
    let store = &conv.store;
    let scope = scope(conv);
    let row = store.domains.peer.row(identity).ok_or_else(|| "This peer is no longer in the roster.".to_owned())?;
    let fence = held_fence(&scope.session_id).ok_or_else(|| "Take control of this session to do this".to_owned())?;
    let command = console_command(action, steer_text).ok_or_else(|| "That action is not available right now.".to_owned())?;
    let target = xd::ControlTarget::for_row(row.operation_id.as_deref().unwrap_or(""), &row.turn_id)
        .ok_or_else(|| "That action is not available right now.".to_owned())?;
    store.domains.peer.update_row(identity, |r| r.control = Some(RowControl::Sending));
    match xd::control(conv.client(), &scope, &fence, &target, &command).await {
        Ok(receipt) => {
            store.domains.peer.update_row(identity, |r| r.control = Some(RowControl::Receipt { duplicate: receipt.duplicate }));
            Ok(if receipt.duplicate { "Already applied".into() } else { "Newly applied".into() })
        }
        Err(e) => {
            let kind = e.refusal_kind().unwrap_or("peer_control_refused").to_owned();
            let label = control_refusal_label(&kind).to_owned();
            store.domains.peer.update_row(identity, |r| r.control = Some(RowControl::Refused { kind }));
            Err(label)
        }
    }
}

/// The external-master control SEAT's target (`peerControlTargetFor`,
/// `use-octos-session.ts:622-636`): the held acquire's first pending work +
/// the master's live turn (a protocol UUID). `None` = no seat panel.
pub fn seat_target(session: &str, live_turn: Option<&str>) -> Option<xd::ControlTarget> {
    let pending = held_pending_work(session);
    let op = pending.first()?;
    xd::ControlTarget::for_row(op, live_turn?)
}

/// The seat's four synthetic commands (`buildPeerControlCommand`,
/// `peer-control-commands.ts:61-99`).
pub fn seat_command(kind: &str) -> Option<xd::ControlCommand> {
    Some(match kind {
        "approval_respond" => xd::ControlCommand::ApprovalRespond {
            approval_id: "synthetic-approval".into(),
            approve: true,
            approval_scope: None,
        },
        "question_respond" => xd::ControlCommand::QuestionRespond {
            question_id: "synthetic-question".into(),
            answers: vec![xd::QuestionAnswer { selected_labels: None, free_text: Some("synthetic-answer".into()) }],
        },
        "steer" => xd::ControlCommand::Steer { text: "synthetic-steer".into() },
        "interrupt" => xd::ControlCommand::Interrupt,
        _ => return None,
    })
}

/// ONE seat control (`performPeerControl`): `Ok((worker slug, duplicate))`
/// or `Err(bounded refusal label)`.
pub async fn seat_control(conv: &Conversation, kind: &str, live_turn: Option<&str>) -> Result<(String, bool), String> {
    let scope = scope(conv);
    let fence = held_fence(&scope.session_id).ok_or_else(|| control_refusal_label("peer_control_unavailable").to_owned())?;
    let target = seat_target(&scope.session_id, live_turn).ok_or_else(|| control_refusal_label("peer_control_unavailable").to_owned())?;
    let command = seat_command(kind).ok_or_else(|| control_refusal_label("").to_owned())?;
    xd::control(conv.client(), &scope, &fence, &target, &command)
        .await
        .map(|r| (r.slug, r.duplicate))
        .map_err(|e| control_refusal_label(e.refusal_kind().unwrap_or("peer_control_refused")).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_store::domains::peer::{Activity, PeerRow};

    #[test]
    fn row_commands_bind_the_rows_real_ids_and_fail_closed() {
        let mut r = PeerRow::opening("m#peer-a", "a", Origin::Dispatch, "00000000-0000-4000-8000-0000000000d1", 1);
        assert!(row_command(&r, RowAction::Stop, "").is_none(), "no accepted operation id");
        r.operation_id = Some("op-1".into());
        assert_eq!(row_command(&r, RowAction::Stop, ""), Some(xd::ControlCommand::Interrupt));
        assert!(row_command(&r, RowAction::Steer, "   ").is_none(), "blank steer");
        assert_eq!(
            row_command(&r, RowAction::Steer, " focus tests "),
            Some(xd::ControlCommand::Steer { text: "focus tests".into() })
        );
        assert!(row_command(&r, RowAction::Approve, "").is_none(), "no pending approval");
        r.activity = Activity::Blocked;
        r.request_kind = Some(RequestKind::Approval);
        r.request_id = Some("ap-9".into());
        assert_eq!(
            row_command(&r, RowAction::Deny, ""),
            Some(xd::ControlCommand::ApprovalRespond { approval_id: "ap-9".into(), approve: false, approval_scope: None })
        );
        assert_eq!(
            row_command(&r, RowAction::ApproveSession, ""),
            Some(xd::ControlCommand::ApprovalRespond {
                approval_id: "ap-9".into(),
                approve: true,
                approval_scope: Some("session".into())
            })
        );
    }

    #[test]
    fn refusal_labels_are_task_words() {
        assert_eq!(dispatch_refusal_label("driver_model_unavailable"), "That model is not configured on this server");
        assert_eq!(dispatch_refusal_label("anything-else"), "Couldn't start that peer.");
        assert_eq!(control_refusal_label("driver_fence_stale"), "Your control of this session expired");
        assert_eq!(control_refusal_label("peer_control_refused"), "That action was refused.");
    }
}
