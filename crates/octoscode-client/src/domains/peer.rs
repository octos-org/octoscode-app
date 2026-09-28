//! `peer/*` — sovereign peer sessions.
//!
//! Requests: `peer/prepare`, `peer/gather`, `peer/dispatch`, `peer/control`.
//! Notifications: `peer/staged`, `peer/closed`.
//!
//! Stub: the fan-out lane for peers owns this file.
use std::sync::Arc;

use octoscode_store::Store;

use crate::registry::Registry;

pub fn register(_reg: &mut Registry, _store: Arc<Store>) {}
