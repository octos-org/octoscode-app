//! A9 — the host half of the A9 surfaces (Activity first): mount into the
//! `a9_dock` Splash, route its taps and inputs, run its jobs on the module's
//! runtime. An `impl OctoscodeView` block in its own file, so `lib.rs` only
//! carries the call sites (the dispatcher arms stay small and local).
use makepad_widgets::*;

use crate::screens::{a9_settings, activity};
use crate::OctoscodeView;

/// Every action id this host owns (one owner per id).
pub fn routes(action: &str) -> bool {
    activity::routes(action) || a9_settings::routes(action)
}

/// Whether Escape belongs to an A9 surface right now.
pub fn escape_owned() -> bool {
    activity::is_open() || a9_settings::pending().is_some()
}

impl OctoscodeView {
    /// Mount the open A9 surface (or hide the dock). Called from
    /// `sync_labels`, so every store change re-lowers it; the mount cache
    /// remounts only when the DSL changed.
    pub(crate) fn sync_a9(&mut self, cx: &mut Cx) {
        let rect = self.view.area().rect(cx);
        activity::set_frame(rect.size.x, rect.size.y);
        let store = { self.bridge.lock().unwrap().store.clone() };
        // The leave confirmation sits over everything A9 shows (it opens from
        // Settings); else the Activity dialog.
        let frame = activity::state().frame;
        let lowered = a9_settings::lower(&frame).or_else(|| activity::lower(&store));
        self.view.widget(cx, ids!(a9_dock)).set_visible(cx, lowered.is_some());
        let Some(lowered) = lowered else {
            self.a9_taps.clear();
            self.a9_inputs.clear();
            return;
        };
        self.a9_taps = lowered
            .taps
            .iter()
            .map(|(n, e)| (LiveId::from_str(n), e.clone()))
            .collect();
        self.a9_inputs = lowered
            .inputs
            .iter()
            .map(|(n, k)| (LiveId::from_str(n), k.clone()))
            .collect();
        // Routed by `clicked()` below; the NAV drain must skip them (the A5
        // dialog-events set is the drain's skip list).
        self.dialog_events.extend(lowered.taps.iter().map(|(_, e)| e.clone()));
        let search_focused = self
            .view
            .text_input(cx, &[live_id!(a9_splash), live_id!(a9_act_search)])
            .key_focus(cx);
        let splash = self.view.splash(cx, ids!(a9_splash));
        match self.mounts.mount(cx, &splash, &lowered.dsl) {
            Err(e) => makepad_widgets::log!("[octoscode] a9 mount: {e}"),
            Ok(true) => {
                makepad_widgets::log!(
                    "[octoscode] a9 activity mounted: {} tap(s), {} row(s)",
                    self.a9_taps.len(),
                    activity::state().shown.len()
                );
                // A refresh remount must not take the search field away from
                // someone typing in it.
                if search_focused {
                    self.view
                        .text_input(cx, &[live_id!(a9_splash), live_id!(a9_act_search)])
                        .set_key_focus(cx);
                }
            }
            Ok(false) => {}
        }
        self.a9_live(cx);
    }

    /// The search's live effects: row/empty visibility and the empty copy,
    /// applied without a remount.
    fn a9_live(&mut self, cx: &mut Cx) {
        for (id, vis) in activity::live_visibility() {
            self.view
                .widget(cx, &[live_id!(a9_splash), LiveId::from_str(&id)])
                .set_visible(cx, vis);
        }
        for (id, text) in activity::live_texts() {
            self.view
                .label(cx, &[live_id!(a9_splash), LiveId::from_str(&id)])
                .set_text(cx, &text);
        }
        self.view.redraw(cx);
    }

    /// The A9 dock's clicks and inputs in this `Actions` batch.
    pub(crate) fn a9_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let taps = self.a9_taps.clone();
        for (id, ev) in &taps {
            if self
                .view
                .button(cx, &[live_id!(a9_splash), *id])
                .clicked(actions)
            {
                makepad_widgets::log!("[octoscode] a9 tap: {ev}");
                let (base, row) = crate::screens::taps::split_row(ev);
                self.perform_action(cx, base, row.unwrap_or(0));
            }
        }
        let inputs = self.a9_inputs.clone();
        let mut dirty = false;
        for (id, key) in &inputs {
            let input = self.view.text_input(cx, &[live_id!(a9_splash), *id]);
            if let Some(text) = input.changed(actions) {
                activity::input_changed(key, &text);
                dirty = true;
            }
        }
        if dirty {
            self.a9_live(cx);
        }
    }

    /// Perform one A9 action id (from a tap, the palette, or a key).
    pub(crate) fn perform_a9(&mut self, cx: &mut Cx, action: &str, index: usize) {
        if a9_settings::routes(action) {
            self.perform_a9_leave(cx, action);
            self.sync_labels(cx);
            self.view.redraw(cx);
            return;
        }
        let (store, conv) = {
            let b = self.bridge.lock().unwrap();
            (b.store.clone(), b.conv.clone())
        };
        let outcome = activity::perform(action, index, &store);
        makepad_widgets::log!("[octoscode] a9 action {action} #{index} -> {outcome:?}");
        match outcome {
            activity::Outcome::Read(gen) => match (self.runtime.as_ref(), conv) {
                (Some(rt), Some(conv)) => {
                    rt.spawn(activity::refresh_loop(conv, gen, activity::REFRESH));
                }
                _ => makepad_widgets::log!("[octoscode] a9 activity: no connection to read"),
            },
            activity::Outcome::OpenSession { index, session } => {
                makepad_widgets::log!("[octoscode] a9 activity: open session {session}");
                self.perform_action(cx, "thread.open", index);
            }
            activity::Outcome::Action(id) => self.perform_action(cx, &id, 0),
            activity::Outcome::Done | activity::Outcome::Unrouted => {}
        }
        self.sync_labels(cx);
        self.view.redraw(cx);
    }

    /// Settings > Connection: Disconnect / Forget server, through the web's
    /// leave confirmation (`LeaveConnectionDialog`).
    fn perform_a9_leave(&mut self, cx: &mut Cx, action: &str) {
        use a9_settings::LeaveKind;
        let now = match action {
            a9_settings::ACTION_DISCONNECT | a9_settings::ACTION_FORGET => {
                let kind = if action == a9_settings::ACTION_FORGET { LeaveKind::Forget } else { LeaveKind::Disconnect };
                let (store, ui) = {
                    let b = self.bridge.lock().unwrap();
                    (b.store.clone(), b.ui.clone())
                };
                let (turn_active, draft) = {
                    let u = ui.lock().unwrap();
                    (u.turn_active(), u.draft())
                };
                let unfinished = a9_settings::unfinished_work(turn_active, &store);
                // The native composer keeps no saved draft: any typed text
                // would be lost (the web's `draftCapacityBlocked`).
                let unsaved = !draft.trim().is_empty();
                let now = a9_settings::request(kind, unfinished, unsaved);
                makepad_widgets::log!(
                    "[octoscode] a9 leave {kind:?}: unfinished={unfinished} unsaved={unsaved} -> {}",
                    if now.is_some() { "now" } else { "confirm" }
                );
                now
            }
            a9_settings::ACTION_CONFIRM => {
                let kind = a9_settings::confirm();
                makepad_widgets::log!("[octoscode] a9 leave confirmed: {kind:?}");
                kind
            }
            a9_settings::ACTION_CANCEL => {
                a9_settings::cancel();
                makepad_widgets::log!("[octoscode] a9 leave cancelled");
                None
            }
            _ => None,
        };
        if let Some(kind) = now {
            self.a9_leave_now(cx, kind);
        }
    }

    /// Close the connection for real: the transport's voluntary
    /// `Disconnect` (it drains and exits — nothing reconnects), the store
    /// Offline, Settings closed; the Connect card returns. Disconnect keeps
    /// the remembered server and its token; Forget removes both.
    fn a9_leave_now(&mut self, cx: &mut Cx, kind: a9_settings::LeaveKind) {
        let (conv, store, screens) = {
            let mut b = self.bridge.lock().unwrap();
            (b.conv.take(), b.store.clone(), b.screens.clone())
        };
        if let Some(conv) = conv {
            match self.runtime.as_ref() {
                Some(rt) => {
                    rt.spawn(async move { a9_settings::disconnect(&conv).await });
                }
                None => {
                    let _ = conv
                        .command_sender()
                        .try_send(octos_app_transport::OutboundCommand::Disconnect);
                }
            }
        }
        store.set_connection("Offline".to_owned(), false);
        if kind == a9_settings::LeaveKind::Forget {
            let mut ui = screens.lock().unwrap_or_else(|e| e.into_inner());
            a9_settings::forget_saved(&ui.server);
            let fresh = crate::screens::connect::ConnectUi::default();
            ui.server = fresh.server;
            ui.token.clear();
            ui.endpoint_error = None;
            ui.failure = None;
            ui.raw_error = None;
            ui.connecting = false;
        }
        makepad_widgets::log!("[octoscode] a9 leave done: {kind:?} (transport closed, Offline)");
        self.perform_action(cx, "settings.panel.close", 0);
        self.connect_key = None;
    }

    /// Escape closes the open A9 surface (`ModalSurface` `onEscape`).
    pub(crate) fn a9_escape(&mut self, cx: &mut Cx) {
        if a9_settings::pending().is_some() {
            a9_settings::cancel();
            makepad_widgets::log!("[octoscode] a9 leave cancelled (Escape)");
        } else if activity::is_open() {
            activity::close();
            makepad_widgets::log!("[octoscode] a9 activity closed (Escape)");
        }
        self.sync_labels(cx);
        self.view.redraw(cx);
    }
}
