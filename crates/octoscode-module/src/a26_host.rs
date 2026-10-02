//! A26 — the host half of three rows: the LIVE re-theme behind the sidebar
//! footer's theme toggle, Settings > General's Theme row and Settings >
//! Preferences' palette (parity rows `preferences/g-timeline` "five named
//! display palettes" and `shell/g-settings` "sidebar footer entries"), and
//! the error toasts (`error/error` "error toasts"). An `impl OctoscodeView`
//! block in its own file (the a9_host pattern), so `lib.rs` only carries the
//! call sites.
//!
//! # Why a re-theme re-runs the script_mods
//!
//! The shell's colours are `theme.*` roles and spliced inks captured when the
//! module's `script_mod`s evaluate (`screens::theme::eval_roles` runs first in
//! each), so a choice made later used to reach only the surfaces lowered after
//! it: Settings > General > Theme = Dark left the sidebar, header and Settings
//! light (measured on main b1fe23fb: only the status strip flipped). The shell
//! re-themes a running module exactly this way when the WM's palette changes
//! (`octosense crates/shell/src/module_host.rs` `apply_style`): re-run the
//! module's registration inside `vm.with_reload`, then re-apply the fresh
//! template to the live root with `Apply::ScriptReapply`, which keeps every
//! runtime value (texts, visibility, the typed draft) while the captured
//! colours take the new values. Every lowered surface is then re-lowered and
//! re-mounted (its retint reads the new look).
use makepad_widgets::*;

use crate::OctoscodeView;

impl OctoscodeView {
    /// Re-theme the running module in place for the current look
    /// (`screens::theme::current_look`): the shell's roles and inks, every
    /// template, every mounted surface.
    pub(crate) fn retheme(&mut self, cx: &mut Cx) {
        let look = crate::screens::theme::current_look();
        cx.with_vm(|vm| {
            vm.with_reload(|vm| {
                crate::chrome::script_mod(vm);
                crate::code_view::script_mod(vm);
                crate::script_mod(vm);
            });
            let value = script_eval!(vm, {
                use mod.widgets.*
                OctoscodeView {}
            });
            <Self as ScriptApply>::script_apply(self, vm, &Apply::ScriptReapply, &mut Scope::empty(), value);
        });
        // Every lowering embeds the look (retint, icons, code colours):
        // re-lower and re-mount them all; the geometry the shell applied at
        // runtime (column widths, the dock's paddings, the palette size) is
        // re-applied on the next sync.
        self.cache = crate::ScreenCache::default();
        self.mounts = crate::ComponentMounts::default();
        self.connect_key = None;
        self.applied_metrics = None;
        self.palette_w = 0.0;
        self.palette_list_h = 0.0;
        self.chrome.applied.clear();
        makepad_widgets::log!("[octoscode] a26 retheme -> {}", look.name());
        self.sync_labels(cx);
        self.view.redraw(cx);
    }

    /// Whether a modal surface is open — the toasts are HELD then (not
    /// drawn, their clocks stopped), so they never cover a dialog's primary
    /// action; the first-run screen holds them too.
    fn toasts_held(&mut self, cx: &mut Cx) -> bool {
        HOLDING_DOCKS.iter().any(|id| {
            let w = self.view.widget(cx, &[LiveId::from_str(id)]);
            !w.is_empty() && w.visible()
        })
    }

    /// Age, lower, place and mount the error toasts (`screens::toasts`).
    /// Called at the end of `sync_labels`, after every dock's visibility is
    /// settled.
    pub(crate) fn sync_toasts(&mut self, cx: &mut Cx) {
        use crate::screens::toasts;
        let held = self.toasts_held(cx);
        toasts::tick(toasts::now_ms(), held);
        let module = self.view.area().rect(cx);
        let compact = self.chrome.compact;
        let col = self.view.widget(cx, ids!(conversation_column)).area().rect(cx);
        let head = self.view.widget(cx, ids!(oc_header)).area().rect(cx);
        let avail = if compact || col.size.x <= 0.0 { module.size.x } else { col.size.x };
        let width = toasts::stack_width(avail, compact);
        let lowered = if held { None } else { toasts::lower(width) };
        let dock_shown = lowered.is_some();
        self.view.widget(cx, ids!(toast_dock)).set_visible(cx, dock_shown);
        let Some(lowered) = lowered else {
            self.toast_taps.clear();
            return;
        };
        // Under the conversation header; right-aligned in the conversation
        // column on a desktop, the window's width less 12 px gutters on a
        // phone. Overlay margins are module-local.
        let below = if head.size.y > 0.0 { head.pos.y + head.size.y } else { module.pos.y };
        let (left, top) = if compact {
            (12.0, below - module.pos.y + 8.0)
        } else {
            let right = if col.size.x > 0.0 { col.pos.x + col.size.x } else { module.pos.x + module.size.x };
            (right - 16.0 - width - module.pos.x, below - module.pos.y + 12.0)
        };
        let key = format!("{left:.0}|{top:.0}");
        if self.toast_key != key {
            self.toast_key = key;
            let margin = Inset { left: left.max(0.0), top: top.max(0.0), right: 0.0, bottom: 0.0 };
            let mut dock = self.view.widget(cx, ids!(toast_dock));
            script_apply_eval!(cx, dock, { margin: #(margin) });
        }
        self.toast_taps = lowered.taps.iter().map(|(n, e)| (LiveId::from_str(n), e.clone())).collect();
        // Routed by `clicked()` (`toast_actions`); the NAV drain skips them.
        self.dialog_events.extend(lowered.taps.iter().map(|(_, e)| e.clone()));
        let splash = self.view.splash(cx, ids!(toast_splash));
        match self.mounts.mount(cx, &splash, &lowered.dsl) {
            Err(e) => makepad_widgets::log!("[octoscode] toast mount: {e}"),
            Ok(true) => makepad_widgets::log!("[octoscode] toasts shown: {}", self.toast_taps.len()),
            Ok(false) => {}
        }
        // Wake when the next one is due to leave.
        if let Some(ms) = toasts::next_expiry_ms() {
            self.toast_timer = cx.start_timeout(ms as f64 / 1000.0 + 0.05);
        }
    }

    /// The toasts' × taps in this `Actions` batch.
    pub(crate) fn toast_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let taps = self.toast_taps.clone();
        for (id, ev) in &taps {
            if self.view.button(cx, &[live_id!(toast_splash), *id]).clicked(actions) {
                makepad_widgets::log!("[octoscode] toast tap: {ev}");
                let (base, row) = crate::screens::taps::split_row(ev);
                self.perform_action(cx, base, row.unwrap_or(0));
            }
        }
    }

    /// Perform one A26 toast action id (`toast.dismiss#<id>`).
    pub(crate) fn perform_toast(&mut self, cx: &mut Cx, action: &str, index: usize) {
        if action == crate::screens::toasts::ACTION_DISMISS {
            crate::screens::toasts::dismiss(index as u64);
        }
        self.sync_labels(cx);
        self.view.redraw(cx);
    }

    /// The toast clock: a toast is due to leave.
    pub(crate) fn toast_timer_fired(&mut self, cx: &mut Cx, event: &Event) -> bool {
        if let Event::Timer(te) = event {
            if self.toast_timer.is_timer(te).is_some() {
                self.sync_toasts(cx);
                self.view.redraw(cx);
                return true;
            }
        }
        false
    }
}

/// The surfaces that hold the toasts while they are open (each is a
/// full-window dock in `lib.rs`): Settings, the Stop confirm, the workspace
/// menu, the command palette, the dialog hosts (A5, board 1, board 3, A6's
/// task detail, A9), a docked screen, the phone drawer's scrim, and the
/// first-run screen.
pub const HOLDING_DOCKS: &[&str] = &[
    "settings_dock",
    "stop_dock",
    "sb_menu_dock",
    "palette_dock",
    "dialog_dock",
    "board1_dock",
    "board3_dock",
    "surfaces_dock",
    "a9_dock",
    "screen_dock",
    "drawer_scrim",
    "first_run",
];
