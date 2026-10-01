//! A5 — the native **dialog host**: the Stage-B screens the shell could lower
//! but no user could open or click.
//!
//! The nine cards here (Models `setup-07`, Context `setup-09`, Skills
//! `setup-10`, Goal / Loops / Monitors `autonomy-03/04/05`, Fleet
//! `autonomy-06`, Tasks `autonomy-07`, Code review `autonomy-02`) each had a
//! lowerer and an action table, but they mounted only behind
//! `OCTOSCODE_SCREEN` / `OCTOSCODE_CHROME` (a developer switch), and they
//! mounted with `to_makepad_ui`, whose `abs_pos` is the **OS window** origin:
//! inside the OctoSense shell the OctosCode window starts at x=54, so every
//! card was cut at its left edge and drawn over the conversation without a
//! backdrop. Their buttons were wired by matching atlas `source_bounds`
//! against the emitted `abs_pos` (1.5 px tolerance), which silently wired
//! nothing on the cards whose two atlas passes disagree (goal: 4 / 16 px), and
//! the per-row loop/monitor/peer controls are `Svg`/`Text` nodes that cannot
//! carry a handler at all. With an empty store the cards also kept their
//! authored SAMPLE copy ("3 models", "System 8k", a fake goal), which the
//! dialog replaces with the live value or the web's own empty-state line.
//!
//! ## Web oracle
//!
//! Every one of these surfaces is a modal dialog in the web app: a dimmed
//! backdrop (`--dsw-alias-bg-mask-1`), the dialog centred
//! (`place-items: center; padding: 16px`), `width: min(640|760px, 100%)`,
//! `max-height: calc(100dvh - 32px); overflow: auto`
//! (`features/context/ContextPanel.module.css` `.backdrop`/`.dialog`,
//! `AutonomyDialog.module.css:1-19`, `SkillsDialog.module.css:1-18`,
//! `NativeReviewDialog.module.css:1-18`), with Escape closing it
//! (`ui/ModalSurface.tsx` `onEscape`). They open from slash commands
//! (`App.tsx:1339-1495`: `context`, `autonomy` for /goal /loop /monitor
//! /agents, `peers`, `native-review`, `skills`, `models`), and each loads its
//! data when it opens.
//!
//! ## What this module owns
//!
//! * the open dialog (UI-local state, like the palette's selection);
//! * the command → dialog table ([`for_command`]);
//! * the **slot-relative lowering**: the card's tree is lowered with
//!   `to_makepad_ui_in_slot` (parent-relative margins, the path
//!   `components.rs` already uses for the conversation rows), so the card
//!   seats inside the dialog wherever the dialog is;
//! * **wiring by node id**, not by position: a `Button` node gets its
//!   `tapto` (the renderer emits `on_click: || { NAV(t: …) }` for it), and a
//!   control drawn as an `Svg`/`Text` (loop/monitor icons, `Clear goal`, the
//!   fleet `Steer` link, `Remove`) gets a transparent hit target over its own
//!   bounds — the control the design already draws becomes clickable; nothing
//!   new is drawn;
//! * the live-state edits the cards lacked (empty states, per-status icons and
//!   labels, the context breakdown rows, the compaction-mode selection);
//! * the chrome: backdrop, centred frame, vertical scroll, a close button.
//!
//! The host (`lib.rs`) only mounts [`Mounted::dsl`] into its `dialog_splash`
//! and routes [`Mounted::taps`] through the same `perform_action` table every
//! other card tap uses (one owner per action id).
use std::sync::Mutex;

use octoscript_render::{Attrs, NodeKind, UiNode};
use serde_json::Value;

use crate::bindings::Ctx;
use crate::screens::autonomy::AutonomyState;

// ------------------------------------------------------------------ the set

/// One dialog the host can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    Models,
    Context,
    Skills,
    Goal,
    Loops,
    Monitors,
    Fleet,
    Tasks,
    Review,
}

pub const ALL: &[Dialog] = &[
    Dialog::Models,
    Dialog::Context,
    Dialog::Skills,
    Dialog::Goal,
    Dialog::Loops,
    Dialog::Monitors,
    Dialog::Fleet,
    Dialog::Tasks,
    Dialog::Review,
];

impl Dialog {
    /// The stable id the action ids and logs use (`dialog.open.<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Dialog::Models => "models",
            Dialog::Context => "context",
            Dialog::Skills => "skills",
            Dialog::Goal => "goal",
            Dialog::Loops => "loops",
            Dialog::Monitors => "monitors",
            Dialog::Fleet => "fleet",
            Dialog::Tasks => "tasks",
            Dialog::Review => "review",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        ALL.iter().copied().find(|d| d.id() == id)
    }

    /// The Stage-B card this dialog shows.
    pub fn card(self) -> &'static str {
        match self {
            Dialog::Models => "setup-07",
            Dialog::Context => "setup-09",
            Dialog::Skills => "setup-10",
            Dialog::Goal => "autonomy-03",
            Dialog::Loops => "autonomy-04",
            Dialog::Monitors => "autonomy-05",
            Dialog::Fleet => "autonomy-06",
            Dialog::Tasks => "autonomy-07",
            Dialog::Review => "autonomy-02",
        }
    }

    /// The accessible name (the web's dialog `<h2>`).
    pub fn title(self) -> &'static str {
        match self {
            Dialog::Models => "Models",
            Dialog::Context => "Context",
            Dialog::Skills => "Skills",
            Dialog::Goal => "Goal",
            Dialog::Loops => "Loops",
            Dialog::Monitors => "Monitors",
            Dialog::Fleet => "Fleet",
            Dialog::Tasks => "Tasks",
            Dialog::Review => "Code review",
        }
    }

    /// The action ids the host performs when the dialog opens — the web loads
    /// each dialog's data on open (`AutonomyPanel` `controller.refresh()` on
    /// mount, the settings models section's `profile/llm/list`, the skills
    /// dialog's `profile/skills/list`, the context dialog's authoritative
    /// status read, the fleet's `peer/gather`). Every id has one owner.
    pub fn on_open(self) -> &'static [&'static str] {
        match self {
            Dialog::Models | Dialog::Skills => &[ACTION_REFRESH_PROFILE],
            Dialog::Context => &[ACTION_REFRESH_CONTEXT],
            Dialog::Goal => &["goal.refresh"],
            Dialog::Loops | Dialog::Monitors => &["loops.refresh"],
            Dialog::Fleet | Dialog::Tasks => &[ACTION_REFRESH_FLEET],
            Dialog::Review => &[],
        }
    }
}

/// The web's command → surface table (`registry.ts` `intent`, dispatched in
/// `App.tsx:1339-1495`), restricted to the surfaces this host shows. Aliases
/// are the web's (`registry.ts:331` context: ctx/compact/compress, skills:
/// skill, review: code-review, agents: agent, ps: tasks).
pub fn for_command(name: &str) -> Option<Dialog> {
    let name = name.trim().trim_start_matches('/').to_ascii_lowercase();
    Some(match name.as_str() {
        "model" | "models" => Dialog::Models,
        "context" | "ctx" | "compact" | "compress" => Dialog::Context,
        "skills" | "skill" => Dialog::Skills,
        // `agents` and `goal` share the web's `autonomy` intent; the goal is
        // the dialog's first section (AutonomyPanel.tsx:114).
        "goal" | "agents" | "agent" => Dialog::Goal,
        "loop" | "loops" => Dialog::Loops,
        "monitor" | "monitors" => Dialog::Monitors,
        "peer" | "peers" | "fleet" => Dialog::Fleet,
        "ps" | "tasks" => Dialog::Tasks,
        "review" | "code-review" => Dialog::Review,
        _ => return None,
    })
}

// -------------------------------------------------------------------- state

static OPEN: Mutex<Option<Dialog>> = Mutex::new(None);

/// The dialog that is open, if any.
pub fn current() -> Option<Dialog> {
    *OPEN.lock().unwrap()
}

/// Open `d` (replacing any open dialog — the web opens one product surface at
/// a time; a second command swaps it).
pub fn open(d: Dialog) {
    *OPEN.lock().unwrap() = Some(d);
}

/// Close the open dialog (Escape / the close button).
pub fn close() {
    *OPEN.lock().unwrap() = None;
}

pub const ACTION_CLOSE: &str = "dialog.close";
pub const ACTION_REFRESH_PROFILE: &str = "dialog.refresh.profile";
pub const ACTION_REFRESH_CONTEXT: &str = "dialog.refresh.context";
pub const ACTION_REFRESH_FLEET: &str = "dialog.refresh.fleet";

/// The host's own action ids: `dialog.close`, `dialog.open.<id>` and the
/// three on-open refreshes that have no other owner.
pub fn is_action(id: &str) -> bool {
    !matches!(resolve(id), Effect::Unhandled(_))
}

/// The palette's effect id that opens `d`.
pub fn open_action(d: Dialog) -> String {
    format!("dialog.open.{}", d.id())
}

/// What a host action means. Pure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Open(Dialog),
    Close,
    /// `profile/llm/list` + `profile/skills/list` + `profile/sub_providers/list`
    /// (`screens::models::refresh`).
    RefreshProfile,
    /// The authoritative `session/status/read` (`ContextDialog`'s refresh).
    RefreshContext,
    /// `task/list` + `peer/gather` (`screens::fleet::refresh`).
    RefreshFleet,
    Unhandled(String),
}

pub fn resolve(id: &str) -> Effect {
    if id == ACTION_CLOSE {
        return Effect::Close;
    }
    if let Some(d) = id.strip_prefix("dialog.open.").and_then(Dialog::from_id) {
        return Effect::Open(d);
    }
    match id {
        ACTION_REFRESH_PROFILE => Effect::RefreshProfile,
        ACTION_REFRESH_CONTEXT => Effect::RefreshContext,
        ACTION_REFRESH_FLEET => Effect::RefreshFleet,
        other => Effect::Unhandled(other.to_owned()),
    }
}

/// Apply the UI-local half of an effect (open/close); the transport halves
/// are the host's. Returns the dialog that was opened, so the host can run its
/// [`Dialog::on_open`] loads.
pub fn apply(effect: &Effect) -> Option<Dialog> {
    match effect {
        Effect::Open(d) => {
            open(*d);
            Some(*d)
        }
        Effect::Close => {
            close();
            None
        }
        _ => None,
    }
}

/// Whether the server advertises `name`: a METHOD (`a/b`) is read from the
/// negotiated method list (`config.supported_methods`, where the transport
/// puts methods — `autonomy.rs` #P4e1b F1), a FEATURE (`a.b.v1`) from the
/// capability list. The capability list is consulted for a method too, so a
/// store seeded the older way still answers.
pub fn advertises(store: &octoscode_store::Store, name: &str) -> bool {
    store.capabilities().iter().any(|c| c == name)
        || (name.contains('/')
            && store
                .domains
                .config
                .supported_methods()
                .iter()
                .any(|m| m == name))
}

// ----------------------------------------------------------------- the trees

/// Restore the authored ids `l0::inspectable` replaced with positional ones,
/// from the inventory it returned (`id` → `original_id`).
fn restore_ids(tree: &mut UiNode, inventory: &[Value]) {
    let map: std::collections::HashMap<String, String> = inventory
        .iter()
        .filter_map(|v| {
            Some((
                v.get("id")?.as_str()?.to_owned(),
                v.get("original_id")?.as_str()?.to_owned(),
            ))
        })
        .collect();
    walk_mut(tree, &mut |n| {
        if let Some(id) = n.attrs.id.as_deref() {
            if let Some(orig) = map.get(id) {
                n.attrs.id = Some(orig.clone());
            }
        }
    });
}

fn screen3(d: Dialog) -> Option<crate::screens::autonomy::Screen3> {
    use crate::screens::autonomy::Screen3;
    Some(match d {
        Dialog::Goal => Screen3::Goal,
        Dialog::Loops => Screen3::Loops,
        Dialog::Monitors => Screen3::Monitors,
        _ => return None,
    })
}

/// The card's prepared tree with the live values applied, its authored ids
/// intact (before `inspectable`).
pub fn card_tree(d: Dialog, ctx: &Ctx<'_>, st: &AutonomyState) -> Result<UiNode, String> {
    match d {
        Dialog::Models | Dialog::Context | Dialog::Skills => {
            let (src, data, kit) = crate::screens::models::lower_card_src(d.card(), ctx)?;
            let prepared = octoscript_makepad::l0::prepare(&src, &data, &kit)
                .map_err(|e| format!("prepare {}: {e}", d.card()))?;
            Ok(prepared.tree)
        }
        Dialog::Goal | Dialog::Loops | Dialog::Monitors => {
            let screen = screen3(d).expect("an autonomy dialog");
            let lowered = crate::screens::autonomy::lower_tree(screen, st)?;
            let mut tree = lowered.card.tree;
            restore_ids(&mut tree, &lowered.inventory);
            Ok(tree)
        }
        Dialog::Fleet | Dialog::Tasks => crate::screens::fleet::lower_tree(d.card(), ctx),
        Dialog::Review => crate::screens::review::lower_tree(d.card(), ctx).map(|(t, _)| t),
    }
}

// ------------------------------------------------------------ tree helpers

fn walk_mut(n: &mut UiNode, f: &mut impl FnMut(&mut UiNode)) {
    f(n);
    for c in &mut n.children {
        walk_mut(c, f);
    }
}

fn walk(n: &UiNode, f: &mut impl FnMut(&UiNode)) {
    f(n);
    for c in &n.children {
        walk(c, f);
    }
}

pub fn find<'a>(n: &'a UiNode, id: &str) -> Option<&'a UiNode> {
    if n.attrs.id.as_deref() == Some(id) {
        return Some(n);
    }
    n.children.iter().find_map(|c| find(c, id))
}

fn find_mut<'a>(n: &'a mut UiNode, id: &str) -> Option<&'a mut UiNode> {
    if n.attrs.id.as_deref() == Some(id) {
        return Some(n);
    }
    n.children.iter_mut().find_map(|c| find_mut(c, id))
}

/// `(x, y, w, h)` of a node (artboard coordinates).
pub fn rect(n: &UiNode) -> (f64, f64, f64, f64) {
    let a = &n.attrs;
    (
        a.x.unwrap_or(0.0),
        a.y.unwrap_or(0.0),
        a.w.unwrap_or(0.0) as f64,
        a.h.unwrap_or(0.0) as f64,
    )
}

fn rect_of(tree: &UiNode, id: &str) -> Option<(f64, f64, f64, f64)> {
    find(tree, id).map(rect)
}

/// Remove every node whose id is in `ids` (with its subtree).
fn remove(tree: &mut UiNode, ids: &[&str]) {
    tree.children
        .retain(|c| !c.attrs.id.as_deref().is_some_and(|id| ids.contains(&id)));
    for c in &mut tree.children {
        remove(c, ids);
    }
}

/// Remove every node whose id starts with one of `prefixes`.
fn remove_prefixed(tree: &mut UiNode, prefixes: &[&str]) {
    tree.children.retain(|c| {
        !c.attrs
            .id
            .as_deref()
            .is_some_and(|id| prefixes.iter().any(|p| id.starts_with(p)))
    });
    for c in &mut tree.children {
        remove_prefixed(c, prefixes);
    }
}

fn set_text(tree: &mut UiNode, id: &str, text: &str) {
    if let Some(n) = find_mut(tree, id) {
        n.attrs.text = Some(text.to_owned());
    }
}

/// Shift a node AND its subtree (children carry artboard coordinates too).
fn shift(n: &mut UiNode, dx: f64, dy: f64) {
    walk_mut(n, &mut |m| {
        if let Some(x) = m.attrs.x.as_mut() {
            *x += dx;
        }
        if let Some(y) = m.attrs.y.as_mut() {
            *y += dy;
        }
    });
}

fn shift_id(tree: &mut UiNode, id: &str, dx: f64, dy: f64) {
    if let Some(n) = find_mut(tree, id) {
        shift(n, dx, dy);
    }
}

/// Shift every top-level-of-its-container node whose top is at or below `y0`
/// (and its subtree), skipping frame containers. Used to close the gap a
/// removed section leaves.
fn shift_below(tree: &mut UiNode, y0: f64, dy: f64, frames: &[String]) {
    fn go(n: &mut UiNode, y0: f64, dy: f64, frames: &[String]) {
        for c in &mut n.children {
            let is_frame = c.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id));
            if is_frame {
                go(c, y0, dy, frames);
            } else if c.attrs.y.unwrap_or(0.0) >= y0 - 0.5 {
                shift(c, 0.0, dy);
            } else {
                go(c, y0, dy, frames);
            }
        }
    }
    go(tree, y0, dy, frames);
}

fn set_h(tree: &mut UiNode, id: &str, h: f64) {
    if let Some(n) = find_mut(tree, id) {
        n.attrs.h = Some(h as f32);
    }
}

/// Make a text node right-aligned inside a box ending at `right`, `w` wide,
/// so a live value of any length stays flush with the authored right edge.
fn right_align(tree: &mut UiNode, id: &str, right: f64, w: f64) {
    if let Some(n) = find_mut(tree, id) {
        n.attrs.x = Some(right - w);
        n.attrs.w = Some(w as f32);
        n.attrs.alignx = Some(1.0);
    }
}

/// The frame containers: the root and a direct child that spans the artboard
/// (`goal_screen`, `loops_screen`, …). Everything else is content.
fn frame_ids(tree: &UiNode) -> Vec<String> {
    let (_, _, rw, rh) = rect(tree);
    let mut out: Vec<String> = tree.attrs.id.iter().cloned().collect();
    for c in &tree.children {
        let (_, _, w, h) = rect(c);
        if c.kind == NodeKind::Stack && w >= rw * 0.9 && h >= rh * 0.85 {
            if let Some(id) = &c.attrs.id {
                out.push(id.clone());
            }
        }
    }
    out
}

// --------------------------------------------------------- live transforms

/// The web's family label (`model-management-projection.ts:207-215`) is what
/// the bindings already compose; this only decides which provider cards
/// exist. Distinct providers, in store order.
fn provider_groups(store: &octoscode_store::Store) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    for m in store.domains.profile.llm_models() {
        if !groups.contains(&m.provider) {
            groups.push(m.provider);
        }
    }
    groups
}

/// setup-07 — one provider card per live provider (max three, the card's
/// slots); the selected model's check follows the selection; no models →
/// the empty line. The atlas's vertical scroll rule (`vline`) is chrome of the
/// phone frame, not of a dialog.
fn live_models(tree: &mut UiNode, ctx: &Ctx<'_>) {
    remove(tree, &["vline"]);
    let groups = provider_groups(ctx.store);
    const KIMI: &[&str] = &["card_kimi", "t_kimi_head", "t_kimi_count", "dot_kimi", "icon_chev_kimi"];
    const GLM: &[&str] = &["card_glm", "t_glm_head", "t_glm_count", "dot_glm", "icon_chev_glm"];
    const FIRST: &[&str] = &[
        "card_deepseek",
        "t_ds_head",
        "inner_card",
        "inner_div",
        "dot_ds",
        "icon_chev_ds",
        "t_flash",
        "t_pro",
        "icon_check",
        "btn_test",
        "btn_discover",
    ];
    if groups.len() < 3 {
        remove(tree, GLM);
    }
    if groups.len() < 2 {
        remove(tree, KIMI);
    }
    // A live head ("{family} • {route}") is longer than the atlas sample:
    // its box runs to the status dot, not the sample's measured width.
    for (head, dot) in [("t_ds_head", "dot_ds"), ("t_kimi_head", "dot_kimi"), ("t_glm_head", "dot_glm")] {
        if let (Some((hx, _, _, _)), Some((dx, _, _, _))) = (rect_of(tree, head), rect_of(tree, dot)) {
            if let Some(n) = find_mut(tree, head) {
                n.attrs.w = Some((dx - 12.0 - hx) as f32);
            }
        }
    }
    if groups.is_empty() {
        remove(tree, FIRST);
        // The count line keeps its grey body style and carries the empty
        // state, directly under the title.
        let title_bottom = rect_of(tree, "t_title").map(|(_, y, _, h)| y + h).unwrap_or(86.0);
        if let Some(n) = find_mut(tree, "t_ds_count") {
            n.attrs.text = Some("No models are configured for this Profile.".to_owned());
            n.attrs.y = Some(title_bottom + 24.0);
            n.attrs.w = Some(320.0);
        }
        return;
    }
    // The expanded provider's rows (t_flash = row 0, t_pro = row 1); the
    // lowering already blanks a missing second row — drop the empty node.
    let first = &groups[0];
    let mine: Vec<_> = ctx
        .store
        .domains
        .profile
        .llm_models()
        .into_iter()
        .filter(|m| &m.provider == first)
        .collect();
    if mine.len() < 2 {
        remove(tree, &["t_pro", "inner_div"]);
    }
    match mine.iter().position(|m| m.selected) {
        Some(0) => {}
        Some(1) => {
            let pro_y = rect_of(tree, "t_pro").map(|r| r.1);
            let flash_y = rect_of(tree, "t_flash").map(|r| r.1);
            if let (Some(p), Some(f)) = (pro_y, flash_y) {
                shift_id(tree, "icon_check", 0.0, p - f);
            }
        }
        _ => remove(tree, &["icon_check"]),
    }
}

/// The confirmed compaction mode (`session/compact/mode/set` read-back).
fn compact_mode(ctx: &Ctx<'_>) -> Option<String> {
    let session = ctx.store.active_session().unwrap_or_default();
    crate::screens::models::compact_mode(&session)
}

fn fmt_count(v: u64) -> String {
    // `toLocaleString()` grouping (en): 1,234,567.
    let s = v.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// setup-09 — the web's ContextPanel facts in the card's three fact rows
/// (`ContextPanel.tsx`: estimate · items, generation, recovery — the authored
/// "System / Conversation / Tools" split has no wire source), the occupancy
/// bar filled to the real percentage, the compaction line
/// ("Compacting context · trigger" / "Last compaction: …"), and the
/// server-confirmed compaction mode selected.
fn live_context(tree: &mut UiNode, ctx: &Ctx<'_>) {
    let session = ctx.store.active_session().unwrap_or_default();
    let life = ctx.store.domains.session.context(&session);
    let state = life.as_ref().map(|l| l.state.clone()).unwrap_or(Value::Null);
    // The bar: track width × the bound percentage; unknown → no fill.
    let pct = crate::screens::models::query_binding(ctx, "context.pct")
        .and_then(|v| v.as_str().map(str::to_owned))
        .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok());
    match (pct, rect_of(tree, "bar_track")) {
        (Some(p), Some((_, _, tw, _))) => {
            if let Some(n) = find_mut(tree, "bar_fill") {
                n.attrs.w = Some((tw * (p / 100.0).clamp(0.0, 1.0)) as f32);
            }
        }
        _ => remove(tree, &["bar_fill"]),
    }
    let item = |k: &str| state.get(k).cloned().unwrap_or(Value::Null);
    let rows: [(&str, &str, &str, String); 3] = [
        (
            "t_row3",
            "t_val6",
            "Items",
            item("item_count").as_u64().map(fmt_count).unwrap_or_else(|| "—".into()),
        ),
        (
            "t_row4",
            "t_val7",
            "Generation",
            item("generation").as_u64().map(|g| g.to_string()).unwrap_or_else(|| "—".into()),
        ),
        (
            "t_row5",
            "t_val8",
            "Recovery",
            item("recovery_state").as_str().map(str::to_owned).unwrap_or_else(|| "—".into()),
        ),
    ];
    // The values right-align at the bar's right edge (the authored values end
    // at x≈363, the track's own right edge).
    let right = rect_of(tree, "bar_track").map(|(x, _, w, _)| x + w).unwrap_or(362.0);
    for (label, value, l, v) in rows {
        set_text(tree, label, l);
        set_text(tree, value, &v);
        right_align(tree, value, right, 170.0);
        // The label box was measured for the atlas word ("Tools", 44px).
        if let Some(n) = find_mut(tree, label) {
            n.attrs.w = Some(150.0);
        }
    }
    // The compaction line, where the card authored "Keeps the last 4 turns".
    // Kinds as the client's lifecycle handlers fold them
    // (`domains/session.rs` ContextCompaction*Handler); the authoritative
    // status read carries the last compaction as its detail.
    let detail = life.as_ref().and_then(|l| l.detail.clone()).unwrap_or(Value::Null);
    let kind = life.as_ref().map(|l| l.kind.trim_start_matches("context/").to_owned());
    let line = match kind.as_deref() {
        Some("compaction_started") => Some(format!(
            "Compacting context · {}",
            detail.get("trigger").and_then(|t| t.as_str()).unwrap_or("manual")
        )),
        _ if detail.get("token_estimate_before").is_some() || detail.get("compaction").is_some() => {
            let c = detail.get("compaction").cloned().unwrap_or(detail.clone());
            let before = c.get("token_estimate_before").and_then(|v| v.as_u64());
            let after = c.get("token_estimate_after").and_then(|v| v.as_u64());
            let status = c.get("status").and_then(|v| v.as_str()).unwrap_or("completed");
            Some(format!(
                "Last compaction: {status} · {} → {} tokens",
                before.map(fmt_count).unwrap_or_else(|| "—".into()),
                after.map(fmt_count).unwrap_or_else(|| "not reported".into()),
            ))
        }
        _ => None,
    };
    match line {
        Some(l) => {
            if let Some(n) = find_mut(tree, "t_keep") {
                n.attrs.text = Some(l);
                // Centred under the button across the card width.
                n.attrs.x = Some(35.0);
                n.attrs.w = Some(328.0);
                n.attrs.alignx = Some(0.5);
            }
        }
        None => remove(tree, &["t_keep"]),
    }
    // The compaction mode: the confirmed one is selected (ink + weight); an
    // unconfirmed mode selects neither half (the web's select starts empty).
    let mode = compact_mode(ctx);
    for (id, m) in [("t_llm", "llm"), ("t_heur", "heuristic")] {
        if let Some(n) = find_mut(tree, id) {
            let on = mode.as_deref() == Some(m);
            n.attrs.color = Some(if on { 0xff00_0000 } else { 0xff6e_6e73 });
            n.attrs.weight = Some(if on { 600 } else { 400 });
        }
    }
    if let (Some(m), Some((bx, by, bw, bh)), Some((dx, _, _, _))) =
        (mode.as_deref(), rect_of(tree, "seg_box"), rect_of(tree, "seg_div"))
    {
        // The selected half's fill, inset inside the pill (the segmented
        // control's own selected state; seg-control component, #F0F0F2).
        let (x0, x1) = if m == "llm" { (bx + 4.0, dx - 3.0) } else { (dx + 4.5, bx + bw - 4.0) };
        let mut a = Attrs::default();
        a.id = Some("seg_sel".into());
        a.x = Some(x0);
        a.y = Some(by + 4.0);
        a.w = Some((x1 - x0) as f32);
        a.h = Some((bh - 8.0) as f32);
        a.bg = Some(0xffef_eff1);
        a.radius = Some(((bh - 8.0) / 2.0) as f32);
        a.variant = Some("surface".into());
        insert_after(tree, "seg_div", UiNode { kind: NodeKind::Stack, attrs: a, children: vec![] });
    }
}

/// setup-10 — one installed row per skill (three slots), one registry row per
/// fetched package (two slots); the sections shrink to their rows and the
/// web's empty lines replace the samples ("No skills installed in this
/// Profile.", `SkillsDialog.tsx:176`).
fn live_skills(tree: &mut UiNode, ctx: &Ctx<'_>) {
    let frames = frame_ids(tree);
    let installed = ctx.store.domains.profile.installed_skills();
    let registry = ctx.store.domains.profile.registry_packages();
    const PITCH: f64 = 64.0;
    let n = installed.len().min(3);
    // Rows n..3 go (name, version, remove, and the divider above each).
    let row_ids = |i: usize| -> Vec<String> {
        vec![
            format!("t_name{}", 3 + i),
            format!("t_ver{}", 6 + i),
            format!("t_remove{i}"),
            format!("div_{i}"),
        ]
    };
    let mut drop: Vec<String> = Vec::new();
    for i in n.max(1)..3 {
        drop.extend(row_ids(i));
    }
    if n == 0 {
        drop.extend(["t_ver6".to_owned(), "t_remove0".to_owned()]);
    }
    let drop_refs: Vec<&str> = drop.iter().map(String::as_str).collect();
    remove(tree, &drop_refs);
    if n == 0 {
        if let Some(t) = find_mut(tree, "t_name3") {
            t.attrs.text = Some("No skills installed in this Profile.".to_owned());
            t.attrs.w = Some(320.0);
            t.attrs.color = Some(0xff6e_6e73);
        }
    }
    let rows_shown = n.max(1);
    let shrink = (3 - rows_shown) as f64 * PITCH;
    if shrink > 0.0 {
        if let Some((_, y, _, h)) = rect_of(tree, "card_installed") {
            set_h(tree, "card_installed", h - shrink);
            shift_below(tree, y + h - 1.0, -shrink, &frames);
        }
    }
    // Registry: fetched packages only (search results land in the store).
    let m = registry.len().min(2);
    if m == 0 {
        remove(
            tree,
            &[
                "t_reg_head", "card_registry", "t_name10", "t_ver11", "btn_3_install", "t_name12",
                "t_ver13", "btn_4_install", "div_3",
            ],
        );
    } else if m == 1 {
        remove(tree, &["t_name12", "t_ver13", "btn_4_install", "div_3"]);
        if let Some((_, _, _, h)) = rect_of(tree, "card_registry") {
            set_h(tree, "card_registry", h - 70.0);
        }
    }
    // Versions right after the name column; "—" when not reported.
    for i in 0..n {
        let id = format!("t_ver{}", 6 + i);
        if installed[i].version.is_none() {
            set_text(tree, &id, "—");
        }
    }
}

/// The goal's status word (`describeGoalStatus`, AutonomyPanel) and its badge
/// colours: active green (the authored badge), paused/blocked amber, terminal
/// grey.
fn goal_badge(status: &str) -> (&'static str, u32, u32) {
    match status {
        "active" => ("Active", 0xffe6_f6ea, 0xff28_7f3b),
        "paused" => ("Paused", 0xffff_f4e5, 0xff8a_5a00),
        "budget_limited" => ("Budget limited", 0xffff_f4e5, 0xff8a_5a00),
        "blocked" => ("Blocked", 0xffff_f4e5, 0xff8a_5a00),
        "complete" | "completed" => ("Complete", 0xfff2_f2f4, 0xff61_666b),
        _ => ("Unknown", 0xfff2_f2f4, 0xff61_666b),
    }
}

/// autonomy-03 — no goal → the web's "No active goal for this session."
/// (`AutonomyPanel.tsx:176`) and no controls; a goal → its status badge, the
/// budget ("server default" when the budget is 0, `formatGoalBudget`), and
/// Pause↔Resume following the status. Controls show only while the goal can
/// transition (`["active","paused","budget_limited","blocked"]`, :129).
fn live_goal(tree: &mut UiNode, st: &AutonomyState) {
    let Some(goal) = st.goal.as_ref() else {
        set_text(tree, "t_goal", "No active goal for this session.");
        if let Some(n) = find_mut(tree, "t_goal") {
            n.attrs.color = Some(0xff6e_6e73);
            n.attrs.w = Some(320.0);
        }
        remove(
            tree,
            &[
                "goal_badge", "t_budget", "t_budget_val", "bar_track", "bar_fill", "t_elapsed",
                "t_elapsed_val", "pause_btn", "stop_btn", "clear_goal",
            ],
        );
        // The card ends under the empty line.
        if let (Some((_, cy, _, _)), Some((_, ty, _, th))) =
            (rect_of(tree, "goal_card"), rect_of(tree, "t_goal"))
        {
            set_h(tree, "goal_card", ty + th + 28.0 - cy);
        }
        return;
    };
    let status = goal["status"].as_str().unwrap_or("active");
    let (word, bg, ink) = goal_badge(status);
    set_text(tree, "goal_badge_label", word);
    if let Some(n) = find_mut(tree, "goal_badge") {
        n.attrs.bg = Some(bg);
        // The pill hugs its word (the authored 72px fits "Active").
        let w = 27.0 + word.chars().count() as f64 * 8.4;
        n.attrs.w = Some(w as f32);
    }
    if let Some(n) = find_mut(tree, "goal_badge_label") {
        n.attrs.color = Some(ink);
        n.attrs.w = Some((word.chars().count() as f64 * 8.4 + 4.0) as f32);
    }
    if goal["token_budget"].as_u64().unwrap_or(0) == 0 {
        set_text(tree, "t_budget_val", "server default");
        right_align(tree, "t_budget_val", 369.7, 160.0);
        remove(tree, &["bar_track", "bar_fill"]);
    } else {
        right_align(tree, "t_budget_val", 369.7, 160.0);
    }
    right_align(tree, "t_elapsed_val", 369.8, 120.0);
    let can_transition = matches!(status, "active" | "paused" | "budget_limited" | "blocked");
    if !can_transition {
        remove(tree, &["pause_btn", "stop_btn"]);
    } else if status != "active" {
        set_text(tree, "pause_btn_label", "Resume");
        if let Some(n) = find_mut(tree, "pause_btn_label") {
            n.attrs.x = n.attrs.x.map(|x| x - 4.0);
            n.attrs.w = Some(64.0);
        }
    }
}

/// The asset sources of the loop card's status dot (green, active) and its
/// paused variant (grey) — the authored rows 1 and 3.
fn loop_rows(tree: &mut UiNode, st: &AutonomyState) {
    let n = st.loops.len().min(3);
    let green = find(tree, "loop_1_dot").and_then(|d| d.attrs.src.clone());
    let grey = find(tree, "loop_3_dot").and_then(|d| d.attrs.src.clone());
    let pause_tpl = find(tree, "loop_1_pause").cloned();
    for i in 1..=n {
        let paused = st.loops[i - 1]["status"].as_str() == Some("paused");
        if let Some(dot) = find_mut(tree, &format!("loop_{i}_dot")) {
            if let Some(src) = if paused { grey.clone() } else { green.clone() } {
                dot.attrs.src = Some(src);
            }
        }
        let has_pause = find(tree, &format!("loop_{i}_pause")).is_some();
        if paused && has_pause {
            remove(tree, &[&format!("loop_{i}_pause")]);
        } else if !paused && !has_pause {
            // Row 3 is authored paused-shaped: give an active row its pause.
            if let (Some(mut p), Some((_, py, _, _)), Some((_, ry, _, _))) = (
                pause_tpl.clone(),
                rect_of(tree, "loop_1_play"),
                rect_of(tree, &format!("loop_{i}_play")),
            ) {
                p.attrs.id = Some(format!("loop_{i}_pause"));
                shift(&mut p, 0.0, ry - py);
                insert_after(tree, &format!("loop_{i}_dot"), p);
            }
        }
    }
}

/// autonomy-04 — per-status row icons and dots (the web shows Pause only for
/// an active loop, Resume only for a paused one, `AutonomyPanel.tsx:262-300`).
fn live_loops(tree: &mut UiNode, st: &AutonomyState) {
    loop_rows(tree, st);
}

/// autonomy-02 — idle: the web's own preamble ("Run the server's native review
/// specialists on the current project changes…", `NativeReviewDialog.tsx`)
/// or the typed withholding reason; running: the receipt's specialists with
/// the spinner. Never the authored sample run.
fn live_review(tree: &mut UiNode, ctx: &Ctx<'_>) {
    let status = crate::screens::review::query(ctx, "review.status")
        .and_then(|v| v.as_str().map(str::to_owned));
    let running = crate::screens::review::ui().agents.is_some();
    if running {
        return;
    }
    let blocked = {
        let ui = ctx.ui.lock().unwrap();
        crate::screens::review::blocked_reason(ctx.store, &ui)
    };
    let (head, sub) = match (status, blocked) {
        (Some(s), _) => (s, String::new()),
        (None, Some(b)) => (b.to_owned(), String::new()),
        (None, None) => (
            "Ready to review the current project changes.".to_owned(),
            "This starts a Session turn; it is not a diff preview.".to_owned(),
        ),
    };
    remove(tree, &["status_spinner"]);
    // Two lines of room: the typed reasons run ~60 characters. The kit
    // authored this text single-line (no wrap); a reason must wrap, never clip.
    if let Some(n) = find_mut(tree, "t_status") {
        n.attrs.text = Some(head);
        n.attrs.x = Some(42.0);
        n.attrs.w = Some(320.0);
        n.attrs.h = Some(48.0);
        n.attrs.variant = None;
    }
    if sub.is_empty() {
        remove(tree, &["t_status_sub"]);
        if let Some((_, y, _, _)) = rect_of(tree, "run_status_card") {
            set_h(tree, "run_status_card", 118.2 + 48.0 + 22.0 - y);
        }
    } else {
        if let Some(n) = find_mut(tree, "t_status_sub") {
            n.attrs.text = Some(sub);
            n.attrs.x = Some(42.0);
            n.attrs.w = Some(320.0);
            n.attrs.y = Some(170.0);
            n.attrs.h = Some(24.0);
        }
        if let Some((_, y, _, _)) = rect_of(tree, "run_status_card") {
            set_h(tree, "run_status_card", 170.0 + 24.0 + 20.0 - y);
        }
    }
}

/// Every kit button (`X` with `X_surface` + `X_label` children): the label is
/// centred on its pill. The atlas measured some labels off their surface
/// (setup-09 `Compact now` sat 11.5 px below the pill's centre line).
fn centre_button_labels(tree: &mut UiNode) {
    walk_mut(tree, &mut |n| {
        let Some(id) = n.attrs.id.clone() else { return };
        let surface = n
            .children
            .iter()
            .find(|c| c.attrs.id.as_deref() == Some(&format!("{id}_surface")))
            .map(rect);
        let Some((sx, sy, sw, sh)) = surface else { return };
        if let Some(label) = n
            .children
            .iter_mut()
            .find(|c| c.attrs.id.as_deref() == Some(&format!("{id}_label")))
        {
            let lh = label.attrs.h.unwrap_or(0.0) as f64;
            label.attrs.x = Some(sx);
            label.attrs.w = Some(sw as f32);
            label.attrs.alignx = Some(0.5);
            label.attrs.y = Some(sy + (sh - lh) / 2.0);
        }
    });
}

/// autonomy-06 / -07: the goal heading spans the card (the atlas box fitted
/// its sample "Fix steer queue"); an empty task list ends under its line.
fn live_fleet_tasks(tree: &mut UiNode) {
    if let (Some((_, _, _, _)), Some((cx, _, cw, _))) =
        (rect_of(tree, "fleet_goal_label"), rect_of(tree, "fleet_card"))
    {
        if let Some(n) = find_mut(tree, "fleet_goal_label") {
            let x = n.attrs.x.unwrap_or(cx);
            n.attrs.w = Some((cx + cw - x) as f32);
        }
        if let Some(n) = find_mut(tree, "fleet_goal") {
            let x = n.attrs.x.unwrap_or(cx);
            n.attrs.w = Some((cx + cw - x) as f32);
        }
    }
    if let (Some((_, cy, _, _)), Some((_, ty, _, th))) =
        (rect_of(tree, "tasks_card"), rect_of(tree, "tasks_empty"))
    {
        set_h(tree, "tasks_card", ty + th + 24.0 - cy);
    }
}

/// Insert `node` right after the node `after` (in its parent's children) —
/// drawn above it, and first in the event order (`EventOrder::Up`).
fn insert_after(tree: &mut UiNode, after: &str, node: UiNode) -> bool {
    fn go(n: &mut UiNode, after: &str, node: &mut Option<UiNode>) -> bool {
        if let Some(pos) = n.children.iter().position(|c| c.attrs.id.as_deref() == Some(after)) {
            if let Some(new) = node.take() {
                n.children.insert(pos + 1, new);
            }
            return true;
        }
        n.children.iter_mut().any(|c| go(c, after, node))
    }
    let mut slot = Some(node);
    go(tree, after, &mut slot)
}

// ---------------------------------------------------------------- controls

/// The minimum hit square (the brief's ≥28 px control rule, with margin).
const MIN_HIT: f64 = 36.0;

/// One clickable control: the node drawn for it and the action it routes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub node: String,
    pub event: String,
}

fn ctl(node: impl Into<String>, event: impl Into<String>) -> Control {
    Control { node: node.into(), event: event.into() }
}

/// The controls a dialog wires, from the LIVE state (a paused row's icon
/// resumes; a paused goal's button resumes). Per-row events carry their row
/// as the shared `#<row>` suffix (`taps::split_row`), which the host strips
/// back into `perform_action`'s index.
pub fn controls(d: Dialog, ctx: &Ctx<'_>, st: &AutonomyState) -> Vec<Control> {
    match d {
        Dialog::Models => vec![
            ctl("btn_test_control", "models.test_route"),
            ctl("btn_discover_control", "models.discover"),
        ],
        Dialog::Context => vec![
            ctl("btn_compact_control", "context.compact_now"),
            ctl("seg_llm_hit", "context.mode.llm"),
            ctl("seg_heur_hit", "context.mode.heuristic"),
        ],
        Dialog::Skills => {
            let mut v = Vec::new();
            for i in 0..ctx.store.domains.profile.installed_skills().len().min(3) {
                v.push(ctl(format!("t_remove{i}"), format!("skills.remove_{i}")));
            }
            for (i, b) in [(3usize, "btn_3_install_control"), (4, "btn_4_install_control")] {
                v.push(ctl(b, format!("skills.install_{i}")));
            }
            v
        }
        Dialog::Goal => {
            let status = st
                .goal
                .as_ref()
                .and_then(|g| g["status"].as_str().map(str::to_owned))
                .unwrap_or_default();
            let pause = if status == "active" { "goal.pause" } else { "goal.resume" };
            vec![
                ctl("pause_btn_control", pause),
                ctl("stop_btn_control", "goal.stop"),
                ctl("clear_goal", "goal.clear"),
            ]
        }
        Dialog::Loops => {
            let mut v = vec![ctl("new_loop_control", "loop.create")];
            for (i, l) in st.loops.iter().take(3).enumerate() {
                let paused = l["status"].as_str() == Some("paused");
                let r = i + 1;
                v.push(ctl(format!("loop_{r}_pause"), format!("loop.pause#{i}")));
                v.push(ctl(
                    format!("loop_{r}_play"),
                    if paused { format!("loop.resume#{i}") } else { format!("loop.fire_now#{i}") },
                ));
                v.push(ctl(format!("loop_{r}_trash"), format!("loop.delete#{i}")));
            }
            v
        }
        Dialog::Monitors => {
            let mut v = Vec::new();
            for (i, m) in st.monitors.iter().take(3).enumerate() {
                let paused = m["status"].as_str() == Some("paused");
                let r = i + 1;
                v.push(ctl(
                    format!("mon_{r}_pause"),
                    if paused { format!("monitor.resume#{i}") } else { format!("monitor.pause#{i}") },
                ));
                v.push(ctl(format!("mon_{r}_trash"), format!("monitor.delete#{i}")));
            }
            v
        }
        Dialog::Fleet => {
            let n = ctx.store.domains.peer.list().len();
            (0..n).map(|i| ctl(format!("peer_r{i}_steer"), format!("peer.steer#{i}"))).collect()
        }
        Dialog::Tasks => vec![ctl("cancel_control", "task.cancel")],
        Dialog::Review => vec![ctl("start_review_control", "review.start")],
    }
}

/// Wire `controls` into the tree by node id. A `Button` node takes the
/// `tapto`; any other node gets a transparent `Button` sibling over its own
/// bounds (grown to [`MIN_HIT`]). Returns the controls that found no node
/// (logged by the host — a drawn-but-dead control is never silent).
pub fn wire(tree: &mut UiNode, controls: &[Control]) -> Vec<Control> {
    let mut missing = Vec::new();
    for c in controls {
        // The two synthetic halves of the context segmented control.
        if c.node == "seg_llm_hit" || c.node == "seg_heur_hit" {
            if let (Some((bx, by, bw, bh)), Some((dx, _, _, _))) =
                (rect_of(tree, "seg_box"), rect_of(tree, "seg_div"))
            {
                let (x0, x1) = if c.node == "seg_llm_hit" { (bx, dx) } else { (dx, bx + bw) };
                let hit = hit_node(&c.node, (x0, by, x1 - x0, bh), &c.event, 0.0);
                if !insert_after(tree, "t_heur", hit) {
                    missing.push(c.clone());
                }
            } else {
                missing.push(c.clone());
            }
            continue;
        }
        // A control whose node the live state removed is not drawn, so it is
        // not a dead control (the tests pin that the FULL state wires all).
        let Some(node) = find_mut(tree, &c.node) else {
            continue;
        };
        if node.kind == NodeKind::Button {
            node.attrs.tapto = Some(c.event.clone());
            node.attrs.enabled = Some(1);
            continue;
        }
        let r = rect(node);
        let hit = hit_node(&c.node, r, &c.event, MIN_HIT);
        if !insert_after(tree, &c.node, hit) {
            missing.push(c.clone());
        }
    }
    missing
}

/// A transparent hit target (`DesignNativeButton`, which draws nothing) over
/// `r`, grown to `min` on each axis around its centre.
fn hit_node(id: &str, r: (f64, f64, f64, f64), event: &str, min: f64) -> UiNode {
    let (x, y, w, h) = r;
    let (gw, gh) = (w.max(min), h.max(min));
    let mut a = Attrs::default();
    a.id = Some(format!("{id}_hit").replace("_hit_hit", "_hit"));
    a.x = Some(x - (gw - w) / 2.0);
    a.y = Some(y - (gh - h) / 2.0);
    a.w = Some(gw as f32);
    a.h = Some(gh as f32);
    a.tapto = Some(event.to_owned());
    a.enabled = Some(1);
    UiNode { kind: NodeKind::Button, attrs: a, children: vec![] }
}

// ---------------------------------------------------------------- geometry

/// The dialog's inner padding around the card content.
const PAD_X: f64 = 20.0;
const PAD_TOP: f64 = 20.0;
const PAD_BOTTOM: f64 = 24.0;

/// Crop the artboard to its content: translate the content to the padded
/// origin and size the frame containers to the content box. Frame containers
/// lose their fill (the dialog frame paints the surface and its rounded
/// corners). Returns the card size.
fn normalize(tree: &mut UiNode) -> (f64, f64) {
    let frames = frame_ids(tree);
    let is_frame = |n: &UiNode| n.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id));
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    walk(tree, &mut |n| {
        if is_frame(n) {
            return;
        }
        let (x, y, w, h) = rect(n);
        if w <= 0.5 || h <= 0.5 {
            return;
        }
        if n.kind == NodeKind::Text && n.attrs.text.as_deref().is_none_or(|t| t.trim().is_empty()) {
            return;
        }
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + w);
        y1 = y1.max(y + h);
    });
    if x0 == f64::MAX {
        return (rect(tree).2, rect(tree).3);
    }
    let (dx, dy) = (PAD_X - x0, PAD_TOP - y0);
    let w = (x1 - x0) + 2.0 * PAD_X;
    let h = (y1 - y0) + PAD_TOP + PAD_BOTTOM;
    fn go(n: &mut UiNode, frames: &[String], dx: f64, dy: f64, w: f64, h: f64) {
        let frame = n.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id));
        if frame {
            n.attrs.x = Some(0.0);
            n.attrs.y = Some(0.0);
            n.attrs.w = Some(w as f32);
            n.attrs.h = Some(h as f32);
            n.attrs.bg = None;
            n.attrs.radius = None;
            n.attrs.border = None;
            for c in &mut n.children {
                go(c, frames, dx, dy, w, h);
            }
        } else {
            shift(n, dx, dy);
        }
    }
    go(tree, &frames, dx, dy, w, h);
    (w, h)
}

/// Uniform scale of every geometric attribute (the phone fit). Colours,
/// weights and line-height multipliers are untouched.
fn scale(tree: &mut UiNode, k: f64) {
    if (k - 1.0).abs() < 1e-6 {
        return;
    }
    let kf = k as f32;
    walk_mut(tree, &mut |n| {
        let a = &mut n.attrs;
        for v in [&mut a.x, &mut a.y] {
            if let Some(v) = v.as_mut() {
                *v *= k;
            }
        }
        for v in [
            &mut a.w,
            &mut a.h,
            &mut a.size,
            &mut a.radius,
            &mut a.border,
            &mut a.pad,
            &mut a.padx,
            &mut a.pady,
            &mut a.padleft,
            &mut a.padright,
            &mut a.padtop,
            &mut a.padbottom,
            &mut a.spacing,
            &mut a.line_height,
        ] {
            if let Some(v) = v.as_mut() {
                *v *= kf;
            }
        }
    });
}

/// Prefix every authored id so the mounted card can never shadow a host
/// widget (`status`, `palette`, …) and stays readable in `/snap`.
fn prefix_ids(tree: &mut UiNode, prefix: &str) {
    walk_mut(tree, &mut |n| {
        if let Some(id) = n.attrs.id.as_mut() {
            *id = format!("{prefix}{id}");
        }
    });
}

/// The close button's square, top-right, centred on the card's title row and
/// inset into the bordered card that holds the title when there is one.
/// Returns `(x, y)` in card coordinates.
fn close_slot(tree: &UiNode, card_w: f64, frames: &[String]) -> (f64, f64) {
    // The title: the topmost text node (reading order).
    let mut title: Option<(f64, f64, f64, f64)> = None;
    walk(tree, &mut |n| {
        if n.kind == NodeKind::Text && n.attrs.text.as_deref().is_some_and(|t| !t.trim().is_empty()) {
            let r = rect(n);
            if title.is_none_or(|t| r.1 < t.1 - 0.5 || (r.1 - t.1).abs() <= 0.5 && r.0 < t.0) {
                title = Some(r);
            }
        }
    });
    let Some((tx, ty, _, th)) = title else {
        return (card_w - CLOSE_INSET - CLOSE_SIZE, CLOSE_INSET);
    };
    // The innermost bordered container holding the title.
    let mut right = card_w;
    walk(tree, &mut |n| {
        let frame = n.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id));
        if frame || n.kind != NodeKind::Stack || n.attrs.border.is_none() {
            return;
        }
        let (x, y, w, h) = rect(n);
        if tx >= x && tx <= x + w && ty >= y && ty <= y + h {
            right = right.min(x + w);
        }
    });
    let cy = ty + th / 2.0;
    (right - CLOSE_INSET - CLOSE_SIZE, (cy - CLOSE_SIZE / 2.0).max(6.0))
}

const CLOSE_SIZE: f64 = 28.0;
const CLOSE_INSET: f64 = 12.0;

/// Move every non-frame top-level-in-container node that collides with the
/// close square left, so the title row's own control (`+ New loop`, `Start
/// native review`) never sits under the close button.
fn clear_close(tree: &mut UiNode, slot: (f64, f64), frames: &[String]) {
    let (cx, cy) = slot;
    let (l, t, r, b) = (cx - 8.0, cy, cx + CLOSE_SIZE, cy + CLOSE_SIZE);
    fn go(n: &mut UiNode, frames: &[String], l: f64, t: f64, r: f64, b: f64) {
        for c in &mut n.children {
            let frame = c.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id));
            let (x, y, w, h) = rect(c);
            let bordered_container = c.kind == NodeKind::Stack && c.attrs.border.is_some();
            if frame || bordered_container {
                go(c, frames, l, t, r, b);
                continue;
            }
            let hits = w > 0.5 && h > 0.5 && x < r && x + w > l && y < b && y + h > t;
            if hits {
                let dx = l - (x + w);
                shift(c, dx, 0.0);
            }
        }
    }
    go(tree, frames, l, t, r, b);
}

// ------------------------------------------------------------------- lower

/// The lowered dialog: the DSL the host mounts and the taps it routes.
#[derive(Debug, Clone)]
pub struct Mounted {
    pub dsl: String,
    /// `(widget name, action id)` — the host's `[dialog_splash, name]` clicks.
    pub taps: Vec<(String, String)>,
    /// Controls the dialog meant to wire but found no node for.
    pub missing: Vec<Control>,
    /// The frame's size in the host's logical pixels, and the card scale.
    pub frame: (f64, f64),
    pub scale: f64,
}

/// The phone threshold: below this host width the dialog is a full-bleed
/// sheet scaled to the width (the web's `width: min(…, 100%)` with the
/// backdrop padding taken back on a phone-sized window).
const SHEET_BELOW: f64 = 520.0;
/// The desktop margin around the frame (`.backdrop { padding: 16px }`).
const MARGIN: f64 = 16.0;

/// The per-dialog live edits, then the shared button-label centring.
fn live(d: Dialog, tree: &mut UiNode, ctx: &Ctx<'_>, st: &AutonomyState) {
    match d {
        Dialog::Models => live_models(tree, ctx),
        Dialog::Context => live_context(tree, ctx),
        Dialog::Skills => live_skills(tree, ctx),
        Dialog::Goal => live_goal(tree, st),
        Dialog::Loops => live_loops(tree, st),
        Dialog::Review => live_review(tree, ctx),
        Dialog::Fleet | Dialog::Tasks => live_fleet_tasks(tree),
        Dialog::Monitors => {}
    }
    centre_button_labels(tree);
}

/// Lower dialog `d` for a host area of `avail_w × avail_h`.
pub fn lower(d: Dialog, ctx: &Ctx<'_>, avail_w: f64, avail_h: f64) -> Result<Mounted, String> {
    let st = autonomy_view(ctx);
    let mut tree = card_tree(d, ctx, &st)?;
    live(d, &mut tree, ctx, &st);
    let missing = wire(&mut tree, &controls(d, ctx, &st));
    let (cw, ch) = normalize(&mut tree);
    let frames = frame_ids(&tree);
    let slot = close_slot(&tree, cw, &frames);
    clear_close(&mut tree, slot, &frames);

    // Desktop: the card at its design size, centred, the frame no taller than
    // the host minus the backdrop padding (the rest scrolls). Phone: a
    // full-bleed sheet, the card scaled to the width.
    let sheet = avail_w > 0.0 && avail_w < SHEET_BELOW;
    let k = if sheet { (avail_w / cw).min(1.0) } else { 1.0 };
    scale(&mut tree, k);
    let (fw, fh) = if sheet {
        (avail_w, avail_h.max(1.0))
    } else {
        (cw, ch.min((avail_h - 2.0 * MARGIN).max(120.0)))
    };
    prefix_ids(&mut tree, &format!("dlg_{}_", d.id()));
    let card = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui_in_slot(&tree))
        .map_err(|e| format!("to_makepad_ui_in_slot {}: {e}", d.card()))?;
    let card = crate::screens::theme::retint_dsl(&localize_card_assets(&card));
    let dsl = chrome(d, &card, (fw, fh), (cw * k, ch * k), (slot.0 * k, slot.1 * k), sheet);
    let taps = crate::screens::taps::wired_taps(&dsl);
    Ok(Mounted { dsl, taps, missing, frame: (fw, fh), scale: k })
}

/// Rewrite every `http_resource("<loopback>/ux-images/<card>/assets/<file>")`
/// the lowering emits for a card SVG into `file_resource("<abs>")` on the
/// card's own asset on disk — the components path's
/// `localize_asset_resources` (card #21d item 6) for the Stage-B screen cards.
/// The URL names the design lab's ad-hoc `:8170` server, which is not ours to
/// run, so without this every chevron, check and row icon drew nothing.
pub fn localize_card_assets(dsl: &str) -> String {
    const MARK: &str = "http_resource(\"";
    const STAGES: &[&str] = &[
        "stage-b/setup/cards",
        "stage-b/autonomy/cards",
        "stage-b/conversation/cards",
        "stage-b/phase4/cards",
        "stage-b/phase4-new2/cards",
        "stage-b/phase4-new3/cards",
    ];
    let mut out = String::with_capacity(dsl.len());
    let mut rest = dsl;
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
        let found = STAGES
            .iter()
            .map(|s| crate::design::dir(s).join(rel))
            .find(|p| p.is_file());
        match found {
            Some(p) => {
                let abs = std::fs::canonicalize(&p).unwrap_or(p);
                out.push_str(&format!("file_resource({:?})", abs.to_string_lossy()));
                rest = after[endq + 1..].strip_prefix(')').unwrap_or(&after[endq + 1..]);
            }
            None => {
                // Keep the original call; the missing file is the asset
                // table's bug, and the app log shows the empty icon.
                out.push_str(&tail[..MARK.len() + endq + 1]);
                rest = &after[endq + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The dialog chrome around the card: the backdrop (which also swallows the
/// clicks meant for the conversation behind it — the web's modal), the
/// centred frame, the vertical scroll, and the close button.
fn chrome(
    d: Dialog,
    card: &str,
    frame: (f64, f64),
    card_size: (f64, f64),
    close: (f64, f64),
    sheet: bool,
) -> String {
    let (fw, fh) = frame;
    let (cw, ch) = card_size;
    let (radius, border) = if sheet { (0.0, 0.0) } else { (14.0, 1.0) };
    let close_icon = crate::design::icon_resource("icon_close.svg");
    let dark = crate::screens::theme::resolved() == "dark";
    let (surface, edge, scrim) = if dark {
        ("#1c1f22", "#3a3a3c", "#00000080")
    } else {
        ("#ffffff", "#e5e5e7", "#1d1d1f40")
    };
    format!(
        "dialog_root := View {{ width: Fill height: Fill flow: Overlay\n\
         dialog_scrim := SolidView {{ width: Fill height: Fill draw_bg.color: {scrim} }}\n\
         dialog_backdrop := Button {{ width: Fill height: Fill text: \"\" draw_bg.color: #00000000 draw_bg.color_hover: #00000000 draw_bg.color_down: #00000000 draw_bg.border_size: 0.0 draw_bg.color_2: #00000000 draw_bg.border_color: #00000000 draw_bg.border_color_2: #00000000 }}\n\
         View {{ width: Fill height: Fill align: Align{{x: 0.5 y: 0.5}} flow: Overlay\n\
         dialog_frame := RoundedView {{ width: {fw} height: {fh} flow: Overlay\n\
         draw_bg +: {{color: {surface} border_radius: {radius} border_size: {border} border_color: {edge}}}\n\
         dialog_scroll := ScrollYView {{ width: Fill height: Fill flow: Down\n\
         dialog_card_{id} := View {{ width: {cw} height: {ch} flow: Overlay\n\
         {card}\n\
         }}\n\
         }}\n\
         dialog_close_wrap := View {{ width: {cs} height: {cs} margin: Inset{{left: {clx} top: {cly}}} flow: Overlay align: Align{{x: 0.5 y: 0.5}}\n\
         Svg {{ width: 12 height: 12 animating: false draw_svg.svg: file_resource({close_icon:?}) draw_svg.preserve_viewbox: true }}\n\
         dialog_close := DesignNativeButton {{\n\
         on_click: || {{ NAV(t: \"{close_ev}\") }}\n\
         width: {cs} height: {cs}\n\
         enabled: true\n\
         }}\n\
         }}\n\
         }}\n\
         }}\n\
         }}",
        id = d.id(),
        cs = CLOSE_SIZE,
        clx = close.0,
        cly = close.1,
        close_ev = ACTION_CLOSE,
    )
}

/// The autonomy cards' state: the screen cache (the last `*/list` /
/// `goal/get` reads, kept current by the `*/updated` notifications), so a
/// dialog shows exactly what the server last said.
fn autonomy_view(_ctx: &Ctx<'_>) -> AutonomyState {
    crate::screens::autonomy::state_snapshot()
}

/// The capability set the recorded live server negotiated
/// (`crates/octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl`, its
/// `session/open` reply): the features, and the methods the dialogs and the
/// palette gate on. The fixture seed installs exactly these.
pub const RECORDED_FEATURES: &[&str] = &[
    "approval.typed.v1", "pane.snapshots.v1", "session.workspace_cwd.v1", "harness.task_control.v1",
    "state.session_hydrate.v1", "state.thread_graph.v1", "state.turn_state_get.v1",
    "projection.envelope.v2", "auxiliary.rest_to_ws.v1", "coding.autonomy.v1",
    "coding.agent_control.v1", "coding.goal_runtime.v1", "coding.loop_runtime.v1",
    "coding.monitor_runtime.v1", "review.start.v1", "context.lifecycle.v1",
    "harness.task_artifacts.v1", "user_question.v1", "plan.todos.v1", "permission.profile.v1",
];
pub const RECORDED_METHODS: &[&str] = &[
    "session/open", "turn/start", "turn/interrupt", "approval/respond", "session/btw",
    "permission/profile/list", "permission/profile/set", "diff/preview/get", "task/list",
    "task/cancel", "task/output/read", "session/hydrate", "session/goal/get", "session/goal/set",
    "session/goal/clear", "loop/create", "loop/list", "loop/delete", "loop/pause", "loop/resume",
    "loop/fire_now", "monitor/create", "monitor/list", "monitor/pause", "monitor/resume",
    "monitor/delete", "review/start", "session/list", "session/status/read", "profile/llm/list",
    "profile/llm/test", "profile/llm/fetch_models", "profile/sub_providers/list", "peer/prepare",
    "peer/gather", "profile/skills/list", "profile/skills/registry/search",
    "profile/skills/install", "profile/skills/remove", "session/compact",
    "session/compact/mode/set",
];

/// The reference-board fixture (the Stage-A setup/autonomy atlas prompts'
/// fixture: DeepSeek/Kimi/GLM routes, rust-review/git-helper/docs-writer,
/// "Fix steer queue on reconnect", Run CI smoke / Sync main / Nightly review,
/// `cargo test -q` / `tail -n 50 app.log`, tests/docs/review peers) put into
/// the SAME store slots and screen caches the wire fills — the capture seed
/// for the dialogs, the `fleet::capture_store` precedent. Gated by
/// `OCTOSCODE_DIALOG_SEED` in the host; tests use it as the full state.
pub fn seed_fixture(store: &octoscode_store::Store) {
    use octoscode_store::domains::peer::Peer;
    use octoscode_store::domains::profile::{InstalledSkill, ProfileLlmModel, SkillPackage};
    use serde_json::json;
    store.set_capabilities(RECORDED_FEATURES.iter().map(|s| s.to_string()).collect());
    store
        .domains
        .config
        .set_supported_methods(RECORDED_METHODS.iter().map(|s| s.to_string()).collect());
    let session = store.active_session().unwrap_or_else(|| {
        store.domains.session.set_active(Some("dsflash:main".into()));
        "dsflash:main".to_owned()
    });
    let model = |provider: &str, route: &str, model: &str, selected: bool| ProfileLlmModel {
        model: model.into(),
        provider: provider.into(),
        title: model.into(),
        family: Some(provider.into()),
        route: Some(route.into()),
        selected,
        available: true,
    };
    store.domains.profile.set_llm_models(vec![
        model("deepseek", "Official API", "deepseek-v4-flash", true),
        model("deepseek", "Official API", "deepseek-v4-pro", false),
        model("kimi", "Coding Plan", "kimi-k2", false),
        model("kimi", "Coding Plan", "kimi-k2-turbo", false),
        model("kimi", "Coding Plan", "kimi-latest", false),
        model("zai", "Coding Plan", "glm-4.6", false),
        model("zai", "Coding Plan", "glm-4.5", false),
        model("zai", "Coding Plan", "glm-4.5-air", false),
    ]);
    let skill = |name: &str, version: &str| InstalledSkill {
        name: name.into(),
        version: Some(version.into()),
        tool_count: 2,
        source_repo: None,
    };
    store.domains.profile.set_installed_skills(vec![
        skill("rust-review", "1.2.0"),
        skill("git-helper", "0.9.1"),
        skill("docs-writer", "2.0.0"),
    ]);
    let pkg = |name: &str, version: &str| SkillPackage {
        name: name.into(),
        description: String::new(),
        repo: format!("octos-org/{name}"),
        version: Some(version.into()),
        author: None,
        license: None,
        ..serde_json::from_value(json!({"name": name})).expect("a package")
    };
    store
        .domains
        .profile
        .set_registry_packages(vec![pkg("code-linter", "1.1.0"), pkg("api-client", "0.4.2")]);
    store.domains.session.set_context(
        &session,
        octoscode_store::domains::session::ContextLifecycle {
            kind: "session/status/read".into(),
            state: json!({
                "session_id": session, "token_estimate": 124000, "item_count": 312,
                "generation": 3, "recovery_state": "exact",
            }),
            detail: None,
        },
    );
    crate::screens::models::note_token_cost(&session, 200_000);
    crate::screens::models::note_compact_mode(&json!({"session_id": session, "mode": "llm"}));
    crate::screens::autonomy::update_state(|st| {
        st.goal = Some(json!({
            "goal_id": "goal_01", "objective": "Fix steer queue on reconnect", "status": "active",
            "token_budget": 100000, "tokens_used": 41000, "time_used_seconds": 1080,
        }));
        st.loops = vec![
            json!({"loop_id": "loop_01", "prompt": "Run CI smoke", "status": "active", "interval_seconds": 900}),
            json!({"loop_id": "loop_02", "prompt": "Sync main", "status": "active", "interval_seconds": 1800}),
            json!({"loop_id": "loop_03", "prompt": "Nightly review", "status": "paused", "interval_seconds": 86400}),
        ];
        st.monitors = vec![
            json!({"monitor_id": "monitor_01", "name": "tests", "argv": ["cargo", "test", "-q"], "status": "active", "interval_seconds": 30}),
            json!({"monitor_id": "monitor_02", "name": "log", "argv": ["tail", "-n", "50", "app.log"], "status": "active", "interval_seconds": 30}),
        ];
    });
    for name in ["tests", "docs", "review"] {
        store.domains.peer.upsert(Peer::named(name));
    }
    store.domains.peer.mark_closed("review");
    // The Tasks card's running + settled rows (the atlas command), the
    // `fleet::capture_store` shapes.
    use octoscode_store::domains::task::TaskSnapshot;
    let task = |id: &str, title: &str, state: &str| {
        TaskSnapshot::from_list_row(
            id.into(),
            title.into(),
            state.into(),
            state.into(),
            Some(title.into()),
            None,
            None,
            None,
            0,
            Vec::new(),
            None,
            None,
        )
    };
    store.domains.task.upsert_snapshot(task("t-run", "cargo test -p octos-cli steer_queue", "running"));
    store.domains.task.upsert_snapshot(task("t-done", "cargo clippy -p octos-cli", "done"));
    for line in [
        "Compiling octos-cli v0.24.1",
        "Finished test [unoptimized + debuginfo] target(s) in 1.23s",
        "Running unittests src/lib.rs",
        "running 12 tests",
    ] {
        store.domains.task.append_output("t-run", &format!("{line}\n"));
    }
}

/// One line per node: depth, kind, id, frame, text, fill — the dump the
/// geometry tests and the dev probe print.
pub fn describe(tree: &UiNode) -> Vec<String> {
    fn go(n: &UiNode, depth: usize, out: &mut Vec<String>) {
        let a = &n.attrs;
        out.push(format!(
            "{}{:?} {} [{:.1},{:.1},{:.1},{:.1}] text={:?} bg={:?} radius={:?} border={:?} size={:?} tap={:?}",
            "  ".repeat(depth),
            n.kind,
            a.id.as_deref().unwrap_or("-"),
            a.x.unwrap_or(0.0),
            a.y.unwrap_or(0.0),
            a.w.unwrap_or(0.0),
            a.h.unwrap_or(0.0),
            a.text.as_deref().map(|t| t.chars().take(40).collect::<String>()),
            a.bg.map(|c| format!("{c:08x}")),
            a.radius,
            a.border,
            a.size,
            a.tapto,
        ));
        for c in &n.children {
            go(c, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    go(tree, 0, &mut out);
    out
}

/// The live tree before lowering (dev probe + tests).
pub fn live_tree(d: Dialog, ctx: &Ctx<'_>) -> Result<(UiNode, Vec<Control>), String> {
    let st = autonomy_view(ctx);
    let mut tree = card_tree(d, ctx, &st)?;
    live(d, &mut tree, ctx, &st);
    let missing = wire(&mut tree, &controls(d, ctx, &st));
    normalize(&mut tree);
    let frames = frame_ids(&tree);
    let (w, _) = (rect(&tree).2, rect(&tree).3);
    let slot = close_slot(&tree, w, &frames);
    clear_close(&mut tree, slot, &frames);
    Ok((tree, missing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    use crate::flow::FlowUi;
    use octoscode_store::Store;

    /// Dev probe: `cargo test -p octoscode-module --lib dialog::tests::dump -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dump() {
        let store = Arc::new(Store::new());
        let ui = StdMutex::new(FlowUi::default());
        let ctx = Ctx::new(&store, &ui);
        for d in ALL {
            match live_tree(*d, &ctx) {
                Ok((t, missing)) => {
                    println!("===== {} ({}) missing={missing:?}", d.id(), d.card());
                    for l in describe(&t) {
                        println!("{l}");
                    }
                }
                Err(e) => println!("===== {} ERR {e}", d.id()),
            }
            match lower(*d, &ctx, 990.0, 603.0) {
                Ok(m) => println!("  frame={:?} k={} taps={:?}", m.frame, m.scale, m.taps),
                Err(e) => println!("  lower ERR {e}"),
            }
        }
    }
}
