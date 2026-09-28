//! `profile/*` + `onboarding/*` — profiles, LLM providers, skills.
//!
//! Requests: `profile/local/create`, `profile/llm/catalog`, `profile/llm/list`,
//! `profile/llm/select`, `profile/llm/upsert`, `profile/llm/delete`,
//! `profile/llm/test`, `profile/llm/fetch_models`, `profile/skills/list`,
//! `profile/skills/registry/search`, `profile/skills/install`,
//! `profile/skills/remove`, `profile/sub_providers/list|upsert|remove`,
//! `onboarding/workspace_list`, `onboarding/workspace_create`.
//!
//! Stub: the fan-out lane for profiles owns this file.
use std::sync::Arc;

use octoscode_store::Store;

use crate::registry::Registry;

pub fn register(_reg: &mut Registry, _store: Arc<Store>) {}
