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
pub mod fallback;
pub mod flow;

use flow::{Conversation, FlowUi};

script_mod! {
    use mod.prelude.widgets.*
    mod.widgets.OctoscodeView = set_type_default() do #(OctoscodeView::register_widget(vm)) {
        ..mod.widgets.RectView
        width: Fill height: Fill
        draw_bg.color: theme.color_bg_app
        flow: Down padding: 24 spacing: 14
        heading := Label {
            text: "OctosCode"
            draw_text.text_style.font_size: 20
        }
        status := Label { width: Fill draw_text.wrap: Words text: "conn: (connecting…)" }
        sessions := Label { width: Fill text: "sessions: 0" }
        refresh := Button { text: "session/list" }
        new_chat := Button { text: "New chat" }
        // Card #12 §4: the plain FALLBACK conversation view (binding-only),
        // replaced by the mounted L0 cards (#11b) later. Every id below is a
        // binding id (`threads`, `timeline.entries`, `tools`,
        // `answer.worked_for`, `composer.*`); the view never sees a Rust type.
        threads_label := Label { width: Fill draw_text.text_style.font_size: 13 text: "threads: (none)" }
        timeline_label := Label { width: Fill height: Fill draw_text.wrap: Words text: "(no timeline)" }
        tools_label := Label { width: Fill draw_text.text_style.font_size: 13 text: "tools: (none)" }
        answer_label := Label { width: Fill draw_text.text_style.font_size: 13 text: "" }
        composer_row := View {
            width: Fill height: Fit
            flow: Right spacing: 8
            draft := TextInput { width: Fill empty_text: "Ask Octos anything" }
            send := Button { text: "Send" }
            stop := Button { text: "Stop" }
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
}

impl OctoscodeView {
    fn start(&mut self) {
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
        fallback::render(&self.view, cx, &resolver);
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
