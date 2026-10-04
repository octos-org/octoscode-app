//! A36 — Memory (board 5, `design/stage-a/phase4-new5`, signed off D1 yes /
//! D2 A / D3 Capabilities / D4 follow the app theme) through the production
//! path: the Settings > Capabilities row's action (`b3.open.memory`),
//! `board3::host::open` / `perform` / `input_*` for the dialog's controls and
//! `host::run` for its jobs, against a WebSocket server answering with
//! FAITHFUL replies:
//!
//! - `a36-memory-a6ea8505.jsonl` — RECORDED on a private serve at a6ea8505:
//!   `memory/overview` answers the token identity's (`admin`) empty memory
//!   with no profile named, `memory/search` / `load` / `ingest` are refused
//!   `-32603 runtime_unavailable` (the scope finding);
//! - `a36-memory-proposal-synthetic.jsonl` — octos-cli's own reply shapes
//!   plus the upstream proposal's `profile_id` echo
//!   (`docs/proposals/memory-profile-scope.md`), board 5's content
//!   (`tools/fixtures/a36_memory_fixture.py`).
//!
//! D2 A: Memory is built for the Session's profile — every call names it
//! (`profile_id`), and a reply that does not say it answered for that
//! profile is never shown as its memory. D1: the scope line is plain text in
//! the muted ink; a refusal explains the problem and the next step in
//! bounded copy, never the server's raw error.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::Conversation;
use octoscode_module::i18n::{self, Lang};
use octoscode_module::screens::board3::host::{self, Dialog, Job, Outcome};
use octoscode_module::screens::board3::memory::{self, Failure, KindFilter, Op, Page, Refusal};
use octoscode_module::screens::settings;
use octoscode_store::Store;

const RECORDED: &str = "a36-memory-a6ea8505.jsonl";
const PROPOSAL: &str = "a36-memory-proposal-synthetic.jsonl";
const DOC_ID: &str = "doc:octoscode:7f3a9c2e5b1d4a60";
const ALL_MEMORY: &[&str] = &["memory/overview", "memory/entity", "memory/search", "memory/load", "memory/ingest"];

fn frames(name: &str) -> Vec<Value> {
    let path = format!("{}/../octoscode-client/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("JSON"))
        .collect()
}

fn replies(name: &str, method: &str) -> Vec<Value> {
    frames(name).into_iter().filter(|f| f["dir"] == "in" && f["method"] == method).map(|f| f["body"].clone()).collect()
}

fn reply(name: &str, method: &str) -> Value {
    replies(name, method).into_iter().next().unwrap_or_else(|| panic!("{name}: no {method} reply"))
}

/// The recorded `-32603 runtime_unavailable` refusal (a6ea8505).
fn runtime_unavailable() -> Value {
    reply(RECORDED, "memory/search")
}

fn recorded_open() -> Value {
    let mut value = frames("r1-autonomy-a6ea8505.jsonl")
        .into_iter()
        .filter(|f| f["dir"] == "in" && f["method"] == "session/open")
        .map(|f| f["body"].clone())
        .find(|b| b.get("active_profile_id").is_some())
        .expect("r1 open");
    value["capabilities"]["supported_features"].as_array_mut().unwrap().push(json!("memory.session_scope.v1"));
    value
}

fn proposal_overview() -> Value {
    reply(PROPOSAL, "memory/overview")
}

fn proposal_load(id: &str) -> Value {
    replies(PROPOSAL, "memory/load").into_iter().find(|b| b["record"]["id"] == id).expect("a load")
}

fn proposal_entity(name: &str) -> Value {
    replies(PROPOSAL, "memory/entity").into_iter().find(|b| b["name"] == name).expect("an entity")
}

/// A fake octos: per method, the canned bodies in order (the last repeats);
/// a body `{"__error": {...}}` answers that JSON-RPC error.
struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
    async fn start(canned: Vec<(&str, Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let mut queues: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for (m, b) in canned {
            queues.entry(m.to_owned()).or_default().push(b);
        }
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (mut tx, mut rx) = ws.split();
            let mut pos: BTreeMap<String, usize> = BTreeMap::new();
            while let Some(Ok(msg)) = rx.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                log.lock().unwrap().push((method.clone(), v["params"].clone()));
                let mut body = match queues.get(&method) {
                    Some(list) => {
                        let k = pos.entry(method.clone()).or_insert(0);
                        let b = list[(*k).min(list.len() - 1)].clone();
                        *k += 1;
                        b
                    }
                    None => json!({}),
                };
                if let Some(delay) = body["__delay_ms"].as_u64() { tokio::time::sleep(std::time::Duration::from_millis(delay)).await; }
                if method.starts_with("memory/") && body["profile_id"].is_string() && body.get("scope").is_none() {
                    body["scope"] = json!({"kind":"profile","namespace":null,"session_id":v["params"]["context"]["session_id"]});
                }
                let frame = match body.get("__error").or_else(|| body.get("error")) {
                    Some(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
                    None => json!({"jsonrpc": "2.0", "id": id, "result": body}),
                };
                let _ = tx.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { base_url: format!("http://{addr}"), seen }
    }

    fn sent(&self, method: &str) -> Vec<Value> {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }
}

async fn connect(server: &Server, methods: &[&str]) -> Arc<Conversation> {
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "dsflash", None, None).expect("connect");
    let conv = Arc::new(conv);
    let drv = conv.clone();
    tokio::spawn(async move {
        while let Some(evt) = events.recv().await {
            let _ = drv.on_event(evt);
        }
    });
    conv.open_workspace(None).await.expect("session/open");
    conv.store.domains.session.set_active(Some(conv.session_id()));
    conv.store.domains.profile.set_current("dsflash".into());
    // The recorded open advertises the memory methods (auxiliary.rest_to_ws.v1);
    // a test names exactly the set it needs.
    conv.store.domains.config.set_supported_methods(methods.iter().map(|m| m.to_string()).collect());
    conv
}

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    let g = L.lock().unwrap_or_else(|e| e.into_inner());
    host::reset();
    i18n::set_language(Lang::En);
    // The theme is process-wide (tests never install the OS reader, so
    // `system` would read dark): every test here starts in light.
    octoscode_module::screens::theme::set_preference("light");
    g
}

async fn run(conv: &Conversation, out: Outcome) -> Result<String, String> {
    match out {
        Outcome::Spawn(job) => host::run(job, conv).await,
        other => panic!("expected a job, got {other:?}"),
    }
}

fn dsl(store: &Store) -> String {
    host::lower_open(store).expect("the dialog lowers").dsl
}

/// Open Memory the way the Settings row does, and fold the overview.
async fn open_memory(conv: &Conversation) -> String {
    let out = host::perform("b3.open.memory", 0, &conv.store);
    assert!(matches!(out, Outcome::Spawn(Job::MemoryOverview(_))), "{out:?}");
    assert_eq!(host::open_dialog(), Some(Dialog::Memory));
    run(conv, out).await.unwrap_or_else(|e| e)
}

fn store_with(methods: &[&str]) -> Store {
    let store = Store::new();
    store.set_connection("Live".into(), true);
    store.domains.config.set_supported_methods(methods.iter().map(|m| m.to_string()).collect());
    store
}

// ------------------------------------------------------------ the entry

#[test]
fn capabilities_offers_memory_only_when_the_server_advertises_it() {
    let _g = serial();
    settings::reset_state();
    assert!(settings::capability_rows(&store_with(&["profile/skills/list", "mcp/status/list"])).iter().all(|r| *r != "memory"));
    assert_eq!(settings::capability_rows(&store_with(&["memory/overview"])), vec!["memory"]);
    assert_eq!(
        settings::capability_rows(&store_with(&["profile/skills/list", "mcp/status/list", "memory/overview"])),
        vec!["skills", "mcp", "memory"],
        "Memory is the section's third row (board 5 frame 1)"
    );
    let row = settings::CAPABILITY_ROWS.iter().find(|r| r.id == "memory").expect("a memory row");
    assert_eq!((row.hit, row.action), ("set_cap_memory", "b3.open.memory"));
    assert!(host::routes(row.action), "the board-3 host owns it");
    let store = store_with(ALL_MEMORY);
    assert!(matches!(host::perform(row.action, 0, &store), Outcome::Spawn(Job::MemoryOverview(_))));
    assert_eq!(host::open_dialog(), Some(Dialog::Memory));
    // Before the reply: the loading state (frame 10b), no stale content.
    let d = dsl(&store);
    assert!(d.contains("Loading memory…"), "{d}");
    assert!(d.contains("b3_mem_loading"));
}

// ------------------------------------------------------- the overview

#[tokio::test]
async fn the_overview_asks_for_the_session_profile_and_draws_board_5() {
    let _g = serial();
    let server = Server::start(vec![("session/open", json!({"opened": recorded_open()})), ("memory/overview", proposal_overview())]).await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    assert_eq!(server.sent("memory/overview"), vec![json!({"profile_id": "dsflash", "context":{"session_id":conv.session_id()}})], "D2 A: the Session's profile");
    let d = dsl(&conv.store);
    for want in [
        "Memory",
        "Long-term memory",
        "Prefers Rust 2024 edition and cargo nextest.",
        "Show all",
        "Today",
        "Fixed the steer queue redelivery",
        "Entities",
        "octos-core",
        "Rust crate with the UI protocol and its methods.",
        "steer-queue",
        "dsflash",
        "2 notes waiting for the next memory refresh.",
        "Add note",
        "Search memory",
    ] {
        assert!(d.contains(want), "missing {want:?}");
    }
    for id in ["b3_mem_title", "b3_mem_scope", "b3_mem_add", "b3_mem_refresh", "b3_close", "b3_mem_query", "b3_mem_lt", "b3_mem_lt_more", "b3_mem_today", "b3_mem_entity_0", "b3_mem_staging"] {
        assert!(d.contains(id), "no {id}");
    }
    let st = host::state();
    assert!(st.mem.confirmed, "the reply named the Session's profile");
    assert_eq!(st.mem.page, Page::Overview);
}

#[tokio::test]
async fn the_scope_line_is_plain_text_in_the_muted_ink() {
    let _g = serial();
    let server = Server::start(vec![("session/open", json!({"opened": recorded_open()})), ("memory/overview", proposal_overview())]).await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    assert_eq!(memory::scope_line("dsflash"), "Server Profile: dsflash");
    let d = dsl(&conv.store);
    let block = d.split("b3_mem_scope := Label {").nth(1).expect("the scope label").split("\n}").next().unwrap().to_owned();
    assert!(block.contains("Server Profile: dsflash"), "{block}");
    assert!(block.contains("Inter-400"), "D1: plain text, not monospace: {block}");
    assert!(!block.contains("LiberationMono"), "D1: not monospace: {block}");
    assert!(block.contains("#61666bff"), "D1: the theme's muted ink: {block}");
}

#[tokio::test]
async fn todays_server_answers_for_the_signed_in_account_so_the_dialog_refuses_honestly() {
    let _g = serial();
    // a6ea8505: admin's empty memory, no profile named.
    let server = Server::start(vec![("session/open", json!({"opened": recorded_open()})), ("memory/overview", reply(RECORDED, "memory/overview"))]).await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    let d = dsl(&conv.store);
    assert!(d.contains("Couldn't read memory."), "{d}");
    assert!(d.contains("b3_mem_error"));
    let (lead, problem, next) = memory::copy(&Failure { op: Op::Read, refusal: Refusal::Scope }, "dsflash");
    assert_eq!(lead, "Couldn't read memory.");
    assert!(d.contains(&problem) && d.contains(&next), "the bounded problem + next step: {problem} / {next}");
    assert!(problem.contains("dsflash"), "it names the profile asked for: {problem}");
    // Never another profile's memory presented as this one's, never a write.
    assert!(!d.contains("No knowledge pages yet"), "admin's empty memory is not dsflash's");
    assert!(!d.contains("b3_mem_add"), "no Add note while the profile is unconfirmed");
    assert!(!host::state().mem.confirmed);
    assert_eq!(host::perform("b3.mem.add", 0, &conv.store), Outcome::Done);
    assert_eq!(host::state().mem.page, Page::Overview, "Add note refuses to open");
}

#[tokio::test]
async fn an_empty_profile_says_so() {
    let _g = serial();
    let mut empty = proposal_overview();
    empty["overview"] = reply(RECORDED, "memory/overview")["overview"].clone();
    let server = Server::start(vec![("session/open", json!({"opened": recorded_open()})), ("memory/overview", empty)]).await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    let d = dsl(&conv.store);
    assert!(d.contains("No knowledge pages yet") && d.contains("b3_mem_empty"), "{d}");
    assert!(d.contains("No long-term pages or daily notes here yet. Recall records may still be available through search."));
    assert!(!d.contains("Long-term memory"), "no empty section headings");
}

// ------------------------------------------------------------- search

async fn searched(server: &Server, conv: &Conversation, query: &str) {
    host::input_changed("mem.query", query);
    let out = host::input_returned("mem.query", &conv.store);
    assert!(matches!(out, Outcome::Spawn(Job::MemorySearch(_))), "Enter runs the search: {out:?}");
    run(conv, out).await.ok();
    let _ = server;
}

#[tokio::test]
async fn search_sends_the_query_the_kind_and_the_limit_and_lists_the_hits() {
    let _g = serial();
    let hits = reply(PROPOSAL, "memory/search");
    let server = Server::start(vec![
        ("session/open", json!({"opened": recorded_open()})),
        ("memory/overview", proposal_overview()),
        ("memory/search", hits.clone()),
    ])
    .await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    searched(&server, &conv, "  steer queue ").await;
    assert_eq!(
        server.sent("memory/search")[0],
        json!({"profile_id": "dsflash", "context":{"session_id":conv.session_id()}, "query": "steer queue", "limit": 20}),
        "All sends no kinds; the query is trimmed"
    );
    let d = dsl(&conv.store);
    assert_eq!(host::state().mem.page, Page::Results);
    for want in ["3 results", "steer-queue", "Fix steer queue drop on reconnect", "Backoff for redelivery", "Knowledge", "Episode", "Document", "bank", "episodes", "octoscode"] {
        assert!(d.contains(want), "missing {want:?}");
    }
    // The server's trust field drives the chip: the episode AND the document.
    assert_eq!(d.matches("\"untrusted\"").count(), 2, "{d}");
    for i in 0..3 {
        assert!(d.contains(&format!("b3_mem_hit_{i}")), "hit {i}");
    }
    // The kind filter re-runs the search with `kinds`.
    let out = host::perform("b3.mem.kind.document", 0, &conv.store);
    assert!(matches!(out, Outcome::Spawn(Job::MemorySearch(_))), "{out:?}");
    run(&conv, out).await.ok();
    assert_eq!(server.sent("memory/search")[1]["kinds"], json!(["document"]));
    assert_eq!(host::state().mem.kind, KindFilter::Document);
    // A blank query never reaches the server (it refuses one with -32602).
    host::perform("b3.mem.clear", 0, &conv.store);
    host::input_changed("mem.query", "   ");
    assert_eq!(host::input_returned("mem.query", &conv.store), Outcome::Done);
    assert_eq!(server.sent("memory/search").len(), 2);
    assert_eq!(host::state().mem.page, Page::Overview, "clearing the search returns to the overview");
}

#[tokio::test]
async fn a_refused_search_explains_the_problem_and_the_next_step_never_the_raw_error() {
    let _g = serial();
    let server = Server::start(vec![
        ("session/open", json!({"opened": recorded_open()})),
        ("memory/overview", proposal_overview()),
        ("memory/search", runtime_unavailable()),
    ])
    .await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    searched(&server, &conv, "steer queue").await;
    let d = dsl(&conv.store);
    assert!(d.contains("Couldn't search memory."), "{d}");
    // The profile is confirmed, so runtime_unavailable means ITS runtime.
    let (_, problem, next) = memory::copy(&Failure { op: Op::Search, refusal: Refusal::NotRunning }, "dsflash");
    assert!(d.contains(&problem) && d.contains(&next), "{problem} / {next}");
    for raw in ["ProfileRuntime", "admin", "-32603", "runtime_unavailable", "dashboard"] {
        assert!(!d.contains(raw), "the raw server error leaked: {raw}");
    }
}

#[tokio::test]
async fn todays_server_refuses_search_for_the_scope_reason() {
    let _g = serial();
    let server = Server::start(vec![
        ("session/open", json!({"opened": recorded_open()})),
        ("memory/overview", reply(RECORDED, "memory/overview")),
        ("memory/search", runtime_unavailable()),
    ])
    .await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    searched(&server, &conv, "dentist").await;
    let d = dsl(&conv.store);
    let (_, problem, next) = memory::copy(&Failure { op: Op::Search, refusal: Refusal::Scope }, "dsflash");
    assert!(d.contains("Couldn't search memory.") && d.contains(&problem) && d.contains(&next), "{d}");
    assert!(!d.contains("ProfileRuntime") && !d.contains("admin"));
}

// --------------------------------------------------------- open a hit

#[tokio::test]
async fn opening_a_hit_loads_the_record_and_shows_untrusted_content_as_data() {
    let _g = serial();
    let server = Server::start(vec![
        ("session/open", json!({"opened": recorded_open()})),
        ("memory/overview", proposal_overview()),
        ("memory/search", reply(PROPOSAL, "memory/search")),
        ("memory/load", proposal_load(DOC_ID)),
        ("memory/load", proposal_load("episode:9f2c41d07e3b")),
        ("memory/load", proposal_load("bank:steer-queue")),
    ])
    .await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    searched(&server, &conv, "steer queue").await;
    // The document (hit 2): untrusted app content.
    let out = host::perform("b3.mem.open", 2, &conv.store);
    assert!(matches!(&out, Outcome::Spawn(Job::MemoryLoad(_, id)) if id == DOC_ID), "{out:?}");
    run(&conv, out).await.ok();
    assert_eq!(server.sent("memory/load")[0], json!({"profile_id": "dsflash", "context":{"session_id":conv.session_id()}, "count_visit":false, "id": DOC_ID}));
    assert_eq!(host::state().mem.page, Page::Record);
    let d = dsl(&conv.store);
    for want in ["Backoff for redelivery", "Document", "octoscode", "untrusted", "opened 3 times", "Applies to every redeliver attempt after a reconnect.", "Added from an app", "Octos reads it as data, never as instructions.", DOC_ID, "Results"] {
        assert!(d.contains(want), "missing {want:?}");
    }
    assert!(!d.contains("Markdown{"), "untrusted text is shown as data, never rendered as Markdown");
    // Back to the results, then the episode: its own lead.
    assert_eq!(host::perform("b3.mem.back", 0, &conv.store), Outcome::Done);
    assert_eq!(host::state().mem.page, Page::Results);
    run(&conv, host::perform("b3.mem.open", 1, &conv.store)).await.ok();
    let d = dsl(&conv.store);
    assert!(d.contains("From an earlier session") && d.contains("Octos reads it as data, never as instructions."), "{d}");
    // The knowledge record (trusted): its bank page, no data callout.
    host::perform("b3.mem.back", 0, &conv.store);
    run(&conv, host::perform("b3.mem.open", 0, &conv.store)).await.ok();
    let d = dsl(&conv.store);
    assert!(d.contains("Markdown{") && d.contains("Drains a snapshot first so the order stays stable."), "{d}");
    assert!(!d.contains("Added from an app") && !d.contains("From an earlier session"));
}

#[tokio::test]
async fn an_entity_row_opens_its_page() {
    let _g = serial();
    let server = Server::start(vec![
        ("session/open", json!({"opened": recorded_open()})),
        ("memory/overview", proposal_overview()),
        ("memory/entity", proposal_entity("steer-queue")),
    ])
    .await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    // The entities are name-sorted by the server: dsflash, octos-core, steer-queue.
    let out = host::perform("b3.mem.entity", 2, &conv.store);
    assert!(matches!(&out, Outcome::Spawn(Job::MemoryEntity(_, n)) if n == "steer-queue"), "{out:?}");
    run(&conv, out).await.ok();
    assert_eq!(server.sent("memory/entity")[0], json!({"profile_id": "dsflash", "context":{"session_id":conv.session_id()}, "name": "steer-queue"}));
    assert_eq!(host::state().mem.page, Page::Entity);
    let d = dsl(&conv.store);
    assert!(d.contains("Entity page") && d.contains("Markdown{") && d.contains("Backoff starts at 250 ms"), "{d}");
    assert_eq!(host::perform("b3.mem.back", 0, &conv.store), Outcome::Done);
    assert_eq!(host::state().mem.page, Page::Overview);
}

/// Memory is read here, never navigated from: a link keeps its text, an
/// autolink its address — and every other `>` (a quote, a comparison) stays.
#[tokio::test]
async fn a_page_keeps_its_text_but_not_its_links() {
    let _g = serial();
    let mut page = proposal_entity("steer-queue");
    page["content"] = json!("# steer-queue\n\n> Order matters: 2 > 1.\n\n- see <https://example.com/x>\n- [the docs](https://example.com/docs)\n");
    let server = Server::start(vec![
        ("session/open", json!({"opened": recorded_open()})),
        ("memory/overview", proposal_overview()),
        ("memory/entity", page),
    ])
    .await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    run(&conv, host::perform("b3.mem.entity", 2, &conv.store)).await.ok();
    let d = dsl(&conv.store);
    let body = d.split("b3_mem_ent_md := Markdown{").nth(1).expect("the page").split("\n").nth(1).unwrap_or("").to_owned();
    assert!(body.contains("> Order matters: 2 > 1."), "quotes and comparisons keep their `>`: {body}");
    assert!(body.contains("see https://example.com/x") && !body.contains("<https"), "an autolink is its address: {body}");
    assert!(body.contains("the docs") && !body.contains("](https://example.com/docs)"), "a link is its text: {body}");
}

#[tokio::test]
async fn show_all_opens_long_term_memory_and_says_when_the_server_cut_it() {
    let _g = serial();
    let mut big = proposal_overview();
    let line = "- Keep the steer queue's order stable across reconnects.\n";
    let kept: String = line.repeat(96 * 1024 / line.len() + 1).chars().take(96 * 1024).collect();
    big["overview"]["long_term"] = json!(kept);
    big["overview"]["long_term_truncated"] = json!(true);
    big["overview"]["long_term_total_bytes"] = json!(140 * 1024);
    let server = Server::start(vec![("session/open", json!({"opened": recorded_open()})), ("memory/overview", big)]).await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    assert_eq!(host::perform("b3.mem.lt", 0, &conv.store), Outcome::Done);
    assert_eq!(host::state().mem.page, Page::LongTerm);
    let d = dsl(&conv.store);
    assert!(d.contains("MEMORY.md"), "{}", &d[..400.min(d.len())]);
    assert!(d.contains("Showing the first 96 KB of 140 KB. The rest stays on the server."), "never a cut page shown as whole");
    // Said BEFORE the page is read (96 KB of it would bury a notice at the end).
    assert!(d.find("b3_mem_lt_cut").unwrap() < d.find("b3_mem_lt_full").unwrap(), "the notice leads the page");
}

// ----------------------------------------------------------- add a note

#[test]
fn a_note_is_one_document_record_the_server_accepts() {
    let _g = serial();
    let key = "0123456789abcdef";
    let ts = "2026-10-03T12:00:00Z";
    let r = memory::note_record("Backoff for redelivery", "Start at 250 ms, double up to 8 s.", ts, key).expect("a record");
    assert_eq!(
        r,
        json!({"id": "doc:octoscode:0123456789abcdef", "kind": "document", "source": "octoscode", "timestamp": ts,
               "title": "Backoff for redelivery", "abstract": "Start at 250 ms, double up to 8 s."}),
        "the doc:<source>: id the server requires; no trust (the server forces untrusted)"
    );
    // No title: the note's first line.
    let r = memory::note_record("  ", "Dentist on Tuesday\nat 9:30 with Dr. Lin.", ts, key).unwrap();
    assert_eq!(r["title"], "Dentist on Tuesday");
    // A long note: the abstract is its first 300 bytes on a char boundary,
    // the whole note rides as the body (search matches title + abstract).
    let long = "记".repeat(200); // 600 bytes
    let r = memory::note_record("t", &long, ts, key).unwrap();
    let a = r["abstract"].as_str().unwrap();
    assert!(a.len() <= 300 && long.starts_with(a) && a.chars().count() == 100, "{}", a.len());
    assert_eq!(r["body"], json!(long));
    // Past 16 KiB the server would cut the body silently: refused here.
    assert_eq!(memory::note_record("t", &"x".repeat(16 * 1024 + 1), ts, key), Err(Refusal::TooLong));
    assert!(memory::note_record("t", "   ", ts, key).is_err(), "an empty note is not sent");
    assert_eq!(
        memory::ingest_params("dsflash", json!({"id": "x"})),
        json!({"profile_id": "dsflash", "records": [{"id": "x"}]})
    );
}

#[tokio::test]
async fn add_note_writes_one_record_and_reports_the_receipt() {
    let _g = serial();
    let server = Server::start(vec![
        ("session/open", json!({"opened": recorded_open()})),
        ("memory/overview", proposal_overview()),
        ("memory/ingest", reply(PROPOSAL, "memory/ingest")),
    ])
    .await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    assert_eq!(host::perform("b3.mem.add", 0, &conv.store), Outcome::Done);
    assert_eq!(host::state().mem.page, Page::Add);
    let d = dsl(&conv.store);
    for want in ["Add a note", "Title", "Note", "Search matches the title and the start of the note.", "The agent finds this note when it searches memory. It is kept as data, never as an instruction, and is not added to long-term memory.", "Cancel", "Add to memory"] {
        assert!(d.contains(want), "missing {want:?}");
    }
    host::input_changed("mem.add.title", "Backoff for redelivery");
    host::input_changed("mem.add.note", "Start at 250 ms, double up to 8 s, reset after a successful send.");
    let out = host::perform("b3.mem.add.submit", 0, &conv.store);
    assert!(matches!(out, Outcome::Spawn(Job::MemoryIngest(_))), "{out:?}");
    run(&conv, out).await.ok();
    let sent = server.sent("memory/ingest");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["profile_id"], "dsflash");
    let rec = &sent[0]["records"][0];
    assert_eq!((rec["kind"].as_str(), rec["source"].as_str()), (Some("document"), Some("octoscode")));
    let id = rec["id"].as_str().unwrap();
    assert!(id.starts_with("doc:octoscode:") && id.len() == "doc:octoscode:".len() + 16, "{id}");
    assert_eq!(rec["title"], "Backoff for redelivery");
    let st = host::state();
    assert_eq!(st.mem.page, Page::Overview, "back to the overview");
    assert_eq!(st.mem.receipt, Some("Added to memory."));
    assert!(st.mem.add_note.is_empty(), "the form is cleared after a landed note");
    drop(st);
    assert!(dsl(&conv.store).contains("Added to memory."));
    assert_eq!(server.sent("memory/overview").len(), 2, "the overview is read again");
}

/// The walk's first capture of the form showed EMPTY fields under an armed
/// "Add to memory": the submit's initial visibility followed the live note,
/// so the first keystroke changed the DSL, the dialog remounted and rebuilt
/// both fields from their (empty) snapshots. Typing must never change the
/// lowered dialog (the kit's rule); the submit arms through `visibility`.
#[tokio::test]
async fn typing_a_note_or_a_query_never_rebuilds_the_fields() {
    let _g = serial();
    let server = Server::start(vec![("session/open", json!({"opened": recorded_open()})), ("memory/overview", proposal_overview())]).await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    let before = dsl(&conv.store);
    host::input_changed("mem.query", "steer");
    assert_eq!(dsl(&conv.store), before, "typing a query changes nothing until Enter");
    host::perform("b3.mem.add", 0, &conv.store);
    let form = dsl(&conv.store);
    host::input_changed("mem.add.title", "Dentist");
    host::input_changed("mem.add.note", "Tuesday 9:30");
    assert_eq!(dsl(&conv.store), form, "typing in the form changes nothing in the lowered dialog");
    let vis: std::collections::BTreeMap<String, bool> = host::live_visibility(&conv.store).into_iter().collect();
    assert_eq!((vis["b3_mem_add_submit_on"], vis["b3_mem_add_submit_off"]), (true, false), "the submit arms live");
}

#[test]
fn the_receipt_follows_the_server_counts() {
    let _g = serial();
    let r = |i: u64, u: u64, n: u64| memory::parse_ingest(&json!({"inserted": i, "updated": u, "unchanged": n, "vectors_stored": 0, "embedded": 0}));
    assert_eq!(r(1, 0, 0).map(|x| (x.inserted, x.updated, x.unchanged)), Some((1, 0, 0)));
    assert_eq!(memory::receipt(&r(1, 0, 0).unwrap()), "Added to memory.");
    assert_eq!(memory::receipt(&r(0, 1, 0).unwrap()), "Updated in memory.");
    assert_eq!(memory::receipt(&r(0, 0, 1).unwrap()), "Already in memory.");
}

#[tokio::test]
async fn a_refused_note_keeps_the_draft() {
    let _g = serial();
    let server = Server::start(vec![
        ("session/open", json!({"opened": recorded_open()})),
        ("memory/overview", proposal_overview()),
        ("memory/ingest", runtime_unavailable()),
    ])
    .await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    host::perform("b3.mem.add", 0, &conv.store);
    host::input_changed("mem.add.title", "Dentist");
    host::input_changed("mem.add.note", "Tuesday 9:30, Dr. Lin.");
    run(&conv, host::perform("b3.mem.add.submit", 0, &conv.store)).await.ok();
    let st = host::state();
    assert_eq!(st.mem.page, Page::Add, "the form stays");
    assert_eq!((st.mem.add_title.as_str(), st.mem.add_note.as_str()), ("Dentist", "Tuesday 9:30, Dr. Lin."));
    drop(st);
    let d = dsl(&conv.store);
    assert!(d.contains("Couldn't add the note.") && d.contains("Tuesday 9:30, Dr. Lin."), "{d}");
    assert!(!d.contains("ProfileRuntime"));
}

#[test]
fn add_note_needs_a_confirmed_profile_and_the_ingest_method() {
    let _g = serial();
    let mut st = memory::MemState::default();
    st.confirmed = true;
    assert!(memory::can_add(&st, &store_with(ALL_MEMORY)));
    assert!(!memory::can_add(&st, &store_with(&["memory/overview", "memory/search"])), "ingest not advertised");
    st.confirmed = false;
    assert!(!memory::can_add(&st, &store_with(ALL_MEMORY)), "the server did not say whose memory this is");
}

// ------------------------------------------------- refusals, bounded copy

#[test]
fn refusals_are_classified_from_the_server_code_and_kind() {
    use octos_core::ui_protocol::RpcError;
    use octoscode_client::ClientError;
    let rpc = |code: i64, kind: Option<&str>| ClientError::Rpc {
        method: "memory/search".into(),
        error: RpcError { code, message: "raw".into(), data: kind.map(|k| json!({"kind": k})) },
    };
    assert_eq!(memory::classify(&rpc(-32603, Some("runtime_unavailable")), false), Refusal::Scope);
    assert_eq!(memory::classify(&rpc(-32603, Some("runtime_unavailable")), true), Refusal::NotRunning);
    assert_eq!(memory::classify(&rpc(-32170, Some("not_found")), true), Refusal::NotFound);
    assert_eq!(memory::classify(&rpc(-32003, Some("forbidden")), true), Refusal::Forbidden);
    assert_eq!(memory::classify(&rpc(-32602, None), true), Refusal::Invalid);
    assert_eq!(memory::classify(&rpc(-32601, None), true), Refusal::Unavailable);
    assert_eq!(memory::classify(&rpc(-32603, None), true), Refusal::Failed);
    let gone = ClientError::Transport { method: "memory/search".into(), reason: "closed".into() };
    assert_eq!(memory::classify(&gone, true), Refusal::Offline);
    // Every class: a lead, one problem sentence, one next step — bounded.
    for refusal in [
        Refusal::Scope,
        Refusal::OtherProfile("admin".into()),
        Refusal::NotRunning,
        Refusal::NotFound,
        Refusal::Forbidden,
        Refusal::Invalid,
        Refusal::Unavailable,
        Refusal::Offline,
        Refusal::Failed,
        Refusal::TooLong,
    ] {
        for op in [Op::Read, Op::Search, Op::Open, Op::Page, Op::Add] {
            let (lead, problem, next) = memory::copy(&Failure { op, refusal: refusal.clone() }, "dsflash");
            assert!(lead.starts_with("Couldn't"), "{lead}");
            assert!(!problem.is_empty() && problem.len() <= 140, "{refusal:?}: {problem}");
            assert!(!next.is_empty() && next.len() <= 100, "{refusal:?}: {next}");
            assert!(!problem.contains("raw") && !next.contains("raw"));
        }
    }
}

#[tokio::test]
async fn a_reply_for_another_profile_is_refused() {
    let _g = serial();
    let mut other = proposal_overview();
    other["profile_id"] = json!("admin");
    let server = Server::start(vec![("session/open", json!({"opened": recorded_open()})), ("memory/overview", other)]).await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    let d = dsl(&conv.store);
    let (_, problem, _) = memory::copy(&Failure { op: Op::Read, refusal: Refusal::OtherProfile("admin".into()) }, "dsflash");
    assert!(d.contains("Couldn't read memory.") && d.contains(&problem), "{d}");
    assert!(!d.contains("Prefers Rust 2024"), "another profile's memory is never shown as this one's");
}

// ------------------------------------------------------- theme, language

#[tokio::test]
async fn memory_follows_the_app_theme_and_reads_in_chinese() {
    let _g = serial();
    let server = Server::start(vec![("session/open", json!({"opened": recorded_open()})), ("memory/overview", proposal_overview())]).await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    {
        // The theme is process-wide; this binary's tests run under `serial()`.
        octoscode_module::screens::theme::set_preference("light");
        let light = dsl(&conv.store);
        assert!(light.contains("#ffffffff"), "the light surface");
        octoscode_module::screens::theme::set_preference("dark");
        let dark = dsl(&conv.store);
        octoscode_module::screens::theme::set_preference("light");
        assert!(dark.contains("#1c1f22ff"), "frame 11's dark surface");
        let frame = dark.split("b3_dialog := DesignSurface {").nth(1).expect("the dialog surface");
        assert!(!frame.lines().nth(1).unwrap_or("").contains("draw_bg.color: #ffffffff"), "the dialog is not white in dark");
        assert!(!dark.contains("#fff3e0ff"), "no light amber fill in dark");
    }
    i18n::set_language(Lang::Zh);
    let zh = dsl(&conv.store);
    i18n::set_language(Lang::En);
    for want in ["记忆", "服务器配置档案：dsflash", "搜索记忆", "长期记忆", "实体", "添加笔记", "显示全部", "今天"] {
        assert!(zh.contains(want), "zh: missing {want:?}");
    }
    assert!(zh.contains("NotoSansSC") || zh.contains("Noto"), "CJK text in Noto Sans SC");
}

#[tokio::test]
async fn an_old_overview_cannot_repopulate_memory_after_a_session_switch() {
    let _g = serial();
    let mut body = proposal_overview(); body["__delay_ms"] = json!(80);
    let server = Server::start(vec![("session/open", json!({"opened": recorded_open()})), ("memory/overview",body)]).await;
    let conv = connect(&server, ALL_MEMORY).await;
    let out = host::perform("b3.open.memory",0,&conv.store);
    let old = conv.clone();
    let pending = tokio::spawn(async move { run(&old, out).await });
    for _ in 0..50 { if !server.sent("memory/overview").is_empty() { break; } tokio::time::sleep(std::time::Duration::from_millis(2)).await; }
    assert!(!server.sent("memory/overview").is_empty());
    memory::invalidate(); conv.adopt_profile("other".into());
    pending.await.unwrap().unwrap();
    let state = host::state();
    assert!(state.mem.overview.is_none() && !state.mem.confirmed && state.mem.profile.is_empty());
}

#[tokio::test]
async fn a_matching_profile_with_the_wrong_session_echo_is_refused() {
    let _g = serial();
    let mut body = proposal_overview(); body["scope"] = json!({"kind":"profile","namespace":null,"session_id":"dsflash:api:other"});
    let server = Server::start(vec![("session/open", json!({"opened": recorded_open()})), ("memory/overview",body)]).await;
    let conv = connect(&server, ALL_MEMORY).await;
    open_memory(&conv).await;
    let state = host::state();
    assert!(state.mem.overview.is_none() && !state.mem.confirmed);
}
