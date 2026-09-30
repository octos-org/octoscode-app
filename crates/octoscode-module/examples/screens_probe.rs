//! Card #29d — the headless capture host for the three Stage C screens.
//!
//! Mounts one Stage B screen card (`OCTOSCODE_SCREEN=palette|error|loading`,
//! default `palette`) through the SAME path the module uses
//! ([`octoscode_module::screens::palette::mount_screen`]), with live slots:
//!
//! - `error` seeds a sample diagnostic (secrets included) through
//!   `report_error`, so the capture proves the redaction boundary renders;
//! - `loading` runs `connection.retry` three times, so the banner shows the
//!   module's own attempt count ("attempt 3"), not the authored copy;
//! - `palette` sets the draft to "/mo" and runs `palette.query.set`, so the
//!   query box shows the live query instead of the authored "/ mo".
//!
//! ```sh
//! MAKEPAD_HIDE_WINDOWS=1 OCTOSCODE_SCREEN=palette \
//!   cargo run -p octoscode-module --example screens_probe -- 8370
//! curl -s 'http://127.0.0.1:8370/snap?all=1'
//! curl -s 'http://127.0.0.1:8370/g?raw=1' -o palette.png
//! ```
use makepad_widgets::*;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::FlowUi;
use octoscode_module::mount::MountCache;
use octoscode_module::screens::palette;

pub use makepad_widgets;

app_main!(App);

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl MatchEvent for App {}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        // The screen cards' DSL names the design/kit vocabulary (DesignSurface,
        // kit surfaces, ...) — register it into THIS VM, the one the probe's
        // Splash was minted in (the lib.rs "register directly" precedent; the
        // first probe run failed with "variable DesignSurface not found").
        octoscript_widgets::design::script_mod(vm);
        octoscript_widgets::kit::script_mod(vm);
        self::script_mod(vm)
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let ProbeRoot = #(ScreensProbe::register_widget(vm)) {
        width: Fill height: Fill flow: Down
        screen_splash := Splash { width: Fill height: Fill }
    }

    mod.gc.set_static(ProbeRoot)
    mod.gc.run()

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(420 780)
                body +: { probe := ProbeRoot{} }
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct ScreensProbe {
    #[deref]
    view: View,
    #[rust]
    cache: MountCache,
    #[rust]
    mounted: bool,
}


impl Widget for ScreensProbe {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.mounted {
            self.mounted = true;
            let which =
                std::env::var("OCTOSCODE_SCREEN").unwrap_or_else(|_| "palette".to_owned());
            let store = std::sync::Arc::new(octoscode_store::Store::new());
            store.set_connection("Live".into(), true);
            store.set_capabilities(vec!["state.session_hydrate.v1".to_owned()]);
            let ui = std::sync::Arc::new(std::sync::Mutex::new(FlowUi::default()));
            match which.as_str() {
                // Live data: the crash report the host would hand over — with
                // secrets, so the rendered copy proves the redaction boundary.
                "error" => palette::report_error(
                    "Render panicked: bad connection state\n\
                     GET https://octos.example/ws?token=abc123&x=1\n\
                     Authorization: Bearer sk-test-9f8e7d6c"
                        .to_owned(),
                ),
                // Live data: three retries -> the banner counts the module's
                // own attempts ("attempt 3", not the authored 2).
                "loading" => {
                    let ctx = Ctx::new(&store, &ui);
                    for _ in 0..3 {
                        palette::resolve("connection.retry", 0, &ctx);
                    }
                }
                // Live data: the palette query follows the composer draft.
                "palette" => {
                    ui.lock().unwrap().set_draft_inner("/mo");
                    let ctx = Ctx::new(&store, &ui);
                    palette::resolve("palette.query.set", 0, &ctx);
                }
                other => ::log::warn!("screens_probe: unknown OCTOSCODE_SCREEN {other:?}"),
            }
            let splash = self.view.splash(cx, ids!(screen_splash));
            let mut cache = std::mem::take(&mut self.cache);
            let r = palette::mount_screen(&mut cache, cx, splash, &which, &store);
            self.cache = cache;
            if let Err(e) = r {
                ::log::warn!("screens_probe: mount {which}: {e}");
            }
            self.view.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
