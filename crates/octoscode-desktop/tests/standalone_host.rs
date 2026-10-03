//! A33 — the standalone host registers and mounts the octoscode module the way
//! the OctoSense shell does (`module_host.rs` `ModuleHost::create`): its own
//! isolate, `register` + `create` in one trusted entry, the module's
//! `OctoscodeView` as the root, no script error, and a clean teardown.
//!
//! Before A33 nothing hosted the module outside OctoSense; this is the seam
//! the standalone app's `main.rs` mounts it through.
use makepad_widgets::*;
use octoscode_desktop::{create_instance, OCTOSCODE_MODULE};

#[test]
fn the_standalone_host_mounts_the_module_in_its_own_isolate() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.with_vm(makepad_widgets::script_mod);
    cx.with_vm(octoscode_desktop::script_mod);
    let instance =
        create_instance(&mut cx, &OCTOSCODE_MODULE, dvec2(1280.0, 800.0)).expect("the module mounts");
    assert_eq!(instance.module.id(), "octoscode");
    assert_eq!(instance.module.label(), octoscode_desktop::APP_NAME);
    assert_ne!(instance.vm_id, MAIN_SPLASH_VM_ID, "the module runs in an isolate of its own, as in the shell");
    assert!(
        instance.root.borrow::<octoscode_module::OctoscodeView>().is_some(),
        "the root is the module's OctoscodeView"
    );
    // The frames the window shows: the first-run frame (the Connect card's
    // home), the base chrome and the screen dock.
    for (name, path) in [("first_run", ids!(first_run)), ("base", ids!(base)), ("screen_dock", ids!(screen_dock))] {
        assert!(!instance.root.widget(&mut cx, path).is_empty(), "{name} is in the mounted tree");
    }
    let errors = cx.with_script_vm_id_trusted(instance.vm_id, |vm| vm.take_errors());
    assert!(errors.is_empty(), "the isolate's script errors: {errors:?}");
    assert_eq!(instance.manifest().id, "octoscode", "the executor answers for the module");
    instance.teardown(&mut cx);
}
