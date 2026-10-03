//! A30 — the host half of the sidebar peer dock (`screens::peer_dock`,
//! parity row 270): mount it in `threads_column` between the session tree
//! and the footer, route its taps, ⌥P / ⌥Y / ⌥N, set its live texts in place
//! and tick its elapsed clock. An `impl OctoscodeView` block in its own file
//! (the a9_host pattern), so `lib.rs` only carries the call sites.
use makepad_widgets::*;

use crate::screens::peer_dock::{self as dock, Outcome, Seat};
use crate::screens::{keys, peers};
use crate::OctoscodeView;

impl OctoscodeView {
    /// Lower and mount the dock (or hide it: no peers, or the collapsed
    /// rail), publish its taps, set its live texts, keep its clock running.
    /// Called from `sync_labels` after the chrome settled the sidebar's seat.
    pub(crate) fn sync_peer_dock(&mut self, cx: &mut Cx) {
        let store = { self.bridge.lock().unwrap().store.clone() };
        let compact = self.chrome.compact;
        let rail = self.view.widget(cx, ids!(oc_sidebar_rail)).visible();
        // The Splash has no visibility of its own: its View row carries it.
        let slot = self.view.widget(cx, ids!(peer_dock_row));
        // The column's content width (its 10 + 10 padding), else the seat's.
        let col = self.view.widget(cx, ids!(threads_column)).area().rect(cx);
        let width = if col.size.x > 40.0 { col.size.x - 20.0 } else if compact { 292.0 } else { 260.0 };
        // The height the tree and the dock share — from the tree's top to the
        // footer's top: two rects the dock never moves, so the cap can never
        // feed back on the dock's own height (0 = not laid out yet: no cap).
        let tree = self.view.widget(cx, ids!(thread_list)).area().rect(cx);
        let foot = self.view.widget(cx, ids!(oc_sidebar_foot)).area().rect(cx);
        let room = if tree.size.y > 0.0 && foot.pos.y > tree.pos.y { foot.pos.y - tree.pos.y } else { 0.0 };
        let lowered = if rail { None } else { dock::lower(&store, peers::now_ms(), Seat { compact, width, room }) };
        if slot.visible() != lowered.is_some() {
            slot.set_visible(cx, lowered.is_some());
        }
        let Some(lowered) = lowered else {
            self.peer_dock_taps.clear();
            return;
        };
        self.peer_dock_taps = lowered.taps.iter().map(|(n, e)| (LiveId::from_str(n), e.clone())).collect();
        // Routed by `clicked()` (`peer_dock_actions`); the NAV drain skips them.
        self.dialog_events.extend(lowered.taps.iter().map(|(_, e)| e.clone()));
        let dsl = crate::screens::theme::retint_dsl(&crate::screens::board3::ui::themed_icons(&lowered.dsl));
        let splash = self.view.splash(cx, ids!(peer_dock_splash));
        match self.mounts.mount(cx, &splash, &dsl) {
            Err(e) => makepad_widgets::log!("[octoscode] peer dock mount: {e}"),
            Ok(true) => {
                makepad_widgets::log!(
                    "[octoscode] peer dock: {} ({} tap(s), {}{})",
                    if lowered.folded { "folded" } else { "expanded" },
                    self.peer_dock_taps.len(),
                    if compact { "phone" } else { "desktop" },
                    if lowered.dsl.contains("pd_rows := ScrollYView") { ", rows scroll" } else { "" }
                );
            }
            Ok(false) => {}
        }
        // The live values (elapsed, tokens, the acknowledgment) are set in
        // place, so they never remount the dock.
        for (id, text) in &lowered.texts {
            let label = self.view.label(cx, &[live_id!(peer_dock_splash), LiveId::from_str(id)]);
            if label.text() != *text {
                label.set_text(cx, text);
            }
        }
        // The elapsed clock: once a second while a shown, expanded dock has a
        // row that is still running.
        let now = peers::now_ms();
        let running = !lowered.folded
            && (dock::rows(&store, now).iter().any(|r| !r.status.terminal()) || dock::note_showing(now));
        if running && slot.visible() && !self.peer_dock_ticking {
            self.peer_dock_ticking = true;
            self.peer_dock_timer = cx.start_timeout(1.0);
        }
    }

    /// The dock's taps in this `Actions` batch. A hidden sidebar's buttons
    /// still report MouseUp, so a tap counts only while the dock is shown.
    pub(crate) fn peer_dock_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if !self.view.widget(cx, ids!(sidebar_dock)).visible() || !self.view.widget(cx, ids!(peer_dock_row)).visible() {
            return;
        }
        let taps = self.peer_dock_taps.clone();
        for (id, ev) in &taps {
            if self.view.button(cx, &[live_id!(peer_dock_splash), *id]).clicked(actions) {
                makepad_widgets::log!("[octoscode] peer dock tap: {ev}");
                let (base, row) = crate::screens::taps::split_row(ev);
                self.perform_action(cx, base, row.unwrap_or(0));
            }
        }
    }

    /// Perform one `pd.*` action: fold / focus locally; a row action spawns
    /// its ONE control job on the runtime (or says why nothing was sent).
    pub(crate) fn perform_peer_dock(&mut self, cx: &mut Cx, action: &str, slot: usize) {
        let (store, conv) = {
            let b = self.bridge.lock().unwrap();
            (b.store.clone(), b.conv.clone())
        };
        let outcome = dock::perform(action, slot, &store, self.chrome.compact);
        match outcome {
            Outcome::Done => makepad_widgets::log!("[octoscode] peer dock {action}#{slot}"),
            Outcome::Refused(why) => makepad_widgets::log!("[octoscode] peer dock {action}#{slot} refused: {why} (nothing sent)"),
            Outcome::Spawn(job) => {
                let who = job.drawn.identity.clone().unwrap_or_default();
                let req = job.drawn.request_id.clone().unwrap_or_default();
                makepad_widgets::log!("[octoscode] peer dock {action}#{slot} -> {:?} on {who} (drawn request {req})", job.action);
                match (self.runtime.as_ref(), conv) {
                    (Some(rt), Some(conv)) => {
                        rt.spawn(async move {
                            match dock::run(job, &conv).await {
                                Ok(ack) => makepad_widgets::log!("[octoscode] peer dock control: {ack}"),
                                Err(e) => makepad_widgets::log!("[octoscode] peer dock control refused: {e}"),
                            }
                            SignalToUI::set_ui_signal();
                        });
                    }
                    _ => {
                        // No connection: nothing sent; the card answers the next press.
                        dock::abandon(&job);
                        makepad_widgets::log!("[octoscode] peer dock {action}: no connection (nothing sent)");
                    }
                }
            }
        }
        self.sync_labels(cx);
        self.view.redraw(cx);
    }

    /// ⌥P folds the dock; ⌥Y / ⌥N approve / deny the FOCUSED row's drawn
    /// approval. Never while a text field or a dialog owns the keyboard
    /// (`keys::shortcut_suppressed`). Returns whether the key was consumed.
    pub(crate) fn peer_dock_key(&mut self, cx: &mut Cx, e: &KeyEvent) -> bool {
        let m = e.modifiers;
        let Some(action) = dock::key_action(e.key_code, m.control, m.alt, m.logo) else { return false };
        // No dock on screen (no peers, the rail, a closed phone drawer): the
        // chord is not the dock's.
        if !self.view.widget(cx, ids!(sidebar_dock)).visible() || !self.view.widget(cx, ids!(peer_dock_row)).visible() {
            return false;
        }
        let facts = self.shortcut_facts(cx);
        if keys::shortcut_suppressed(facts) {
            makepad_widgets::log!("[octoscode] peer dock key {action} suppressed ({facts:?})");
            return false;
        }
        match dock::focused_slot() {
            Some(slot) => {
                makepad_widgets::log!("[octoscode] peer dock key -> {action}#{slot}");
                self.perform_peer_dock(cx, action, slot);
            }
            None => makepad_widgets::log!("[octoscode] peer dock key {action}: no focused peer row (nothing sent)"),
        }
        true
    }

    /// ⌥P (`App.tsx:1110-1117`): fold / unfold the dock — only while it is
    /// on screen (no peers, the rail, a closed phone drawer: nothing to fold,
    /// and a hidden flip would surprise the next time peers appear).
    pub(crate) fn toggle_peer_dock(&mut self, cx: &mut Cx) {
        if !self.view.widget(cx, ids!(sidebar_dock)).visible() || !self.view.widget(cx, ids!(peer_dock_row)).visible() {
            makepad_widgets::log!("[octoscode] shortcut Alt+P: no peer dock on screen");
            return;
        }
        let folded = dock::toggle(self.chrome.compact);
        makepad_widgets::log!("[octoscode] shortcut Alt+P -> peer dock {}", if folded { "folded" } else { "expanded" });
        self.sync_labels(cx);
        self.view.redraw(cx);
    }

    /// The elapsed clock's tick.
    pub(crate) fn peer_dock_timer_fired(&mut self, cx: &mut Cx, event: &Event) -> bool {
        if let Event::Timer(te) = event {
            if self.peer_dock_timer.is_timer(te).is_some() {
                self.peer_dock_ticking = false;
                self.sync_peer_dock(cx);
                self.view.redraw(cx);
                return true;
            }
        }
        false
    }
}
