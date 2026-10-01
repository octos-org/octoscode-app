//! Stage C screens: board-2 setup screens wired to the store and the production
//! client (bindings + actions). One submodule per card; each owns its ids.
//!
//! #29d added `palette` (board 2.8/2.11/2.12) on task/29d while main carried
//! `connect`/`models`/`workspace` (#29a/#29c) — the #29d2 merge keeps all four.
//! #30c adds `fleet` (board 3.6/3.7) on task/30c.
pub mod autonomy;
pub mod connect;
pub mod fleet;
pub mod keys;
pub mod models;
pub mod palette;
// #35b item 1: the ONE card-tap wiring every docked screen shares (connect.rs
// delegates here; the palette/theme mount paths call it directly).
pub mod taps;
pub mod transcript;
pub mod workspace;
pub mod review;
pub mod sessions;
pub mod theme;
