//! Board-3 screen 8 — the SESSION STRIP (rows: session-config × 2).
//!
//! Web: `features/session-config/SessionStatusStrip.tsx` mounted in the
//! composer footer (`App.tsx:2963-2982`): `{model} · {permission} · {state}`
//! with the fallbacks "Model not reported" / "Permissions not reported"
//! (`:93-97`), described as "Model, permissions, sandbox" (`:100-114`); a
//! click opens the Session settings pane (`App.tsx:3139-3156`). The state
//! word follows the precedence table (`App.tsx:2047-2088`): not connected ->
//! Reconnecting; seat handover -> Resuming chat… / Handing back control…;
//! pending approval -> Waiting for your approval; pending question ->
//! Waiting for your answer; foreign holder -> Another app is using this
//! session; active turn -> the LIVE activity word (`turn-activity.ts:33-55`:
//! reasoning -> "Thinking…", assistant text -> "Writing…", tool start ->
//! "Running {tool}…") or "Responding"; peers running -> "Peers running
//! (n)"; else "Ready". The approved board draws the three facts as three
//! segments split by hairlines.
//!
//! Model: `session/status/read`'s `model.title ?? model.model`
//! (`App.tsx:1999-2000`, `session-status-result.ts:141-165`); permission:
//! the current permission profile mode ("Read only" / "Workspace write" /
//! "Full access", `App.tsx:2037-2045`).
use octoscode_store::timeline::EntryKind;
use octoscode_store::Store;

use super::ui::{tok, Dsl, Face, Txt, W};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StripState {
    /// session id -> the status read's model label.
    pub model: Option<(String, String)>,
    pub status_for: Option<String>,
    pub handover: Option<String>,
    /// The composer's measured width (the strip aligns to it).
    pub width: Option<f64>,
}

/// The permission label (`App.tsx:2037-2045`).
pub fn permission_label(mode: Option<&str>) -> Option<&'static str> {
    Some(match mode? {
        "read_only" => "Read only",
        "workspace_write" => "Workspace write",
        _ => "Full access",
    })
}

/// The live activity word of the active turn (`turn-activity.ts:33-55`).
pub fn activity_word(store: &Store, session: &str, turn: &str) -> Option<String> {
    let last = store
        .domains
        .session
        .timeline
        .entries(session)
        .into_iter()
        .filter(|e| e.turn_id.as_deref() == Some(turn))
        .last()?;
    if last.kind == EntryKind::REASONING {
        Some("Thinking…".into())
    } else if last.kind == EntryKind::ASSISTANT_TEXT {
        Some("Writing…".into())
    } else if last.kind == EntryKind::TOOL_CALL {
        let running = store.domains.tool.calls().into_iter().rev().find(|c| c.name == last.text);
        match running {
            Some(c) if c.status == "running" => Some(format!("Running {}…", last.text)),
            _ => Some("Thinking…".into()), // a tool end hands back to thinking
        }
    } else {
        None
    }
}

/// The state word by the web's precedence.
pub fn state_word(store: &Store, active_turn: Option<&str>, handover: Option<&str>) -> String {
    if !store.is_live() {
        return "Reconnecting".into();
    }
    if let Some(h) = handover {
        return h.to_owned();
    }
    let session = store.active_session().unwrap_or_default();
    // A6: only an ACTIONABLE approval of this session waits (the raw list
    // keeps decided / cancelled rows, so a settled approval read "Waiting"
    // forever); then a pending question (`App.tsx:2047-2088`).
    if store.domains.approval.actionable_count(&session) > 0 {
        return "Waiting for your approval".into();
    }
    if store.domains.approval.question().map(|q| q.session_id == session).unwrap_or(false) {
        return "Waiting for your answer".into();
    }
    if let Some(turn) = active_turn {
        return activity_word(store, &session, turn).unwrap_or_else(|| "Responding".into());
    }
    let peers = store.domains.peer.list().into_iter().filter(|p| !p.closed).count();
    if peers > 0 {
        return format!("Peers running ({peers})");
    }
    "Ready".into()
}

/// The three facts.
pub fn facts(store: &Store, st: &StripState, active_turn: Option<&str>, mode: Option<&str>) -> (String, String, String) {
    let session = store.active_session().unwrap_or_default();
    let model = st
        .model
        .as_ref()
        .filter(|(s, _)| s == &session)
        .map(|(_, m)| m.clone())
        .unwrap_or_else(|| "Model not reported".into());
    // The server's current permission selection first (the web's
    // `currentPermission`, `App.tsx:2037-2045`), else the workspace card's
    // read of `permission/profile/list`.
    let perm = store
        .domains
        .profile
        .permission()
        .map(|sel| {
            use octoscode_store::domains::profile::PermissionProfileMode as M;
            match sel.mode {
                M::ReadOnly => "Read only",
                M::WorkspaceWrite => "Workspace write",
                M::DangerFullAccess => "Full access",
            }
        })
        .or_else(|| permission_label(mode))
        .unwrap_or("Permissions not reported")
        .to_owned();
    (model, state_word(store, active_turn, st.handover.as_deref()), perm)
}

/// `session/status/read` -> the model label for the active Session.
pub async fn load_status(conv: &crate::flow::Conversation) -> Result<String, String> {
    let session = conv.session_id();
    let v = conv
        .client()
        .request("session/status/read", serde_json::json!({ "session_id": session }))
        .await
        .map_err(|e| e.to_string())?;
    // The identity check the web performs (`session-status-result.ts:6`).
    if v.get("session_id").and_then(|s| s.as_str()) != Some(session.as_str()) {
        return Err("status belongs to another Session".into());
    }
    let model = v.get("model").and_then(|m| {
        m.get("title")
            .and_then(|t| t.as_str())
            .or_else(|| m.get("model").and_then(|t| t.as_str()))
            .map(str::to_owned)
    });
    let mut st = super::host::state();
    if let Some(m) = &model {
        st.strip.model = Some((session.clone(), m.clone()));
    }
    Ok(model.unwrap_or_else(|| "no model reported".into()))
}

/// The strip DSL (mounted above the composer).
pub fn lower(
    store: &Store,
    st: &StripState,
    active_turn: Option<&str>,
    mode: Option<&str>,
    vim_note: Option<&str>,
) -> String {
    let (model, state, perm) = facts(store, st, active_turn, mode);
    let mut d = Dsl::new();
    let width = st.width.map(|w| format!("{w}")).unwrap_or_else(|| "Fill".into());
    let narrow = st.width.is_some_and(|w| w < 420.0);
    let px = if narrow { 12.0 } else { 13.0 };
    d.view("b3_strip_root", "width: Fill height: Fit flow: Down spacing: 4 padding: Inset{left: 0 right: 0 top: 2 bottom: 4}");
    // The board: one white rounded strip, three equal cells split by
    // hairlines; a long fact wraps inside its cell ("Permissions not
    // reported", "Workspace write allowed").
    d.surface(
        "b3_strip",
        &format!("width: {width} height: Fit flow: Overlay"),
        tok::SURFACE,
        10.0,
        Some(tok::HAIRLINE),
    );
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5}");
    // Three EQUAL cells (the board): a Fill cell shrank to its neighbours'
    // leftovers on a phone and clipped "Permissions not reported".
    let cell_w = st
        .width
        .map(|w| format!("{}", ((w - 2.0) / 3.0).floor()))
        .unwrap_or_else(|| "Fill".into());
    let cell = |d: &mut Dsl, id: &str, text: &str, muted: bool, mono: bool, center: bool| {
        let align = if center { "0.5" } else { "0.0" };
        d.view(
            &format!("{id}_cell"),
            &format!("width: {cell_w} height: Fit flow: Right align: Align{{x: {align} y: 0.5}} padding: Inset{{left: 10 right: 8 top: 8 bottom: 8}}"),
        );
        let face = if mono { Face::Mono } else { Face::Regular };
        d.text(id, text, &Txt::new(px, face, if muted { tok::FAINT } else { tok::TEXT }).w(W::Fill).wrap());
        d.close();
    };
    let model_missing = model == "Model not reported";
    cell(&mut d, "b3_strip_model", &model, model_missing, false, false);
    d.vrule(26.0);
    cell(&mut d, "b3_strip_state", &state, false, false, false);
    d.vrule(26.0);
    cell(&mut d, "b3_strip_perm", &perm, perm == "Permissions not reported", false, false);
    d.close();
    d.tap("b3_strip_tap", "b3.strip.settings");
    d.close();
    // The board's caption under the strip is the web strip's own title
    // (`SessionStatusStrip.tsx:106`, "Model, permissions, sandbox"); on its
    // right, the composer's Vim field note while Vim editing is on
    // (`ComposerInput.tsx:270-274`: `Vim · Insert` / `Vim · Normal`).
    let caption = d.anon();
    d.view(
        &caption,
        &format!("width: {width} height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 2 right: 2}}"),
    );
    // A Fill run before a Fit one takes the whole row (the flow is one
    // pass), so with the note present the caption gets an explicit width.
    let caption_w = match (vim_note, st.width) {
        (Some(note), Some(w)) => W::Px((w - 4.0 - super::ui::text_w(note, 12.0, Face::Medium) - 8.0).max(60.0)),
        (Some(_), None) => W::Px(super::ui::text_w("Model, permissions, sandbox", 12.0, Face::Regular) + 12.0),
        _ => W::Fill,
    };
    d.text("b3_strip_caption", "Model, permissions, sandbox", &Txt::new(12.0, Face::Regular, tok::MUTED).w(caption_w));
    if let Some(note) = vim_note {
        d.text("b3_strip_vim", note, &Txt::new(12.0, Face::Medium, tok::TEXT));
    }
    d.close();
    // The transitional states read in blue under it (the board's
    // "Reconnecting" / "Resuming chat…" / "Handing back control…").
    if matches!(state.as_str(), "Reconnecting" | "Resuming chat…" | "Handing back control…") {
        d.text("b3_strip_transition", &state, &Txt::new(12.5, Face::Regular, tok::BLUE));
    }
    d.close();
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_store() -> Store {
        let s = Store::new();
        s.set_connection("Live".into(), true);
        s.set_active(Some("s".into()));
        s
    }

    #[test]
    fn fallbacks_and_permission_labels_are_the_web_copy() {
        let s = live_store();
        let (m, st, p) = facts(&s, &StripState::default(), None, None);
        assert_eq!((m.as_str(), st.as_str(), p.as_str()), ("Model not reported", "Ready", "Permissions not reported"));
        assert_eq!(permission_label(Some("workspace_write")), Some("Workspace write"));
        assert_eq!(permission_label(Some("danger_full_access")), Some("Full access"));
    }

    #[test]
    fn the_state_word_follows_the_precedence_table() {
        let s = Store::new();
        assert_eq!(state_word(&s, None, None), "Reconnecting", "not connected first");
        let s = live_store();
        assert_eq!(state_word(&s, None, Some("Resuming chat…")), "Resuming chat…");
        s.domains.session.timeline.append("s", Some("t".into()), EntryKind::REASONING, "hmm".into());
        assert_eq!(state_word(&s, Some("t"), None), "Thinking…");
        s.domains.session.timeline.append("s", Some("t".into()), EntryKind::ASSISTANT_TEXT, "ok".into());
        assert_eq!(state_word(&s, Some("t"), None), "Writing…");
        assert_eq!(state_word(&s, Some("other"), None), "Responding", "no live activity yet");
    }

    #[test]
    fn the_model_is_scoped_to_its_session() {
        let s = live_store();
        let st = StripState { model: Some(("other".into(), "glm-5.3".into())), ..Default::default() };
        assert_eq!(facts(&s, &st, None, None).0, "Model not reported");
        let st = StripState { model: Some(("s".into(), "glm-5.3".into())), ..Default::default() };
        assert_eq!(facts(&s, &st, None, None).0, "glm-5.3");
    }

    #[test]
    fn the_strip_lowers_balanced_with_its_settings_tap() {
        let s = live_store();
        let dsl = lower(&s, &StripState::default(), None, Some("read_only"), None);
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        assert!(crate::screens::taps::wired_taps(&dsl).iter().any(|(_, e)| e == "b3.strip.settings"));
    }
}
