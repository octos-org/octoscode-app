//! `visual/*`, `voice/*`, `content/*`, `file/attached`, `smart_home/*` — media.
//!
//! Requests: `content/list`, `content/delete`, `content/bulk_delete`,
//! `smart_home/device.list`, `smart_home/device.command`,
//! `smart_home/camera.stream_start`, `smart_home/camera.stream_stop`,
//! `smart_home/status.get`. Notifications: `visual/generating`,
//! `visual/succeeded`, `visual/failed`, `voice/audio_chunk`, `voice/exit`,
//! `file/attached`.
//!
//! Stub: the fan-out lane for media owns this file.
use std::sync::Arc;

use octoscode_store::Store;

use crate::registry::Registry;

pub fn register(_reg: &mut Registry, _store: Arc<Store>) {}
