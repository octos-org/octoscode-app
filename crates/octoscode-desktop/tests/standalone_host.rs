//! The desktop window creates the native view without an AppModule host.
use makepad_widgets::*;
use octoscode_desktop::{create_view, register_widgets};

#[test]
fn the_standalone_app_creates_its_native_view() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.with_vm(makepad_widgets::script_mod);
    cx.with_vm(register_widgets);
    let root = cx.with_vm(create_view);
    assert!(root.borrow::<octoscode_module::OctoscodeView>().is_some());
    for (name, path) in [
        ("first_run", ids!(first_run)),
        ("base", ids!(base)),
        ("screen_dock", ids!(screen_dock)),
    ] {
        assert!(
            !root.widget(&mut cx, path).is_empty(),
            "{name} is in the native tree"
        );
    }
    let errors = cx.with_vm(|vm| vm.take_errors());
    assert!(errors.is_empty(), "native view script errors: {errors:?}");
}
