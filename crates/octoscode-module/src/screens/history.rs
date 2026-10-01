//! P4f1 — history: undo / rewind / fork, ported field-by-field from the web
//! oracle (`apps/web/src/features/history/checkpoints.ts:15-70`,
//! `history-binding.ts:53-227`, `history-coordinator.ts:99-335`; the call
//! shapes in `packages/client/src/history.ts:180-251`).
//!
//! Ported here, as pure functions plus thin async production paths:
//! * [`conversation_checkpoints`] — canonical `session/hydrate` messages grouped
//!   by distinct user-rooted `thread_id` (Core's `drop_last_n_user_turns`, NOT a
//!   user-message count), newest first, with preview + media/user-message counts.
//! * [`resolve_checkpoint`] — identity recomputed against FRESH history; a stale
//!   numerical index is refused (key+prefill must still match).
//! * [`history_supported`] — per-mode capability gate: every mode needs
//!   `session/hydrate` (each mutation ends in the canonical rehydrate); undo
//!   additionally needs `snapshot/list`+`snapshot/restore`, rewind
//!   `session/rollback`, fork `session/fork`+`session/open`.
//! * [`blocked_reason`] — the blocked-reason gate: a mutation waits until the
//!   affected turns, queued prompts, questions and unhealthy recovery settle;
//!   workspace undo additionally blocks busy sibling records in the same
//!   workspace (not another workspace).
//! * [`read_history`] — `session/hydrate` with `include: [messages, turns,
//!   pending_approvals]` for the BOUND session, rejecting a reply that belongs
//!   to another Session.
//! * [`undo_workspace_changes`] — `snapshot/list` → `snapshot/restore` →
//!   canonical rehydration of the owning record, with the web's freshness
//!   re-check and target-match validation.
//! * [`rewind_conversation`] — `session/rollback` (`num_turns`) → canonical
//!   rehydration; refuses once the Session became active.
//! * [`fork_conversation`] — `session/fork` (`new_chat_id`, optional
//!   `copy_messages`) → open the exact child in the background, never stealing
//!   focus and never sending a kickoff.
//!
//! The dialog surface itself (row 10) is a NEW SURFACE and stays on the design
//! flow; this file owns the state + production paths the dialog will call.
use serde_json::{json, Value};

use octoscode_client::domains::config::{SnapshotListMethod, SnapshotRestoreMethod};
use octoscode_client::domains::session::{SessionFork, SessionHydrate, SessionRollback};
use octoscode_client::Client;
use octoscode_store::Store;

/// The three history modes (`history-binding.ts:18`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryMode {
    Undo,
    Rewind,
    Fork,
}

impl HistoryMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Undo => "undo",
            Self::Rewind => "rewind",
            Self::Fork => "fork",
        }
    }

    /// The dialog title per mode (`HistoryDialog.tsx`): the web shows
    /// "Undo workspace changes" / "Rewind conversation" / "Fork conversation".
    pub fn title(self) -> &'static str {
        match self {
            Self::Undo => "Undo workspace changes",
            Self::Rewind => "Rewind conversation",
            Self::Fork => "Fork conversation",
        }
    }

    /// The completed notice per mode (`history-coordinator.ts:283-291`),
    /// verbatim: each mode states exactly what it did NOT change.
    pub fn completed_notice(self) -> &'static str {
        match self {
            Self::Undo => "Workspace snapshot restored. Conversation history was not changed.",
            Self::Rewind => "Conversation rewound. Workspace files were not restored.",
            Self::Fork => {
                "Conversation fork opened in the background. Your selection was not changed."
            }
        }
    }
}

// ------------------------------------------------------------ checkpoints

/// One rewind target (`ConversationCheckpoint`, `checkpoints.ts:6-13`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationCheckpoint {
    /// The stable identity: `messageKey(first message)` — NOT the index.
    pub key: String,
    /// 1-based, oldest first — the displayed ordinal. Never trusted for apply.
    pub checkpoint: usize,
    /// Whitespace-collapsed, 180-char preview.
    pub preview: String,
    /// The FULL first message text: the prefill the web hands back.
    pub prefill: String,
    pub media_count: usize,
    pub user_message_count: usize,
}

/// The web's `messageKey` (`checkpoints.ts:33-40`): the tuple that makes a
/// checkpoint addressable across rehydrations. Built as the same JSON array so
/// a `seq`/`thread_id` change produces a different key.
fn message_key(m: &Value) -> String {
    let thread_id = m.get("thread_id").and_then(|v| v.as_str()).unwrap_or("");
    let id = m
        .get("message_id")
        .and_then(|v| v.as_str())
        .or_else(|| m.get("client_message_id").and_then(|v| v.as_str()))
        .unwrap_or("");
    let seq = m.get("seq").and_then(|v| v.as_i64());
    let turn_id = m.get("turn_id").and_then(|v| v.as_str()).unwrap_or("");
    // `seq` is `undefined` when absent; the web stringifies `undefined` inside
    // the array, and `null` only when the field is present but non-numeric.
    let seq = match seq {
        Some(n) => n.to_string(),
        None => "null".to_owned(),
    };
    serde_json::to_string(&json!([thread_id, id, seq, turn_id])).unwrap_or_default()
}

fn message_seq(m: &Value) -> i64 {
    m.get("seq").and_then(|v| v.as_i64()).unwrap_or(i64::MIN)
}

/// The web's `userTurns` (`checkpoints.ts:15-32`): group by DISTINCT user-rooted
/// `thread_id`, sorted by `seq` first, and DROP unthreaded user messages (they
/// have no `thread_id`, so they can never be a rollback target). The order of
/// the map's values is the sorted-by-seq insertion order — stable, so the index
/// derived from it is the Core `drop_last_n_user_turns` count.
fn user_turns(messages: &[Value]) -> Vec<Vec<&Value>> {
    let mut sorted: Vec<&Value> = messages.iter().collect();
    sorted.sort_by_key(|m| message_seq(m));
    let mut order: Vec<String> = Vec::new();
    let mut groups: Vec<Vec<&Value>> = Vec::new();
    for m in sorted {
        let Some(role) = m.get("role").and_then(|v| v.as_str()) else {
            continue;
        };
        if !role.to_lowercase().eq("user") {
            continue;
        }
        let Some(thread_id) = m.get("thread_id").and_then(|v| v.as_str()) else {
            continue; // unthreaded: survives Core, but is not a checkpoint
        };
        match order.iter().position(|t| t == thread_id) {
            Some(i) => groups[i].push(m),
            None => {
                order.push(thread_id.to_owned());
                groups.push(vec![m]);
            }
        }
    }
    groups
}

/// `conversationCheckpoints` (`checkpoints.ts:41-57`): one checkpoint per
/// user-rooted thread, `checkpoint` = its 1-based oldest-first index, and the
/// list REVERSED (newest first) for the picker.
pub fn conversation_checkpoints(messages: &[Value]) -> Vec<ConversationCheckpoint> {
    let mut out: Vec<ConversationCheckpoint> = user_turns(messages)
        .into_iter()
        .enumerate()
        .map(|(index, group)| {
            let first = group[0];
            let content = first
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned();
            let preview = collapse_whitespace(&content);
            let preview = if preview.chars().count() > 180 {
                preview.chars().take(180).collect()
            } else {
                preview
            };
            let media_count = group
                .iter()
                .map(|m| {
                    m.get("media")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0)
                })
                .sum();
            ConversationCheckpoint {
                key: message_key(first),
                checkpoint: index + 1,
                preview,
                prefill: content,
                media_count,
                user_message_count: group.len(),
            }
        })
        .collect();
    out.reverse();
    out
}

/// `preview`: `content.replace(/\s+/g, " ").slice(0, 180)` (checkpoints.ts:48).
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_ws = false;
    for c in text.chars() {
        if c.is_whitespace() {
            // `\s+` collapses to a single space (including leading/trailing runs).
            if !in_ws {
                out.push(' ');
                in_ws = true;
            }
        } else {
            out.push(c);
            in_ws = false;
        }
    }
    out
}

/// The rewind target the web hands to `session/rollback`
/// (`resolveCheckpoint`, `checkpoints.ts:60-70`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewindTarget {
    /// `num_turns`: how many user-rooted turns Core drops, counted from the end.
    pub num_turns: usize,
    pub prefill: String,
}

/// Recompute the identity against FRESH canonical history
/// (`checkpoints.ts:60-70`): find the first user message of each group whose
/// `messageKey` equals the selected `key`; if the index is unknown **or** the
/// prefill text changed, return `None` — the stale numerical index is refused
/// rather than applied to a different turn.
pub fn resolve_checkpoint(
    messages: &[Value],
    selected: &ConversationCheckpoint,
) -> Option<RewindTarget> {
    let users: Vec<&Value> = user_turns(messages).into_iter().map(|g| g[0]).collect();
    let index = users
        .iter()
        .position(|m| message_key(m) == selected.key)?;
    let content = users[index].get("content").and_then(|v| v.as_str())?;
    if content != selected.prefill {
        return None; // same key, replaced text: refuse, do not guess
    }
    Some(RewindTarget {
        num_turns: users.len() - index,
        prefill: content.to_owned(),
    })
}

// ------------------------------------------------------------ the gates

/// `historySupported` (`history-binding.ts:53-67`): EVERY mode needs
/// `session/hydrate`, because every mutation ends in the engine's canonical
/// hydrate — including file undo. Then per mode: undo needs `snapshot/list` +
/// `snapshot/restore`; rewind `session/rollback`; fork `session/fork` +
/// `session/open`.
pub fn history_supported(supported: &[String], mode: HistoryMode) -> bool {
    let has = |m: &str| supported.iter().any(|x| x == m);
    if !has("session/hydrate") {
        return false;
    }
    match mode {
        HistoryMode::Undo => has("snapshot/list") && has("snapshot/restore"),
        HistoryMode::Rewind => has("session/rollback"),
        HistoryMode::Fork => has("session/fork") && has("session/open"),
    }
}

/// The advertised `session/open` the native client uses; exported so the gate
/// and the call site cannot drift.
pub const SESSION_OPEN: &str = "session/open";

/// `blockedReason` (`history-binding.ts:97-127`), ported field-by-field over
/// the native store's equivalent state. Returns the web's verbatim copy.
/// Order matters: authority → capability → busy state.
pub fn blocked_reason(store: &Store, session_id: &str, mode: HistoryMode) -> Option<String> {
    let supported = store.domains.config.supported_methods();
    if !history_supported(&supported, mode) {
        return Some(
            "This server does not advertise the required history methods.".to_owned(),
        );
    }
    // The web's "affected" set: for undo, every open record in the same
    // workspace (a file undo touches every conversation in it); for rewind/fork,
    // the bound record only. Ported onto the state the native store actually
    // keeps: a live turn, a standing question, or unsettled recovery.
    let question_pending = store
        .domains
        .approval
        .question()
        .map(|q| q.session_id == session_id)
        .unwrap_or(false);
    let recovery_unsettled = store.domains.config.recovery(session_id).phase
        != octoscode_store::domains::config::LossyPhase::Healthy;
    let busy = |id: &str| -> bool {
        // A turn in flight anywhere on this connection.
        store.domains.turn.in_flight_count() > 0
            || store
                .domains
                .session
                .list()
                .iter()
                .any(|s| s.id == id && s.active_turn)
            || (id == session_id && question_pending)
            || store.domains.config.recovery(id).phase
                != octoscode_store::domains::config::LossyPhase::Healthy
    };
    match mode {
        HistoryMode::Undo => {
            let workspace = store.domains.session.workspace_root(session_id);
            for sibling in store.domains.session.list() {
                if busy(&sibling.id)
                    && store.domains.session.workspace_root(&sibling.id) == workspace
                {
                    return Some(SETTLE.to_owned());
                }
            }
            // `session/list` may not carry the bound record itself; check it too.
            if busy(session_id) {
                return Some(SETTLE.to_owned());
            }
        }
        _ => {
            if busy(session_id) {
                return Some(SETTLE.to_owned());
            }
        }
    }
    // The web checks recovery per candidate as its own `state.recovery.phase`
    // arm; the bound record's own unsettled recovery blocks every mode.
    if recovery_unsettled {
        return Some(SETTLE.to_owned());
    }
    None
}

/// The web's verbatim blocked copy for unsettled work
/// (`history-binding.ts:121`).
const SETTLE: &str =
    "Wait for affected turns, queued prompts, and questions to settle before changing history.";

// ------------------------------------------------- canonical history read

/// `readHistory` (`history-binding.ts:141-155`): `session/hydrate` for the BOUND
/// session with the canonical include list, and the identity check that
/// rejects a reply belonging to another Session.
pub async fn read_history(client: &Client, session_id: &str) -> Result<Value, String> {
    let reply = client
        .call::<SessionHydrate>(octos_core::ui_protocol::SessionHydrateParams {
            session_id: octos_core::SessionKey(session_id.to_owned()),
            include: vec![
                "messages".to_owned(),
                "turns".to_owned(),
                "pending_approvals".to_owned(),
            ],
            after: None,
        })
        .await
        .map_err(|e| e.to_string())?;
    let reply = serde_json::to_value(&reply).map_err(|e| e.to_string())?;
    if session_of(&reply) != Some(session_id) {
        return Err("History belongs to another Session.".to_owned());
    }
    Ok(reply)
}

/// The reply's `session_id` as the web reads it (`thread.session_id`).
fn session_of(thread: &Value) -> Option<&str> {
    thread.get("session_id").and_then(|v| v.as_str())
}

/// The hydrated `messages` array (`thread.messages ?? []`).
fn messages_of(thread: &Value) -> &[Value] {
    thread
        .get("messages")
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

/// Whether the fresh history has an ACTIVE turn
/// (`fresh.turns?.some(t => t.state === "active")`, history-coordinator.ts:178).
pub fn has_active_turn(thread: &Value) -> bool {
    thread
        .get("turns")
        .and_then(|v| v.as_array())
        .map(|turns| {
            turns.iter().any(|t| {
                t.get("state").and_then(|v| v.as_str()) == Some("active")
            })
        })
        .unwrap_or(false)
}

/// The web's `validForkChatId` (`packages/client/src/history.ts`) — a
/// conversation name the fork may use. Rejects empty/whitespace and anything
/// with a path separator or control character.
pub fn valid_fork_chat_id(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name.len() <= 128
        && !name.contains(['/', '\\'])
        && !name.chars().any(|c| c.is_control())
}

// ------------------------------------------------------- the three mutations

/// Undo workspace changes (`history-coordinator.ts:161-186` + the web's
/// `history.ts:192-212`): `snapshot/list` → re-check the FRESH list still
/// advertises the target and is available → `snapshot/restore` → validate the
/// response restored EXACTLY the requested snapshot → canonical rehydration of
/// the owning record.
pub async fn undo_workspace_changes(
    client: &Client,
    store: &Store,
    session_id: &str,
    snapshot_id: &str,
) -> Result<HistoryOutcome, String> {
    if snapshot_id.is_empty() {
        return Err("A snapshot is required".to_owned());
    }
    let fresh: octoscode_client::domains::config::SnapshotListResult = client
        .call::<SnapshotListMethod>(octoscode_client::domains::config::SnapshotListParams {
            session_id: session_id.to_owned(),
        })
        .await
        .map_err(|e| e.to_string())?;
    if fresh.session_id != session_id {
        return Err("Snapshots belong to another Session.".to_owned());
    }
    // Freshness re-check: the target must still be offered.
    if !fresh.available
        || !fresh.snapshots.iter().any(|s| s.id == snapshot_id)
    {
        return Err("The selected snapshot is no longer available. Reload history.".to_owned());
    }
    let restored: octoscode_client::domains::config::SnapshotRestoreResult = client
        .call::<SnapshotRestoreMethod>(octoscode_client::domains::config::SnapshotRestoreParams {
            session_id: session_id.to_owned(),
            snapshot_id: snapshot_id.to_owned(),
        })
        .await
        .map_err(|e| e.to_string())?;
    if restored.session_id != session_id || restored.restored != snapshot_id {
        return Err("Snapshot restoration returned another target.".to_owned());
    }
    // The restore returns the NEW list; it replaces the folded projection.
    store
        .domains
        .config
        .set_snapshots(restored.clone().into_store());
    // Every mode ends in the canonical rehydrate of the OWNING record.
    let thread = read_history(client, session_id).await?;
    Ok(HistoryOutcome {
        notice: HistoryMode::Undo.completed_notice().to_owned(),
        thread,
        prefill: None,
        forked_session_id: None,
    })
}

/// Rewind conversation (`history-coordinator.ts:187-195`): read FRESH history,
/// refuse when the Session became active, recompute the checkpoint identity,
/// then `session/rollback` with `num_turns` and canonical rehydration. The
/// prefill is returned for the composer, never written into the selected draft.
pub async fn rewind_conversation(
    client: &Client,
    store: &Store,
    session_id: &str,
    selected: &ConversationCheckpoint,
) -> Result<HistoryOutcome, String> {
    let _ = store; // the gate is `blocked_reason`; kept for the uniform signature
    let fresh = read_history(client, session_id).await?;
    if has_active_turn(&fresh) {
        return Err("The Session became active. Wait before rewinding.".to_owned());
    }
    let target = resolve_checkpoint(messages_of(&fresh), selected).ok_or_else(|| {
        "History changed. Reload the checkpoint picker.".to_owned()
    })?;
    let rollback: octos_core::ui_protocol::SessionRollbackResult = client
        .call::<SessionRollback>(octos_core::ui_protocol::SessionRollbackParams {
            session_id: octos_core::SessionKey(session_id.to_owned()),
            num_turns: target.num_turns as u32,
        })
        .await
        .map_err(|e| e.to_string())?;
    // The web asserts the rolled-back thread (`assertThread(response.thread)`).
    if rollback.thread.session_id.0 != session_id {
        return Err("History belongs to another Session.".to_owned());
    }
    // Canonical rehydration of the owning record after the mutation.
    let thread = read_history(client, session_id).await?;
    Ok(HistoryOutcome {
        notice: HistoryMode::Rewind.completed_notice().to_owned(),
        thread,
        prefill: Some(target.prefill),
        forked_session_id: None,
    })
}

/// Fork conversation (`history-coordinator.ts:196-205` + 263-264): validate
/// the name, `session/fork` with the new chat id (and optional
/// `copy_messages`), then verify the response names THIS parent and a DIFFERENT
/// child. The caller opens the child in the background; this returns the exact
/// child id and never selects it.
pub async fn fork_conversation(
    client: &Client,
    session_id: &str,
    new_chat_id: &str,
    copy_messages: Option<u32>,
) -> Result<ForkOutcome, String> {
    if !valid_fork_chat_id(new_chat_id) {
        return Err("Choose a valid conversation name.".to_owned());
    }
    let fork: octos_core::ui_protocol::SessionForkResult = client
        .call::<SessionFork>(octos_core::ui_protocol::SessionForkParams {
            session_id: octos_core::SessionKey(session_id.to_owned()),
            new_chat_id: new_chat_id.trim().to_owned(),
            copy_messages,
        })
        .await
        .map_err(|e| e.to_string())?;
    let parent = fork.parent_session_id.0;
    let child = fork.new_session_id.0;
    if parent != session_id || child.is_empty() || child == session_id {
        return Err("The fork response belongs to another Session.".to_owned());
    }
    Ok(ForkOutcome {
        forked_session_id: child,
        notice: HistoryMode::Fork.completed_notice().to_owned(),
    })
}

/// What an accepted mutation leaves behind, after the canonical rehydration
/// (`HistorySnapshot` completed arm, history-coordinator.ts:265-293).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryOutcome {
    /// The per-mode completed notice, verbatim from the web.
    pub notice: String,
    /// The FRESH canonical history the picker/dialog should re-render.
    pub thread: Value,
    /// Rewind only: the text the owning scope's EMPTY draft may prefill.
    pub prefill: Option<String>,
    pub forked_session_id: Option<String>,
}

/// Fork's outcome (a fork has no rehydration of the PARENT's history).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkOutcome {
    pub forked_session_id: String,
    pub notice: String,
}

/// Load arm per mode (`history-coordinator.ts:99-131`): undo lists snapshots,
/// rewind reads canonical history into checkpoints, fork has nothing to load.
pub async fn load_history(
    client: &Client,
    store: &Store,
    session_id: &str,
    mode: HistoryMode,
) -> Result<Vec<ConversationCheckpoint>, String> {
    match mode {
        HistoryMode::Undo => {
            let list = client
                .call::<SnapshotListMethod>(octoscode_client::domains::config::SnapshotListParams {
                    session_id: session_id.to_owned(),
                })
                .await
                .map_err(|e| e.to_string())?;
            if list.session_id != session_id {
                return Err("Snapshots belong to another Session.".to_owned());
            }
            store.domains.config.set_snapshots(list.into_store());
            Ok(Vec::new())
        }
        HistoryMode::Rewind => {
            let thread = read_history(client, session_id).await?;
            Ok(conversation_checkpoints(messages_of(&thread)))
        }
        HistoryMode::Fork => Ok(Vec::new()),
    }
}

// ---------------------------------------------- production action surface

/// The action ids this screen owns. The dialog is a design-flow surface, so the
/// ids here are the state-machine entry points a mounted surface dispatches.
pub fn owns(action: &str) -> bool {
    matches!(
        action,
        "history.undo_workspace_changes" | "history.rewind_conversation" | "history.fork_conversation"
    )
}

/// The production mutation path behind those ids, dispatched on the bound
/// session. `value` is the mode-specific payload: the snapshot id, the
/// checkpoint key, or the new conversation name.
pub async fn perform(
    conv: &crate::flow::Conversation,
    action: &str,
    store: &Store,
    value: Option<&str>,
) -> Result<String, String> {
    let session_id = conv.session_id().to_owned();
    let (mode, payload) = match action {
        "history.undo_workspace_changes" => (HistoryMode::Undo, value.unwrap_or_default()),
        "history.rewind_conversation" => (HistoryMode::Rewind, value.unwrap_or_default()),
        "history.fork_conversation" => (HistoryMode::Fork, value.unwrap_or_default()),
        other => return Err(format!("history: unhandled action {other:?}")),
    };
    if let Some(why) = blocked_reason(store, &session_id, mode) {
        return Err(why);
    }
    match mode {
        HistoryMode::Undo => {
            let out = undo_workspace_changes(conv.client(), store, &session_id, payload).await?;
            Ok(out.notice)
        }
        HistoryMode::Rewind => {
            // The checkpoint key is the identity the dialog selected; rebuild the
            // checkpoint from the CURRENT picker list so `resolve_checkpoint`
            // can refuse a stale index against fresh history.
            let checkpoints = load_history(conv.client(), store, &session_id, HistoryMode::Rewind).await?;
            let selected = checkpoints
                .iter()
                .find(|c| c.key == payload)
                .cloned()
                .ok_or_else(|| "History changed. Reload the checkpoint picker.".to_owned())?;
            let out = rewind_conversation(conv.client(), store, &session_id, &selected).await?;
            Ok(out.notice)
        }
        HistoryMode::Fork => {
            let out = fork_conversation(conv.client(), &session_id, payload, None).await?;
            Ok(out.forked_session_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- checkpoints.test.ts:29 "counts user-rooted threads once and does
    // ---- not target unthreaded messages"
    #[test]
    fn groups_by_thread_id_not_by_user_message_count() {
        let messages = vec![
            json!({"role": "user", "content": "one", "seq": 1, "thread_id": "t1", "message_id": "m1"}),
            json!({"role": "assistant", "content": "reply", "seq": 2, "thread_id": "t1"}),
            json!({"role": "user", "content": "two", "seq": 3, "thread_id": "t1", "message_id": "m2"}),
            json!({"role": "user", "content": "unthreaded", "seq": 4}),
            json!({"role": "user", "content": "three", "seq": 5, "thread_id": "t2", "message_id": "m3"}),
        ];
        let cps = conversation_checkpoints(&messages);
        // 2 checkpoints (t1 counted ONCE, the unthreaded row is not a target),
        // newest first.
        assert_eq!(cps.len(), 2, "unthreaded user rows are not checkpoints");
        assert_eq!(cps[0].preview, "three");
        assert_eq!(cps[0].checkpoint, 2, "ordinal is oldest-first, list is reversed");
        assert_eq!(cps[0].user_message_count, 1);
        assert_eq!(cps[1].user_message_count, 2, "t1 holds two user messages");
        assert_eq!(cps[1].checkpoint, 1);
    }

    // ---- checkpoints.test.ts:54 "keeps duplicate prompt rows distinct and
    // ---- lists newest first"
    #[test]
    fn duplicate_prompt_rows_stay_distinct_and_newest_first() {
        let messages = vec![
            json!({"role": "user", "content": "same text", "seq": 1, "thread_id": "t1", "message_id": "m1"}),
            json!({"role": "user", "content": "same text", "seq": 2, "thread_id": "t2", "message_id": "m2"}),
            json!({"role": "user", "content": "newest", "seq": 3, "thread_id": "t3", "message_id": "m3"}),
        ];
        let cps = conversation_checkpoints(&messages);
        assert_eq!(cps.len(), 3);
        assert_eq!(cps[0].preview, "newest");
        // identical CONTENT in different threads is still two checkpoints
        assert_ne!(cps[1].key, cps[2].key, "identity is the key, not the text");
        assert_eq!(cps[1].preview, "same text");
        assert_eq!(cps[2].preview, "same text");
    }

    #[test]
    fn preview_collapses_whitespace_and_truncates_at_180() {
        let long = "a".repeat(200);
        let messages = vec![
            json!({"role": "user", "content": format!("  line one\n\tline   two  {long}"), "seq": 1, "thread_id": "t1", "message_id": "m1"}),
        ];
        let cps = conversation_checkpoints(&messages);
        // `replace(/\s+/g, " ")` collapses each run to ONE space and does NOT
        // trim: the leading two spaces become a single leading space and the
        // trailing run a single trailing space.
        assert!(cps[0].preview.starts_with(" line one line two "));
        assert_eq!(cps[0].preview.chars().count(), 180);
        // the prefill is the FULL text, never the truncated preview
        assert_eq!(cps[0].prefill.chars().count(), 200 + "  line one\n\tline   two  ".len());
    }

    #[test]
    fn media_counts_sum_across_the_group() {
        let messages = vec![
            json!({"role": "user", "content": "a", "seq": 1, "thread_id": "t1", "message_id": "m1", "media": [{"id": "1"}, {"id": "2"}]}),
            json!({"role": "user", "content": "b", "seq": 2, "thread_id": "t1", "message_id": "m2", "media": [{"id": "3"}]}),
        ];
        assert_eq!(conversation_checkpoints(&messages)[0].media_count, 3);
    }

    // ---- checkpoints.test.ts:62 "recomputes drop count after more turns
    // ---- arrive, rejects replaced history" / coordinator.test.ts:374
    #[test]
    fn identity_is_recomputed_against_fresh_history() {
        let before = vec![
            json!({"role": "user", "content": "first", "seq": 1, "thread_id": "t1", "message_id": "m1"}),
            json!({"role": "user", "content": "second", "seq": 2, "thread_id": "t2", "message_id": "m2"}),
        ];
        let selected = &conversation_checkpoints(&before)[0]; // "second" (newest)
        // a NEW turn arrived -> the drop count must grow, not reuse the index
        let after: Vec<Value> = {
            let mut v = before.clone();
            v.push(json!({"role": "user", "content": "third", "seq": 3, "thread_id": "t3", "message_id": "m3"}));
            v
        };
        let target = resolve_checkpoint(&after, selected).expect("still resolvable");
        assert_eq!(target.num_turns, 2, "recomputed: drop third + second");
        assert_eq!(target.prefill, "second");
    }

    #[test]
    fn a_stale_or_replaced_target_is_refused() {
        let messages = vec![
            json!({"role": "user", "content": "first", "seq": 1, "thread_id": "t1", "message_id": "m1"}),
            json!({"role": "user", "content": "second", "seq": 2, "thread_id": "t2", "message_id": "m2"}),
        ];
        let selected = &conversation_checkpoints(&messages)[0];
        // 1. the target is GONE from fresh history -> refused
        let truncated = vec![messages[0].clone()];
        assert!(resolve_checkpoint(&truncated, selected).is_none());
        // 2. the same key now carries REPLACED text -> refused (key+prefill must match)
        let replaced = vec![
            messages[0].clone(),
            json!({"role": "user", "content": "EDITED", "seq": 2, "thread_id": "t2", "message_id": "m2"}),
        ];
        assert!(resolve_checkpoint(&replaced, selected).is_none());
    }

    // ---- history-binding.ts:53-67, per-mode capability gating
    fn methods(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn every_mode_requires_canonical_hydrate() {
        let no_hydrate = methods(&["snapshot/list", "snapshot/restore", "session/rollback", "session/fork", "session/open"]);
        for mode in [HistoryMode::Undo, HistoryMode::Rewind, HistoryMode::Fork] {
            assert!(
                !history_supported(&no_hydrate, mode),
                "{} must need session/hydrate",
                mode.as_str()
            );
        }
    }

    #[test]
    fn per_mode_capability_gate_matches_the_web() {
        let undo_ok = methods(&["session/hydrate", "snapshot/list", "snapshot/restore"]);
        assert!(history_supported(&undo_ok, HistoryMode::Undo));
        assert!(!history_supported(&undo_ok, HistoryMode::Rewind), "undo's methods do not enable rewind");
        assert!(!history_supported(&undo_ok, HistoryMode::Fork));

        let rewind_ok = methods(&["session/hydrate", "session/rollback"]);
        assert!(history_supported(&rewind_ok, HistoryMode::Rewind));
        assert!(!history_supported(&rewind_ok, HistoryMode::Undo));

        let fork_ok = methods(&["session/hydrate", "session/fork", "session/open"]);
        assert!(history_supported(&fork_ok, HistoryMode::Fork));
        // fork needs BOTH session/fork and session/open
        assert!(!history_supported(&methods(&["session/hydrate", "session/fork"]), HistoryMode::Fork));
        // undo needs BOTH snapshot methods
        assert!(!history_supported(&methods(&["session/hydrate", "snapshot/list"]), HistoryMode::Undo));
    }

    // ---- history-coordinator.ts:178 the active-turn refusal
    #[test]
    fn an_active_turn_blocks_the_rewind() {
        assert!(has_active_turn(&json!({"turns": [{"state": "done"}, {"state": "active"}]})));
        assert!(!has_active_turn(&json!({"turns": [{"state": "done"}]})));
        assert!(!has_active_turn(&json!({})), "no turns -> not active");
    }

    // ---- packages/client/src/history.ts:249 the fork-name validation
    #[test]
    fn fork_chat_id_validation_rejects_unusable_names() {
        assert!(valid_fork_chat_id("r3child"));
        assert!(valid_fork_chat_id("  spaced  "));
        assert!(!valid_fork_chat_id(""));
        assert!(!valid_fork_chat_id("   "));
        assert!(!valid_fork_chat_id("a/b"), "a path is not a conversation name");
        assert!(!valid_fork_chat_id("a\nb"), "a control character is not a name");
    }
}
