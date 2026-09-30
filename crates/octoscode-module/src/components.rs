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
            Binding { copy: "composer_idle_input_text", binding: "composer.draft" },
            Binding { copy: "composer_idle_input_placeholder", binding: "composer.placeholder" },
            // The model pill (`v4-flash ▾`) and the approval pill are the
            // component's own copy — static until `composer.model` is declared.
            Binding { copy: "t04_text", binding: "" },
            Binding { copy: "pill1_t_text", binding: "" },
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
            other => return Err(format!("{} has no arm for binding {other:?}", kind.id())),
        };
        out.push((b.copy.to_owned(), value));
    }
    Ok(out)
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
        ItemKind::UserBubble => bubble_live_layout(&ui),
        // Card #21d item 5: a session title is live and unbounded, so the row's
        // single-line label must ELLIPSIZE rather than hard-clip ("…print").
        ItemKind::ThreadRow => ellipsize_single_line(&ui),
        // Card #21d item 5: scene 01's new-chat row carries a compose icon at
        // its right edge (beauty-host renders it too: 446 ink px in the
        // component's own right 20%). The #16 ledger omits the node, so add it.
        ItemKind::NewChat => new_chat_with_compose_icon(&ui),
        // Card #21e item 5: the timestamp sits at the artboard's own x=243.64 in a
        // 364px row, which lands mid-column once mounted in a wider slot.
        ItemKind::AnswerActions => right_align_timestamp(&ui),
        // Card #21e item 6: fenced code must not soft-wrap.
        ItemKind::AssistantProse => reachable_code(&fit_heights(&ui)),
        // Card #21e item 3: the send control is a flat black disc.
        ItemKind::Composer => flat_send_button(&ui),
        // Card #21e item 8: the activity row's spinner keeps its 18px box.
        ItemKind::WorkingRow => working_row_layout(&ui),
        ItemKind::WorkedFor => worked_for_style(&ui),
        ItemKind::ToolCell => fit_heights(&ui),
    };
    // Card #21d item 6: resolve every emitted `http_resource(…)` icon to the
    // component's own file on disk, so the app needs no dev asset server.
    let ui = localize_asset_resources(&ui);
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
fn bubble_live_layout(ui: &str) -> String {
    // Heights hug (card #21c); the ROOT also hugs, capped at 80% of the column,
    // so a long message widens to the cap then wraps. Inner boxes keep the
    // measured widths the artboard gave them (`Fill` under a `Fit` root collapses
    // to the minimum — measured: a 16px-wide bubble), and the labels WRAP.
    let s = fit_heights(ui);
    // Card #21e item 7: symmetric vertical padding (the artboard's absolute tops
    // leave the last wrapped line on the bottom edge).
    let s = symmetric_bubble_padding(&s);
    let s = set_first_width_fit_capped(&s, "80%");
    let mut s = s.replace("flow: Right\n", "flow: Right{wrap: true}\n");
    s = s.replace("flow: Right ", "flow: Right{wrap: true} ");
    format!("user_align := View{{width:Fill height:Fit flow:Down align: Align{{x: 1.0}} {s}}}")
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
    s.replace(
        "show_bg: true draw_bg.color: #040303ff",
        "show_bg: true draw_bg.color: #000000ff",
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
