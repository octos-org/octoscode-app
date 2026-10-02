//! A7 — explicit image upload: dedup / cancel / late-receipt guard, at the
//! wire (production path). The images dialog's own controls
//! (`board3::host::perform`: `b3.img.upload`, `b3.img.remove#i`,
//! `b3.img.cancel`) and the job the host spawns (`board3::host::run` →
//! `images::upload` → `POST /api/upload`) run against a fake server whose
//! upload route answers SLOWLY, so the second click / the removal / the
//! cancel land while the transfer is in flight.
//!
//! Ports `attachment-drafts.test.ts`: "deduplicates same-tick uploads and
//! waits for all selected image receipts", "removing an in-flight entry aborts
//! it and ignores a late receipt", and the cancel arm (`cancelUploads`: rows
//! back to `selected`, nothing deleted on the server, the late receipt
//! ignored).
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::board3::host::{self, Job, Outcome};
use octoscode_module::screens::media::{self, DraftStatus};

const PROFILE: &str = "a7";

struct Server {
    base_url: String,
    uploads: Arc<AtomicUsize>,
}

fn handle_for(name: &str) -> String {
    match name {
        "a.png" => "up/YTcvdXBsb2Fkcy9hLnBuZw/a.png".into(),
        "b.png" => "up/YTcvdXBsb2Fkcy9iLnBuZw/b.png".into(),
        _ => "up/YTcvdXBsb2Fkcy9zaG90LnBuZw/shot.png".into(),
    }
}

async fn serve(mut stream: TcpStream, uploads: Arc<AtomicUsize>) {
    let mut head = [0u8; 16];
    let n = stream.peek(&mut head).await.unwrap_or(0);
    if String::from_utf8_lossy(&head[..n]).starts_with("POST /api/upload") {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        let header_end = loop {
            let k = stream.read(&mut chunk).await.unwrap_or(0);
            if k == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..k]);
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
        };
        let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let len = headers
            .lines()
            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
            .unwrap_or(0);
        while buf.len() < header_end + len {
            let k = stream.read(&mut chunk).await.unwrap_or(0);
            if k == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..k]);
        }
        let body = String::from_utf8_lossy(&buf[header_end..]).to_string();
        uploads.fetch_add(1, Ordering::SeqCst);
        let name = if body.contains("filename=\"a.png\"") { "a.png" } else if body.contains("filename=\"b.png\"") { "b.png" } else { "shot.png" };
        // The transfer is slow: the test acts while it is in flight.
        tokio::time::sleep(Duration::from_millis(400)).await;
        let payload = json!([handle_for(name)]).to_string();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let _ = stream.write_all(resp.as_bytes()).await;
        return;
    }
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
    let (mut tx, mut rx) = ws.split();
    while let Some(Ok(msg)) = rx.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        if v.get("id").is_none() {
            continue;
        }
        let session = v["params"]["session_id"].as_str().unwrap_or("a7:main").to_owned();
        let result = match v["method"].as_str().unwrap_or("") {
            "session/open" => json!({"opened": {
                "session_id": session, "active_profile_id": PROFILE,
                "cursor": {"stream": session, "seq": 1},
                "capabilities": {
                    "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                    "capabilities_schema_version": 1,
                    "supported_methods": ["session/open", "session/list", "turn/start"],
                    "supported_notifications": [], "supported_features": []
                }
            }}),
            "session/list" => json!({"sessions": []}),
            _ => json!({}),
        };
        let frame = json!({"jsonrpc": "2.0", "id": v["id"], "result": result});
        let _ = tx.send(Message::Text(frame.to_string().into())).await;
    }
}

impl Server {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let uploads = Arc::new(AtomicUsize::new(0));
        let u2 = uploads.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                tokio::spawn(serve(stream, u2.clone()));
            }
        });
        Self { base_url, uploads }
    }
}

async fn connected(server: &Server) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", PROFILE, None, None).expect("connect");
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
    Arc::new(conv)
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|p| p.into_inner());
    host::reset();
    g
}

fn pngs(names: &[&str]) -> Vec<std::path::PathBuf> {
    let dir = std::env::temp_dir().join(format!("a7-media-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    names
        .iter()
        .map(|n| {
            let p = dir.join(n);
            std::fs::write(&p, format!("\u{89}PNG fake bytes {n}")).unwrap();
            p
        })
        .collect()
}

fn job(o: Outcome) -> Job {
    match o {
        Outcome::Spawn(j) => j,
        other => panic!("expected a job, got {other:?}"),
    }
}

fn reset_drafts(conv: &Conversation) {
    if let Some(d) = media::drafts_for_session(&conv.session_id()) {
        for e in d.entries() {
            d.remove(&e.id);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_upload_click_never_sends_an_image_twice_and_both_receipts_land() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    reset_drafts(&conv);
    assert_eq!(host::command("images", "", &conv), Some(Outcome::Done));
    host::files_chosen(&pngs(&["a.png", "b.png"]));
    let drafts = media::drafts_for_session(&conv.session_id()).expect("drafts");
    assert_eq!(drafts.len(), 2);

    let first = job(host::perform("b3.img.upload", 0, &conv.store));
    let c1 = conv.clone();
    let running = tokio::spawn(async move { host::run(first, &c1).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    // The same-tick repeat: everything is already claimed → nothing new.
    let again = job(host::perform("b3.img.upload", 0, &conv.store));
    assert_eq!(host::run(again, &conv).await.unwrap(), "nothing to upload", "every entry is already claimed");
    assert!(drafts.uploading());
    // A turn cannot take the batch until EVERY receipt landed.
    conv.ui().lock().unwrap().set_draft_inner("look");
    assert!(media::take_for_submit(&conv).is_err(), "unready images refuse the send");
    running.await.unwrap().expect("upload");
    assert_eq!(server.uploads.load(Ordering::SeqCst), 2, "one POST per image, never twice");
    assert!(!drafts.uploading());
    assert!(drafts.entries().iter().all(|e| e.status == DraftStatus::Ready));
    reset_drafts(&conv);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn removing_an_uploading_image_ignores_its_late_receipt_and_frees_the_picker() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    reset_drafts(&conv);
    assert_eq!(host::command("images", "", &conv), Some(Outcome::Done));
    host::files_chosen(&pngs(&["a.png"]));
    let drafts = media::drafts_for_session(&conv.session_id()).expect("drafts");
    let up = job(host::perform("b3.img.upload", 0, &conv.store));
    let c1 = conv.clone();
    let running = tokio::spawn(async move { host::run(up, &c1).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(drafts.uploading());
    // The slot's ✕ while the transfer is in flight.
    assert_eq!(host::perform("b3.img.remove", 0, &conv.store), Outcome::Done);
    assert!(!drafts.uploading(), "the in-flight claim is released with the row");
    running.await.unwrap().expect("job");
    assert_eq!(server.uploads.load(Ordering::SeqCst), 1);
    assert_eq!(drafts.len(), 0, "the late receipt never resurrects the removed row");
    assert_eq!(host::state().img.error, None, "an ignored receipt is not an error");
    assert_eq!(host::perform("b3.img.choose", 0, &conv.store), Outcome::PickFiles, "the picker is usable again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_returns_rows_to_selected_and_ignores_the_late_receipt() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    reset_drafts(&conv);
    assert_eq!(host::command("images", "", &conv), Some(Outcome::Done));
    host::files_chosen(&pngs(&["shot.png"]));
    let drafts = media::drafts_for_session(&conv.session_id()).expect("drafts");
    let up = job(host::perform("b3.img.upload", 0, &conv.store));
    let c1 = conv.clone();
    let running = tokio::spawn(async move { host::run(up, &c1).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(host::perform("b3.img.cancel", 0, &conv.store), Outcome::Done);
    assert_eq!(
        host::state().img.notice.as_deref(),
        Some("Transfers canceled locally. No server files were deleted.")
    );
    assert_eq!(drafts.entries()[0].status, DraftStatus::Selected);
    running.await.unwrap().expect("job");
    assert_eq!(drafts.entries()[0].status, DraftStatus::Selected, "the late receipt is ignored");
    // An explicit retry uploads it again (the server may keep the first).
    let retry = job(host::perform("b3.img.upload", 0, &conv.store));
    host::run(retry, &conv).await.expect("retry");
    assert_eq!(server.uploads.load(Ordering::SeqCst), 2);
    assert_eq!(drafts.entries()[0].status, DraftStatus::Ready);
    reset_drafts(&conv);
}
