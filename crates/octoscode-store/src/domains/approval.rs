//! `approval` state: the pending approval the person must decide.
//!
//! Stub for the fan-out lane. The shape: an id, the tool/target it is about,
//! and whether a decision has been recorded. Its lane fills this in.
use std::sync::Mutex;

/// One pending approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    pub id: String,
    pub target: Option<String>,
    pub decided: bool,
}

/// The approval domain.
#[derive(Debug, Default)]
pub struct Approvals {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    pending: Vec<PendingApproval>,
}

impl Approvals {
    pub fn push(&self, approval: PendingApproval) {
        self.inner.lock().unwrap().pending.push(approval);
    }

    pub fn pending(&self) -> Vec<PendingApproval> {
        self.inner.lock().unwrap().pending.clone()
    }

    /// Mark one decided by id; returns whether it was found.
    pub fn decide(&self, id: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        match i.pending.iter_mut().find(|a| a.id == id) {
            Some(a) => {
                a.decided = true;
                true
            }
            None => false,
        }
    }
}
