//! The **fallback** renderer (card #12 §4) — plain Makepad widgets, not a
//! design-flow card.
//!
//! It exists so the live gate can run before the L0 cards (#11b) land: a
//! thread list, a timeline (user / assistant / tool entries) and a composer
//! with send + stop.
//!
//! **Binding-only (8.8 condition 2).** Every value it renders comes from
//! [`crate::bindings::query`] by **binding id**; every control it offers emits
//! a binding **action id** ([`ACTION_SEND`] / [`ACTION_STOP`]) the module
//! performs. It never touches a `Store`, a `Conversation` or any other Rust
//! type — it speaks ids, which is exactly what a card will do. It is replaced
//! by the mounted L0 cards later.
//!
//! It is a renderer (not its own `Widget`) so the module keeps **one**
//! `script_mod!` block: makepad's `script_mod!` defines widgets into the VM at
//! the point the macro's generated function runs, and a second block in a
//! second file would not be invoked by the module's `register`.
use makepad_widgets::*;
use serde_json::Value;

/// The action ids this renderer can emit. A card that needed more would
/// declare them here — never reach past the binding table.
pub const ACTION_SEND: &str = crate::bindings::ACTION_SUBMIT;
/// The composer's STOP (an in-flight turn).
pub const ACTION_STOP: &str = crate::bindings::ACTION_INTERRUPT;

/// Re-read every binding this view renders and push it into the widgets.
///
/// `values` is the module's binding resolver ([`crate::bindings::query`]), so
/// this renderer stays pure and testable: give it any resolver and it renders
/// whatever the binding ids resolve to.
pub fn render(view: &View, cx: &mut Cx, values: &dyn Fn(&str) -> Option<Value>) {
    let as_text = |id: &str| -> String {
        match values(id) {
            Some(Value::String(s)) => s,
            Some(Value::Null) | None => String::new(),
            Some(other) => other.to_string(),
        }
    };

    // conversation-01 THREAD LIST — binding `threads` (rows of {id,title,…}).
    let threads = values("threads")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let threads_text = if threads.is_empty() {
        "threads: (none)".to_owned()
    } else {
        let mut out = format!("threads: {}", threads.len());
        for row in threads.iter().take(5) {
            let title = row.get("title").and_then(|t| t.as_str()).unwrap_or("(untitled)");
            out.push_str(&format!("\n· {title}"));
        }
        out
    };
    view.label(cx, ids!(threads_label)).set_text(cx, &threads_text);

    // conversation-03/09 TIMELINE — binding `timeline.entries` (kind + text).
    let entries = values("timeline.entries")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let timeline_text = if entries.is_empty() {
        "(no timeline)".to_owned()
    } else {
        entries
            .iter()
            .map(|e| {
                let kind = e.get("kind").and_then(|k| k.as_str()).unwrap_or("?");
                let text = e.get("text").and_then(|t| t.as_str()).unwrap_or("");
                format!("[{kind}] {text}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    view.label(cx, ids!(timeline_label)).set_text(cx, &timeline_text);

    // conversation-04 TOOL CELLS — binding `tools` (name, summary, status).
    let tools = values("tools").and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let tools_text = if tools.is_empty() {
        "tools: (none)".to_owned()
    } else {
        let mut out = format!("tools: {}", tools.len());
        for t in &tools {
            let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let status = t.get("status").and_then(|s| s.as_str()).unwrap_or("?");
            let summary = t.get("summary").and_then(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!("\n· {name} [{status}] {summary}"));
        }
        out
    };
    view.label(cx, ids!(tools_label)).set_text(cx, &tools_text);

    // conversation-09 COMPLETED ANSWER — binding `answer.worked_for`.
    view.label(cx, ids!(answer_label)).set_text(cx, &as_text("answer.worked_for"));

    // conversation-08 COMPOSER — binding `composer.placeholder`.
    let placeholder = as_text("composer.placeholder");
    if !placeholder.is_empty() {
        view.text_input(cx, ids!(draft)).set_empty_text(cx, placeholder);
    }

    // The composer's STOP vs send follows `turn.active` (a binding) — the
    // renderer reads the id, it does not consult any turn state of its own.
    let active = values("turn.active").and_then(|v| v.as_bool()).unwrap_or(false);
    view.button(cx, ids!(send)).set_text(cx, if active { crate::i18n::tr_ctx("verb", "Queue") } else { crate::i18n::tr("Send") });
    view.button(cx, ids!(stop)).set_text(cx, crate::i18n::tr("Stop"));
}
