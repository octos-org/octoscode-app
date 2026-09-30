//! Entry #29c — Stage C wiring for board 2.7 Model settings (`setup-07`),
//! 2.9 Context panel (`setup-09`) and 2.10 Skills (`setup-10`).
//!
//! ## Shape (the module owns the meaning, 8.8 condition 2)
//!
//! The three Stage-B cards are DATA: each names `copy` slots (the text runs
//! the Gate-B render showed) and `service-actions.json` control events
//! (`models.test_route`, `models.discover`, `context.compact_now`,
//! `skills.remove_N`, `skills.install_N`). This module owns both directions:
//!
//! - **bindings** — [`query_binding`] turns store state into the JSON a copy
//!   slot shows ([`COPY_SLOTS`] is the declared slot→id table);
//! - **actions** — [`action_params`] is the PURE `(action, store) →
//!   (protocol method, params)` table, and [`perform`] executes it through
//!   the production client (`Conversation::client()` → `Client::request`,
//!   `flow.rs:542` / `client lib.rs:108` — the same path the web's
//!   `client.ts:488` generic request takes).
//!
//! The screens are NOT in `design/cards/index.json` (that manifest's `Slot`
//! enum covers the five conversation cards only), and #28e's drawer/palette
//! containers are not merged yet, so mounting is temporary: [`lower`]
//! reuses the module's own L0 chain (`l0_host::set_copy` → `l0::prepare` →
//! `inspectable` → `design::to_makepad_ui`) and lib.rs mounts the DSL behind
//! the `OCTOSCODE_STAGE_C_SCREENS` env flag until #28e lands.
use serde_json::{json, Value};

use octoscode_store::domains::profile::{InstalledSkill, ProfileLlmModel, SkillPackage};
use octoscode_store::Store;

use crate::bindings::Ctx;
use crate::flow::Conversation;

/// The three cards this screen owns: Stage-B id → artifacts directory under
/// `design/stage-b/setup/cards/` (`gate_b_score` from the accepted reviews).
pub const SCREENS: &[(&str, &str, f64)] = &[
    ("setup-07", "Model settings", 9.0),
    ("setup-09", "Context panel", 9.0),
    ("setup-10", "Skills", 9.0),
];

/// The declared copy-slot → binding-id table (`copy id`, `binding id`).
/// Every id resolves in [`query_binding`]; the f29c coverage test asserts it.
pub const COPY_SLOTS: &[(&str, &str)] = &[
    // setup-07 Model settings
    ("models.title", "models.title"),
    ("models.ds_head", "models.head.0"),
    ("models.ds_count", "models.count.0"),
    ("models.flash", "models.row.0.0"),
    ("models.pro", "models.row.0.1"),
    ("models.kimi_head", "models.head.1"),
    ("models.kimi_count", "models.count.1"),
    ("models.glm_head", "models.head.2"),
    ("models.glm_count", "models.count.2"),
    // setup-09 Context panel
    ("context.title", "context.title"),
    ("context.usage", "context.usage"),
    ("context.pct", "context.pct"),
    ("context.row3", "context.row.0"),
    ("context.row4", "context.row.1"),
    ("context.row5", "context.row.2"),
    ("context.val6", "context.val.0"),
    ("context.val7", "context.val.1"),
    ("context.val8", "context.val.2"),
    ("context.compact", "context.compact_label"),
    ("context.llm", "context.mode_llm"),
    ("context.heur", "context.mode_heur"),
    ("context.keep", "context.keep"),
    // setup-10 Skills
    ("skills.title", "skills.title"),
    ("skills.installed_head", "skills.installed_head"),
    ("skills.registry_head", "skills.registry_head"),
    ("skills.search", "skills.search_ph"),
    ("skills.name0", "skills.installed.0.name"),
    ("skills.name1", "skills.installed.1.name"),
    ("skills.name2", "skills.installed.2.name"),
    ("skills.ver0", "skills.installed.0.version"),
    ("skills.ver1", "skills.installed.1.version"),
    ("skills.ver2", "skills.installed.2.version"),
    ("skills.remove0", "skills.remove_label"),
    ("skills.remove1", "skills.remove_label"),
    ("skills.remove2", "skills.remove_label"),
    ("skills.name10", "skills.registry.0.name"),
    ("skills.name12", "skills.registry.1.name"),
    ("skills.ver11", "skills.registry.0.version"),
    ("skills.ver13", "skills.registry.1.version"),
    ("skills.install3", "skills.install_label"),
    ("skills.install4", "skills.install_label"),
];

/// The control events `service-actions.json` declares for the three cards.
pub fn owns(action: &str) -> bool {
    matches!(action, "models.test_route" | "models.discover" | "context.compact_now")
        || action.starts_with("skills.remove_")
        || action.starts_with("skills.install_")
}

// --------------------------------------------------------------------- bindings

/// `t_usage` composition — the web's occupancy line
/// (`ContextPanel.tsx:59,100`: token estimate + "% of" the window).
fn usage_line(estimate: Option<u64>, window: Option<u64>) -> String {
    match (estimate, window) {
        (Some(e), Some(w)) if w > 0 => format!("{e} of {w} tokens"),
        (Some(e), _) => format!("{e} tokens"),
        _ => "—".to_owned(),
    }
}

/// `t_pct` composition — `ContextPanel.tsx:100` ("62% of 200,000" shows the
/// percent here; the window total lives in `t_keep` on this card).
fn pct_line(estimate: Option<u64>, window: Option<u64>) -> String {
    match (estimate, window) {
        (Some(e), Some(w)) if w > 0 => format!("{}%", e * 100 / w),
        _ => "—".to_owned(),
    }
}

/// One provider card head: `"{provider} • {route}"` — the web renders the
/// route label next to the provider (`model-settings.ts` routeSelection /
/// the atlas card `t_ds_head "DeepSeek • Official API"`).
fn provider_head(m: &ProfileLlmModel) -> String {
    format!("{} • {}", m.provider, m.route.as_deref().unwrap_or("default route"))
}

fn provider_count(models: &[&ProfileLlmModel]) -> String {
    let n = models.len();
    format!("{n} model{}" , if n == 1 { "" } else { "s" })
}

/// A model row: `"{model} (default)"` marks the selected route — the atlas
/// `t_flash "deepseek-v4-flash (default)"` (`ProfileLlmModel.selected`).
fn model_row(m: &ProfileLlmModel) -> String {
    if m.selected {
        format!("{} (default)", m.model)
    } else {
        m.model.clone()
    }
}

fn version_text(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "—".to_owned())
}

/// Resolve one binding id against the store. `None` = not declared here.
pub fn query_binding(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    let store = ctx.store;
    let session = store.active_session().unwrap_or_default();
    let lifecycle = store.domains.session.context(&session);
    let (estimate, window) = lifecycle
        .map(|l| {
            let s = &l.state;
            (
                s.get("token_estimate").and_then(|v| v.as_u64()),
                s.get("window").and_then(|v| v.as_u64()),
            )
        })
        .unwrap_or((None, None));
    let models = store.domains.profile.llm_models();
    let skills = store.domains.profile.installed_skills();
    let registry = store.domains.profile.registry_packages();

    let text = || -> Option<String> {
        Some(match id {
            "models.title" => "Models".to_owned(),
            // Provider cards, grouped by provider in store order.
            "models.head.0" => models.first().map(provider_head)?,
            "models.count.0" => {
                let p = models.first()?.provider.as_str();
                let mine: Vec<&ProfileLlmModel> =
                    models.iter().filter(|m| m.provider == p).collect();
                provider_count(&mine)
            }
            "models.row.0.0" | "models.row.0.1" => {
                let p = models.first()?.provider.as_str();
                let mine: Vec<&ProfileLlmModel> =
                    models.iter().filter(|m| m.provider == p).collect();
                let i = if id.ends_with("0.0") { 0 } else { 1 };
                mine.get(i).map(|m| model_row(m))?
            }
            "models.head.1" => {
                let p0 = models.first()?.provider.clone();
                let first = models.iter().find(|m| m.provider != p0)?;
                provider_head(first)
            }
            "models.count.1" => {
                let p0 = models.first()?.provider.clone();
                let p = models
                    .iter()
                    .map(|m| m.provider.as_str())
                    .find(|p| *p != p0)?;
                let mine: Vec<&ProfileLlmModel> =
                    models.iter().filter(|m| &m.provider == p).collect();
                provider_count(&mine)
            }
            "models.head.2" => "GLM • default route".to_owned(),
            "models.count.2" => "2 models".to_owned(),
            "context.title" => "Context".to_owned(),
            "context.usage" => usage_line(estimate, window),
            "context.pct" => pct_line(estimate, window),
            "context.row.0" => "Input transcripts".to_owned(),
            "context.row.1" => "Tool outputs".to_owned(),
            "context.row.2" => "Reasoning traces".to_owned(),
            "context.val.0" => estimate.map(|e| format!("{e}"))?,
            "context.val.1" => "0".to_owned(),
            "context.val.2" => "0".to_owned(),
            "context.compact_label" => "Compact now".to_owned(),
            "context.mode_llm" => "LLM".to_owned(),
            "context.mode_heur" => "Heuristic".to_owned(),
            "context.keep" => window.map(|w| format!("Keep recent turns · {w}-token window"))?,
            "skills.title" => "Skills".to_owned(),
            "skills.installed_head" => "Installed".to_owned(),
            "skills.registry_head" => "Registry".to_owned(),
            "skills.search_ph" => "Search registry".to_owned(),
            "skills.remove_label" => "Remove".to_owned(),
            "skills.install_label" => "Install".to_owned(),
            "skills.installed.0.name" => skills.first().map(|s: &InstalledSkill| s.name.clone())?,
            "skills.installed.1.name" => skills.get(1).map(|s| s.name.clone())?,
            "skills.installed.2.name" => skills.get(2).map(|s| s.name.clone())?,
            "skills.installed.0.version" => skills.first().and_then(|s| s.version.clone()).map(|_| version_text(&skills.first().unwrap().version))?,
            "skills.installed.1.version" => skills.get(1).map(|s| version_text(&s.version))?,
            "skills.installed.2.version" => skills.get(2).map(|s| version_text(&s.version))?,
            "skills.registry.0.name" => registry.first().map(|s: &SkillPackage| s.name.clone())?,
            "skills.registry.1.name" => registry.get(1).map(|s| s.name.clone())?,
            "skills.registry.0.version" => registry.first().map(|s| version_text(&s.version))?,
            "skills.registry.1.version" => registry.get(1).map(|s| version_text(&s.version))?,
            _ => return None,
        })
    };
    text().map(Value::String)
}

// ---------------------------------------------------------------------- actions

/// The PURE action table: `(action id, store) → (protocol method, params)`.
/// Cited per row (web call sites in `packages/client/src/`):
///
/// | action | method | params cite |
/// |---|---|---|
/// | `models.test_route` | `profile/llm/test` | `client.ts:787`, `LlmProvisionParams` (profile.rs:192) |
/// | `models.discover` | `profile/llm/fetch_models` | `LlmModelFetchSelection` (profile.rs:489) |
/// | `context.compact_now` | `session/compact` | `context-commands.ts:46` (`{session_id}`) |
/// | `skills.remove_N` | `profile/skills/remove` | `skills.ts:191` (`{name}`) |
/// | `skills.install_N` | `profile/skills/install` | `skills.ts:164` (`{repo}`; branch defaults server-side) |
pub fn action_params(action: &str, store: &Store) -> Option<(String, Value)> {
    let session = store.active_session().unwrap_or_default();
    match action {
        "context.compact_now" => Some(("session/compact".to_owned(), json!({ "session_id": session }))),
        "models.test_route" | "models.discover" => {
            let m = store.domains.profile.llm_models().into_iter().next()?;
            let route = json!({
                "route_id": m.route,
                "label": m.provider,
            });
            if action == "models.test_route" {
                Some((
                    "profile/llm/test".to_owned(),
                    json!({ "selection": { "family_id": m.provider, "model_id": m.model, "route": route } }),
                ))
            } else {
                Some((
                    "profile/llm/fetch_models".to_owned(),
                    json!({ "selection": { "family_id": m.provider, "route": route } }),
                ))
            }
        }
        a if a.starts_with("skills.remove_") => {
            let i: usize = a.rsplit('_').next()?.parse().ok()?;
            let name = store.domains.profile.installed_skills().get(i)?.name.clone();
            Some(("profile/skills/remove".to_owned(), json!({ "name": name })))
        }
        a if a.starts_with("skills.install_") => {
            let i: usize = a.rsplit('_').next()?.parse().ok()?;
            let repo = store.domains.profile.registry_packages().get(i)?.repo.clone();
            Some(("profile/skills/install".to_owned(), json!({ "repo": repo })))
        }
        _ => None,
    }
}

/// Execute one screen action through the production client — the same
/// `Client::request` generic path the web's `client.ts:488` uses.
pub async fn perform(conv: &Conversation, action: &str, store: &Store) -> Result<Value, String> {
    let Some((method, params)) = action_params(action, store) else {
        return Err(format!("screens/models: no protocol mapping for {action:?}"));
    };
    conv.client()
        .request(&method, params)
        .await
        .map_err(|e| format!("{method}: {e}"))
}

// --------------------------------------------------------------------- lowering

fn cards_root() -> std::path::PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../design/stage-b/setup/cards")
}

/// Lower one screen card to Splash DSL with the CURRENT store values in its
/// `copy` slots — the same chain `l0_host::lower_slot` runs for the five
/// conversation cards (the renderer that produced the accepted Gate-B PNGs).
/// The artboard stays the card's authored 406×776; the caller supplies the
/// temporary container until #28e's drawer/palette areas land.
pub fn lower(screen_id: &str, ctx: &Ctx<'_>) -> Result<String, String> {
    let dir = cards_root().join(screen_id);
    let read = |rel: &str| -> Result<String, String> {
        std::fs::read_to_string(dir.join(rel)).map_err(|e| format!("read {screen_id}/{rel}: {e}"))
    };
    let mut card_src = read("page.card")?;
    let data: Value = serde_json::from_str(&read("page.data.json")?)
        .map_err(|e| format!("parse {screen_id} data: {e}"))?;
    for (copy_id, binding) in COPY_SLOTS {
        if let Some(v) = query_binding(ctx, binding) {
            if let Value::String(s) = v {
                if let Some(next) = crate::l0_host::set_copy(&card_src, copy_id, &s) {
                    card_src = next;
                }
            }
        }
    }
    let kit_dir = dir.join("kit");
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &kit_dir)
        .map_err(|e| format!("prepare {screen_id}: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let ui = octoscript_makepad::design::to_makepad_ui(&tree)
        .map_err(|e| format!("to_makepad_ui {screen_id}: {e}"))?;
    // Same collision guard as `lower_slot`: rename `beauty_0` per screen.
    let prefix = format!("scr_{}", screen_id.trim_start_matches("setup-"));
    Ok(ui.replace("beauty_0", &prefix))
}

use std::path::PathBuf;
