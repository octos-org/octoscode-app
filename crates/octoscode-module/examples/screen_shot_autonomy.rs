//! #30b4 — the feature-flagged **temporary mount + headless capture host**
//! for the board-3 autonomy screens (the #29a `screen_shot` shape; the entry:
//! "re-capture through the same path the app uses").
//!
//! ```sh
//! OCTOSCODE_SCREEN=loops OCTOSCODE_STORE=1 \
//!   cargo build -p octoscode-module --example screen_shot_autonomy
//! MAKEPAD_HIDE_WINDOWS=1 harness/headless.sh start \
//!   target/debug/examples/screen_shot_autonomy 8385
//! harness/headless.sh shot 8385 tmp/30b4-loops-1.png
//! harness/headless.sh stop 8385
//! ```
//!
//! `OCTOSCODE_SCREEN` picks the wired card (`goal` | `loops` | `monitors`);
//! `OCTOSCODE_STORE` seeds the screen cache with 0/1/3 items (the recorded
//! r1-autonomy shapes). The screen is lowered through the PRODUCTION
//! [`octoscode_module::screens::autonomy::lower_screen`] — the same
//! `l0::prepare` → row/sizing/empty-state rules → `to_makepad_ui` chain the
//! in-app mount will use — and mounted with the module's own
//! [`octoscode_module::mount::MountCache`]. No capture-side tree edits: what
//! the PNG shows is what `lower_tree` produced.
use makepad_widgets::*;

pub use makepad_widgets;

use octoscode_module::l0_host;
use std::io::{Read, Write};

use octoscode_module::mount::MountCache;
use octoscode_module::screens::autonomy::{self, AutonomyState, Screen3};

/// The kit SVGs (loop dots/pause/play/trash, monitor pause/trash) carry the
/// capture-time asset origin (`http://127.0.0.1:8170/ux-images/<card>/assets/*`
/// in `page.data.json`). This host serves those files itself on a port from
/// MY headless block and rewrites the origin before mounting (the example-side
/// equivalent of card-host's `--static ux-images=<dir>`; capture plumbing,
/// not a renderer change).
const ASSET_PORT: u16 = 8394;

/// Serve `design/stage-b/autonomy/cards/<card>/assets/*` at
/// `/ux-images/<card>/assets/*` (in-process, loopback only).
fn start_asset_server() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../design/stage-b/autonomy/cards");
        let Ok(root) = root.canonicalize() else {
            makepad_widgets::log!("[screen_shot_autonomy] asset root missing");
            return;
        };
        let listener = match std::net::TcpListener::bind(("127.0.0.1", ASSET_PORT)) {
            Ok(l) => l,
            Err(e) => {
                makepad_widgets::log!("[screen_shot_autonomy] asset bind {ASSET_PORT}: {e}");
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
        // The Splash's isolate inherits THIS VM's scope, so the design/kit
        // vocabulary the lowered card names must be registered here (the
        // card #21b note — a later register_splash_isolate_mod never reaches
        // an already-allocated isolate).
        octoscript_widgets::design::script_mod(vm);
        octoscript_widgets::kit::script_mod(vm);
        self::script_mod(vm)
    }
}

/// The screen cache seed: the recorded r1-autonomy shapes at 0/1/3 items.
fn seed() -> (Screen3, AutonomyState) {
    let screen = Screen3::from_env().unwrap_or(Screen3::Loops);
    let n = std::env::var("OCTOSCODE_STORE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1);
    let loop_row = serde_json::json!({
        "loop_id": "loop_01", "prompt": "r1 replay probe",
        "mode": "fixed_interval", "interval_seconds": 3600, "status": "active",
    });
    let monitor = |k: usize| {
        serde_json::json!({
            "monitor_id": format!("monitor_0{k}"),
            "name": format!("probe {k}"),
            "argv": ["./scripts/watch.sh"],
            "status": if k == 3 { "paused" } else { "active" },
            "interval_seconds": 3600,
        })
    };
    let goal = serde_json::json!({
        "goal_id": "goal_01", "objective": "r1 replay probe",
        "status": "active", "tokens_used": 0, "token_budget": 100000000u64,
        "time_used_seconds": 0,
    });
    let state = match screen {
        Screen3::Goal => AutonomyState {
            goal: Some(goal),
            ..Default::default()
        },
        Screen3::Loops => AutonomyState {
            loops: (0..n).map(|_| loop_row.clone()).collect(),
            ..Default::default()
        },
        Screen3::Monitors => AutonomyState {
            monitors: (1..=n).map(monitor).collect(),
            ..Default::default()
        },
    };
    (screen, state)
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
        // Mount on the first draw: lower the flagged screen with its seeded
        // state through the PRODUCTION lowering and make the mounted tree the
        // Splash's own view.
        if !self.mounted {
            self.mounted = true;
            l0_host::register_vocabulary();
            let (screen, state) = seed();
            start_asset_server();
            match autonomy::lower_screen(screen, &state) {
                Ok(dsl) => {
                    // The kit SVG origins point at the capture-time asset
                    // server; this host serves the same files (above).
                    let dsl = dsl.replace(
                        "http://127.0.0.1:8170/ux-images/",
                        &format!("http://127.0.0.1:{ASSET_PORT}/ux-images/"),
                    );
                    let splash = self.view.splash(cx, ids!(screen_splash));
                    let mut mounts = std::mem::take(&mut self.mounts);
                    let mounted = mounts.mount(cx, &splash, &dsl);
                    self.mounts = mounts;
                    if let Err(e) = mounted {
                        makepad_widgets::log!("[screen_shot_autonomy] mount {screen:?}: {e}");
                    }
                }
                Err(e) => makepad_widgets::log!("[screen_shot_autonomy] lower {screen:?}: {e}"),
            }
            self.view.redraw(cx);
        }
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        DrawStep::done()
    }
}
