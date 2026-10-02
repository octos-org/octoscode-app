//! A8 — the external-driver DISCLOSURE: the read-only walk of a session's
//! `session/driver/get` operations chain, ported from the web's
//! `features/session/driver-discovery.ts` (+ the strict decoders it relies on,
//! `packages/client/src/external-driver.ts:146-197`
//! `parseSessionDriverGetResult` and `external-driver-operations.ts:140-421`
//! `parseDriverOperationsPageResult`).
//!
//! Presentation of server facts ONLY (`driver-discovery.ts:10-14`): no control
//! proof, no token, no raw payload ever reaches a label. The walk starts from
//! `operations: {limit: 50}` (no cursor), follows ONLY the returned
//! `next_cursor`, and is `Complete` only when one snapshot exhausts with
//! strictly increasing UTF-8-ordered unique operation ids, no repeated cursor
//! and one consistent mode/binding/recovery disclosure across pages. Every
//! bound (32 pages / 3200 rows / 30 s) is an explicit error, never a
//! truncation. A rejected read keeps ONLY an allowlisted typed refusal kind
//! (`driver-discovery.ts:23-27`, the server error's `data.kind`); anything
//! else is the generic failure — the server's message is never echoed.
//!
//! Rendered by the Session settings pane's Advanced section
//! (`screens::board3::session_pane`), the web's `SessionConfigPane` Advanced
//! body (`App.tsx:3307-3350` + `SessionControlBar.tsx:786-835`
//! `DriverControllerDisclosure`).
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// `DRIVER_DISCOVERY_LIMITS` (`driver-discovery.ts:16-21`).
pub const PAGE_SIZE: u64 = 50;
pub const MAX_PAGES: usize = 32;
pub const MAX_ROWS: usize = 3200;
pub const TIMEOUT: Duration = Duration::from_secs(30);
/// `DRIVER_OPERATIONS_LIMITS` (`external-driver-operations.ts:22-29`).
pub const MAX_PAGE_ROWS: usize = 100;
pub const MAX_CURSOR_BYTES: usize = 4096;
pub const MAX_PAGE_BYTES: usize = 256 * 1024;

/// The ONLY refusal kinds a discovery read may surface
/// (`DRIVER_DISCOVERY_REFUSAL_KINDS`, `driver-discovery.ts:23-27`).
pub const REFUSAL_KINDS: &[&str] = &[
    "driver_operations_cursor_reset",
    "driver_operations_view_too_large",
    "driver_scope_mismatch",
];

/// The method and feature the read needs (`decodeExternalDriverCapabilities`,
/// `external-driver.ts:122-139`).
pub const METHOD: &str = "session/driver/get";
pub const FEATURE: &str = "external_driver_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Internal,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    None,
    Interrupted,
    RecoveryRequired,
}

/// The four PUBLIC binding fields (`DriverInventoryDisclosureBinding`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub driver_id: String,
    pub epoch: u64,
    pub revision: u64,
    pub lease_expires_at_ms: u64,
}

/// `DriverInventoryDisclosure` — display data only, never control authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    pub mode: Mode,
    pub recovery: Recovery,
    pub binding: Option<Binding>,
}

/// One walked operation (`DriverInventoryRow` + the acceptance fields kept).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub operation_id: String,
    pub slug: String,
    pub lifecycle: String,
    pub adopted_session_id: String,
    pub model: String,
    pub model_lane: String,
    pub accepted_at_ms: u64,
}

/// Why a walk failed (`DriverInventoryState` error reasons).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorReason {
    Stale,
    Unknown,
    Refused(&'static str),
}

/// `DriverInventoryState` (`driver-discovery.ts:82-102`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Inventory {
    /// The server does not advertise the read (or no session).
    #[default]
    Unavailable,
    Loading,
    Complete {
        snapshot: String,
        observed_revision: String,
        operations: Vec<Operation>,
        disclosure: Disclosure,
    },
    Error(ErrorReason),
}

impl Inventory {
    pub fn disclosure(&self) -> Option<&Disclosure> {
        match self {
            Inventory::Complete { disclosure, .. } => Some(disclosure),
            _ => None,
        }
    }
}

fn u64_of(v: Option<&Value>) -> Option<u64> {
    v?.as_u64()
}

fn non_empty(v: Option<&Value>) -> Option<String> {
    v?.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

fn only_keys(v: &serde_json::Map<String, Value>, allowed: &[&str]) -> bool {
    v.keys().all(|k| allowed.contains(&k.as_str()))
}

/// `parseSessionDriverGetResult` (`external-driver.ts:146-197`): mode +
/// recovery + the binding's four public fields; an external mode with no
/// binding, or an internal binding with a live lease, is malformed.
pub fn parse_view(v: &Value) -> Option<Disclosure> {
    let o = v.as_object()?;
    let mode = match o.get("mode")?.as_str()? {
        "internal" => Mode::Internal,
        "external" => Mode::External,
        _ => return None,
    };
    let recovery = match o.get("recovery")?.as_str()? {
        "none" => Recovery::None,
        "interrupted" => Recovery::Interrupted,
        "recovery_required" => Recovery::RecoveryRequired,
        _ => return None,
    };
    let binding = match o.get("binding") {
        None | Some(Value::Null) => {
            if mode == Mode::External {
                return None;
            }
            None
        }
        Some(b) => {
            let b = b.as_object()?;
            let binding = Binding {
                driver_id: non_empty(b.get("driver_id"))?,
                epoch: u64_of(b.get("epoch"))?,
                revision: u64_of(b.get("revision"))?,
                lease_expires_at_ms: u64_of(b.get("lease_expires_at_ms"))?,
            };
            if mode == Mode::Internal && binding.lease_expires_at_ms != 0 {
                return None;
            }
            Some(binding)
        }
    };
    Some(Disclosure { mode, recovery, binding })
}

/// `peer_slug_is_safe` (`external-driver.ts` / native `peers/mod.rs:354`).
pub fn slug_is_safe(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s != "."
        && s != ".."
        && !s.ends_with('.')
        && !s.ends_with(' ')
        && !s.chars().any(|c| matches!(c, '/' | '\\' | ':') || (c as u32) < 0x20 || c as u32 == 0x7f)
}

const LIFECYCLES: &[&str] = &["accepted", "admitted", "started", "terminal", "recovery_required"];

fn parse_row(v: &Value) -> Option<Operation> {
    let o = v.as_object()?;
    if !only_keys(o, &["operation_id", "kind", "acceptance", "lifecycle", "created_at_ms", "started_at_ms", "terminal_at_ms"]) {
        return None;
    }
    let operation_id = non_empty(o.get("operation_id"))?;
    if o.get("kind")?.as_str()? != "peer_dispatch" {
        return None;
    }
    let lifecycle = o.get("lifecycle")?.as_str()?;
    if !LIFECYCLES.contains(&lifecycle) {
        return None;
    }
    u64_of(o.get("created_at_ms"))?;
    for k in ["started_at_ms", "terminal_at_ms"] {
        if o.get(k).is_some() {
            u64_of(o.get(k))?;
        }
    }
    let a = o.get("acceptance")?.as_object()?;
    if !only_keys(
        a,
        &["model", "model_lane", "workspace_root", "scoped_goal", "adopted_turn_id", "adopted_session_id", "slug", "accepted_at_ms", "payload_digest"],
    ) {
        return None;
    }
    let slug = a.get("slug")?.as_str()?.to_owned();
    if !slug_is_safe(&slug) {
        return None;
    }
    non_empty(a.get("workspace_root"))?;
    non_empty(a.get("payload_digest"))?;
    a.get("adopted_turn_id")?.as_str()?;
    Some(Operation {
        operation_id,
        slug,
        lifecycle: lifecycle.to_owned(),
        adopted_session_id: non_empty(a.get("adopted_session_id"))?,
        model: non_empty(a.get("model"))?,
        model_lane: non_empty(a.get("model_lane"))?,
        accepted_at_ms: u64_of(a.get("accepted_at_ms"))?,
    })
}

/// Canonical decimal u64 (`parseObservedRevision`).
fn revision_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0')) && s.parse::<u64>().is_ok()
}

/// One decoded operations page (`DriverOperationsPageView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub items: Vec<Operation>,
    pub snapshot: String,
    pub observed_revision: String,
    pub complete: bool,
    pub next_cursor: Option<String>,
}

/// `parseDriverOperationsPageResult` (`external-driver-operations.ts:327-394`):
/// unknown fields, a missing `next_cursor`, a non-canonical revision, an
/// over-size page or a bad row are all malformed (`None`).
pub fn parse_page(v: &Value) -> Option<Page> {
    let o = v.as_object()?;
    if !only_keys(o, &["items", "snapshot", "observed_revision", "complete", "next_cursor"]) {
        return None;
    }
    let items = o.get("items")?.as_array()?;
    if items.len() > MAX_PAGE_ROWS {
        return None;
    }
    let snapshot = non_empty(o.get("snapshot"))?;
    let observed_revision = o.get("observed_revision")?.as_str()?.to_owned();
    if !revision_ok(&observed_revision) {
        return None;
    }
    let complete = o.get("complete")?.as_bool()?;
    let next_cursor = match o.get("next_cursor")? {
        Value::Null => {
            if !complete {
                return None;
            }
            None
        }
        Value::String(s) => {
            if s.is_empty() || s.len() > MAX_CURSOR_BYTES || complete {
                return None;
            }
            Some(s.clone())
        }
        _ => return None,
    };
    if v.to_string().len() > MAX_PAGE_BYTES {
        return None;
    }
    let items = items.iter().map(parse_row).collect::<Option<Vec<_>>>()?;
    Some(Page { items, snapshot, observed_revision, complete, next_cursor })
}

/// The typed refusal of a rejected read: ONLY an allowlisted `data.kind`
/// survives (`typedRefusal`, `driver-discovery.ts:348-356`); the message and
/// any other payload are never kept.
pub fn typed_refusal(err: &octoscode_client::ClientError) -> Option<&'static str> {
    let octoscode_client::ClientError::Rpc { error, .. } = err else { return None };
    let kind = error.data.as_ref()?.get("kind")?.as_str()?;
    REFUSAL_KINDS.iter().copied().find(|k| *k == kind)
}

/// The fingerprint every page of one chain must share (`disclosureOf`).
fn fingerprint(v: &Value) -> String {
    json!([v.get("mode"), v.get("recovery"), v.get("binding")]).to_string()
}

/// The walk's state between pages (`walkDriverInventoryChain`'s locals).
#[derive(Debug, Default)]
pub struct Walk {
    pages: usize,
    snapshot: Option<String>,
    observed_revision: String,
    fingerprint: Option<String>,
    disclosure: Option<Disclosure>,
    operations: Vec<Operation>,
    cursors: Vec<String>,
}

/// What the walk does after one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Ask for the next page with this cursor.
    Next(String),
    Done(Inventory),
}

impl Walk {
    /// The params of the next `session/driver/get` (`{session_id,
    /// operations: {limit[, cursor]}}`).
    pub fn params(session: &str, cursor: Option<&str>) -> Value {
        match cursor {
            None => json!({"session_id": session, "operations": {"limit": PAGE_SIZE}}),
            Some(c) => json!({"session_id": session, "operations": {"cursor": c, "limit": PAGE_SIZE}}),
        }
    }

    fn fail() -> Step {
        Step::Done(Inventory::Error(ErrorReason::Unknown))
    }

    /// Fold one reply (the loop body of `walkDriverInventoryChain`).
    pub fn accept(&mut self, reply: &Value) -> Step {
        if self.pages >= MAX_PAGES {
            return Self::fail();
        }
        let Some(disclosure) = parse_view(reply) else { return Self::fail() };
        // Requested + absent = unsupported, never an empty inventory.
        let Some(page) = reply.get("operations").and_then(parse_page) else { return Self::fail() };
        self.pages += 1;
        match &self.snapshot {
            None => {
                self.snapshot = Some(page.snapshot.clone());
                self.observed_revision = page.observed_revision.clone();
                self.fingerprint = Some(fingerprint(reply));
                self.disclosure = Some(disclosure);
            }
            Some(s) => {
                if *s != page.snapshot || self.fingerprint.as_deref() != Some(fingerprint(reply).as_str()) {
                    return Self::fail();
                }
            }
        }
        for op in page.items {
            if let Some(last) = self.operations.last() {
                // Strictly increasing in UTF-8 BYTE order (Rust's `str`
                // ordering IS byte order); a duplicate fails too.
                if op.operation_id.as_bytes() <= last.operation_id.as_bytes() {
                    return Self::fail();
                }
            }
            self.operations.push(op);
            if self.operations.len() > MAX_ROWS {
                return Self::fail();
            }
        }
        match page.next_cursor {
            None => {
                if !page.complete {
                    return Self::fail();
                }
                Step::Done(Inventory::Complete {
                    snapshot: self.snapshot.clone().unwrap_or_default(),
                    observed_revision: self.observed_revision.clone(),
                    operations: std::mem::take(&mut self.operations),
                    disclosure: self.disclosure.clone().expect("first page sets it"),
                })
            }
            Some(next) => {
                if self.cursors.contains(&next) {
                    return Self::fail(); // cursor loop
                }
                self.cursors.push(next.clone());
                Step::Next(next)
            }
        }
    }
}

/// Whether the server advertises the read (method AND feature).
pub fn advertised(methods: &[String], features: &[String]) -> bool {
    methods.iter().any(|m| m == METHOD) && features.iter().any(|f| f == FEATURE)
}

/// Walk ONE complete chain through the production client (bounded; no retry,
/// no restart on a cursor reset).
pub async fn walk(conv: &crate::flow::Conversation) -> Inventory {
    let methods = conv.store.domains.config.supported_methods();
    let features = conv.store.capabilities();
    if !advertised(&methods, &features) {
        return Inventory::Unavailable;
    }
    let session = conv.session_id();
    let deadline = Instant::now() + TIMEOUT;
    let mut w = Walk::default();
    let mut cursor: Option<String> = None;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Inventory::Error(ErrorReason::Unknown);
        }
        let call = conv.client().request(METHOD, Walk::params(&session, cursor.as_deref()));
        let reply = match tokio::time::timeout(left, call).await {
            Err(_) => return Inventory::Error(ErrorReason::Unknown),
            Ok(Err(e)) => {
                return Inventory::Error(match typed_refusal(&e) {
                    Some(kind) => ErrorReason::Refused(kind),
                    None => ErrorReason::Unknown,
                })
            }
            Ok(Ok(v)) => v,
        };
        // The session changed while the chain was in flight: stale.
        if conv.session_id() != session {
            return Inventory::Error(ErrorReason::Stale);
        }
        match w.accept(&reply) {
            Step::Next(c) => cursor = Some(c),
            Step::Done(state) => return state,
        }
    }
}

// --------------------------------------------------------------- copy

/// `DriverControllerDisclosure`'s summary (`SessionControlBar.tsx:791-794`).
pub fn mode_label(d: &Disclosure) -> &'static str {
    match d.mode {
        Mode::External => "External controller",
        Mode::Internal => "Internal controller",
    }
}

/// The recovery row (`:795-800`).
pub fn recovery_label(d: &Disclosure) -> &'static str {
    match d.recovery {
        Recovery::Interrupted => "Interrupted",
        Recovery::RecoveryRequired => "Recovery required",
        Recovery::None => "No recovery pending",
    }
}

/// `leaseCopy` (`:770-777`): ISO-8601 UTC, or "No active lease".
pub fn lease_label(ms: u64) -> String {
    if ms == 0 {
        return "No active lease".into();
    }
    let secs = (ms / 1000) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // civil-from-days (Howard Hinnant), proleptic Gregorian.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "Lease expires {y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60,
        ms % 1000
    )
}

/// "Who controls this session" (`App.tsx:3309-3315`): a complete walk names
/// the controller; anything else is "Nobody".
pub fn controller_label(inv: &Inventory) -> &'static str {
    match inv.disclosure() {
        Some(d) if d.mode == Mode::External => "Another controller",
        Some(_) => "This app",
        None => "Nobody",
    }
}

/// The failed walk in words — task words only, never the server's text
/// (the web's refusal labels, `peer-control-commands.ts:155-166`).
pub fn error_label(reason: &ErrorReason) -> &'static str {
    match reason {
        ErrorReason::Refused("driver_scope_mismatch") => "This session can't be controlled from here",
        ErrorReason::Refused("driver_operations_cursor_reset") => "The controller list changed while it was read. Open the pane again.",
        ErrorReason::Refused("driver_operations_view_too_large") => "The controller list is too large to show.",
        ErrorReason::Refused(_) => "That action was refused.",
        ErrorReason::Stale => "The session changed while the controller was read.",
        ErrorReason::Unknown => "The controller could not be read from this server.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(id: &str, slug: &str) -> Value {
        json!({
            "operation_id": id, "kind": "peer_dispatch", "lifecycle": "started", "created_at_ms": 1,
            "acceptance": {
                "model": "deepseek-v4-flash", "model_lane": "strong", "workspace_root": "/home/user/octos",
                "adopted_turn_id": "01920000-0000-7000-8000-000000000001",
                "adopted_session_id": format!("p:main#peer-{slug}"), "slug": slug,
                "accepted_at_ms": 5, "payload_digest": "sha256:x"
            }
        })
    }

    fn reply(items: Vec<Value>, next: Option<&str>, snapshot: &str) -> Value {
        json!({
            "mode": "external", "recovery": "none",
            "binding": {"driver_id": "drv-tui", "epoch": 3, "revision": 7, "lease_expires_at_ms": 1790000000000u64},
            "operations": {"items": items, "snapshot": snapshot, "observed_revision": "7",
                           "complete": next.is_none(), "next_cursor": next}
        })
    }

    #[test]
    fn walks_a_valid_multi_page_chain_to_complete_exactly_once_per_row() {
        // driver-discovery.test.ts "walks a valid multi-page chain to complete exactly once per row"
        let mut w = Walk::default();
        assert_eq!(w.accept(&reply(vec![op("op-1", "a"), op("op-2", "b")], Some("c1"), "s1")), Step::Next("c1".into()));
        match w.accept(&reply(vec![op("op-3", "c")], None, "s1")) {
            Step::Done(Inventory::Complete { operations, disclosure, .. }) => {
                assert_eq!(operations.iter().map(|o| o.operation_id.as_str()).collect::<Vec<_>>(), ["op-1", "op-2", "op-3"]);
                assert_eq!(disclosure.mode, Mode::External);
                assert_eq!(disclosure.binding.as_ref().unwrap().driver_id, "drv-tui");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rejects_cursor_loops_snapshot_drift_duplicates_and_disorder() {
        let mut w = Walk::default();
        assert!(matches!(w.accept(&reply(vec![op("op-1", "a")], Some("c1"), "s1")), Step::Next(_)));
        assert!(matches!(w.accept(&reply(vec![op("op-2", "b")], Some("c1"), "s1")), Step::Done(Inventory::Error(_))), "cursor loop");
        let mut w = Walk::default();
        w.accept(&reply(vec![op("op-1", "a")], Some("c1"), "s1"));
        assert!(matches!(w.accept(&reply(vec![op("op-2", "b")], None, "s2")), Step::Done(Inventory::Error(_))), "snapshot drift");
        let mut w = Walk::default();
        assert!(matches!(w.accept(&reply(vec![op("op-2", "a"), op("op-1", "b")], None, "s1")), Step::Done(Inventory::Error(_))), "disorder");
        let mut w = Walk::default();
        assert!(matches!(w.accept(&reply(vec![op("op-1", "a"), op("op-1", "b")], None, "s1")), Step::Done(Inventory::Error(_))), "duplicate");
    }

    #[test]
    fn requested_operations_absent_is_an_error_never_an_empty_inventory() {
        let mut w = Walk::default();
        let r = json!({"mode": "internal", "recovery": "none", "binding": null});
        assert_eq!(w.accept(&r), Step::Done(Inventory::Error(ErrorReason::Unknown)));
    }

    #[test]
    fn strict_decoders_refuse_unknown_fields_and_bad_shapes() {
        let mut bad = op("op-1", "a");
        bad["acceptance"]["control_token"] = json!("secret");
        assert!(parse_row(&bad).is_none(), "an unknown acceptance field (a token) never decodes");
        assert!(parse_row(&op("op-1", "../x")).is_none(), "unsafe slug");
        assert!(parse_view(&json!({"mode": "external", "recovery": "none"})).is_none(), "external needs a binding");
        assert!(
            parse_view(&json!({"mode": "internal", "recovery": "none", "binding": {"driver_id": "d", "epoch": 1, "revision": 1, "lease_expires_at_ms": 5}})).is_none(),
            "an internal binding must be inactive"
        );
        assert!(!revision_ok("07") && revision_ok("0") && revision_ok("18446744073709551615") && !revision_ok("18446744073709551616"));
    }

    #[test]
    fn only_allowlisted_refusal_kinds_survive() {
        let mk = |kind: &str| octoscode_client::ClientError::Rpc {
            method: METHOD.into(),
            error: octos_core::ui_protocol::RpcError { code: -32000, message: "raw server text".into(), data: Some(json!({"kind": kind})) },
        };
        assert_eq!(typed_refusal(&mk("driver_scope_mismatch")), Some("driver_scope_mismatch"));
        assert_eq!(typed_refusal(&mk("driver_fence_stale")), None, "a control refusal is not a discovery refusal");
        assert_eq!(typed_refusal(&mk("whatever")), None);
        assert_eq!(error_label(&ErrorReason::Refused("driver_scope_mismatch")), "This session can't be controlled from here");
    }

    #[test]
    fn the_lease_and_controller_copy_follow_the_web() {
        assert_eq!(lease_label(0), "No active lease");
        assert_eq!(lease_label(1_790_000_000_123), "Lease expires 2026-09-21T14:13:20.123Z");
        assert_eq!(controller_label(&Inventory::Unavailable), "Nobody");
    }
}
