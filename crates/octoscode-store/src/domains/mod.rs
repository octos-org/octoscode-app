//! The store's state, one file per protocol domain — the same split the client
//! uses, so a fan-out lane owns **one file** here too.
//!
//! [`State::new`] is the ONE place every domain is constructed. A lane adds
//! its domain with one `pub mod` line above and one field in [`State`].
pub mod approval;
pub mod autonomy;
pub mod config;
pub mod media;
pub mod peer;
pub mod profile;
pub mod review;
pub mod session;
pub mod task;
pub mod tool;
pub mod turn;

/// Every domain's state, held together by [`crate::Store`].
///
/// Constructed once, in [`State::new`]. Each field is independently locked, so
/// a `message/delta` fold (session timeline) never blocks a tool inventory
/// write.
#[derive(Debug, Default)]
pub struct State {
    pub session: session::Sessions,
    pub turn: turn::Turns,
    pub tool: tool::Tools,
    pub approval: approval::Approvals,
    pub review: review::Reviews,
    pub task: task::Tasks,
    pub autonomy: autonomy::Autonomy,
    pub peer: peer::Peers,
    pub profile: profile::Profiles,
    pub media: media::Media,
    pub config: config::Config,
}

impl State {
    /// Construct every domain. The single construction site (card #10 step 1).
    pub fn new() -> Self {
        Self::default()
    }
}
