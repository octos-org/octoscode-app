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
//! A native string with no web counterpart is not listed here: its reviewed
//! Chinese lives in the native-only supplement (`native.rs`, consulted after
//! this table). An alias is kept only when the web key's Chinese is a correct
//! rendering of the native text; a looser "generic" match is a native entry.
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
    a("Search chats", "Search sessions", "features/shell/ProductSidebar.tsx:568"),
    a("By workspace", "Workspace", "features/shell/ProductSidebar.tsx:1107"),
    a("All", "In one list", "features/shell/ProductSidebar.tsx:1117"),
    a("No chats yet.", "No sessions yet.", "features/shell/ProductSidebar.tsx:848"),
    // the first-run column's empty state (lib.rs), the same sidebar empty state
    a("No threads yet", "No sessions yet.", "features/shell/ProductSidebar.tsx:938"),
    // ---- the header (web app/App.tsx conversation header)
    a("Review", "Review changes", "app/App.tsx:2446"),
    // ---- Settings (web features/product-settings, session-config, connection)
    a("Stop server\u{2026}", "Stop server", "features/product-settings/GeneralSettingsContent.tsx:332"),
    a("Advanced\u{2026}", "Advanced", "features/session-config/SessionConfigPane.tsx:367"),
    // the server-address row (About; the Connect card and pairing's field)
    a("Server", "Server origin", "features/connection/ConnectionPanel.tsx:199"),
    a("Server defaults", "server default", "features/autonomy/AutonomyPanel.tsx:126"),
    a("not offered by this server", "Not supported by this server", "features/session-config/sandbox-section.tsx:33"),
    a("not connected", "Not connected", "features/product-settings/GeneralSettingsContent.tsx:158"),
    // ---- the slash menu: each command's description (registry.ts, CommandPalette.tsx:77 t(description))
    a("Manage session monitors", "Inspect and manage session monitors", "features/commands/registry.ts:435"),
    a("Ask a side question", "Ask a temporary side question without changing the main turn", "features/commands/registry.ts:188"),
    a("Run native code review", "Run the server\u{2019}s native code-review workflow", "features/commands/registry.ts:123"),
    a("Stop the active turn", "Stop the active foreground turn", "features/commands/registry.ts:274"),
    a("Manage installed skills", "Manage installed skills and registry packages in this Profile", "features/commands/registry.ts:368"),
    a("Inspect and manage the goal", "Inspect and manage the session goal", "features/commands/registry.ts:401"),
    a("Inspect and manage loops", "Inspect and manage scheduled loops", "features/commands/registry.ts:420"),
    a("Rewind to an earlier turn", "Rewind this conversation to an earlier user turn", "features/commands/registry.ts:150"),
    a("Fork the conversation", "Copy this conversation to a separate Session", "features/commands/registry.ts:164"),
    a("Show model settings", "Show runtime model and Profile model settings", "features/commands/registry.ts:323"),
    a("Context and compaction", "Inspect context, cache, and compaction controls", "features/commands/registry.ts:332"),
    a("Resume a session", "Browse unverified historical candidates and confirm a Session", "features/commands/registry.ts:454"),
    a("Remembered decisions", "Inspect remembered decisions; Session controls set approval mode", "features/commands/registry.ts:230"),
    a("Inspect the thread graph", "Inspect the server\u{2019}s native thread graph", "features/commands/registry.ts:197"),
    a("Set thinking effort", "Set thinking effort for this Session\u{2019}s new prompts", "features/commands/registry.ts:248"),
    a("Attach images", "Attach images to this Session\u{2019}s next prompt", "features/commands/registry.ts:257"),
    a("Inspect tool availability", "Inspect server-owned tool availability and policy", "features/commands/registry.ts:349"),
    a("Inspect MCP connections", "Inspect server-reported MCP connections", "features/commands/registry.ts:358"),
    // ---- the composer, its strip and the connection banner
    a("Reconnecting\u{2026}", "Reconnecting", "features/session-config/SessionStatusStrip.tsx:68"),
    a("Running", "running", "features/timeline/Timeline.tsx:401"),
    // ---- the Connect card and pairing (web features/connection/ConnectionPanel.tsx)
    a("Access token", "Auth token", "features/connection/ConnectionPanel.tsx:246"),
    a("That pairing link didn\u{2019}t work.", "That pairing link did not work", "features/connection/ConnectionPanel.tsx:174"),
    // ---- board 1's workspace picker (web features/workspace-create; the
    // web renders the server row's label without t(), its zh entry is the
    // web's own translation of the same row)
    a("Server folder", "Server's working directory", "features/workspace-create/server-working-directory.ts:14"),
    a(
        "Server folder (path not reported)",
        "Server's working directory (path not reported)",
        "features/workspace-create/server-working-directory.ts:21",
    ),
    a("No recent workspaces yet.", "No recent workspace paths", "features/workspace-create/NewSessionWorkspacePicker.tsx:287"),
    a("Browse folders\u{2026}", "Browse\u{2026}", "features/workspace-create/NewSessionWorkspacePicker.tsx:360"),
    // ---- the thinking-effort dialog (web features/reasoning/ReasoningDialog.tsx;
    // the board's segment says "Max" for REASONING_CHOICES' "Maximum")
    a("Max", "Maximum", "features/reasoning/ReasoningDialog.tsx:80"),
];

/// The web key a native string renders through, if it is an alias.
pub fn web_key(native: &str) -> Option<&'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| ALIASES.iter().map(|x| (x.native, x.web)).collect())
        .get(native)
        .copied()
}
