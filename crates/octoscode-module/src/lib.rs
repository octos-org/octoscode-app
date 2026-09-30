//! octoscode-module — the `AppModule` (re-homed from the D9 shape-(i) spike).
//!
//! It owns a [`Store`] and a transport on a background tokio runtime, renders
//! the connection state + session count **from the store**, and exposes store
//! data to cards only through [`bindings`] (a card never sees a Rust type).
//! L0 cards mount here later; this card ships no card.
//!
//! ## Card #12: the conversation
//!
//! The module now drives a [`flow::Conversation`] — the gate's end-to-end path
//! (connect → pick profile → open a workspace → `turn/start` → deltas → tool
//! rows → `turn/interrupt` → `turn/completed`) — and renders the
//! **fallback** view ([`fallback`]) **only through binding ids**. The view
//! emits binding **action ids**; [`perform_action`] maps them back to flow
//! calls. That is 8.8 condition 2 end to end: the view names ids, the module
//! owns the meaning.
//!
//! Threading rule (from the spike): the UI thread never blocks. The tokio
//! runtime owns the transport; its event waker (`SignalToUI`) wakes the UI,
//! which drains events into the store and re-reads bindings into widgets.
pub use makepad_widgets;

use makepad_app_module::{
    makepad_ai_services::wire::{ServiceCall, ServiceManifest, ToolResult},
    AppModule, ExecOutcome, InstanceHandles, InstanceParts, OpenSchema, ServiceExecutor,
    ValidatedOpen,
};
use makepad_widgets::*;
use std::sync::{Arc, Mutex};

use octoscode_store::Store;

pub mod actions;
pub mod bindings;
pub mod cards;
pub mod components;
pub mod fallback;
pub mod l0_host;
pub mod flow;
pub mod mount;
pub mod screen;
pub mod screens;

use flow::{Conversation, FlowUi};
// A top-level `::` path is not a `#[rust]` field type the `Script` derive's
// parser accepts (its `eat_type` reads one ident + optional generics), so the
// cache types are aliased to bare idents here.
use mount::MountCache as ComponentMounts;
use screen::Cache as ScreenCache;

script_mod! {
    use mod.prelude.widgets.*
    mod.widgets.OctoscodeView = set_type_default() do #(OctoscodeView::register_widget(vm)) {
        ..mod.widgets.RectView
        width: Fill height: Fill
        draw_bg.color: theme.color_bg_app
        flow: Down padding: 16 spacing: 10
        // Card #21c item 7: the debug header is GONE from the visible UI. The
        // connection state / session count stay as 1px labels so `/g` still
        // carries them and `sync_labels` keeps a target (board: "connection
        // state can go to /g metadata").
        header_meta := View {
            width: 0 height: 0 flow: Right
            status := Label { width: 0 height: 0 draw_text.text_style.font_size: 1 text: "" }
            sessions := Label { width: 0 height: 0 draw_text.text_style.font_size: 1 text: "" }
        }
        // Card #17 (D12): native columns + virtualized L0 items. The host owns
        // structure + scale; each list item's LOOK comes from an L0 component
        // lowered in-process (`components.rs`, the same chain as a card). Left =
        // thread list, center = timeline with the composer docked at its bottom,
        // right = the review slot (empty for now).
        //
        // Card #21c items 2/3/6: the component IS the item — no native `kind`
        // label, no row chrome, and each row's height comes from its lowered
        // component (`Splash height: Fit`), so a bubble hugs its text and prose
        // is not clipped.
        columns := View {
            width: Fill height: Fill
            flow: Right spacing: 10

            threads_column := View {
                width: 220 height: Fill flow: Down spacing: 6
                // Card #21c item 7: `New chat` is #16's own `new-chat` component
                // at the top of the thread column (scene 01). A transparent hit
                // target overlays it so the HOST routes `session.new` — the
                // component's own button lives in the Splash isolate and never
                // reports to the host (the `row_hit` pattern, below).
                new_chat_row := View {
                    width: Fill height: Fit flow: Overlay
                    // Card #21g item 3: the #16 `new-chat` card ran flush to the
                    // thread column's right edge, while the thread rows below it
                    // stop short of that edge (the list reserves its scrollbar).
                    // Give the card the same right inset so the two align.
                    margin: Inset{left: 0 top: 0 right: 13 bottom: 0}
                    // Card #21e item 2: the #16 `new-chat` component's artboard is
                    // 374x76 (scene-01 `mapped.json`); a fixed 44px slot clipped its
                    // bottom edge flat under the label. `Fit` takes the component's
                    // own measured height (the same idiom `thread_splash` uses).
                    new_chat_splash := Splash { width: Fill height: Fit }
                    new_chat_hit := Button {
                        width: Fill height: Fill text: ""
                        draw_bg.color: #00000000
                        draw_bg.color_hover: #00000010
                        draw_bg.color_down: #00000020
                        draw_bg.border_size: 0.0
                        draw_bg.color_2: #00000000
                        draw_bg.border_color: #00000000
                        draw_bg.border_color_2: #00000000
                    }
                }
                // Card #21c item 6: one #16 `thread-row` per session (selected
                // state, ellipsized title) — the component draws its own label,
                // so no native title Label sits under it. A transparent `row_hit`
                // routes the click with the item id (`thread.open`).
                thread_list := PortalList {
                    width: Fill height: Fill flow: Down drag_scrolling: true
                    // Card #21g item 3: the rows must share the New chat card's
                    // right inset so the two align (the atlas insets the card and
                    // the selected row to one right edge).
                    margin: Inset{left: 0 top: 0 right: 13 bottom: 0}
                    ThreadRowTpl := View {
                        width: Fill height: Fit flow: Overlay
                        thread_splash := Splash { width: Fill height: Fit }
                        row_hit := Button {
                            width: Fill height: Fill text: ""
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000012
                            draw_bg.color_down: #00000022
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
                        }
                    }
                }
            }

            conversation_column := View {
                width: Fill height: Fill flow: Down spacing: 6
                // Card #21c item 2: the component IS the item. No native `kind`
                // label and no row chrome — the row is just the lowered
                // component plus a transparent hit target.
                timeline_list := PortalList {
                    width: Fill height: Fill flow: Down drag_scrolling: true
                    // Card #21c L2: tail the newest item so a newly appended
                    // turn scrolls into view. Without it the list stayed pinned
                    // to row 0 and turn 2 (`Worked for` alone changed) never
                    // showed. `auto_tail` only tails while already at the end
                    // (portal_list.rs:770), so scrolling up to read is preserved.
                    auto_tail: true
                    TimelineItemTpl := View {
                        // Card #21c item 3: the row height comes from the lowered
                        // component (`Splash height: Fit` measures its root), so a
                        // bubble hugs its text and prose is not clipped.
                        width: Fill height: Fit flow: Overlay
                        item_splash := Splash { width: Fill height: Fit }
                        row_hit := Button {
                            width: Fill height: Fill text: ""
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000010
                            draw_bg.color_down: #00000020
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
                        }
                    }
                }
                // Card #21c item 5: ONE composer — the #16 `composer` component is
                // the whole input surface (its own input + `+` + mic + send). The
                // old native TextInput + `Steer now / Send / ×` row is GONE. The
                // component's own controls live in the Splash isolate and do not
                // report to the host, so transparent host hit targets are laid
                // over its fixed control rects (fixed chrome, RULES: measured
                // layout is for chrome) and routed to the declared action ids.
                composer_row := View {
                    width: Fill height: Fit flow: Overlay
                    composer_splash := Splash { width: Fill height: 190 }
                    composer_hits := View {
                        width: Fill height: 190 flow: Overlay
                        plus_hit := Button {
                            width: 36 height: 36 text: ""
                            margin: Inset{left: 5.0 top: 123.0}
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000010
                            draw_bg.color_down: #00000020
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
                            draw_bg.color_hover: #00000000
                            draw_bg.color_down: #00000000
                            draw_bg.color_focus: #00000000
                            draw_bg.color_disabled: #00000000
                            draw_bg.color_2_hover: #00000000
                            draw_bg.color_2_down: #00000000
                            draw_bg.color_2_focus: #00000000
                            draw_bg.color_2_disabled: #00000000
                            draw_bg.border_color_hover: #00000000
                            draw_bg.border_color_down: #00000000
                            draw_bg.border_color_focus: #00000000
                            draw_bg.border_color_disabled: #00000000
                            draw_bg.border_color_2_hover: #00000000
                            draw_bg.border_color_2_down: #00000000
                            draw_bg.border_color_2_focus: #00000000
                            draw_bg.border_color_2_disabled: #00000000
                        }
                        mic_hit := Button {
                            width: 36 height: 36 text: ""
                            margin: Inset{left: 280.0 top: 121.0}
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000010
                            draw_bg.color_down: #00000020
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
                            draw_bg.color_hover: #00000000
                            draw_bg.color_down: #00000000
                            draw_bg.color_focus: #00000000
                            draw_bg.color_disabled: #00000000
                            draw_bg.color_2_hover: #00000000
                            draw_bg.color_2_down: #00000000
                            draw_bg.color_2_focus: #00000000
                            draw_bg.color_2_disabled: #00000000
                            draw_bg.border_color_hover: #00000000
                            draw_bg.border_color_down: #00000000
                            draw_bg.border_color_focus: #00000000
                            draw_bg.border_color_disabled: #00000000
                            draw_bg.border_color_2_hover: #00000000
                            draw_bg.border_color_2_down: #00000000
                            draw_bg.border_color_2_focus: #00000000
                            draw_bg.border_color_2_disabled: #00000000
                        }
                        send_hit := Button {
                            width: 44 height: 44 text: ""
                            margin: Inset{left: 324.0 top: 115.0}
                            draw_bg.color: #00000000
                            draw_bg.color_hover: #00000010
                            draw_bg.color_down: #00000020
                            draw_bg.border_size: 0.0
                            draw_bg.color_2: #00000000
                            draw_bg.border_color: #00000000
                            draw_bg.border_color_2: #00000000
                            draw_bg.color_hover: #00000000
                            draw_bg.color_down: #00000000
                            draw_bg.color_focus: #00000000
                            draw_bg.color_disabled: #00000000
                            draw_bg.color_2_hover: #00000000
                            draw_bg.color_2_down: #00000000
                            draw_bg.color_2_focus: #00000000
                            draw_bg.color_2_disabled: #00000000
                            draw_bg.border_color_hover: #00000000
                            draw_bg.border_color_down: #00000000
                            draw_bg.border_color_focus: #00000000
                            draw_bg.border_color_disabled: #00000000
                            draw_bg.border_color_2_hover: #00000000
                            draw_bg.border_color_2_down: #00000000
                            draw_bg.border_color_2_focus: #00000000
                            draw_bg.border_color_2_disabled: #00000000
                        }
                    }
                }
            }

            review_column := View {
                width: 200 height: Fill flow: Down spacing: 6
                Label { width: Fill height: Fit text: "Review" draw_text.text_style.font_size: 14 }
            }
        }
    }
}

/// What the UI thread needs to drive the conversation, behind one lock.
#[derive(Default)]
pub(crate) struct Bridge {
    /// The conversation (transport + store + flow UI). `None` until `start`.
    conv: Option<Arc<Conversation>>,
    /// The store, so a binding read still works before the flow exists.
    pub(crate) store: Arc<Store>,
    /// The flow UI, for the same reason.
    pub(crate) ui: Arc<Mutex<FlowUi>>,
}

/// Seed a store with `n` synthetic timeline rows and one session, for the
/// virtualization proof (`OCTOSCODE_SYNTHETIC_TIMELINE`). No transport: the
/// window draws the virtualized list on its own.
fn seed_synthetic(store: &Arc<Store>, n: usize) {
    use octoscode_store::Session;
    store.set_connection("Live".into(), false);
    store.set_sessions(vec![Session {
        id: "synthetic:main".into(),
        title: Some("Synthetic 2000".into()),
        message_count: n,
        updated_at: None,
        last_prompt: None,
        active_turn: false,
    }]);
    store.set_active(Some("synthetic:main".into()));
    // One TURN per row, each with its own user message: the rows are then
    // DISTINCT (`synthetic row #i`), so a scroll is visible in the `/snap`
    // bodies — which is what makes the virtualization proof checkable.
    let tl = &store.domains.session.timeline;
    for i in 0..n {
        tl.upsert_user_message(
            "synthetic:main",
            &format!("t{i}"),
            &format!("synthetic row #{i}"),
            serde_json::json!({}),
        );
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct OctoscodeView {
    #[deref]
    view: View,
    #[rust]
    bridge: Arc<Mutex<Bridge>>,
    #[rust]
    runtime: Option<tokio::runtime::Runtime>,
    #[rust]
    started: bool,
    /// The memoised per-item lowerings (a `PortalList` re-instantiates its
    /// visible rows every frame; without this the CPU re-lowers them each
    /// frame). See [`screen::Cache`].
    #[rust]
    cache: ScreenCache,
    /// Card #21b: the per-`Splash` mounted components. Each visible row's
    /// lowered DSL is evaluated in the app's VM and made the Splash's own
    /// `view` (`mount`), memoised by the widget uid so a `PortalList` that
    /// re-instantiates its rows every frame does not re-evaluate them.
    #[rust]
    mounts: ComponentMounts,
    /// The two virtualized lists' widget uids (0 = not captured yet), so
    /// `draw_walk` can tell which `PortalList` a draw step belongs to.
    #[rust]
    thread_uid: u64,
    #[rust]
    timeline_uid: u64,
}

impl OctoscodeView {
    fn start(&mut self) {
        // The 2,000-entry synthetic timeline (the virtualization proof): no
        // transport at all — a store with 2,000 rows and a session, so the
        // window draws the virtualized list on its own (`/g` then shows the
        // timeline drawing ~20 items with a scroll range over 2,000).
        if let Ok(n) = std::env::var("OCTOSCODE_SYNTHETIC_TIMELINE") {
            let n: usize = n.parse().unwrap_or(2000);
            {
                let b = self.bridge.lock().unwrap();
                seed_synthetic(&b.store, n);
            }
            ::log::info!("[octoscode] synthetic timeline: {n} entries (no transport)");
            return;
        }
        let base =
            std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50082".to_string());
        let bearer = std::env::var("OCTOS_BEARER").unwrap_or_default();
        let profile =
            std::env::var("OCTOS_PROFILE_ID").unwrap_or_else(|_| "octoscode".to_string());
        // The workspace cwd the web passes to `session/open` (`session-defaults.ts:5-7`).
        let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();

        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                ::log::error!("octoscode: runtime: {e}");
                return;
            }
        };

        let connected = {
            let _guard = runtime.enter();
            Conversation::connect(
                &base,
                &bearer,
                &profile,
                cwd.clone(),
                Some(Arc::new(|| SignalToUI::set_ui_signal())),
            )
        };

        let (conv, mut evt_rx) = match connected {
            Ok(pair) => pair,
            Err(e) => {
                ::log::error!("octoscode: connect: {e}");
                return;
            }
        };
        let conv = Arc::new(conv);

        {
            let mut b = self.bridge.lock().unwrap();
            b.store = conv.store.clone();
            b.ui = conv.ui();
            b.conv = Some(conv.clone());
        }
        // Entry #29c: the stage-C screens fold their three profile reads once
        // at startup, behind the temporary-mount flag (until #28e's shell).
        if std::env::var_os("OCTOSCODE_STAGE_C_SCREENS").is_some() {
            let drv = conv.clone();
            runtime.spawn(async move {
                match screens::models::refresh(&drv, &drv.store).await {
                    Ok(n) => ::log::info!("octoscode: screens: {n} profile reads folded"),
                    Err(e) => ::log::warn!("octoscode: screens refresh: {e}"),
                }
                SignalToUI::set_ui_signal();
            });
        }

        // Drive the conversation: open the workspace, then drain events.
        let drv = conv.clone();
        runtime.spawn(async move {
            if let Err(e) = drv.open_workspace(cwd).await {
                ::log::error!("octoscode: session/open: {e}");
                SignalToUI::set_ui_signal();
                return;
            }
            // Optionally onboard a profile (the live gate's `profile/local/create`).
            if std::env::var("OCTOS_CREATE_PROFILE").is_ok() {
                if let Err(e) = drv.create_profile().await {
                    ::log::warn!("octoscode: profile/local/create: {e}");
                }
            }
            while let Some(evt) = evt_rx.recv().await {
                let e = drv.on_event(evt);
                ::log::debug!("[octoscode] {e:?}");
                SignalToUI::set_ui_signal();
            }
        });

        self.runtime = Some(runtime);
    }

    /// Run one binding action with the item index that emitted it (card #21 §3).
    /// The mapping is the pure [`actions::resolve`]; this only performs the
    /// resulting effect (off the UI thread).
    fn perform_action(&self, action: &str, index: usize) {
        let (store, ui, conv) = {
            let b = self.bridge.lock().unwrap();
            (b.store.clone(), b.ui.clone(), b.conv.clone())
        };
        let effect = {
            let ctx = bindings::Ctx::new(&store, &ui);
            actions::resolve(action, index, &ctx)
        };
        // UI-local effects need no runtime/transport.
        match &effect {
            actions::Effect::ToggleTool(key) => {
                let _ = ui.lock().map(|mut u| u.toggle_expanded(key));
                return;
            }
            actions::Effect::Unhandled(id) => {
                ::log::warn!("octoscode: unhandled action id {id:?}");
                return;
            }
            _ => {}
        }
        let Some(rt) = self.runtime.as_ref() else {
            return;
        };
        let Some(conv) = conv else {
            return;
        };
        // Entry #29c: the stage-C screens own their action ids (the cards'
        // service-actions events); route them through the production client.
        if screens::models::owns(action) {
            let action = action.to_string();
            let store = store.clone();
            rt.spawn(async move {
                if let Err(e) = screens::models::perform(&conv, &action, &store).await {
                    ::log::warn!("octoscode: screens: {action:?}: {e}");
                }
            });
            return;
        }
        match effect {
            actions::Effect::Refresh => {
                rt.spawn(async move {
                    if let Err(e) = conv.refresh_sessions().await {
                        ::log::warn!("octoscode: session.refresh: {e}");
                    }
                });
            }
            // Card #14 defect 4: "New chat" mints a FRESH session id, so a new
            // chat never reuses the previous run's context.
            actions::Effect::NewChat => {
                let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();
                rt.spawn(async move {
                    match conv.new_chat(cwd).await {
                        Ok(id) => ::log::info!("octoscode: new chat opened {id}"),
                        Err(e) => ::log::warn!("octoscode: session.new: {e}"),
                    }
                });
            }
            actions::Effect::Submit => {
                rt.spawn(async move {
                    if let Err(e) = conv.submit_draft().await {
                        ::log::warn!("octoscode: composer.submit: {e}");
                    }
                });
            }
            actions::Effect::Steer(text) => {
                rt.spawn(async move {
                    if let Err(e) = conv.steer(&text).await {
                        ::log::warn!("octoscode: turn.steer: {e}");
                    }
                });
            }
            actions::Effect::Interrupt(turn) => {
                rt.spawn(async move {
                    if let Err(e) = conv.interrupt(&turn).await {
                        ::log::warn!("octoscode: turn.interrupt: {e}");
                    }
                });
            }
            actions::Effect::Open(session) => {
                let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();
                rt.spawn(async move {
                    match conv.open_session(&session, cwd).await {
                        Ok(id) => ::log::info!("octoscode: thread.open opened {id}"),
                        Err(e) => ::log::warn!("octoscode: thread.open: {e}"),
                    }
                });
            }
            // Handled above / needs `cx` (copy).
            actions::Effect::ToggleTool(_)
            | actions::Effect::Unhandled(_)
            | actions::Effect::CopyAnswer => {}
        }
    }

    fn sync_labels(&mut self, cx: &mut Cx) {
        // Header labels only. The two virtualized lists are drawn in `draw_walk`
        // (a `PortalList` instantiates its visible items each frame — that is the
        // virtualization: a 2,000-entry timeline builds ~20 rows, not 2,000).
        let (status, sessions) = {
            let b = self.bridge.lock().unwrap();
            (b.store.summary(), b.store.session_count())
        };
        let text = format!("conn: {}", status.trim_start_matches("conn: "));
        self.view.label(cx, ids!(status)).set_text(cx, &text);
        self.view
            .label(cx, ids!(sessions))
            .set_text(cx, &format!("sessions: {sessions}"));
        // The #16 `composer` component is the dock's look. It is NOT virtualized
        // (one instance), so it is lowered here rather than in `draw_walk`; its
        // two live slots are the draft and the idle placeholder. Card #21b: it is
        // MOUNTED (evaluated in our VM + `mem::replace` + deep insert), not
        // `set_text` — the latter mints a standalone tree that never seats.
        let bridge = self.bridge.clone();
        let composer_live = {
            let b = self.bridge.lock().unwrap();
            let ctx = bindings::Ctx::new(&b.store, &b.ui);
            bindings::query(&ctx, "turn.active")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        };
        let composer = {
            let mut cache = std::mem::take(&mut self.cache);
            let c = cache
                .lower(&bridge, components::ItemKind::Composer, 0, None)
                .unwrap_or_default();
            self.cache = cache;
            c
        };
        // Card #21f item 1b: while a turn runs the SAME dock is the STOP control
        // (atlas conversation-08 `stop2`, a white 12×12 rounded square on the flat
        // black disc). The composer component carries one send glyph, so swap it
        // for the stop asset — otherwise the arrow persists through the whole
        // running turn (`g3b-turn2-running.png`). The mount cache compares the DSL
        // string, so the swap also forces exactly one repaint when `turn.active`
        // flips either way.
        let composer = if composer_live {
            composer
                .replace("icon_send-3fe1783d764e.svg", "icon_stop.svg")
                .replace("icon_send.svg", "icon_stop.svg")
        } else {
            composer
        };
        let composer_splash = self.view.splash(cx, ids!(composer_splash));
        if let Err(e) = self.mounts.mount(cx, &composer_splash, &composer) {
            makepad_widgets::log!("[octoscode] composer mount: {e}");
        }
        // Card #21d item 5: the `new-chat` component (#16) is the thread
        // column's first row. The row existed but was never mounted, so it
        // rendered as an empty pill (no label, no compose icon).
        let new_chat = {
            let mut cache = std::mem::take(&mut self.cache);
            let c = cache
                .lower(&bridge, components::ItemKind::NewChat, 0, None)
                .unwrap_or_default();
            self.cache = cache;
            c
        };
        let new_chat_splash = self.view.splash(cx, ids!(new_chat_splash));
        if let Err(e) = self.mounts.mount(cx, &new_chat_splash, &new_chat) {
            makepad_widgets::log!("[octoscode] new-chat mount: {e}");
        }
        ::log::info!("[octoscode] {text} | sessions: {sessions}");
    }

}

impl Widget for OctoscodeView {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // The two lists share makepad's default item template, so a draw step's
        // widget uid is how it says WHICH list it is (captured once).
        if self.thread_uid == 0 {
            self.thread_uid = self.view.portal_list(cx, ids!(thread_list)).widget_uid().0;
            self.timeline_uid = self.view.portal_list(cx, ids!(timeline_list)).widget_uid().0;
        }
        let (thread_uid, timeline_uid) = (self.thread_uid, self.timeline_uid);
        // Both the lowering cache and the mount cache are taken OUT of self so the
        // loop body borrows only `bridge` (a local Arc) — `self.view.draw_walk`
        // already holds `self.view`.
        let mut cache = std::mem::take(&mut self.cache);
        let mut mounts = std::mem::take(&mut self.mounts);
        let bridge = self.bridge.clone();

        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = step.as_portal_list().borrow_mut() {
                let uid = list.widget_uid().0;
                if uid == thread_uid {
                    // The LEFT column: one `thread-row` component per session.
                    let rows = {
                        let b = bridge.lock().unwrap();
                        screen::thread_rows(&b.store)
                    };
                    list.set_item_range(cx, 0, rows.len());
                    while let Some(id) = list.next_visible_item(cx) {
                        let Some(_row) = rows.get(id) else { continue };
                        let item = list.item(cx, id, id!(ThreadRowTpl));
                        let body = cache
                            .lower(&bridge, components::ItemKind::ThreadRow, id, None)
                            .unwrap_or_default();
                        let splash = item.splash(cx, ids!(thread_splash));
                        if let Err(e) = mounts.mount(cx, &splash, &body) {
                            makepad_widgets::log!("[octoscode] thread-row mount: {e}");
                        }
                        item.draw_all_unscoped(cx);
                    }
                } else if uid == timeline_uid {
                    // The CENTER column: one component per timeline entry, in
                    // display order (`screen::timeline_rows`).
                    let (live, rows) = {
                        let b = bridge.lock().unwrap();
                        let ctx = bindings::Ctx::new(&b.store, &b.ui);
                        let live = bindings::query(&ctx, "turn.active")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let rows = screen::timeline_rows(&b.store, live);
                        (live, rows)
                    };
                    let _ = live;
                    list.set_item_range(cx, 0, rows.len());
                    while let Some(id) = list.next_visible_item(cx) {
                        let Some(row) = rows.get(id) else { continue };
                        let item = list.item(cx, id, id!(TimelineItemTpl));
                        // Card #21c item 2: no native kind label on screen; the
                        // kind is carried by the component's own node ids in `/g`.
                        let body = cache
                            .lower(&bridge, row.kind, row.index, row.turn.as_deref())
                            .unwrap_or_default();
                        let splash = item.splash(cx, ids!(item_splash));
                        if let Err(e) = mounts.mount(cx, &splash, &body) {
                            makepad_widgets::log!("[octoscode] {} mount: {e}", row.kind.id());
                        }
                        item.draw_all_unscoped(cx);
                    }
                }
            }
        }
        self.cache = cache;
        self.mounts = mounts;
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        if !self.started {
            self.started = true;
            self.start();
            self.sync_labels(cx);
        }
        match event {
            Event::Signal => self.sync_labels(cx),
            Event::Actions(actions) => {
                // Card #21c item 5: the ONE composer is the mounted #16
                // component. Its own input is a real `TextInput` (id
                // `i0_composer_0` inside the mounted tree); its `changed` action
                // is the `composer.draft` binding (the component's own
                // `service-actions.json` declares exactly that behavior).
                if let Some(text) = self
                    .view
                    .text_input(cx, &[live_id!(i0_composer_0)])
                    .changed(actions)
                {
                    self.bridge.lock().unwrap().ui.lock().unwrap().set_draft_inner(text);
                }
                if self.view.button(cx, ids!(refresh)).clicked(actions) {
                    self.perform_action("session.refresh", 0);
                }
                // The #16 `new-chat` component overlaid by a host hit target.
                if self.view.button(cx, ids!(new_chat_hit)).clicked(actions) {
                    self.perform_action(bindings::ACTION_NEW_CHAT, 0);
                }
                // The composer's send control. While a turn is running the same
                // control is STOP (scene 08 / Codex) and sends `turn/interrupt`
                // (the L1 fix); otherwise it submits the draft.
                if self.view.button(cx, ids!(send_hit)).clicked(actions) {
                    let live = {
                        let b = self.bridge.lock().unwrap();
                        let ctx = bindings::Ctx::new(&b.store, &b.ui);
                        bindings::query(&ctx, "turn.active")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    };
                    if live {
                        self.perform_action(bindings::ACTION_INTERRUPT, 0);
                    } else {
                        self.perform_action(bindings::ACTION_SUBMIT, 0);
                    }
                }
                // `+` (attach) and the mic are not wired to a protocol method
                // yet; they are present as hit targets so the component's own
                // chrome stays clickable and the ids exist for a later card.
                // Card #21 §3 — the per-item controls. A row click is routed
                // WITH its item id (the same `items_with_actions` contract the
                // makepad examples use).
                let thread_list = self.view.portal_list(cx, ids!(thread_list));
                for (item_id, item) in thread_list.items_with_actions(actions) {
                    if item.button(cx, ids!(row_hit)).clicked(actions) {
                        self.perform_action("thread.open", item_id);
                    }
                }
                let timeline_list = self.view.portal_list(cx, ids!(timeline_list));
                let (live, rows) = {
                    let b = self.bridge.lock().unwrap();
                    let ctx = bindings::Ctx::new(&b.store, &b.ui);
                    let live = bindings::query(&ctx, "turn.active")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let rows = screen::timeline_rows(&b.store, live);
                    (live, rows)
                };
                let _ = live;
                for (item_id, item) in timeline_list.items_with_actions(actions) {
                    let Some(row) = rows.get(item_id) else { continue };
                    if !item.button(cx, ids!(row_hit)).clicked(actions) {
                        continue;
                    }
                    // Each row kind owns a different control id.
                    let control = match row.kind {
                        components::ItemKind::ToolCell => "expand",
                        components::ItemKind::AnswerActions => "copy",
                        _ => continue,
                    };
                    if let Some(action) = components::action_for(row.kind, control) {
                        if action == "tool.toggle" {
                            self.perform_action(action, row.index);
                        } else {
                            self.perform_action(action, 0);
                        }
                    }
                }
                self.sync_labels(cx);
            }
            _ => {}
        }
    }
}

pub struct OctoscodeModule;
pub static OCTOSCODE_MODULE: OctoscodeModule = OctoscodeModule;

impl AppModule for OctoscodeModule {
    fn id(&self) -> &'static str {
        "octoscode"
    }
    fn label(&self) -> &'static str {
        "OctosCode"
    }
    fn register(&self, vm: &mut ScriptVm) {
        script_mod(vm);
        // Card #21b: the design/kit vocabulary every lowered #16 component names
        // (`DesignSurface`, `KitButton`, …) must be in THIS VM — the isolate the
        // shell hosts the module in (`module_host.rs:133` allocates it, then
        // calls `register(vm)`/`create(vm, ..)` inside it). `register_vocabulary`
        // below uses `register_splash_isolate_mod`, which only reaches isolates
        // allocated AFTER it, so it cannot reach ours; register directly, exactly
        // as `beauty-host` does in the App's own `script_mod`
        // (`beauty.rs:356-364`).
        octoscript_widgets::design::script_mod(vm);
        octoscript_widgets::kit::script_mod(vm);
        // Card #15b: the same vocabulary, process-wide, for any OTHER isolate
        // (the tests and the card probes lower there).
        l0_host::register_vocabulary();
        // Card #21b: log what the RUNNING app resolves at startup
        // (`id -> path -> on-disk|placeholder`), so a capture's `/log` proves
        // which components root the launched process used — the test harness
        // passing from the repo root proved nothing.
        components::log_resolutions();
    }
    fn open_schema(&self) -> OpenSchema {
        OpenSchema::new(1)
    }
    fn capabilities(&self) -> &'static [&'static str] {
        &[]
    }
    fn create(
        &self,
        vm: &mut ScriptVm,
        _open: ValidatedOpen,
        _handles: InstanceHandles,
    ) -> InstanceParts {
        let value = script_eval!(vm, {
            use mod.widgets.*
            OctoscodeView {}
        });
        let root = WidgetRef::script_from_value(vm, value);
        // Inject the bridge into the widget's `#[rust]` field.
        let bridge = Arc::new(Mutex::new(Bridge {
            conv: None,
            store: Arc::new(Store::new()),
            ui: Arc::new(Mutex::new(FlowUi::default())),
        }));
        if let Some(mut view) = root.borrow_mut::<OctoscodeView>() {
            view.bridge = bridge;
        }
        InstanceParts {
            root,
            executor: Box::new(OctoscodeExecutor),
            shutdown: Box::new(|_| {}),
        }
    }
}

struct OctoscodeExecutor;
impl ServiceExecutor for OctoscodeExecutor {
    fn manifest(&self) -> ServiceManifest {
        ServiceManifest::new("octoscode", "OctosCode", "Native octoscode app (AppModule).")
    }
    fn execute(&mut self, _cx: &mut Cx, call: &ServiceCall) -> ExecOutcome {
        ExecOutcome::Done(ToolResult::unavailable(
            &call.call_id,
            "OctosCode (native module) has no tools",
        ))
    }
}
