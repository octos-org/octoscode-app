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

/// The built-in placeholder component for `kind` (see the module docs).
pub fn builtin(kind: ItemKind) -> Component {
    let (leaves, bindings): (&[Leaf], &'static [Binding]) = match kind {
        ItemKind::ThreadRow => (
            &[
                Leaf("title", "thread_row_title", "Thread title", 320.0, 20.0),
                Leaf("meta", "thread_row_meta", "0 messages", 320.0, 16.0),
            ],
            &[
                Binding { copy: "thread_row_title", binding: "threads[].title" },
                Binding { copy: "thread_row_meta", binding: "threads[].meta" },
            ],
        ),
        ItemKind::UserBubble => (
            &[Leaf("text", "user_bubble_text", "User message", 330.0, 36.0)],
            &[Binding { copy: "user_bubble_text", binding: "timeline.entries[].text" }],
        ),
        ItemKind::AssistantProse => (
            &[Leaf("text", "assistant_prose_text", "Assistant reply", 340.0, 60.0)],
            &[Binding { copy: "assistant_prose_text", binding: "timeline.entries[].text" }],
        ),
        ItemKind::ToolCell => (
            &[
                Leaf("name", "tool_cell_name", "tool", 200.0, 18.0),
                Leaf("status", "tool_cell_status", "running", 120.0, 16.0),
                Leaf("summary", "tool_cell_summary", "", 340.0, 28.0),
            ],
            &[
                Binding { copy: "tool_cell_name", binding: "tools[].name" },
                Binding { copy: "tool_cell_status", binding: "tools[].status" },
                Binding { copy: "tool_cell_summary", binding: "tools[].summary" },
            ],
        ),
        ItemKind::WorkingRow => (
            &[Leaf("text", "working_row_text", "Working", 200.0, 18.0)],
            &[Binding { copy: "working_row_text", binding: "turn.activity" }],
        ),
        ItemKind::WorkedFor => (
            &[Leaf("label", "worked_row_label", "Worked for 0s", 240.0, 20.0)],
            &[Binding { copy: "worked_row_label", binding: "answer.worked_for" }],
        ),
        ItemKind::AnswerActions => (
            &[
                Leaf("copy_btn", "answer_actions_copy", "Copy", 60.0, 16.0),
                Leaf("time", "answer_actions_time", "", 160.0, 16.0),
            ],
            &[
                Binding { copy: "answer_actions_copy", binding: "" },
                Binding { copy: "answer_actions_time", binding: "answer.timestamp" },
            ],
        ),
    };
    let (ledger, data) = ledger(leaves);
    Component { id: kind.id(), kind, ledger, data, kit_dir: components_dir(), bindings }
}

/// The component directory: `OCTOSCODE_COMPONENTS_DIR`, else `design/components`
/// under the CWD, else beside this crate (so a test run from anywhere finds a
/// dropped-in component set).
pub fn components_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OCTOSCODE_COMPONENTS_DIR") {
        return PathBuf::from(dir);
    }
    let cwd = PathBuf::from("design/components");
    if cwd.is_dir() {
        return cwd;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/components")
}

/// Resolve the component for `kind`: an on-disk `<id>.l0` + `<id>.json` when
/// present (card #16's component, or any dropped-in file), else the built-in
/// placeholder. The bool reports whether the on-disk component was used.
///
/// A dropped-in component may ship its own kit (`<id>/native/<mood>/kit.json`);
/// absent one it lowers against the shared placeholder pack.
pub fn resolve(kind: ItemKind) -> (Component, bool) {
    let dir = components_dir();
    let l0 = dir.join(format!("{}.l0", kind.id()));
    if let Ok(ledger) = std::fs::read_to_string(&l0) {
        let json_path = dir.join(format!("{}.json", kind.id()));
        let data = std::fs::read_to_string(&json_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or_else(|| json!({}));
        let own_kit = dir.join(kind.id());
        let kit_dir = if own_kit.join("native/light/kit.json").is_file() { own_kit } else { dir };
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

    let component = resolve(kind).0;
    let mut out = Vec::new();
    for b in component.bindings {
        if b.binding.is_empty() {
            continue; // static copy (the ledger's own en: stays)
        }
        let value = match kind {
            ItemKind::ThreadRow => match b.binding {
                "threads[].title" => text(&row("threads")?.get("title").cloned().unwrap_or(Value::Null)),
                "threads[].meta" => {
                    let r = row("threads")?;
                    let n = r.get("message_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    format!("{n} messages")
                }
                other => return Err(format!("thread-row has no arm for {other:?}")),
            },
            ItemKind::UserBubble | ItemKind::AssistantProse => {
                text(&row("timeline.entries")?.get("text").cloned().unwrap_or(Value::Null))
            }
            ItemKind::ToolCell => match b.binding {
                "tools[].name" => text(&row("tools")?.get("name").cloned().unwrap_or(Value::Null)),
                "tools[].status" => text(&row("tools")?.get("status").cloned().unwrap_or(Value::Null)),
                "tools[].summary" => text(&row("tools")?.get("summary").cloned().unwrap_or(Value::Null)),
                other => return Err(format!("tool-cell has no arm for {other:?}")),
            },
            ItemKind::WorkingRow => text(&get("turn.activity")?),
            ItemKind::WorkedFor => text(&get("answer.worked_for")?),
            ItemKind::AnswerActions => text(&get("answer.timestamp")?),
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
    let ui = octoscript_makepad::design::to_makepad_ui(&tree)?;
    Ok(ui.replace("beauty_0", &format!("i{token}_{}", kind.id().replace('-', ""))))
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
