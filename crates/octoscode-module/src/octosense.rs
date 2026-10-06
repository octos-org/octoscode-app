//! Optional adapter for embedding the native UI in an OctoSense shell.
//!
//! The shell runs its own octos kernel. The module opens a port to it at
//! creation (Makepad's `OctosUiPort`; OctoSense ADR 0003, "An app that is an
//! octos client") and the conversation runs over that port: no server to
//! dial, no token, no pairing. The shell holds the port to OctosCode's
//! scope.
use makepad_app_module::{
    makepad_ai_services::ui_port::{OctosUiPort, UiPortEvent},
    makepad_ai_services::wire::{ServiceCall, ServiceManifest, ToolResult},
    AppModule, ExecOutcome, InstanceHandles, InstanceParts, OpenSchema, ServiceExecutor,
    ValidatedOpen,
};
use makepad_widgets::*;
use octos_app_transport::host::{HostPort, HostPortEvent};

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
        // The shell restyles every app it hosts alike: follow its palette.
        crate::screens::theme::set_embedded(true);
        crate::register_widgets(vm);
    }
    fn open_schema(&self) -> OpenSchema {
        OpenSchema::new(1)
    }
    fn capabilities(&self) -> &'static [&'static str] {
        &["storage", "net"]
    }
    fn create(
        &self,
        vm: &mut ScriptVm,
        _open: ValidatedOpen,
        _handles: InstanceHandles,
    ) -> InstanceParts {
        let root = crate::create_view(vm);
        let port = OctosUiPort::open(vm.cx_mut());
        if let Some(mut view) = root.borrow_mut::<crate::OctoscodeView>() {
            view.embedded_in_octosense = true;
            view.host_port = Some(host_port(port));
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
        ServiceManifest::new("octoscode", "OctosCode", "Native OctosCode UI.")
    }
    fn execute(&mut self, _cx: &mut Cx, call: &ServiceCall) -> ExecOutcome {
        ExecOutcome::Done(ToolResult::unavailable(
            &call.call_id,
            "OctosCode has no tools",
        ))
    }
}

/// The shell's port, as the transport sees it.
fn host_port(mut port: OctosUiPort) -> HostPort {
    let sender = port.sender();
    HostPort {
        send: Box::new(move |frame| sender.send(frame)),
        recv: Box::new(move || match port.recv() {
            UiPortEvent::Frame(frame) => HostPortEvent::Frame(frame),
            UiPortEvent::Reset { reason } => HostPortEvent::Reset(reason),
            UiPortEvent::Closed { reason } => HostPortEvent::Closed(reason),
        }),
    }
}
