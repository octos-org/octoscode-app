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

/// Every alias, grouped by surface. "generic" marks an entry whose web
/// control is worded around the browser (or names no exact counterpart), so
/// the alias is the web's generic key for the same action — still the web's
/// own translation.
pub static ALIASES: &[Alias] = &[
    // ---- the sidebar (board 2 screens 1-5; web features/shell/ProductSidebar.tsx)
    a("New chat", "New Session", "features/shell/ProductSidebar.tsx:558"),
    a("Search chats", "Search sessions", "features/shell/ProductSidebar.tsx:568"),
    // generic: the workspace menu's new-session item (the menu names the workspace)
    a("New chat here", "New session", "features/shell/ProductSidebar.tsx:553"),
    a("By workspace", "Workspace", "features/shell/ProductSidebar.tsx:1107"),
    a("All", "In one list", "features/shell/ProductSidebar.tsx:1117"),
    a("No chats yet.", "No sessions yet.", "features/shell/ProductSidebar.tsx:848"),
    // the first-run column's empty state (lib.rs), the same sidebar empty state
    a("No threads yet", "No sessions yet.", "features/shell/ProductSidebar.tsx:938"),
    // ---- the header (web app/App.tsx conversation header)
    a("Review", "Review changes", "app/App.tsx:2442"),
    // ---- Settings (web features/product-settings, session-config, connection)
    a("Stop server\u{2026}", "Stop server", "features/product-settings/GeneralSettingsContent.tsx:332"),
    a("Advanced\u{2026}", "Advanced", "features/session-config/SessionConfigPane.tsx:367"),
    // generic: the web's button is "Save browser preferences" (names the browser)
    a("Save preferences", "Save", "features/product-settings/ModelManagementSection.tsx:914"),
    // the server-address row (About; the Connect card and pairing's field)
    a("Server", "Server origin", "features/connection/ConnectionPanel.tsx:199"),
    a("Server defaults", "server default", "features/autonomy/AutonomyPanel.tsx:126"),
    // generic: the failure line after a refused preset save
    a("Failed: {value0}", "Couldn't save: {value0}", "features/session-config/session-config-copy.ts:23"),
    a("not offered by this server", "Not supported by this server", "features/session-config/sandbox-section.tsx:4"),
    a("not connected", "Not connected", "app/App.tsx:3460"),
    // ---- the slash menu: each command's description (registry.ts, CommandPalette.tsx:77 t(description))
    a("Manage session monitors", "Inspect and manage session monitors", "features/commands/registry.ts:435"),
    a("Ask a side question", "Ask a temporary side question without changing the main turn", "features/commands/registry.ts:188"),
    a("Run native code review", "Run the server\u{2019}s native code-review workflow", "features/commands/registry.ts:123"),
    a("Stop the active turn", "Stop the active foreground turn", "features/commands/registry.ts:274"),
    a("Manage installed skills", "Manage installed skills and registry packages in this Profile", "features/commands/registry.ts:368"),
    a("Inspect and manage the goal", "Inspect and manage the session goal", "features/commands/registry.ts:401"),
    a("Inspect and manage loops", "Inspect and manage scheduled loops", "features/commands/registry.ts:420"),
    a("Rewind to an earlier turn", "Rewind this conversation to an earlier user turn", "features/commands/registry.ts:150"),
    a("Fork the conversation", "Fork conversation", "features/history/HistoryDialog.tsx:79"),
    a("Inspect the thread graph", "Inspect the server\u{2019}s native thread graph", "features/commands/registry.ts:197"),
    a("Set thinking effort", "Set thinking effort for this Session\u{2019}s new prompts", "features/commands/registry.ts:248"),
    a("Attach images", "Attach images to this Session\u{2019}s next prompt", "features/commands/registry.ts:257"),
    a("Inspect tool availability", "Inspect server-owned tool availability and policy", "features/commands/registry.ts:349"),
    a("Inspect MCP connections", "Inspect server-reported MCP connections", "features/commands/registry.ts:358"),
    // ---- the composer, its strip and the connection banner
    a("Ask Octos anything", "Ask Octos to change, explain, or review code\u{2026}", "app/App.tsx:2943"),
    // generic: the queued chip's steer action (the Fleet's Steer)
    a("Steer now", "Steer", "features/fleet/FleetView.tsx:730"),
    a("Reconnecting\u{2026}", "Reconnecting", "features/session-config/SessionStatusStrip.tsx:68"),
    a("Running", "running", "features/timeline/Timeline.tsx:401"),
    // generic: the banner's title and its retry action
    a("Not connected to Octos", "Not connected", "app/App.tsx:3460"),
    a("Retry now", "Retry", "features/shell/ProductSidebar.tsx:696"),
    // ---- the Connect card and pairing (web features/connection/ConnectionPanel.tsx)
    a("Access token", "Auth token", "features/connection/ConnectionPanel.tsx:246"),
    a("That pairing link didn\u{2019}t work.", "That pairing link did not work", "features/connection/ConnectionPanel.tsx:174"),
    // ---- board 1's workspace picker (web features/workspace-create)
    a("Server folder", "Server's working directory", "features/workspace-create/server-working-directory.ts:14"),
    a(
        "Server folder (path not reported)",
        "Server's working directory (path not reported)",
        "features/workspace-create/server-working-directory.ts:21",
    ),
    a("No recent workspaces yet.", "No recent workspace paths", "features/workspace-create/NewSessionWorkspacePicker.tsx:287"),
    a("Browse folders\u{2026}", "Browse\u{2026}", "features/workspace-create/NewSessionWorkspacePicker.tsx:360"),
];

/// The web key a native string renders through, if it is an alias.
pub fn web_key(native: &str) -> Option<&'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| ALIASES.iter().map(|x| (x.native, x.web)).collect())
        .get(native)
        .copied()
}
