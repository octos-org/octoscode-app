//! #30d — Stage C wiring for board 3.8 Resume, 3.9 Attachments, 3.10 Side
//! question (`design/stage-b/autonomy/cards/autonomy-{08,09,10}`).
//!
//! Same door rule as every screens module (8.8 condition 2): the cards see
//! binding ids and action ids only; this module owns the board-3 table. The
//! action ids live ONLY here (the one-owner rule, #29d3) and route through an
//! `is_action` arm in [`crate::AppModule::perform_action`].
//!
//! Behaviour citations (docs/parity-matrix.csv rows):
//! * resume 331 — list unverified candidates (cwd + profile) without creating
//!   or selecting records (`resume-binding.ts:180`);
//! * resume 332 — resume ONLY after exact confirmation; a bare row never opens
//!   (`resume-binding.ts:152`) — the card's own confirm/cancel buttons;
//! * resume 333 — the confirmed open goes out as `session/open` + hydrate and
//!   never starts a turn (`resume-binding.ts:227`);
//! * media 150/151/152 — per-session attachment drafts, max 4 images / 20 MiB,
//!   consumed only at the accepted enqueue boundary
//!   (`attachment-drafts.ts:6,16,19`, `AttachmentsDialog.tsx:1`);
//! * btw 2/5/7 — the aside answers out-of-band (`registry.ts:186`), dismisses
//!   with an explicit control and a late answer stays hidden
//!   (`lazy-btw-controller.ts:110`), and fails closed when the server does not
//!   advertise `session/btw` (`btw.ts:66`) — the call itself is
//!   `packages/client/src/btw.ts:77` (`{session_id, question}`).
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{json, Value};

use crate::bindings::Ctx;
use crate::flow::Conversation;

/// The board-3 cards this module wires.
pub const RESUME_CARD: &str = "autonomy-08";
pub const ATTACHMENTS_CARD: &str = "autonomy-09";
pub const ASIDE_CARD: &str = "autonomy-10";

/// How many resume rows the card draws (the atlas slice row_1..row_4).
const RESUME_ROWS: usize = 4;

/// One attachment in the screen-local draft (`attachment-drafts.ts:6`: max 4
/// images / 20 MiB — the DRAFT is screen state; the protocol never pushes it).
#[derive(Debug, Clone)]
pub struct Attachment {
    pub name: String,
    pub bytes: usize,
}

/// Screen-local UI state (the values the protocol never carries — the same
/// category as the web's component state, `resume-binding.ts` / btw's
/// controller state).
#[derive(Default)]
pub struct SessUi {
    /// The staged resume row (index + the row's exact title) shown in the
    /// confirm dialog; `None` = the dialog is hidden.
    pub resume_staged: Option<(usize, String)>,
    /// The attachment draft (autonomy-09's slots; the card draws two).
    pub attachments: Vec<Attachment>,
    /// The aside's question (fed from the composer draft on `aside.ask`).
    pub aside_question: String,
    /// The aside's answer once it arrives.
    pub aside_answer: String,
    /// hidden | asking | answered | unavailable (`lazy-btw-controller.ts:85`
    /// admission states; `unavailable` = the not-advertised fail-closed).
    pub aside_state: &'static str,
}

impl SessUi {
    fn reset(&mut self) {
        *self = SessUi::default();
    }
}

static SESS: OnceLock<Mutex<SessUi>> = OnceLock::new();

fn state() -> &'static Mutex<SessUi> {
    SESS.get_or_init(|| Mutex::new(SessUi::default()))
}

/// Test seam: reset the screen-local state between tests.
pub fn reset_state() {
    state().lock().unwrap().reset();
}

/// Probe/test seam: seed the attachment draft (the card draws two slots).
pub fn seed_attachments(items: Vec<(&str, usize)>) {
    let mut s = state().lock().unwrap();
    s.attachments = items
        .into_iter()
        .map(|(name, bytes)| Attachment { name: name.to_owned(), bytes })
        .collect();
}

/// Probe/test seam: seed the aside as ANSWERED (question + answer on the
/// panel — the capture proves the live slots).
pub fn seed_aside(question: &str, answer: &str) {
    let mut s = state().lock().unwrap();
    s.aside_question = question.to_owned();
    s.aside_answer = answer.to_owned();
    s.aside_state = "answered";
}

/// One resume row as the card's slot sees it (`store.sessions()` order —
/// parity 331: candidates listed WITHOUT creating or selecting records).
fn resume_rows(store: &Arc<crate::Store>) -> Vec<Value> {
    store
        .sessions()
        .into_iter()
        .take(RESUME_ROWS)
        .map(|s| {
            // The card's meta grammar: "<host> • <when> • <n> turns".
            let host = s.id.split(':').next().unwrap_or("octos").to_owned();
            let when = s.updated_at.clone().unwrap_or_default();
            json!({
                "id": s.id,
                "title": s.title,
                "meta": format!("{} • {} • {} turns", host, when, s.message_count),
                "active_turn": s.active_turn,
            })
        })
        .collect()
}

// ---- tables (one-owner: these ids exist nowhere else) ------------------------

/// The board-3 data slots (autonomy-08/09/10).
pub const BINDINGS: &[(&str, &str)] = &[
    ("resume.rows", "list: [{id,title,meta,active_turn}] — the first four sessions (resume-binding.ts:180)"),
    ("resume.rows[].title", "text: each row's title"),
    ("resume.rows[].meta", "text: each row's '<host> • <when> • <n> turns' line"),
    ("resume.confirm", "text: the confirm line, e.g. 'Resume \"<title>\"?' — empty while hidden (resume-binding.ts:152)"),
    ("resume.pending", "bool: a confirm dialog is staged"),
    ("att.count", "text: '<n> of 4 images • 20 MB max' (attachment-drafts.ts:6)"),
    ("att.sizes", "list: each draft attachment's size label"),
    ("aside.header", "text: the aside panel's 'Aside · /btw' header (registry.ts:186)"),
    ("aside.question", "text: the asked question (fed from the composer draft)"),
    ("aside.answer", "text: the answer once it arrives (BtwAsidePanel.tsx:22)"),
    ("aside.state", "text: hidden | asking | answered | unavailable (lazy-btw-controller.ts:85, btw.ts:66)"),
];

/// The board-3 action ids the autonomy cards emit. ONLY in this table
/// (`service-actions.json` controls + the ask affordance).
pub const ACTIONS: &[(&str, &str)] = &[
    ("resume.stage", "stage the clicked row into the confirm dialog (index) — never opens (resume-binding.ts:152)"),
    ("resume.confirm", "session/open + hydrate the staged row (resume-binding.ts:227)"),
    ("resume.cancel", "hide the confirm dialog without opening"),
    ("attachment.remove", "drop the clicked draft attachment (index; UI-local, attachment-drafts.ts:16)"),
    ("aside.ask", "session/btw with the composer draft as the question (btw.ts:77; advertised-gated, btw.ts:66)"),
    ("aside.dismiss", "hide the aside; a late answer stays hidden (lazy-btw-controller.ts:110)"),
];

/// Whether `id` is one of this module's action ids.
pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

/// The fail-closed gate for `session/btw` (btw.ts:66): the store carries the
/// server's accepted list; no recorded fixture advertises the method, so every
/// real-traffic run fails closed until a server does.
fn btw_advertised(store: &Arc<crate::Store>) -> bool {
    store.capabilities().iter().any(|c| c == "session/btw")
}

// ---- bindings ----------------------------------------------------------------

/// Resolve one of this module's binding ids. Called from
/// [`crate::bindings::query`]'s delegating arm.
pub fn query(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    let store = ctx.store;
    let s = state().lock().unwrap();
    Some(match id {
        "resume.rows" => json!(resume_rows(store)),
        "resume.rows[].title" => json!(resume_rows(store)
            .into_iter()
            .map(|r| r["title"].clone())
            .collect::<Vec<_>>()),
        "resume.rows[].meta" => json!(resume_rows(store)
            .into_iter()
            .map(|r| r["meta"].clone())
            .collect::<Vec<_>>()),
        "resume.confirm" => json!(match &s.resume_staged {
            Some((_, title)) => format!("Resume \"{title}\"?"),
            None => String::new(),
        }),
        "resume.pending" => json!(s.resume_staged.is_some()),

        "att.count" => json!(format!(
            "{} of 4 images • 20 MB max",
            s.attachments.len()
        )),
        "att.sizes" => json!(s
            .attachments
            .iter()
            .map(|a| format!("{:.1} MB", a.bytes as f64 / (1024.0 * 1024.0)))
            .collect::<Vec<_>>()),

        "aside.header" => json!("Aside · /btw"),
        "aside.question" => json!(s.aside_question),
        "aside.answer" => json!(s.aside_answer),
        "aside.state" => json!(s.aside_state),
        _ => return None,
    })
}

// ---- actions -----------------------------------------------------------------

/// What a board-3 action means. Pure — resolved against the store + screen
/// state; [`apply`] performs it over the production client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// `session/list` — refresh the resume candidates (parity 331).
    Refresh,
    /// Stage the clicked row into the confirm dialog (UI-local).
    ResumeStage(usize),
    /// `session/open` + hydrate the staged row — never starts a turn
    /// (resume-binding.ts:227).
    ResumeConfirm(String),
    /// Hide the confirm dialog (UI-local).
    ResumeCancel,
    /// Drop a draft attachment (UI-local).
    AttachmentRemove(usize),
    /// `session/btw` — the gate already passed in resolve. Carries the question.
    AsideAsk(String),
    /// Hide the aside (UI-local; a late answer stays hidden).
    AsideDismiss,
    /// A declared id with no resolvable target — logged by name, never fatal.
    Unhandled(String),
}

/// Route one board-3 action. `index` is the row/control that emitted it.
pub fn resolve(action: &str, index: usize, ctx: &Ctx<'_>) -> Effect {
    let mut s = state().lock().unwrap();
    match action {
        "resume.stage" => match resume_rows(ctx.store).get(index) {
            Some(row) => {
                // The exact title is what the confirm must echo
                // (resume-binding.ts:152's "exact confirmation").
                let title = row["title"].as_str().unwrap_or_default().to_owned();
                s.resume_staged = Some((index, title.clone()));
                Effect::ResumeStage(index)
            }
            None => Effect::Unhandled(format!("{action}[{index}]")),
        },
        "resume.confirm" => {
            // Confirming CONSUMES the staged row: the dialog closes and the
            // confirm line clears (resume-binding.ts:152's exact-confirmation
            // Ok path — a second submit finds nothing staged).
            let staged = s.resume_staged.take();
            match staged {
                Some((_, title)) => {
                    let id = resume_rows(ctx.store)
                        .iter()
                        .find(|r| r["title"] == json!(title))
                        .and_then(|r| r["id"].as_str().map(str::to_owned));
                    match id {
                        Some(id) => Effect::ResumeConfirm(id),
                        None => Effect::Unhandled(format!("{action} (staged row vanished)")),
                    }
                }
                None => Effect::Unhandled(format!("{action} (nothing staged)")),
            }
        }
        "resume.cancel" => {
            s.resume_staged = None;
            Effect::ResumeCancel
        }
        "attachment.remove" => {
            if index < s.attachments.len() {
                s.attachments.remove(index);
                Effect::AttachmentRemove(index)
            } else {
                Effect::Unhandled(format!("{action}[{index}]"))
            }
        }
        "aside.ask" => {
            // The question is the composer draft (the card's own affordance);
            // admission is synchronous and typed (lazy-btw-controller.ts:85).
            let question = ctx.ui.lock().unwrap().draft();
            if question.trim().is_empty() {
                return Effect::Unhandled("aside.ask[empty]".to_owned());
            }
            if !btw_advertised(ctx.store) {
                s.aside_state = "unavailable";
                return Effect::Unhandled("aside.ask[not-advertised]".to_owned());
            }
            s.aside_question = question.clone();
            s.aside_state = "asking";
            Effect::AsideAsk(question)
        }
        "aside.dismiss" => {
            // A late answer stays hidden: the panel state goes away entirely
            // (lazy-btw-controller.ts:110).
            s.aside_state = "hidden";
            s.aside_answer = String::new();
            Effect::AsideDismiss
        }
        other => Effect::Unhandled(other.to_owned()),
    }
}

// ---- performing (the production client, never a fake) ------------------------

/// Perform one resolved effect. UI-local effects return before any wire call;
/// protocol effects go through [`Conversation::client`] — the same transport
/// every live call takes — and land in the flow's trace.
pub async fn apply(effect: Effect, conv: &Conversation) -> Result<(), String> {
    match effect {
        Effect::Refresh => {
            conv.refresh_sessions()
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        // resume 333: open in the background, verify history, NEVER start a
        // turn. `Conversation::open_session` is exactly that production path
        // (`session/open`, then the session/list refresh).
        Effect::ResumeConfirm(id) => conv
            .open_session(&id, None)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string()),
        Effect::AsideAsk(question) => {
            let client = conv.client();
            let result = client
                .request(
                    "session/btw",
                    json!({
                        "session_id": conv.session_id(),
                        "question": question,
                    }),
                )
                .await
                .map_err(|e| e.to_string());
            let answer = match result {
                Ok(v) => v["answer"].as_str().unwrap_or_default().to_owned(),
                Err(e) => {
                    // Typed admission failure: the draft was never consumed
                    // (lazy-btw-controller.ts:85) — the panel says unavailable.
                    state().lock().unwrap().aside_state = "unavailable";
                    return Err(e);
                }
            };
            {
                let mut s = state().lock().unwrap();
                s.aside_answer = answer;
                s.aside_state = "answered";
            }
            Ok(())
        }
        Effect::ResumeStage(_)
        | Effect::ResumeCancel
        | Effect::AttachmentRemove(_)
        | Effect::AsideDismiss => Ok(()),
        Effect::Unhandled(id) => {
            ::log::warn!("octoscode: unhandled screen action {id:?}");
            Ok(())
        }
    }
}

/// Spawn one effect on the module's runtime (the `workspace::spawn` shape).
pub fn spawn(effect: Effect, rt: &tokio::runtime::Runtime, conv: Arc<Conversation>) {
    rt.spawn(async move {
        if let Err(e) = apply(effect, &conv).await {
            ::log::warn!("octoscode: screen action: {e}");
        }
    });
}

// ---- lowering (the DESIGN branch: the artifact Gate B rendered) --------------

/// The screen card's lowered DSL with the module's live-slot swaps applied. A
/// slot keeps its authored copy when the live text is empty (idle = the Stage B
/// render). These three screens are fixed chrome, so the DESIGN branch's
/// measured coordinates are within the RULES 8.10 carve-out.
pub fn lower_screen(which: &str, store: &Arc<crate::Store>) -> Result<String, String> {
    let card = match which {
        "resume" => RESUME_CARD,
        "attachments" => ATTACHMENTS_CARD,
        "aside" => ASIDE_CARD,
        other => return Err(format!("octoscode: unknown screen {other:?}")),
    };
    let dir = std::path::Path::new(&std::env::var("OCTOSCODE_CARDS_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/stage-b/autonomy/cards").to_owned()
    }))
    .join(card);
    let card_text = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("octoscode: {card}/page.card: {e}"))?;
    let data_text =
        std::fs::read_to_string(dir.join("page.data.json")).unwrap_or_else(|_| "{}".into());
    let data: Value =
        serde_json::from_str(&data_text).map_err(|e| format!("octoscode: {card}/page.data.json: {e}"))?;
    let prepared = octoscript_makepad::l0::prepare(&card_text, &data, &dir.join("kit"))?;
    let mut dsl = octoscript_makepad::design::to_makepad_ui(&prepared.tree)?;

    let s = state().lock().unwrap();
    // (authored probed literal, live text) — empty `to` keeps the authored copy.
    let swaps: Vec<(String, String)> = match card {
        RESUME_CARD => {
            let rows = resume_rows(store);
            let mut swaps: Vec<(String, String)> = Vec::new();
            for (i, (authored_title, authored_meta)) in [
                ("Add session fork", "octos • 2h ago • 14 turns"),
                ("Fix steer queue drop on reconnect", "octos • 1d ago • 23 turns"),
                ("Review PR #2566", "octos • 3d ago • 17 turns"),
                ("Why is hydrate slow?", "octos • 4d ago • 9 turns"),
            ]
            .into_iter()
            .enumerate()
            {
                if let Some(row) = rows.get(i) {
                    let title = row["title"].as_str().unwrap_or_default();
                    let meta = row["meta"].as_str().unwrap_or_default();
                    if !title.is_empty() && title != authored_title {
                        swaps.push((
                            format!(r#"text: "{authored_title}""#),
                            format!(r#"text: "{}""#, escape_splash(title)),
                        ));
                    }
                    if !meta.is_empty() && meta != authored_meta {
                        swaps.push((
                            format!(r#"text: "{authored_meta}""#),
                            format!(r#"text: "{}""#, escape_splash(meta)),
                        ));
                    }
                }
            }
            // The staged confirm echoes the row's exact title
            // (resume-binding.ts:152) — swap the authored line when staged.
            if let Some((_, staged)) = &s.resume_staged {
                swaps.push((
                    r#"text: "Resume \"Add session fork\"?""#.to_owned(),
                    format!(r#"text: "Resume \"{}\"?""#, escape_splash(staged)),
                ));
            }
            swaps
        }
        ATTACHMENTS_CARD => {
            vec![(
                r#"text: "2 of 4 images • 20 MB max""#.to_owned(),
                format!(
                    r#"text: "{}""#,
                    escape_splash(&format!(
                        "{} of 4 images • 20 MB max",
                        s.attachments.len()
                    ))
                ),
            )]
        }
        _ => {
            let mut swaps: Vec<(String, String)> = Vec::new();
            if !s.aside_question.is_empty() {
                swaps.push((
                    r#"text: "Why is the steer queue dropping""#.to_owned(),
                    format!(r#"text: "{}""#, escape_splash(&s.aside_question)),
                ));
            }
            if !s.aside_answer.is_empty() {
                swaps.push((
                    r#"text: "It's a metric that increments when messages are dropped from the steer queue due to a reconnect or protocol error. It helps track message loss.""#.to_owned(),
                    format!(r#"text: "{}""#, escape_splash(&s.aside_answer)),
                ));
            }
            swaps
        }
    };
    for (from, to) in swaps {
        dsl = dsl.replace(&from, &to);
    }
    Ok(dsl)
}

/// Splash DSL string literals are double-quoted (`text: "..."`); escape a
/// backslash or quote so live text cannot break the literal.
fn escape_splash(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Trace helper for tests/assertions: the outbound method an effect performs.
pub fn wire_method(effect: &Effect) -> Option<&'static str> {
    match effect {
        Effect::Refresh => Some("session/list"),
        Effect::ResumeConfirm(_) => Some("session/open"),
        Effect::AsideAsk(_) => Some("session/btw"),
        _ => None,
    }
}
