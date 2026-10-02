//! A10 — the model-selection notice board, kept per Session (the web keeps
//! it "on the session record"): `features/models/model-notices.ts` —
//! [`parse_runtime_disposition`] (`:47-80`), [`notice_message`] (the five
//! §4.2 messages exactly, plus the legacy-persisted and refused fallbacks,
//! `:105-138`) and [`next_board`] (the sticky rules, `:189-283`):
//!
//! - `deferred` clears when a turn stamp shows the model, or a `reloaded`
//!   result arrives for the same selection;
//! - `persisted_but_not_live` is sticky: neither a matching turn stamp nor a
//!   list refresh clears it — only a later `reloaded` for that selection;
//! - `restart_required` persists until restart; `unchanged` keeps every
//!   outstanding notice; a `refused` save records the failure and reverts
//!   the selection;
//! - a list refresh that shows another saved selection than the one last
//!   seen flags an external change ("case 23").
use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

/// The §4.2 `profile/llm/select` runtime dispositions plus the two derived
/// kinds (`ModelRuntimeDisposition`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Legacy servers: `applied` without a disposition (persisted only).
    Persisted,
    Reloaded,
    Deferred,
    RestartRequired,
    PersistedButNotLive,
    Unchanged,
    /// The select was refused (an error, or an unknown disposition).
    Refused,
}

impl Disposition {
    pub fn wire(self) -> &'static str {
        match self {
            Disposition::Persisted => "persisted",
            Disposition::Reloaded => "reloaded",
            Disposition::Deferred => "deferred",
            Disposition::RestartRequired => "restart_required",
            Disposition::PersistedButNotLive => "persisted_but_not_live",
            Disposition::Unchanged => "unchanged",
            Disposition::Refused => "refused",
        }
    }
}

/// What [`parse_runtime_disposition`] read off one select result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDisposition {
    pub disposition: Disposition,
    /// `persisted_but_not_live` reason.
    pub runtime_error: Option<String>,
    /// `deferred` condition, when the server returned one.
    pub condition: Option<String>,
}

/// `parseRuntimeDisposition`: `runtime_disposition` off the raw result; a
/// missing one with `applied: true` is the legacy persisted contract; an
/// unknown string is a refusal; anything else is `None`.
pub fn parse_runtime_disposition(raw: &Value) -> Option<RuntimeDisposition> {
    let record = raw.as_object()?;
    let text = |k: &str| record.get(k).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(str::to_owned);
    let Some(d) = record.get("runtime_disposition") else {
        return (record.get("applied") == Some(&Value::Bool(true))).then(|| RuntimeDisposition {
            disposition: Disposition::Persisted,
            runtime_error: None,
            condition: None,
        });
    };
    let d = d.as_str()?;
    let disposition = match d {
        "reloaded" => Disposition::Reloaded,
        "deferred" => Disposition::Deferred,
        "restart_required" => Disposition::RestartRequired,
        "persisted_but_not_live" => Disposition::PersistedButNotLive,
        "unchanged" => Disposition::Unchanged,
        _ => {
            return Some(RuntimeDisposition { disposition: Disposition::Refused, runtime_error: None, condition: None })
        }
    };
    Some(RuntimeDisposition { disposition, runtime_error: text("runtime_error"), condition: text("condition") })
}

/// The saved/running selection identity the board compares
/// (`ModelSelectionIdentity`); a missing route equals an empty one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identity {
    pub model: String,
    pub provider: String,
    pub route: Option<String>,
}

impl Identity {
    pub fn same(&self, other: &Identity) -> bool {
        self.model == other.model
            && self.provider == other.provider
            && self.route.as_deref().unwrap_or("") == other.route.as_deref().unwrap_or("")
    }
}

fn same(a: Option<&Identity>, b: Option<&Identity>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if a.same(b))
}

/// `DispositionMessageInput`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageInput {
    pub saved_model: Option<String>,
    pub condition: Option<String>,
    pub running_model: Option<String>,
    pub runtime_error: Option<String>,
    pub reason: Option<String>,
}

/// `noticeMessage` — the exact copy (en).
pub fn notice_message(disposition: Disposition, i: &MessageInput) -> String {
    match disposition {
        Disposition::Reloaded => {
            format!("Saved. Your next message uses {}", i.saved_model.as_deref().unwrap_or("the new model"))
        }
        Disposition::Deferred => match i.condition.as_deref() {
            Some(c) => format!("Saved. The model is not active yet ({c})"),
            None => "Saved. The model is not active yet".to_owned(),
        },
        Disposition::RestartRequired => format!(
            "Saved. The server keeps running {} until it restarts",
            i.running_model.as_deref().unwrap_or("the previous model")
        ),
        Disposition::PersistedButNotLive => format!(
            "Saved, but not usable right now: {}",
            i.runtime_error.as_deref().unwrap_or("the runtime could not start")
        ),
        Disposition::Unchanged => "Already selected".to_owned(),
        Disposition::Persisted => "Saved".to_owned(),
        Disposition::Refused => {
            format!("Couldn't save: {}", i.reason.as_deref().unwrap_or("the server refused the change"))
        }
    }
}

/// One sticky notice with its time (`DispositionNoticeOutcome`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub kind: Disposition,
    pub model: Option<Identity>,
    pub message: String,
    /// `restart_required`: the model the boot snapshot keeps serving.
    pub running_model: Option<String>,
    pub at_ms: u64,
}

/// `ModelNoticeBoard`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Board {
    pub notices: Vec<Notice>,
    /// Right after a refused save (the selection reverts).
    pub selection_reverted: bool,
    /// A select is in flight ("Saving…"; transient, not a notice).
    pub saving: bool,
    /// A list refresh showed another saved selection than the last seen.
    pub external_change: bool,
}

impl Board {
    /// The line the surface shows (the web renders the LAST notice).
    pub fn latest(&self) -> Option<&Notice> {
        self.notices.last()
    }
}

/// `ModelNoticeEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Disposition {
        disposition: Disposition,
        saved_model: Option<Identity>,
        input: MessageInput,
        at_ms: u64,
    },
    /// A running turn's stamp shows this model.
    TurnStamp { model: Option<Identity>, at_ms: u64 },
    /// A `profile/llm/list` refresh: (identity, selected) rows, and the
    /// selection last seen before it.
    ListRefreshed { models: Vec<(Identity, bool)>, last_seen: Option<Identity>, at_ms: u64 },
    Saving(bool),
    ClearSelectionReverted,
}

/// `nextModelNoticeBoard` — the sticky rules.
pub fn next_board(board: &Board, event: Event) -> Board {
    match event {
        Event::Saving(saving) => Board { saving, ..board.clone() },
        Event::ClearSelectionReverted => Board { selection_reverted: false, ..board.clone() },
        Event::TurnStamp { model, .. } => Board {
            notices: board
                .notices
                .iter()
                .filter(|n| n.kind != Disposition::Deferred || !same(n.model.as_ref(), model.as_ref()))
                .cloned()
                .collect(),
            ..board.clone()
        },
        Event::ListRefreshed { models, last_seen, .. } => {
            let selected = models.iter().find(|(_, s)| *s).map(|(i, _)| i.clone());
            // A refresh never clears persisted_but_not_live (the list carries
            // no runtime-error field, so its absence proves nothing).
            Board {
                external_change: last_seen.is_some() && !same(last_seen.as_ref(), selected.as_ref()),
                ..board.clone()
            }
        }
        Event::Disposition { disposition, saved_model, mut input, at_ms } => {
            if disposition == Disposition::Unchanged {
                return board.clone();
            }
            if input.saved_model.is_none() {
                input.saved_model = saved_model.as_ref().map(|m| m.model.clone());
            }
            let message = notice_message(disposition, &input);
            if disposition == Disposition::Refused {
                let mut notices = board.notices.clone();
                notices.push(Notice { kind: disposition, model: saved_model, message, running_model: None, at_ms });
                return Board { notices, selection_reverted: true, ..board.clone() };
            }
            // `reloaded` for a selection proves the server runs it: it clears
            // that selection's deferred / persisted_but_not_live /
            // restart_required notices; other selections' notices stay.
            let mut notices: Vec<Notice> = if disposition == Disposition::Reloaded {
                board.notices.iter().filter(|n| !same(n.model.as_ref(), saved_model.as_ref())).cloned().collect()
            } else {
                board.notices.clone()
            };
            notices.push(Notice { kind: disposition, model: saved_model, message, running_model: input.running_model, at_ms });
            Board { notices, selection_reverted: false, ..board.clone() }
        }
    }
}

/// The per-Session boards (+ the selection last seen per Session).
#[derive(Debug, Default)]
pub struct ModelNotices {
    inner: Mutex<HashMap<String, (Board, Option<Identity>)>>,
}

impl ModelNotices {
    pub fn board(&self, session: &str) -> Board {
        self.inner.lock().unwrap().get(session).map(|(b, _)| b.clone()).unwrap_or_default()
    }

    /// Fold one event into `session`'s board; returns the new board.
    pub fn apply(&self, session: &str, event: Event) -> Board {
        let mut inner = self.inner.lock().unwrap();
        let entry = inner.entry(session.to_owned()).or_default();
        entry.0 = next_board(&entry.0, event);
        entry.0.clone()
    }

    pub fn last_seen(&self, session: &str) -> Option<Identity> {
        self.inner.lock().unwrap().get(session).and_then(|(_, s)| s.clone())
    }

    pub fn set_last_seen(&self, session: &str, identity: Option<Identity>) {
        self.inner.lock().unwrap().entry(session.to_owned()).or_default().1 = identity;
    }

    /// A new transport authority (reconnect): every board starts empty.
    pub fn reset(&self) {
        self.inner.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn glm() -> Identity {
        Identity { model: "glm-5.3".into(), provider: "zai".into(), route: Some("official".into()) }
    }

    fn notice(kind: Disposition, at: u64) -> Notice {
        Notice { kind, model: Some(glm()), message: format!("{kind:?}"), running_model: None, at_ms: at }
    }

    fn disposition(d: Disposition, at: u64) -> Event {
        Event::Disposition { disposition: d, saved_model: Some(glm()), input: MessageInput::default(), at_ms: at }
    }

    #[test]
    fn parse_maps_every_wire_disposition() {
        for (w, d) in [
            ("reloaded", Disposition::Reloaded),
            ("deferred", Disposition::Deferred),
            ("restart_required", Disposition::RestartRequired),
            ("persisted_but_not_live", Disposition::PersistedButNotLive),
            ("unchanged", Disposition::Unchanged),
            ("something_new", Disposition::Refused),
        ] {
            assert_eq!(parse_runtime_disposition(&json!({ "runtime_disposition": w })).unwrap().disposition, d);
        }
        assert_eq!(parse_runtime_disposition(&json!({ "applied": true })).unwrap().disposition, Disposition::Persisted);
        assert_eq!(parse_runtime_disposition(&json!({})), None);
        let p = parse_runtime_disposition(&json!({"runtime_disposition": "persisted_but_not_live", "runtime_error": "no provider for family"}))
            .unwrap();
        assert_eq!(p.runtime_error.as_deref(), Some("no provider for family"));
        let p = parse_runtime_disposition(&json!({"runtime_disposition": "deferred", "condition": "profile disabled"})).unwrap();
        assert_eq!(p.condition.as_deref(), Some("profile disabled"));
    }

    #[test]
    fn the_messages_are_the_webs_exactly() {
        let m = |d, i: MessageInput| notice_message(d, &i);
        assert_eq!(m(Disposition::Reloaded, MessageInput { saved_model: Some("glm-5.3".into()), ..Default::default() }), "Saved. Your next message uses glm-5.3");
        assert_eq!(m(Disposition::Deferred, MessageInput { condition: Some("profile disabled".into()), ..Default::default() }), "Saved. The model is not active yet (profile disabled)");
        assert_eq!(m(Disposition::Deferred, MessageInput::default()), "Saved. The model is not active yet");
        assert_eq!(m(Disposition::RestartRequired, MessageInput { running_model: Some("glm-4.7".into()), ..Default::default() }), "Saved. The server keeps running glm-4.7 until it restarts");
        assert_eq!(m(Disposition::PersistedButNotLive, MessageInput { runtime_error: Some("no provider for family".into()), ..Default::default() }), "Saved, but not usable right now: no provider for family");
        assert_eq!(m(Disposition::Unchanged, MessageInput::default()), "Already selected");
        assert_eq!(m(Disposition::Refused, MessageInput { reason: Some("read-only profile".into()), ..Default::default() }), "Couldn't save: read-only profile");
    }

    #[test]
    fn the_sticky_rules_are_the_webs() {
        // reloaded clears that selection's deferred + persisted_but_not_live.
        let b = Board { notices: vec![notice(Disposition::Deferred, 1), notice(Disposition::PersistedButNotLive, 2)], ..Default::default() };
        let b = next_board(&b, disposition(Disposition::Reloaded, 3));
        assert!(b.notices.iter().all(|n| n.kind == Disposition::Reloaded));
        // a turn stamp clears deferred, never persisted_but_not_live.
        let b = Board { notices: vec![notice(Disposition::Deferred, 1)], ..Default::default() };
        assert!(next_board(&b, Event::TurnStamp { model: Some(glm()), at_ms: 5 }).notices.is_empty());
        let sticky = Board { notices: vec![notice(Disposition::PersistedButNotLive, 1)], ..Default::default() };
        assert_eq!(next_board(&sticky, Event::TurnStamp { model: Some(glm()), at_ms: 5 }), sticky);
        // a list refresh does not clear it; unchanged keeps it; reloaded does.
        let b = next_board(&sticky, Event::ListRefreshed { models: vec![(glm(), true)], last_seen: None, at_ms: 2 });
        assert_eq!(b.notices.len(), 1);
        let b = next_board(&b, disposition(Disposition::Unchanged, 3));
        assert_eq!(b.notices.len(), 1);
        let b = next_board(&b, disposition(Disposition::Reloaded, 4));
        assert!(b.notices.iter().all(|n| n.kind == Disposition::Reloaded));
        // refused records the failure and reverts the selection.
        let b = next_board(&Board::default(), Event::Disposition {
            disposition: Disposition::Refused,
            saved_model: None,
            input: MessageInput { reason: Some("read-only profile".into()), ..Default::default() },
            at_ms: 1,
        });
        assert_eq!(b.notices[0].message, "Couldn't save: read-only profile");
        assert!(b.selection_reverted);
        // saving is transient.
        let b = next_board(&Board::default(), Event::Saving(true));
        assert!(b.saving && b.notices.is_empty());
        // external change: the refreshed selection differs from the last seen.
        let other = Identity { model: "glm-4.7".into(), ..glm() };
        assert!(!next_board(&Board::default(), Event::ListRefreshed { models: vec![(glm(), true)], last_seen: Some(glm()), at_ms: 1 }).external_change);
        assert!(next_board(&Board::default(), Event::ListRefreshed { models: vec![(glm(), true)], last_seen: Some(other), at_ms: 1 }).external_change);
    }

    #[test]
    fn boards_are_per_session() {
        let n = ModelNotices::default();
        n.apply("a:main", disposition(Disposition::RestartRequired, 1));
        assert_eq!(n.board("a:main").notices.len(), 1);
        assert!(n.board("b:main").notices.is_empty(), "another Session's board is its own");
    }
}
