//! A12 — the host half of the outage behaviour: mount the connection banner
//! (`screens::reconnect`) into the composer dock, route its taps, and refuse
//! the wire-bound actions honestly while a retained connection is down (or
//! when there is no conversation at all). An `impl OctoscodeView` block in its
//! own file (the a9_host pattern), so `lib.rs` carries only the call sites.
use makepad_widgets::*;

use crate::screens::reconnect;
use crate::{bindings, conv_layout, OctoscodeView};

/// The actions that need the wire even though they look local: a send (and
/// a typed command), a steer, Stop, a session switch, a new chat, a session
/// list refresh, the side question. (Everything a screen table spawns with
/// the conversation is refused by the link itself, at the relay.)
pub fn needs_wire(action: &str) -> bool {
    matches!(
        action,
        bindings::ACTION_SUBMIT
            | bindings::ACTION_INTERRUPT
            | bindings::ACTION_NEW_CHAT
            | "turn.steer"
            | "thread.open"
            | "session.refresh"
            | "aside.ask"
            | "workspace.new_chat_here"
    )
}

/// The banner's line for a refused action: a typed slash command by its
/// name (`/review was not run — …`), a prompt as `Not sent — …`, the rest by
/// what did not happen. The composer text is never consumed.
pub fn held_line(action: &str, draft: &str) -> String {
    match action {
        bindings::ACTION_SUBMIT => {
            let t = draft.trim_start();
            let cmd = t
                .starts_with('/')
                .then(|| t.split_whitespace().next().unwrap_or(t).to_owned())
                .filter(|c| c.len() > 1);
            reconnect::held_line(cmd.as_deref())
        }
        "aside.ask" => reconnect::held_line(Some("/btw")),
        "turn.steer" => "Not steered — your text stays in the composer.".to_owned(),
        bindings::ACTION_INTERRUPT => "Stop was not sent — Octos is unreachable.".to_owned(),
        bindings::ACTION_NEW_CHAT | "workspace.new_chat_here" => {
            "No new chat was started — Octos is unreachable.".to_owned()
        }
        "thread.open" => "Not switched — the session list needs Octos.".to_owned(),
        "session.refresh" => "Not refreshed — Octos is unreachable.".to_owned(),
        other => format!("{other} was not run — Octos is unreachable."),
    }
}

impl OctoscodeView {
    /// Mount the banner (or hide its row) and, during an outage, keep the
    /// composer form: the takeover card waits for the Session to come back
    /// (the web's banner replaces the approval / question panels while
    /// recovering, `App.tsx:2724-2759`).
    pub(crate) fn sync_link(&mut self, cx: &mut Cx) {
        let (store, has_conv) = {
            let b = self.bridge.lock().unwrap();
            (b.store.clone(), b.conv.is_some())
        };
        let view = reconnect::view(&store, has_conv);
        self.view.widget(cx, ids!(link_row)).set_visible(cx, view.is_some());
        if store.outage().is_some() {
            self.view.widget(cx, ids!(takeover_row)).set_visible(cx, false);
            self.view.widget(cx, ids!(strip_row)).set_visible(cx, true);
            self.view.widget(cx, ids!(composer_row)).set_visible(cx, true);
        }
        // The mounted tree is keyed WITHOUT the retry count: the count moves
        // on every re-dial (five in the first second), and a remount per
        // attempt replaced the banner's widgets under a reader (a /snap in
        // the live proof caught it half-built). The count is set in place.
        let dsl = match &view {
            Some(v) => {
                let phone = conv_layout::current().density == conv_layout::Density::Phone;
                let width = self.view.widget(cx, ids!(composer_row)).area().rect(cx).size.x;
                let low = reconnect::lower(&reconnect::View { attempt: 0, ..v.clone() }, width, phone);
                self.dialog_events.extend(low.taps.iter().map(|(_, e)| e.clone()));
                self.link_taps = low.taps.iter().map(|(n, e)| (LiveId::from_str(n), e.clone())).collect();
                crate::screens::theme::retint_dsl(&low.dsl)
            }
            None => {
                self.link_taps.clear();
                String::new()
            }
        };
        let splash = self.view.splash(cx, ids!(link_splash));
        match self.mounts.mount(cx, &splash, &dsl) {
            Err(e) => makepad_widgets::log!("[octoscode] link banner mount: {e}"),
            Ok(true) => {
                if let Some(v) = &view {
                    let (title, detail) = reconnect::copy(v);
                    makepad_widgets::log!(
                        "[octoscode] link banner: {title} | {detail}{}",
                        v.held.as_deref().map(|h| format!(" | {h}")).unwrap_or_default()
                    );
                }
            }
            Ok(false) => {}
        }
        if let Some(v) = &view {
            let (_, detail) = reconnect::copy(v);
            let label = self.view.label(cx, &[live_id!(link_splash), live_id!(a12_link_detail)]);
            if label.text() != detail {
                label.set_text(cx, &detail);
            }
        }
    }

    /// The banner's taps in this `Actions` batch.
    pub(crate) fn link_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let taps = self.link_taps.clone();
        for (id, ev) in &taps {
            if self.view.button(cx, &[live_id!(link_splash), *id]).clicked(actions) {
                makepad_widgets::log!("[octoscode] link tap: {ev}");
                self.perform_action(cx, ev, 0);
            }
        }
    }

    /// Retry now / Disconnect / Dismiss.
    pub(crate) fn perform_link(&mut self, cx: &mut Cx, action: &str) {
        match action {
            reconnect::ACTION_RETRY => {
                let conv = { self.bridge.lock().unwrap().conv.clone() };
                let ok = conv.map(|c| c.retry_now()).unwrap_or(false);
                makepad_widgets::log!("[octoscode] link: retry now -> {}", if ok { "re-dialing" } else { "nothing to retry" });
            }
            reconnect::ACTION_DISCONNECT => {
                // The explicit give-up is A9's Disconnect (its confirmation
                // when unfinished work or unsaved input would be lost).
                makepad_widgets::log!("[octoscode] link: disconnect requested");
                self.perform_action(cx, crate::screens::a9_settings::ACTION_DISCONNECT, 0);
            }
            reconnect::ACTION_DISMISS => {
                reconnect::clear_held();
                makepad_widgets::log!("[octoscode] link: notice dismissed");
            }
            _ => {}
        }
        self.sync_labels(cx);
        self.view.redraw(cx);
    }

    /// Refuse a wire-bound action while a retained connection is down: the
    /// banner names what was not done, the composer keeps its text, nothing
    /// is queued for later. Returns whether the action was refused.
    pub(crate) fn refuse_offline(&mut self, cx: &mut Cx, action: &str) -> bool {
        if !needs_wire(action) {
            return false;
        }
        let (conv, ui) = {
            let b = self.bridge.lock().unwrap();
            (b.conv.clone(), b.ui.clone())
        };
        let Some(conv) = conv else { return false }; // the no-conversation guard says so
        if !conv.in_outage() {
            return false;
        }
        let draft = ui.lock().map(|u| u.draft()).unwrap_or_default();
        if action == bindings::ACTION_SUBMIT && draft.trim().is_empty() {
            return true; // an empty send is no send: nothing to say
        }
        reconnect::note_held(held_line(action, &draft));
        makepad_widgets::log!(
            "[octoscode] offline: {action} refused while reconnecting to {} (draft kept, {} chars)",
            conv.endpoint(),
            draft.chars().count()
        );
        self.sync_labels(cx);
        self.view.redraw(cx);
        true
    }

    /// A wire-bound screen action (research, history, peers, the router's
    /// own sends) during an outage: refused on the banner.
    pub(crate) fn refuse_outage_generic(&mut self, cx: &mut Cx, action: &str) {
        let draft = { self.bridge.lock().unwrap().ui.lock().map(|u| u.draft()).unwrap_or_default() };
        if action == bindings::ACTION_SUBMIT && draft.trim().is_empty() {
            return;
        }
        reconnect::note_held(held_line(action, &draft));
        makepad_widgets::log!("[octoscode] offline: {action} refused while reconnecting (draft kept)");
        self.sync_labels(cx);
        self.view.redraw(cx);
    }

    /// The no-conversation guard: an action that needed a connection when
    /// there is none says so on the banner (never a silent drop).
    pub(crate) fn refuse_no_conversation(&mut self, cx: &mut Cx, action: &str) {
        let draft = { self.bridge.lock().unwrap().ui.lock().map(|u| u.draft()).unwrap_or_default() };
        if action == bindings::ACTION_SUBMIT && draft.trim().is_empty() {
            // An empty send is no send (`submit_draft`'s own rule): nothing
            // to refuse, nothing to say.
            return;
        }
        reconnect::note_held(held_line(action, &draft));
        makepad_widgets::log!("[octoscode] action refused: no conversation ({action}); the banner says so, draft kept");
        self.sync_labels(cx);
        self.view.redraw(cx);
    }

    /// A palette command run during an outage: refused BEFORE the palette
    /// clears the typed command, so the text stays in the composer. (Every
    /// palette command is gated on server methods, `palette::COMMANDS`; with
    /// NO conversation at all — a capture seed — the dialogs still open on
    /// their fixtures and only the wire-bound routes are refused, by the
    /// no-conversation guard.)
    pub(crate) fn refuse_palette_offline(&mut self, cx: &mut Cx, name: &str) -> bool {
        let (conv, ui) = {
            let b = self.bridge.lock().unwrap();
            (b.conv.clone(), b.ui.clone())
        };
        if !conv.as_ref().is_some_and(|c| c.in_outage()) {
            return false;
        }
        if let Ok(mut u) = ui.lock() {
            u.set_palette_open(false);
        }
        crate::screens::palette::set_query("");
        reconnect::note_held(reconnect::held_line(Some(name)));
        makepad_widgets::log!("[octoscode] offline: palette {name} refused while reconnecting; draft kept");
        self.sync_labels(cx);
        self.view.redraw(cx);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_bound_actions_and_their_labels() {
        assert!(needs_wire(bindings::ACTION_SUBMIT));
        assert!(needs_wire("thread.open"));
        assert!(!needs_wire("settings.panel.open"), "UI-local actions stay usable offline");
        assert!(!needs_wire("tool.toggle"));
        assert_eq!(held_line(bindings::ACTION_SUBMIT, "hello there"), "Not sent — your text stays in the composer.");
        assert_eq!(
            held_line(bindings::ACTION_SUBMIT, "/review the diff"),
            "/review was not run — your text stays in the composer."
        );
        assert_eq!(held_line(bindings::ACTION_SUBMIT, "/"), "Not sent — your text stays in the composer.");
        assert_eq!(held_line("thread.open", ""), "Not switched — the session list needs Octos.");
    }
}
