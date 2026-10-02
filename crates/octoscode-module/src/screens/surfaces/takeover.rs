//! A6 — the two TAKEOVER cards that replace the composer while the turn waits
//! on the person: the approval card (Gate-B `conversation-05`, the web's
//! `features/approval/ApprovalPanel.tsx`) and the structured user-question
//! card (Gate-B `conversation-06`, `features/questions/UserQuestionPanel.tsx`).
//!
//! Both are session-scoped takeovers (`hidesBackground={false}`,
//! `ApprovalPanel.tsx:61`): they sit where the composer was (`App.tsx:2759-
//! 2826`, the composer form is the else-branch), the sidebar and every other
//! session stay reachable, and only the waiting session shows its card.
//!
//! Look: the approved Gate-B cards — the shield / question glyph beside the
//! title, the command in a grey mono box, the reason under it, one black
//! primary pill, an outline secondary, a quiet third action and the key hint
//! (`Y / S / N`). On a desktop window the actions sit in one right-aligned row
//! (the web's `.approval-actions`, `styles.css:1098-1104`); on a phone they
//! stack full width as the board draws them.
//!
//! The answer model is the web's, ported verbatim: `answers.ts`
//! (`toggleQuestionOption`, `answersComplete`, `toWireAnswers`) and
//! `question-card.ts` (`submitBlockedReason`, `choiceHint`,
//! `nextOptionIndex`, `arrowDelta`).
use octoscode_store::domains::approval::{ApprovalDetail, PendingQuestion};
use serde_json::{json, Value};

use crate::screens::board3::ui::{self, tok, Btn, Dsl, Face, Txt, W};

// ------------------------------------------------------------ the question

/// One structured question (`UserQuestion`, octos-core `ui_protocol.rs:5616`),
/// parsed the way the web's `parseQuestion` does (`interaction.ts:165-195`):
/// a malformed question makes the whole request unrenderable (fail closed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub header: String,
    pub question: String,
    pub options: Vec<(String, String)>,
    pub multi_select: bool,
    pub allow_free_text: bool,
}

/// Parse the stored `questions` JSON. `None` = malformed (the web returns
/// `null` from `parseUserQuestionRequested` and shows no structured card).
pub fn parse_questions(v: &Value) -> Option<Vec<Question>> {
    let arr = v.as_array()?;
    arr.iter()
        .map(|q| {
            let options = q
                .get("options")?
                .as_array()?
                .iter()
                .map(|o| {
                    Some((
                        o.get("label")?.as_str()?.to_owned(),
                        o.get("description")?.as_str()?.to_owned(),
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Question {
                header: q.get("header")?.as_str()?.to_owned(),
                question: q.get("question")?.as_str()?.to_owned(),
                options,
                multi_select: q.get("multi_select").and_then(|b| b.as_bool()) == Some(true),
                allow_free_text: q.get("allow_free_text").and_then(|b| b.as_bool()) == Some(true),
            })
        })
        .collect()
}

/// One question's draft answer (`DraftAnswer`, `answers.ts:6-9`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    pub selected: Vec<String>,
    pub free_text: String,
}

/// `emptyAnswers` (`answers.ts:11-16`).
pub fn empty_answers(n: usize) -> Vec<Draft> {
    vec![Draft::default(); n]
}

/// `toggleQuestionOption` (`answers.ts:18-33`): multi-select toggles the
/// label independently; single-select REPLACES the selection.
pub fn toggle_option(q: &Question, d: &Draft, label: &str) -> Draft {
    let selected = d.selected.iter().any(|l| l == label);
    let next = if q.multi_select {
        if selected {
            d.selected.iter().filter(|l| *l != label).cloned().collect()
        } else {
            let mut v = d.selected.clone();
            v.push(label.to_owned());
            v
        }
    } else {
        vec![label.to_owned()]
    };
    Draft { selected: next, free_text: d.free_text.clone() }
}

/// `answersComplete` (`answers.ts:35-42`): every question has a selection or
/// a non-blank free text.
pub fn answers_complete(answers: &[Draft]) -> bool {
    answers.iter().all(|a| !a.selected.is_empty() || !a.free_text.trim().is_empty())
}

/// `toWireAnswers` (`answers.ts:44-55`): `selected_labels` only when some are
/// chosen, `free_text` only when non-blank (trimmed).
pub fn to_wire_answers(answers: &[Draft]) -> Value {
    Value::Array(
        answers
            .iter()
            .map(|a| {
                let mut o = serde_json::Map::new();
                if !a.selected.is_empty() {
                    o.insert("selected_labels".into(), json!(a.selected));
                }
                let t = a.free_text.trim();
                if !t.is_empty() {
                    o.insert("free_text".into(), json!(t));
                }
                Value::Object(o)
            })
            .collect(),
    )
}

/// `submitBlockedReason` (`question-card.ts:15-27`): why the primary action
/// cannot run yet — `None` means it is live. A disabled control always has a
/// reason next to it.
pub fn submit_blocked_reason(busy: bool, answers: &[Draft]) -> Option<&'static str> {
    if busy {
        return Some("Sending your answer…");
    }
    if answers_complete(answers) {
        return None;
    }
    let remaining = answers
        .iter()
        .filter(|a| a.selected.is_empty() && a.free_text.trim().is_empty())
        .count();
    Some(if remaining == answers.len() && answers.len() == 1 {
        "Choose an option to continue"
    } else {
        "Answer every question to continue"
    })
}

/// `choiceHint` (`question-card.ts:30-39`).
pub fn choice_hint(q: &Question) -> &'static str {
    match (q.multi_select, q.allow_free_text) {
        (true, true) => "Choose any that apply, or write your own",
        (true, false) => "Choose any that apply",
        (false, true) => "Choose one, or write your own",
        (false, false) => "Choose one",
    }
}

/// `nextOptionIndex` (`question-card.ts:46-53`): wraps at both ends, like a
/// radio group.
pub fn next_option_index(current: usize, delta: i32, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let c = count as i64;
    (((current as i64 + delta as i64) % c + c) % c) as usize
}

/// `arrowDelta` (`question-card.ts:56-60`): only the four arrows move.
pub fn arrow_delta(key: &str) -> Option<i32> {
    match key {
        "ArrowDown" | "ArrowRight" => Some(1),
        "ArrowUp" | "ArrowLeft" => Some(-1),
        _ => None,
    }
}

/// The consequence line the primary action carries (round 2, judge #4:
/// `UserQuestionPanel.tsx:195-199` — what Continue does, not a bare submit).
/// The web's copy says "resumes the peer"; natively the answered question
/// resumes the waiting TURN (`runtime_resumed`), which is what this says.
pub const CONSEQUENCE: &str = "Sends this answer and resumes the turn";

/// The question card's UI state (the web keeps it in the panel's
/// `useState`, keyed by `request.questionId`: a new question resets it).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuestionUi {
    pub question_id: String,
    pub answers: Vec<Draft>,
    /// The free-text values as last MOUNTED (the DSL embeds these; typing
    /// updates `answers` only, so a keystroke never remounts the input).
    pub free_snap: Vec<String>,
    /// The keyboard focus: (question, option).
    pub focus: (usize, usize),
    /// Draw the focus ring only after the keyboard moved it.
    pub focus_visible: bool,
    pub busy: bool,
    pub error: Option<String>,
    /// The DSL last handed to the mount (`lower_takeover` compares against
    /// it to tell a remount from a keystroke).
    pub last_dsl: Option<String>,
}

impl QuestionUi {
    /// Bind to `q` (reset on a new question id, `UserQuestionPanel.tsx:40-42`).
    pub fn bind(&mut self, q: &PendingQuestion, n: usize) {
        if self.question_id != q.question_id || self.answers.len() != n {
            *self = QuestionUi {
                question_id: q.question_id.clone(),
                answers: empty_answers(n),
                free_snap: vec![String::new(); n],
                ..Default::default()
            };
        }
    }
}

/// The approval card's UI state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApprovalUi {
    /// The approval id a decision is in flight for (`busy`).
    pub busy: Option<String>,
    /// (approval id, readable error) of the last failed decision.
    pub error: Option<(String, String)>,
}

// --------------------------------------------------------------- the view

/// The responsive look: the desktop row, or the phone's stacked pills.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub phone: bool,
    /// The card's outer width (the composer dock's inner width).
    pub width: f64,
    /// The most the card may grow (the web's `max-height: min(620px, 100dvh - 120px)`).
    pub max_h: f64,
}

impl Look {
    fn pad(&self) -> f64 {
        if self.phone {
            16.0
        } else {
            18.0
        }
    }
    fn inner(&self) -> f64 {
        (self.width - 2.0 * self.pad()).max(120.0)
    }
}

fn card_open(d: &mut Dsl, id: &str, look: &Look) {
    let p = look.pad();
    d.surface(
        id,
        &format!(
            "width: Fill height: Fit flow: Down spacing: 0 padding: Inset{{left: {p} right: {p} top: {top} bottom: {bottom}}}",
            top = if look.phone { 16.0 } else { 16.0 },
            bottom = if look.phone { 12.0 } else { 14.0 },
        ),
        tok::SURFACE,
        16.0,
        Some(tok::HAIRLINE),
    );
}

/// Insert line breaks into runs too long to wrap at a space (a command line,
/// a path): the web's `word-break: break-all` (`styles.css:1085-1091`). The
/// renderer only breaks at word boundaries, so an unbroken run would clip.
pub fn hard_wrap(s: &str, max_chars: usize) -> String {
    let max = max_chars.max(8);
    let mut out = String::with_capacity(s.len() + 8);
    for (li, line) in s.lines().enumerate() {
        if li > 0 {
            out.push('\n');
        }
        let mut col = 0usize;
        for word in line.split_inclusive(' ') {
            let n = word.chars().count();
            if col + n > max && col > 0 {
                out.push('\n');
                col = 0;
            }
            if n > max {
                for (i, c) in word.chars().enumerate() {
                    if i > 0 && (col % max) == 0 {
                        out.push('\n');
                        col = 0;
                    }
                    out.push(c);
                    col += 1;
                }
            } else {
                out.push_str(word);
                col += n;
            }
        }
    }
    out
}

/// The approval card (`ApprovalPanel.tsx:58-127` + Gate-B conversation-05).
/// `diff` = the payload's preview id exists (the `D` key / Review diff).
pub fn approval_card(
    d: &mut Dsl,
    p: &octoscode_store::domains::approval::PendingApproval,
    a: &ApprovalDetail,
    ui_state: &ApprovalUi,
    look: &Look,
) {
    let busy = ui_state.busy.as_deref() == Some(p.id.as_str());
    let diff = p.preview_id.is_some();
    card_open(d, "cv_ap_card", look);
    // Title row: the Gate-B shield + the server's title; the risk on the right
    // (the web's strip `{risk} risk`, ApprovalPanel.tsx:71-76).
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
    d.icon("cv_ap_icon", "cv_shield.svg", if look.phone { 24.0 } else { 22.0 }, tok::TEXT);
    let title_px = if look.phone { 17.0 } else { 16.0 };
    let risk = a.risk.as_deref().map(str::trim).filter(|r| !r.is_empty());
    let risk_w = risk.map(|r| ui::text_w(&format!("{r} risk"), 11.0, Face::Medium) + 22.0).unwrap_or(0.0);
    let title_budget = look.inner() - 34.0 - risk_w;
    let title = if a.title.trim().is_empty() { "Approval required" } else { a.title.trim() };
    d.text(
        "cv_ap_title",
        &ui::fit_w(title, title_budget, title_px, Face::Semibold),
        &Txt::new(title_px, Face::Semibold, tok::TEXT).w(W::Fill),
    );
    if let Some(r) = risk {
        let (fg, bg, line) = match r.to_ascii_lowercase().as_str() {
            "high" | "critical" => (tok::RED_TEXT, tok::RED_BG, Some("#f5c2c7ff")),
            "medium" | "moderate" => (tok::AMBER, tok::AMBER_BG, Some(tok::AMBER_LINE)),
            _ => (tok::MUTED, tok::SURFACE2, Some(tok::HAIRLINE)),
        };
        d.chip("cv_ap_risk", &format!("{r} risk"), fg, bg, line, false);
    }
    d.close();
    d.gap(W::Fill, 12.0);
    // The typed command (`approvalCommand`, ApprovalPanel.tsx:131-139) in the
    // Gate-B grey mono box; long runs break like the web's `break-all`.
    if let Some(cmd) = a.command.as_deref().filter(|c| !c.trim().is_empty()) {
        let box_inner = look.inner() - 28.0;
        let per_line = ((box_inner / (13.0 * 0.6)).floor() as usize).max(12);
        d.surface(
            "cv_ap_cmd_box",
            "width: Fill height: Fit flow: Down padding: Inset{left: 14 right: 14 top: 11 bottom: 11}",
            tok::SURFACE2,
            10.0,
            Some(tok::HAIRLINE),
        );
        d.text("cv_ap_cmd", &hard_wrap(cmd.trim(), per_line), &Txt::new(13.0, Face::Mono, tok::TEXT).w(W::Fill).wrap());
        d.close();
        d.gap(W::Fill, 10.0);
    }
    // The body (the server's reason / description).
    if !a.body.trim().is_empty() && Some(a.body.trim()) != a.command.as_deref().map(str::trim) {
        d.text(
            "cv_ap_body",
            a.body.trim(),
            &Txt::new(if look.phone { 14.0 } else { 13.5 }, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
        );
        d.gap(W::Fill, 6.0);
    }
    // `{tool} · {kind}` (ApprovalPanel.tsx:81-84).
    let tool_line = match a.kind.as_deref().filter(|k| !k.is_empty()) {
        Some(k) => format!("{} · {k}", a.tool_name),
        None => a.tool_name.clone(),
    };
    d.text("cv_ap_tool", &tool_line, &Txt::new(12.0, Face::Regular, tok::FAINT).w(W::Fill));
    if let Some((id, err)) = &ui_state.error {
        if id == &p.id {
            d.gap(W::Fill, 6.0);
            d.text("cv_ap_error", err, &Txt::new(12.5, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
        }
    }
    d.gap(W::Fill, if look.phone { 16.0 } else { 14.0 });
    let hint = if diff { "Y / S / N / D" } else { "Y / S / N" };
    let kind = |primary: bool| {
        if busy {
            Btn::Disabled
        } else if primary {
            Btn::Primary
        } else {
            Btn::Outline
        }
    };
    let once_label = if busy { "Sending…" } else { "Approve once" };
    if look.phone {
        // Gate-B: three stacked pills, the key hint centred under them.
        let col = d.anon();
        d.view(&col, "width: Fill height: Fit flow: Down spacing: 10");
        d.button("cv_ap_once", once_label, "cv.approval.once", kind(true), W::Fill, 46.0);
        d.button("cv_ap_session", "Approve for session", "cv.approval.session", kind(false), W::Fill, 46.0);
        d.button("cv_ap_deny", "Deny", "cv.approval.deny", if busy { Btn::Disabled } else { Btn::Ghost }, W::Fill, 40.0);
        if diff {
            d.button("cv_ap_diff", "Review diff", "cv.approval.diff", if busy { Btn::Disabled } else { Btn::Ghost }, W::Fill, 36.0);
        }
        d.close();
        d.gap(W::Fill, 6.0);
        let h = d.anon();
        d.view(&h, "width: Fill height: Fit flow: Right align: Align{x: 0.5 y: 0.5}");
        d.text("cv_ap_hint", hint, &Txt::new(12.0, Face::Regular, tok::FAINT));
        d.close();
    } else {
        // The web's right-aligned action row (`.approval-actions`), the key
        // hint at its start.
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        d.text("cv_ap_hint", hint, &Txt::new(12.0, Face::Regular, tok::FAINT));
        d.gap(W::Fill, 1.0);
        if diff {
            d.button("cv_ap_diff", "Review diff", "cv.approval.diff", kind(false), W::Fit, 34.0);
        }
        d.button("cv_ap_deny", "Deny", "cv.approval.deny", kind(false), W::Fit, 34.0);
        d.button("cv_ap_session", "Approve for session", "cv.approval.session", kind(false), W::Fit, 34.0);
        d.button("cv_ap_once", once_label, "cv.approval.once", kind(true), W::Fit, 34.0);
        d.close();
    }
    d.close();
}

/// The question card (`UserQuestionPanel.tsx:70-209` + Gate-B
/// conversation-06). The submit button is built TWICE — live and disabled
/// with its reason — and the host flips their visibility from the draft
/// (`live_visibility`), so typing an answer never remounts the input.
pub fn question_card(d: &mut Dsl, q: &PendingQuestion, qs: &[Question], st: &QuestionUi, look: &Look) {
    card_open(d, "cv_q_card", look);
    // Header: the Gate-B decision glyph + "Octos needs a decision".
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
    d.icon("cv_q_icon", "cv_question.svg", if look.phone { 24.0 } else { 22.0 }, tok::TEXT);
    d.text(
        "cv_q_eyebrow",
        "Octos needs a decision",
        &Txt::new(if look.phone { 17.0 } else { 16.0 }, Face::Semibold, tok::TEXT).w(W::Fill),
    );
    d.close();
    // The request's own title (the decision), then its body when it says
    // more than the title (the generic fallback text usually repeats it).
    let title = q.title.trim();
    let single = qs.len() == 1 && qs[0].question.trim() == title;
    d.gap(W::Fill, 10.0);
    if !title.is_empty() {
        d.text(
            "cv_q_title",
            title,
            &Txt::new(if look.phone { 16.0 } else { 15.0 }, Face::Regular, tok::TEXT).w(W::Fill).wrap(),
        );
    }
    let body = q.body.trim();
    if !body.is_empty() && !body.to_lowercase().contains(&title.to_lowercase()) {
        d.gap(W::Fill, 4.0);
        d.text("cv_q_body", body, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    // The questions scroll inside the card when they outgrow it (the web's
    // card `max-height` + `overflow-y: auto`).
    let chrome = 170.0;
    d.open(
        "cv_q_scroll",
        "ScrollYView",
        &format!(
            "width: Fill height: Fit max_height: {} flow: Down padding: Inset{{left: 0 top: 0 right: 4 bottom: 0}}",
            (look.max_h - chrome).max(160.0).floor()
        ),
    );
    for (qi, question) in qs.iter().enumerate() {
        let draft = st.answers.get(qi).cloned().unwrap_or_default();
        d.gap(W::Fill, if qi == 0 { 12.0 } else { 18.0 });
        // The legend: header (caps, muted) then the question unless the
        // card's title already says it.
        d.text(
            &format!("cv_q_{qi}_header"),
            &question.header.to_uppercase(),
            &Txt::new(11.0, Face::Medium, tok::MUTED).w(W::Fill),
        );
        if !single {
            d.gap(W::Fill, 3.0);
            d.text(
                &format!("cv_q_{qi}_question"),
                question.question.trim(),
                &Txt::new(13.5, Face::Medium, tok::TEXT).w(W::Fill).wrap(),
            );
        }
        d.gap(W::Fill, 3.0);
        d.text(&format!("cv_q_{qi}_hint"), choice_hint(question), &Txt::new(11.5, Face::Regular, tok::FAINT).w(W::Fill));
        d.gap(W::Fill, 6.0);
        for (oi, (label, desc)) in question.options.iter().enumerate() {
            let selected = draft.selected.iter().any(|l| l == label);
            let focused = st.focus_visible && st.focus == (qi, oi);
            option_row(d, qi, oi, label, desc, selected, focused, question.multi_select, look);
        }
        if question.allow_free_text {
            d.gap(W::Fill, 6.0);
            d.text(&format!("cv_q_{qi}_other_label"), "Other", &Txt::new(12.0, Face::Medium, tok::MUTED).w(W::Fill));
            d.gap(W::Fill, 5.0);
            let snap = st.free_snap.get(qi).cloned().unwrap_or_default();
            d.input(
                &format!("cv_q_{qi}_other"),
                &format!("cv.q.other#{qi}"),
                &snap,
                "Type another answer",
                false,
                if look.phone { 44.0 } else { 38.0 },
            );
        }
    }
    d.close(); // scroll
    if let Some(err) = &st.error {
        d.gap(W::Fill, 8.0);
        d.text("cv_q_error", err, &Txt::new(12.5, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
    }
    d.gap(W::Fill, 14.0);
    let reason = submit_blocked_reason(st.busy, &st.answers).unwrap_or("Choose an option to continue");
    let primary_label = if st.busy { "Sending…" } else { "Submit answer" };
    if look.phone {
        d.text("cv_q_consequence", CONSEQUENCE, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill));
        d.gap(W::Fill, 4.0);
        d.text("cv_q_reason", reason, &Txt::new(12.0, Face::Regular, tok::FAINT).w(W::Fill));
        d.gap(W::Fill, 8.0);
        d.button("cv_q_submit", primary_label, "cv.q.submit", Btn::Primary, W::Fill, 46.0);
        d.button("cv_q_submit_off", primary_label, "cv.q.submit", Btn::Disabled, W::Fill, 46.0);
        d.gap(W::Fill, 4.0);
        d.button("cv_q_stop", "Stop turn", "cv.q.stop", if st.busy { Btn::Disabled } else { Btn::Ghost }, W::Fill, 40.0);
    } else {
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
        // `Esc stops the active turn` (UserQuestionPanel.tsx:188-192), as a
        // control too: the phone has no Escape key.
        d.link("cv_q_stop", "Stop turn · Esc", if st.busy { None } else { Some("cv.q.stop") }, 12.5);
        d.gap(W::Fill, 1.0);
        let col = d.anon();
        d.view(&col, "width: Fit height: Fit flow: Down align: Align{x: 1.0 y: 0.5} spacing: 2");
        d.text("cv_q_consequence", CONSEQUENCE, &Txt::new(12.0, Face::Regular, tok::MUTED));
        d.text("cv_q_reason", reason, &Txt::new(12.0, Face::Regular, tok::FAINT));
        d.close();
        d.button("cv_q_submit", primary_label, "cv.q.submit", Btn::Primary, W::Px(150.0), 38.0);
        d.button("cv_q_submit_off", primary_label, "cv.q.submit", Btn::Disabled, W::Px(150.0), 38.0);
        d.close();
    }
    d.close();
}

#[allow(clippy::too_many_arguments)]
fn option_row(
    d: &mut Dsl,
    qi: usize,
    oi: usize,
    label: &str,
    desc: &str,
    selected: bool,
    focused: bool,
    multi: bool,
    look: &Look,
) {
    let id = format!("cv_q_{qi}_opt_{oi}");
    let ring = if focused { Some(tok::BLUE) } else { None };
    let fill = if selected { tok::BLUE_BG } else { tok::SURFACE };
    d.surface(
        &format!("{id}_box"),
        "width: Fill height: Fit flow: Overlay",
        fill,
        10.0,
        ring,
    );
    let row = d.anon();
    d.view(
        &row,
        &format!(
            "width: Fill height: Fit flow: Right align: Align{{x: 0.0 y: 0.0}} spacing: 12 padding: Inset{{left: 8 right: 10 top: {v} bottom: {v}}}",
            v = if look.phone { 10.0 } else { 8.0 }
        ),
    );
    let glyph = match (multi, selected) {
        (true, true) => "cv_check_on.svg",
        (true, false) => "cv_check_off.svg",
        (false, true) => "cv_radio_on.svg",
        (false, false) => "cv_radio_off.svg",
    };
    d.icon(&format!("{id}_mark"), glyph, 20.0, tok::BLUE);
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 2");
    d.text(
        &format!("{id}_label"),
        label,
        &Txt::new(if look.phone { 15.0 } else { 14.0 }, if selected { Face::Medium } else { Face::Regular }, tok::TEXT)
            .w(W::Fill)
            .wrap(),
    );
    if !desc.trim().is_empty() {
        d.text(&format!("{id}_desc"), desc.trim(), &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    d.close();
    d.close();
    d.tap(&id, &format!("cv.q.opt#{}", qi * 100 + oi));
    d.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(multi: bool, free: bool) -> Question {
        Question {
            header: "Color".into(),
            question: "Which color?".into(),
            options: vec![("Blue".into(), "calm".into()), ("Red".into(), "bold".into())],
            multi_select: multi,
            allow_free_text: free,
        }
    }

    #[test]
    fn answers_follow_the_web_rules() {
        let single = q(false, true);
        let multi = q(true, false);
        let d = Draft::default();
        // answers.test.ts: single REPLACES, multi toggles independently.
        let d1 = toggle_option(&single, &d, "Blue");
        assert_eq!(toggle_option(&single, &d1, "Red").selected, vec!["Red"]);
        let m1 = toggle_option(&multi, &d, "Blue");
        let m2 = toggle_option(&multi, &m1, "Red");
        assert_eq!(m2.selected, vec!["Blue", "Red"]);
        assert_eq!(toggle_option(&multi, &m2, "Blue").selected, vec!["Red"]);
        // Wire shape: labels only when chosen, free text only when non-blank.
        let wire = to_wire_answers(&[m2.clone(), Draft { selected: vec![], free_text: "  teal ".into() }]);
        assert_eq!(wire, json!([{"selected_labels": ["Blue", "Red"]}, {"free_text": "teal"}]));
        assert!(answers_complete(&[m2]));
        assert!(!answers_complete(&[Draft { selected: vec![], free_text: "   ".into() }]));
    }

    #[test]
    fn a_disabled_primary_always_says_why() {
        // question-card.test.ts
        assert_eq!(submit_blocked_reason(false, &[Draft::default()]), Some("Choose an option to continue"));
        assert_eq!(
            submit_blocked_reason(false, &[Draft::default(), Draft::default()]),
            Some("Answer every question to continue")
        );
        let done = Draft { selected: vec!["Blue".into()], free_text: String::new() };
        assert_eq!(submit_blocked_reason(false, &[done.clone()]), None);
        // A pending send is explained before a missing answer.
        assert_eq!(submit_blocked_reason(true, &[Draft::default()]), Some("Sending your answer…"));
        assert_eq!(choice_hint(&q(true, true)), "Choose any that apply, or write your own");
        assert_eq!(choice_hint(&q(false, false)), "Choose one");
    }

    #[test]
    fn arrows_wrap_inside_a_group_and_only_arrows_move() {
        assert_eq!(next_option_index(0, -1, 3), 2);
        assert_eq!(next_option_index(2, 1, 3), 0);
        assert_eq!(next_option_index(1, 1, 3), 2);
        assert_eq!(next_option_index(0, 1, 0), 0);
        for (k, v) in [("ArrowDown", Some(1)), ("ArrowRight", Some(1)), ("ArrowUp", Some(-1)), ("ArrowLeft", Some(-1)), ("Tab", None), ("Enter", None)] {
            assert_eq!(arrow_delta(k), v, "{k}");
        }
    }

    #[test]
    fn the_recorded_question_parses_and_a_malformed_one_fails_closed() {
        let recorded = json!([{"allow_free_text": true, "header": "Color choice", "multi_select": false,
            "options": [{"description": "Calm", "label": "Blue"}, {"description": "Bold", "label": "Red"}],
            "question": "Which color would you like to pick?"}]);
        let qs = parse_questions(&recorded).expect("parses");
        assert_eq!(qs[0].options.len(), 2);
        assert!(qs[0].allow_free_text && !qs[0].multi_select);
        assert!(parse_questions(&json!([{"header": "x", "question": "y", "options": [{"label": 3}]}])).is_none());
    }

    #[test]
    fn long_commands_break_like_break_all() {
        let s = hard_wrap("git push origin feat/a-very-long-branch-name-that-keeps-going", 20);
        assert!(s.lines().all(|l| l.chars().count() <= 20), "{s}");
        assert_eq!(s.replace('\n', ""), "git push origin feat/a-very-long-branch-name-that-keeps-going");
    }
}
