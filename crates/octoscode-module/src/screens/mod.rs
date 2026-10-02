//! Stage C screens: board-2 setup screens wired to the store and the production
//! client (bindings + actions). One submodule per card; each owns its ids.
//!
//! #29d added `palette` (board 2.8/2.11/2.12) on task/29d while main carried
//! `connect`/`models`/`workspace` (#29a/#29c) — the #29d2 merge keeps all four.
//! #30c adds `fleet` (board 3.6/3.7) on task/30c.
// A9: Activity — the operator-opened cross-session task scan.
pub mod activity;
// A9: Settings connection additions (status, info rows, Disconnect/Forget).
pub mod a9_settings;
// A9: the fatal + per-surface error boundaries (crash screen, unavailable panel).
pub mod a9_boundary;
// A9: the Connect failure probe (unreachable / refused token / origin, honestly).
pub mod a9_connect;
// A9: display preferences (the web's display.v1 whitelist; Vim editing).
pub mod a9_prefs;
pub mod autonomy;
// #A2: board 1 as live, reachable native surfaces (the host + its view kit).
pub mod board1;
pub mod board1_kit;
pub mod board3;
pub mod browser;
pub mod connect;
// A8: the header's "Copy as Markdown" (CopyConversationButton phases).
pub mod copy_button;
// A5: the dialog host — the Stage-B screens reachable from the palette and
// the sidebar, lowered slot-relative, wired by node id.
pub mod dialog;
// A14: the dialog host's family drawn with the board-3 kit (frame, header,
// type ramp, pills, per-dialog widths), the ids its walks address kept.
pub mod dialog_view;
// A8: per-Session composer drafts, durable per authenticated principal.
pub mod drafts;
// A8: the external-driver disclosure walk (session/driver/get operations chain).
pub mod driver_discovery;
pub mod fleet;
// A10: the external-driver chains behind the Fleet (seat, Start, row control).
pub mod fleet_driver;
pub mod history;
pub mod keys;
// A8: the workspace launch (launch/resolve, the decision panel, the lease).
pub mod launch;
pub mod media;
pub mod models;
// #D1: the five native-pairing cards (p4-01..p4-05) — one owner per action id.
pub mod pairing;
// #D1: the two provider-editor cards (p4-06/07) — draft kept, error redacted.
pub mod provider;
pub mod palette;
pub mod peers;
// P4h1 rows 304-307: the recent-workspaces cache (the web's
// `features/workspace/workspace-recents.ts`), with its own storage seam.
pub mod recents;
// A12: the connection recovery banner (a live connection that dropped keeps
// the conversation and says "Reconnecting to Octos").
pub mod reconnect;
pub mod research;
// #D2a: board 2's sidebar half (screens 1-5) — the one owner of the 13 control
// events the phase4-new2 cards declare (grouped tree, statuses, search,
// collapsed rail, compact drawer).
pub mod sidebar;
// #D2b / A3: board 2's settings half (screens 6-12) — one owner for the
// settings cards' ids AND the native Settings chrome's.
pub mod settings;
// A8: the new-session defaults (persisted per endpoint, applied at creation only).
pub mod session_defaults;
// A8: the full-Session identity grammar (Core split_base_key + channels).
pub mod session_identity;
// #35b item 1: the ONE card-tap wiring every docked screen shares (connect.rs
// delegates here; the palette/theme mount paths call it directly).
pub mod taps;
pub mod transcript;
pub mod workspace;
pub mod review;
pub mod sessions;
pub mod theme;
// A6: the conversation pane's surfaces (approval/question takeovers, the
// plan card, the Trajectory + task detail, fold-all and view state).
pub mod surfaces;

/// P4d4: the ONE production entry point for the control surfaces (media +
/// peers), so `lib.rs`'s action router has a single target to call.
pub async fn perform_control(
    conv: &crate::flow::Conversation,
    action: &str,
    store: &octoscode_store::Store,
) -> Result<String, String> {
    if media::owns(action) {
        return media::perform(conv, action, store, None).await;
    }
    if peers::owns(action) {
        return peers::perform(conv, action, store, None).await;
    }
    Err(format!("control: unhandled action {action:?}"))
}

/// A5 — whether one of the screen modules whose `perform_action` arms run
/// AFTER the conversation router's `Unhandled` check owns `action`. Such an id
/// must fall through to its owner: returning at `Unhandled` made the research,
/// board-3, history, media, peers, transcript, models/skills/context, provider
/// and browser tables unreachable from any click (measured: the Models
/// dialog's `models.test_route` never reached the wire).
pub fn owned_after_router(action: &str) -> bool {
    research::owns(action)
        || board3::owns(action)
        || history::owns(action)
        || media::owns(action)
        || peers::owns(action)
        || transcript::owns(action)
        || models::owns(action)
        || provider::is_action(action)
        || browser::is_action(action)
}
