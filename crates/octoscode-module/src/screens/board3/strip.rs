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
use octoscode_store::domains::models::Identity;
use octoscode_store::domains::profile::ProfileLlmModel;
use octoscode_store::Store;

use super::ui::{tok, Dsl, Face, Txt, W};
use crate::i18n::tr;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StripState {
    /// session id -> the status read's model label.
    pub model: Option<(String, String)>,
    pub status_for: Option<String>,
    pub status_model: Option<Identity>,
    pub status_turn: Option<String>,
    pub status_generation: u64,
    pub handover: Option<String>,
    /// The composer's measured width (the strip aligns to it).
    pub width: Option<f64>,
    /// A8 — a peer THIS app started holds the session's seat
    /// (`seatHolderKind` SELF, `seat-holder.ts:18-29`): its turn is never
    /// "another client", and with no peer row it still reads
    /// "Peers running (1)" (`App.tsx:2084-2086`).
    pub self_held: bool,
    /// A15 — session id -> the status stamp's `approval_policy`
    /// (`runtime_policy_stamp.approval_policy`: `on-request` | `never`), the
    /// web's approval readback (`App.tsx:3228-3234`).
    pub approval: Option<(String, String)>,
}

fn selected_model(models: &[ProfileLlmModel]) -> Option<Identity> {
    models.iter().find(|m| m.selected).map(|m| Identity {
        model: m.model.clone(),
        provider: m.provider.clone(),
        route: m.route.clone(),
    })
}

impl StripState {
    /// Re-read after a selection or turn changes, including the end of a
    /// response that was still using the previous model. The selected model
    /// invalidates the cache; only a server status reply supplies its label.
    pub fn needs_status(&mut self, session: &str, models: &[ProfileLlmModel], turn: Option<&str>) -> bool {
        let selected = selected_model(models);
        if self.status_for.as_deref() == Some(session)
            && self.status_model == selected
            && self.status_turn.as_deref() == turn
        {
            return false;
        }
        self.status_for = Some(session.to_owned());
        self.status_model = selected;
        self.status_turn = turn.map(str::to_owned);
        self.status_generation += 1;
        true
    }

    fn apply_status(&mut self, session: &str, generation: u64, value: &serde_json::Value) -> Option<super::session_pane::StatusFacts> {
        if self.status_generation != generation {
            return None;
        }
        let facts = super::session_pane::parse_status(value, session)?;
        // A missing runtime is meaningful (for example a failed reload),
        // so it must clear the old model instead of keeping a stale label.
        self.model = facts.model.as_ref().map(|m| (session.to_owned(), m.clone()));
        self.approval = facts.approval_policy.as_ref().map(|a| (session.to_owned(), a.clone()));
        Some(facts)
    }
}

/// A15 — the approval policy the last status read reported for `session`.
pub fn stamp_approval(session: &str) -> Option<String> {
    super::host::state()
        .strip
        .approval
        .as_ref()
        .filter(|(s, _)| s == session)
        .map(|(_, a)| a.clone())
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

/// The state word by the web's precedence (`App.tsx:2047-2088`, words
/// `SessionStatusStrip.tsx:62-82`): not connected or an unhealthy recovery ->
/// Reconnecting; a seat handover; a pending approval; a pending question; a
/// FOREIGN seat holder; the live turn (another client's turn ->
/// "Another client is working in this session", our own -> its live
/// activity word, else Responding); running peers; our own held seat; Ready.
pub fn state_word(store: &Store, active_turn: Option<&str>, handover: Option<&str>) -> String {
    state_word_held(store, active_turn, handover, false)
}

/// [`state_word`] with the self-held fact (`StripState::self_held`).
pub fn state_word_held(store: &Store, active_turn: Option<&str>, handover: Option<&str>, self_held: bool) -> String {
    if !store.is_live() {
        return "Reconnecting".into();
    }
    let session = store.active_session().unwrap_or_default();
    if store.domains.config.recovery(&session).phase != octoscode_store::domains::config::LossyPhase::Healthy {
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
    if store.domains.approval.question_for(&session).is_some() {
        return "Waiting for your answer".into();
    }
    if crate::chrome::held_by_other(store).is_some() {
        return "Another app is using this session".into();
    }
    if let Some(turn) = active_turn {
        // `origin === "adopted" && !selfSeatHeld`: a turn this client never
        // dispatched belongs to another attached client.
        if !crate::flow::is_own_turn(turn) && !self_held {
            return "Another client is working in this session".into();
        }
        return activity_word(store, &session, turn).unwrap_or_else(|| "Responding".into());
    }
    // "Peers running (n)" counts the peer manager's ROSTER rows that are
    // opening or started (`App.tsx:2071-2083`: `peers.manager.snapshot()
    // .peers` with status `opening` | `started`). A14: never the staged map
    // `store.domains.peer.list()`, which also holds the Profile BLACKBOARD
    // rows a `peer/gather` folds (`fleet::fold_peer_gather`) — the web keeps
    // those apart ("Gathering never creates an owned session or opens a
    // peer", `peer-manager.ts:266-279`), so a gathered `r6-smoke` read
    // "Peers running (1)" beside a Fleet slice of 0 peers.
    let peers = store
        .domains
        .peer
        .rows()
        .iter()
        .filter(|r| matches!(r.status, octoscode_store::domains::peer::RowStatus::Opening | octoscode_store::domains::peer::RowStatus::Started))
        .count();
    if peers > 0 {
        return format!("Peers running ({peers})");
    }
    if self_held {
        return "Peers running (1)".into();
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
    (model, state_word_held(store, active_turn, st.handover.as_deref(), st.self_held), perm)
}

/// `session/status/read` -> the model label for the active Session.
///
/// A15 — the strip's model is the web's: the RUNTIME model the status read
/// reports (`App.tsx:1999-2000` `runtimeModelLabel`, passed as the strip's
/// `model`, `:2974-2980`), while the model seat names the profile's selected
/// model (`profile/llm/list`). The read names the Profile: the server
/// resolves `params.profile_id`, else the one embedded in a full
/// `<profile>:<channel>:<chat>` key, else the connection's
/// (`raw_profile_id`, octos `ui_protocol_transport.rs:9676-9686`). The web's
/// ids always embed it; this app's startup Session (`<profile>:main`) does
/// not, so a token connection fell back to `_main` ("profile '_main' is not
/// configured for this AppUI session") and the strip read "Model not
/// reported" beside a seat that knew the model. The Session pane and the
/// context read already send it (`session_pane.rs`, `models.rs`).
pub async fn load_status(conv: &crate::flow::Conversation) -> Result<String, String> {
    let session = conv.session_id();
    let scope = conv.resource_identity();
    let generation = super::host::state().strip.status_generation;
    let selected = selected_model(&conv.store.domains.profile.llm_models());
    let v = conv
        .client()
        .request("session/status/read", serde_json::json!({ "session_id": session, "profile_id": conv.profile() }))
        .await
        .map_err(|e| e.to_string())?;
    // The identity check the web performs (`session-status-result.ts:6`).
    if v.get("session_id").and_then(|s| s.as_str()) != Some(session.as_str()) {
        return Err("status belongs to another Session".into());
    }
    if conv.resource_identity() != scope || selected_model(&conv.store.domains.profile.llm_models()) != selected {
        return Ok("stale status read dropped".into());
    }
    let model = {
        let mut st = super::host::state();
        let Some(facts) = st.strip.apply_status(&session, generation, &v) else {
            return Ok("stale status read dropped".into());
        };
        let model = facts.model.clone();
        if st.pane.session == session {
            st.pane.status = Some(facts);
        }
        model
    };
    // A8 — the permission fact: the web reads the session's permission
    // profile when it opens (`refreshPermission`, use-octos-session's open
    // path), so the strip names the mode from the start instead of
    // "Permissions not reported" until the pane is opened.
    if conv.store.domains.config.supported_methods().iter().any(|m| m == "permission/profile/list") {
        use octoscode_client::domains::profile::PermissionProfileList;
        if let Ok(r) = conv
            .client()
            .call::<PermissionProfileList>(octos_core::ui_protocol::PermissionProfileListParams {
                session_id: octos_core::SessionKey(session.clone()),
            })
            .await
        {
            if r.session_id.0 == session && conv.resource_identity() == scope
                && super::host::state().strip.status_generation == generation {
                use octoscode_store::domains::profile::PermissionProfileSelection as Sel;
                let sel = |s: &octos_core::ui_protocol::PermissionProfileSelection| {
                    serde_json::to_value(s).ok().and_then(|v| serde_json::from_value::<Sel>(v).ok())
                };
                if let Some(cur) = sel(&r.current) {
                    let profiles: Vec<Sel> = r.profiles.iter().filter_map(sel).collect();
                    conv.store.domains.profile.set_permission(cur, profiles);
                }
            }
        }
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
    // Three EQUAL cells (the board): a Fill cell shrank to its neighbours'
    // leftovers on a phone and clipped "Permissions not reported".
    let split = |n: f64| {
        st.width
            .map(|w| format!("{}", ((w - (n - 1.0)) / n).floor()))
            .unwrap_or_else(|| "Fill".into())
    };
    let cell = |d: &mut Dsl, id: &str, text: &str, muted: bool, cell_w: &str| {
        d.view(
            &format!("{id}_cell"),
            &format!("width: {cell_w} height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 10 right: 8 top: 8 bottom: 8}}"),
        );
        d.text(id, text, &Txt::new(px, Face::Regular, if muted { tok::FAINT } else { tok::TEXT }).w(W::Fill).wrap());
        d.close();
    };
    let model_missing = model == "Model not reported";
    let perm_missing = perm == "Permissions not reported";
    // A24 — the strip in the current language: its fixed words and labels
    // (the model's own name is server data and stays as reported).
    let model = if model_missing { tr(&model).to_owned() } else { model };
    let perm = tr(&perm).to_owned();
    let state_shown = state_display(&state);
    if narrow {
        // A8 — a phone width is the web's <=760 px strip
        // (`SessionConfig.module.css:54-75`): the state word first, on its
        // own line, the model and the permission under it. Three cells in a
        // row broke "deepseek-v4-flash" and "Workspace write" mid-word at
        // 360 px.
        let col = d.anon();
        d.view(&col, "width: Fill height: Fit flow: Down");
        cell(&mut d, "b3_strip_state", &state_shown, false, &split(1.0));
        let line = d.anon();
        d.rule(&line, "width: Fill height: 1", tok::HAIRLINE);
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5}");
        let half = split(2.0);
        cell(&mut d, "b3_strip_model", &model, model_missing, &half);
        d.vrule(26.0);
        cell(&mut d, "b3_strip_perm", &perm, perm_missing, &half);
        d.close();
        d.close();
    } else {
        let row = d.anon();
        d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5}");
        let third = split(3.0);
        cell(&mut d, "b3_strip_model", &model, model_missing, &third);
        d.vrule(26.0);
        cell(&mut d, "b3_strip_state", &state_shown, false, &third);
        d.vrule(26.0);
        cell(&mut d, "b3_strip_perm", &perm, perm_missing, &third);
        d.close();
    }
    d.tap("b3_strip_tap", "b3.strip.settings");
    d.close();
    // The board's caption under the strip is the web strip's own title
    // (`SessionStatusStrip.tsx:106`, "Model, permissions, sandbox"); on its
    // right, the composer's Vim field note while Vim editing is on
    // (`ComposerInput.tsx:270-274`: `Vim · Insert` / `Vim · Normal`).
    // A13 (judge: phone chat chrome stacked up above the composer): a phone
    // drops the caption line — the web shows it only as the strip's tooltip
    // (`SessionStatusStrip.tsx:106` `title=`), and the strip itself says
    // what it is. The Vim note keeps the line.
    if !narrow || vim_note.is_some() {
        let caption = d.anon();
        d.view(
            &caption,
            &format!("width: {width} height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 2 right: 2}}"),
        );
        // A Fill run before a Fit one takes the whole row (the flow is one
        // pass), so with the note present the caption gets an explicit width.
        let caption_w = match (vim_note, st.width) {
            (Some(note), Some(w)) => W::Px((w - 4.0 - super::ui::text_w(note, 12.0, Face::Medium) - 8.0).max(60.0)),
            (Some(_), None) => W::Px(super::ui::text_w(tr("Model, permissions, sandbox"), 12.0, Face::Regular) + 12.0),
            _ => W::Fill,
        };
        d.text("b3_strip_caption", tr("Model, permissions, sandbox"), &Txt::new(12.0, Face::Regular, tok::MUTED).w(caption_w));
        if let Some(note) = vim_note {
            d.text("b3_strip_vim", note, &Txt::new(12.0, Face::Medium, tok::TEXT));
        }
        d.close();
    }
    // The transitional states read in blue under it (the board's
    // "Reconnecting" / "Resuming chat…" / "Handing back control…").
    if matches!(state.as_str(), "Reconnecting" | "Resuming chat…" | "Handing back control…") {
        d.text("b3_strip_transition", &state_shown, &Txt::new(12.5, Face::Regular, tok::BLUE_TEXT));
    }
    d.close();
    // A18 — the strip and its caption sit on the composer's surface, which
    // follows the theme (the byte passthrough in light): unmapped, a dark
    // composer area showed a white strip and its caption at 3.26:1.
    crate::screens::theme::retint_dsl(&d.finish())
}

/// A24 — the state word as shown: the web's key for it in the current
/// language (`SessionStatusStrip.tsx:60/91`: "Peers running ({count})" and
/// "Running {value0}…" carry their count / tool name).
fn state_display(state: &str) -> String {
    if let Some(n) = state.strip_prefix("Peers running (").and_then(|r| r.strip_suffix(')')) {
        return crate::i18n::tr_with("Peers running ({count})", &[("count", n)]);
    }
    if let Some(tool) = state.strip_prefix("Running ").and_then(|r| r.strip_suffix('…')) {
        return crate::i18n::tr1("Running {value0}…", tool);
    }
    tr(state).to_owned()
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

    fn select(store: &Store, model: &str, provider: &str, route: &str) {
        store.domains.profile.set_llm_models(vec![ProfileLlmModel {
            model: model.into(), provider: provider.into(), title: model.into(),
            family: Some(provider.into()), route: Some(route.into()),
            selected: true, available: true,
        }]);
    }

    fn status(model: &str, provider: &str) -> serde_json::Value {
        serde_json::json!({"session_id": "s", "model": {"model": model, "provider": provider}})
    }

    #[test]
    fn composer_runtime_refreshes_after_model_and_route_selection() {
        let store = live_store();
        let mut strip = StripState::default();
        select(&store, "deepseek-v4-flash", "deepseek", "official");
        assert!(strip.needs_status("s", &store.domains.profile.llm_models(), None));
        strip.apply_status("s", strip.status_generation, &status("deepseek-v4-flash", "deepseek")).unwrap();
        assert!(!strip.needs_status("s", &store.domains.profile.llm_models(), None));

        select(&store, "k3", "moonshot-coding", "coding");
        assert_eq!(super::super::seats::model_seat_label(&store), "k3");
        assert!(strip.needs_status("s", &store.domains.profile.llm_models(), None), "the selected model invalidates runtime status");
        // Persisted selection alone is not evidence of a live reload.
        assert_eq!(facts(&store, &strip, None, None).0, "deepseek-v4-flash");
        strip.apply_status("s", strip.status_generation, &status("k3", "moonshot-coding")).unwrap();
        assert_eq!(facts(&store, &strip, None, None).0, "k3");
        assert!(!strip.needs_status("s", &store.domains.profile.llm_models(), None));

        select(&store, "k3", "moonshot-coding", "other-route");
        assert!(strip.needs_status("s", &store.domains.profile.llm_models(), None), "route-only changes refresh too");
        select(&store, "k3", "other-provider", "other-route");
        assert!(strip.needs_status("s", &store.domains.profile.llm_models(), None), "provider identity is significant");
    }

    #[test]
    fn composer_runtime_rejects_status_from_before_the_switch() {
        let store = live_store();
        let mut strip = StripState::default();
        select(&store, "deepseek-v4-flash", "deepseek", "official");
        strip.needs_status("s", &store.domains.profile.llm_models(), None);
        let old = strip.status_generation;
        select(&store, "k3", "moonshot-coding", "coding");
        strip.needs_status("s", &store.domains.profile.llm_models(), None);
        strip.apply_status("s", strip.status_generation, &status("k3", "moonshot-coding")).unwrap();
        assert!(strip.apply_status("s", old, &status("deepseek-v4-flash", "deepseek")).is_none());
        assert_eq!(facts(&store, &strip, None, None).0, "k3");
        let other = serde_json::json!({"session_id": "other", "model": {"model": "wrong", "provider": "other"}});
        assert!(strip.apply_status("s", strip.status_generation, &other).is_none());
        assert_eq!(facts(&store, &strip, None, None).0, "k3");
    }

    #[test]
    fn composer_runtime_waits_for_live_status_and_refreshes_when_turn_finishes() {
        let store = live_store();
        let mut strip = StripState::default();
        select(&store, "k3", "moonshot-coding", "coding");
        assert!(strip.needs_status("s", &store.domains.profile.llm_models(), Some("old-turn")));
        // Deferred/restart-required changes must keep the runtime the server reports.
        strip.apply_status("s", strip.status_generation, &status("deepseek-v4-flash", "deepseek")).unwrap();
        assert_eq!(facts(&store, &strip, Some("old-turn"), None).0, "deepseek-v4-flash");
        assert!(strip.needs_status("s", &store.domains.profile.llm_models(), None));
        strip.apply_status("s", strip.status_generation, &status("k3", "moonshot-coding")).unwrap();
        assert_eq!(facts(&store, &strip, None, None).0, "k3");
        assert!(strip.needs_status("s", &store.domains.profile.llm_models(), Some("next-turn")));
        strip.apply_status("s", strip.status_generation, &serde_json::json!({"session_id": "s"})).unwrap();
        assert_eq!(facts(&store, &strip, None, None).0, "Model not reported", "a missing runtime clears the stale model");
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
        crate::flow::forget_own_turns();
        let s = Store::new();
        assert_eq!(state_word(&s, None, None), "Reconnecting", "not connected first");
        let s = live_store();
        assert_eq!(state_word(&s, None, Some("Resuming chat…")), "Resuming chat…");
        s.domains.session.timeline.append("s", Some("t".into()), EntryKind::REASONING, "hmm".into());
        assert_eq!(state_word(&s, Some("t"), None), "Another client is working in this session", "not dispatched here");
        crate::flow::note_own_turn("t");
        crate::flow::note_own_turn("other");
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
    fn a_phone_width_puts_the_state_word_over_the_model_and_the_permission() {
        let s = live_store();
        let st = |w: f64| StripState { width: Some(w), ..Default::default() };
        let phone = lower(&s, &st(330.0), None, Some("workspace_write"), None);
        let at = |dsl: &str, id: &str| dsl.find(&format!("{id} := View")).unwrap();
        assert!(at(&phone, "b3_strip_state_cell") < at(&phone, "b3_strip_model_cell"), "the state word first");
        assert!(phone.contains("b3_strip_state_cell := View {\nwidth: 330 "), "on its own full-width line");
        for id in ["b3_strip_model_cell", "b3_strip_perm_cell"] {
            assert!(phone.contains(&format!("{id} := View {{\nwidth: 164 ")), "{id}: half the strip");
        }
        assert_eq!(phone.matches('{').count(), phone.matches('}').count());
        let desk = lower(&s, &st(660.0), None, Some("workspace_write"), None);
        assert!(at(&desk, "b3_strip_model_cell") < at(&desk, "b3_strip_state_cell"), "desktop: model | state | permission");
        for id in ["b3_strip_model_cell", "b3_strip_state_cell", "b3_strip_perm_cell"] {
            assert!(desk.contains(&format!("{id} := View {{\nwidth: 219 ")), "{id}: a third");
        }
        // A13: the caption line is the desktop's; a phone drops it (the web's
        // tooltip) unless the Vim note needs the line.
        assert!(desk.contains("b3_strip_caption := Label"));
        assert!(!phone.contains("b3_strip_caption"), "no caption line on a phone");
        let vim = lower(&s, &st(330.0), None, Some("workspace_write"), Some("Vim · Insert"));
        assert!(vim.contains("b3_strip_vim := Label") && vim.contains("b3_strip_caption"), "the Vim note keeps its line");
    }

    /// A14 — "Peers running (n)" counts the peer manager's roster rows that
    /// are opening or started (`App.tsx:2071-2083`), never a Profile
    /// blackboard row a `peer/gather` folded (`peer-manager.ts:266-279`); a
    /// self-held seat with no row still reads "(1)" (`:2084-2086`).
    #[test]
    fn peers_running_counts_the_roster_not_the_blackboard() {
        use octoscode_store::domains::peer::{Origin, PeerRow};
        let s = live_store();
        crate::screens::fleet::fold_peer_gather(
            serde_json::json!({"peers": [{"slug": "r6-smoke", "topic": "peer-r6-smoke", "closed": false}], "profile_id": "dsflash"}),
            &s,
        );
        assert_eq!(s.domains.peer.list().len(), 1, "the gathered row is folded");
        assert_eq!(state_word(&s, None, None), "Ready", "a blackboard row is not a running peer");
        assert_eq!(state_word_held(&s, None, None, true), "Peers running (1)", "the self-held seat");
        assert!(s.domains.peer.stage_row(PeerRow::opening("dsflash:main#peer-a", "a", Origin::Dispatch, "t", 1_000), false));
        assert_eq!(state_word(&s, None, None), "Peers running (1)", "an opening roster row runs");
    }

    #[test]
    fn the_strip_lowers_balanced_with_its_settings_tap() {
        let s = live_store();
        let dsl = lower(&s, &StripState::default(), None, Some("read_only"), None);
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        assert!(crate::screens::taps::wired_taps(&dsl).iter().any(|(_, e)| e == "b3.strip.settings"));
    }
}
