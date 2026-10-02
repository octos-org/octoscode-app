//! The store's state, one file per protocol domain — the same split the client
//! uses, so a fan-out lane owns **one file** here too.
//!
//! [`State::new`] is the ONE place every domain is constructed. A lane adds
//! its domain with one `pub mod` line above and one field in [`State`].
pub mod approval;
pub mod autonomy;
// A7: the composer's prompt queue + turn controller state.
pub mod composer;
pub mod config;
pub mod media;
pub mod models;
pub mod peer;
pub mod profile;
pub mod review;
pub mod session;
// A31: background skill-action jobs (parity row 15).
pub mod skill_jobs;
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
    /// A10 — the per-Session model-selection notice boards.
    pub models: models::ModelNotices,
    pub config: config::Config,
    pub composer: composer::Composer,
    /// A31 — background skill-action jobs per (Profile, Session).
    pub skill_jobs: skill_jobs::SkillJobs,
}

impl State {
    /// Construct every domain. The single construction site (card #10 step 1).
    pub fn new() -> Self {
        Self::default()
    }
}
