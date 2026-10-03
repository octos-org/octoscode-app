//! A14 — the A5 dialog family drawn with the board-3 kit.
//!
//! The family (Models, Context, Skills, Goal, Loops, Monitors, the per-session
//! Fleet slice, Tasks, Code review, and their confirm / create cards) used to
//! lower the Stage-B PHONE artboards (406 px wide, absolute positions) and
//! centre them in the desktop window: a ~410 px card with a 14 px radius,
//! 20 px titles and 40-55 px pills inside the 990 px OctosCode window — phone
//! UI pasted into the desktop app (the operator's verdict, the integrator's
//! judge pass). The board-3 dialogs (Undo, Fork, Thinking, Agents, Research,
//! Resume, …) read right. This module draws the family with THEIR kit
//! (`board3::ui`): the same backdrop and frame (white, 16 px radius, 1 px
//! hairline, 20 px padding — 16 on a phone), the same header (17 px semibold
//! title, the mono scope line, the 28 px close glyph), the same type ramp
//! (13 px body, 12 px meta), 32 px pills and 18 px row glyphs in 32 px hit
//! boxes; each dialog is `min(<max>, avail - 32)` wide like every board-3
//! dialog (328 px on a 360 px phone, centred over the dimmed backdrop).
//!
//! What each dialog SAYS and DOES is unchanged: the copy, the empty states,
//! the advertised-method gates, the per-row `#<row>` events and the confirm /
//! create flows are A5's and A10's (`screens::dialog`), and every widget id
//! the walks and tests address keeps its `dlg_<dialog>_<node>` name (the
//! chrome keeps `dialog_frame`, `dialog_scroll` and `dialog_close`).
use serde_json::Value;

use crate::bindings::Ctx;
use crate::screens::autonomy::{self as au, AutonomyState};
use crate::screens::board3::fleetview::Status;
use crate::screens::board3::ui::{self, tok, Btn, Dsl, Face, Frame, ShellIds, Txt, W};
use crate::screens::dialog::{self as dlg, Confirm, Dialog, Form};
use crate::i18n::{tr, tr1, tr_with};

/// The family's chrome ids: the walks address `dialog_frame`,
/// `dialog_scroll` and `dialog_close`; the judge tour closes overlays by
/// `dialog_close`. The backdrop takes a press and routes nothing (A5's
/// modal: Escape or the close control only).
pub const IDS: ShellIds = ShellIds {
    root: "dialog_root",
    backdrop: "dialog_scrim",
    backdrop_box: "dialog_backdrop_box",
    backdrop_hit: "dialog_backdrop",
    backdrop_event: None,
    dialog: "dialog_frame",
    scroll: "dialog_scroll",
    close: "dialog_close",
};

/// A row action's glyph (design px): ONE size for every loop / monitor row
/// icon, so one stroke weight (the module's 24-unit line icons).
pub const ROW_ICON: f64 = 18.0;
/// The glyph's square hit box (>= 28 px, the brief's minimum).
pub const ROW_HIT: f64 = 32.0;
/// The spacing between a row's hit boxes (pitch = [`ROW_HIT`] + this).
pub const ROW_GAP: f64 = 4.0;
/// The family's pill height (the board-3 dialogs' 30-34 px controls).
pub const PILL_H: f64 = 32.0;

/// Each dialog's max width (design px), the web's `width: min(<max>px, 100%)`
/// (`SkillsDialog.module.css` 760, `NativeReviewDialog` / `AutonomyDialog`
/// 640) brought to the board-3 kit's own range (480-800).
pub fn max_width(d: Dialog) -> f64 {
    match d {
        Dialog::Skills => 720.0,
        Dialog::Tasks | Dialog::Review => 640.0,
        Dialog::Models | Dialog::Loops | Dialog::Monitors | Dialog::Fleet => 600.0,
        Dialog::Goal | Dialog::Context => 560.0,
    }
}

/// The confirm card's max width (a short title, a detail line, two pills).
pub const CONFIRM_W: f64 = 480.0;
/// The create form's max width.
pub const FORM_W: f64 = 520.0;

/// One lowered dialog: its DSL, its laid-out width and the frame's max height.
pub struct Built {
    pub dsl: String,
    pub width: f64,
    pub max_h: f64,
}

/// Draw dialog `d` (or its pending confirm / create card) for `frame`.
pub fn build(
    d: Dialog,
    ctx: &Ctx<'_>,
    st: &AutonomyState,
    frame: Frame,
    confirm: Option<&Confirm>,
    form: Option<&Form>,
    notice: Option<(String, bool)>,
) -> Built {
    let max = match (confirm, form) {
        (Some(_), _) => CONFIRM_W,
        (None, Some(_)) => FORM_W,
        _ => max_width(d),
    };
    let width = frame.dialog_w(max);
    let mut dsl = Dsl::new();
    {
        let mut b = B::new(&mut dsl, d, frame, width);
        let notice = notice.as_ref();
        match (confirm, form) {
            (Some(c), _) => confirm_card(&mut b, ctx, c),
            (None, Some(f)) => form_card(&mut b, ctx, f),
            _ => match d {
                Dialog::Models => models(&mut b, ctx, notice),
                Dialog::Context => context(&mut b, ctx, notice),
                Dialog::Skills => skills(&mut b, ctx, notice),
                Dialog::Goal => goal(&mut b, ctx, st, notice),
                Dialog::Loops => loops(&mut b, ctx, st, notice),
                Dialog::Monitors => monitors(&mut b, ctx, st, notice),
                Dialog::Fleet => fleet(&mut b, ctx, notice),
                Dialog::Tasks => tasks(&mut b, ctx, notice),
                Dialog::Review => review(&mut b, ctx, notice),
            },
        }
    }
    Built { dsl: dsl.finish(), width, max_h: frame.dialog_max_h() }
}

// ------------------------------------------------------------- the builder

/// The kit DSL builder plus the dialog's geometry; every local id becomes
/// `dlg_<dialog>_<local>` (an empty local id stays anonymous).
struct B<'a> {
    d: &'a mut Dsl,
    dlg: Dialog,
    frame: Frame,
    width: f64,
    pad: f64,
    /// The scroll body's content width (the card padding and the scroll
    /// bar's gutter off).
    inner: f64,
    /// Phone-narrow (stacked actions, Install under its row).
    compact: bool,
}

impl<'a> B<'a> {
    fn new(d: &'a mut Dsl, dlg: Dialog, frame: Frame, width: f64) -> Self {
        let pad = ui::dialog_pad(&frame, width);
        Self { d, dlg, frame, width, pad, inner: width - 2.0 * pad - 10.0, compact: frame.compact(width) }
    }

    fn id(&self, local: &str) -> String {
        if local.is_empty() {
            String::new()
        } else {
            format!("dlg_{}_{local}", self.dlg.id())
        }
    }

    /// A list card's content width (its 14 px side padding off).
    fn card_w(&self) -> f64 {
        self.inner - 28.0
    }

    fn text(&mut self, local: &str, s: &str, t: &Txt) {
        let id = self.id(local);
        self.d.text(&id, s, t);
    }

    fn view(&mut self, local: &str, props: &str) {
        let id = self.id(local);
        self.d.view(&id, props);
    }

    fn close(&mut self) {
        self.d.close();
    }

    fn gap(&mut self, h: f64) {
        self.d.gap(W::Fill, h);
    }

    /// A bordered section card (the kit's `card_open`: radius 12, 14 / 12
    /// padding).
    fn card(&mut self, local: &str, spacing: f64) {
        let id = self.id(local);
        ui::card_open(self.d, &id, spacing);
    }

    /// A list card: rows separated by hairlines, each row's own 10 px band
    /// setting the rhythm.
    fn list_card(&mut self, local: &str) {
        let id = self.id(local);
        self.d.surface(
            &id,
            "width: Fill height: Fit flow: Down padding: Inset{left: 14 right: 14 top: 2 bottom: 2}",
            tok::SURFACE,
            12.0,
            Some(tok::HAIRLINE),
        );
    }

    /// One list row: a centred Right flow with a 10 px band above and below.
    fn row(&mut self, local: &str) {
        self.view(
            local,
            "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{top: 10 bottom: 10}",
        );
    }

    /// A pill under A5's ids: `<base>_surface`, `<base>_label`, `<base>_control`.
    fn pill(&mut self, base: &str, label: &str, event: &str, kind: Btn, width: W) {
        let base = self.id(base);
        self.d.button_ids(
            &format!("{base}_surface"),
            &format!("{base}_label"),
            &format!("{base}_control"),
            label,
            event,
            kind,
            width,
            PILL_H,
        );
    }

    /// A text link under A5's ids: `<base>_box`, `<base>_label`, `<base>_control`.
    fn link(&mut self, base: &str, label: &str, event: Option<&str>, color: &'static str) {
        let base = self.id(base);
        self.d.link_ids(
            &format!("{base}_box"),
            &format!("{base}_label"),
            &format!("{base}_control"),
            label,
            event,
            13.0,
            color,
        );
    }

    /// A row glyph (`<base>`) in its hit box (`<base>_box`, tap `<base>_hit`).
    fn icon_hit(&mut self, base: &str, file: &str, event: Option<&str>) {
        let base = self.id(base);
        self.d.icon_hit(&base, file, ROW_ICON, ROW_HIT, event);
    }

    /// An empty slot the size of a row glyph's hit box (keeps the columns
    /// when a row has no control there).
    fn icon_slot(&mut self) {
        self.d.gap(W::Px(ROW_HIT), ROW_HIT);
    }

    fn chip(&mut self, local: &str, word: &str, ink: (&'static str, &'static str)) {
        let id = self.id(local);
        self.d.chip(&id, word, ink.0, ink.1, None, false);
    }

    /// A status light (8 px).
    fn dot(&mut self, local: &str, color: &str) {
        let id = self.id(local);
        self.d.surface(&id, "width: 8 height: 8", color, 4.0, None);
        self.d.close();
    }

    /// A one-line field (36 px, the board-3 inputs).
    fn input(&mut self, local: &str, text: &str, placeholder: &str) {
        let id = self.id(local);
        self.d.input(&id, &id, text, placeholder, false, 36.0);
    }

    /// A progress bar: a grey track and its dark fill (`fill` 0..1).
    fn bar(&mut self, local: &str, fill: Option<f64>, track_w: f64) {
        let track = self.id(&format!("{local}_track"));
        self.d.surface(&track, "width: Fill height: 6 flow: Right align: Align{x: 0.0 y: 0.5}", tok::CHIP, 3.0, None);
        if let Some(f) = fill.filter(|f| *f > 0.0) {
            let id = self.id(&format!("{local}_fill"));
            let w = (track_w * f.clamp(0.0, 1.0)).max(6.0).floor();
            self.d.surface(&id, &format!("width: {w} height: 6"), tok::TEXT, 3.0, None);
            self.d.close();
        }
        self.d.close();
    }
}

/// A pill's laid-out width (the kit's `W::Fit` estimate).
fn pill_w(label: &str) -> f64 {
    ui::text_w(label, 13.0, Face::Medium) + 32.0
}

/// Right-aligned action pills; when a phone-narrow frame cannot hold them in
/// one row, full-width pills stacked, the last (primary) first.
fn actions(b: &mut B<'_>, items: &[(&str, &str, String, Btn)], avail: f64) {
    let need: f64 =
        items.iter().map(|(_, l, _, _)| pill_w(tr(l))).sum::<f64>() + 8.0 * items.len().saturating_sub(1) as f64;
    let row = b.d.anon();
    if need <= avail {
        b.d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5} spacing: 8");
        for (base, label, ev, kind) in items {
            b.pill(base, tr(label), ev, *kind, W::Fit);
        }
    } else {
        b.d.view(&row, "width: Fill height: Fit flow: Down spacing: 8");
        for (base, label, ev, kind) in items.iter().rev() {
            b.pill(base, tr(label), ev, *kind, W::Fill);
        }
    }
    b.close();
}

/// The dialog's scope line: the Profile for its Profile-wide dialogs (the
/// web's "Server Profile:" `.scope`), else the Session (the web's
/// `.scope` = `sessionId`).
fn scope_line(d: Dialog, ctx: &Ctx<'_>) -> String {
    match d {
        Dialog::Models | Dialog::Skills => ctx
            .store
            .domains
            .profile
            .current()
            // The web's `t("Server Profile:") + " "` + the id (SkillsDialog.tsx:155).
            .map(|p| format!("{} {p}", crate::i18n::tr("Server Profile:")))
            .unwrap_or_default(),
        _ => ctx.store.active_session().unwrap_or_default(),
    }
}

/// Open the frame, the header (title + the close glyph), the scope line and
/// the notice, then the scroll body.
fn open(b: &mut B<'_>, title_local: &str, title: &str, scope: &str, notice: Option<&(String, bool)>) {
    ui::shell_open_ids(b.d, &b.frame, b.width, &IDS);
    let row = b.d.anon();
    b.d.view(&row, "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    let room = b.width - 2.0 * b.pad - 36.0;
    b.text(title_local, &ui::fit_w(tr(title), room, 17.0, Face::Semibold), &ui::title().w(W::Fill));
    ui::close_glyph_id(b.d, IDS.close, dlg::ACTION_CLOSE);
    b.close();
    // The header's height as the body's cap sees it (the row, the scope
    // line, the notice, the gap under them).
    let mut chrome = 32.0 + 12.0;
    if !scope.is_empty() {
        let s = ui::fit_w(scope, b.width - 2.0 * b.pad, 11.5, Face::Mono);
        b.text("scope", &s, &ui::scope().w(W::Fill));
        chrome += 15.0;
    }
    if let Some((text, alert)) = notice {
        let text = tr(&dlg::display_error(text)).to_owned();
        b.gap(6.0);
        let ink = if *alert { tok::RED_TEXT } else { tok::MUTED };
        b.text("dialog_notice", &text, &Txt::new(12.5, Face::Regular, ink).w(W::Fill).wrap());
        let lines = (ui::text_w(&text, 12.5, Face::Regular) / (b.width - 2.0 * b.pad)).ceil().max(1.0);
        chrome += 6.0 + lines * 16.0;
    }
    b.gap(12.0);
    ui::body_open_id(b.d, &b.frame, b.width, chrome, IDS.scroll);
}

/// Close the scroll body, the card and the backdrop root.
fn finish(b: &mut B<'_>) {
    ui::body_close(b.d);
    ui::shell_close(b.d);
}

/// A paragraph (12.5 px, wrapping).
fn para(color: &'static str) -> Txt {
    Txt::new(12.5, Face::Regular, color).w(W::Fill).wrap()
}

/// A row's primary line (13.5 px semibold).
fn row_title() -> Txt {
    Txt::new(13.5, Face::Semibold, tok::TEXT)
}

/// A row's mono line (commands, argv).
fn row_mono() -> Txt {
    Txt::new(12.5, Face::Mono, tok::TEXT)
}

// ------------------------------------------------------------------ Models

/// The first three provider cards keep A5's slot ids (the walks address the
/// expanded card as `card_deepseek`, whichever provider it shows).
fn provider_ids(gi: usize) -> (String, String, String, String) {
    match gi {
        0 => ("card_deepseek".into(), "t_ds_head".into(), "t_ds_count".into(), "dot_ds".into()),
        1 => ("card_kimi".into(), "t_kimi_head".into(), "t_kimi_count".into(), "dot_kimi".into()),
        2 => ("card_glm".into(), "t_glm_head".into(), "t_glm_count".into(), "dot_glm".into()),
        n => (format!("card_p{n}"), format!("t_p{n}_head"), format!("t_p{n}_count"), format!("dot_p{n}")),
    }
}

/// setup-07: one card per provider (store order), the first expanded with
/// its models (the selected one checked) and its route operations, each
/// gated on its own method (`model-settings.ts:211`); "Manage providers"
/// opens the board-3 Routes dialog.
fn models(b: &mut B<'_>, ctx: &Ctx<'_>, notice: Option<&(String, bool)>) {
    use crate::screens::models as m;
    open(b, "t_title", "Models", &scope_line(b.dlg, ctx), notice);
    let all = ctx.store.domains.profile.llm_models();
    let mut groups: Vec<String> = Vec::new();
    for x in &all {
        if !groups.contains(&x.provider) {
            groups.push(x.provider.clone());
        }
    }
    if groups.is_empty() {
        b.text("t_ds_count", tr("No models are configured for this Profile."), &para(tok::MUTED));
        // A23 — an empty Profile still reaches its providers (the web's
        // empty state carries "Add provider", ModelManagementSection.tsx:1270).
        if dlg::advertises(ctx.store, "profile/llm/list") {
            b.gap(8.0);
            let avail = b.inner;
            actions(b, &[("manage_providers", "Manage providers", "b3.open.routes".into(), Btn::Outline)], avail);
        }
        finish(b);
        return;
    }
    let col = b.d.anon();
    b.d.view(&col, "width: Fill height: Fit flow: Down spacing: 10");
    let head_w = b.card_w() - 16.0;
    for (gi, p) in groups.iter().enumerate() {
        let mine: Vec<&octoscode_store::domains::profile::ProfileLlmModel> =
            all.iter().filter(|x| &x.provider == p).collect();
        let (card, head, count, dot) = provider_ids(gi);
        let card_id = b.id(&card);
        // A uniform 14 px inset: the route pills keep it under them too.
        b.d.surface(
            &card_id,
            "width: Fill height: Fit flow: Down spacing: 6 padding: Inset{left: 14 right: 14 top: 12 bottom: 14}",
            tok::SURFACE,
            12.0,
            Some(tok::HAIRLINE),
        );
        let hr = b.d.anon();
        b.d.view(&hr, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        let h = m::provider_head(mine[0]);
        b.text(&head, &ui::fit_w(&h, head_w, 13.5, Face::Semibold), &row_title().w(W::Fill));
        b.dot(&dot, tok::GREEN);
        b.close();
        b.text(&count, &m::provider_count(&mine), &ui::meta());
        if gi == 0 {
            b.gap(2.0);
            let inner = b.id("inner_card");
            b.d.surface(
                &inner,
                "width: Fill height: Fit flow: Down padding: Inset{left: 12 right: 8 top: 0 bottom: 0}",
                tok::SURFACE2,
                10.0,
                Some(tok::HAIRLINE),
            );
            for (k, x) in mine.iter().enumerate() {
                if k > 0 {
                    b.d.hairline();
                }
                let id = match k {
                    0 => "t_flash".to_owned(),
                    1 => "t_pro".to_owned(),
                    n => format!("t_m{n}"),
                };
                let r = b.d.anon();
                b.d.view(&r, "width: Fill height: 36 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
                b.text(&id, &ui::fit_w(&m::model_row(x), b.card_w() - 60.0, 13.0, Face::Regular), &ui::body().w(W::Fill));
                if x.selected {
                    let ck = b.id("icon_check");
                    b.d.icon(&ck, "b3_check.svg", 16.0, tok::TEXT);
                }
                b.close();
            }
            b.close();
            let test = dlg::advertises(ctx.store, "profile/llm/test");
            let discover = dlg::advertises(ctx.store, "profile/llm/fetch_models");
            if test || discover {
                b.gap(4.0);
                let pr = b.d.anon();
                b.d.view(&pr, "width: Fill height: Fit flow: Right spacing: 8");
                if test {
                    b.pill("btn_test", tr("Test route"), "models.test_route", Btn::Outline, W::Fill);
                }
                if discover {
                    b.pill("btn_discover", tr("Discover models"), "models.discover", Btn::Outline, W::Fill);
                }
                b.close();
            }
        }
        b.close();
    }
    if dlg::advertises(ctx.store, "profile/llm/list") {
        b.gap(2.0);
        let avail = b.inner;
        actions(b, &[("manage_providers", "Manage providers", "b3.open.routes".into(), Btn::Outline)], avail);
    }
    b.close();
    finish(b);
}

// ----------------------------------------------------------------- Context

/// setup-09: the web's ContextPanel facts (the occupancy line + bar, items,
/// generation, recovery, the compaction line), "Compact now" (asks first)
/// and the server-confirmed compaction mode — each control only when its
/// method is advertised (`ContextPanel.tsx` compactAvailable /
/// modeAvailable).
fn context(b: &mut B<'_>, ctx: &Ctx<'_>, notice: Option<&(String, bool)>) {
    use crate::screens::models as m;
    open(b, "t_title", "Context", &scope_line(b.dlg, ctx), notice);
    let q = |id: &str| m::query_binding(ctx, id).and_then(|v| v.as_str().map(str::to_owned));
    let usage = q("context.usage").unwrap_or_else(|| "—".into());
    let pct = q("context.pct").unwrap_or_else(|| "—".into());
    let ur = b.d.anon();
    b.d.view(&ur, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    b.text("t_usage", &usage, &ui::body_medium().w(W::Fill));
    b.text("t_pct", &pct, &Txt::new(13.0, Face::Semibold, tok::TEXT));
    b.close();
    b.gap(8.0);
    let fill = pct.trim_end_matches('%').parse::<f64>().ok().map(|p| p / 100.0);
    let track_w = b.inner;
    b.bar("bar", fill, track_w);
    b.gap(14.0);
    let session = ctx.store.active_session().unwrap_or_default();
    let life = ctx.store.domains.session.context(&session);
    let state = life.as_ref().map(|l| l.state.clone()).unwrap_or(Value::Null);
    let item = |k: &str| state.get(k).cloned().unwrap_or(Value::Null);
    let rows: [(&str, &str, &str, String); 3] = [
        ("t_row3", "t_val6", "Items", item("item_count").as_u64().map(fmt_count).unwrap_or_else(|| "—".into())),
        (
            "t_row4",
            "t_val7",
            "Generation",
            item("generation").as_u64().map(|g| g.to_string()).unwrap_or_else(|| "—".into()),
        ),
        (
            "t_row5",
            "t_val8",
            "Recovery",
            item("recovery_state").as_str().map(str::to_owned).unwrap_or_else(|| "—".into()),
        ),
    ];
    b.list_card("facts_card");
    for (i, (lid, vid, label, value)) in rows.iter().enumerate() {
        if i > 0 {
            b.d.hairline();
        }
        let r = b.d.anon();
        b.d.view(&r, "width: Fill height: 34 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        b.text(lid, tr(label), &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill));
        b.text(vid, value, &Txt::new(12.5, Face::Medium, tok::TEXT));
        b.close();
    }
    b.close();
    // The compaction line (the client's lifecycle kinds: a running pass, or
    // the last one from the authoritative status read's detail).
    let detail = life.as_ref().and_then(|l| l.detail.clone()).unwrap_or(Value::Null);
    let kind = life.as_ref().map(|l| l.kind.trim_start_matches("context/").to_owned());
    let line = match kind.as_deref() {
        Some("compaction_started") => Some(tr1(
            "Compacting context · {value0}",
            detail.get("trigger").and_then(|t| t.as_str()).unwrap_or("manual"),
        )),
        _ if detail.get("token_estimate_before").is_some() || detail.get("compaction").is_some() => {
            let c = detail.get("compaction").cloned().unwrap_or(detail.clone());
            let before = c.get("token_estimate_before").and_then(|v| v.as_u64());
            let after = c.get("token_estimate_after").and_then(|v| v.as_u64());
            let status = c.get("status").and_then(|v| v.as_str()).unwrap_or("completed");
            Some(tr_with(
                "Last compaction: {value0} · {value1} → {value2} tokens",
                &[
                    ("value0", status),
                    ("value1", &before.map(fmt_count).unwrap_or_else(|| "—".into())),
                    ("value2", &after.map(fmt_count).unwrap_or_else(|| tr("not reported").into())),
                ],
            ))
        }
        _ => None,
    };
    if let Some(l) = line {
        b.gap(10.0);
        b.text("t_keep", &l, &para(tok::MUTED));
    }
    let can_compact = dlg::advertises(ctx.store, "session/compact");
    let can_mode = dlg::advertises(ctx.store, "session/compact/mode/set");
    if can_compact || can_mode {
        b.gap(14.0);
        let r = b.d.anon();
        let flow = if b.compact { "flow: Down spacing: 10" } else { "flow: Right align: Align{x: 0.0 y: 0.5} spacing: 12" };
        b.d.view(&r, &format!("width: Fill height: Fit {flow}"));
        if can_mode {
            let mr = b.d.anon();
            b.d.view(&mr, "width: Fit height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
            b.text("t_comp", tr("Compaction:"), &Txt::new(12.5, Face::Regular, tok::MUTED));
            let mode = crate::screens::models::compact_mode(&session);
            segmented(
                b,
                "seg_box",
                &[
                    ("seg_llm", "t_llm", "seg_llm_hit", "LLM", "context.mode.llm".into(), mode.as_deref() == Some("llm")),
                    (
                        "seg_heur",
                        "t_heur",
                        "seg_heur_hit",
                        tr("Heuristic"),
                        "context.mode.heuristic".into(),
                        mode.as_deref() == Some("heuristic"),
                    ),
                ],
                W::Px(200.0),
            );
            b.close();
        }
        if !b.compact {
            b.d.gap(W::Fill, 1.0);
        }
        if can_compact {
            b.pill(
                "btn_compact",
                tr("Compact now"),
                &format!("{}context.compact_now", dlg::ACTION_ASK),
                Btn::Outline,
                W::Fit,
            );
        }
        b.close();
    }
    finish(b);
}

/// `toLocaleString()` grouping (en): 1,234,567.
fn fmt_count(v: u64) -> String {
    let s = v.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// A segmented control (the kit's `Seg::Tab` look: a raised white segment on
/// the grey track) under explicit ids: `(segment, label, tap, text, event,
/// selected)`. Every segment routes (the selected one re-asserts its mode).
#[allow(clippy::type_complexity)]
fn segmented(b: &mut B<'_>, track: &str, segs: &[(&str, &str, &str, &str, String, bool)], width: W) {
    let track = b.id(track);
    let w = match width {
        W::Px(v) => format!("{}", v.floor()),
        W::Fill => "Fill".into(),
        W::Fit => "Fit".into(),
    };
    b.d.surface(
        &track,
        &format!(
            "width: {w} height: 34 flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 2 padding: Inset{{left: 3 right: 3 top: 3 bottom: 3}}"
        ),
        tok::SURFACE2,
        9.0,
        Some(tok::HAIRLINE),
    );
    for (seg, label, tap, text, event, on) in segs {
        let (fill, border, face, ink) = if *on {
            (tok::SURFACE, Some(tok::HAIRLINE), Face::Medium, tok::TEXT)
        } else {
            (tok::TRANSPARENT, None, Face::Regular, tok::MUTED)
        };
        let sid = b.id(seg);
        b.d.surface(&sid, "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5}", fill, 7.0, border);
        let inner = b.d.anon();
        b.d.view(&inner, "width: Fill height: Fill flow: Right align: Align{x: 0.5 y: 0.5}");
        b.text(label, text, &Txt::new(13.0, face, ink));
        b.close();
        let tid = b.id(tap);
        b.d.tap(&tid, event);
        b.close();
    }
    b.close();
}

// ------------------------------------------------------------------ Skills

/// setup-10 + the web's SkillsDialog: the warning, the Profile lock, the
/// registry search (only when advertised; Enter searches), the installed
/// rows (version · tools · source, Remove asks first), the searched
/// packages with the web's fields and Install (asks first), and "Install
/// from source". While the Profile is busy every mutation is drawn paused
/// and routes nothing (`disabled={busy || profileBusy}`).
fn skills(b: &mut B<'_>, ctx: &Ctx<'_>, notice: Option<&(String, bool)>) {
    // A31 — the dialog's own copy through `tr` (the web's keys, or the
    // aliases of the same controls), so a Chinese dialog reads Chinese
    // around its Background jobs section.
    use crate::i18n::tr;
    let store = ctx.store;
    open(b, "t_title", tr("Skills"), &scope_line(b.dlg, ctx), notice);
    let locked = dlg::profile_locked(ctx);
    b.text("skills_warning", tr(dlg::SKILLS_WARNING), &para(tok::MUTED));
    if locked {
        b.gap(6.0);
        b.text("skills_locked", tr(dlg::SKILLS_LOCKED), &Txt::new(12.5, Face::Medium, tok::AMBER).w(W::Fill).wrap());
    }
    // A31 — the Background jobs section at the top (parity row 15).
    skill_jobs_section(b, ctx);
    if dlg::advertises(store, "profile/skills/registry/search") {
        b.gap(12.0);
        let id = b.id("skills_query");
        let q = dlg::skills_query().unwrap_or_default();
        b.d.input_icon(&id, &id, &q, tr("Search registry"), "b3_search.svg", 36.0);
    }
    // ---- Installed.
    b.gap(16.0);
    b.text("t_inst_head", tr("Installed"), &ui::heading().w(W::Fill));
    b.gap(8.0);
    let installed = store.domains.profile.installed_skills();
    b.list_card("card_installed");
    if installed.is_empty() {
        let r = b.d.anon();
        b.d.view(&r, "width: Fill height: Fit padding: Inset{top: 12 bottom: 12}");
        b.text("t_name3", tr("No skills installed in this Profile."), &para(tok::MUTED));
        b.close();
    }
    let line_w = b.card_w() - 80.0;
    for (i, s) in installed.iter().enumerate() {
        if i > 0 {
            b.d.hairline();
        }
        b.row(&format!("row_{i}"));
        let c = b.d.anon();
        b.d.view(&c, "width: Fill height: Fit flow: Down spacing: 3");
        b.text(&format!("t_name{}", 3 + i), &ui::fit_w(&s.name, line_w, 13.5, Face::Semibold), &row_title().w(W::Fill));
        // The web's `{n} {t("tools")}` in Chinese; English keeps its singular.
        let tools = match (s.tool_count, crate::i18n::is_zh()) {
            (n, true) => format!("{n} {}", tr("tools")),
            (1, false) => "1 tool".to_owned(),
            (n, false) => format!("{n} tools"),
        };
        let mut line = format!("{} · {tools}", s.version.as_deref().unwrap_or(tr("Version not reported")));
        if let Some(repo) = s.source_repo.as_deref().filter(|r| !r.is_empty()) {
            line = format!("{line} · {repo}");
        }
        b.text(&format!("t_ver{}", 6 + i), &ui::fit_w(&line, line_w, 12.0, Face::Regular), &ui::meta().w(W::Fill));
        b.close();
        // Remove: a red link that asks first; paused (faint, no tap) while
        // the Profile is busy.
        let (label, tap) = (b.id(&format!("t_remove{i}")), b.id(&format!("t_remove{i}_hit")));
        let ev = format!("{}skills.remove_{i}", dlg::ACTION_ASK);
        b.d.link_ids(
            &format!("{label}_box"),
            &label,
            &tap,
            tr("Remove"),
            (!locked).then_some(ev.as_str()),
            12.5,
            if locked { tok::DISABLED_INK } else { tok::RED_TEXT },
        );
        b.close();
    }
    b.close();
    // ---- Registry (the searched packages; "No matching skill packages."
    // after a search that found none).
    let packages = store.domains.profile.registry_packages();
    let install_ok = dlg::advertises(store, "profile/skills/install");
    let searched = dlg::skills_query().is_some();
    if !packages.is_empty() || searched {
        b.gap(18.0);
        b.text("t_reg_head", tr("Registry"), &ui::heading().w(W::Fill));
        b.gap(8.0);
        b.list_card("card_registry");
        if packages.is_empty() {
            let r = b.d.anon();
            b.d.view(&r, "width: Fill height: Fit padding: Inset{top: 12 bottom: 12}");
            b.text("t_name10", tr("No matching skill packages."), &para(tok::MUTED));
            b.close();
        }
        let text_w = b.card_w() - if install_ok && !b.compact { pill_w("Install") + 8.0 } else { 0.0 };
        for (j, p) in packages.iter().enumerate() {
            if j > 0 {
                b.d.hairline();
            }
            let rid = format!("reg_{j}");
            b.view(
                &rid,
                &format!(
                    "width: Fill height: Fit flow: {} align: Align{{x: 0.0 y: 0.0}} spacing: 8 padding: Inset{{top: 10 bottom: 10}}",
                    if b.compact { "Down" } else { "Right" }
                ),
            );
            let c = b.d.anon();
            b.d.view(&c, "width: Fill height: Fit flow: Down spacing: 3");
            b.text(&format!("{rid}_name"), &ui::fit_w(&p.name, text_w, 13.5, Face::Semibold), &row_title().w(W::Fill));
            if !p.description.trim().is_empty() {
                b.text(&format!("{rid}_desc"), &p.description, &para(tok::TEXT));
            }
            if !p.repo.trim().is_empty() {
                b.text(&format!("{rid}_repo"), &ui::fit_w(&p.repo, text_w, 12.0, Face::Mono), &Txt::new(12.0, Face::Mono, tok::MUTED).w(W::Fill));
            }
            let kind = tr(if p.provides_tools { "Provides executable tools" } else { "Instruction skills" });
            let licence = p.license.clone().unwrap_or_else(|| tr("License not reported").to_owned());
            b.text(&format!("{rid}_kind"), &format!("{kind} · {licence}"), &para(tok::MUTED));
            if !p.requires.is_empty() {
                b.text(&format!("{rid}_requires"), &format!("{} {}", tr("Requires:"), p.requires.join(", ")), &para(tok::MUTED));
            }
            if p.installed {
                b.text(
                    &format!("{rid}_installed"),
                    &format!("{} {}", tr("Installed:"), p.installed_skills.join(", ")),
                    &Txt::new(12.5, Face::Regular, tok::GREEN_TEXT).w(W::Fill).wrap(),
                );
            }
            b.close();
            if install_ok {
                let ev = format!("{}skills.install_{}", dlg::ACTION_ASK, j + 3);
                let kind = if locked { Btn::OutlineOff } else { Btn::Outline };
                b.pill(&format!("{rid}_install"), tr("Install"), &ev, kind, W::Fit);
            }
            b.close();
        }
        b.close();
    }
    // ---- Install from source (only when install is advertised).
    if install_ok {
        b.gap(18.0);
        b.text("src_head", tr("Install from source"), &ui::heading().w(W::Fill));
        b.gap(8.0);
        b.card("card_source", 6.0);
        let (repo, branch) = dlg::skills_source();
        for (id, label, value) in [
            ("src_repo", "Repository or server-side path", repo),
            ("src_branch", "Branch (server default: main)", branch),
        ] {
            let lid = b.id(&format!("{id}_label"));
            ui::field_label(b.d, &lid, label);
            b.input(id, &value, "");
            b.gap(4.0);
        }
        let avail = b.card_w();
        let kind = if locked { Btn::OutlineOff } else { Btn::Outline };
        actions(
            b,
            &[("src_review", tr("Review installation"), format!("{}skills.install_source", dlg::ACTION_ASK), kind)],
            avail,
        );
        b.close();
    }
    finish(b);
}

/// A31 — "Background jobs" (board 4 region 3, `screens::skill_jobs`): the
/// heading with the active-job count on its right, the no-feature note, a
/// list failure, then one row per job, newest first — the skill and action
/// (mono), the file with its status chip and its time, and the message
/// line(s) of a finished, failed or abandoned job.
fn skill_jobs_section(b: &mut B<'_>, ctx: &Ctx<'_>) {
    use crate::i18n::{tr, tr1, tr_with};
    use crate::screens::skill_jobs as sj;
    use octoscode_store::domains::skill_jobs::ListState;
    let Some(sec) = sj::section(ctx.store, ui::now_ms()) else {
        return;
    };
    b.gap(16.0);
    let hr = b.d.anon();
    b.d.view(&hr, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    b.text("jobs_head", tr(sj::HEADING), &ui::heading().w(W::Fill));
    let mut count = Vec::new();
    if sec.running > 0 {
        count.push(tr_with(sj::RUNNING_COUNT, &[("count", &sec.running.to_string())]));
    }
    if sec.queued > 0 {
        count.push(tr1(sj::QUEUED_COUNT, &sec.queued.to_string()));
    }
    if !count.is_empty() {
        b.text("jobs_count", &count.join(" · "), &ui::meta());
    }
    b.close();
    if sec.announced_only {
        b.gap(4.0);
        b.text("jobs_note", tr(sj::ANNOUNCED_ONLY), &para(tok::MUTED));
    }
    if let ListState::Failed(e) = &sec.state {
        b.gap(6.0);
        let id = b.id("jobs_error");
        ui::failure(b.d, &id, sj::LOAD_FAILED, &dlg::display_error(e));
    }
    if sec.rows.is_empty() && matches!(sec.state, ListState::Failed(_)) {
        // The failure says it; "no jobs" would be a guess.
        return;
    }
    b.gap(8.0);
    b.list_card("card_jobs");
    if sec.rows.is_empty() {
        let r = b.d.anon();
        b.d.view(&r, "width: Fill height: Fit padding: Inset{top: 12 bottom: 12}");
        let empty = if sec.state == ListState::Loading { sj::LOADING } else { sj::EMPTY };
        b.text("jobs_empty", tr(empty), &para(tok::MUTED));
        b.close();
    }
    let w = b.card_w();
    // A24 — one time column for every row, as wide as its longest time.
    let time_w = sj::time_column_w(&sec.rows);
    for (i, row) in sec.rows.iter().enumerate() {
        if i > 0 {
            b.d.hairline();
        }
        let rid = format!("job_{i}");
        b.view(&rid, "width: Fill height: Fit flow: Down spacing: 4 padding: Inset{top: 10 bottom: 10}");
        // The skill and its action (mono), as the board's first line.
        let l1 = b.d.anon();
        b.d.view(&l1, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
        let skill = ui::fit_w(&row.skill, w * 0.5, 12.0, Face::Mono);
        let action_room = (w - ui::text_w(&skill, 12.0, Face::Mono) - 6.0).max(40.0);
        b.text(&format!("{rid}_skill"), &skill, &Txt::new(12.0, Face::Mono, tok::TEXT));
        b.text(
            &format!("{rid}_action"),
            &ui::fit_w(&format!("· {}", row.action), action_room, 12.0, Face::Mono),
            &Txt::new(12.0, Face::Mono, tok::MUTED),
        );
        b.close();
        // The file, its status chip, the time since its last change.
        let l2 = b.d.anon();
        b.d.view(&l2, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
        let chip = format!("{} {}", row.chip.glyph, tr(row.chip.word));
        let chip_w = ui::text_w(&chip, 11.0, Face::Medium) + 14.0;
        let name_w = (w - chip_w - time_w - 20.0).max(48.0);
        b.text(&format!("{rid}_name"), &ui::fit_w(&row.name, name_w, 13.5, Face::Semibold), &row_title().w(W::Fill));
        b.chip(&format!("{rid}_state"), &chip, (row.chip.fg, row.chip.bg));
        let tb = b.d.anon();
        b.d.view(&tb, &format!("width: {time_w} height: Fit flow: Right align: Align{{x: 1.0 y: 0.5}}"));
        b.text(&format!("{rid}_time"), &row.time, &ui::meta());
        b.close();
        b.close();
        // The message: at most two lines (a long output or error is cut).
        let two = (w * 1.7).max(80.0);
        match &row.message {
            sj::Message::None => {}
            sj::Message::Output(s) => {
                b.text(&format!("{rid}_msg"), &ui::fit_w(s, two, 12.5, Face::Regular), &para(tok::MUTED));
            }
            sj::Message::Failure { lead, cause } => {
                b.text(&format!("{rid}_msg"), tr(lead), &Txt::new(12.5, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
                if !cause.is_empty() {
                    b.text(
                        &format!("{rid}_cause"),
                        &ui::fit_w(cause, two, 12.0, Face::Mono),
                        &Txt::new(12.0, Face::Mono, tok::MUTED).w(W::Fill).wrap(),
                    );
                }
            }
            sj::Message::Note(s) => {
                b.text(&format!("{rid}_msg"), tr(s), &para(tok::MUTED));
            }
        }
        b.close();
    }
    if sec.omitted > 0 {
        b.d.hairline();
        let r = b.d.anon();
        b.d.view(&r, "width: Fill height: Fit padding: Inset{top: 8 bottom: 8}");
        b.text("jobs_omitted", &tr1(sj::OMITTED, &sec.omitted.to_string()), &ui::meta());
        b.close();
    }
    b.close();
}

// -------------------------------------------------------------------- Goal

/// The goal's status word (`describeGoalStatus`) and its chip ink: active
/// green, paused / budget-limited / blocked amber, terminal grey.
pub fn goal_badge(status: &str) -> (String, (&'static str, &'static str)) {
    let (word, ink) = match status {
        "active" => ("Active", (tok::GREEN_TEXT, tok::GREEN_BG)),
        "paused" => ("Paused", (tok::AMBER, tok::AMBER_BG)),
        "budget_limited" => ("Budget limited", (tok::AMBER, tok::AMBER_BG)),
        "blocked" => ("Blocked", (tok::AMBER, tok::AMBER_BG)),
        "complete" => ("Complete", (tok::MUTED, tok::CHIP)),
        // `describeGoalStatus` returns an unknown status verbatim.
        other => (other, (tok::MUTED, tok::CHIP)),
    };
    (tr(word).to_owned(), ink)
}

/// autonomy-03: no goal → the web's "No active goal for this session."
/// (`AutonomyPanel.tsx:176`) and "Set goal"; a goal → its objective and
/// status, the budget ("server default" when 0, `formatGoalBudget`) with its
/// bar, the elapsed time, and Pause↔Resume / Stop while it can transition
/// (`["active","paused","budget_limited","blocked"]`, :129), Clear goal.
fn goal(b: &mut B<'_>, ctx: &Ctx<'_>, st: &AutonomyState, notice: Option<&(String, bool)>) {
    open(b, "t_title", "Goal", &scope_line(b.dlg, ctx), notice);
    let Some(g) = st.goal.as_ref() else {
        b.text("t_goal", tr("No active goal for this session."), &para(tok::MUTED));
        b.gap(14.0);
        let r = b.d.anon();
        b.d.view(&r, "width: Fill height: Fit flow: Right");
        b.pill("pause_btn", tr("Set goal"), &format!("{}goal.set", dlg::ACTION_FORM), Btn::Primary, W::Fit);
        b.close();
        finish(b);
        return;
    };
    let status = g["status"].as_str().unwrap_or("active");
    let (word, ink) = goal_badge(status);
    b.card("goal_card", 10.0);
    let hr = b.d.anon();
    b.d.view(&hr, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.0} spacing: 10");
    b.text("t_goal", g["objective"].as_str().unwrap_or_default(), &Txt::new(14.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap());
    b.chip("goal_badge_label", &word, ink);
    b.close();
    b.d.hairline();
    let used = g["tokens_used"].as_u64().unwrap_or(0);
    let budget = g["token_budget"].as_u64().unwrap_or(0);
    let budget_text = if budget == 0 {
        tr("server default").to_owned()
    } else {
        format!("{} / {}", au::format_tokens(used), au::format_tokens(budget))
    };
    fn fact(b: &mut B<'_>, l: &str, v: &str, label: &str, value: &str) {
        let r = b.d.anon();
        b.d.view(&r, "width: Fill height: 24 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        b.text(l, tr(label), &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill));
        b.text(v, value, &Txt::new(12.5, Face::Medium, tok::TEXT));
        b.close();
    }
    fact(b, "t_budget", "t_budget_val", "Token budget", &budget_text);
    if budget > 0 {
        let track_w = b.card_w();
        b.bar("bar", Some(used as f64 / budget as f64), track_w);
    }
    fact(b, "t_elapsed", "t_elapsed_val", "Elapsed", &au::elapsed_atlas(g["time_used_seconds"].as_u64().unwrap_or(0)));
    let can_transition = matches!(status, "active" | "paused" | "budget_limited" | "blocked");
    b.gap(4.0);
    let ar = b.d.anon();
    b.d.view(&ar, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    if can_transition {
        let (label, ev) = if status == "active" { ("Pause", "goal.pause") } else { ("Resume", "goal.resume") };
        b.pill("pause_btn", tr(label), ev, Btn::Outline, W::Fit);
        b.pill("stop_btn", tr("Stop"), "goal.stop", Btn::Outline, W::Fit);
    }
    b.d.gap(W::Fill, 1.0);
    b.link("clear_goal", tr("Clear goal"), Some("goal.clear"), tok::RED_TEXT);
    b.close();
    b.close();
    finish(b);
}

// ------------------------------------------------------------------- Loops

/// autonomy-04: one row per loop (the prompt, the web's cadence words), the
/// status light, and its controls — Pause only while active, the play glyph
/// resumes a paused loop and fires an active one now, Delete
/// (`AutonomyPanel.tsx:262-300`); "+ New loop" opens the create form.
fn loops(b: &mut B<'_>, ctx: &Ctx<'_>, st: &AutonomyState, notice: Option<&(String, bool)>) {
    open(b, "t_title", "Loops", &scope_line(b.dlg, ctx), notice);
    if st.loops.is_empty() {
        b.text("loops_empty", tr("No loops in this session."), &para(tok::MUTED));
    } else {
        b.list_card("loops_card");
        let text_w = b.card_w() - (16.0 + 3.0 * (ROW_HIT + ROW_GAP)) - 8.0;
        for (i, l) in st.loops.iter().enumerate() {
            if i > 0 {
                b.d.hairline();
            }
            let r = i + 1;
            let paused = l["status"].as_str() == Some("paused");
            b.view(
                &format!("loop_{r}"),
                "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 4 padding: Inset{top: 10 bottom: 10}",
            );
            let c = b.d.anon();
            b.d.view(&c, "width: Fill height: Fit flow: Down spacing: 3");
            // The prompt is the loop's identity (the web shows it whole):
            // it wraps, bounded to two lines (a prompt may run to 8 KB).
            let prompt = l["prompt"].as_str().unwrap_or_default();
            b.text(
                &format!("loop_{r}_name"),
                &ui::fit_w(prompt, 2.0 * text_w, 13.5, Face::Semibold),
                &row_title().w(W::Fill).wrap(),
            );
            b.text(&format!("loop_{r}_cad"), &au::cadence(l), &ui::meta().w(W::Fill));
            b.close();
            b.dot(&format!("loop_{r}_dot"), if paused { "#c7c7ccff" } else { tok::GREEN });
            b.d.gap(W::Px(4.0), 1.0);
            if paused {
                b.icon_slot();
            } else {
                b.icon_hit(&format!("loop_{r}_pause"), "b3_pause.svg", Some(&format!("loop.pause#{i}")));
            }
            let play = if paused { format!("loop.resume#{i}") } else { format!("loop.fire_now#{i}") };
            b.icon_hit(&format!("loop_{r}_play"), "b3_play.svg", Some(&play));
            b.icon_hit(&format!("loop_{r}_trash"), "b3_trash.svg", Some(&format!("loop.delete#{i}")));
            b.close();
        }
        b.close();
    }
    b.gap(8.0);
    b.link("new_loop", tr("+ New loop"), Some(&format!("{}loop.create", dlg::ACTION_FORM)), tok::BLUE_TEXT);
    finish(b);
}

// ---------------------------------------------------------------- Monitors

/// autonomy-05: one row per monitor (its command, its state with the pause
/// reason, its interval), Pause / Resume and Delete, the count line; "+ New
/// monitor" only when `monitor/create` is advertised (`AutonomyPanel.tsx:431`).
fn monitors(b: &mut B<'_>, ctx: &Ctx<'_>, st: &AutonomyState, notice: Option<&(String, bool)>) {
    open(b, "t_title", "Monitors", &scope_line(b.dlg, ctx), notice);
    if st.monitors.is_empty() {
        b.text("monitors_footer_label", tr("No monitors in this session."), &para(tok::MUTED));
    } else {
        b.list_card("monitors_card");
        let text_w = b.card_w() - 2.0 * (ROW_HIT + ROW_GAP) - 8.0;
        for (i, m) in st.monitors.iter().enumerate() {
            if i > 0 {
                b.d.hairline();
            }
            let r = i + 1;
            let paused = m["status"].as_str() == Some("paused");
            b.view(
                &format!("mon_{r}"),
                "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 4 padding: Inset{top: 10 bottom: 10}",
            );
            let c = b.d.anon();
            b.d.view(&c, "width: Fill height: Fit flow: Down spacing: 4");
            let argv = m["argv"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" "))
                .unwrap_or_default();
            // The command wraps too (the web's `.argv` `overflow-wrap: anywhere`),
            // bounded to two lines.
            b.text(&format!("mon_{r}_cmd"), &ui::fit_w(&argv, 2.0 * text_w, 12.5, Face::Mono), &row_mono().w(W::Fill).wrap());
            let mut state = m["status"].as_str().unwrap_or_default().to_owned();
            if let Some(reason) = m["pause_reason"].as_str() {
                state = format!("{state} ({reason})");
            }
            let sub = b.d.anon();
            b.d.view(&sub, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
            b.text(&format!("mon_{r}_state"), &state, &ui::meta());
            b.text("", "·", &ui::meta());
            let clock = b.id(&format!("mon_{r}_clock"));
            b.d.icon(&clock, "b3_clock.svg", 13.0, tok::MUTED);
            b.text(&format!("mon_{r}_int"), &au::interval_short(m["interval_seconds"].as_u64()), &ui::meta());
            b.close();
            b.close();
            let (file, ev) = if paused {
                ("b3_play.svg", format!("monitor.resume#{i}"))
            } else {
                ("b3_pause.svg", format!("monitor.pause#{i}"))
            };
            b.icon_hit(&format!("mon_{r}_pause"), file, Some(&ev));
            b.icon_hit(&format!("mon_{r}_trash"), "b3_trash.svg", Some(&format!("monitor.delete#{i}")));
            b.close();
        }
        b.close();
        let total = st.monitors.len();
        let active = st.monitors.iter().filter(|m| m["status"].as_str() == Some("active")).count();
        b.gap(8.0);
        b.text(
            "monitors_footer_label",
            &tr_with(
                if total == 1 { "{value0} monitor · {value1} active" } else { "{value0} monitors · {value1} active" },
                &[("value0", &total.to_string()), ("value1", &active.to_string())],
            ),
            &ui::meta().w(W::Fill),
        );
    }
    if au::gated(ctx.store, "monitors", "monitor/create") {
        b.gap(8.0);
        b.link("new_monitor", tr("+ New monitor"), Some(&format!("{}monitor.create", dlg::ACTION_FORM)), tok::BLUE_TEXT);
    }
    finish(b);
}

// ------------------------------------------------------------------- Fleet

/// The status chip's ink: terminal grey, waiting amber, else green.
fn peer_ink(s: Status) -> (&'static str, &'static str) {
    if s.terminal() {
        (tok::MUTED, tok::DISABLED_BG)
    } else if matches!(s, Status::WaitingApproval | Status::WaitingAnswer) {
        (tok::AMBER, tok::AMBER_BG)
    } else {
        (tok::GREEN_TEXT, tok::GREEN_BG)
    }
}

/// autonomy-06, the per-session Fleet slice: the Fleet union's rows
/// (`fleet-facts.ts:239-312`: walked acceptance facts ∪ the peer manager's
/// roster; `Peer N · model` labels), each with its status, its meta at
/// minute granularity (a ticking clock never remounts the dialog) and
/// Steer; "No peers yet" (`FleetView.tsx:478-480`) when the union is empty.
fn fleet(b: &mut B<'_>, ctx: &Ctx<'_>, notice: Option<&(String, bool)>) {
    use crate::screens::fleet as f;
    let title = f::query_binding(ctx, "fleet.title").and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
    open(b, "t_title", &title, &scope_line(b.dlg, ctx), notice);
    let rows = f::peer_rows(ctx.store);
    if rows.is_empty() {
        // #32c item 11: with no peer the goal heading goes too (an empty
        // slice heads nothing).
        empty_box(b, "fleet_empty", tr(f::FLEET_EMPTY));
    } else {
        // The session goal heads its peers (the Fleet pane's group heading).
        if let Some(goal) = f::query_binding(ctx, "fleet.goal").and_then(|v| v.as_str().map(str::to_owned)) {
            let g = ui::fit_w(&goal, b.inner, 12.0, Face::Semibold);
            b.text("fleet_goal_label", &g, &Txt::new(12.0, Face::Semibold, tok::MUTED).w(W::Fill));
            b.gap(8.0);
        }
        b.list_card("fleet_card");
        let steer_w = ui::text_w(tr("Steer"), 13.0, Face::Regular) + 4.0;
        for (i, p) in rows.iter().enumerate() {
            if i > 0 {
                b.d.hairline();
            }
            let id = format!("peer_r{i}");
            let word = f::badge_word(p.status);
            // The name takes the room its status chip and Steer leave.
            let text_w = b.card_w() - chip_w(&word) - steer_w - 2.0 * 8.0 - 4.0;
            b.row(&id);
            let c = b.d.anon();
            b.d.view(&c, "width: Fill height: Fit flow: Down spacing: 3");
            b.text(&format!("{id}_name"), &ui::fit_w(&p.label, text_w, 13.5, Face::Semibold), &row_title().w(W::Fill));
            b.text(&format!("{id}_meta"), &ui::fit_w(&dlg::minute_granularity(&f::row_meta(p)), text_w, 12.0, Face::Regular), &ui::meta().w(W::Fill));
            b.close();
            b.chip(&format!("{id}_status"), &word, peer_ink(p.status));
            let (label, tap) = (b.id(&format!("{id}_steer")), b.id(&format!("{id}_steer_hit")));
            let ev = format!("peer.steer#{i}");
            b.d.link_ids(&format!("{label}_box"), &label, &tap, tr("Steer"), Some(&ev), 13.0, tok::BLUE_TEXT);
            b.close();
        }
        b.close();
    }
    finish(b);
}

/// A status chip's width (the kit's `chip`: 11 px medium, 7 px sides).
fn chip_w(word: &str) -> f64 {
    ui::text_w(word, 11.0, Face::Medium) + 14.0
}

/// A list's empty state: the line centred in a quiet box.
fn empty_box(b: &mut B<'_>, local: &str, text: &str) {
    let id = b.id(&format!("{local}_box"));
    b.d.surface(
        &id,
        "width: Fill height: Fit flow: Right align: Align{x: 0.5 y: 0.5} padding: Inset{left: 12 right: 12 top: 16 bottom: 16}",
        tok::SURFACE2,
        10.0,
        Some(tok::HAIRLINE),
    );
    b.text(local, text, &Txt::new(13.0, Face::Regular, tok::MUTED));
    b.close();
}

// ------------------------------------------------------------------- Tasks

/// A task state's chip ink.
fn task_ink(state: &str) -> (&'static str, &'static str) {
    match state {
        "running" | "done" | "completed" => (tok::GREEN_TEXT, tok::GREEN_BG),
        "failed" => (tok::RED_TEXT, tok::RED_BG),
        "pending" => (tok::AMBER, tok::AMBER_BG),
        _ => (tok::MUTED, tok::CHIP),
    }
}

/// The caret the run console draws after its last output line (the
/// card's `t_cursor_text`).
pub const CURSOR: &str = "|";

/// autonomy-07: one block per RUNNING task (its command, Running, the first
/// lines of its output with the caret after the last one — none while it is
/// still "Waiting for output…" — and Cancel), then one row per settled task;
/// "No background tasks in this session." (`SessionTrajectory.tsx:137-139`).
fn tasks(b: &mut B<'_>, ctx: &Ctx<'_>, notice: Option<&(String, bool)>) {
    use crate::screens::fleet as f;
    open(b, "t_title", "Tasks", &scope_line(b.dlg, ctx), notice);
    let runs = f::running_tasks(ctx.store);
    let dones = f::settled_tasks(ctx.store);
    if runs.is_empty() && dones.is_empty() {
        b.text("tasks_empty", tr(f::TASKS_EMPTY), &para(tok::MUTED));
        finish(b);
        return;
    }
    let col = b.d.anon();
    b.d.view(&col, "width: Fill height: Fit flow: Down spacing: 10");
    // A row's command takes the room its glyph and its status chip leave.
    let card_w = b.card_w();
    let cmd_w = |word: &str| card_w - 16.0 - chip_w(word) - 2.0 * 8.0 - 4.0;
    for (i, t) in runs.iter().enumerate() {
        let id = format!("run_r{i}");
        b.card(&id, 10.0);
        let hr = b.d.anon();
        b.d.view(&hr, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        let icon = b.id(&format!("{id}_icon"));
        b.d.icon(&icon, "b3_terminal.svg", 16.0, tok::MUTED);
        let word = f::status_word(&t.state);
        b.text(&format!("{id}_cmd"), &ui::fit_w(&f::task_label(t), cmd_w(&word), 12.5, Face::Mono), &row_mono().w(W::Fill));
        b.chip(&format!("{id}_status"), &word, task_ink(&t.state));
        b.close();
        let console = b.id(&format!("{id}_console"));
        b.d.surface(
            &console,
            "width: Fill height: Fit flow: Down spacing: 4 padding: Inset{left: 12 right: 12 top: 10 bottom: 10}",
            tok::SURFACE2,
            8.0,
            Some(tok::HAIRLINE),
        );
        let lines: Vec<String> = (0..4).map(|k| f::output_line(ctx.store, &t.id, k).unwrap_or_default()).collect();
        let line_w = b.card_w() - 24.0 - 10.0;
        match lines.iter().rposition(|l| !l.trim().is_empty()) {
            // No output yet: the waiting line alone — no caret under it.
            None => b.text(&format!("{id}_log0"), tr("Waiting for output\u{2026}"), &Txt::new(12.0, Face::Regular, tok::MUTED)),
            Some(last) => {
                for (k, l) in lines.iter().enumerate().take(last + 1) {
                    let text = ui::fit_w(if l.is_empty() { " " } else { l }, line_w, 12.0, Face::Mono);
                    let t = Txt::new(12.0, Face::Mono, tok::TEXT);
                    if k < last {
                        b.text(&format!("{id}_log{k}"), &text, &t.w(W::Fill));
                    } else {
                        // The caret follows the last line, left-aligned.
                        let lr = b.d.anon();
                        b.d.view(&lr, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 1");
                        b.text(&format!("{id}_log{k}"), &text, &t);
                        b.text(&format!("{id}_cursor"), CURSOR, &Txt::new(12.0, Face::Mono, tok::MUTED));
                        b.close();
                    }
                }
            }
        }
        b.close();
        let ar = b.d.anon();
        b.d.view(&ar, "width: Fill height: Fit flow: Right");
        b.pill(&format!("{id}_cancel"), tr("Cancel"), &format!("task.cancel#{i}"), Btn::Outline, W::Fit);
        b.close();
        b.close();
    }
    if !dones.is_empty() {
        b.list_card("done_card");
        for (j, t) in dones.iter().enumerate() {
            if j > 0 {
                b.d.hairline();
            }
            let id = format!("done_r{j}");
            b.row(&id);
            let icon = b.id(&format!("{id}_icon"));
            b.d.icon(&icon, "b3_terminal.svg", 16.0, tok::MUTED);
            let word = f::status_word(&t.state);
            b.text(&format!("{id}_cmd"), &ui::fit_w(&f::task_label(t), cmd_w(&word), 12.5, Face::Mono), &row_mono().w(W::Fill));
            b.chip(&format!("{id}_status"), &word, task_ink(&t.state));
            b.close();
        }
        b.close();
    }
    b.close();
    finish(b);
}

// ------------------------------------------------------------------ Review

/// autonomy-02 + the web's NativeReviewDialog: the status (the typed
/// withholding reason, the accepted receipt's specialists while running, or
/// ready), the web's two paragraphs (`NativeReviewDialog.tsx:61-71`) and
/// "Review instructions (optional)" — a real multi-line field the host reads
/// at Start — then "Start native review".
fn review(b: &mut B<'_>, ctx: &Ctx<'_>, notice: Option<&(String, bool)>) {
    use crate::screens::review as rv;
    open(b, "t_title", "Code review", &scope_line(b.dlg, ctx), notice);
    let status = rv::query(ctx, "review.status").and_then(|v| v.as_str().map(str::to_owned));
    let running = rv::ui().agents.is_some();
    let blocked = {
        let ui = ctx.ui.lock().unwrap();
        rv::blocked_reason(ctx.store, &ui)
    };
    let head = match (status, blocked) {
        (Some(s), _) => tr(&s).to_owned(),
        (None, Some(r)) => tr(r).to_owned(),
        (None, None) => tr("Ready to review the current project changes.").to_owned(),
    };
    let card = b.id("run_status_card");
    b.d.surface(
        &card,
        "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}",
        tok::SURFACE2,
        10.0,
        Some(tok::HAIRLINE),
    );
    let icon = b.id(if running { "status_spinner" } else { "status_icon" });
    b.d.icon(&icon, if running { "b1_spinner.svg" } else { "b3_info.svg" }, 16.0, tok::MUTED);
    b.text("t_status", &head, &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Fill).wrap());
    b.close();
    if !running {
        b.gap(14.0);
        b.text("review_not_preview", tr(dlg::REVIEW_NOT_A_PREVIEW), &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Fill).wrap());
        b.gap(8.0);
        b.text("review_results", tr(dlg::REVIEW_RESULTS_HERE), &para(tok::MUTED));
        b.gap(14.0);
        let lid = b.id("prompt_label");
        ui::field_label(b.d, &lid, tr("Review instructions (optional)"));
        b.gap(6.0);
        b.d.input_multiline(
            dlg::REVIEW_PROMPT_INPUT,
            dlg::REVIEW_PROMPT_INPUT,
            "",
            tr("Leave empty to review the current project changes."),
            96.0,
        );
    }
    b.gap(16.0);
    let avail = b.inner;
    actions(b, &[("start_review", "Start native review", "review.start".into(), Btn::Primary)], avail);
    finish(b);
}

// ---------------------------------------------------- confirm / create cards

/// The web's explicit confirm step (`SkillsDialog.tsx:308-353`,
/// `ContextDialog.tsx:155`): the title, the object of the change, the body
/// naming the Profile, Cancel and the primary confirm.
fn confirm_card(b: &mut B<'_>, ctx: &Ctx<'_>, c: &Confirm) {
    open(b, "cf_title", &c.title, &scope_line(b.dlg, ctx), None);
    if !c.detail.is_empty() {
        b.text("cf_detail", &c.detail, &Txt::new(13.5, Face::Semibold, tok::TEXT).w(W::Fill).wrap());
        b.gap(6.0);
    }
    b.text("cf_body", tr(&c.body), &para(tok::MUTED));
    b.gap(18.0);
    let avail = b.inner;
    actions(
        b,
        &[
            ("cf_cancel", "Cancel", dlg::ACTION_CANCEL.into(), Btn::Outline),
            ("cf_confirm", &c.confirm_label, dlg::ACTION_CONFIRM.into(), Btn::Primary),
        ],
        avail,
    );
    finish(b);
}

/// A create form (the web's goal form, LoopCreationControls, the monitor
/// form): the loop cadence, one labelled real input per field (the host
/// reads them at Create), the help, the static note, the refusal, Cancel and
/// the primary submit.
fn form_card(b: &mut B<'_>, ctx: &Ctx<'_>, f: &Form) {
    open(b, "cf_title", &f.title, &scope_line(b.dlg, ctx), None);
    if let Some(mode) = f.mode.as_deref() {
        let segs: Vec<(String, String, String, &str, String, bool)> = dlg::LOOP_MODES
            .iter()
            .map(|(m, label)| {
                (
                    format!("fm_seg_{m}"),
                    format!("fm_seg_{m}_label"),
                    format!("fm_seg_{m}_control"),
                    tr(label),
                    format!("{}{m}", dlg::ACTION_FORM_MODE),
                    *m == mode,
                )
            })
            .collect();
        let refs: Vec<(&str, &str, &str, &str, String, bool)> = segs
            .iter()
            .map(|(s, l, t, x, e, on)| (s.as_str(), l.as_str(), t.as_str(), *x, e.clone(), *on))
            .collect();
        segmented(b, "fm_seg", &refs, W::Fill);
        b.gap(12.0);
    }
    for (k, (id, placeholder, value)) in f.fields.iter().enumerate() {
        if let Some(label) = f.labels.get(k).filter(|l| !l.is_empty()) {
            let lid = b.id(&format!("{id}_label"));
            ui::field_label(b.d, &lid, tr(label));
            b.gap(6.0);
        }
        b.input(id, value, tr(placeholder));
        b.gap(10.0);
    }
    if !f.help.is_empty() {
        b.text("cf_body", tr(&f.help), &para(tok::MUTED));
    }
    if !f.note.is_empty() {
        b.gap(6.0);
        b.text("ff_note", tr(&f.note), &Txt::new(12.0, Face::Regular, tok::FAINT).w(W::Fill).wrap());
    }
    if let Some(err) = &f.error {
        b.gap(8.0);
        b.text("ff_error", tr(err), &para(tok::RED_TEXT));
    }
    b.gap(16.0);
    let avail = b.inner;
    actions(
        b,
        &[
            ("ff_cancel", "Cancel", dlg::ACTION_FORM_CANCEL.into(), Btn::Outline),
            ("ff_submit", &f.submit_label, dlg::ACTION_FORM_SUBMIT.into(), Btn::Primary),
        ],
        avail,
    );
    finish(b);
}
