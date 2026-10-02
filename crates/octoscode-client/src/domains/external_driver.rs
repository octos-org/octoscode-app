//! A10 — the typed external-driver leaf: `session/driver/{get,acquire,renew,
//! release}`, `peer/dispatch`, `peer/control`.
//!
//! A port of the web client's fail-closed codec
//! (`packages/client/src/external-driver.ts`, `external-driver-peer-control.ts`,
//! `external-driver-operations.ts`, `external-driver-meta.ts`):
//!
//! * every control call is gated on the EXACT wire method being advertised
//!   AND the negotiated `external_driver_v1` feature — a missing half fails
//!   closed BEFORE any frame ([`DriverError::Capability`]);
//! * caller arguments are validated offline (a blank id, an empty steer text,
//!   a non-UUID expected turn never reach the wire);
//! * the wire is the web's exact snake_case encoding
//!   (`external-driver.ts:749-755` acquire, `external-driver-peer-control.ts:
//!   632-644` dispatch, `:694-703` control);
//! * exactly ONE RPC per call — no acquire/renew/retry/fallback here;
//! * a rejected RPC surfaces ONLY an allowlisted typed refusal kind read off the
//!   JSON-RPC error's `data.kind` (`external-driver-meta.ts:27-38`), never the
//!   server's raw message;
//! * receipts are decoded strictly and fenced against the request (operation
//!   id, requested lane, expected turn, the captured master's wire base +
//!   `#peer-<slug>` identity).
//!
//! The control token is held only inside [`ControlFence`] / [`AcquireView`],
//! whose `Debug` impls redact it.
use std::fmt;

use serde_json::{json, Map, Value};

use crate::protocol_id::is_protocol_uuid;
use crate::{Client, ClientError};

pub const EXTERNAL_DRIVER_V1_FEATURE: &str = "external_driver_v1";
pub const SESSION_DRIVER_GET: &str = "session/driver/get";
pub const SESSION_DRIVER_ACQUIRE: &str = "session/driver/acquire";
pub const SESSION_DRIVER_RENEW: &str = "session/driver/renew";
pub const SESSION_DRIVER_RELEASE: &str = "session/driver/release";
pub const PEER_DISPATCH: &str = "peer/dispatch";
pub const PEER_CONTROL: &str = "peer/control";
pub const PROFILE_SUB_PROVIDERS_LIST: &str = "profile/sub_providers/list";

/// The allowlisted typed refusal kinds (`external-driver-meta.ts:27-38`): the
/// ONLY server facts a rejected driver RPC may surface.
pub const REFUSAL_KINDS: &[&str] = &[
    "driver_operations_cursor_reset",
    "driver_operations_view_too_large",
    "driver_scope_mismatch",
    "driver_fence_stale",
    "driver_revision_conflict",
    "driver_busy_handover",
    "driver_operation_conflict",
    "interaction_recovery_required",
    "driver_model_unavailable",
    "peer_control_refused",
];

/// The bounded controller lease (`use-octos-session.ts:476`,
/// `PEER_CONTROL_LEASE_SECONDS`).
pub const LEASE_SECONDS: u64 = 120;

/// The inventory walk's bounds (`driver-discovery.ts:16-21`).
pub const WALK_PAGE_SIZE: u64 = 50;
pub const WALK_MAX_PAGES: usize = 32;
pub const WALK_MAX_ROWS: usize = 3200;

// ------------------------------------------------------------------ errors

/// Why a driver call did not produce its typed result. Every variant carries
/// only CONSTANT copy — never the server's raw message or payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverError {
    /// The method or `external_driver_v1` is not advertised: no frame was sent.
    Capability(&'static str),
    /// A caller argument was invalid: no frame was sent.
    InvalidArgs(&'static str),
    /// The server refused with an allowlisted typed kind.
    Refused { method: &'static str, kind: String },
    /// The RPC was rejected without a typed kind (scrubbed).
    Rejected(&'static str),
    /// The reply did not decode, or failed a request-derived fence.
    Malformed(&'static str),
    /// The transport is gone.
    Transport(&'static str),
}

impl DriverError {
    /// The typed refusal kind, when the server sent one.
    pub fn refusal_kind(&self) -> Option<&str> {
        match self {
            Self::Refused { kind, .. } => Some(kind),
            _ => None,
        }
    }
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(m) => write!(f, "{m} failed: capability not advertised (method + external_driver_v1)"),
            Self::InvalidArgs(m) => write!(f, "{m} failed: invalid arguments"),
            Self::Refused { method, kind } => write!(f, "{method} failed: server refused: {kind}"),
            Self::Rejected(m) => write!(f, "{m} failed: rpc rejected"),
            Self::Malformed(m) => write!(f, "{m} failed: result malformed"),
            Self::Transport(m) => write!(f, "{m} failed: transport closed"),
        }
    }
}

impl std::error::Error for DriverError {}

/// The allowlisted refusal kind on a client error, if any
/// (`external-driver.ts:73-87`: only a REAL protocol error's `data.kind`).
pub fn typed_refusal(error: &ClientError) -> Option<String> {
    let ClientError::Rpc { error, .. } = error else { return None };
    let kind = error.data.as_ref()?.get("kind")?.as_str()?;
    REFUSAL_KINDS.contains(&kind).then(|| kind.to_owned())
}

fn scrub(method: &'static str, e: ClientError) -> DriverError {
    if let Some(kind) = typed_refusal(&e) {
        return DriverError::Refused { method, kind };
    }
    match e {
        ClientError::Transport { .. } => DriverError::Transport(method),
        _ => DriverError::Rejected(method),
    }
}

// ------------------------------------------------------------- capability

/// The capability gate (`requireExternalDriverControlCapability`,
/// `external-driver.ts:369-376`): the EXACT wire method AND the feature.
pub fn admitted(methods: &[String], features: &[String], method: &str) -> bool {
    features.iter().any(|f| f == EXTERNAL_DRIVER_V1_FEATURE) && methods.iter().any(|m| m == method)
}

/// The read gate (`decodeExternalDriverCapabilities`, `external-driver.ts:
/// 128-146`): `session/driver/get` + the feature.
pub fn driver_get_available(methods: &[String], features: &[String]) -> bool {
    admitted(methods, features, SESSION_DRIVER_GET)
}

/// The captured scope every driver call carries (the confirmed master session
/// + profile, captured before any await).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverScope {
    pub session_id: String,
    pub profile_id: String,
    /// The negotiated method list (`supported_methods`).
    pub methods: Vec<String>,
    /// The negotiated feature list (`supported_features`).
    pub features: Vec<String>,
}

impl DriverScope {
    fn gate(&self, method: &'static str) -> Result<(), DriverError> {
        if admitted(&self.methods, &self.features, method) {
            Ok(())
        } else {
            Err(DriverError::Capability(method))
        }
    }
}

// ---------------------------------------------------------------- identity

/// Port of `peerSlugIsSafe` (`external-driver.ts:1043-1058`): <= 64 UTF-8
/// bytes, not `.`/`..`, no trailing dot/space, no `/ \ : #`, no control bytes.
pub fn peer_slug_is_safe(slug: &str) -> bool {
    if slug.is_empty() || slug == "." || slug == ".." {
        return false;
    }
    if slug.ends_with('.') || slug.ends_with(' ') {
        return false;
    }
    if slug.chars().any(|c| matches!(c, '/' | '\\' | ':' | '#') || (c as u32) < 0x20 || c as u32 == 0x7f) {
        return false;
    }
    slug.len() <= 64
}

fn has_control(s: &str) -> bool {
    s.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f)
}

fn is_ordinary_topic(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(|c| c.is_whitespace() || c == '#') && !has_control(s)
}

fn is_valid_profile_id(s: &str) -> bool {
    !s.trim().is_empty() && !s.chars().any(|c| c == ':' || c == '#' || c.is_whitespace())
}

/// Core `is_reserved_channel_name` (`external-driver.ts:385-409`).
const RESERVED_CHANNEL_NAMES: &[&str] = &[
    "acp", "api", "cli", "dingtalk", "discord", "email", "feishu", "line", "local", "matrix",
    "qq-bot", "slack", "system", "telegram", "test", "twilio", "wechat", "wecom", "wecom-bot",
    "whatsapp",
];

/// `capturedProfileSegment` (`external-driver.ts:418-433`).
fn captured_profile_segment(base: &str) -> Option<&str> {
    let first_colon = base.find(':')?;
    let first = &base[..first_colon];
    let rest = &base[first_colon + 1..];
    let second_colon = rest.find(':')?;
    let second = &rest[..second_colon];
    (!RESERVED_CHANNEL_NAMES.contains(&first) && RESERVED_CHANNEL_NAMES.contains(&second)).then_some(first)
}

/// `masterWireBase` (`external-driver.ts:1074-1105`): strip only the first
/// `#topic`; a qualified base must byte-match the captured profile.
pub fn master_wire_base(master_session_id: &str, profile_id: &str) -> Option<String> {
    if !is_valid_profile_id(profile_id) || master_session_id.trim().is_empty() {
        return None;
    }
    if master_session_id.contains('\u{0}') {
        return None;
    }
    let (base, topic) = match master_session_id.find('#') {
        Some(i) => (&master_session_id[..i], Some(&master_session_id[i + 1..])),
        None => (master_session_id, None),
    };
    if let Some(t) = topic {
        if !is_ordinary_topic(t) {
            return None;
        }
    }
    if base.is_empty() || base.chars().any(|c| c.is_whitespace() || c == '#') || has_control(base) {
        return None;
    }
    if let Some(seg) = captured_profile_segment(base) {
        if !is_valid_profile_id(seg) || seg != profile_id {
            return None;
        }
    }
    Some(base.to_owned())
}

/// `workerSessionMatchesCapturedMaster` (`external-driver.ts:996-1012`): the
/// adopted session is EXACTLY the master wire base + `#peer-<slug>`.
pub fn worker_session_matches_master(master: &str, profile: &str, adopted: &str, slug: &str) -> bool {
    if !peer_slug_is_safe(slug) {
        return false;
    }
    match master_wire_base(master, profile) {
        Some(base) => adopted == format!("{base}#peer-{slug}"),
        None => false,
    }
}

/// `adoptedIdentityIsNativeShared` (`external-driver.ts:1014-1027`).
pub fn adopted_identity_is_native(turn_id: &str, session_id: &str, slug: &str) -> bool {
    if !is_protocol_uuid(&Value::String(turn_id.to_owned())) || !peer_slug_is_safe(slug) {
        return false;
    }
    let Some(hash) = session_id.find('#') else { return false };
    let (base, topic) = (&session_id[..hash], &session_id[hash + 1..]);
    if topic.contains('#') || topic != format!("peer-{slug}") {
        return false;
    }
    !base.is_empty() && !has_control(base) && !base.chars().any(char::is_whitespace)
}

/// A fresh protocol UUID (v4 shape) for an operation id. No uuid crate: two
/// OS-seeded `RandomState` hashes of a process counter give 128 random bits.
pub fn new_operation_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let word = |salt: u64| {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(n);
        h.write_u64(salt);
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        h.finish()
    };
    let (a, b) = (word(0x9e37_79b9), word(0x7f4a_7c15));
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_be_bytes());
    bytes[8..].copy_from_slice(&b.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // RFC 4122 variant
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

// ---------------------------------------------------------- the get view

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverMode {
    Internal,
    External,
}

impl DriverMode {
    fn parse(v: &Value) -> Option<Self> {
        match v.as_str()? {
            "internal" => Some(Self::Internal),
            "external" => Some(Self::External),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "internal",
            Self::External => "external",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverRecovery {
    None,
    Interrupted,
    RecoveryRequired,
}

impl DriverRecovery {
    fn parse(v: &Value) -> Option<Self> {
        match v.as_str()? {
            "none" => Some(Self::None),
            "interrupted" => Some(Self::Interrupted),
            "recovery_required" => Some(Self::RecoveryRequired),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Interrupted => "interrupted",
            Self::RecoveryRequired => "recovery_required",
        }
    }
}

/// The public binding disclosure (`DriverBindingView`). Never a proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverBinding {
    pub driver_id: String,
    pub epoch: u64,
    /// The public CAS target. Reading it is never control authority.
    pub revision: u64,
    /// 0 = no live lease.
    pub lease_expires_at_ms: u64,
    pub workspace_root: Option<String>,
    pub accepted_work: Vec<String>,
}

fn non_empty(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

fn decode_binding(v: &Value) -> Option<DriverBinding> {
    let o = v.as_object()?;
    let workspace_root = match o.get("workspace_root") {
        None => None,
        Some(w) => Some(non_empty(w)?),
    };
    let accepted_work = match o.get("accepted_work") {
        None => Vec::new(),
        Some(a) => a.as_array()?.iter().map(non_empty).collect::<Option<Vec<_>>>()?,
    };
    Some(DriverBinding {
        driver_id: non_empty(o.get("driver_id")?)?,
        epoch: o.get("epoch")?.as_u64()?,
        revision: o.get("revision")?.as_u64()?,
        lease_expires_at_ms: o.get("lease_expires_at_ms")?.as_u64()?,
        workspace_root,
        accepted_work,
    })
}

/// The dispatch's scoped goal (`DriverScopedGoalView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedGoal {
    pub goal_id: String,
    pub task_id: Option<String>,
    pub revision: Option<u64>,
}

fn decode_goal(v: &Value, strict_keys: bool) -> Result<Option<ScopedGoal>, ()> {
    if v.is_null() {
        return Ok(None);
    }
    let o = v.as_object().ok_or(())?;
    if strict_keys && o.keys().any(|k| !matches!(k.as_str(), "goal_id" | "task_id" | "revision")) {
        return Err(());
    }
    let goal_id = o.get("goal_id").and_then(non_empty).ok_or(())?;
    let task_id = match o.get("task_id") {
        None => None,
        Some(t) => Some(non_empty(t).ok_or(())?),
    };
    let revision = match o.get("revision") {
        None => None,
        Some(r) => Some(r.as_u64().ok_or(())?),
    };
    Ok(Some(ScopedGoal { goal_id, task_id, revision }))
}

/// One accepted dispatch's immutable acceptance facts (`parseAcceptance`,
/// `external-driver-operations.ts:183-248`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acceptance {
    pub model: String,
    pub model_lane: String,
    pub workspace_root: String,
    pub scoped_goal: Option<ScopedGoal>,
    pub adopted_turn_id: String,
    pub adopted_session_id: String,
    pub slug: String,
    pub accepted_at_ms: u64,
    pub payload_digest: String,
}

/// One walked operation row (`OperationRecoveryRow`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRow {
    pub operation_id: String,
    pub acceptance: Acceptance,
    /// accepted | admitted | started | terminal | recovery_required
    pub lifecycle: String,
    pub created_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub terminal_at_ms: Option<u64>,
}

/// One operations page (`DriverOperationsPageView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationsPage {
    pub items: Vec<OperationRow>,
    pub snapshot: String,
    pub observed_revision: String,
    pub complete: bool,
    pub next_cursor: Option<String>,
}

const LIFECYCLES: &[&str] = &["accepted", "admitted", "started", "terminal", "recovery_required"];

fn decode_acceptance(v: &Value, scope: &DriverScope) -> Option<Acceptance> {
    let o = v.as_object()?;
    let allowed = [
        "model", "model_lane", "workspace_root", "scoped_goal", "adopted_turn_id",
        "adopted_session_id", "slug", "accepted_at_ms", "payload_digest",
    ];
    if o.keys().any(|k| !allowed.contains(&k.as_str())) {
        return None;
    }
    let a = Acceptance {
        model: non_empty(o.get("model")?)?,
        model_lane: non_empty(o.get("model_lane")?)?,
        workspace_root: non_empty(o.get("workspace_root")?)?,
        scoped_goal: decode_goal(o.get("scoped_goal").unwrap_or(&Value::Null), true).ok()?,
        adopted_turn_id: o.get("adopted_turn_id")?.as_str()?.to_owned(),
        adopted_session_id: non_empty(o.get("adopted_session_id")?)?,
        slug: o.get("slug")?.as_str()?.to_owned(),
        accepted_at_ms: o.get("accepted_at_ms")?.as_u64()?,
        payload_digest: non_empty(o.get("payload_digest")?)?,
    };
    if !peer_slug_is_safe(&a.slug)
        || !adopted_identity_is_native(&a.adopted_turn_id, &a.adopted_session_id, &a.slug)
        || !worker_session_matches_master(&scope.session_id, &scope.profile_id, &a.adopted_session_id, &a.slug)
    {
        return None;
    }
    Some(a)
}

fn decode_row(v: &Value, scope: &DriverScope) -> Option<OperationRow> {
    let o = v.as_object()?;
    let allowed =
        ["operation_id", "kind", "acceptance", "lifecycle", "created_at_ms", "started_at_ms", "terminal_at_ms"];
    if o.keys().any(|k| !allowed.contains(&k.as_str())) {
        return None;
    }
    if o.get("kind")?.as_str()? != "peer_dispatch" {
        return None;
    }
    let lifecycle = o.get("lifecycle")?.as_str()?.to_owned();
    if !LIFECYCLES.contains(&lifecycle.as_str()) {
        return None;
    }
    let opt_u64 = |k: &str| -> Result<Option<u64>, ()> {
        match o.get(k) {
            None => Ok(None),
            Some(v) => v.as_u64().map(Some).ok_or(()),
        }
    };
    Some(OperationRow {
        operation_id: non_empty(o.get("operation_id")?)?,
        acceptance: decode_acceptance(o.get("acceptance")?, scope)?,
        lifecycle,
        created_at_ms: o.get("created_at_ms")?.as_u64()?,
        started_at_ms: opt_u64("started_at_ms").ok()?,
        terminal_at_ms: opt_u64("terminal_at_ms").ok()?,
    })
}

/// Canonical decimal u64 (`parseObservedRevision`).
fn canonical_revision(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 20
        && s.bytes().all(|b| b.is_ascii_digit())
        && (s == "0" || !s.starts_with('0'))
        && s.parse::<u64>().is_ok()
}

/// `parseDriverOperationsPageResult` (`external-driver-operations.ts:316-388`).
pub fn decode_operations_page(v: &Value, scope: &DriverScope) -> Option<OperationsPage> {
    let o = v.as_object()?;
    if o.keys().any(|k| !matches!(k.as_str(), "items" | "snapshot" | "observed_revision" | "complete" | "next_cursor")) {
        return None;
    }
    let items = o.get("items")?.as_array()?;
    if items.len() > 100 {
        return None;
    }
    let snapshot = non_empty(o.get("snapshot")?)?;
    let observed_revision = o.get("observed_revision")?.as_str()?.to_owned();
    if !canonical_revision(&observed_revision) {
        return None;
    }
    let complete = o.get("complete")?.as_bool()?;
    let next_cursor = match o.get("next_cursor")? {
        Value::Null if complete => None,
        Value::String(c) if !complete && !c.is_empty() && c.len() <= 4096 => Some(c.clone()),
        _ => return None,
    };
    let items = items.iter().map(|r| decode_row(r, scope)).collect::<Option<Vec<_>>>()?;
    Some(OperationsPage { items, snapshot, observed_revision, complete, next_cursor })
}

/// The decoded `session/driver/get` (`SessionDriverGetView` + the optional
/// operations page).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverGetView {
    pub mode: DriverMode,
    pub binding: Option<DriverBinding>,
    pub recovery: DriverRecovery,
    pub operations: Option<OperationsPage>,
}

impl DriverGetView {
    /// `nextExpectedRevision`: 0 only for a never-bound scope.
    pub fn next_expected_revision(&self) -> u64 {
        self.binding.as_ref().map(|b| b.revision).unwrap_or(0)
    }
}

/// `parseSessionDriverGetResult` (`external-driver.ts:156-197`) + the page.
pub fn decode_driver_get(v: &Value, scope: &DriverScope, requested_page: bool) -> Option<DriverGetView> {
    let o = v.as_object()?;
    let mode = DriverMode::parse(o.get("mode")?)?;
    let recovery = DriverRecovery::parse(o.get("recovery")?)?;
    let binding = match o.get("binding") {
        None | Some(Value::Null) => {
            if mode == DriverMode::External {
                return None;
            }
            None
        }
        Some(b) => {
            let b = decode_binding(b)?;
            // A retained binding in internal mode must be INACTIVE.
            if mode == DriverMode::Internal && b.lease_expires_at_ms != 0 {
                return None;
            }
            Some(b)
        }
    };
    let operations = match o.get("operations") {
        // Requested-but-absent is UNSUPPORTED, never an empty inventory.
        None if requested_page => return None,
        None => None,
        Some(p) => Some(decode_operations_page(p, scope)?),
    };
    Some(DriverGetView { mode, binding, recovery, operations })
}

/// `session/driver/get {session_id[, operations]}` — ONE read.
pub async fn driver_get(
    client: &Client,
    scope: &DriverScope,
    page: Option<(Option<String>, u64)>,
) -> Result<DriverGetView, DriverError> {
    if !driver_get_available(&scope.methods, &scope.features) {
        return Err(DriverError::Capability(SESSION_DRIVER_GET));
    }
    let mut params = json!({ "session_id": scope.session_id });
    if let Some((cursor, limit)) = &page {
        if !(1..=100).contains(limit) {
            return Err(DriverError::InvalidArgs(SESSION_DRIVER_GET));
        }
        let mut ops = Map::new();
        if let Some(c) = cursor {
            if c.is_empty() || c.len() > 4096 {
                return Err(DriverError::InvalidArgs(SESSION_DRIVER_GET));
            }
            ops.insert("cursor".into(), json!(c));
        }
        ops.insert("limit".into(), json!(limit));
        params["operations"] = Value::Object(ops);
    }
    let raw = client
        .request(SESSION_DRIVER_GET, params)
        .await
        .map_err(|e| scrub(SESSION_DRIVER_GET, e))?;
    decode_driver_get(&raw, scope, page.is_some()).ok_or(DriverError::Malformed(SESSION_DRIVER_GET))
}

/// A COMPLETE inventory walk (`walkDriverInventoryChain`,
/// `driver-discovery.ts:234-345`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub mode: DriverMode,
    pub recovery: DriverRecovery,
    pub binding: Option<DriverBinding>,
    pub snapshot: String,
    pub observed_revision: String,
    pub operations: Vec<OperationRow>,
}

impl Inventory {
    /// The CAS revision an acquire targets (`peerControlAcquireInput`,
    /// `use-octos-session.ts:572-583`): the binding's revision, or 0.
    pub fn expected_revision(&self) -> u64 {
        self.binding.as_ref().map(|b| b.revision).unwrap_or(0)
    }
}

/// Walk every operations page to the end. Any bound, snapshot change,
/// duplicate/non-increasing id, cursor loop or disclosure change is an
/// explicit error — never truncation.
pub async fn walk_inventory(client: &Client, scope: &DriverScope) -> Result<Inventory, DriverError> {
    const M: &str = SESSION_DRIVER_GET;
    let mut cursor: Option<String> = None;
    let mut first: Option<(DriverMode, DriverRecovery, Option<DriverBinding>, String, String)> = None;
    let mut rows: Vec<OperationRow> = Vec::new();
    let mut seen_cursors: Vec<String> = Vec::new();
    for _ in 0..WALK_MAX_PAGES {
        let view = driver_get(client, scope, Some((cursor.clone(), WALK_PAGE_SIZE))).await?;
        let page = view.operations.clone().ok_or(DriverError::Malformed(M))?;
        match &first {
            None => {
                first = Some((
                    view.mode,
                    view.recovery,
                    view.binding.clone(),
                    page.snapshot.clone(),
                    page.observed_revision.clone(),
                ))
            }
            Some((mode, recovery, binding, snapshot, _)) => {
                if &page.snapshot != snapshot
                    || *mode != view.mode
                    || *recovery != view.recovery
                    || *binding != view.binding
                {
                    return Err(DriverError::Malformed(M));
                }
            }
        }
        for row in page.items {
            if let Some(last) = rows.last() {
                // Strictly increasing in UTF-8 byte order, no duplicates.
                if last.operation_id.as_bytes() >= row.operation_id.as_bytes() {
                    return Err(DriverError::Malformed(M));
                }
            }
            rows.push(row);
            if rows.len() > WALK_MAX_ROWS {
                return Err(DriverError::Malformed(M));
            }
        }
        match page.next_cursor {
            None => {
                let (mode, recovery, binding, snapshot, observed_revision) =
                    first.ok_or(DriverError::Malformed(M))?;
                return Ok(Inventory { mode, recovery, binding, snapshot, observed_revision, operations: rows });
            }
            Some(next) => {
                if seen_cursors.contains(&next) {
                    return Err(DriverError::Malformed(M));
                }
                seen_cursors.push(next.clone());
                cursor = Some(next);
            }
        }
    }
    Err(DriverError::Malformed(M))
}

// ------------------------------------------------------------- the seat

/// The caller-held fence. The token is redacted from `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct ControlFence {
    pub driver_id: String,
    pub epoch: u64,
    control_token: String,
}

impl ControlFence {
    pub fn new(driver_id: impl Into<String>, epoch: u64, control_token: impl Into<String>) -> Self {
        Self { driver_id: driver_id.into(), epoch, control_token: control_token.into() }
    }
    /// The proof, for the ONE frame that carries it.
    pub fn reveal(&self) -> &str {
        &self.control_token
    }
    fn valid(&self) -> bool {
        !self.driver_id.is_empty() && self.epoch > 0 && !self.control_token.is_empty()
    }
}

impl fmt::Debug for ControlFence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControlFence")
            .field("driver_id", &self.driver_id)
            .field("epoch", &self.epoch)
            .field("control_token", &"<redacted>")
            .finish()
    }
}

/// The decoded `session/driver/acquire` (`DriverAcquireView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireView {
    pub fence: ControlFence,
    pub binding: DriverBinding,
    pub pending_work: Vec<String>,
    pub recovery: DriverRecovery,
}

/// The acquire wire (`external-driver.ts:749-755`).
pub fn acquire_params(scope: &DriverScope, driver_id: &str, expected_revision: u64, lease_seconds: u64) -> Value {
    json!({
        "session_id": scope.session_id,
        "driver_id": driver_id,
        "expected_revision": expected_revision,
        "lease_seconds": lease_seconds,
    })
}

/// `parseDriverAcquireResult` (`external-driver.ts:630-664`): the binding must
/// name OUR driver id.
pub fn decode_acquire(v: &Value, expected_driver_id: &str) -> Option<AcquireView> {
    let o = v.as_object()?;
    let token = non_empty(o.get("control_token")?)?;
    let binding = decode_binding(o.get("binding")?)?;
    if binding.driver_id != expected_driver_id {
        return None;
    }
    let recovery = DriverRecovery::parse(o.get("recovery")?)?;
    let pending_work = match o.get("pending_work") {
        None => Vec::new(),
        Some(p) => p.as_array()?.iter().map(non_empty).collect::<Option<Vec<_>>>()?,
    };
    Some(AcquireView {
        fence: ControlFence::new(binding.driver_id.clone(), binding.epoch, token),
        binding,
        pending_work,
        recovery,
    })
}

/// ONE `session/driver/acquire` (CAS on `expected_revision`).
pub async fn acquire(
    client: &Client,
    scope: &DriverScope,
    driver_id: &str,
    expected_revision: u64,
) -> Result<AcquireView, DriverError> {
    scope.gate(SESSION_DRIVER_ACQUIRE)?;
    if driver_id.is_empty() {
        return Err(DriverError::InvalidArgs(SESSION_DRIVER_ACQUIRE));
    }
    let raw = client
        .request(SESSION_DRIVER_ACQUIRE, acquire_params(scope, driver_id, expected_revision, LEASE_SECONDS))
        .await
        .map_err(|e| scrub(SESSION_DRIVER_ACQUIRE, e))?;
    decode_acquire(&raw, driver_id).ok_or(DriverError::Malformed(SESSION_DRIVER_ACQUIRE))
}

/// ONE `session/driver/renew` against the held fence; the refreshed deadline.
pub async fn renew(client: &Client, scope: &DriverScope, fence: &ControlFence) -> Result<u64, DriverError> {
    scope.gate(SESSION_DRIVER_RENEW)?;
    if fence.driver_id.is_empty() || fence.control_token.is_empty() {
        return Err(DriverError::InvalidArgs(SESSION_DRIVER_RENEW));
    }
    let raw = client
        .request(
            SESSION_DRIVER_RENEW,
            json!({
                "session_id": scope.session_id,
                "driver_id": fence.driver_id,
                "epoch": fence.epoch,
                "control_token": fence.reveal(),
                "lease_seconds": LEASE_SECONDS,
            }),
        )
        .await
        .map_err(|e| scrub(SESSION_DRIVER_RENEW, e))?;
    raw.get("lease_expires_at_ms").and_then(Value::as_u64).ok_or(DriverError::Malformed(SESSION_DRIVER_RENEW))
}

/// The release wire (`external-driver.ts:806-817`).
pub fn release_params(scope: &DriverScope, fence: &ControlFence, expected_revision: u64, next: DriverMode) -> Value {
    json!({
        "session_id": scope.session_id,
        "driver_id": fence.driver_id,
        "epoch": fence.epoch,
        "control_token": fence.reveal(),
        "expected_revision": expected_revision,
        "next": next.as_str(),
    })
}

/// ONE `session/driver/release`. `next = external` parks the binding (lease
/// 0); `internal` hands the session back (no binding).
pub async fn release(
    client: &Client,
    scope: &DriverScope,
    fence: &ControlFence,
    expected_revision: u64,
    next: DriverMode,
) -> Result<(DriverMode, Option<DriverBinding>), DriverError> {
    scope.gate(SESSION_DRIVER_RELEASE)?;
    let raw = client
        .request(SESSION_DRIVER_RELEASE, release_params(scope, fence, expected_revision, next))
        .await
        .map_err(|e| scrub(SESSION_DRIVER_RELEASE, e))?;
    let o = raw.as_object().ok_or(DriverError::Malformed(SESSION_DRIVER_RELEASE))?;
    let mode = o.get("mode").and_then(DriverMode::parse).ok_or(DriverError::Malformed(SESSION_DRIVER_RELEASE))?;
    if mode != next {
        return Err(DriverError::Malformed(SESSION_DRIVER_RELEASE));
    }
    match next {
        DriverMode::Internal => match o.get("binding") {
            None | Some(Value::Null) => Ok((mode, None)),
            Some(_) => Err(DriverError::Malformed(SESSION_DRIVER_RELEASE)),
        },
        DriverMode::External => {
            let b = o.get("binding").and_then(decode_binding).ok_or(DriverError::Malformed(SESSION_DRIVER_RELEASE))?;
            if b.lease_expires_at_ms != 0 {
                return Err(DriverError::Malformed(SESSION_DRIVER_RELEASE));
            }
            Ok((mode, Some(b)))
        }
    }
}

// -------------------------------------------------------------- dispatch

/// The tagged dispatch target (`PeerDispatchTargetInput`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchTarget {
    NewBrief { brief: String, title: Option<String>, worktree: Option<bool> },
    ExistingSlug { slug: String },
}

/// One `peer/dispatch` request (camelCase args in, snake_case wire out).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRequest {
    pub operation_id: String,
    /// The REQUESTED lane key (echoed back as `model_lane`).
    pub model: String,
    pub target: DispatchTarget,
    pub kickoff_text: Option<String>,
    pub goal_id: Option<String>,
    pub task_id: Option<String>,
}

/// The exact dispatch wire (`external-driver-peer-control.ts:632-644`);
/// `None` = the arguments are invalid (no frame).
pub fn dispatch_params(scope: &DriverScope, fence: &ControlFence, r: &DispatchRequest) -> Option<Value> {
    if !fence.valid() || r.operation_id.is_empty() || r.model.is_empty() {
        return None;
    }
    let dispatch = match &r.target {
        DispatchTarget::NewBrief { brief, title, worktree } => {
            if brief.is_empty() {
                return None;
            }
            let mut d = json!({ "kind": "new_brief", "brief": brief });
            if let Some(t) = title {
                d["title"] = json!(t);
            }
            if let Some(w) = worktree {
                d["worktree"] = json!(w);
            }
            d
        }
        DispatchTarget::ExistingSlug { slug } => {
            // `existing_slug` REQUIRES an explicit kickoff.
            if !peer_slug_is_safe(slug) || r.kickoff_text.as_deref().is_none_or(str::is_empty) {
                return None;
            }
            json!({ "kind": "existing_slug", "slug": slug })
        }
    };
    let mut wire = json!({
        "session_id": scope.session_id,
        "driver_id": fence.driver_id,
        "epoch": fence.epoch,
        "control_token": fence.reveal(),
        "operation_id": r.operation_id,
        "model": r.model,
        "dispatch": dispatch,
    });
    if let Some(k) = &r.kickoff_text {
        if k.is_empty() {
            return None;
        }
        wire["kickoff_input"] = json!([{ "kind": "text", "text": k }]);
    }
    if let Some(g) = &r.goal_id {
        if g.is_empty() {
            return None;
        }
        wire["goal_id"] = json!(g);
    }
    if let Some(t) = &r.task_id {
        if t.is_empty() {
            return None;
        }
        wire["task_id"] = json!(t);
    }
    Some(wire)
}

/// An accepted dispatch receipt (`PeerDispatchReceiptView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchReceipt {
    pub operation_id: String,
    /// The server-RESOLVED model (may differ from the requested lane).
    pub model: String,
    pub model_lane: String,
    pub workspace_root: String,
    pub scoped_goal: Option<ScopedGoal>,
    pub adopted_turn_id: String,
    pub adopted_session_id: String,
    pub slug: String,
    pub duplicate: bool,
    pub accepted_at_ms: u64,
    pub payload_digest: String,
}

/// `decodePeerDispatchReceipt` (`external-driver-peer-control.ts:310-389`).
pub fn decode_dispatch_receipt(v: &Value, scope: &DriverScope, r: &DispatchRequest) -> Option<DispatchReceipt> {
    let o = v.as_object()?;
    if o.get("state")?.as_str()? != "accepted" {
        return None;
    }
    let receipt = DispatchReceipt {
        operation_id: non_empty(o.get("operation_id")?)?,
        model: non_empty(o.get("model")?)?,
        model_lane: non_empty(o.get("model_lane")?)?,
        workspace_root: non_empty(o.get("workspace_root")?)?,
        scoped_goal: decode_goal(o.get("scoped_goal").unwrap_or(&Value::Null), false).ok()?,
        adopted_turn_id: o.get("adopted_turn_id")?.as_str()?.to_owned(),
        adopted_session_id: non_empty(o.get("adopted_session_id")?)?,
        slug: o.get("slug")?.as_str()?.to_owned(),
        duplicate: o.get("duplicate")?.as_bool()?,
        accepted_at_ms: o.get("accepted_at_ms")?.as_u64()?,
        payload_digest: non_empty(o.get("payload_digest")?)?,
    };
    // Request-derived fences — never inferred from the reply.
    if receipt.operation_id != r.operation_id || receipt.model_lane != r.model {
        return None;
    }
    if let Some(g) = &r.goal_id {
        let ok = receipt.scoped_goal.as_ref().is_some_and(|s| {
            &s.goal_id == g && r.task_id.as_ref().is_none_or(|t| s.task_id.as_ref() == Some(t))
        });
        if !ok {
            return None;
        }
    }
    if let DispatchTarget::ExistingSlug { slug } = &r.target {
        if &receipt.slug != slug {
            return None;
        }
    }
    if !worker_session_matches_master(&scope.session_id, &scope.profile_id, &receipt.adopted_session_id, &receipt.slug)
        || !adopted_identity_is_native(&receipt.adopted_turn_id, &receipt.adopted_session_id, &receipt.slug)
    {
        return None;
    }
    Some(receipt)
}

/// EXACTLY ONE `peer/dispatch`.
pub async fn dispatch(
    client: &Client,
    scope: &DriverScope,
    fence: &ControlFence,
    r: &DispatchRequest,
) -> Result<DispatchReceipt, DriverError> {
    scope.gate(PEER_DISPATCH)?;
    let wire = dispatch_params(scope, fence, r).ok_or(DriverError::InvalidArgs(PEER_DISPATCH))?;
    let raw = client.request(PEER_DISPATCH, wire).await.map_err(|e| scrub(PEER_DISPATCH, e))?;
    decode_dispatch_receipt(&raw, scope, r).ok_or(DriverError::Malformed(PEER_DISPATCH))
}

// --------------------------------------------------------------- control

/// One answer to a peer's question (`PeerUserQuestionAnswer`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestionAnswer {
    pub selected_labels: Option<Vec<String>>,
    pub free_text: Option<String>,
}

/// The tagged control command (`PeerControlCommand`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlCommand {
    ApprovalRespond { approval_id: String, approve: bool, approval_scope: Option<String> },
    QuestionRespond { question_id: String, answers: Vec<QuestionAnswer> },
    Steer { text: String },
    Interrupt,
}

impl ControlCommand {
    /// The wire kind (`approval_respond` | `question_respond` | `steer` |
    /// `interrupt`).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ApprovalRespond { .. } => "approval_respond",
            Self::QuestionRespond { .. } => "question_respond",
            Self::Steer { .. } => "steer",
            Self::Interrupt => "interrupt",
        }
    }

    /// `encodeControlCommand` (`external-driver-peer-control.ts:463-544`);
    /// `None` = an encoder-invalid command (no frame).
    pub fn encode(&self) -> Option<Value> {
        Some(match self {
            Self::ApprovalRespond { approval_id, approve, approval_scope } => {
                if approval_id.is_empty() {
                    return None;
                }
                let mut w = json!({
                    "kind": "approval_respond",
                    "approval_id": approval_id,
                    "decision": if *approve { "approve" } else { "deny" },
                });
                if let Some(s) = approval_scope {
                    w["approval_scope"] = json!(s);
                }
                w
            }
            Self::QuestionRespond { question_id, answers } => {
                if question_id.is_empty() || answers.is_empty() {
                    return None;
                }
                let answers: Vec<Value> = answers
                    .iter()
                    .map(|a| {
                        let mut e = Map::new();
                        if let Some(l) = &a.selected_labels {
                            e.insert("selected_labels".into(), json!(l));
                        }
                        if let Some(t) = &a.free_text {
                            e.insert("free_text".into(), json!(t));
                        }
                        Value::Object(e)
                    })
                    .collect();
                json!({ "kind": "question_respond", "question_id": question_id, "answers": answers })
            }
            Self::Steer { text } => {
                if text.is_empty() {
                    return None;
                }
                json!({ "kind": "steer", "input": [{ "kind": "text", "text": text }] })
            }
            Self::Interrupt => json!({ "kind": "interrupt" }),
        })
    }
}

/// The target identity of ONE control activation (`PeerControlTarget`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlTarget {
    /// A FRESH idempotency key for this control operation.
    pub control_operation_id: String,
    /// The ACCEPTED dispatch being controlled.
    pub target_operation_id: String,
    /// The adopted peer turn (a protocol UUID).
    pub expected_turn_id: String,
}

impl ControlTarget {
    /// `buildPeerRowControlTarget` (`peer-controller-staging.ts:404-416`):
    /// NULL for a missing operation id or a non-UUID turn.
    pub fn for_row(target_operation_id: &str, turn_id: &str) -> Option<Self> {
        if target_operation_id.is_empty() || !is_protocol_uuid(&Value::String(turn_id.to_owned())) {
            return None;
        }
        Some(Self {
            control_operation_id: new_operation_id(),
            target_operation_id: target_operation_id.to_owned(),
            expected_turn_id: turn_id.to_owned(),
        })
    }
}

/// The exact control wire (`external-driver-peer-control.ts:694-703`).
pub fn control_params(scope: &DriverScope, fence: &ControlFence, t: &ControlTarget, c: &ControlCommand) -> Option<Value> {
    if !fence.valid()
        || t.control_operation_id.is_empty()
        || t.target_operation_id.is_empty()
        || !is_protocol_uuid(&Value::String(t.expected_turn_id.clone()))
    {
        return None;
    }
    Some(json!({
        "session_id": scope.session_id,
        "driver_id": fence.driver_id,
        "epoch": fence.epoch,
        "control_token": fence.reveal(),
        "operation_id": t.control_operation_id,
        "target_operation_id": t.target_operation_id,
        "expected_turn_id": t.expected_turn_id,
        "command": c.encode()?,
    }))
}

/// An accepted control receipt (`PeerControlReceiptView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlReceipt {
    pub operation_id: String,
    pub target_operation_id: String,
    pub expected_turn_id: String,
    pub target_session_id: String,
    pub slug: String,
    pub accepted_at_ms: u64,
    pub payload_digest: String,
    pub duplicate: bool,
}

const CONTROL_RESULT_KEYS: &[&str] = &[
    "operation_id", "state", "target_operation_id", "expected_turn_id", "target_session_id", "slug",
    "accepted_at_ms", "payload_digest", "duplicate",
];

/// `decodePeerControlReceipt` (`external-driver-peer-control.ts:398-460`):
/// `Err(refusal)` for the Core's typed REFUSED receipt, `Ok(None)` for a
/// malformed one.
pub fn decode_control_receipt(
    v: &Value,
    scope: &DriverScope,
    t: &ControlTarget,
) -> Result<Option<ControlReceipt>, DriverError> {
    let Some(o) = v.as_object() else { return Ok(None) };
    if o.keys().any(|k| !CONTROL_RESULT_KEYS.contains(&k.as_str())) {
        return Ok(None);
    }
    if o.get("state").and_then(Value::as_str) == Some("refused") {
        return Err(DriverError::Refused { method: PEER_CONTROL, kind: "peer_control_refused".into() });
    }
    let decode = || -> Option<ControlReceipt> {
        if o.get("state")?.as_str()? != "accepted" {
            return None;
        }
        let r = ControlReceipt {
            operation_id: non_empty(o.get("operation_id")?)?,
            target_operation_id: non_empty(o.get("target_operation_id")?)?,
            expected_turn_id: o.get("expected_turn_id")?.as_str()?.to_owned(),
            target_session_id: non_empty(o.get("target_session_id")?)?,
            slug: o.get("slug")?.as_str()?.to_owned(),
            accepted_at_ms: o.get("accepted_at_ms")?.as_u64()?,
            payload_digest: non_empty(o.get("payload_digest")?)?,
            duplicate: o.get("duplicate")?.as_bool()?,
        };
        if r.operation_id != t.control_operation_id
            || r.target_operation_id != t.target_operation_id
            || r.expected_turn_id != t.expected_turn_id
        {
            return None;
        }
        if !worker_session_matches_master(&scope.session_id, &scope.profile_id, &r.target_session_id, &r.slug)
            || !adopted_identity_is_native(&r.expected_turn_id, &r.target_session_id, &r.slug)
        {
            return None;
        }
        Some(r)
    };
    Ok(decode())
}

/// EXACTLY ONE `peer/control` frame — never retried.
pub async fn control(
    client: &Client,
    scope: &DriverScope,
    fence: &ControlFence,
    t: &ControlTarget,
    c: &ControlCommand,
) -> Result<ControlReceipt, DriverError> {
    scope.gate(PEER_CONTROL)?;
    let wire = control_params(scope, fence, t, c).ok_or(DriverError::InvalidArgs(PEER_CONTROL))?;
    let raw = client.request(PEER_CONTROL, wire).await.map_err(|e| scrub(PEER_CONTROL, e))?;
    decode_control_receipt(&raw, scope, t)?.ok_or(DriverError::Malformed(PEER_CONTROL))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> DriverScope {
        DriverScope {
            session_id: "dsflash:main".into(),
            profile_id: "dsflash".into(),
            methods: vec![
                SESSION_DRIVER_GET.into(),
                SESSION_DRIVER_ACQUIRE.into(),
                SESSION_DRIVER_RENEW.into(),
                SESSION_DRIVER_RELEASE.into(),
                PEER_DISPATCH.into(),
                PEER_CONTROL.into(),
            ],
            features: vec![EXTERNAL_DRIVER_V1_FEATURE.into()],
        }
    }

    const TURN: &str = "00000000-0000-4000-8000-0000000000d1";

    #[test]
    fn identities_follow_the_native_shape() {
        assert_eq!(master_wire_base("dsflash:main", "dsflash").as_deref(), Some("dsflash:main"));
        assert_eq!(master_wire_base("dsflash:main#coding", "dsflash").as_deref(), Some("dsflash:main"));
        // A qualified base must byte-match the captured profile.
        assert_eq!(master_wire_base("other:local:tui", "dsflash"), None);
        assert_eq!(master_wire_base("dsflash:local:tui", "dsflash").as_deref(), Some("dsflash:local:tui"));
        assert!(worker_session_matches_master("dsflash:main", "dsflash", "dsflash:main#peer-r6-smoke", "r6-smoke"));
        assert!(!worker_session_matches_master("dsflash:main", "dsflash", "foreign:main#peer-r6-smoke", "r6-smoke"));
        assert!(adopted_identity_is_native(TURN, "dsflash:main#peer-x", "x"));
        assert!(!adopted_identity_is_native("not-a-uuid", "dsflash:main#peer-x", "x"));
        assert!(!peer_slug_is_safe("a/b") && !peer_slug_is_safe("..") && peer_slug_is_safe("peer-1"));
    }

    #[test]
    fn operation_ids_are_distinct_protocol_uuids() {
        let a = new_operation_id();
        let b = new_operation_id();
        assert_ne!(a, b);
        assert!(is_protocol_uuid(&Value::String(a.clone())), "{a}");
        assert_eq!(&a[14..15], "4", "version 4: {a}");
    }

    #[test]
    fn the_gate_needs_the_method_and_the_feature() {
        let mut s = scope();
        assert!(admitted(&s.methods, &s.features, PEER_DISPATCH));
        s.features.clear();
        assert!(!admitted(&s.methods, &s.features, PEER_DISPATCH));
        assert_eq!(s.gate(PEER_DISPATCH), Err(DriverError::Capability(PEER_DISPATCH)));
    }

    #[test]
    fn dispatch_and_control_wires_are_the_web_encoding() {
        let s = scope();
        let fence = ControlFence::new("octoscode-native:1", 7, "tok");
        let r = DispatchRequest {
            operation_id: "op-1".into(),
            model: "lane-primary".into(),
            target: DispatchTarget::NewBrief { brief: "Review".into(), title: Some("review".into()), worktree: None },
            kickoff_text: Some("You are a peer agent.".into()),
            goal_id: None,
            task_id: None,
        };
        let w = dispatch_params(&s, &fence, &r).unwrap();
        assert_eq!(
            w,
            json!({
                "session_id": "dsflash:main", "driver_id": "octoscode-native:1", "epoch": 7,
                "control_token": "tok", "operation_id": "op-1", "model": "lane-primary",
                "dispatch": {"kind": "new_brief", "brief": "Review", "title": "review"},
                "kickoff_input": [{"kind": "text", "text": "You are a peer agent."}],
            })
        );
        // epoch 0 is not a fence (`validateFence`: positive integer).
        assert!(dispatch_params(&s, &ControlFence::new("d", 0, "t"), &r).is_none());
        let t = ControlTarget::for_row("op-1", TURN).unwrap();
        let c = control_params(&s, &fence, &t, &ControlCommand::Steer { text: "focus tests".into() }).unwrap();
        assert_eq!(c["command"], json!({"kind": "steer", "input": [{"kind": "text", "text": "focus tests"}]}));
        assert_eq!(c["expected_turn_id"], json!(TURN));
        assert_eq!(c["target_operation_id"], json!("op-1"));
        assert_ne!(c["operation_id"], json!("op-1"), "a fresh control operation id");
        assert!(ControlCommand::Steer { text: String::new() }.encode().is_none(), "blank steer: no frame");
        assert!(ControlTarget::for_row("op-1", "master-turn").is_none(), "non-UUID turn: no target");
        let deny = ControlCommand::ApprovalRespond { approval_id: "ap-1".into(), approve: false, approval_scope: None };
        assert_eq!(deny.encode().unwrap(), json!({"kind": "approval_respond", "approval_id": "ap-1", "decision": "deny"}));
        assert!(format!("{fence:?}").contains("<redacted>") && !format!("{fence:?}").contains("tok\""));
    }

    #[test]
    fn receipts_are_fenced_against_the_request() {
        let s = scope();
        let r = DispatchRequest {
            operation_id: "op-1".into(),
            model: "lane-primary".into(),
            target: DispatchTarget::NewBrief { brief: "b".into(), title: None, worktree: None },
            kickoff_text: None,
            goal_id: None,
            task_id: None,
        };
        let ok = json!({
            "operation_id": "op-1", "state": "accepted", "model": "gpt-5.4", "model_lane": "lane-primary",
            "workspace_root": "/srv/work/x", "scoped_goal": null, "adopted_turn_id": TURN,
            "adopted_session_id": "dsflash:main#peer-review-1", "slug": "review-1", "duplicate": false,
            "accepted_at_ms": 1770000000000u64, "payload_digest": "d",
        });
        let rec = decode_dispatch_receipt(&ok, &s, &r).expect("accepted");
        assert_eq!(rec.model, "gpt-5.4");
        let mut wrong_lane = ok.clone();
        wrong_lane["model_lane"] = json!("lane-review");
        assert!(decode_dispatch_receipt(&wrong_lane, &s, &r).is_none());
        let mut foreign = ok.clone();
        foreign["adopted_session_id"] = json!("other:main#peer-review-1");
        assert!(decode_dispatch_receipt(&foreign, &s, &r).is_none());
        let t = ControlTarget::for_row("op-1", TURN).unwrap();
        let receipt = json!({
            "operation_id": t.control_operation_id, "state": "accepted", "target_operation_id": "op-1",
            "expected_turn_id": TURN, "target_session_id": "dsflash:main#peer-review-1", "slug": "review-1",
            "accepted_at_ms": 1, "payload_digest": "d", "duplicate": false,
        });
        assert!(decode_control_receipt(&receipt, &s, &t).unwrap().is_some());
        let mut extra = receipt.clone();
        extra["model"] = json!("x");
        assert_eq!(decode_control_receipt(&extra, &s, &t), Ok(None), "deny_unknown_fields");
        let mut refused = receipt.clone();
        refused["state"] = json!("refused");
        assert_eq!(
            decode_control_receipt(&refused, &s, &t).unwrap_err().refusal_kind(),
            Some("peer_control_refused")
        );
    }

    #[test]
    fn driver_get_decodes_the_disclosure_and_the_page() {
        let s = scope();
        let v = json!({
            "mode": "external", "recovery": "none",
            "binding": {"driver_id": "octoscode-native:1", "epoch": 7, "revision": 42, "lease_expires_at_ms": 1770000000000u64},
            "operations": {"items": [{
                "operation_id": "op-a", "kind": "peer_dispatch", "lifecycle": "started", "created_at_ms": 1,
                "acceptance": {"model": "gpt-5.4", "model_lane": "lane-primary", "workspace_root": "/w",
                    "scoped_goal": {"goal_id": "goal_01"}, "adopted_turn_id": TURN,
                    "adopted_session_id": "dsflash:main#peer-a", "slug": "a", "accepted_at_ms": 5, "payload_digest": "d"}
            }], "snapshot": "s1", "observed_revision": "42", "complete": true, "next_cursor": null},
        });
        let view = decode_driver_get(&v, &s, true).expect("decodes");
        assert_eq!(view.next_expected_revision(), 42);
        let page = view.operations.unwrap();
        assert_eq!(page.items[0].acceptance.scoped_goal.as_ref().unwrap().goal_id, "goal_01");
        // External with no binding is malformed; requested-but-absent page too.
        assert!(decode_driver_get(&json!({"mode": "external", "recovery": "none"}), &s, false).is_none());
        assert!(decode_driver_get(&json!({"mode": "internal", "recovery": "none"}), &s, true).is_none());
        assert!(decode_driver_get(&json!({"mode": "internal", "recovery": "none"}), &s, false).is_some());
        // A live lease on an internal binding is not disclosure.
        let bad = json!({"mode": "internal", "recovery": "none",
            "binding": {"driver_id": "d", "epoch": 1, "revision": 1, "lease_expires_at_ms": 9}});
        assert!(decode_driver_get(&bad, &s, false).is_none());
    }

    #[test]
    fn a_refusal_surfaces_only_an_allowlisted_kind() {
        let e = ClientError::Rpc {
            method: PEER_DISPATCH.into(),
            error: octos_core::ui_protocol::RpcError {
                code: -32602,
                message: "driver operation refused: peer/dispatch".into(),
                data: Some(json!({"kind": "driver_model_unavailable"})),
            },
        };
        assert_eq!(typed_refusal(&e).as_deref(), Some("driver_model_unavailable"));
        let odd = ClientError::Rpc {
            method: PEER_DISPATCH.into(),
            error: octos_core::ui_protocol::RpcError {
                code: -32602,
                message: "secret path /x".into(),
                data: Some(json!({"kind": "not_allowlisted"})),
            },
        };
        assert_eq!(typed_refusal(&odd), None);
        assert_eq!(scrub(PEER_DISPATCH, odd).to_string(), "peer/dispatch failed: rpc rejected");
    }
}
