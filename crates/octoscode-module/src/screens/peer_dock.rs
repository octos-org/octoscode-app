//! A30 — the peer dock in the sidebar (parity row 270; web
//! `features/peers/PeerDock.tsx`, `peer-row-view.ts` (`formatElapsed`,
//! `formatPeerTokens`, `formatPeerDockPill`), `features/shell/
//! ProductSidebar.tsx:959-967`, `app/App.tsx:1088-1119` (Alt+P); board 4
//! regions 6 "Peer dock · expanded" and 7 "Peer dock · collapsed", whose
//! README copy and Errata win over the pixels).
//!
//! * **Placement** — `lib.rs` `threads_column`: the session tree
//!   (`oc_sidebar_body`), then this dock (`peer_dock_splash`), then the
//!   footer (Add workspace / Fleet). Hidden while there are no peers (the web
//!   renders nothing) and in the collapsed rail.
//! * **Rows** — the Fleet's own union (`fleetview::rows`), so "Peer N · model"
//!   is the SAME label the Fleet shows for the same peer (never the slug):
//!   glyph, label, elapsed in the web's format ("4m12s", frozen at the
//!   terminal) and the Fleet's status word (Requested … Outcome unknown) with
//!   "↓ tokens" and the acknowledgment on the second line. Glyphs follow
//!   `fleetview::Status::glyph` (○ ● ⚠ ✓ ✕ ?), drawn as the session tree's
//!   own marks: a hollow ring, a green dot, and — Errata — an AMBER DOT for a
//!   waiting row.
//! * **Threaded approval** — only a row blocked on an approval WITH its real
//!   pending id and contents grows the card: "asks to run ‹tool›", the
//!   target, Approve once / Deny / Stop and the link "Approve for session"
//!   (the operator's ONE action set), ⌥Y / ⌥N on the focused row.
//! * **Safety** — every action carries a SLOT: the row and the exact ids it
//!   was drawn for (the pending approval id, the accepted operation, the
//!   adopted turn). A new pending id is a new slot (so the DSL — and the
//!   mounted widget — changes with it). `perform` refuses a slot whose row
//!   no longer shows those ids; the job re-checks them right before its ONE
//!   frame (`fleet_driver::row_control_drawn`), so a stale approval id sends
//!   nothing and Approve on one row can never answer another row's approval.
//! * **Fold** — expanded on a desktop, FOLDED on a phone (operator default);
//!   ⌥P, "Hide peers" and the pill toggle it. Folded = ONE pill, the web's
//!   `formatPeerDockPill` in the Fleet's words ("Peers 3 · 1 working ·
//!   ⚠ 1 waiting · 1/3 finished"), with the "Show peers · ⌥P" hint.
//! * **Height** — on a short desktop column the dock never takes the whole
//!   tree: past `room - TREE_MIN` its rows scroll, and a new waiting card is
//!   scrolled into view once.
//! * **Live values** — elapsed and the token / acknowledgment tail change
//!   every second; they are set IN PLACE after the mount (`Lowered::texts`),
//!   so the mounted tree (and a scroll position, and a press in progress)
//!   survives them; only a structural change remounts.
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, OnceLock};

use makepad_widgets::KeyCode;
use octoscode_store::domains::peer::{Ack, Activity, FleetInventory, RequestDetail, RequestKind};
use octoscode_store::Store;

use super::board3::fleetview::{self, Status};
use super::board3::ui::{self, tok, Dsl, Face, Txt, W};
use super::fleet_driver::{self, DrawnTarget, STALE_DRAWN};
use super::peers::{self, RowAction};
use crate::i18n::{keep, tr, tr1, tr_with};

pub const ACTION_FOLD: &str = "pd.fold";
pub const ACTION_FOCUS: &str = "pd.focus";
pub const ACTION_APPROVE: &str = "pd.approve";
pub const ACTION_APPROVE_SESSION: &str = "pd.approve_session";
pub const ACTION_DENY: &str = "pd.deny";
pub const ACTION_STOP: &str = "pd.stop";

/// The dock owns every `pd.*` id (one owner).
pub fn routes(action: &str) -> bool {
    action.starts_with("pd.")
}

/// The session tree keeps at least this much of the shared height (a
/// workspace header and one row); while a card waits, only the header.
const TREE_MIN: f64 = 68.0;
const TREE_MIN_WAITING: f64 = 36.0;
/// Fixed metrics (logical px) — the session tree's own (`chrome.rs`
/// `SbRowTpl`: rows padded 6 left / 8 right, a 16 px glyph box, 6 px gaps).
const HEAD_H: f64 = 28.0;
const ROW_H: f64 = 40.0;
const RULE_BLOCK: f64 = 9.0;
const PAD_L: f64 = 6.0;
const PAD_R: f64 = 8.0;
const GLYPH: f64 = 16.0;
const GAP: f64 = 6.0;
/// The card's inset: its left edge at the glyph column's right edge (the
/// board's card starts where the row's glyph ends).
const CARD_INSET: f64 = 16.0;
/// The card's padding (left/right, top/bottom).
const CARD_PAD_X: f64 = 10.0;
const CARD_PAD_Y: f64 = 8.0;
const BTN_H: f64 = 30.0;
const BTN_PX: f64 = 12.0;
const BTN_PAD: f64 = 10.0;
/// A refusal note stays this long (a no-op press is said once, not forever).
const NOTE_MS: u64 = 8_000;

/// Where the dock is drawn: the phone drawer (`compact`) or the desktop
/// column, its content width and the height it shares with the session tree
/// (0 = not laid out yet: no cap).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seat {
    pub compact: bool,
    pub width: f64,
    pub room: f64,
}

/// A row's pending APPROVAL with its real id and contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    pub request_id: String,
    pub tool: String,
    pub target: Option<String>,
}

/// One dock row (the Fleet's row, plus the ids its controls carry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockRow {
    /// The Fleet's union key (the adopted session id).
    pub key: String,
    /// The roster identity actions address (`None`: an inventory-only row).
    pub identity: Option<String>,
    /// 1-based, the Fleet's numbering.
    pub number: usize,
    pub model: Option<String>,
    /// "Peer N · model" (English; drawn through `tr`).
    pub label: String,
    pub status: Status,
    pub elapsed_ms: u64,
    pub tokens: u64,
    pub ack: Option<Ack>,
    pub turn_changed: bool,
    /// Only while waiting for an approval with a REAL id and its contents.
    pub approval: Option<PendingApproval>,
    /// The roster row's pending request id (any kind).
    pub request_id: Option<String>,
    pub operation_id: Option<String>,
    pub turn_id: String,
    pub control_supported: bool,
}

/// What one slot's controls were drawn for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawn {
    pub key: String,
    pub identity: Option<String>,
    pub request_id: Option<String>,
    pub operation_id: Option<String>,
    pub turn_id: String,
}

impl Drawn {
    pub fn target(&self) -> DrawnTarget {
        DrawnTarget {
            request_id: self.request_id.clone(),
            operation_id: self.operation_id.clone(),
            turn_id: self.turn_id.clone(),
        }
    }
}

/// One lowering: the DSL the sidebar mounts, its taps, the texts set in
/// place after the mount, and where to scroll the rows (a new waiting card).
#[derive(Debug, Clone, PartialEq)]
pub struct Lowered {
    pub dsl: String,
    pub taps: Vec<(String, String)>,
    pub texts: Vec<(String, String)>,
    pub folded: bool,
    /// The estimated natural height of the dock (logical px).
    pub height: f64,
    /// Scroll the capped rows region to this offset once (a new waiting card).
    pub scroll_to: Option<f64>,
}

/// One routed action's job (the host spawns `run`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub slot: usize,
    pub drawn: Drawn,
    pub action: RowAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Spawn(Job),
    /// Nothing was sent; the bounded copy says why.
    Refused(String),
}

// ------------------------------------------------------------------ state

#[derive(Default)]
struct State {
    /// The operator's fold choice; `None` = the seat's default.
    folded: Option<bool>,
    /// The focused row's key: ⌥Y / ⌥N act on it.
    focused: Option<String>,
    /// slot → what it was drawn for (this lowering's and the previous one's,
    /// so a press on a just-replaced control resolves — and is refused).
    slots: HashMap<usize, Drawn>,
    /// The last lowering's slot per row key, and the one before.
    current: HashMap<String, usize>,
    previous: HashSet<usize>,
    next_slot: usize,
    /// (row key, request id) of a job in flight: a second press sends nothing.
    inflight: HashSet<(String, Option<String>)>,
    /// The last refusal per row: (the request it was for, bounded copy,
    /// when it was said).
    notes: HashMap<String, (Option<String>, String, u64)>,
    /// The waiting card last scrolled into view: (row key, request id).
    revealed: Option<(String, String)>,
}

fn state() -> MutexGuard<'static, State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(State { next_slot: 1, ..Default::default() }))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Forget every fold / focus / slot / note (tests; a new connection).
pub fn reset() {
    *state() = State { next_slot: 1, ..Default::default() };
}

// ------------------------------------------------------------------- rows

/// The dock's rows: the Fleet's union, in the Fleet's order and numbering.
pub fn rows(store: &Store, now_ms: u64) -> Vec<DockRow> {
    let roster = store.domains.peer.rows();
    let ops = match store.domains.peer.inventory() {
        Some(FleetInventory::Complete { operations, .. }) => operations,
        _ => Vec::new(),
    };
    fleetview::rows(store, now_ms)
        .into_iter()
        .enumerate()
        .map(|(i, f)| {
            let r = f.identity.as_deref().and_then(|id| roster.iter().find(|x| x.identity == id));
            // The web freezes a landed row's clock (`peerElapsed`): the
            // Fleet's inventory rows run from acceptance to now, so an ended
            // one is re-frozen at its terminal here.
            let elapsed_ms = match (ops.iter().find(|o| o.adopted_session_id == f.key), r) {
                (Some(op), Some(r)) if r.activity == Activity::Done => {
                    r.finished_at_ms.unwrap_or(now_ms).saturating_sub(op.accepted_at_ms)
                }
                _ => f.elapsed_ms,
            };
            let approval = r.and_then(|r| {
                let id = r.request_id.clone().filter(|id| !id.trim().is_empty())?;
                if f.status != Status::WaitingApproval || r.request_kind != Some(RequestKind::Approval) {
                    return None;
                }
                match &r.request_detail {
                    Some(RequestDetail::Approval(d)) if !d.tool_name.trim().is_empty() => Some(PendingApproval {
                        request_id: id,
                        tool: d.tool_name.clone(),
                        target: d.target.clone().filter(|t| !t.trim().is_empty()),
                    }),
                    _ => None,
                }
            });
            DockRow {
                key: f.key.clone(),
                identity: f.identity.clone(),
                number: i + 1,
                model: f.label.split_once(" · ").map(|(_, m)| m.to_owned()),
                label: f.label.clone(),
                status: f.status,
                elapsed_ms,
                tokens: f.tokens,
                ack: f.ack,
                turn_changed: f.turn_changed,
                approval,
                request_id: r.and_then(|r| r.request_id.clone()),
                operation_id: r.and_then(|r| r.operation_id.clone()),
                turn_id: r.map(|r| r.turn_id.clone()).unwrap_or_default(),
                control_supported: f.control_supported,
            }
        })
        .collect()
}

/// The pill's counts in the Fleet's words: (total, working, waiting,
/// finished) — `summarizeRoster` + `fleetLanded` with the Fleet's status.
fn counts(rows: &[DockRow]) -> (usize, usize, usize, usize) {
    let working = rows.iter().filter(|r| r.status == Status::Working).count();
    let waiting = rows.iter().filter(|r| matches!(r.status, Status::WaitingApproval | Status::WaitingAnswer)).count();
    let finished = rows.iter().filter(|r| r.status.terminal()).count();
    (rows.len(), working, waiting, finished)
}

/// `formatPeerDockPill` (`peer-row-view.ts:168-180`) in the Fleet's words
/// (board 4 README): the total, the working count, the waiting count (only
/// above zero, as the web's blocked), and finished of total. English source.
pub fn pill(rows: &[DockRow]) -> String {
    let (total, working, waiting, finished) = counts(rows);
    let mut parts = vec![format!("{total}"), format!("{working} working")];
    if waiting > 0 {
        parts.push(format!("⚠ {waiting} waiting"));
    }
    parts.push(format!("{finished}/{total} finished"));
    parts.join(" · ")
}

// ------------------------------------------------------------------- fold

/// Whether the dock is folded: the operator's choice, else the seat's
/// default (a phone starts folded — board 4 "Operator decisions").
pub fn folded(compact: bool) -> bool {
    state().folded.unwrap_or(compact)
}

/// ⌥P / "Hide peers" / the pill: flip the fold; returns the new state.
pub fn toggle(compact: bool) -> bool {
    let mut st = state();
    let now = !st.folded.unwrap_or(compact);
    st.folded = Some(now);
    now
}

// ------------------------------------------------------------------- keys

/// ⌥Y approves, ⌥N denies the FOCUSED row (`PeerDock.tsx` `rowDecision`,
/// TUI event_loop.rs:1553-1585): Alt required, Ctrl / Cmd rejected, matched
/// on the physical key (macOS Option+Y is "¥", Option+N a dead key).
pub fn key_action(code: KeyCode, ctrl: bool, alt: bool, logo: bool) -> Option<&'static str> {
    if !alt || ctrl || logo {
        return None;
    }
    match code {
        KeyCode::KeyY => Some(ACTION_APPROVE),
        KeyCode::KeyN => Some(ACTION_DENY),
        _ => None,
    }
}

/// The focused row's slot in the last lowering (`None`: nothing focused, or
/// the row is gone).
pub fn focused_slot() -> Option<usize> {
    let st = state();
    st.focused.as_ref().and_then(|k| st.current.get(k).copied())
}

/// A row's current slot (the walk and the tests address a row by its key).
pub fn slot_of(key: &str) -> Option<usize> {
    state().current.get(key).copied()
}

/// What a slot was drawn for.
pub fn drawn(slot: usize) -> Option<Drawn> {
    state().slots.get(&slot).cloned()
}

// ---------------------------------------------------------------- actions

/// Route one `pd.*` tap (or ⌥Y / ⌥N on the focused row's slot).
pub fn perform(action: &str, slot: usize, store: &Store, compact: bool) -> Outcome {
    let act = match action {
        ACTION_FOLD => {
            toggle(compact);
            return Outcome::Done;
        }
        ACTION_FOCUS => {
            let mut st = state();
            if let Some(d) = st.slots.get(&slot).cloned() {
                st.focused = Some(d.key);
            }
            return Outcome::Done;
        }
        ACTION_APPROVE => RowAction::Approve,
        ACTION_APPROVE_SESSION => RowAction::ApproveSession,
        ACTION_DENY => RowAction::Deny,
        ACTION_STOP => RowAction::Stop,
        _ => return Outcome::Done,
    };
    let mut st = state();
    let Some(d) = st.slots.get(&slot).cloned() else {
        return Outcome::Refused(tr("That action is not available right now.").to_owned());
    };
    // Acting on a row focuses it (the web refocuses the row's button).
    st.focused = Some(d.key.clone());
    let Some(identity) = d.identity.clone() else {
        return Outcome::Refused(tr("That action is not available right now.").to_owned());
    };
    let Some(row) = store.domains.peer.row(&identity) else {
        return Outcome::Refused(tr("This peer is no longer in the roster.").to_owned());
    };
    // The row must still show EXACTLY the ids this control was drawn for.
    if !fleet_driver::still_drawn(&row, act, &d.target()) {
        st.notes.insert(d.key.clone(), (d.request_id.clone(), STALE_DRAWN.to_owned(), peers::now_ms()));
        return Outcome::Refused(tr(STALE_DRAWN).to_owned());
    }
    if !peers::row_actions(&row).contains(&act) {
        return Outcome::Refused(tr("That action is not available right now.").to_owned());
    }
    // One frame per drawn request: a second press while it is in flight
    // sends nothing.
    if !st.inflight.insert((d.key.clone(), d.request_id.clone())) {
        return Outcome::Done;
    }
    st.notes.remove(&d.key);
    Outcome::Spawn(Job { slot, drawn: d, action: act })
}

/// Run ONE dock action through the Fleet's control chain, re-checking the
/// drawn ids right before the frame (`fleet_driver::row_control_drawn`).
/// `Ok` = the acknowledgment ("Sent" / "Stop requested", folded into the
/// row by the chain); `Err` = the bounded refusal (kept on the row).
pub async fn run(job: Job, conv: &crate::flow::Conversation) -> Result<String, String> {
    let res = match job.drawn.identity.as_deref() {
        Some(identity) => {
            fleet_driver::row_control_drawn(conv, identity, job.action, "", Some(&job.drawn.target())).await
        }
        None => Err("That action is not available right now.".to_owned()),
    };
    let mut st = state();
    st.inflight.remove(&(job.drawn.key.clone(), job.drawn.request_id.clone()));
    match &res {
        Ok(_) => {
            st.notes.remove(&job.drawn.key);
        }
        Err(label) => {
            st.notes.insert(job.drawn.key.clone(), (job.drawn.request_id.clone(), label.clone(), peers::now_ms()));
        }
    }
    res
}

// ------------------------------------------------------------------ slots

/// Assign this lowering's slots: a row whose drawn ids are unchanged keeps
/// its slot; a new pending id (or operation / turn) mints a new one.
fn assign_slots(st: &mut State, rows: &[DockRow]) -> Vec<usize> {
    let mut out = Vec::with_capacity(rows.len());
    let mut current = HashMap::new();
    for r in rows {
        let d = Drawn {
            key: r.key.clone(),
            identity: r.identity.clone(),
            request_id: r.request_id.clone(),
            operation_id: r.operation_id.clone(),
            turn_id: r.turn_id.clone(),
        };
        let reuse = st.current.get(&r.key).copied().filter(|s| st.slots.get(s) == Some(&d));
        let slot = reuse.unwrap_or_else(|| {
            let s = st.next_slot;
            st.next_slot += 1;
            s
        });
        st.slots.insert(slot, d);
        current.insert(r.key.clone(), slot);
        out.push(slot);
    }
    let keep: HashSet<usize> = current.values().copied().chain(st.current.values().copied()).collect();
    st.slots.retain(|s, _| keep.contains(s));
    st.previous = st.current.values().copied().collect();
    st.current = current;
    // A focused row that left the dock is no longer a ⌥Y target.
    if let Some(k) = st.focused.clone() {
        if !st.current.contains_key(&k) {
            st.focused = None;
        }
    }
    out
}

// ------------------------------------------------------------------- view

fn fmt(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// The row's live tail after the status word: tokens, then the
/// acknowledgment ("Sent" / "Stop requested") or the replacement-turn marker.
fn tail(r: &DockRow) -> String {
    let mut parts: Vec<String> = Vec::new();
    if r.tokens > 0 {
        parts.push(peers::format_tokens(r.tokens));
    }
    if r.turn_changed {
        parts.push(tr("Peer started a new turn").to_owned());
    } else if let Some(a) = r.ack {
        parts.push(tr(if a == Ack::StopRequested { "Stop requested" } else { "Sent" }).to_owned());
    }
    parts.iter().map(|p| format!(" · {p}")).collect()
}

/// The status word's ink: amber while waiting, red when failed, else the
/// secondary ink (the board's "Working · ↓ 12.4k" is grey).
fn status_ink(s: Status) -> &'static str {
    match s {
        Status::WaitingApproval | Status::WaitingAnswer => tok::AMBER,
        Status::Failed => tok::RED_TEXT,
        _ => tok::MUTED,
    }
}

/// The row's mark in the glyph column (the session tree's marks).
fn glyph(d: &mut Dsl, id: &str, s: Status) {
    d.view(id, &format!("width: {g} height: {g} flow: Overlay align: Align{{x: 0.5 y: 0.5}}", g = fmt(GLYPH)));
    match s {
        Status::Working => d.dot(tok::GREEN, 8.0),
        // Errata (board 4): the waiting row's ⚠ is an amber DOT — the
        // session tree's own waiting dot (`sb_st_wait`).
        Status::WaitingApproval | Status::WaitingAnswer => d.dot("#f5a524ff", 8.0),
        Status::Requested | Status::Starting | Status::StillStarting => {
            let ring = d.anon();
            d.surface(&ring, "width: 9 height: 9", tok::TRANSPARENT, 4.5, Some("#aeaeb2ff"));
            d.close();
        }
        Status::Failed => d.text("", s.glyph(), &Txt::new(13.0, Face::Medium, tok::RED_TEXT)),
        _ => d.text("", s.glyph(), &Txt::new(13.0, Face::Medium, tok::TEXT)),
    }
    d.close();
}

/// A 30 px pill with a 12.5 px label (the kit's `Btn::Primary` / `Outline`
/// looks at the dock's scale): `<id>_box`, `<id>_label`, the tap `<id>`.
fn pill_button(d: &mut Dsl, id: &str, label: &str, event: &str, primary: bool) -> f64 {
    let w = (ui::text_w(label, BTN_PX, Face::Medium) + 2.0 * BTN_PAD).ceil();
    let (fill, fg, border) = if primary { (tok::BLACK, tok::WHITE, None) } else { (tok::SURFACE, tok::TEXT, Some("#c7c7ccff")) };
    d.surface(
        &format!("{id}_box"),
        &format!("width: {} height: {} flow: Overlay align: Align{{x: 0.5 y: 0.5}}", fmt(w), fmt(BTN_H)),
        fill,
        BTN_H / 2.0,
        border,
    );
    let inner = d.anon();
    d.view(&inner, "width: Fill height: Fill flow: Right align: Align{x: 0.5 y: 0.5}");
    d.text(&format!("{id}_label"), label, &Txt::new(BTN_PX, Face::Medium, fg));
    d.close();
    d.tap(id, event);
    d.close();
    w
}

/// Lines a wrapped run takes at `px` in `w` (the kit's width estimate).
fn lines(s: &str, w: f64, px: f64, face: Face) -> f64 {
    (ui::text_w(s, px, face) / w.max(1.0)).ceil().max(1.0)
}

/// The threaded approval card's estimated height.
fn card_h(r: &DockRow, inner_w: f64, control_ready: bool) -> f64 {
    let Some(a) = &r.approval else { return 0.0 };
    let asks = format!("{} {}", tr("asks to run"), a.tool);
    let mut h = 2.0 * CARD_PAD_Y + 17.0 * lines(&asks, inner_w, 13.0, Face::Regular);
    if let Some(t) = &a.target {
        h += 5.0 + 8.0 + 15.0 * lines(t, inner_w - 14.0, 12.0, Face::Mono);
    }
    if control_ready && r.control_supported {
        h += 5.0 + 2.0 + BTN_H + 5.0 + 28.0;
    } else {
        h += 5.0 + 16.0 * 2.0;
    }
    h
}

/// The note shown under a row: the last refusal for the request it shows,
/// or a stale refusal once the card is gone — for [`NOTE_MS`].
fn note_for(st: &State, r: &DockRow, now_ms: u64) -> Option<String> {
    let (req, label, at) = st.notes.get(&r.key)?;
    if now_ms.saturating_sub(*at) > NOTE_MS {
        return None;
    }
    (req == &r.request_id || (r.request_id.is_none() && label == STALE_DRAWN)).then(|| tr(label).to_owned())
}

/// Whether a refusal note is still showing (the dock's clock keeps ticking
/// so it can leave).
pub fn note_showing(now_ms: u64) -> bool {
    state().notes.values().any(|(_, _, at)| now_ms.saturating_sub(*at) <= NOTE_MS)
}

/// One row: the head (glyph, label, elapsed; the status word + tail), the
/// threaded card, the refusal note. Returns the row's estimated height.
#[allow(clippy::too_many_arguments)]
fn row_view(
    d: &mut Dsl,
    texts: &mut Vec<(String, String)>,
    i: usize,
    r: &DockRow,
    slot: usize,
    seat: Seat,
    control_ready: bool,
    focused: bool,
    note: Option<String>,
) -> f64 {
    let id = format!("pd_row_{i}");
    d.view(&id, "width: Fill height: Fit flow: Down spacing: 4");
    // ---- the head (a tap focuses the row: ⌥Y / ⌥N act on it). The ring
    // marks the focused row while it holds a card ⌥Y / ⌥N can answer.
    d.view(&format!("{id}_head"), &format!("width: Fill height: {} flow: Overlay", fmt(ROW_H)));
    if focused && r.approval.is_some() {
        d.surface(&format!("{id}_ring"), "width: Fill height: Fill", tok::TRANSPARENT, 8.0, Some(tok::BLUE));
        d.close();
    }
    let inner = d.anon();
    d.view(
        &inner,
        &format!(
            "width: Fill height: Fill flow: Down spacing: 2 align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: {} right: {} top: 2 bottom: 2}}",
            fmt(PAD_L),
            fmt(PAD_R)
        ),
    );
    let line1 = d.anon();
    d.view(&line1, &format!("width: Fill height: 18 flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: {}", fmt(GAP)));
    glyph(d, &format!("{id}_glyph"), r.status);
    let elapsed = peers::format_elapsed(r.elapsed_ms);
    let label = match &r.model {
        Some(m) => format!("{} · {m}", tr1("Peer {value0}", &r.number.to_string())),
        None => tr1("Peer {value0}", &r.number.to_string()),
    };
    // One line: the renderer ellipsizes what the elapsed leaves (the
    // session tree's own `sb_r_title`), never pushing the elapsed out.
    d.open(
        &format!("{id}_label"),
        "Label",
        &format!(
            "width: Fill height: Fit padding: 0 text: {} flow: Right max_lines: 1 text_overflow: TextOverflow.Ellipsis\ndraw_text.text_style: {}\ndraw_text.color: {}",
            ui::lit(&label),
            ui::text_style(Face::Regular, 14.0),
            tok::TEXT
        ),
    );
    d.close();
    // Elapsed ticks every second: set in place (an empty run in the DSL).
    d.text(&format!("{id}_elapsed"), "", &Txt::new(12.0, Face::Regular, tok::MUTED));
    texts.push((format!("{id}_elapsed"), elapsed));
    d.close();
    let line2 = d.anon();
    d.view(
        &line2,
        &format!("width: Fill height: 16 flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: {}}}", fmt(GLYPH + GAP)),
    );
    d.text(&format!("{id}_status"), tr(r.status.word()), &Txt::new(12.0, Face::Regular, status_ink(r.status)));
    d.text(&format!("{id}_meta"), "", &Txt::new(12.0, Face::Regular, tok::MUTED));
    texts.push((format!("{id}_meta"), tail(r)));
    d.close();
    d.close(); // inner
    d.tap(&format!("{id}_focus"), &format!("{ACTION_FOCUS}#{slot}"));
    d.close(); // head
    let mut h = ROW_H;
    // ---- the threaded approval (only a row blocked on an approval with its
    // real id and contents grows)
    if let Some(a) = &r.approval {
        let wrap = d.anon();
        d.view(&wrap, &format!("width: Fill height: Fit flow: Down padding: Inset{{left: {}}}", fmt(CARD_INSET)));
        d.surface(
            &format!("{id}_card"),
            &format!(
                "width: Fill height: Fit flow: Down spacing: 5 padding: Inset{{left: {x} right: {x} top: {y} bottom: {y}}}",
                x = fmt(CARD_PAD_X),
                y = fmt(CARD_PAD_Y)
            ),
            tok::SURFACE2,
            10.0,
            Some(tok::HAIRLINE),
        );
        let inner_w = seat.width - CARD_INSET - 2.0 * CARD_PAD_X;
        d.text(
            &format!("{id}_asks"),
            &format!("{} {}", tr("asks to run"), a.tool),
            &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Fill).wrap(),
        );
        if let Some(t) = &a.target {
            d.surface(
                &format!("{id}_target_box"),
                "width: Fill height: Fit flow: Right padding: Inset{left: 7 right: 7 top: 4 bottom: 4}",
                tok::CHIP,
                6.0,
                None,
            );
            d.text(&format!("{id}_target"), t, &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill).wrap());
            d.close();
        }
        if control_ready && r.control_supported {
            let acts = d.anon();
            d.view(&acts, "width: Fill height: Fit flow: Right{wrap: true} spacing: 6 padding: Inset{top: 2}");
            pill_button(d, &format!("{id}_approve"), tr("Approve once"), &format!("{ACTION_APPROVE}#{slot}"), true);
            pill_button(d, &format!("{id}_deny"), tr("Deny"), &format!("{ACTION_DENY}#{slot}"), false);
            pill_button(d, &format!("{id}_stop"), tr("Stop"), &format!("{ACTION_STOP}#{slot}"), false);
            d.close();
            let foot = d.anon();
            d.view(&foot, "width: Fill height: 28 flow: Right align: Align{x: 0.0 y: 0.5}");
            d.link_ids(
                &format!("{id}_session_box"),
                &format!("{id}_session_label"),
                &format!("{id}_session"),
                tr("Approve for session"),
                Some(&format!("{ACTION_APPROVE_SESSION}#{slot}")),
                13.0,
                tok::BLUE_TEXT,
            );
            d.gap(W::Fill, 1.0);
            d.surface(
                &format!("{id}_keys_box"),
                "width: Fit height: 20 flow: Right align: Align{x: 0.5 y: 0.5} padding: Inset{left: 6 right: 6}",
                tok::CHIP,
                5.0,
                None,
            );
            d.text(&format!("{id}_keys"), keep("⌥Y / ⌥N"), &Txt::new(11.5, Face::Regular, tok::MUTED));
            d.close();
            d.close();
        } else {
            d.text(
                &format!("{id}_unsupported"),
                tr("This server does not support remote control of peers"),
                &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
            );
        }
        d.close(); // card
        d.close(); // wrap
        h += 4.0 + card_h(r, inner_w, control_ready);
    }
    if let Some(n) = note {
        let wrap = d.anon();
        d.view(&wrap, &format!("width: Fill height: Fit flow: Down padding: Inset{{left: {}}}", fmt(CARD_INSET)));
        d.text(&format!("{id}_note"), &n, &Txt::new(12.0, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
        d.close();
        h += 4.0 + 16.0 * lines(&n, seat.width - CARD_INSET, 12.0, Face::Regular);
    }
    d.close(); // row
    h
}

/// The hairline block above / below the dock (the web's `.dock` border-top;
/// the board's rule over the footer).
fn rule(d: &mut Dsl, id: &str) {
    d.view(id, "width: Fill height: Fit flow: Down padding: Inset{top: 4 bottom: 4}");
    d.hairline();
    d.close();
}

/// The collapsed pill: "Peers · 3 · 1 working · ⚠ 1 waiting · 1/3
/// finished" laid out over as many lines as the width needs (a segment is
/// never split, a line never starts with a separator), the chevron at the
/// right, then the "Show peers · ⌥P" hint.
fn pill_view(d: &mut Dsl, rows: &[DockRow], seat: Seat) -> f64 {
    let (total, working, waiting, finished) = counts(rows);
    // (id, text, ink, face) per segment piece; a segment is one or two pieces.
    type Piece = (&'static str, String, &'static str, Face);
    let mut segs: Vec<Vec<Piece>> = vec![
        vec![("pd_pill_peers", tr("Peers").to_owned(), tok::TEXT, Face::Medium)],
        vec![("pd_pill_total", total.to_string(), tok::MUTED, Face::Regular)],
        vec![("pd_pill_working", tr1("{value0} working", &working.to_string()), tok::MUTED, Face::Regular)],
    ];
    if waiting > 0 {
        segs.push(vec![
            ("pd_pill_warn", "⚠".to_owned(), tok::AMBER, Face::Medium),
            ("pd_pill_waiting", tr1("{value0} waiting", &waiting.to_string()), tok::MUTED, Face::Regular),
        ]);
    }
    segs.push(vec![(
        "pd_pill_finished",
        tr_with("{value0}/{value1} finished", &[("value0", &finished.to_string()), ("value1", &total.to_string())]),
        tok::MUTED,
        Face::Regular,
    )]);
    let px = |face: Face| if face == Face::Medium { 13.0 } else { 12.0 };
    let seg_w = |s: &Vec<Piece>| s.iter().map(|(_, t, _, f)| ui::text_w(t, px(*f), *f)).sum::<f64>() + 4.0 * (s.len() as f64 - 1.0);
    let sep_w = ui::text_w("·", 12.0, Face::Regular) + 8.0;
    // The text column: the pill's padding (12 + 12) and the chevron (12 + 6).
    let budget = seat.width - 24.0 - 18.0;
    let mut lines_of: Vec<Vec<Vec<Piece>>> = vec![Vec::new()];
    let mut used = 0.0;
    for s in segs {
        let w = seg_w(&s);
        let fresh = lines_of.last().is_some_and(|l| l.is_empty());
        if !fresh && used + sep_w + w > budget {
            lines_of.push(vec![s]);
            used = w;
        } else {
            used += if fresh { w } else { sep_w + w };
            if let Some(line) = lines_of.last_mut() {
                line.push(s);
            }
        }
    }
    let n_lines = lines_of.len() as f64;
    let h = (16.0 * n_lines + 2.0 * (n_lines - 1.0) + 20.0).max(36.0);
    d.surface("pd_pill_box", &format!("width: Fill height: {} flow: Overlay", fmt(h)), tok::SURFACE2, 10.0, Some(tok::HAIRLINE));
    let col = d.anon();
    d.view(
        &col,
        "width: Fill height: Fill flow: Down spacing: 2 align: Align{x: 0.0 y: 0.5} padding: Inset{left: 12 right: 30}",
    );
    for line in &lines_of {
        let row = d.anon();
        d.view(&row, "width: Fill height: 16 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 4");
        for (si, seg) in line.iter().enumerate() {
            if si > 0 {
                d.text("", "·", &Txt::new(12.0, Face::Regular, tok::MUTED));
            }
            for (pid, text, ink, face) in seg {
                d.text(pid, text, &Txt::new(px(*face), *face, *ink));
            }
        }
        d.close();
    }
    d.close();
    let chev = d.anon();
    d.view(&chev, "width: Fill height: Fill flow: Right align: Align{x: 1.0 y: 0.5} padding: Inset{right: 12}");
    d.icon("pd_pill_chev", "oc_chev_right.svg", 12.0, tok::TEXT);
    d.close();
    d.tap("pd_pill", ACTION_FOLD);
    d.close();
    // The hint under the pill.
    d.view("pd_hint_row", "width: Fill height: 26 flow: Right align: Align{x: 0.5 y: 0.5}");
    d.text("pd_hint", &format!("{} · {}", tr("Show peers"), keep("⌥P")), &Txt::new(12.0, Face::Regular, tok::MUTED));
    d.close();
    h + 26.0
}

/// Lower the dock for `seat`, or `None` while there are no peers (the web
/// renders nothing). Assigns this lowering's slots.
pub fn lower(store: &Store, now_ms: u64, seat: Seat) -> Option<Lowered> {
    let rows = rows(store, now_ms);
    if rows.is_empty() {
        return None;
    }
    let is_folded = folded(seat.compact);
    let control_ready = fleet_driver::control_supported(store);
    let mut st = state();
    let slots = assign_slots(&mut st, &rows);
    let mut d = Dsl::new();
    let mut texts: Vec<(String, String)> = Vec::new();
    d.view("pd_dock", "width: Fill height: Fit flow: Down");
    rule(&mut d, "pd_rule_top");
    let mut height = 2.0 * RULE_BLOCK;
    let mut scroll_to = None;
    if is_folded {
        height += pill_view(&mut d, &rows, seat);
    } else {
        // ---- the header: PEERS · Hide peers
        d.view(
            "pd_head",
            &format!("width: Fill height: {} flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: {}}}", fmt(HEAD_H), fmt(PAD_L)),
        );
        d.text("pd_title", &tr("Peers").to_uppercase(), &Txt::new(11.0, Face::Semibold, tok::MUTED).w(W::Fill));
        let hide = tr("Hide peers");
        let hw = (ui::text_w(hide, 12.0, Face::Regular) + 2.0 * PAD_R).ceil();
        d.view("pd_hide_box", &format!("width: {} height: {} flow: Overlay align: Align{{x: 1.0 y: 0.5}} padding: Inset{{right: {}}}", fmt(hw), fmt(HEAD_H), fmt(PAD_R)));
        d.text("pd_hide_label", hide, &Txt::new(12.0, Face::Regular, tok::MUTED));
        d.tap("pd_hide", ACTION_FOLD);
        d.close();
        d.close();
        height += HEAD_H;
        // ---- the rows (measured first: the cap decides their container)
        let mut body = Dsl::new();
        let mut offsets = Vec::with_capacity(rows.len());
        let mut natural = 0.0;
        for (i, (r, slot)) in rows.iter().zip(&slots).enumerate() {
            offsets.push(natural);
            let focused = st.focused.as_deref() == Some(r.key.as_str());
            let note = note_for(&st, r, now_ms);
            natural += row_view(&mut body, &mut texts, i, r, *slot, seat, control_ready, focused, note) + 2.0;
        }
        let waiting = rows.iter().any(|r| r.approval.is_some());
        let tree_min = if waiting { TREE_MIN_WAITING } else { TREE_MIN };
        let cap = if seat.room > 0.0 { (seat.room - tree_min - height).max(ROW_H + 8.0) } else { f64::INFINITY };
        if natural > cap {
            d.open("pd_rows", "ScrollYView", &format!("width: Fill height: {} flow: Down spacing: 2", fmt(cap.floor())));
            height += cap.floor();
            // A NEW waiting card is brought into view once (the remount put
            // the region back at its top).
            if let Some((i, r)) = rows.iter().enumerate().find(|(_, r)| r.approval.is_some()) {
                let req = r.approval.as_ref().map(|a| a.request_id.clone()).unwrap_or_default();
                let key = (r.key.clone(), req);
                if st.revealed.as_ref() != Some(&key) {
                    st.revealed = Some(key);
                    scroll_to = Some(offsets[i]);
                }
            }
        } else {
            d.view("pd_rows", "width: Fill height: Fit flow: Down spacing: 2");
            height += natural;
        }
        let body_taps = std::mem::take(&mut body.taps);
        d.raw(body.finish().trim_end());
        d.taps.extend(body_taps);
        d.close();
    }
    rule(&mut d, "pd_rule_bottom");
    d.close();
    let taps = d.taps.clone();
    Some(Lowered { dsl: d.finish(), taps, texts, folded: is_folded, height, scroll_to })
}
