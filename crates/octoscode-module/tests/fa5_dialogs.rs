//! A5 — the dialog host's production paths, against RECORDED traffic.
//!
//! Each test drives the same functions the app's click routes run
//! (`screens::models::perform` / `refresh_context`, `screens::autonomy::apply`,
//! the palette's suggestion table, the command layer's queue, the dispatcher's
//! fall-through predicate) through a fake WS server that answers with the
//! committed recordings' own replies
//! (`crates/octoscode-client/tests/fixtures/{r1-autonomy,r3-session}-a6ea8505.jsonl`),
//! and asserts both the request the server saw and what the dialog folds.

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::{autonomy, dialog, models, palette};
use octoscode_store::Store;

// ------------------------------------------------------------- recordings

struct Frame {
    dir: String,
    method: String,
    body: Value,
}

fn fixture(name: &str) -> Vec<Frame> {
    let path = format!(
        "{}/../octoscode-client/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(Value::Null),
            }
        })
        .collect()
}

/// Every inbound body of `method` (r3 records replies as `res:<method>`).
fn recorded(frames: &[Frame], method: &str) -> Vec<Value> {
    frames
        .iter()
        .filter(|f| f.dir == "in" && (f.method == method || f.method == format!("res:{method}")))
        .map(|f| f.body.clone())
        .collect()
}

/// r1-autonomy's recorded `session/open` result (capabilities: 34 features,
/// 106 methods), the reply the replay server sends as `{"opened": …}`.
fn recorded_open() -> Value {
    recorded(&fixture("r1-autonomy-a6ea8505.jsonl"), "session/open")
        .into_iter()
        .find(|b| b.get("active_profile_id").is_some())
        .expect("r1 recorded the open result")
}

/// Re-point a recorded body at the session the test opened (the replay
/// server's own rewrite, `examples/replay_serve.rs` `rewrite_session`).
fn repoint(v: &Value, from: &str, to: &str) -> Value {
    serde_json::from_str(&v.to_string().replace(from, to)).expect("json")
}

// ------------------------------------------------------------- the server

/// A fake WS server: canned replies per method, `{}` otherwise, and a log of
/// every (method, params) it received.
struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
    async fn start(canned: Vec<(String, Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (tx, mut rx) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                log.lock().unwrap().push((method.clone(), v["params"].clone()));
                let body = canned
                    .iter()
                    .find(|(m, _)| *m == method)
                    .map(|(_, b)| b.clone())
                    .unwrap_or(json!({}));
                let frame = json!({"jsonrpc": "2.0", "id": id, "result": body});
                let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { base_url: format!("http://{addr}"), seen }
    }

    fn sent(&self, method: &str) -> Option<Value> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .find(|(m, _)| m == method)
            .map(|(_, p)| p.clone())
    }
}

/// Connect and DRAIN the transport events into the conversation, the way the
/// app's event loop does (`lib.rs` start: `drv.on_event(evt)`) — the open
/// reply binds the autonomy authority there.
async fn connect(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    let conv = Arc::new(conv);
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(None).await.expect("session/open");
    // The open reply's event folds on the drain task; give it a beat.
    for _ in 0..50 {
        if conv.store.domains.autonomy.bound_session().is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    conv
}

/// The autonomy cache is a process static: one test at a time touches it.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

// ------------------------------------------------------------------ tests

/// Context dialog → "Heuristic" (`context.mode.heuristic`): the recorded
/// request shape reaches the wire (`{"mode":"heuristic","session_id":…}`,
/// r3-session), and the dialog then selects the SERVER-CONFIRMED mode — the
/// web's `setMode` keeps `confirmed`, never the requested value.
#[tokio::test]
async fn the_compaction_mode_click_sends_the_recorded_request_and_selects_the_reply() {
    let r3 = fixture("r3-session-a6ea8505.jsonl");
    let reply = recorded(&r3, "session/compact/mode/set").remove(0);
    assert_eq!(reply["mode"], "heuristic", "the recording's confirmed mode");
    let server = Server::start(vec![
        // The recorded open reply advertises the methods the dialog gates on.
        ("session/open".into(), json!({"opened": recorded_open()})),
        (
            "session/compact/mode/set".into(),
            repoint(&reply, "dsflash:api:main", "dsflash:main"),
        ),
    ])
    .await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    store.domains.session.set_active(Some(conv.session_id()));

    models::perform(&conv, "context.mode.heuristic", &store).await.expect("mode set");
    let sent = server.sent("session/compact/mode/set").expect("the request went out");
    assert_eq!(sent["mode"], "heuristic");
    assert_eq!(sent["session_id"], conv.session_id().as_str());
    assert_eq!(models::compact_mode(&conv.session_id()).as_deref(), Some("heuristic"));

    // The dialog renders the confirmed half selected (ink + weight).
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    let (tree, _) = dialog::live_tree(dialog::Dialog::Context, &ctx).expect("context lowers");
    let heur = dialog::find(&tree, "t_heur").expect("the heuristic half");
    assert_eq!(heur.attrs.weight, Some(600), "the confirmed mode is selected");
    assert!(dialog::find(&tree, "seg_sel").is_some(), "the selected half has its fill");
}

/// Context dialog open → the AUTHORITATIVE `session/status/read` (the
/// recorded r3 reply) folds the lifecycle snapshot; a delayed read never
/// erases a NEWER lifecycle event (`ContextDialog.tsx` refresh).
#[tokio::test]
async fn the_authoritative_status_read_folds_and_a_stale_read_is_refused() {
    let r3 = fixture("r3-session-a6ea8505.jsonl");
    let status = repoint(
        &recorded(&r3, "session/status/read").remove(0),
        "dsflash:api:main",
        "dsflash:main",
    );
    let server = Server::start(vec![("session/status/read".into(), status.clone())]).await;
    let conv = connect(&server).await;
    let store = conv.store.clone();
    store.domains.session.set_active(Some(conv.session_id()));
    store.domains.config.set_supported_methods(vec!["session/status/read".into()]);

    let applied = models::refresh_context(&conv, &store).await.expect("status read");
    assert!(applied, "the authoritative read folds");
    let life = store.domains.session.context(&conv.session_id()).expect("a snapshot");
    assert_eq!(life.state["recovery_state"], "exact", "the recorded state");
    assert_eq!(
        server.sent("session/status/read").expect("sent")["session_id"],
        conv.session_id().as_str()
    );

    // The race: a lifecycle notification (generation 2) lands while a read
    // that saw generation 1 is in flight → the read is refused.
    let session = conv.session_id();
    let before = models::context_revision();
    models::note_context_event();
    store.domains.session.set_context(
        &session,
        octoscode_store::domains::session::ContextLifecycle {
            kind: "compaction_completed".into(),
            state: json!({"session_id": session, "generation": 2, "token_estimate": 9}),
            detail: None,
        },
    );
    let mut stale = status.clone();
    stale["context_state"]["generation"] = json!(1);
    assert!(!models::fold_status_read(&stale, &session, before, &store), "stale read refused");
    assert_eq!(store.domains.session.context(&session).unwrap().state["generation"], 2);
    // A read NEWER than the event still applies.
    let mut newer = status.clone();
    newer["context_state"]["generation"] = json!(3);
    assert!(models::fold_status_read(&newer, &session, before, &store));
    // A read for another session never folds.
    assert!(!models::fold_status_read(&newer, "other:main", models::context_revision(), &store));
}

/// Loops dialog: the row controls send the recorded request shapes and fold
/// the recorded replies (r1-autonomy's loop at each step) the way the web
/// store does — an ID upsert for pause/resume, a filter for delete.
#[tokio::test]
async fn loop_row_controls_send_and_fold_the_recorded_replies() {
    let _s = serial();
    autonomy::reset_state();
    let r1 = fixture("r1-autonomy-a6ea8505.jsonl");
    let updates = recorded(&r1, "loop/updated");
    assert!(updates.len() >= 4, "r1 recorded create/pause/resume/delete");
    // The recorded open reply binds the autonomy authority (the epoch fence
    // drops every result of an unbound session — #P4e1b row 4).
    let opened = recorded(&r1, "session/open")
        .into_iter()
        .find(|b| b.get("active_profile_id").is_some())
        .expect("r1 recorded the open result");
    let server = Server::start(vec![
        ("session/open".into(), json!({"opened": opened})),
        ("loop/list".into(), json!({"loops": [updates[0]["loop"].clone()]})),
        ("monitor/list".into(), json!({"monitors": []})),
        ("loop/pause".into(), updates[1].clone()),
        ("loop/delete".into(), updates[3].clone()),
    ])
    .await;
    let conv = connect(&server).await;

    autonomy::apply(autonomy::Effect::RefreshLists, &conv).await.expect("lists");
    let id = autonomy::state_snapshot().loops[0]["loop_id"].as_str().unwrap().to_owned();
    assert_eq!(autonomy::state_snapshot().loops[0]["status"], "active");

    autonomy::apply(autonomy::Effect::LoopPause(id.clone()), &conv).await.expect("pause");
    assert_eq!(server.sent("loop/pause").unwrap(), json!({"loop_id": id}));
    assert_eq!(
        autonomy::state_snapshot().loops[0]["status"],
        "paused",
        "the recorded paused loop was upserted"
    );

    autonomy::apply(autonomy::Effect::LoopDelete(id.clone()), &conv).await.expect("delete");
    assert_eq!(server.sent("loop/delete").unwrap(), json!({"loop_id": id}));
    assert!(autonomy::state_snapshot().loops.is_empty(), "the deleted row is filtered");
}

/// `+ New loop`: the composer text becomes a self-paced loop, or a
/// fixed-interval one with ` | 5m` (the web's interval grammar, 60 s..24 h);
/// an empty text never leaves the client.
#[test]
fn new_loop_parses_the_web_interval_grammar() {
    assert_eq!(autonomy::parse_loop_interval("5m"), Some(300));
    assert_eq!(autonomy::parse_loop_interval("60s"), Some(60));
    assert_eq!(autonomy::parse_loop_interval("2h"), Some(7200));
    assert_eq!(autonomy::parse_loop_interval("1d"), Some(86_400));
    assert_eq!(autonomy::parse_loop_interval("90000ms"), Some(90));
    for bad in ["59s", "25h", "1.5m", "5", "m", "", "5 m", "-5m"] {
        assert_eq!(autonomy::parse_loop_interval(bad), None, "{bad:?}");
    }
    let store = Arc::new(Store::new());
    store.domains.config.set_supported_methods(vec!["loop/create".into(), "loop/list".into()]);
    store.set_capabilities(vec!["coding.loop_runtime.v1".into(), "coding.autonomy.v1".into()]);
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    assert_eq!(
        autonomy::resolve("loop.create", 0, Some("Run CI smoke | 15m"), &ctx),
        autonomy::Effect::LoopCreate { prompt: "Run CI smoke".into(), interval_seconds: Some(900) }
    );
    assert_eq!(
        autonomy::resolve("loop.create", 0, Some("Sync main"), &ctx),
        autonomy::Effect::LoopCreate { prompt: "Sync main".into(), interval_seconds: None }
    );
    assert!(matches!(
        autonomy::resolve("loop.create", 0, Some("  "), &ctx),
        autonomy::Effect::Unhandled(ref r) if r == "loop.create[empty]"
    ));
    assert!(dialog::notice_for_refusal("loop.create[empty]").is_some());
}

/// The wire keeps the cache current between reads: r1's own notification
/// sequence (create → pause → resume → delete; goal set → cleared) folds,
/// and a goal update older than the held generation is refused.
#[test]
fn the_recorded_notifications_keep_the_dialog_cache_current() {
    let _s = serial();
    autonomy::reset_state();
    let r1 = fixture("r1-autonomy-a6ea8505.jsonl");
    let loops = recorded(&r1, "loop/updated");
    autonomy::note_notification("loop/updated", &loops[0]);
    assert_eq!(autonomy::state_snapshot().loops.len(), 1);
    autonomy::note_notification("loop/updated", &loops[1]);
    assert_eq!(autonomy::state_snapshot().loops[0]["status"], "paused");
    autonomy::note_notification("loop/updated", &loops[3]);
    assert!(autonomy::state_snapshot().loops.is_empty(), "deleted:true removes the row");

    let monitors = recorded(&r1, "monitor/updated");
    autonomy::note_notification("monitor/updated", &monitors[0]);
    assert_eq!(autonomy::state_snapshot().monitors.len(), 1);

    let goal = recorded(&r1, "session/goal/updated").remove(0);
    let cleared = recorded(&r1, "session/goal/cleared").remove(0);
    autonomy::note_notification("session/goal/updated", &goal);
    assert!(autonomy::state_snapshot().goal.is_some());
    autonomy::note_notification("session/goal/cleared", &cleared);
    assert!(autonomy::state_snapshot().goal.is_none());
    // The older set (generation 1) cannot resurrect the cleared goal (2).
    autonomy::note_notification("session/goal/updated", &goal);
    assert!(autonomy::state_snapshot().goal.is_none(), "generation-gated");
}

/// Compact now: the outcome line is the web's (`ContextDialog.tsx` mutate)
/// over the RECORDED reply — r3's pass refused with a typed reason.
#[test]
fn the_compaction_outcome_is_the_web_line_over_the_recorded_reply() {
    let r3 = fixture("r3-session-a6ea8505.jsonl");
    let reply = recorded(&r3, "session/compact").remove(0);
    let (line, ok) = models::compaction_outcome(&reply);
    assert!(!ok, "r3's pass did not compact");
    assert_eq!(
        line,
        format!("Compaction {}: no_safe_semantic_boundary", reply["status"].as_str().unwrap())
    );
    assert_eq!(models::compaction_outcome(&json!({"compacted": true})).0, "Context compacted.");
    assert_eq!(models::compaction_outcome(&json!({"status": "skipped"})).0, "Compaction skipped.");
}

/// A create notification that lands BEFORE its reply does not duplicate the
/// row (`store.ts:669` ID upsert, never a blind prepend).
#[test]
fn a_create_notification_before_its_reply_does_not_duplicate_the_row() {
    let _s = serial();
    autonomy::reset_state();
    let r1 = fixture("r1-autonomy-a6ea8505.jsonl");
    let created = recorded(&r1, "loop/updated").remove(0);
    autonomy::note_notification("loop/updated", &created);
    autonomy::fold_loop_reply(&created, false);
    assert_eq!(autonomy::state_snapshot().loops.len(), 1, "one row per loop id");
    let monitor = recorded(&r1, "monitor/updated").remove(0);
    autonomy::note_notification("monitor/updated", &monitor);
    autonomy::fold_monitor_reply(&monitor, false);
    assert_eq!(autonomy::state_snapshot().monitors.len(), 1);
}

/// The palette lists the commands the server advertises (r1's recorded
/// capability set), filtered by prefix over names AND aliases
/// (`registry.ts` `commandSuggestions`); a draft with arguments shows no menu.
#[test]
fn the_palette_lists_advertised_commands_filtered_like_the_web() {
    let store = Arc::new(Store::new());
    let names = |q: &str| -> Vec<&'static str> {
        palette::suggestions(&store, q).into_iter().map(|i| palette::COMMANDS[i].name).collect()
    };
    assert!(names("/").is_empty(), "nothing advertised, nothing listed (fail closed)");
    store.set_capabilities(dialog::RECORDED_FEATURES.iter().map(|s| s.to_string()).collect());
    store
        .domains
        .config
        .set_supported_methods(dialog::RECORDED_METHODS.iter().map(|s| s.to_string()).collect());
    assert_eq!(names("/mo"), vec!["/model", "/monitor", "/mode"]);
    assert_eq!(names("/ctx"), vec!["/compact"], "the alias finds /compact");
    assert_eq!(names("/tasks"), vec!["/ps"]);
    assert_eq!(names("/btw why"), Vec::<&str>::new(), "arguments: no menu");
    // A7: r1 records no snapshot/* and no session/fork, so the history
    // dialog's /undo and /fork rows (registry.ts:133/:162) stay hidden there
    // (fail closed); every other row is advertised by r1.
    let listed = names("/");
    let hidden: Vec<&str> = palette::COMMANDS.iter().map(|c| c.name).filter(|n| !listed.contains(n)).collect();
    assert_eq!(hidden, vec!["/undo", "/fork"], "every other row is advertised by r1");
    // Every row runs a native effect (none is a silent no-op).
    for c in palette::COMMANDS {
        assert!(c.effect.is_some(), "{} has no effect", c.name);
    }
}

/// A known command submitted from the composer runs LOCALLY: the command
/// layer queues the palette row (never `turn/start`), aliases included.
#[test]
fn a_typed_command_queues_its_row_instead_of_a_prompt() {
    let _ = palette::take_queued();
    for (typed, row_name) in [("/model", "/model"), ("/context", "/compact"), ("/goal ship it", "/goal")] {
        let Some(palette::CommandMatch::Known(args, name)) = palette::match_command(typed) else {
            panic!("{typed} must be a known, runnable command");
        };
        assert!(palette::queue_run(&name, &args), "{typed} queues");
        let q = palette::take_queued();
        assert_eq!(q.len(), 1);
        assert_eq!(palette::COMMANDS[q[0].0].name, row_name, "{typed}");
        if typed.contains(' ') {
            assert_eq!(q[0].1, "ship it", "the arguments ride along to be REPORTED");
        }
    }
}

/// The dispatcher's fall-through: every id the screen modules own AFTER the
/// conversation router is `Unhandled` to that router — so the predicate must
/// claim it, or the click dies at the router (the measured defect).
#[test]
fn screen_owned_ids_fall_through_the_conversation_router() {
    let store = Arc::new(Store::new());
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    for id in [
        "models.test_route",
        "models.discover",
        "context.compact_now",
        "context.mode.llm",
        "context.mode.heuristic",
        "skills.remove_0",
        "skills.install_3",
        "research.lane_upsert",
        "history.rewind_conversation",
        "thinking.effort.high",
        "composer.copy_transcript",
        "media.submit",
        "peer.approve",
    ] {
        assert!(
            matches!(
                octoscode_module::actions::resolve(id, 0, &ctx),
                octoscode_module::actions::Effect::Unhandled(_)
            ),
            "{id} is not the conversation router's"
        );
        assert!(octoscode_module::screens::owned_after_router(id), "{id} must fall through");
    }
    for id in ["session.new", "thread.open", "goal.pause", "dialog.close"] {
        assert!(!octoscode_module::screens::owned_after_router(id), "{id} has its own arm");
    }
}

/// The context lifecycle never regresses its generation
/// (`context-events.ts:25` `applyContextNotification`): r23's RECORDED
/// `context/normalization_reported` frames, replayed through the REAL
/// registry, leave the newest snapshot; the oldest frame arriving again late
/// is dropped, and the Context dialog's occupancy keeps the newest estimate.
#[test]
fn a_late_context_notification_never_regresses_the_occupancy() {
    use octos_core::app_ui::AppUiBackendEvent as UiNotification;
    use octoscode_client::registry::Registry;
    let r23 = fixture("r23-conversation-a6ea8505.jsonl");
    let frames: Vec<&Frame> = r23
        .iter()
        .filter(|f| f.dir == "in" && f.method == "context/normalization_reported")
        .collect();
    assert!(frames.len() >= 2, "r23 records the normalization stream");
    let store = Arc::new(Store::new());
    let mut reg = Registry::new();
    octoscode_client::domains::register_all(&mut reg, store.clone());
    let dispatch = |reg: &mut Registry, f: &Frame| {
        let n = UiNotification::from_method_and_params(&f.method, f.body.clone()).expect("decodes");
        assert!(reg.dispatch(&n), "the session domain claims {}", f.method);
    };
    for f in &frames {
        dispatch(&mut reg, f);
    }
    let (first, last) = (frames[0], frames[frames.len() - 1]);
    let session = last.body["session_id"].as_str().expect("session").to_owned();
    let held = |store: &Store| {
        store.domains.session.context(&session).map(|c| c.state["generation"].clone())
    };
    let newest = last.body["context_state"]["generation"].clone();
    assert_eq!(held(&store), Some(newest.clone()));
    assert!(first.body["context_state"]["generation"].as_u64() < newest.as_u64());
    dispatch(&mut reg, first);
    assert_eq!(held(&store), Some(newest), "a late older frame never regresses the snapshot");
    // The dialog's occupancy line reads the newest estimate.
    store.domains.session.set_active(Some(session.clone()));
    let estimate = last.body["context_state"]["token_estimate"].as_u64().expect("estimate");
    let state = store.domains.session.context(&session).expect("held").state;
    assert_eq!(state["token_estimate"].as_u64(), Some(estimate));
}

/// A `!command` is the web's `local-shell-unavailable` intent
/// (`intent.ts:59/175`): submitted from the composer it is REPORTED — no
/// `turn/start` reaches the wire — and the text stays editable.
#[tokio::test]
async fn a_local_shell_bang_is_reported_and_never_sent() {
    assert!(palette::is_local_shell_bang("!ls -la"));
    assert!(palette::is_local_shell_bang(" \u{200B}！git status"), "format chars + full-width");
    assert!(!palette::is_local_shell_bang("why does ! mean not?"));
    let server = Server::start(vec![("session/open".into(), recorded_open())]).await;
    let conv = connect(&server).await;
    conv.set_draft("!ls -la");
    let id = conv.submit_draft().await.expect("a report, not an error");
    assert!(id.is_empty(), "no turn started");
    assert!(server.sent("turn/start").is_none(), "nothing reached the model");
    assert_eq!(conv.ui().lock().unwrap().draft(), "!ls -la", "the text stays editable");
    let session = conv.store.active_session().unwrap_or_default();
    let entries = conv.store.domains.session.timeline.entries(&session);
    assert!(
        entries.iter().any(|e| e.kind == palette::REPORT_KIND && e.text.starts_with("Local shell unavailable")),
        "the receipt row"
    );
}

/// The Skills dialog's search box (Enter) sends ONE
/// `profile/skills/registry/search` with the query and the connection's
/// Profile (`skills.ts:152`), folds the reply's packages over the previous
/// result (`setPackages`), and the dialog draws them. No recording carries
/// this method, so the reply is the web contract's shape (`skills.ts:79`)
/// and the assertion is the request the production client sends.
#[tokio::test]
async fn the_registry_search_sends_the_query_and_folds_the_packages() {
    let reply = json!({
        "profile_id": "dsflash",
        "packages": [
            {"name": "lint-kit", "description": "Lints", "repo": "octos/lint-kit", "version": "0.3.0",
             "license": "MIT", "tags": ["lint"], "provides_tools": true, "installed": false},
            {"name": "no-version"}
        ]
    });
    let server = Server::start(vec![
        ("session/open".into(), recorded_open()),
        ("profile/skills/registry/search".into(), reply),
    ])
    .await;
    let conv = connect(&server).await;
    conv.store.domains.profile.set_current("dsflash".into());
    let n = models::search_registry(&conv, &conv.store, "lint").await.expect("search");
    assert_eq!(n, 2);
    let sent = server.sent("profile/skills/registry/search").expect("one search request");
    assert_eq!(sent["q"], "lint");
    assert_eq!(sent["profile_id"], "dsflash");
    let pkgs = conv.store.domains.profile.registry_packages();
    assert_eq!(pkgs[0].name, "lint-kit");
    assert_eq!(pkgs[0].license.as_deref(), Some("MIT"));
    // A malformed reply folds as NO packages, never the stale ones.
    assert_eq!(models::fold_registry_search(json!({"packages": "nope"}), &conv.store), 0);
    assert!(conv.store.domains.profile.registry_packages().is_empty());
}

/// `/stop` (aliases `/interrupt`, `/esc`) is the web's interrupt intent
/// (`registry.ts:272`): a palette row gated on `turn/interrupt` whose effect
/// is the composer Stop button's own action, and typed it runs locally.
#[test]
fn stop_is_the_interrupt_intent() {
    let i = palette::command_index("stop").expect("/stop is a row");
    assert_eq!(palette::COMMANDS[i].effect, Some("turn.interrupt"));
    assert_eq!(palette::command_index("interrupt"), Some(i));
    let Some(palette::CommandMatch::Known(_, name)) = palette::match_command("/esc") else {
        panic!("/esc is a known, runnable command");
    };
    assert_eq!(palette::command_index(&name), Some(i));
    let store = Arc::new(Store::new());
    assert!(!palette::available(&store, &palette::COMMANDS[i]), "fail closed unadvertised");
    store.domains.config.set_supported_methods(vec!["turn/interrupt".into()]);
    assert!(palette::available(&store, &palette::COMMANDS[i]));
}
