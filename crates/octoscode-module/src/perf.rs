//! A35b — what one UI event costs, measured on the running app.
//!
//! The operator found the folder browser "super slow" on the standalone app.
//! The Makepad instrument's `[ui-hang]` sampler names an event that ran long
//! and its top stack frames; this names WHY in the app's own terms:
//! `OCTOSCODE_PERF=1` logs one line per handled event (and per draw) that
//! took a millisecond or more — its wall time, the widget-tree path lookups
//! it made and how many of them walked the tree (path-cache misses and the
//! nodes visited), and the board-1 composes and mounts it ran:
//!
//! ```text
//! [octoscode] perf: Signal 3.1 ms @5120.4 lookups=+212 misses=+4 walked=+880 inval=+1 unstored=+0 b1_compose=+1 b1_mount=+1
//! ```
//!
//! `@` is when the event ENDED, in ms since the probe's first event: a walk
//! reads a step's latency off the app's own clock (from the input's event
//! start to the draw that showed the result) without polling the instrument
//! in between.
//!
//! Off (the default) it is one cached bool per event. The board-1 counters
//! are always kept (two relaxed atomic adds per compose/mount): the walk
//! and the tests read them to pin "one remount per navigation, none per
//! keystroke".
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use makepad_widgets::widget_tree::WidgetTreeStats;
use makepad_widgets::*;

/// `OCTOSCODE_PERF` is set (and not `0`).
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("OCTOSCODE_PERF").map(|v| !v.is_empty() && v != "0").unwrap_or(false))
}

static BOARD1_COMPOSES: AtomicU64 = AtomicU64::new(0);
static BOARD1_MOUNTS: AtomicU64 = AtomicU64::new(0);

/// Board 1 lowered its open surface to DSL (`board1::view` rebuilt it).
pub fn note_board1_compose() {
    BOARD1_COMPOSES.fetch_add(1, Ordering::Relaxed);
}

/// Board 1's dock was really remounted (the DSL changed).
pub fn note_board1_mount() {
    BOARD1_MOUNTS.fetch_add(1, Ordering::Relaxed);
}

/// (composes, mounts) since the process started.
pub fn board1_counts() -> (u64, u64) {
    (BOARD1_COMPOSES.load(Ordering::Relaxed), BOARD1_MOUNTS.load(Ordering::Relaxed))
}

/// One measured event.
pub struct Span {
    what: &'static str,
    t0: Instant,
    stats: WidgetTreeStats,
    board1: (u64, u64),
}

/// The name a logged event goes by (`None`: not worth a line — hover moves,
/// frame ticks — unless it ran long).
fn name(event: &Event) -> &'static str {
    match event {
        Event::Signal => "Signal",
        Event::Actions(_) => "Actions",
        Event::KeyDown(_) => "KeyDown",
        Event::KeyUp(_) => "KeyUp",
        Event::TextInput(_) => "TextInput",
        Event::MouseDown(_) => "MouseDown",
        Event::MouseUp(_) => "MouseUp",
        Event::MouseMove(_) => "MouseMove",
        Event::Scroll(_) => "Scroll",
        Event::Timer(_) => "Timer",
        Event::NextFrame(_) => "NextFrame",
        Event::WindowGeomChange(_) => "WindowGeomChange",
        Event::Draw(_) => "Draw",
        _ => "event",
    }
}

/// Start measuring `event` (`None` when the probe is off).
pub fn begin(cx: &Cx, event: &Event) -> Option<Span> {
    begin_named(cx, name(event))
}

/// Start measuring a named phase (the module's draw).
pub fn begin_named(cx: &Cx, what: &'static str) -> Option<Span> {
    if !enabled() {
        return None;
    }
    let _ = epoch();
    Some(Span { what, t0: Instant::now(), stats: cx.widget_tree().stats(), board1: board1_counts() })
}

/// The probe's clock origin (its first event).
fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

/// Log the span when it took 1 ms or more (every Actions and Draw: a walk
/// reads a step's latency from them).
pub fn end(cx: &Cx, span: Option<Span>) {
    let Some(span) = span else { return };
    let ms = span.t0.elapsed().as_secs_f64() * 1000.0;
    if ms < 1.0 && !matches!(span.what, "Actions" | "Draw") {
        return;
    }
    let at = epoch().elapsed().as_secs_f64() * 1000.0;
    let now = cx.widget_tree().stats();
    let (composes, mounts) = board1_counts();
    makepad_widgets::log!(
        "[octoscode] perf: {} {:.1} ms @{:.1} lookups=+{} misses=+{} walked=+{} inval=+{} unstored=+{} b1_compose=+{} b1_mount=+{}",
        span.what,
        ms,
        at,
        now.lookups - span.stats.lookups,
        now.cache_misses - span.stats.cache_misses,
        now.walk_nodes - span.stats.walk_nodes,
        now.invalidations - span.stats.invalidations,
        now.stores_skipped - span.stats.stores_skipped,
        composes - span.board1.0,
        mounts - span.board1.1,
    );
}
