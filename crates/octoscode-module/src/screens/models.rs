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

/// #M1: the card→screen gate the mount ladder calls, exactly as
/// `theme::card_for` (theme.rs:261) does for the theme cards. Without it
/// `models::lower` had ZERO call sites: the three cards were accepted Stage-B
/// designs with wired handlers, but no mount path could reach them, so every
/// skills/models/context row was production-path yet user-unreachable (RULES 3).
///
/// Returns the screen's namespace, which `lower_card_src` also keys on
/// (`screen_ns`), so the gate and the lowerer cannot drift.
pub fn card_for(which: &str) -> Option<&'static str> {
    match which {
        "setup-07" => Some("models"),
        "setup-09" => Some("context"),
        "setup-10" => Some("skills"),
        _ => None,
    }
}

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

/// The control events `service-actions.json` declares for the three cards,
/// plus the context dialog's two compaction-mode halves (A5:
/// `ContextPanel.tsx` `onModeChange("llm" | "heuristic")`).
pub fn owns(action: &str) -> bool {
    matches!(
        action,
        "models.test_route"
            | "models.discover"
            | "context.compact_now"
            | "context.mode.llm"
            | "context.mode.heuristic"
    ) || action.starts_with("skills.remove_")
        || action.starts_with("skills.install_")
}

/// A5 — the SERVER-CONFIRMED compaction mode per session: the
/// `session/compact/mode/set` read-back (`res:session/compact/mode/set
/// {"mode":"heuristic","session_id":…}`, r3-session recording). The web keeps
/// the confirmed value and selects it (`ContextPanel.tsx` `value={mode ?? ""}`)
/// — never the requested one before the server answered.
static COMPACT_MODE: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);

/// The confirmed mode for `session`, if the server has confirmed one.
pub fn compact_mode(session: &str) -> Option<String> {
    match COMPACT_MODE.lock().unwrap().as_ref() {
        Some((s, m)) if s == session => Some(m.clone()),
        _ => None,
    }
}

/// Fold a `session/compact/mode/set` result. Only `llm`/`heuristic` are modes;
/// anything else is not a confirmation.
pub fn note_compact_mode(result: &Value) -> Option<String> {
    let session = result.get("session_id")?.as_str()?;
    let mode = result.get("mode")?.as_str()?;
    if !matches!(mode, "llm" | "heuristic") {
        return None;
    }
    *COMPACT_MODE.lock().unwrap() = Some((session.to_owned(), mode.to_owned()));
    Some(mode.to_owned())
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
pub(crate) fn provider_head(m: &ProfileLlmModel) -> String {
    format!("{} • {}", family_label(&m.provider), m.route.as_deref().unwrap_or("default route"))
}

pub(crate) fn provider_count(models: &[&ProfileLlmModel]) -> String {
    let n = models.len();
    format!("{n} model{}" , if n == 1 { "" } else { "s" })
}

/// A model row: `"{model} (default)"` marks the selected route — the atlas
/// `t_flash "deepseek-v4-flash (default)"` (`ProfileLlmModel.selected`).
pub(crate) fn model_row(m: &ProfileLlmModel) -> String {
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

/// #P4h1 row 300 — the merged `TokenCostUpdate` projection, ported field-for-
/// field from the web's `mergeTokenCost` (`features/workspace/model.ts:58`):
///
/// ```ts
/// if (!current || current.sessionId !== next.sessionId) return next;
/// return { ...current, ...entries(next).filter(([, v]) => v !== undefined) };
/// ```
///
/// So a live update for a DIFFERENT session REPLACES the projection, and an
/// update for the same session MERGES only the fields it actually carries —
/// a sparse frame that carries `context_window` alone must not erase the
/// `session_cost` an earlier frame brought. Every recorded token_cost frame is
/// dense (5/5 fields), which is exactly why this gap survived: the merge only
/// differs on a sparse frame, and the native fold read `context_window` alone
/// and kept only that one number.
static TOKEN_COST: std::sync::Mutex<Option<TokenCostUpdate>> = std::sync::Mutex::new(None);

/// One merged token-cost update: the `sessionId` plus the carried fields, kept
/// as a JSON object so "this frame did not carry this field" stays
/// representable (`undefined` is what the web's spread filter drops).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TokenCostUpdate {
    pub session_id: String,
    /// The carried fields verbatim (the web keeps the typed object whole; a
    /// JSON object is the same map the wire sends).
    pub fields: serde_json::Value,
}

impl TokenCostUpdate {
    /// The web's `mergeTokenCost`, pure so it is testable without the fold.
    /// `current` is the projection so far; `next` is the frame that just
    /// arrived.
    pub fn merge(
        current: Option<&TokenCostUpdate>,
        next: TokenCostUpdate,
    ) -> TokenCostUpdate {
        let Some(current) = current else {
            return next;
        };
        // A different session never merges: the web returns `next` outright.
        if current.session_id != next.session_id {
            return next;
        }
        let serde_json::Value::Object(current_fields) = &current.fields else {
            return next;
        };
        let mut merged = current_fields.clone();
        if let serde_json::Value::Object(next_fields) = &next.fields {
            for (key, value) in next_fields {
                // The web's `value !== undefined` filter. A JSON `null` is NOT
                // undefined: the web spreads a null over the old value, so we
                // do too. Absent keys are the only thing dropped.
                merged.insert(key.clone(), value.clone());
            }
        }
        TokenCostUpdate {
            session_id: next.session_id,
            fields: serde_json::Value::Object(merged),
        }
    }
}

/// The merged projection for a session, if the last update named it.
pub fn token_cost(session: &str) -> Option<TokenCostUpdate> {
    let guard = TOKEN_COST.lock().unwrap();
    match guard.as_ref() {
        Some(u) if u.session_id == session => Some(u.clone()),
        _ => None,
    }
}

/// Fold one token-cost update into the merged projection (`observeTokenCost`,
/// `use-workspace-product.ts:158-162`). This is the production entry the
/// `progress/updated` fold below calls.
pub fn observe_token_cost(next: TokenCostUpdate) -> TokenCostUpdate {
    let mut guard = TOKEN_COST.lock().unwrap();
    let merged = TokenCostUpdate::merge(guard.as_ref(), next);
    *guard = Some(merged.clone());
    merged
}

/// Test seam: drop the merged projection.
pub fn reset_token_cost() {
    *TOKEN_COST.lock().unwrap() = None;
}

/// Record a token_cost window for a session. This is the CONTEXT PANEL's slot
/// (`context.pct` divides by it) and the test seam — the recorded live-gate
/// value is 1_048_576. The wire fold writes it via [`note_transport_event`],
/// which also feeds the merged projection.
pub fn note_token_cost(session: &str, window: u64) {
    *WINDOW.lock().unwrap() = Some((session.to_owned(), window));
}

/// A5 — the context lifecycle's event revision: every `context/*`
/// notification bumps it, so a `session/status/read` that was issued BEFORE a
/// newer lifecycle event cannot erase it (`ContextDialog.tsx` `refresh`: "A
/// delayed read cannot erase newer lifecycle notifications").
static CONTEXT_REV: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The current context event revision.
pub fn context_revision() -> u64 {
    CONTEXT_REV.load(std::sync::atomic::Ordering::SeqCst)
}

/// Record that a context lifecycle notification landed.
pub fn note_context_event() {
    CONTEXT_REV.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// Fold one transport event if it carries a token_cost_update.
pub fn note_transport_event(evt: &octos_app_transport::TransportEvent) {
    use octos_app_transport::TransportEvent;
    let payload = match evt {
        TransportEvent::DurableNotification { payload, .. }
        | TransportEvent::EphemeralNotification { payload } => payload,
        _ => return,
    };
    if payload.method().starts_with("context/") {
        note_context_event();
        return;
    }
    if payload.method() != "progress/updated" {
        return;
    }
    let body = octoscode_client::trace::wire_params(payload);
    if body["metadata"]["kind"] != "token_cost_update" {
        return;
    }
    let (Some(cost), Some(session)) = (
        body["metadata"]["token_cost"].as_object(),
        body["session_id"].as_str(),
    ) else {
        return;
    };
    // #P4h1 row 300: fold the WHOLE carried object into the merged projection
    // first, so the context panel's window is a READER of that projection
    // rather than a second, un-merged copy of one field.
    let fields = serde_json::Value::Object(cost.clone());
    let merged = observe_token_cost(TokenCostUpdate {
        session_id: session.to_owned(),
        fields,
    });
    if let Some(window) = merged
        .fields
        .get("context_window")
        .and_then(|v| v.as_u64())
    {
        note_token_cost(session, window);
    }
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
    // #P4h1 row 300: the window now comes from the MERGED token-cost
    // projection, which is what the `progress/updated` fold writes. It was a
    // second, un-merged copy of one field (`WINDOW`) with no other consumer of
    // the frame, so a sparse frame that carried only `context_window` was the
    // whole of what native knew about the update. Reading the projection makes
    // the merge a production path and gives the cost row a source at the same
    // time.
    let cost = token_cost(&session);
    let window = cost
        .as_ref()
        .and_then(|u| u.fields.get("context_window"))
        .and_then(|v| v.as_u64())
        .or_else(|| match WINDOW.lock().unwrap().as_ref() {
            // The panel keeps working for a window written by a caller that
            // only has the one number (the test seam, and any future reader).
            Some((s, w)) if *s == session => Some(*w),
            _ => None,
        });
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
        // A5 — the recorded request (r3-session): `{"mode":"heuristic",
        // "session_id":…}`; the web sends the same pair (`context-commands.ts`
        // `setMode`).
        "context.mode.llm" | "context.mode.heuristic" => Some((
            "session/compact/mode/set".to_owned(),
            json!({
                "mode": if action.ends_with("llm") { "llm" } else { "heuristic" },
                "session_id": session,
            }),
        )),
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
        // A10: the web's skill wires (`packages/client/src/skills.ts:160-200`):
        // scoped to the Profile; an install says `force: false` and names a
        // branch only when one was typed (the r2 recording's install shape).
        a if a.starts_with("skills.remove_") => {
            let i: usize = a.rsplit('_').next()?.parse().ok()?;
            let name = store.domains.profile.installed_skills().get(i)?.name.clone();
            let mut p = json!({ "name": name });
            if let Some(profile) = store.domains.profile.current() {
                p["profile_id"] = json!(profile);
            }
            Some(("profile/skills/remove".to_owned(), p))
        }
        "skills.install_source" => {
            let (repo, branch) = crate::screens::dialog::skills_source();
            install_params(store, &repo, &branch)
        }
        a if a.starts_with("skills.install_") => {
            let i: usize = a.rsplit('_').next()?.parse().ok()?;
            let row = i.checked_sub(INSTALL_BASE)?;
            let repo = store.domains.profile.registry_packages().get(row)?.repo.clone();
            install_params(store, &repo, "")
        }
        _ => None,
    }
}

/// `profile/skills/install` (`skills.ts:160-187`): `{profile_id, repo,
/// branch?, force: false}` — the repo/branch trimmed, a blank branch left out
/// (the server's default, main). `None` for a blank repo.
fn install_params(store: &Store, repo: &str, branch: &str) -> Option<(String, Value)> {
    let repo = repo.trim();
    if repo.is_empty() {
        return None;
    }
    let mut p = json!({ "repo": repo, "force": false });
    if let Some(profile) = store.domains.profile.current() {
        p["profile_id"] = json!(profile);
    }
    if !branch.trim().is_empty() {
        p["branch"] = json!(branch.trim());
    }
    Some(("profile/skills/install".to_owned(), p))
}

/// The web's install receipt line (`SkillsDialog.tsx:121-123`).
pub fn install_notice(result: &Value) -> String {
    let list = |k: &str| -> String {
        let v: Vec<&str> = result
            .get(k)
            .and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|s| s.as_str()).collect())
            .unwrap_or_default();
        if v.is_empty() { "none".to_owned() } else { v.join(", ") }
    };
    format!(
        "Server installed: {}. Skipped: {}. Dependencies: {}.",
        list("installed"),
        list("skipped"),
        list("deps_installed")
    )
}

/// The web's mutation failure copy (`SkillsDialog.tsx:76-79`): the change
/// may have applied, so it never echoes a raw transport error.
pub const SKILLS_MUTATION_FAILED: &str = "Could not confirm the server change. It may have been applied; \
                                          refresh the Profile before reviewing another attempt.";

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
    // A10: the reads are scoped to the connection's Profile (the r2
    // recording's `{profile_id}` requests; the web's `skillCommands(profileId)`),
    // and the store learns that Profile when no `session/opened` named one —
    // the Skills/Research copy and the mutation wires need it.
    if store.domains.profile.current().is_none() {
        store.domains.profile.set_current(conv.profile());
    }
    let profile = store.domains.profile.current().unwrap_or_else(|| conv.profile());
    for (method, fold) in [
        ("profile/llm/list", fold_llm_list as fn(Value, &Store)),
        ("profile/skills/list", fold_skills_list),
        ("profile/sub_providers/list", fold_sub_providers),
    ] {
        match client.request(method, json!({ "profile_id": profile })).await {
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

/// A5 — `profile/skills/registry/search` (`skills.ts:152` `search(q)`; the
/// Skills dialog's search box, Enter): the packages for `query`, scoped to
/// the connection's Profile, folded into the store the dialog draws from.
pub async fn search_registry(conv: &Conversation, store: &Store, query: &str) -> Result<usize, String> {
    let mut params = json!({ "q": query });
    if let Some(profile) = store.domains.profile.current() {
        params["profile_id"] = json!(profile);
    }
    let v = conv
        .client()
        .request("profile/skills/registry/search", params)
        .await
        .map_err(|e| format!("profile/skills/registry/search: {e}"))?;
    Ok(fold_registry_search(v, store))
}

/// The search result's `packages` (`skills.ts:79` `parseSkillPackages`)
/// REPLACE the previous result (the web's `setPackages`); a malformed reply
/// folds as no packages, never as stale ones.
pub fn fold_registry_search(v: Value, store: &Store) -> usize {
    let packages: Vec<SkillPackage> = v
        .get("packages")
        .and_then(|p| p.as_array())
        .map(|list| list.iter().filter_map(|p| serde_json::from_value(p.clone()).ok()).collect())
        .unwrap_or_default();
    let n = packages.len();
    store.domains.profile.set_registry_packages(packages);
    n
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
///
/// A5: the result is FOLDED the way the web's dialogs fold theirs, so the open
/// dialog shows the server's answer rather than the pre-click state:
/// * `context.mode.*` — the confirmed mode (`ContextDialog.tsx` `setMode`
///   keeps `confirmed`, never the requested value), then the authoritative
///   refresh (`mutate` → `refresh()`);
/// * `context.compact_now` — the authoritative refresh (the lifecycle
///   notifications carry the pass itself);
/// * `skills.install_N` / `skills.remove_N` — the installed list is re-read
///   (`SkillsDialog.tsx` reloads after a confirmed change).
pub async fn perform(conv: &Conversation, action: &str, store: &Store) -> Result<Value, String> {
    let Some((method, params)) = action_params(action, store) else {
        return Err(format!("screens/models: no protocol mapping for {action:?}"));
    };
    // A10 — a skill mutation holds the Profile lease (the web's
    // `onMutationStart` / ProfileMutationLeases): one at a time — a second
    // one while it is held is refused (deduplicated), and every other
    // Profile mutation (research lanes) pauses until it is released.
    let mutation = action.starts_with("skills.install_") || action.starts_with("skills.remove_");
    if mutation {
        if store.domains.profile.profile_busy() {
            // The web's `run` returns silently on a pending request; the
            // dialog already shows the lock line and wires no mutation.
            return Err(format!("{method}: the Profile is busy (a mutation is pending)"));
        }
        store.domains.profile.set_profile_busy(true);
        // The open dialog re-lowers with the lock line while it is held.
        makepad_widgets::SignalToUI::set_ui_signal();
    }
    let sent = conv.client().request(&method, params).await;
    if mutation {
        store.domains.profile.set_profile_busy(false);
    }
    let result = match sent {
        Ok(v) => v,
        Err(e) => {
            // The open dialog shows the failure under its controls (the web
            // dialogs' `setError(errorText(cause))` alert line); a skill
            // mutation's failure is the web's fixed may-have-applied copy.
            if mutation {
                crate::screens::dialog::set_notice(SKILLS_MUTATION_FAILED);
            } else {
                crate::screens::dialog::set_notice(format!("{e}"));
            }
            return Err(format!("{method}: {e}"));
        }
    };
    match action {
        "context.mode.llm" | "context.mode.heuristic" => {
            if note_compact_mode(&result).is_none() {
                ::log::warn!("octoscode: {method}: no confirmed mode in the reply");
            }
            let _ = refresh_context(conv, store).await;
        }
        "context.compact_now" => {
            // `ContextDialog.tsx` mutate("compact"): the outcome line —
            // "Context compacted." or "Compaction {status}: {reason}".
            // Both outcomes are the web's `role="status"` result line (only
            // a failed REQUEST is its `role="alert"` error, the Err above).
            let (line, _compacted) = compaction_outcome(&result);
            crate::screens::dialog::set_info(line);
            let _ = refresh_context(conv, store).await;
        }
        "models.test_route" => {
            // `LlmTestResult` {applied, message, error}.
            match result.get("error").and_then(|e| e.as_str()).filter(|e| !e.is_empty()) {
                Some(err) => crate::screens::dialog::set_notice(format!("Route test failed: {err}")),
                None => {
                    let msg = result.get("message").and_then(|m| m.as_str()).unwrap_or("");
                    crate::screens::dialog::set_info(if msg.is_empty() {
                        "Route test finished.".to_owned()
                    } else {
                        format!("Route test: {msg}")
                    });
                }
            }
        }
        "models.discover" => {
            // `LlmFetchModelsResult` {models, reason, status}.
            let models: Vec<&str> = result
                .get("models")
                .and_then(|m| m.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            if models.is_empty() {
                let why = result.get("reason").and_then(|r| r.as_str()).unwrap_or("no models reported");
                crate::screens::dialog::set_notice(format!("Model discovery: {why}"));
            } else {
                crate::screens::dialog::set_info(format!(
                    "Found {} model{}: {}",
                    models.len(),
                    if models.len() == 1 { "" } else { "s" },
                    models.join(", ")
                ));
            }
        }
        a if a.starts_with("skills.install_") || a.starts_with("skills.remove_") => {
            // A10: the web's receipt line, the searched packages cleared
            // (`setPackages(null)`), then the Profile's list re-read.
            if a.starts_with("skills.install_") {
                crate::screens::dialog::set_info(install_notice(&result));
            } else {
                let name = result.get("removed").and_then(|r| r.as_str()).map(str::to_owned).unwrap_or_else(|| {
                    a.rsplit('_').next().and_then(|i| i.parse::<usize>().ok()).and_then(|i| {
                        store.domains.profile.installed_skills().get(i).map(|s| s.name.clone())
                    }).unwrap_or_default()
                });
                let profile = store.domains.profile.current().unwrap_or_default();
                crate::screens::dialog::set_info(format!("Removed {name} from server Profile {profile}."));
            }
            store.domains.profile.set_registry_packages(Vec::new());
            crate::screens::dialog::set_skills_query(None);
            let mut p = json!({});
            if let Some(profile) = store.domains.profile.current() {
                p["profile_id"] = json!(profile);
            }
            if let Ok(v) = conv.client().request("profile/skills/list", p).await {
                fold_skills_list(v, store);
            }
        }
        _ => {}
    }
    Ok(result)
}

/// The web's compaction result line (`ContextDialog.tsx` `mutate`):
/// `compacted ? "Context compacted." : "Compaction {status}{: reason | .}"`.
/// Returns the line and whether the pass compacted.
pub fn compaction_outcome(result: &Value) -> (String, bool) {
    if result.get("compacted").and_then(|c| c.as_bool()) == Some(true) {
        return ("Context compacted.".to_owned(), true);
    }
    let status = result.get("status").and_then(|s| s.as_str()).unwrap_or("not completed");
    match result.get("reason").and_then(|r| r.as_str()).filter(|r| !r.is_empty()) {
        Some(reason) => (format!("Compaction {status}: {reason}"), false),
        None => (format!("Compaction {status}."), false),
    }
}

/// A5 — the context dialog's AUTHORITATIVE refresh: `session/status/read`
/// (`ContextDialog.tsx` `refresh`, gated on `SESSION_STATUS_READ`). The read's
/// `context_state` replaces the session's lifecycle snapshot UNLESS a
/// lifecycle notification landed while the read was in flight and the read is
/// not newer than it (generation) — the web's "a delayed read cannot erase
/// newer lifecycle notifications". Returns whether the read was applied.
pub async fn refresh_context(conv: &Conversation, store: &Store) -> Result<bool, String> {
    let Some(session) = store.active_session() else {
        return Ok(false);
    };
    if !crate::screens::dialog::advertises(store, "session/status/read") {
        return Ok(false);
    }
    let revision = context_revision();
    let status = conv
        .client()
        .request(
            "session/status/read",
            json!({ "session_id": session, "profile_id": conv.profile() }),
        )
        .await
        .map_err(|e| format!("session/status/read: {e}"))?;
    Ok(fold_status_read(&status, &session, revision, store))
}

/// The pure half of [`refresh_context`]: fold one `session/status/read` reply
/// that was requested at event revision `revision`.
pub fn fold_status_read(status: &Value, session: &str, revision: u64, store: &Store) -> bool {
    let incoming = status
        .get("context_state")
        .filter(|v| v.is_object())
        .or_else(|| status.get("context").and_then(|c| c.get("state")))
        .cloned()
        .unwrap_or(Value::Null);
    // Bound to the session it was read for (`status.session_id !== sessionId`
    // and `incoming.state.session_id !== sessionId` both drop the read).
    if incoming.get("session_id").and_then(|s| s.as_str()) != Some(session) {
        return false;
    }
    let current = store.domains.session.context(session);
    if revision != context_revision() {
        if let Some(cur) = &current {
            let cur_gen = cur.state.get("generation").and_then(|g| g.as_u64()).unwrap_or(0);
            let new_gen = incoming.get("generation").and_then(|g| g.as_u64()).unwrap_or(0);
            if new_gen <= cur_gen {
                return false;
            }
        }
    }
    let last = status
        .get("context")
        .and_then(|c| c.get("compaction"))
        .and_then(|c| c.get("last"))
        .filter(|l| !l.is_null())
        .cloned();
    store.domains.session.set_context(
        session,
        octoscode_store::domains::session::ContextLifecycle {
            kind: "session/status/read".to_owned(),
            state: incoming,
            detail: last,
        },
    );
    true
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
                // #32d item 6: the OUTER card sizes to its content too. The
                // authored card holds two model rows (inner_card 213..339)
                // with the route buttons at y356; with the rows gone the
                // buttons keep their authored 17px gap and the card ends at
                // their bottom — no blank band above Test route/Discover.
                let delta = 126.0_f64 - 63.5;
                if let Some(h) = placements
                    .get_mut("card_deepseek")
                    .and_then(|c| c.get_mut("layout"))
                    .and_then(|l| l.get_mut("h"))
                {
                    if h.as_f64() == Some(290.0) {
                        *h = json!(290.0 - delta + 12.0); // 12px bottom padding below the buttons
                    }
                }
                let keys: Vec<String> = placements
                    .as_object()
                    .map(|o| o.keys().cloned().collect())
                    .unwrap_or_default();
                for key in keys {
                    if !(key.starts_with("btn_test") || key.starts_with("btn_discover")) {
                        continue;
                    }
                    if let Some(y) = placements
                        .get_mut(&key)
                        .and_then(|c| c.get_mut("layout"))
                        .and_then(|l| l.get_mut("y"))
                    {
                        // The whole button band moves together: the pill
                        // surfaces sit at y356 AND their labels at y369 (+13,
                        // the authored in-button offset). Matching only 356
                        // left the labels behind — empty pills with text
                        // floating outside the card (the first live capture).
                        if y.as_f64().is_some_and(|v| v >= 356.0 && v <= 370.0) {
                            *y = json!(y.as_f64().unwrap() - delta);
                        }
                    }
                }
                // #32d r2 (outer review): the FOLLOWING cards ride the same
                // delta — kimi (y442) and glm (y565) with their heads, counts,
                // dots and chevrons — or the shrunk card leaves a ~200px
                // empty band above Kimi. Same <=370 style guard: heads sit
                // +23 above their card, counts +57 below it (authored
                // in-card offsets around y465/y499 and y587/y623).
                let follow: Vec<String> = placements
                    .as_object()
                    .map(|o| {
                        o.keys()
                            .filter(|k| {
                                let k = k.as_str();
                                ["card_kimi", "t_kimi", "dot_kimi", "icon_chev_kimi",
                                 "card_glm", "t_glm", "dot_glm", "icon_chev_glm"]
                                    .iter()
                                    .any(|p| k.starts_with(p))
                            })
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                let net = delta - 12.0; // the card bottom rose 50.5, not 62.5 —
                // 12px of the shrink came back as bottom padding, so the
                // followers ride 50.5 to keep the authored 42px list gap.
                for key in follow {
                    if let Some(y) = placements
                        .get_mut(&key)
                        .and_then(|c| c.get_mut("layout"))
                        .and_then(|l| l.get_mut("y"))
                    {
                        if let Some(v) = y.as_f64() {
                            *y = json!(v - net);
                        }
                    }
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
