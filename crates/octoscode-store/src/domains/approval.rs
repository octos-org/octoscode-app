//! `approval` state: the pending approval the person must decide, and the
//! Session's standing approval scopes from `approval/scopes/list`.
use std::sync::Mutex;

/// One pending approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    pub id: String,
    pub target: Option<String>,
    pub decided: bool,
}

/// One standing approval scope, flattened from `ApprovalScopeEntry`
/// (`approval/scopes/list`): which scope, how it matches, the decision, and
/// the bound turn for a `turn`-scoped entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredScope {
    pub scope: String,
    pub scope_match: String,
    pub decision: String,
    pub turn_id: Option<String>,
}

/// The approval domain.
#[derive(Debug, Default)]
pub struct Approvals {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    pending: Vec<PendingApproval>,
    /// The last `approval/scopes/list` result.
    scopes: Vec<StoredScope>,
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

    /// Replace the standing scope list with a fresh `approval/scopes/list`.
    pub fn set_scopes(&self, scopes: Vec<StoredScope>) {
        self.inner.lock().unwrap().scopes = scopes;
    }

    /// The last scope list read (empty until one lands).
    pub fn scopes(&self) -> Vec<StoredScope> {
        self.inner.lock().unwrap().scopes.clone()
    }
}
