//! A29 — parity row 6: the `/btw` aside panel above the composer of the
//! Session that asked (board 4, `design/stage-a/phase4-new4`: region 4
//! "answering", region 5 "answered / collapsed / stale", zh specimen Z5).
//!
//! The web's `BtwAsidePanel` (`features/btw/BtwAsidePanel.tsx`, its CSS
//! module) inside `composer-wrap` (`App.tsx:2707`): the header "Aside — /btw"
//! with Close, the question, "Answering…" then the whole Markdown answer
//! (one reply, no streaming), the failed / stale copy, and "This aside is not
//! saved to the conversation." The panel is bounded to min(50 % of the
//! window, 480 px) and scrolls inside (`.panel { max-height: min(50dvh,
//! 480px); overflow: auto }`); it spans the composer's width (the chat
//! column). The operator's board additions (2026-10-02): a collapse chevron
//! with a one-row folded state, and a monospace scope line naming the asking
//! Session ("octos · <Session title>").
//!
//! The panel shows the ACTIVE Session's aside from the store's `btw` domain
//! — a Session without one shows none, and switching back brings it back.
//! Its two controls are host-routed hits (`lib.rs`): `btw_aside_close_hit`
//! → `aside.dismiss`, `btw_aside_toggle_hit` → `aside.toggle`
//! (`screens::sessions`, the one owner of the `aside.*` ids).
use octoscode_store::domains::btw::{Aside, AsideState, Failure};
use octoscode_store::Store;

use crate::conv_layout::Metrics;
use crate::fluid::{hit, label, markdown_region, scale, style, svg, Face, BORDER, FIT_WRAP_LINES, INK, MUTED, SURFACE};
use crate::i18n::tr;

/// Red TEXT (the stale / failed lead): the kits' error ink, the web's
/// `--dsw-alias-state-error-text` (`board3::ui::tok::RED_TEXT`; dark twin
/// #FF6B6B through `theme::retint_dsl`).
const RED_TEXT: &str = "#c50f0fff";

/// The web's copy (`lazy-btw-controller.ts:52-54`), English source keys.
pub const FAILED_COPY: &str = "The aside could not be answered. Try again.";
pub const STALE_COPY: &str =
    "The Session connection changed before the aside completed. Ask again when it is ready.";
/// The same copy as the board draws it: a red lead over a muted cause (each
/// half its own source key: a translation has no ". " to split at).
pub const FAILED_LEAD: &str = "The aside could not be answered.";
pub const FAILED_CAUSE: &str = "Try again.";
pub const STALE_LEAD: &str = "The Session connection changed before the aside completed.";
pub const STALE_CAUSE: &str = "Ask again when it is ready.";

/// The panel's header row height (the chevron and Close hits are >= 32 px).
const HEAD_H: f64 = 36.0;
/// The card's own vertical padding (top + bottom) and the head-to-body gap.
const PAD_TOP: f64 = 8.0;
const PAD_BOTTOM: f64 = 14.0;
const HEAD_GAP: f64 = 2.0;

/// The failed / stale copy, in the current language (the web renders the
/// controller's English string; the Chinese is native, `i18n::native`).
pub fn failure_copy(f: Failure) -> &'static str {
    match f {
        Failure::Failed => tr(FAILED_COPY),
        Failure::Stale => tr(STALE_COPY),
    }
}

/// The copy as the board draws it: a red lead and a muted cause, like the
/// kit's notices (`ui::failure`).
pub fn failure_split(f: Failure) -> (String, String) {
    let (lead, cause) = match f {
        Failure::Failed => (FAILED_LEAD, FAILED_CAUSE),
        Failure::Stale => (STALE_LEAD, STALE_CAUSE),
    };
    (tr(lead).to_owned(), tr(cause).to_owned())
}

/// The panel's height bound: min(50 % of the window, 480 px)
/// (`BtwAsidePanel.module.css:2`).
pub fn cap_height(window_h: f64) -> f64 {
    let h = if window_h > 0.0 { window_h } else { 600.0 };
    (0.5 * h).min(480.0).floor()
}

/// The scrolling body's bound: the cap minus the fixed header and padding.
pub fn body_cap(window_h: f64) -> f64 {
    (cap_height(window_h) - HEAD_H - PAD_TOP - PAD_BOTTOM - HEAD_GAP).max(96.0).floor()
}

/// What the panel draws for one Session's aside.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelView {
    /// The asking Session ("<workspace> · <title>", monospace).
    pub scope: String,
    pub aside: Aside,
}

/// The scope line naming the asking Session: its workspace folder (the
/// sidebar's group label, `workspaceName`) and its title (the sidebar row's,
/// `label_stem`, else "New chat").
pub fn scope_line(store: &Store, session: &str) -> String {
    let title = store
        .sessions()
        .into_iter()
        .find(|s| s.id == session)
        .and_then(|s| s.label_stem())
        .unwrap_or_else(|| tr("New chat").to_owned());
    match store.domains.session.workspace_root(session).filter(|r| !r.trim().is_empty()) {
        Some(root) => format!("{} · {title}", crate::screens::recents::workspace_name(&root)),
        None => title,
    }
}

/// The ACTIVE Session's aside, if it holds one.
pub fn view(store: &Store) -> Option<PanelView> {
    let session = store.active_session()?;
    let aside = store.domains.btw.get(&session)?;
    Some(PanelView { scope: scope_line(store, &session), aside })
}

/// The panel's lowered DSL for the active Session (empty: no panel).
pub fn lower(store: &Store, m: &Metrics, window_h: f64) -> String {
    view(store).map(|v| panel(&v, m, window_h)).unwrap_or_default()
}

/// The collapsed row's state word.
fn state_word(state: &AsideState) -> (&'static str, &'static str) {
    match state {
        AsideState::Answering => (tr("Answering…"), MUTED),
        AsideState::Answered { .. } => (tr("Answered"), MUTED),
        AsideState::Failed(_) => (tr("Failed"), RED_TEXT),
    }
}

/// The outline Close pill — the web's bordered button
/// (`BtwAsidePanel.module.css:24-31`) in the kit's 32 px pill shape (board 4
/// K1 / `dialog_view::PILL_H`, as region 4 draws it) — with its host hit.
fn close_pill(m: &Metrics) -> String {
    let s = scale(m.density);
    format!(
        "btw_aside_close := View{{width: Fit height: 32 flow: Overlay\n\
         RoundedView{{width: Fit height: 32 flow: Right align: Align{{y: 0.5}} padding: Inset{{left: 12 right: 12}}\n\
         draw_bg +: {{color: {SURFACE} border_radius: 16.0 border_size: 1.0 border_color: {BORDER}}}\n\
         {l}}}\n\
         {h}}}\n",
        l = label(
            "btw_aside_close_label",
            tr("Close"),
            &style(Face::Medium, s.small, s.small_line),
            INK,
            "width: Fit height: Fit",
        ),
        h = hit("btw_aside_close_hit", 16.0),
    )
}

/// The panel (`BtwAsidePanel`): expanded, or folded to one row.
pub fn panel(v: &PanelView, m: &Metrics, window_h: f64) -> String {
    let s = scale(m.density);
    let title_style = style(Face::SemiBold, s.body, s.body_line);
    let title = label("btw_aside_title", tr("Aside — /btw"), &title_style, INK, "width: Fit height: Fit");
    if v.aside.collapsed {
        // Region 5b: › Aside — /btw · the question (one line) · state · Close.
        // A phone's 336 px card keeps the question readable: "Answered" (the
        // quiet state) yields its room there; Answering… and Failed stay.
        let (word, word_color) = state_word(&v.aside.state);
        let phone = m.density == crate::conv_layout::Density::Phone;
        let gap = if phone { 8.0 } else { 10.0 };
        let word = if phone && matches!(v.aside.state, AsideState::Answered { .. }) {
            String::new()
        } else {
            label(
                "btw_aside_state",
                word,
                &style(Face::Regular, s.small, s.small_line),
                word_color,
                "width: Fit height: Fit",
            )
        };
        return format!(
            "btw_aside := RoundedView{{width: Fill height: Fit flow: Right align: Align{{y: 0.5}} spacing: 8 \
             padding: Inset{{left: 8 right: 10 top: 6 bottom: 6}}\n\
             draw_bg +: {{color: {SURFACE} border_radius: 12.0 border_size: 1.0 border_color: {BORDER}}}\n\
             btw_aside_toggle := View{{width: Fill height: {HEAD_H} flow: Overlay\n\
             View{{width: Fill height: Fill flow: Right align: Align{{y: 0.5}} spacing: {gap} padding: Inset{{left: 4 right: 4}}\n\
             {chev}{title}{question}{word}}}\n\
             {toggle}}}\n\
             {close}}}\n",
            chev = svg("btw_aside_chev", "chevron_right.svg", 14.0, INK),
            question = label(
                "btw_aside_question",
                &v.aside.question,
                &style(Face::Regular, s.small, s.small_line),
                MUTED,
                "width: Fill height: Fit max_lines: 1 text_overflow: TextOverflow.Ellipsis",
            ),
            toggle = hit("btw_aside_toggle_hit", 8.0),
            close = close_pill(m),
        );
    }
    let wrap = format!("width: Fill height: Fit flow: Right{{wrap: true}} max_lines: {FIT_WRAP_LINES}");
    let mut body = String::new();
    // The asking Session (the board's monospace scope line).
    body.push_str(&label(
        "btw_aside_scope",
        &v.scope,
        &style(Face::Mono, s.tiny, s.small_line - 2.0),
        MUTED,
        "width: Fill height: Fit max_lines: 1 text_overflow: TextOverflow.Ellipsis",
    ));
    // The question (`.question { white-space: pre-wrap }`).
    body.push_str(&label(
        "btw_aside_question",
        &v.aside.question,
        &style(Face::Regular, s.body, s.body_line),
        INK,
        &wrap,
    ));
    match &v.aside.state {
        AsideState::Answering => {
            // `<p role="status">Answering…</p>`, with the working row's
            // spinner (the board's dashed ring).
            body.push_str(&format!(
                "btw_aside_status := View{{width: Fill height: 28 flow: Right align: Align{{y: 0.5}} spacing: 8\n{}{}}}\n",
                svg("btw_aside_spin", "components/working-row/assets/icon_spinner.svg", 16.0, MUTED),
                label(
                    "btw_aside_status_label",
                    tr("Answering…"),
                    &style(Face::Regular, s.small + 1.0, s.small_line),
                    MUTED,
                    "width: Fit height: Fit",
                ),
            ));
        }
        AsideState::Answered { answer, .. } => {
            // The whole Markdown answer, one native flow region at the
            // conversation's prose rhythm (`MarkdownBody`).
            body.push_str(&markdown_region(
                "btw_aside_answer",
                &crate::markdown::sanitize(answer, true),
                false,
                m,
            ));
        }
        AsideState::Failed(f) => {
            // `<p role="alert">` — the board's red lead over a muted cause.
            let (lead, cause) = failure_split(*f);
            let mut err = label(
                "btw_aside_error",
                &lead,
                &style(Face::Medium, s.body, s.body_line),
                RED_TEXT,
                &wrap,
            );
            if !cause.is_empty() {
                err.push_str(&label(
                    "btw_aside_error_detail",
                    &cause,
                    &style(Face::Regular, s.small, s.small_line),
                    MUTED,
                    &wrap,
                ));
            }
            body.push_str(&format!("View{{width: Fill height: Fit flow: Down spacing: 4\n{err}}}\n"));
        }
    }
    // `.note` (0.85rem, secondary), a little apart from what it qualifies.
    body.push_str(&label(
        "btw_aside_note",
        tr("This aside is not saved to the conversation."),
        &style(Face::Regular, s.small, s.small_line),
        MUTED,
        &format!("{wrap} margin: Inset{{top: 4}}"),
    ));
    format!(
        "btw_aside := RoundedView{{width: Fill height: Fit flow: Down spacing: {HEAD_GAP} \
         padding: Inset{{left: 16 right: 12 top: {PAD_TOP} bottom: {PAD_BOTTOM}}}\n\
         draw_bg +: {{color: {SURFACE} border_radius: 12.0 border_size: 1.0 border_color: {BORDER}}}\n\
         btw_aside_head := View{{width: Fill height: {HEAD_H} flow: Right align: Align{{y: 0.5}} spacing: 8\n\
         btw_aside_toggle := View{{width: Fit height: {HEAD_H} flow: Overlay\n\
         View{{width: Fit height: Fill flow: Right align: Align{{y: 0.5}} spacing: 6 padding: Inset{{right: 6}}\n\
         {title}{chev}}}\n\
         {toggle}}}\n\
         View{{width: Fill height: 1}}\n\
         {close}}}\n\
         btw_aside_body := ScrollYView{{width: Fill height: Fit max_height: {cap} flow: Down spacing: 10 \
         padding: Inset{{right: 6}}\n\
         {body}}}\n\
         }}\n",
        chev = svg("btw_aside_chev", "chevron_down.svg", 14.0, INK),
        toggle = hit("btw_aside_toggle_hit", 8.0),
        close = close_pill(m),
        cap = body_cap(window_h),
    )
}

/// The sidebar marker's label: the command's own name (never translated).
pub const MARK_LABEL: &str = "/btw";

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_store::domains::btw::{Gate, Reply};

    const LIVE: Gate = Gate { advertised: true, connected: true };

    fn store_with(session: &str) -> Store {
        let store = Store::new();
        store.set_sessions(vec![octoscode_store::Session {
            id: session.into(),
            title: Some("Fix steer queue drop on reconnect".into()),
            message_count: 4,
            updated_at: None,
            last_prompt: None,
            active_turn: false,
        }]);
        store.domains.session.set_workspace_root(session, "/home/user/src/octos");
        store.set_active(Some(session.into()));
        store
    }

    #[test]
    fn the_panel_follows_the_active_sessions_aside() {
        let store = store_with("p:x");
        let m = Metrics::for_window(990.0, true);
        assert_eq!(lower(&store, &m, 600.0), "", "no aside, no panel");
        let t = match store.domains.btw.ask("p:x", "why does redeliver drain the whole queue first?", LIVE) {
            octoscode_store::domains::btw::Admission::Accepted(t) => t,
            other => panic!("{other:?}"),
        };
        let answering = lower(&store, &m, 600.0);
        assert!(answering.contains("btw_aside_status_label"), "Answering…");
        assert!(answering.contains("Aside — /btw"));
        assert!(answering.contains("octos · Fix steer queue drop on reconnect"), "the scope line names the asker");
        assert!(answering.contains("This aside is not saved to the conversation."));
        assert!(answering.contains("btw_aside_close_hit") && answering.contains("btw_aside_toggle_hit"));
        assert!(answering.contains("max_height: 240"), "min(50% of 600, 480) minus the fixed head: {answering}");
        store.domains.btw.settle(&t, Reply::Answer { answer: "Draining first keeps `redeliver` ordered.".into(), model: None }, true);
        let answered = lower(&store, &m, 600.0);
        assert!(answered.contains("btw_aside_answer := Markdown"), "the whole Markdown answer");
        assert!(!answered.contains("btw_aside_status"), "no Answering… once answered");
        // Another Session is active: no panel.
        store.set_active(Some("p:y".into()));
        assert_eq!(lower(&store, &m, 600.0), "");
    }

    #[test]
    fn the_collapsed_row_and_the_stale_copy() {
        let store = store_with("p:x");
        let m = Metrics::for_window(360.0, false);
        let _ = store.domains.btw.ask("p:x", "why?", LIVE);
        store.domains.btw.link_changed();
        let stale = lower(&store, &m, 780.0);
        assert!(stale.contains("The Session connection changed before the aside completed."), "the red lead");
        assert!(stale.contains("Ask again when it is ready."), "the muted cause");
        assert!(stale.contains(RED_TEXT));
        store.domains.btw.toggle_collapsed("p:x");
        let folded = lower(&store, &m, 780.0);
        assert!(folded.contains("chevron_right.svg") && folded.contains("btw_aside_state"));
        assert!(!folded.contains("btw_aside_body"), "one row");
        assert!(folded.contains("TextOverflow.Ellipsis"));
    }

    #[test]
    fn the_cap_is_half_the_window_up_to_480() {
        assert_eq!(cap_height(600.0), 300.0);
        assert_eq!(cap_height(1200.0), 480.0);
        assert_eq!(cap_height(0.0), 300.0);
        assert_eq!(body_cap(780.0), 330.0);
    }

    #[test]
    fn the_failure_copy_reads_chinese_in_zh() {
        use crate::i18n::{set_language, Lang};
        set_language(Lang::Zh);
        assert_eq!(failure_split(Failure::Stale), ("旁问完成前，会话连接已变更。".to_owned(), "请在连接就绪后重新提问。".to_owned()));
        assert_eq!(failure_copy(Failure::Failed), "无法回答此旁问。请重试。");
        assert_eq!(tr("Answered"), "已回答");
        set_language(Lang::En);
        assert_eq!(failure_copy(Failure::Failed), FAILED_COPY);
    }

    #[test]
    fn failure_copy_splits_into_lead_and_cause() {
        assert_eq!(
            failure_split(Failure::Stale),
            ("The Session connection changed before the aside completed.".to_owned(), "Ask again when it is ready.".to_owned())
        );
        assert_eq!(failure_split(Failure::Failed), ("The aside could not be answered.".to_owned(), "Try again.".to_owned()));
    }
}
