//! A9 — error boundaries: the native counterpart of the web's
//! `FatalErrorBoundary` (a failed client view becomes a crash screen with
//! Reload app / Copy diagnostics / Report this crash) and `SurfaceBoundary`
//! (a failed SURFACE is replaced by "<Name> unavailable" while the rest of
//! the app — the connection, the conversation, the sidebar — stays alive).
//!
//! A React boundary catches a render throw; natively the module's event
//! handling, its surface lowering/mounting and its drawing are guarded with
//! `std::panic::catch_unwind` (the host profile is `panic = "unwind"`,
//! octosense `Cargo.toml` [profile.release]). A panic hook records the
//! message and location; the report is the web's `buildSafeDiagnostic`:
//! redacted query/bearer credentials, capped at 4000 characters
//! (`screens::palette::redact_secrets`, `FatalErrorBoundary.tsx:93-107`).
//!
//! Web copy: `FatalErrorBoundary.tsx:55-90` (crash screen),
//! `SurfaceBoundary.tsx:44-104` (unavailable surface: a modal when the surface
//! is dismissable, an inline section otherwise).
use std::sync::Mutex;

use super::activity::Lowered;
use super::board3::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

/// The crash report link (`FatalErrorBoundary.tsx:86`, this app's issues).
pub const REPORT_URL: &str = "https://github.com/octos-org/octoscode-app/issues/new";

/// How a failed surface is shown (`SurfaceBoundary.tsx:85-104`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A dialog / sheet: a modal panel with Close.
    Modal,
    /// An inline region (a pane): a section in its place.
    Inline,
}

/// A failed surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable {
    pub name: String,
    pub kind: Kind,
    pub report: String,
}

/// The copy-diagnostics button's state (`FatalCrashScreen`'s copyState).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyState {
    Idle,
    Copied,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crash {
    pub report: String,
    pub copy: CopyState,
}

static CRASH: Mutex<Option<Crash>> = Mutex::new(None);
static UNAVAILABLE: Mutex<Option<Unavailable>> = Mutex::new(None);
/// The last panic's message and location (the hook writes it).
static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

fn lock<T>(m: &'static Mutex<T>) -> std::sync::MutexGuard<'static, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Install the panic hook once: record the message + location for the
/// report, then run the previous hook (the log keeps its line).
pub fn install_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
                (*s).to_owned()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown render error".to_owned()
            };
            let at = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_default();
            *lock(&LAST_PANIC) = Some(if at.is_empty() { msg } else { format!("{msg}\n    at {at}") });
            prev(info);
        }));
    });
}

/// `buildSafeDiagnostic` for a caught panic: `Panic: <message>` + where it
/// was caught, redacted, 4000-capped.
pub fn safe_report(payload: &(dyn std::any::Any + Send), surface: &str) -> String {
    let recorded = lock(&LAST_PANIC).take();
    let message = recorded.unwrap_or_else(|| {
        if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_owned()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown render error".to_owned()
        }
    });
    super::palette::redact_secrets(&format!("Panic: {message}\n    in {surface}"))
}

pub fn crashed() -> bool {
    lock(&CRASH).is_some()
}

pub fn crash() -> Option<Crash> {
    lock(&CRASH).clone()
}

/// The fatal boundary caught a panic: the whole client view is replaced.
pub fn note_crash(report: String) {
    *lock(&CRASH) = Some(Crash { report, copy: CopyState::Idle });
    *lock(&UNAVAILABLE) = None;
}

pub fn set_copy(state: CopyState) {
    if let Some(c) = lock(&CRASH).as_mut() {
        c.copy = state;
    }
}

pub fn unavailable() -> Option<Unavailable> {
    lock(&UNAVAILABLE).clone()
}

/// A surface boundary caught a panic: that surface is replaced.
pub fn note_unavailable(name: &str, kind: Kind, report: String) {
    *lock(&UNAVAILABLE) = Some(Unavailable { name: name.to_owned(), kind, report });
}

/// Reload / Close: back to the app.
pub fn clear() {
    *lock(&CRASH) = None;
    *lock(&UNAVAILABLE) = None;
}

pub fn dismiss_unavailable() {
    *lock(&UNAVAILABLE) = None;
}

/// The test seam that arms a panic at a named point (`OCTOSCODE_PANIC_PROBE`
/// = `fatal:<action id>` or `surface:<name>`), so a hidden walk can drive the
/// boundary through the real UI. Unset in production: never panics.
pub fn probe(point: &str) {
    if let Ok(v) = std::env::var("OCTOSCODE_PANIC_PROBE") {
        if v == point {
            panic!("panic probe {point}: GET /ws?token=probe-secret-123 Authorization: Bearer probe-bearer-456");
        }
    }
}

/// The fatal probe point for an action id (`fatal:<action>`), checked only
/// when the seam is set.
pub fn probe_action(action: &str) {
    if std::env::var_os("OCTOSCODE_PANIC_PROBE").is_some() {
        probe(&format!("fatal:{action}"));
    }
}

pub const ACTION_RELOAD: &str = "a9.crash.reload";
pub const ACTION_COPY: &str = "a9.crash.copy";
pub const ACTION_REPORT: &str = "a9.crash.report";
pub const ACTION_CLOSE: &str = "a9.unavail.close";
pub const ACTION_NOOP: &str = "a9.unavail.noop";

pub fn routes(action: &str) -> bool {
    matches!(action, ACTION_RELOAD | ACTION_COPY | ACTION_REPORT | ACTION_CLOSE | ACTION_NOOP)
}

// ------------------------------------------------------------- the surfaces

/// The crash screen (`FatalCrashScreen`, `FatalErrorBoundary.tsx:55-90`): the
/// eyebrow, "Client view unavailable", the consequence copy, the redacted
/// report, Reload app (primary) / Copy diagnostics, the report link.
pub fn lower_crash(frame: &Frame) -> Option<Lowered> {
    let c = crash()?;
    let compact = frame.avail_w < 640.0;
    let w = if compact { (frame.avail_w - 32.0).max(240.0) } else { (frame.avail_w - 64.0).min(640.0) }.floor();
    let mut d = Dsl::new();
    d.view("a9_cr_root", "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5}");
    d.rule("a9_cr_bg", "width: Fill height: Fill", tok::SURFACE2);
    d.surface(
        "a9_cr_card",
        &format!("width: {w} height: Fit flow: Down spacing: 12 padding: Inset{{left: 28 right: 28 top: 28 bottom: 24}}"),
        tok::SURFACE,
        16.0,
        Some(tok::HAIRLINE),
    );
    d.text("a9_cr_eyebrow", "OCTOSCODE STOPPED RENDERING", &Txt::new(11.0, Face::Medium, tok::MUTED));
    d.text("a9_cr_title", "Client view unavailable", &Txt::new(22.0, Face::Semibold, tok::TEXT));
    d.text(
        "a9_cr_body",
        "The client could not recover this view. Closing its connection may have stopped running work. Octos keeps persisted history; unsent drafts and queued messages may be lost when you reload.",
        &Txt::new(14.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    // The redacted diagnostics (`<pre aria-label="Redacted crash diagnostics">`).
    d.surface(
        "a9_cr_report_box",
        "width: Fill height: Fit flow: Down padding: Inset{left: 12 right: 12 top: 10 bottom: 10}",
        tok::SURFACE2,
        8.0,
        Some(tok::HAIRLINE),
    );
    d.text("a9_cr_report", &c.report, &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill).wrap());
    d.close();
    d.view("a9_cr_actions", "width: Fill height: Fit flow: Right spacing: 10 align: Align{x: 0.0 y: 0.5} margin: Inset{top: 4}");
    d.button("a9_cr_reload", "Reload app", ACTION_RELOAD, Btn::Primary, W::Fit, 40.0);
    let copy_label = match c.copy {
        CopyState::Idle => "Copy diagnostics",
        CopyState::Copied => "Copied",
        CopyState::Failed => "Copy failed",
    };
    d.button("a9_cr_copy", copy_label, ACTION_COPY, Btn::Outline, W::Px(ui::text_w("Copy diagnostics", 13.0, Face::Medium) + 32.0), 40.0);
    d.close();
    d.link("a9_cr_report_link", "Report this crash ↗", Some(ACTION_REPORT), 13.0);
    d.close(); // card
    d.close(); // root
    let taps = d.taps.clone();
    Some(Lowered { dsl: d.finish(), taps, inputs: Vec::new() })
}

/// `UnavailableSurface` (`SurfaceBoundary.tsx:44-104`): "<Name> unavailable",
/// the two alert paragraphs, Close (dismiss) and Reload app. A dialog's
/// failure is a modal panel over the app; an inline region's is a section
/// card in that region (`content` = the region's frame, module-local).
pub fn lower_unavailable(frame: &Frame, content: Option<(f64, f64, f64, f64)>) -> Option<Lowered> {
    let u = unavailable()?;
    let mut d = Dsl::new();
    let modal = u.kind == Kind::Modal;
    if modal {
        let w = (frame.avail_w - 32.0).min(440.0).max(240.0).floor();
        d.view("a9_un_root", "width: Fill height: Fill flow: Overlay align: Align{x: 0.5 y: 0.5}");
        d.rule("a9_un_mask", "width: Fill height: Fill", tok::MASK);
        d.view("a9_un_backdrop_box", "width: Fill height: Fill flow: Overlay");
        d.tap("a9_un_backdrop", ACTION_CLOSE);
        d.close();
        panel(&mut d, &u, w, modal);
        d.close();
    } else {
        // In the failed region's place (`<section>` in the web): the region
        // is replaced by a plain surface with the panel at its top; the rest
        // of the app (sidebar, header) stays live around it.
        let (x, y, rw, rh) = content.unwrap_or((0.0, 0.0, frame.avail_w, frame.avail_h));
        let w = (rw - 48.0).min(560.0).max(240.0).floor();
        d.view("a9_un_root", "width: Fill height: Fill flow: Overlay");
        d.surface(
            "a9_un_region",
            &format!(
                "width: {} height: {} margin: Inset{{left: {} top: {}}} flow: Overlay",
                rw.floor(),
                rh.floor(),
                x.floor(),
                y.floor()
            ),
            tok::SURFACE,
            0.0,
            None,
        );
        // The region swallows presses: nothing hidden behind it (the
        // conversation, the composer) may take a click.
        d.view("a9_un_region_swallow_box", "width: Fill height: Fill flow: Overlay");
        d.tap("a9_un_region_swallow", ACTION_NOOP);
        d.close();
        d.view(
            "a9_un_region_col",
            "width: Fill height: Fill flow: Down align: Align{x: 0.5 y: 0.0} padding: Inset{top: 24}",
        );
        panel(&mut d, &u, w, modal);
        d.close();
        d.close();
        d.close();
    }
    let taps = d.taps.clone();
    Some(Lowered { dsl: d.finish(), taps, inputs: Vec::new() })
}

fn panel(d: &mut Dsl, u: &Unavailable, w: f64, modal: bool) {
    d.surface(
        "a9_un_panel",
        &format!("width: {w} height: Fit flow: Overlay"),
        tok::SURFACE,
        16.0,
        Some(tok::HAIRLINE),
    );
    if modal {
        d.view("a9_un_swallow_box", "width: Fill height: Fill flow: Overlay");
        d.tap("a9_un_swallow", ACTION_NOOP);
        d.close();
    }
    d.view("a9_un_col", "width: Fill height: Fit flow: Down spacing: 10 padding: Inset{left: 22 right: 22 top: 22 bottom: 20}");
    d.text("a9_un_title", &format!("{} unavailable", u.name), &Txt::new(18.0, Face::Semibold, tok::TEXT).w(W::Fill).wrap());
    d.text(
        "a9_un_p0",
        "This view could not be displayed. Other parts of the app remain available.",
        &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Fill).wrap(),
    );
    d.text(
        "a9_un_p1",
        "Reload the app to try again. Reloading may stop running work and discard drafts and queued messages.",
        &Txt::new(13.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    d.view("a9_un_actions", "width: Fill height: Fit flow: Right spacing: 10 align: Align{x: 1.0 y: 0.5} margin: Inset{top: 6}");
    d.button("a9_un_close", "Close", ACTION_CLOSE, Btn::Outline, W::Fit, 36.0);
    d.button("a9_un_reload", "Reload app", ACTION_RELOAD, Btn::Primary, W::Fit, 36.0);
    d.close();
    d.close(); // col
    d.close(); // panel
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> std::sync::MutexGuard<'static, ()> {
        crate::screens::theme::test_lock()
    }

    #[test]
    fn a_caught_panic_becomes_a_redacted_capped_report() {
        let _g = guard();
        install_hook();
        let caught = std::panic::catch_unwind(|| {
            panic!("bad state GET /ws?token=abc123&x=1 Authorization: Bearer sk-live-999");
        })
        .expect_err("panicked");
        let report = safe_report(caught.as_ref(), "Activity");
        assert!(report.starts_with("Panic: bad state"), "{report}");
        assert!(report.contains("token=[redacted]") && report.contains("Bearer [redacted]"), "{report}");
        assert!(!report.contains("abc123") && !report.contains("sk-live-999"));
        assert!(report.contains("at ") && report.contains("in Activity"), "location + surface: {report}");
        let long = std::panic::catch_unwind(|| panic!("{}", "x".repeat(9000))).expect_err("panicked");
        assert_eq!(safe_report(long.as_ref(), "s").chars().count(), 4000, "capped at 4000");
    }

    #[test]
    fn the_crash_screen_carries_the_web_copy_and_its_three_actions() {
        let _g = guard();
        note_crash("Panic: boom".into());
        assert!(crashed());
        let low = lower_crash(&Frame::DESKTOP).unwrap();
        for s in ["Client view unavailable", "OCTOSCODE STOPPED RENDERING", "Reload app", "Copy diagnostics", "Report this crash ↗", "Panic: boom"] {
            assert!(low.dsl.contains(s), "{s}");
        }
        let events: Vec<&str> = low.taps.iter().map(|(_, e)| e.as_str()).collect();
        assert_eq!(events, [ACTION_RELOAD, ACTION_COPY, ACTION_REPORT]);
        assert_eq!(crate::screens::taps::wired_taps(&low.dsl), low.taps);
        set_copy(CopyState::Copied);
        assert!(lower_crash(&Frame::DESKTOP).unwrap().dsl.contains("\"Copied\""));
        clear();
        assert!(!crashed() && lower_crash(&Frame::DESKTOP).is_none());
    }

    #[test]
    fn a_failed_surface_is_a_modal_or_an_inline_section() {
        let _g = guard();
        note_unavailable("Activity", Kind::Modal, "Panic: x".into());
        let modal = lower_unavailable(&Frame::DESKTOP, None).unwrap();
        assert!(modal.dsl.contains("Activity unavailable") && modal.dsl.contains("a9_un_mask"));
        assert!(modal.dsl.contains("This view could not be displayed. Other parts of the app remain available."));
        let events: Vec<&str> = modal.taps.iter().map(|(_, e)| e.as_str()).collect();
        assert!(events.contains(&ACTION_CLOSE) && events.contains(&ACTION_RELOAD));
        note_unavailable("Fleet", Kind::Inline, "Panic: y".into());
        let inline = lower_unavailable(&Frame::DESKTOP, Some((281.0, 0.0, 709.0, 603.0))).unwrap();
        assert!(inline.dsl.contains("Fleet unavailable"));
        assert!(!inline.dsl.contains("a9_un_mask"), "an inline section has no backdrop");
        dismiss_unavailable();
        assert!(unavailable().is_none());
        clear();
    }

    #[test]
    fn the_probe_is_inert_unless_armed() {
        probe("surface:activity"); // OCTOSCODE_PANIC_PROBE unset: no panic
    }
}
