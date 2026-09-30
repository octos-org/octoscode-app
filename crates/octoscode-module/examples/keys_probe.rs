//! Entry #31e — the keyboard evidence host: the REAL shell
//! (`mod.widgets.OctoscodeView`) against a tiny in-process protocol server
//! (the f30d ReplayServer shape, on MY headless block), driven through the
//! app's own `/k` key instrument.
//!
//! Every keyboard binding gets a live receipt:
//! * palette: Cmd/Ctrl+K toggle, "/" open, ↓/↑ move (the highlight follows
//!   the LIVE selection), Esc close — `/snap` + `/g` before/after;
//! * composer: `/t` + Enter → the server SEES `turn/start` (ComposerInput
//!   .tsx:237-243's bare-Enter submit over the production path);
//! * Esc with a live turn → the server SEES `turn/interrupt` (:245-252);
//! * Y / S / N on a pushed `approval/requested` (the r5-turn fixture's
//!   frame) → the server SEES `approval/respond` with the decision (+ the
//!   web's `approval_scope`);
//! * Alt+A → the module's log line names the parity shortcut (registry.ts:614).
//!
//! ```sh
//! MAKEPAD_HIDE_WINDOWS=1 OCTOS_BASE_URL=http://127.0.0.1:8375 \
//!   ./target/debug/examples/keys_probe --remote 8374
//! curl -s 'http://127.0.0.1:8374/k?c=cmd+k&wait=1'   # (mods are flags: see below)
//! ```
use makepad_widgets::*;

// The AppModule trait (`register`) — the module's own import path.
use makepad_app_module::AppModule;

pub use makepad_widgets;

use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// The connection's sink, shared with the scheduled-push task (a concrete
/// alias — the closure's parameter type must be inferable).
type WsTx = Arc<
    tokio::sync::Mutex<
        futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
            Message,
        >,
    >,
>;

app_main!(App);

// ---------------- the in-process protocol server (my block: 8375) ------------

const SERVER_PORT: u16 = 8375;

fn start_server() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("server runtime");
            rt.block_on(async move {
                let listener =
                    TcpListener::bind(("127.0.0.1", SERVER_PORT)).await.expect("bind");
                println!("[keys-probe-server] listening on {SERVER_PORT}");
                loop {
                    let Ok((stream, _)) = listener.accept().await else { continue };
                    tokio::spawn(async move {
                        let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
                            return;
                        };
                        let (tx, mut rx) = ws.split();
                        let tx: WsTx = Arc::new(tokio::sync::Mutex::new(tx));
                        let send = |tx: WsTx, v: serde_json::Value| {
                            async move {
                                let _ = tx
                                    .lock()
                                    .await
                                    .send(Message::Text(v.to_string().into()))
                                    .await;
                            }
                        };
                        // Scheduled pushes: two approval cards (t+2.5s, t+6s),
                        // so the Y/S/N evidence has REAL pending state.
                        {
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                // Three cards — one per keyboard decision
                                // (Y approve/request, S approve/session, N deny).
                                for (ms, id) in [
                                    (2500u64, "a1-approve-me"),
                                    (6000, "a2-approve-session"),
                                    (9500, "a3-deny-me"),
                                ] {
                                    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                                    let frame = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "method": "approval/requested",
                                        "params": {
                                            "approval_id": id,
                                            "approval_kind": "command",
                                            "body": "printf keys-probe",
                                            "risk": "low",
                                            "session_id": "octoscode:main",
                                            "title": "Keyboard parity fixture",
                                            "tool_name": "shell",
                                            "turn_id": "t-live",
                                            "typed_details": {"command": "printf keys-probe"}
                                        }
                                    });
                                    println!("[keys-probe-server] -> approval/requested ({id})");
                                    send(tx.clone(), frame).await;
                                }
                            });
                        }
                        while let Some(Ok(msg)) = rx.next().await {
                            let Message::Text(text) = msg else { continue };
                            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                                continue;
                            };
                            let method = v["method"].as_str().unwrap_or("").to_owned();
                            let id = v["id"].as_str().unwrap_or("").to_owned();
                            // THE LINE RECEIPT: every outbound wire action the
                            // keyboard performs shows up in the app log.
                            println!("[keys-probe-server] <- {method} {}", {
                                let p = &v["params"];
                                if p.is_object() { p.to_string() } else { String::new() }
                            });
                            match method.as_str() {
                                "session/open" => {
                                    // Echo the client's requested session id —
                                    // session-scoped handlers (approval) drop
                                    // frames for any OTHER session (the first
                                    // drive's silent-approval bug).
                                    let session = v["params"]["session_id"]
                                        .as_str()
                                        .unwrap_or("octoscode:main")
                                        .to_owned();
                                    let frame = serde_json::json!({
                                        "jsonrpc": "2.0", "id": id,
                                        "result": {"opened": {
                                            "session_id": session,
                                            "active_profile_id": "keys",
                                            "cursor": {"stream": "keys:main", "seq": 1},
                                            "capabilities": {
                                                "version": {"protocol": "octos-ui/v1alpha1",
                                                            "schema_version": 1, "jsonrpc": "2.0"},
                                                "capabilities_schema_version": 1,
                                                "supported_methods": [
                                                    "session/open", "session/list", "turn/start",
                                                    "turn/interrupt", "approval/respond"],
                                                "supported_notifications": [
                                                    "projection/envelope", "turn/started",
                                                    "approval/requested"],
                                                "supported_features": [
                                                    "projection.envelope.v2",
                                                    "state.session_hydrate.v1",
                                                    "session/btw"]
                                            }
                                        }}
                                    });
                                    send(tx.clone(), frame).await;
                                }
                                "session/list" => {
                                    let frame = serde_json::json!({
                                        "jsonrpc": "2.0", "id": id,
                                        "result": {"sessions": [{
                                            "id": "keys:main", "title": "Keyboard probe",
                                            "message_count": 1, "active_turn": false
                                        }]}
                                    });
                                    send(tx.clone(), frame).await;
                                }
                                "turn/start" => {
                                    // A live turn for the Esc-interrupt proof.
                                    let frame = serde_json::json!({
                                        "jsonrpc": "2.0", "id": id,
                                        "result": {"turn_id": "t-live", "status": "started"}
                                    });
                                    send(tx.clone(), frame).await;
                                    let note = serde_json::json!({
                                        "jsonrpc": "2.0", "method": "turn/started",
                                        "params": {"turn_id": "t-live", "session_id": "keys:main"}
                                    });
                                    send(tx.clone(), note).await;
                                }
                                "turn/interrupt" | "approval/respond" => {
                                    // The r5-turn recording's reply grammar.
                                    let frame = serde_json::json!({
                                        "jsonrpc": "2.0", "id": id,
                                        "result": {"accepted": true, "runtime_resumed": true,
                                                   "status": "accepted"}
                                    });
                                    send(tx.clone(), frame).await;
                                    // The live server broadcasts the decision, so
                                    // the store settles the row and the NEXT
                                    // keyboard decision walks to the next card.
                                    if method == "approval/respond" {
                                        let note = serde_json::json!({
                                            "jsonrpc": "2.0", "method": "approval/decided",
                                            "params": {
                                                "approval_id": v["params"]["approval_id"],
                                                "decision": v["params"]["decision"],
                                                "session_id": "octoscode:main"
                                            }
                                        });
                                        println!("[keys-probe-server] -> approval/decided");
                                        send(tx.clone(), note).await;
                                    }
                                }
                                _ => {
                                    let frame = serde_json::json!({
                                        "jsonrpc": "2.0", "id": id, "result": {}
                                    });
                                    send(tx.clone(), frame).await;
                                }
                            }
                        }
                    });
                }
            });
        });
    });
}

// ------------------------------ the host app ---------------------------------

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl MatchEvent for App {}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        // ORDER MATTERS: the widgets vocabulary FIRST (it defines
        // `mod.widgets.RectView` and the `theme.*` globals the shell's default
        // names), THEN the module's register (its script_mod references both;
        // it also registers the design/kit vocabulary itself), then this
        // host's own script. The first run panicked in
        // script/traits.rs:585 with the module registered first.
        crate::makepad_widgets::script_mod(vm);
        octoscode_module::OCTOSCODE_MODULE.register(vm);
        self::script_mod(vm)
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        // The in-process protocol server: started on the first event (ONCE
        // inside), so the shell's connect finds it and the approval pushes
        // schedule from here. Without this call the server never runs (the
        // empty-receipt bug the first drive exposed).
        start_server();
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(1280 800)
                body +: {
                    // The REAL shell — the same widget the desktop host mounts.
                    shell := OctoscodeView {}
                }
            }
        }
    }
}
