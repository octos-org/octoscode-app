//! A26 — error toasts (parity row `error/error` "error toasts (transient,
//! bounded queue)").
//!
//! The web oracle has NO toast component (A9's evidence: `grep -ri toast
//! apps/web/src` finds nothing); the row came from the old appcard's
//! `ToastQueue` (depth 3, `octos-app-store/src/toasts.rs:28`). The operator
//! chose to build them, from the app's own notice kit: A13's plain-language
//! lead with the raw cause muted under it (`board3::ui::failure`) on board 3's
//! notice card (the transcript's system-notice row: a 26 px icon chip, a
//! 13 px title, a muted body; a white card, a 1 px hairline, radius 12).
//!
//! What they carry: the failures that, before them, reached ONLY the app log —
//! an open, a new chat, a send, a Stop, a steer, a session-list refresh, a
//! copy of the conversation (each `lib.rs` arm that logged `… : {e}` now also
//! calls [`failed`]). An error a surface already shows inline (a dialog's
//! error line, the connection banner, the Connect card) is never toasted.
//!
//! The rules:
//! * **Transient** — a toast stays [`SHOW_MS`] on screen, then leaves on its
//!   own; × dismisses it at once.
//! * **Bounded** — at most [`CAPACITY`] at a time; a newer one pushes the
//!   oldest out, and the stack SAYS so ("1 earlier error no longer shown")
//!   until it empties. The same failure again bumps a count on its toast
//!   ("×2") and restarts its clock instead of stacking a copy.
//! * **Never in the way** — the stack sits under the conversation header,
//!   right-aligned (full width on a phone), and is fitted to the room ABOVE
//!   the composer: the newest toasts that fit are drawn, the rest wait (and
//!   are counted in the note) — so it never covers the composer; while any
//!   modal surface is open (Settings, a dialog, the command palette, the
//!   phone drawer…) the stack is HELD — not drawn, its clocks stopped — so it
//!   can never cover a dialog's primary action; it shows when that closes.
//!   Only a drawn toast's clock runs.
//! * **Announced honestly** — every toast is logged by name when it arrives
//!   (`[octoscode] toast: <lead> — <cause>`), and when it leaves (dismissed,
//!   expired, or pushed out); the lead says what failed, plainly, and never
//!   suggests it worked; the cause is the client's own error text.
use std::collections::VecDeque;
use std::sync::Mutex;

use super::board3::ui::{self, tok, Dsl, Face, Txt, W};

/// At most three toasts at a time (the appcard's `ToastQueue::default()`).
pub const CAPACITY: usize = 3;
/// How long a toast stays on screen (the clock runs only while it is drawn).
pub const SHOW_MS: u64 = 8_000;
/// A cause longer than this is cut (the whole text stays in the log).
pub const CAUSE_MAX: usize = 180;

/// The dismiss tap's action id (`toast.dismiss#<toast id>`).
pub const ACTION_DISMISS: &str = "toast.dismiss";

/// A failure that, before the toasts, reached only the log — each names
/// what failed in plain words (A13's lead).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// A session row / switcher open (`thread.open`).
    OpenSession,
    /// New chat (`conv.new_chat`).
    NewChat,
    /// A prompt's send (`conv.submit_draft`).
    Send,
    /// Stop (`turn/interrupt`).
    Stop,
    /// Steer now (`turn/steer`).
    Steer,
    /// The sessions list refresh (`session/list`).
    Refresh,
    /// Copy as Markdown / copy the transcript.
    CopyConversation,
}

impl Op {
    /// The plain-language lead.
    pub fn lead(self) -> &'static str {
        match self {
            Op::OpenSession => "Couldn't open that session.",
            Op::NewChat => "Couldn't start a new chat.",
            Op::Send => "Your message was not sent.",
            Op::Stop => "Couldn't stop the turn.",
            Op::Steer => "Couldn't steer the turn.",
            Op::Refresh => "Couldn't refresh the sessions.",
            Op::CopyConversation => "Couldn't copy the conversation.",
        }
    }
}

/// One toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub lead: String,
    pub cause: String,
    /// How many times this same failure arrived while it was up.
    pub count: u32,
    /// Milliseconds it has been ON SCREEN (stopped while held).
    pub shown_ms: u64,
    /// Drawn by the last lowering (only a drawn toast ages).
    pub drawn: bool,
}

/// The queue (one per process, like the theme preference).
#[derive(Debug, Default)]
pub struct Queue {
    pub items: VecDeque<Toast>,
    /// Toasts pushed out by newer ones since the stack was last empty.
    pub dropped: u32,
    next_id: u64,
    last_tick_ms: Option<u64>,
}

static QUEUE: Mutex<Option<Queue>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Queue) -> R) -> R {
    let mut g = QUEUE.lock().unwrap_or_else(|p| p.into_inner());
    f(g.get_or_insert_with(Queue::default))
}

/// The client's error as a toast cause: whitespace collapsed (the web prints
/// a cause in a `<p>`), cut at [`CAUSE_MAX`] characters.
pub fn clean_cause(s: &str) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= CAUSE_MAX {
        return flat;
    }
    let mut out: String = flat.chars().take(CAUSE_MAX - 1).collect();
    out = out.trim_end().to_owned();
    out.push('…');
    out
}

/// A failure of `op` with the client's error text: queue it and wake the UI.
/// Called from the runtime threads (the `lib.rs` arms that used to only log).
pub fn failed(op: Op, cause: &str) {
    push(op.lead(), cause);
    makepad_widgets::SignalToUI::set_ui_signal();
}

/// Queue one toast. Returns its id.
pub fn push(lead: &str, cause: &str) -> u64 {
    let cause_shown = clean_cause(cause);
    makepad_widgets::log!("[octoscode] toast: {lead} — {}", cause.trim());
    with(|q| {
        if let Some(t) = q.items.iter_mut().find(|t| t.lead == lead && t.cause == cause_shown) {
            t.count += 1;
            t.shown_ms = 0;
            return t.id;
        }
        if q.items.is_empty() {
            // A fresh stack: its clock starts at the next tick (an old
            // timestamp would age the newcomer by the idle time).
            q.last_tick_ms = None;
        }
        if q.items.len() >= CAPACITY {
            if let Some(old) = q.items.pop_front() {
                q.dropped += 1;
                makepad_widgets::log!("[octoscode] toast pushed out (queue full, {CAPACITY}): {}", old.lead);
            }
        }
        q.next_id += 1;
        let id = q.next_id;
        q.items.push_back(Toast { id, lead: lead.to_owned(), cause: cause_shown, count: 1, shown_ms: 0, drawn: false });
        id
    })
}

/// × on a toast.
pub fn dismiss(id: u64) -> bool {
    with(|q| {
        let before = q.items.len();
        q.items.retain(|t| {
            if t.id == id {
                makepad_widgets::log!("[octoscode] toast dismissed: {}", t.lead);
            }
            t.id != id
        });
        let gone = q.items.len() != before;
        if q.items.is_empty() {
            q.dropped = 0;
        }
        gone
    })
}

/// Advance the on-screen clocks to `now_ms` (a monotonic clock): while
/// `held`, nothing ages. Expired toasts leave. `true` when the stack changed.
pub fn tick(now_ms: u64, held: bool) -> bool {
    with(|q| {
        let dt = q.last_tick_ms.map(|t| now_ms.saturating_sub(t)).unwrap_or(0);
        q.last_tick_ms = Some(now_ms);
        if held || dt == 0 {
            return false;
        }
        let before = q.items.len();
        for t in q.items.iter_mut().filter(|t| t.drawn) {
            t.shown_ms += dt;
        }
        q.items.retain(|t| {
            let keep = t.shown_ms < SHOW_MS;
            if !keep {
                makepad_widgets::log!("[octoscode] toast expired: {}", t.lead);
            }
            keep
        });
        if q.items.is_empty() {
            q.dropped = 0;
        }
        q.items.len() != before
    })
}

/// The time until the next DRAWN toast expires (ms), if any is up.
pub fn next_expiry_ms() -> Option<u64> {
    with(|q| q.items.iter().filter(|t| t.drawn).map(|t| SHOW_MS.saturating_sub(t.shown_ms)).min())
}

/// Mark which toasts the screen shows (the rest wait; held = none).
pub fn set_drawn(ids: &[u64]) {
    with(|q| {
        for t in q.items.iter_mut() {
            t.drawn = ids.contains(&t.id);
        }
    })
}

/// The toasts now, oldest first, and the pushed-out count.
pub fn snapshot() -> (Vec<Toast>, u32) {
    with(|q| (q.items.iter().cloned().collect(), q.dropped))
}

/// Test seam: an empty queue.
pub fn reset() {
    *QUEUE.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

/// A monotonic millisecond clock for [`tick`].
pub fn now_ms() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
}

/// The ids this surface owns.
pub fn routes(action: &str) -> bool {
    action == ACTION_DISMISS
}

/// The stack's line under the toasts: earlier errors that newer ones
/// pushed out, or that wait for room.
pub fn dropped_line(n: u32) -> String {
    if n == 1 {
        "1 earlier error no longer shown".to_owned()
    } else {
        format!("{n} earlier errors no longer shown")
    }
}

/// The note's height (one 11.5 px line, 7 + 8 px padding) and the 1 px
/// hairline between rows.
const NOTE_H: f64 = 31.0;
const GAP: f64 = 1.0;

/// A toast's estimated height at `width` (the kit's advance estimate):
/// 10 + 10 px padding, the 3 px top inset, the lead (16 px lines), the cause
/// (17 px lines) and the count line; never under the 26 px mark's row.
pub fn toast_height(t: &Toast, width: f64) -> f64 {
    // 12 left + 26 mark + 10 gap + text + 10 gap + 28 close + 6 right.
    let col = (width - 92.0).max(80.0);
    let lines = |s: &str, px: f64, face: Face| -> f64 {
        if s.is_empty() {
            0.0
        } else {
            (ui::text_w(s, px, face) / (col * 0.97)).ceil().max(1.0)
        }
    };
    let lead = lines(&t.lead, 13.0, Face::Medium) * 16.0;
    let cause = if t.cause.is_empty() || t.cause == t.lead { 0.0 } else { 3.0 + lines(&t.cause, 12.0, Face::Regular) * 17.0 };
    let count = if t.count > 1 { 3.0 + 15.0 } else { 0.0 };
    (20.0 + 3.0 + lead + cause + count).max(46.0)
}

/// The lowered stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lowered {
    pub dsl: String,
    /// (widget id, routed event) — the × taps.
    pub taps: Vec<(String, String)>,
}

/// The stack's width: 360 px at most; the window less its 12 px gutters on a
/// phone.
pub fn stack_width(avail_w: f64, compact: bool) -> f64 {
    if compact {
        (avail_w - 24.0).max(200.0)
    } else {
        (avail_w - 32.0).clamp(240.0, 360.0)
    }
}

/// Lower the stack for `width` px within `room` px of height (board-3 kit,
/// the theme's surfaces), or `None` when nothing is up. The newest toasts
/// that fit are drawn (always one); the others wait, counted in the note.
pub fn lower(width: f64, room: f64) -> Option<Lowered> {
    let (items, dropped) = snapshot();
    if items.is_empty() {
        return None;
    }
    // Newest on top: the one that just arrived is where the eye lands.
    let mut shown: Vec<&Toast> = Vec::new();
    let mut used = 0.0;
    for (i, t) in items.iter().rev().enumerate() {
        let h = toast_height(t, width) + if shown.is_empty() { 0.0 } else { GAP };
        let rest_after = items.len() - i - 1;
        let note = if dropped > 0 || rest_after > 0 { GAP + NOTE_H } else { 0.0 };
        if !shown.is_empty() && used + h + note > room {
            break;
        }
        used += h;
        shown.push(t);
    }
    let waiting = (items.len() - shown.len()) as u32;
    set_drawn(&shown.iter().map(|t| t.id).collect::<Vec<_>>());
    // ONE notice card (board 3's row lists: "each row is separated by a
    // hairline divider, not a box"), so nothing under the stack shows
    // between its rows; raised over a dark look's window (the window and
    // the light card are both the surface colour there).
    let fill = if crate::screens::theme::resolved() == "dark" { tok::CHIP } else { tok::SURFACE };
    let mut d = Dsl::new();
    d.surface(
        "a26_toasts",
        &format!("width: {} height: Fit flow: Down padding: 0", width.round()),
        fill,
        12.0,
        Some(tok::HAIRLINE),
    );
    for (i, t) in shown.into_iter().enumerate() {
        let id = t.id;
        if i > 0 {
            d.hairline();
        }
        d.view(
            &format!("a26_toast_{id}"),
            "width: Fill height: Fit flow: Right spacing: 10 align: Align{x: 0.0 y: 0.0} padding: Inset{left: 12 right: 6 top: 10 bottom: 10}",
        );
        // The notice row's icon chip, with the error mark.
        d.surface(
            &format!("a26_toast_mark_box_{id}"),
            "width: 26 height: 26 flow: Overlay align: Align{x: 0.5 y: 0.5}",
            tok::RED_BG,
            13.0,
            None,
        );
        let mark = crate::design::icon_resource("a26_error.svg");
        d.raw(&format!(
            "a26_toast_mark_{id} := Svg {{\nwidth: 15 height: 15 animating: false draw_svg.svg: file_resource({mark:?}) draw_svg.preserve_viewbox: true draw_svg.color: {}\n}}",
            // The red TEXT ink: Solarized's red fill read 2.67:1 on its tint
            // (screens::theme::CONTRAST_PAIRS).
            tok::RED_TEXT
        ));
        d.close();
        let col = d.anon();
        d.view(&col, "width: Fill height: Fit flow: Down spacing: 3 padding: Inset{top: 3}");
        d.text(&format!("a26_toast_lead_{id}"), &t.lead, &Txt::new(13.0, Face::Medium, tok::RED_TEXT).w(W::Fill).wrap());
        if !t.cause.is_empty() && t.cause != t.lead {
            d.text(&format!("a26_toast_cause_{id}"), &t.cause, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        }
        if t.count > 1 {
            d.text(&format!("a26_toast_count_{id}"), &format!("×{}", t.count), &Txt::new(11.5, Face::Medium, tok::MUTED));
        }
        d.close();
        // ×: a 28 px target (the board's small close glyph).
        let x = format!("a26_toast_x_{id}");
        d.view(&format!("{x}_box"), "width: 28 height: 28 flow: Overlay align: Align{x: 0.5 y: 0.5}");
        d.icon("", "b3_x_small.svg", 12.0, tok::TEXT);
        d.tap(&x, &format!("{ACTION_DISMISS}#{id}"));
        d.close();
        d.close();
    }
    let dropped = dropped + waiting;
    if dropped > 0 {
        d.hairline();
        d.view(
            "a26_toasts_dropped_box",
            "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 14 right: 12 top: 7 bottom: 8}",
        );
        d.text("a26_toasts_dropped", &dropped_line(dropped), &Txt::new(11.5, Face::Regular, tok::MUTED).w(W::Fill));
        d.close();
    }
    d.close();
    let taps = d.taps.clone();
    let dsl = ui::themed_icons(&crate::screens::theme::retint_dsl(&d.finish()));
    Some(Lowered { dsl, taps })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> std::sync::MutexGuard<'static, ()> {
        crate::screens::theme::test_lock()
    }

    #[test]
    fn the_queue_is_bounded_and_says_what_it_pushed_out() {
        let _g = guard();
        reset();
        for i in 0..5 {
            push("Couldn't open that session.", &format!("session/open: rpc error {i}"));
        }
        let (items, dropped) = snapshot();
        assert_eq!(items.len(), CAPACITY);
        assert_eq!(dropped, 2);
        assert_eq!(items[0].cause, "session/open: rpc error 2", "the oldest went first");
        let low = lower(360.0, 1000.0).unwrap();
        assert!(low.dsl.contains("2 earlier errors no longer shown"), "{}", low.dsl);
        assert_eq!(low.taps.len(), CAPACITY, "one × per toast");
        reset();
    }

    #[test]
    fn the_same_failure_counts_instead_of_stacking() {
        let _g = guard();
        reset();
        let a = push("Couldn't stop the turn.", "turn/interrupt: transport: closed");
        let b = push("Couldn't stop the turn.", "turn/interrupt:  transport:\nclosed");
        assert_eq!(a, b);
        let (items, _) = snapshot();
        assert_eq!((items.len(), items[0].count), (1, 2));
        assert!(lower(360.0, 1000.0).unwrap().dsl.contains("×2"));
        reset();
    }

    #[test]
    fn a_toast_expires_only_while_it_is_on_screen() {
        let _g = guard();
        reset();
        push("Couldn't refresh the sessions.", "session/list: timeout");
        tick(1_000, false);
        tick(1_000 + SHOW_MS, false);
        assert_eq!(snapshot().0.len(), 1, "not drawn yet: its clock has not started");
        assert!(lower(360.0, 1000.0).is_some(), "drawn");
        tick(1_000 + 2 * SHOW_MS, true);
        assert_eq!(snapshot().0.len(), 1, "held: the clock stops");
        tick(1_000 + 3 * SHOW_MS - 1, false);
        assert_eq!(snapshot().0.len(), 1, "one ms short");
        assert!(tick(1_000 + 3 * SHOW_MS, false));
        assert!(snapshot().0.is_empty());
        assert!(lower(360.0, 1000.0).is_none());
        reset();
    }

    #[test]
    fn the_stack_fits_the_room_above_the_composer_and_counts_who_waits() {
        let _g = guard();
        reset();
        for i in 0..3 {
            push("Couldn't stop the turn.", &format!("turn/interrupt: transport: closed ({i})"));
        }
        let one = toast_height(&snapshot().0[0], 360.0);
        // Room for two toasts and the note: the oldest waits, undrawn.
        let low = lower(360.0, 2.0 * one + GAP + GAP + NOTE_H + 1.0).unwrap();
        assert_eq!(low.taps.len(), 2, "{}", low.dsl);
        assert!(low.dsl.contains("1 earlier error no longer shown"));
        let (items, _) = snapshot();
        assert!(!items[0].drawn && items[1].drawn && items[2].drawn, "newest drawn: {items:?}");
        // Its clock does not run while it waits.
        tick(10, false);
        tick(10 + SHOW_MS - 1, false);
        assert_eq!(snapshot().0.len(), 3);
        // A tiny room still shows the newest one.
        assert_eq!(lower(360.0, 10.0).unwrap().taps.len(), 1);
        reset();
    }

    #[test]
    fn dismiss_takes_one_and_the_note_clears_with_the_stack() {
        let _g = guard();
        reset();
        let ids: Vec<u64> = (0..4).map(|i| push("Couldn't start a new chat.", &format!("e{i}"))).collect();
        assert_eq!(snapshot().1, 1);
        assert!(!dismiss(ids[0]), "already pushed out");
        for id in &ids[1..] {
            assert!(dismiss(*id));
        }
        assert_eq!(snapshot(), (vec![], 0));
        reset();
    }

    /// The lowered stack evaluates in the app VM (the call `MountCache::mount`
    /// makes) in both looks, and every × it routes exists in the evaluated
    /// tree — a control the DSL lost is a dead tap.
    #[test]
    fn the_stack_evaluates_in_the_app_vm_with_every_dismiss() {
        use makepad_widgets::*;
        let _g = guard();
        reset();
        for i in 0..4 {
            push("Couldn't start a new chat.", &format!("session/open refused ({i})"));
        }
        push("Couldn't start a new chat.", "session/open refused (3)");
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(makepad_widgets::script_mod);
        cx.with_vm(octoscript_widgets::design::script_mod);
        cx.with_vm(octoscript_widgets::kit::script_mod);
        for dark in [false, true] {
            crate::screens::theme::set_preference(if dark { "dark" } else { "light" });
            let low = lower(360.0, 1000.0).expect("the stack");
            assert!(low.dsl.contains("×2"), "the repeat counts: {}", low.dsl);
            let view = crate::mount::eval_component(&mut cx, MAIN_SPLASH_VM_ID, &low.dsl)
                .unwrap_or_else(|e| panic!("dark={dark}: {e}\n{}", low.dsl));
            assert_eq!(low.taps.len(), CAPACITY);
            for (id, ev) in &low.taps {
                assert!(!view.widget(&mut cx, &[LiveId::from_str(id)]).is_empty(), "{id} ({ev}) is not in the tree");
            }
            assert!(!view.widget(&mut cx, ids!(a26_toasts_dropped)).is_empty(), "the note");
        }
        crate::screens::theme::reset_state();
        reset();
    }

    #[test]
    fn a_long_cause_is_cut_but_logged_whole() {
        let long = "x".repeat(500);
        let c = clean_cause(&long);
        assert_eq!(c.chars().count(), CAUSE_MAX);
        assert!(c.ends_with('…'));
    }
}
