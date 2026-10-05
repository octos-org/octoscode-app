//! The protocol, one file per domain.
//!
//! [`register_all`] is the ONLY shared line per domain: it calls each
//! domain's `register()` once, so a fan-out lane that owns one domain edits
//! one file and (at most) one line here.
//!
//! ## Domains and the matrix rows they own
//! | file | methods (from `docs/protocol-matrix.csv` + `-ext`) |
//! | --- | --- |
//! | `session.rs` | session/list, session/delete, session/fork, session/hydrate, session/snapshot, session/title.set, session/messages_page, session/tasks.list, session/files.list, session/workspace.get, session/compact, session/rollback, session/btw, session/status/read, session/event, session/orchestration, session/goal/* (8), session/driver/* (4), session/wake/* (2) |
//! | `turn.rs` | turn/start, turn/steer, turn/interrupt, turn/state/get; notifications turn/started, turn/completed, turn/error, turn/steer_dropped, message/delta, message/reasoning_delta |
//! | `tool.rs` | tool/status/list (ext); notifications tool/started, tool/progress, tool/completed |
//! | `approval.rs` | approval/respond, approval/scopes/list; notifications approval/requested, approval/decided, approval/cancelled, approval/auto_resolved |
//! | `review.rs` | review/start |
//! | `task.rs` | task/list, task/cancel, task/restart_from_node, task/output/read, task/artifact/list, task/artifact/read; notifications task/updated, task/output/delta |
//! | `autonomy.rs` | peer/agent autonomy: agent/list, agent/interrupt, agent/close, agent/status/read, agent/output/read, agent/artifact/list, agent/artifact/read; notifications agent/updated, agent/output/delta, agent/artifact/updated, agent/status/read |
//! | `peer.rs` | peer/prepare, peer/gather, peer/dispatch, peer/control; notifications peer/staged, peer/closed |
//! | `profile.rs` | profile/local/create, profile/llm/* (7), profile/skills/* (4), profile/sub_providers/* (3); onboarding/* (2) |
//! | `media.rs` | visual/* (3), voice/* (2), content/* (3), file/attached, smart_home/* (5) |
//! | `config.rs` | config/capabilities/list; plus the misc singletons (cron/*, diff/preview/get, launch/resolve, memory/*, permission/profile/*, plan/updated, progress/updated, projection/envelope, protocol/replay_lossy, queue/state, router/*, server/shutdown, snapshot/*, system/status.get, thread/graph/get, user_question/*, warning, background/activity, mcp/status/list) |
//! | `skill_jobs.rs` | A31: skill/action/job/list; notification skill/action/job/updated (the Skills dialog's Background jobs, parity row 15) |
pub mod approval;
pub mod autonomy;
pub mod config;
// A10: the typed external-driver leaf (session/driver/*, peer/dispatch, peer/control).
pub mod external_driver;
pub mod media;
pub mod peer;
pub mod profile;
pub mod review;
pub mod session;
// A31: background skill-action jobs (parity row 15).
pub mod skill_jobs;
pub mod task;
pub mod tool;
pub mod turn;

use crate::registry::Registry;
use std::sync::Arc;
use octoscode_store::Store;

/// Register every domain's notifications on `registry`, once, against the
/// shared `store`. The fan-out lane for a domain adds its handler line here.
pub fn register_all(registry: &mut Registry, store: Arc<Store>) {
    session::register(registry, store.clone());
    turn::register(registry, store.clone());
    tool::register(registry, store.clone());
    approval::register(registry, store.clone());
    review::register(registry, store.clone());
    task::register(registry, store.clone());
    autonomy::register(registry, store.clone());
    peer::register(registry, store.clone());
    profile::register(registry, store.clone());
    media::register(registry, store.clone());
    config::register(registry, store.clone());
    // A31: `skill/action/job/updated` (parity row 15), no longer ignored.
    skill_jobs::register(registry, store.clone());
    // Card #22 §2: the no-silent-drops guard records unhandled kinds here.
    registry.set_store(store);
    register_ignored(registry);
}

/// Card #22 §2: the core notification kinds octos can push unprompted that the
/// web **ignores**, each with the reason it is deliberately not acted on. An
/// entry here is a decision (the guard does not count it) and is the source of
/// `docs/cards/22-ignored.csv`. Every method here has NO handler and NO web
/// consumer outside `packages/client/src/generated/core-contract.ts`.
///
/// - `router/status`, `router/failover` — the adaptive router's lane
///   snapshots (`RouterStatusEvent` `ui_protocol.rs:6409`, `RouterFailoverEvent`
///   `:6432`). The web renders no routing pill and subscribes to neither; we
///   surface routing only in logs.
/// - `queue/state` — `QueueStateEvent` (`ui_protocol.rs:6452`); its own doc says
///   "Client-manufactured today — server never emits this." Nothing to consume.
/// - `background/activity` — `BackgroundActivityEvent` (`ui_protocol.rs:6551`);
///   gated by `event.background_activity.v1`, which our feature set does not
///   negotiate. We never receive it; if we did, it is a human-facing wake
///   notice with no store projection yet.
/// - `turn/spawn_complete` — the **legacy** pre-v2 child-completion notification
///   (`TurnSpawnCompleteEvent` `ui_protocol.rs:3794`). Superseded by the v2
///   `background/spawn_complete` `PayloadV2`, which rides `projection/envelope`
///   and IS handled (`turn.rs`, `PayloadV2::BackgroundChildCompleted`). The
///   legacy bare frame has no web consumer.
/// - `agent/output/delta`, `agent/artifact/updated` — the M15 agent tail
///   (`AgentOutputDeltaEvent` `ui_protocol.rs:5905`, `AgentArtifactUpdatedEvent`
///   `:5913`). The web ignores both. We *do* handle `agent/updated` (the agent
///   lifecycle snapshot, `autonomy.rs`); these two are high-frequency tails with
///   no consumer — the artifact metadata that matters arrives via
///   `agent/artifact/list`, which autonomy owns.
///
/// A31: `skill/action/job/updated` left this list — the Skills dialog's
/// "Background jobs" section consumes it (`skill_jobs.rs`, parity row 15).
pub fn register_ignored(registry: &mut Registry) {
    registry.ignore("router/status", "web ignores; routing shown in logs only");
    registry.ignore("router/failover", "web ignores; routing shown in logs only");
    registry.ignore("queue/state", "client-manufactured; server never emits it");
    registry.ignore(
        "background/activity",
        "feature not negotiated; no store projection yet",
    );
    registry.ignore(
        "turn/spawn_complete",
        "legacy pre-v2; superseded by projection/envelope background/spawn_complete (handled)",
    );
    registry.ignore("agent/output/delta", "web ignores; agent tail has no consumer");
    registry.ignore(
        "agent/artifact/updated",
        "web ignores; artifact metadata read via agent/artifact/list",
    );
    // The host's own frames: a host tool's call and cancel, and input for an
    // app peer. octos sends them on the host's connection only (in OctoSense
    // the shell's relay answers them; its router never gives them to an app).
    registry.ignore("peer/tool/call", "the host's own: a host tool's call (never a client's)");
    registry.ignore("peer/tool/cancel", "the host's own: a host tool's cancel (never a client's)");
    registry.ignore("peer/input", "the host's own: input for an app peer (never a client's)");
}
