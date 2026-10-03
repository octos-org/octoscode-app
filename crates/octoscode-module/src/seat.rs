//! A7 — the driver-seat handover before ONE send (the web's
//! `features/composer/composer-seat-handover.ts` planner and §6 copy, wired
//! the way `features/session/use-octos-session.ts`
//! `releaseControlSeatForUserTurn` / `resumeChatSend` wire it).
//!
//! A user turn may cross a Session whose driver mode is `external`. Exactly
//! one plan is chosen BEFORE any `turn/start` frame:
//!
//! 1. this app holds the seat (we kept the acquire proof): release it with
//!    `next: "internal"` first, then send once; a refused release sends
//!    nothing and the draft comes back;
//! 2. another app holds it (or it is parked): the send is refused without a
//!    frame — "Another app is using this session" — and the held banner's
//!    Take over runs the resume-chat three-step (acquire on the OBSERVED
//!    revision → release `internal` with that proof → send the draft once);
//! 3. a live lease under OUR driver id with no proof (a lost acquire reply):
//!    nothing may be sent with the unproven binding; wait for its expiry;
//! 4. otherwise a plain send.
//!
//! This module is pure (wire parsing, the planner, the copy) plus a small
//! per-Session cell for the kept proof, the observed disclosure and the
//! status line; the I/O lives in `flow_controller.rs`.
use std::sync::Mutex;

use serde_json::{json, Value};

/// §6 row 1: the `ExternalMasterHeld` refusal as task words.
pub const FOREIGN_SEAT_HOLDER_MESSAGE: &str = "Another app is using this session";
/// §5.2 case 3: the affordance that runs acquire → release(internal) → send
/// (the native held banner words it "Take over", board 12).
pub const RESUME_CHAT_LABEL: &str = "Resume chat";
/// §5.2 case 2: the status while the release is in flight.
pub const HANDING_BACK_CONTROL_STATUS: &str = "Handing back control…";
/// §5.2 case 3: the status while Resume chat runs.
pub const RESUMING_CHAT_STATUS: &str = "Resuming chat…";
/// §6 row 8: the release was refused — the draft was kept, nothing was sent.
pub const RELEASE_FAILED_MESSAGE: &str = "Couldn't hand back control — your message wasn't sent";
/// §5.2/20: an unproven binding under our own id.
pub const WAIT_FOR_EXPIRY_MESSAGE: &str = "Couldn't confirm — waiting for the previous attempt to expire";
/// App.tsx `resumeChat`: a failed Resume chat with no typed reason.
pub const RESUME_FAILED_MESSAGE: &str = "Couldn't resume chat — nothing was sent";
/// §5.2 "live foreign lease": the busy copy with the disclosed expiry.
pub const FOREIGN_LEASE_BUSY_TEMPLATE: &str =
    "Another app is using this session — try again when it finishes or after {time}";
/// The Core's own admission-refusal token for an external-held Session.
const EXTERNAL_MASTER_HELD: &str = "ExternalMasterHeld";

/// Does this raw send failure mean a foreign controller holds the Session?
pub fn is_external_master_held(message: &str) -> bool {
    message.contains(EXTERNAL_MASTER_HELD)
}

/// §6: bound ONE send failure to operator copy (`boundedTurnAdmissionError`):
/// a seat refusal collapses to the human message, anything else passes.
pub fn bounded_turn_admission_error(message: &str) -> String {
    if is_external_master_held(message) {
        FOREIGN_SEAT_HOLDER_MESSAGE.to_owned()
    } else {
        message.to_owned()
    }
}

/// `foreignLeaseBusyCopy`: the busy copy with the expiry as a local clock
/// time (`toLocaleTimeString`).
pub fn foreign_lease_busy_copy(expires_at_ms: u64) -> String {
    use chrono::TimeZone;
    let time = chrono::Local
        .timestamp_millis_opt(expires_at_ms as i64)
        .single()
        .map(|t| t.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| crate::i18n::tr("the lease expires").to_owned());
    crate::i18n::tr_with(FOREIGN_LEASE_BUSY_TEMPLATE, &[("time", &time)])
}

/// A driver binding as disclosed on the wire (`DriverBindingView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub driver_id: String,
    pub epoch: u64,
    pub revision: u64,
    pub lease_expires_at_ms: u64,
}

/// A `session/driver/get` result (`SessionDriverGetView`): the mode and the
/// binding, never any proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    pub external: bool,
    pub binding: Option<Binding>,
}

fn non_empty(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.trim().is_empty()).map(str::to_owned)
}

fn binding(v: &Value) -> Option<Binding> {
    Some(Binding {
        driver_id: non_empty(v.get("driver_id")?)?,
        epoch: v.get("epoch")?.as_u64()?,
        revision: v.get("revision")?.as_u64()?,
        lease_expires_at_ms: v.get("lease_expires_at_ms")?.as_u64()?,
    })
}

/// Strict `parseSessionDriverGetResult`: `mode` internal|external; external
/// needs a binding; a retained internal binding must be inactive (lease 0).
pub fn parse_disclosure(v: &Value) -> Option<Disclosure> {
    let external = match v.get("mode")?.as_str()? {
        "external" => true,
        "internal" => false,
        _ => return None,
    };
    let raw = v.get("binding").filter(|b| !b.is_null());
    let binding = match raw {
        None if external => return None,
        None => None,
        Some(b) => Some(binding(b)?),
    };
    if !external && binding.as_ref().is_some_and(|b| b.lease_expires_at_ms != 0) {
        return None;
    }
    Some(Disclosure { external, binding })
}

/// The acquire proof (`DriverAcquireView.capability` + its binding). The
/// control token is never printed (`Debug` redacts it) and travels only in
/// the release frame.
#[derive(Clone, PartialEq, Eq)]
pub struct Proof {
    pub driver_id: String,
    pub epoch: u64,
    token: String,
    pub revision: u64,
    pub lease_expires_at_ms: u64,
}

impl Proof {
    /// The proof of a seat taken through the typed driver leaf (the console's
    /// "Acquire seat" or the Fleet's Start, `screens::fleet_driver`): the web
    /// holds ONE `controlAcquire` for the console and the composer handover.
    pub fn from_acquire(driver_id: &str, epoch: u64, token: &str, revision: u64, lease_expires_at_ms: u64) -> Self {
        Self { driver_id: driver_id.to_owned(), epoch, token: token.to_owned(), revision, lease_expires_at_ms }
    }
}

impl std::fmt::Debug for Proof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Proof")
            .field("driver_id", &self.driver_id)
            .field("epoch", &self.epoch)
            .field("token", &"<redacted>")
            .field("revision", &self.revision)
            .finish()
    }
}

/// `parseDriverAcquireResult`: a non-empty control token and a binding for
/// the driver id WE asked for (a foreign driver's reply is never accepted).
pub fn parse_acquire(reply: &Value, expected_driver: &str) -> Option<Proof> {
    let token = non_empty(reply.get("control_token")?)?;
    let b = binding(reply.get("binding")?)?;
    if b.driver_id != expected_driver {
        return None;
    }
    Some(Proof {
        driver_id: b.driver_id,
        epoch: b.epoch,
        token,
        revision: b.revision,
        lease_expires_at_ms: b.lease_expires_at_ms,
    })
}

/// `session/driver/release` with the proof and `next: "internal"` — never
/// `external`, which would park the binding and keep chat refused.
pub fn release_params(session: &str, p: &Proof) -> Value {
    json!({
        "session_id": session,
        "driver_id": p.driver_id,
        "epoch": p.epoch,
        "control_token": p.token,
        "expected_revision": p.revision,
        "next": "internal",
    })
}

/// `parseDriverReleaseResult` for `next: "internal"`: the reply must echo
/// the mode and omit the binding.
pub fn release_confirmed(reply: &Value) -> bool {
    reply.get("mode").and_then(Value::as_str) == Some("internal")
        && reply.get("binding").map_or(true, Value::is_null)
}

/// The ONE handover plan for a user turn (`planComposerSeatHandover`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Send,
    ReleaseThenSend(Proof),
    WaitForExpiry(u64),
    ResumeChat { expected_revision: u64, foreign_lease_expires_at_ms: Option<u64> },
}

/// Decide the plan (pure, fail closed): a PROVEN own seat hands back first;
/// an UNPROVEN live lease under our id waits; a foreign/parked binding only
/// through Resume chat; with neither, a plain send. The inputs are derived
/// as `releaseControlSeatForUserTurn` derives them.
pub fn plan(own: Option<&Proof>, observed: Option<&Disclosure>, own_driver: &str, now_ms: u64) -> Plan {
    if let Some(p) = own.filter(|p| p.driver_id == own_driver) {
        return Plan::ReleaseThenSend(p.clone());
    }
    let Some(b) = observed.filter(|d| d.external).and_then(|d| d.binding.as_ref()) else {
        return Plan::Send;
    };
    let live = b.lease_expires_at_ms > now_ms;
    if b.driver_id == own_driver && live {
        return Plan::WaitForExpiry(b.lease_expires_at_ms);
    }
    Plan::ResumeChat {
        expected_revision: b.revision,
        foreign_lease_expires_at_ms: (b.driver_id != own_driver && live).then_some(b.lease_expires_at_ms),
    }
}

/// The refusal message a non-send plan reports (bounded by the caller).
pub fn refusal(plan: &Plan) -> Option<String> {
    match plan {
        Plan::Send | Plan::ReleaseThenSend(_) => None,
        Plan::WaitForExpiry(_) => Some(WAIT_FOR_EXPIRY_MESSAGE.to_owned()),
        Plan::ResumeChat { foreign_lease_expires_at_ms: Some(t), .. } => Some(format!(
            "turn admission refused for this session: ExternalMasterHeld ({})",
            foreign_lease_busy_copy(*t)
        )),
        Plan::ResumeChat { .. } => Some("turn admission refused for this session: ExternalMasterHeld".to_owned()),
    }
}

// ------------------------------------------------------------ per Session

#[derive(Default)]
struct Cell {
    proof: Option<Proof>,
    observed: Option<Disclosure>,
    /// (text, is_error): the in-flight status or the last failure.
    status: Option<(String, bool)>,
}

static CELLS: Mutex<Vec<(String, Cell)>> = Mutex::new(Vec::new());

fn with<R>(session: &str, f: impl FnOnce(&mut Cell) -> R) -> R {
    let mut cells = CELLS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((_, c)) = cells.iter_mut().find(|(s, _)| s == session) {
        return f(c);
    }
    cells.push((session.to_owned(), Cell::default()));
    let (_, c) = cells.last_mut().expect("just pushed");
    f(c)
}

/// Keep the acquire proof this app now holds for `session`.
pub fn keep_proof(session: &str, proof: Proof) {
    with(session, |c| c.proof = Some(proof));
}

/// The proof this app holds for `session`, if any.
pub fn proof(session: &str) -> Option<Proof> {
    with(session, |c| c.proof.clone())
}

/// Drop the proof (the seat was handed back, or it is gone).
pub fn drop_proof(session: &str) {
    with(session, |c| c.proof = None);
}

/// Record the last `session/driver/get` disclosure (`None`: unreadable).
pub fn observe(session: &str, d: Option<Disclosure>) {
    with(session, |c| c.observed = d);
}

/// The last observed disclosure.
pub fn observed(session: &str) -> Option<Disclosure> {
    with(session, |c| c.observed.clone())
}

/// Set (or clear) the composer's seat status line.
pub fn set_status(session: &str, status: Option<(&str, bool)>) {
    with(session, |c| c.status = status.map(|(t, e)| (t.to_owned(), e)));
}

/// The seat status line for `session`: (text, is_error).
pub fn status(session: &str) -> Option<(String, bool)> {
    with(session, |c| c.status.clone())
}

/// Wall-clock milliseconds (the lease clock).
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "octoscode-native";
    const NOW: u64 = 1_700_000_000_000;

    fn proof() -> Proof {
        parse_acquire(
            &json!({"control_token": "tok-1", "recovery": "none",
                    "binding": {"driver_id": ME, "epoch": 3, "revision": 12, "lease_expires_at_ms": NOW + 120_000}}),
            ME,
        )
        .expect("a valid acquire reply")
    }

    fn external(driver: &str, revision: u64, lease: u64) -> Disclosure {
        parse_disclosure(&json!({"mode": "external", "recovery": "none",
            "binding": {"driver_id": driver, "epoch": 2, "revision": revision, "lease_expires_at_ms": lease}}))
        .expect("a valid disclosure")
    }

    // composer-seat-handover.test.ts:68/:81/:88
    #[test]
    fn copy_is_bounded_and_worded_as_the_web_words_it() {
        assert!(is_external_master_held("turn admission refused for this session: ExternalMasterHeld"));
        assert_eq!(
            bounded_turn_admission_error("turn admission refused for this session: ExternalMasterHeld"),
            FOREIGN_SEAT_HOLDER_MESSAGE
        );
        assert_eq!(bounded_turn_admission_error("some other failure"), "some other failure");
        assert_eq!(bounded_turn_admission_error(""), "");
        assert_eq!(RESUME_CHAT_LABEL, "Resume chat");
        assert_eq!(HANDING_BACK_CONTROL_STATUS, "Handing back control…");
        assert_eq!(RELEASE_FAILED_MESSAGE, "Couldn't hand back control — your message wasn't sent");
        assert_eq!(FOREIGN_SEAT_HOLDER_MESSAGE, "Another app is using this session");
    }

    // :101 this-tab seat + send: release(next:internal) first, then send once
    #[test]
    fn a_proven_own_seat_releases_internal_before_the_send() {
        let p = proof();
        let plan = plan(Some(&p), Some(&external(ME, 12, NOW + 120_000)), ME, NOW);
        assert_eq!(plan, Plan::ReleaseThenSend(p.clone()));
        let wire = release_params("s1", &p);
        assert_eq!(wire["next"], "internal", "never park with external");
        assert_eq!(wire["control_token"], "tok-1");
        assert_eq!(wire["expected_revision"], 12);
        assert_eq!(wire["epoch"], 3);
        assert!(!format!("{p:?}").contains("tok-1"), "the proof never prints");
    }

    // :119 foreign holder + send: resume-chat three-step, never a silent send
    #[test]
    fn a_foreign_holder_plans_resume_chat_and_refuses_the_plain_send() {
        let plan = plan(None, Some(&external("octos-tui", 7, 0)), ME, NOW);
        assert_eq!(plan, Plan::ResumeChat { expected_revision: 7, foreign_lease_expires_at_ms: None });
        let msg = refusal(&plan).expect("refused");
        assert_eq!(bounded_turn_admission_error(&msg), FOREIGN_SEAT_HOLDER_MESSAGE);
    }

    // :134 no seat, nobody holds: plain send, no frame
    #[test]
    fn no_seat_and_nobody_holding_is_a_plain_send() {
        assert_eq!(plan(None, None, ME, NOW), Plan::Send);
        let internal = parse_disclosure(&json!({"mode": "internal", "recovery": "none", "binding": null})).unwrap();
        assert_eq!(plan(None, Some(&internal), ME, NOW), Plan::Send);
    }

    // :143 a LIVE OUR-ID lease with NO proof waits for expiry
    #[test]
    fn a_live_own_lease_without_proof_waits_for_its_expiry() {
        let plan = plan(None, Some(&external(ME, 9, NOW + 30_000)), ME, NOW);
        assert_eq!(plan, Plan::WaitForExpiry(NOW + 30_000));
        assert_eq!(refusal(&plan).as_deref(), Some(WAIT_FOR_EXPIRY_MESSAGE));
    }

    // :155 a live FOREIGN lease still plans resume-chat with the busy note
    #[test]
    fn a_live_foreign_lease_still_plans_resume_chat_with_the_busy_note() {
        let plan = plan(None, Some(&external("octos-tui", 7, NOW + 60_000)), ME, NOW);
        assert_eq!(
            plan,
            Plan::ResumeChat { expected_revision: 7, foreign_lease_expires_at_ms: Some(NOW + 60_000) }
        );
        let msg = refusal(&plan).unwrap();
        assert!(msg.contains("try again when it finishes or after"), "{msg}");
        assert_eq!(bounded_turn_admission_error(&msg), FOREIGN_SEAT_HOLDER_MESSAGE);
    }

    #[test]
    fn the_wire_parsers_fail_closed() {
        // External without a binding, an active internal binding, an unknown
        // mode: no disclosure.
        assert!(parse_disclosure(&json!({"mode": "external"})).is_none());
        assert!(parse_disclosure(&json!({"mode": "internal",
            "binding": {"driver_id": "x", "epoch": 1, "revision": 1, "lease_expires_at_ms": 5}}))
        .is_none());
        assert!(parse_disclosure(&json!({"mode": "elsewhere"})).is_none());
        // An acquire reply for another driver, or without a token, is no proof.
        let foreign = json!({"control_token": "t", "binding":
            {"driver_id": "octos-tui", "epoch": 1, "revision": 2, "lease_expires_at_ms": 9}});
        assert!(parse_acquire(&foreign, ME).is_none());
        let tokenless = json!({"binding": {"driver_id": ME, "epoch": 1, "revision": 2, "lease_expires_at_ms": 9}});
        assert!(parse_acquire(&tokenless, ME).is_none());
        // A release must echo internal and drop the binding.
        assert!(release_confirmed(&json!({"mode": "internal", "recovery": "none"})));
        assert!(!release_confirmed(&json!({"mode": "external", "recovery": "none"})));
        assert!(!release_confirmed(&json!({"mode": "internal",
            "binding": {"driver_id": ME, "epoch": 1, "revision": 2, "lease_expires_at_ms": 0}})));
    }
}
