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
pub mod fallback;
pub mod l0_host;
pub mod flow;

use flow::{Conversation, FlowUi};

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
        // Card #15b (D12): the HOST lays out the desktop and shows ONE card at
        // a time. Every card is a 406x776 artboard whose children sit at
        // MEASURED `abs_pos` — the box Gate-B captured — so the slot gives it
        // exactly that box. A squeezed box clips every absolute child
        // (measured: a 384x109 slot left 20 of 23 nodes at zero geometry).
        // All five stay MOUNTED (the binding layer drives every one); the
        // selector picks the visible one and the area scrolls, because the
        // tile is shorter than the artboard.
        tabs := View {
            width: Fill height: Fit
            flow: Right spacing: 6
            tab_0 := Button { text: "01 threads" }
            tab_1 := Button { text: "03 turn" }
            tab_2 := Button { text: "04 tools" }
            tab_3 := Button { text: "09 answer" }
            tab_4 := Button { text: "08 composer" }
        }
        card_area := ScrollYView {
            width: Fill height: Fill
            // The canonical makepad scroll pattern: a `Fit`-height column INSIDE
            // the scroll view. A direct child of `ScrollYView` is constrained to
            // the viewport (measured: a 776-tall slot came out 471), so the card
            // is clipped; the `Fit` column takes the artboard's full height and
            // the view scrolls it.
            cards_col := View {
            // A DEFINITE height, not `Fit`: an isolate-hosted `Splash` does not
            // report intrinsic height to a scroll parent (measured: `Fit` came
            // out equal to the 471 viewport, so the artboard's lower rows were
            // clipped and unreachable). A definite 830 gives the view a scroll
            // range that covers the whole 776 card.
            width: Fill height: 830
            flow: Down
            thread_card := Splash { width: 406 height: 776 }
            threads_label := Label { width: Fill height: Fit draw_text.wrap: Words draw_text.text_style.font_size: 12 text: "threads: (none)" }
            timeline_card := Splash { width: 406 height: 776 }
            timeline_label := Label { width: Fill height: Fit draw_text.wrap: Words draw_text.text_style.font_size: 12 text: "(no timeline)" }
            tools_card := Splash { width: 406 height: 776 }
            tools_label := Label { width: Fill height: Fit draw_text.wrap: Words draw_text.text_style.font_size: 12 text: "tools: (none)" }
            answer_card := Splash { width: 406 height: 776 }
            answer_label := Label { width: Fill height: Fit draw_text.wrap: Words draw_text.text_style.font_size: 12 text: "" }
            composer_card := Splash { width: 406 height: 776 }
            // The composer card IS the composer: its `composer_input` is a real
            // `TextInput`. `composer_row` is the plain fallback, up only while
            // the composer card is BROKEN — one composer.
            composer_row := View {
                width: Fill height: Fit
                flow: Right spacing: 8
                draft := TextInput { width: Fill height: Fit empty_text: "Ask Octos anything" }
                send := Button { text: "Send" }
                stop := Button { text: "Stop" }
            }
            }
        }
    }
}

/// What the UI thread needs to drive the conversation, behind one lock.
#[derive(Default)]
struct Bridge {
    /// The conversation (transport + store + flow UI). `None` until `start`.
    conv: Option<Arc<Conversation>>,
    /// The store, so a binding read still works before the flow exists.
    store: Arc<Store>,
    /// The flow UI, for the same reason.
    ui: Arc<Mutex<FlowUi>>,
}

impl Bridge {
    /// The binding table's two inputs (`store` + the flow's UI state).
    fn ctx(&self) -> (&Arc<Store>, &Mutex<FlowUi>) {
        (&self.store, &self.ui)
    }

    /// Resolve one binding id through the table.
    fn value(&self, id: &str) -> Option<serde_json::Value> {
        let (store, ui) = self.ctx();
        bindings::query(&bindings::Ctx::new(store, ui), id)
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
    /// Which card the host shows (index into [`cards::Slot::ALL`]). Every card
    /// stays MOUNTED (the binding layer drives each one); the 406x776 artboards
    /// cannot share one tile, so the host shows one. `OCTOSCODE_ACTIVE_SLOT` =
    /// a slot name picks the initial one, so a capture run needs no click.
    #[rust]
    active_slot: usize,
}

impl OctoscodeView {
    fn start(&mut self) {
        self.active_slot = std::env::var("OCTOSCODE_ACTIVE_SLOT")
            .ok()
            .and_then(|name| cards::Slot::from_name(&name))
            .and_then(|slot| cards::Slot::ALL.iter().position(|x| *x == slot))
            .unwrap_or(0);
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
        // Header labels (store summary), then the binding-only fallback view.
        let (status, sessions, values) = {
            let b = self.bridge.lock().unwrap();
            let ids = bindings::all_binding_ids();
            let values: Vec<Option<serde_json::Value>> =
                ids.iter().map(|id| b.value(id)).collect();
            (
                b.store.summary(),
                b.store.session_count(),
                values,
            )
        };
        let text = format!("conn: {}", status.trim_start_matches("conn: "));
        self.view.label(cx, ids!(status)).set_text(cx, &text);
        self.view
            .label(cx, ids!(sessions))
            .set_text(cx, &format!("sessions: {sessions}"));

        // The fallback view reads bindings by id; it never sees a Rust type.
        let ids = bindings::all_binding_ids();
        let resolver = |id: &str| {
            ids.iter()
                .position(|i| *i == id)
                .and_then(|i| values.get(i).cloned().flatten())
        };
        // Card #15b: mount each card through the **L0 runtime** — the same
        // renderer that produced the Gate-B `*-native.png` renders
        // (`card-host/src/host.rs:144-154`): the card is realized + lowered to
        // Splash DSL with the live binding values injected, and that DSL is set
        // into the slot's `Splash` widget (`host.rs:301-304`). Each slot is
        // independent: a slot whose card is missing or fails to lower keeps its
        // plain fallback widget visible and is logged by name, so the screen
        // always renders. (`ids!` per arm: makepad's id macros want literals.)
        let active = self.active_slot.min(cards::Slot::ALL.len() - 1);
        let mut card_shown = Vec::new();
        macro_rules! mount {
            ($idx:expr, $slot:expr, $card:ident, $fb:ident, $name:literal) => {
                // Every card is lowered with the LIVE bindings; the selector
                // only decides which one the host shows (the artboards cannot
                // share one tile).
                // Only the ACTIVE slot carries a body: an empty `Splash` body
                // tears the instance down (`Splash::reapply_text` docs) and the
                // `height: Fit` slot then takes no space, so exactly one card is
                // on screen at its natural 406x776 — which is what makes each
                // slot comparable to its Gate-B render.
                let body = if $idx == active {
                    l0_host::slot_body($slot, &resolver)
                } else {
                    // Still exercise the path off-screen? No: skip the work.
                    Ok(String::new())
                };
                match body {
                    Ok(body) => {
                        self.view.splash(cx, ids!($card)).set_text(cx, &body);
                        self.view.widget(cx, ids!($fb)).set_visible(cx, false);
                        if $idx == active {
                            card_shown.push($name);
                        }
                    }
                    Err(e) => {
                        ::log::warn!(
                            "octoscode: card slot `{}` fell back to the plain widget: {e}",
                            $name
                        );
                        self.view.splash(cx, ids!($card)).set_text(cx, "");
                        self.view
                            .widget(cx, ids!($fb))
                            .set_visible(cx, $idx == active);
                    }
                }
            };
        }
        mount!(0, cards::Slot::ThreadList, thread_card, threads_label, "thread_list");
        mount!(1, cards::Slot::Conversation, timeline_card, timeline_label, "conversation");
        mount!(2, cards::Slot::ToolCells, tools_card, tools_label, "tool_cells");
        mount!(3, cards::Slot::CompletedAnswer, answer_card, answer_label, "completed_answer");
        // The composer card IS the composer: its `composer_input` is a real
        // `TextInput`, so ONE composer shows. `composer_row` is the plain
        // fallback and is up only while the composer card is BROKEN (entry #2).
        let composer_body = if active == 4 {
            l0_host::slot_body(cards::Slot::Composer, &resolver)
        } else {
            Ok(String::new())
        };
        match composer_body {
            Ok(body) => {
                self.view.splash(cx, ids!(composer_card)).set_text(cx, &body);
                self.view.widget(cx, ids!(composer_row)).set_visible(cx, false);
                if active == 4 {
                    card_shown.push("composer");
                }
            }
            Err(e) => {
                ::log::warn!("octoscode: card slot `composer` fell back to the plain composer: {e}");
                self.view.splash(cx, ids!(composer_card)).set_text(cx, "");
                self.view
                    .widget(cx, ids!(composer_row))
                    .set_visible(cx, active == 4);
            }
        }
        // The selector's own state (idempotent: setting the same text each pass
        // would otherwise grow the label).
        let tab_names = ["01 threads", "03 turn", "04 tools", "09 answer", "08 composer"];
        for (idx, tab) in [
            (0usize, ids!(tab_0)),
            (1, ids!(tab_1)),
            (2, ids!(tab_2)),
            (3, ids!(tab_3)),
            (4, ids!(tab_4)),
        ] {
            let label = if idx == active {
                format!("[{}]", tab_names[idx])
            } else {
                tab_names[idx].to_owned()
            };
            if self.view.button(cx, tab).text() != label {
                self.view.button(cx, tab).set_text(cx, &label);
            }
        }
        // The fallback renderer still fills its own widgets; the ones a card
        // covered are hidden above, so what shows is the card.
        fallback::render(&self.view, cx, &resolver);
        ::log::info!("[octoscode] cards mounted: {}", card_shown.join(", "));
        ::log::info!("[octoscode] {text} | sessions: {sessions}");
    }
}

impl Widget for OctoscodeView {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
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
                // The card selector (D12: the host chooses what it shows).
                let mut picked = None;
                for (idx, tab) in [
                    (0usize, ids!(tab_0)),
                    (1, ids!(tab_1)),
                    (2, ids!(tab_2)),
                    (3, ids!(tab_3)),
                    (4, ids!(tab_4)),
                ] {
                    if self.view.button(cx, tab).clicked(actions) {
                        picked = Some(idx);
                    }
                }
                if let Some(idx) = picked {
                    self.active_slot = idx;
                    self.sync_labels(cx);
                }
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
