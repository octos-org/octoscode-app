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
        // A10: `/agents` opens its own Agents panel (board-3 host), not the
        // goal section.
        "goal" => Dialog::Goal,
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

/// A pending confirmation: the web asks before its irreversible or costly
/// writes — `ContextDialog.tsx:155` ("Compact this session now? …"),
/// `SkillsDialog.tsx:305` ("Confirm removal" / "Confirm server
/// installation", naming the Profile the change applies to). While one is
/// pending, its dialog shows the confirm card in place of its own card;
/// Confirm performs `action`, Cancel returns to the dialog, and the close
/// button / Escape close both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    pub dialog: Dialog,
    /// The action Confirm performs (`context.compact_now`, `skills.remove_N`, …).
    pub action: String,
    pub title: String,
    /// The object of the change (a skill name, `repo · branch main`); may be empty.
    pub detail: String,
    pub body: String,
    pub confirm_label: String,
}

static CONFIRM: Mutex<Option<Confirm>> = Mutex::new(None);

/// The Skills registry query last SEARCHED (`None`: no search yet). Set when
/// a search is sent (the web's `search(query)`), so the remounted input shows
/// it and an empty result reads "No matching skill packages.".
static SKILLS_QUERY: Mutex<Option<String>> = Mutex::new(None);

pub fn skills_query() -> Option<String> {
    SKILLS_QUERY.lock().unwrap().clone()
}

pub fn set_skills_query(q: Option<String>) {
    *SKILLS_QUERY.lock().unwrap() = q;
}

/// The Skills search input's widget id (after the dialog's `dlg_skills_` prefix).
pub const SKILLS_QUERY_INPUT: &str = "dlg_skills_skills_query";

/// A create form inside an autonomy dialog — the web's AutonomyPanel forms
/// (goal objective + optional budget, `AutonomyPanel.tsx:194-219`; the loop
/// prompt + interval, `LoopCreationControls.tsx:18`). Its fields are real
/// inputs; Create composes the entry text the autonomy table already parses
/// ("objective | budget", "prompt | 15m") and runs `action`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub dialog: Dialog,
    /// The autonomy action Create runs (`goal.set`, `loop.create`).
    pub action: String,
    pub title: String,
    /// `(field id, placeholder, value)`; the value survives a re-lowering.
    pub fields: Vec<(String, String, String)>,
    pub help: String,
    pub submit_label: String,
    /// Why the last Create did not run (the web's `required` / interval
    /// refusals), drawn above the buttons.
    pub error: Option<String>,
}

static FORM: Mutex<Option<Form>> = Mutex::new(None);

pub fn pending_form() -> Option<Form> {
    FORM.lock().unwrap().clone()
}

pub fn set_form(f: Option<Form>) {
    *FORM.lock().unwrap() = f;
}

/// The form `action` opens, seeded from `seed` ("prompt | 15m" fills both
/// loop fields; any text seeds the goal objective).
pub fn form_for(action: &str, seed: &str) -> Option<Form> {
    let (first, second) = match seed.split_once('|') {
        Some((a, b)) => (a.trim().to_owned(), b.trim().to_owned()),
        None => (seed.trim().to_owned(), String::new()),
    };
    match action {
        "goal.set" => Some(Form {
            dialog: Dialog::Goal,
            action: action.to_owned(),
            title: "Set goal".to_owned(),
            fields: vec![
                ("gf_objective".to_owned(), "Objective".to_owned(), first),
                ("gf_budget".to_owned(), "Token budget (optional)".to_owned(), second),
            ],
            help: "A blank budget is left out; the server applies its default.".to_owned(),
            submit_label: "Set goal".to_owned(),
            error: None,
        }),
        "loop.create" => Some(Form {
            dialog: Dialog::Loops,
            action: action.to_owned(),
            title: "New loop".to_owned(),
            fields: vec![
                ("lf_prompt".to_owned(), "What the loop runs".to_owned(), first),
                ("lf_interval".to_owned(), "Interval, e.g. 15m".to_owned(), second),
            ],
            help: "A fixed interval runs every 60s to 24h; leave it empty for a self-paced loop."
                .to_owned(),
            submit_label: "Create loop".to_owned(),
            error: None,
        }),
        _ => None,
    }
}

/// The entry text a form's values compose: the first field, then " | " and
/// the second when it is not blank.
pub fn form_value(values: &[String]) -> String {
    let first = values.first().map(|v| v.trim()).unwrap_or("");
    if first.is_empty() {
        // The first field is the required one: no entry text at all, so the
        // table refuses it as empty (never "| 15m" read as the prompt).
        return String::new();
    }
    match values.get(1).map(|v| v.trim()).filter(|v| !v.is_empty()) {
        Some(second) => format!("{first} | {second}"),
        None => first.to_owned(),
    }
}

/// The widget id of form field `field` in dialog `d` (after the prefix).
pub fn form_input_id(d: Dialog, field: &str) -> String {
    format!("dlg_{}_{field}", d.id())
}

/// The pending confirmation, if any.
pub fn pending_confirm() -> Option<Confirm> {
    CONFIRM.lock().unwrap().clone()
}

pub fn set_confirm(c: Option<Confirm>) {
    *CONFIRM.lock().unwrap() = c;
}

/// The confirmation the web asks before `action`, in the web's words, for
/// the row the action names (`None`: the action does not ask, or its row is
/// gone). The Profile is the connection's (`profile.current()`).
pub fn confirmation_for(action: &str, store: &octoscode_store::Store) -> Option<Confirm> {
    let profile = store.domains.profile.current().unwrap_or_else(|| "this Profile".to_owned());
    if action == "context.compact_now" {
        return Some(Confirm {
            dialog: Dialog::Context,
            action: action.to_owned(),
            title: "Compact context".to_owned(),
            detail: String::new(),
            body: "Compact this session now? Older context may be summarized. \
                   The durable transcript remains on the server."
                .to_owned(),
            confirm_label: "Confirm compaction".to_owned(),
        });
    }
    if let Some(i) = action.strip_prefix("skills.remove_").and_then(|n| n.parse::<usize>().ok()) {
        let name = store.domains.profile.installed_skills().get(i)?.name.clone();
        return Some(Confirm {
            dialog: Dialog::Skills,
            action: action.to_owned(),
            title: "Confirm removal".to_owned(),
            detail: name,
            body: format!(
                "Applies to Profile {profile} and rebuilds its server skill runtime. \
                 Reinstall from the original source to recover the removed skill."
            ),
            confirm_label: "Confirm remove".to_owned(),
        });
    }
    if let Some(i) = action.strip_prefix("skills.install_").and_then(|n| n.parse::<usize>().ok()) {
        // `skills.install_N` names registry row N - 3 (`models.rs` INSTALL_BASE).
        let pkg = store.domains.profile.registry_packages().get(i.checked_sub(3)?)?.clone();
        return Some(Confirm {
            dialog: Dialog::Skills,
            action: action.to_owned(),
            title: "Confirm server installation".to_owned(),
            detail: format!("{} · branch main", pkg.repo),
            body: format!(
                "Skills are shared by this Profile, not installed on this device. \
                 Installation may download executable tools and dependencies; review \
                 and trust the source first. Applies to Profile {profile} and rebuilds \
                 its server skill runtime. Existing skills are not forcibly overwritten."
            ),
            confirm_label: "Confirm install".to_owned(),
        });
    }
    None
}

/// The open dialog's notice: why a control the user just clicked did not run
/// (the web renders each family's error under its section with
/// `role="alert"`, `AutonomyPanel.tsx:227/:328/:485`; a create/steer with no
/// text is refused before any request, like the web's `required` field).
static NOTICE: Mutex<Option<(Dialog, String, bool)>> = Mutex::new(None);

/// An ALERT notice (red, the web's `role="alert"` error line).
pub fn set_notice(text: impl Into<String>) {
    if let Some(d) = current() {
        *NOTICE.lock().unwrap() = Some((d, text.into(), true));
    }
}

/// An informational notice (secondary grey, the web's `role="status"`
/// result line — e.g. "Context compacted.").
pub fn set_info(text: impl Into<String>) {
    if let Some(d) = current() {
        *NOTICE.lock().unwrap() = Some((d, text.into(), false));
    }
}

pub fn clear_notice() {
    *NOTICE.lock().unwrap() = None;
}

/// The notice for dialog `d`, if one is up.
pub fn notice(d: Dialog) -> Option<String> {
    notice_tone(d).map(|(t, _)| t)
}

/// The notice for dialog `d` and whether it is an alert.
pub fn notice_tone(d: Dialog) -> Option<(String, bool)> {
    match NOTICE.lock().unwrap().as_ref() {
        Some((n, t, alert)) if *n == d => Some((t.clone(), *alert)),
        _ => None,
    }
}

/// The notice a fail-closed control id earns (`autonomy::resolve`'s typed
/// `Unhandled` reasons). `None` = nothing to tell the user.
pub fn notice_for_refusal(id: &str) -> Option<&'static str> {
    if id.ends_with("[not-advertised]") {
        return Some("This server does not advertise that control.");
    }
    Some(match id {
        "loop.create[empty]" => "Type what the loop runs first.",
        i if i.starts_with("loop.create[interval") => {
            "Use a whole-number interval from 60s to 24h, such as 15m."
        }
        "loop.create[prompt-too-long]" => "The loop prompt is limited to 8 KB.",
        "goal.set[empty]" => "Type the goal's objective first.",
        i if i.starts_with("goal.set[budget") => {
            "The token budget is a whole number of tokens, such as 100000."
        }
        _ => return None,
    })
}

pub const ACTION_CLOSE: &str = "dialog.close";
/// `dialog.ask.<action>`: show `<action>`'s confirm card instead of running it.
pub const ACTION_ASK: &str = "dialog.ask.";
pub const ACTION_CONFIRM: &str = "dialog.confirm";
pub const ACTION_CANCEL: &str = "dialog.cancel";
/// `dialog.form.<action>`: open `<action>`'s create form.
pub const ACTION_FORM: &str = "dialog.form.";
pub const ACTION_FORM_SUBMIT: &str = "dialog.form_submit";
pub const ACTION_FORM_CANCEL: &str = "dialog.form_cancel";
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
    /// Ask before running this action (its confirm card).
    Ask(String),
    /// Run the pending confirmation's action.
    Confirm,
    /// Drop the pending confirmation (back to the dialog's own card).
    Cancel,
    /// Open this action's create form.
    OpenForm(String),
    /// Run the open form's action with its fields' text.
    SubmitForm,
    /// Close the form (back to the dialog's own card).
    CancelForm,
    Unhandled(String),
}

pub fn resolve(id: &str) -> Effect {
    if id == ACTION_CLOSE {
        return Effect::Close;
    }
    if let Some(d) = id.strip_prefix("dialog.open.").and_then(Dialog::from_id) {
        return Effect::Open(d);
    }
    if let Some(a) = id.strip_prefix(ACTION_ASK).filter(|a| !a.is_empty()) {
        return Effect::Ask(a.to_owned());
    }
    if let Some(a) = id.strip_prefix(ACTION_FORM).filter(|a| !a.is_empty()) {
        return Effect::OpenForm(a.to_owned());
    }
    match id {
        ACTION_CONFIRM => Effect::Confirm,
        ACTION_CANCEL => Effect::Cancel,
        ACTION_FORM_SUBMIT => Effect::SubmitForm,
        ACTION_FORM_CANCEL => Effect::CancelForm,
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
            clear_notice();
            set_confirm(None);
            set_form(None);
            open(*d);
            Some(*d)
        }
        Effect::Close => {
            clear_notice();
            set_confirm(None);
            set_form(None);
            close();
            None
        }
        _ => None,
    }
}

/// Insert the notice line directly UNDER THE TITLE (post-normalize card
/// coordinates): a reply remounts the card at the top of its scroll, so the
/// line under the title is the one a user sees after a click near the bottom
/// of a tall card. Content below moves down; a bordered card spanning the
/// line grows; the frame containers grow. The line keeps the card's own body
/// face (cloned from a text node) — the atlas red for an alert, the secondary
/// grey for a result.
fn append_notice(tree: &mut UiNode, text: &str, alert: bool, card: (f64, f64)) -> (f64, f64) {
    let (w, h) = card;
    let frames = frame_ids(tree);
    let is_frame =
        |n: &UiNode| n.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id));
    let mut proto: Option<UiNode> = None;
    let mut title: Option<(f64, f64, f64, f64)> = None;
    walk(tree, &mut |n| {
        if n.kind == NodeKind::Text && n.attrs.font_src.is_some() {
            if proto.is_none() {
                proto = Some(n.clone());
            }
            if n.attrs.text.as_deref().is_some_and(|t| !t.trim().is_empty()) {
                let r = rect(n);
                if title.is_none_or(|t| r.1 < t.1 - 0.5) {
                    title = Some(r);
                }
            }
        }
    });
    let (Some(mut node), Some((tx, ty, _, th))) = (proto, title) else { return card };
    // The bordered container holding the title bounds the line's width.
    let mut right = w - PAD_X;
    walk(tree, &mut |n| {
        let (x, y, cw, ch) = rect(n);
        if !is_frame(n) && n.kind == NodeKind::Stack && n.attrs.border.is_some()
            && tx >= x && tx <= x + cw && ty >= y && ty <= y + ch
        {
            right = right.min(x + cw - 12.0);
        }
    });
    let line_h = 40.0;
    let y0 = ty + th + 10.0;
    let delta = line_h + 6.0;
    // Containers that span the insertion line grow; everything below moves.
    walk_mut(tree, &mut |n| {
        let (_, y, _, ch) = rect(n);
        let frame = n.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id));
        if !frame && n.kind == NodeKind::Stack && y < y0 && y + ch > y0 {
            n.attrs.h = Some((ch + delta) as f32);
        }
    });
    shift_below(tree, y0, delta, &frames);
    node.children.clear();
    let a = &mut node.attrs;
    a.id = Some("dialog_notice".into());
    a.text = Some(text.to_owned());
    a.x = Some(tx);
    a.y = Some(y0);
    a.w = Some((right - tx).max(80.0) as f32);
    a.h = Some(line_h as f32);
    a.size = Some(13.5);
    a.weight = Some(400);
    a.color = Some(if alert { 0xffcf_222e } else { 0xff6e_6e73 });
    a.alignx = Some(0.0);
    a.variant = None;
    a.fillw = None;
    let grown = (w, h + delta);
    walk_mut(tree, &mut |n| {
        if n.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id)) {
            n.attrs.h = Some(grown.1 as f32);
        }
    });
    tree.children.push(node);
    grown
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
    // Each operation is gated on its own advertised method
    // (`model-settings.ts:211`: read-only when the method is absent).
    if !advertises(ctx.store, "profile/llm/test") {
        remove(tree, &["btn_test"]);
    }
    if !advertises(ctx.store, "profile/llm/fetch_models") {
        remove(tree, &["btn_discover"]);
    }
    seat_route_pills(tree);
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

/// The expanded provider card holds its route pills with the card's own
/// inset on every side. The atlas ended the card (y 110 h 290 → 400) exactly
/// where the 44 px pills end (y 356 → 400), so the card's border cut the
/// pills' bottoms (A10 judge: clipped on desktop and phone). The card grows
/// by the missing bottom inset (the pills' 16 px side inset) and every row
/// below it moves down by the same amount.
fn seat_route_pills(tree: &mut UiNode) {
    let Some((cx, cy, _, ch)) = rect_of(tree, "card_deepseek") else { return };
    let pills: Vec<(f64, f64, f64, f64)> =
        ["btn_test", "btn_discover"].iter().filter_map(|id| rect_of(tree, id)).collect();
    let Some(bottom) = pills.iter().map(|(_, y, _, h)| y + h).reduce(f64::max) else { return };
    let inset = pills.iter().map(|(x, _, _, _)| x - cx).reduce(f64::min).unwrap_or(16.0).max(12.0);
    let grow = (bottom + inset) - (cy + ch);
    if grow <= 0.5 {
        return;
    }
    let frames = frame_ids(tree);
    // Everything that starts below the card's old bottom edge moves first,
    // then the card grows (its own top is above the cut, so it stays put).
    shift_below(tree, cy + ch - 0.5, grow, &frames);
    set_h(tree, "card_deepseek", ch + grow);
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
        right_align(tree, value, right, 140.0);
        // The label box was measured for the atlas word ("Tools", 44px); it
        // runs up to the value's box, never under it.
        let vx = right - 140.0;
        if let Some(n) = find_mut(tree, label) {
            let x = n.attrs.x.unwrap_or(0.0);
            n.attrs.w = Some((vx - 8.0 - x).max(40.0) as f32);
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
    // Fail closed (`ContextPanel.tsx`: `compactAvailable` / `modeAvailable`):
    // a control the server does not advertise is not drawn at all.
    if !advertises(ctx.store, "session/compact") {
        remove(tree, &["btn_compact"]);
    }
    if !advertises(ctx.store, "session/compact/mode/set") {
        remove(tree, &["t_comp", "seg_box", "seg_div", "t_llm", "t_heur"]);
        return;
    }
    // The segmented control ends on the values' right edge (the atlas put it
    // 6 px past the bar/values column).
    if let Some((bx, _, bw, _)) = rect_of(tree, "seg_box") {
        let shift = right - (bx + bw);
        if shift.abs() > 0.5 {
            for id in ["seg_box", "seg_div", "t_llm", "t_heur"] {
                if let Some(n) = find_mut(tree, id) {
                    n.attrs.x = n.attrs.x.map(|x| x + shift);
                }
            }
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
    // Skills are the server Profile's (the web's "Server Profile: <id>" scope
    // line, `SkillsDialog.tsx:154`): the title names the Profile.
    if let Some(profile) = ctx.store.domains.profile.current() {
        if let Some(n) = find_mut(tree, "t_title") {
            n.attrs.text = Some(format!("Skills · {profile}"));
            n.attrs.w = Some(280.0);
        }
    }
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
    // The registry search (`SkillsDialog.tsx:203-226`): only when the server
    // advertises profile/skills/registry/search; the authored box becomes a
    // real input (Enter searches) holding the last searched query.
    if advertises(ctx.store, "profile/skills/registry/search") {
        if let Some(n) = find_mut(tree, "t_search") {
            n.kind = NodeKind::Input;
            let a = &mut n.attrs;
            a.id = Some("skills_query".to_owned());
            a.placeholder = Some("Search registry".to_owned());
            a.text = Some(skills_query().unwrap_or_default());
            a.color = Some(0xff1d_1d1f);
            a.w = Some(250.0);
            a.variant = None;
        }
    } else {
        remove(tree, &["search_box", "icon_search", "t_search"]);
    }
    // Registry: the searched packages (`search` folds them into the store).
    let m = registry.len().min(2);
    let searched = skills_query().is_some();
    if m == 0 && searched {
        // "No matching skill packages." in the first row's place.
        remove(tree, &["t_ver11", "btn_3_install", "t_name12", "t_ver13", "btn_4_install", "div_3"]);
        if let Some(t) = find_mut(tree, "t_name10") {
            t.attrs.text = Some("No matching skill packages.".to_owned());
            t.attrs.w = Some(300.0);
            t.attrs.color = Some(0xff6e_6e73);
        }
        if let Some((_, _, _, h)) = rect_of(tree, "card_registry") {
            set_h(tree, "card_registry", h - 70.0);
        }
    } else if m == 0 {
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
    // The web's installed row (`SkillsDialog.tsx:179-185`): the name, then
    // "<version> · N tools" and the source repo. The version slot becomes that
    // secondary line under the name (the pair centred on the row, the Remove
    // link beside it), so every field shows and nothing is fabricated.
    for i in 0..n {
        let s = &installed[i];
        let (name_id, ver_id) = (format!("t_name{}", 3 + i), format!("t_ver{}", 6 + i));
        let (Some((nx, ny, _, nh)), Some((rx, _, _, _))) =
            (rect_of(tree, &name_id), rect_of(tree, &format!("t_remove{i}")))
        else {
            continue;
        };
        let tools = if s.tool_count == 1 { "1 tool".to_owned() } else { format!("{} tools", s.tool_count) };
        let mut line = format!("{} · {tools}", s.version.as_deref().unwrap_or("Version not reported"));
        if let Some(repo) = s.source_repo.as_deref().filter(|r| !r.is_empty()) {
            line = format!("{line} · {repo}");
        }
        let (w, size) = (rx - 10.0 - nx, 12.5_f32);
        if let Some(n) = find_mut(tree, &name_id) {
            n.attrs.y = Some(ny - 9.0);
        }
        if let Some(n) = find_mut(tree, &ver_id) {
            let a = &mut n.attrs;
            a.text = Some(ellipsize(&line, w, size as f64));
            a.x = Some(nx);
            a.y = Some(ny - 9.0 + nh + 1.0);
            a.w = Some(w as f32);
            a.h = Some(17.0);
            a.size = Some(size);
            a.weight = Some(400);
            a.color = Some(0xff6e_6e73);
            a.alignx = Some(0.0);
            a.variant = None;
        }
    }
    // A registry row (`SkillsDialog.tsx:229-251`): the name, then the
    // version and licence (and the installed state) under it, ending before
    // the Install button.
    for (j, (name_id, ver_id, btn)) in
        [("t_name10", "t_ver11", "btn_3_install"), ("t_name12", "t_ver13", "btn_4_install")].iter().enumerate().take(m)
    {
        let pkg = &registry[j];
        let (Some((nx, ny, _, nh)), Some((bx, _, _, _))) = (rect_of(tree, name_id), rect_of(tree, btn)) else {
            continue;
        };
        let mut line = format!(
            "{} · {}",
            pkg.version.as_deref().unwrap_or("Version not reported"),
            pkg.license.as_deref().unwrap_or("License not reported")
        );
        if pkg.installed {
            line = format!("{line} · installed");
        }
        let (w, size) = (bx - 10.0 - nx, 12.5_f32);
        if let Some(n) = find_mut(tree, name_id) {
            n.attrs.y = Some(ny - 9.0);
        }
        if let Some(n) = find_mut(tree, ver_id) {
            let a = &mut n.attrs;
            a.text = Some(ellipsize(&line, w, size as f64));
            a.x = Some(nx);
            a.y = Some(ny - 9.0 + nh + 1.0);
            a.w = Some(w as f32);
            a.h = Some(17.0);
            a.size = Some(size);
            a.weight = Some(400);
            a.color = Some(0xff6e_6e73);
            a.alignx = Some(0.0);
            a.variant = None;
        }
    }
}

/// The goal's status word (`describeGoalStatus`, AutonomyPanel) and its badge
/// colours: active green (the authored badge), paused/blocked amber, terminal
/// grey.
fn goal_badge(status: &str) -> (String, u32, u32) {
    let (word, bg, ink) = match status {
        "active" => ("Active", 0xffe6_f6ea, 0xff28_7f3b),
        "paused" => ("Paused", 0xffff_f4e5, 0xff8a_5a00),
        "budget_limited" => ("Budget limited", 0xffff_f4e5, 0xff8a_5a00),
        "blocked" => ("Blocked", 0xffff_f4e5, 0xff8a_5a00),
        "complete" => ("Complete", 0xfff2_f2f4, 0xff61_666b),
        // `describeGoalStatus` returns an unknown status verbatim.
        other => (other, 0xfff2_f2f4, 0xff61_666b),
    };
    (word.to_owned(), bg, ink)
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
                "t_elapsed_val", "stop_btn", "clear_goal",
            ],
        );
        // The authored Pause pill becomes "Set goal", under the empty line.
        set_text(tree, "pause_btn_label", "Set goal");
        if let (Some((_, ty, _, th)), Some((_, py, _, _))) = (rect_of(tree, "t_goal"), rect_of(tree, "pause_btn")) {
            if let Some(b) = find_mut(tree, "pause_btn") {
                shift(b, 0.0, ty + th + 18.0 - py);
            }
        }
        // The card ends under the pill.
        if let (Some((_, cy, _, _)), Some((_, by, _, bh))) =
            (rect_of(tree, "goal_card"), rect_of(tree, "pause_btn"))
        {
            set_h(tree, "goal_card", by + bh + 22.0 - cy);
        }
        return;
    };
    let status = goal["status"].as_str().unwrap_or("active");
    let (word, bg, ink) = goal_badge(status);
    set_text(tree, "goal_badge_label", &word);
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
    for i in 1..=st.loops.len().min(3) {
        row_icons(tree, "loops_card", &format!("loop_{i}"), &["pause", "play", "trash"], Some("dot"));
    }
}

/// The row-icon glyph size (design px) every autonomy dialog draws: the
/// board's 24 px grid less the lowering's growth. The authored loop row mixed
/// a 30 px pause, a 29 px play and a 44 px trash (each grown 1.1x), so the
/// same 1.6-unit stroke rendered 2.0 / 1.9 / 2.9 px wide and the trash stood
/// ~36 px tall (A10 judge). One size gives one stroke (1.6 x 22/24 = 1.47 px,
/// the weight of the module's own 24-unit line icons at this size).
pub const ROW_ICON: f64 = 22.0;
/// Centre-to-centre pitch of a row's icons: the hit targets ([`MIN_HIT`])
/// tile without overlapping.
pub const ROW_ICON_PITCH: f64 = 36.0;

/// Seat a row's icons on one grid: every glyph [`ROW_ICON`] square, centred
/// on the row's text band (its first text line's top to its last line's
/// bottom), the last icon's right edge 16 px inside the card, the others
/// [`ROW_ICON_PITCH`] apart leftwards; `lead` (the status dot) one pitch
/// further left, its own size kept. Icons the live state removed are skipped
/// without leaving a hole.
fn row_icons(tree: &mut UiNode, card: &str, row: &str, icons: &[&str], lead: Option<&str>) {
    let Some((cx, _, cw, _)) = rect_of(tree, card) else { return };
    // The row's text band: every Text node of the row.
    let (mut top, mut bottom) = (f64::MAX, f64::MIN);
    walk(tree, &mut |n| {
        let mine = n.attrs.id.as_deref().is_some_and(|id| id.starts_with(&format!("{row}_")));
        if mine && n.kind == NodeKind::Text && n.attrs.text.as_deref().is_some_and(|t| !t.trim().is_empty()) {
            let (_, y, _, h) = rect(n);
            top = top.min(y);
            bottom = bottom.max(y + h);
        }
    });
    if top == f64::MAX {
        return;
    }
    let mid = (top + bottom) / 2.0;
    let present: Vec<String> = icons
        .iter()
        .map(|k| format!("{row}_{k}"))
        .filter(|id| find(tree, id).is_some())
        .collect();
    let right = cx + cw - 16.0;
    let n = present.len();
    for (k, id) in present.iter().enumerate() {
        // The rightmost icon ends at `right`; earlier ones step left.
        let centre_x = right - ROW_ICON / 2.0 - (n - 1 - k) as f64 * ROW_ICON_PITCH;
        if let Some(node) = find_mut(tree, id) {
            let a = &mut node.attrs;
            a.x = Some(centre_x - ROW_ICON / 2.0);
            a.y = Some(mid - ROW_ICON / 2.0);
            a.w = Some(ROW_ICON as f32);
            a.h = Some(ROW_ICON as f32);
        }
    }
    if let Some(lead) = lead {
        let id = format!("{row}_{lead}");
        // One pitch left of the FULL icon set's first column, so every row's
        // dot shares one column (a paused row has no pause icon, and its dot
        // must not drift right).
        let first_centre = right - ROW_ICON / 2.0 - (icons.len().max(1) - 1) as f64 * ROW_ICON_PITCH;
        if let Some(node) = find_mut(tree, &id) {
            let (_, _, w, h) = rect(node);
            node.attrs.x = Some(first_centre - ROW_ICON_PITCH - w / 2.0);
            node.attrs.y = Some(mid - h / 2.0);
        }
    }
}

/// autonomy-05 — a row's status line carries the pause reason
/// ("paused (user)"); its box runs to the interval column instead of the
/// atlas word's measured width ("fired 3×" clipped the live "paused (").
fn live_monitors(tree: &mut UiNode, st: &AutonomyState) {
    for i in 1..=st.monitors.len().min(3) {
        let (state, int) = (format!("mon_{i}_state"), format!("mon_{i}_int"));
        if let (Some((sx, _, _, _)), Some((ix, _, _, _))) = (rect_of(tree, &state), rect_of(tree, &int)) {
            if let Some(n) = find_mut(tree, &state) {
                n.attrs.w = Some((ix - 10.0 - sx).max(40.0) as f32);
            }
        }
        // The same glyph size as the Loops dialog's row icons (one icon set).
        for k in ["pause", "trash"] {
            let id = format!("mon_{i}_{k}");
            if let Some(n) = find_mut(tree, &id) {
                let (x, y, w, h) = rect(n);
                let (mx, my) = (x + w / 2.0, y + h / 2.0);
                n.attrs.x = Some(mx - ROW_ICON / 2.0);
                n.attrs.y = Some(my - ROW_ICON / 2.0);
                n.attrs.w = Some(ROW_ICON as f32);
                n.attrs.h = Some(ROW_ICON as f32);
            }
        }
    }
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

/// Shorten `text` with a trailing "…" so it fits `w` px at font `size` (an
/// Inter estimate, 0.5 em per character: "cargo test -p octos-cli
/// steer_queue" measures 216 px at 13, i.e. 0.475 em); fitting text is kept.
pub fn ellipsize(text: &str, w: f64, size: f64) -> String {
    let per = 0.5 * size.max(1.0);
    if (text.chars().count() as f64) * per <= w {
        return text.to_owned();
    }
    let keep = ((w / per).floor() as usize).saturating_sub(1).max(1);
    let mut s: String = text.chars().take(keep).collect::<String>().trim_end().to_owned();
    s.push('…');
    s
}

/// A mounted card is rebuilt whenever its lowered text changes, so a clock
/// that ticks every second (`formatElapsed`'s `42s` / `1m30s`) would remount
/// the dialog every second — flicker, and a click can land mid-remount. The
/// dialog shows elapsed time at MINUTE granularity: `<1m`, `1m`, `2h05m`.
pub fn minute_granularity(meta: &str) -> String {
    let (head, tail) = meta.split_once(" · ").map(|(h, t)| (h, Some(t))).unwrap_or((meta, None));
    let head = head.trim();
    let minutes = if let Some(s) = head.strip_suffix('s') {
        match s.split_once('m') {
            // "1m30s" → "1m"
            Some((m, _)) => format!("{m}m"),
            // "42s" → "<1m"
            None if s.chars().all(|c| c.is_ascii_digit()) => "<1m".to_owned(),
            None => head.to_owned(),
        }
    } else {
        head.to_owned()
    };
    match tail {
        Some(t) => format!("{minutes} · {t}"),
        None => minutes,
    }
}

/// autonomy-06 / -07: the goal heading spans the card (the atlas box fitted
/// its sample "Fix steer queue"); an empty task list ends under its line.
fn live_fleet_tasks(tree: &mut UiNode) {
    // A task row's command runs up to its status pill, never under it.
    for prefix in ["run_r", "done_r"] {
        for i in 0..8 {
            let (cmd, pill) = (format!("{prefix}{i}_cmd"), format!("{prefix}{i}_pill"));
            let (status, dur) = (format!("{prefix}{i}_status"), format!("{prefix}{i}_dur"));
            // No duration is reported (the slot stays blank): the status pill
            // takes the row's right end, and the command the room it leaves.
            let blank = find_mut(tree, &dur)
                .map(|n| n.attrs.text.as_deref().unwrap_or("").trim().is_empty())
                .unwrap_or(false);
            if let (true, Some((dx, _, dw, _)), Some((px, _, pw, _))) =
                (blank, rect_of(tree, &dur), rect_of(tree, &pill))
            {
                let shift = (dx + dw) - (px + pw);
                if shift > 0.5 {
                    for id in [&pill, &status] {
                        if let Some(n) = find_mut(tree, id) {
                            n.attrs.x = n.attrs.x.map(|x| x + shift);
                        }
                    }
                }
            }
            if let (Some((cx, _, _, _)), Some((px, _, _, _))) = (rect_of(tree, &cmd), rect_of(tree, &pill)) {
                if let Some(n) = find_mut(tree, &cmd) {
                    let w = n.attrs.w.unwrap_or(0.0) as f64;
                    if cx + w > px - 8.0 {
                        n.attrs.w = Some((px - 8.0 - cx).max(40.0) as f32);
                    }
                    // A command longer than its box ends in "…" (never clipped
                    // mid-glyph at the box edge).
                    let w = n.attrs.w.unwrap_or(0.0) as f64;
                    let size = n.attrs.size.unwrap_or(13.0) as f64;
                    if let Some(t) = n.attrs.text.as_mut() {
                        *t = ellipsize(t, w, size);
                    }
                }
            }
        }
    }
    walk_mut(tree, &mut |n| {
        let is_meta = n
            .attrs
            .id
            .as_deref()
            .is_some_and(|id| id.starts_with("peer_r") && id.ends_with("_meta"));
        if is_meta {
            if let Some(t) = n.attrs.text.as_mut() {
                *t = minute_granularity(t);
            }
        }
    });
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
            ctl("btn_compact_control", format!("{ACTION_ASK}context.compact_now")),
            ctl("seg_llm_hit", "context.mode.llm"),
            ctl("seg_heur_hit", "context.mode.heuristic"),
        ],
        Dialog::Skills => {
            let mut v = Vec::new();
            for i in 0..ctx.store.domains.profile.installed_skills().len().min(3) {
                v.push(ctl(format!("t_remove{i}"), format!("{ACTION_ASK}skills.remove_{i}")));
            }
            for (i, b) in [(3usize, "btn_3_install_control"), (4, "btn_4_install_control")] {
                v.push(ctl(b, format!("{ACTION_ASK}skills.install_{i}")));
            }
            v
        }
        Dialog::Goal => {
            let status = st
                .goal
                .as_ref()
                .and_then(|g| g["status"].as_str().map(str::to_owned))
                .unwrap_or_default();
            if st.goal.is_none() {
                // No goal: the pill is "Set goal" (the web's goal form).
                return vec![ctl("pause_btn_control", format!("{ACTION_FORM}goal.set"))];
            }
            let pause = if status == "active" { "goal.pause" } else { "goal.resume" };
            vec![
                ctl("pause_btn_control", pause),
                ctl("stop_btn_control", "goal.stop"),
                ctl("clear_goal", "goal.clear"),
            ]
        }
        Dialog::Loops => {
            let mut v = vec![ctl("new_loop_control", format!("{ACTION_FORM}loop.create"))];
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
        Dialog::Tasks => {
            // The generated run blocks (`fleet::rewrite_tasks_rows`): one
            // Cancel per running task, addressing its own row.
            let running = ctx
                .store
                .domains
                .task
                .snapshots()
                .into_iter()
                .filter(|t| t.state == "running")
                .count();
            (0..running)
                .map(|i| ctl(format!("run_r{i}_cancel_control"), format!("task.cancel#{i}")))
                .collect()
        }
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
        // The two synthetic halves of the context segmented control (absent
        // when the server does not advertise the mode method: not drawn).
        if c.node == "seg_llm_hit" || c.node == "seg_heur_hit" {
            if let (Some((bx, by, bw, bh)), Some((dx, _, _, _))) =
                (rect_of(tree, "seg_box"), rect_of(tree, "seg_div"))
            {
                let (x0, x1) = if c.node == "seg_llm_hit" { (bx, dx) } else { (dx, bx + bw) };
                let hit = hit_node(&c.node, (x0, by, x1 - x0, bh), &c.event, 0.0);
                if !insert_after(tree, "t_heur", hit) {
                    missing.push(c.clone());
                }
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

/// Whether a node DRAWS something: text with content, a vector, an image, or
/// a surface with a fill or border. Hit targets and bare groups draw nothing,
/// so they never decide a margin or a gap.
fn draws(n: &UiNode) -> bool {
    let (_, _, w, h) = rect(n);
    if w <= 0.5 || h <= 0.5 {
        return false;
    }
    match n.kind {
        NodeKind::Text => n.attrs.text.as_deref().is_some_and(|t| !t.trim().is_empty()),
        NodeKind::Svg | NodeKind::Image | NodeKind::Input => true,
        NodeKind::Button => false,
        _ => n.attrs.bg.is_some() || n.attrs.border.is_some(),
    }
}

/// The largest empty vertical band a dialog keeps between drawn rows. The
/// Stage-B cards are PHONE artboards that spread their rows over 776 px; in a
/// dialog the same rows with 70–110 px voids pushed each card's last action
/// below the fold (the goal's Clear goal, the context's Compact now). Bands
/// above this are closed to it — the web dialogs' own compact rhythm.
const MAX_GAP: f64 = 32.0;

/// Close every empty vertical band taller than [`MAX_GAP`]. Occupancy is every
/// drawing node, with a container's top and bottom EDGES counted as drawn, so
/// a cut never crosses a card border; containers that span a cut shrink with
/// it. Cuts apply bottom-up so earlier coordinates stay valid. Returns the
/// height removed.
fn squeeze(tree: &mut UiNode) -> f64 {
    let frames = frame_ids(tree);
    let mut spans: Vec<(f64, f64)> = Vec::new();
    walk(tree, &mut |n| {
        let frame = n.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id));
        if frame || !draws(n) {
            return;
        }
        let (_, y, _, h) = rect(n);
        let container = n.kind == NodeKind::Stack && n.children.iter().any(draws);
        if container {
            spans.push((y, y + 1.0));
            spans.push((y + h - 1.0, y + h));
        } else {
            spans.push((y, y + h));
        }
    });
    spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut bands: Vec<(f64, f64)> = Vec::new();
    for (s, e) in spans {
        match bands.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => bands.push((s, e)),
        }
    }
    let mut cuts: Vec<(f64, f64)> = Vec::new(); // (at, amount)
    for pair in bands.windows(2) {
        let gap = pair[1].0 - pair[0].1;
        if gap > MAX_GAP {
            cuts.push((pair[1].0, gap - MAX_GAP));
        }
    }
    let mut removed = 0.0;
    for (at, amount) in cuts.into_iter().rev() {
        // Containers spanning the cut shrink; everything at/below it rises.
        walk_mut(tree, &mut |n| {
            let frame = n.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id));
            let (_, y, _, h) = rect(n);
            if !frame && y < at - 0.5 && y + h > at + 0.5 {
                n.attrs.h = Some((h - amount) as f32);
            }
        });
        shift_below(tree, at, -amount, &frames);
        removed += amount;
    }
    removed
}

/// Crop the artboard to its content: translate the content to the padded
/// origin and size the frame containers to the content box. Frame containers
/// lose their fill (the dialog frame paints the surface and its rounded
/// corners). Returns the card size.
fn normalize(tree: &mut UiNode) -> (f64, f64) {
    let frames = frame_ids(tree);
    let is_frame = |n: &UiNode| n.attrs.id.as_deref().is_some_and(|id| frames.iter().any(|f| f == id));
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    walk(tree, &mut |n| {
        if is_frame(n) || !draws(n) {
            return;
        }
        let (x, y, w, h) = rect(n);
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

/// The autonomy family's recorded error (#P4e1b row 8: kept only while the
/// op stays authorized), shown as the dialog's alert line — the web renders
/// `goalError` / `loopsError` / `monitorsError` under its section with
/// `role="alert"` (`AutonomyPanel.tsx:227/:328/:485`).
fn family_error(d: Dialog, ctx: &Ctx<'_>) -> Option<(String, bool)> {
    let family = match d {
        Dialog::Goal => crate::screens::autonomy::FAMILY_GOAL,
        Dialog::Loops => crate::screens::autonomy::FAMILY_LOOPS,
        Dialog::Monitors => crate::screens::autonomy::FAMILY_MONITORS,
        _ => return None,
    };
    ctx.store.domains.autonomy.error(family).map(|e| (e, true))
}

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
        Dialog::Monitors => live_monitors(tree, st),
    }
    centre_button_labels(tree);
}

/// Lower dialog `d` for a host area of `avail_w × avail_h`.
/// The form card's two controls.
fn form_controls() -> Vec<Control> {
    vec![ctl("ff_cancel_control", ACTION_FORM_CANCEL), ctl("ff_submit_control", ACTION_FORM_SUBMIT)]
}

/// The form card: the title, one bordered real input per field (the Skills
/// card's search box face), the help line, the refusal line, and Cancel /
/// the primary submit pill — built like [`confirm_tree`] from the cards'
/// own faces. Card coordinates; `normalize` adds the margins.
fn form_tree(d: Dialog, ctx: &Ctx<'_>, st: &AutonomyState, f: &Form) -> Result<UiNode, String> {
    const W: f64 = 340.0;
    const FIELD_H: f64 = 48.0;
    let skills = card_tree(Dialog::Skills, ctx, st)?;
    let (Some(field_face), Some(input_face)) = (find(&skills, "search_box").cloned(), find(&skills, "t_search").cloned())
    else {
        return Err(format!("{}: the form's field faces are missing", d.card()));
    };
    let mut c = Confirm {
        dialog: d,
        action: f.action.clone(),
        title: f.title.clone(),
        detail: String::new(),
        body: String::new(),
        confirm_label: f.submit_label.clone(),
    };
    c.body = f.help.clone();
    // The confirm card gives the title, the help (as its body) and the pills;
    // the fields go between the title and the help.
    let mut tree = confirm_tree(d, ctx, st, &c)?;
    let title_bottom = rect_of(&tree, "cf_title").map(|(_, y, _, h)| y + h).unwrap_or(28.0);
    let mut y = title_bottom + 14.0;
    let mut nodes = Vec::new();
    for (id, placeholder, value) in &f.fields {
        let mut b = field_face.clone();
        b.children.clear();
        b.attrs.id = Some(format!("{id}_box"));
        b.attrs.x = Some(0.0);
        b.attrs.y = Some(y);
        b.attrs.w = Some(W as f32);
        b.attrs.h = Some(FIELD_H as f32);
        b.attrs.tapto = None;
        nodes.push(b);
        let mut i = input_face.clone();
        i.children.clear();
        i.kind = NodeKind::Input;
        let a = &mut i.attrs;
        a.id = Some(id.clone());
        a.placeholder = Some(placeholder.clone());
        a.text = Some(value.clone());
        a.color = Some(0xff1d_1d1f);
        a.x = Some(14.0);
        a.y = Some(y + (FIELD_H - 26.0) / 2.0);
        a.w = Some((W - 28.0) as f32);
        a.h = Some(26.0);
        a.variant = None;
        a.tapto = None;
        nodes.push(i);
        y += FIELD_H + 10.0;
    }
    let fields_h = y - (title_bottom + 14.0);
    // Everything under the title moves down by the fields' height.
    for n in tree.children.iter_mut() {
        if n.attrs.id.as_deref() != Some("cf_title") {
            shift(n, 0.0, fields_h);
        }
    }
    let mut extra = 0.0;
    if let Some(err) = &f.error {
        // The refusal, above the pills (the web's role="alert" line).
        let pills_y = rect_of(&tree, "cf_cancel").map(|(_, y, _, _)| y).unwrap_or(y);
        for id in ["cf_cancel", "cf_confirm"] {
            if let Some(n) = find_mut(&mut tree, id) {
                shift(n, 0.0, 30.0);
            }
        }
        let mut e = input_face.clone();
        e.children.clear();
        e.kind = NodeKind::Text;
        let a = &mut e.attrs;
        a.id = Some("ff_error".to_owned());
        a.text = Some(err.clone());
        a.placeholder = None;
        a.x = Some(0.0);
        a.y = Some(pills_y - 6.0);
        a.w = Some(W as f32);
        a.h = Some(20.0);
        a.size = Some(13.5);
        a.weight = Some(400);
        a.color = Some(0xffcf_222e);
        a.variant = None;
        a.tapto = None;
        nodes.push(e);
        extra = 30.0;
    }
    tree.children.extend(nodes);
    // The confirm card's ids become the form's.
    walk_mut(&mut tree, &mut |n| {
        if let Some(id) = n.attrs.id.as_mut() {
            if let Some(rest) = id.strip_prefix("cf_confirm") {
                *id = format!("ff_submit{rest}");
            } else if let Some(rest) = id.strip_prefix("cf_cancel") {
                *id = format!("ff_cancel{rest}");
            }
        }
    });
    let h = tree.attrs.h.unwrap_or(0.0) as f64 + fields_h + extra;
    tree.attrs.h = Some(h as f32);
    Ok(tree)
}

/// The confirm card's two controls.
fn confirm_controls() -> Vec<Control> {
    vec![ctl("cf_cancel_control", ACTION_CANCEL), ctl("cf_confirm_control", ACTION_CONFIRM)]
}

/// The confirm card, drawn with the dialogs' own faces: the dialog's title
/// text, the Context card's body text and its kit pill (outlined Cancel,
/// filled Confirm — the primary). Card coordinates; `normalize` adds the
/// margins.
fn confirm_tree(d: Dialog, ctx: &Ctx<'_>, st: &AutonomyState, c: &Confirm) -> Result<UiNode, String> {
    const W: f64 = 340.0;
    const GAP: f64 = 12.0;
    const BTN_H: f64 = 44.0;
    let own = card_tree(d, ctx, st)?;
    let faces = if d == Dialog::Context { own.clone() } else { card_tree(Dialog::Context, ctx, st)? };
    let title_face = find(&own, "t_title").or_else(|| find(&faces, "t_title")).cloned();
    let body_face = find(&faces, "t_usage").cloned();
    let pill = find(&faces, "btn_compact").cloned();
    let (Some(title_face), Some(body_face), Some(pill)) = (title_face, body_face, pill) else {
        return Err(format!("{}: the confirm card's faces are missing", d.card()));
    };
    // `w`: the text box width. The title's stops short of the close button
    // (`clear_close` would otherwise push a box under it to the left).
    let text = |face: &UiNode, id: &str, t: &str, y: f64, w: f64, size: f32, weight: i32, color: u32| -> (UiNode, f64) {
        let mut n = face.clone();
        n.children.clear();
        let per_line = (w / (0.5 * size as f64)).floor().max(1.0);
        let lines = (t.chars().count() as f64 / per_line).ceil().max(1.0);
        let h = (lines * size as f64 * 1.4).ceil();
        let a = &mut n.attrs;
        a.id = Some(id.to_owned());
        a.text = Some(t.to_owned());
        a.x = Some(0.0);
        a.y = Some(y);
        a.w = Some(w as f32);
        a.h = Some(h as f32);
        a.size = Some(size);
        a.line_height = None;
        a.weight = Some(weight);
        a.color = Some(color);
        a.alignx = Some(0.0);
        a.variant = None;
        a.fillw = None;
        a.tapto = None;
        (n, h)
    };
    let button = |id: &str, label: &str, x: f64, y: f64, w: f64, primary: bool| -> UiNode {
        let mut b = pill.clone();
        b.attrs.id = Some(id.to_owned());
        b.attrs.x = Some(x);
        b.attrs.y = Some(y);
        b.attrs.w = Some(w as f32);
        b.attrs.h = Some(BTN_H as f32);
        for ch in &mut b.children {
            let old = ch.attrs.id.clone().unwrap_or_default();
            let suffix = old.strip_prefix("btn_compact").unwrap_or("").to_owned();
            let a = &mut ch.attrs;
            a.id = Some(format!("{id}{suffix}"));
            a.x = Some(x);
            a.w = Some(w as f32);
            a.tapto = None;
            match suffix.as_str() {
                "_surface" | "_control" => {
                    a.y = Some(y);
                    a.h = Some(BTN_H as f32);
                    if primary && suffix == "_surface" {
                        a.bg = Some(0xff1d_1d1f);
                        a.border = None;
                    }
                }
                "_label" => {
                    let lh = 22.0;
                    a.h = Some(lh as f32);
                    a.y = Some(y + (BTN_H - lh) / 2.0);
                    a.size = Some(15.0);
                    a.line_height = None;
                    a.text = Some(label.to_owned());
                    a.alignx = Some(0.5);
                    a.weight = Some(if primary { 600 } else { 500 });
                    if primary {
                        a.color = Some(0xffff_ffff);
                    }
                }
                _ => {}
            }
        }
        b
    };
    let mut page = own;
    page.children.clear();
    let title_size = title_face.attrs.size.unwrap_or(20.0).min(20.0);
    let (title, h) = text(&title_face, "cf_title", &c.title, 0.0, W - 48.0, title_size, 700, 0xff1d_1d1f);
    page.children.push(title);
    let mut y = h + 14.0;
    if !c.detail.is_empty() {
        let (detail, h) = text(&body_face, "cf_detail", &c.detail, y, W, 16.0, 600, 0xff1d_1d1f);
        page.children.push(detail);
        y += h + 8.0;
    }
    let (body, h) = text(&body_face, "cf_body", &c.body, y, W, 14.5, 400, 0xff6e_6e73);
    page.children.push(body);
    y += h + 22.0;
    let half = (W - GAP) / 2.0;
    page.children.push(button("cf_cancel", "Cancel", 0.0, y, half, false));
    page.children.push(button("cf_confirm", &c.confirm_label, half + GAP, y, half, true));
    page.attrs.w = Some(W as f32);
    page.attrs.h = Some((y + BTN_H) as f32);
    Ok(page)
}

pub fn lower(d: Dialog, ctx: &Ctx<'_>, avail_w: f64, avail_h: f64) -> Result<Mounted, String> {
    let st = autonomy_view(ctx);
    let confirm = pending_confirm().filter(|c| c.dialog == d);
    let form = pending_form().filter(|f| f.dialog == d);
    let (mut tree, ctrls) = match (&confirm, &form) {
        (Some(c), _) => (confirm_tree(d, ctx, &st, c)?, confirm_controls()),
        (None, Some(f)) => (form_tree(d, ctx, &st, f)?, form_controls()),
        (None, None) => {
            let mut tree = card_tree(d, ctx, &st)?;
            live(d, &mut tree, ctx, &st);
            squeeze(&mut tree);
            (tree, controls(d, ctx, &st))
        }
    };
    let missing = wire(&mut tree, &ctrls);
    let (cw, ch) = normalize(&mut tree);
    let frames = frame_ids(&tree);
    let slot = close_slot(&tree, cw, &frames);
    clear_close(&mut tree, slot, &frames);
    let notice = if confirm.is_some() || form.is_some() {
        None
    } else {
        notice_tone(d).or_else(|| family_error(d, ctx))
    };
    let (cw, ch) = match notice {
        Some((text, alert)) => append_notice(&mut tree, &text, alert, (cw, ch)),
        None => (cw, ch),
    };

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
    // `bar`: the scroll handle. The default handle is `theme.color_outset`
    // (white): invisible on the white frame except where it poked out past
    // the rounded top-right corner. A grey handle inset past the corner
    // radius (bar_side_margin) shows only when the card overflows.
    let (surface, edge, scrim, bar) = if dark {
        ("#1c1f22", "#3a3a3c", "#00000080", "#5a5a5e")
    } else {
        ("#ffffff", "#e5e5e7", "#1d1d1f40", "#c7c7cc")
    };
    format!(
        "dialog_root := View {{ width: Fill height: Fill flow: Overlay\n\
         dialog_scrim := SolidView {{ width: Fill height: Fill draw_bg.color: {scrim} }}\n\
         dialog_backdrop := Button {{ width: Fill height: Fill text: \"\" draw_bg.color: #00000000 draw_bg.color_hover: #00000000 draw_bg.color_down: #00000000 draw_bg.border_size: 0.0 draw_bg.color_2: #00000000 draw_bg.border_color: #00000000 draw_bg.border_color_2: #00000000 }}\n\
         View {{ width: Fill height: Fill align: Align{{x: 0.5 y: 0.5}} flow: Overlay\n\
         dialog_frame := RoundedView {{ width: {fw} height: {fh} flow: Overlay\n\
         draw_bg +: {{color: {surface} border_radius: {radius} border_size: {border} border_color: {edge}}}\n\
         dialog_scroll := ScrollYView {{ width: Fill height: Fill flow: Down\n\
         scroll_bars.scroll_bar_y.bar_side_margin: {bar_margin}\n\
         scroll_bars.scroll_bar_y.draw_bg.color: {bar}\n\
         scroll_bars.scroll_bar_y.draw_bg.color_hover: {bar}\n\
         scroll_bars.scroll_bar_y.draw_bg.color_drag: {bar}\n\
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
        bar_margin = radius + 4.0,
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
    // A5: the board-3 commands' gates (r1's open reply advertises them).
    "session/rollback", "thread/graph/get", "turn/state/get", "approval/scopes/list",
    "tool/status/list", "mcp/status/list",
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
    // The recorded server Profile (the r1/r2 recordings' `dsflash`).
    if store.domains.profile.current().is_none() {
        store.domains.profile.set_current("dsflash".into());
    }
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
    let skill = |name: &str, version: &str, tools: u64| InstalledSkill {
        name: name.into(),
        version: Some(version.into()),
        tool_count: tools,
        source_repo: Some(format!("octos/{name}")),
    };
    store.domains.profile.set_installed_skills(vec![
        skill("rust-review", "1.2.0", 3),
        skill("git-helper", "0.9.1", 2),
        skill("docs-writer", "2.0.0", 1),
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
    squeeze(&mut tree);
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

    /// The dialog state + the autonomy cache are process statics; the tests
    /// that read them run one at a time.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static L: StdMutex<()> = StdMutex::new(());
        L.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn full() -> (Arc<Store>, StdMutex<FlowUi>) {
        let store = Arc::new(Store::new());
        store.domains.session.set_active(Some("dsflash:main".into()));
        seed_fixture(&store);
        (store, StdMutex::new(FlowUi::default()))
    }

    fn empty() -> (Arc<Store>, StdMutex<FlowUi>) {
        crate::screens::autonomy::reset_state();
        let store = Arc::new(Store::new());
        store.domains.session.set_active(Some("dsflash:main".into()));
        (store, StdMutex::new(FlowUi::default()))
    }

    fn events(m: &Mounted) -> Vec<String> {
        m.taps.iter().map(|(_, e)| e.clone()).collect()
    }

    /// Every dialog, on the full fixture state, wires EVERY control it draws to
    /// its owner's action id (by node id — no atlas-bounds matching), plus the
    /// close button; nothing drawn is left dead.
    #[test]
    fn every_dialog_wires_every_drawn_control_on_the_full_state() {
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        let want: &[(Dialog, &[&str])] = &[
            (Dialog::Models, &["models.test_route", "models.discover"]),
            // Compact / Remove / Install ASK first (the confirm card).
            (Dialog::Context, &["dialog.ask.context.compact_now", "context.mode.llm", "context.mode.heuristic"]),
            (
                Dialog::Skills,
                &[
                    "dialog.ask.skills.remove_0", "dialog.ask.skills.remove_1", "dialog.ask.skills.remove_2",
                    "dialog.ask.skills.install_3", "dialog.ask.skills.install_4",
                ],
            ),
            (Dialog::Goal, &["goal.pause", "goal.stop", "goal.clear"]),
            (
                Dialog::Loops,
                &[
                    "dialog.form.loop.create", "loop.pause#0", "loop.fire_now#0", "loop.delete#0", "loop.pause#1",
                    "loop.fire_now#1", "loop.delete#1", "loop.resume#2", "loop.delete#2",
                ],
            ),
            (Dialog::Monitors, &["monitor.pause#0", "monitor.delete#0", "monitor.pause#1", "monitor.delete#1"]),
            (Dialog::Fleet, &["peer.steer#0", "peer.steer#1", "peer.steer#2"]),
            (Dialog::Tasks, &["task.cancel#0"]),
            (Dialog::Review, &["review.start"]),
        ];
        for (d, evs) in want {
            let m = lower(*d, &ctx, 990.0, 603.0).unwrap_or_else(|e| panic!("{d:?}: {e}"));
            assert!(m.missing.is_empty(), "{d:?} dead controls: {:?}", m.missing);
            let got = events(&m);
            for e in *evs {
                assert!(got.iter().any(|g| g == e), "{d:?} must wire {e}; got {got:?}");
            }
            assert!(got.iter().any(|g| g == ACTION_CLOSE), "{d:?} has a close button");
            // The paused third loop has NO pause control (web: Pause only
            // while active, Resume only while paused).
            if *d == Dialog::Loops {
                assert!(!got.iter().any(|g| g == "loop.pause#2"), "{got:?}");
            }
        }
    }

    /// The web's explicit confirm steps: Compact now / Remove / Install show
    /// a confirm card (in the web's words, naming the Profile and the skill)
    /// whose Confirm runs exactly the asked action and whose Cancel returns.
    #[test]
    fn compact_remove_and_install_ask_before_they_run() {
        let _s = serial();
        let (store, ui) = full();
        store.domains.profile.set_current("dsflash".into());
        let ctx = Ctx::new(&store, &ui);
        assert_eq!(resolve("dialog.ask.skills.remove_1"), Effect::Ask("skills.remove_1".into()));
        assert_eq!(resolve(ACTION_CONFIRM), Effect::Confirm);
        assert_eq!(resolve(ACTION_CANCEL), Effect::Cancel);
        assert_eq!(resolve("dialog.ask."), Effect::Unhandled("dialog.ask.".into()));

        let remove = confirmation_for("skills.remove_1", &store).expect("remove asks");
        assert_eq!(remove.dialog, Dialog::Skills);
        assert_eq!(remove.detail, store.domains.profile.installed_skills()[1].name);
        assert!(remove.body.contains("Applies to Profile dsflash"), "{}", remove.body);
        let install = confirmation_for("skills.install_3", &store).expect("install asks");
        assert!(install.detail.ends_with("· branch main") && install.body.contains("executable tools"));
        let compact = confirmation_for("context.compact_now", &store).expect("compact asks");
        assert!(compact.body.starts_with("Compact this session now?"));
        assert!(confirmation_for("skills.remove_9", &store).is_none(), "a gone row asks nothing");
        assert!(confirmation_for("goal.clear", &store).is_none());

        open(Dialog::Skills);
        set_confirm(Some(remove.clone()));
        let m = lower(Dialog::Skills, &ctx, 990.0, 603.0).expect("confirm card lowers");
        assert!(m.missing.is_empty(), "{:?}", m.missing);
        let got = events(&m);
        assert!(got.contains(&ACTION_CONFIRM.to_owned()) && got.contains(&ACTION_CANCEL.to_owned()), "{got:?}");
        assert!(!got.iter().any(|e| e.starts_with("dialog.ask.")), "the card replaces the dialog's own");
        assert!(m.dsl.contains("Confirm removal") && m.dsl.contains(&remove.detail));
        let phone = lower(Dialog::Skills, &ctx, 360.0, 776.0).expect("phone confirm");
        assert!(phone.frame.0 <= 360.0 + 0.5);
        // Another dialog's card is not replaced by this confirmation.
        assert!(!lower(Dialog::Context, &ctx, 990.0, 603.0).unwrap().dsl.contains("Confirm removal"));
        // Open / close drop a pending confirmation.
        apply(&Effect::Close);
        assert!(pending_confirm().is_none());
    }

    /// An installed skill row shows every field the web shows
    /// (`SkillsDialog.tsx:179-185`): name, then "<version> · N tools" and the
    /// source repo on the line under it, ending before the Remove link.
    #[test]
    fn an_installed_skill_shows_version_tools_and_repo() {
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        let (tree, _) = live_tree(Dialog::Skills, &ctx).expect("skills");
        for (i, s) in store.domains.profile.installed_skills().iter().take(3).enumerate() {
            let line = find(&tree, &format!("t_ver{}", 6 + i)).and_then(|n| n.attrs.text.clone()).unwrap_or_default();
            let want = format!(
                "{} · {} tool{} · {}",
                s.version.as_deref().unwrap(),
                s.tool_count,
                if s.tool_count == 1 { "" } else { "s" },
                s.source_repo.as_deref().unwrap()
            );
            assert_eq!(line, want);
            let (x, _, w, _) = rect_of(&tree, &format!("t_ver{}", 6 + i)).unwrap();
            let (rx, _, _, _) = rect_of(&tree, &format!("t_remove{i}")).unwrap();
            assert!(x + w <= rx - 9.9, "the line ends before Remove");
        }
    }

    /// The autonomy create forms (the web's goal form and
    /// LoopCreationControls): + New loop / Set goal open a card of REAL
    /// inputs whose Create composes the entry text the autonomy table
    /// parses; a refusal is drawn on the form; Cancel / close drop it.
    #[test]
    fn the_create_forms_are_real_inputs_inside_their_dialogs() {
        use crate::screens::autonomy as au;
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        set_form(None);
        let f = form_for("loop.create", "Run CI smoke | 15m").expect("loop form");
        assert_eq!(f.dialog, Dialog::Loops);
        assert_eq!(f.fields[0].2, "Run CI smoke");
        assert_eq!(f.fields[1].2, "15m");
        let v = |a: &str, b: &str| form_value(&[a.to_owned(), b.to_owned()]);
        assert_eq!(v("Run CI smoke", "15m"), "Run CI smoke | 15m");
        assert_eq!(v("Run CI smoke", " "), "Run CI smoke");
        assert_eq!(v("", "15m"), "", "the first field is required");
        assert_eq!(
            au::resolve("loop.create", 0, Some(&v("Run CI smoke", "15m")), &ctx),
            au::Effect::LoopCreate { prompt: "Run CI smoke".into(), interval_seconds: Some(900) }
        );
        assert_eq!(resolve("dialog.form.loop.create"), Effect::OpenForm("loop.create".into()));
        assert_eq!(resolve(ACTION_FORM_SUBMIT), Effect::SubmitForm);
        assert_eq!(resolve(ACTION_FORM_CANCEL), Effect::CancelForm);
        open(Dialog::Loops);
        set_form(Some(f.clone()));
        let m = lower(Dialog::Loops, &ctx, 990.0, 603.0).expect("form lowers");
        assert!(m.missing.is_empty(), "{:?}", m.missing);
        assert!(m.dsl.contains("dlg_loops_lf_prompt := DesignInput"));
        assert!(m.dsl.contains("dlg_loops_lf_interval := DesignInput"));
        assert!(m.dsl.contains("empty_text: \"Interval, e.g. 15m\""));
        assert!(m.dsl.contains("text: \"Run CI smoke\""));
        let got = events(&m);
        assert!(got.contains(&ACTION_FORM_SUBMIT.to_owned()), "{got:?}");
        assert!(got.contains(&ACTION_FORM_CANCEL.to_owned()), "{got:?}");
        assert!(!got.iter().any(|e| e.starts_with("loop.")), "the form replaces the list");
        let mut refused = f.clone();
        refused.error = Some("Type what the loop runs first.".into());
        set_form(Some(refused));
        assert!(lower(Dialog::Loops, &ctx, 990.0, 603.0).unwrap().dsl.contains("Type what the loop runs first."));
        assert!(lower(Dialog::Loops, &ctx, 360.0, 776.0).unwrap().frame.0 <= 360.5, "the phone sheet");
        apply(&Effect::Close);
        assert!(pending_form().is_none(), "close drops the form");
        // No goal: the Goal dialog's pill is Set goal, opening the goal form.
        au::reset_state();
        let goal = lower(Dialog::Goal, &ctx, 990.0, 603.0).unwrap();
        assert!(events(&goal).contains(&"dialog.form.goal.set".to_owned()), "{:?}", events(&goal));
        assert!(goal.dsl.contains("\"Set goal\""));
        let g = form_for("goal.set", "").unwrap();
        assert_eq!(g.fields.len(), 2);
        assert_eq!(
            au::resolve("goal.set", 0, Some(&v("Ship it", "2000")), &ctx),
            au::Effect::SetGoal { objective: "Ship it".into(), token_budget: Some(2000) }
        );
    }

    /// The registry search box is a real input only when the server
    /// advertises the search (fail closed: no box otherwise); it keeps the
    /// last searched query, and an empty result says so (`SkillsDialog.tsx:222`).
    #[test]
    fn the_registry_search_is_a_real_input_and_an_empty_result_says_so() {
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        set_skills_query(None);
        let m = lower(Dialog::Skills, &ctx, 990.0, 603.0).unwrap();
        assert!(m.dsl.contains("dlg_skills_skills_query := DesignInput"), "a real input");
        assert!(m.dsl.contains("empty_text: \"Search registry\""));
        assert!(m.dsl.contains("1.1.0 · License not reported"), "a registry row's detail line");
        set_skills_query(Some("zzz".into()));
        store.domains.profile.set_registry_packages(vec![]);
        let m = lower(Dialog::Skills, &ctx, 990.0, 603.0).unwrap();
        assert!(m.dsl.contains("No matching skill packages."));
        assert!(m.dsl.contains("text: \"zzz\""), "the input keeps the searched query");
        set_skills_query(None);
        let m = lower(Dialog::Skills, &ctx, 990.0, 603.0).unwrap();
        assert!(!m.dsl.contains("No matching skill packages.") && !m.dsl.contains("\"Registry\""));
        store.domains.config.set_supported_methods(vec!["profile/skills/list".into()]);
        let m = lower(Dialog::Skills, &ctx, 990.0, 603.0).unwrap();
        assert!(!m.dsl.contains("DesignInput") && !m.dsl.contains("Search registry"), "fail closed");
    }

    /// With nothing folded, no dialog shows the atlas SAMPLE copy: each
    /// shows the web's own empty-state line (or the live value) instead.
    #[test]
    fn an_empty_store_never_shows_the_atlas_sample_data() {
        let _s = serial();
        let (store, ui) = empty();
        let ctx = Ctx::new(&store, &ui);
        let dsl = |d| lower(d, &ctx, 990.0, 603.0).unwrap().dsl;
        let models = dsl(Dialog::Models);
        assert!(models.contains("No models are configured for this Profile."));
        for sample in ["3 models", "Kimi Coding Plan", "GLM Coding Plan", "deepseek-v4-pro"] {
            assert!(!models.contains(sample), "models shows the sample {sample:?}");
        }
        let context = dsl(Dialog::Context);
        // (`text: "…"` — the bare word also appears in the macOS emoji font
        // path `/System/Library/Fonts/…` the lowering names.)
        for sample in [
            "text: \"System\"",
            "text: \"Conversation\"",
            "text: \"Tools\"",
            "\"8k\"",
            "\"96k\"",
            "Keeps the last 4 turns",
        ] {
            assert!(!context.contains(sample), "context shows the sample {sample:?}");
        }
        assert!(context.contains("\"Items\"") && context.contains("\"Recovery\""));
        let skills = dsl(Dialog::Skills);
        assert!(skills.contains("No skills installed in this Profile."));
        for sample in ["rust-review", "code-linter", "api-client"] {
            assert!(!skills.contains(sample), "skills shows the sample {sample:?}");
        }
        let goal = dsl(Dialog::Goal);
        assert!(goal.contains("No active goal for this session."));
        assert!(!goal.contains("41k of 100k") && !goal.contains("\"Pause\""));
        let review = dsl(Dialog::Review);
        assert!(!review.contains("Reviewing 3 files"), "review shows the sample run");
    }

    /// Desktop: the frame fits the host with the web's 16 px backdrop margin
    /// and the card keeps its design size; phone (360 wide): a full-bleed
    /// sheet, the card scaled to the width. Every node stays inside the card
    /// and every hit target is at least 28 px.
    #[test]
    fn the_frame_fits_desktop_and_phone_and_nothing_spills() {
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        for d in ALL {
            let m = lower(*d, &ctx, 990.0, 603.0).unwrap();
            assert_eq!(m.scale, 1.0, "{d:?} desktop keeps the design size");
            assert!(m.frame.0 <= 990.0 - 32.0 && m.frame.1 <= 603.0 - 32.0 + 0.5, "{d:?} {:?}", m.frame);
            let p = lower(*d, &ctx, 360.0, 780.0).unwrap();
            assert_eq!(p.frame, (360.0, 780.0), "{d:?} phone sheet");
            assert!(p.scale <= 1.0 && p.scale > 0.8, "{d:?} phone scale {}", p.scale);
            let (tree, _) = live_tree(*d, &ctx).unwrap();
            let (_, _, cw, _) = rect(&tree);
            walk(&tree, &mut |n| {
                let (x, _, w, h) = rect(n);
                if w > 0.5 && h > 0.5 {
                    assert!(x >= -0.5 && x + w <= cw + 0.5, "{d:?} {:?} spills [{x},{w}] of {cw}", n.attrs.id);
                }
                if n.kind == NodeKind::Button && n.attrs.tapto.is_some() {
                    assert!(w >= 28.0 && h >= 28.0, "{d:?} {:?} hit {w}x{h} < 28", n.attrs.id);
                }
            });
        }
    }

    /// A10 judge fix 1: the Models dialog's route pills sit INSIDE the
    /// expanded provider card with the card's own inset under them (the atlas
    /// ended the card on the pills' bottom edge, so its border cut them), and
    /// the collapsed provider cards below keep their gap.
    #[test]
    fn the_route_pills_sit_inside_their_provider_card() {
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        let (tree, _) = live_tree(Dialog::Models, &ctx).expect("models");
        let card = rect_of(&tree, "card_deepseek").expect("the expanded card");
        for id in ["btn_test", "btn_discover"] {
            let (x, y, w, h) = rect_of(&tree, id).unwrap_or_else(|| panic!("{id}"));
            let bottom_inset = (card.1 + card.3) - (y + h);
            let side_inset = (x - card.0).min((card.0 + card.2) - (x + w));
            assert!(bottom_inset >= 12.0, "{id}: bottom inset {bottom_inset} (card {card:?})");
            assert!((bottom_inset - side_inset).abs() <= 2.5, "{id}: bottom {bottom_inset} vs side {side_inset}");
        }
        let kimi = rect_of(&tree, "card_kimi").expect("the second provider");
        assert!(kimi.1 >= card.1 + card.3 + 12.0, "the next card keeps its gap: {kimi:?} after {card:?}");
    }

    /// A10 judge fix 2: every Loops row icon is ONE square size (so one
    /// stroke weight), on the row's centre line, at one pitch; the Monitors
    /// dialog draws the same size.
    #[test]
    fn loop_and_monitor_row_icons_are_one_size_on_one_grid() {
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        let (tree, _) = live_tree(Dialog::Loops, &ctx).expect("loops");
        let mut dots = Vec::new();
        for i in 1..=3 {
            let icons: Vec<(f64, f64, f64, f64)> = ["pause", "play", "trash"]
                .iter()
                .filter_map(|k| rect_of(&tree, &format!("loop_{i}_{k}")))
                .collect();
            assert!(icons.len() >= 2, "row {i}: {icons:?}");
            for (_, _, w, h) in &icons {
                assert!((*w - ROW_ICON).abs() < 0.01 && (*h - ROW_ICON).abs() < 0.01, "row {i}: {icons:?}");
            }
            let mids: Vec<f64> = icons.iter().map(|(_, y, _, h)| y + h / 2.0).collect();
            assert!(mids.iter().all(|m| (m - mids[0]).abs() < 0.01), "row {i} centre line {mids:?}");
            for pair in icons.windows(2) {
                assert!(((pair[1].0 - pair[0].0) - ROW_ICON_PITCH).abs() < 0.01, "row {i} pitch {icons:?}");
            }
            dots.push(rect_of(&tree, &format!("loop_{i}_dot")).expect("dot").0);
        }
        assert!(dots.iter().all(|x| (x - dots[0]).abs() < 0.01), "one dot column {dots:?}");
        let (tree, _) = live_tree(Dialog::Monitors, &ctx).expect("monitors");
        for id in ["mon_1_pause", "mon_1_trash", "mon_2_pause", "mon_2_trash"] {
            let (_, _, w, h) = rect_of(&tree, id).unwrap_or_else(|| panic!("{id}"));
            assert!((w - ROW_ICON).abs() < 0.01 && (h - ROW_ICON).abs() < 0.01, "{id} {w}x{h}");
        }
    }

    /// The close square never sits on a title-row control (`+ New loop`,
    /// `Start native review`): those move left of it.
    #[test]
    fn the_close_button_never_covers_a_title_row_control() {
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        for (d, control) in [(Dialog::Loops, "new_loop"), (Dialog::Review, "start_review")] {
            let (tree, _) = live_tree(d, &ctx).unwrap();
            let frames = frame_ids(&tree);
            let (cx, cy) = close_slot(&tree, rect(&tree).2, &frames);
            let (x, y, w, h) = rect_of(&tree, control).unwrap();
            let overlap = x < cx + CLOSE_SIZE && x + w > cx && y < cy + CLOSE_SIZE && y + h > cy;
            assert!(!overlap, "{d:?}: {control} [{x},{y},{w},{h}] under the close at ({cx},{cy})");
        }
    }

    /// The web's command → surface table (`App.tsx:1339-1495` intents), and
    /// the host's own ids resolve one owner each.
    #[test]
    fn commands_open_the_web_surfaces_and_the_host_ids_resolve() {
        for (cmd, d) in [
            ("/model", Dialog::Models),
            ("/context", Dialog::Context),
            ("/compact", Dialog::Context),
            ("ctx", Dialog::Context),
            ("/skills", Dialog::Skills),
            ("/goal", Dialog::Goal),
            ("/loop", Dialog::Loops),
            ("/monitor", Dialog::Monitors),
            ("/peer", Dialog::Fleet),
            ("/ps", Dialog::Tasks),
            ("/tasks", Dialog::Tasks),
            ("/review", Dialog::Review),
            ("/code-review", Dialog::Review),
        ] {
            assert_eq!(for_command(cmd), Some(d), "{cmd}");
        }
        assert_eq!(for_command("/help"), None);
        for d in ALL {
            assert_eq!(resolve(&open_action(*d)), Effect::Open(*d));
            assert!(is_action(&open_action(*d)));
        }
        assert_eq!(resolve(ACTION_CLOSE), Effect::Close);
        assert!(!is_action("dialog.open.nope") && !is_action("goal.pause"));
        // Every palette row that opens a dialog names a real dialog id.
        for c in crate::screens::palette::COMMANDS {
            if let Some(e) = c.effect.filter(|e| e.starts_with("dialog.")) {
                assert!(matches!(resolve(e), Effect::Open(_)), "{} -> {e}", c.name);
            }
        }
    }

    /// The fail-closed notice: a refused create explains itself in the open
    /// dialog, and opening/closing clears it.
    #[test]
    fn a_refused_control_leaves_a_notice_in_its_dialog() {
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        open(Dialog::Loops);
        set_notice(notice_for_refusal("loop.create[empty]").unwrap());
        let m = lower(Dialog::Loops, &ctx, 990.0, 603.0).unwrap();
        assert!(m.dsl.contains("Type what the loop runs first."), "the notice renders");
        let plain = {
            clear_notice();
            lower(Dialog::Loops, &ctx, 990.0, 603.0).unwrap()
        };
        assert!(m.frame.1 > plain.frame.1, "the frame grows to hold the notice");
        set_notice("x");
        apply(&Effect::Close);
        assert_eq!(notice(Dialog::Loops), None, "closing clears the notice");
        assert!(notice_for_refusal("goal.pause[not-advertised]").is_some());
        assert!(notice_for_refusal("loop.pause[3]").is_none());
    }

    /// A failed autonomy op's family error shows as the dialog's alert line
    /// (`AutonomyPanel.tsx:328` `role="alert"`), only while it is recorded
    /// (the store keeps it only while the op stays authorized).
    #[test]
    fn a_family_error_shows_as_the_dialog_alert() {
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        clear_notice();
        let a = &store.domains.autonomy;
        a.bind_identity("client-1:dsflash:main");
        assert!(a.record_error("loops", a.epoch(), "loop/pause: rpc error -32602 (no such loop)"));
        let m = lower(Dialog::Loops, &ctx, 990.0, 603.0).unwrap();
        assert!(m.dsl.contains("no such loop"), "the loops error renders");
        let goal = lower(Dialog::Goal, &ctx, 990.0, 603.0).unwrap();
        assert!(!goal.dsl.contains("no such loop"), "only its own family");
        a.clear_error("loops");
        let m = lower(Dialog::Loops, &ctx, 990.0, 603.0).unwrap();
        assert!(!m.dsl.contains("no such loop"));
    }

    /// Fail closed: a context control the server does not advertise is not
    /// drawn (`ContextPanel.tsx` compactAvailable / modeAvailable), and the
    /// models route operations follow their own methods.
    #[test]
    fn unadvertised_controls_are_not_drawn() {
        let _s = serial();
        let (store, ui) = full();
        store.domains.config.set_supported_methods(vec!["profile/llm/list".into()]);
        store.set_capabilities(vec![]);
        let ctx = Ctx::new(&store, &ui);
        let m = lower(Dialog::Context, &ctx, 990.0, 603.0).unwrap();
        let ev = events(&m);
        assert!(!ev.iter().any(|e| e.starts_with("context.")), "{ev:?}");
        assert!(!m.dsl.contains("\"Compact now\"") && !m.dsl.contains("\"Heuristic\""));
        let m = lower(Dialog::Models, &ctx, 990.0, 603.0).unwrap();
        assert!(!events(&m).iter().any(|e| e.starts_with("models.")), "read-only models");
    }

    /// A task row: the status pill takes the blank duration slot's place at
    /// the row end, the command fits the room left (the seed's command whole),
    /// and a command too long for its box ends in "…".
    #[test]
    fn a_task_command_is_never_clipped_by_its_pill() {
        assert_eq!(ellipsize("cargo test", 100.0, 13.0), "cargo test");
        let long = ellipsize(&"x".repeat(60), 213.0, 13.0);
        assert!(long.ends_with('…') && long.chars().count() as f64 * 6.5 <= 213.0, "{long}");
        let _g = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        let (tree, _) = live_tree(Dialog::Tasks, &ctx).expect("tasks tree");
        let r = |id: &str| rect_of(&tree, id).unwrap_or_else(|| panic!("{id}"));
        let (cx, _, cw, _) = r("run_r0_cmd");
        let (px, _, pw, _) = r("run_r0_pill");
        let (dx, _, dw, _) = r("run_r0_dur");
        assert!(cx + cw <= px - 7.9, "command {cx}+{cw} runs under the pill at {px}");
        assert!(((px + pw) - (dx + dw)).abs() < 0.6, "pill ends at the row end");
        let cmd = find(&tree, "run_r0_cmd").and_then(|n| n.attrs.text.clone()).unwrap_or_default();
        assert_eq!(cmd, "cargo test -p octos-cli steer_queue");
    }

    /// A per-second clock never remounts the dialog: elapsed shows at minute
    /// granularity, and two lowerings a few seconds apart are identical.
    #[test]
    fn the_fleet_dialog_is_stable_across_a_ticking_clock() {
        assert_eq!(minute_granularity("42s · —"), "<1m · —");
        assert_eq!(minute_granularity("1m30s · —"), "1m · —");
        assert_eq!(minute_granularity("2h05m · —"), "2h05m · —");
        assert_eq!(minute_granularity("7s"), "<1m");
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        let a = lower(Dialog::Fleet, &ctx, 990.0, 603.0).unwrap().dsl;
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let b = lower(Dialog::Fleet, &ctx, 990.0, 603.0).unwrap().dsl;
        assert_eq!(a, b, "the fleet DSL must not change with the wall clock");
    }

    /// The card SVGs resolve to their on-disk assets (the `:8170` design-lab
    /// URLs are not ours to serve).
    #[test]
    fn card_svgs_resolve_to_files_on_disk() {
        let out = localize_card_assets(
            "draw_svg.svg: http_resource(\"http://127.0.0.1:8170/ux-images/setup-07/assets/nope.svg\")",
        );
        // A file that does not exist keeps the original call (logged, never
        // silently pointed somewhere else).
        assert!(out.contains("http_resource"), "{out}");
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        let models = lower(Dialog::Models, &ctx, 990.0, 603.0).unwrap().dsl;
        if crate::design::dir("stage-b/setup/cards/setup-07/assets").is_dir() {
            assert!(!models.contains("http_resource(\"http://127.0.0.1:8170"), "an unresolved card svg");
            assert!(models.contains("file_resource("), "the chevrons/check are files");
        }
    }

    /// Dev probe: the confirm card's tree with every attribute.
    #[test]
    #[ignore]
    fn dump_confirm() {
        let _s = serial();
        let (store, ui) = full();
        let ctx = Ctx::new(&store, &ui);
        let c = confirmation_for("context.compact_now", &store).unwrap();
        let st = autonomy_view(&ctx);
        let raw = card_tree(Dialog::Context, &ctx, &st).unwrap();
        println!("RAW t_title {:?}", find(&raw, "t_title").map(|n| n.attrs.clone()));
        println!("RAW t_usage {:?}", find(&raw, "t_usage").map(|n| n.attrs.clone()));
        let mut t = confirm_tree(Dialog::Context, &ctx, &st, &c).unwrap();
        let _ = wire(&mut t, &confirm_controls());
        let _ = normalize(&mut t);
        for l in describe(&t) {
            println!("{l}");
        }
    }

    /// Dev probe: `cargo test -p octoscode-module --lib dialog::tests::dump -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dump() {
        let _s = serial();
        let (store, ui) = full();
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
