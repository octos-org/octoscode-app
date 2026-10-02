//! A6 — the host glue for the conversation surfaces: an `impl OctoscodeView`
//! block (a descendant module may reach the root widget's private fields), so
//! `lib.rs` keeps only its small call sites — `sync_surfaces` from
//! `sync_labels`, `surfaces_actions` from `Event::Actions`, the two key arms,
//! `surfaces_outcome` from `perform_action`, and `surfaces_after_draw` from
//! `draw_walk`.
use makepad_widgets::*;

use super::{Job, KeyOutcome, Outcome};
use crate::screens::surfaces as sf;
use crate::screens::taps;

/// Debug aid: `OCTOSCODE_SURFACES_DUMP=<dir>` writes each REMOUNTED surface
/// DSL to `<dir>/<slot>-<n>.dsl`, so a script error the VM reports by DSL
/// line/column can be read against the exact text.
fn dump(slot: &str, dsl: &str) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    if let Ok(dir) = std::env::var("OCTOSCODE_SURFACES_DUMP") {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(format!("{dir}/{slot}-{n}.dsl"), dsl);
    }
}

/// The DSL as mounted: the surfaces route every tap through the widgets'
/// `clicked()` actions (`cv_taps`, harvested by `taps::wired_taps` from the
/// lowered text first), so the buttons' `on_click: || { NAV(…) }` script hook
/// is dropped — it only reached the connect screen's router (a no-op for
/// `cv.*`) and, deferred past a remount its own click caused, the VM ran it on
/// a freed tree (`[E] … pop_stack_value on empty stack`).
pub(crate) fn quiet(dsl: &str) -> String {
    let mut out = String::with_capacity(dsl.len());
    for line in dsl.lines() {
        if !line.trim_start().starts_with("on_click: || { NAV(") {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

impl crate::OctoscodeView {
    /// Mount the surfaces for this sync: the header tabs, the Trajectory
    /// pane (or the transcript + composer), the takeover (or the composer),
    /// the plan card, and the task detail; publish their taps.
    pub(crate) fn sync_surfaces(&mut self, cx: &mut Cx) {
        let (store, ui) = {
            let b = self.bridge.lock().unwrap();
            (b.store.clone(), b.ui.clone())
        };
        let rect = self.view.area().rect(cx);
        sf::set_frame(rect.size.x, rect.size.y);
        let m = crate::conv_layout::current();
        let phone = m.density == crate::conv_layout::Density::Phone;
        let mut taps_out: Vec<(LiveId, LiveId, String)> = Vec::new();
        let mut publish = |splash: LiveId, dsl: &str, out: &mut Vec<(LiveId, LiveId, String)>| {
            for (name, ev) in taps::wired_taps(dsl) {
                out.push((splash, LiveId::from_str(&name), ev));
            }
        };

        // 1. The header's "Chat | Trajectory" tabs.
        let tabs = sf::tabs_available(&store);
        let traj = sf::showing_trajectory(&store);
        self.view.widget(cx, ids!(hd_tabs)).set_visible(cx, tabs);
        for (id, on) in [
            (live_id!(hd_tab_chat_on), !traj),
            (live_id!(hd_tab_chat_off), traj),
            (live_id!(hd_tab_chat_bar), !traj),
            (live_id!(hd_tab_traj_on), traj),
            (live_id!(hd_tab_traj_off), !traj),
            (live_id!(hd_tab_traj_bar), traj),
        ] {
            self.view.widget(cx, &[id]).set_visible(cx, on);
        }

        // 2. The Trajectory replaces the transcript and the composer.
        self.view.widget(cx, ids!(conversation_inner)).set_visible(cx, !traj);
        self.view.widget(cx, ids!(traj_view)).set_visible(cx, traj);
        self.view.widget(cx, ids!(composer_dock)).set_visible(cx, !traj);
        let traj_dsl = sf::lower_trajectory(&store, m.pane_w, phone).map(|l| l.dsl).unwrap_or_default();
        let splash = self.view.splash(cx, ids!(traj_splash));
        match self.mounts.mount(cx, &splash, &quiet(&traj_dsl)) {
            Err(e) => makepad_widgets::log!("[octoscode] trajectory mount: {e}"),
            Ok(true) => dump("trajectory", &quiet(&traj_dsl)),
            Ok(false) => {}
        }
        publish(live_id!(traj_splash), &traj_dsl, &mut taps_out);
        if sf::trajectory_refresh_needed(&store) {
            self.surfaces_outcome(cx, Outcome::Spawn(Job::Refresh));
        }

        // 3. The takeover replaces the composer form (strip + composer).
        let lowered = sf::lower_takeover(&store, &m);
        let showing = lowered.is_some();
        self.view.widget(cx, ids!(takeover_row)).set_visible(cx, showing);
        self.view.widget(cx, ids!(strip_row)).set_visible(cx, !showing);
        self.view.widget(cx, ids!(composer_row)).set_visible(cx, !showing);
        let lowered = lowered.unwrap_or_default();
        let splash = self.view.splash(cx, ids!(takeover_splash));
        match self.mounts.mount(cx, &splash, &quiet(&lowered.dsl)) {
            Err(e) => makepad_widgets::log!("[octoscode] takeover mount: {e}"),
            Ok(true) if showing => {
                dump("takeover", &quiet(&lowered.dsl));
                makepad_widgets::log!(
                    "[octoscode] takeover mounted {:?}: {} tap(s), {} input(s)",
                    sf::takeover(&store),
                    lowered.taps.len(),
                    lowered.inputs.len()
                )
            }
            Ok(_) => {}
        }
        publish(live_id!(takeover_splash), &lowered.dsl, &mut taps_out);
        self.cv_inputs = lowered.inputs.iter().map(|(n, k)| (LiveId::from_str(n), k.clone())).collect();
        self.surfaces_live(cx, &store);
        // The composer is gone while the card waits: its input must not keep
        // the keyboard (a typed `y` would land in a hidden draft).
        if showing {
            let composer = self.view.text_input(cx, &[live_id!(i0_composer_0)]);
            if composer.key_focus(cx) {
                cx.set_key_focus(Area::Empty);
            }
        }
        // `focus-restore.ts`: the removed card held the keyboard -> the
        // composer gets it back.
        if sf::take_focus_restore(showing) {
            // A phone has no hardware keyboard to keep: a programmatic focus
            // there would pop the on-screen keyboard over the conversation
            // (mobile browsers ignore a focus() without a gesture too).
            if !phone {
                self.view.widget(cx, &[live_id!(i0_composer_0)]).set_key_focus(cx);
                makepad_widgets::log!("[octoscode] takeover removed: focus restored to the composer");
            }
            // A list that was following keeps following the taller viewport.
            if let Some(mut list) = self.view.portal_list(cx, ids!(timeline_list)).borrow_mut() {
                if list.is_at_end() {
                    list.set_tail_range(true);
                }
            }
        }

        // 4. The plan card above the composer.
        let plan = sf::lower_plan(&store, &m).unwrap_or_default();
        self.view.widget(cx, ids!(plan_row)).set_visible(cx, !plan.is_empty());
        let splash = self.view.splash(cx, ids!(plan_splash));
        match self.mounts.mount(cx, &splash, &quiet(&plan)) {
            Err(e) => makepad_widgets::log!("[octoscode] plan mount: {e}"),
            Ok(true) => dump("plan", &quiet(&plan)),
            Ok(false) => {}
        }
        publish(live_id!(plan_splash), &plan, &mut taps_out);

        // 5. The task detail dialog.
        let detail = sf::lower_detail(&store).map(|l| l.dsl).unwrap_or_default();
        self.view.widget(cx, ids!(surfaces_dock)).set_visible(cx, !detail.is_empty());
        let splash = self.view.splash(cx, ids!(surfaces_splash));
        match self.mounts.mount(cx, &splash, &quiet(&detail)) {
            Err(e) => makepad_widgets::log!("[octoscode] task detail mount: {e}"),
            Ok(true) => dump("detail", &quiet(&detail)),
            Ok(false) => {}
        }
        publish(live_id!(surfaces_splash), &detail, &mut taps_out);

        self.cv_taps = taps_out;

        // 6. The fold memory drops blocks that left the transcript, and the
        //    reading position follows the session (`use-conversation-scroll.ts`).
        if sf::folds_changed(&store) {
            sf::folds::prune(&store, &ui);
        }
        self.surfaces_session_view(cx, &store);
    }

    /// The question card's live (no-remount) state: the submit pair and its
    /// reason line follow the draft as the person types.
    pub(crate) fn surfaces_live(&mut self, cx: &mut Cx, store: &octoscode_store::Store) {
        for (id, vis) in sf::live_visibility(store) {
            self.view
                .widget(cx, &[live_id!(takeover_splash), LiveId::from_str(&id)])
                .set_visible(cx, vis);
        }
        if let Some(reason) = sf::live_reason(store) {
            self.view
                .label(cx, &[live_id!(takeover_splash), live_id!(cv_q_reason)])
                .set_text(cx, &reason);
        }
    }

    /// Remember the shown session's reading position and restore the next
    /// one's when the active session changes (`useConversationScroll`).
    fn surfaces_session_view(&mut self, cx: &mut Cx, store: &octoscode_store::Store) {
        let active = store.active_session();
        let shown = sf::state().view.shown.clone();
        if active == shown {
            return;
        }
        let list = self.view.portal_list(cx, ids!(timeline_list));
        if let Some(mut l) = list.borrow_mut() {
            if let Some(old) = shown.as_deref() {
                let p = sf::view::Position {
                    following: l.is_at_end(),
                    first_id: l.first_id(),
                    first_scroll: l.first_scroll(),
                };
                sf::state().view.remember(old, p);
            }
            if let Some(new) = active.as_deref() {
                let p = sf::state().view.recall(new);
                if p.following {
                    l.set_tail_range(true);
                } else {
                    l.set_tail_range(false);
                    l.set_first_id_and_scroll(p.first_id, p.first_scroll);
                }
                makepad_widgets::log!(
                    "[octoscode] view: session {new} restored ({})",
                    if p.following { "following".to_owned() } else { format!("row {} at {}", p.first_id, p.first_scroll) }
                );
            }
        }
        sf::state().view.shown = active;
        self.view.redraw(cx);
    }

    /// Route this actions pass for the surfaces: their wired taps, the
    /// takeover's inputs, and the header tabs.
    pub(crate) fn surfaces_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let cv_taps = self.cv_taps.clone();
        for (splash, id, ev) in &cv_taps {
            if self.view.button(cx, &[*splash, *id]).clicked(actions) {
                makepad_widgets::log!("[octoscode] surfaces tap: {ev}");
                let (base, row) = taps::split_row(ev);
                self.perform_action(cx, base, row.unwrap_or(0));
            }
        }
        let inputs = self.cv_inputs.clone();
        let mut dirty = false;
        for (id, key) in &inputs {
            let input = self.view.text_input(cx, &[live_id!(takeover_splash), *id]);
            if let Some(text) = input.changed(actions) {
                sf::input_changed(key, &text);
                dirty = true;
            }
        }
        if dirty {
            let store = { self.bridge.lock().unwrap().store.clone() };
            self.surfaces_live(cx, &store);
        }
        if self.view.widget(cx, ids!(hd_tabs)).visible() {
            if self.view.button(cx, ids!(hd_tab_chat_hit)).clicked(actions) {
                self.perform_action(cx, "cv.tab.chat", 0);
            }
            if self.view.button(cx, ids!(hd_tab_traj_hit)).clicked(actions) {
                self.perform_action(cx, "cv.tab.trajectory", 0);
            }
        }
    }

    /// Carry out what a surfaces action asked for.
    pub(crate) fn surfaces_outcome(&mut self, cx: &mut Cx, outcome: Outcome) {
        match outcome {
            Outcome::Spawn(job) => {
                let conv = { self.bridge.lock().unwrap().conv.clone() };
                match (self.runtime.as_ref(), conv) {
                    (Some(rt), Some(conv)) => {
                        rt.spawn(async move {
                            match sf::run(job.clone(), &conv).await {
                                Ok(s) => makepad_widgets::log!("[octoscode] surfaces {job:?}: {s}"),
                                Err(e) => makepad_widgets::log!("[octoscode] surfaces {job:?} failed: {e}"),
                            }
                            SignalToUI::set_ui_signal();
                        });
                    }
                    _ => {
                        sf::job_unavailable(&job);
                        makepad_widgets::log!("[octoscode] surfaces {job:?}: no connection");
                    }
                }
            }
            Outcome::Action(id) => self.perform_action(cx, &id, 0),
            Outcome::ReviewDiff(preview_id) => {
                // The `D` path (`ApprovalPanel.tsx:45`, `:93-99`): the diff
                // review (`DiffReviewDialog`) reads THIS preview through ONE
                // `diff/preview/get` (A10: the board-3 dialog).
                crate::screens::review::set_preview_id(preview_id);
                self.open_diff_review(cx);
                makepad_widgets::log!("[octoscode] approval: review diff opened");
            }
            Outcome::Done | Outcome::Unrouted => {}
        }
        self.sync_labels(cx);
        self.view.redraw(cx);
    }

    /// One key for the surfaces. Returns true when the key was consumed (the
    /// shell's resolver then never sees it).
    pub(crate) fn surfaces_key(&mut self, cx: &mut Cx, e: &KeyEvent) -> bool {
        // A dialog above the takeover owns the keyboard.
        if crate::screens::board3::host::is_open()
            || crate::screens::board1::is_open()
            || crate::screens::dialog::current().is_some()
            || sf::detail_open()
        {
            return false;
        }
        let store = { self.bridge.lock().unwrap().store.clone() };
        let name = match e.key_code {
            KeyCode::KeyY => "y",
            KeyCode::KeyS => "s",
            KeyCode::KeyN => "n",
            KeyCode::KeyD => "d",
            KeyCode::ReturnKey | KeyCode::NumpadEnter => "Enter",
            KeyCode::ArrowUp => "ArrowUp",
            KeyCode::ArrowDown => "ArrowDown",
            KeyCode::ArrowLeft => "ArrowLeft",
            KeyCode::ArrowRight => "ArrowRight",
            KeyCode::Space => "Space",
            _ => return false,
        };
        let text_focus = self
            .cv_inputs
            .clone()
            .iter()
            .any(|(id, _)| self.view.text_input(cx, &[live_id!(takeover_splash), *id]).key_focus(cx));
        let m = e.modifiers;
        match sf::key(&store, name, m.shift, m.control, m.alt, m.logo, text_focus) {
            KeyOutcome::Pass => false,
            KeyOutcome::Swallow => {
                makepad_widgets::log!("[octoscode] surfaces key {name}: held by the takeover");
                self.sync_labels(cx);
                self.view.redraw(cx);
                true
            }
            KeyOutcome::Action(action, index) => {
                makepad_widgets::log!("[octoscode] surfaces key {name} -> {action}#{index}");
                sf::state().view.focus_inside = true;
                self.perform_action(cx, &action, index);
                true
            }
        }
    }

    /// After the timeline drew: reveal a row the person just opened
    /// ([`sf::view::reveal_delta`]), so its grown body is in view.
    pub(crate) fn surfaces_after_draw(&mut self, cx: &mut Cx) {
        // The list's viewport changed height under it (the plan card or a
        // takeover appeared in the composer dock): a list that was following
        // the tail keeps the newest row in view (`use-conversation-scroll.ts`:
        // following stays pinned to the bottom).
        let mut repinned = false;
        {
            let list_ref = self.view.portal_list(cx, ids!(timeline_list));
            let borrowed = list_ref.borrow_mut();
            if let Some(mut list) = borrowed {
                let h = list.area().rect(cx).size.y;
                let at_end = list.is_at_end();
                let (prev_h, prev_end) = std::mem::replace(&mut sf::state().view.list_h, (h, at_end));
                if prev_h > 0.0 && (h - prev_h).abs() > 0.5 && prev_end && !at_end {
                    list.set_tail_range(true);
                    sf::state().view.list_h = (h, true);
                    repinned = true;
                }
            }
        }
        if repinned {
            self.view.redraw(cx);
        }
        let pending = sf::state().view.reveal.clone();
        let Some((key, tries)) = pending else { return };
        let Some(item_id) = key.strip_prefix("item:").and_then(|n| n.parse::<usize>().ok()) else {
            sf::state().view.reveal = None;
            return;
        };
        let list_ref = self.view.portal_list(cx, ids!(timeline_list));
        let Some(mut list) = list_ref.borrow_mut() else { return };
        let list_rect = list.area().rect(cx);
        let item_rect = list.get_item(item_id).map(|(_, w)| w.area().rect(cx));
        let done = match item_rect {
            Some(r) if r.size.y > 0.0 && list_rect.size.y > 0.0 => {
                let top = r.pos.y - list_rect.pos.y;
                let delta = sf::view::reveal_delta(top, r.size.y, list_rect.size.y, 12.0);
                if delta > 0.5 {
                    let (first, scroll) = (list.first_id(), list.first_scroll());
                    list.set_tail_range(false);
                    list.set_first_id_and_scroll(first, scroll - delta);
                    makepad_widgets::log!(
                        "[octoscode] reveal: row {item_id} grew to {:.0} px, scrolled {delta:.0} px",
                        r.size.y
                    );
                }
                // Re-check once more: the first frame may still lay out the
                // pre-growth height.
                tries <= 1 || delta <= 0.5
            }
            _ => tries <= 1,
        };
        drop(list);
        let mut st = sf::state();
        st.view.reveal = if done { None } else { Some((key, tries - 1)) };
        drop(st);
        if !done {
            self.view.redraw(cx);
        }
    }
}
