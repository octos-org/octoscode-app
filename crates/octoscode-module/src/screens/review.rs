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
    pub lines: Vec<Line>,
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

/// Record the wire's confirmations. Tolerates every unknown payload by name
/// (LESSONS 6): only `turn/completed` is read, for the confirmed turn id and
/// (when the server attaches one) the turn's diff preview id.
pub fn note_transport_event(evt: &octos_app_transport::TransportEvent) {
    use octos_app_transport::TransportEvent;
    let payload = match evt {
        TransportEvent::DurableNotification { payload, .. }
        | TransportEvent::EphemeralNotification { payload } => payload,
        _ => return,
    };
    if payload.method() != "turn/completed" {
        return;
    }
    let body = octoscode_client::trace::wire_params(payload);
    let turn = body["turn_id"].as_str().map(str::to_owned);
    let preview = body["diff"]["preview_id"]
        .as_str()
        .or_else(|| body["preview_id"].as_str())
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
pub fn query(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
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
            // 3.2's status row: the typed blocked reason wins
            // (`NativeReviewDialog.tsx:84-97` shows it as role=status), then
            // the accepted receipt's specialists count.
            "review.status" => {
                if let Some(b) = st.blocked {
                    return Some(b.to_owned());
                }
                match st.agents {
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
pub fn fold_preview(v: &Value) {
    let mut files = Vec::new();
    let mut lines = Vec::new();
    for file in v["preview"]["files"].as_array().unwrap_or(&Vec::new()) {
        let (add, del) = file_counts(file);
        files.push(FileRow {
            path: file["path"].as_str().unwrap_or_default().to_string(),
            add,
            del,
        });
        for hunk in file["hunks"].as_array().unwrap_or(&Vec::new()) {
            for line in hunk["lines"].as_array().unwrap_or(&Vec::new()) {
                if lines.len() < 8 {
                    lines.push(Line {
                        kind: line["kind"].as_str().unwrap_or("context").to_string(),
                        content: line["content"].as_str().unwrap_or_default().to_string(),
                        old_line: line["old_line"].as_u64().map(|n| n as u32),
                        new_line: line["new_line"].as_u64().map(|n| n as u32),
                    });
                }
            }
        }
    }
    let mut st = state();
    st.files = files;
    st.lines = lines;
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
    // autonomy-02 Code review run
    ("t_status_text", "review.status"),
    ("start_review_label_text", "review.start_label"),
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
pub fn lower_screen(card: &str, ctx: &Ctx<'_>) -> Result<String, String> {
    let (mut card_src, data, kit_dir) = lower_card_src(card)?;
    for (copy_id, binding) in COPY_SLOTS {
        let wants = match card {
            "autonomy-01" => binding.starts_with("review."),
            "autonomy-02" => *binding == "review.status" || *binding == "review.start_label",
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
