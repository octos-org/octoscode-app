//! #31e — keyboard parity: the web's keyboard model resolved ONCE.
//!
//! The shell's `Event::KeyDown` arm (lib.rs) feeds each key event through
//! [`resolve`] and performs the resulting [`KeyAction`]. Every rule below
//! carries its web citation; the deltas from the typed `resolve` are the
//! contract the tests pin (`tests/f31e_keys.rs`).
//!
//! * Enter sends / Shift+Enter newlines — `ComposerInput.tsx:237-243` (the
//!   send guard is `!shiftKey && !ctrlKey && !metaKey`; Shift+Enter falls
//!   through to the multiline input's own newline, which is makepad
//!   `TextInput`'s default on Return).
//! * Esc — closes the palette overlay (`CommandPalette.tsx:40-43`), else
//!   interrupts the live turn (`ComposerInput.tsx:245-252`; the approval
//!   card's Escape is the same interrupt, `ApprovalPanel.tsx:38-39`).
//! * Cmd/Ctrl+K toggles the palette (`App.tsx:1096-1105` accepts meta OR
//!   ctrl); "/" on an EMPTY draft opens it (#28e item 5 — the composer
//!   `changed` action already does this; the key path matches).
//! * ↑/↓ move, Enter runs, in the palette (`CommandPalette.tsx:44-49` and the
//!   listbox's Enter → run).
//! * Alt+A is the web's show-approval parity shortcut (`registry.ts:611-616`:
//!   Alt REQUIRED, Ctrl/Meta rejected at `:633`).
//! * Y/S/N decide the pending approval — approve/request, approve/session,
//!   deny/request (`ApprovalPanel.tsx:46-54`), and ONLY bare keys decide
//!   (`:37-43` rejects Ctrl/Meta/Alt).
use makepad_widgets::KeyCode;

use crate::Store;

/// What one key event means. Pure data — [`resolve`] decides, the shell
/// performs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// Send the draft (the composer's bare Enter).
    ComposerSubmit,
    /// Close the palette overlay (Esc while open).
    PaletteClose,
    /// Toggle the palette (Cmd/Ctrl+K).
    PaletteToggle,
    /// Open the palette ("/" on an empty draft — idempotent set, because the
    /// composer's `changed` action performs the same open (#28e item 5)).
    PaletteOpen,
    /// Move the palette selection (↑/↓ while open); carries the direction.
    PaletteMove(isize),
    /// Run the palette's selected command (Enter while open).
    PaletteRun,
    /// Interrupt the live turn (Esc, no overlay) — the wire frame is
    /// "turn/interrupt" (Conversation::interrupt; the web's
    /// ComposerInput.tsx:245-252, and UserQuestionPanel.tsx:79 keeps the
    /// same Escape route while a question card waits — #P4d1 row 147).
    Interrupt,
    /// Y — approve for this request only (`ApprovalPanel.tsx:46-48`).
    ApprovalApproveRequest,
    /// S — approve for the whole session (`ApprovalPanel.tsx:49-51`).
    ApprovalApproveSession,
    /// N — deny the request (`ApprovalPanel.tsx:52-54`).
    ApprovalDenyRequest,
    /// Alt+A — the web's show-approval parity shortcut (`registry.ts:614`).
    /// The native approval surface lands with the approval Stage-C screen;
    /// the binding and its gate are wired and observable now.
    ShowApproval,
    /// #28e's existing chrome chords, kept in the table so one resolver owns
    /// the whole KeyDown surface.
    ReviewToggle,
    SettingsToggle,
    /// Not ours (typing, Shift+Enter's newline, unknown chords) — the event
    /// falls through untouched.
    Ignore,
}

/// Resolve one key event. `mods` are the four booleans the platform carries
/// (`RemoteKeyModifiers`); the UI state (`palette_open`, `approval_pending`,
/// `turn_active`, `draft_empty`) comes from the shell's `FlowUi`.
pub fn resolve(
    key_code: KeyCode,
    shift: bool,
    ctrl: bool,
    alt: bool,
    logo: bool,
    palette_open: bool,
    approval_pending: bool,
    turn_active: bool,
    draft_empty: bool,
) -> KeyAction {
    match key_code {
        KeyCode::ReturnKey if !ctrl && !alt && !logo => {
            if shift {
                // :237's `!shiftKey` guard — Shift+Enter is the newline; the
                // multiline TextInput does it by default, we never intercept.
                KeyAction::Ignore
            } else if palette_open {
                // The palette listbox's Enter runs the selection.
                KeyAction::PaletteRun
            } else {
                // :237-243 — bare Enter sends.
                KeyAction::ComposerSubmit
            }
        }
        KeyCode::Escape if !ctrl && !alt && !logo => {
            if palette_open {
                // CommandPalette.tsx:40-43.
                KeyAction::PaletteClose
            } else if turn_active {
                // ComposerInput.tsx:245-252 — bare Esc interrupts.
                KeyAction::Interrupt
            } else {
                KeyAction::Ignore
            }
        }
        KeyCode::KeyK if (logo || ctrl) && !alt && !shift => {
            // App.tsx:1096 — (metaKey || ctrlKey) && "k".
            KeyAction::PaletteToggle
        }
        KeyCode::Slash if !shift && !ctrl && !alt && !logo && !palette_open && draft_empty => {
            // The composer's "/" affordance (#28e item 5) is also a KEY
            // binding: an empty draft's "/" opens, never types. OPEN (not
            // toggle): the composer's changed-action may fire the same open.
            KeyAction::PaletteOpen
        }
        KeyCode::ArrowUp if !shift && !ctrl && !alt && !logo && palette_open => {
            KeyAction::PaletteMove(-1) // CommandPalette.tsx:44-49
        }
        KeyCode::ArrowDown if !shift && !ctrl && !alt && !logo && palette_open => {
            KeyAction::PaletteMove(1)
        }
        KeyCode::KeyA if alt && !ctrl && !logo => {
            // registry.ts:614 — Alt REQUIRED; :633 rejects Ctrl/Meta (so
            // Cmd+Alt+A stays inert rather than shadowing a text chord).
            KeyAction::ShowApproval
        }
        KeyCode::KeyY if !shift && !ctrl && !alt && !logo => {
            // ApprovalPanel.tsx:46-48 — bare keys only (:37-43).
            if approval_pending {
                KeyAction::ApprovalApproveRequest
            } else {
                KeyAction::Ignore
            }
        }
        KeyCode::KeyS if !shift && !ctrl && !alt && !logo => {
            // :49-51.
            if approval_pending {
                KeyAction::ApprovalApproveSession
            } else {
                KeyAction::Ignore
            }
        }
        KeyCode::KeyN if !shift && !ctrl && !alt && !logo => {
            // :52-54.
            if approval_pending {
                KeyAction::ApprovalDenyRequest
            } else {
                KeyAction::Ignore
            }
        }
        // #28e's chrome chords, preserved verbatim.
        KeyCode::KeyE if logo => KeyAction::ReviewToggle,
        KeyCode::Period if logo => KeyAction::SettingsToggle,
        _ => KeyAction::Ignore,
    }
}

/// The oldest actionable pending approval's id — the id a keyboard decision
/// answers (the web decides the card that is showing; natively the store's
/// pending list is FIFO, `domains/approval.rs:79`).
pub fn oldest_pending_id(store: &Store) -> Option<String> {
    store
        .domains
        .approval
        .pending()
        .into_iter()
        .find(|a| !a.decided && !a.cancelled)
        .map(|a| a.id)
}

/// The outbound `approval/respond` body for a keyboard decision — the r5-turn
/// recording's exact grammar (`approval/respond {approval_id, decision,
/// session_id}`; octos-core `ui_protocol.rs:2135-2143`, the optional
/// `approval_scope` carries the web's request/session distinction).
pub fn respond_body(
    action: &KeyAction,
    session_id: &str,
    approval_id: &str,
) -> Option<serde_json::Value> {
    let (decision, scope) = match action {
        KeyAction::ApprovalApproveRequest => ("approve", Some("request")),
        KeyAction::ApprovalApproveSession => ("approve", Some("session")),
        KeyAction::ApprovalDenyRequest => ("deny", Some("request")),
        _ => return None,
    };
    Some(serde_json::json!({
        "session_id": session_id,
        "approval_id": approval_id,
        "decision": decision,
        "approval_scope": scope,
    }))
}
