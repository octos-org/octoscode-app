//! Card #31e — the keyboard surface's contract: one resolver
//! (`screens::keys::resolve`), every rule pinned to its web source. Pure
//! table tests here; the live `/k` evidence rides the shell probe.

use makepad_widgets::KeyCode;

use octoscode_module::screens::keys::{self, respond_body, KeyAction};
use octoscode_store::{domains::approval::PendingApproval, Store};

/// Shorthand: resolve with the common "nothing open, nothing live" shell.
fn r(key_code: KeyCode, shift: bool, ctrl: bool, alt: bool, logo: bool) -> KeyAction {
    keys::resolve(
        key_code, shift, ctrl, alt, logo,
        false, // palette_open
        false, // approval_pending
        None,  // approval_preview (#P4f2 row 7 — no approval is showing)
        false, // turn_active
        true,  // draft_empty
    )
}

// -------------------------------------------- Enter send / Shift+Enter newline

#[test]
fn enter_sends_and_shift_enter_is_the_newline() {
    // ComposerInput.tsx:237-243 — bare Enter submits.
    assert_eq!(r(KeyCode::ReturnKey, false, false, false, false), KeyAction::ComposerSubmit);
    // :237's `!shiftKey` guard — Shift+Enter is the multiline input's own
    // newline; the resolver never intercepts it.
    assert_eq!(r(KeyCode::ReturnKey, true, false, false, false), KeyAction::Ignore);
    // The same guard rejects ctrl/meta Enter (a chord a text control may own).
    assert_eq!(r(KeyCode::ReturnKey, false, true, false, false), KeyAction::Ignore);
    assert_eq!(r(KeyCode::ReturnKey, false, false, false, true), KeyAction::Ignore);
}

// ------------------------------------------------------- Esc: overlay → turn

#[test]
fn esc_closes_the_overlay_then_interrupts_the_turn() {
    // CommandPalette.tsx:40-43 — Esc closes the open palette FIRST.
    assert_eq!(
        keys::resolve(KeyCode::Escape, false, false, false, false, true, false, None, false, true),
        KeyAction::PaletteClose
    );
    // ComposerInput.tsx:245-252 — bare Esc interrupts the LIVE turn.
    assert_eq!(
        keys::resolve(KeyCode::Escape, false, false, false, false, false, false, None, true, true),
        KeyAction::Interrupt
    );
    // …and does nothing with no overlay and no turn.
    assert_eq!(r(KeyCode::Escape, false, false, false, false), KeyAction::Ignore);
    // Modifiered Esc stays out of the way (:245's guards).
    assert_eq!(r(KeyCode::Escape, false, true, false, false), KeyAction::Ignore);
    assert_eq!(r(KeyCode::Escape, false, false, true, false), KeyAction::Ignore);
}

// ----------------------------------------------------- Cmd/Ctrl+K and "/"

#[test]
fn cmd_ctrl_k_toggles_and_slash_opens_on_an_empty_draft() {
    // App.tsx:1096 — (metaKey || ctrlKey) && "k".
    assert_eq!(r(KeyCode::KeyK, false, false, false, true), KeyAction::PaletteToggle);
    assert_eq!(r(KeyCode::KeyK, false, true, false, false), KeyAction::PaletteToggle);
    // Plain k is typing; Alt+K is nobody's binding.
    assert_eq!(r(KeyCode::KeyK, false, false, false, false), KeyAction::Ignore);
    assert_eq!(r(KeyCode::KeyK, false, false, true, false), KeyAction::Ignore);

    // "/" on an EMPTY draft opens (idempotent set — the composer's
    // changed-action may fire the same open, #28e item 5).
    assert_eq!(r(KeyCode::Slash, false, false, false, false), KeyAction::PaletteOpen);
    // A non-empty draft keeps "/" as text (the changed-action path owns it).
    assert_eq!(
        keys::resolve(KeyCode::Slash, false, false, false, false, false, false, None, false, false),
        KeyAction::Ignore
    );
    // Shift+Slash ("?") is typing.
    assert_eq!(r(KeyCode::Slash, true, false, false, false), KeyAction::Ignore);
    // Already open: the key binding does nothing (Esc closes).
    assert_eq!(
        keys::resolve(KeyCode::Slash, false, false, false, false, true, false, None, false, true),
        KeyAction::Ignore
    );
}

// ------------------------------------------ palette arrows (↑↓ + Enter run)

#[test]
fn arrows_move_the_palette_only_while_open() {
    // CommandPalette.tsx:44-49.
    assert_eq!(
        keys::resolve(KeyCode::ArrowDown, false, false, false, false, true, false, None, false, true),
        KeyAction::PaletteMove(1)
    );
    assert_eq!(
        keys::resolve(KeyCode::ArrowUp, false, false, false, false, true, false, None, false, true),
        KeyAction::PaletteMove(-1)
    );
    // Closed palette: the arrows scroll/navigate whatever else owns them.
    assert_eq!(r(KeyCode::ArrowDown, false, false, false, false), KeyAction::Ignore);
    assert_eq!(r(KeyCode::ArrowUp, false, false, false, false), KeyAction::Ignore);
    // Enter while open RUNS the selection (the listbox's Enter).
    assert_eq!(
        keys::resolve(KeyCode::ReturnKey, false, false, false, false, true, false, None, false, true),
        KeyAction::PaletteRun
    );
}

// ----------------------------------------------- Alt+A (the parity shortcut)

#[test]
fn alt_a_is_the_only_approval_chord() {
    // registry.ts:614 — Alt REQUIRED; :633 rejects Ctrl/Meta so Cmd+Alt+A and
    // AltGr stay inert rather than shadowing a text-input chord.
    assert_eq!(r(KeyCode::KeyA, false, false, true, false), KeyAction::ShowApproval);
    assert_eq!(r(KeyCode::KeyA, false, false, true, true), KeyAction::Ignore);
    assert_eq!(r(KeyCode::KeyA, false, true, true, false), KeyAction::Ignore);
    // Plain A is typing.
    assert_eq!(r(KeyCode::KeyA, false, false, false, false), KeyAction::Ignore);
}

// ------------------------------------------- Y/S/N decide ONLY a pending card

#[test]
fn ysn_decide_only_bare_keys_with_a_pending_approval() {
    // ApprovalPanel.tsx:46-54 — approve/request, approve/session, deny/request.
    let with = |kc| {
        keys::resolve(kc, false, false, false, false, false, true, None, false, true)
    };
    assert_eq!(with(KeyCode::KeyY), KeyAction::ApprovalApproveRequest);
    assert_eq!(with(KeyCode::KeyS), KeyAction::ApprovalApproveSession);
    assert_eq!(with(KeyCode::KeyN), KeyAction::ApprovalDenyRequest);
    // :37-43 — chords never decide.
    assert_eq!(
        keys::resolve(KeyCode::KeyY, false, true, false, false, false, true, None, false, true),
        KeyAction::Ignore
    );
    assert_eq!(
        keys::resolve(KeyCode::KeyY, false, false, true, false, false, true, None, false, true),
        KeyAction::Ignore
    );
    // No pending card: the keys are typing.
    assert_eq!(r(KeyCode::KeyY, false, false, false, false), KeyAction::Ignore);
    assert_eq!(r(KeyCode::KeyS, false, false, false, false), KeyAction::Ignore);
    assert_eq!(r(KeyCode::KeyN, false, false, false, false), KeyAction::Ignore);
}

// ------------------------------------------------- #28e's chrome chords hold

#[test]
fn the_28e_chrome_chords_are_preserved() {
    assert_eq!(r(KeyCode::KeyE, false, false, false, true), KeyAction::ReviewToggle);
    assert_eq!(r(KeyCode::Period, false, false, false, true), KeyAction::SettingsToggle);
}

// ------------------------------- the approval decision's outbound grammar

#[test]
fn the_keyboard_decision_writes_the_recorded_respond_grammar() {
    let session = "octoscode49213:main";
    let approval = "01a0e773-f844-7d50-b171-b38d159f00aa";

    // The r5-turn recording's exact frames, now with the web's scope:
    // approve/request (Y), approve/session (S), deny/request (N).
    let y = respond_body(&KeyAction::ApprovalApproveRequest, session, approval).unwrap();
    assert_eq!(y["decision"], serde_json::json!("approve"));
    assert_eq!(y["approval_scope"], serde_json::json!("request"));
    assert_eq!(y["approval_id"], serde_json::json!(approval));
    assert_eq!(y["session_id"], serde_json::json!(session));

    let s = respond_body(&KeyAction::ApprovalApproveSession, session, approval).unwrap();
    assert_eq!(s["decision"], serde_json::json!("approve"));
    assert_eq!(s["approval_scope"], serde_json::json!("session"));

    let n = respond_body(&KeyAction::ApprovalDenyRequest, session, approval).unwrap();
    assert_eq!(n["decision"], serde_json::json!("deny"));

    // A non-decision action writes nothing.
    assert!(respond_body(&KeyAction::ComposerSubmit, session, approval).is_none());
}

// ----------------------------------------- the store's pending-approval FIFO

#[test]
fn the_oldest_undecided_approval_is_what_the_keyboard_answers() {
    let store = Store::new();
    let mk = |id: &str, decided: bool, cancelled: bool| PendingApproval {
        id: id.to_owned(),
        target: None,
        decided,
        auto_resolved: false,
        cancelled,
        preview_id: None,
    };
    // A20: the keyboard answers the card of the Session ON SCREEN — every row
    // here carries its recorded origin.
    let on = |store: &Store, id: &str, session: &str| {
        store.domains.approval.set_detail(
            id,
            octoscode_store::domains::approval::ApprovalDetail { session_id: session.into(), ..Default::default() },
        );
    };
    store.set_active(Some("s1".into()));
    // Another Session's earlier request is never this Session's to answer.
    store.domains.approval.push(mk("z-other-session", false, false));
    on(&store, "z-other-session", "s2");
    // A row with no recorded origin is never answered either.
    store.domains.approval.push(mk("y-unattributed", false, false));
    // The web decides the SHOWING card; the store's list is FIFO
    // (domains/approval.rs) — the first actionable row OF THIS SESSION wins.
    for (id, decided, cancelled) in [("a-first", false, false), ("b-decided", true, false), ("c-cancelled", false, true)] {
        store.domains.approval.push(mk(id, decided, cancelled));
        on(&store, id, "s1");
    }
    assert_eq!(keys::oldest_pending_id(&store).as_deref(), Some("a-first"));
    store.set_active(Some("s2".into()));
    assert_eq!(keys::oldest_pending_id(&store).as_deref(), Some("z-other-session"), "s2's own card on s2");
    store.set_active(Some("s3".into()));
    assert_eq!(keys::oldest_pending_id(&store), None, "a Session with no card answers nothing");

    // All settled → nothing for the keyboard to answer.
    let store2 = Store::new();
    store2.set_active(Some("s1".into()));
    store2.domains.approval.push(mk("only", true, false));
    on(&store2, "only", "s1");
    assert_eq!(keys::oldest_pending_id(&store2), None);
}

// ------------------------------------- the one-owner shapes stay pinned (29d3)

#[test]
fn the_conversation_tables_keep_their_pinned_shapes() {
    // The keyboard work touched NO conversation table.
    assert_eq!(octoscode_module::bindings::ACTIONS.len(), 9);
    assert_eq!(octoscode_module::actions::unrouted(), vec!["answer.expand"]);
}

#[test]
fn ctrl_x_sends_pending_and_tab_queues_only_in_an_active_conversation() {
    assert_eq!(keys::resolve(KeyCode::KeyX, false, true, false, false, false, false, None, true, true), KeyAction::SendPendingNow);
    assert_eq!(keys::resolve(KeyCode::KeyX, false, true, false, false, true, false, None, true, true), KeyAction::Ignore);
    assert_eq!(r(KeyCode::KeyX, false, true, false, false), KeyAction::Ignore);
    assert_eq!(r(KeyCode::KeyX, false, false, false, true), KeyAction::Ignore);
    assert_eq!(keys::resolve(KeyCode::Tab, false, false, false, false, false, false, None, true, false), KeyAction::ComposerQueue);
    assert_eq!(keys::resolve(KeyCode::Tab, false, false, false, false, false, false, None, true, true), KeyAction::Ignore);
}
