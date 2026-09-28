//! `review/start` — start the server-owned native code review. No
//! notifications.
//!
//! Web caller: `src-web/packages/client/src/history.ts` (`startReview`, the
//! review branch of the history surface). The web sends
//! `{session_id, turn_id, delivery: "inline", prompt?}` and reads
//! `{accepted, session_id, turn_id, workflow: "code_review", backend:
//! "native", agent_count}`. octos-core declares no type for this method, so
//! the shapes below mirror the server's result `json!`
//! (`crates/octos-cli/src/api/ui_protocol_transport.rs`, the `REVIEW_START`
//! handler) and the web's parser (`history.ts` `parseReviewStartResult`).
//!
//! Gated by the `review.start.v1` feature on the server side; the caller
//! checks capability before calling (the web throws
//! "Native review is not advertised by this server" when it is absent).
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use octos_core::ui_protocol::methods;
use octoscode_store::Store;

use crate::method::Method;
use crate::registry::Registry;

/// `review/start` — start the native code review for one confirmed turn.
pub struct ReviewStart;

/// Params as the web sends them (`history.ts`): `delivery` is always
/// `"inline"` from the web; `prompt` is omitted when absent.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewStartParams {
    pub session_id: String,
    pub turn_id: String,
    /// Always `"inline"` from the web.
    pub delivery: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

/// The server's accepted `review/start` receipt.
#[derive(Debug, Clone, Deserialize)]
pub struct ReviewStartResult {
    pub accepted: bool,
    pub session_id: String,
    pub turn_id: String,
    /// `"code_review"` on the wire.
    pub workflow: String,
    /// `"native"` on the wire.
    pub backend: String,
    pub agent_count: u32,
}

impl Method for ReviewStart {
    const NAME: &'static str = methods::REVIEW_START;
    type Params = ReviewStartParams;
    type Result = ReviewStartResult;
}

/// This domain has no notifications.
pub fn register(_reg: &mut Registry, _store: Arc<Store>) {}
