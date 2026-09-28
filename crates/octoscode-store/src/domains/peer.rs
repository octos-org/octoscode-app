//! `peer` state: staged/closed peer sessions.
//!
//! Stub for the fan-out lane.
use std::collections::HashMap;
use std::sync::Mutex;

/// One peer, as `peer/staged` / `peer/closed` describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub name: String,
    pub closed: bool,
}

/// The peer domain.
#[derive(Debug, Default)]
pub struct Peers {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    peers: HashMap<String, Peer>,
}

impl Peers {
    pub fn stage(&self, name: String) {
        self.inner
            .lock()
            .unwrap()
            .peers
            .insert(name.clone(), Peer { name, closed: false });
    }

    pub fn close(&self, name: &str) {
        let mut i = self.inner.lock().unwrap();
        if let Some(p) = i.peers.get_mut(name) {
            p.closed = true;
        }
    }

    pub fn list(&self) -> Vec<Peer> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<Peer> = i.peers.values().cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    pub fn open_count(&self) -> usize {
        self.inner.lock().unwrap().peers.values().filter(|p| !p.closed).count()
    }
}
