//! #30a — the headless capture host for the board-3 review screens.
//!
//! Mounts one Stage B screen card (`OCTOSCODE_SCREEN=review_panel|review_run`,
//! default `review_panel`) through the SAME chain the module uses
//! ([`octoscode_module::screens::review::lower_screen`]), with live slots:
//!
//! - `review_panel` folds a 3-file preview through the module's own
//!   [`review::fold_preview`] — totals +62/−5, the authored header's numbers —
//!   so the +/− totals, the per-file rows, the 8 diff lines and the gutter
//!   marks/numbers are the module's wire-shaped output, not authored copy;
//! - `review_run` seeds the accepted `review/start` receipt (2 specialists)
//!   on top, so the status row composes
//!   "Reviewing 3 files · 2 specialists" from wire-derived state.
//!
//! ```sh
//! MAKEPAD_HIDE_WINDOWS=1 OCTOSCODE_SCREEN=review_panel \
//!   cargo run -p octoscode-module --example review_probe -- 8368
//! curl -s 'http://127.0.0.1:8368/snap?all=1'
//! curl -s 'http://127.0.0.1:8368/g?raw=1' -o autonomy-01.png
//! ```
use makepad_widgets::*;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::FlowUi;
use octoscode_module::mount::MountCache;
use std::io::{Read, Write};

/// The kit SVGs (diff/file icons, the status spinner) carry the capture-time
/// asset origin (`http://127.0.0.1:8170/ux-images/<card>/assets/*.svg` in the
/// cards' page.data.json). This host serves those files itself on a port from
/// MY headless block (8360–8369) and rewrites the origin before mounting —
/// the `screens_probe` precedent (capture plumbing, not a renderer change).
const ASSET_PORT: u16 = 8366;

/// Serve `design/stage-b/autonomy/cards/<card>/assets/*` at
/// `/ux-images/<card>/assets/*` (card-host's AssetServer shape, in-process,
/// loopback only).
fn start_asset_server() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../design/stage-b/autonomy/cards");
        let Ok(root) = root.canonicalize() else {
            makepad_widgets::log!("[review_probe] asset root missing");
            return;
        };
        let listener = match std::net::TcpListener::bind(("127.0.0.1", ASSET_PORT)) {
            Ok(l) => l,
            Err(e) => {
                makepad_widgets::log!("[review_probe] asset bind {ASSET_PORT}: {e}");
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
use octoscode_module::screens::review;

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
        // The cards' DSL names the design/kit vocabulary — register it into
        // THIS VM (the screens_probe lesson: "variable DesignSurface not
        // found" without it).
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

    let ProbeRoot = #(ReviewProbe::register_widget(vm)) {
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
pub struct ReviewProbe {
    #[deref]
    view: View,
    #[rust]
    cache: MountCache,
    #[rust]
    mounted: bool,
}

/// A preview wire-shaped like the octos-core `DiffPreview` types
/// (`ui_protocol.rs:2700-2790`), totalling **+62/−5 over 3 files** — the
/// authored header's numbers — so the capture's live slots carry real counted
/// values that still match the Stage B render the card was accepted with.
fn build_preview() -> serde_json::Value {
    fn line(kind: &str, content: &str, old: Option<u32>, new: Option<u32>) -> serde_json::Value {
        let mut o = serde_json::json!({"kind": kind, "content": content});
        if let Some(n) = old {
            o["old_line"] = serde_json::json!(n);
        }
        if let Some(n) = new {
            o["new_line"] = serde_json::json!(n);
        }
        o
    }
    fn added_run(start: u32, n: u32, stem: &str) -> Vec<serde_json::Value> {
        (0..n)
            .map(|i| line("added", &format!("{stem} {start + i}"), None, Some(start + i)))
            .collect()
    }
    let file1_lines: Vec<serde_json::Value> = [
        vec![
            line("context", "use octoscode_module::screens::review;", Some(1), Some(1)),
            line("removed", "use octoscode_module::screens::old_review;", Some(2), None),
            line("added", "use octoscode_module::screens::review as rv;", None, Some(2)),
        ],
        added_run(3, 38, "    let wired = rv::query(ctx, id);"),
    ]
    .concat();
    let file2_lines: Vec<serde_json::Value> = [
        vec![
            line("removed", "action_id,method", Some(1), None),
            line("added", "action_id,protocol_method", None, Some(1)),
        ],
        added_run(2, 13, "models.test_route,profile/llm/test"),
        vec![line("context", "…", Some(15), Some(15))],
    ]
    .concat();
    let file3_lines: Vec<serde_json::Value> = [
        vec![line("removed", "- old quickstart", Some(3), None)],
        added_run(3, 7, "- new quickstart step"),
    ]
    .concat();
    serde_json::json!({
        "status": "ready", "source": "pending_store",
        "preview": {"session_id": "dsflash:main",
            "preview_id": "01920000-0000-7000-8000-0000000000f1",
            "title": "Working tree",
            "files": [
                {"path": "crates/octoscode-module/src/screens/review.rs", "status": "modified",
                 "hunks": [{"header": "@@ -1,7 +1,45", "lines": file1_lines}]},
                {"path": "docs/cards/30a-wiring.csv", "status": "modified",
                 "hunks": [{"header": "@@ -1,5 +1,16", "lines": file2_lines}]},
                {"path": "README.md", "status": "modified",
                 "hunks": [{"header": "@@ -3,2 +3,8", "lines": file3_lines}]}
            ]}})
}

impl Widget for ReviewProbe {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.mounted {
            self.mounted = true;
            let which =
                std::env::var("OCTOSCODE_SCREEN").unwrap_or_else(|_| "review_panel".to_owned());
            let card = match which.as_str() {
                "review_panel" => "autonomy-01",
                "review_run" => "autonomy-02",
                other => {
                    makepad_widgets::log!("[review_probe] unknown OCTOSCODE_SCREEN {other:?}");
                    "autonomy-01"
                }
            };
            let store = std::sync::Arc::new(octoscode_store::Store::new());
            store.set_connection("Live".into(), true);
            // review/start is gated on the method AND the feature
            // (native-review.ts:18-24) — the seed carries both, so the status
            // row renders the receipt, not a typed blocked reason.
            store.set_capabilities(vec![
                "review/start".to_owned(),
                "review.start.v1".to_owned(),
            ]);
            store.domains.session.set_active(Some("dsflash:main".into()));
            let ui = std::sync::Arc::new(std::sync::Mutex::new(FlowUi::default()));

            // Live data: the diff preview folded through the module's own
            // fold (counts, flattened lines, marks, gutter numbers), and the
            // confirmed turn + preview id the wire would have announced.
            review::fold_preview(&build_preview());
            {
                let mut st = review::ui();
                st.last_turn_id = Some("01920000-0000-7000-8000-0000000000a1".to_owned());
                st.preview_id = Some("01920000-0000-7000-8000-0000000000f1".to_owned());
                // review_run: the accepted review/start receipt — the status
                // row composes "Reviewing N files · 2 specialists".
                if which == "review_run" {
                    st.agents = Some(2);
                }
            }

            start_asset_server();
            let ctx = Ctx::new(&store, &ui);
            let splash = self.view.splash(cx, ids!(screen_splash));
            let mut cache = std::mem::take(&mut self.cache);
            // Lower through the module's chain, then point the kit SVGs at
            // THIS host's asset server (the authored origin is the design
            // flow's 8170, held by a process RULES forbid touching).
            let r = review::lower_screen(card, &ctx).map(|dsl| {
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
                makepad_widgets::log!("[review_probe] mount {which}: {e}");
            }
            self.view.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
