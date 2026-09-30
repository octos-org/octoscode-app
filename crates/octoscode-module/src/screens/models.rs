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

use octoscode_store::domains::profile::{
    InstalledSkill, ProfileLlmModel, SkillPackage, SubProvider,
};
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
    // copy ids exactly as authored in each card's `page.card` (`copy X { en: … }`);
    // the same copy id may appear in several cards (t_title_text) — set_copy runs
    // per card, so that is fine.
    // setup-07 Model settings
    ("t_title_text", "models.title"),
    ("t_ds_head_text", "models.head.0"),
    ("t_ds_count_text", "models.count.0"),
    ("t_flash_text", "models.row.0.0"),
    ("t_pro_text", "models.row.0.1"),
    ("btn_test_label_text", "models.test_label"),
    ("btn_discover_label_text", "models.discover_label"),
    ("t_kimi_head_text", "models.head.1"),
    ("t_kimi_count_text", "models.count.1"),
    ("t_glm_head_text", "models.head.2"),
    ("t_glm_count_text", "models.count.2"),
    // setup-09 Context panel
    ("t_title_text", "context.title"),
    ("t_usage_text", "context.usage"),
    ("t_pct_text", "context.pct"),
    ("btn_compact_label_text", "context.compact_label"),
    ("t_llm_text", "context.mode_llm"),
    ("t_heur_text", "context.mode_heur"),
    // setup-10 Skills
    ("t_title_text", "skills.title"),
    ("t_inst_head_text", "skills.installed_head"),
    ("t_reg_head_text", "skills.registry_head"),
    ("t_search_text", "skills.search_ph"),
    ("t_name3_text", "skills.installed.0.name"),
    ("t_name4_text", "skills.installed.1.name"),
    ("t_name5_text", "skills.installed.2.name"),
    ("t_ver6_text", "skills.installed.0.version"),
    ("t_ver7_text", "skills.installed.1.version"),
    ("t_ver8_text", "skills.installed.2.version"),
    ("t_remove0_text", "skills.remove_label"),
    ("t_remove1_text", "skills.remove_label"),
    ("t_remove2_text", "skills.remove_label"),
    ("t_name10_text", "skills.registry.0.name"),
    ("t_ver11_text", "skills.registry.0.version"),
    ("t_name12_text", "skills.registry.1.name"),
    ("t_ver13_text", "skills.registry.1.version"),
    ("btn_3_install_label_text", "skills.install_label"),
    ("btn_4_install_label_text", "skills.install_label"),
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
    /// k-format, as the authored card reads it ("124k of 200k tokens").
    fn k(v: u64) -> String {
        if v >= 1000 && v % 1000 == 0 {
            format!("{}k", v / 1000)
        } else {
            v.to_string()
        }
    }
    match (estimate, window) {
        (Some(e), Some(w)) if w > 0 => format!("{} of {} tokens", k(e), k(w)),
        (Some(e), _) => format!("{} tokens", k(e)),
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

/// The web's family display label — `familyLabel`
/// (`model-management-projection.ts:207-215`): a known-id map with a
/// `titleCase` fallback, NOT the raw id.
fn family_label(id: &str) -> String {
    match id {
        "zai" => "Z.AI".to_owned(),
        "zai-coding" => "Z.AI Coding Plan".to_owned(),
        "deepseek" => "DeepSeek".to_owned(),
        "ollama" => "Ollama".to_owned(),
        "openai" => "OpenAI".to_owned(),
        "anthropic" => "Anthropic".to_owned(),
        _ => title_case(id),
    }
}

fn title_case(id: &str) -> String {
    id.split('-')
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// One provider card head: `"{display label} • {route}"` (the web renders the
/// family label next to the route; the authored head reads
/// "DeepSeek • Official API").
fn provider_head(m: &ProfileLlmModel) -> String {
    format!("{} • {}", family_label(&m.provider), m.route.as_deref().unwrap_or("default route"))
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

/// The context-occupancy WINDOW, exactly where the web gets it: the
/// `progress/updated` payload whose `metadata.kind == "token_cost_update"`
/// carries `token_cost.context_window` (`workspace-events.ts:6-10` —
/// `parseTokenCostUpdate` listens on PROGRESS_UPDATED; live-gate fixture
/// frame: 1048576 for deepseek-v4-flash). Guarded by session id the way the
/// panel guards `usage.sessionId === sessionId` (`ContextPanel.tsx:39`,
/// `model.ts:25`). UI-adjacent state with no store field, so it lives here;
/// every test writes the SAME (session, 200_000) pair, so parallel-test
/// races converge on one value.
static WINDOW: std::sync::Mutex<Option<(String, u64)>> = std::sync::Mutex::new(None);

/// Record a token_cost window for a session (the wire fold; also the test
/// seam — the recorded live-gate value is 1_048_576).
pub fn note_token_cost(session: &str, window: u64) {
    *WINDOW.lock().unwrap() = Some((session.to_owned(), window));
}

/// Fold one transport event if it carries a token_cost_update.
pub fn note_transport_event(evt: &octos_app_transport::TransportEvent) {
    use octos_app_transport::TransportEvent;
    let payload = match evt {
        TransportEvent::DurableNotification { payload, .. }
        | TransportEvent::EphemeralNotification { payload } => payload,
        _ => return,
    };
    if payload.method() != "progress/updated" {
        return;
    }
    let body = octoscode_client::trace::wire_params(payload);
    if body["metadata"]["kind"] != "token_cost_update" {
        return;
    }
    let (Some(window), Some(session)) = (
        body["metadata"]["token_cost"]["context_window"].as_u64(),
        body["session_id"].as_str(),
    ) else {
        return;
    };
    note_token_cost(session, window);
}

/// Resolve one binding id against the store. `None` = not declared here.
pub fn query_binding(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    let store = ctx.store;
    let session = store.active_session().unwrap_or_default();
    // `context.usage`/`context.pct` composition (web ContextPanel.tsx:96-104):
    // the estimate is the lifecycle's `token_estimate` (UiContextState,
    // r3-session fixture); the window is the provider entry's
    // `default_context_window` (store `SubProvider`).
    let estimate = store
        .domains
        .session
        .context(&session)
        .and_then(|l| l.state.get("token_estimate").and_then(|v| v.as_u64()));
    let window = match WINDOW.lock().unwrap().as_ref() {
        Some((s, w)) if *s == session => Some(*w),
        _ => None,
    };
    let models = store.domains.profile.llm_models();
    let skills = store.domains.profile.installed_skills();
    let registry = store.domains.profile.registry_packages();

    // #31b: the context-panel lifecycle leg (web `ContextPanel.tsx:50,135-143`
    // renders a compaction spinner while a pass runs and
    // "Last compaction: {status} · before → after" from the record). The
    // store's last `context/*` lifecycle event is the single source; the
    // (kind, detail) pair distinguishes in-progress from finished.
    if id == "context.lifecycle" {
        return Some(match store.domains.session.context(&session) {
            Some(l) => json!({
                "kind": l.kind,
                "state": l.state,
                "detail": l.detail,
            }),
            None => Value::Null,
        });
    }
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
                match mine.get(i) {
                    Some(m) => model_row(m),
                    // live count ("1 model") over a chrome-frozen second
                    // slot: blank beats the authored demo row contradicting
                    // the count.
                    None => String::new(),
                }
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
            "models.head.2" => {
                let p0 = models.first()?.provider.clone();
                let p1 = models.iter().map(|m| m.provider.clone()).find(|p| *p != p0)?;
                let first = models.iter().find(|m| m.provider != p0 && m.provider != p1)?;
                provider_head(first)
            }
            "models.count.2" => {
                let p0 = models.first()?.provider.clone();
                let p1 = models.iter().map(|m| m.provider.clone()).find(|p| *p != p0)?;
                let mine: Vec<&ProfileLlmModel> = models
                    .iter()
                    .filter(|m| m.provider != p0 && m.provider != p1)
                    .collect();
                provider_count(&mine)
            }
            "models.test_label" => "Test route".to_owned(),
            "models.discover_label" => "Discover models".to_owned(),
            "context.title" => "Context".to_owned(),
            "context.usage" => usage_line(estimate, window),
            "context.pct" => pct_line(estimate, window),
            "context.compact_label" => "Compact now".to_owned(),
            "context.mode_llm" => "LLM".to_owned(),
            "context.mode_heur" => "Heuristic".to_owned(),
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
            // The store row keeps the route's DISPLAY label ("Official API",
            // the string the web renders); the recorded wire route_id equals
            // the family (r2: route_id "deepseek", family "deepseek"), so
            // reconstruct the pair as (family, label).
            let route = json!({
                "route_id": m.provider,
                "label": m.route,
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
            let row = i.checked_sub(INSTALL_BASE)?;
            let repo = store.domains.profile.registry_packages().get(row)?.repo.clone();
            Some(("profile/skills/install".to_owned(), json!({ "repo": repo })))
        }
        _ => None,
    }
}

/// `skills.install_N` targets registry row `N - INSTALL_BASE`: the authored
/// card puts buttons 3/4 on registry rows 0/1 (`setup-10/service-actions.json`:
/// `btn_3_install` sits on the row bearing `t_name10_text`).
const INSTALL_BASE: usize = 3;

// --------------------------------------------------------------------- refresh

/// Pull the three profile reads and fold them into the store — the web's
/// settings/dialog load path (`profile/llm/list` via `llm-methods.ts` /
/// `client.ts:759`; `profile/skills/list` via `skills.ts:142`;
/// `profile/sub_providers/list` via `research.ts:145`). Production call site:
/// lib.rs `start()` behind `OCTOSCODE_STAGE_C_SCREENS` until #28e lands;
/// the f29c replay test calls it directly against the recorded frames.
pub async fn refresh(conv: &Conversation, store: &Store) -> Result<usize, String> {
    let client = conv.client();
    let mut done = 0usize;
    let mut errs = Vec::new();
    for (method, fold) in [
        ("profile/llm/list", fold_llm_list as fn(Value, &Store)),
        ("profile/skills/list", fold_skills_list),
        ("profile/sub_providers/list", fold_sub_providers),
    ] {
        match client.request(method, json!({})).await {
            Ok(v) => {
                fold(v, store);
                done += 1;
            }
            Err(e) => errs.push(format!("{method}: {e}")),
        }
    }
    if done == 0 {
        return Err(errs.join("; "));
    }
    Ok(done)
}

/// One wire model entry -> a store row. The recorded `profile/llm/list`
/// carries `primary` + `fallbacks` (r2-profile-a6ea8505.jsonl; the typed
/// result's `models` key is absent on the wire), so rows are built from
/// those. `route` keeps the wire's DISPLAY label ("Official API", the string
/// the web renders) and falls back to the route id.
fn model_row_from(obj: &Value, selected: bool) -> Option<ProfileLlmModel> {
    let model = obj.get("model_id").and_then(|v| v.as_str())?.to_string();
    let provider = obj.get("family_id").and_then(|v| v.as_str())?.to_string();
    let route = obj.get("route").and_then(|r| {
        r.get("label")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| r.get("route_id").and_then(|v| v.as_str()).map(str::to_owned))
    });
    Some(ProfileLlmModel {
        title: model.clone(),
        family: Some(provider.clone()),
        model,
        provider,
        route,
        selected,
        available: obj.get("available").and_then(|v| v.as_bool()).unwrap_or(true),
    })
}

pub fn fold_llm_list(v: Value, store: &Store) {
    let mut rows: Vec<ProfileLlmModel> = Vec::new();
    if let Some(m) = v.get("primary").and_then(|p| model_row_from(p, true)) {
        rows.push(m);
    }
    if let Some(fallbacks) = v.get("fallbacks").and_then(|f| f.as_array()) {
        for f in fallbacks {
            if let Some(m) = model_row_from(f, false) {
                rows.push(m);
            }
        }
    }
    if !rows.is_empty() {
        store.domains.profile.set_llm_models(rows);
    }
}

pub fn fold_skills_list(v: Value, store: &Store) {
    let skills = v
        .get("skills")
        .and_then(|s| s.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|s| {
                    Some(InstalledSkill {
                        name: s.get("name")?.as_str()?.to_string(),
                        version: s.get("version").and_then(|v| v.as_str()).map(str::to_owned),
                        tool_count: s.get("tool_count").and_then(|v| v.as_u64()).unwrap_or(0),
                        source_repo: s.get("source_repo").and_then(|v| v.as_str()).map(str::to_owned),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    store.domains.profile.set_installed_skills(skills);
}

fn opt_str(s: &Value, key: &str) -> Option<String> {
    s.get(key).and_then(|v| v.as_str()).map(str::to_owned)
}

fn opt_u32(s: &Value, key: &str) -> Option<u32> {
    s.get(key).and_then(|v| v.as_u64()).map(|w| w as u32)
}

pub fn fold_sub_providers(v: Value, store: &Store) {
    let subs = v
        .get("sub_providers")
        .and_then(|s| s.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|s| {
                    Some(SubProvider {
                        key: s.get("key")?.as_str()?.to_string(),
                        provider: opt_str(s, "provider").unwrap_or_default(),
                        model: opt_str(s, "model"),
                        api_key_env: opt_str(s, "api_key_env"),
                        base_url: opt_str(s, "base_url"),
                        description: opt_str(s, "description"),
                        default_context_window: opt_u32(s, "default_context_window"),
                        max_output_tokens: opt_u32(s, "max_output_tokens"),
                        api_type: opt_str(s, "api_type"),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    store.domains.profile.set_sub_providers(subs);
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

/// The binding namespace one screen owns (`setup-07` -> `models.`).
fn screen_ns(screen_id: &str) -> &'static str {
    match screen_id {
        "setup-07" => "models",
        "setup-09" => "context",
        "setup-10" => "skills",
        _ => "",
    }
}

fn cards_root() -> std::path::PathBuf {
    crate::design::dir("stage-b/setup/cards")
}

/// Lower one screen card to Splash DSL with the CURRENT store values in its
/// `copy` slots — the same chain `l0_host::lower_slot` runs for the five
/// conversation cards (the renderer that produced the accepted Gate-B PNGs).
/// The artboard stays the card's authored 406×776; the caller supplies the
/// temporary container until #28e's drawer/palette areas land.
/// The card source with the CURRENT store values written into its `copy`
/// slots, plus its data + kit dir — what a renderer (or the visual driver)
/// consumes. Pub for the f29c capture test.
pub fn lower_card_src(screen_id: &str, ctx: &Ctx<'_>) -> Result<(String, Value, PathBuf), String> {
    let dir = cards_root().join(screen_id);
    let read = |rel: &str| -> Result<String, String> {
        std::fs::read_to_string(dir.join(rel)).map_err(|e| format!("read {screen_id}/{rel}: {e}"))
    };
    let mut card_src = read("page.card")?;
    // mut: #29c2 item 1 rewrites the inner-card placements for the live row count
    let mut data: Value = serde_json::from_str(&read("page.data.json")?)
        .map_err(|e| format!("parse {screen_id} data: {e}"))?;
    // Only this screen's namespace may write its copies: `t_title_text` is
    // authored in all three cards and the table maps it once per screen
    // (models.title / context.title / skills.title) — unfiltered, the LAST
    // entry won and every screen rendered the title "Skills".
    let ns = format!("{}.", screen_ns(screen_id));
    for (copy_id, binding) in COPY_SLOTS {
        if !binding.starts_with(&ns) {
            continue;
        }
        if let Some(v) = query_binding(ctx, binding) {
            if let Value::String(s) = v {
                if let Some(next) = crate::l0_host::set_copy(&card_src, copy_id, &s) {
                    card_src = next;
                }
            }
        }
    }
    // Entry #29c2 item 1: the inner card sizes to its LIVE rows. Authored
    // geometry holds two model rows (inner_card 213..339, divider y276.5);
    // with fewer rows than that, shrink the card to the divider line and zero
    // the divider — no blank second row, no divider.
    if ns == "models." {
        let models = ctx.store.domains.profile.llm_models();
        let rows = match models.first() {
            Some(first) => models.iter().filter(|m| m.provider == first.provider).count(),
            None => 0,
        };
        if rows < 2 {
            if let Some(placements) = data
                .get_mut("$kit")
                .and_then(|k| k.get_mut("placements"))
            {
                if let Some(h) = placements
                    .get_mut("inner_card")
                    .and_then(|c| c.get_mut("layout"))
                    .and_then(|l| l.get_mut("h"))
                {
                    *h = json!(63.5);
                }
                if let Some(h) = placements
                    .get_mut("inner_div")
                    .and_then(|c| c.get_mut("layout"))
                    .and_then(|l| l.get_mut("h"))
                {
                    *h = json!(0.0);
                }
            }
        }
    }
    Ok((card_src, data, dir.join("kit")))
}

/// Lower one screen card to Splash DSL with the CURRENT store values.
pub fn lower(screen_id: &str, ctx: &Ctx<'_>) -> Result<String, String> {
    let (card_src, data, kit_dir) = lower_card_src(screen_id, ctx)?;
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &kit_dir)
        .map_err(|e| format!("prepare {screen_id}: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let ui = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&tree))
        .map_err(|e| format!("to_makepad_ui {screen_id}: {e}"))?;
    // Same collision guard as `lower_slot`: rename `beauty_0` per screen.
    let prefix = format!("scr_{}", screen_id.trim_start_matches("setup-"));
    Ok(ui.replace("beauty_0", &prefix))
}

use std::path::PathBuf;
