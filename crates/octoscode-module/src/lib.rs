//! octoscode-module — the `AppModule` (re-homed from the D9 shape-(i) spike).
//!
//! It owns a [`Store`] and a transport on a background tokio runtime, renders
//! the connection state + session count **from the store**, and exposes store
//! data to cards only through [`bindings`] (a card never sees a Rust type).
//! L0 cards mount here later; this card ships no card.
//!
//! Threading rule (from the spike): the UI thread never blocks. The tokio
//! runtime owns the transport; its event waker (`SignalToUI`) wakes the UI,
//! which drains into the store and re-reads the store into labels.
pub use makepad_widgets;

use makepad_app_module::{
    makepad_ai_services::wire::{ServiceCall, ServiceManifest, ToolResult},
    AppModule, ExecOutcome, InstanceHandles, InstanceParts, OpenSchema, ServiceExecutor,
    ValidatedOpen,
};
use makepad_widgets::*;
use std::sync::{Arc, Mutex};

use octos_app_transport::{
    ws, Capabilities, ConnectionState, LifecycleResult, OutboundCommand, ProfileId, SecretString,
    TransportConfig, TransportEvent,
};
use octos_core::ui_protocol::SessionOpenParams;
use octoscode_client::{domains, registry::Registry, Client};
use octoscode_store::Store;
use url::Url;

pub mod bindings;

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
    }
}

/// What the UI thread needs to drive the transport, behind one lock.
#[derive(Default)]
struct Bridge {
    store: Arc<Store>,
    registry: Registry,
    client: Option<Client>,
    cmd_tx: Option<tokio::sync::mpsc::Sender<OutboundCommand>>,
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
        let base_url =
            Url::parse(&base).unwrap_or_else(|_| Url::parse("http://127.0.0.1:50082").unwrap());

        let cfg = TransportConfig {
            base_url,
            bearer: SecretString::new(bearer),
            profile_id: ProfileId::new(&profile),
            cursor: None,
            cursor_file: None,
            requested_capabilities: Capabilities::requested(),
            workspace_cwd: None,
            local_kernel: false,
        };

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

        let (cmd_tx, mut evt_rx) = {
            let _guard = runtime.enter();
            ws::spawn_with_waker(cfg, Some(Arc::new(|| SignalToUI::set_ui_signal())))
        };

        {
            let mut b = self.bridge.lock().unwrap();
            b.cmd_tx = Some(cmd_tx.clone());
            b.client = Some(Client::new(cmd_tx.clone()));
            b.store
                .set_connection(format!("{:?}", ConnectionState::Dialing), false);
        }

        let open = SessionOpenParams {
            session_id: octos_core::SessionKey::new("octoscode", "main"),
            topic: None,
            profile_id: Some(profile),
            cwd: None,
            sandbox: None,
            after: None,
            client_commands: None,
        };

        let bridge = self.bridge.clone();
        runtime.spawn(async move {
            if cmd_tx.send(OutboundCommand::OpenSession(open)).await.is_err() {
                ::log::error!("octoscode: transport channel closed");
                SignalToUI::set_ui_signal();
                return;
            }
            while let Some(evt) = evt_rx.recv().await {
                let mut b = bridge.lock().unwrap();
                match evt {
                    TransportEvent::ConnectionState(s) => {
                        let live = matches!(s, ConnectionState::Live);
                        b.store.set_connection(format!("{s:?}"), live);
                    }
                    TransportEvent::CapabilityNegotiated(caps) => {
                        let accepted: Vec<String> = caps.raw.keys().cloned().collect();
                        b.store.set_capabilities(accepted);
                    }
                    TransportEvent::RpcResult(LifecycleResult::SessionOpen(r)) => {
                        b.store.set_active(Some(r.opened.session_id.0.clone()));
                    }
                    TransportEvent::SessionsListed { sessions } => {
                        // `SessionsListed` carries the rows ARRAY (the
                        // transport already unwrapped `SessionListResult`).
                        if let Ok(rows) = serde_json::from_value::<Vec<octoscode_client::domains::session::SessionListRow>>(sessions) {
                            b.store
                                .set_sessions(rows.into_iter().map(Into::into).collect());
                        }
                    }
                    TransportEvent::DurableNotification { payload, .. }
                    | TransportEvent::EphemeralNotification { payload } => {
                        b.registry.dispatch(&payload);
                    }
                    TransportEvent::RpcError { method, error, .. } => {
                        ::log::warn!("octoscode: rpc error {method}: {}", error.message);
                    }
                    _ => {}
                }
                SignalToUI::set_ui_signal();
            }
        });

        self.runtime = Some(runtime);
    }

    fn refresh_sessions(&self) {
        let b = self.bridge.lock().unwrap();
        if let Some(tx) = &b.cmd_tx {
            let _ = tx.try_send(OutboundCommand::ListSessions);
        }
    }

    fn sync_labels(&mut self, cx: &mut Cx) {
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
                if self.view.button(cx, ids!(refresh)).clicked(actions) {
                    self.refresh_sessions();
                }
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
        let store = Arc::new(Store::new());
        let mut registry = Registry::new();
        domains::register_all(&mut registry, store.clone());
        let value = script_eval!(vm, {
            use mod.widgets.*
            OctoscodeView {}
        });
        let root = WidgetRef::script_from_value(vm, value);
        // Inject the bridge into the widget's `#[rust]` field.
        let bridge = Arc::new(Mutex::new(Bridge {
            store,
            registry,
            client: None,
            cmd_tx: None,
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
