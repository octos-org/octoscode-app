//! Card #17 — the **item components**: the L0 layer that owns each list item's
//! *look*, while native Rust (the module) owns structure + scale.
//!
//! ## Why this exists (the operator's "option 1, hybrid")
//!
//! #15b mounted whole Stage-B *screens* (406x776 artboards) as cards and showed
//! one at a time behind tabs. A live conversation screen is not one screen: it
//! is a thread list, a timeline of entries and a composer. So the structure is
//! native (`PortalList` virtualization, the D12 columns) and the *item look*
//! comes from a **reusable L0 component**, lowered exactly the way a card is
//! ([`crate::l0_host`] — the same `l0::prepare` → `design::to_makepad_ui` chain
//! the Gate-B renders used; card-host `host.rs:144-154`).
//!
//! ## A component is data, like a card
//!
//! [`cards`](crate::cards) reads a card from `design/cards/index.json`. A
//! component is the same shape, smaller: a ledger with a `copy` per live value,
//! a **declared binding id** per `copy` ([`Binding`]), and a kit. **No measured
//! artboard geometry is read** (RULES 8.10: runtime content → native flow
//! regions) — only leaf sizes, which are the item's own box.
//!
//! ## The kit (measured, not assumed)
//!
//! A probe (`examples/l0try.rs`) settled the contract: `l0::prepare` has two
//! branches — the **native-kit pack** branch when the tree contains a
//! `Kit(component: …)` node, else the design branch. The native branch requires
//! a `<dir>/native/<mood>/kit.json` **pack** whose `components` map declares
//! every component the ledger (and its placements) references — including the
//! `view root` component itself. Plain design vocabulary (`View { … }`) is not
//! an accepted L0 constructor.
//!
//! So the placeholder components ship ONE shared pack,
//! `design/components/native/light/kit.json`, declaring two neutral components
//! `PSurface` (a `stack`/`surface` root) and `PText` (a `text` leaf). Its token
//! table uses only the `color`/`size`/`line_height`/`weight` properties the kit
//! pack reader understands.
//!
//! ## Where the components come from
//!
//! Card #16 (`design/components/`) owns the real components and runs in
//! parallel. Until each lands, [`builtin`] supplies a **clearly-labelled
//! placeholder** ledger so the host work is not blocked. [`resolve`] prefers
//! `design/components/<id>.l0` + `<id>.json` when present, so swapping in #16's
//! component is a **file drop by id** — no Rust change.
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::bindings;
use crate::l0_host;

/// The item kinds the conversation screen instantiates (card #17 §"Lists are
/// native & virtualized").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// One row of the left thread list.
    ThreadRow,
    /// The person's message bubble.
    UserBubble,
    /// Assistant prose (deltas fold here; reasoning is folded away).
    AssistantProse,
    /// One tool call cell.
    ToolCell,
    /// The live activity row ("Working · 12s").
    WorkingRow,
    /// The "Worked for …" disclosure row of a completed turn.
    WorkedFor,
    /// The answer actions row (timestamp) under a completed answer.
    AnswerActions,
    /// The composer dock (card #16's `composer` component): draft, model, the
    /// running/queued states.
    Composer,
    /// The "New chat" affordance (card #16's `new-chat` component).
    NewChat,
}

impl ItemKind {
    /// Every item kind, in the order the tests enumerate them.
    pub const ALL: &'static [ItemKind] = &[
        ItemKind::ThreadRow,
        ItemKind::UserBubble,
        ItemKind::AssistantProse,
        ItemKind::ToolCell,
        ItemKind::WorkingRow,
        ItemKind::WorkedFor,
        ItemKind::AnswerActions,
        ItemKind::Composer,
        ItemKind::NewChat,
    ];

    /// The component id (the file stem under `design/components/`, #16's index
    /// key, and the string a swap names).
    pub fn id(self) -> &'static str {
        match self {
            ItemKind::ThreadRow => "thread-row",
            ItemKind::UserBubble => "user-bubble",
            ItemKind::AssistantProse => "assistant-prose",
            ItemKind::ToolCell => "tool-cell",
            ItemKind::WorkingRow => "working-row",
            ItemKind::WorkedFor => "worked-for",
            ItemKind::AnswerActions => "answer-actions",
            ItemKind::Composer => "composer",
            ItemKind::NewChat => "new-chat",
        }
    }

    /// Resolve a component id back to its kind (the on-disk → host direction).
    pub fn from_id(id: &str) -> Option<ItemKind> {
        ItemKind::ALL.iter().copied().find(|k| k.id() == id)
    }
}

/// One live value in a component: the `copy` id in the ledger, and the binding
/// id ([`crate::bindings`]) that fills it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// The `copy <id> { en: … }` entry the value is written into.
    pub copy: &'static str,
    /// The data binding id the value is read from (`bindings.rs`); empty =
    /// static copy (the ledger's own `en:` stays).
    pub binding: &'static str,
}

/// A component: id + kind + its ledger, its data, its kit dir, its bindings.
#[derive(Debug, Clone)]
pub struct Component {
    pub id: &'static str,
    pub kind: ItemKind,
    /// The ledger source (`.l0`), with the local component declarations and the
    /// `view root` tree.
    pub ledger: String,
    /// The ledger's data/placements (what `l0::prepare` needs to lay a leaf out).
    pub data: Value,
    /// The kit dir `l0::prepare` resolves `native/<mood>/kit.json` against.
    pub kit_dir: PathBuf,
    /// The declared live bindings (the coverage contract).
    pub bindings: &'static [Binding],
}

/// The shared placeholder kit's two component names (declared in
/// `design/components/native/light/kit.json`).
const P_SURFACE: &str = "PSurface";
const P_TEXT: &str = "PText";

/// One text leaf of a placeholder ledger: `instance` id, `copy` id, its `en:`
/// text and its authored `w`/`h` (leaf sizes only — never an artboard).
struct Leaf(&'static str, &'static str, &'static str, f64, f64);

/// Build a placeholder ledger: `theme light`, one `copy` per leaf, the two
/// local kit components, and a `view root` of `PText` leaves.
fn ledger(leaves: &[Leaf]) -> (String, Value) {
    let mut src = String::from(
        "// PLACEHOLDER component (card #17). Card #16 (design/components/) replaces\n\
         // this by id with no code change. Carries no artboard geometry.\n\
         # level: L0\n# profile: ui/l0\ntheme light\n",
    );
    for Leaf(_, copy_id, en, _, _) in leaves {
        src.push_str(&format!("copy {copy_id} {{ class: user-copy, en: \"{en}\" }}\n"));
    }
    src.push_str(&format!(
        "component {P_SURFACE}(instance: text) {{\n  view Kit(component: \"{P_SURFACE}\", instance: instance) {{ slot }}\n}}\n\
         component {P_TEXT}(instance: text, text: text) {{\n  view Kit(component: \"{P_TEXT}\", instance: instance, text: text)\n}}\n"
    ));
    src.push_str(&format!("view root {P_SURFACE}(instance: \"page\") {{\n"));
    for Leaf(instance, copy_id, _, _, _) in leaves {
        src.push_str(&format!("  {P_TEXT}(instance: \"{instance}\", text: copy.{copy_id})\n"));
    }
    src.push_str("}\n");

    let mut placements = Map::new();
    placements.insert(
        "page".into(),
        json!({"component": P_SURFACE, "layout": {"x": 0, "y": 0, "w": 360, "h": 40}}),
    );
    let mut y = 4.0;
    for Leaf(instance, _, _, w, h) in leaves {
        placements.insert(
            (*instance).into(),
            json!({"component": P_TEXT, "layout": {"x": 10, "y": y, "w": w, "h": h}}),
        );
        y += h + 2.0;
    }
    (src, json!({"$kit": {"theme": "light", "placements": placements}}))
}

/// The live slots of one component (card #21 §2): the `copy` id card #16's
/// ledger declares, and the binding id whose value fills it.
///
/// These are the REAL copy ids read off `design/components/<id>/page.card`
/// (`thread_1_label_text`, `t01_text`, `answer_md_text`, …), so both the
/// on-disk component and the placeholder fallback share one table: swapping in
/// #16's component is a file drop, not a Rust change.
///
/// A `binding` of `""` is a **static** slot (the ledger's own `en:` stays); the
/// sentinel [`CLEAR`] blanks the slot (the base ledgers ship a measured fixture
/// string that must not show through live data).
pub fn slots(kind: ItemKind) -> &'static [Binding] {
    match kind {
        ItemKind::ThreadRow => &[Binding { copy: "thread_1_label_text", binding: "threads[].title" }],
        // The "New chat" label is the component's own copy (`New chat`) — static.
        ItemKind::NewChat => &[Binding { copy: "new_chat_label_text", binding: "" }],
        // The bubble's two measured lines: the message goes in the first; the
        // second is a fixture line that must be cleared.
        ItemKind::UserBubble => &[
            Binding { copy: "t01_text", binding: "timeline.entries[].text" },
            Binding { copy: "t02_text", binding: CLEAR },
        ],
        ItemKind::AssistantProse => {
            &[Binding { copy: "answer_md_text", binding: "timeline.entries[].text" }]
        }
        ItemKind::ToolCell => &[
            Binding { copy: "t01_text", binding: "tools[].summary" },
            Binding { copy: "t02_text", binding: "tools[].status" },
        ],
        ItemKind::WorkingRow => &[Binding { copy: "t03_text", binding: "turn.activity" }],
        ItemKind::WorkedFor => {
            &[Binding { copy: "worked_row_label_text", binding: "answer.worked_for" }]
        }
        ItemKind::AnswerActions => &[Binding { copy: "t11_text", binding: "answer.timestamp" }],
        ItemKind::Composer => &[
            // #32h TOP: the live draft is NOT baked into the lowered DSL any
            // more — every keystroke used to change the DSL, miss the mount
            // cache (mount.rs:95 compares strings) and remount the whole
            // composer with a NEW TextInput, so the Android IME lost its
            // target after the first character (the device: val stuck at
            // "S"). The widget owns its text while focused; the store draft
            // reaches it only on a real external change (lib.rs
            // composer_synced). The placeholder stays authored.
            Binding { copy: "composer_idle_input_placeholder", binding: "composer.placeholder" },
            // A10 — the model seat names the selected configured model (the
            // web `ModelControl` trigger); the approval pill below carries
            // the permission read-back.
            Binding { copy: "t04_text", binding: "composer.model" },
            // #P4a1 — the approval pill carries the live permission mode
            // (read-back from permission/profile/set); the static art text
            // stays until the server has answered at least once.
            Binding { copy: "pill1_t_text", binding: "set.permission_mode" },
        ],
    }
}

/// The sentinel binding that blanks a slot (see [`slots`]).
pub const CLEAR: &str = "@clear";

/// The built-in placeholder component for `kind` (see the module docs). Its
/// `copy` ids are [`slots`]' own, so a placeholder renders the same slots the
/// real component does and the swap is invisible to the screen.
pub fn builtin(kind: ItemKind) -> Component {
    let leaves: Vec<Leaf> = slots(kind)
        .iter()
        .map(|b| Leaf(b.copy, b.copy, "", 300.0, 22.0))
        .collect();
    let (ledger, data) = ledger(&leaves);
    Component { id: kind.id(), kind, ledger, data, kit_dir: components_dir(), bindings: slots(kind) }
}

/// The component directory: `OCTOSCODE_COMPONENTS_DIR`, else `design/components`
/// under the CWD, else beside this crate (so a test run from anywhere finds a
/// dropped-in component set).
pub fn components_dir() -> PathBuf {
    components_dir_candidates()
        .into_iter()
        .find(|p| p.is_dir())
        .unwrap_or_else(|| crate::design::dir("components"))
}

/// The candidate roots for `design/components/`, most-specific first (card #21b).
///
/// The #21 captures painted placeholders because the *launched* app resolved a
/// directory that holds only the shared placeholder kit
/// (`design/components/native/light/kit.json`) and no per-id component dirs —
/// the tests passed only because they run from the repo root (RULES: a value
/// that only a test sees is worth nothing). These candidates make the running
/// app find the real components whatever its cwd:
/// 1. `OCTOSCODE_COMPONENTS_DIR` (the launcher sets it),
/// 2. `design/components` under the CWD (the authored / repo-root layout),
/// 3. the in-repo checkout layout (`crate::design::dir`),
/// 4. `design/components` walking up from the executable (the installed app).
pub fn components_dir_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(dir) = std::env::var("OCTOSCODE_COMPONENTS_DIR") {
        out.push(PathBuf::from(dir));
    }
    out.push(PathBuf::from("design/components"));
    out.push(crate::design::dir("components"));
    if let Ok(exe) = std::env::current_exe() {
        let mut p = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        for _ in 0..6 {
            out.push(p.join("design/components"));
            match p.parent() {
                Some(parent) => p = parent.to_path_buf(),
                None => break,
            }
        }
    }
    out
}

/// Log every component this process resolves at startup (card #21b step 1):
/// `id -> path -> on-disk|placeholder`, so a capture's log proves which root the
/// running app used. Returns the lines (also emitted through `log::info!`).
pub fn log_resolutions() -> Vec<String> {
    let dir = components_dir();
    let mut lines = vec![format!("[components] root = {}", dir.display())];
    for kind in ItemKind::ALL {
        let (c, on_disk) = resolve(*kind);
        let id = kind.id();
        let path = dir.join(id).join("page.card");
        lines.push(format!(
            "[components] {id} -> {} -> {}",
            path.display(),
            if on_disk { "on-disk" } else { "placeholder" }
        ));
        debug_assert!(c.id == id);
    }
    // Makepad's own `log!` (the shell's `wm:` lines reach `/log` this way); the
    // `log` crate has no logger installed in this runtime, so `log::info!` was
    // a silent no-op and the capture's `/log` showed nothing (card #21b step 1).
    for l in &lines {
        makepad_widgets::log!("{l}");
    }
    lines
}

/// The recorded asset-server base card #16's `page.data.json` files name
/// (`http://127.0.0.1:8170/ux-images/<id>/assets/<file>`).
pub const RECORDED_ASSET_BASE: &str = "http://127.0.0.1:8170/ux-images";

/// Rebase every SVG `src` in a component's data onto the asset server we
/// actually run (card #21 §4).
///
/// #16's committed data names the design-lab's ad-hoc asset port (`:8170`),
/// which is not ours to run during a headless capture. `OCTOSCODE_ASSET_BASE`
/// (e.g. `http://127.0.0.1:8180/ux-images`) repoints them without editing #16's
/// committed files — the JSON is rewritten in memory, before lowering.
fn rebase_assets(data: &mut Value) {
    let Ok(base) = std::env::var("OCTOSCODE_ASSET_BASE") else {
        return;
    };
    let base = base.trim_end_matches('/').to_owned();
    fn visit(n: &mut Value, from: &str, to: &str) {
        match n {
            Value::Object(map) => {
                if let Some(Value::String(src)) = map.get_mut("src") {
                    if let Some(rest) = src.strip_prefix(from) {
                        *src = format!("{to}{rest}");
                    }
                }
                for (_, v) in map.iter_mut() {
                    visit(v, from, to);
                }
            }
            Value::Array(items) => {
                for v in items.iter_mut() {
                    visit(v, from, to);
                }
            }
            _ => {}
        }
    }
    visit(data, RECORDED_ASSET_BASE, &base);
}

/// Resolve the component for `kind`, preferring card #16's on-disk component
/// over the placeholder. The bool reports whether the on-disk one was used.
///
/// Card #16 ships each component as a **directory** — `design/components/<id>/`
/// with `page.card` (the ledger), `page.data.json` (its placements) and its own
/// `kit/native/light/kit.json` pack (the one the renderer used). A dropped-in
/// component may also ship a flat `<id>.l0` + `<id>.json`; both shapes are
/// accepted, the directory first (that is what #16 writes).
pub fn resolve(kind: ItemKind) -> (Component, bool) {
    let dir = components_dir();
    let own = dir.join(kind.id());
    // (ledger, data) for the on-disk component, in preference order.
    let sources = [
        // #16's shape: a per-component directory with its own kit pack.
        (own.join("page.card"), Some(own.join("page.data.json")), Some(own.clone())),
        // The dropped-in shape a swap may use: a flat file beside the dir.
        (dir.join(format!("{}.l0", kind.id())), Some(dir.join(format!("{}.json", kind.id()))), Some(own)),
    ];
    for (ledger_path, data_path, kit_candidate) in sources {
        let Ok(ledger) = std::fs::read_to_string(&ledger_path) else {
            continue;
        };
        let data = data_path
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or_else(|| json!({}));
        let mut data = data;
        rebase_assets(&mut data);
        // A component that ships its own kit pack lowers against that pack.
        // #16 writes it at `<id>/kit/native/<mood>/kit.json` (the kit's parent
        // is the dir `l0::prepare` appends `native/<mood>/kit.json` to); a flat
        // drop-in may instead put it at `<id>/native/<mood>/kit.json`.
        let kit_dir = match kit_candidate {
            Some(c) => {
                let nested = c.join("kit");
                if nested.join("native/light/kit.json").is_file() {
                    nested
                } else if c.join("native/light/kit.json").is_file() {
                    c
                } else {
                    dir.clone()
                }
            }
            None => dir.clone(),
        };
        let base = builtin(kind);
        return (Component { ledger, data, kit_dir, ..base }, true);
    }
    (builtin(kind), false)
}

/// The live `(copy_id, value)` pairs for one item of `kind` at `index`.
///
/// This is the **per-item binding** the card asks for: values come from
/// [`bindings::query`] (the store / the flow), keyed by the declared binding id,
/// projected onto the item's index. `Err` names a binding that does not resolve.
pub fn item_copies(
    kind: ItemKind,
    ctx: &bindings::Ctx<'_>,
    index: usize,
    turn: Option<&str>,
) -> Result<Vec<(String, String)>, String> {
    let get = |id: &str| bindings::query(ctx, id).ok_or_else(|| format!("binding {id:?} is not declared"));
    let arr = |id: &str| -> Result<Vec<Value>, String> { Ok(get(id)?.as_array().cloned().unwrap_or_default()) };
    let row = |id: &str| -> Result<Value, String> { Ok(arr(id)?.get(index).cloned().unwrap_or(Value::Null)) };
    let text = |v: &Value| -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        }
    };

    let mut out = Vec::new();
    for b in slots(kind) {
        if b.binding == CLEAR {
            // A measured fixture string that must not show through live data.
            out.push((b.copy.to_owned(), String::new()));
            continue;
        }
        if b.binding.is_empty() {
            continue; // static copy (the ledger's own en: stays)
        }
        let value = match b.binding {
            // ---- thread list ------------------------------------------------
            // A session may carry no title (the opened one often doesn't); fall
            // back to its id, the same way the row model does (`screen::thread_rows`).
            "threads[].title" => {
                let r = row("threads")?;
                let title = text(&r.get("title").cloned().unwrap_or(Value::Null));
                if title.is_empty() {
                    text(&r.get("id").cloned().unwrap_or(Value::Null))
                } else {
                    title
                }
            }
            "threads[].id" => text(&row("threads")?.get("id").cloned().unwrap_or(Value::Null)),
            // ---- timeline ---------------------------------------------------
            "timeline.entries[].text" => {
                text(&row("timeline.entries")?.get("text").cloned().unwrap_or(Value::Null))
            }
            // ---- tool cells -------------------------------------------------
            "tools[].summary" | "tools[].name" => {
                text(&row("tools")?.get("summary").cloned().unwrap_or(Value::Null))
            }
            "tools[].status" => text(&row("tools")?.get("status").cloned().unwrap_or(Value::Null)),
            "tools[].detail" => text(&row("tools")?.get("summary").cloned().unwrap_or(Value::Null)),
            // ---- composer pills ---------------------------------------------
            // #P4a1 — the approval pill carries the live mode when the
            // server has one; empty (never read) keeps the authored static
            // copy ("Ask for approval", the web's on-request label).
            "set.permission_mode" => {
                let v = text(&get("set.permission_mode")?);
                if v.is_empty() {
                    continue;
                }
                v
            }
            // ---- turn / answer ---------------------------------------------
            "turn.activity" => text(&get("turn.activity")?),
            // **Card #21j**: the settled tail renders from ITS OWN turn's
            // terminal, so a later turn cannot relabel an earlier row.
            "answer.worked_for" => {
                let v = ctx
                    .ui
                    .lock()
                    .unwrap()
                    .worked_for_for(turn);
                text(&Value::String(v))
            }
            "answer.timestamp" => {
                let v = ctx
                    .ui
                    .lock()
                    .unwrap()
                    .answer_timestamp_for(turn);
                text(&Value::String(v))
            }
            // ---- composer ---------------------------------------------------
            "composer.draft" => text(&get("composer.draft")?),
            "composer.placeholder" => text(&get("composer.placeholder")?),
            // A10 — the model seat's label (the selected configured model).
            "composer.model" => text(&get("composer.model")?),
            other => return Err(format!("{} has no arm for binding {other:?}", kind.id())),
        };
        out.push((b.copy.to_owned(), value));
    }
    // A1: the fluid rows' extra live facts ride the same copies list, so the
    // lowering cache key (`screen::Cache`, keyed on the copies) changes
    // exactly when the drawn row does.
    match kind {
        ItemKind::ToolCell => {
            let (view, pos, open, _) = tool_view(ctx, index, turn);
            out.push((TOOL_TITLE.to_owned(), view.title));
            out.push((TOOL_TARGET.to_owned(), view.target));
            out.push((TOOL_STATE.to_owned(), view.state));
            out.push((TOOL_SECS.to_owned(), view.secs.map(|s| s.to_string()).unwrap_or_default()));
            out.push((TOOL_POS.to_owned(), pos.id().to_owned()));
            out.push((TOOL_OPEN.to_owned(), if open { "1" } else { "0" }.to_owned()));
            if open {
                out.push((TOOL_OUTPUT.to_owned(), view.output));
            }
        }
        ItemKind::WorkedFor => {
            let n = turn_tool_count(ctx, turn);
            let folded = turn.is_some_and(|t| ctx.ui.lock().unwrap().is_turn_folded(t));
            out.push((WORKED_TOOLS.to_owned(), n.to_string()));
            out.push((WORKED_OPEN.to_owned(), if folded { "0" } else { "1" }.to_owned()));
        }
        _ => {}
    }
    Ok(out)
}

/// A1 — three tool calls on `turn` (list the root, list `.octos`, read the
/// workspace file), written the way the live client writes them: a
/// TOOL_CALL timeline entry carrying the call id, and the tool domain's
/// start/end records. `live` leaves the last call running and marks the
/// session's turn live (the running-turn capture). A capture seed
/// (`OCTOSCODE_SYNTHETIC_TOOLS`), never called on the live path.
pub fn seed_tool_calls(store: &octoscode_store::Store, session: &str, turn: &str, live: bool) {
    let calls = [
        ("list_dir", "path: \".\"", "2 entries in .:\n[dir]  .octos\n[file] .octos-workspace.toml"),
        ("list_dir", "path: \".octos\"", "2 entries in .octos:\n[dir]  dsflash\n[file] active-profile"),
        ("read_file", "path: \".octos-workspace.toml\"", "[workspace]\nkind = \"session\""),
    ];
    let last = calls.len() - 1;
    for (k, (name, args, out)) in calls.into_iter().enumerate() {
        let id = format!("seed-{turn}-{k}");
        store.domains.session.timeline.append_data(
            session,
            Some(turn.to_owned()),
            octoscode_store::EntryKind::TOOL_CALL,
            name.to_owned(),
            serde_json::json!({ "tool_call_id": id }),
        );
        store.domains.tool.call_started(&id, name, Some(args));
        if !(live && k == last) {
            store.domains.tool.call_ended(&id, "complete", Some(out), Some(120));
        }
    }
    if live {
        store.domains.turn.started(turn);
        let mut sessions = store.sessions();
        for s in &mut sessions {
            if s.id == session {
                s.active_turn = true;
            }
        }
        store.set_sessions(sessions);
    }
}

/// A1 — a settled Chinese turn after the fixture's first (the CJK capture
/// seed, `OCTOSCODE_SYNTHETIC_TOOLS=zh`): the prompt and the answer a live
/// dsflash turn returned to it, with its three tool calls.
pub fn seed_zh_turn(store: &octoscode_store::Store, session: &str, turn: &str) {
    let prompt = "请用中文回答。每次只调用一个工具：先列出当前目录，再列出 .octos，最后读取 \
        .octos-workspace.toml。然后给出一个小标题、三个要点（路径用行内代码），最后用 toml \
        代码块引用 [workspace] 段。";
    let answer = "## 工作区结构一览\n\n\
        - 根目录 `.` 只有两项：隐藏目录 `.octos` 和配置文件 `.octos-workspace.toml`。\n\
        - `.octos` 内含子目录 `dsflash` 和文件 `active-profile`，后者记录当前生效的档案名。\n\
        - `.octos-workspace.toml` 共 217 行，开头声明 `schema_version = 1`，并把工作区类型定为会话级。\n\n\
        ```toml\n[workspace]\nkind = \"session\"\n```";
    let tl = &store.domains.session.timeline;
    tl.upsert_user_message(session, turn, prompt, serde_json::json!({}));
    seed_tool_calls(store, session, turn, false);
    tl.append(session, Some(turn.to_owned()), octoscode_store::EntryKind::ASSISTANT_TEXT, answer.to_owned());
    tl.finalize_assistant(session, turn, answer);
    tl.close_turn(session, turn);
    store.domains.turn.started(turn);
    store.domains.turn.set_terminal(turn, "completed");
}

/// A1 — the GFM sample the web's `MarkdownBody` test renders (heading,
/// strong, a table, an ordered list) plus raw HTML that must stay inert
/// (`OCTOSCODE_SYNTHETIC_TOOLS=gfm`, a settled turn after the fixture's).
pub const GFM_SAMPLE: &str = "# Workspace summary\n\n\
    The root holds **two** entries:\n\n\
    | Path | Kind |\n\
    |---|---|\n\
    | `.octos` | directory |\n\
    | `.octos-workspace.toml` | file |\n\n\
    1. `.octos/dsflash` keeps the session state.\n\
    2. `.octos/active-profile` names the profile.\n\n\
    <script>alert(\"raw HTML\")</script>\n\n\
    Inline <b>tags</b> stay plain text.";

/// A1 — a settled turn whose answer is [`GFM_SAMPLE`] (the markdown
/// capture seed), with no tool calls.
pub fn seed_gfm_turn(store: &octoscode_store::Store, session: &str, turn: &str) {
    let tl = &store.domains.session.timeline;
    tl.upsert_user_message(session, turn, "Summarize the workspace as a table.", serde_json::json!({}));
    tl.append(session, Some(turn.to_owned()), octoscode_store::EntryKind::ASSISTANT_TEXT, GFM_SAMPLE.to_owned());
    tl.finalize_assistant(session, turn, GFM_SAMPLE);
    tl.close_turn(session, turn);
    store.domains.turn.started(turn);
    store.domains.turn.set_terminal(turn, "completed");
}

/// A1 pseudo-copies: live facts the fluid rows draw that no authored `copy`
/// slot carries (the cache keys on them like on any copy).
pub const TOOL_TITLE: &str = "@tool.title";
pub const TOOL_TARGET: &str = "@tool.target";
pub const TOOL_STATE: &str = "@tool.state";
pub const TOOL_SECS: &str = "@tool.secs";
pub const TOOL_POS: &str = "@tool.pos";
pub const TOOL_OPEN: &str = "@tool.open";
pub const TOOL_OUTPUT: &str = "@tool.output";
pub const WORKED_TOOLS: &str = "@worked.tools";
pub const WORKED_OPEN: &str = "@worked.open";

/// The TOOL_CALL timeline entries of `turn` (every one in the active session
/// for `None`), in order.
fn turn_tool_entries(ctx: &bindings::Ctx<'_>, turn: Option<&str>) -> Vec<octoscode_store::timeline::TimelineEntry> {
    let Some(session) = ctx.store.active_session() else {
        return Vec::new();
    };
    ctx.store
        .domains
        .session
        .timeline
        .entries(&session)
        .into_iter()
        .filter(|e| e.kind == octoscode_store::EntryKind::TOOL_CALL)
        .filter(|e| turn.is_none() || e.turn_id.as_deref() == turn)
        .collect()
}

/// How many tool calls `turn` made (the worked-for header's count).
pub fn turn_tool_count(ctx: &bindings::Ctx<'_>, turn: Option<&str>) -> usize {
    turn_tool_entries(ctx, turn).len()
}

/// The disclosure key of a tool row: its call id, else `turn:ordinal`.
pub fn tool_key(ctx: &bindings::Ctx<'_>, index: usize, turn: Option<&str>) -> String {
    tool_view(ctx, index, turn).3
}

/// A1 — the `index`-th tool call OF ITS OWN TURN, resolved per turn.
///
/// Before A1 a tool row read `tools[index]` from the flow's GLOBAL list, so
/// turn 2's first call showed turn 1's first call. The row now starts from
/// its turn's own TOOL_CALL entry (`turn.rs` appends one per `tool_start`,
/// carrying the `tool_call_id`) and folds the store's call record (name,
/// arguments preview, terminal status, duration, output preview), with the
/// flow's row as the fallback for a call the store never saw.
///
/// Returns the view, the row's place in its turn's card, whether the person
/// disclosed it, and its disclosure key.
pub fn tool_view(
    ctx: &bindings::Ctx<'_>,
    index: usize,
    turn: Option<&str>,
) -> (crate::fluid::ToolView, crate::fluid::GroupPos, bool, String) {
    let entries = turn_tool_entries(ctx, turn);
    let pos = crate::fluid::GroupPos::of(index, entries.len());
    let entry = entries.get(index);
    let call_id = entry.and_then(|e| {
        e.data
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
    });
    let record = call_id.as_ref().and_then(|id| {
        ctx.store
            .domains
            .tool
            .calls()
            .into_iter()
            .find(|c| &c.tool_call_id == id)
    });
    let (flow_row, open_key_open) = {
        let ui = ctx.ui.lock().unwrap();
        let row = match &call_id {
            Some(id) => ui.tools().into_iter().find(|t| &t.tool_call_id == id),
            // No call id (a hand-built or hydrated entry): the flow's own
            // ordinal, the pre-A1 projection, only when no turn scopes it.
            None if turn.is_none() => ui.tools().get(index).cloned(),
            None => None,
        };
        let key = call_id
            .clone()
            .unwrap_or_else(|| format!("{}:{index}", turn.unwrap_or("")));
        let open = ui.is_expanded(&key);
        (row, (key, open))
    };
    let (key, open) = open_key_open;
    let title = record
        .as_ref()
        .map(|r| r.name.clone())
        .filter(|n| !n.is_empty() && Some(n) != call_id.as_ref())
        .or_else(|| entry.map(|e| e.text.clone()).filter(|t| !t.is_empty()))
        .or_else(|| flow_row.as_ref().map(|t| t.name.clone()))
        .unwrap_or_default();
    // The web's `toolTarget` reads the call's arguments JSON.
    let target = match record.as_ref().and_then(|r| r.arguments_preview.clone()) {
        Some(args) => crate::fluid::preview_target(&args),
        None => entry
            .and_then(|e| e.data.as_object())
            .map(crate::fluid::target_of)
            .unwrap_or_default(),
    };
    let turn_settled = turn.is_some_and(|t| ctx.store.domains.turn.terminal(t).is_some());
    let mut state = record
        .as_ref()
        .map(|r| r.status.clone())
        .or_else(|| flow_row.as_ref().map(|t| t.status.clone()))
        .or_else(|| {
            entry
                .and_then(|e| e.data.get("status"))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .unwrap_or_default();
    // A call still "running" when its turn already settled never reported its
    // end: the web labels it by the turn's outcome (model.ts:722-733).
    if turn_settled && (state.is_empty() || state == "running") {
        state = "finished".to_owned();
    }
    if state.is_empty() {
        state = "running".to_owned();
    }
    let secs = record
        .as_ref()
        .and_then(|r| r.duration_ms)
        .map(|ms| (ms + 500) / 1000);
    let output = record
        .as_ref()
        .and_then(|r| r.output_preview.clone())
        .unwrap_or_default();
    (
        crate::fluid::ToolView { title, target, state, secs, output },
        pos,
        open,
        key,
    )
}

/// A1 — lower one conversation row as a FLUID layout ([`crate::fluid`]) from
/// its copies, or `None` for the kinds that keep the authored artboard (the
/// sidebar's `thread-row` / `new-chat`, owned by the sidebar card).
fn lower_fluid(kind: ItemKind, token: &str, copies: &[(String, String)]) -> Option<String> {
    let m = crate::conv_layout::current();
    let get = |id: &str| -> String {
        copies
            .iter()
            .find(|(c, _)| c == id)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let dark = crate::screens::theme::resolved() == "dark";
    let ui = match kind {
        ItemKind::UserBubble => crate::fluid::user_bubble(token, &get("t01_text"), &m, dark),
        ItemKind::AssistantProse => crate::fluid::assistant_prose(token, &get("answer_md_text"), &m),
        ItemKind::ToolCell => {
            let view = crate::fluid::ToolView {
                title: get(TOOL_TITLE),
                target: get(TOOL_TARGET),
                state: get(TOOL_STATE),
                secs: get(TOOL_SECS).parse().ok(),
                output: get(TOOL_OUTPUT),
            };
            let pos = crate::fluid::GroupPos::from_id(&get(TOOL_POS));
            crate::fluid::tool_row(token, &view, pos, get(TOOL_OPEN) == "1", &m)
        }
        ItemKind::WorkingRow => {
            let text = get("t03_text");
            let text = if text.is_empty() { "Working…".to_owned() } else { text };
            crate::fluid::working_row(token, &text, &m)
        }
        ItemKind::WorkedFor => crate::fluid::worked_for(
            token,
            &get("worked_row_label_text"),
            get(WORKED_TOOLS).parse().unwrap_or(0),
            get(WORKED_OPEN) != "0",
            &m,
        ),
        ItemKind::AnswerActions => crate::fluid::answer_actions(token, &get("t11_text"), &m),
        ItemKind::Composer => {
            let placeholder = get("composer_idle_input_placeholder");
            let approval = get("pill1_t_text");
            crate::fluid::composer(
                &crate::fluid::ComposerView {
                    placeholder: if placeholder.is_empty() {
                        "Ask Octos anything".to_owned()
                    } else {
                        placeholder
                    },
                    approval: if approval.is_empty() {
                        "Ask for approval".to_owned()
                    } else {
                        approval
                    },
                    // A10 — the model seat's label (`composer.model`).
                    model: {
                        let m = get("t04_text");
                        if m.is_empty() {
                            crate::screens::board3::seats::MODEL_SELECT.to_owned()
                        } else {
                            m
                        }
                    },
                },
                &m,
            )
        }
        ItemKind::ThreadRow | ItemKind::NewChat => return None,
    };
    Some(crate::screens::theme::retint_dsl(&ui))
}

/// Lower one component with its live copies injected — the SAME chain a card
/// uses ([`l0_host`]): rewrite the ledger's `copy` entries, `l0::prepare`, then
/// `design::to_makepad_ui` to the Splash DSL.
///
/// `token` disambiguates the node-id prefix: the module gives every instantiated
/// item a unique token (its virtual index), so two instances of the same
/// component in one isolate never re-bind each other's ids (the #15b lesson).
pub fn lower(kind: ItemKind, token: &str, copies: &[(String, String)]) -> Result<String, String> {
    l0_host::register_vocabulary();
    // A1: the conversation rows are fluid (kit tokens, no artboard widths).
    if let Some(ui) = lower_fluid(kind, token, copies) {
        return Ok(ui);
    }
    lower_artboard(kind, token, copies)
}

/// The authored-artboard lowering (the L0 chain + the #21x post-processes),
/// kept for the sidebar's `thread-row` / `new-chat` — and, for the record of
/// what the conversation kinds lowered to before A1, still callable for them.
pub fn lower_artboard(kind: ItemKind, token: &str, copies: &[(String, String)]) -> Result<String, String> {
    l0_host::register_vocabulary();
    let component = resolve(kind).0;
    let mut src = component.ledger.clone();
    for (copy_id, value) in copies {
        if let Some(next) = l0_host::set_copy(&src, copy_id, value) {
            src = next;
        }
    }
    let prepared = octoscript_makepad::l0::prepare(&src, &component.data, &component.kit_dir)?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    // Card #21b: a component mounted into a SLOT must be laid out relative to its
    // parent. `to_makepad_ui` positions the tree with the card's own artboard
    // `abs_pos`, which makepad applies at the WINDOW origin — right for the
    // Gate-B renders (the card IS the window) but wrong for an item at (300,219),
    // whose nodes would pin to (0,0) and be clipped away by the slot.
    let ui = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui_in_slot(&tree))?;
    let ui = ui.replace("beauty_0", &format!("i{token}_{}", kind.id().replace('-', "")));
    // Card #21c item 3: content-driven height. Every node the component emits
    // carries the MEASURED artboard height of the fixture it was compiled from
    // (the prose root is `height: 492`, the bubble `85.09`), so a one-word live
    // answer still occupied 492 px — clipping long prose and pushing short rows
    // off the timeline. For the TEXT-FLOW kinds every emitted `height: <n>`
    // becomes `Fit`, so each node takes its content's height. Chrome kinds
    // (buttons, the composer dock) keep their measured box.
    let ui = match kind {
        // #36f: in dark resolution the components' light-artboard literals
        // become the theme roles the shell assigns (#32f pattern) — the
        // measured bubble was surface #f5f5f7 with #fafbfb text (1.05:1,
        // invisible) and the composer's pill labels were #434343/#252525 on
        // the #1c1f22 surface (1.00:1). Light stays byte-identical (the
        // replay tests pin the lowered string).
        ItemKind::UserBubble => bubble_live_layout(&ui),
        // Card #21e item 3: the send control is a flat black disc. #36f
        // item 3: its artboard margin (left 328 on the 374 board) put the
        // disc 11px past the mounted 333 column (device /snap: composer_5
        // [339,612,5,38] vs column right 344) — re-anchored to 287 in
        // flat_send_button. Dark is retint_dsl at the tail of lower().
        ItemKind::Composer => flat_send_button(&ui),
        // Card #21d item 5: a session title is live and unbounded, so the row's
        // single-line label must ELLIPSIZE rather than hard-clip ("…print").
        ItemKind::ThreadRow => ellipsize_single_line(&ui),
        // Card #21d item 5: scene 01's new-chat row carries a compose icon at
        // its right edge (beauty-host renders it too: 446 ink px in the
        // component's own right 20%). The #16 ledger omits the node, so add it.
        // #32h B: the measured label ink is near-black (#070606ff) — invisible
        // on the dark shell. Bind the fg to the THEME ROLE the shell already
        // assigns per resolved mode (lib.rs script_mod references
        // theme.color_fg_app; theme.rs fills it), the #32f title pattern.
        ItemKind::NewChat => new_chat_with_compose_icon(
            &ui.replace(
                "draw_text.color: #070606ff",
                "draw_text.color: theme.color_fg_app",
            ),
        ),
        // Card #21e item 5: the timestamp sits at the artboard's own x=243.64 in a
        // 364px row, which lands mid-column once mounted in a wider slot.
        ItemKind::AnswerActions => right_align_timestamp(&answer_actions_ink(&ui)),
        // Card #21e item 6: fenced code must not soft-wrap.
        ItemKind::AssistantProse => reachable_code(&fit_heights(&ui)),
        // Card #21e item 8: the activity row's spinner keeps its 18px box.
        ItemKind::WorkingRow => working_row_layout(&ui),
        ItemKind::WorkedFor => worked_for_style(&ui),
        ItemKind::ToolCell => fit_heights(&ui),
    };
    // Card #21d item 6: resolve every emitted `http_resource(…)` icon to the
    // component's own file on disk, so the app needs no dev asset server.
    let ui = localize_asset_resources(&ui);
    // #36f item 2: component svgs lower as ABSOLUTE BUILD-MACHINE paths
    // (file_resource("<build-machine>/…/components/<id>/assets/icon_….svg"));
    // the phone has no such path, so the answer-actions icons draw NOTHING
    // on the device (the #32g font-path lesson, svg edition). Re-point them
    // through the materialized-root resolver.
    let ui = localize_component_icons(&ui);
    // #31d workflow 1: EVERY mounted component reads the app-wide token set —
    // `retint_dsl` is the byte passthrough in light (no repaint churn) and the
    // token rewrite in dark.
    let ui = crate::screens::theme::retint_dsl(&ui);
    Ok(ui)
}

/// Card #21d item 3 — the person's bubble must hug its text up to ~80% of the
/// column, WRAP a long line, and grow in height.
///
/// The component's artboard fixes the bubble at 284 px and each label at
/// `flow: Right` (no wrap), so a live message longer than the two-line fixture
/// hard-clips ("One wo…"). Let the boxes hug (`width: Fit`) and the labels wrap,
/// and cap the whole bubble at 80% of the column. The `user_align` wrapper
/// right-aligns it (card #21c item 4).
/// #36f item 1 — the bubble's dark SURFACE, by role. The live card carries
/// the same literal for its surface and its text (#fafbfb; the repo copy
/// carries #f5f5f7/#fafbfb), so a retint TABLE entry cannot serve both:
/// #f5f5f7 is also the dark INK token and a key there eats the correct
/// light text on every dark line (measured: the double-retint cut left the
/// text #2c2c2e on #2c2c2e). Only `draw_bg.color:` lines are the surface.
fn bubble_dark_surface(ui: &str) -> String {
    ui.lines()
        .map(|l| {
            if l.contains("draw_bg.color:") {
                pin_opaque_hex(l, "2c2c2e")
            } else if l.contains("draw_text.color:") {
                pin_opaque_hex(l, "f5f5f7")
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Overwrite the FIRST color run after '#' with `rgb` (keeping the 6/8-digit
/// shape); translucent fills (alpha != ff) are left untouched so they keep
/// compositing — the same rule retint_dsl applies. Value-INDEPENDENT by
/// design: the bubble's literals live in kit references that move under us
/// (the shared materialized root was touched mid-card), so matching exact
/// values cannot be stable; the ROLE is stable (a draw_bg line on the user
/// bubble is its raised fill, a draw_text line is the message ink).
fn pin_opaque_hex(line: &str, rgb: &str) -> String {
    let bytes = line.as_bytes();
    let Some(hash) = line.find('#') else { return line.to_owned() };
    let mut n = 0usize;
    while n < 8 && hash + 1 + n < bytes.len() && bytes[hash + 1 + n].is_ascii_hexdigit() {
        n += 1;
    }
    if n != 6 && n != 8 {
        return line.to_owned();
    }
    if n == 8 && &bytes[hash + 7..hash + 9] != b"ff" {
        return line.to_owned(); // translucent: keep compositing
    }
    let mut out = String::with_capacity(line.len());
    out.push_str(&line[..hash + 1]);
    out.push_str(rgb);
    if n == 8 {
        out.push_str("ff");
    }
    out
}

fn bubble_live_layout(ui: &str) -> String {
    let ui = if crate::screens::theme::resolved() == "dark" {
        bubble_dark_surface(ui)
    } else {
        ui.to_owned()
    };
    // Heights hug (card #21c); the ROOT also hugs, capped at 80% of the column,
    // so a long message widens to the cap then wraps. Inner boxes keep the
    // measured widths the artboard gave them (`Fill` under a `Fit` root collapses
    // to the minimum — measured: a 16px-wide bubble), and the labels WRAP.
    let s = fit_heights(&ui);
    // Card #21e item 7: symmetric vertical padding (the artboard's absolute tops
    // leave the last wrapped line on the bottom edge).
    let s = symmetric_bubble_padding(&s);
    let s = set_first_width_fit_capped(&s, "80%");
    let mut s = s.replace("flow: Right\n", "flow: Right{wrap: true}\n");
    s = s.replace("flow: Right ", "flow: Right{wrap: true} ");
    // #32b2 item 2: the wrapper insets the bubble from the column's right
    // edge — the PortalList's scrollbar (`bar_size: 10, bar_side_margin: 3`,
    // scroll_bar.rs:25-27 ≈ a 13px handle) draws over the list's last pixels,
    // and a Fill wrapper put the bubble's right edge under it (the same
    // inset the #21e timestamp took).
    format!("user_align := View{{width:Fill height:Fit flow:Down align: Align{{x: 1.0}} padding: Inset{{right: 20}} {s}}}")
}

/// Card #21e item 8 — the activity row's spinner keeps its own 18px box.
///
/// The atlas's `working-row` is a 29.5px row whose spinner is 18×18 (scene-03
/// `icon_spinner` at 198..216). `fit_heights` leaves the `Svg` at `height: Fit`,
/// and its `preserve_aspect: false` then stretches the 24×24 viewBox to the
/// label's 24px line box (measured live: the icon occupied y211..235 in a
/// 206..235 row), so the glyph drew past its own box. Pin both the wrapper and
/// the `Svg` to the measured 18px.
fn working_row_layout(ui: &str) -> String {
    let s = fit_heights(ui);
    let s = s.replace(
        "View {width: 18 height: Fit margin: Inset{left: 0 top: 5",
        "View {width: 18 height: 18 margin: Inset{left: 0 top: 5",
    );
    s.replace("width: 18 height: Fit", "width: 18 height: 18")
}

/// Card #21e item 1 — a settled turn whose terminal was `interrupted` shows the
/// marker (`answer.worked_for`); the `working-row` component itself is only ever
/// the LIVE tail (`screen.rs:113-116`), so it keeps its spinner.
///
/// Card #21e item 5 — the answer-actions timestamp sits mid-column.
///
/// #36g item 1 — the answer-actions' svg ink follows the theme. The
/// artboard's svgs carry a FIXED mid-grey stroke (#6E6E73): readable on
/// light, near-invisible on the dark shell (the 6T: no icons in dark).
/// `DrawSvg.color` REPLACES the geometry's color when set (draw/src/shader/
/// draw_svg.rs get_color: color.rgb*color.a*base.a; the (-1,-1,-1,-1)
/// sentinel passes through) — pin it per resolved mode: dark ink on light,
/// light ink on dark.
fn answer_actions_ink(ui: &str) -> String {
    let ink = if crate::screens::theme::resolved() == "dark" {
        "#f5f5f7ff"
    } else {
        "#1c1f22ff"
    };
    ui.lines()
        .map(|l| {
            if l.contains("draw_svg.svg:") && !l.contains("draw_svg.color:") {
                format!("{l} draw_svg.color: {ink}")
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The artboard places the timestamp flush with its row's right edge
/// (`answer-actions/mapped.json`: t11 x=267.64 w=121.5 → right 389.14, row right
/// 388.11). The app's row is only 364px inside a 404px column, so even at its
/// authored x the label stops ~40px short of the column edge, and a short
/// timestamp (`now`) reads mid-column. Widen the row to its slot, seat the
/// label's wrapper at the column's right (a 16px inset, matching the atlas's own
/// gap), and right-align the run — an `align` on a child of an Overlay is a
/// no-op (`design.rs:164-174`), the wrapper must be the filled, aligned one.
fn right_align_timestamp(ui: &str) -> String {
    let s = fit_heights(ui);
    // 1. the row spans its slot (the artboard's 364px left a gutter).
    let s = s.replace("width: 364.11", "width: Fill");
    // 2. both the label wrapper and the label itself become Fill so the run can
    //    reach the column edge. The wrapper keeps a right inset that clears the
    //    PortalList's scrollbar: `ScrollBar { bar_size: 10, bar_side_margin: 3 }`
    //    (scroll_bar.rs:25-27) draws a ~13px handle over the list's last pixels,
    //    and the #21e flush-right push put `now`/`Sep 29, 8:17 AM` under it.
    //    The emitted wrapper margin is the artboard's own `left: 243.64`
    //    (NOT the `268` an earlier revision anchored on, which never matched) —
    //    anchor on the real text.
    let s = s.replace("width: 121.5", "width: Fill");
    let s = s.replace(
        "margin: Inset{left: 243.64 top: 1.5 right: 0 bottom: 0}",
        "margin: Inset{left: 0 top: 1.5 right: 20 bottom: 0}",
    );
    // 3. right-align the label's own text run.
    s.replace("align: Align{x: 0 y: 0.5}", "align: Align{x: 1.0 y: 0.5}")
}

/// Card #21e item 6 — a fenced code block must not soft-wrap its lines.
///
/// `Markdown` folds fenced code into the body text flow
/// (`widgets/src/markdown.rs:219` `use_code_block_widget` defaults false), whose
/// `code_layout.flow` is `Flow.Right{wrap: true}` (`text_flow.rs:192-196`). Card
/// #21e pinned that to a non-wrapping `flow: Right`, which HARD-CLIPS an
/// over-long line at the block's edge (`g4-interrupted.png`: `let y = x + 10; //
/// panic in debug,…` cut off). Card #21g item 2 restores wrapping — the board's
/// accepted alternative to horizontal scroll — so every glyph is painted over
/// the wrapped rows and the full line is reachable.
fn reachable_code(ui: &str) -> String {
    // Undo the #21e pin (the exact string it inserted), returning the block to
    // the theme's wrapping code layout.
    ui.replace(
        " := Markdown {\ncode_layout: Layout{flow: Right}",
        " := Markdown {",
    )
}

/// Card #21e item 7 — the person's bubble needs symmetric vertical padding.
///
/// The lowered bubble is an `Overlay` whose `Fit` height is the tallest child's
/// bottom edge; the two label wrappers carry the artboard's absolute tops
/// (11.09 / 48.59), so once `t01` wraps past the second wrapper the last line
/// lands exactly on the bottom edge (measured: label bottom == bubble bottom).
/// Stack the wrappers in a `Down` flow inside symmetric padding instead.
fn symmetric_bubble_padding(ui: &str) -> String {
    let s = ui.replacen(
        "flow: Overlay padding: 0 clip_x: false clip_y: false",
        "flow: Down padding: Inset{left: 15.72 top: 12 right: 15.72 bottom: 12} clip_x: false clip_y: false",
        1,
    );
    let s = s.replace(
        "margin: Inset{left: 15.72 top: 11.090000000000003 right: 0 bottom: 0}",
        "margin: 0",
    );
    // The cleared second line is bound to `@clear` for live data — the whole
    // message rides `t01` (which wraps). An empty `Label` still reserves its
    // line box, which read as ~29px of dead black under the last text line
    // (`g3-completed.png`), so collapse the label and let its `Fit` wrapper
    // shrink to zero with it.
    s.replace(
        "margin: Inset{left: 15.790000000000006 top: 48.59 right: 0 bottom: 0}",
        "margin: 0",
    )
    .replace(
        "i0_userbubble_1 := Label {\nwidth: 224.5 height: Fit",
        "i0_userbubble_1 := Label {\nwidth: 224.5 height: 0",
    )
}

/// Card #21e item 3 — the send control is a flat black disc, never a gloss.
///
/// The shell/kit surface skin fills with a second stop and a bevel on top of
/// `color` (the #21d item-1 lesson), which reads as a glossy radial gradient
/// around the white arrow. Force one flat black fill and no second stop/bevel.
fn flat_send_button(ui: &str) -> String {
    let s = ui.replace(
        "draw_bg.radius: 18 draw_bg.ellipse: 0 draw_bg.border_width: 0 draw_bg.border_position: 0 draw_bg.border_color: #00000000",
        "draw_bg.radius: 18 draw_bg.ellipse: 0 draw_bg.border_width: 0 draw_bg.border_position: 0 \
         draw_bg.border_color: #00000000 draw_bg.color2: #00000000 draw_bg.gradient: 0.0",
    );
    let s = s.replace(
        "show_bg: true draw_bg.color: #040303ff",
        "show_bg: true draw_bg.color: #000000ff",
    );
    // #36f item 3: the disc's left margin is 374-artboard math; the mounted
    // composer is 333 wide, so 328 put the disc 11px past the column (device
    // /snap: composer_5 [339,612,5,38] vs column right 344). Keep the
    // artboard's 10px right inset: 333 - 36 - 10 = 287.
    s.replace(
        "margin: Inset{left: 328 top: 118.222 right: 0 bottom: 0}",
        "margin: Inset{left: 287 top: 118.222 right: 0 bottom: 0}",
    )
}

/// Card #21d item 5 — scene 01's new-chat row carries a compose icon at its
/// right edge (beauty-host renders it too: 446 ink px in the component's own
/// right 20%), but the #16 ledger ships no icon node. Add one, bound to the
/// component's own `assets/icon_compose.svg` (no asset server).
///
/// Geometry is the scene's own measured chrome: the icon sits at x=331..355 of
/// the 374px row (scene `icon_compose` 347,130,24,28 minus the row origin 16,109).
fn new_chat_with_compose_icon(ui: &str) -> String {
    let icon = components_dir().join("new-chat/assets/icon_compose.svg");
    let icon = std::fs::canonicalize(&icon).unwrap_or(icon);
    // Card #21h: place the icon at the card's RIGHT padding.
    //
    // #21c pinned it with an absolute `left = 220 − 24 − 4` computed from the
    // column width; #21g then inset the card by 13px, narrowing it to 207, so the
    // fixed left pushed the icon's right edge to 216 > 207 and it was cut in half
    // (snap: `i0_newchat_icon r=[262,123,15,28]`).
    //
    // A `Fill` + `Align{x: 1.0}` wrapper is NOT available here: the icon is a
    // child of the `KitButton` root, and a `Fill` child under that root collapses
    // to `[0,0,0,0]` (measured again this card — the same #21c finding), while
    // `align` is a property of the *parent* and would drag the sibling label too.
    // So keep the fixed-size wrapper and compute `left` from the card's real
    // width. The thread column is a FIXED 220px (`lib.rs` `threads_column`), and
    // #21g insets the card 13px → the card is a constant 207px at every window
    // width, so this is width-exact. `item_h_...` asserts the icon cannot overflow.
    const CARD_W: f64 = 207.0; // `threads_column` 220 (lib.rs) − #21g's 13px inset
    const ICON_W: f64 = 24.0;
    const RIGHT_INSET: f64 = 4.0;
    let left = CARD_W - ICON_W - RIGHT_INSET;
    let node = format!(
        "View {{width: {ICON_W} height: 28 margin: Inset{{left: {left} top: 21 right: 0 bottom: 0}} \
         flow: Overlay padding: 0 clip_x: false clip_y: false\n\
         i0_newchat_icon := Svg {{\nwidth: {ICON_W} height: 28\nmargin: 0\n\
         animating: false draw_svg.svg: file_resource({:?}) \
         draw_svg.preserve_viewbox: true draw_svg.preserve_aspect: false\n}}\n}}\n",
        icon.to_string_lossy()
    );
    match ui.rfind('}') {
        Some(at) => format!("{}{}{}", &ui[..at], node, &ui[at..]),
        None => ui.to_owned(),
    }
}

/// Card #21d item 5 — a thread row's title is live and unbounded, so its
/// single-line label must ELLIPSIZE rather than hard-clip ("…print").
///
/// The lowering emits a bare `flow: Right` for a measured single-line label
/// (`design.rs:685`); the renderer only adds the ellipsis pair when the node
/// fills its slot (`design.rs:683`, at `fillw == 1`). A `PortalList` row binds a
/// title of any length into that measured box, so add the pair here.
fn ellipsize_single_line(ui: &str) -> String {
    // The component is authored for the 406px scene panel (row 379, label 302);
    // the app's column is 220px. A `Fill` chain lets the label take the column's
    // real width so the ellipsis fires, instead of laying out at 302 and being
    // clipped by the parent. (Measured: label ink fills the whole 197px box.)
    let ul = fill_widths(ui);
    ul.replacen(
        "flow: Right\n",
        "flow: Right max_lines: 1 text_overflow: TextOverflow.Ellipsis\n",
        1,
    )
}

/// Rewrite every `width: <number>` to `width: Fill`, so a component authored for
/// one panel width adopts the slot it is mounted in. Boundary-checked, so
/// `max_width`/`min_width` are untouched, and a non-numeric value is kept.
fn fill_widths(ui: &str) -> String {
    const KEY: &str = "width: ";
    let mut out = String::with_capacity(ui.len());
    let bytes = ui.as_bytes();
    let mut i = 0;
    while i < ui.len() {
        let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
        if prev_ok && ui[i..].starts_with(KEY) {
            let mut j = i + KEY.len();
            let start = j;
            while j < ui.len() && (bytes[j].is_ascii_digit() || bytes[j] == b'.') {
                j += 1;
            }
            if j > start {
                out.push_str(KEY);
                out.push_str("Fill");
                i = j;
                continue;
            }
        }
        let ch = ui[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Rewrite only the FIRST `width: <number>` to `width: Fit max_width: "<cap>"`,
/// leaving the rest for a later pass. The first width in a lowered component is
/// its root surface — the box that must hug and be capped.
fn set_first_width_fit_capped(ui: &str, cap: &str) -> String {
    const KEY: &str = "width: ";
    if let Some(at) = ui.find(KEY) {
        let after = &ui[at + KEY.len()..];
        let digits = after
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(after.len());
        if digits > 0 {
            return format!(
                "{}width: Fit max_width: {:?}{}",
                &ui[..at],
                cap,
                &after[digits..]
            );
        }
    }
    ui.to_owned()
}

/// Card #21d item 6 — rewrite every `http_resource("<loopback>/ux-images/<rest>")`
/// the lowering emits for an SVG/image `src` into a `file_resource("<abs>")`
/// pointing at the component's own asset on disk (`<components_dir>/<rest>`).
///
/// The lowering hard-codes an HTTP wrapper (`design.rs:743,764`) because a
/// design card may only name a loopback asset URL (`design_asset_allowed`), and
/// the recorded fixtures name the design lab's ad-hoc `:8170` server — which is
/// not ours to run. The bytes are already on disk beside the component, so bind
/// the file directly and the icons load with no asset server at all.
/// #36f item 2 — re-point `file_resource("…/components/<id>/assets/<file>")`
/// at the MATERIALIZED design root (`design::font_file` resolves root-first,
/// device-verified in #32g for the kit faces). Non-component paths and
/// already-relative ones pass through.
fn localize_component_icons(ui: &str) -> String {
    const MARK: &str = "file_resource(\"";
    let mut out = String::with_capacity(ui.len());
    let mut rest = ui;
    while let Some(at) = rest.find(MARK) {
        let (head, tail) = rest.split_at(at);
        out.push_str(head);
        let after = &tail[MARK.len()..];
        let Some(endq) = after.find('"') else {
            out.push_str(tail);
            return out;
        };
        let path = &after[..endq];
        if let Some(rel) = path.split_once("/components/") {
            // keep the components/ layer: font_file resolves under the
            // design ROOT, and the assets live at
            // <root>/components/<id>/assets/<file>
            let resolved =
                crate::design::font_file(&format!("components/{}", rel.1))
                    .display()
                    .to_string();
            out.push_str(&format!("file_resource({resolved:?})"));
        } else {
            out.push_str(&format!("file_resource({path:?})"));
        }
        // the rewrite replaced the call INCLUDING its closing paren — drop
        // the original's (the same contract localize_asset_resources runs on)
        rest = after[endq + 1..].strip_prefix(')').unwrap_or(&after[endq + 1..]);
    }
    out.push_str(rest);
    out
}

fn localize_asset_resources(ui: &str) -> String {
    const MARK: &str = "http_resource(\"";
    let root = components_dir();
    let mut out = String::with_capacity(ui.len());
    let mut rest = ui;
    while let Some(at) = rest.find(MARK) {
        let (head, tail) = rest.split_at(at);
        out.push_str(head);
        let after = &tail[MARK.len()..];
        let Some(endq) = after.find('"') else {
            out.push_str(tail);
            return out;
        };
        let url = &after[..endq];
        let rel = url.split_once("/ux-images/").map(|(_, r)| r).unwrap_or(url);
        // Absolute, because the launched app's cwd is not the repo root.
        let abs = std::fs::canonicalize(root.join(rel)).unwrap_or_else(|_| root.join(rel));
        out.push_str(&format!("file_resource({:?})", abs.to_string_lossy()));
        // Continue after the original call's closing quote, dropping its `)`.
        rest = after[endq + 1..].strip_prefix(')').unwrap_or(&after[endq + 1..]);
    }
    out.push_str(rest);
    out
}

/// Rewrite every `height: <number>` in a lowered DSL to `height: Fit` (card #21c
/// item 3). The numbers are the compiled fixture's measured artboard metrics;
/// `Fit` lets each node take its live content's height instead.
/// Card #21g item 1 — the worked-for row: small secondary-grey label over a
/// hairline rule.
///
/// The supervisor's yardstick is Codex (`outer/codex-refs/03-worked.png`,
/// "Worked for 3m 4s ⌄"): ~0.85× the prose body size, weight 400, secondary
/// grey `#6b6b6b`, a small chevron after it (the row IS the disclosure toggle,
/// appended by `flow.rs::worked_for`), and a 1px light divider under the row
/// across the column. The size/weight/colour live in the component's OWN kit
/// tokens (`design/components/worked-for/kit/native/light/kit.json`), so this
/// only appends the rule the #16 ledger does not draw — as a SIBLING after the
/// root (the mount wrapper is `flow: Down`), the same append idiom
/// `new_chat_with_compose_icon` uses for chrome the ledger omits.
fn worked_for_style(ui: &str) -> String {
    let ui = fit_heights(ui);
    // `#ececec` — the light rule Codex draws under the settled row.
    let rule = "View {width: Fill height: 1 margin: Inset{left: 0 top: 0 right: 0 bottom: 0} \
                show_bg: true draw_bg.color: #ecececff}\n";
    format!("{ui}{rule}")
}

fn fit_heights(ui: &str) -> String {
    const KEY: &str = "height: ";
    let mut out = String::with_capacity(ui.len());
    let bytes = ui.as_bytes();
    let mut i = 0;
    while i < ui.len() {
        if ui[i..].starts_with(KEY) {
            out.push_str("height: Fit");
            i += KEY.len();
            while i < ui.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
        } else {
            let ch = ui[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// The copy ids a component declares (the coverage audit: every live id a
/// component can receive is one of these).
pub fn declared_copies(kind: ItemKind) -> Vec<String> {
    resolve(kind).0.bindings.iter().map(|b| b.copy.to_owned()).collect()
}

/// Every component id (the swap set).
pub fn all_ids() -> Vec<&'static str> {
    ItemKind::ALL.iter().map(|k| k.id()).collect()
}

/// The per-item control actions a component emits (card #21 §3): the semantic
/// control name inside `kind`, and the **declared action id**
/// ([`bindings::ACTIONS`]) the host dispatches for it.
///
/// This is the routing table the screen uses — the same "view names an id, the
/// module owns the meaning" rule as the header controls (lib.rs). `row` is the
/// whole-row hit (a `thread-row`/`new-chat` is a button); the rest name the
/// specific control the component shows.
pub const CONTROLS: &[(ItemKind, &str, &str)] = &[
    (ItemKind::ThreadRow, "row", "thread.open"),
    (ItemKind::NewChat, "row", "session.new"),
    (ItemKind::ToolCell, "expand", "tool.toggle"),
    (ItemKind::AnswerActions, "copy", "answer.copy"),
    (ItemKind::Composer, "send", "composer.submit"),
    (ItemKind::Composer, "stop", "turn.interrupt"),
    (ItemKind::Composer, "steer", "turn.steer"),
];

/// The action id a `control` inside `kind` emits, or `None` when that control
/// is not a declared action on that component.
pub fn action_for(kind: ItemKind, control: &str) -> Option<&'static str> {
    CONTROLS
        .iter()
        .find(|(k, c, _)| *k == kind && *c == control)
        .map(|(_, _, a)| *a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings;
    use crate::flow::FlowUi;
    use octoscode_store::Store;
    use std::sync::{Arc, Mutex};

    /// #32h TOP — the typing fix's structural proof: the live draft must not
    /// ride the lowered composer DSL. When it did (composer_idle_input_text),
    /// every keystroke changed the DSL, missed the mount cache (mount.rs:95
    /// compares strings) and remounted the whole composer with a NEW
    /// TextInput — the Android IME lost its target after the first character.
    /// #36f — the measured dark repro (384x788, OCTOSCODE_THEME=dark): the
    /// user bubble was surface #f5f5f7 with #fafbfb text = 1.05:1 and the
    /// composer's pill ink #434343/#252525 on the #1c1f22 card = 1.00:1,
    /// because lower()'s dark layer (retint_dsl) had no table entries for
    /// the components' artboard literals. FAILS ON MAIN (retint passes the
    /// literals through); passes once the four keys exist.
    #[test]
    fn dark_retint_maps_the_component_artboard_literals() {
        let _theme = crate::screens::theme::test_lock();
        // The INKS ride retint (role-blind is correct for text: light inks
        // map to the dark palette's light inks).
        let dsl = concat!(
            "draw_text.color: #fafbfbff\n",
            "draw_text.color: #434343ff\n",
            "draw_text.color: #252525ff\n",
            "draw_bg.color: #1c1f22ff"
        );
        let out = crate::screens::theme::retint_dsl(dsl);
        assert!(out.contains("#f5f5f7ff"), "bubble text unmapped");
        assert!(!out.contains("#434343ff"), "pill ink unmapped (1.00:1 on the device)");
        assert!(!out.contains("#252525ff"), "secondary ink unmapped");
        // #1c1f22 is the SHELL's dark surface token: retint must leave it
        // (the f31d fixed-point), and the bubble's role pin owns the ink.
        assert!(
            out.contains("draw_bg.color: #1c1f22ff"),
            "retint no longer fixes the dark surface token"
        );
        // The SURFACE is role-scoped: #f5f5f7 is also the dark ink, so the
        // table must never carry it — the bubble layer rewrites draw_bg
        // lines only, and retint passes the result through untouched.
        for surface in ["#fafbfbff", "#f5f5f7ff"] {
            let line = format!("show_bg: true draw_bg.color: {surface}");
            let fixed = bubble_dark_surface(&line);
            assert!(
                fixed.contains("#2c2c2eff"),
                "the bubble surface stayed light-board {surface} (1.05:1 on the device)"
            );
            assert!(
                crate::screens::theme::retint_dsl(&fixed).contains("#2c2c2eff"),
                "retint ate the corrected dark surface"
            );
        }
        // The bubble's dark INK (the #1c1f22 timestamp) is the pin's job —
        // retint cannot take it (the shell's own dark token).
        let ts = bubble_dark_surface("draw_text.color: #1c1f22ff");
        assert!(
            ts.contains("draw_text.color: #f5f5f7ff"),
            "the timestamp ink stayed light-board on the dark bubble"
        );
        // ...and the text side of the SAME literal survives retint as ink:
        assert!(
            crate::screens::theme::retint_dsl("draw_text.color: #fafbfbff").contains("#f5f5f7ff"),
            "the text ink lost its light value"
        );
    }

    /// #36f diagnostic: the FULL lowering path (not just retint) — tests
    /// have no OS-appearance reader, so resolved() falls back to dark and
    /// this runs the real dark pipeline. Asserts the surface key lands; on
    /// failure the panic prints the ACTUAL draw_bg line (ground truth for
    /// the "surface does not retint" contradiction).
    /// #36f item 3 — the send disc rode the 374-wide artboard margin
    /// (left 328), 11px past the mounted 333 column (device /snap on the
    /// pre-fix tree: composer_5 [339,612,5,38] vs column right 344).
    /// Fails on main (no re-anchor there); 333-36-10 = 287.
    /// #36g item 1 — the icons' ink is themed per resolved mode (bare
    /// literals: a theme ROLE ref draws 0 ink inside a component isolate,
    /// the #36c lesson). Tests have no OS-appearance reader, so resolved()
    /// falls back to dark; light is set explicitly and restored.
    #[test]
    fn the_answer_action_icons_carry_theme_ink() {
        let _theme = crate::screens::theme::test_lock();
        // A1: the actions are secondary controls — the shell's muted ink per
        // resolved mode (#36g's point stands: dark gets a LIGHT ink, never
        // the fixed artboard stroke).
        let dark = lower(ItemKind::AnswerActions, "t0", &[]).expect("lower");
        assert!(
            dark.contains("draw_svg.color: #98989dff"),
            "the icons kept the light-mode ink in dark"
        );
        crate::screens::theme::set_preference("light");
        let light = lower(ItemKind::AnswerActions, "t1", &[]).expect("lower");
        crate::screens::theme::set_preference("dark");
        assert!(
            light.contains("draw_svg.color: #6e6e73ff"),
            "the icons lost the muted light ink"
        );
    }

    /// A1 (supersedes #36f item 3): the send disc is placed by the control
    /// row's FLOW at the card's right padding, so no artboard margin (328 on
    /// the 374 board, 287 on the 333 column) can push it past the column at
    /// any width.
    #[test]
    fn the_send_disc_stays_inside_the_mounted_column() {
        let _theme = crate::screens::theme::test_lock();
        let dsl = lower(ItemKind::Composer, "t0", &[]).expect("lower");
        assert!(!dsl.contains("left: 328"), "the overflowing artboard margin survived");
        assert!(!dsl.contains("left: 287"), "an artboard-anchored disc survived");
        let row = dsl.find("i0_composer_row := View{width: Fill").expect("a flowing control row");
        let fill = dsl[row..].find("View{width: Fill height: 1}").expect("the spacer");
        let send = dsl[row..].find("send_hit := Button").expect("the send control");
        assert!(fill < send, "the send control sits right of the spacer, at the row's end");
    }

    #[test]
    fn the_lowered_bubble_surface_retints_in_dark() {
        let _theme = crate::screens::theme::test_lock();
        // The full dark pipeline: bubble_dark_surface rewrites the draw_bg
        // lines role-scoped, retint maps the inks, and neither eats the
        // other's output (#2c2c2e is not in the light key set).
        let dsl = lower(ItemKind::UserBubble, "t0", &[]).expect("lower");
        let line = dsl
            .lines()
            .find(|l| l.contains("draw_bg.color:") && l.contains("show_bg"))
            .unwrap_or("(no show_bg surface line in the lowered DSL)");
        assert!(
            dsl.contains("#2c2c2eff"),
            "the lowered bubble surface did not retint to the dark token — actual line: {line}"
        );
        assert!(
            dsl.contains("draw_text.color: #f5f5f7ff"),
            "the bubble text lost its light ink on the dark surface"
        );
    }

    #[test]
    fn the_composer_dsl_is_draft_free_and_stable() {
        let _theme = crate::screens::theme::test_lock();
        let store = Arc::new(Store::new());
        let ui = Arc::new(Mutex::new(FlowUi::default()));
        ui.lock().unwrap().set_draft_inner("hello ime");
        let ctx = bindings::Ctx::new(&store, &ui);
        let copies = item_copies(ItemKind::Composer, &ctx, 0, None).expect("copies");
        assert!(
            !copies
                .iter()
                .any(|(id, _)| id == "composer_idle_input_text"),
            "the live draft rides the lowered DSL again"
        );
        assert!(
            copies
                .iter()
                .any(|(id, _)| id == "composer_idle_input_placeholder"),
            "the placeholder copy vanished"
        );
        // Whatever the draft, the lowered DSL is byte-identical — the mount
        // cache hits while typing, so the composer never remounts.
        let idle = lower(ItemKind::Composer, "t0", &[]).expect("lower");
        ui.lock().unwrap().set_draft_inner("different text entirely");
        let ctx2 = bindings::Ctx::new(&store, &ui);
        let copies2 = item_copies(ItemKind::Composer, &ctx2, 0, None).expect("copies2");
        let typed = lower(ItemKind::Composer, "t0", &copies2).expect("lower 2");
        assert_eq!(idle, typed, "typing changed the lowered DSL");
        assert!(!typed.contains("different text entirely"));
    }
}
