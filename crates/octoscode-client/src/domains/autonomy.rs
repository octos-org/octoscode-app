//! `agent/*` (autonomy) — the sub-agent list and their lifecycle.
//!
//! Requests: `agent/list`, `agent/interrupt`, `agent/close`,
//! `agent/status/read`, `agent/output/read`, `agent/artifact/list`,
//! `agent/artifact/read`. Notifications: `agent/updated`, `agent/output/delta`,
//! `agent/artifact/updated`. Also `loop/*`, `monitor/*`, `session/goal/*`,
//! `background/activity` (the M15 autonomy surface).
//!
//! Stub: the fan-out lane for autonomy owns this file. Its notifications land
//! on the tolerated-unknown arm until then (logged by name, never fatal).
use std::sync::Arc;

use octoscode_store::Store;

use crate::registry::Registry;

pub fn register(_reg: &mut Registry, _store: Arc<Store>) {}
