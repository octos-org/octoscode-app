//! `tool` state: the runtime tool inventory from `tool/status/list`.
//!
//! A lane that wants tool activity counters adds them here.
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// One tool row (`RuntimeTool` in the web client's `packages/client/src/inventory.ts:15`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeTool {
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub policy: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// The tool domain: the last inventory seen.
#[derive(Debug, Default)]
pub struct Tools {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    tools: Vec<RuntimeTool>,
}

impl Tools {
    pub fn set(&self, tools: Vec<RuntimeTool>) {
        self.inner.lock().unwrap().tools = tools;
    }

    pub fn list(&self) -> Vec<RuntimeTool> {
        self.inner.lock().unwrap().tools.clone()
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().tools.len()
    }
}
