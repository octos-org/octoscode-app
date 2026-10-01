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

#[derive(Debug, Clone, Default)]
pub struct SwitchState {
    pub loading: bool,
    pub error: Option<String>,
    pub opening: Option<String>,
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
    let tail: String = s.id.chars().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
    format!("Session {tail}")
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
    {
        super::host::state().switch.loading = true;
    }
    let r = conv.refresh_sessions().await;
    let mut st = super::host::state();
    st.switch.loading = false;
    match r {
        Ok(n) => Ok(format!("{n} sessions")),
        Err(e) => {
            st.switch.error = Some("Could not load sessions.".into());
            Err(e.to_string())
        }
    }
}

/// Open `id` fresh (no replay cursor: `open_workspace_as` sends `after:
/// None`), in its own workspace when the store knows it.
pub async fn open(conv: &crate::flow::Conversation, id: String) -> Result<String, String> {
    let cwd = conv.store.domains.session.workspace_root(&id);
    let r = conv.open_session(&id, cwd).await;
    let mut st = super::host::state();
    st.switch.opening = None;
    match r {
        Ok(opened) => {
            st.open = None;
            Ok(format!("opened {opened}"))
        }
        Err(e) => {
            st.switch.error = Some(format!("Could not open that session: {e}"));
            Err(e)
        }
    }
}

pub fn perform(st: &mut SwitchState, action: &str, index: usize, store: &Store) -> Outcome {
    match action {
        "b3.switch.open" => {
            let rows = rows(store);
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
    let rows = rows(store);
    if st.loading && rows.is_empty() {
        d.text("b3_switch_loading", "Loading sessions…", &ui::meta());
    }
    if let Some(e) = &st.error {
        d.text("b3_switch_error", e, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
    }
    if rows.is_empty() && !st.loading {
        d.text("b3_switch_empty", "No sessions yet.", &ui::meta());
    }
    let list = d.anon();
    d.view(&list, "width: Fill height: Fit flow: Down spacing: 6");
    for (i, r) in rows.iter().enumerate() {
        let rid = format!("b3_switch_row_{i}");
        let (fill, border) = if r.current { (tok::SURFACE2, tok::HAIRLINE) } else { (tok::SURFACE, tok::HAIRLINE) };
        d.surface(&rid, "width: Fill height: Fit flow: Overlay", fill, 10.0, Some(border));
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{left: 12 right: 12 top: 10 bottom: 10}");
        let col = d.anon();
        d.view(&col, "width: Fill height: Fit flow: Down spacing: 6");
        let color = if r.current { tok::MUTED } else { tok::TEXT };
        d.text(&format!("{rid}_title"), &super::inventory::fit(&r.title, inner_w - 120.0, 13.0, false), &Txt::new(13.0, Face::Medium, color).w(W::Fill));
        let meta = d.anon();
        d.view(&meta, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        d.chip(&format!("{rid}_tag"), &r.tag, tok::MUTED, tok::SURFACE2, Some(tok::HAIRLINE), true);
        let opening = st.opening.as_deref() == Some(r.id.as_str());
        let when = if opening { "Opening…".to_owned() } else { r.when.clone() };
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
        d.close();
    }
    d.close();
}

pub fn build(d: &mut Dsl, st: &SwitchState, frame: &Frame, store: &Store) {
    let width = frame.dialog_w(480.0);
    let pad = ui::dialog_pad(frame, width);
    ui::shell_open(d, frame, width);
    ui::header(d, "Open a different session", "b3.close");
    d.gap(W::Fill, 12.0);
    ui::body_open(d, frame, width, 50.0);
    panel(d, st, store, width - 2.0 * pad);
    ui::body_close(d);
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_store::Session;

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
        assert_eq!(r[2].title, "Session 23456789", "the last 8 of the id");
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
