//! A28 — parity row 23: word-level change marks and per-file syntax colours
//! in the diff review, bounded so a large preview falls back to plain text,
//! and the phone review as a full-screen sheet (design board 4 frames 1, 1b,
//! 2 — design/stage-a/phase4-new4/README.md, "Row 23").
//!
//! The web: `features/review/diff-presentation.ts:26-160` (`canDecorateDiff`,
//! `diffLanguage`, `decorateDiffHunk`, `changedWords`), its unit tests
//! `diff-presentation.test.ts` (ported first, case for case), the dialog
//! `DiffReviewDialog.tsx` (`plainNotice`), its CSS (`DiffReviewDialog.module.css`
//! `.changedWord`, `app/styles.css:1615-2030` + the compact block :2155-2200)
//! and the e2e intent of `e2e/diff-review.spec.ts:164` (large previews stay
//! complete, plain) and `:188` (a file without a grammar stays plain but keeps
//! its word marks).
//!
//! The expected word marks were produced by the web's OWN `changedWords` /
//! `annotateTokens` (diff-presentation.ts run by node on these exact lines).
//!
//! Production path: the review opens / reads / lowers through the same
//! functions the clicks run (`board3::host::{perform, run, lower_open}`)
//! against a fake server answering `diff/preview/get` with the synthetic
//! fixture `a28-diff-words-synthetic.jsonl` (tools/fixtures/
//! a28_diff_words_fixture.py) — the same previews the click walk
//! (tools/walk/a28_diff_words.py) reads from replay_serve.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use octos_core::ui_protocol::{DiffPreviewFile, DiffPreviewLine};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::highlight::Tok;
use octoscode_module::screens::board3::diff_review as dr;
use octoscode_module::screens::board3::diff_words as dw;
use octoscode_module::screens::board3::host::{self, Job, Outcome};
use octoscode_module::screens::review;

const WORDS: &str = "01920000-0000-7000-8000-0000000000f1";
const LARGE: &str = "01920000-0000-7000-8000-0000000000f2";

// ------------------------------------------------------------------ helpers

fn line(kind: &str, content: &str) -> DiffPreviewLine {
    serde_json::from_value(json!({"kind": kind, "content": content})).expect("a line")
}

fn files(lines: Vec<DiffPreviewLine>) -> Vec<DiffPreviewFile> {
    serde_json::from_value(json!([{"path": "large.ts", "status": "modified", "hunks": [{"header": "@@", "lines": lines}]}]))
        .expect("files")
}

/// The changed pieces of each decorated line (what the web's
/// `[class*=changedWord]` spans hold).
fn changed(lines: &[DiffPreviewLine], lang: Option<&str>) -> Vec<Vec<String>> {
    dw::decorate_hunk(lines, lang)
        .into_iter()
        .map(|row| {
            // Adjacent changed pieces of ONE range read as one word run.
            let mut out: Vec<String> = Vec::new();
            let mut open = false;
            for t in row {
                if t.changed {
                    if open {
                        out.last_mut().unwrap().push_str(&t.text);
                    } else {
                        out.push(t.text.clone());
                    }
                }
                open = t.changed;
            }
            out
        })
        .collect()
}

fn fixture(name: &str) -> Value {
    let path = format!("{}/../octoscode-client/tests/fixtures/a28-diff-words-synthetic.jsonl", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).expect("the A28 fixture");
    text.lines()
        .map(|l| serde_json::from_str::<Value>(l).expect("json"))
        .find(|f| f["fixture"] == name)
        .map(|f| f["body"].clone())
        .expect("the fixture frame")
}

fn fixture_lines(name: &str, file: usize) -> Vec<String> {
    fixture(name)["preview"]["files"][file]["hunks"][0]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["content"].as_str().unwrap().to_owned())
        .collect()
}

/// One DSL node: id, kind, the `text:` literal (decoded), the property text,
/// and its children (the line-based DSL: `id := Kind {` opens, a lone `}`
/// closes, a one-line `id := Kind{…}` is a leaf).
#[derive(Debug, Default, Clone)]
struct Node {
    id: String,
    kind: String,
    text: Option<String>,
    props: String,
    children: Vec<Node>,
}

fn decode(lit: &str) -> String {
    // The inverse of Rust's `{:?}` for the escapes it emits.
    let mut out = String::new();
    let mut it = lit.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('0') => out.push('\0'),
            Some('u') => {
                let hex: String = it.by_ref().skip(1).take_while(|c| *c != '}').collect();
                out.push(char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap());
            }
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

/// The `text: "…"` literal of a property string.
fn text_of(props: &str) -> Option<String> {
    let at = props.find("text: \"")? + "text: \"".len();
    let b = props[at..].as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Some(decode(&props[at..at + i])),
            _ => i += 1,
        }
    }
    None
}

fn parse(dsl: &str) -> Node {
    let mut stack = vec![Node { id: "root".into(), ..Default::default() }];
    for l in dsl.lines() {
        let t = l.trim();
        if t == "}" {
            let n = stack.pop().unwrap();
            stack.last_mut().unwrap().children.push(n);
            continue;
        }
        if let Some((id, rest)) = t.split_once(" := ") {
            if !id.contains(' ') && !id.is_empty() {
                let kind: String = rest.chars().take_while(|c| *c != '{' && *c != ' ').collect();
                let opens = rest.ends_with('{') && !rest.ends_with("{}");
                let leaf_props = rest.find('{').map(|i| rest[i + 1..].trim_end_matches('}').to_owned()).unwrap_or_default();
                let node = Node { id: id.to_owned(), kind, text: text_of(rest), props: leaf_props, children: vec![] };
                if opens && rest.matches('{').count() > rest.matches('}').count() {
                    stack.push(node);
                } else {
                    stack.last_mut().unwrap().children.push(node);
                }
                continue;
            }
        }
        // A property line of the open node.
        let top = stack.last_mut().unwrap();
        top.props.push_str(t);
        top.props.push('\n');
        if top.text.is_none() {
            top.text = text_of(t);
        }
    }
    assert_eq!(stack.len(), 1, "balanced DSL");
    stack.pop().unwrap()
}

fn find<'a>(n: &'a Node, id: &str) -> Option<&'a Node> {
    if n.id == id {
        return Some(n);
    }
    n.children.iter().find_map(|c| find(c, id))
}

fn all<'a>(n: &'a Node, out: &mut Vec<&'a Node>) {
    out.push(n);
    for c in &n.children {
        all(c, out);
    }
}

/// `<line>_c<k>_<class>` (inside a mark `<line>_w<m>_c<k>_<class>`): the
/// code runs of one line, in order, with their class and the mark they sit
/// in — read from the DSL's NESTING, and the id must agree with it.
fn runs(root: &Node, lid: &str) -> Vec<(String, String, Option<String>)> {
    let code = find(root, &format!("{lid}_code")).unwrap_or_else(|| panic!("{lid}_code"));
    let mut out = Vec::new();
    fn walk(n: &Node, lid: &str, mark: Option<&str>, out: &mut Vec<(String, String, Option<String>)>) {
        let owner = mark.unwrap_or(lid);
        for c in &n.children {
            if let Some(rest) = c.id.strip_prefix(&format!("{owner}_c")) {
                let class = rest.split_once('_').map(|(_, cl)| cl.to_owned()).unwrap_or_default();
                out.push((c.text.clone().unwrap_or_default(), class, mark.map(str::to_owned)));
            } else if mark.is_none() && c.id.starts_with(&format!("{lid}_w")) {
                walk(c, lid, Some(&c.id), out);
            } else {
                panic!("{}: not a run of {owner}", c.id);
            }
        }
    }
    walk(code, lid, None, &mut out);
    out
}

fn line_text(root: &Node, lid: &str) -> String {
    runs(root, lid).into_iter().map(|(t, _, _)| t).collect()
}

/// The words each mark of a line covers, in order.
fn marks(root: &Node, lid: &str) -> Vec<String> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (t, _, m) in runs(root, lid) {
        if let Some(m) = m {
            match out.last_mut() {
                Some((id, s)) if *id == m => s.push_str(&t),
                _ => out.push((m, t)),
            }
        }
    }
    out.into_iter().map(|(_, s)| s).collect()
}

fn classes(root: &Node, prefix: &str) -> std::collections::BTreeSet<String> {
    let mut nodes = Vec::new();
    all(root, &mut nodes);
    nodes
        .iter()
        .filter(|n| n.id.starts_with(prefix) && n.id.contains("_c"))
        .filter_map(|n| {
            let rest = n.id.strip_prefix(prefix)?;
            let (_, after) = rest.split_once("_c")?;
            after.split_once('_').map(|(_, c)| c.to_owned())
        })
        .collect()
}

fn ids(root: &Node) -> Vec<String> {
    let mut nodes = Vec::new();
    all(root, &mut nodes);
    nodes.iter().map(|n| n.id.clone()).collect()
}

fn prop(n: &Node, key: &str) -> Option<String> {
    let at = n.props.find(&format!("{key}: "))? + key.len() + 2;
    Some(n.props[at..].split([' ', '\n']).next().unwrap_or("").to_owned())
}

// --------------------------------------------- the web's unit tests, ported

/// diff-presentation.test.ts:12 — "does not pair unequal blocks or cross
/// context boundaries" (and a pair sharing too little stays unmarked).
#[test]
fn does_not_pair_unequal_blocks_or_cross_context_boundaries() {
    for lines in [
        vec![line("removed", "return oldName;"), line("added", "return newName;"), line("added", "return extra;")],
        vec![line("removed", "return oldName;"), line("context", "// break"), line("added", "return newName;")],
        vec![line("removed", "old unrelated text"), line("added", "another replacement")],
    ] {
        assert!(
            dw::decorate_hunk(&lines, None).iter().flatten().all(|t| !t.changed),
            "{:?}",
            lines.iter().map(|l| &l.content).collect::<Vec<_>>()
        );
    }
    // The control: the same pair alone IS marked (equal 1:1 block).
    let pair = [line("removed", "return oldName;"), line("added", "return newName;")];
    assert_eq!(changed(&pair, None), vec![vec!["oldName".to_owned()], vec!["newName".to_owned()]]);
}

/// diff-presentation.test.ts:37 — "bounds both total preview work and
/// quadratic word comparison": 400 lines / 40,000 characters / 2,000 per line
/// (UTF-16 code units, the web's `.length`), and a word LCS of at most 160
/// tokens per side.
#[test]
fn bounds_both_total_preview_work_and_quadratic_word_comparison() {
    let n = |count: usize, content: &str| (0..count).map(|_| line("added", content)).collect::<Vec<_>>();
    assert!(dw::can_decorate(&files(n(400, "a"))), "400 lines decorate");
    assert!(!dw::can_decorate(&files(n(401, "a"))), "401 lines do not");
    assert!(dw::can_decorate(&files(vec![line("added", &"a".repeat(2_000))])), "a 2,000-character line decorates");
    assert!(!dw::can_decorate(&files(vec![line("added", &"a".repeat(2_001))])), "a 2,001-character line does not");
    assert!(dw::can_decorate(&files(n(40, &"a".repeat(1_000)))), "40,000 characters decorate");
    assert!(!dw::can_decorate(&files(n(40, &"a".repeat(1_001)))), "40,040 characters do not");
    // `.length` counts UTF-16 units: 1,001 emoji are 2,002.
    assert!(dw::can_decorate(&files(vec![line("added", &"😀".repeat(1_000))])));
    assert!(!dw::can_decorate(&files(vec![line("added", &"😀".repeat(1_001))])));
    assert_eq!(dw::js_len("😀a名"), 4);
    // The LCS bound: more than 160 words on a side are never compared.
    let lines = [line("removed", &format!("{}before", "a + ".repeat(100))), line("added", &format!("{}after", "a + ".repeat(100)))];
    assert!(dw::decorate_hunk(&lines, None).iter().flatten().all(|t| !t.changed));
    assert_eq!(dw::changed_words(&format!("{}before", "a + ".repeat(100)), &format!("{}after", "a + ".repeat(100))), None);
}

/// diff-presentation.test.ts:76 — "uses the existing lazy grammar and
/// preserves multiline context on each side": each side is highlighted with
/// its context, so a comment opened in a context line colours the removed
/// AND the added line; an added line after the context is lexed as code.
#[test]
fn uses_the_grammar_and_preserves_multiline_context_on_each_side() {
    let lines = [
        line("context", "/* starts here"),
        line("removed", "old comment"),
        line("added", "new comment"),
        line("context", "ends here */"),
        line("added", "const ready = true;"),
    ];
    let d = dw::decorate_hunk(&lines, Some("ts"));
    assert!(d[1].iter().all(|t| t.tok == Some(Tok::Comment)), "{:?}", d[1]);
    assert!(d[2].iter().all(|t| t.tok == Some(Tok::Comment)), "{:?}", d[2]);
    assert!(d[4].iter().any(|t| t.tok == Some(Tok::Keyword)), "{:?}", d[4]);
    assert_eq!(dw::diff_language("src/App.TSX").as_deref(), Some("tsx"));
    assert_eq!(dw::diff_language("/home/user/.zshrc").as_deref(), Some("sh"));
    assert_eq!(dw::diff_language("C:\\work\\.bash_profile").as_deref(), Some("sh"));
    assert_eq!(dw::diff_language("Makefile"), None);
    assert_eq!(dw::diff_language("config/octos.conf").as_deref(), Some("conf"));
}

/// diff-presentation.test.ts:105 — "never interprets source as markup or
/// drops embedded line endings": every line's tokens join to its exact text.
#[test]
fn never_interprets_source_or_drops_embedded_line_endings() {
    let contents = ["const html = \"<img src=x onerror=alert(1)>\";", "a\r\nb", "\t", "", "e\u{301}😀\u{2028}end", "a\rb"];
    let lines: Vec<_> = contents.iter().map(|c| line("added", c)).collect();
    let d = dw::decorate_hunk(&lines, Some("ts"));
    let joined: Vec<String> = d.iter().map(|row| row.iter().map(|t| t.text.as_str()).collect()).collect();
    assert_eq!(joined, contents);
    // A side holding a line break is not highlighted (shiki splits it, the
    // line count no longer matches: the web shows that side verbatim).
    assert!(d[0].iter().all(|t| t.tok.is_none()), "{:?}", d[0]);
}

/// The web's own changed words for the board fixture and the e2e fixture
/// (computed by diff-presentation.ts under node on these lines).
#[test]
fn word_marks_match_the_web_on_every_fixture() {
    let steer: Vec<_> = fixture("words")["preview"]["files"][0]["hunks"][0]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| serde_json::from_value::<DiffPreviewLine>(l.clone()).unwrap())
        .collect();
    let got = changed(&steer, Some("rs"));
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(got[2], s(&["Duration::from_millis", "500"]));
    assert_eq!(got[3], s(&["self.backoff.next", "attempt"]));
    assert_eq!(got[5], s(&["send"]));
    assert_eq!(got[6], s(&["send_with_retry", ", &self.backoff"]));
    for i in [0, 1, 4, 7, 8, 9, 10, 11] {
        assert!(got[i].is_empty(), "line {i}: {:?}", got[i]);
    }
    let pair = |a: &str, b: &str, lang: Option<&str>| changed(&[line("removed", a), line("added", b)], lang);
    assert_eq!(pair("steer.retry.delay_ms = 500", "steer.retry.delay_ms = 250", Some("conf")), vec![s(&["500"]), s(&["250"])]);
    assert_eq!(pair("Retries happen immediately.", "Backoff doubles up to 30 s.", Some("md")), vec![s(&[]), s(&[])]);
    assert_eq!(pair("version = \"0.24.0\"", "version = \"0.24.1\"", Some("toml")), vec![s(&["0"]), s(&["1"])]);
    // e2e/diff-review.spec.ts:56 — CJK words, an emoji, a tab, trailing spaces.
    assert_eq!(
        pair("\tconst 名称 = \"旧值😀\"; return false;  ", "\tconst 名称 = \"新值🌏\"; return true;  ", Some("ts")),
        vec![s(&["旧值😀", "false"]), s(&["新值🌏", "true"])]
    );
    // \p{M}: a combining mark belongs to its word (`e` + U+0301).
    assert_eq!(pair("let accent = cafe\u{301};", "let accent = cafe;", None), vec![s(&["cafe\u{301}"]), s(&["cafe"])]);
}

// ------------------------------------------------ the production path (DSL)

#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    Fixture,
}

struct Server {
    base_url: String,
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Server {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        let _ = Answer::Fixture;
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
                let seen = seen2.clone();
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
                        let session = p["session_id"].as_str().unwrap_or("a28:main").to_owned();
                        let result = match method.as_str() {
                            "session/open" => json!({"opened": {
                                "session_id": session, "active_profile_id": "a28",
                                "workspace_root": "/home/user/src/octos",
                                "cursor": {"stream": session, "seq": 1},
                                "capabilities": {
                                    "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                                    "capabilities_schema_version": 1,
                                    "supported_methods": ["session/open", "session/list", "session/hydrate", "turn/start",
                                        "turn/interrupt", "diff/preview/get"],
                                    "supported_notifications": ["turn/started", "turn/completed"],
                                    "supported_features": []
                                }
                            }}),
                            "session/list" => json!({"sessions": []}),
                            "diff/preview/get" => {
                                let name = if p["preview_id"] == LARGE { "large" } else { "words" };
                                let mut body = fixture(name);
                                body["preview"]["session_id"] = json!(session);
                                body
                            }
                            _ => json!({}),
                        };
                        let _ = tx.send(json!({"jsonrpc": "2.0", "id": v["id"], "result": result}).to_string());
                    }
                });
            }
        });
        Self { base_url, seen }
    }

    fn reads(&self) -> usize {
        self.seen.lock().unwrap().iter().filter(|(m, _)| m == "diff/preview/get").count()
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
    let (conv, mut events) = Conversation::connect(&server.base_url, "dummy", "a28", None, None).expect("connect");
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

/// Open the review on `preview` (the approval's / the header's path:
/// `review::set_preview_id` + `b3.open.diff_review`), run its ONE read, and
/// return the lowered dialog.
async fn open(conv: &Conversation, preview: &str, frame: (f64, f64)) -> Node {
    host::set_frame(frame.0, frame.1);
    review::set_preview_id(preview.into());
    let job = match host::perform("b3.open.diff_review", 0, &conv.store) {
        Outcome::Spawn(j) => j,
        other => panic!("expected the read, got {other:?}"),
    };
    assert!(matches!(job, Job::DiffReviewLoad(_)));
    host::run(job, conv).await.expect("diff/preview/get");
    parse(&host::lower_open(&conv.store).expect("the review is open").dsl)
}

/// Row 23 on the production path: per-file syntax runs on context, removed
/// and added lines; word marks only in the equal 1:1 blocks, over the web's
/// changed words; none in the unequal block nor on the < 25 % pair; the
/// `.conf` file (no grammar) is plain text that keeps its word marks; every
/// line whole.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_review_draws_syntax_runs_and_word_marks() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    let root = open(&conv, WORDS, (990.0, 603.0)).await;
    assert_eq!(server.reads(), 1, "ONE diff/preview/get");
    // Every line is whole: its runs join to the wire text.
    for (fi, file) in fixture("words")["preview"]["files"].as_array().unwrap().iter().enumerate() {
        for (li, l) in file["hunks"][0]["lines"].as_array().unwrap().iter().enumerate() {
            let lid = format!("b3_diff_file_{fi}_h0_l{li}");
            assert_eq!(line_text(&root, &lid), l["content"].as_str().unwrap(), "{lid}");
        }
    }
    // Syntax per file, on every kind of line (context l0, removed l2, added l3).
    let class = |lid: &str| runs(&root, lid).into_iter().map(|(t, c, _)| (t, c)).collect::<Vec<_>>();
    assert!(class("b3_diff_file_0_h0_l0").contains(&("fn".into(), "kw".into())), "{:?}", class("b3_diff_file_0_h0_l0"));
    assert!(class("b3_diff_file_0_h0_l0").contains(&("redeliver".into(), "fn".into())));
    assert!(class("b3_diff_file_0_h0_l2").contains(&("let".into(), "kw".into())));
    assert!(class("b3_diff_file_0_h0_l3").contains(&("next".into(), "fn".into())));
    assert!(class("b3_diff_file_0_h0_l10").contains(&("Ok".into(), "cn".into())));
    let kw = find(&root, &runs_id(&root, "b3_diff_file_0_h0_l0", "fn")).unwrap();
    assert!(kw.props.contains(Tok::Keyword.color(false)), "a keyword run carries the keyword colour: {}", kw.props);
    // Word marks: the equal blocks only, over the web's words.
    assert_eq!(marks(&root, "b3_diff_file_0_h0_l2"), ["Duration::from_millis", "500"]);
    assert_eq!(marks(&root, "b3_diff_file_0_h0_l3"), ["self.backoff.next", "attempt"]);
    assert_eq!(marks(&root, "b3_diff_file_0_h0_l5"), ["send"]);
    assert_eq!(marks(&root, "b3_diff_file_0_h0_l6"), ["send_with_retry", ", &self.backoff"]);
    for li in [0, 1, 4, 7, 8, 9, 10, 11] {
        assert!(marks(&root, &format!("b3_diff_file_0_h0_l{li}")).is_empty(), "line {li} has no mark");
    }
    // The mark's fill: the success colour at 22 % on added rows, the error
    // colour at 20 % on removed rows (over the row tint).
    let w_add = find(&root, "b3_diff_file_0_h0_l3_w0").expect("a mark on the added line");
    let w_rem = find(&root, "b3_diff_file_0_h0_l2_w0").expect("a mark on the removed line");
    assert!(w_add.props.contains(dr::MARK_ADDED), "{}", w_add.props);
    assert!(w_rem.props.contains(dr::MARK_REMOVED), "{}", w_rem.props);
    assert_eq!(dr::MARK_ADDED, dr::mark_fill(true), "the composited fill");
    assert_eq!(dr::MARK_REMOVED, dr::mark_fill(false));
    // The .conf file: no grammar -> plain runs only, word marks kept
    // (e2e/diff-review.spec.ts:188).
    assert_eq!(classes(&root, "b3_diff_file_1_"), ["tx".to_owned()].into_iter().collect());
    assert_eq!(marks(&root, "b3_diff_file_1_h0_l0"), ["500"]);
    assert_eq!(marks(&root, "b3_diff_file_1_h0_l1"), ["250"]);
    // The < 25 % pair keeps the line tint only.
    assert!(marks(&root, "b3_diff_file_2_h0_l1").is_empty() && marks(&root, "b3_diff_file_2_h0_l2").is_empty());
    // Decorated: no plain-text note.
    assert!(find(&root, "b3_diff_plain_note").is_none());
    host::close();
}

fn runs_id(root: &Node, lid: &str, text: &str) -> String {
    let code = find(root, &format!("{lid}_code")).unwrap();
    let mut nodes = Vec::new();
    all(code, &mut nodes);
    nodes
        .iter()
        .find(|n| n.text.as_deref() == Some(text) && n.id.starts_with(lid) && n.id.contains("_c"))
        .map(|n| n.id.clone())
        .unwrap()
}

/// The bound (`canDecorateDiff`): past 400 lines NOTHING is decorated — no
/// run, no mark, no syntax colour anywhere, even in the file that has a
/// grammar and a pair that would be marked — the note says so, and every line
/// is still drawn (never truncated; e2e/diff-review.spec.ts:164).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_preview_past_the_bound_is_plain_and_complete() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    let root = open(&conv, LARGE, (990.0, 603.0)).await;
    let note = find(&root, "b3_diff_plain_note").expect("the plain-text note");
    assert_eq!(note.text.as_deref(), Some(dr::PLAIN_NOTE));
    assert_eq!(dr::PLAIN_NOTE, "Large preview shown as plain text. All lines are included.");
    let all_ids = ids(&root);
    assert!(!all_ids.iter().any(|i| i.contains("_c") && i.starts_with("b3_diff_file_") && i.contains("_l")), "no run");
    assert!(!all_ids.iter().any(|i| i.starts_with("b3_diff_file_") && i.contains("_w")), "no word mark");
    for t in [Tok::Keyword, Tok::String, Tok::Constant, Tok::Function, Tok::Comment] {
        let c = t.color(false);
        let mut nodes = Vec::new();
        all(&root, &mut nodes);
        assert!(!nodes.iter().any(|n| n.id.starts_with("b3_diff_file_") && n.props.contains(c)), "no {t:?} colour");
    }
    // Every line, in order, in the hunk's plain blocks.
    for (fi, name) in [(0usize, "Cargo.lock"), (1, "Cargo.toml")] {
        let want = fixture_lines("large", fi).join("\n");
        let mut got = Vec::new();
        let mut b = 0;
        while let Some(block) = find(&root, &format!("b3_diff_file_{fi}_h0_b{b}_code")) {
            got.push(block.text.clone().unwrap_or_default());
            b += 1;
        }
        assert!(b > 0, "{name}: plain blocks");
        assert_eq!(got.join("\n"), want, "{name}: every line, in order");
    }
    assert_eq!(fixture_lines("large", 0).len(), 520);
    host::close();
}

/// `canDecorateDiff` runs over the WHOLE preview: one 2,001-character line in
/// one file turns decoration off for every file (all or nothing).
#[test]
fn the_bound_is_all_or_nothing_across_files() {
    let words: Vec<DiffPreviewFile> = serde_json::from_value(fixture("words")["preview"]["files"].clone()).unwrap();
    assert!(dw::decorate_preview(&words).is_some());
    let mut over = words.clone();
    let long: DiffPreviewFile = serde_json::from_value(json!({"path": "min.js", "status": "added", "hunks": [{"header": "@@",
        "lines": [{"kind": "added", "content": "x".repeat(2_001), "new_line": 1}]}]}))
    .unwrap();
    over.push(long);
    assert!(dw::decorate_preview(&over).is_none(), "one long line anywhere -> nothing decorated");
}

/// The source never escapes its `text:` literal: a hostile line (quotes,
/// braces, a fake widget, a line break) decodes back to itself and adds no
/// node.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hostile_source_stays_text_in_the_dsl() {
    let _g = lock();
    let hostile = "x\" }\nb3_pwned := Label{text: \"pwned\"} \\ {";
    let lines = [line("removed", hostile), line("added", "plain")];
    let d = dw::decorate_hunk(&lines, Some("rs"));
    let joined: String = d[0].iter().map(|t| t.text.as_str()).collect();
    assert_eq!(joined, hostile);
    let st = dr::DiffReviewState {
        preview_id: Some(WORDS.into()),
        result: Some(serde_json::from_value(json!({"status": "ready", "source": "pending_store", "preview": {
            "session_id": "s", "preview_id": WORDS, "files": [{"path": "evil.rs", "status": "modified", "hunks": [{
                "header": "@@", "lines": [{"kind": "removed", "content": hostile, "old_line": 1},
                                          {"kind": "added", "content": "plain", "new_line": 1}]}]}]}}))
        .unwrap()),
        ..Default::default()
    };
    let mut dsl = octoscode_module::screens::board3::ui::Dsl::new();
    dr::build(&mut dsl, &st, &octoscode_module::screens::board3::ui::Frame::DESKTOP, &octoscode_store::Store::new());
    let root = parse(&dsl.finish());
    assert!(find(&root, "b3_pwned").is_none(), "no injected node");
    assert_eq!(line_text(&root, "b3_diff_file_0_h0_l0"), hostile);
}

/// The layout: desktop min(1080, 100%) x min(780, 100%) inside the web's
/// 24 px backdrop inset (`.review-backdrop` padding `--dsw-space-5`); at
/// <= 760 px a FULL-SCREEN sheet (no inset, no radius, no border) with the
/// preview id hidden (`@media (max-width: 760px)`), each hunk in its own
/// sideways scroller.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn desktop_dialog_and_the_phone_full_screen_sheet() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    for (w, h, want_w, want_h) in [(990.0, 603.0, 942.0, 555.0), (1400.0, 900.0, 1080.0, 780.0)] {
        let root = open(&conv, WORDS, (w, h)).await;
        let d = find(&root, "b3_dialog").expect("the dialog");
        assert_eq!(prop(d, "width").as_deref(), Some(&*format!("{want_w}")), "{w}x{h}: {}", d.props);
        assert_eq!(prop(d, "height").as_deref(), Some(&*format!("{want_h}")), "{w}x{h}: {}", d.props);
        assert!(find(&root, "b3_diff_preview_id").is_some(), "desktop shows the preview id");
        host::close();
    }
    let root = open(&conv, WORDS, (360.0, 780.0)).await;
    let d = find(&root, "b3_dialog").expect("the sheet");
    assert_eq!(prop(d, "width").as_deref(), Some("360"), "{}", d.props);
    assert_eq!(prop(d, "height").as_deref(), Some("780"), "{}", d.props);
    assert_eq!(prop(d, "draw_bg.radius").as_deref(), Some("0"), "a sheet has square corners");
    assert_eq!(prop(d, "draw_bg.border_width").as_deref(), Some("0"), "and no border");
    assert!(find(&root, "b3_diff_preview_id").is_none(), "the phone hides the preview id");
    for f in 0..3 {
        let s = find(&root, &format!("b3_diff_file_{f}_h0_scroll")).expect("each hunk scrolls sideways");
        assert_eq!(s.kind, "ScrollXView");
    }
    // The marks and runs are the same on the phone.
    assert_eq!(marks(&root, "b3_diff_file_0_h0_l6"), ["send_with_retry", ", &self.backoff"]);
    host::close();
}

/// The review's copy goes through `tr()` with Chinese for every string —
/// the web's catalog where the web has one, the native catalog for the note
/// the web leaves untranslated.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_review_copy_is_chinese_in_zh() {
    let _g = lock();
    let server = Server::start().await;
    let conv = connected(&server).await;
    octoscode_module::i18n::set_language(octoscode_module::i18n::Lang::Zh);
    let root = open(&conv, LARGE, (990.0, 603.0)).await;
    let zh = |s: &str| octoscode_module::i18n::tr_in(octoscode_module::i18n::Lang::Zh, s).to_owned();
    assert_eq!(find(&root, "b3_diff_eyebrow").and_then(|n| n.text.clone()), Some(zh(dr::EYEBROW)));
    assert_ne!(zh(dr::EYEBROW), dr::EYEBROW);
    let note = find(&root, "b3_diff_plain_note").and_then(|n| n.text.clone()).unwrap_or_default();
    assert_ne!(note, dr::PLAIN_NOTE, "the note is translated");
    assert!(note.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)), "{note}");
    for s in [dr::NO_PREVIEW_HEAD, dr::NO_PREVIEW_BODY, dr::NO_METHOD, dr::NATIVE_REVIEW, dr::PLAIN_NOTE] {
        assert_ne!(zh(s), s, "{s} has Chinese");
    }
    octoscode_module::i18n::set_language(octoscode_module::i18n::Lang::En);
    host::close();
}
