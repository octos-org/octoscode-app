//! `task` state: background tasks and their output.
//!
//! Stub for the fan-out lane (`task/list`, `task/output/read`, `task/artifact/*`).
use std::collections::HashMap;
use std::sync::Mutex;

/// One background task, as the list/update rows describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<String>,
}

/// The task domain.
#[derive(Debug, Default)]
pub struct Tasks {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    tasks: HashMap<String, Task>,
}

impl Tasks {
    pub fn upsert(&self, task: Task) {
        self.inner.lock().unwrap().tasks.insert(task.id.clone(), task);
    }

    pub fn list(&self) -> Vec<Task> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<Task> = i.tasks.values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().tasks.len()
    }
}
