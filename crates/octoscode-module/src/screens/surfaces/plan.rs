//! A6 — the agent's PLAN checklist (Gate-B `conversation-10`; the web's
//! `features/supervision/plan.ts` + `PlanCard.tsx`).
//!
//! Rules (ported verbatim from `plan.ts`):
//! * `plan/updated` REPLACES the session's plan wholesale — never a diff
//!   (`applyPlanUpdated`, `:20-26`; the client's `PlanUpdatedHandler` writes
//!   it through `Tasks::set_plan`);
//! * a plan is scoped to the turn that authored it and is dropped on THAT
//!   turn's terminal (`clearPlanForTurn`, `:31-36`; the turn domain's three
//!   terminal handlers call `clear_plan_for_turn`);
//! * the feature gate fails closed: no `plan.todos.v1`, no plan, or an empty
//!   checklist -> no card (`planCardVisible`, `:101-106`);
//! * presentation: per-status counts (`planProgress`, `:45-54`), the headline
//!   (server title -> the in-progress item -> the card's own label,
//!   `planHeadline`, `:61-66`), and a quiet relative "Updated …" label
//!   (`planUpdatedLabel`, `:81-99`).
//!
//! The card rides the sticky composer (`App.tsx:2653-2667`), hidden while an
//! approval or a question takes the composer over.
use octoscode_store::domains::task::{Plan, PlanItem};
use octoscode_store::Store;

use crate::screens::board3::ui::{tok, Dsl, Face, Txt, W};

/// The `plan.todos.v1` gate (`supportsFeature(capabilities,
/// CORE_UI_FEATURES.PLAN_TODOS_V1)`, `use-supervision.ts:68-71`).
pub fn available(store: &Store) -> bool {
    store.domains.config.has_capability("plan.todos.v1")
}

/// Per-status counts (`planProgress`, `plan.ts:45-54`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    pub total: usize,
    pub completed: usize,
    pub in_progress: usize,
    pub pending: usize,
}

pub fn progress(items: &[PlanItem]) -> Progress {
    Progress {
        total: items.len(),
        completed: items.iter().filter(|i| i.status == "completed").count(),
        in_progress: items.iter().filter(|i| i.status == "in_progress").count(),
        pending: items.iter().filter(|i| i.status == "pending").count(),
    }
}

/// `planHeadline` (`plan.ts:61-66`): the server's title, else the
/// in-progress item; both are model prose. `None` = the card's own label.
pub fn headline(plan: &Plan) -> Option<String> {
    if let Some(t) = plan.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        return Some(t.to_owned());
    }
    plan.items
        .iter()
        .find(|i| i.status == "in_progress")
        .map(|i| i.title.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// `planStatusLabel` (`plan.ts:69-78`).
pub fn status_label(status: &str) -> &'static str {
    match status {
        "completed" => "Done",
        "in_progress" => "In progress",
        _ => "Pending",
    }
}

/// `planUpdatedLabel` (`plan.ts:81-99`): sub-minute reads "just now" so a
/// fast-moving plan never flickers a new number.
pub fn updated_label(updated_at_ms: i64, now_ms: i64) -> String {
    let elapsed = now_ms - updated_at_ms;
    if elapsed < 60_000 {
        return "Updated just now".into();
    }
    let minutes = elapsed / 60_000;
    if minutes < 60 {
        return format!("Updated {minutes}m ago");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("Updated {hours}h ago");
    }
    format!("Updated {}d ago", hours / 24)
}

/// `planCardVisible` (`plan.ts:101-106`) for the active session.
pub fn visible_plan(store: &Store) -> Option<Plan> {
    if !available(store) {
        return None;
    }
    let session = store.active_session()?;
    store.domains.task.plan(&session).filter(|p| !p.items.is_empty())
}

/// The card's UI state (the web's `useState(false)` collapse).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanUi {
    pub collapsed: bool,
}

/// The step glyph (Gate-B conversation-10's marks).
pub fn mark(status: &str) -> &'static str {
    match status {
        "completed" => "cv_step_done.svg",
        "in_progress" => "cv_step_active.svg",
        _ => "cv_step_pending.svg",
    }
}

/// The header line: `Plan · <headline>` (the Gate-B "Plan · …" header with
/// the web's headline rule), or just `Plan`; fitted with an ellipsis.
pub fn header_text(plan: &Plan, budget_px: f64) -> String {
    let full = match headline(plan) {
        Some(h) => format!("Plan · {h}"),
        None => "Plan".to_owned(),
    };
    crate::screens::board3::ui::fit_w(&full, budget_px, 13.5, Face::Semibold)
}

/// Lower the plan card. `width` is the card's outer width; `phone` tightens
/// the spacing.
pub fn card(d: &mut Dsl, plan: &Plan, ui: &PlanUi, width: f64, phone: bool, now_ms: i64) {
    let prog = progress(&plan.items);
    d.surface(
        "cv_pl_card",
        &format!(
            "width: Fill height: Fit flow: Down padding: Inset{{left: {h} right: {h} top: 8 bottom: {b}}}",
            h = if phone { 12.0 } else { 14.0 },
            b = if ui.collapsed { 8.0 } else { 10.0 },
        ),
        tok::SURFACE,
        14.0,
        Some(tok::HAIRLINE),
    );
    // The header IS the toggle (`PlanCard.tsx:43-58`): chevron, the
    // headline, `{done} of {total} done`.
    d.view("cv_pl_head", "width: Fill height: 34 flow: Overlay");
    let row = d.anon();
    d.view(&row, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 2 right: 4}");
    d.icon(
        "cv_pl_chev",
        if ui.collapsed { "b3_chevron_right_dark.svg" } else { "b3_chevron_down_dark.svg" },
        13.0,
        tok::TEXT,
    );
    let summary = format!("{} of {} done", prog.completed, prog.total);
    let budget = (width - 2.0 * if phone { 12.0 } else { 14.0 } - 40.0 - crate::screens::board3::ui::text_w(&summary, 12.0, Face::Regular)).max(80.0);
    d.text("cv_pl_title", &header_text(plan, budget), &Txt::new(13.5, Face::Semibold, tok::TEXT).w(W::Fill));
    d.text("cv_pl_summary", &summary, &Txt::new(12.0, Face::Regular, tok::MUTED));
    d.close();
    d.tap("cv_pl_toggle", "cv.plan.toggle");
    d.close();
    if !ui.collapsed {
        let list = d.anon();
        d.view(&list, "width: Fill height: Fit flow: Down spacing: 2 padding: Inset{top: 2}");
        for (i, item) in plan.items.iter().enumerate() {
            let row = d.anon();
            d.view(
                &row,
                "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{left: 2 right: 4 top: 4 bottom: 4}",
            );
            d.icon(&format!("cv_pl_mark_{i}"), mark(&item.status), 20.0, tok::BLUE);
            let (face, color) = match item.status.as_str() {
                "in_progress" => (Face::Medium, tok::TEXT),
                "completed" => (Face::Regular, tok::TEXT),
                _ => (Face::Regular, tok::MUTED),
            };
            d.text(&format!("cv_pl_item_{i}"), item.title.trim(), &Txt::new(13.5, face, color).w(W::Fill).wrap());
            if let Some(p) = item.priority.as_deref().filter(|p| !p.trim().is_empty()) {
                d.chip(&format!("cv_pl_prio_{i}"), p.trim(), tok::MUTED, tok::SURFACE, Some(tok::HAIRLINE), false);
            }
            d.text(&format!("cv_pl_status_{i}"), status_label(&item.status), &Txt::new(11.5, Face::Regular, tok::FAINT));
            d.close();
        }
        d.close();
        d.gap(W::Fill, 2.0);
        d.text(
            "cv_pl_updated",
            &updated_label(plan.updated_at_ms, now_ms),
            &Txt::new(11.5, Face::Regular, tok::FAINT).w(W::Fill),
        );
    }
    d.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, status: &str) -> PlanItem {
        PlanItem { id: id.into(), title: format!("Step {id}"), status: status.into(), priority: None }
    }

    #[test]
    fn counts_headline_and_labels_are_the_webs() {
        let plan = Plan {
            items: vec![item("1", "completed"), item("2", "in_progress"), item("3", "pending")],
            title: None,
            updated_at_ms: 0,
            turn_id: Some("t".into()),
        };
        assert_eq!(progress(&plan.items), Progress { total: 3, completed: 1, in_progress: 1, pending: 1 });
        assert_eq!(headline(&plan).as_deref(), Some("Step 2"), "the in-progress item names it");
        let titled = Plan { title: Some("  Building memory panel… ".into()), ..plan.clone() };
        assert_eq!(headline(&titled).as_deref(), Some("Building memory panel…"), "the server's title wins");
        let none = Plan { items: vec![item("1", "pending")], ..plan.clone() };
        assert_eq!(headline(&none), None, "the card's own label then");
        // plan.test.ts:108 — relative labels, sub-minute is "just now".
        assert_eq!(updated_label(0, 59_999), "Updated just now");
        assert_eq!(updated_label(0, 5 * 60_000), "Updated 5m ago");
        assert_eq!(updated_label(0, 3 * 3_600_000), "Updated 3h ago");
        assert_eq!(updated_label(0, 2 * 86_400_000), "Updated 2d ago");
        assert_eq!(status_label("in_progress"), "In progress");
    }

    #[test]
    fn no_feature_or_an_empty_checklist_means_no_card() {
        let store = Store::new();
        store.set_active(Some("s".into()));
        store.domains.task.set_plan(
            "s",
            Plan { items: vec![item("1", "pending")], title: None, updated_at_ms: 0, turn_id: None },
        );
        assert!(visible_plan(&store).is_none(), "plan.todos.v1 not advertised: fail closed");
        store.set_capabilities(vec!["plan.todos.v1".into()]);
        assert!(visible_plan(&store).is_some());
        store.domains.task.set_plan("s", Plan { items: vec![], title: None, updated_at_ms: 0, turn_id: None });
        assert!(visible_plan(&store).is_none(), "an empty checklist renders no card");
    }

    #[test]
    fn the_card_lowers_balanced_with_its_toggle() {
        let plan = Plan { items: vec![item("1", "completed"), item("2", "pending")], title: None, updated_at_ms: 0, turn_id: None };
        let mut d = Dsl::new();
        card(&mut d, &plan, &PlanUi::default(), 600.0, false, 0);
        assert_eq!(header_text(&plan, 400.0), "Plan", "no headline: the card's own label, once");
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        assert!(dsl.contains("1 of 2 done"));
        assert!(crate::screens::taps::wired_taps(&dsl).iter().any(|(_, e)| e == "cv.plan.toggle"));
    }
}
