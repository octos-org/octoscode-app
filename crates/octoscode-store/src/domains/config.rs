//! `config` state: the server's advertised capabilities.
//!
//! Implemented in this card: the capability set from `config/capabilities/list`
//! (and the handshake). The other config singletons (router, cron, memory,
//! snapshot, …) are the fan-out lane's; they add fields here.
use std::sync::Mutex;

/// The config domain.
#[derive(Debug, Default)]
pub struct Config {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    capabilities: Vec<String>,
}

impl Config {
    /// Replace the advertised capability list.
    pub fn set_capabilities(&self, capabilities: Vec<String>) {
        self.inner.lock().unwrap().capabilities = capabilities;
    }

    pub fn capabilities(&self) -> Vec<String> {
        self.inner.lock().unwrap().capabilities.clone()
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().capabilities.len()
    }

    /// Whether a capability id is advertised (case-insensitive, trimmed).
    pub fn has_capability(&self, id: &str) -> bool {
        let want = id.trim().to_ascii_lowercase();
        self.inner
            .lock()
            .unwrap()
            .capabilities
            .iter()
            .any(|c| c.trim().to_ascii_lowercase() == want)
    }
}
