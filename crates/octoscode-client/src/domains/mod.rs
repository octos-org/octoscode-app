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
//! | `config.rs` | config/capabilities/list; plus the misc singletons (cron/*, diff/preview/get, launch/resolve, memory/*, permission/profile/*, plan/updated, progress/updated, projection/envelope, protocol/replay_lossy, queue/state, router/*, server/shutdown, skill/action/job/updated, snapshot/*, system/status.get, thread/graph/get, user_question/*, warning, background/activity, mcp/status/list) |
pub mod approval;
pub mod autonomy;
pub mod config;
pub mod media;
pub mod peer;
pub mod profile;
pub mod review;
pub mod session;
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
    config::register(registry, store);
}
