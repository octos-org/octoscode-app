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

pub mod bindings;
pub mod cards;
pub mod components;
pub mod fallback;
pub mod l0_host;
pub mod flow;
pub mod screen;

use flow::{Conversation, FlowUi};
// A top-level `::` path is not a `#[rust]` field type the `Script` derive's
// parser accepts (its `eat_type` reads one ident + optional generics), so the
// cache type is aliased to a bare ident here.
use screen::Cache as ScreenCache;

script_mod! {
    use mod.prelude.widgets.*
    mod.widgets.OctoscodeView = set_type_default() do #(OctoscodeView::register_widget(vm)) {
        ..mod.widgets.RectView
        width: Fill height: Fill
        draw_bg.color: theme.color_bg_app
        flow: Down padding: 16 spacing: 10
        header := View {
            width: Fill height: Fit
            flow: Right spacing: 10
            heading := Label {
                text: "OctosCode"
                draw_text.text_style.font_size: 20
            }
            status := Label { width: Fill draw_text.wrap: Words text: "conn: (connecting…)" }
            sessions := Label { width: Fit text: "sessions: 0" }
            refresh := Button { text: "session/list" }
            new_chat := Button { text: "New chat" }
        }
        // Card #17 (D12): native columns + virtualized L0 items. The host owns
        // structure + scale; each list item's LOOK comes from an L0 component
        // lowered in-process (`components.rs`, the same chain as a card). Left =
        // thread list, center = timeline with the composer docked at its bottom,
        // right = the review slot (empty for now). The #15b tab switcher and the
        // fallback composer are GONE.
        //
        // A row is a native `Label` (the kind tag, always present so the snap
        // names the row) + a `Splash` carrying that row's lowered component.
        // `Splash::set_text("")` tears the instance down and the `Fit` row takes
        // no space (Splash docs), so an empty body costs nothing.
        columns := View {
            width: Fill height: Fill
            flow: Right spacing: 10

            threads_column := View {
                width: 220 height: Fill flow: Down spacing: 6
                Label { width: Fill height: Fit text: "Threads" draw_text.text_style.font_size: 14 }
                // One `thread-row` component per session (virtualized).
                thread_list := PortalList {
                    width: Fill height: Fill flow: Down drag_scrolling: true
                    ThreadRowTpl := View {
                        width: Fill height: Fit flow: Down padding: 2
                        thread_name := Label { width: Fill height: Fit draw_text.wrap: Words draw_text.text_style.font_size: 13 text: "(thread)" }
                        thread_splash := Splash { width: Fill height: 34 }
                    }
                }
            }

            conversation_column := View {
                width: Fill height: Fill flow: Down spacing: 6
                Label { width: Fill height: Fit text: "Conversation" draw_text.text_style.font_size: 14 }
                // One L0 component per timeline entry, from the store timeline in
                // DISPLAY order (user first, reasoning folded, answer, tools,
                // worked-for) — `screen::timeline_rows`.
                timeline_list := PortalList {
                    width: Fill height: Fill flow: Down drag_scrolling: true
                    TimelineItemTpl := View {
                        width: Fill height: Fit flow: Down
                        item_kind := Label { width: Fill height: Fit draw_text.text_style.font_size: 9 text: "" }
                        item_splash := Splash { width: Fill height: 56 }
                    }
                }
                // The composer docked at the center column's bottom — ONE
                // composer. (The conversation-08 composer CARD, re-homed as the
                // dock, is a follow-up: #16 owns the composer component.)
                composer_row := View {
                    width: Fill height: Fit
                    flow: Right spacing: 8
                    draft := TextInput { width: Fill height: Fit empty_text: "Ask Octos anything" }
                    send := Button { text: "Send" }
                    stop := Button { text: "Stop" }
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

    /// Run one binding action, off the UI thread.
    fn perform_action(&self, action: &str) {
        let Some(rt) = self.runtime.as_ref() else {
            return;
        };
        let Some(conv) = ({
            let b = self.bridge.lock().unwrap();
            b.conv.clone()
        }) else {
            return;
        };
        match action {
            bindings::ACTION_SUBMIT => {
                rt.spawn(async move {
                    if let Err(e) = conv.submit_draft().await {
                        ::log::warn!("octoscode: composer.submit: {e}");
                    }
                });
            }
            bindings::ACTION_INTERRUPT => {
                let turn = conv.ui().lock().unwrap().active_turn();
                if let Some(turn) = turn {
                    rt.spawn(async move {
                        if let Err(e) = conv.interrupt(&turn).await {
                            ::log::warn!("octoscode: turn.interrupt: {e}");
                        }
                    });
                }
            }
            "session.refresh" => {
                rt.spawn(async move {
                    if let Err(e) = conv.refresh_sessions().await {
                        ::log::warn!("octoscode: session.refresh: {e}");
                    }
                });
            }
            // Card #14 defect 4: "New chat" mints a FRESH session id, so a new
            // chat never reuses the previous run's context.
            bindings::ACTION_NEW_CHAT => {
                let cwd = std::env::var("OCTOS_WORKSPACE_CWD").ok();
                rt.spawn(async move {
                    match conv.new_chat(cwd).await {
                        Ok(id) => ::log::info!("octoscode: new chat opened {id}"),
                        Err(e) => ::log::warn!("octoscode: session.new: {e}"),
                    }
                });
            }
            other => ::log::warn!("octoscode: unhandled action id {other:?}"),
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
        // The cache is taken OUT of self so the loop body borrows only `bridge`
        // (a local Arc) — `self.view.draw_walk` already holds `self.view`.
        let mut cache = std::mem::take(&mut self.cache);
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
                        let Some(row) = rows.get(id) else { continue };
                        let item = list.item(cx, id, id!(ThreadRowTpl));
                        item.label(cx, ids!(thread_name)).set_text(cx, &row.title);
                        let body = cache
                            .lower(&bridge, components::ItemKind::ThreadRow, id)
                            .unwrap_or_default();
                        item.splash(cx, ids!(thread_splash)).set_text(cx, &body);
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
                        item.label(cx, ids!(item_kind)).set_text(cx, row.kind.id());
                        let body = cache.lower(&bridge, row.kind, row.index).unwrap_or_default();
                        item.splash(cx, ids!(item_splash)).set_text(cx, &body);
                        item.draw_all_unscoped(cx);
                    }
                }
            }
        }
        self.cache = cache;
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
                // The composer draft is the input's `changed` value — how the
                // web binds `composer.draft` (`bindings.json` composer.draft
                // note: behavior.event='changed' updates it).
                if let Some(text) = self.view.text_input(cx, ids!(draft)).changed(actions) {
                    self.bridge.lock().unwrap().ui.lock().unwrap().set_draft_inner(text);
                }
                // Header + composer controls emit BINDING ACTION ids.
                if self.view.button(cx, ids!(refresh)).clicked(actions) {
                    self.perform_action("session.refresh");
                }
                if self.view.button(cx, ids!(new_chat)).clicked(actions) {
                    self.perform_action(bindings::ACTION_NEW_CHAT);
                }
                if self.view.button(cx, ids!(send)).clicked(actions) {
                    // Card #13 §4: the draft clears on send. The flow clears
                    // it in the STORE (`start_turn`), but the widget keeps its
                    // own text, so clear the widget too — otherwise the sent
                    // prompt stays visible in the composer.
                    let len = self
                        .bridge
                        .lock()
                        .unwrap()
                        .ui
                        .lock()
                        .unwrap()
                        .draft()
                        .len();
                    self.perform_action(bindings::ACTION_SUBMIT);
                    if len > 0 {
                        let _ = self.view.text_input(cx, ids!(draft)).replace_range(
                            cx,
                            0..len,
                            "",
                            makepad_widgets::text_input::UndoGroup::New,
                        );
                    }
                }
                if self.view.button(cx, ids!(stop)).clicked(actions) {
                    self.perform_action(bindings::ACTION_INTERRUPT);
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
        // Card #15b: the L0 vocabulary every lowered card names
        // (`DesignSurface`, `DesignNativeButton`, …). Process-wide, exactly as
        // card-host registers it (`host.rs:206-215`).
        l0_host::register_vocabulary();
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
