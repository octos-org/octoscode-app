//! A10 — the Profile's configured model providers (web
//! `product-settings/ModelManagementSection.tsx` over
//! `features/models/model-settings.ts`), reached from the Models dialog's
//! "Manage providers": the configured routes (`profile/llm/list
//! {profile_id}` — primary + fallbacks), per route "Fetch available models"
//! (`profile/llm/fetch_models`) whose "Available from endpoint" suggestions
//! pick the Model ID of a new configuration on that route, saved the web's
//! way (ONE request built, `profile/llm/test` then `profile/llm/upsert`,
//! `set_primary:false`), and "Delete" behind the web's typed confirmation
//! (`DELETE <family>/<model>`, `profile/llm/delete`).
//!
//! No Stage-A board draws it: built with the native dialog kit
//! (`board3/ui.rs`), the web component as the reference.
use octoscode_store::Store;
use serde_json::{json, Value};

use super::host::{Job, Outcome};
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

pub const TITLE: &str = "Model providers";
pub const EMPTY: &str = "No model providers configured";
pub const DELETED: &str = "Provider deleted. Restart Octos before relying on the updated runtime policy.";
pub const DELETE_FAILED: &str = "Could not delete this provider. The configuration was kept; try again.";
pub const SAVED: &str = "Provider saved.";
pub const READ_ONLY: &str = "Core did not report a complete route identity. This entry is read-only.";

/// One configured provider route (`ConfiguredModelProvider`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Route {
    pub family_id: String,
    pub model_id: String,
    pub route_id: String,
    pub label: String,
    pub api_type: String,
    pub base_url: String,
    pub api_key_env: String,
    pub has_key: bool,
    pub primary: bool,
}

impl Route {
    fn from(v: &Value, primary: bool) -> Option<Route> {
        let s = |v: &Value| v.as_str().unwrap_or("").to_owned();
        let family_id = s(&v["family_id"]);
        let model_id = s(&v["model_id"]);
        if family_id.is_empty() || model_id.is_empty() {
            return None;
        }
        let r = &v["route"];
        Some(Route {
            family_id,
            model_id,
            route_id: s(&r["route_id"]),
            label: s(&r["label"]),
            api_type: s(&r["api_type"]),
            base_url: s(&r["base_url"]),
            api_key_env: s(&r["api_key_env"]),
            has_key: v["has_api_key"] == json!(true),
            primary,
        })
    }

    /// `mutationSafe` (`model-management-projection.ts:150`): a route id and
    /// an API protocol.
    pub fn removable(&self) -> bool {
        !self.route_id.is_empty() && !self.api_type.is_empty()
    }

    /// `providerName`: "<family> · <model>".
    pub fn name(&self) -> String {
        format!("{} · {}", self.family_id, self.model_id)
    }

    /// `providerDeleteConfirmation`: the exact, case-sensitive phrase.
    pub fn delete_phrase(&self) -> String {
        format!("DELETE {}/{}", self.family_id, self.model_id)
    }

    /// `routeSelection` (`model-settings.ts:556-569`).
    pub fn selection(&self) -> Value {
        let mut r = json!({ "route_id": self.route_id, "api_key_env": self.api_key_env,
                            "api_type": if self.api_type.is_empty() { "openai" } else { &self.api_type } });
        if !self.label.is_empty() {
            r["label"] = json!(self.label);
        }
        if !self.base_url.is_empty() {
            r["base_url"] = json!(self.base_url);
        }
        r
    }
}

/// The configuration a `profile/llm/list {profile_id}` or a delete receipt
/// carries: primary first, then the fallbacks.
pub fn routes_of(v: &Value) -> Vec<Route> {
    let mut out = Vec::new();
    if let Some(p) = v.get("primary").and_then(|p| Route::from(p, true)) {
        out.push(p);
    }
    for f in v.get("fallbacks").and_then(|f| f.as_array()).into_iter().flatten() {
        if let Some(r) = Route::from(f, false) {
            out.push(r);
        }
    }
    out
}

/// `fetchFailureMessage` (`model-settings.ts:603-609`).
pub fn fetch_failure(reason: &str) -> String {
    match reason {
        "no_api_key" => "Add an API key before checking models.".to_owned(),
        "provider_unavailable" => "The provider did not return an available-model catalog.".to_owned(),
        other => format!("Could not check provider models: {}", crate::screens::provider::redact(other, "")),
    }
}

#[derive(Debug, Clone, Default)]
pub struct RoutesState {
    pub routes: Vec<Route>,
    pub loaded: bool,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    /// The route whose "Add model provider" editor is open (index), with the
    /// endpoint's suggestions and the Model ID draft (live + mount snapshot).
    pub editing: Option<usize>,
    pub fetched: Vec<String>,
    pub model: String,
    pub model_snap: String,
    /// The route awaiting its typed delete confirmation, and the phrase.
    pub deleting: Option<usize>,
    pub phrase: String,
    pub delete_failed: bool,
    pub generation: u64,
    pub form_gen: u64,
}

fn has(store: &Store, m: &str) -> bool {
    store.domains.config.supported_methods().iter().any(|x| x == m)
}

pub fn on_open(st: &mut RoutesState) -> Outcome {
    let (generation, form_gen) = (st.generation + 1, st.form_gen + 1);
    *st = RoutesState { generation, form_gen, ..Default::default() };
    Outcome::Spawn(Job::RoutesLoad(generation))
}

pub fn perform(st: &mut RoutesState, action: &str, index: usize, store: &Store) -> Outcome {
    st.model_snap = st.model.clone();
    if st.busy && action != "b3.routes.cancel" {
        return Outcome::Done;
    }
    match action {
        "b3.routes.refresh" => Outcome::Spawn(Job::RoutesLoad(st.generation)),
        "b3.routes.add" if has(store, "profile/llm/test") && has(store, "profile/llm/upsert") => {
            if index < st.routes.len() {
                st.editing = Some(index);
                st.fetched.clear();
                st.model.clear();
                st.model_snap.clear();
                st.deleting = None;
                st.error = None;
                st.form_gen += 1;
            }
            Outcome::Done
        }
        "b3.routes.fetch" if has(store, "profile/llm/fetch_models") => match st.editing {
            Some(i) => Outcome::Spawn(Job::RoutesFetch(st.generation, i)),
            None => Outcome::Done,
        },
        "b3.routes.pick" => {
            if let Some(m) = st.fetched.get(index).cloned() {
                st.model = m.clone();
                st.model_snap = m;
                st.form_gen += 1;
            }
            Outcome::Done
        }
        "b3.routes.save" => match st.editing {
            Some(i) if !st.model.trim().is_empty() => Outcome::Spawn(Job::RoutesSave(st.generation, i, st.model.trim().to_owned())),
            _ => Outcome::Done,
        },
        "b3.routes.delete" if has(store, "profile/llm/delete") => {
            if st.routes.get(index).is_some_and(Route::removable) {
                st.deleting = Some(index);
                st.phrase.clear();
                st.delete_failed = false;
                st.editing = None;
                st.notice = None;
                st.form_gen += 1;
            }
            Outcome::Done
        }
        "b3.routes.confirm_delete" => match st.deleting {
            Some(i) if st.routes.get(i).is_some_and(|r| st.phrase == r.delete_phrase()) => {
                Outcome::Spawn(Job::RoutesDelete(st.generation, i))
            }
            _ => Outcome::Done,
        },
        "b3.routes.cancel" => {
            if !st.busy {
                st.editing = None;
                st.deleting = None;
                st.phrase.clear();
                st.form_gen += 1;
            }
            Outcome::Done
        }
        a if a.starts_with("b3.routes.") => Outcome::Done,
        _ => Outcome::Unrouted,
    }
}

pub fn input_changed(st: &mut RoutesState, key: &str, text: &str) {
    match key {
        "routes.model" => st.model = text.to_owned(),
        "routes.phrase" => st.phrase = text.to_owned(),
        _ => {}
    }
}

/// Live gates (no remount while typing): Save needs a Model ID; "Delete
/// provider" is armed only by the exact phrase.
pub fn visibility(st: &RoutesState) -> Vec<(String, bool)> {
    let save = !st.model.trim().is_empty();
    let matches = st.deleting.and_then(|i| st.routes.get(i)).is_some_and(|r| st.phrase == r.delete_phrase());
    vec![
        ("b3_routes_save_on".into(), save),
        ("b3_routes_save_off".into(), !save),
        ("b3_routes_delete_on".into(), matches),
        ("b3_routes_delete_off".into(), !matches),
    ]
}

// ------------------------------------------------------------- transport

fn with<R>(f: impl FnOnce(&mut RoutesState) -> R) -> R {
    f(&mut super::host::state().routes)
}

fn begin(generation: u64) -> bool {
    with(|st| {
        if st.busy || st.generation != generation {
            return false;
        }
        st.busy = true;
        st.error = None;
        true
    })
}

fn current(generation: u64) -> bool {
    with(|st| st.generation == generation)
}

fn profile(conv: &crate::flow::Conversation) -> String {
    conv.store.domains.profile.current().unwrap_or_else(|| conv.profile())
}

/// The read (`readProfileLlmConfig`): the Profile echoed.
pub async fn load(conv: &crate::flow::Conversation, generation: u64) -> Result<String, String> {
    if !begin(generation) {
        return Ok("refused".into());
    }
    let p = profile(conv);
    let r = conv.client().request("profile/llm/list", json!({ "profile_id": p })).await;
    if !current(generation) {
        return Ok("superseded".into());
    }
    let out = match r {
        Ok(v) if v["profile_id"] != json!(p) => Err("The model settings response belongs to another profile.".to_owned()),
        Ok(v) => Ok(routes_of(&v)),
        Err(e) => Err(crate::screens::dialog::display_error(&e.to_string())),
    };
    with(|st| {
        st.busy = false;
        match out {
            Ok(routes) => {
                let n = routes.len();
                st.routes = routes;
                st.loaded = true;
                Ok(format!("{n} provider(s)"))
            }
            Err(e) => {
                st.error = Some(e.clone());
                Err(e)
            }
        }
    })
}

/// `fetchModels` (`model-settings.ts:286-330`): `{profile_id, selection:
/// {family_id, route}}`; the family echoed; an empty list with a reason is
/// the web's failure line.
pub async fn fetch(conv: &crate::flow::Conversation, generation: u64, index: usize) -> Result<String, String> {
    let Some(route) = with(|st| st.routes.get(index).cloned()) else { return Ok("no route".into()) };
    if !begin(generation) {
        return Ok("refused".into());
    }
    let p = profile(conv);
    let params = json!({ "profile_id": p, "selection": { "family_id": route.family_id, "route": route.selection() } });
    let r = conv.client().request("profile/llm/fetch_models", params).await;
    if !current(generation) {
        return Ok("superseded".into());
    }
    let out = match r {
        Ok(v) if v["profile_id"] != json!(p) => Err("The model settings response belongs to another profile.".to_owned()),
        Ok(v) if v["family_id"] != json!(route.family_id) => Err("profile/llm/fetch_models returned another family".to_owned()),
        Ok(v) => {
            let models: Vec<String> = v["models"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(str::to_owned)).collect();
            let reason = v["reason"].as_str().map(str::to_owned);
            Ok((models, reason))
        }
        Err(e) => Err(crate::screens::dialog::display_error(&e.to_string())),
    };
    with(|st| {
        st.busy = false;
        match out {
            Ok((models, reason)) => {
                let n = models.len();
                if models.is_empty() {
                    st.error = reason.as_deref().map(fetch_failure);
                }
                st.fetched = models;
                Ok(format!("{n} model(s)"))
            }
            Err(e) => {
                st.error = Some(e.clone());
                Err(e)
            }
        }
    })
}

/// `save` (`model-settings.ts:345-395`): the provision built ONCE —
/// `profile/llm/test`, then (only on a passing test) `profile/llm/upsert`
/// with `set_primary:false`; then the configuration re-read.
pub async fn save(conv: &crate::flow::Conversation, generation: u64, index: usize, model: String) -> Result<String, String> {
    let Some(route) = with(|st| st.routes.get(index).cloned()) else { return Ok("no route".into()) };
    if !begin(generation) {
        return Ok("refused".into());
    }
    let p = profile(conv);
    let provision = json!({ "profile_id": p, "selection": { "family_id": route.family_id, "model_id": model, "route": route.selection() } });
    let result: Result<(), String> = async {
        let t = conv.client().request("profile/llm/test", provision.clone()).await.map_err(|e| crate::screens::dialog::display_error(&e.to_string()))?;
        if t["profile_id"] != json!(p) {
            return Err("The model settings response belongs to another profile.".to_owned());
        }
        if t["applied"] != json!(true) || t["error"].as_str().is_some_and(|e| !e.is_empty()) {
            let why = t["error"].as_str().filter(|e| !e.is_empty()).or_else(|| t["message"].as_str().filter(|m| !m.is_empty()));
            // `redactModelSettingsError`: provider prose can echo inputs.
            return Err(crate::screens::provider::redact(why.unwrap_or("The provider test did not pass."), ""));
        }
        let mut upsert = provision.clone();
        upsert["set_primary"] = json!(false);
        let u = conv.client().request("profile/llm/upsert", upsert).await.map_err(|e| crate::screens::dialog::display_error(&e.to_string()))?;
        if u["profile_id"] != json!(p) {
            return Err("The model settings response belongs to another profile.".to_owned());
        }
        if u["applied"] != json!(true) {
            return Err("The server did not save the tested model configuration.".to_owned());
        }
        Ok(())
    }
    .await;
    if !current(generation) {
        return Ok("superseded".into());
    }
    match result {
        Ok(()) => {
            with(|st| {
                st.busy = false;
                st.editing = None;
                st.fetched.clear();
                st.model.clear();
                st.model_snap.clear();
                st.notice = Some(SAVED.to_owned());
                st.form_gen += 1;
            });
            let _ = load(conv, generation).await;
            Ok(SAVED.into())
        }
        Err(e) => {
            with(|st| {
                st.busy = false;
                st.error = Some(e.clone());
            });
            Err(e)
        }
    }
}

/// `delete` (`model-settings.ts:399-428`): `{profile_id, family_id,
/// model_id, route_id}`; the receipt's configuration replaces the list.
pub async fn delete(conv: &crate::flow::Conversation, generation: u64, index: usize) -> Result<String, String> {
    let Some(route) = with(|st| st.routes.get(index).cloned()) else { return Ok("no route".into()) };
    if !begin(generation) {
        return Ok("refused".into());
    }
    let p = profile(conv);
    let params = json!({ "profile_id": p, "family_id": route.family_id, "model_id": route.model_id, "route_id": route.route_id });
    let r = conv.client().request("profile/llm/delete", params).await;
    if !current(generation) {
        return Ok("superseded".into());
    }
    let out = match r {
        Ok(v) if v["profile_id"] == json!(p) => Ok(routes_of(&v)),
        Ok(_) => Err("The model settings response belongs to another profile.".to_owned()),
        Err(e) => Err(e.to_string()),
    };
    with(|st| {
        st.busy = false;
        match out {
            Ok(routes) => {
                st.routes = routes;
                st.deleting = None;
                st.phrase.clear();
                st.notice = Some(DELETED.to_owned());
                st.form_gen += 1;
                Ok(DELETED.into())
            }
            Err(e) => {
                st.delete_failed = true;
                Err(e)
            }
        }
    })
}

// ------------------------------------------------------------------ view

fn pill_w(label: &str) -> f64 {
    ui::text_w(label, 13.0, Face::Medium) + 32.0
}

fn route_card(d: &mut Dsl, st: &RoutesState, i: usize, r: &Route, inner_w: f64, store: &Store) {
    let id = format!("b3_routes_row_{i}");
    ui::card_open(d, &id, 4.0);
    let head = d.anon();
    d.view(&head, "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 0.0 y: 0.5}");
    d.text(&format!("{id}_name"), &ui::fit_w(&r.name(), inner_w - 90.0, 13.5, Face::Semibold), &Txt::new(13.5, Face::Semibold, tok::TEXT).w(W::Fill));
    if r.primary {
        d.chip(&format!("{id}_primary"), "Primary", tok::BLUE, tok::BLUE_BG, None, false);
    }
    d.close();
    let label = if r.label.is_empty() { r.route_id.clone() } else { r.label.clone() };
    d.text(&format!("{id}_meta"), &ui::fit_w(&format!("{} · {label}", r.family_id), inner_w, 12.5, Face::Regular), &ui::meta().w(W::Fill));
    let mut endpoint = r.model_id.clone();
    if !r.api_type.is_empty() {
        endpoint.push_str(&format!(" · {}", r.api_type));
    }
    d.text(&format!("{id}_endpoint"), &ui::fit_w(&endpoint, inner_w, 12.0, Face::Mono), &Txt::new(12.0, Face::Mono, tok::MUTED).w(W::Fill));
    // The base URL on its own line, never cut: it tells two routes of one
    // model apart.
    if !r.base_url.is_empty() {
        d.text(&format!("{id}_base_url"), &r.base_url, &Txt::new(12.0, Face::Mono, tok::MUTED).w(W::Fill).wrap());
    }
    if !r.removable() {
        d.text(&format!("{id}_readonly"), READ_ONLY, &Txt::new(12.0, Face::Regular, tok::AMBER).w(W::Fill).wrap());
    } else {
        let acts = d.anon();
        // One row when both fit, else a column (wrapped pill rows touch).
        let fits = pill_w("Add a model on this route") + 8.0 + pill_w("Delete") <= inner_w;
        d.view(&acts, if fits {
            "width: Fill height: Fit flow: Right spacing: 8 padding: Inset{top: 4}"
        } else {
            "width: Fill height: Fit flow: Down spacing: 8 padding: Inset{top: 4}"
        });
        let idle = !st.busy && st.editing.is_none() && st.deleting.is_none();
        if has(store, "profile/llm/test") && has(store, "profile/llm/upsert") {
            let kind = if idle { Btn::Outline } else { Btn::OutlineOff };
            d.button(&format!("{id}_add"), "Add a model on this route", &format!("b3.routes.add#{i}"), kind, W::Fit, 32.0);
        }
        if has(store, "profile/llm/delete") {
            let kind = if idle { Btn::Outline } else { Btn::OutlineOff };
            d.button(&format!("{id}_delete"), "Delete", &format!("b3.routes.delete#{i}"), kind, W::Fit, 32.0);
        }
        d.close();
    }
    d.close();
}

/// The "Add model provider" editor for one route: Model ID, "Fetch available
/// models" and its "Available from endpoint" suggestions, Cancel / Save.
fn editor(d: &mut Dsl, st: &RoutesState, r: &Route, store: &Store, compact: bool, inner_w: f64) {
    d.surface(
        &format!("b3_routes_editor_{}", st.form_gen),
        "width: Fill height: Fit flow: Down spacing: 10 padding: Inset{left: 14 right: 14 top: 12 bottom: 14}",
        tok::SURFACE,
        12.0,
        Some(tok::HAIRLINE),
    );
    d.text("b3_routes_editor_title", "Add model provider", &ui::heading().w(W::Fill));
    d.text(
        "b3_routes_editor_route",
        &format!("{} · {} ({})", r.family_id, if r.label.is_empty() { &r.route_id } else { &r.label }, r.route_id),
        &ui::meta().w(W::Fill),
    );
    ui::field_label(d, "b3_routes_model_label", "Model ID");
    d.input("b3_routes_model", "routes.model", &st.model_snap, "", true, 36.0);
    if has(store, "profile/llm/fetch_models") {
        let kind = if st.busy { Btn::OutlineOff } else { Btn::Outline };
        let label = if st.busy { "Fetching…" } else { "Fetch available models" };
        d.button("b3_routes_fetch", label, "b3.routes.fetch", kind, W::Fit, 34.0);
    }
    if !st.fetched.is_empty() {
        ui::field_label(d, "b3_routes_fetched_label", "Available from endpoint");
        let chips = d.anon();
        let total: f64 = st.fetched.iter().map(|m| pill_w(m) + 8.0).sum();
        d.view(&chips, if total <= inner_w + 8.0 {
            "width: Fill height: Fit flow: Right spacing: 8"
        } else {
            "width: Fill height: Fit flow: Down spacing: 8"
        });
        for (j, m) in st.fetched.iter().enumerate() {
            let chosen = st.model_snap == *m;
            let kind = if chosen { Btn::Primary } else { Btn::Secondary };
            d.button(&format!("b3_routes_pick_{j}"), m, &format!("b3.routes.pick#{j}"), kind, W::Fit, 30.0);
        }
        d.close();
    }
    let foot = d.anon();
    d.view(&foot, &format!("width: Fill height: Fit flow: {} align: Align{{x: 1.0 y: 0.5}} spacing: 8", if compact { "Down" } else { "Right" }));
    let w = if compact { W::Fill } else { W::Fit };
    d.button("b3_routes_cancel", "Cancel", "b3.routes.cancel", if st.busy { Btn::OutlineOff } else { Btn::Outline }, w, 34.0);
    let both = d.anon();
    d.view(&both, &format!("width: {} height: 34 flow: Overlay", if compact { "Fill".to_owned() } else { format!("{}", (ui::text_w("Save", 13.0, Face::Medium) + 32.0).ceil()) }));
    d.view("b3_routes_save_off", "width: Fill height: Fit flow: Right");
    d.button("b3_routes_save_disabled", "Save", "b3.routes.save", Btn::Disabled, W::Fill, 34.0);
    d.close();
    d.view("b3_routes_save_on", "width: Fill height: Fit flow: Right");
    let label = if st.busy { "Saving…" } else { "Save" };
    d.button("b3_routes_save", label, "b3.routes.save", if st.busy { Btn::Disabled } else { Btn::Primary }, W::Fill, 34.0);
    d.close();
    d.close();
    d.close();
    d.close();
}

/// `DeleteProviderDialog`: the typed phrase arms "Delete provider".
fn delete_card(d: &mut Dsl, st: &RoutesState, r: &Route, compact: bool) {
    d.surface(
        &format!("b3_routes_delete_card_{}", st.form_gen),
        "width: Fill height: Fit flow: Down spacing: 8 padding: Inset{left: 14 right: 14 top: 12 bottom: 14}",
        tok::RED_BG,
        12.0,
        Some("#f3c4c7ff"),
    );
    d.text("b3_routes_delete_title", "Delete model provider?", &ui::heading().w(W::Fill));
    d.text(
        "b3_routes_delete_body",
        &format!("This removes the configured route for {}. Existing sessions may still refer to it.", r.name()),
        &ui::meta().w(W::Fill).wrap(),
    );
    d.text("b3_routes_delete_prompt", &format!("Type {} to confirm", r.delete_phrase()), &Txt::new(12.5, Face::Medium, tok::TEXT).w(W::Fill).wrap());
    d.input("b3_routes_phrase", "routes.phrase", "", "", true, 36.0);
    if st.delete_failed {
        d.text("b3_routes_delete_failed", DELETE_FAILED, &Txt::new(12.5, Face::Regular, tok::RED).w(W::Fill).wrap());
    }
    let foot = d.anon();
    d.view(&foot, &format!("width: Fill height: Fit flow: {} align: Align{{x: 1.0 y: 0.5}} spacing: 8", if compact { "Down" } else { "Right" }));
    let w = if compact { W::Fill } else { W::Fit };
    d.button("b3_routes_delete_cancel", "Cancel", "b3.routes.cancel", if st.busy { Btn::OutlineOff } else { Btn::Outline }, w, 34.0);
    let both = d.anon();
    let label = if st.busy { "Deleting…" } else { "Delete provider" };
    d.view(&both, &format!("width: {} height: 34 flow: Overlay", if compact { "Fill".to_owned() } else { format!("{}", (ui::text_w("Delete provider", 13.0, Face::Medium) + 32.0).ceil()) }));
    d.view("b3_routes_delete_off", "width: Fill height: Fit flow: Right");
    d.button("b3_routes_delete_disabled", label, "b3.routes.confirm_delete", Btn::Disabled, W::Fill, 34.0);
    d.close();
    d.view("b3_routes_delete_on", "width: Fill height: Fit flow: Right");
    danger(d, "b3_routes_delete_go", label, "b3.routes.confirm_delete", !st.busy);
    d.close();
    d.close();
    d.close();
    d.close();
}

fn danger(d: &mut Dsl, id: &str, label: &str, event: &str, armed: bool) {
    let (fill, ink) = if armed { (tok::RED, tok::WHITE) } else { (tok::DISABLED_BG, tok::FAINT) };
    d.surface(&format!("{id}_box"), "width: Fill height: 34 flow: Overlay align: Align{x: 0.5 y: 0.5}", fill, 17.0, None);
    d.text(&format!("{id}_label"), label, &Txt::new(13.0, Face::Medium, ink));
    if armed {
        d.tap(id, event);
    }
    d.close();
}

pub fn build(d: &mut Dsl, st: &RoutesState, frame: &Frame, store: &Store) {
    let width = frame.dialog_w(640.0);
    let compact = frame.compact(width);
    let pad = ui::dialog_pad(frame, width);
    let inner_w = width - 2.0 * pad - 10.0 - 28.0;
    ui::shell_open(d, frame, width);
    let row = d.anon();
    d.view(&row, "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 4");
    d.text("b3_title", TITLE, &ui::title().w(W::Fill));
    ui::icon_button(d, "b3_routes_refresh", "b3_refresh.svg", 16.0, "b3.routes.refresh");
    ui::close_glyph(d, "b3.close");
    d.close();
    let profile = store.domains.profile.current().unwrap_or_default();
    d.text("b3_routes_scope", &format!("Server Profile: {profile}"), &ui::micro().w(W::Fill));
    d.gap(W::Fill, 10.0);
    ui::body_open(d, frame, width, 60.0);
    if st.busy && !st.loaded {
        d.text("b3_routes_loading", "Loading model providers…", &ui::meta());
    }
    if let Some(e) = &st.error {
        d.text("b3_routes_error", e, &Txt::new(12.5, Face::Regular, tok::RED).w(W::Fill).wrap());
    }
    if let Some(n) = &st.notice {
        d.text("b3_routes_notice", n, &Txt::new(12.5, Face::Regular, tok::GREEN).w(W::Fill).wrap());
    }
    if let Some(r) = st.deleting.and_then(|i| st.routes.get(i)) {
        delete_card(d, st, r, compact);
        d.gap(W::Fill, 4.0);
    }
    if let Some(r) = st.editing.and_then(|i| st.routes.get(i)) {
        editor(d, st, r, store, compact, inner_w);
        d.gap(W::Fill, 4.0);
    }
    if st.loaded && st.routes.is_empty() {
        d.text("b3_routes_empty", EMPTY, &ui::body_medium().w(W::Fill));
        d.text("b3_routes_empty_hint", "Add a provider route to make a model available to Core.", &ui::meta().w(W::Fill).wrap());
    }
    for (i, r) in st.routes.iter().enumerate() {
        route_card(d, st, i, r, inner_w, store);
        d.gap(W::Fill, 4.0);
    }
    ui::body_close(d);
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r2_config() -> Value {
        json!({"profile_id": "dsflash",
            "primary": {"family_id": "deepseek", "model_id": "deepseek-v4-flash", "has_api_key": true, "selected": true,
                        "route": {"api_type": "openai", "label": "Official API", "route_id": "deepseek", "api_key_env": "DEEPSEEK_API_KEY"}},
            "fallbacks": [{"family_id": "deepseek", "model_id": "deepseek-v4-flash", "has_api_key": true,
                           "route": {"api_type": "openai", "base_url": "http://127.0.0.1:9/v1", "route_id": "r2-route"}}]})
    }

    #[test]
    fn routes_and_the_typed_phrase_are_the_webs() {
        let routes = routes_of(&r2_config());
        assert_eq!(routes.len(), 2);
        assert!(routes[0].primary && !routes[1].primary);
        assert_eq!(routes[1].delete_phrase(), "DELETE deepseek/deepseek-v4-flash");
        assert!(routes[1].removable());
        assert_eq!(
            routes[1].selection(),
            json!({"route_id": "r2-route", "api_key_env": "", "api_type": "openai", "base_url": "http://127.0.0.1:9/v1"})
        );
        assert_eq!(fetch_failure("no_api_key"), "Add an API key before checking models.");
        let mut st = RoutesState { routes, loaded: true, ..Default::default() };
        let store = Store::new();
        store.domains.config.set_supported_methods(
            ["profile/llm/list", "profile/llm/delete", "profile/llm/test", "profile/llm/upsert", "profile/llm/fetch_models"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        // Delete asks for the exact phrase.
        assert_eq!(perform(&mut st, "b3.routes.delete", 1, &store), Outcome::Done);
        assert_eq!(perform(&mut st, "b3.routes.confirm_delete", 0, &store), Outcome::Done, "no phrase yet");
        input_changed(&mut st, "routes.phrase", "delete deepseek/deepseek-v4-flash");
        assert_eq!(perform(&mut st, "b3.routes.confirm_delete", 0, &store), Outcome::Done, "case-sensitive");
        input_changed(&mut st, "routes.phrase", "DELETE deepseek/deepseek-v4-flash");
        assert!(visibility(&st).contains(&("b3_routes_delete_on".into(), true)));
        assert_eq!(perform(&mut st, "b3.routes.confirm_delete", 0, &store), Outcome::Spawn(Job::RoutesDelete(0, 1)));
        // Add: fetch, pick, save.
        perform(&mut st, "b3.routes.cancel", 0, &store);
        perform(&mut st, "b3.routes.add", 0, &store);
        assert_eq!(perform(&mut st, "b3.routes.fetch", 0, &store), Outcome::Spawn(Job::RoutesFetch(0, 0)));
        st.fetched = vec!["deepseek-v4-pro".into(), "deepseek-v4-flash".into()];
        perform(&mut st, "b3.routes.pick", 0, &store);
        assert_eq!(st.model, "deepseek-v4-pro");
        assert_eq!(perform(&mut st, "b3.routes.save", 0, &store), Outcome::Spawn(Job::RoutesSave(0, 0, "deepseek-v4-pro".into())));
        for frame in [Frame::DESKTOP, Frame { avail_w: 360.0, avail_h: 776.0 }] {
            let mut d = Dsl::new();
            build(&mut d, &st, &frame, &store);
            let dsl = d.finish();
            assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
            assert!(dsl.contains("Available from endpoint") && dsl.contains("Add model provider"));
        }
    }
}
