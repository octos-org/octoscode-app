//! Stage C screens: board-2 setup screens wired to the store and the production
//! client (bindings + actions). One submodule per card; each owns its ids.
//!
//! #29d added `palette` (board 2.8/2.11/2.12) on task/29d while main carried
//! `connect`/`models`/`workspace` (#29a/#29c) — the #29d2 merge keeps all four.
//! #30c adds `fleet` (board 3.6/3.7) on task/30c.
pub mod autonomy;
pub mod board3;
pub mod browser;
pub mod connect;
// A5: the dialog host — the Stage-B screens reachable from the palette and
// the sidebar, lowered slot-relative, wired by node id.
pub mod dialog;
pub mod fleet;
pub mod history;
pub mod keys;
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
pub mod research;
// #D2a: board 2's sidebar half (screens 1-5) — the one owner of the 13 control
// events the phase4-new2 cards declare (grouped tree, statuses, search,
// collapsed rail, compact drawer).
pub mod sidebar;
// #35b item 1: the ONE card-tap wiring every docked screen shares (connect.rs
// delegates here; the palette/theme mount paths call it directly).
pub mod taps;
pub mod transcript;
pub mod workspace;
pub mod review;
pub mod sessions;
pub mod theme;

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
