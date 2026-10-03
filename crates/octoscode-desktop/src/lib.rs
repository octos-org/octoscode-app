//! Native OctosCode desktop UI, independent of the OctoSense shell and module host.

pub use octoscode_module::{create_view, register_widgets};

pub const APP_NAME: &str = "OctosCode";
pub const DEFAULT_WINDOW_SIZE: (f64, f64) = (1280.0, 800.0);

/// `WxH` (`1280x800`, `360X780`) as logical pixels; `None` for anything else.
pub fn parse_window_size(spec: &str) -> Option<(f64, f64)> {
    let (w, h) = spec.trim().split_once(['x', 'X'])?;
    let (w, h) = (w.trim().parse::<f64>().ok()?, h.trim().parse::<f64>().ok()?);
    (w.is_finite() && h.is_finite() && w >= 200.0 && h >= 120.0).then_some((w, h))
}

/// The size the window asks for: `OCTOSENSE_WINDOW_SIZE` (the size the module
/// itself treats as authoritative, `lib.rs` env_frame), then
/// `OCTOSCODE_WINDOW_SIZE`, then [`DEFAULT_WINDOW_SIZE`].
pub fn window_size_from(octosense: Option<&str>, octoscode: Option<&str>) -> (f64, f64) {
    octosense
        .and_then(parse_window_size)
        .or_else(|| octoscode.and_then(parse_window_size))
        .unwrap_or(DEFAULT_WINDOW_SIZE)
}

/// [`window_size_from`] over this process's environment.
pub fn window_size() -> (f64, f64) {
    window_size_from(
        std::env::var("OCTOSENSE_WINDOW_SIZE").ok().as_deref(),
        std::env::var("OCTOSCODE_WINDOW_SIZE").ok().as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_sizes_parse_and_fall_back() {
        assert_eq!(parse_window_size("360x780"), Some((360.0, 780.0)));
        assert_eq!(parse_window_size(" 1400X900 "), Some((1400.0, 900.0)));
        for bad in ["", "x", "360", "360x", "axb", "10x10", "nanxinf"] {
            assert_eq!(parse_window_size(bad), None, "{bad:?}");
        }
        assert_eq!(window_size_from(None, None), DEFAULT_WINDOW_SIZE);
        assert_eq!(
            window_size_from(Some("360x780"), Some("990x600")),
            (360.0, 780.0)
        );
        assert_eq!(
            window_size_from(Some("junk"), Some("990x600")),
            (990.0, 600.0)
        );
    }
}
