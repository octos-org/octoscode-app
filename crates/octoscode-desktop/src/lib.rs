//! OctosCode as a standalone desktop app (A33, decision D10f).
//!
//! The octoscode module is an [`AppModule`]: the OctoSense shell hosts it
//! in-process (`crates/shell/src/module_host.rs` creates it,
//! `module_view.rs` seats its root in a window-manager tile). This crate is
//! the other host: a plain makepad app whose window holds the module's root
//! and nothing else. No OctoSense shell, desktop, dock or
//! `MAKEPAD_WM_TEST_APP`. Both hosts run the SAME module code.
//!
//! What the shell gives the module, and what this host gives it instead:
//!
//! | the shell (module_host.rs / module_view.rs) | here |
//! |---|---|
//! | an isolate per instance (`alloc_splash_vm_with_network(false)`) | the same |
//! | `register` + `create` in ONE trusted entry (`with_script_vm_id_trusted`) | the same |
//! | the WM palette (`makepad_wm_theme::apply`) | none: it is a no-op outside the WM (no `MAKEPAD_WM_THEME_SPLASH`); the module assigns its own theme roles |
//! | a desktop style sheet (`apply_module_style`) | none: the shell installs none on macOS either |
//! | storage jail `cx.storage("octoscode.1")`, viewport, reply sink | the same (the module keeps none of them) |
//! | extra windows (`ModuleWindows`) | unsupported, like the phone shell (the module opens none) |
//! | the tile: root linked into the widget tree, drawn and fed events inside the isolate, modals bounded to the tile | the same, over the whole window |
//! | keys only while the WM focus is on the tile | always: the module IS the window |
//! | the window (1400x900 desktop, a ~990x600 tile) | the app's own window, titled "OctosCode", [`DEFAULT_WINDOW_SIZE`] or `OCTOSENSE_WINDOW_SIZE` / `OCTOSCODE_WINDOW_SIZE` |
//! | clipboard, URL opening, notifications, `--remote` | makepad's own (`Cx`), the same calls in both hosts |
//!
//! The module's design files and its own fonts and icons ride inside the
//! binary (the module's build.rs embed, materialized under
//! `$HOME/.octoscode/design`); makepad's resources ride in the app bundle
//! (tools/package-macos.sh).

use makepad_app_module::{
    AppModule, InstanceHandles, InstanceScope, ModuleUpstream, ModuleWindows, ReplySink, ServiceExecutor,
    Viewport,
};
use makepad_widgets::widget_async::{enter_isolate, leave_isolate};
use makepad_widgets::*;
use std::sync::mpsc::Receiver;

pub use octoscode_module::OCTOSCODE_MODULE;

/// What the window, the app menu and the Dock read.
pub const APP_NAME: &str = "OctosCode";

/// The window size when nothing asks for another: a desktop window wide
/// enough for the module's docked sidebar (it hides below 760 px).
pub const DEFAULT_WINDOW_SIZE: (f64, f64) = (1280.0, 800.0);

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    mod.widgets.OctoscodeHostBase = #(OctoscodeHost::register_widget(vm))

    mod.widgets.OctoscodeHost = set_type_default() do mod.widgets.OctoscodeHostBase {
        width: Fill
        height: Fill
        // The ground under a root that paints only its own chrome; replaced by
        // the module's own `theme.color_bg_app` once it is mounted.
        draw_bg +: { color: theme.color_bg_app }
    }
}

/// `WxH` (`1280x800`, `360X780`) as logical pixels; `None` for anything else.
pub fn parse_window_size(spec: &str) -> Option<(f64, f64)> {
    let (w, h) = spec.trim().split_once(['x', 'X'])?;
    let (w, h) = (w.trim().parse::<f64>().ok()?, h.trim().parse::<f64>().ok()?);
    (w.is_finite() && h.is_finite() && w >= 200.0 && h >= 120.0).then_some((w, h))
}

/// The size the window asks for: `OCTOSENSE_WINDOW_SIZE` (the size the module
/// itself treats as authoritative, `lib.rs` env_frame), then
/// `OCTOSCODE_WINDOW_SIZE`, then [`DEFAULT_WINDOW_SIZE`].
pub fn window_size_from(octosense: Option<&str>, octoscode: Option<&str>) -> (f64, f64) {
    octosense
        .and_then(parse_window_size)
        .or_else(|| octoscode.and_then(parse_window_size))
        .unwrap_or(DEFAULT_WINDOW_SIZE)
}

/// [`window_size_from`] over this process's environment.
pub fn window_size() -> (f64, f64) {
    window_size_from(
        std::env::var("OCTOSENSE_WINDOW_SIZE").ok().as_deref(),
        std::env::var("OCTOSCODE_WINDOW_SIZE").ok().as_deref(),
    )
}

/// One hosted module instance: its isolate, its root, and what the module
/// handed back at `create`.
pub struct Instance {
    pub module: &'static dyn AppModule,
    pub vm_id: SplashVmId,
    pub root: WidgetRef,
    executor: Box<dyn ServiceExecutor>,
    shutdown: Option<Box<dyn FnOnce(&mut ScriptVm)>>,
    /// Results the executor sends later. This host runs no assistant bus,
    /// so nothing reads them; the receiver lives as long as the instance so
    /// a late reply is not an error.
    _upstream: Receiver<ModuleUpstream>,
}

impl Instance {
    /// The module's service manifest (this host exposes no assistant bus;
    /// kept for parity with the shell and for tests).
    pub fn manifest(&self) -> makepad_app_module::makepad_ai_services::wire::ServiceManifest {
        self.executor.manifest()
    }

    /// End the instance the way the shell does: the root is dropped by the
    /// caller first, `shutdown` runs inside the isolate, then the isolate is
    /// freed.
    pub fn teardown(mut self, cx: &mut Cx) {
        let vm_id = self.vm_id;
        if let Some(shutdown) = self.shutdown.take() {
            cx.with_script_vm_id_trusted(vm_id, |vm| shutdown(vm));
        }
        drop(self);
        cx.free_splash_vm(vm_id);
    }
}

/// Build one instance of `module` exactly as the OctoSense shell does
/// (`module_host.rs` `ModuleHost::create`): an isolate of its own, then
/// `register` and `create` inside ONE trusted entry into it, so the module
/// never holds a second `&mut Cx` beside the VM. `viewport` is the size the
/// root will be given.
pub fn create_instance(cx: &mut Cx, module: &'static dyn AppModule, viewport: DVec2) -> Result<Instance, String> {
    let open = module.open_schema().empty_open()?;
    let storage = cx.storage(&format!("{}.1", module.id()));
    let (replies, upstream) = ReplySink::pair();
    let handles = InstanceHandles {
        scope: InstanceScope::new(1, 1),
        storage,
        viewport: Viewport { size: viewport },
        replies,
        // No extra host windows (as on the phone shell): a module presents
        // its secondary surfaces in its own window.
        windows: ModuleWindows::new(false),
    };
    let vm_id = cx.alloc_splash_vm_with_network(false);
    let parts = cx.with_script_vm_id_trusted(vm_id, |vm| {
        module.register(vm);
        module.create(vm, open, handles)
    });
    if parts.root.is_empty() {
        cx.free_splash_vm(vm_id);
        return Err(format!("module {} created no root widget", module.id()));
    }
    Ok(Instance {
        module,
        vm_id,
        root: parts.root,
        executor: parts.executor,
        shutdown: Some(parts.shutdown),
        _upstream: upstream,
    })
}

/// The window's one widget: draws the hosted root over the whole window and
/// feeds it every event, inside the instance's isolate (the shell's
/// `MpModuleView`, without the window-manager focus gate).
#[derive(Script, ScriptHook, Widget)]
pub struct OctoscodeHost {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[redraw]
    #[live]
    draw_bg: DrawColor,
    #[rust]
    instance: Option<Instance>,
    #[rust]
    area: Area,
}

impl OctoscodeHost {
    /// Seat `instance`: its root becomes this widget's child in the widget
    /// tree (what `/snap` and widget lookups walk), then draws here.
    pub fn set_instance(&mut self, cx: &mut Cx, instance: Instance) {
        cx.widget_tree_insert_child(self.uid, live_id!(root), instance.root.clone());
        log!(
            "[octoscode-desktop] module {} ({}) mounted in isolate {:?}",
            instance.module.id(),
            instance.module.label(),
            instance.vm_id
        );
        self.instance = Some(instance);
        self.draw_bg.redraw(cx);
    }

    pub fn instance(&self) -> Option<&Instance> {
        self.instance.as_ref()
    }

    /// The hosted root, if a module is mounted.
    pub fn root(&self) -> Option<WidgetRef> {
        self.instance.as_ref().map(|i| i.root.clone())
    }

    /// Drop the root first (nothing may draw a widget whose heap is about to
    /// go), then end the instance.
    pub fn teardown(&mut self, cx: &mut Cx) {
        if let Some(mut instance) = self.instance.take() {
            instance.root = WidgetRef::empty();
            instance.teardown(cx);
            log!("[octoscode-desktop] module torn down");
        }
    }
}

impl Widget for OctoscodeHost {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Event::Shutdown = event {
            self.teardown(cx);
            return;
        }
        let Some((root, vm_id)) = self.instance.as_ref().map(|i| (i.root.clone(), i.vm_id)) else {
            return;
        };
        let entry = enter_isolate(cx, vm_id);
        root.handle_event(cx, event, scope);
        leave_isolate(cx, entry);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        if let Some(vm_id) = self.instance.as_ref().map(|i| i.vm_id) {
            // The instance's current ground (its theme can change at run time).
            if let Some(color) =
                cx.with_script_vm_id_trusted(vm_id, |vm| script_eval!(vm, { mod.theme.color_bg_app })).as_color()
            {
                self.draw_bg.color = Vec4f::from_u32(color);
            }
        }
        self.draw_bg.draw_abs(cx, rect);
        if let Some((root, vm_id)) = self.instance.as_ref().map(|i| (i.root.clone(), i.vm_id)) {
            let entry = enter_isolate(cx, vm_id);
            // The instance's modals dim and centre within this widget (the
            // whole window here), exactly as within a shell tile.
            let outer = std::mem::replace(&mut cx.global::<ModalBounds>().0, Some(rect));
            root.draw_walk_all(cx, scope, Walk::fill());
            cx.global::<ModalBounds>().0 = outer;
            leave_isolate(cx, entry);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_sizes_parse_and_fall_back() {
        assert_eq!(parse_window_size("360x780"), Some((360.0, 780.0)));
        assert_eq!(parse_window_size(" 1400X900 "), Some((1400.0, 900.0)));
        for bad in ["", "x", "360", "360x", "axb", "10x10", "nanxinf"] {
            assert_eq!(parse_window_size(bad), None, "{bad:?}");
        }
        assert_eq!(window_size_from(None, None), DEFAULT_WINDOW_SIZE);
        assert_eq!(window_size_from(Some("360x780"), Some("990x600")), (360.0, 780.0));
        assert_eq!(window_size_from(Some("junk"), Some("990x600")), (990.0, 600.0));
    }
}
