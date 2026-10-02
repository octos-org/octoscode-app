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
    /// A real preview image URL for the tile (`None` keeps the authored
    /// thumb). The capture host serves real PNGs in-process (#29a2 path), so
    /// the tile decodes an actual image — never a grey 404 box (#30d2).
    pub image: Option<String>,
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
        .map(|(name, bytes)| Attachment { name: name.to_owned(), bytes, image: None })
        .collect();
}

/// Probe seam: seed drafts WITH real preview URLs (the tiles decode actual
/// images served by the capture host).
pub fn seed_attachments_live(items: Vec<(String, usize, String)>) {
    let mut s = state().lock().unwrap();
    s.attachments = items
        .into_iter()
        .map(|(name, bytes, image)| Attachment { name, bytes, image: Some(image) })
        .collect();
}

/// #P4e2a — `parseSessionBtwResult` (`packages/client/src/btw.ts:34-52`):
/// an aside result is admitted ONLY when it is a record, carries THIS
/// Session's id, and has a non-blank string `answer`; an optional `model`,
/// when present, must itself be a non-blank string. Anything else is the
/// web's `BtwProtocolError("Invalid or wrong-Session aside result")` —
/// never a blank or foreign answer rendered as if it were the reply.
pub fn parse_aside_result(value: &Value, asked_session: &str) -> Result<String, String> {
    const INVALID: &str = "Invalid or wrong-Session aside result";
    let Some(obj) = value.as_object() else {
        return Err(INVALID.to_owned());
    };
    // btw.ts:40 — the session_id must equal the one the question was sent to.
    if obj.get("session_id").and_then(Value::as_str) != Some(asked_session) {
        return Err(INVALID.to_owned());
    }
    // btw.ts:42-43 — a string, and not whitespace-only.
    let answer = obj.get("answer").and_then(Value::as_str).unwrap_or_default();
    if answer.trim().is_empty() {
        return Err(INVALID.to_owned());
    }
    // btw.ts:43-45 — model is optional, but a present one must be non-blank.
    if let Some(model) = obj.get("model") {
        if !model.is_null() {
            match model.as_str() {
                Some(m) if !m.trim().is_empty() => {}
                _ => return Err(INVALID.to_owned()),
            }
        }
    }
    Ok(answer.to_owned())
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
    // #P4g2 row 231: the web's row projection falls the title back to the
    // row's last prompt (`workspace-session-catalog.ts:124`
    // `title: entry.title ?? entry.lastPrompt`); the old native projection
    // passed the bare title through.
    let mut rows: Vec<Value> = store
        .sessions()
        .into_iter()
        .take(RESUME_ROWS)
        .map(|s| {
            // The card's meta grammar: "<host> • <when> • <n> turns".
            let host = s.id.split(':').next().unwrap_or("octos").to_owned();
            let when = s.updated_at.clone().unwrap_or_default();
            // #P4h1 row 301: the shared label STEM
            // (`Session::label_stem`), which is the web's ingestion rule
            // (`workspace-session-catalog.ts:79-80` — `title?.trim() || null`,
            // then `last_prompt?.trim() || null`). This projection keeps the
            // `null`: the web's sidebar row has NO id fallback
            // (`workspace-session-catalog.ts:120` — `entry.title ??
            // entry.lastPrompt`), unlike the display label
            // (`model.ts:71` — `|| session.id`). It replaces the duplicated
            // local copy of the trim rule.
            let title = s.label_stem();
            json!({
                "id": s.id,
                "title": title,
                "meta": format!("{} • {} • {} turns", host, when, s.message_count),
                "active_turn": s.active_turn,
            })
        })
        .collect();
    // #P4g2 row 230: retained peers merge into the known rows WITHOUT
    // stealing focus — appended AFTER the session rows (the web's
    // `mergeConfirmedRetainedSessions` gives retained entries
    // `lastOpenedAt: 0`, sorting them last — retained-session-catalog.ts:18)
    // and no `session/open` is issued for them. Open peers only: a
    // `peer/closed` peer is gone, so its row drops out.
    for p in store.domains.peer.list() {
        if p.closed || rows.len() >= RESUME_ROWS {
            continue;
        }
        rows.push(json!({
            "id": p.topic.clone().unwrap_or_else(|| format!("peer-{}", p.name)),
            "title": p.name,
            "meta": format!("peer • staged • {}", p.name),
            "active_turn": false,
        }));
    }
    rows
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
        //
        // A20 (row 247): the candidate is a catalog row of the workspace the
        // catalog was read for (`workspace-session-catalog.ts:26-35`, a row
        // is "a candidate id the server reported for this workspace"): a
        // Session this app already confirmed under ANOTHER workspace is
        // refused before any `session/open` or `session/hydrate`
        // (`screens::saved_link::precondition`), and the open carries that
        // workspace — never folder-less — so the open reply's exact check
        // (row 204) refuses another root before history is read.
        Effect::ResumeConfirm(id) => {
            let scope = conv.catalog_params().cwd.filter(|c| !c.trim().is_empty());
            if let Some(workspace) = &scope {
                let candidate = crate::screens::saved_link::SavedReference {
                    workspace_root: workspace.clone(),
                    profile_id: conv.profile(),
                    session_id: id.clone(),
                };
                let known = conv.store.domains.session.workspace_root(&id);
                if let Err(refusal) = crate::screens::saved_link::precondition(&candidate, known.as_deref(), None) {
                    makepad_widgets::log!("[octoscode] resume {id} refused before any open: {}", refusal.lead);
                    return Err(refusal.lead);
                }
            }
            conv.open_session(&id, scope.or_else(|| conv.resume_cwd(&id)))
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        Effect::AsideAsk(question) => {
            let client = conv.client();
            let asked = conv.session_id();
            let result = client
                .request(
                    "session/btw",
                    json!({
                        "session_id": asked,
                        "question": question,
                    }),
                )
                .await
                .map_err(|e| e.to_string());
            let answer = match result {
                Ok(v) => match parse_aside_result(&v, &asked) {
                    Ok(answer) => answer,
                    Err(e) => {
                        // A reply that is not THIS Session's, or is blank, is a
                        // protocol error — never a shown answer (btw.ts:81-87).
                        state().lock().unwrap().aside_state = "unavailable";
                        return Err(e);
                    }
                },
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
        crate::design::dir("stage-b/autonomy/cards").to_string_lossy().to_string()
    }))
    .join(card);
    let card_text = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("octoscode: {card}/page.card: {e}"))?;
    let data_text =
        std::fs::read_to_string(dir.join("page.data.json")).unwrap_or_else(|_| "{}".into());
    let data: Value =
        serde_json::from_str(&data_text).map_err(|e| format!("octoscode: {card}/page.data.json: {e}"))?;
    let prepared = octoscript_makepad::l0::prepare(&card_text, &data, &dir.join("kit"))?;
    let mut dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&prepared.tree))?;

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
            // The caption counts the TRUE list (attachment-drafts.ts:6); the
            // tiles/sizes are per-item surgery after the swaps.
            let n = s.attachments.len();
            vec![(
                r#"text: "2 of 4 images • 20 MB max""#.to_owned(),
                format!(
                    r#"text: "{}""#,
                    escape_splash(&format!("{n} of 4 images • 20 MB max"))
                ),
            )]
        }
        _ => {
            let mut swaps: Vec<(String, String)> = Vec::new();
            // The question swaps via the t_q1 node surgery below (one wrapping
            // label — the design's second line is a stale leftover, #30d2 3).
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

    // Per-item surgery (#30d2): a fixed-chrome card's tile/row count comes
    // from the DATA, never from the design's frozen count (LESSONS: per item,
    // not the design's count).
    match card {
        ATTACHMENTS_CARD => {
            let n = s.attachments.len();
            if n == 0 {
                // 0 → no tile row at all.
                for node in ["att_1", "att_2", "t_sz1", "t_sz2", "att2_ring", "att2_pct"] {
                    dsl = cut_node(&dsl, node);
                }
            } else {
                if n < 2 {
                    for node in ["att_2", "t_sz2", "att2_ring", "att2_pct"] {
                        dsl = cut_node(&dsl, node);
                    }
                }
                // Real previews: each surviving tile decodes the attached
                // image — never a grey 404 box (the capture host serves them
                // in-process, the #29a2 path).
                for (i, node) in ["att_1_thumb", "att_2_thumb"].iter().enumerate() {
                    if let Some(url) = s.attachments.get(i).and_then(|a| a.image.as_deref()) {
                        dsl = swap_node_src(&dsl, node, url);
                    }
                }
                let sz = |i: usize| {
                    s.attachments
                        .get(i)
                        .map(|a| format!("{:.1} MB", a.bytes as f64 / (1024.0 * 1024.0)))
                        .unwrap_or_default()
                };
                dsl = set_node_text(&dsl, "t_sz1", &sz(0));
                // #36a: the "68%" label must sit INSIDE the ring's hole, be
                // legible, and NOT clip its own glyphs. Measured on main with
                // the instrument: the hole is R = (9.2 - 2.6/2) x (52/24) = 17.12
                // (⌀ 34.23 — the #32b2 figure was right), the ring's centre is
                // (313.0, 275.5) from the authored seat y 249.5, and the label
                // box 40.2 x 24.76 has half-diagonal hypot(20.1, 12.38) = 23.61 —
                // every corner sat on the stroke, and the authored seat
                // (275.04, 294.73) hung the whole label below the ring.
                //
                // Review r2: sizing the box from the AUTHORED ratio
                // (40.2/15.85 = 2.5363) left the run flush against the box edge —
                // measured ink 298.5..327.0 = 28.50px in a 29.04px box, a 0.02px
                // left margin, so the "6" and "%" were cut. And the run/font
                // ratio is NOT constant across sizes: 28.50/11.45 = 2.4891 but
                // 27.00/10.50 = 2.5714 (small-size hinting/AA), so the box is
                // sized from the run MEASURED at the size actually used, not
                // from a scaled authored constant. At 10.0pt the measured run is
                // 26.00px (tmp/36a-evidence/attachments-after4.png), so a 29.00px
                // box holds it with 3.00px of slack, and its half-diagonal 16.02
                // stays 1.10px inside the 17.12 hole.
                dsl = set_node_abs_pos(&dsl, "att2_pct", 298.5, 268.7);
                dsl = set_node_box(&dsl, "att2_pct", 29.0, 13.6);
                dsl = set_node_font_size(&dsl, "att2_pct", 10.0);
                dsl = centre_node_text(&dsl, "att2_pct");
                // Legibility: the card authors the label WHITE (color
                // 4294967295) over an arbitrary thumbnail, and `att_2` has no
                // scrim child at all — the run reads through the pale preview
                // and over the #48484A track. Paint a dark disc filling the
                // hole, behind the label, so the white glyph always has contrast.
                dsl = add_hole_scrim(&dsl);
                if n > 1 {
                    dsl = set_node_text(&dsl, "t_sz2", &sz(1));
                }
            }
        }
        ASIDE_CARD => {
            // The bubble is ONE wrapping label (the #16 user-bubble shape);
            // the design's second line is a stale leftover (#30d2 defect 3).
            if !s.aside_question.is_empty() {
                dsl = set_node_text(&dsl, "t_q1", &s.aside_question);
                dsl = fit_node_height(&dsl, "t_q1");
            }
            dsl = cut_node(&dsl, "t_q2");
            // #32b2 item 1 (review): the surface hugs AND pads symmetrically
            // — height:Fit with padding:0 left the one-line text sitting on
            // the bottom edge (the design flow's own #16e fix for the main
            // bubble: components.rs:655-661).
            dsl = fit_node_box_height(&dsl, "user_bubble");
            dsl = dsl.replace(
                "flow: Overlay padding: 0 clip_x: false clip_y: false",
                "flow: Down padding: Inset{left: 14.93 top: 12 right: 14.93 bottom: 12} clip_x: false clip_y: false",
            );
            // v3: the label is seated by abs_pos — an abs child ignores the
            // Down flow, so the Fit surface collapsed to its padding and the
            // text rendered BELOW the bar (the after2 capture). Clear the
            // seat, let the bubble's padding place it, Fill the width so a
            // long question wraps at the padded box, and wrap the run.
            dsl = dsl.replace("\nabs_pos: vec2(129.93, 167.4)", "");
            dsl = dsl.replace(
                "t_q1 := Label {\nwidth: 240.69 height: Fit",
                "t_q1 := Label {\nwidth: Fill height: Fit",
            );
            dsl = wrap_node_text(&dsl, "t_q1");
        }
        RESUME_CARD => {
            // Meta lines take their full room (#30d2 defect 4: "7 turn…"
            // clipped at the authored 167.76).
            for node in ["row_1_meta", "row_2_meta", "row_3_meta", "row_4_meta"] {
                dsl = widen_node(&dsl, node);
            }
            // Per-item rows: the design's rows beyond the data are stale text.
            let n = resume_rows(store).len();
            for i in 1..=4usize {
                if i > n {
                    for node in [
                        format!("row_{i}_sel"),
                        format!("row_{i}_radio"),
                        format!("row_{i}_title"),
                        format!("row_{i}_meta"),
                    ] {
                        dsl = cut_node(&dsl, &node);
                    }
                    if i >= 2 {
                        dsl = cut_node(&dsl, &format!("div_{}", i - 2));
                    }
                }
            }
        }
        _ => {}
    }
    // #31d workflow 1: the session screens' kits are LIGHT (#FEFEFE/#F6F6F7) —
    // the app-wide token set rewrites them in dark mode (light-authored DSL
    // only; see theme.rs).
    Ok(crate::screens::theme::retint_dsl(&dsl))
}

/// Splash DSL string literals are double-quoted (`text: "..."`); escape a
/// backslash or quote so live text cannot break the literal.
fn escape_splash(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Cut one authored node (`name := Type { ... }`, braces balanced) out of the
/// lowered DSL — the per-item surgery for fixed-chrome cards whose design
/// ships a fixed tile count (#30d2: one tile per ATTACHMENT, not the design's).
fn cut_node(dsl: &str, node: &str) -> String {
    let Some(start) = dsl.find(&format!("{node} := ")) else {
        return dsl.to_owned();
    };
    let bytes = dsl.as_bytes();
    let Some(open) = bytes[start..].iter().position(|&b| b == b'{').map(|i| start + i) else {
        return dsl.to_owned();
    };
    let mut depth = 0usize;
    let mut end = dsl.len();
    for (i, &b) in bytes[open..].iter().enumerate() {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    // Drop the node plus the newline that followed it.
    let rest = &dsl[end..];
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    format!("{}{}", &dsl[..start], rest)
}

/// Set one Label node's `text:` (node-anchored — for labels whose authored
/// literals are NOT unique, e.g. the two "1.2 MB" size labels).
fn set_node_text(dsl: &str, node: &str, text: &str) -> String {
    let Some(npos) = dsl.find(&format!("{node} := Label {{")) else {
        return dsl.to_owned();
    };
    let Some(tkey) = dsl[npos..].find("text: \"") else {
        return dsl.to_owned();
    };
    let start = npos + tkey + "text: \"".len();
    let Some(end_rel) = dsl[start..].find('"') else {
        return dsl.to_owned();
    };
    let end = start + end_rel;
    format!("{}{}{}", &dsl[..start], escape_splash(text), &dsl[end..])
}

/// Swap one DesignImage node's `src:` URL (per-tile real previews).
fn swap_node_src(dsl: &str, node: &str, url: &str) -> String {
    let Some(npos) = dsl.find(&format!("{node} := ")) else {
        return dsl.to_owned();
    };
    let key = "src: http_resource(\"";
    let Some(skey) = dsl[npos..].find(key) else {
        return dsl.to_owned();
    };
    let start = npos + skey + key.len();
    let Some(end_rel) = dsl[start..].find("\")") else {
        return dsl.to_owned();
    };
    let end = start + end_rel;
    format!("{}{}{}", &dsl[..start], url, &dsl[end..])
}

/// Move one node's seated origin (`abs_pos: vec2(x, y)` in the lowered DSL)
/// — #32b2 item 3: the "68%" label centres in the progress ring's hole
/// instead of clipping its lower arc.
fn set_node_abs_pos(dsl: &str, node: &str, x: f64, y: f64) -> String {
    let Some(npos) = dsl.find(&format!("{node} := ")) else {
        return dsl.to_owned();
    };
    let window_end = (npos + 240).min(dsl.len());
    let key = "abs_pos: vec2(";
    let Some(ppos) = dsl[npos..window_end].find(key) else {
        return dsl.to_owned();
    };
    let start = npos + ppos + key.len();
    let Some(len) = dsl[start..].find(')') else {
        return dsl.to_owned();
    };
    format!("{}{}, {}{}", &dsl[..start], x, y, &dsl[start + len..])
}

/// Make one label's text run wrap (`flow: Right` -> `flow: Right{wrap: true}`,
/// first occurrence in the node's head) — a long side question must wrap, not
/// hard-clip (#32b2 item 1; the main-bubble precedent, components.rs:571).
fn wrap_node_text(dsl: &str, node: &str) -> String {
    let Some(npos) = dsl.find(&format!("{node} := ")) else {
        return dsl.to_owned();
    };
    let window_end = (npos + 700).min(dsl.len());
    let key = "flow: Right\n";
    let Some(fpos) = dsl[npos..window_end].find(key) else {
        return dsl.to_owned();
    };
    let at = npos + fpos;
    format!("{}flow: Right{{wrap: true}}{}", &dsl[..at], &dsl[at + key.len()..])
}

/// Centre one label's text run in its own box (`align x: 0` -> `x: 0.5`,
/// first occurrence in the node's head) — #32b2 item 3 review: the ~21px
/// "68%" run fits the ring's ~34px hole once it centres; left-seated it
/// rode onto the ink arc.
fn centre_node_text(dsl: &str, node: &str) -> String {
    let Some(npos) = dsl.find(&format!("{node} := ")) else {
        return dsl.to_owned();
    };
    let window_end = (npos + 700).min(dsl.len());
    let key = "align: Align{x: 0 y: 0.5}";
    let Some(apos) = dsl[npos..window_end].find(key) else {
        return dsl.to_owned();
    };
    let at = npos + apos;
    format!(
        "{}{}{}",
        &dsl[..at],
        "align: Align{x: 0.5 y: 0.5}",
        &dsl[at + key.len()..]
    )
}

/// Resize one node's own box in the lowered DSL — #36a: the "68%" label's
/// authored box is sized to the OCR ink bounding box, which is wider than the
/// ring's circular hole, so the box is seated to the run instead. The lowered
/// head carries `width: W height: H` on the line AFTER `<node> := Kind {`.
fn set_node_box(dsl: &str, node: &str, w: f64, h: f64) -> String {
    let Some(npos) = dsl.find(&format!("{node} := ")) else {
        return dsl.to_owned();
    };
    let window_end = (npos + 200).min(dsl.len());
    // Resolve BOTH value spans against the SAME, unmutated string — resolving
    // the second one after a width edit shifts its offsets (40.2 -> 29.04 is a
    // one-character move) and re-inserts the old number.
    let span_of = |key: &str| -> Option<(usize, usize)> {
        let k = npos + dsl[npos..window_end].find(key)?;
        let s = k + key.len();
        let e = s + dsl[s..window_end].find(|c: char| !c.is_ascii_digit() && c != '.')?;
        Some((s, e))
    };
    let (Some((ws, we)), Some((hs, he))) = (span_of("width: "), span_of("height: ")) else {
        return dsl.to_owned();
    };
    // `width: W height: H` — the height span starts later, so replace it first
    // and the width offsets stay valid.
    let out = format!("{}{}{}", &dsl[..hs], h, &dsl[he..]);
    format!("{}{}{}", &out[..ws], w, &out[we..])
}

/// Set one Label's `font_size:` in the lowered DSL (the lowered head carries
/// `draw_text.text_style: TextStyle{... font_size: 11.887501 ...}`) — #36a:
/// the run has to shrink with its box or it still rides the ring's stroke.
fn set_node_font_size(dsl: &str, node: &str, size: f64) -> String {
    let Some(npos) = dsl.find(&format!("{node} := ")) else {
        return dsl.to_owned();
    };
    // A1: `font_size` follows the whole font family in the label head, and the
    // family grew (the bundled sans CJK member + its lazy fallback,
    // design::cjk_members): at 700 chars the size was out of reach and this
    // rewrite silently stopped (f36a measured the 11.89 pt run again).
    let window_end = (npos + 2500).min(dsl.len());
    let key = "font_size: ";
    let Some(fpos) = dsl[npos..window_end].find(key) else {
        return dsl.to_owned();
    };
    let start = npos + fpos + key.len();
    let Some(len) = dsl[start..].find(|c: char| !c.is_ascii_digit() && c != '.') else {
        return dsl.to_owned();
    };
    format!("{}{}{}", &dsl[..start], size, &dsl[start + len..])
}

/// Paint a dark disc filling the ring's hole, immediately BEFORE the label, so
/// the white "68%" always has contrast over an arbitrary thumbnail (#36a). The
/// card authors no scrim node at all — `att_2`'s children are the thumb, the
/// close button, the ring and the label — so this is injected here. Sized to
/// the hole (⌀ 34.23) and seated on the ring centre (313, 276).
fn add_hole_scrim(dsl: &str) -> String {
    let Some(npos) = dsl.find("att2_pct := ") else {
        return dsl.to_owned();
    };
    // `DesignSurface`, not a bare `View`: the design vocabulary's containers
    // lower to View and lay a rect out but paint no background (measured: the
    // disc's annulus showed the thumbnail through it), while every authored
    // surface (att_2, att_2_close_bg, user_bubble) lowers to DesignSurface and
    // does paint. Props mirror the authored surface heads.
    let disc = "att2_pct_scrim := DesignSurface {width: 34.23 height: 34.23 \
                abs_pos: vec2(295.88, 258.38) \
                draw_bg.radius: 17.12 draw_bg.ellipse: 1.0 draw_bg.border_width: 0 \
                draw_bg.border_position: 0 draw_bg.border_color: #00000000 \
                flow: Down padding: 0 clip_x: false clip_y: false \
                show_bg: true draw_bg.color: #1c1c1eff}\n";
    format!("{}{}{}", &dsl[..npos], disc, &dsl[npos..])
}

/// `fit_node_height` for ANY node kind (`user_bubble := DesignSurface {`):
/// find `<node> := `, then the first `height: <num>` inside the node's head,
/// and make it `Fit` (#32b2 item 1).
fn fit_node_box_height(dsl: &str, node: &str) -> String {
    let Some(npos) = dsl.find(&format!("{node} := ")) else {
        return dsl.to_owned();
    };
    let window_end = (npos + 120).min(dsl.len());
    let key = "height: ";
    let Some(hpos) = dsl[npos..window_end].find(key) else {
        return dsl.to_owned();
    };
    let start = npos + hpos + key.len();
    let Some(len) = dsl[start..].find(|c: char| !c.is_ascii_digit() && c != '.') else {
        return dsl.to_owned();
    };
    format!("{}Fit{}", &dsl[..start], &dsl[start + len..])
}

/// Patch one Label node's fixed height to `Fit` so a long line wraps and the
/// label grows with it (the #16 user-bubble behaviour for the aside question).
fn fit_node_height(dsl: &str, node: &str) -> String {
    let Some(npos) = dsl.find(&format!("{node} := Label {{")) else {
        return dsl.to_owned();
    };
    let window_end = (npos + 140).min(dsl.len());
    let key = "height: ";
    let Some(hpos) = dsl[npos..window_end].find(key) else {
        return dsl.to_owned();
    };
    let start = npos + hpos + key.len();
    let Some(len) = dsl[start..].find(|c: char| !c.is_ascii_digit() && c != '.') else {
        return dsl.to_owned();
    };
    format!("{}Fit{}", &dsl[..start], &dsl[start + len..])
}

/// Patch one authored node's width (`name := Type {\nwidth: <old>` -> 310) so
/// runtime meta lines take their full room (#30d2 defect 4: "7 turn…" clips).
fn widen_node(dsl: &str, node: &str) -> String {
    let marker = format!("{node} := Label {{\nwidth: ");
    let Some(pos) = dsl.find(&marker) else {
        return dsl.to_owned();
    };
    let after = pos + marker.len();
    let rest = &dsl[after..];
    let Some(len) = rest.find(|c: char| !c.is_ascii_digit() && c != '.').map(|i| i) else {
        return dsl.to_owned();
    };
    format!("{}310{}", &dsl[..after], &dsl[after + len..])
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
