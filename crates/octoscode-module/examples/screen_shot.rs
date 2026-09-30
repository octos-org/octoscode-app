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
use std::io::{Read, Write};

use octoscode_module::mount::MountCache;
use octoscode_module::screens::connect::{self, ConnectUi, Screen};

/// The kit SVGs (radio rings, password eyes) carry the capture-time asset
/// origin (`http://127.0.0.1:8170/ux-images/<card>/assets/*.svg` in
/// `page.data.json`). This host serves those files itself on a port from MY
/// headless block and rewrites the origin before mounting — the example-side
/// equivalent of card-host's `--static ux-images=<dir>` (the app-side
/// canonical path); it is capture plumbing, not a renderer change.
const ASSET_PORT: u16 = 8334;

/// Serve `design/stage-b/setup/cards/<card>/assets/*` at
/// `/ux-images/<card>/assets/*` (card-host's AssetServer::start_with_static
/// shape, in-process, loopback only).
fn start_asset_server() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../design/stage-b/setup/cards");
        let Ok(root) = root.canonicalize() else {
            makepad_widgets::log!("[screen_shot] asset root missing");
            return;
        };
        let listener = match std::net::TcpListener::bind(("127.0.0.1", ASSET_PORT)) {
            Ok(l) => l,
            Err(e) => {
                makepad_widgets::log!("[screen_shot] asset bind {ASSET_PORT}: {e}");
                return;
            }
        };
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 1024];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let Some(path) = req.split_whitespace().nth(1) else { continue };
                // /ux-images/<card>/assets/<file> — no traversal.
                let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
                let serve = if segs.len() == 4
                    && segs[0] == "ux-images"
                    && segs[2] == "assets"
                    && segs.iter().all(|s| *s != "..")
                {
                    root.join(segs[1]).join("assets").join(segs[3])
                } else {
                    root.join("__missing__")
                };
                match std::fs::read(&serve) {
                    Ok(body) => {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\n                             Content-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(&body);
                    }
                    Err(_) => {
                        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n");
                    }
                }
                let _ = stream.flush();
            }
        });
    })
}

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
            start_asset_server();
            match connect::lower_screen(screen, &ui) {
                Ok(dsl) => {
                    // The kits' eye/radio SVG origins point at the capture-time
                    // asset server; this host serves the same files (above).
                    let dsl = dsl.replace(
                        "http://127.0.0.1:8170/ux-images/",
                        &format!("http://127.0.0.1:{ASSET_PORT}/ux-images/"),
                    );
                    let splash = self.view.splash(cx, ids!(screen_splash));
                    let mut mounts = std::mem::take(&mut self.mounts);
                    let mounted = mounts.mount(cx, &splash, &dsl);
                    self.mounts = mounts;
                    if let Err(e) = mounted {
                        makepad_widgets::log!("[screen_shot] mount {screen:?}: {e}");
                    } else {
                        // The mounted kit inputs are real `TextInput`s; their
                        // authored `text:` copy is a DSL property, not the edit
                        // buffer, so the live value is pushed after the mount —
                        // the same production accessor the composer uses
                        // (`lib.rs:823 text_input(..)`; makepad
                        // `TextInputRef::set_text`). The token/apikey fields
                        // stay empty: their masked dots are the card's own row
                        // (bind it, not paint it).
                        let live = match screen {
                            Screen::Connect | Screen::ConnectFailed => &ui.server,
                            Screen::Onboarding => &ui.profile_name,
                        };
                        self.view
                            .text_input(cx, &[live_id!(beauty_0_0_2_0)])
                            .set_text(cx, live);
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
