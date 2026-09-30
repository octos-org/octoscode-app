//! Entry #30c — the headless capture host for board 3.6 Fleet / 3.7 Tasks.
//!
//! Mounts one board-3 screen card (`OCTOSCODE_SCREEN=fleet|tasks`, default
//! `fleet`) through the SAME path the module uses
//! ([`octoscode_module::screens::fleet::lower`] over
//! [`octoscode_module::screens::fleet::capture_store`] — the Stage-B fixture
//! shape: three peers (`review` closed), the goal line, a running + a settled
//! task with four output lines), with live slots:
//!
//! - Fleet: the roster count ("Fleet · 3 peers"), the goal label, the three
//!   peer rows (name + the one terminal status word the store owns — `Done`
//!   for the closed `review`);
//! - Tasks: the running/settled command labels, their states, and the four
//!   recorded output lines.
//!
//! ```sh
//! MAKEPAD_HIDE_WINDOWS=1 OCTOSCODE_SCREEN=fleet \
//!   harness/headless.sh start target/debug/examples/fleet_probe 8336
//! harness/headless.sh shot 8336 tmp/30c-fleet.png
//! ```
//!
//! Precedents: `screen_shot.rs` (board 2.1–2.3, #29a/#29a2 — the mount gate
//! lives in `draw_walk` because `handle_event` fires before the first draw,
//! and the App's own `script_mod` registers the design/kit vocabulary because
//! a `register_splash_isolate_mod` call never reaches an already-allocated
//! isolate) and `screens_probe.rs` (board 2.7–2.10, #29c/#29d — the in-process
//! asset server for the kits' capture-time SVG origins).
use makepad_widgets::*;
use std::io::{Read, Write};
use std::sync::Mutex;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::FlowUi;
use octoscode_module::l0_host;
use octoscode_module::mount::MountCache;
use octoscode_module::screens::fleet;

/// The kit SVGs (`fleet_goal_icon`, `peer_2_attn`, `icon_run_task`,
/// `icon_done_task`, `icon_tasks`) carry the capture-time asset origin
/// (`http://127.0.0.1:8170/ux-images/<card>/assets/*.svg` in each card's
/// `page.data.json`). This host serves those files itself on a port from MY
/// headless block and rewrites the origin before mounting — the #29a2
/// precedent (capture plumbing, not a renderer change).
const ASSET_PORT: u16 = 8335;

/// Serve `design/stage-b/autonomy/cards/<card>/assets/*` at
/// `/ux-images/<card>/assets/*` (card-host's `--static` shape, in-process,
/// loopback only).
fn start_asset_server() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../design/stage-b/autonomy/cards");
        let Ok(root) = root.canonicalize() else {
            makepad_widgets::log!("[fleet_probe] asset root missing");
            return;
        };
        let listener = match std::net::TcpListener::bind(("127.0.0.1", ASSET_PORT)) {
            Ok(l) => l,
            Err(e) => {
                makepad_widgets::log!("[fleet_probe] asset bind {ASSET_PORT}: {e}");
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
                            "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n",
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

pub use makepad_widgets;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let FleetRoot = #(FleetProbe::register_widget(vm)) {
        width: Fill height: Fill flow: Down
        body := View {
            width: Fill height: Fill flow: Down
            screen_splash := Splash { width: Fill height: Fill }
        }
    }

    mod.gc.set_static(FleetRoot)
    mod.gc.run()

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(406 776)
                body +: { probe := FleetRoot{} }
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
        // The Splash's isolate inherits THIS VM's scope, so the design/kit
        // vocabulary the lowered card names (`DesignSurface`, `KitButton`, …)
        // must be registered directly here — `register_splash_isolate_mod`
        // only reaches isolates allocated AFTER it (the card #21b note,
        // module `lib.rs:916-925`; the #29a2 lesson).
        octoscript_widgets::design::script_mod(vm);
        octoscript_widgets::kit::script_mod(vm);
        self::script_mod(vm)
    }
}

#[derive(Script, ScriptHook, Widget)]
struct FleetProbe {
    #[deref]
    view: View,
    #[rust]
    mounts: MountCache,
    /// One-shot mount flag (draw_walk only — `handle_event` fires before the
    /// first draw, so a gate there never sees the first frame; #29a2).
    #[rust]
    mounted: bool,
    /// Extra redraws so the mounted tree's first paint is deterministic for
    /// the capture (the pl_probe pattern).
    #[rust]
    ticks: usize,
}

impl Widget for FleetProbe {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.ticks < 2 {
            self.ticks += 1;
            self.view.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.mounted {
            self.mounted = true;
            l0_host::register_vocabulary();
            let which = std::env::var("OCTOSCODE_SCREEN").unwrap_or_else(|_| "fleet".to_owned());
            let screen_id = match which.as_str() {
                "tasks" => "autonomy-07",
                _ => "autonomy-06",
            };
            // The Stage-B fixture shape, LIVE from the store — the capture
            // proves the wiring, not the authored copy.
            let store = fleet::capture_store();
            let ui = Mutex::new(FlowUi::default());
            let ctx = Ctx::new(&store, &ui);
            start_asset_server();
            match fleet::lower(screen_id, &ctx) {
                Ok(dsl) => {
                    // The kits' SVG origins point at the design flow's 8170
                    // (another process, 501 for these paths today); this host
                    // serves the same files (above) and rewrites first.
                    let dsl = dsl.replace(
                        "http://127.0.0.1:8170/ux-images/",
                        &format!("http://127.0.0.1:{ASSET_PORT}/ux-images/"),
                    );
                    let splash = self.view.splash(cx, ids!(screen_splash));
                    let mut mounts = std::mem::take(&mut self.mounts);
                    let mounted = mounts.mount(cx, &splash, &dsl);
                    self.mounts = mounts;
                    if let Err(e) = mounted {
                        makepad_widgets::log!("[fleet_probe] mount {screen_id}: {e}");
                    }
                }
                Err(e) => makepad_widgets::log!("[fleet_probe] lower {screen_id}: {e}"),
            }
            self.view.redraw(cx);
        }
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        DrawStep::done()
    }
}
