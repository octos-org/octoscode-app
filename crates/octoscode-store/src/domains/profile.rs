//! `profile` state: profiles, LLM providers, skills.
//!
//! Stub for the fan-out lane (`profile/llm/*`, `profile/skills/*`,
//! `profile/sub_providers/*`, `onboarding/*`).
use std::sync::Mutex;

/// The profile domain.
#[derive(Debug, Default)]
pub struct Profiles {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// The profile the connection is using (from `session/open`).
    current: Option<String>,
    /// LLM provider ids the server listed.
    providers: Vec<String>,
}

impl Profiles {
    pub fn set_current(&self, profile_id: String) {
        self.inner.lock().unwrap().current = Some(profile_id);
    }

    pub fn current(&self) -> Option<String> {
        self.inner.lock().unwrap().current.clone()
    }

    pub fn set_providers(&self, providers: Vec<String>) {
        self.inner.lock().unwrap().providers = providers;
    }

    pub fn providers(&self) -> Vec<String> {
        self.inner.lock().unwrap().providers.clone()
    }
}
