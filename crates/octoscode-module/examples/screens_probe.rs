//! Card #29d — the headless capture host for the three Stage C screens.
//!
//! Mounts one Stage B screen card (`OCTOSCODE_SCREEN=palette|error|loading`,
//! or the board-3 `resume|attachments|aside`),
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
/// #30e: env-overridable (`SCREENS_PROBE_ASSET_PORT`), default 8384 — block 8
/// (8380–8389), this lane's block (the docs' 8374 sits in p0-proto's block).
fn asset_port() -> u16 {
    std::env::var("SCREENS_PROBE_ASSET_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8384)
}

/// Serve the Stage B card assets at `/ux-images/<card>/assets/*` (card-host's
/// AssetServer shape, in-process, loopback only). #30e: the theme-wired cards
/// live under THREE stage roots (setup/, conversation/, autonomy/), so the
/// lookup tries each.
fn start_asset_server() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // Both Stage B card trees serve (#30d2/#30e): setup, conversation AND
        // autonomy cards — first existing file wins. The port is THIS lane's
        // block (env-overridable, default 8384).
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/stage-b");
        let roots: Vec<std::path::PathBuf> = ["setup/cards", "conversation/cards", "autonomy/cards"]
            .iter()
            .filter_map(|r| base.join(r).canonicalize().ok())
            .collect();
        if roots.is_empty() {
            makepad_widgets::log!("[screens_probe] asset roots missing");
            return;
        }
        let listener = match std::net::TcpListener::bind(("127.0.0.1", asset_port())) {
            Ok(l) => l,
            Err(e) => {
                let port = asset_port();
                makepad_widgets::log!("[screens_probe] asset bind {port}: {e}");
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
                // /ux-images/<card>/assets/<file> — no traversal; the card dir
                // may sit under any stage root (setup/conversation/autonomy).
                let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
                let serve = if segs.len() == 4
                    && segs[0] == "ux-images"
                    && segs[2] == "assets"
                    && segs.iter().all(|s| *s != "..")
                {
                    roots
                        .iter()
                        .map(|r| r.join(segs[1]).join("assets").join(segs[3]))
                        .find(|f| f.exists())
                } else {
                    None
                };
                match serve.as_deref().map(std::fs::read) {
                    Some(Ok(body)) => {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(&body);
                    }
                    _ => {
                        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n");
                    }
                }
                let _ = stream.flush();
            }
        });
    })
}
use octoscode_module::screens::palette;
use octoscode_module::screens::sessions;
use octoscode_module::screens::models;

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

    // #31d — the capture host shows the theme as the APP draws it: assign the
    // shell theme roles (loads the persisted preference) before this class
    // body evaluates, exactly like lib.rs's script_mod.
    #(octoscode_module::screens::theme::eval_roles(vm))

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
            // #30e — seed the theme preference the same way lib.rs's mount
            // arm does (OCTOSCODE_THEME=system|dark|light; unset = system).
            if let Ok(pref) = std::env::var("OCTOSCODE_THEME") {
                octoscode_module::screens::theme::set_preference(&pref);
            }
            // The item-count variant knob (#30d2: re-capture 0/1/3 where
            // relevant — attachments 0/1/2 tiles, resume N rows).
            let n: usize = std::env::var("OCTOSCODE_N")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2);
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
                // #30d — board 3.8: the store's live rows + a staged confirm
                // (live data, not the authored copy).
                "resume" => {
                    let rows: Vec<octoscode_store::Session> = (1..=n)
                        .map(|i| octoscode_store::Session {
                            id: format!("dsflash:live-{i}"),
                            title: Some(format!("Live row {i}")),
                            message_count: 6 + i,
                            updated_at: Some(format!("{}m ago", i * 5)),
                            last_prompt: None,
                            active_turn: false,
                        })
                        .collect();
                    store.set_sessions(rows);
                    let ctx = Ctx::new(&store, &ui);
                    sessions::resolve("resume.stage", 0, &ctx);
                }
                // #30d — board 3.9: one live draft attachment (the count slot).
                "attachments" => {
                    // Real decoded previews (never a grey 404 box): the card
                    // assets dir ships two distinct real PNGs; the 8170 origin
                    // in the seeded URLs is rewritten to THIS host's port by
                    // the capture plumbing below.
                    const THUMB_A: &str = "http://127.0.0.1:8170/ux-images/autonomy-09/assets/att_1_thumb-d08ed1b523f0.png";
                    const THUMB_B: &str = "http://127.0.0.1:8170/ux-images/autonomy-09/assets/att_1_thumb-f187c53eac2c.png";
                    match n {
                        0 => {}
                        1 => sessions::seed_attachments_live(vec![(
                            "screenshot.png".to_owned(),
                            1_258_291,
                            THUMB_A.to_owned(),
                        )]),
                        _ => sessions::seed_attachments_live(vec![
                            ("screenshot.png".to_owned(), 1_258_291, THUMB_A.to_owned()),
                            ("diagram.png".to_owned(), 2_621_440, THUMB_B.to_owned()),
                        ]),
                    }
                }
                // #30d — board 3.10: the answered aside (question + answer).
                "aside" => {
                    sessions::seed_aside(
                        "What does steer_dropped mean?",
                        "It's a metric that increments when messages are dropped from the steer queue due to a reconnect or protocol error. It helps track message loss.",
                    );
                }
                other => ::log::warn!("screens_probe: unknown OCTOSCODE_SCREEN {other:?}"),
            }
            start_asset_server();
            let splash = self.view.splash(cx, ids!(screen_splash));
            let mut cache = std::mem::take(&mut self.cache);
            // Lower, then point the kit SVGs at THIS host's asset server (the
            // authored origin is the design flow's 8170, held by a process
            // RULES forbid touching). #30e: the theme-wired card names lower
            // through screens::theme (dark Stage B card vs its light twin by
            // the CURRENT resolved preference); #30d's session screens lower
            // through sessions::.
            let lowered = match which.as_str() {
                "resume" | "attachments" | "aside" => sessions::lower_screen(&which, &store),
                // #32d item 2 evidence: the question-card component card
                // through the SAME chain `components::lower` uses
                // (l0::prepare -> design::to_makepad_ui) — an IIFE because
                // handle_event does not return Result.
                "question" => (|| {
                    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../../design/components/question-card");
                    let card = std::fs::read_to_string(dir.join("page.card"))
                        .map_err(|e| e.to_string())?;
                    let data: serde_json::Value = serde_json::from_str(
                        &std::fs::read_to_string(dir.join("page.data.json"))
                            .map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    let prepared = octoscript_makepad::l0::prepare(
                        &card,
                        &data,
                        &dir.join("kit"),
                    )
                    .map_err(|e| e.to_string())?;
                    octoscript_makepad::design::to_makepad_ui(&prepared.tree)
                        .map_err(|e| e.to_string())
                })(),
                // #32d item 6 evidence: the models screen through its
                // production lower (the outer card sizes to its live rows).
                "models" => models::lower("setup-07", &Ctx::new(&store, &ui)),
                _ if octoscode_module::screens::theme::card_for(&which).is_some() => {
                    octoscode_module::screens::theme::lower(&which, &store)
                }
                _ => palette::lower_screen(&which, &store),
            };
            let r = lowered.map(|dsl| {
                dsl.replace(
                    "http://127.0.0.1:8170/ux-images/",
                    &format!("http://127.0.0.1:{}/ux-images/", asset_port()),
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
