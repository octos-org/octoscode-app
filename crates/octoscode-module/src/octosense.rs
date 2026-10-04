//! Optional adapter for embedding the native UI in an OctoSense shell.
use makepad_app_module::{
    makepad_ai_services::wire::{ServiceCall, ServiceManifest, ToolResult},
    AppModule, ExecOutcome, InstanceHandles, InstanceParts, OpenSchema, ServiceExecutor,
    ValidatedOpen,
};
use makepad_widgets::*;

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
        if let Some(mut view) = root.borrow_mut::<crate::OctoscodeView>() {
            view.embedded_in_octosense = true;
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
