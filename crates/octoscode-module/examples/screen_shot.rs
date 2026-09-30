//! #29a — the feature-flagged **temporary mount + headless capture host** for
//! the board-2 screens, until #28e's containers land (the entry: "use a
//! feature-flagged temporary mount until then").
//!
//! ```sh
//! OCTOSCODE_SCREEN=connect cargo build -p octoscode-module --example screen_shot
//! MAKEPAD_HIDE_WINDOWS=1 harness/headless.sh start \
//!   target/debug/examples/screen_shot 8330
//! harness/headless.sh shot 8330 tmp/29a-connect.png
//! harness/headless.sh stop  8330
//! ```
//!
//! `OCTOSCODE_SCREEN` picks the screen (`connect` | `connect_failed` |
//! `onboarding`); `OCTOSCODE_SEED` seeds live state for the capture
//! (`failed` = a classified §5.1 token rejection + the Last-tried stamp;
//! `typed` = a typed token's masked dots). The screen is lowered through the
//! production [`octoscode_module::screens::connect::lower_screen`] — the same
//! `l0::prepare` → `to_makepad_ui` chain the in-app mount will use — and
//! mounted with the module's own [`octoscode_module::mount::MountCache`].
use makepad_widgets::*;

pub use makepad_widgets;

use octoscode_module::l0_host;
use octoscode_module::mount::MountCache;
use octoscode_module::screens::connect::{self, ConnectUi, Screen};

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let ShotRoot = #(ScreenShot::register_widget(vm)) {
        width: Fill height: Fill flow: Down
        body := View {
            width: Fill height: Fill flow: Down
            screen_splash := Splash { width: Fill height: Fill }
        }
    }

    mod.gc.set_static(ShotRoot)
    mod.gc.run()

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(406 776)
                body +: { shot := ShotRoot{} }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl MatchEvent for App {}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }

    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        // The Splash's isolate inherits THIS VM's scope (the app's own
        // `script_mod` allocates it), so the design/kit vocabulary the lowered
        // card names (`DesignSurface`, `KitButton`, …) must be registered
        // directly here — `register_splash_isolate_mod` below only reaches
        // isolates allocated AFTER it (the card #21b note, `lib.rs:916-925`).
        octoscript_widgets::design::script_mod(vm);
        octoscript_widgets::kit::script_mod(vm);
        self::script_mod(vm)
    }
}

#[derive(Script, ScriptHook, Widget)]
struct ScreenShot {
    #[deref]
    view: View,
    #[rust]
    mounts: MountCache,
    /// One-shot mount flag (draw_walk only — `handle_event` fires before the
    /// first draw, so a tick counter there never gates a first mount).
    #[rust]
    mounted: bool,
    /// Extra redraws so the mounted tree's first paint is deterministic for
    /// the capture (the pl_probe pattern).
    #[rust]
    ticks: usize,
}

impl Widget for ScreenShot {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.ticks < 2 {
            self.ticks += 1;
            self.view.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Mount on the first draw: lower the flagged screen with its live
        // state and make the mounted tree the Splash's own view.
        if !self.mounted {
            self.mounted = true;
            l0_host::register_vocabulary();
            let screen = Screen::from_env().unwrap_or(Screen::Connect);
            let mut ui = ConnectUi::default();
            match std::env::var("OCTOSCODE_SEED").as_deref() {
                Ok("failed") => {
                    // A typed value is KEPT on a rejected token (the web's
                    // focus-without-clear, ConnectionPanel.tsx:76-90) — the
                    // dots row + the §5.1 message + the Last-tried stamp.
                    ui.token = "sk-wrong-1".to_owned();
                    ui.note_connect_error("The server refused this token", "9:41 PM");
                }
                Ok("typed") => ui.token = "sk-test-1234".to_owned(),
                _ => {}
            }
            match connect::lower_screen(screen, &ui) {
                Ok(dsl) => {
                    let splash = self.view.splash(cx, ids!(screen_splash));
                    let mut mounts = std::mem::take(&mut self.mounts);
                    let mounted = mounts.mount(cx, &splash, &dsl);
                    self.mounts = mounts;
                    if let Err(e) = mounted {
                        makepad_widgets::log!("[screen_shot] mount {screen:?}: {e}");
                    }
                }
                Err(e) => makepad_widgets::log!("[screen_shot] lower {screen:?}: {e}"),
            }
            self.view.redraw(cx);
        }
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        DrawStep::done()
    }
}
