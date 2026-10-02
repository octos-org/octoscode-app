//! A24 — native copy that names the SAME control as a web string worded
//! differently.
//!
//! The approved boards word some controls their own way (board 2's
//! "New chat" row is the web sidebar's "New Session",
//! `ProductSidebar.tsx:558`). In Chinese such a control shows the web's own
//! translation of the web's key — never a new translation. Each entry is the
//! native English, the web key, and the web call site that renders that key
//! for the same control; a test proves every key is in the web catalog and
//! that both sides carry the same placeholders.
//!
//! A native string with no web counterpart is not listed here: it stays
//! English in Chinese and is reported as copy without a web key.
use std::collections::HashMap;
use std::sync::OnceLock;

/// One native string and the web key of the same control.
#[derive(Debug, Clone, Copy)]
pub struct Alias {
    pub native: &'static str,
    pub web: &'static str,
    /// The web call site (`apps/web/src/...:line`) rendering `web` for the
    /// same control.
    pub cite: &'static str,
}

const fn a(native: &'static str, web: &'static str, cite: &'static str) -> Alias {
    Alias { native, web, cite }
}

/// Every alias, grouped by surface.
pub static ALIASES: &[Alias] = &[
    // ---- the sidebar (board 2 screens 1-5; web features/shell/ProductSidebar.tsx)
    a("New chat", "New Session", "features/shell/ProductSidebar.tsx:558"),
];

/// The web key a native string renders through, if it is an alias.
pub fn web_key(native: &str) -> Option<&'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| ALIASES.iter().map(|x| (x.native, x.web)).collect())
        .get(native)
        .copied()
}
