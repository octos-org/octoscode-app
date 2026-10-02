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
    /// D — open the diff review for the SHOWING approval's preview
    /// (`ApprovalPanel.tsx:45`: `key === "d" && previewId && onReviewDiff`).
    /// Carries the preview id read from the approval payload
    /// (`typedDetails.diff.preview_id`, #P4f2 row 7). Only produced when a
    /// preview id exists, so a non-diff approval leaves D inert exactly as
    /// the web does.
    ApprovalReviewDiff(String),
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
    // #P4f2 row 7: the SHOWING approval's diff preview id, read by the caller
    // from the store (`Approvals::oldest_preview_id`). Passed in rather than
    // read here so `resolve` stays a PURE function over its arguments — the
    // same shape every other rule in this table already has.
    approval_preview: Option<String>,
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
        KeyCode::KeyD if !shift && !ctrl && !alt && !logo => {
            // #P4f2 row 7 — `ApprovalPanel.tsx:45`: D opens the diff review,
            // but ONLY when the showing approval carries a preview id. The web
            // reads it off the card; natively the store's oldest ACTIONABLE
            // pending approval is the card that is showing (FIFO,
            // `domains/approval.rs`), so its preview id is the one to use.
            // A non-diff approval leaves D inert — same as `previewId` absent.
            //
            // The web's other two early-returns (`:36-38`): `busy` is already
            // covered by `approval_pending`, and the IME arm
            // (`isComposing` / `keyCode === 229`) has NO native input — the
            // pinned makepad `KeyEvent` carries only
            // {key_code, is_repeat, modifiers, time}
            // (platform/studio/src/keyboard.rs:7-12) and exposes no composing
            // flag anywhere in the tree, so there is no value to read. Recorded
            // as unported in .peer/report-P4f2.md rather than silently claimed.
            match approval_preview {
                Some(preview_id) if approval_pending => KeyAction::ApprovalReviewDiff(preview_id),
                _ => KeyAction::Ignore,
            }
        }
        // #28e's chrome chords, preserved verbatim.
        KeyCode::KeyE if logo => KeyAction::ReviewToggle,
        KeyCode::Period if logo => KeyAction::SettingsToggle,
        _ => KeyAction::Ignore,
    }
}

/// A7 — the §8 suppression facts (`composer/shortcut-suppression.ts:13-24`),
/// read STRUCTURALLY from the widget tree by the host: whether the key focus
/// sits in a text-entry control (a `TextInput`), and whether a modal dialog
/// is open (the board-1 / board-3 / A5 dialogs, Settings, the palette).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShortcutFacts {
    pub target_is_text_input: bool,
    pub in_dialog: bool,
}

/// `shortcutSuppressed` (`shortcut-suppression.ts:21-23`): a parity chord must
/// NOT fire while either fact holds (no action, no focus steal — the text
/// control or the dialog owns the key).
pub fn shortcut_suppressed(f: ShortcutFacts) -> bool {
    f.target_is_text_input || f.in_dialog
}

/// The web's keyboard parity chords (`commands/registry.ts:611-623`
/// `KEYBOARD_PARITY_SHORTCUTS`): Alt REQUIRED, Ctrl/Meta rejected (`:633`),
/// matched on the physical key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityShortcut {
    /// Alt+A — reveal the waiting approval.
    ShowApproval,
    /// Alt+P — fold/unfold the peer dock.
    TogglePeerDock,
    /// Alt+D — open Fleet and focus its dispatch entry.
    FocusDispatch,
}

/// `matchKeyboardParityShortcut` (`registry.ts:629-640`).
pub fn match_parity_shortcut(key_code: KeyCode, ctrl: bool, alt: bool, logo: bool) -> Option<ParityShortcut> {
    if !alt || ctrl || logo {
        return None;
    }
    match key_code {
        KeyCode::KeyA => Some(ParityShortcut::ShowApproval),
        KeyCode::KeyP => Some(ParityShortcut::TogglePeerDock),
        KeyCode::KeyD => Some(ParityShortcut::FocusDispatch),
        _ => None,
    }
}

/// Whether a parity chord is suppressed for these facts. Show-approval's own
/// target is the approval surface, so focus already inside it is not a steal
/// (`App.tsx:1059-1076`): that chord waives the DIALOG half when the dialog
/// is the approval surface, never the text half.
pub fn parity_suppressed(shortcut: ParityShortcut, f: ShortcutFacts, inside_approval: bool) -> bool {
    match shortcut {
        ParityShortcut::ShowApproval if inside_approval => f.target_is_text_input,
        _ => shortcut_suppressed(f),
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

/// A22 audit — the approval the keyboard answers in `session`: the card
/// that Session shows (its payload names it — the same row the card's own
/// buttons answer, `surfaces::perform` `showing(session)`), never another
/// Session's; a row with no payload (a proof seed) names no Session and is
/// answerable as before. [`oldest_pending_id`] is the store-wide FIFO.
pub fn oldest_pending_id_in(store: &Store, session: &str) -> Option<String> {
    if let Some((p, _)) = store.domains.approval.showing(session) {
        return Some(p.id);
    }
    store
        .domains
        .approval
        .pending()
        .into_iter()
        .find(|a| !a.decided && !a.cancelled && store.domains.approval.detail(&a.id).is_none())
        .map(|a| a.id)
}

/// A22 audit — the diff preview id of the row [`oldest_pending_id_in`]
/// returns for `session`, so `D` and Y/S/N act on the same card.
pub fn preview_id_in(store: &Store, session: &str) -> Option<String> {
    let id = oldest_pending_id_in(store, session)?;
    store.domains.approval.pending().into_iter().find(|a| a.id == id).and_then(|a| a.preview_id)
}

/// A22 audit — the `approval/respond` body a decision key sends in
/// `session` (the Session on screen when it was pressed): its own showing
/// approval, under its own id; `None` = nothing to answer there.
pub fn key_decision(store: &Store, session: &str, action: &KeyAction) -> Option<serde_json::Value> {
    let id = oldest_pending_id_in(store, session)?;
    respond_body(action, session, &id)
}

/// #P4f2 row 7: the diff preview id of the SAME FIFO row
/// [`oldest_pending_id`] returns, so `D` and Y/S/N can never act on different
/// cards. `None` when the showing approval is not a diff approval — the web's
/// `previewId` absent, which leaves `D` inert (`ApprovalPanel.tsx:45`).
pub fn preview_id(store: &Store) -> Option<String> {
    store.domains.approval.oldest_preview_id()
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

#[cfg(test)]
mod a7_suppression_tests {
    use super::*;

    // ---- shortcut-suppression.ts:21 + e2e keyboard-parity.spec.ts:379
    // ---- "Alt+D focuses Fleet's capability notice; suppressed inside inputs"
    #[test]
    fn a_parity_chord_is_suppressed_inside_a_text_input_or_dialog() {
        let none = ShortcutFacts::default();
        let input = ShortcutFacts { target_is_text_input: true, in_dialog: false };
        let dialog = ShortcutFacts { target_is_text_input: false, in_dialog: true };
        assert!(!shortcut_suppressed(none));
        assert!(shortcut_suppressed(input));
        assert!(shortcut_suppressed(dialog));
        for s in [ParityShortcut::FocusDispatch, ParityShortcut::TogglePeerDock] {
            assert!(parity_suppressed(s, input, false));
            assert!(parity_suppressed(s, dialog, false));
            assert!(!parity_suppressed(s, none, false));
        }
        // Alt+A inside its own approval surface is not a steal — but the text
        // half still holds there.
        assert!(!parity_suppressed(ParityShortcut::ShowApproval, dialog, true));
        assert!(parity_suppressed(ParityShortcut::ShowApproval, input, true));
        assert!(parity_suppressed(ParityShortcut::ShowApproval, dialog, false));
    }

    #[test]
    fn parity_chords_need_alt_and_reject_ctrl_or_meta() {
        assert_eq!(match_parity_shortcut(KeyCode::KeyD, false, true, false), Some(ParityShortcut::FocusDispatch));
        assert_eq!(match_parity_shortcut(KeyCode::KeyP, false, true, false), Some(ParityShortcut::TogglePeerDock));
        assert_eq!(match_parity_shortcut(KeyCode::KeyA, false, true, false), Some(ParityShortcut::ShowApproval));
        assert_eq!(match_parity_shortcut(KeyCode::KeyD, false, false, false), None, "Alt is required");
        assert_eq!(match_parity_shortcut(KeyCode::KeyD, true, true, false), None, "AltGr / Ctrl+Alt stays inert");
        assert_eq!(match_parity_shortcut(KeyCode::KeyD, false, true, true), None, "Cmd+Alt stays inert");
        assert_eq!(match_parity_shortcut(KeyCode::KeyX, false, true, false), None);
    }
}
