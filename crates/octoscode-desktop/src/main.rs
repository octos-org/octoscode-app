//! `octoscode`: the standalone OctosCode desktop app (A33, decision D10f).
//!
//! One window, titled "OctosCode", holding the octoscode module's root and
//! nothing else (see the crate doc in lib.rs for what the OctoSense shell
//! provides the module and what this host provides instead).
//!
//!   octoscode                     # a normal window
//!   octoscode --remote <port>     # + the makepad instrument bridge (D10c token), only when asked
//!
//! The server, token and Session come from the module's own Connect card, or
//! from OCTOS_BASE_URL / OCTOS_BEARER / OCTOS_PROFILE_ID like every host.
use makepad_widgets::*;
use octoscode_desktop::{create_instance, window_size, OctoscodeHost, OCTOSCODE_MODULE};

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "OctosCode"
                window.inner_size: vec2(1280, 800)
                body +: {
                    module_host := mod.widgets.OctoscodeHost{}
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    sized: bool,
}

impl App {
    fn mount(&mut self, cx: &mut Cx) {
        let host = self.ui.widget(cx, ids!(module_host));
        if host.borrow::<OctoscodeHost>().is_some_and(|h| h.instance().is_some()) {
            return;
        }
        let (w, h) = window_size();
        match create_instance(cx, &OCTOSCODE_MODULE, dvec2(w, h)) {
            Ok(instance) => {
                if let Some(mut host) = host.borrow_mut::<OctoscodeHost>() {
                    host.set_instance(cx, instance);
                }
            }
            Err(e) => log!("[octoscode-desktop] the module did not mount: {e}"),
        }
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.mount(cx);
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::script_mod(vm);
        octoscode_desktop::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        // OCTOSENSE_WINDOW_SIZE / OCTOSCODE_WINDOW_SIZE (e.g. the phone's
        // 360x780) size the window once it exists.
        if let Event::Draw(_) = event {
            if !self.sized {
                self.sized = true;
                let (w, h) = window_size();
                if (w, h) != octoscode_desktop::DEFAULT_WINDOW_SIZE {
                    self.ui.window(cx, ids!(main_window)).resize(cx, dvec2(w, h));
                }
            }
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
