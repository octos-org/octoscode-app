//! Card #15 — the **Stage-B conversation cards**, mounted as data.
//!
//! A card is not code here: it is an id, a slot, a set of declared binding ids
//! and a kit. [`manifest`] reads `design/cards/index.json`; [`structure`] reads
//! the card's authored tree (`mapped.json`). Swapping which card fills a slot
//! is an edit to that manifest — no Rust change.
//!
//! ## Why not the design-flow's own mapping?
//!
//! The authored `mapped.json` carries each node's **measured** `x/y/w/h` from
//! the 406-wide artboard. RULES (8.10, "Dynamic content → native flow
//! regions") forbids rendering runtime content at measured coordinates: prose
//! and lists map to native flowing widgets. So this renderer uses the tree's
//! **order, kinds and ids** — never its geometry — and lays the result out in
//! the D12 columns (thread list | conversation | composer dock).
//!
//! ## Per-slot fallback
//!
//! Every slot renders independently. A card whose artefacts are missing or
//! whose tree does not parse leaves its slot to the module's [`crate::fallback`]
//! widget and is logged by name ([`render_slot`]'s `Err`), so the screen always
//! renders.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::Value;

/// The committed manifest, embedded so a run without a `design/cards` tree
/// (a headless isolate, another CWD) still knows which card fills which slot.
const EMBEDDED_MANIFEST: &str = include_str!("../../../design/cards/index.json");

/// A card slot on the conversation screen (D12 columns + the sub-slots the
/// conversation column hosts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// Left column: the thread list (`conversation-01`).
    ThreadList,
    /// Middle column: the streaming turn (`conversation-03`).
    Conversation,
    /// Docked at the middle column's bottom: the composer (`conversation-08`).
    Composer,
    /// Inside the conversation column: the tool cells (`conversation-04`).
    ToolCells,
    /// Inside the conversation column: the completed answer (`conversation-09`).
    CompletedAnswer,
}

impl Slot {
    /// Every slot, in D12 layout order.
    pub const ALL: &'static [Slot] = &[
        Slot::ThreadList,
        Slot::Conversation,
        Slot::Composer,
        Slot::ToolCells,
        Slot::CompletedAnswer,
    ];

    /// The slot's manifest name (this is the string `index.json` uses, so a
    /// manifest edit is the whole wiring).
    pub fn name(self) -> &'static str {
        match self {
            Slot::ThreadList => "thread_list",
            Slot::Conversation => "conversation",
            Slot::Composer => "composer",
            Slot::ToolCells => "tool_cells",
            Slot::CompletedAnswer => "completed_answer",
        }
    }

    pub fn from_name(name: &str) -> Option<Slot> {
        Slot::ALL.iter().copied().find(|s| s.name() == name)
    }
}

/// The card manifest (`design/cards/index.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub stage: String,
    pub source_repo: String,
    pub source_branch: String,
    pub source_commit: String,
    pub mount: String,
    pub cards: Vec<Card>,
}

/// One mounted card.
#[derive(Debug, Clone, Deserialize)]
pub struct Card {
    pub id: String,
    /// The slot this card fills (see [`Slot`]).
    pub slot: String,
    pub title: String,
    pub source_commit: String,
    pub gate_b_score: f64,
    /// Every binding id this card declares (data + action). 8.8 condition 2:
    /// the card names ids; the module owns the meaning.
    pub bindings: Vec<String>,
    pub artifacts: Artifacts,
}

/// The card's committed artefacts, **relative to the manifest's directory**
/// (i.e. `conversation-01/mapped.json`, not `design/cards/conversation-01/…`),
/// because [`mounted`] resolves them against the directory holding
/// `index.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Artifacts {
    pub card: String,
    pub data: String,
    pub kit_tokens: String,
    pub kit_components: String,
    pub semantic_map: String,
    pub mapped: String,
}

/// One node of the authored tree (`mapped.json`). Geometry is deliberately
/// **not** deserialized — see the module docs.
#[derive(Debug, Clone, Deserialize)]
pub struct Node {
    /// Node kind: `stack` | `text` | `button` | `input`.
    #[serde(rename = "t")]
    pub kind: String,
    /// The authored node id (`t01`, `thread_1`, `composer_input`, …).
    #[serde(default)]
    pub id: String,
    /// The authored copy, when the node has any.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub c: Vec<Node>,
}

/// One rendered row of a slot: a native widget's worth of content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The native widget this row maps to (`text` → Label, `button` → Button,
    /// `input` → TextInput).
    pub kind: String,
    /// The authored node id (also the binding's `card_source` key).
    pub id: String,
    /// The authored copy; a binding may override it (see [`render_slot`]).
    pub text: String,
}

/// The resolved manifest, with the directory its artefacts are read from.
#[derive(Debug)]
pub struct Mounted {
    pub manifest: Manifest,
    /// The directory the manifest's relative artefact paths resolve against.
    pub dir: PathBuf,
}

/// Where the card directory is.
///
/// Precedence: `OCTOSCODE_CARDS_DIR`, then `design/cards` under the CWD, then
/// `design/cards` beside this crate (`CARGO_MANIFEST_DIR/../../design/cards` —
/// so a test or a binary run from anywhere in the workspace still finds the
/// committed manifest).
fn cards_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OCTOSCODE_CARDS_DIR") {
        return PathBuf::from(dir);
    }
    let cwd = PathBuf::from("design/cards");
    if cwd.join("index.json").is_file() {
        return cwd;
    }
    let beside: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../design/cards")
        .into();
    if beside.join("index.json").is_file() {
        return beside;
    }
    cwd
}

/// The mounted cards. Prefers the on-disk manifest (so a card swap is a file
/// edit) and falls back to the embedded copy (so a run with no card tree still
/// knows its slots).
pub fn mounted() -> &'static Mounted {
    static MOUNTED: OnceLock<Mounted> = OnceLock::new();
    MOUNTED.get_or_init(|| {
        let dir = cards_dir();
        let on_disk = dir.join("index.json");
        match std::fs::read_to_string(&on_disk)
            .ok()
            .and_then(|s| serde_json::from_str::<Manifest>(&s).ok())
        {
            Some(manifest) => Mounted { manifest, dir },
            None => Mounted {
                manifest: serde_json::from_str(EMBEDDED_MANIFEST)
                    .expect("the embedded card manifest is valid JSON"),
                // The embedded manifest's paths are workspace-relative.
                dir: PathBuf::from("."),
            },
        }
    })
}

/// The card filling `slot`, per the manifest.
pub fn card_for_slot(slot: Slot) -> Option<&'static Card> {
    mounted()
        .manifest
        .cards
        .iter()
        .find(|c| c.slot == slot.name())
}

/// Every binding id the mounted cards declare (the coverage set).
pub fn declared_bindings() -> BTreeSet<String> {
    mounted()
        .manifest
        .cards
        .iter()
        .flat_map(|c| c.bindings.iter().cloned())
        .collect()
}

/// Parse a card's authored tree. `Err` names the reason (the slot then falls
/// back, and the caller logs it).
pub fn structure(card: &Card) -> Result<Node, String> {
    let path: PathBuf = mounted().dir.join(&card.artifacts.mapped);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str::<Value>(&text)
        .map_err(|e| format!("parse {}: {e}", path.display()))
        .and_then(|v| {
            let tree = v.get("tree").ok_or("no `tree` field")?;
            serde_json::from_value::<Node>(tree.clone()).map_err(|e| format!("tree shape: {e}"))
        })
}

/// Flatten a card tree into rows, document order, dropping the root surface.
///
/// The order (not the geometry) is what carries the reading order — the same
/// choice the web client makes when it folds `projection/envelope` frames.
pub fn plan(root: &Node) -> Vec<Row> {
    /// The node's own copy, or the first non-empty text in its subtree (a
    /// control's label is its child text node in the authored tree, e.g.
    /// `thread_1` → `t03` "Fix steer queue drop on reconnect").
    fn label_of(node: &Node) -> String {
        if !node.text.is_empty() {
            return node.text.clone();
        }
        for child in &node.c {
            let t = label_of(child);
            if !t.is_empty() {
                return t;
            }
        }
        String::new()
    }

    fn walk(node: &Node, out: &mut Vec<Row>) {
        // A **control** is itself the row — its children are its label. This is
        // the node the card's declared binding keys off (`thread_1`,
        // `new_chat`, `tool_1`, `worked_row`, `composer_input`), so recursing
        // past it would lose the very id the binding names.
        if matches!(node.kind.as_str(), "button" | "input") {
            out.push(Row {
                kind: node.kind.clone(),
                id: node.id.clone(),
                text: label_of(node),
            });
            return;
        }
        // A container: recurse, in document order.
        if !node.c.is_empty() {
            for child in &node.c {
                walk(child, out);
            }
            return;
        }
        out.push(Row {
            kind: node.kind.clone(),
            id: node.id.clone(),
            text: node.text.clone(),
        });
    }
    let mut out = Vec::new();
    for child in &root.c {
        walk(child, &mut out);
    }
    out
}

/// Render a slot's rows as the native flow text its slot widget shows, with the
/// declared binding ids resolved for values the server owns.
///
/// `values` is the module's binding resolver ([`crate::bindings::query`]), so
/// this stays pure: give it any resolver and it renders whatever the binding
/// ids resolve to. `Err` = the slot falls back (missing/failing card).
pub fn render_slot(
    slot: Slot,
    values: &dyn Fn(&str) -> Option<Value>,
) -> Result<String, String> {
    let card = card_for_slot(slot)
        .ok_or_else(|| format!("no card declares slot {}", slot.name()))?;
    let root = structure(card)?;
    let rows = plan(&root);

    let as_text = |id: &str| -> Option<String> {
        match values(id) {
            Some(Value::String(s)) => Some(s),
            Some(Value::Null) | None => None,
            Some(other) => Some(other.to_string()),
        }
    };

    let mut out = String::new();
    match slot {
        // The thread list is a native list of rows: the authored 5 rows carry
        // the design's copy, and the `threads` binding overrides them with the
        // server's own rows when it has any.
        Slot::ThreadList => {
            let server = values("threads").and_then(|v| v.as_array().cloned()).unwrap_or_default();
            let titles: Vec<String> = server
                .iter()
                .map(|r| {
                    r.get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("(untitled)")
                        .to_owned()
                })
                .collect();
            let active = as_text("threads.active").unwrap_or_default();
            let mut index = 0usize;
            for row in rows.iter().filter(|r| r.id.starts_with("thread_")) {
                let id = server.get(index).and_then(|r| r.get("id")).and_then(|v| v.as_str());
                let text = match (titles.get(index), id) {
                    // A titled session shows its title; an untitled one shows
                    // its id (still identifiable) rather than a dead
                    // "(untitled)"; with no server row at all, the card's own
                    // authored copy stands in.
                    (Some(t), _) if !t.is_empty() && t != "(untitled)" => t.clone(),
                    (_, Some(id)) => id.to_owned(),
                    _ => row.text.clone(),
                };
                let selected = id.map(|id| id == active).unwrap_or(false);
                out.push_str(&format!("{}\n", format_row(row, &text, selected)));
                index += 1;
            }
            for row in rows.iter().filter(|r| r.id == "new_chat") {
                out.push_str(&format!("{}\n", format_row(row, &row.text, false)));
            }
            if out.is_empty() {
                out.push_str("(no threads)\n");
            }
        }
        // The conversation column is a native flow region: the timeline
        // entries in protocol order (the user entry precedes its turn's
        // replies — card #14 defect 1), then the live activity row.
        Slot::Conversation => {
            let entries = values("timeline.entries")
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            if entries.is_empty() {
                // No turn yet: the card's authored copy is the empty state.
                for row in &rows {
                    out.push_str(&format!("{}\n", format_row(row, &row.text, false)));
                }
            } else {
                for e in &entries {
                    let kind = e.get("kind").and_then(|k| k.as_str()).unwrap_or("?");
                    let text = e.get("text").and_then(|t| t.as_str()).unwrap_or("");
                    out.push_str(&format!("[{kind}] {text}\n"));
                }
                if let Some(activity) = as_text("turn.activity") {
                    if !activity.is_empty() {
                        out.push_str(&format!("[{activity}]\n"));
                    }
                }
            }
        }
        // The composer dock: the input's placeholder + the live draft.
        Slot::Composer => {
            for row in &rows {
                let text = match row.id.as_str() {
                    "composer_input" => as_text("composer.placeholder")
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| row.text.clone()),
                    _ => row.text.clone(),
                };
                out.push_str(&format!("{}\n", format_row(row, &text, false)));
            }
            let draft = as_text("composer.draft").unwrap_or_default();
            out.push_str(&format!("[draft] {draft}\n"));
        }
        // The tool cells: the authored rows, overridden by the `tools`
        // binding when the turn has called any.
        Slot::ToolCells => {
            let server = values("tools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
            let mut index = 0usize;
            for row in rows.iter().filter(|r| r.id.starts_with("tool_")) {
                let text = match server.get(index) {
                    Some(t) => {
                        let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                        let status = t.get("status").and_then(|s| s.as_str()).unwrap_or("?");
                        format!("{name} [{status}]")
                    }
                    None => row.text.clone(),
                };
                out.push_str(&format!("{}\n", format_row(row, &text, false)));
                index += 1;
            }
            if out.is_empty() {
                out.push_str("tools: (none)\n");
            }
        }
        // The completed answer: the `Worked for` disclosure row, then the
        // answer prose from the timeline's last assistant entry.
        Slot::CompletedAnswer => {
            let worked = as_text("answer.worked_for").unwrap_or_default();
            for row in rows.iter().filter(|r| r.id == "worked_row") {
                let text = if worked.is_empty() { row.text.clone() } else { worked.clone() };
                out.push_str(&format!("{}\n", format_row(row, &text, false)));
            }
            let answer: String = values("timeline.entries")
                .and_then(|v| v.as_array().cloned())
                .map(|entries| {
                    entries
                        .iter()
                        .filter(|e| {
                            e.get("kind").and_then(|k| k.as_str()) == Some("assistant.text")
                        })
                        .filter_map(|e| e.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            if !answer.is_empty() {
                out.push_str(&answer);
                out.push('\n');
            } else {
                for row in rows.iter().filter(|r| r.id != "worked_row") {
                    out.push_str(&format!("{}\n", row.text));
                }
            }
        }
    }
    Ok(out.trim_end().to_owned())
}

/// One row as slot text: `[button] New chat`, with a selection mark.
fn format_row(row: &Row, text: &str, selected: bool) -> String {
    let mark = if selected { "▸ " } else { "" };
    format!("[{} {}] {mark}{text}", row.kind, row.id)
}

/// The Gate-B score of the card in `slot` (0.0 when the slot is empty).
pub fn gate_b_score(slot: Slot) -> f64 {
    card_for_slot(slot).map(|c| c.gate_b_score).unwrap_or(0.0)
}

/// The artefacts directory of `slot`'s card, for a caller that wants the kit.
pub fn card_dir(slot: Slot) -> Option<PathBuf> {
    card_for_slot(slot).map(|c| mounted().dir.join(&c.artifacts.card))
}

/// `slot`'s kit token table (`kit.json`), parsed; `Err` names the reason.
pub fn kit_tokens(slot: Slot) -> Result<Value, String> {
    let card = card_for_slot(slot).ok_or_else(|| format!("no card in slot {}", slot.name()))?;
    let path = Path::new(&mounted().dir).join(&card.artifacts.kit_tokens);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
}
