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
//! * the seat itself (`use-octos-session.ts:1744-2095`): opt-in — only an
//!   explicit "Acquire seat" (or the Fleet's Start) takes a lease
//!   ([`acquire_seat`], CAS on the walked revision, a COLD internal session
//!   at revision 0); the lease is renewed every 45 s while held
//!   ([`spawn_renew`]: three ticks inside one 120 s lease) and a typed
//!   `driver_fence_stale` from ANY call drops it ([`note_refusal`], the §6
//!   label stays); "Release seat" parks it (`next: external`,
//!   [`release_seat`]); a chat send hands it back first (`next: internal`,
//!   the composer's seat gate, [`handed_back`]). The SAME driver id and the
//!   SAME proof serve the composer's handover (`crate::seat`), the session
//!   pane and the Fleet, as the web's one `controlDriverId` + `controlAcquire`.
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

/// `PEER_DRIVER_ID_PREFIX` — the native client's own prefix
/// (`octoscode-web:` in the browser).
pub const DRIVER_ID_PREFIX: &str = "octoscode-native:";

/// (the id, whether it is already persisted).
static DRIVER_ID: Mutex<Option<(String, bool)>> = Mutex::new(None);

fn driver_id_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("OCTOSCODE_DRIVER_ID_PATH") {
        return p.into();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".octoscode").join("driver-id")
}

/// `readStoredPeerDriverId`: the prefix + a protocol UUID, nothing else (a
/// malformed id would fail the Core's identity check and wedge the seat).
pub fn valid_driver_id(s: &str) -> bool {
    s.strip_prefix(DRIVER_ID_PREFIX)
        .is_some_and(|u| octoscode_client::protocol_id::is_protocol_uuid(&serde_json::Value::String(u.to_owned())))
}

/// `OCTOSCODE_DRIVER_ID` pins the id for a test run or an isolated walk
/// (never persisted); a malformed value is ignored.
fn pinned_driver_id() -> Option<String> {
    std::env::var("OCTOSCODE_DRIVER_ID").ok().map(|s| s.trim().to_owned()).filter(|s| valid_driver_id(s))
}

/// `stablePeerDriverId` (`use-octos-session.ts:525-540`): the persisted id
/// when it is well-formed, else a fresh `octoscode-native:<uuid>` for this
/// run. Reading it never writes: the id is persisted only when a lease is
/// about to be taken under it ([`acquiring_driver_id`]) — a binding under
/// our id can only exist once the id is stored.
pub fn driver_id() -> String {
    if let Some(id) = pinned_driver_id() {
        return id;
    }
    let mut g = DRIVER_ID.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((id, _)) = g.as_ref() {
        return id.clone();
    }
    let stored = std::fs::read_to_string(driver_id_path()).ok().map(|s| s.trim().to_owned()).filter(|s| valid_driver_id(s));
    let entry = match stored {
        Some(id) => (id, true),
        None => (format!("{DRIVER_ID_PREFIX}{}", xd::new_operation_id()), false),
    };
    *g = Some(entry.clone());
    entry.0
}

/// The id an ACQUIRE presents: [`driver_id`], persisted first (best-effort,
/// fail-open — a blocked store still yields a usable id this run).
pub fn acquiring_driver_id() -> String {
    let id = driver_id();
    if pinned_driver_id().is_some() {
        return id;
    }
    let mut g = DRIVER_ID.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((_, persisted @ false)) = g.as_mut() {
        let path = driver_id_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, &id);
        *persisted = true;
    }
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

/// `peerControlAdmitted`: `peer/control` AND `external_driver_v1`.
pub fn peer_control_admitted(store: &Store) -> bool {
    let (m, f) = (store.domains.config.supported_methods(), store.capabilities());
    xd::admitted(&m, &f, xd::PEER_CONTROL)
}

/// `peerControlAdmitted && peerDispatchAdmitted` — the advertised half of
/// "remote control of peers" (`FleetView.tsx:238-241`), and the controller
/// console's own gate (`PeerControllerPanel.tsx:337`).
pub fn control_advertised(store: &Store) -> bool {
    let (m, f) = (store.domains.config.supported_methods(), store.capabilities());
    xd::admitted(&m, &f, xd::PEER_CONTROL) && xd::admitted(&m, &f, xd::PEER_DISPATCH)
}

/// `deriveControlReadiness` (`control-readiness.ts:10-26`), the record's
/// fail-closed projection: `peer/control` + `external_driver_v1` advertised
/// AND a COMPLETE (known) inventory walk for the active session — any mode,
/// binding or none, so a COLD internal session can still seat. Unknown caps,
/// a missing half, or a loading / failed / absent walk = unavailable.
pub fn control_ready(store: &Store) -> bool {
    peer_control_admitted(store) && complete_inventory(store).is_some()
}

/// `controlSupported` (`FleetView.tsx:242-243`): the Fleet's remote control
/// = both methods advertised AND the record ready.
pub fn control_supported(store: &Store) -> bool {
    control_advertised(store) && control_ready(store)
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
/// The session whose lease was revoked under us (`driver_fence_stale` from a
/// renew or any control call): the §6 label stays until the next acquire.
static EXPIRED: Mutex<Option<String>> = Mutex::new(None);

/// Whether this app holds the seat for `session`.
pub fn seat_held(session: &str) -> bool {
    SEAT.lock().unwrap().as_ref().is_some_and(|s| s.session_id == session)
}

/// Whether the operator parked the seat for `session`.
pub fn seat_parked(session: &str) -> bool {
    PARKED.lock().unwrap().as_deref() == Some(session)
}

/// Whether `session`'s lease was revoked under us (`peerControlFenceStaleIn`:
/// the seat was dropped, the label "Your control of this session expired"
/// stays until a new acquire).
pub fn seat_expired(session: &str) -> bool {
    EXPIRED.lock().unwrap().as_deref() == Some(session)
}

/// The seat's proof in the composer's handover cell (`crate::seat`): the web
/// reads ONE `controlAcquire` for both the console and
/// `releaseControlSeatForUserTurn`, so a chat send while this app holds the
/// seat hands it back (`next: internal`) before its one `turn/start`.
fn share_proof(session: &str, view: &xd::AcquireView) {
    crate::seat::keep_proof(
        session,
        crate::seat::Proof::from_acquire(
            &view.fence.driver_id,
            view.fence.epoch,
            view.fence.reveal(),
            view.binding.revision,
            view.binding.lease_expires_at_ms,
        ),
    );
}

/// `dropControlSeat` (`use-octos-session.ts:2017-2021`): a `driver_fence_stale`
/// means the held proof is dead (another app acquired, or the lease lapsed):
/// the seat is dropped WITHOUT a frame — never retried with the same fence —
/// and the bounded label stays. A no-op when `session` holds no seat.
pub fn drop_stale(session: &str) {
    let mut g = SEAT.lock().unwrap();
    if g.as_ref().is_some_and(|s| s.session_id == session) {
        *g = None;
        drop(g);
        crate::seat::drop_proof(session);
        *EXPIRED.lock().unwrap() = Some(session.to_owned());
        makepad_widgets::log!("[octoscode] seat: {session}: driver_fence_stale — the seat is dropped");
        makepad_widgets::SignalToUI::set_ui_signal();
    }
}

/// `settleControlState`: a typed `driver_fence_stale` from ANY call (renew,
/// control, dispatch, a handover's release) never leaves a dead fence
/// mounted — nor a dead proof in the composer's cell.
pub fn note_refusal(session: &str, kind: Option<&str>) {
    if kind == Some("driver_fence_stale") {
        drop_stale(session);
        crate::seat::drop_proof(session);
    }
}

/// The composer's handover released the seat (`next: internal`, confirmed or
/// reconciled): `parkControlSeat(record)` — the console re-offers Acquire.
pub fn handed_back(session: &str) {
    let mut g = SEAT.lock().unwrap();
    if g.as_ref().is_some_and(|s| s.session_id == session) {
        *g = None;
        *PARKED.lock().unwrap() = Some(session.to_owned());
    }
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

/// Test seam: drop the held seat (and its proof in the composer's cell).
pub fn reset_seat() {
    if let Some(s) = SEAT.lock().unwrap().take() {
        crate::seat::drop_proof(&s.session_id);
    }
    *PARKED.lock().unwrap() = None;
    *EXPIRED.lock().unwrap() = None;
}

/// ONE `session/driver/acquire` with CAS on the walked revision
/// (`peerControlAcquireInput`: the disclosure's binding revision, 0 for an
/// unbound — cold — session). No complete inventory → no CAS basis → no
/// frame (`buildFleetStartAcquire` returns null). The reply must name OUR
/// driver id (`acquirePeerControlFence`); the held seat's proof is shared
/// with the composer's handover and its lease renewed until it ends.
pub async fn acquire_seat(conv: &Conversation) -> Result<(), xd::DriverError> {
    let store = &conv.store;
    let Some(FleetInventory::Complete { disclosure, .. }) = complete_inventory(store) else {
        return Err(xd::DriverError::InvalidArgs(xd::SESSION_DRIVER_ACQUIRE));
    };
    let revision = disclosure.binding.as_ref().map(|b| b.2).unwrap_or(0);
    let scope = scope(conv);
    let view = xd::acquire(conv.client(), &scope, &acquiring_driver_id(), revision).await?;
    let epoch = view.fence.epoch;
    share_proof(&scope.session_id, &view);
    let previous = SEAT.lock().unwrap().replace(Seat { session_id: scope.session_id.clone(), view });
    if let Some(old) = previous.filter(|old| old.session_id != scope.session_id) {
        // One seat at a time: another Session's hold ends here (no frame; its
        // renewals stop, its proof leaves the composer's cell).
        crate::seat::drop_proof(&old.session_id);
    }
    *PARKED.lock().unwrap() = None;
    *EXPIRED.lock().unwrap() = None;
    spawn_renew(conv, &scope.session_id, epoch);
    Ok(())
}

/// ONE `session/driver/release {next: "external"}` (`planPeerSeatRelease`):
/// the seat is PARKED until an explicit acquire. A stale fence drops it.
pub async fn release_seat(conv: &Conversation) -> Result<(), xd::DriverError> {
    let scope = scope(conv);
    let (fence, revision) = {
        let g = SEAT.lock().unwrap();
        let Some(s) = g.as_ref().filter(|s| s.session_id == scope.session_id) else {
            return Err(xd::DriverError::InvalidArgs(xd::SESSION_DRIVER_RELEASE));
        };
        (s.view.fence.clone(), s.view.binding.revision)
    };
    if let Err(e) = xd::release(conv.client(), &scope, &fence, revision, xd::DriverMode::External).await {
        note_refusal(&scope.session_id, e.refusal_kind());
        return Err(e);
    }
    *SEAT.lock().unwrap() = None;
    crate::seat::drop_proof(&scope.session_id);
    *PARKED.lock().unwrap() = Some(scope.session_id);
    Ok(())
}

/// ONE `session/driver/renew` of the held lease (`peerControlRenewParams`:
/// the held fence + the same bounded lease). A typed `driver_fence_stale`
/// drops the seat; any other failure keeps it (the next tick retries).
pub async fn renew_seat(conv: &Conversation) -> Result<u64, xd::DriverError> {
    let scope = scope(conv);
    let fence = held_fence(&scope.session_id).ok_or(xd::DriverError::InvalidArgs(xd::SESSION_DRIVER_RENEW))?;
    match xd::renew(conv.client(), &scope, &fence).await {
        Ok(lease) => {
            if let Some(s) = SEAT.lock().unwrap().as_mut().filter(|s| s.session_id == scope.session_id && s.view.fence == fence) {
                s.view.binding.lease_expires_at_ms = lease;
            }
            Ok(lease)
        }
        Err(e) => {
            // Only for the SAME fence: a re-acquire in between owns a new one.
            if held_fence(&scope.session_id).as_ref() == Some(&fence) {
                note_refusal(&scope.session_id, e.refusal_kind());
            }
            Err(e)
        }
    }
}

/// `PEER_CONTROL_RENEW_INTERVAL_MS` (`use-octos-session.ts:484`): 45 s — three
/// ticks inside one 120 s lease, so one missed tick never lets it lapse.
/// `OCTOSCODE_SEAT_RENEW_MS` shortens it for a fixture walk.
pub fn renew_interval() -> Duration {
    let ms = std::env::var("OCTOSCODE_SEAT_RENEW_MS").ok().and_then(|v| v.parse::<u64>().ok()).filter(|ms| *ms >= 50);
    Duration::from_millis(ms.unwrap_or(45_000))
}

/// The renewal cadence for ONE held lease (the web's `setInterval` effect
/// keyed on the acquire): it ends when this seat is released, handed back,
/// dropped or replaced by a new acquire (a new epoch), or the conversation
/// is gone. Needs the shared handle ([`Conversation::attach`]) and a runtime.
fn spawn_renew(conv: &Conversation, session: &str, epoch: u64) {
    let (Some(me), Ok(rt)) = (conv.shared(), tokio::runtime::Handle::try_current()) else {
        return;
    };
    let weak = std::sync::Arc::downgrade(&me);
    drop(me);
    let session = session.to_owned();
    rt.spawn(async move {
        let same = |s: &str| SEAT.lock().unwrap().as_ref().is_some_and(|x| x.session_id == s && x.view.fence.epoch == epoch);
        loop {
            tokio::time::sleep(renew_interval()).await;
            if !same(&session) {
                return;
            }
            let Some(conv) = weak.upgrade() else { return };
            if conv.session_id() != session {
                // "A DIFFERENT Session is selected: the held lease belongs to
                // the old one" — the hold is dropped without a frame.
                let mut g = SEAT.lock().unwrap();
                if g.as_ref().is_some_and(|x| x.session_id == session && x.view.fence.epoch == epoch) {
                    *g = None;
                    drop(g);
                    crate::seat::drop_proof(&session);
                }
                return;
            }
            match renew_seat(&conv).await {
                Ok(_) => makepad_widgets::log!("[octoscode] seat: {session}: lease renewed"),
                Err(e) => {
                    makepad_widgets::log!("[octoscode] seat: {session}: renew: {e}");
                    if !same(&session) {
                        // The lease is gone: re-walk, so the disclosure (and
                        // the next CAS) name who holds the session now.
                        let _ = load_inventory(&conv).await;
                        crate::screens::board3::session_pane::mirror_inventory(&conv.store);
                    }
                }
            }
            makepad_widgets::SignalToUI::set_ui_signal();
        }
    });
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
            note_refusal(&scope.session_id, e.refusal_kind());
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

/// A30 — the ids one row control was DRAWN for (the sidebar peer dock's
/// `peer_dock::Drawn`, the Fleet's `FleetRow::target`): the pending request
/// it showed, the accepted operation and the adopted turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawnTarget {
    pub request_id: Option<String>,
    pub operation_id: Option<String>,
    pub turn_id: String,
}

/// The bounded copy of a refused stale decision whose request was RESOLVED
/// (nothing is pending now): FLEET_ZH "Already handled".
pub const STALE_DRAWN: &str = "Already handled";

/// A30 follow-up — the bounded copy of a refused control whose row CHANGED
/// since it was drawn: another request replaced the one it showed, or the
/// peer moved to another turn / operation. Shown on the card (never a
/// silent no-op: a person who taps Stop and sees nothing assumes it
/// stopped); `i18n/native.rs` carries its Chinese.
pub const CHANGED_DRAWN: &str = "This peer changed. Review it and tap again.";

/// A30 follow-up — why a control drawn with `drawn`'s ids must NOT act on
/// `row` now (`None` = it acts):
///
/// * the accepted operation or the adopted turn moved (the control's
///   target is gone) -> [`CHANGED_DRAWN`];
/// * a decision (Approve once / Approve for session / Deny) or an Answer is
///   bound to the request it was drawn for: another pending request ->
///   [`CHANGED_DRAWN`], none -> [`STALE_DRAWN`];
/// * Stop and Steer target the TURN, not a request: on the same operation
///   and turn they re-resolve to the current row (one frame, the right
///   target) even when the pending approval was re-issued meanwhile.
pub fn drawn_refusal(row: &octoscode_store::domains::peer::PeerRow, action: RowAction, drawn: &DrawnTarget) -> Option<&'static str> {
    if row.operation_id != drawn.operation_id || row.turn_id != drawn.turn_id {
        return Some(CHANGED_DRAWN);
    }
    let kind = match action {
        RowAction::Approve | RowAction::ApproveSession | RowAction::Deny => RequestKind::Approval,
        RowAction::Answer => RequestKind::Question,
        RowAction::Stop | RowAction::Steer => return None,
    };
    if drawn.request_id.is_some() && row.request_kind == Some(kind) && row.request_id == drawn.request_id {
        return None;
    }
    Some(if row.request_id.is_some() { CHANGED_DRAWN } else { STALE_DRAWN })
}

/// Whether a control drawn with `drawn`'s ids acts on `row` now
/// ([`drawn_refusal`] is `None`).
pub fn still_drawn(row: &octoscode_store::domains::peer::PeerRow, action: RowAction, drawn: &DrawnTarget) -> bool {
    drawn_refusal(row, action, drawn).is_none()
}

/// A30 follow-up — the walks' deterministic seam for "the row changes
/// between the tap and the send" (a loaded machine's real window): with
/// `OCTOSCODE_PEER_CONTROL_DELAY_MS` set, a DRAWN control waits that long
/// before its send-time check, so a replay hook can change the row inside
/// the window. Inert when unset (production: no delay).
pub fn drawn_control_delay() -> Option<Duration> {
    static DELAY: std::sync::OnceLock<Option<Duration>> = std::sync::OnceLock::new();
    *DELAY.get_or_init(|| {
        std::env::var("OCTOSCODE_PEER_CONTROL_DELAY_MS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|ms| *ms > 0)
            .map(Duration::from_millis)
    })
}

/// EXACTLY ONE `peer/control` frame for one row activation (never retried).
/// `Ok(ack copy)` — "Sent" / "Stop requested" (an ACKNOWLEDGMENT, never an
/// outcome); `Err(bounded copy)` — a refusal label or a fail-closed reason
/// (no frame).
pub async fn row_control(conv: &Conversation, identity: &str, action: RowAction, text: &str) -> Result<String, String> {
    row_control_drawn(conv, identity, action, text, None).await
}

/// A30 — [`row_control`] for a control drawn with `drawn`'s ids (the
/// sidebar peer dock and the Fleet's row cards): the row is re-read right
/// before the frame and the action is REFUSED with no frame and a bounded
/// reason ([`drawn_refusal`]: [`CHANGED_DRAWN`] / [`STALE_DRAWN`]) when it no
/// longer acts on that row; the command is then built from the current row,
/// which for a decision IS the drawn approval id.
pub async fn row_control_drawn(
    conv: &Conversation,
    identity: &str,
    action: RowAction,
    text: &str,
    drawn: Option<&DrawnTarget>,
) -> Result<String, String> {
    let store = &conv.store;
    let scope = scope(conv);
    if drawn.is_some() {
        if let Some(delay) = drawn_control_delay() {
            tokio::time::sleep(delay).await;
        }
    }
    let row = store.domains.peer.row(identity).ok_or_else(|| "This peer is no longer in the roster.".to_owned())?;
    if let Some(reason) = drawn.and_then(|d| drawn_refusal(&row, action, d)) {
        return Err(reason.to_owned());
    }
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
            note_refusal(&scope.session_id, e.refusal_kind());
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
            note_refusal(&scope.session_id, e.refusal_kind());
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
    xd::control(conv.client(), &scope, &fence, &target, &command).await.map(|r| (r.slug, r.duplicate)).map_err(|e| {
        note_refusal(&scope.session_id, e.refusal_kind());
        control_refusal_label(e.refusal_kind().unwrap_or("peer_control_refused")).to_owned()
    })
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

    /// A30 — a dock control drawn for (approval id, operation, turn) acts
    /// only while the row still shows exactly those ids.
    #[test]
    fn a_drawn_target_refuses_once_the_rows_ids_moved() {
        let mut r = PeerRow::opening("m#peer-a", "a", Origin::Dispatch, "00000000-0000-4000-8000-0000000000d1", 1);
        r.operation_id = Some("op-1".into());
        r.activity = Activity::Blocked;
        r.request_kind = Some(RequestKind::Approval);
        r.request_id = Some("ap-1".into());
        let drawn = DrawnTarget { request_id: Some("ap-1".into()), operation_id: Some("op-1".into()), turn_id: r.turn_id.clone() };
        let all = [RowAction::Approve, RowAction::ApproveSession, RowAction::Deny, RowAction::Stop];
        assert!(all.iter().all(|a| still_drawn(&r, *a, &drawn)));
        let decisions = [RowAction::Approve, RowAction::ApproveSession, RowAction::Deny];
        let mut moved = r.clone();
        moved.request_id = Some("ap-2".into());
        assert!(
            decisions.iter().all(|a| drawn_refusal(&moved, *a, &drawn) == Some(CHANGED_DRAWN)),
            "a re-issued approval: every decision refused, visibly"
        );
        assert!(still_drawn(&moved, RowAction::Stop, &drawn), "Stop re-resolves: same operation and turn");
        let mut turned = r.clone();
        turned.turn_id = "00000000-0000-4000-8000-0000000000d2".into();
        assert!(all.iter().all(|a| drawn_refusal(&turned, *a, &drawn) == Some(CHANGED_DRAWN)), "a replacement turn: refused");
        let mut redispatched = r.clone();
        redispatched.operation_id = Some("op-2".into());
        assert!(all.iter().all(|a| !still_drawn(&redispatched, *a, &drawn)), "another operation: refused");
        let mut resolved = r.clone();
        resolved.request_id = None;
        resolved.request_kind = None;
        assert_eq!(drawn_refusal(&resolved, RowAction::Deny, &drawn), Some(STALE_DRAWN), "resolved: Already handled");
        let idle = DrawnTarget { request_id: None, ..drawn.clone() };
        assert!(!still_drawn(&resolved, RowAction::Approve, &idle), "an approval decision needs a pending approval");
    }

    #[test]
    fn refusal_labels_are_task_words() {
        assert_eq!(dispatch_refusal_label("driver_model_unavailable"), "That model is not configured on this server");
        assert_eq!(dispatch_refusal_label("anything-else"), "Couldn't start that peer.");
        assert_eq!(control_refusal_label("driver_fence_stale"), "Your control of this session expired");
        assert_eq!(control_refusal_label("peer_control_refused"), "That action was refused.");
    }
}
