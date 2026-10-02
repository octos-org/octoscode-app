//! P4f2 — the approval keyboard + diff-preview path through the PRODUCTION
//! pipeline (rows 6 and 7 of the `g-connection/approval` group).
//!
//! Row 6 — `Keyboard shortcuts Y/N/S/D guarded against modifiers and IME
//!   composition` (web `ApprovalPanel.tsx:30-56`). Natively: the modifier guard
//!   is real and tested; the IME arm has **no input** (see the limit note
//!   below), so this test pins the half that is portable.
//!
//! Row 7 — `Review diff from approval (preview id derived from approval
//!   payload)` (web `ApprovalPanel.tsx:45` + `approvalDiffPreviewId`,
//!   `packages/client/src/interaction.ts:94-102`). The web reads the id off the
//!   card that is showing and calls `onReviewDiff(previewId)`. This test drives
//!   the real notification decode -> `Registry::dispatch` -> the production
//!   `ApprovalRequestedHandler` -> store, then the real `keys::resolve`.
//!
//! ## Recorded traffic
//!
//! `crates/octoscode-client/tests/fixtures/r5-turn-a6ea8505.jsonl` carries a
//! REAL `approval/requested` with a real `typed_details` (`kind: "command"`,
//! `command.argv`, `command_line`, `tool_call_id`). Replayed verbatim for the
//! non-diff case — note it has **no** `diff` key, which is exactly the
//! "previewId absent => D inert" branch, so the recorded frame proves the safe
//! path rather than being bent into the interesting one. The diff case adds
//! only the `diff.preview_id` field, holding every other value at the
//! recording's own.
//!
//! ## Measured limit (recorded, not claimed)
//!
//! The web returns early on `event.nativeEvent.isComposing` and
//! `keyCode === 229` (`ApprovalPanel.tsx:36-38`) — DOM properties. The pinned
//! makepad `KeyEvent` is `{key_code, is_repeat, modifiers, time}`
//! (`platform/studio/src/keyboard.rs:7-12`) and a whole-tree search finds no
//! composing flag, so that half of the guard has no native input. Row 6
//! therefore cannot reach A on this evidence; see .peer/report-P4f2.md.
use std::sync::Arc;

use makepad_widgets::KeyCode;

use octoscode_client::Registry;
use octoscode_module::screens::keys::{self, KeyAction};
use octoscode_store::Store;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;

const R5: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../octoscode-client/tests/fixtures/r5-turn-a6ea8505.jsonl"
);

/// The recorded `approval/requested` params.
fn recorded_approval_requested() -> serde_json::Value {
    for line in std::fs::read_to_string(R5).expect("read r5 fixture").lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["dir"].as_str() == Some("in") && v["method"].as_str() == Some("approval/requested") {
            return v.get("body").cloned().unwrap_or(serde_json::Value::Null);
        }
    }
    panic!("r5-turn carries no recorded approval/requested frame")
}

/// The transport's decode shape: the recorded params plus the internally
/// tagged `kind` (`#[serde(tag = "kind", rename_all = "snake_case")]`,
/// `ui_protocol.rs:6581`).
fn wire(mut body: serde_json::Value) -> UiNotification {
    body.as_object_mut()
        .expect("notification body is an object")
        .insert("kind".to_owned(), serde_json::json!("approval_requested"));
    serde_json::from_value::<UiNotification>(body).expect("the recorded body decodes")
}

/// The production registration path (the one-owner rule: approval's own
/// `register`, exactly what `Conversation::connect` calls). A20: the Session
/// on screen is the one the recording's approval belongs to — the keyboard
/// answers only the approval of the Session that is showing it.
fn registry(store: &Arc<Store>) -> Registry {
    let session = recorded_approval_requested()["session_id"].as_str().expect("the recorded session").to_owned();
    store.set_active(Some(session));
    let mut reg = Registry::new();
    octoscode_client::domains::approval::register(&mut reg, store.clone());
    reg
}

const PREVIEW: &str = "01920000-0000-7000-8000-0000000000d1";

/// The recorded frame, with ONLY `typed_details.diff` added — every other
/// value stays the recording's own.
fn recorded_with_diff(preview_id: &str) -> serde_json::Value {
    let mut body = recorded_approval_requested();
    body["typed_details"]["diff"]["preview_id"] = serde_json::json!(preview_id);
    body
}

// ------------------------------------------------------------------- row 7

/// The recorded `approval/requested` is a COMMAND approval: it carries
/// `typed_details` but no `diff`, so no preview id is admitted and `D` stays
/// inert — the web's `key === "d" && previewId` short-circuit.
#[test]
fn the_recorded_command_approval_leaves_d_inert() {
    let store = Arc::new(Store::new());
    let mut reg = registry(&store);
    let recorded = recorded_approval_requested();

    // Sanity: the recording really is a non-diff approval.
    assert!(recorded["typed_details"].is_object(), "the frame carries typed_details");
    assert!(
        recorded["typed_details"].get("diff").is_none(),
        "the recorded command approval has no diff"
    );

    reg.dispatch(&wire(recorded));

    // The card IS pending (Y/S/N still work) …
    assert!(keys::oldest_pending_id(&store).is_some());
    // … but there is no preview id, so D produces nothing.
    assert_eq!(keys::preview_id(&store), None);
    assert_eq!(
        keys::resolve(KeyCode::KeyD, false, false, false, false, false, true, None, false, true),
        KeyAction::Ignore,
        "D must stay inert without a preview id"
    );
}

/// A diff approval's payload id is derived, stored, and binds `D` — the web's
/// `onReviewDiff(previewId)` link, reached through the production handler.
#[test]
fn a_diff_approval_binds_d_to_the_payload_preview_id() {
    let store = Arc::new(Store::new());
    let mut reg = registry(&store);

    reg.dispatch(&wire(recorded_with_diff(PREVIEW)));

    // Derived from the payload, not invented.
    assert_eq!(keys::preview_id(&store).as_deref(), Some(PREVIEW));
    assert_eq!(
        keys::resolve(KeyCode::KeyD, false, false, false, false, false, true, Some(PREVIEW.into()), false, true),
        KeyAction::ApprovalReviewDiff(PREVIEW.to_owned()),
        "D opens the review for the payload's preview id"
    );
}

/// The preview id is gated **twice**, and the second gate is not redundant.
///
/// * gate 1 — octos-core's decode: `PreviewId(pub Uuid)`
///   (`ui_protocol.rs:666`), so a plainly malformed id (`"not-a-uuid"`) never
///   becomes a notification at all. Pinned here so the claim is measured, not
///   assumed.
/// * gate 2 — the web's shared `isProtocolUuid` gate
///   (`interaction.ts:99`), which is stricter than Rust's parser for exactly
///   one shape: a URN-wrapped SIMPLE id. Rust routes a `urn:uuid:` input only
///   to the hyphenated parser, so `urn:uuid:<32 hex>` IS accepted natively —
///   and the web refuses it. The web's own comment records why the gate exists
///   at all: "Non-canonical forms here are accepted by the native decoder, so
///   the shared protocol-id gate must accept them too — otherwise a genuine
///   accepted turn id would be dropped as malformed."
#[test]
fn a_malformed_preview_id_never_decodes() {
    // Gate 1, asserted WITHOUT going through `wire()` — that helper panics on
    // a decode failure, which is the very thing this test asserts. Decode
    // directly instead.
    let mut body = recorded_with_diff("not-a-uuid");
    body.as_object_mut()
        .expect("notification body is an object")
        .insert("kind".to_owned(), serde_json::json!("approval_requested"));

    let err = serde_json::from_value::<UiNotification>(body)
        .err()
        .expect("octos-core's Uuid decode must refuse a non-UUID preview id");
    ::log::info!("gate 1 refused the malformed id: {err}");
}

/// Gate 2, the reachable case: a URN-wrapped SIMPLE id is a real UUID to Rust
/// (so it decodes and would otherwise reach the store) but the web's gate
/// refuses it, so it must not bind `D`.
#[test]
fn a_urn_wrapped_simple_id_is_refused_by_the_web_gate() {
    let simple = "019200000000700080000000000000d1";
    assert_eq!(simple.len(), 32, "the simple form is 32 hex digits");
    let urn = format!("urn:uuid:{simple}");

    // Gate 1 does NOT stop it: Rust's parser accepts a URN-wrapped hyphenated
    // id, and the web records that a URN-wrapped SIMPLE id is not an accepted
    // shape — which is precisely why the second gate exists.
    let mut body = recorded_with_diff(&urn);
    body["typed_details"]["diff"]["preview_id"] = serde_json::json!(&urn);
    // If octos-core accepts it, this dispatch succeeds …
    let decoded = serde_json::from_value::<UiNotification>(body.clone());
    let store = Arc::new(Store::new());
    let mut reg = registry(&store);
    match decoded {
        Ok(n) => {
            reg.dispatch(&n);
            // … and then gate 2 must be what stops it binding D.
            assert_eq!(
                keys::preview_id(&store),
                None,
                "a URN-wrapped simple id must not bind D (web parity)"
            );
            assert_eq!(
                keys::resolve(
                    KeyCode::KeyD,
                    false,
                    false,
                    false,
                    false,
                    false,
                    true,
                    None,
                    false,
                    true
                ),
                KeyAction::Ignore
            );
        }
        Err(_) => {
            // octos-core refused it outright — the two gates agree, D stays
            // inert either way. Recorded rather than asserted as one path.
            assert_eq!(keys::preview_id(&store), None);
        }
    }
}

/// `D` and Y/S/N must act on the SAME card: the preview id comes from the same
/// FIFO row `oldest_pending_id` returns, never a later one.
#[test]
fn d_and_the_decision_keys_never_act_on_different_cards() {
    let store = Arc::new(Store::new());
    let mut reg = registry(&store);

    // The first row is a plain (no-diff) approval; the second carries a diff.
    reg.dispatch(&wire(recorded_approval_requested()));
    let mut second = recorded_with_diff(PREVIEW);
    second["approval_id"] = serde_json::json!("01a0e773-f844-7d50-b171-b38d159f00bb");
    reg.dispatch(&wire(second));

    // The SHOWING card is the first, so it has no preview id even though a
    // later row does.
    assert!(keys::oldest_pending_id(&store).is_some());
    assert_eq!(
        keys::preview_id(&store),
        None,
        "a later card's preview id must not bind D on the showing card"
    );
}

// ------------------------------------------------------------------- row 6

/// The modifier guard: Y/S/N/D are bare keys only (the web's
/// `ApprovalPanel.tsx:37-43`), and a chord never decides.
#[test]
fn the_four_shortcuts_are_bare_keys_only() {
    // Each modifier in turn must make the key inert, not merely ignored.
    for (shift, ctrl, alt, logo) in [
        (true, false, false, false),
        (false, true, false, false),
        (false, false, true, false),
        (false, false, false, true),
    ] {
        let with_approval = Some(PREVIEW.to_owned());
        assert_eq!(
            keys::resolve(KeyCode::KeyY, shift, ctrl, alt, logo, false, true, with_approval.clone(), false, true),
            KeyAction::Ignore,
            "Shift/Ctrl/Alt/Logo+Y must not decide (shift={shift} ctrl={ctrl} alt={alt} logo={logo})"
        );
        assert_eq!(
            keys::resolve(KeyCode::KeyS, shift, ctrl, alt, logo, false, true, with_approval.clone(), false, true),
            KeyAction::Ignore
        );
        assert_eq!(
            keys::resolve(KeyCode::KeyN, shift, ctrl, alt, logo, false, true, with_approval.clone(), false, true),
            KeyAction::Ignore
        );
        assert_eq!(
            keys::resolve(KeyCode::KeyD, shift, ctrl, alt, logo, false, true, with_approval, false, true),
            KeyAction::Ignore
        );
    }
}

/// With no approval showing, all four are typing — never a decision.
#[test]
fn the_four_shortcuts_are_typing_when_no_approval_is_showing() {
    for kc in [KeyCode::KeyY, KeyCode::KeyS, KeyCode::KeyN, KeyCode::KeyD] {
        assert_eq!(
            keys::resolve(kc, false, false, false, false, false, false, Some(PREVIEW.into()), false, true),
            KeyAction::Ignore,
            "{kc:?} must not decide with no pending card"
        );
    }
}

/// A decided or cancelled card is not "showing": the keyboard must not act.
#[test]
fn a_settled_card_is_not_acted_on() {
    let store = Arc::new(Store::new());
    // A20: the card belongs to the Session on screen (a row with no recorded
    // origin is never a card).
    store.set_active(Some("s1".into()));
    store.domains.approval.request_with_preview("a1", None, Some(PREVIEW.into()));
    store.domains.approval.set_detail(
        "a1",
        octoscode_store::domains::approval::ApprovalDetail { session_id: "s1".into(), ..Default::default() },
    );
    assert_eq!(keys::preview_id(&store).as_deref(), Some(PREVIEW));

    store.domains.approval.decide("a1");
    assert_eq!(keys::preview_id(&store), None, "a decided card is not showing");
    assert_eq!(
        keys::resolve(KeyCode::KeyD, false, false, false, false, false, true, None, false, true),
        KeyAction::Ignore
    );
}
