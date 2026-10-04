//! Board-3 screen 12 (left) — SESSION SWITCHER (row: session × 1, "switch to
//! another Session (fresh open without the other session's replay cursor)").
//!
//! Web: `/sessions` (alias `/ss`, `registry.ts:338-345`) reveals the session
//! browser (`features/shell/ProductSidebar.tsx`); a click on a row opens it
//! FRESH — `session/open {session_id, profile_id, cwd}` with NO `after`
//! cursor, then the canonical hydrate (`session-record-manager.ts:505-530`,
//! `active-session-runtime.ts:160-174`); the current row only cancels
//! (`App.tsx:1758-1786`). Rows carry the title (server title, else
//! `last_prompt`, else `Session <last 8 of id>` — `SessionSidebar.tsx:245-249`),
//! the workspace tag and the relative time (`relative-time.ts:1-17`).
//!
//! The approved board draws this as the compact "Open a different session"
//! panel with the current session greyed and checked.
use octoscode_store::Store;

use super::host::Outcome;
use super::ui::{self, tok, Dsl, Face, Frame, Txt, W};
use crate::i18n::{tr, tr1, tr_with};

#[derive(Debug, Clone, Default)]
pub struct SwitchState {
    pub catalog: Option<Vec<HistoryItem>>,
    pub next_offset: Option<usize>,
    pub offset: usize,
    pub total: usize,
    pub ticket: u64,
    pub coverage: String,
    pub loading: bool,
    pub error: Option<String>,
    pub opening: Option<String>,
    /// A8 — the row whose delete waits for its confirmation.
    pub confirm_delete: Option<String>,
    /// A8 — the one delete in flight (`deletingSessionRef`, single-flight).
    pub deleting: Option<String>,
    /// A13 — what failed, in plain words, with the cause it was recorded
    /// for (`ui::dialog_error`).
    pub failed: Option<(&'static str, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HistoryTarget {
    pub profile: String,
    pub session: String,
    pub cwd: Option<String>,
}
impl HistoryTarget {
    pub fn key(&self) -> String { serde_json::to_string(self).unwrap_or_default() }
}
#[derive(Debug, Clone)]
pub struct HistoryItem { pub target: HistoryTarget, pub title: String, pub updated_at: Option<String> }

fn catalog_rows(st: &SwitchState, store: &Store) -> Vec<Row> {
    let Some(catalog) = st.catalog.as_ref() else { return rows(store) };
    let active = store.active_session();
    let current_root = active.as_deref().and_then(|id| store.domains.session.workspace_root(id));
    let current_profile = store.domains.profile.current();
    catalog.iter().map(|item| Row {
        id: item.target.key(), title:item.title.clone(),
        tag:format!("{} · {}", item.target.profile, item.target.cwd.as_deref().unwrap_or("Profile history")),
        when:item.updated_at.as_deref().and_then(ui::parse_iso_ms).map(|ms| ui::rel_time(ui::now_ms(), ms)).unwrap_or_default(),
        current:active.as_deref() == Some(item.target.session.as_str()) && current_profile.as_deref() == Some(item.target.profile.as_str()) && item.target.cwd == current_root,
    }).collect()
}

fn parse_history(v: &serde_json::Value) -> Result<Vec<HistoryItem>, String> {
    let rows = v["sessions"].as_array().ok_or("Invalid history catalog")?;
    rows.iter().map(|row| {
        let profile = row["profile_id"].as_str().ok_or("Missing history profile")?;
        let session = row["id"].as_str().ok_or("Missing history session")?;
        if !session.starts_with(&format!("{profile}:")) { return Err("History scope mismatch".into()); }
        let cwd = match &row["workspace_root"] {
            serde_json::Value::Null => None,
            serde_json::Value::String(p) if !p.is_empty() => Some(p.clone()),
            _ => return Err("Invalid history workspace".into()),
        };
        let title = row["title"].as_str().or_else(|| row["last_prompt"].as_str()).filter(|s| !s.is_empty()).map(str::to_owned).unwrap_or_else(|| known_session_title(session));
        Ok(HistoryItem { target:HistoryTarget {profile:profile.into(),session:session.into(),cwd}, title,
            updated_at:row["updated_at"].as_str().map(str::to_owned) })
    }).collect()
}

/// A13 — the plain lead over a failed open (the cause shows muted under it).
pub const OPEN_FAILED: &str = "Couldn't open that session.";

/// A8 — `deleteSession` is offered only when `session/delete` is advertised
/// (`use-workspace-product.ts:96-107`: `supportsMethod(…SESSION_DELETE)`).
pub fn delete_offered(store: &Store) -> bool {
    store.domains.config.supported_methods().iter().any(|m| m == "session/delete")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub tag: String,
    pub when: String,
    pub current: bool,
}

/// `SessionSidebar.tsx:245-249` title fallback.
pub fn title_of(s: &octoscode_store::Session) -> String {
    if let Some(t) = s.title.as_ref().filter(|t| !t.trim().is_empty()) {
        return t.trim().to_owned();
    }
    if let Some(p) = s.last_prompt.as_ref().filter(|t| !t.trim().is_empty()) {
        return p.trim().to_owned();
    }
    known_session_title(&s.id)
}

/// `knownSessionTitle` (`SessionSidebar.tsx:245-249`): the id's last
/// `:`-segment, cut to its last 8 characters when longer than 10.
pub fn known_session_title(id: &str) -> String {
    let leaf = id.split(':').next_back().map(str::trim).filter(|l| !l.is_empty()).unwrap_or(id.trim());
    let n = leaf.chars().count();
    let compact: String = if n > 10 { leaf.chars().skip(n - 8).collect() } else { leaf.to_owned() };
    tr_with("Session {id}", &[("id", if compact.is_empty() { tr("unknown") } else { &compact })])
}

/// The rows, newest first, the current one marked.
pub fn rows(store: &Store) -> Vec<Row> {
    let active = store.active_session();
    let now = ui::now_ms();
    let mut list = store.sessions();
    list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    list.into_iter()
        .map(|s| {
            let root = store.domains.session.workspace_root(&s.id);
            let tag = match &root {
                Some(r) if !r.is_empty() => ui::leaf(r),
                _ => s.id.split(':').next().unwrap_or("").to_owned(),
            };
            let when = s
                .updated_at
                .as_deref()
                .and_then(ui::parse_iso_ms)
                .map(|ms| ui::rel_time(now, ms))
                .unwrap_or_default();
            Row {
                current: active.as_deref() == Some(s.id.as_str()),
                title: title_of(&s),
                id: s.id,
                tag,
                when,
            }
        })
        .collect()
}

pub async fn load(conv: &crate::flow::Conversation) -> Result<String, String> {
    let identity = conv.resource_identity();
    let (ticket, offset) = {
        let mut st = super::host::state();
        st.switch.loading = true; st.switch.ticket += 1;
        (st.switch.ticket, st.switch.offset)
    };
    if conv.store.domains.config.supported_methods().iter().any(|m| m == "session/history/list") {
        let mut roots = crate::screens::recents::history_workspaces(&*crate::screens::recents::store(), &crate::screens::recents::endpoint());
        if let Some(root) = conv.store.domains.session.workspace_root(&conv.session_id()) { roots.push(root); }
        roots.sort(); roots.dedup(); roots.truncate(128);
        let result = conv.client().request("session/history/list", serde_json::json!({"workspaces":roots,"offset":offset,"limit":100})).await;
        let mut st = super::host::state();
        if st.switch.ticket != ticket || conv.resource_identity() != identity { return Ok("stale history reply discarded".into()); }
        st.switch.loading = false;
        match result {
            Ok(v) => {
                let items = parse_history(&v)?;
                if offset == 0 { st.switch.catalog = Some(items); }
                else { st.switch.catalog.get_or_insert_with(Vec::new).extend(items); }
                st.switch.next_offset = v["next_offset"].as_u64().map(|n| n as usize);
                st.switch.total = v["total"].as_u64().unwrap_or_default() as usize;
                let skipped = v["unavailable_workspaces"].as_array().map(Vec::len).unwrap_or(0);
                st.switch.coverage = if crate::i18n::is_zh() {
                    format!("已授权 Profile 与已知项目的会话，共 {} 个。未列出的旧项目可通过添加工作区查找。{} 个路径暂不可用。", st.switch.total, skipped)
                } else {
                    format!("{} sessions in authorized Profiles and known projects. Add a workspace to include an older project. {skipped} paths unavailable.", st.switch.total)
                };
                st.switch.error = None;
                Ok(format!("{} history rows", st.switch.catalog.as_ref().map(Vec::len).unwrap_or(0)))
            }
            Err(e) => { st.switch.error = Some("Could not load session history.".into()); Err(e.to_string()) }
        }
    } else {
        let r = conv.refresh_sessions().await;
        let mut st = super::host::state();
        if conv.resource_identity() != identity || st.switch.ticket != ticket { return Ok("stale history reply discarded".into()); }
        st.switch.loading = false;
        st.switch.catalog = None;
        st.switch.coverage = "This server lists the current workspace only; update the server for history across projects.".into();
        r.map(|n| format!("{n} sessions")).map_err(|e| e.to_string())
    }
}

/// Open `id` fresh (no replay cursor: `open_workspace_as` sends `after:
/// None`), in its own workspace when the store knows it.
pub async fn open(conv: &crate::flow::Conversation, id: String) -> Result<String, String> {
    let target = {
        let st = super::host::state();
        st.switch.catalog.as_ref().and_then(|rows| rows.iter().find(|r| r.target.key() == id)).map(|r| r.target.clone())
    };
    let r = if let Some(target) = target {
        conv.open_history_target(&target).await
    } else if id.starts_with('{') {
        Err("History catalog changed; refresh and select the session again.".into())
    } else {
        let cwd = conv.store.domains.session.workspace_root(&id);
        conv.open_session(&id, cwd).await
    };
    let mut st = super::host::state();
    st.switch.opening = None;
    match r {
        Ok(opened) => {
            st.open = None;
            Ok(format!("opened {opened}"))
        }
        Err(e) => {
            st.switch.error = Some(e.clone());
            st.switch.failed = Some((OPEN_FAILED, e.clone()));
            Err(e)
        }
    }
}

/// A8 — `session/delete` (`use-workspace-product.ts:96-125`): never the
/// open Session, one at a time; the row goes on SUCCESS, a failure keeps it
/// and says why (the server's own message, as the web's `errorMessage`).
pub async fn delete(conv: &crate::flow::Conversation, id: String) -> Result<String, String> {
    if conv.session_id() == id {
        super::host::state().switch.deleting = None;
        return Err("the open Session is never deleted".into());
    }
    let r = conv
        .client()
        .call::<octoscode_client::domains::session::SessionDelete>(octos_core::ui_protocol::SessionDeleteParams {
            session_id: id.clone(),
        })
        .await;
    let mut st = super::host::state();
    st.switch.deleting = None;
    match r {
        Ok(_) => {
            drop(st);
            conv.store.domains.session.forget(&id);
            crate::screens::drafts::restore_for(&crate::screens::drafts::key_of(conv, &id), "");
            // A22 row 216 — the deleted Session's record retires: its
            // returned prompts and images go (`retire`, session-composer-drafts.ts:196-201).
            crate::screens::composer_drafts::retire(conv, &id);
            Ok(format!("deleted {id}"))
        }
        Err(e) => {
            let msg = match &e {
                octoscode_client::ClientError::Rpc { error, .. } => error.message.clone(),
                other => other.to_string(),
            };
            st.switch.error = Some(tr1("Couldn't delete the session: {value0}", &msg));
            Err(msg)
        }
    }
}

pub fn perform(st: &mut SwitchState, action: &str, index: usize, store: &Store) -> Outcome {
    if action.starts_with("b3.switch.delete") && st.catalog.is_some() { return Outcome::Done; }
    match action {
        "b3.switch.more" => {
            if st.loading { return Outcome::Done; }
            if let Some(offset) = st.next_offset { st.offset = offset; return Outcome::Spawn(super::host::Job::SwitchLoad); }
            return Outcome::Done;
        }
        "b3.switch.refresh" => { st.offset = 0; return Outcome::Spawn(super::host::Job::SwitchLoad); }
        "b3.switch.delete" => {
            let rows = catalog_rows(st, store);
            let Some(row) = rows.get(index) else { return Outcome::Done };
            if row.current || !delete_offered(store) || st.deleting.is_some() {
                return Outcome::Done;
            }
            st.error = None;
            st.confirm_delete = Some(row.id.clone());
            return Outcome::Done;
        }
        "b3.switch.delete.cancel" => {
            st.confirm_delete = None;
            return Outcome::Done;
        }
        "b3.switch.delete.confirm" => {
            let Some(id) = st.confirm_delete.take() else { return Outcome::Done };
            if st.deleting.is_some() || store.active_session().as_deref() == Some(id.as_str()) {
                return Outcome::Done;
            }
            st.deleting = Some(id.clone());
            return Outcome::Spawn(super::host::Job::SwitchDelete(id));
        }
        _ => {}
    }
    match action {
        "b3.switch.open" => {
            let rows = catalog_rows(st, store);
            let Some(row) = rows.get(index) else { return Outcome::Done };
            if row.current {
                // Selecting the current session is a no-op (`App.tsx:1758`).
                return Outcome::Close;
            }
            st.opening = Some(row.id.clone());
            Outcome::Spawn(super::host::Job::SwitchOpen(row.id.clone()))
        }
        _ => Outcome::Unrouted,
    }
}


/// The panel body (shared by the dialog and the vim split view).
pub fn panel(d: &mut Dsl, st: &SwitchState, store: &Store, inner_w: f64) {
    let rows = catalog_rows(st, store);
    if !st.coverage.is_empty() { d.text("b3_switch_coverage", &st.coverage, &ui::meta().w(W::Fill).wrap()); d.gap(W::Fill, 8.0); }
    d.button("b3_switch_refresh", tr("Refresh history"), "b3.switch.refresh", ui::Btn::Outline, W::Fit, 30.0);
    if st.next_offset.is_some() {
        d.button("b3_switch_more", tr("Load more"), "b3.switch.more", ui::Btn::Outline, W::Fit, 30.0);
    }
    // A phone-narrow list: the delete confirmation sits on the row's meta
    // line (bottom right), so the title keeps its width.
    let narrow = inner_w > 0.0 && inner_w < 360.0;
    if st.loading && rows.is_empty() {
        d.text("b3_switch_loading", tr("Loading sessions…"), &ui::meta());
    }
    if let Some(e) = &st.error {
        ui::dialog_error(d, "b3_switch_error", e, st.failed.as_ref(), OPEN_FAILED);
    }
    if rows.is_empty() && !st.loading {
        d.text("b3_switch_empty", tr("No sessions yet."), &ui::meta());
    }
    let list = d.anon();
    d.view(&list, "width: Fill height: Fit flow: Down");
    for (i, r) in rows.iter().enumerate() {
        let rid = format!("b3_switch_row_{i}");
        // The board: the current session is a grey card with a black check;
        // the others are open rows split by hairlines.
        if i > 0 && !r.current && !rows[i - 1].current {
            d.hairline();
        }
        let (fill, border) = if r.current { (tok::SURFACE2, Some(tok::HAIRLINE)) } else { (tok::TRANSPARENT, None) };
        d.surface(&rid, "width: Fill height: Fit flow: Overlay", fill, 12.0, border);
        // A8 — the trailing delete control sits on a layer over the row, so
        // the row's text keeps clear of it (a phone-width title ran under the
        // "Delete? Cancel Delete" pill): the right inset is the control's
        // width plus its 8 px edge and a 6 px gap.
        let confirming = st.confirm_delete.as_deref() == Some(r.id.as_str());
        let trailing = if !r.current && st.catalog.is_none() && delete_offered(store) {
            if st.deleting.as_deref() == Some(r.id.as_str()) {
                ui::text_w(tr("Deleting…"), 12.0, Face::Regular) + 14.0
            } else if confirming && !narrow {
                10.0 + ui::text_w(tr("Delete?"), 12.0, Face::Medium)
                    + 4.0
                    + ui::text_w(tr("Cancel"), 12.5, Face::Regular)
                    + 4.0
                    + 12.0
                    + ui::text_w(tr("Delete"), 12.5, Face::Medium)
                    + 4.0
                    + 14.0
            } else {
                28.0 + 14.0
            }
        } else {
            0.0
        };
        let row = d.anon();
        d.view(
            &row,
            &format!(
                "width: Fill height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 10 padding: Inset{{left: 12 right: {} top: 12 bottom: 12}}",
                trailing.max(12.0).ceil()
            ),
        );
        let col = d.anon();
        d.view(&col, "width: Fill height: Fit flow: Down spacing: 8");
        let color = if r.current { tok::MUTED } else { tok::TEXT };
        d.text(&format!("{rid}_title"), &r.title, &Txt::new(14.0, Face::Regular, color).w(W::Fill).wrap());
        let meta = d.anon();
        d.view(&meta, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        d.chip(&format!("{rid}_tag"), &r.tag, tok::TEXT, tok::CHIP, None, true);
        let opening = st.opening.as_deref() == Some(r.id.as_str());
        let when = if opening { tr("Opening…").to_owned() } else { r.when.clone() };
        d.text(&format!("{rid}_when"), &when, &Txt::new(12.0, Face::Regular, tok::MUTED));
        d.close();
        d.close();
        if r.current {
            let badge = format!("{rid}_check");
            d.surface(&badge, "width: 24 height: 24 flow: Overlay align: Align{x: 0.5 y: 0.5}", tok::BLACK, 12.0, None);
            d.icon(&format!("{badge}_icon"), "b3_check_white.svg", 14.0, tok::WHITE);
            d.close();
        }
        d.close();
        d.tap(&format!("{rid}_tap"), &format!("b3.switch.open#{i}"));
        // A8 — delete: a trailing × above the row's open target (never on the
        // open Session; only when `session/delete` is advertised), then an
        // explicit confirmation in place.
        if !r.current && st.catalog.is_none() && delete_offered(store) {
            let layer = d.anon();
            if confirming && narrow {
                d.view(&layer, "width: Fill height: Fill flow: Right align: Align{x: 1.0 y: 1.0} padding: Inset{right: 8 bottom: 8}");
            } else {
                d.view(&layer, "width: Fill height: Fill flow: Right align: Align{x: 1.0 y: 0.5} padding: Inset{right: 8}");
            }
            if st.deleting.as_deref() == Some(r.id.as_str()) {
                d.text(&format!("{rid}_deleting"), tr("Deleting…"), &Txt::new(12.0, Face::Regular, tok::MUTED));
            } else if st.confirm_delete.as_deref() == Some(r.id.as_str()) {
                d.surface(
                    &format!("{rid}_confirm"),
                    "width: Fit height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 4 padding: Inset{left: 10 right: 4 top: 2 bottom: 2}",
                    tok::SURFACE,
                    10.0,
                    Some(tok::HAIRLINE),
                );
                d.text(&format!("{rid}_confirm_q"), tr("Delete?"), &Txt::new(12.0, Face::Medium, tok::TEXT));
                d.link(&format!("{rid}_confirm_no"), tr("Cancel"), Some("b3.switch.delete.cancel"), 12.5);
                d.view(&format!("{rid}_confirm_yes_box"), "width: Fit height: 28 flow: Overlay align: Align{x: 0.5 y: 0.5} padding: Inset{left: 6 right: 6}");
                d.text(&format!("{rid}_confirm_yes_label"), tr("Delete"), &Txt::new(12.5, Face::Medium, tok::RED_TEXT));
                d.tap(&format!("{rid}_confirm_yes"), "b3.switch.delete.confirm");
                d.close();
                d.close();
            } else {
                ui::icon_button(d, &format!("{rid}_delete"), "b3_x_small.svg", 12.0, &format!("b3.switch.delete#{i}"));
            }
            d.close();
        }
        d.close();
    }
    d.close();
}

/// The switcher. With Vim editing on, the board's split: the session list on
/// the left, the composer's key legend on the right (stacked under the list
/// on a phone-narrow frame).
pub fn build(d: &mut Dsl, st: &SwitchState, frame: &Frame, store: &Store, vim: &super::vim::VimState) {
    let split = vim.enabled && !frame.compact(frame.dialog_w(760.0));
    let width = if split { frame.dialog_w(760.0) } else { frame.dialog_w(480.0) };
    let pad = ui::dialog_pad(frame, width);
    ui::shell_open(d, frame, width);
    if split {
        ui::header(d, tr("Open a different session"), "b3.close");
        d.gap(W::Fill, 12.0);
        let cols = d.anon();
        d.view(&cols, "width: Fill height: Fit flow: Right spacing: 18");
        let left = d.anon();
        d.view(&left, "width: Fill height: Fit flow: Down");
        ui::body_open(d, frame, width, 50.0);
        panel(d, st, store, width - 2.0 * pad - 18.0 - LEGEND_W);
        ui::body_close(d);
        d.close();
        super::vim::legend(d, vim, W::Px(LEGEND_W));
        d.close();
    } else {
        ui::header(d, tr("Open a different session"), "b3.close");
        d.gap(W::Fill, 12.0);
        ui::body_open(d, frame, width, 50.0);
        panel(d, st, store, width - 2.0 * pad);
        if vim.enabled {
            d.gap(W::Fill, 14.0);
            super::vim::legend(d, vim, W::Fill);
        }
        ui::body_close(d);
    }
    ui::shell_close(d);
}

/// The legend column's width in the split (the board's right third).
const LEGEND_W: f64 = 260.0;

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_store::Session;

    /// A13 — a failed open leads with "Couldn't open that session." and keeps
    /// the cause muted under it; the delete refusal (already a sentence for
    /// people, A8's walk reads it whole) shows alone.
    #[test]
    fn a_failed_open_leads_with_plain_words_and_a_delete_refusal_stays_whole() {
        let store = Store::new();
        let cause = "session/open: transport: channel closed";
        let st = SwitchState { error: Some(cause.into()), failed: Some((OPEN_FAILED, cause.into())), ..Default::default() };
        let mut d = Dsl::new();
        panel(&mut d, &st, &store, 600.0);
        let dsl = d.finish();
        let lead = dsl.find("b3_switch_error := Label").expect("the lead");
        let detail = dsl.find("b3_switch_error_detail := Label").expect("the cause");
        assert!(lead < detail && dsl[lead..detail].contains(OPEN_FAILED) && dsl[detail..].contains(cause));
        let st = SwitchState { error: Some("Couldn't delete the session: session is busy".into()), ..Default::default() };
        let mut d = Dsl::new();
        panel(&mut d, &st, &store, 600.0);
        let dsl = d.finish();
        assert!(dsl.contains("Couldn't delete the session: session is busy") && !dsl.contains("b3_switch_error_detail"));
    }

    fn sess(id: &str, title: Option<&str>, at: &str) -> Session {
        Session {
            id: id.into(),
            title: title.map(Into::into),
            message_count: 1,
            updated_at: Some(at.into()),
            last_prompt: Some("why is hydrate slow?".into()),
            active_turn: false,
        }
    }

    #[test]
    fn rows_are_newest_first_with_the_web_title_fallbacks_and_current_marked() {
        let store = Store::new();
        store.set_sessions(vec![
            sess("dsflash:a", Some("Add session fork"), "2026-09-30T10:00:00Z"),
            sess("dsflash:b", None, "2026-10-01T10:00:00Z"),
            Session { last_prompt: None, title: None, ..sess("dsflash:0123456789", None, "2026-09-01T00:00:00Z") },
        ]);
        store.set_active(Some("dsflash:a".into()));
        let r = rows(&store);
        assert_eq!(r[0].title, "why is hydrate slow?", "last_prompt fallback");
        assert_eq!(r[1].title, "Add session fork");
        assert!(r[1].current);
        assert_eq!(r[2].title, "Session 0123456789", "a leaf of 10 stays whole");
        assert_eq!(known_session_title("dsflash:main"), "Session main");
        assert_eq!(known_session_title("dsflash:abcdefghijkl"), "Session efghijkl", "the last 8 of a long leaf");
        assert_eq!(known_session_title(""), "Session unknown");
    }

    #[test]
    fn delete_never_touches_the_open_session_and_asks_first() {
        let store = Store::new();
        store.set_sessions(vec![sess("dsflash:a", Some("A"), "2026-09-30T10:00:00Z"), sess("dsflash:b", Some("B"), "2026-10-01T10:00:00Z")]);
        store.set_active(Some("dsflash:a".into()));
        let mut st = SwitchState::default();
        assert_eq!(perform(&mut st, "b3.switch.delete", 0, &store), Outcome::Done);
        assert_eq!(st.confirm_delete, None, "unadvertised: no delete");
        store.domains.config.set_supported_methods(vec!["session/delete".into()]);
        perform(&mut st, "b3.switch.delete", 1, &store);
        assert_eq!(st.confirm_delete, None, "the open Session (row 1, older) is never offered");
        perform(&mut st, "b3.switch.delete", 0, &store);
        assert_eq!(st.confirm_delete.as_deref(), Some("dsflash:b"), "asks first");
        assert_eq!(perform(&mut st, "b3.switch.delete.confirm", 0, &store), Outcome::Spawn(super::super::host::Job::SwitchDelete("dsflash:b".into())));
        perform(&mut st, "b3.switch.delete", 0, &store);
        assert_eq!(st.confirm_delete, None, "single-flight while one delete runs");
    }

    #[test]
    fn a_row_keeps_its_text_clear_of_the_delete_control() {
        let store = Store::new();
        store.set_sessions(vec![sess("dsflash:a", Some("A"), "2026-09-30T10:00:00Z"), sess("dsflash:b", Some("Review PR #2566"), "2026-10-01T10:00:00Z")]);
        store.set_active(Some("dsflash:a".into()));
        store.domains.config.set_supported_methods(vec!["session/delete".into()]);
        let mut st = SwitchState::default();
        // The right inset of the row content right before `id` in the DSL.
        let inset = |dsl: &str, id: &str| -> f64 {
            let at = dsl.find(id).unwrap();
            let head = &dsl[..at];
            let p = head.rfind("padding: Inset{left: 12 right: ").unwrap() + "padding: Inset{left: 12 right: ".len();
            head[p..].split(' ').next().unwrap().parse().unwrap()
        };
        let lower = |st: &SwitchState| {
            let mut d = Dsl::new();
            build(&mut d, st, &Frame { avail_w: 360.0, avail_h: 780.0 }, &store, &Default::default());
            d.finish()
        };
        let plain = lower(&st);
        assert!(inset(&plain, "b3_switch_row_0_title") >= 42.0, "the × is reserved");
        assert_eq!(inset(&plain, "b3_switch_row_1_title"), 12.0, "the open Session has no delete");
        perform(&mut st, "b3.switch.delete", 0, &store);
        let asking = lower(&st);
        assert!(inset(&asking, "b3_switch_row_0_title") < 60.0, "phone: the title keeps its width");
        assert!(asking.contains("align: Align{x: 1.0 y: 1.0} padding: Inset{right: 8 bottom: 8}"), "phone: the pill sits on the meta line");
        // A desktop-wide list keeps the pill beside the title, reserved.
        let mut d = Dsl::new();
        build(&mut d, &st, &Frame::DESKTOP, &store, &Default::default());
        let wide = d.finish();
        let pill = 10.0 + ui::text_w("Delete?", 12.0, Face::Medium) + ui::text_w("Cancel", 12.5, Face::Regular) + ui::text_w("Delete", 12.5, Face::Medium);
        assert!(inset(&wide, "b3_switch_row_0_title") >= pill, "desktop: the confirm pill is reserved");
    }

    #[test]
    fn the_current_row_closes_and_another_opens_fresh() {
        let store = Store::new();
        store.set_sessions(vec![sess("dsflash:a", Some("A"), "2026-09-30T10:00:00Z"), sess("dsflash:b", Some("B"), "2026-10-01T10:00:00Z")]);
        store.set_active(Some("dsflash:a".into()));
        let mut st = SwitchState::default();
        assert_eq!(perform(&mut st, "b3.switch.open", 1, &store), Outcome::Close);
        assert_eq!(
            perform(&mut st, "b3.switch.open", 0, &store),
            Outcome::Spawn(super::super::host::Job::SwitchOpen("dsflash:b".into()))
        );
    }
}

#[cfg(test)]
mod history_scope_tests {
    use super::*;
    #[test]
    fn same_wire_id_in_two_projects_is_two_history_rows() {
        let value = serde_json::json!({"sessions":[
            {"id":"dev:main","profile_id":"dev","workspace_root":"/project/a","title":"First"},
            {"id":"dev:main","profile_id":"dev","workspace_root":"/project/b","title":"Second"},
            {"id":"dev:main","profile_id":"dev","workspace_root":null,"title":"Profile"}]});
        let rows = parse_history(&value).unwrap();
        assert_eq!(rows.len(),3);
        let keys: std::collections::HashSet<_> = rows.iter().map(|r| r.target.key()).collect();
        assert_eq!(keys.len(),3);
        assert_eq!(rows[2].target.cwd,None);
        assert!(parse_history(&serde_json::json!({"sessions":[{"id":"other:main","profile_id":"dev","workspace_root":null}]})).is_err());
    }
}
