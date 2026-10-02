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
//! A23 — the web section's remaining surface, through the shared controller
//! ([`crate::screens::model_settings`]): the configuration AND the catalog
//! read side by side; the projection's states (unavailable / loading / an
//! unread configuration as an error with "Try again", never an empty list /
//! ready); per row the credential indicator, "Edit" on ANY editable row and
//! the read-only reason of a row the editor cannot preserve (still
//! deletable); the catalog-driven "Add provider"; the read-only notice and
//! the runtime warning. "Edit" / "Add provider" open the board-1 editor
//! (A2's p4-06/07, `screens::provider`) in this dialog's place; Back and Save
//! return here (`host::reopen_routes`), re-read, with the web's success line.
//!
//! Drawn with the native dialog kit (`board3/ui.rs`); the element each
//! surface reuses is listed in `screens::model_settings`.
use octoscode_store::Store;
use serde_json::{json, Value};

use super::host::{Job, Outcome};
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};
use crate::screens::model_settings::{self as ms, copy, Config, ConfiguredModel, ViewState};
use crate::screens::provider;

pub const TITLE: &str = copy::TITLE;
pub const EMPTY: &str = copy::EMPTY;
pub const DELETED: &str = copy::DELETED;
pub const DELETE_FAILED: &str = copy::DELETE_FAILED;
pub const SAVED: &str = copy::SAVED;
pub const READ_ONLY: &str = copy::INCOMPLETE_IDENTITY;

/// The board-1 opener the dialog routes when Edit / Add provider seeded the
/// editor (`board1::OPENERS`).
pub const OPEN_EDITOR: &str = "b1.open.provider.routes";

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
    /// From one parsed configured row.
    pub fn from_configured(m: &ConfiguredModel) -> Route {
        Route {
            family_id: m.family_id.clone(),
            model_id: m.model_id.clone(),
            route_id: m.route.route_id.clone(),
            label: m.route.label.clone(),
            api_type: m.route.api_type.clone(),
            base_url: m.route.base_url.clone(),
            api_key_env: m.route.api_key_env.clone(),
            has_key: m.has_api_key,
            primary: m.selected,
        }
    }

    /// `mutationSafe` (`model-management-projection.ts:150`): a route id and
    /// an API protocol.
    pub fn removable(&self) -> bool {
        !self.route_id.is_empty() && !self.api_type.is_empty()
    }

    /// "<family> · <model>".
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
/// carries: primary first, then the fallbacks — parsed the web's way (a
/// malformed configuration lists nothing).
pub fn routes_of(v: &Value) -> Vec<Route> {
    ms::parse_config(v).map(|c| c.rows().into_iter().map(Route::from_configured).collect()).unwrap_or_default()
}

/// `fetchFailureMessage` (`model-settings.ts:603-609`).
pub fn fetch_failure(reason: &str) -> String {
    copy::fetch_failure(reason)
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
    /// A23 — the authoritative configuration (`None` = unread: never shown
    /// as an empty list).
    pub config: Option<Config>,
    /// A23 — the provider catalog (the Add form's families).
    pub catalog: Option<octoscode_client::domains::profile::LlmCatalogResult>,
}

impl RoutesState {
    fn set_config(&mut self, c: Config) {
        self.routes = c.rows().into_iter().map(Route::from_configured).collect();
        self.config = Some(c);
        self.loaded = true;
    }

    /// The web's projection of this state (`projectModelManagement`). A
    /// first read not yet started counts as loading, never as a failure.
    pub fn projection(&self, caps: ms::Caps) -> ms::Projection {
        let loading = self.config.is_none() && (self.busy || (!self.loaded && self.error.is_none()));
        ms::project(caps, loading, self.config.as_ref(), self.catalog.as_ref(), self.error.as_deref())
    }
}

fn has(store: &Store, m: &str) -> bool {
    store.domains.config.supported_methods().iter().any(|x| x == m)
}

fn caps_of(store: &Store) -> ms::Caps {
    ms::Caps::from_methods(&store.domains.config.supported_methods())
}

pub fn on_open(st: &mut RoutesState) -> Outcome {
    let (generation, form_gen) = (st.generation + 1, st.form_gen + 1);
    *st = RoutesState { generation, form_gen, ..Default::default() };
    Outcome::Spawn(Job::RoutesLoad(generation))
}

/// A23 — back from the board-1 editor: the same dialog, re-read (the list it
/// showed stays until the reply lands), with the editor's outcome line.
pub fn on_reopen(st: &mut RoutesState, notice: Option<String>) -> Outcome {
    st.generation += 1;
    st.form_gen += 1;
    st.busy = false;
    st.editing = None;
    st.deleting = None;
    st.error = None;
    st.notice = notice;
    Outcome::Spawn(Job::RoutesLoad(st.generation))
}

/// The editor's opening facts (`ModelManagementSettings` props).
fn seed(st: &RoutesState, store: &Store, proj: &ms::Projection) -> provider::Seed {
    let methods = store.domains.config.supported_methods();
    provider::Seed {
        profile_id: st
            .config
            .as_ref()
            .map(|c| c.profile_id.clone())
            .or_else(|| store.domains.profile.current()),
        caps: provider::Caps::from_methods(&methods),
        can_fetch: methods.iter().any(|m| m == "profile/llm/fetch_models"),
        families: proj.families.clone(),
        protocols: proj.protocols.clone(),
        configured: proj.providers.clone(),
        origin: provider::Origin::Routes,
    }
}

pub fn perform(st: &mut RoutesState, action: &str, index: usize, store: &Store) -> Outcome {
    st.model_snap = st.model.clone();
    if st.busy && action != "b3.routes.cancel" {
        return Outcome::Done;
    }
    let caps = caps_of(store);
    match action {
        "b3.routes.refresh" | "b3.routes.retry" => Outcome::Spawn(Job::RoutesLoad(st.generation)),
        // A23 — `openEdit`: the board-1 editor for ANY editable row.
        "b3.routes.edit" if caps.can_save() => {
            let proj = st.projection(caps);
            match proj.providers.get(index) {
                Some(p) if p.editable && proj.state == ViewState::Ready && st.editing.is_none() && st.deleting.is_none() => {
                    provider::set(provider::for_row(p, &seed(st, store, &proj)));
                    st.notice = None;
                    Outcome::Action(OPEN_EDITOR.into())
                }
                _ => Outcome::Done,
            }
        }
        // A23 — `openAdd`: the catalog-driven new provider.
        "b3.routes.add_provider" if caps.can_save() => {
            let proj = st.projection(caps);
            if proj.state != ViewState::Ready || st.editing.is_some() || st.deleting.is_some() {
                return Outcome::Done;
            }
            let empty = st.config.as_ref().is_some_and(Config::is_empty);
            provider::set(provider::for_add(&seed(st, store, &proj), empty));
            st.notice = None;
            Outcome::Action(OPEN_EDITOR.into())
        }
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

/// The read (`refresh`, `model-settings.ts:211-257`): the configuration and
/// the catalog side by side, each on its own advertised method; the
/// configuration must be the Profile's own and well formed. A failed read
/// keeps what was known (an unread configuration stays unread).
pub async fn load(conv: &crate::flow::Conversation, generation: u64) -> Result<String, String> {
    if !begin(generation) {
        return Ok("refused".into());
    }
    let p = profile(conv);
    let caps = ms::Caps::from_methods(&conv.store.domains.config.supported_methods());
    let (config, catalog) = ms::read(conv.client(), &p, caps).await;
    if !current(generation) {
        return Ok("superseded".into());
    }
    with(|st| {
        st.busy = false;
        if let Some(Ok(c)) = catalog {
            st.catalog = Some(c);
        }
        match config {
            Some(Ok(c)) => {
                let n = c.rows().len();
                st.set_config(c);
                Ok(format!("{n} provider(s)"))
            }
            Some(Err(e)) => {
                st.error = Some(e.clone());
                Err(e)
            }
            None => Ok("profile/llm/list is not advertised".into()),
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
        Ok(v) if v["profile_id"] != json!(p) => Err(copy::OTHER_PROFILE.to_owned()),
        Ok(v) if v["family_id"] != json!(route.family_id) => Err(copy::OTHER_FAMILY.to_owned()),
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
            return Err(copy::OTHER_PROFILE.to_owned());
        }
        if t["applied"] != json!(true) || t["error"].as_str().is_some_and(|e| !e.is_empty()) {
            let why = t["error"].as_str().filter(|e| !e.is_empty()).or_else(|| t["message"].as_str().filter(|m| !m.is_empty()));
            // `redactModelSettingsError`: provider prose can echo inputs.
            return Err(ms::redact(why.unwrap_or(copy::TEST_DID_NOT_PASS), ""));
        }
        let mut upsert = provision.clone();
        upsert["set_primary"] = json!(false);
        let u = conv.client().request("profile/llm/upsert", upsert).await.map_err(|e| crate::screens::dialog::display_error(&e.to_string()))?;
        if u["profile_id"] != json!(p) {
            return Err(copy::OTHER_PROFILE.to_owned());
        }
        if u["applied"] != json!(true) {
            return Err(copy::NOT_SAVED.to_owned());
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
    let target = ms::Draft {
        family_id: route.family_id.clone(),
        model_id: route.model_id.clone(),
        route: ms::RouteDraft { route_id: route.route_id.clone(), ..Default::default() },
        ..Default::default()
    };
    let r = ms::delete(conv.client(), &p, &target).await;
    if !current(generation) {
        return Ok("superseded".into());
    }
    with(|st| {
        st.busy = false;
        match r {
            Ok((config, applied)) if applied => {
                st.set_config(config);
                st.deleting = None;
                st.phrase.clear();
                st.notice = Some(DELETED.to_owned());
                st.form_gen += 1;
                Ok(DELETED.into())
            }
            Ok(_) => {
                st.delete_failed = true;
                Err("profile/llm/delete: not applied".into())
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

/// A neutral note with the info glyph (board 1 #4's callout in the dialog
/// kit): the read-only notice, the unavailable state.
fn note(d: &mut Dsl, id: &str, text: &str) {
    d.surface(
        id,
        "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.0} spacing: 8 padding: Inset{left: 12 right: 12 top: 10 bottom: 10}",
        tok::SURFACE2,
        10.0,
        Some(tok::HAIRLINE),
    );
    d.icon(&format!("{id}_icon"), "b3_info.svg", 15.0, tok::MUTED);
    d.text(&format!("{id}_text"), text, &Txt::new(12.5, Face::Regular, tok::TEXT).w(W::Fill).wrap());
    d.close();
}

/// A status line: a round light and its sentence (board 3 #1's "● connected").
/// The light sits on the FIRST line's centre (a 12.5 px line box is 15 px
/// tall), so a sentence that wraps on a phone keeps it beside its start.
fn status_line(d: &mut Dsl, id: &str, text: &str, ok: bool) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.0} spacing: 7");
    let light = d.anon();
    d.view(&light, "width: Fit height: Fit padding: Inset{top: 3.5}");
    d.dot(if ok { tok::GREEN } else { tok::RED }, 8.0);
    d.close();
    d.text(id, text, &Txt::new(12.5, Face::Regular, if ok { tok::GREEN_TEXT } else { tok::RED_TEXT }).w(W::Fill).wrap());
    d.close();
}

/// `stateNotice` (`ModelManagementSection.module.css:150-166`): the loading
/// line in the web's quiet box — the tip fill and hairline of the read-only
/// note (board 1 #4's neutral callout), without its glyph, as on the web.
fn state_notice(d: &mut Dsl, id: &str, text: &str) {
    d.surface(
        &format!("{id}_box"),
        "width: Fill height: Fit flow: Right padding: Inset{left: 13 right: 13 top: 11 bottom: 11}",
        tok::SURFACE2,
        10.0,
        Some(tok::HAIRLINE),
    );
    d.text(id, text, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.close();
}

/// `stateError` (`ModelManagementSection.tsx:1203-1217`, `.module.css:168-175`
/// and `:683-686`): an UNREAD configuration's cause in board 1 #3's light red
/// callout, "Try again" beside it (under it on a phone).
fn state_error(d: &mut Dsl, st: &RoutesState, msg: &str, compact: bool) {
    d.surface(
        "b3_routes_error_box",
        &format!(
            "width: Fill height: Fit flow: {} align: Align{{x: 0.0 y: 0.5}} spacing: {} padding: Inset{{left: 13 right: 13 top: 11 bottom: 11}}",
            if compact { "Down" } else { "Right" },
            if compact { 10 } else { 12 }
        ),
        tok::RED_BG,
        10.0,
        Some("#f3c4c7ff"),
    );
    let cause = d.anon();
    d.view(&cause, "width: Fill height: Fit flow: Down");
    ui::error_line(d, "b3_routes_error", copy::LOAD_FAILED, msg);
    d.close();
    // The phone column stretches its button (`align-items: stretch`).
    let w = if compact { W::Fill } else { W::Fit };
    d.button("b3_routes_retry", copy::TRY_AGAIN, "b3.routes.retry", if st.busy { Btn::OutlineOff } else { Btn::Outline }, w, 32.0);
    d.close();
}

/// `emptyState` (`ModelManagementSection.tsx:1403-1408`, `.module.css:613-633`):
/// the title and hint in a bordered box (26 / 16 insets), centred while the
/// hint fits on one line, left-aligned where it must wrap (a phone).
fn empty_state(d: &mut Dsl, body_w: f64) {
    let fits = ui::text_w(copy::EMPTY_HINT, 12.0, Face::Regular) + 34.0 <= body_w;
    d.surface(
        "b3_routes_empty_box",
        &format!(
            "width: Fill height: Fit flow: Down spacing: 3 align: Align{{x: {} y: 0.0}} padding: Inset{{left: 16 right: 16 top: 26 bottom: 26}}",
            if fits { "0.5" } else { "0.0" }
        ),
        tok::SURFACE,
        12.0,
        Some(tok::HAIRLINE),
    );
    let w = if fits { W::Fit } else { W::Fill };
    d.text("b3_routes_empty", EMPTY, &ui::body_medium().w(w));
    let hint = ui::meta().w(w);
    d.text("b3_routes_empty_hint", copy::EMPTY_HINT, &if fits { hint } else { hint.wrap() });
    d.close();
}

#[allow(clippy::too_many_arguments)]
fn route_card(d: &mut Dsl, st: &RoutesState, i: usize, r: &Route, p: Option<&ms::Provider>, inner_w: f64, caps: ms::Caps, compact: bool) {
    let id = format!("b3_routes_row_{i}");
    ui::card_open(d, &id, 4.0);
    let head = d.anon();
    d.view(&head, "width: Fill height: Fit flow: Right spacing: 8 align: Align{x: 0.0 y: 0.5}");
    let title = p.map(|p| p.name().to_owned()).unwrap_or_else(|| r.name());
    d.text(&format!("{id}_name"), &ui::fit_w(&title, inner_w - 90.0, 13.5, Face::Semibold), &Txt::new(13.5, Face::Semibold, tok::TEXT).w(W::Fill));
    if r.primary {
        d.chip(&format!("{id}_primary"), copy::PRIMARY, tok::BLUE_TEXT, tok::BLUE_BG, None, false);
    }
    d.close();
    let (family, label) = match p {
        Some(p) => (p.family_label.clone(), p.route.label.clone()),
        None => (r.family_id.clone(), if r.label.is_empty() { r.route_id.clone() } else { r.label.clone() }),
    };
    d.text(&format!("{id}_meta"), &ui::fit_w(&format!("{family} · {label}"), inner_w, 12.5, Face::Regular), &ui::meta().w(W::Fill));
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
    // `CredentialIndicator`: a value-safe boolean, never the key.
    let cred = d.anon();
    d.view(&cred, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 7 padding: Inset{top: 2}");
    d.dot(if r.has_key { tok::GREEN } else { tok::DISABLED_INK }, 8.0);
    let ctext = if r.has_key { copy::CREDENTIAL_CONFIGURED } else { copy::CREDENTIAL_MISSING };
    d.text(&format!("{id}_credential"), ctext, &ui::meta());
    d.close();
    // `mutationUnavailableReason` (board 3 #7's locked row).
    if let Some(reason) = p.and_then(|p| p.reason).or_else(|| (!r.removable()).then_some(READ_ONLY)) {
        let lock = d.anon();
        d.view(&lock, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.0} spacing: 6 padding: Inset{top: 2}");
        d.icon(&format!("{id}_lock"), "b3_lock.svg", 13.0, tok::AMBER);
        let rid = if r.removable() { format!("{id}_reason") } else { format!("{id}_readonly") };
        d.text(&rid, reason, &Txt::new(12.0, Face::Regular, tok::AMBER).w(W::Fill).wrap());
        d.close();
    }
    if r.removable() {
        let idle = !st.busy && st.editing.is_none() && st.deleting.is_none();
        let kind = if idle { Btn::Outline } else { Btn::OutlineOff };
        let editable = p.is_some_and(|p| p.editable) && caps.can_save();
        let mut pills: Vec<(String, &str, String)> = Vec::new();
        if editable {
            pills.push((format!("{id}_edit"), "Edit", format!("b3.routes.edit#{i}")));
        }
        if caps.can_save() {
            pills.push((format!("{id}_add"), "Add a model on this route", format!("b3.routes.add#{i}")));
        }
        if caps.delete {
            pills.push((format!("{id}_delete"), "Delete", format!("b3.routes.delete#{i}")));
        }
        let total: f64 = pills.iter().map(|(_, l, _)| pill_w(l) + 8.0).sum();
        if total - 8.0 <= inner_w || !compact {
            let acts = d.anon();
            d.view(&acts, if total - 8.0 <= inner_w {
                "width: Fill height: Fit flow: Right spacing: 8 padding: Inset{top: 4}"
            } else {
                "width: Fill height: Fit flow: Down spacing: 8 padding: Inset{top: 4}"
            });
            for (pid, label, ev) in &pills {
                d.button(pid, label, ev, kind, W::Fit, 32.0);
            }
            d.close();
        } else {
            // Phone: the short pills share a row, the long one takes its own.
            let (short, long): (Vec<_>, Vec<_>) = pills.iter().partition(|(_, l, _)| l.len() < 12);
            let col = d.anon();
            d.view(&col, "width: Fill height: Fit flow: Down spacing: 8 padding: Inset{top: 4}");
            let row = d.anon();
            d.view(&row, "width: Fill height: Fit flow: Right spacing: 8");
            for (pid, label, ev) in short {
                d.button(pid, label, ev, kind, W::Fit, 32.0);
            }
            d.close();
            for (pid, label, ev) in long {
                d.button(pid, label, ev, kind, W::Fit, 32.0);
            }
            d.close();
        }
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
        let label = if st.busy { copy::FETCHING } else { copy::FETCH };
        d.button("b3_routes_fetch", label, "b3.routes.fetch", kind, W::Fit, 34.0);
    }
    if !st.fetched.is_empty() {
        ui::field_label(d, "b3_routes_fetched_label", copy::FROM_ENDPOINT);
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
    d.button("b3_routes_cancel", copy::CANCEL, "b3.routes.cancel", if st.busy { Btn::OutlineOff } else { Btn::Outline }, w, 34.0);
    let both = d.anon();
    d.view(&both, &format!("width: {} height: 34 flow: Overlay", if compact { "Fill".to_owned() } else { format!("{}", (ui::text_w("Save", 13.0, Face::Medium) + 32.0).ceil()) }));
    d.view("b3_routes_save_off", "width: Fill height: Fit flow: Right");
    d.button("b3_routes_save_disabled", copy::SAVE, "b3.routes.save", Btn::Disabled, W::Fill, 34.0);
    d.close();
    d.view("b3_routes_save_on", "width: Fill height: Fit flow: Right");
    let label = if st.busy { copy::SAVING } else { copy::SAVE };
    d.button("b3_routes_save", label, "b3.routes.save", if st.busy { Btn::Disabled } else { Btn::Primary }, W::Fill, 34.0);
    d.close();
    d.close();
    d.close();
    d.close();
}

/// `DeleteProviderDialog`: the typed phrase arms "Delete provider".
fn delete_card(d: &mut Dsl, st: &RoutesState, r: &Route, name: &str, compact: bool) {
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
        &format!("This removes the configured route for {name}. Existing sessions may still refer to it."),
        &ui::meta().w(W::Fill).wrap(),
    );
    d.text("b3_routes_delete_prompt", &format!("Type {} to confirm", r.delete_phrase()), &Txt::new(12.5, Face::Medium, tok::TEXT).w(W::Fill).wrap());
    d.input("b3_routes_phrase", "routes.phrase", "", "", true, 36.0);
    if st.delete_failed {
        d.text("b3_routes_delete_failed", DELETE_FAILED, &Txt::new(12.5, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
    }
    let foot = d.anon();
    d.view(&foot, &format!("width: Fill height: Fit flow: {} align: Align{{x: 1.0 y: 0.5}} spacing: 8", if compact { "Down" } else { "Right" }));
    let w = if compact { W::Fill } else { W::Fit };
    d.button("b3_routes_delete_cancel", copy::CANCEL, "b3.routes.cancel", if st.busy { Btn::OutlineOff } else { Btn::Outline }, w, 34.0);
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
    let (fill, ink) = if armed { (tok::RED, tok::WHITE) } else { (tok::DISABLED_BG, tok::DISABLED_INK) };
    d.surface(&format!("{id}_box"), "width: Fill height: 34 flow: Overlay align: Align{x: 0.5 y: 0.5}", fill, 17.0, None);
    d.text(&format!("{id}_label"), label, &Txt::new(13.0, Face::Medium, ink));
    if armed {
        d.tap(id, event);
    }
    d.close();
}

/// The section heading under the title (`sectionHeading`): the web's intro,
/// and "Add provider" when the server lets this client save.
fn heading(d: &mut Dsl, st: &RoutesState, ready: bool, caps: ms::Caps, compact: bool) {
    let add = ready && caps.can_save();
    let idle = !st.busy && st.editing.is_none() && st.deleting.is_none();
    let row = d.anon();
    d.view(&row, &format!(
        "width: Fill height: Fit flow: {} align: Align{{x: 0.0 y: 0.0}} spacing: {}",
        if compact { "Down" } else { "Right" },
        if compact { 8 } else { 16 }
    ));
    d.text("b3_routes_intro", copy::INTRO, &ui::meta().w(W::Fill).wrap());
    if add {
        let kind = if idle { Btn::Primary } else { Btn::Disabled };
        d.button("b3_routes_add_provider", copy::ADD_PROVIDER, "b3.routes.add_provider", kind, W::Fit, 32.0);
    }
    d.close();
}

pub fn build(d: &mut Dsl, st: &RoutesState, frame: &Frame, store: &Store) {
    let caps = caps_of(store);
    let width = frame.dialog_w(640.0);
    let compact = frame.compact(width);
    let pad = ui::dialog_pad(frame, width);
    let inner_w = width - 2.0 * pad - 10.0 - 28.0;
    let proj = st.projection(caps);
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
    heading(d, st, proj.state == ViewState::Ready, caps, compact);
    // The web section's rhythm (`.section` gap 14, `.providerRows` gap 10):
    // one SECTION gap before every block, ROWS between provider cards.
    const SECTION: f64 = 12.0;
    const ROWS: f64 = 10.0;
    match &proj.state {
        ViewState::Loading => {
            d.gap(W::Fill, SECTION);
            state_notice(d, "b3_routes_loading", copy::LOADING);
        }
        // An UNREAD configuration: its cause and "Try again" — never the
        // empty state (`model-management-projection.ts:57-63`).
        ViewState::Error(msg) => {
            d.gap(W::Fill, SECTION);
            state_error(d, st, msg, compact);
        }
        ViewState::Unavailable(msg) => {
            d.gap(W::Fill, SECTION);
            note(d, "b3_routes_unavailable", msg);
        }
        ViewState::Ready => {
            if !caps.can_save() {
                d.gap(W::Fill, SECTION);
                note(d, "b3_routes_readonly_note", copy::READ_ONLY);
            }
            if caps.can_save() || caps.delete {
                d.gap(W::Fill, SECTION);
                ui::banner(d, "b3_routes_warning", copy::RUNTIME_WARNING, "");
            }
        }
    }
    if let Some(n) = &st.notice {
        d.gap(W::Fill, SECTION);
        status_line(d, "b3_routes_notice", n, true);
    }
    if proj.state == ViewState::Ready {
        if let Some(e) = &st.error {
            d.gap(W::Fill, SECTION);
            d.text("b3_routes_error", e, &Txt::new(12.5, Face::Regular, tok::RED_TEXT).w(W::Fill).wrap());
        }
        if let Some(i) = st.deleting {
            if let Some(r) = st.routes.get(i) {
                let name = proj.providers.get(i).map(|p| p.name().to_owned()).unwrap_or_else(|| r.name());
                d.gap(W::Fill, SECTION);
                delete_card(d, st, r, &name, compact);
            }
        }
        if let Some(r) = st.editing.and_then(|i| st.routes.get(i)) {
            d.gap(W::Fill, SECTION);
            editor(d, st, r, store, compact, inner_w);
        }
        if st.routes.is_empty() {
            d.gap(W::Fill, SECTION);
            empty_state(d, inner_w + 28.0);
        }
        for (i, r) in st.routes.iter().enumerate() {
            d.gap(W::Fill, if i == 0 { SECTION } else { ROWS });
            route_card(d, st, i, r, proj.providers.get(i), inner_w, caps, compact);
        }
    }
    ui::body_close(d);
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r2_config() -> Value {
        json!({"profile_id": "dsflash",
            "primary": {"family_id": "deepseek", "model_id": "deepseek-v4-flash", "has_api_key": true, "selected": true, "available": true,
                        "route": {"api_type": "openai", "label": "Official API", "route_id": "deepseek", "api_key_env": "DEEPSEEK_API_KEY"}},
            "fallbacks": [{"family_id": "deepseek", "model_id": "deepseek-v4-flash", "has_api_key": true, "selected": false, "available": true,
                           "route": {"api_type": "openai", "base_url": "http://127.0.0.1:9/v1", "route_id": "r2-route"}}]})
    }

    fn store_with(methods: &[&str]) -> Store {
        let store = Store::new();
        store.domains.config.set_supported_methods(methods.iter().map(|s| s.to_string()).collect());
        store
    }

    const ALL: [&str; 6] = ["profile/llm/list", "profile/llm/catalog", "profile/llm/delete", "profile/llm/test", "profile/llm/upsert", "profile/llm/fetch_models"];

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
        let mut st = RoutesState::default();
        st.set_config(ms::parse_config(&r2_config()).unwrap());
        let store = store_with(&ALL);
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

    /// The section's capability states (`ModelManagementSection.test.tsx:63`):
    /// read-only without upsert (no Add, no Edit, no Delete), loading, and an
    /// unread configuration as an error with Try again.
    #[test]
    fn the_section_fails_closed_into_read_only_and_capability_states() {
        let mut st = RoutesState::default();
        st.set_config(ms::parse_config(&r2_config()).unwrap());
        let read_only = store_with(&["profile/llm/list"]);
        let mut d = Dsl::new();
        build(&mut d, &st, &Frame::DESKTOP, &read_only);
        let dsl = d.finish();
        assert!(dsl.contains(copy::READ_ONLY));
        for absent in ["b3_routes_add_provider", "b3_routes_row_0_edit", "b3_routes_row_0_delete", copy::RUNTIME_WARNING] {
            assert!(!dsl.contains(absent), "{absent}");
        }
        let loading = RoutesState { busy: true, ..Default::default() };
        let mut d = Dsl::new();
        build(&mut d, &loading, &Frame::DESKTOP, &store_with(&ALL));
        let dsl = d.finish();
        assert!(dsl.contains(copy::LOADING) && !dsl.contains(EMPTY));
        let failed = RoutesState { error: Some("Provider catalog timed out".into()), ..Default::default() };
        let mut d = Dsl::new();
        build(&mut d, &failed, &Frame::DESKTOP, &store_with(&ALL));
        let dsl = d.finish();
        assert!(dsl.contains("Provider catalog timed out") && dsl.contains(copy::TRY_AGAIN) && !dsl.contains(EMPTY));
    }
}
