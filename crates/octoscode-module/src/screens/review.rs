//! #30a — Stage C wiring for board 3.1 Review panel, 3.2 Code review run
//! (`design/stage-b/autonomy/cards/autonomy-{01,02}`).
//!
//! Same door rule as the other screens (8.8 condition 2): the cards see
//! binding ids and action ids only; this module owns the board-3 review table.
//!
//! * [`query`] — store + the screen cache ([`RevUi`]) → the cards' data slots;
//! * [`resolve`] — an action id → a pure [`Effect`];
//! * [`spawn`] — the effect → `diff/preview/get` / `review/start` over the
//!   production client ([`Conversation::client`]), receipts folded into the
//!   store's review domain and the screen cache.
//!
//! Behaviour citations (docs/parity-matrix.csv, review rows):
//! * authoritative diff preview — `diff/preview/get`
//!   (`DiffReviewDialog.tsx:25`; the +N/−N totals are counted over the
//!   preview's lines, `DiffReviewDialog.tsx:32-42`, header `+{a}`/`−{d}` at
//!   `:79-80`);
//! * word-level marks — `diffKind` collapses the wire's line kinds to
//!   added/removed/context (`diff-presentation.ts:17-23`); the card carries
//!   the marks as its `mk_*` gutter slots (decoration limits
//!   `diff-presentation.ts:26-40` are a renderer concern, never a truncation);
//! * native review start — `review/start` gated on the method AND the
//!   `review.start.v1` feature (`native-review.ts:18-24`); every withholding
//!   cause is one typed literal (`native-review.ts:33-55`); the dialog
//!   disables Start while blocked and shows the reason as `role=status`
//!   (`NativeReviewDialog.tsx:84-97`); the web sends
//!   `{session_id, turn_id, delivery: "inline"}` (`history.ts:250`).
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use octoscode_store::domains::review::StartedReview;
use octoscode_store::Store;

use crate::bindings::Ctx;
use crate::flow::Conversation;

/// The declared binding ids (`id`, `description`) — every slot the two cards
/// fill from live state. Authored chrome the wire never carries (title, fold
/// note, findings badges) stays authored and is NOT declared here.
pub const BINDINGS: &[(&str, &str)] = &[
    ("review.scope", "the scope pill label, e.g. \"Last turn ▾\""),
    ("review.add", "\"+62\"-style header additions total"),
    ("review.del", "\"-5\"-style header deletions total"),
    ("review.file1.path", "first changed file's path"),
    ("review.file1.add", "first file's \"+n\""),
    ("review.file1.del", "first file's \"-n\""),
    ("review.file2.path", "second changed file's path"),
    ("review.file2.add", "second file's \"+n\""),
    ("review.file2.del", "second file's \"-n\""),
    ("review.file3.path", "third changed file's path"),
    ("review.file3.add", "third file's \"+n\""),
    ("review.file3.del", "third file's \"-n\""),
    ("review.line0", "first preview line's content"),
    ("review.line1", "second preview line's content"),
    ("review.line2", "third preview line's content"),
    ("review.line3", "fourth preview line's content"),
    ("review.line4", "fifth preview line's content"),
    ("review.line5", "sixth preview line's content"),
    ("review.line6", "seventh preview line's content"),
    ("review.line7", "eighth preview line's content"),
    ("review.num0", "first line's gutter number"),
    ("review.num1", "second line's gutter number"),
    ("review.num2", "third line's gutter number"),
    ("review.num3", "fourth line's gutter number"),
    ("review.num4", "fifth line's gutter number"),
    ("review.num5", "sixth line's gutter number"),
    ("review.num6", "seventh line's gutter number"),
    ("review.num7", "eighth line's gutter number"),
    ("review.mark2", "third line's word-level mark (+/-)"),
    ("review.mark3", "fourth line's word-level mark (+/-)"),
    ("review.mark4", "fifth line's word-level mark (+/-)"),
    ("review.mark5", "sixth line's mark (+/-)"),
    ("review.mark6", "seventh line's mark (+/-)"),
    ("review.status", "code-review run status or typed blocked reason"),
    ("review.start_label", "the start button's label (\"Start native review\")"),
    ("review.file_path", "the diff header: the SELECTED file's path"),
    ("review.fold", "\"⋮ N unmodified lines ⋮\" from the real fold"),
    ("review.finding_high.path", "high finding's path (empty until a review result)"),
    ("review.finding_high.text", "high finding's text (empty until a review result)"),
    ("review.finding_low.path", "low finding's path (empty until a review result)"),
    ("review.finding_low.text", "low finding's text (empty until a review result)"),
];

/// The action ids the cards' `service-actions.json` declare. ONE OWNER: these
/// never reach the conversation router (lib.rs routes them first).
pub const ACTIONS: &[(&str, &str)] = &[
    ("diff.scope", "cycle the diff scope pill (last turn / project)"),
    ("review.start", "start the server's native code review"),
];

pub fn owns_binding(id: &str) -> bool {
    BINDINGS.iter().any(|(b, _)| *b == id)
}

pub fn owns_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

/// lib.rs routes owned action ids before the conversation router.
pub fn is_action(id: &str) -> bool {
    owns_action(id)
}

// ---------------------------------------------------------------- screen state

/// The diff scope the pill shows. The web's dialog reviews either the last
/// confirmed turn or the current project changes
/// (`NativeReviewDialog.tsx:80-82` placeholder, `DiffReviewDialog` title).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    LastTurn,
    Project,
}

impl Scope {
    fn label(&self) -> &'static str {
        match self {
            Scope::LastTurn => "Last turn",
            Scope::Project => "Project",
        }
    }
    fn cycle(&self) -> Self {
        match self {
            Scope::LastTurn => Scope::Project,
            Scope::Project => Scope::LastTurn,
        }
    }
}

/// One flattened preview line for the slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub kind: String,
    pub content: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

impl Line {
    /// `diffKind` (`diff-presentation.ts:17-23`): the wire kinds collapse to
    /// added/removed/context; the gutter mark is `+`/`-`/empty.
    pub fn mark(&self) -> &'static str {
        match self.kind.as_str() {
            "added" => "+",
            "removed" => "-",
            _ => "",
        }
    }
    /// The gutter number the web shows: the new side, else the old side.
    pub fn num(&self) -> String {
        match (self.new_line, self.old_line) {
            (Some(n), _) => n.to_string(),
            (None, Some(o)) => o.to_string(),
            _ => String::new(),
        }
    }
}

/// One file's live row values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    pub path: String,
    pub add: u64,
    pub del: u64,
}

/// The screen cache (the web keeps the same data in component state; the
/// protocol never pushes it). UI-local, like `workspace::WsState`.
#[derive(Debug, Default)]
pub struct RevUi {
    pub scope: Option<Scope>,
    /// The last CONFIRMED turn id the wire announced (`turn/completed`) —
    /// the turn a native review is started FOR (`history.ts:250`).
    pub last_turn_id: Option<String>,
    /// The preview id the wire attached to that turn
    /// (`interaction.ts:99-100` reads it off the turn's diff).
    pub preview_id: Option<String>,
    /// The fetched preview's files (flattened rows + per-file counts).
    pub files: Vec<FileRow>,
    /// The file whose hunks the diff body renders (the first file that
    /// carries a change, else the first) — the header names it.
    pub selected_file: Option<String>,
    /// The selected file's rows, windowed to the card's row slots.
    pub lines: Vec<Line>,
    /// Preview rows not displayed (the real fold count under "⋮ N unmodified
    /// lines ⋮").
    pub hidden: u64,
    /// The accepted `review/start` receipt (agents admitted).
    pub agents: Option<u32>,
    /// The typed reason native review is withheld, when it is.
    pub blocked: Option<&'static str>,
}

fn state() -> MutexGuard<'static, RevUi> {
    static STATE: OnceLock<Mutex<RevUi>> = OnceLock::new();
    // Poison-tolerant: the cache is plain data; a panicking holder (a test
    // assert) must not cascade PoisonError through every later query.
    STATE
        .get_or_init(|| Mutex::new(RevUi::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Test/probe access to the screen cache (a separate binary sees only the
/// crate's public API). Sets the full scenario BEFORE the query/resolve.
pub fn ui() -> MutexGuard<'static, RevUi> {
    state()
}

/// Serialise whole set+act+assert scenarios in tests (query/resolve take the
/// STATE lock internally; this lock is a different mutex, so no cycle).
#[doc(hidden)]
pub fn test_lock() -> MutexGuard<'static, ()> {
    static SEQ: OnceLock<Mutex<()>> = OnceLock::new();
    // Poison-tolerant: one test's assert failure must not fail the rest.
    SEQ
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Clear the screen cache (tests start from a known state).
pub fn reset() {
    *state() = RevUi::default();
}
use std::sync::MutexGuard;

/// Record the wire's confirmations. The ONLY turn-terminal path the real
/// server sends is the projection envelope (`payload.type ==
/// "turn_terminal"`, 28 frames across the committed fixtures; live-gate seq
/// 156 carries `turn_id` + `data.outcome:"completed"` top-level) — no
/// fixture ever carries a `turn/completed` METHOD frame, so listening on one
/// wired the screen to a path production never takes (LESSONS 5).
pub fn note_transport_event(evt: &octos_app_transport::TransportEvent) {
    use octos_app_transport::TransportEvent;
    let payload = match evt {
        TransportEvent::DurableNotification { payload, .. }
        | TransportEvent::EphemeralNotification { payload } => payload,
        _ => return,
    };
    if payload.method() != "projection/envelope" {
        return;
    }
    note_envelope(&octoscode_client::trace::wire_params(payload));
}

/// Fold one projection-envelope body: a COMPLETED `turn_terminal` confirms
/// the turn a native review reviews (`history.ts:250`). The preview id, when
/// the server attaches one, rides the same record (the web reads it off the
/// turn's diff, `interaction.ts:99-100`); the committed fixtures carry none,
/// so the slot honestly stays None (the authored copy renders) until a real
/// preview arrives.
pub fn note_envelope(body: &Value) {
    if body["payload"]["type"] != "turn_terminal"
        || body["payload"]["data"]["outcome"] != "completed"
    {
        return;
    }
    let turn = body["turn_id"].as_str().map(str::to_owned);
    let preview = body["payload"]["data"]["diff"]["preview_id"]
        .as_str()
        .or_else(|| body["diff"]["preview_id"].as_str())
        .map(str::to_owned);
    let mut st = state();
    if let Some(t) = turn {
        st.last_turn_id = Some(t);
    }
    if let Some(p) = preview {
        st.preview_id = Some(p);
    }
}

// ------------------------------------------------------------------- bindings

/// Per-file +/− counts over the file's hunk lines
/// (`DiffReviewDialog.tsx:34-41` counts the same way per file).
fn file_counts(file: &Value) -> (u64, u64) {
    let mut add = 0u64;
    let mut del = 0u64;
    for hunk in file["hunks"].as_array().unwrap_or(&Vec::new()) {
        for line in hunk["lines"].as_array().unwrap_or(&Vec::new()) {
            match line["kind"].as_str() {
                Some("added") => add += 1,
                Some("removed") => del += 1,
                _ => {}
            }
        }
    }
    (add, del)
}

/// The typed withholding reason, precedence per `native-review.ts:33-55`.
/// (`authorityChanged` needs RPC-authority identity the store does not keep —
/// not detectable here; the other four are.)
fn blocked_reason(store: &Store, ui: &crate::flow::FlowUi) -> Option<&'static str> {
    let caps = store.capabilities();
    let method_ok = caps.iter().any(|c| c == "review/start");
    let feature_ok = caps.iter().any(|c| c == "review.start.v1");
    if !(method_ok && feature_ok) {
        return Some("This server does not advertise native code review.");
    }
    if ui.turn_active() {
        return Some(
            "Wait for this Session's active turn and queued prompts to settle before starting review.",
        );
    }
    if ui.approval_pending() || ui.question_pending() {
        return Some(
            "Wait for this Session's pending questions and approvals to settle before starting review.",
        );
    }
    if state().last_turn_id.is_none() {
        return Some("Wait for this Session to finish recovery before starting review.");
    }
    None
}

/// Resolve one binding id. `None` = not declared (authored copy stays).
pub fn query(_ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    // NO FlowUi lock here: bindings::query already holds it when it
    // delegates to this arm (std Mutex is not reentrant — a second lock
    // self-deadlocks; caught by f30a's coverage test + a thread sample).
    let st = state();

    let text = || -> Option<String> {
        Some(match id {
            "review.scope" => format!("{} ▾", st.scope.as_ref().unwrap_or(&Scope::LastTurn).label()),
            // Header totals over the fetched preview
            // (`DiffReviewDialog.tsx:32-42,79-80`); None keeps the authored.
            "review.add" => {
                let a: u64 = st.files.iter().map(|f| f.add).sum();
                (a > 0 || !st.files.is_empty()).then(|| format!("+{a}"))?
            }
            "review.del" => {
                let d: u64 = st.files.iter().map(|f| f.del).sum();
                (!st.files.is_empty()).then(|| format!("-{d}"))?
            }
            "review.file1.path" => st.files.first()?.path.clone(),
            "review.file2.path" => st.files.get(1)?.path.clone(),
            "review.file3.path" => st.files.get(2)?.path.clone(),
            "review.file1.add" => st.files.first().map(|f| format!("+{}", f.add))?,
            "review.file2.add" => st.files.get(1).map(|f| format!("+{}", f.add))?,
            "review.file3.add" => st.files.get(2).map(|f| format!("+{}", f.add))?,
            "review.file1.del" => st.files.first().map(|f| format!("-{}", f.del))?,
            "review.file2.del" => st.files.get(1).map(|f| format!("-{}", f.del))?,
            "review.file3.del" => st.files.get(2).map(|f| format!("-{}", f.del))?,
            other if other.starts_with("review.line") => {
                let i: usize = other.trim_start_matches("review.line").parse().ok()?;
                st.lines.get(i)?.content.clone()
            }
            other if other.starts_with("review.num") => {
                let i: usize = other.trim_start_matches("review.num").parse().ok()?;
                st.lines.get(i).map(Line::num)?
            }
            other if other.starts_with("review.mark") => {
                let i: usize = other.trim_start_matches("review.mark").parse().ok()?;
                st.lines.get(i).map(|l| l.mark().to_owned())?
            }
            "review.start_label" => "Start native review".to_owned(),
            // The diff header names the SELECTED file (the rendered hunks'),
            // not a design placeholder.
            "review.file_path" => st.selected_file.clone()?,
            // The real fold: how many preview rows the card does not show.
            "review.fold" => {
                if st.hidden > 0 {
                    format!("⋮ {} unmodified lines ⋮", st.hidden)
                } else {
                    String::new()
                }
            }
            // Findings come from a review RESULT; the wire carries none until
            // the server reports one (`native-review.ts:63` — a server-owned
            // workflow). No result -> the authored sample rows go EMPTY (the
            // web's running/empty state), never a fake finding.
            "review.finding_high.path"
            | "review.finding_high.text"
            | "review.finding_low.path"
            | "review.finding_low.text" => String::new(),
            // 3.2's status row: the typed blocked reason wins
            // (`NativeReviewDialog.tsx:84-97` shows it as role=status), then
            // the accepted receipt's specialists count.
            "review.status" => {
                if let Some(b) = st.blocked {
                    return Some(b.to_owned());
                }
                match st.agents {
                    // "Reviewing {files} files · {agents} specialists": the
                    // file count is the folded preview's, the specialist count
                    // the accepted review/start receipt's (agent_count).
                    Some(a) if !st.files.is_empty() => {
                        Some(format!("Reviewing {} files · {a} specialists", st.files.len()))
                    }
                    Some(a) => Some(format!("Reviewing · {a} specialists")),
                    None => None,
                }?
            }
            _ => return None,
        })
    };
    text().map(Value::String)
}

// --------------------------------------------------------------------- actions

/// The pure action table.
#[derive(Debug)]
pub enum Effect {
    /// `diff.scope` — cycle the pill and re-fetch the preview for the scope.
    ScopeCycle,
    /// `review.start` — `review/start` for the confirmed turn
    /// (`history.ts:250`).
    StartReview { session_id: String, turn_id: String },
    /// The start was withheld; the typed reason goes to the status row.
    Blocked(&'static str),
    Unhandled(String),
}

pub fn resolve(action: &str, _index: usize, ctx: &Ctx<'_>) -> Effect {
    match action {
        "diff.scope" => {
            let next = {
                let mut st = state();
                let next = st.scope.as_ref().unwrap_or(&Scope::LastTurn).cycle();
                st.scope = Some(next.clone());
                next
            };
            let _ = next;
            Effect::ScopeCycle
        }
        "review.start" => {
            let session_id = ctx.store.active_session().unwrap_or_default();
            let (turn, blocked) = {
                let st = state();
                (st.last_turn_id.clone(), st.blocked)
            };
            let ui = ctx.ui.lock().unwrap();
            let blocked = blocked.or(blocked_reason(ctx.store, &ui));
            drop(ui);
            match (turn, blocked) {
                (Some(turn_id), None) => Effect::StartReview { session_id, turn_id },
                (_, Some(reason)) => Effect::Blocked(reason),
                (None, None) => Effect::Blocked(
                    "Wait for this Session to finish recovery before starting review.",
                ),
            }
        }
        other => Effect::Unhandled(other.to_owned()),
    }
}

/// UI-local half: a blocked start writes its typed reason into the status row
/// (`NativeReviewDialog.tsx:84-97`).
pub fn apply(effect: &Effect) {
    if let Effect::Blocked(reason) = effect {
        state().blocked = Some(reason);
    }
}

/// The transport half, over the production client.
pub async fn perform(effect: Effect, conv: &Conversation) -> Result<(), String> {
    match effect {
        Effect::ScopeCycle => {
            // Re-fetch the preview for the (cycled) scope.
            let (session, preview_id) = {
                let st = state();
                (conv.session_id(), st.preview_id.clone())
            };
            let Some(preview_id) = preview_id else {
                // No preview the wire has announced — the authored stays.
                return Ok(());
            };
            let v = conv
                .client()
                .request("diff/preview/get", json!({"session_id": session, "preview_id": preview_id}))
                .await
                .map_err(|e| format!("diff/preview/get: {e}"))?;
            fold_preview(&v);
            Ok(())
        }
        Effect::StartReview { session_id, turn_id } => {
            let v = conv
                .client()
                .request(
                    "review/start",
                    json!({"session_id": session_id, "turn_id": turn_id, "delivery": "inline"}),
                )
                .await
                .map_err(|e| format!("review/start: {e}"))?;
            // Fold the accepted receipt (`review.rs` ReviewStartResult; the
            // web's parseReviewStartResult) into the store's review domain.
            if v["accepted"].as_bool() == Some(true) {
                let agents = v["agent_count"].as_u64().unwrap_or(0) as u32;
                conv.store
                    .domains
                    .review
                    .note_review(StartedReview { session_id, turn_id, agent_count: agents });
                state().agents = Some(agents);
                state().blocked = None;
            }
            Ok(())
        }
        Effect::Blocked(_) | Effect::Unhandled(_) => Ok(()),
    }
}

/// lib.rs helper: resolve + apply + spawn in one call (the workspace shape).
pub fn spawn(effect: Effect, rt: &tokio::runtime::Runtime, conv: std::sync::Arc<Conversation>) {
    apply(&effect);
    rt.spawn(async move {
        if let Err(e) = perform(effect, &conv).await {
            ::log::warn!("octoscode: screens/review: {e}");
        }
    });
}

/// Fold a `diff/preview/get` result into the screen cache (files with counts,
/// flattened lines). Shapes are the octos-core types (`ui_protocol.rs:2700+`).
/// The card's diff-body row slots (`ln_0..ln_7` / `dl_0..dl_7`).
const ROW_SLOTS: usize = 8;

pub fn fold_preview(v: &Value) {
    struct RawFile {
        path: String,
        lines: Vec<Line>,
    }
    let mut files = Vec::new();
    let mut raws: Vec<RawFile> = Vec::new();
    let mut total = 0u64;
    for file in v["preview"]["files"].as_array().unwrap_or(&Vec::new()) {
        let (add, del) = file_counts(file);
        let mut lines = Vec::new();
        for hunk in file["hunks"].as_array().unwrap_or(&Vec::new()) {
            for line in hunk["lines"].as_array().unwrap_or(&Vec::new()) {
                lines.push(Line {
                    kind: line["kind"].as_str().unwrap_or("context").to_string(),
                    content: line["content"].as_str().unwrap_or_default().to_string(),
                    old_line: line["old_line"].as_u64().map(|n| n as u32),
                    new_line: line["new_line"].as_u64().map(|n| n as u32),
                });
            }
        }
        total += lines.len() as u64;
        files.push(FileRow {
            path: file["path"].as_str().unwrap_or_default().to_string(),
            add,
            del,
        });
        raws.push(RawFile { path: files.last().unwrap().path.clone(), lines });
    }
    // The rendered file: the first one that carries a change, else the first
    // (a preview of untouched files still shows its first file, honestly
    // empty).
    let sel = raws
        .iter()
        .position(|f| f.lines.iter().any(|l| l.kind != "context"))
        .unwrap_or(0);
    let mut shown: Vec<Line> = Vec::new();
    if let Some(f) = raws.get(sel) {
        if f.lines.len() <= ROW_SLOTS {
            shown = f.lines.clone();
        } else {
            // Window so the FIRST CHANGE is visible, with up to two context
            // lines above it (#28 diff-view's changed-window + fold shape).
            let first = f
                .lines
                .iter()
                .position(|l| l.kind != "context")
                .unwrap_or(0);
            let start = first.saturating_sub(2);
            let end = (start + ROW_SLOTS).min(f.lines.len());
            shown = f.lines[start..end].to_vec();
        }
    }
    let hidden = total - shown.len() as u64;
    let mut st = state();
    st.files = files;
    st.selected_file = raws.get(sel).map(|f| f.path.clone());
    st.lines = shown;
    st.hidden = hidden;
}

// ------------------------------------------------------------------- lowering

/// The copy-slot → binding table (copy ids read off the two authored cards).
pub const COPY_SLOTS: &[(&str, &str)] = &[
    // autonomy-01 Review panel
    ("scope_label_text", "review.scope"),
    ("t_add_text", "review.add"),
    ("t_del_text", "review.del"),
    ("file_1_path_text", "review.file1.path"),
    ("file_1_add_text", "review.file1.add"),
    ("file_1_del_text", "review.file1.del"),
    ("file_2_path_text", "review.file2.path"),
    ("file_2_add_text", "review.file2.add"),
    ("file_2_del_text", "review.file2.del"),
    ("file_3_path_text", "review.file3.path"),
    ("file_3_add_text", "review.file3.add"),
    ("file_3_del_text", "review.file3.del"),
    ("dl_0_text", "review.line0"),
    ("dl_1_text", "review.line1"),
    ("dl_2_text", "review.line2"),
    ("dl_3_text", "review.line3"),
    ("dl_4_text", "review.line4"),
    ("dl_5_text", "review.line5"),
    ("dl_6_text", "review.line6"),
    ("dl_7_text", "review.line7"),
    ("ln_0_text", "review.num0"),
    ("ln_1_text", "review.num1"),
    ("ln_2_text", "review.num2"),
    ("ln_3_text", "review.num3"),
    ("ln_4_text", "review.num4"),
    ("ln_5_text", "review.num5"),
    ("ln_6_text", "review.num6"),
    ("ln_7_text", "review.num7"),
    ("mk_2_text", "review.mark2"),
    ("mk_3_text", "review.mark3"),
    ("mk_4_text", "review.mark4"),
    ("mk_5_text", "review.mark5"),
    ("mk_6_text", "review.mark6"),
    ("diff_file_path_text", "review.file_path"),
    ("t_fold_text", "review.fold"),
    // autonomy-02 Code review run
    ("t_status_text", "review.status"),
    ("start_review_label_text", "review.start_label"),
    ("finding_high_path_text", "review.finding_high.path"),
    ("finding_high_text_text", "review.finding_high.text"),
    ("finding_low_path_text", "review.finding_low.path"),
    ("finding_low_text_text", "review.finding_low.text"),
];

fn cards_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/stage-b/autonomy/cards")
}

/// The card source with the CURRENT values in its `copy` slots + its data and
/// kit dir (the capture host consumes all three).
pub fn lower_card_src(card: &str) -> Result<(String, Value, std::path::PathBuf), String> {
    let dir = cards_root().join(card);
    let read = |rel: &str| -> Result<String, String> {
        std::fs::read_to_string(dir.join(rel)).map_err(|e| format!("read {card}/{rel}: {e}"))
    };
    let card_src = read("page.card")?;
    let data: Value = serde_json::from_str(&read("page.data.json")?)
        .map_err(|e| format!("parse {card} data: {e}"))?;
    Ok((card_src, data, dir.join("kit")))
}

/// Lower one screen card to Splash DSL with the live values injected — the
/// same `l0::prepare` → `inspectable` → `to_makepad_ui` chain the module's
/// other screens run (the renderer behind the accepted Gate-B PNGs).
/// The kit component ids the diff rows are built from (the authored card's
/// own trio per style: context = bare ln/dl texts; removed/added = a chip
/// Surface wrapping ln+mk+dl — chip_130 is the removed chip, chip_132 the
/// added one, `page.card:217-231`).
const ROW_CTX_LN: &str = "Textd6b85f1bbf60";
const ROW_CTX_DL: &str = "Textbd31eef74fbf";
const ROW_RED: &str = "Surface4db4e42a8189";
const ROW_RED_LN: &str = "Text1eead614118f";
const ROW_RED_MK: &str = "Text9ac4cebca82f";
const ROW_RED_DL: &str = "Text0ac44d04eeea";
const ROW_GREEN: &str = "Surface04a27fc304ff";
const ROW_GREEN_LN: &str = "Text90ff5f3e7b35";
const ROW_GREEN_MK: &str = "Text3349daa3095a";
const ROW_GREEN_DL: &str = "Text6d68fc1c53dd";

/// Rebuild the `diff_rows` group with ONE ROW PER LINE of the selected
/// file's real hunks (`ln_i`/`dl_i` texts keep flowing through their
/// `copy.*_text` slots; the +/- mark is baked literally — the card authors
/// only `mk_2..mk_6`). Context rows are bare texts; changed rows get the
/// design's chip. 0 rows -> an empty group (the honest empty diff). Returns
/// the new source plus `(row index, content chars, is_added)` for every
/// CHANGED row — the caller synthesises one kit placement per chip
/// (`prepare` requires a placement per instance; the authored card only
/// carries chip_130/131/132).
pub fn rebuild_diff_rows(card_src: &str) -> (String, Vec<(usize, u64, bool)>) {
    let st = state();
    let open = "Group3d2637879433(instance: \"diff_rows\") {";
    let start = match card_src.find(open) {
        Some(i) => i,
        // the group is absent: nothing to rebuild, no chips emitted
        None => return (card_src.to_owned(), Vec::new()),
    };
    // Find the group's matching close brace.
    let mut depth = 0i32;
    let mut end = start;
    for (i, c) in card_src[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let mut body = String::from(open);
    let mut changed: Vec<(usize, u64, bool)> = Vec::new();
    body.push('\n');
    for (i, line) in st.lines.iter().enumerate() {
        match line.kind.as_str() {
            "removed" | "added" => {
                let (surface, ln, mk, dl) = if line.kind == "removed" {
                    (ROW_RED, ROW_RED_LN, ROW_RED_MK, ROW_RED_DL)
                } else {
                    (ROW_GREEN, ROW_GREEN_LN, ROW_GREEN_MK, ROW_GREEN_DL)
                };
                let mark = if line.kind == "removed" { "-" } else { "+" };
                changed.push((i, line.content.chars().count() as u64, mark == "+"));
                body.push_str(&format!(
                    "      {surface}(instance: \"chip_{i}\") {{\n        \
                     {ln}(instance: \"ln_{i}\", text: copy.ln_{i}_text)\n        \
                     {mk}(instance: \"mk_{i}\", text: \"{mark}\")\n        \
                     {dl}(instance: \"dl_{i}\", text: copy.dl_{i}_text)\n      }}\n"
                ));
            }
            _ => {
                body.push_str(&format!(
                    "      {ROW_CTX_LN}(instance: \"ln_{i}\", text: copy.ln_{i}_text)\n      \
                     {ROW_CTX_DL}(instance: \"dl_{i}\", text: copy.dl_{i}_text)\n"
                ));
            }
        }
    }
    body.push('}');
    (format!("{}{}{}", &card_src[..start], body, &card_src[end..]), changed)
}

pub fn lower_screen(card: &str, ctx: &Ctx<'_>) -> Result<String, String> {
    let (mut card_src, mut data, kit_dir) = lower_card_src(card)?;
    // #30a2 ①: the diff body renders the SELECTED file's real hunks, one row
    // per line (before the copy injection, which then fills ln/dl texts).
    if card == "autonomy-01" {
        let (src2, changed) = rebuild_diff_rows(&card_src);
        card_src = src2;
        // prepare requires a kit placement per INSTANCE: the surgery emitted
        // chip_<row> instances, so drop the authored chip_130/131/132
        // placements, drop any mk placement whose row is no longer changed,
        // and synthesise one placement per emitted chip — y from the row's
        // own ln_i, width hugging the real content (~8px/char at the
        // design's code size; authored dl_4: 25 chars -> 200px).
        let obj = data
            .get_mut("$kit")
            .and_then(|k| k.get_mut("placements"))
            .and_then(|p| p.as_object_mut())
            .ok_or_else(|| "autonomy-01: no $kit.placements".to_string())?;
        for k in ["chip_130", "chip_131", "chip_132"] {
            obj.remove(k);
        }
        let used: Vec<usize> = changed.iter().map(|(i, _, _)| *i).collect();
        for k in 0..ROW_SLOTS {
            if !used.contains(&k) {
                obj.remove(&format!("mk_{k}"));
            }
        }
        for (i, chars, added) in &changed {
            let ln_key = format!("ln_{i}");
            let y = obj
                .get(&ln_key)
                .and_then(|c| c.get("layout"))
                .and_then(|l| l.get("y"))
                .and_then(Value::as_f64)
                .unwrap_or(353.79 + *i as f64 * 34.96);
            let w = ((*chars as f64) * 8.0 + 24.0).clamp(48.0, 330.0);
            let surface = if *added { ROW_GREEN } else { ROW_RED };
            obj.insert(
                format!("chip_{i}"),
                json!({"component": surface, "layout": {"x": 76.0, "y": y - 3.0, "w": w, "h": 33.0}}),
            );
            let mk_key = format!("mk_{i}");
            if !obj.contains_key(&mk_key) {
                let mkc = if *added { ROW_GREEN_MK } else { ROW_RED_MK };
                obj.insert(
                    mk_key,
                    json!({"component": mkc, "layout": {"x": 76.0, "y": y, "w": 12.0, "h": 31.0}}),
                );
            }
        }
    }
    // #30a2 ③: "Start native review" (`NativeReviewDialog.tsx:90`) is wider
    // than the authored 123px control — widen to 153px, keep the right edge,
    // so the label is never clipped by its own button.
    if card == "autonomy-02" {
        if let Some(pl) = data.get_mut("$kit").and_then(|k| k.get_mut("placements")) {
            for k in ["start_review", "start_review_control", "start_review_surface"] {
                if let Some(l) = pl.get_mut(k).and_then(|c| c.get_mut("layout")) {
                    l["x"] = json!(230.0);
                    l["w"] = json!(153.0);
                }
            }
            if let Some(l) = pl
                .get_mut("start_review_label")
                .and_then(|c| c.get_mut("layout"))
            {
                l["x"] = json!(246.0);
                l["w"] = json!(121.0);
            }
        }
    }
    for (copy_id, binding) in COPY_SLOTS {
        let wants = match card {
            "autonomy-01" => binding.starts_with("review."),
            "autonomy-02" => {
                *binding == "review.status"
                    || *binding == "review.start_label"
                    || binding.starts_with("review.finding")
            }
            _ => false,
        };
        if !wants {
            continue;
        }
        if let Some(v) = query(ctx, binding) {
            if let Value::String(s) = v {
                if let Some(next) = crate::l0_host::set_copy(&card_src, copy_id, &s) {
                    card_src = next;
                }
            }
        }
    }
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &kit_dir)
        .map_err(|e| format!("prepare {card}: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let ui = octoscript_makepad::design::to_makepad_ui(&tree)
        .map_err(|e| format!("to_makepad_ui {card}: {e}"))?;
    let prefix = format!("scr_{}", card.trim_start_matches("autonomy-"));
    Ok(ui.replace("beauty_0", &prefix))
}
