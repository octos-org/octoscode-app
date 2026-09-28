//! `review/start` — the code-review start request. (fan-out lane)
use std::sync::Arc;

use octoscode_store::Store;

use crate::registry::Registry;

/// Request: `review/start`. No notifications.
pub fn register(_reg: &mut Registry, _store: Arc<Store>) {}
