//! A10 — the header's Review entry as the web's `DiffReviewDialog`
//! (`features/review/DiffReviewDialog.tsx` over `use-coding-safety.ts:268-345`),
//! at the wire: the board-3 dialog's open / Refresh / close run the SAME
//! functions the clicks run (`board3::host::{open, perform, run, close}`),
//! against a fake server answering `diff/preview/get` with the octos-core
//! `DiffPreviewGetResult` shape (no successful reply was ever recorded:
//! r30a recorded only the request and a preview-less refusal; the body is
//! A6's `surfaces` scenario's octos-core-shaped preview).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::board3::diff_review as dr;
use octoscode_module::screens::board3::host::{self, Dialog, Job, Outcome};
use octoscode_module::screens::review;

const PREVIEW: &str = "01920000-0000-7000-8000-0000000000f1";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    Preview,
    Empty,
    Refused,
    Mismatched,
}

fn preview(session: &str, preview_id: &Value, files: bool) -> Value {
    let files = if files {
        json!([{"path": "src/main.rs", "status": "modified", "hunks": [{
            "header": "@@ -10,6 +10,9 @@ fn main() {",
            "lines": [
                {"kind": "context", "content": "    let args = Args::parse();", "old_line": 10, "new_line": 10},
                {"kind": "removed", "content": "    run(args);", "old_line": 11},
                {"kind": "added", "content": "    if args.version {", "new_line": 11},
                {"kind": "added", "content": "        println!(\"octos {}\", env!(\"CARGO_PKG_VERSION\"));", "new_line": 12},
                {"kind": "added", "content": "        return;", "new_line": 13},
                {"kind": "added", "content": "    }", "new_line": 14},
                {"kind": "context", "content": "}", "old_line": 12, "new_line": 15}
            ]
        }]}, {"path": "docs/cli.md", "old_path": "docs/usage.md", "status": "renamed"}])
    } else {
        json!([])
    };
    json!({"status": "ready", "source": "pending_store", "preview": {
        "session_id": session, "preview_id": preview_id, "title": "Add a --version flag", "files": files
    }})
}

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    answer: Arc<Mutex<Answer>>,
}

impl Server {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let answer = Arc::new(Mutex::new(Answer::Preview));
        let (seen2, answer2) = (seen.clone(), answer.clone());
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { continue };
                let (mut sink, mut source) = ws.split();
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                tokio::spawn(async move {
                    while let Some(frame) = rx.recv().await {
                        if sink.send(Message::Text(frame.into())).await.is_err() {
                            break;
                        }
                    }
                });
                let (seen, answer) = (seen2.clone(), answer2.clone());
                tokio::spawn(async move {
                    while let Some(Ok(msg)) = source.next().await {
                        let Message::Text(text) = msg else { continue };
                        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                        if v.get("id").is_none() {
                            continue;
                        }
                        let method = v["method"].as_str().unwrap_or("").to_owned();
                        let p = v["params"].clone();
                        seen.lock().unwrap().push((method.clone(), p.clone()));
                        let session = p["session_id"].as_str().unwrap_or("a10:main").to_owned();
                        let reply: Result<Value, Value> = match method.as_str() {
                            "session/open" => Ok(json!({"opened": {
                                "session_id": session, "active_profile_id": "a10",
                                "workspace_root": "/home/user/src/octos",
                                "cursor": {"stream": session, "seq": 1},
                                "capabilities": {
                                    "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                                    "capabilities_schema_version": 1,
                                    "supported_methods": ["session/open", "session/list", "session/hydrate", "turn/start",
                                        "turn/interrupt", "diff/preview/get", "review/start"],
                                    "supported_notifications": ["turn/started", "turn/completed"],
                                    "supported_features": ["review.start.v1"]
                                }
                            }})),
                            "session/list" => Ok(json!({"sessions": []})),
                            "diff/preview/get" => match *answer.lock().unwrap() {
                                Answer::Preview => Ok(preview(&session, &p["preview_id"], true)),
                                Answer::Empty => Ok(preview(&session, &p["preview_id"], false)),
                                Answer::Mismatched => Ok(preview(&session, &json!("01920000-0000-7000-8000-0000000000aa"), true)),
                                Answer::Refused => Err(json!({"code": -32103, "message": "preview target not found"})),
                            },
                            _ => Ok(json!({})),
                        };
                        let frame = match reply {
                            Ok(result) => json!({"jsonrpc": "2.0", "id": v["id"], "result": result}),
                            Err(error) => json!({"jsonrpc": "2.0", "id": v["id"], "error": error}),
                        };
                        let _ = tx.send(frame.to_string());
                    }
                });
            }
        });
        Self { base_url, seen, answer }
    }

    fn sent(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    host::reset();
    review::reset();
    g
}

async fn connected(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "a10", None, None).expect("connect");
    let conv = Arc::new(conv);
    conv.open_workspace(None).await.expect("session/open");
    let mut opened = false;
    for _ in 0..80 {
        if opened && conv.store.is_live() {
            break;
        }
        match tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(evt)) => {
                if matches!(conv.on_event(evt), FlowEvent::WorkspaceOpened(_)) {
                    opened = true;
                }
            }
            _ => break,
        }
    }
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv
}

fn dsl(conv: &Conversation) -> String {
    host::lower_open(&conv.store).expect("the dialog is open").dsl
}

fn job_of(o: Outcome) -> Job {
    match o {
        Outcome::Spawn(j) => j,
        other => panic!("expected a job, got {other:?}"),
    }
}

/// The native entry exists before any preview does (the web hides its
/// "Review changes" until one is announced): the dialog says so — never a
/// blank sheet — reads nothing, and offers the server's native code review.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn with_no_preview_the_entry_says_so_and_offers_code_review() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    assert_eq!(host::perform("b3.open.diff_review", 0, &conv.store), Outcome::Done, "no preview: nothing to read");
    assert_eq!(host::open_dialog(), Some(Dialog::DiffReview));
    let d = dsl(&conv);
    for copy in [dr::EYEBROW, dr::TITLE, dr::NO_PREVIEW_HEAD, dr::NO_PREVIEW_BODY, dr::NATIVE_REVIEW] {
        assert!(d.contains(copy), "{copy}");
    }
    assert!(d.contains("b3_close_box") && d.contains("width: 28 height: 28"), "a 28 px close");
    assert!(server.sent("diff/preview/get").is_empty(), "no read without a preview id");
    assert_eq!(host::perform("b3.diff.refresh", 0, &conv.store), Outcome::Done, "Refresh is inert");
    assert_eq!(
        host::perform("b3.diff.native", 0, &conv.store),
        Outcome::Action("dialog.open.review".into()),
        "the Code review dialog (/review) takes over"
    );
    assert_eq!(host::open_dialog(), None, "one modal at a time");
}

/// An announced preview (an approval's typed diff) is read ONCE through
/// `diff/preview/get {session_id, preview_id}` and rendered as the web's
/// dialog: the preview's title, the +/− totals over its lines, status ·
/// source · id, every file (mark, path, the renamed file's "from", status)
/// with its hunk header and numbered, marked lines; Refresh reads again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_announced_preview_is_read_once_and_rendered_like_the_web_dialog() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    review::set_preview_id(PREVIEW.into());
    let job = job_of(host::perform("b3.open.diff_review", 0, &conv.store));
    assert!(dsl(&conv).contains(dr::LOADING), "the loading state while the read is in flight");
    assert!(dsl(&conv).contains("b3_diff_refresh_box"), "Refresh is drawn");
    host::run(job, &conv).await.expect("diff/preview/get");
    assert_eq!(server.sent("diff/preview/get"), vec![json!({"session_id": conv.session_id(), "preview_id": PREVIEW})]);
    let d = dsl(&conv);
    for copy in [
        dr::EYEBROW,
        "Add a --version flag",
        "+4",
        "−1",
        "ready",
        "pending_store",
        PREVIEW,
        "src/main.rs",
        "modified",
        "@@ -10,6 +10,9 @@ fn main() {",
        "    if args.version {",
        "docs/cli.md",
        "from docs/usage.md",
        "renamed",
        dr::NO_LINES,
    ] {
        assert!(d.contains(copy), "{copy}");
    }
    assert!(!d.contains(dr::LOADING));
    let job = job_of(host::perform("b3.diff.refresh", 0, &conv.store));
    host::run(job, &conv).await.expect("refresh");
    assert_eq!(server.sent("diff/preview/get").len(), 2, "Refresh = one more read");
    host::close();
    assert_eq!(host::open_dialog(), None);
}

/// The web's other states: an empty preview, a refused read ("Preview
/// unavailable" + the bounded reason), a reply naming another preview
/// (mismatched), and a late reply after the dialog closed (dropped).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_refused_mismatched_and_stale_reads_follow_the_web() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    review::set_preview_id(PREVIEW.into());

    *server.answer.lock().unwrap() = Answer::Empty;
    let job = job_of(host::perform("b3.open.diff_review", 0, &conv.store));
    host::run(job, &conv).await.expect("empty");
    assert!(dsl(&conv).contains(dr::NO_FILES));

    *server.answer.lock().unwrap() = Answer::Refused;
    let job = job_of(host::perform("b3.diff.refresh", 0, &conv.store));
    assert!(host::run(job, &conv).await.is_err());
    let d = dsl(&conv);
    assert!(d.contains(dr::UNAVAILABLE), "{d}");
    assert!(d.contains("b3_diff_error_body"));

    *server.answer.lock().unwrap() = Answer::Mismatched;
    let job = job_of(host::perform("b3.diff.refresh", 0, &conv.store));
    assert!(host::run(job, &conv).await.is_err());
    assert!(dsl(&conv).contains(dr::MISMATCH), "another preview's reply is refused");
    assert!(!dsl(&conv).contains("src/main.rs"), "and never rendered");

    // A read that lands after the close is dropped.
    *server.answer.lock().unwrap() = Answer::Preview;
    let job = job_of(host::perform("b3.diff.refresh", 0, &conv.store));
    host::close();
    host::run(job, &conv).await.expect("the late read");
    assert!(host::state().diff.result.is_none(), "a closed dialog keeps nothing");
}
