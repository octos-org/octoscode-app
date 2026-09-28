//! `autonomy` state: sub-agents, loops, monitors, goals (M15).
//!
//! Stub for the fan-out lane. All of `agent/*`, `loop/*`, `monitor/*`,
//! `session/goal/*` and `background/activity` land here.
use std::collections::HashMap;
use std::sync::Mutex;

/// One autonomy entity (a sub-agent, a loop, a monitor, a goal), keyed by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutonomyEntity {
    pub id: String,
    pub kind: String,
    pub detail: Option<String>,
}

/// The autonomy domain.
#[derive(Debug, Default)]
pub struct Autonomy {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    entities: HashMap<(String, String), AutonomyEntity>,
}

impl Autonomy {
    pub fn upsert(&self, entity: AutonomyEntity) {
        self.inner
            .lock()
            .unwrap()
            .entities
            .insert((entity.kind.clone(), entity.id.clone()), entity);
    }

    pub fn of_kind(&self, kind: &str) -> Vec<AutonomyEntity> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<AutonomyEntity> = i
            .entities
            .values()
            .filter(|e| e.kind == kind)
            .cloned()
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().entities.len()
    }
}
