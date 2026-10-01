//! Board-3 screen 5 — INSPECTOR (rows: inspection × 3, session-links × 1).
//!
//! Web: `features/inspection/InspectionDialog.tsx` + `intent.ts` +
//! `inspection-controller.ts`; opened by `/threads` (`/thread`), `/turn` and
//! `/permissions` (`features/commands/registry.ts:194-235`). The approved
//! board draws ONE inspector: a slash header, the title, the scope line with a
//! refresh glyph, then three stacked cards — the thread graph, the remembered
//! approval scopes, and the conversation link — so every command opens that
//! one surface and its title says which read it is (the web's three `h2`s:
//! "Thread graph" / "Turn state" / "Remembered approvals").
//!
//! Reads (all read-only, `packages/client/src/inspection.ts:58-96`):
//! `thread/graph/get {session_id}` (no `at`: current head, never invented
//! point-in-time semantics), `approval/scopes/list {session_id}`,
//! `turn/state/get {session_id, turn_id}` — each through the client's typed
//! `Method` (`domains/turn.rs`, `domains/approval.rs`), gated on the advertised
//! method AND feature (`registry.ts:194-217`, "This server does not advertise
//! the required inspection method and feature.").
//!
//! The link card is `features/session-links/CopySessionLink.tsx`: the copy
//! writes the clipboard and announces "Conversation link copied."; the value
//! stays VISIBLE in a read-only selectable field, which is the web's
//! clipboard-denied fallback (`:71-85`) made permanent — a native clipboard
//! write cannot be refused silently, and the user can always select the text.
use serde_json::Value;

use octoscode_store::Store;

use super::host::Outcome;
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

pub const GRAPH_METHOD: &str = "thread/graph/get";
pub const SCOPES_METHOD: &str = "approval/scopes/list";
pub const TURN_METHOD: &str = "turn/state/get";
pub const GRAPH_FEATURE: &str = "state.thread_graph.v1";
pub const TURN_FEATURE: &str = "state.turn_state_get.v1";

/// Which read the inspector was opened for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Threads,
    Turn(String),
    Permissions,
}

impl Mode {
    pub fn slash(&self) -> &'static str {
        match self {
            Mode::Threads => "/threads",
            Mode::Turn(_) => "/turn",
            Mode::Permissions => "/permissions",
        }
    }
    /// The web's `h2` per kind (`InspectionDialog.tsx:59-63`).
    pub fn title(&self) -> &'static str {
        match self {
            Mode::Threads => "Thread graph",
            Mode::Turn(_) => "Turn state",
            Mode::Permissions => "Remembered approvals",
        }
    }
}

/// `intent.ts:15-24` (threads) / `:25-39` (turn) / composer intent `:153-157`
/// (permissions takes no arguments). `active_turn` is the live turn, used when
/// `/turn state` names none. `Err` carries the web's exact reason.
pub fn parse(name: &str, args: &str, active_turn: Option<&str>) -> Result<Mode, String> {
    let toks: Vec<&str> = args.split_whitespace().collect();
    match name {
        "threads" | "thread" => match toks.as_slice() {
            [] | ["graph"] | ["graph-get"] => Ok(Mode::Threads),
            _ => Err("Use /threads or /thread graph. Extra arguments are not supported.".into()),
        },
        "turn" => {
            if toks.len() > 2 || !matches!(toks.first(), Some(&"state") | Some(&"state-get")) {
                return Err("Use /turn state [turn UUID].".into());
            }
            match toks.get(1) {
                Some(raw) => normalize_uuid(raw)
                    .map(Mode::Turn)
                    .ok_or_else(|| "Invalid turn UUID. Use /turn state <turn UUID>.".into()),
                None => match active_turn.and_then(normalize_uuid) {
                    Some(id) => Ok(Mode::Turn(id)),
                    None => Err("No active turn to inspect. Use /turn state <turn UUID>.".into()),
                },
            }
        }
        "permissions" | "permission" => {
            if toks.is_empty() {
                Ok(Mode::Permissions)
            } else {
                Err("Arguments for /permissions are not supported in this Web build. Open the command without arguments to use its controls. Nothing was sent to the model.".into())
            }
        }
        _ => Err(format!("/{name} is not an inspection command")),
    }
}

/// `intent.ts:44-55`: strip `urn:uuid:` / `{…}`, re-hyphenate a 32-hex form,
/// require 8-4-4-4-12 hex, lowercase.
pub fn normalize_uuid(raw: &str) -> Option<String> {
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix("urn:uuid:") {
        s = rest;
    }
    if let Some(rest) = s.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        s = rest;
    }
    let lower = s.to_ascii_lowercase();
    let hex: String = if lower.len() == 32 && lower.bytes().all(|b| b.is_ascii_hexdigit()) {
        format!("{}-{}-{}-{}-{}", &lower[0..8], &lower[8..12], &lower[12..16], &lower[16..20], &lower[20..32])
    } else {
        lower
    };
    let parts: Vec<&str> = hex.split('-').collect();
    let lens = [8, 4, 4, 4, 12];
    if parts.len() == 5
        && parts.iter().zip(lens).all(|(p, n)| p.len() == n && p.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        Some(hex)
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRow {
    pub thread_id: String,
    pub root_seq: u64,
    pub status: String,
    pub message_seqs: Vec<u64>,
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graph {
    pub cursor: String,
    pub threads: Vec<ThreadRow>,
    pub orphans: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeRow {
    pub scope: String,
    pub scope_match: String,
    pub decision: String,
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnFacts {
    pub state: String,
    pub thread: Option<String>,
    pub started: Option<String>,
    pub completed: Option<String>,
    pub committed: Vec<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct InspState {
    pub mode: Mode,
    pub loading: bool,
    pub error: Option<String>,
    pub graph: Option<Graph>,
    pub scopes: Option<Vec<ScopeRow>>,
    pub turn: Option<TurnFacts>,
    pub link: String,
    pub copied: bool,
    pub ticket: u64,
    pub session: String,
}

/// The conversation link (`create-saved-session-url.ts:8-35`): the saved
/// reference is the tuple `[workspaceRoot, profileId, sessionId]`, JSON in the
/// `s` parameter. Natively there is no page URL to extend, so the tuple rides
/// the app's own scheme. `None` = "This conversation does not have a complete
/// saved reference yet." (no workspace root reported).
pub fn conversation_link(workspace_root: &str, profile: &str, session: &str) -> Option<String> {
    if workspace_root.trim().is_empty() || profile.trim().is_empty() || session.trim().is_empty() {
        return None;
    }
    let tuple = serde_json::json!([workspace_root, profile, session]).to_string();
    let enc: String = url::form_urlencoded::byte_serialize(tuple.as_bytes()).collect();
    Some(format!("octoscode://session?s={enc}"))
}

// --------------------------------------------------------------- transport

fn project_graph(v: &Value) -> Option<Graph> {
    let cursor = v.get("cursor")?;
    let cursor = format!(
        "{}:{}",
        cursor.get("stream").and_then(|s| s.as_str()).unwrap_or(""),
        cursor.get("seq").and_then(|s| s.as_u64()).unwrap_or(0)
    );
    let mut seen = std::collections::HashSet::new();
    let mut threads = Vec::new();
    for t in v.get("threads")?.as_array()? {
        let id = t.get("thread_id")?.as_str()?.to_owned();
        if !seen.insert(id.clone()) {
            return None; // a duplicate thread_id invalidates the whole result
        }
        threads.push(ThreadRow {
            thread_id: id,
            root_seq: t.get("root_seq")?.as_u64()?,
            status: t.get("status").and_then(|s| s.as_str()).unwrap_or("unknown").to_owned(),
            message_seqs: t
                .get("message_seqs")?
                .as_array()?
                .iter()
                .map(|x| x.as_u64())
                .collect::<Option<Vec<_>>>()?,
            turn_id: t.get("turn_id").and_then(|s| s.as_str()).map(str::to_owned),
        });
    }
    let orphans = v
        .get("orphans")
        .and_then(|o| o.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_u64()).collect())
        .unwrap_or_default();
    Some(Graph { cursor, threads, orphans })
}

fn project_scopes(v: &Value, session: &str) -> Option<Vec<ScopeRow>> {
    let mut out = Vec::new();
    for s in v.get("scopes")?.as_array()? {
        // inspection-results.ts:70-101 — every row must belong to THIS Session.
        if s.get("session_id")?.as_str()? != session {
            return None;
        }
        let decision = match s.get("decision")? {
            Value::String(d) => d.clone(),
            other => other
                .as_object()
                .and_then(|o| o.keys().next().cloned())
                .unwrap_or_else(|| other.to_string()),
        };
        out.push(ScopeRow {
            scope: s.get("scope")?.as_str()?.to_owned(),
            scope_match: s.get("scope_match").and_then(|m| m.as_str()).unwrap_or("").to_owned(),
            decision,
            turn_id: s.get("turn_id").and_then(|t| t.as_str()).map(str::to_owned),
        });
    }
    Some(out)
}

fn project_turn(v: &Value) -> Option<TurnFacts> {
    Some(TurnFacts {
        state: v.get("state")?.as_str()?.to_owned(),
        thread: v.get("thread_id").and_then(|s| s.as_str()).map(str::to_owned),
        started: v.get("started_at").and_then(|s| s.as_str()).map(str::to_owned),
        completed: v.get("completed_at").and_then(|s| s.as_str()).map(str::to_owned),
        committed: v
            .get("committed_seqs")
            .and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_u64()).collect())
            .unwrap_or_default(),
    })
}

/// The advertised gate (`registry.ts:194-235`).
pub fn gate(methods: &[String], features: &[String], method: &str, feature: Option<&str>) -> bool {
    methods.iter().any(|m| m == method)
        && feature.is_none_or(|f| features.iter().any(|x| x.eq_ignore_ascii_case(f)))
}

/// Load every read the open mode shows, through the typed client methods.
pub async fn load(conv: &crate::flow::Conversation) -> Result<String, String> {
    use octoscode_client::domains::approval::ApprovalScopesList;
    use octoscode_client::domains::turn::{ThreadGraphGet, TurnStateGet};
    let session = conv.session_id();
    let (mode, ticket) = {
        let mut st = super::host::state();
        st.insp.ticket += 1;
        st.insp.loading = true;
        st.insp.error = None;
        st.insp.session = session.clone();
        (st.insp.mode.clone(), st.insp.ticket)
    };
    let methods = conv.store.domains.config.supported_methods();
    let features = conv.store.capabilities();
    let key = octos_core::SessionKey(session.clone());
    let mut errors: Vec<String> = Vec::new();

    let graph = if gate(&methods, &features, GRAPH_METHOD, Some(GRAPH_FEATURE)) {
        match conv
            .client()
            .call::<ThreadGraphGet>(octos_core::ui_protocol::ThreadGraphGetParams { session_id: key.clone(), at: None })
            .await
        {
            Ok(r) => serde_json::to_value(&r).ok().and_then(|v| project_graph(&v)),
            Err(e) => {
                errors.push(format!("thread/graph/get: {e}"));
                None
            }
        }
    } else {
        errors.push("This server does not advertise the required inspection method and feature.".into());
        None
    };
    let scopes = if gate(&methods, &features, SCOPES_METHOD, None) {
        match conv
            .client()
            .call::<ApprovalScopesList>(octos_core::ui_protocol::ApprovalScopesListParams { session_id: key.clone() })
            .await
        {
            Ok(r) => serde_json::to_value(&r).ok().and_then(|v| project_scopes(&v, &session)),
            Err(e) => {
                errors.push(format!("approval/scopes/list: {e}"));
                None
            }
        }
    } else {
        None
    };
    let turn = match &mode {
        Mode::Turn(id) if gate(&methods, &features, TURN_METHOD, Some(TURN_FEATURE)) => {
            match uuid_turn_id(id) {
                Some(turn_id) => match conv
                    .client()
                    .call::<TurnStateGet>(octos_core::ui_protocol::TurnStateGetParams { session_id: key.clone(), turn_id })
                    .await
                {
                    Ok(r) => serde_json::to_value(&r).ok().and_then(|v| project_turn(&v)),
                    Err(e) => {
                        errors.push(format!("turn/state/get: {e}"));
                        None
                    }
                },
                None => None,
            }
        }
        _ => None,
    };
    let root = conv
        .store
        .domains
        .session
        .workspace_root(&session)
        .unwrap_or_default();
    let link = conversation_link(&root, &conv.profile(), &session).unwrap_or_default();

    let mut st = super::host::state();
    if st.insp.ticket != ticket {
        return Ok("stale inspection reply dropped".into());
    }
    st.insp.loading = false;
    st.insp.graph = graph;
    st.insp.scopes = scopes;
    st.insp.turn = turn;
    st.insp.link = link;
    st.insp.error = (!errors.is_empty()).then(|| {
        // The web replaces remote errors with static copy
        // (inspection-controller.ts:127-136).
        if errors.iter().any(|e| e.contains("does not advertise")) {
            "This server does not advertise the required inspection method and feature.".to_owned()
        } else {
            "The server could not return a valid inspection for this owner. Retry the read.".to_owned()
        }
    });
    let n = st.insp.graph.as_ref().map(|g| g.threads.len()).unwrap_or(0);
    Ok(format!("{n} threads, {} scopes", st.insp.scopes.as_ref().map(Vec::len).unwrap_or(0)))
}

fn uuid_turn_id(id: &str) -> Option<octos_core::TurnId> {
    serde_json::from_value(Value::String(id.to_owned())).ok()
}

// ------------------------------------------------------------------ actions

pub fn perform(st: &mut InspState, action: &str) -> Outcome {
    match action {
        "b3.insp.refresh" => {
            if st.loading {
                return Outcome::Done;
            }
            st.loading = true;
            st.copied = false;
            Outcome::Spawn(super::host::Job::InspectorLoad)
        }
        "b3.insp.copy" => {
            if st.link.is_empty() {
                st.error = Some("This conversation does not have a complete saved reference yet.".into());
                return Outcome::Done;
            }
            st.copied = true;
            Outcome::Clipboard(st.link.clone())
        }
        _ => Outcome::Unrouted,
    }
}

// -------------------------------------------------------------------- view

fn short(id: &str) -> String {
    id.chars().take(8).collect()
}


pub fn build(d: &mut Dsl, st: &InspState, frame: &Frame, _store: &Store) {
    let width = frame.dialog_w(800.0);
    ui::shell_open(d, frame, width);
    // The slash header + close (board: mono "/thread", the X at the right).
    let row = d.anon();
    d.view(&row, "width: Fill height: 28 flow: Right align: Align{x: 0.0 y: 0.5}");
    d.text("b3_insp_slash", st.mode.slash(), &Txt::new(14.0, Face::Mono, tok::TEXT).w(W::Fill));
    ui::close_glyph(d, "b3.close");
    d.close();
    d.gap(W::Fill, 6.0);
    d.text("b3_title", st.mode.title(), &ui::title().w(W::Fill));
    // Scope line + refresh glyph (`InspectionDialog.tsx:70-76`).
    let scope_row = d.anon();
    d.view(&scope_row, "width: Fill height: 28 flow: Right align: Align{x: 0.0 y: 0.5}");
    let mut scope = format!("Session: {}", st.session);
    if let Mode::Turn(id) = &st.mode {
        scope.push_str(&format!(" · Turn: {}", short(id)));
    }
    d.text("b3_insp_scope", &scope, &Txt::new(12.0, Face::Mono, tok::MUTED).w(W::Fill));
    ui::icon_button(d, "b3_insp_refresh_glyph", "b3_refresh.svg", 16.0, "b3.insp.refresh");
    d.close();
    d.text(
        "b3_insp_note",
        "Read-only server snapshot. Reading does not change the conversation, queued prompts, or active Session.",
        &Txt::new(11.5, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    d.gap(W::Fill, 12.0);
    ui::body_open(d, frame, width, 175.0);
    let body = d.anon();
    d.view(&body, "width: Fill height: Fit flow: Down spacing: 12");
    if st.loading {
        d.text("b3_insp_loading", "Reading the captured Session…", &ui::meta());
    }
    if let Some(e) = &st.error {
        d.text("b3_insp_error", e, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
    }
    if let (Mode::Turn(id), Some(t)) = (&st.mode, &st.turn) {
        turn_card(d, id, t);
    }
    graph_card(d, st);
    scopes_card(d, st);
    link_card(d, st);
    d.close();
    ui::body_close(d);
    // The board's bottom-right Refresh pill.
    d.gap(W::Fill, 12.0);
    let foot = d.anon();
    d.view(&foot, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5}");
    let kind = if st.loading { Btn::Disabled } else { Btn::Outline };
    let label = if st.loading { "Reading…" } else { "Refresh" };
    d.button("b3_insp_refresh", label, "b3.insp.refresh", kind, W::Fit, 34.0);
    d.close();
    ui::shell_close(d);
}

fn graph_card(d: &mut Dsl, st: &InspState) {
    ui::card_open(d, "b3_insp_graph", 4.0);
    ui::section_title(d, "b3_insp_graph_title", "Thread graph");
    match &st.graph {
        None => d.text("", "No thread graph read yet.", &ui::meta()),
        Some(g) if g.threads.is_empty() => {
            d.text("b3_insp_graph_empty", "No threads returned for this Session.", &ui::meta())
        }
        Some(g) => {
            d.text(
                "b3_insp_graph_summary",
                &format!(
                    "{} thread{} · Cursor {}",
                    g.threads.len(),
                    if g.threads.len() == 1 { "" } else { "s" },
                    g.cursor
                ),
                &ui::micro().w(W::Fill),
            );
            d.gap(W::Fill, 4.0);
            let last = g.threads.len() - 1;
            for (i, t) in g.threads.iter().enumerate() {
                let rid = format!("b3_insp_thread_{i}");
                d.view(&rid, "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
                let label = if i == last { "current".to_owned() } else { format!("#{}", i + 1) };
                d.text("", &label, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Px(64.0)));
                d.text("", &short(&t.thread_id), &Txt::new(12.5, Face::Mono, tok::TEXT).w(W::Px(84.0)));
                d.text(
                    "",
                    &format!("{} · root seq {}", t.status, t.root_seq),
                    &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill),
                );
                let n = t.message_seqs.len();
                d.text(
                    "",
                    &format!("{n} message{}", if n == 1 { "" } else { "s" }),
                    &Txt::new(12.0, Face::Regular, tok::TEXT),
                );
                d.close();
            }
            if !g.orphans.is_empty() {
                let list: Vec<String> = g.orphans.iter().map(|o| o.to_string()).collect();
                d.text(
                    "b3_insp_orphans",
                    &format!("Orphan message sequences: {}", list.join(", ")),
                    &ui::micro().w(W::Fill),
                );
            }
        }
    }
    d.close();
}

fn decision_tag(d: &mut Dsl, id: &str, decision: &str) {
    let lower = decision.to_ascii_lowercase();
    let (text, fg, bg) = match lower.as_str() {
        "allow" | "allowed" | "approve" | "approved" | "accept" => ("allowed".to_owned(), tok::BLUE, tok::BLUE_BG),
        "deny" | "denied" | "reject" | "rejected" => ("denied".to_owned(), tok::RED, tok::RED_BG),
        other => (other.to_owned(), tok::MUTED, tok::SURFACE2),
    };
    d.chip(id, &text, fg, bg, None, false);
}

fn scopes_card(d: &mut Dsl, st: &InspState) {
    ui::card_open(d, "b3_insp_scopes", 4.0);
    ui::section_title(d, "b3_insp_scopes_title", "Approval scopes");
    match &st.scopes {
        None => d.text("", "Approval scopes were not read.", &ui::meta()),
        Some(rows) if rows.is_empty() => {
            d.text("b3_insp_scopes_empty", "No remembered approval scopes for this Session.", &ui::meta())
        }
        Some(rows) => {
            d.text(
                "b3_insp_scopes_summary",
                &format!(
                    "{} remembered approval scope{} for this Session.",
                    rows.len(),
                    if rows.len() == 1 { "" } else { "s" }
                ),
                &ui::micro().w(W::Fill),
            );
            for (i, s) in rows.iter().enumerate() {
                let rid = format!("b3_insp_scope_{i}");
                d.view(&rid, "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
                let mut name: String = s.scope.clone();
                if let Some(first) = name.get(0..1) {
                    name = format!("{}{}", first.to_uppercase(), &name[1..]);
                }
                d.text("", &name, &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Px(110.0)));
                let m = if s.scope_match.is_empty() { "(empty)".to_owned() } else { s.scope_match.clone() };
                d.text("", &super::inventory::fit(&m, 260.0, 12.0, true), &Txt::new(12.0, Face::Mono, tok::MUTED).w(W::Fill));
                decision_tag(d, &format!("{rid}_decision"), &s.decision);
                d.close();
            }
        }
    }
    d.text(
        "b3_insp_scopes_note",
        "This view lists server-owned decisions only. It does not clear or change permissions.",
        &Txt::new(11.5, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    d.close();
}

fn link_card(d: &mut Dsl, st: &InspState) {
    ui::card_open(d, "b3_insp_link", 8.0);
    let head = d.anon();
    d.view(&head, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5}");
    ui::section_title(d, "b3_insp_link_title", "Copy link");
    d.button(
        "b3_insp_copy_btn",
        if st.copied { "Copied" } else { "Copy conversation link" },
        "b3.insp.copy",
        if st.link.is_empty() { Btn::Disabled } else { Btn::Outline },
        W::Fit,
        30.0,
    );
    d.close();
    if st.link.is_empty() {
        d.text(
            "b3_insp_link_missing",
            "This conversation does not have a complete saved reference yet.",
            &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
        );
    } else {
        let shown = super::inventory::fit(&st.link, 640.0, 12.0, true);
        ui::mono_box(d, "b3_insp_link_value", &shown, Some(("b3_insp_copy", "b3_copy.svg", "b3.insp.copy")));
    }
    if st.copied {
        d.text("b3_insp_copied", "Conversation link copied.", &Txt::new(12.0, Face::Regular, tok::GREEN));
    }
    d.close();
}

/// `InspectionDialog.tsx:201-247`: the turn's facts.
fn turn_card(d: &mut Dsl, id: &str, t: &TurnFacts) {
    ui::card_open(d, "b3_insp_turn", 4.0);
    ui::section_title(d, "b3_insp_turn_title", "Turn state");
    let fact = |d: &mut Dsl, k: &str, v: &str| {
        let r = d.anon();
        d.view(&r, "width: Fill height: 26 flow: Right align: Align{x: 0.0 y: 0.5}");
        d.text("", k, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Px(190.0)));
        d.text("", v, &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill));
        d.close();
    };
    fact(d, "Turn", &short(id));
    fact(d, "State", &t.state);
    if let Some(th) = &t.thread {
        fact(d, "Thread", &short(th));
    }
    if let Some(s) = &t.started {
        fact(d, "Started", s);
    }
    if let Some(c) = &t.completed {
        fact(d, "Completed", c);
    }
    let seqs = if t.committed.is_empty() {
        "None".to_owned()
    } else {
        t.committed.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")
    };
    fact(d, "Committed message sequences", &seqs);
    if t.state == "unknown" {
        d.text(
            "",
            "The Session is known, but the server has no lifecycle record for this turn.",
            &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
        );
    }
    d.close();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_slash_grammar_is_the_web_intent() {
        assert_eq!(parse("threads", "", None), Ok(Mode::Threads));
        assert_eq!(parse("thread", "graph", None), Ok(Mode::Threads));
        assert!(parse("threads", "all", None).unwrap_err().contains("Extra arguments"));
        let id = "01920000-0000-7000-8000-00000000023a";
        assert_eq!(parse("turn", "state", Some(id)), Ok(Mode::Turn(id.into())));
        assert_eq!(
            parse("turn", "state {0192000000007000800000000000023A}", None),
            Ok(Mode::Turn(id.into())),
            "braces stripped, 32-hex re-hyphenated, lowercased"
        );
        assert_eq!(
            parse("turn", "state", None).unwrap_err(),
            "No active turn to inspect. Use /turn state <turn UUID>."
        );
        assert_eq!(parse("turn", id, None).unwrap_err(), "Use /turn state [turn UUID].");
        assert_eq!(
            parse("turn", "state not-a-uuid", None).unwrap_err(),
            "Invalid turn UUID. Use /turn state <turn UUID>."
        );
        assert_eq!(parse("permissions", "", None), Ok(Mode::Permissions));
        assert!(parse("permissions", "clear", None).is_err());
    }

    #[test]
    fn the_recorded_graph_projects_without_inventing_depth() {
        // r23-conversation-a6ea8505.jsonl line 12 (fp4b3_inspection.rs).
        let v = json!({
            "cursor": {"seq": 278, "stream": "dsflash:main"}, "orphans": [], "session_id": "dsflash:main",
            "threads": [
                {"message_seqs": [0, 1], "root_seq": 0, "status": "unknown", "thread_id": "01920000-0000-7000-8000-00000000023b"},
                {"message_seqs": [2, 3, 4, 5], "root_seq": 2, "status": "unknown", "thread_id": "01920000-0000-7000-8000-00000000023c"}
            ]
        });
        let g = project_graph(&v).expect("projects");
        assert_eq!(g.cursor, "dsflash:main:278");
        assert_eq!(g.threads[1].message_seqs.len(), 4);
        let mut dup = v.clone();
        dup["threads"][1]["thread_id"] = dup["threads"][0]["thread_id"].clone();
        assert!(project_graph(&dup).is_none(), "a duplicate thread id invalidates the read");
    }

    #[test]
    fn scopes_from_another_session_are_refused() {
        let v = json!({"scopes": [{"session_id": "s", "scope": "session", "scope_match": "", "decision": "allow"}]});
        assert_eq!(project_scopes(&v, "s").unwrap()[0].decision, "allow");
        assert!(project_scopes(&v, "other").is_none());
    }

    #[test]
    fn the_link_is_the_saved_reference_tuple() {
        let link = conversation_link("/home/user/octos", "dsflash", "dsflash:main").unwrap();
        assert!(link.starts_with("octoscode://session?s="));
        let enc = link.trim_start_matches("octoscode://session?s=");
        let dec: String = url::form_urlencoded::parse(format!("s={enc}").as_bytes())
            .next()
            .unwrap()
            .1
            .into_owned();
        assert_eq!(dec, r#"["/home/user/octos","dsflash","dsflash:main"]"#);
        assert!(conversation_link("", "p", "s").is_none(), "incomplete reference");
    }

    #[test]
    fn copy_fails_closed_without_a_link_and_clipboards_with_one() {
        let mut st = InspState::default();
        assert_eq!(perform(&mut st, "b3.insp.copy"), Outcome::Done);
        assert!(st.error.as_deref().unwrap().contains("complete saved reference"));
        st.link = "octoscode://session?s=x".into();
        assert_eq!(perform(&mut st, "b3.insp.copy"), Outcome::Clipboard("octoscode://session?s=x".into()));
        assert!(st.copied);
    }

    #[test]
    fn the_gate_needs_method_and_feature() {
        let m = vec![GRAPH_METHOD.to_owned()];
        assert!(!gate(&m, &[], GRAPH_METHOD, Some(GRAPH_FEATURE)));
        assert!(gate(&m, &[GRAPH_FEATURE.to_owned()], GRAPH_METHOD, Some(GRAPH_FEATURE)));
    }

    #[test]
    fn the_dialog_lowers_balanced_with_its_controls() {
        let st = InspState {
            graph: Some(Graph { cursor: "s:1".into(), threads: vec![ThreadRow { thread_id: "abcdef0123".into(), root_seq: 0, status: "unknown".into(), message_seqs: vec![0, 1], turn_id: None }], orphans: vec![] }),
            scopes: Some(vec![ScopeRow { scope: "session".into(), scope_match: "".into(), decision: "allow".into(), turn_id: None }]),
            link: "octoscode://session?s=x".into(),
            session: "dsflash:main".into(),
            ..Default::default()
        };
        for frame in [Frame::DESKTOP, Frame { avail_w: 412.0, avail_h: 794.0 }] {
            let mut d = Dsl::new();
            build(&mut d, &st, &frame, &Store::new());
            let dsl = d.finish();
            assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
            let taps = crate::screens::taps::wired_taps(&dsl);
            for ev in ["b3.close", "b3.insp.refresh", "b3.insp.copy"] {
                assert!(taps.iter().any(|(_, e)| e == ev), "{ev}");
            }
        }
    }
}
