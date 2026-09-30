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
use std::io::{Read, Write};

/// The kit SVGs (the error screen's warning icon, the loading spinner) carry
/// the capture-time asset origin (`http://127.0.0.1:8170/ux-images/<card>/
/// assets/*.svg` in page.data.json). This host serves those files itself on a
/// port from MY headless block and rewrites the origin before mounting — the
/// `screen_shot.rs` precedent (capture plumbing, not a renderer change).
const ASSET_PORT: u16 = 8374;

/// Serve `design/stage-b/setup/cards/<card>/assets/*` at
/// `/ux-images/<card>/assets/*` (card-host's AssetServer shape, in-process,
/// loopback only).
fn start_asset_server() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../design/stage-b/setup/cards");
        let Ok(root) = root.canonicalize() else {
            makepad_widgets::log!("[screens_probe] asset root missing");
            return;
        };
        let listener = match std::net::TcpListener::bind(("127.0.0.1", ASSET_PORT)) {
            Ok(l) => l,
            Err(e) => {
                makepad_widgets::log!("[screens_probe] asset bind {ASSET_PORT}: {e}");
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
                            "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
            start_asset_server();
            let splash = self.view.splash(cx, ids!(screen_splash));
            let mut cache = std::mem::take(&mut self.cache);
            // Lower, then point the kit SVGs at THIS host's asset server (the
            // authored origin is the design flow's 8170, held by a process
            // RULES forbid touching).
            let r = palette::lower_screen(&which, &store).map(|dsl| {
                dsl.replace(
                    "http://127.0.0.1:8170/ux-images/",
                    &format!("http://127.0.0.1:{ASSET_PORT}/ux-images/"),
                )
            });
            let r = match r {
                Ok(dsl) => cache.mount(cx, &splash, &dsl),
                Err(e) => Err(e),
            };
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
