//! Board-3 screen 7 — RESUME CANDIDATES (rows: session × 3: list unverified
//! candidates + exact confirmation; canonical history without a turn; parked
//! interaction restore stays C — see the report).
//!
//! Web: `features/resume/ResumeDialog.tsx` + `resume-binding.ts`, opened by
//! `/resume` (`registry.ts:450-466`; an argument only seeds the search,
//! `composer/intent.ts:106-107`). The catalog is `session/list {cwd,
//! profile_id}` (`resume-binding.ts:199-202`): every row is an UNVERIFIED
//! candidate, sorted by `updated_at` descending, filtered over
//! `id + title + last_prompt`; a change of query clears the selection.
//! Opening never submits a prompt: `session/open {session_id, profile_id,
//! cwd}` then the canonical hydrate (`use-octos-session.ts:3652-3690`).
//!
//! The confirmation is the approved board's: the user types the candidate's
//! EXACT title before the primary control becomes live (the web's
//! "Verify history and resume" stays disabled until its confirm checkbox is
//! ticked — `ResumeDialog.tsx` — and the board makes that confirmation an
//! exact-match field). Blocked rows say why and never arm
//! (`resume-binding.ts:152-171`).
use octoscode_store::Store;

use super::host::Outcome;
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: String,
    pub title: String,
    pub message_count: usize,
    pub updated_ms: Option<u64>,
    pub last_prompt: Option<String>,
    pub active_turn: bool,
    /// Why this row cannot be opened (`resume-binding.ts:152-171`), if so.
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ResumeState {
    pub query: String,
    pub query_snap: String,
    pub loading: bool,
    pub opening: bool,
    pub error: Option<String>,
    pub candidates: Vec<Candidate>,
    pub selected: Option<usize>,
    /// The typed confirmation (live) — the primary control arms only when
    /// it equals the selected candidate's title exactly.
    pub confirm: String,
    pub confirm_snap: String,
    pub workspace: String,
    pub profile: String,
    pub ticket: u64,
}

impl ResumeState {
    pub fn visible(&self) -> Vec<usize> {
        let q = self.query.trim().to_lowercase();
        self.candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                q.is_empty()
                    || format!("{} {} {}", c.id, c.title, c.last_prompt.as_deref().unwrap_or(""))
                        .to_lowercase()
                        .contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// The exact-match gate: a selected, unblocked row AND the typed text
    /// equal to its title (no trimming, no case folding).
    pub fn armed(&self) -> bool {
        match self.selected.and_then(|i| self.candidates.get(i)) {
            Some(c) => c.blocked.is_none() && !self.opening && self.confirm == c.title,
            None => false,
        }
    }
}

/// `resume-binding.ts:53-100`, natively: the id must be scoped to the
/// captured Profile (`<profile>:<chat>`); a bare or foreign id is refused
/// rather than guessed.
pub fn blocked_reason(id: &str, profile: &str, known_closed: bool) -> Option<String> {
    if known_closed {
        return Some("This retained Session is closed.".into());
    }
    if profile.is_empty() || !id.starts_with(&format!("{profile}:")) || id.len() <= profile.len() + 1 {
        return Some("This catalog ID does not identify a full Session in the captured Profile. An authoritative full ID is required; no Profile or channel will be guessed.".into());
    }
    None
}

/// Sort newest first by `updated_at` (string order, as the web).
pub fn from_rows(rows: Vec<octoscode_client::domains::session::SessionListRow>, profile: &str, current: &str) -> Vec<Candidate> {
    let mut rows = rows;
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    rows.into_iter()
        .filter(|r| r.id != current)
        .map(|r| {
            let title = r
                .title
                .clone()
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| r.id.clone());
            Candidate {
                blocked: blocked_reason(&r.id, profile, false),
                updated_ms: r.updated_at.as_deref().and_then(ui::parse_iso_ms),
                id: r.id,
                title,
                message_count: r.message_count,
                last_prompt: r.last_prompt,
                active_turn: r.active_turn,
            }
        })
        .collect()
}

pub async fn load(conv: &crate::flow::Conversation) -> Result<String, String> {
    use octoscode_client::domains::session::{SessionList, SessionListParams};
    let session = conv.session_id();
    let profile = conv.profile();
    let workspace = conv.store.domains.session.workspace_root(&session).unwrap_or_default();
    let ticket = {
        let mut st = super::host::state();
        st.resume.ticket += 1;
        st.resume.loading = true;
        st.resume.error = None;
        st.resume.profile = profile.clone();
        st.resume.workspace = workspace.clone();
        st.resume.ticket
    };
    if session.is_empty() || profile.is_empty() {
        let mut st = super::host::state();
        st.resume.loading = false;
        st.resume.error = Some("A confirmed source Session is required to browse history.".into());
        return Err("no source session".into());
    }
    let params = SessionListParams {
        cwd: (!workspace.is_empty()).then(|| workspace.clone()),
        profile_id: Some(profile.clone()),
    };
    let result = conv.client().call::<SessionList>(params).await;
    let mut st = super::host::state();
    if st.resume.ticket != ticket {
        return Ok("A newer history listing replaced this request.".into());
    }
    st.resume.loading = false;
    match result {
        Ok(r) => {
            if r.sessions.len() > 10_000 {
                st.resume.error = Some("History catalog is too large to inspect safely.".into());
                return Err("too large".into());
            }
            st.resume.candidates = from_rows(r.sessions, &profile, &session);
            st.resume.selected = None;
            st.resume.confirm.clear();
            st.resume.confirm_snap.clear();
            Ok(format!("{} candidates", st.resume.candidates.len()))
        }
        Err(e) => {
            st.resume.error = Some("History listing failed.".into());
            Err(e.to_string())
        }
    }
}

/// Open the confirmed candidate — `session/open` + the canonical hydrate the
/// flow runs on every open; NO turn is started.
pub async fn open(conv: &crate::flow::Conversation, id: String) -> Result<String, String> {
    let cwd = {
        let st = super::host::state();
        (!st.resume.workspace.is_empty()).then(|| st.resume.workspace.clone())
    };
    let r = conv.open_session(&id, cwd).await;
    let mut st = super::host::state();
    st.resume.opening = false;
    match r {
        Ok(opened) => {
            st.open = None; // the web closes the picker on success
            Ok(format!("resumed {opened} (no turn started)"))
        }
        Err(e) => {
            st.resume.error = Some("Historical identity was not resolved. The listed conversation was not resumed.".into());
            Err(e)
        }
    }
}

pub fn perform(st: &mut ResumeState, action: &str, index: usize) -> Outcome {
    match action {
        "b3.resume.select" => {
            let Some(c) = st.candidates.get(index) else { return Outcome::Done };
            if c.blocked.is_some() {
                return Outcome::Done; // a blocked row never arms
            }
            if st.selected != Some(index) {
                st.selected = Some(index);
                st.confirm.clear();
                st.confirm_snap.clear();
            }
            st.query_snap = st.query.clone();
            Outcome::Done
        }
        "b3.resume.refresh" => {
            st.query_snap = st.query.clone();
            st.selected = None;
            Outcome::Spawn(super::host::Job::ResumeLoad)
        }
        "b3.resume.confirm" => {
            if !st.armed() {
                st.error = Some("Confirm the exact Session, workspace and Profile before opening.".into());
                return Outcome::Done;
            }
            let id = st.candidates[st.selected.unwrap()].id.clone();
            st.opening = true;
            st.error = None;
            Outcome::Spawn(super::host::Job::ResumeOpen(id))
        }
        _ => Outcome::Unrouted,
    }
}

pub fn input_changed(st: &mut ResumeState, key: &str, text: &str) {
    match key {
        "resume.search" => {
            st.query = text.to_owned();
            // `ResumeDialog`: a query change clears the selection + confirm.
            st.selected = None;
            st.confirm.clear();
        }
        "resume.confirm" => st.confirm = text.to_owned(),
        _ => {}
    }
}

// -------------------------------------------------------------------- view


pub fn build(d: &mut Dsl, st: &ResumeState, frame: &Frame, _store: &Store) {
    let width = frame.dialog_w(720.0);
    let pad = ui::dialog_pad(frame, width);
    let inner_w = width - 2.0 * pad - 10.0;
    ui::shell_open(d, frame, width);
    ui::header(d, "Resume chat", "b3.close");
    let scope = if st.workspace.is_empty() {
        format!("Target Profile: {}", st.profile)
    } else {
        format!("Target Profile: {} · {}", st.profile, ui::leaf(&st.workspace))
    };
    d.text("b3_resume_scope", &scope, &ui::micro().w(W::Fill));
    d.gap(W::Fill, 12.0);
    ui::body_open(d, frame, width, 150.0);
    // The caution (`ResumeDialog.tsx` intro; the board's amber banner). A
    // live authority change replaces it with the web's error copy.
    match &st.error {
        Some(e) => ui::banner(d, "b3_resume_caution", e, "Refresh the catalog before selecting this row."),
        None => ui::banner(
            d,
            "b3_resume_caution",
            "Catalog rows are unverified candidates, not confirmed workspace sessions.",
            "Refresh the catalog before selecting a row. Bare IDs cannot safely identify a historical conversation.",
        ),
    }
    d.gap(W::Fill, 12.0);
    // Search + refresh (`ResumeDialog`: "Search history", "Refresh
    // historical candidates").
    let srow = d.anon();
    d.view(&srow, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    d.input("b3_resume_search", "resume.search", &st.query_snap, "Search history", false, 36.0);
    ui::icon_button(d, "b3_resume_refresh", "b3_refresh.svg", 16.0, "b3.resume.refresh");
    d.close();
    d.gap(W::Fill, 10.0);
    if st.loading {
        d.text("b3_resume_loading", "Reading unverified history candidates…", &ui::meta());
        d.gap(W::Fill, 6.0);
    }
    // The candidates: ONE bordered list, hairlines between rows.
    let open: Vec<(usize, &Candidate)> = st.candidates.iter().enumerate().filter(|(_, c)| c.blocked.is_none()).collect();
    if !open.is_empty() {
        d.surface("b3_resume_list", "width: Fill height: Fit flow: Down", tok::SURFACE, 12.0, Some(tok::HAIRLINE));
        for (n, (i, c)) in open.iter().enumerate() {
            candidate_row(d, *i, c, st.selected == Some(*i), n > 0, inner_w);
        }
        d.close();
    }
    let empty = d.anon();
    d.view(&empty, "width: Fill height: Fit flow: Right align: Align{x: 0.5 y: 0.5} padding: Inset{top: 8 bottom: 4}");
    d.text("b3_resume_empty", "No matching historical candidates.", &ui::meta());
    d.close();
    d.gap(W::Fill, 12.0);
    confirm_area(d, st);
    // Blocked candidates: the board's lock rows, each with the web's reason.
    for (i, c) in st.candidates.iter().enumerate().filter(|(_, c)| c.blocked.is_some()) {
        d.gap(W::Fill, 10.0);
        let rid = format!("b3_resume_row_{i}");
        d.surface(&rid, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 12 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}", tok::SURFACE2, 12.0, Some(tok::HAIRLINE));
        d.icon("", "b3_lock.svg", 16.0, tok::MUTED);
        let col = d.anon();
        d.view(&col, "width: Fill height: Fit flow: Down spacing: 3");
        d.text(&format!("{rid}_title"), &super::inventory::fit(&c.title, inner_w - 80.0, 12.5, false), &Txt::new(12.5, Face::Medium, tok::MUTED).w(W::Fill));
        d.text(&format!("{rid}_blocked"), c.blocked.as_deref().unwrap_or(""), &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        d.close();
        d.close();
    }
    ui::body_close(d);
    ui::shell_close(d);
}

fn candidate_row(d: &mut Dsl, i: usize, c: &Candidate, selected: bool, divider: bool, inner_w: f64) {
    let rid = format!("b3_resume_row_{i}");
    d.view(&rid, "width: Fill height: Fit flow: Down");
    if divider {
        d.hairline();
    }
    let fill = if selected { tok::SURFACE2 } else { tok::TRANSPARENT };
    d.surface(&format!("{rid}_body"), "width: Fill height: Fit flow: Overlay", fill, 0.0, None);
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{left: 14 right: 12 top: 11 bottom: 11}");
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 6");
    d.text(
        &format!("{rid}_title"),
        &super::inventory::fit(&c.title, inner_w - 150.0, 14.0, false),
        &Txt::new(14.0, Face::Regular, tok::TEXT).w(W::Fill),
    );
    let l2 = d.anon();
    d.view(&l2, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
    let tag = c.id.split(':').next().unwrap_or(&c.id).to_owned();
    d.chip("", &tag, tok::TEXT, tok::CHIP, None, true);
    let mut meta = format!("{} listed message{}", c.message_count, if c.message_count == 1 { "" } else { "s" });
    if let Some(ms) = c.updated_ms {
        meta = format!("{} · {meta}", ui::rel_time(ui::now_ms(), ms));
    }
    d.text("", &meta, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill));
    d.close();
    if c.active_turn {
        d.text("", "Busy — another client is working in this session", &Txt::new(11.5, Face::Regular, tok::BLUE).w(W::Fill));
    }
    d.close();
    d.chip(&format!("{rid}_unverified"), "unverified", tok::AMBER, tok::AMBER_BG, Some(tok::AMBER_LINE), false);
    d.close();
    d.tap(&format!("{rid}_tap"), &format!("b3.resume.select#{i}"));
    d.close();
    d.close();
}

fn confirm_area(d: &mut Dsl, st: &ResumeState) {
    let sel = st.selected.and_then(|i| st.candidates.get(i));
    ui::card_open(d, "b3_resume_confirm_card", 8.0);
    ui::field_label(d, "b3_resume_confirm_label", "Confirm exact title to resume:");
    let placeholder = match sel {
        Some(_) => "Type the exact thread title above",
        None => "Select a candidate first",
    };
    d.input("b3_resume_confirm", "resume.confirm", &st.confirm_snap, placeholder, false, 38.0);
    // Both variants are emitted; the live gate shows one (no remount while
    // typing — `live_visibility`).
    let label = if st.opening { "Opening…" } else { "Resume chat" };
    d.view("b3_resume_go_off", "width: Fill height: Fit flow: Down");
    d.button("b3_resume_go_disabled", label, "b3.resume.confirm", Btn::Disabled, W::Fill, 38.0);
    d.close();
    d.view("b3_resume_go_on", "width: Fill height: Fit flow: Down");
    d.button("b3_resume_go", label, "b3.resume.confirm", Btn::Primary, W::Fill, 38.0);
    d.close();
    d.text(
        "b3_resume_note",
        "A confirmed source Session is required to browse history. Opening does not submit a prompt.",
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    d.close();
}

pub fn visibility(st: &ResumeState) -> Vec<(String, bool)> {
    let vis = st.visible();
    let mut out: Vec<(String, bool)> = (0..st.candidates.len())
        .map(|i| (format!("b3_resume_row_{i}"), vis.contains(&i)))
        .collect();
    out.push(("b3_resume_empty".into(), vis.is_empty() && !st.loading));
    let armed = st.armed();
    out.push(("b3_resume_go_on".into(), armed));
    out.push(("b3_resume_go_off".into(), !armed));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_client::domains::session::SessionListRow;

    fn row(id: &str, title: &str, at: &str) -> SessionListRow {
        serde_json::from_value(serde_json::json!({
            "id": id, "title": title, "message_count": 3, "updated_at": at
        }))
        .unwrap()
    }

    fn state() -> ResumeState {
        let rows = vec![
            row("dsflash:a", "Add session fork", "2026-09-30T10:00:00Z"),
            row("dsflash:b", "Fix steer queue drop on reconnect", "2026-10-01T10:00:00Z"),
            row("other:c", "Foreign row", "2026-10-01T11:00:00Z"),
            row("dsflash:main", "Current", "2026-10-01T12:00:00Z"),
        ];
        ResumeState { candidates: from_rows(rows, "dsflash", "dsflash:main"), profile: "dsflash".into(), ..Default::default() }
    }

    #[test]
    fn candidates_are_newest_first_unverified_and_exclude_the_current_session() {
        let st = state();
        let ids: Vec<&str> = st.candidates.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["other:c", "dsflash:b", "dsflash:a"]);
        assert!(st.candidates[0].blocked.is_some(), "a foreign-profile id is never guessed");
        assert!(st.candidates[1].blocked.is_none());
    }

    #[test]
    fn confirmation_requires_the_exact_title() {
        let mut st = state();
        assert!(!st.armed());
        assert_eq!(perform(&mut st, "b3.resume.select", 0), Outcome::Done);
        assert_eq!(st.selected, None, "a blocked row never arms");
        perform(&mut st, "b3.resume.select", 1);
        input_changed(&mut st, "resume.confirm", "fix steer queue drop on reconnect");
        assert!(!st.armed(), "case differs");
        input_changed(&mut st, "resume.confirm", "Fix steer queue drop on reconnect ");
        assert!(!st.armed(), "trailing space differs");
        assert_eq!(perform(&mut st, "b3.resume.confirm", 0), Outcome::Done, "refused");
        input_changed(&mut st, "resume.confirm", "Fix steer queue drop on reconnect");
        assert!(st.armed());
        assert_eq!(
            perform(&mut st, "b3.resume.confirm", 0),
            Outcome::Spawn(super::super::host::Job::ResumeOpen("dsflash:b".into()))
        );
    }

    #[test]
    fn a_query_change_clears_the_selection_and_filters() {
        let mut st = state();
        perform(&mut st, "b3.resume.select", 1);
        input_changed(&mut st, "resume.search", "fork");
        assert_eq!(st.selected, None);
        assert_eq!(st.visible(), vec![2]);
        let vis = visibility(&st);
        assert!(vis.contains(&("b3_resume_row_2".to_owned(), true)));
        assert!(vis.contains(&("b3_resume_row_1".to_owned(), false)));
        assert!(vis.contains(&("b3_resume_go_on".to_owned(), false)));
    }

    #[test]
    fn the_dialog_lowers_balanced_with_row_taps_on_the_shared_path() {
        let st = state();
        let mut d = Dsl::new();
        build(&mut d, &st, &Frame::DESKTOP, &Store::new());
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        let taps = crate::screens::taps::wired_taps(&dsl);
        assert!(taps.iter().any(|(_, e)| e == "b3.resume.select#1"));
        assert!(!taps.iter().any(|(_, e)| e == "b3.resume.select#0"), "blocked row has no tap");
        assert_eq!(crate::screens::taps::split_row("b3.resume.select#1"), ("b3.resume.select", Some(1)));
    }
}
