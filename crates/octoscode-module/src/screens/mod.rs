//! Stage C screens: board-2 setup screens wired to the store and the production
//! client (bindings + actions). One submodule per card; each owns its ids.
//!
//! #29d added `palette` (board 2.8/2.11/2.12) on task/29d while main carried
//! `connect`/`models`/`workspace` (#29a/#29c) — the #29d2 merge keeps all four.
//! #30c adds `fleet` (board 3.6/3.7) on task/30c.
pub mod connect;
pub mod fleet;
pub mod models;
pub mod palette;
pub mod sessions;
pub mod theme;
pub mod workspace;
