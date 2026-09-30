//! Card #15b — the **L0 runtime**, hosted in-process.
//!
//! #15 mounted the cards' *data* and printed their node trees as text. That is
//! not a render. This module hosts the **same renderer that produced the
//! Gate-B `*-native.png` renders**: `card-host`'s chain
//! (`native/OctoSense-App-Hub/crates/card-host/src/host.rs:144-154`) —
//!
//! ```text
//! octoscript_makepad::l0::prepare(card, data, kit_dir)   // realize + lower
//!   -> octoscript_makepad::design::to_makepad_ui(&tree)  // -> DSL source
//!      -> Splash::set_text(cx, &dsl)                      // -> real widgets
//! ```
//!
//! The vocabulary that DSL names (`DesignSurface`, `DesignNativeButton`, …) is
//! registered process-wide by [`register_vocabulary`], exactly as card-host
//! does (`host.rs:206-215`): `octoscript_widgets::{design,kit}::script_mod`.
//!
//! ## Live data
//!
//! A card's strings live in its `page.card` `copy` block
//! (`copy t03_text { class: user-copy, en: "…" }`), and its `view` reads them
//! (`Text…(instance: "t03", text: copy.t03_text)`). So live values are injected
//! by rewriting the named `copy` entries before lowering — the card's own
//! authored structure and kit stay untouched, which is what keeps the result
//! looking like the Gate-B render while showing real data.
use std::path::PathBuf;

use makepad_widgets::ScriptVm;

use crate::bindings;
use crate::cards;

/// Register the design/kit vocabulary every lowered card needs.
///
/// `register_splash_isolate_mod` is process-wide (card-host's own note,
/// `host.rs:206-215`): an isolate starts with the standard widgets only, so a
/// card naming `DesignSurface` does not evaluate without this.
pub fn register_vocabulary() {
    use makepad_widgets::widget_async::register_splash_isolate_mod;
    use std::sync::Once;
    // `register_splash_isolate_mod` PUSHES into a thread-local vec, so calling
    // it once per module instance would register the vocabulary N times.
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        fn design(vm: &mut ScriptVm) {
            octoscript_widgets::design::script_mod(vm);
        }
        fn kit(vm: &mut ScriptVm) {
            octoscript_widgets::kit::script_mod(vm);
        }
        // #32h item 1 (layer 3): the screen cards evaluate inside their OWN
        // Splash isolate (mount.rs), which does not see the module VM where
        // NAV is registered — an unregistered global is NIL (kit.rs:141-143),
        // so the wired `on_click: || { NAV(t: "connect") }` still silently
        // did nothing (the 8367 run: click ok, zero nav logs). This mod is
        // installed into every isolate BEFORE its first body evaluation, so
        // each one gets the NAV global (same thread-free closure shape as
        // register() in lib.rs — NAV_QUEUE/SignalToUI are statics).
        fn nav(vm: &mut ScriptVm) {
            use makepad_widgets::{LiveId, live_id};
            let f = octoscript_render::add_global_fn(
                vm,
                &[(makepad_widgets::live_id!(t), makepad_widgets::ScriptValue::NIL)],
                |vm, a| {
                    let t =
                        octoscript_render::string_prop(vm, a, makepad_widgets::live_id!(t))
                            .unwrap_or_default();
                    makepad_widgets::log!("[octoscode] nav tap: {t}");
                    crate::NAV_QUEUE.lock().unwrap().push(t);
                    makepad_widgets::SignalToUI::set_ui_signal();
                    makepad_widgets::ScriptValue::NIL
                },
            );
            vm.set_injected_global(makepad_widgets::live_id!(NAV), f);
        }
        register_splash_isolate_mod(design);
        register_splash_isolate_mod(kit);
        register_splash_isolate_mod(nav);
    });
}

/// The card's kit directory (`…/kit`, the parent of `native/<mood>/`).
fn kit_dir(card: &cards::Card) -> Result<PathBuf, String> {
    let rel = card
        .artifacts
        .kit_dir
        .clone()
        .ok_or("the manifest has no `kit_dir` for this card")?;
    Ok(cards::mounted().dir.join(rel))
}

fn read(card: &cards::Card, rel: &str) -> Result<String, String> {
    let path = cards::mounted().dir.join(rel);
    std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))
}

/// Rewrite a named `copy` entry's `en:` value in a card's ledger.
///
/// `copy t03_text { class: user-copy, en: "Fix steer queue drop on reconnect" }`
/// — only the quoted `en:` payload is replaced, so the card's authored
/// structure, classes and kit are untouched.
pub(crate) fn set_copy(card_src: &str, copy_id: &str, value: &str) -> Option<String> {
    let needle = format!("copy {copy_id} {{");
    let start = card_src.find(&needle)?;
    let rest = &card_src[start..];
    let line_end = rest.find('\n').unwrap_or(rest.len());
    let line = &rest[..line_end];
    let en_at = line.find("en:")?;
    let after = &line[en_at + 3..];
    let q1 = after.find('"')?;
    let q2 = after[q1 + 1..].find('"')? + q1 + 1;
    // Escape the value so a quote/newline in live text cannot break the ledger.
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    let new_line = format!("{}{}{}", &line[..en_at + 3 + q1 + 1], escaped, &after[q2..]);
    let out = format!(
        "{}{}{}",
        &card_src[..start],
        new_line,
        &card_src[start + line_end..]
    );
    Some(out)
}

/// Apply a whole batch of live copy rewrites (the per-screen lists
/// [`crate::screens::connect::copies`] builds). Each rewrite targets its own
/// `copy <id> {` line, so the order does not matter; an id the card does not
/// author is skipped (the same leniency as [`live_copies`]).
pub(crate) fn apply_copies(card_src: &str, values: &[(String, String)]) -> String {
    let mut src = card_src.to_owned();
    for (id, value) in values {
        if let Some(next) = set_copy(&src, id, value) {
            src = next;
        }
    }
    src
}

/// The live `copy` rewrites for one slot, from the binding values.
///
/// `values` is the module's binding resolver, so the mapping stays declarative:
/// each entry names the card's authored `copy` id and the binding id that
/// overrides it. (Card `conversation-01`'s `thread_1..5` rows carry
/// `t03_text..t07_text`; the ids here are read off the card's own `view`.)
fn live_copies(slot: cards::Slot, values: &dyn Fn(&str) -> Option<serde_json::Value>) -> Vec<(String, String)> {
    let text = |id: &str| -> Option<String> {
        match values(id) {
            Some(serde_json::Value::String(s)) => Some(s),
            Some(serde_json::Value::Null) | None => None,
            Some(other) => Some(other.to_string()),
        }
    };
    let list = |id: &str| -> Vec<serde_json::Value> {
        values(id).and_then(|v| v.as_array().cloned()).unwrap_or_default()
    };
    let mut out = Vec::new();
    match slot {
        // conversation-01 THREAD LIST. Its `KitButton` rows are
        // `thread_1..5` with `thread_N_label_text` as the label copy.
        cards::Slot::ThreadList => {
            let rows = list("threads");
            for (i, copy_id) in [
                "thread_1_label_text",
                "thread_2_label_text",
                "thread_3_label_text",
                "thread_4_label_text",
                "thread_5_label_text",
            ]
            .iter()
            .enumerate()
            {
                let Some(row) = rows.get(i) else { continue };
                let title = row
                    .get("title")
                    .and_then(|t| t.as_str())
                    .filter(|t| !t.is_empty() && *t != "(untitled)")
                    .map(str::to_owned)
                    .or_else(|| row.get("id").and_then(|v| v.as_str()).map(str::to_owned));
                if let Some(title) = title {
                    out.push(((*copy_id).to_owned(), title));
                }
            }
        }
        // conversation-03 STREAMING TURN. The user bubble is `t01+t02`; the
        // assistant prose is `assistant_md`; `t03` is the activity row.
        cards::Slot::Conversation => {
            let entries = list("timeline.entries");
            let first_of = |kind: &str| -> Option<String> {
                entries
                    .iter()
                    .filter(|e| e.get("kind").and_then(|k| k.as_str()) == Some(kind))
                    .find_map(|e| {
                        e.get("text")
                            .and_then(|t| t.as_str())
                            .filter(|t| !t.is_empty())
                            .map(str::to_owned)
                    })
            };
            if let Some(prompt) = first_of("user.message") {
                out.push(("t01_text".to_owned(), prompt));
                // The second bubble line is part of the same message: clear it
                // rather than let the design's fixture copy show through.
                out.push(("t02_text".to_owned(), String::new()));
            }
            if let Some(activity) = text("turn.activity").filter(|a| !a.is_empty()) {
                out.push(("t03_text".to_owned(), activity));
            }
            let answer = entries
                .iter()
                .filter(|e| e.get("kind").and_then(|k| k.as_str()) == Some("assistant.text"))
                .filter_map(|e| e.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n\n");
            if !answer.is_empty() {
                out.push(("assistant_md_text".to_owned(), answer));
            }
            if let Some(placeholder) = text("composer.placeholder").filter(|s| !s.is_empty()) {
                out.push(("composer_input_placeholder".to_owned(), placeholder));
            }
            out.push((
                "composer_input_text".to_owned(),
                text("composer.draft").unwrap_or_default(),
            ));
        }
        // conversation-08 COMPOSER STATES. The idle input is the live composer.
        cards::Slot::Composer => {
            if let Some(p) = text("composer.placeholder").filter(|s| !s.is_empty()) {
                out.push(("composer_idle_input_placeholder".to_owned(), p));
            }
            out.push((
                "composer_idle_input_text".to_owned(),
                text("composer.draft").unwrap_or_default(),
            ));
        }
        // conversation-04 TOOL CELLS. `tool_1..3` are `t01/t02`, `t03/t04`,
        // `t05/t06` (name, then its detail line).
        cards::Slot::ToolCells => {
            let tools = list("tools");
            for (i, (name_id, detail_id)) in [
                ("t01_text", "t02_text"),
                ("t03_text", "t04_text"),
                ("t05_text", "t06_text"),
            ]
            .iter()
            .enumerate()
            {
                let Some(t) = tools.get(i) else { continue };
                let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                let status = t.get("status").and_then(|s| s.as_str()).unwrap_or("");
                let summary = t.get("summary").and_then(|s| s.as_str()).unwrap_or("");
                out.push(((*name_id).to_owned(), name.to_owned()));
                let detail = if !summary.is_empty() {
                    summary.to_owned()
                } else if !status.is_empty() {
                    format!("\u{2022} {status}")
                } else {
                    String::new()
                };
                out.push(((*detail_id).to_owned(), detail));
            }
        }
        // conversation-09 COMPLETED ANSWER. `worked_row_label_text` is the
        // disclosure; `answer_md_text` is the answer prose.
        cards::Slot::CompletedAnswer => {
            if let Some(w) = text("answer.worked_for").filter(|s| !s.is_empty()) {
                out.push(("worked_row_label_text".to_owned(), w));
            }
            let entries = list("timeline.entries");
            let answer = entries
                .iter()
                .filter(|e| e.get("kind").and_then(|k| k.as_str()) == Some("assistant.text"))
                .filter_map(|e| e.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n\n");
            if !answer.is_empty() {
                out.push(("answer_md_text".to_owned(), answer));
            }
        }
    }
    out
}

/// Lower one slot's card to Splash DSL, with the live binding values injected.
///
/// `Err` names the reason (missing artefact, failed realize, failed lowering) —
/// the caller keeps the per-slot fallback and logs the slot by name.
pub fn lower_slot(
    slot: cards::Slot,
    values: &dyn Fn(&str) -> Option<serde_json::Value>,
) -> Result<String, String> {
    let card = cards::card_for_slot(slot)
        .ok_or_else(|| format!("no card declares slot {}", slot.name()))?;
    let mut card_src = read(card, &card.artifacts.card)?;
    let data_text = read(card, &card.artifacts.data).unwrap_or_else(|_| "{}".to_owned());
    let data: serde_json::Value =
        serde_json::from_str(&data_text).map_err(|e| format!("parse {}: {e}", card.artifacts.data))?;

    // Inject the live values into the card's own authored `copy` block.
    for (copy_id, value) in live_copies(slot, values) {
        if let Some(next) = set_copy(&card_src, &copy_id, &value) {
            card_src = next;
        }
    }

    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &kit_dir(card)?)?;

    // EVERY card names its nodes from its own ledger (`page`, `t01`,
    // `thread_1`, …), so five cards in ONE isolate collide: the second
    // `set_text` re-binds `page`/`t01` and most of its subtree never lays out.
    // `inspectable` renames each node to a positional path, and the accepted
    // Gate-B render did exactly this before lowering
    // (`octoscript-makepad/apps/kit-host/src/beauty.rs:90,121` — the ordered
    // `elements` + `beauty_0_3_1` ids in `conversation-01-snap-v4.json`).
    // We use the SLOT as the prefix so all five cards coexist.
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let ui = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&tree))?;
    let prefix = slot_prefix(slot);
    let ui = ui.replace("beauty_0", prefix);
    // The card's ledger is a FIXED 406x776 artboard (`page := DesignSurface {
    // width: 406 height: 776 ...`) whose children sit at MEASURED `abs_pos`.
    // That is the size the Gate-B render captured, so the slot must give the
    // card its own 406x776 box — a smaller box clips the absolutely-positioned
    // children (measured: a 384x109 slot left 20 of 23 nodes at zero geometry).
    // Left at its natural size, each slot is the card exactly as Gate-B drew it.
    Ok(ui)
}

/// The id prefix a slot's nodes get, so several cards can share one isolate
/// without re-binding each other's ids.
pub fn slot_prefix(slot: cards::Slot) -> &'static str {
    match slot {
        cards::Slot::ThreadList => "c01",
        cards::Slot::Conversation => "c03",
        cards::Slot::ToolCells => "c04",
        cards::Slot::Composer => "c08",
        cards::Slot::CompletedAnswer => "c09",
    }
}

/// The full Splash body for one slot: the lowered card, wrapped the way
/// card-host wraps it (`host.rs:131-133`) so the DSL continues the isolate's
/// prelude `View`.
pub fn slot_body(
    slot: cards::Slot,
    values: &dyn Fn(&str) -> Option<serde_json::Value>,
) -> Result<String, String> {
    // card-host fills the whole window (`height:Fill`, its `host.rs:133`) because
    // there the card IS the window. Here the card shares a tile, so it keeps its
    // own height and the slot scrolls: `height:Fit` lets the wrapper take the
    // artboard's 776 rather than the viewport's height.
    lower_slot(slot, values).map(|ui| format!("width:Fill height:Fit flow:Overlay {ui}"))
}

/// The binding ids a mounted card's copy rewrite can consume (the audit the
/// coverage test checks).
pub fn live_binding_ids(slot: cards::Slot) -> Vec<String> {
    live_copies(slot, &|_| None).into_iter().map(|(c, _)| c).collect()
}

/// `bindings::query` as the resolver shape the renderer wants.
pub fn resolver<'a>(
    ctx: &'a bindings::Ctx<'a>,
) -> impl Fn(&str) -> Option<serde_json::Value> + 'a {
    move |id| bindings::query(ctx, id)
}
