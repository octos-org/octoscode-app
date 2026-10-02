//! A9 — the host half of the A9 surfaces (Activity first): mount into the
//! `a9_dock` Splash, route its taps and inputs, run its jobs on the module's
//! runtime. An `impl OctoscodeView` block in its own file, so `lib.rs` only
//! carries the call sites (the dispatcher arms stay small and local).
use makepad_widgets::*;

use crate::screens::{a9_settings, activity};
use crate::OctoscodeView;

/// Every action id this host owns (one owner per id).
pub fn routes(action: &str) -> bool {
    activity::routes(action)
        || a9_settings::routes(action)
        || crate::screens::a9_boundary::routes(action)
}

/// Whether Escape belongs to an A9 surface right now.
pub fn escape_owned() -> bool {
    activity::is_open()
        || a9_settings::pending().is_some()
        || crate::screens::a9_boundary::unavailable().is_some()
}

impl OctoscodeView {
    /// Mount the open A9 surface (or hide the dock). Called from
    /// `sync_labels`, so every store change re-lowers it; the mount cache
    /// remounts only when the DSL changed.
    pub(crate) fn sync_a9(&mut self, cx: &mut Cx) {
        let rect = self.view.area().rect(cx);
        activity::set_frame(rect.size.x, rect.size.y);
        let store = { self.bridge.lock().unwrap().store.clone() };
        // Precedence: the crash screen, a failed surface's panel, the leave
        // confirmation (it opens over Settings), the Activity dialog.
        let frame = activity::state().frame;
        let lowered = if crate::screens::a9_boundary::crashed() {
            crate::screens::a9_boundary::lower_crash(&frame)
        } else if crate::screens::a9_boundary::unavailable().is_some() {
            // An inline failure sits in the conversation column's place,
            // under its header (the header's Review / Settings stay usable).
            let col = self.view.widget(cx, ids!(conversation_column)).area().rect(cx);
            let head = self.view.widget(cx, ids!(oc_header)).area().rect(cx);
            let top = if head.size.y > 0.0 { head.pos.y + head.size.y } else { col.pos.y };
            let content = (col.size.x > 0.0).then(|| {
                (col.pos.x - rect.pos.x, top - rect.pos.y, col.size.x, (col.pos.y + col.size.y - top).max(0.0))
            });
            crate::screens::a9_boundary::lower_unavailable(&frame, content)
        } else {
            a9_settings::lower(&frame).or_else(|| activity::lower(&store))
        };
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
        if crate::screens::a9_boundary::routes(action) {
            self.perform_a9_boundary(cx, action);
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
        } else {
            // Disconnect keeps this server remembered: the Connect card
            // returns on the address just left (durable, like the web's
            // remembered origin) and the session's token stays in the field
            // in memory only (an env-launched token is never written down).
            let mut ui = screens.lock().unwrap_or_else(|e| e.into_inner());
            let endpoint = crate::screens::recents::endpoint();
            if !endpoint.trim().is_empty() {
                ui.endpoint_error = crate::screens::connect::endpoint_error(&endpoint);
                ui.server = endpoint.clone();
                if let Err(e) = crate::credentials::remember_server(&endpoint) {
                    makepad_widgets::log!("[octoscode] a9 leave: {e}");
                }
            }
            if ui.token.is_empty() {
                if let Ok(t) = std::env::var("OCTOS_BEARER") {
                    ui.token = t;
                }
            }
            ui.failure = None;
            ui.raw_error = None;
            ui.connecting = false;
            self.connect_token_pending = !ui.token.is_empty();
        }
        makepad_widgets::log!("[octoscode] a9 leave done: {kind:?} (transport closed, Offline)");
        self.perform_action(cx, "settings.panel.close", 0);
        self.connect_key = None;
    }

    /// Escape closes the open A9 surface (`ModalSurface` `onEscape`).
    pub(crate) fn a9_escape(&mut self, cx: &mut Cx) {
        if crate::screens::a9_boundary::unavailable().is_some() {
            crate::screens::a9_boundary::dismiss_unavailable();
            makepad_widgets::log!("[octoscode] a9 unavailable surface closed (Escape)");
        } else if a9_settings::pending().is_some() {
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

// ------------------------------------------------------------ the boundaries

/// A guarded surface (the web's per-surface `SurfaceBoundary`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    /// A5's dialog host.
    Dialog,
    /// A4's board-3 surfaces (dialogs; the Fleet pane is inline).
    Board3,
    /// A9's surfaces.
    A9,
}

/// The fatal boundary (the web's `FatalErrorBoundary` around the App): the
/// module's event handling and drawing run under `catch_unwind`; a panic
/// becomes the crash screen, drawn alone until Reload app.
impl Widget for OctoscodeView {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if crate::screens::a9_boundary::crashed() {
            // Only the crash screen: the failed view is never drawn again.
            let dock = self.view.widget(cx, ids!(a9_dock));
            let w = dock.walk(cx);
            let _ = dock.draw_walk(cx, scope, w);
            return DrawStep::done();
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.draw_walk_unguarded(cx, scope, walk)
        })) {
            Ok(step) => step,
            Err(p) => {
                let report = crate::screens::a9_boundary::safe_report(&*p, "the client view (draw)");
                makepad_widgets::log!(
                    "[octoscode] a9 fatal boundary (draw): {}",
                    report.lines().next().unwrap_or("")
                );
                crate::screens::a9_boundary::note_crash(report);
                self.view.redraw(cx);
                DrawStep::done()
            }
        }
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        crate::screens::a9_boundary::install_hook();
        if crate::screens::a9_boundary::crashed() {
            self.a9_crash_event(cx, event, scope);
            return;
        }
        if let Err(p) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.handle_event_unguarded(cx, event, scope)
        })) {
            let report = crate::screens::a9_boundary::safe_report(&*p, "the client view");
            makepad_widgets::log!(
                "[octoscode] a9 fatal boundary: {}",
                report.lines().next().unwrap_or("")
            );
            crate::screens::a9_boundary::note_crash(report);
            self.a9_mount_crash(cx);
        }
    }
}

impl OctoscodeView {
    /// Mount the crash screen into the A9 dock (alone: every other dock and
    /// the base chrome are no longer drawn).
    fn a9_mount_crash(&mut self, cx: &mut Cx) {
        let frame = activity::state().frame;
        let Some(lowered) = crate::screens::a9_boundary::lower_crash(&frame) else { return };
        self.a9_taps = lowered.taps.iter().map(|(n, e)| (LiveId::from_str(n), e.clone())).collect();
        self.dialog_events.extend(lowered.taps.iter().map(|(_, e)| e.clone()));
        self.view.widget(cx, ids!(a9_dock)).set_visible(cx, true);
        let splash = self.view.splash(cx, ids!(a9_splash));
        if let Err(e) = self.mounts.mount(cx, &splash, &lowered.dsl) {
            makepad_widgets::log!("[octoscode] a9 crash mount: {e}");
        }
        self.view.redraw(cx);
    }

    /// While crashed, only the crash screen receives events; its three
    /// actions are the way out.
    fn a9_crash_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.widget(cx, ids!(a9_dock)).handle_event(cx, event, scope);
        if let Event::Actions(actions) = event {
            let taps = self.a9_taps.clone();
            for (id, ev) in &taps {
                if self.view.button(cx, &[live_id!(a9_splash), *id]).clicked(actions) {
                    makepad_widgets::log!("[octoscode] a9 crash tap: {ev}");
                    self.perform_a9_boundary(cx, ev);
                    return;
                }
            }
        }
    }

    /// The boundary actions: Reload app, Copy diagnostics, Report this
    /// crash, Close (an unavailable surface).
    pub(crate) fn perform_a9_boundary(&mut self, cx: &mut Cx, action: &str) {
        use crate::screens::a9_boundary as b;
        match action {
            b::ACTION_COPY => {
                if let Some(c) = b::crash() {
                    cx.copy_to_clipboard(&c.report);
                    b::set_copy(b::CopyState::Copied);
                    makepad_widgets::log!(
                        "[octoscode] a9 crash: diagnostics copied ({} chars)",
                        c.report.chars().count()
                    );
                    self.a9_mount_crash(cx);
                }
            }
            b::ACTION_REPORT => {
                // A hidden (test) window never opens a browser: the link is
                // logged instead (the harness sets MAKEPAD_HIDE_WINDOWS).
                if std::env::var_os("MAKEPAD_HIDE_WINDOWS").is_some() {
                    makepad_widgets::log!(
                        "[octoscode] a9 crash: report link {} (hidden window: not opened)",
                        b::REPORT_URL
                    );
                } else {
                    cx.open_url(b::REPORT_URL, OpenUrlInPlace::No);
                    makepad_widgets::log!("[octoscode] a9 crash: report link opened");
                }
            }
            b::ACTION_RELOAD => self.a9_reload(cx),
            b::ACTION_CLOSE => {
                b::dismiss_unavailable();
                makepad_widgets::log!("[octoscode] a9 unavailable surface closed");
                self.sync_labels(cx);
                self.view.redraw(cx);
            }
            _ => {}
        }
    }

    /// Reload app (the web's `window.location.reload()`): every surface
    /// closes, the connection is dropped and the module starts again from
    /// its launch configuration, as a fresh page load does.
    fn a9_reload(&mut self, cx: &mut Cx) {
        crate::screens::a9_boundary::clear();
        crate::screens::dialog::close();
        crate::screens::board3::host::close();
        activity::close();
        a9_settings::cancel();
        {
            let b = self.bridge.lock().unwrap();
            let mut u = b.ui.lock().unwrap();
            if u.settings_open() {
                u.toggle_settings();
            }
            u.set_palette_open(false);
        }
        let conv = self.bridge.lock().unwrap().conv.take();
        if let Some(conv) = conv {
            let _ = conv
                .command_sender()
                .try_send(octos_app_transport::OutboundCommand::Disconnect);
        }
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_background();
        }
        self.a9_taps.clear();
        self.view.widget(cx, ids!(a9_dock)).set_visible(cx, false);
        self.connect_key = None;
        // The next event runs `start` again (the launch path).
        self.started = false;
        makepad_widgets::log!("[octoscode] a9 reload: surfaces closed, connection dropped, starting again");
        self.view.redraw(cx);
    }

    /// Run one surface's sync under its boundary: a panic replaces that
    /// surface with "<Name> unavailable" (modal for a dialog, inline for a
    /// pane) and the rest of the app keeps running.
    pub(crate) fn a9_guarded(&mut self, cx: &mut Cx, which: Guard) {
        use crate::screens::a9_boundary as b;
        use crate::screens::board3::host::Dialog as B3;
        // The surface that is open now (its name, kind, probe point).
        let open: Option<(String, b::Kind, String)> = match which {
            Guard::Dialog => crate::screens::dialog::current()
                .map(|d| (d.title().to_owned(), b::Kind::Modal, format!("surface:{}", d.id()))),
            Guard::Board3 => crate::screens::board3::host::open_dialog().map(|d| {
                let id = format!("{d:?}").to_lowercase();
                let kind = if d == B3::Fleet { b::Kind::Inline } else { b::Kind::Modal };
                let name = match d {
                    B3::Inventory => "Runtime inventory".to_owned(),
                    B3::Switcher => "Sessions".to_owned(),
                    other => format!("{other:?}"),
                };
                (name, kind, format!("surface:{id}"))
            }),
            Guard::A9 => activity::is_open()
                .then(|| ("Activity".to_owned(), b::Kind::Modal, "surface:activity".to_owned())),
        };
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some((_, _, point)) = &open {
                b::probe(point);
            }
            match which {
                Guard::Dialog => self.sync_dialog(cx),
                Guard::Board3 => self.sync_board3(cx),
                Guard::A9 => self.sync_a9(cx),
            }
        }));
        if let Err(p) = r {
            let (name, kind) = open
                .as_ref()
                .map(|(n, k, _)| (n.clone(), *k))
                .unwrap_or_else(|| ("This view".to_owned(), b::Kind::Modal));
            let report = b::safe_report(&*p, &name);
            makepad_widgets::log!("[octoscode] a9 surface boundary: {name} unavailable ({kind:?})");
            // The failed surface closes, so it never fails again; its dock
            // hides; the unavailable panel (A9's dock) takes its place.
            match which {
                Guard::Dialog => {
                    crate::screens::dialog::close();
                    self.view.widget(cx, ids!(dialog_dock)).set_visible(cx, false);
                }
                Guard::Board3 => {
                    crate::screens::board3::host::close();
                    self.view.widget(cx, ids!(board3_dock)).set_visible(cx, false);
                }
                Guard::A9 => {
                    activity::close();
                    a9_settings::cancel();
                }
            }
            b::note_unavailable(&name, kind, report);
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.sync_a9(cx)));
        }
    }
}
