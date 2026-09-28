//! Feature negotiation: the exact UI-feature list the web client sends.
//!
//! Every live lane hit this: the server only *advertises* (and honours)
//! `coding.*`, `harness.*`, `thread_graph`, `turn_state_get`, … when the
//! client asks for them at connect time. Our transport's default
//! (`octos_app_transport::Capabilities::requested()`) asks for only 6, so a
//! client built on it sees **73** methods where the web sees **95**.
//!
//! ## What the web does (cited)
//!
//! `src-web/packages/client/src/client.ts:106-128` defines
//! `DEFAULT_UI_FEATURES` — 21 entries, `client.ts:323` passes them to
//! `buildUiProtocolUrl`. `src-web/packages/client/src/url.ts:24-28` transmits
//! them as **repeated `ui_feature=` query parameters**:
//! ```js
//! url.searchParams.delete("ui_feature");
//! for (const feature of options.features ?? []) {
//!   const normalized = feature.trim();
//!   if (normalized) url.searchParams.append("ui_feature", normalized);
//! }
//! ```
//! The values are the `CORE_UI_FEATURES` constants
//! (`src-web/packages/client/src/generated/core-contract.ts:1-29`) plus
//! `EXTERNAL_DRIVER_V1_UI_FEATURE = "external_driver_v1"` (`client.ts:103`).
//!
//! ## What we do
//!
//! [`WEB_UI_FEATURES`] is that list, in the web's order. [`apply_to_url`]
//! sets the same repeated `ui_feature=` query parameters on a base URL — our
//! transport clones `base_url` verbatim (`ws/mod.rs::build_ws_uri`), so the
//! params survive to the socket, byte-for-byte like the web.
//! [`web_capabilities`] also returns the matching
//! `octos_app_transport::Capabilities`, so the transport's
//! `x-octos-ui-features` handshake header carries the same set.
//!
//! Both paths are additive (the server accepts either); the query path is the
//! web-identical one and is what the module uses.
use octos_app_transport::Capabilities;

/// The 21 features the web client requests, **in the web's order**
/// (`client.ts:106-128`).
pub const WEB_UI_FEATURES: &[&str] = &[
    "approval.typed.v1",
    "pane.snapshots.v1",
    "session.workspace_cwd.v1",
    "auxiliary.rest_to_ws.v1",
    "state.session_hydrate.v1",
    "state.thread_graph.v1",
    "state.turn_state_get.v1",
    "event.turn_steer_dropped.v1",
    "user_question.v1",
    "plan.todos.v1",
    "projection.envelope.v2",
    "harness.task_control.v1",
    "harness.task_artifacts.v1",
    "context.lifecycle.v1",
    "review.start.v1",
    "coding.autonomy.v1",
    "coding.agent_control.v1",
    "coding.goal_runtime.v1",
    "coding.loop_runtime.v1",
    "coding.monitor_runtime.v1",
    "external_driver_v1",
];

/// The subset the transport has a typed boolean for
/// (`octos-app-transport`'s `Capabilities` fields). The rest go through
/// `Capabilities::raw`.
const TYPED: &[&str] = &[
    "approval.typed.v1",
    "pane.snapshots.v1",
    "session.workspace_cwd.v1",
    "auxiliary.rest_to_ws.v1",
    "state.session_hydrate.v1",
    "context.lifecycle.v1",
];

/// A `Capabilities` request that asks for exactly [`WEB_UI_FEATURES`].
///
/// The transport emits `x-octos-ui-features: <list>` from this
/// (`ws/mod.rs:116`, `capability/mod.rs::requested_features`). Note the
/// header's order is the transport's own (typed flags first, then the `raw`
/// map, which is a `BTreeMap` → sorted); the **set** is what matters to the
/// server, which intersects it with its known features.
pub fn web_capabilities() -> Capabilities {
    let mut caps = Capabilities::default();
    // Leave the typed booleans off: everything is requested through `raw`, so
    // there is one source of truth (`WEB_UI_FEATURES`) rather than two.
    for f in WEB_UI_FEATURES {
        caps.raw.insert((*f).to_owned(), serde_json::Value::Bool(true));
    }
    caps
}

/// Which of [`WEB_UI_FEATURES`] have a typed boolean on `Capabilities`.
/// (Kept public so a test can assert the split is complete and honest.)
pub fn typed_features() -> &'static [&'static str] {
    TYPED
}

/// Set the web's repeated `ui_feature=` query parameters on `base`,
/// **replacing** any already present (mirrors `url.ts:24-28`). Our transport
/// preserves the query string when it derives the WS URL, so this is the
/// web-identical connect path.
///
/// Lives here (the module applies it) so this crate needs no `url`
/// dependency; see `octoscode_module::features_apply`.
pub fn feature_query_pairs() -> impl Iterator<Item = (&'static str, &'static str)> {
    WEB_UI_FEATURES.iter().map(|f| ("ui_feature", *f))
}
