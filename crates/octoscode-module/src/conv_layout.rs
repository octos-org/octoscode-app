//! A1 — the conversation pane's responsive geometry, the web's numbers.
//!
//! The Stage-B components were cut from 406 px PHONE artboards, so every
//! lowered row kept a fixed width (prose 356, composer 374, tool cell 371) and
//! a desktop window left the right half of the pane empty. The web solves the
//! same problem with two CSS variables
//! (`src-web/apps/web/src/app/theme.css:29-33`):
//!
//! ```css
//! --dsw-layout-chat: clamp(736px, 62vw, 1040px);
//! --dsw-layout-chat-wide: calc(var(--dsw-layout-chat) + 32px);
//! ```
//!
//! and centres both columns with a padding of at least 24 px
//! (`styles.css:519` `.conversation-scroll`, `:771` `.composer-wrap`), which
//! the ≤760 px media query narrows to 16 px / 12 px (`styles.css:2110-2113`,
//! `:2133-2136`). The assistant's paragraphs are capped at `75ch`
//! (`Timeline.module.css:28-30`), the user bubble at `min(680px, 82%)`
//! (`styles.css:700-707`).
//!
//! [`Metrics::for_window`] is that arithmetic, pure, so the numbers are
//! testable without a window; `lib.rs` applies them to the live tree and the
//! row builders ([`crate::fluid`]) read the resulting widths.
use std::sync::atomic::{AtomicU64, Ordering};

/// The live sidebar's seat (`lib.rs` `sidebar_spacer`: A3's column at the
/// web's 280 px plus its 1 px rule).
pub const SIDEBAR_W: f64 = 281.0;
/// The first-run screen's empty sidebar (`first_run_sidebar`, board 4
/// frame 4: 260 px) plus its 1 px rule.
pub const FIRST_RUN_SIDEBAR_W: f64 = 261.0;
/// Below this module width the sidebar hides and the phone density applies
/// (`lib.rs` `width_hides_sidebar`, the web's `@media (max-width: 760px)`).
pub const PHONE_BREAKPOINT: f64 = 760.0;

/// Text and spacing density: the web's desktop numbers, or the approved
/// phone boards' (the 406 px artboards are the phone design).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Density {
    Desktop,
    Phone,
}

/// The conversation pane's geometry for one module-window width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub density: Density,
    /// The pane the conversation column lives in (window minus the sidebar
    /// when it shows).
    pub pane_w: f64,
    /// The transcript column's content width (`--dsw-layout-chat`, bounded
    /// by the pane minus its side padding).
    pub column_w: f64,
    /// Side padding of the transcript (24 desktop / 16 phone).
    pub column_pad: f64,
    /// The composer's width (`--dsw-layout-chat-wide`, same bound).
    pub composer_w: f64,
    /// Side padding of the composer dock (24 desktop / 12 phone).
    pub composer_pad: f64,
    /// The assistant prose's reading measure (`75ch` at the prose size).
    pub prose_max_w: f64,
    /// The user bubble's cap (`min(680px, 82%)` of the column).
    pub bubble_max_w: f64,
}

impl Metrics {
    /// The geometry for a module window `window_w` wide. `sidebar_shown`
    /// says whether the 260 px thread column takes part of it.
    pub fn for_window(window_w: f64, sidebar_shown: bool) -> Metrics {
        let window_w = if window_w > 0.0 { window_w } else { 990.0 };
        let phone = window_w < PHONE_BREAKPOINT;
        let pane_w = if sidebar_shown && !phone {
            (window_w - SIDEBAR_W).max(1.0)
        } else {
            window_w
        };
        Metrics::for_geometry(window_w, pane_w)
    }

    /// The geometry for a module window `window_w` wide whose conversation
    /// pane MEASURED `pane_w` (the live layout: the sidebar, a docked review
    /// panel or the settings drawer all narrow the pane, and the window width
    /// alone cannot know it). `vw` stays the window's, as in the browser.
    pub fn for_geometry(window_w: f64, pane_w: f64) -> Metrics {
        let window_w = if window_w > 0.0 { window_w } else { 990.0 };
        let pane_w = if pane_w > 0.0 { pane_w } else { window_w };
        let phone = window_w < PHONE_BREAKPOINT;
        let (column_pad, composer_pad) = if phone { (16.0, 12.0) } else { (24.0, 24.0) };
        // clamp(736px, 62vw, 1040px) — `vw` is the whole window, as in the
        // browser (the sidebar is part of the viewport there too).
        let chat = (0.62 * window_w).clamp(736.0, 1040.0);
        let column_w = chat.min(pane_w - 2.0 * column_pad).max(1.0);
        let composer_w = (chat + 32.0).min(pane_w - 2.0 * composer_pad).max(1.0);
        // 75ch at 15 px Inter (`0` advances 0.62 em ≈ 9.3 px) ≈ 700 px on the
        // desktop; the phone column is always narrower than the measure.
        let prose_max_w = if phone { column_w } else { column_w.min(700.0) };
        let bubble_max_w = (0.82 * column_w).min(680.0).floor();
        Metrics {
            density: if phone { Density::Phone } else { Density::Desktop },
            pane_w,
            column_w: column_w.floor(),
            column_pad,
            composer_w: composer_w.floor(),
            composer_pad,
            prose_max_w: prose_max_w.floor(),
            bubble_max_w,
        }
    }

    /// The transcript rows' side padding: centres the column in the pane
    /// with at least the minimum gutter (`max(24px, (100% - chat) / 2)`).
    pub fn column_side_pad(&self) -> f64 {
        centring_pad(self.pane_w, self.column_w, self.column_pad)
    }

    /// The composer dock's side padding (same rule, the wider composer).
    pub fn composer_side_pad(&self) -> f64 {
        centring_pad(self.pane_w, self.composer_w, self.composer_pad)
    }
}

/// The side padding that centres a `content_w` box in `pane_w` with at
/// least `min_pad` on each side (the web's `max(24px, (100% - w) / 2)`).
pub fn centring_pad(pane_w: f64, content_w: f64, min_pad: f64) -> f64 {
    ((pane_w - content_w) / 2.0).max(min_pad).floor()
}

/// The metrics the row builders lower against, set by `lib.rs` from the
/// MEASURED module window and conversation pane. Stored as the two inputs
/// (not the derived numbers) so a reader always sees one consistent
/// [`Metrics`].
static WINDOW_W_BITS: AtomicU64 = AtomicU64::new(0);
static PANE_W_BITS: AtomicU64 = AtomicU64::new(0);

/// Record the live module-window and conversation-pane widths. Returns true
/// when the stored geometry changed by at least half a pixel (the caller
/// re-lowers and re-lays out).
pub fn set_geometry(window_w: f64, pane_w: f64) -> bool {
    let old_w = f64::from_bits(WINDOW_W_BITS.load(Ordering::Relaxed));
    let old_p = f64::from_bits(PANE_W_BITS.load(Ordering::Relaxed));
    if (old_w - window_w).abs() < 0.5 && (old_p - pane_w).abs() < 0.5 {
        return false;
    }
    WINDOW_W_BITS.store(window_w.to_bits(), Ordering::Relaxed);
    PANE_W_BITS.store(pane_w.to_bits(), Ordering::Relaxed);
    true
}

/// The current geometry (the default desktop before any layout reported).
pub fn current() -> Metrics {
    let w = f64::from_bits(WINDOW_W_BITS.load(Ordering::Relaxed));
    let p = f64::from_bits(PANE_W_BITS.load(Ordering::Relaxed));
    if w <= 0.0 {
        return Metrics::for_window(990.0, true);
    }
    Metrics::for_geometry(w, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The operator's desktop: a ~990 px module window in the 1400x900 shell.
    /// The web's column is `clamp(736, 62vw, 1040)` = 736, bounded by the
    /// 709 px pane (990 minus the 281 px sidebar seat) minus 2x24 -> 661:
    /// the transcript fills the pane with a 24 px gutter instead of the
    /// 356 px phone column.
    #[test]
    fn the_default_desktop_window_fills_the_pane_with_the_web_gutter() {
        let m = Metrics::for_window(990.0, true);
        assert_eq!(m.density, Density::Desktop);
        assert_eq!(m.pane_w, 709.0);
        assert_eq!(m.column_w, 661.0);
        assert_eq!(m.composer_w, 661.0, "the composer is bounded by the same gutter");
        assert!(m.column_w > 2.0 * 300.0, "no phone-width column on the desktop");
        assert_eq!(m.bubble_max_w, (0.82f64 * 661.0).floor());
    }

    /// A larger module (the maximized 1376 px window): 62vw = 853 px wins the
    /// clamp, the composer is 32 px wider (`--dsw-layout-chat-wide`), and the
    /// prose keeps its 75ch reading measure.
    #[test]
    fn a_large_window_centres_a_62vw_column_and_a_wider_composer() {
        let m = Metrics::for_window(1376.0, true);
        assert_eq!(m.column_w, (0.62f64 * 1376.0).floor());
        assert_eq!(m.composer_w, (0.62f64 * 1376.0 + 32.0).floor());
        assert_eq!(m.prose_max_w, 700.0);
        assert_eq!(m.bubble_max_w, 680.0, "min(680px, 82%)");
        let side = (m.pane_w - m.column_w) / 2.0;
        assert!(side > 24.0, "centred with more than the minimum gutter: {side}");
    }

    /// The phone (≤760): no sidebar, the web's 16 px transcript and 12 px
    /// composer gutters.
    #[test]
    fn the_phone_uses_the_narrow_gutters_and_the_phone_density() {
        let m = Metrics::for_window(360.0, true);
        assert_eq!(m.density, Density::Phone);
        assert_eq!(m.pane_w, 360.0, "the sidebar never takes phone width");
        assert_eq!(m.column_w, 328.0);
        assert_eq!(m.composer_w, 336.0);
        assert_eq!(m.prose_max_w, m.column_w);
    }

    #[test]
    fn an_unreported_window_falls_back_to_the_default_desktop() {
        assert_eq!(Metrics::for_window(0.0, true), Metrics::for_window(990.0, true));
    }
}
