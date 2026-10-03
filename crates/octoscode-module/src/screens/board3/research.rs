//! A10 — the Research provider lanes dialog (`/research`, alias `/lanes`):
//! the Profile's named provider routes for the server research pipeline —
//! list (`profile/sub_providers/list`), add or replace (`…/upsert`, with an
//! optional credential read only at dispatch), remove (`…/remove`); each
//! mutation behind the web's confirmation card, the Profile lock and lease,
//! and a generation guard so a superseded read or mutation never publishes.
//!
//! Web oracle: `features/research/ResearchDialog.tsx` (opened by the
//! `research` intent, `registry.ts:374-382`), `packages/client/src/research.ts`
//! (strict parse, explicit whitelist, receipt checks). No Stage-A board
//! covers it: built with the native dialog kit (`board3/ui.rs` — the same
//! backdrop, centred card, close glyph and pills as the Agents panel), the
//! web component as the reference.
//!
//! The lanes live in the store (`profile.sub_providers`, folded by the same
//! `models::fold_sub_providers` the Fleet lane picker reads); this module
//! keeps the UI-local draft. The credential is NEVER part of the draft, the
//! store or a confirmation (`ResearchDialog.tsx:131-137`): its masked input's
//! text sits in a private slot, is taken (and cleared) at dispatch, and is
//! cleared by Edit / Remove / Cancel / Clear.
use std::sync::Mutex;

use octoscode_client::domains::profile::{
    SubProviderParams, SubProvidersList, SubProvidersListParams, SubProvidersRemove, SubProvidersRemoveParams,
    SubProvidersUpsert, SubProvidersUpsertParams,
};
use octoscode_client::Method;
use octoscode_store::domains::profile::SubProvider;
use octoscode_store::Store;
use serde_json::Value;

use super::host::{Job, Outcome};
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};
use crate::i18n::{tr, tr1};

/// `ResearchDialog.tsx:208-214`, verbatim.
pub const INTRO: &str = "Named provider routes for the server research pipeline. This is not a separate browser \
                         agent loop. Provider identity and API style are separate fields.";
/// `:216-222` — the Profile lock line.
pub const LOCKED: &str = "Configuration changes are paused while known Profile work is running.";
/// `:83-86` — every mutation failure (the cause is never shown: a provider
/// or transport can echo the submitted credential).
pub const MUTATION_FAILED: &str = "Could not confirm the server change. It may have been applied; refresh before a \
                                   new attempt and re-enter any credential.";
/// `:226-228`.
pub const EMPTY: &str = "No research lanes configured in this Profile.";
/// A13 — the plain lead over a failed lane read.
pub const LOAD_FAILED: &str = "Couldn't load the research lanes.";
pub const HELP: &str =
    "Saving an existing key replaces that lane. Empty optional fields use the server/provider defaults.";
pub const CREDENTIAL_LABEL: &str = "New credential (optional; otherwise reuse server configuration)";

/// The lane draft (`ResearchLane`; the credential is not here).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    pub key: String,
    pub provider: String,
    pub model: String,
    pub api_type: String,
    pub base_url: String,
    pub api_key_env: String,
    pub description: String,
    pub context_window: String,
    pub max_output_tokens: String,
}

/// `text` (`research.ts:28`): at most 8192 UTF-16 units.
fn text_ok(s: &str) -> bool {
    s.encode_utf16().count() <= 8192
}

/// `name`: a non-blank `text`.
fn name_ok(s: &str) -> bool {
    text_ok(s) && !s.trim().is_empty()
}

impl Draft {
    pub fn from_lane(l: &SubProvider) -> Self {
        Draft {
            key: l.key.clone(),
            provider: l.provider.clone(),
            model: l.model.clone().unwrap_or_default(),
            api_type: l.api_type.clone().unwrap_or_default(),
            base_url: l.base_url.clone().unwrap_or_default(),
            api_key_env: l.api_key_env.clone().unwrap_or_default(),
            description: l.description.clone().unwrap_or_default(),
            context_window: l.default_context_window.map(|v| v.to_string()).unwrap_or_default(),
            max_output_tokens: l.max_output_tokens.map(|v| v.to_string()).unwrap_or_default(),
        }
    }

    /// The Review gate (`:330-336`): a key and a provider identity.
    pub fn reviewable(&self) -> bool {
        !self.key.trim().is_empty() && !self.provider.trim().is_empty()
    }

    /// `researchLaneParams` (`research.ts:113-137`): the explicit whitelist,
    /// key/provider trimmed, an empty optional field = absent (`value ||
    /// null`), counts whole u32s. `Err` = "Invalid research lane".
    pub fn params(&self) -> Result<SubProviderParams, String> {
        let opt = |s: &str| (!s.is_empty()).then(|| s.to_owned());
        let count = |s: &str| -> Result<Option<u32>, ()> {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                s.trim().parse::<u32>().map(Some).map_err(|_| ())
            }
        };
        let texts = [&self.model, &self.api_key_env, &self.base_url, &self.description, &self.api_type];
        if !name_ok(&self.key) || !name_ok(&self.provider) || !texts.iter().all(|t| text_ok(t)) {
            return Err("Invalid research lane".into());
        }
        let (Ok(window), Ok(max_out)) = (count(&self.context_window), count(&self.max_output_tokens)) else {
            return Err("Invalid research lane".into());
        };
        Ok(SubProviderParams {
            key: self.key.trim().to_owned(),
            provider: self.provider.trim().to_owned(),
            model: opt(&self.model),
            api_key_env: opt(&self.api_key_env),
            base_url: opt(&self.base_url),
            description: opt(&self.description),
            default_context_window: window,
            max_output_tokens: max_out,
            api_type: opt(&self.api_type),
        })
    }
}

/// The confirmation the web asks before a mutation (`Confirmation`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirm {
    Save(Draft),
    Remove(String),
}

#[derive(Debug, Clone, Default)]
pub struct ResearchState {
    pub draft: Draft,
    /// What the DSL carries (refreshed at every perform and job finish — the
    /// inputs never remount per keystroke).
    pub snap: Draft,
    /// This generation's list arrived (`data !== null`): the lanes and the
    /// empty line show only then.
    pub loaded: bool,
    pub confirm: Option<Confirm>,
    pub busy: bool,
    /// A mutation holds the Profile lease: the dialog does not close.
    pub mutating: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    /// The generation guard (`:54-112`): bumped on open and close; a request
    /// captures it and publishes only while it is current.
    pub generation: u64,
    /// One request at a time (`pending`).
    pub pending: bool,
    /// Bumped whenever the credential must read empty again (dispatch,
    /// Edit, Remove, Cancel, Clear): the form remounts.
    pub form_gen: u64,
    /// A turn runs (the host reports it): part of the Profile lock.
    pub turn_busy: bool,
}

/// The masked credential: (live text, the text the DSL re-seeds a remount
/// with). Never in the draft, the store or a confirmation.
static CREDENTIAL: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));

fn credential() -> std::sync::MutexGuard<'static, (String, String)> {
    CREDENTIAL.lock().unwrap_or_else(|p| p.into_inner())
}

/// `resetCredential()`.
pub fn clear_credential() {
    let mut c = credential();
    c.0.clear();
    c.1.clear();
}

/// Read at dispatch, then cleared.
fn take_credential() -> Option<String> {
    let mut c = credential();
    c.1.clear();
    let v = std::mem::take(&mut c.0);
    (!v.is_empty()).then_some(v)
}

/// Refresh the snapshots the next mount carries (the live values).
fn resnap(st: &mut ResearchState) {
    st.snap = st.draft.clone();
    let mut c = credential();
    c.1 = c.0.clone();
}

fn has(store: &Store, m: &str) -> bool {
    store.domains.config.supported_methods().iter().any(|x| x == m)
}

/// `profileBusy` (`App.tsx:3843-3850`): a Profile mutation lease is held or
/// known work is running.
pub fn locked(st: &ResearchState, store: &Store) -> bool {
    store.domains.profile.profile_busy() || st.turn_busy
}

/// Opening: a fresh surface (the web mounts a new component) and its list.
pub fn on_open(st: &mut ResearchState) -> Outcome {
    let (generation, form_gen, turn_busy) = (st.generation + 1, st.form_gen + 1, st.turn_busy);
    *st = ResearchState { generation, form_gen, turn_busy, ..Default::default() };
    clear_credential();
    Outcome::Spawn(Job::ResearchLoad(generation))
}

/// Closing retires the generation: a reply still in flight never publishes.
pub fn on_close(st: &mut ResearchState) {
    st.generation += 1;
    st.pending = false;
    st.busy = false;
    st.mutating = false;
    clear_credential();
}

/// Route one `b3.research.*` action (disabled controls emit no tap; the
/// guards repeat the web's `disabled` conditions so a stale tap is inert).
pub fn perform(st: &mut ResearchState, action: &str, index: usize, store: &Store) -> Outcome {
    resnap(st);
    let lanes = store.domains.profile.sub_providers();
    let lock = locked(st, store);
    let free = !st.busy;
    match action {
        "b3.research.refresh" if free && !st.pending => Outcome::Spawn(Job::ResearchLoad(st.generation)),
        "b3.research.edit" if free && !lock && has(store, SubProvidersUpsert::NAME) => {
            if let Some(l) = lanes.get(index) {
                clear_credential();
                st.draft = Draft::from_lane(l);
                st.snap = st.draft.clone();
                st.confirm = None;
                st.form_gen += 1;
            }
            Outcome::Done
        }
        "b3.research.remove" if free && !lock && has(store, SubProvidersRemove::NAME) => {
            if let Some(l) = lanes.get(index) {
                clear_credential();
                st.confirm = Some(Confirm::Remove(l.key.clone()));
                st.form_gen += 1;
            }
            Outcome::Done
        }
        "b3.research.review"
            if free && !lock && st.confirm.is_none() && st.draft.reviewable() && has(store, SubProvidersUpsert::NAME) =>
        {
            st.confirm = Some(Confirm::Save(st.draft.clone()));
            Outcome::Done
        }
        "b3.research.clear" if free && st.confirm.is_none() => {
            clear_credential();
            st.draft = Draft::default();
            st.snap = Draft::default();
            st.form_gen += 1;
            Outcome::Done
        }
        "b3.research.cancel" if free => {
            clear_credential();
            st.confirm = None;
            st.form_gen += 1;
            Outcome::Done
        }
        "b3.research.confirm" if free && !lock && !st.pending => match st.confirm.clone() {
            Some(Confirm::Save(d)) => Outcome::Spawn(Job::ResearchSave(st.generation, d)),
            Some(Confirm::Remove(k)) => Outcome::Spawn(Job::ResearchRemove(st.generation, k)),
            None => Outcome::Done,
        },
        a if a.starts_with("b3.research.") => Outcome::Done,
        _ => Outcome::Unrouted,
    }
}

/// A text input changed. The credential goes to its private slot only.
pub fn input_changed(st: &mut ResearchState, key: &str, text: &str) {
    let d = &mut st.draft;
    let slot = match key {
        "research.key" => &mut d.key,
        "research.provider" => &mut d.provider,
        "research.model" => &mut d.model,
        "research.api_type" => &mut d.api_type,
        "research.base_url" => &mut d.base_url,
        "research.api_key_env" => &mut d.api_key_env,
        "research.description" => &mut d.description,
        "research.context_window" => &mut d.context_window,
        "research.max_output_tokens" => &mut d.max_output_tokens,
        "research.credential" => {
            credential().0 = text.to_owned();
            return;
        }
        _ => return,
    };
    *slot = text.to_owned();
}

/// The live gate (no remount while typing): "Review lane save" is armed by
/// a key and a provider identity.
pub fn visibility(st: &ResearchState) -> Vec<(String, bool)> {
    let ok = st.draft.reviewable();
    vec![("b3_research_review_on".into(), ok), ("b3_research_review_off".into(), !ok)]
}

/// The host reports whether a turn runs (part of the Profile lock).
pub fn note_turn_busy(st: &mut ResearchState, busy: bool) {
    st.turn_busy = busy;
}

// --------------------------------------------------------------- transport

fn nullable_text(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Null)) || v.and_then(|x| x.as_str()).is_some_and(text_ok)
}

fn nullable_u32(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Null)) || v.and_then(|x| x.as_u64()).is_some_and(|n| n <= u32::MAX as u64)
}

/// `parseResearchLanes` (`research.ts:34-73`): the Profile echoed exactly,
/// at most 10000 rows, unique non-blank keys, every field present (null or
/// text / u32).
pub fn lanes_ok(v: &Value, profile: &str) -> bool {
    if v.get("profile_id").and_then(|p| p.as_str()) != Some(profile) {
        return false;
    }
    let Some(rows) = v.get("sub_providers").and_then(|s| s.as_array()) else { return false };
    if rows.len() > 10_000 {
        return false;
    }
    let mut keys = std::collections::HashSet::new();
    rows.iter().all(|r| {
        let key = r.get("key").and_then(|k| k.as_str()).unwrap_or("");
        let provider = r.get("provider").and_then(|k| k.as_str()).unwrap_or("");
        name_ok(key)
            && keys.insert(key.to_owned())
            && name_ok(provider)
            && ["model", "api_key_env", "base_url", "description", "api_type"].iter().all(|f| nullable_text(r.get(*f)))
            && ["default_context_window", "max_output_tokens"].iter().all(|f| nullable_u32(r.get(*f)))
    })
}

/// `parseResearchLaneMutation`: the lanes plus boolean `applied` and
/// `restart_required`.
pub fn receipt_ok(v: &Value, profile: &str) -> Option<(bool, bool)> {
    let applied = v.get("applied").and_then(|x| x.as_bool())?;
    let restart = v.get("restart_required").and_then(|x| x.as_bool())?;
    lanes_ok(v, profile).then_some((applied, restart))
}

fn lists_key(v: &Value, key: &str) -> bool {
    v.get("sub_providers")
        .and_then(|s| s.as_array())
        .is_some_and(|rows| rows.iter().any(|r| r.get("key").and_then(|k| k.as_str()) == Some(key)))
}

fn current(generation: u64) -> bool {
    super::host::state().research.generation == generation
}

/// `finally`: when the request is still current, release `pending`/`busy`
/// and publish its result; a superseded one publishes nothing.
fn finish(generation: u64, f: impl FnOnce(&mut ResearchState)) -> bool {
    let mut st = super::host::state();
    if st.research.generation != generation {
        return false;
    }
    st.research.pending = false;
    st.research.busy = false;
    st.research.mutating = false;
    f(&mut st.research);
    resnap(&mut st.research);
    true
}

/// `available()` (`research.ts:143-147`): a confirmed Profile and the method.
fn profile_for(conv: &crate::flow::Conversation, method: &str) -> Result<String, String> {
    let store = &conv.store;
    let profile = store.domains.profile.current().unwrap_or_else(|| conv.profile());
    if !name_ok(&profile) {
        return Err("A confirmed Profile is required".into());
    }
    if !has(store, method) {
        return Err(format!("{method} is not advertised by this server"));
    }
    Ok(profile)
}

/// `refresh()` → `list()` (`{profile_id}`, strict parse).
pub async fn load(conv: &crate::flow::Conversation, generation: u64) -> Result<String, String> {
    let store = &conv.store;
    {
        let mut st = super::host::state();
        let r = &mut st.research;
        if r.pending || r.generation != generation {
            return Ok("refused (pending or superseded)".into());
        }
        r.pending = true;
        r.busy = true;
        r.error = None;
        resnap(r);
    }
    let result = async {
        let profile = profile_for(conv, SubProvidersList::NAME)?;
        let params = SubProvidersListParams { profile_id: Some(profile.clone()) };
        let v = conv
            .client()
            .request(SubProvidersList::NAME, serde_json::to_value(params).map_err(|e| e.to_string())?)
            .await
            .map_err(|e| crate::screens::dialog::display_error(&e.to_string()))?;
        if !lanes_ok(&v, &profile) {
            return Err("Invalid or wrong-profile research lanes".to_owned());
        }
        Ok(v)
    }
    .await;
    match result {
        Ok(v) => {
            if !current(generation) {
                makepad_widgets::log!("[octoscode] research: list generation {generation} superseded — reply dropped");
                return Ok("superseded".into());
            }
            crate::screens::models::fold_sub_providers(v, store);
            finish(generation, |s| s.loaded = true);
            Ok(format!("{} lane(s)", store.domains.profile.sub_providers().len()))
        }
        Err(e) => {
            let shown: String = e.chars().take(512).collect();
            finish(generation, |s| s.error = Some(shown));
            Err(e)
        }
    }
}

/// `confirm()` → `run(work, true)`: refused while pending, superseded or
/// locked; then the Profile lease, the credential read and cleared, the
/// confirmation dropped, the request; the receipt (checked) replaces the
/// lanes and sets the notice — only while its generation is current. Any
/// failure shows [`MUTATION_FAILED`]; the returned `Err` names the cause for
/// the log and never carries the credential.
pub async fn mutate(conv: &crate::flow::Conversation, generation: u64, confirm: Confirm) -> Result<String, String> {
    let store = &conv.store;
    {
        let mut st = super::host::state();
        let lock = locked(&st.research, store);
        let r = &mut st.research;
        if r.pending || r.generation != generation || lock {
            return Ok("refused (pending, superseded or locked)".into());
        }
        r.pending = true;
        r.busy = true;
        r.mutating = true;
        r.error = None;
    }
    // `release = onMutationStart()`.
    store.domains.profile.set_profile_busy(true);
    makepad_widgets::SignalToUI::set_ui_signal();
    let secret = match &confirm {
        Confirm::Save(_) => take_credential(),
        Confirm::Remove(_) => None,
    };
    {
        let mut st = super::host::state();
        st.research.confirm = None;
        st.research.form_gen += 1; // the masked input remounts empty
        resnap(&mut st.research);
    }
    let result = send(conv, &confirm, secret).await;
    store.domains.profile.set_profile_busy(false);
    match result {
        Ok(receipt) => {
            if !current(generation) {
                makepad_widgets::log!("[octoscode] research: mutation generation {generation} superseded — receipt not shown");
                return Ok("superseded".into());
            }
            let applied = receipt.get("applied").and_then(|v| v.as_bool()).unwrap_or(false);
            let restart = receipt.get("restart_required").and_then(|v| v.as_bool()).unwrap_or(false);
            crate::screens::models::fold_sub_providers(receipt, store);
            let notice = crate::screens::research::mutation_notice(applied, restart);
            finish(generation, |s| {
                s.loaded = true;
                s.notice = Some(notice.to_owned());
            });
            Ok(notice.into())
        }
        Err(cause) => {
            finish(generation, |s| s.error = Some(MUTATION_FAILED.into()));
            Err(cause)
        }
    }
}

/// The wire half of a mutation (`createResearchCommands().upsert|remove`).
async fn send(conv: &crate::flow::Conversation, confirm: &Confirm, secret: Option<String>) -> Result<Value, String> {
    match confirm {
        Confirm::Save(d) => {
            let profile = profile_for(conv, SubProvidersUpsert::NAME)?;
            let lane = d.params()?;
            if secret.is_some() && d.api_key_env.trim().is_empty() {
                return Err("An API key environment name is required for a new credential".into());
            }
            let with_key = secret.is_some();
            let key = lane.key.clone();
            let params = SubProvidersUpsertParams { profile_id: Some(profile.clone()), sub_provider: lane, api_key: secret };
            let body = serde_json::to_value(params).map_err(|_| "upsert params".to_owned())?;
            let v = conv.client().request(SubProvidersUpsert::NAME, body).await.map_err(|e| {
                // "Providers/transports can echo input in errors": the cause
                // of a credential-bearing save is never kept.
                if with_key {
                    "Could not confirm the credential-bearing lane save. Refresh before retrying; the server may have applied it."
                        .to_owned()
                } else {
                    crate::screens::dialog::display_error(&e.to_string())
                }
            })?;
            let Some((applied, _)) = receipt_ok(&v, &profile) else {
                return Err("Invalid or wrong-profile research mutation receipt; refresh before retrying".into());
            };
            if applied && !lists_key(&v, &key) {
                return Err("Research save receipt omitted the requested lane; refresh before retrying".into());
            }
            Ok(v)
        }
        Confirm::Remove(key) => {
            let profile = profile_for(conv, SubProvidersRemove::NAME)?;
            if !name_ok(key) {
                return Err("A lane key is required".into());
            }
            let params = SubProvidersRemoveParams { profile_id: Some(profile.clone()), key: key.clone() };
            let body = serde_json::to_value(params).map_err(|_| "remove params".to_owned())?;
            let v = conv
                .client()
                .request(SubProvidersRemove::NAME, body)
                .await
                .map_err(|e| crate::screens::dialog::display_error(&e.to_string()))?;
            let Some((applied, _)) = receipt_ok(&v, &profile) else {
                return Err("Invalid or wrong-profile research mutation receipt; refresh before retrying".into());
            };
            if applied && lists_key(&v, key) {
                return Err("Research removal receipt still contains the requested lane; refresh before retrying".into());
            }
            Ok(v)
        }
    }
}

// -------------------------------------------------------------------- view

fn walk_w(w: W) -> String {
    match w {
        W::Px(v) => format!("{v}"),
        W::Fill => "Fill".into(),
        W::Fit => "Fit".into(),
    }
}

/// A field's caption: the kit's field label, wrapping on a narrow sheet
/// (the credential's long label was cut at 360 px).
fn caption(d: &mut Dsl, id: &str, label: &str) {
    d.text(&format!("{id}_caption"), label, &Txt::new(12.0, Face::Medium, tok::MUTED).w(W::Fill).wrap());
}

/// A pill's laid-out width (the kit's `W::Fit` rule).
fn pill_w(label: &str) -> f64 {
    ui::text_w(label, 13.0, Face::Medium) + 32.0
}

/// A row of pills (right-aligned), or a column of full-width pills when the
/// row does not fit (`fits` = false): wrapped pill rows otherwise touch.
fn pills_open(d: &mut Dsl, fits: bool) {
    let row = d.anon();
    if fits {
        d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5} spacing: 8");
    } else {
        d.view(&row, "width: Fill height: Fit flow: Down spacing: 8");
    }
}

/// A labelled one-line input.
fn field(d: &mut Dsl, id: &str, key: &str, label: &str, value: &str, w: W) {
    let col = d.anon();
    d.view(&col, &format!("width: {} height: Fit flow: Down spacing: 6", walk_w(w)));
    caption(d, id, label);
    d.input(id, key, value, "", false, 36.0);
    d.close();
}

/// The credential input (`type="password" autoComplete="off"`): the kit's
/// field with `is_password: true` — the same masked TextInput the provider
/// key uses (`screens/provider.rs`) — seeded only from the remount snapshot.
fn masked_field(d: &mut Dsl, id: &str, key: &str, label: &str, value: &str) {
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 6");
    caption(d, id, label);
    d.inputs.push((id.to_owned(), key.to_owned()));
    d.surface(
        &format!("{id}_field"),
        "width: Fill height: 36 flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 10 right: 10 top: 0 bottom: 0}",
        tok::SURFACE,
        8.0,
        Some("#d9d9dcff"),
    );
    let props = format!(
        "width: Fill height: Fit padding: Inset{{left: 0 right: 0 top: 4 bottom: 4}} margin: 0\ntext: {} empty_text: \"\"\nflow: Right is_read_only: false is_password: true\ndraw_bg +: {{pixel: fn() {{return vec4(0.0, 0.0, 0.0, 0.0)}}}}\ndraw_text +: {{color: {t} color_hover: {t} color_focus: {t} color_down: {t} color_disabled: {f} color_empty: {f} color_empty_hover: {f} color_empty_focus: {f}}}\ndraw_text.text_style: {}\ndraw_cursor +: {{color: {t}}}",
        ui::lit(value),
        ui::text_style(Face::Regular, 13.0),
        t = tok::TEXT,
        f = tok::FAINT,
    );
    d.open(id, "TextInput", &props);
    d.close();
    d.close();
    d.close();
}

/// Two fields side by side (stacked on a phone-narrow frame).
fn pair(d: &mut Dsl, compact: bool, a: (&str, &str, &str, &str), b: (&str, &str, &str, &str)) {
    let row = d.anon();
    let flow = if compact { "Down" } else { "Right" };
    d.view(&row, &format!("width: Fill height: Fit flow: {flow} spacing: 10"));
    field(d, a.0, a.1, a.2, a.3, W::Fill);
    field(d, b.0, b.1, b.2, b.3, W::Fill);
    d.close();
}

fn status(d: &mut Dsl, id: &str, text: &str, color: &'static str) {
    d.text(id, text, &Txt::new(12.5, Face::Regular, color).w(W::Fill).wrap());
}

pub fn build(d: &mut Dsl, st: &ResearchState, frame: &Frame, store: &Store) {
    let width = frame.dialog_w(720.0);
    let compact = frame.compact(width);
    let pad = ui::dialog_pad(frame, width);
    let inner_w = width - 2.0 * pad - 10.0 - 28.0; // scroll gutter + card padding
    let lanes = store.domains.profile.sub_providers();
    let (can_upsert, can_remove) = (has(store, SubProvidersUpsert::NAME), has(store, SubProvidersRemove::NAME));
    let lock = locked(st, store);
    let profile = store.domains.profile.current().unwrap_or_default();
    ui::shell_open(d, frame, width);
    // Header: the title, "Refresh lanes" (disabled while busy), and the close
    // glyph (inert while a mutation holds the lease, `:198-203`).
    let row = d.anon();
    d.view(&row, "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 4");
    d.text("b3_title", tr("Research provider lanes"), &ui::title().w(W::Fill));
    if st.busy {
        d.view("b3_research_refresh_box", "width: 28 height: 28 flow: Overlay align: Align{x: 0.5 y: 0.5}");
        d.icon("b3_research_refresh_icon", "b3_refresh.svg", 16.0, tok::FAINT);
        d.close();
    } else {
        ui::icon_button(d, "b3_research_refresh", "b3_refresh.svg", 16.0, "b3.research.refresh");
    }
    ui::close_glyph(d, if st.mutating { "b3.noop" } else { "b3.close" });
    d.close();
    d.text("b3_research_scope", &format!("{} {profile}", tr("Server Profile:")), &ui::micro().w(W::Fill));
    d.gap(W::Fill, 10.0);
    ui::body_open(d, frame, width, 60.0);
    d.text("b3_research_intro", tr(INTRO), &ui::meta().w(W::Fill).wrap());
    d.gap(W::Fill, 6.0);
    if lock || st.busy || st.error.is_some() || st.notice.is_some() {
        d.view("b3_research_status", "width: Fill height: Fit flow: Down spacing: 6 padding: Inset{bottom: 4}");
        if lock {
            status(d, "b3_research_locked", tr(LOCKED), tok::AMBER);
        }
        if st.busy {
            status(d, "b3_research_busy", tr("Waiting for the server…"), tok::MUTED);
        }
        if let Some(e) = &st.error {
            // A13: a failed read leads with what failed; the cause the web
            // prints stays under it, muted. The change refusal is already a
            // sentence for people.
            if e == MUTATION_FAILED {
                status(d, "b3_research_error", tr(e), tok::RED_TEXT);
            } else {
                ui::failure(d, "b3_research_error", LOAD_FAILED, e);
            }
        }
        if let Some(n) = &st.notice {
            status(d, "b3_research_notice", tr(n), tok::TEXT);
        }
        d.close();
    }
    // The confirmation (the web renders it after the form; here it sits
    // above the lanes, in view whichever control asked).
    if let Some(c) = &st.confirm {
        d.gap(W::Fill, 4.0);
        d.surface(
            "b3_research_confirm",
            "width: Fill height: Fit flow: Down spacing: 8 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}",
            tok::AMBER_BG,
            12.0,
            Some(tok::AMBER_LINE),
        );
        let (title, detail, go) = match c {
            Confirm::Save(dr) => (
                "Confirm lane save",
                format!(
                    "{} · {} · {}",
                    dr.key,
                    dr.provider,
                    if dr.model.is_empty() { tr("default model") } else { dr.model.as_str() }
                ),
                "Confirm save",
            ),
            Confirm::Remove(k) => ("Confirm lane removal", k.clone(), "Confirm removal"),
        };
        let go = tr(go);
        d.text("b3_research_confirm_title", tr(title), &ui::heading().w(W::Fill));
        d.text(
            "b3_research_confirm_detail",
            &ui::fit_w(&detail, inner_w, 12.0, Face::Mono),
            &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill),
        );
        d.text(
            "b3_research_confirm_body",
            // The web composes it around the Profile (ResearchDialog.tsx).
            &format!(
                "{} {profile}{}",
                tr("This changes server Profile"),
                tr(". The response will report whether a restart is required. Removing a lane requires re-adding its configuration to recover it.")
            ),
            &ui::meta().w(W::Fill).wrap(),
        );
        let cancel = if st.busy { Btn::OutlineOff } else { Btn::Outline };
        let ok = if st.busy || lock { Btn::Disabled } else { Btn::Primary };
        let fits = pill_w(tr("Cancel lane change")) + 8.0 + pill_w(go) <= inner_w;
        pills_open(d, fits);
        if fits {
            d.button("b3_research_cancel", tr("Cancel lane change"), "b3.research.cancel", cancel, W::Fit, 34.0);
            d.button("b3_research_confirm_go", go, "b3.research.confirm", ok, W::Fit, 34.0);
        } else {
            // The phone sheet: the primary on top, full width.
            d.button("b3_research_confirm_go", go, "b3.research.confirm", ok, W::Fill, 36.0);
            d.button("b3_research_cancel", tr("Cancel lane change"), "b3.research.cancel", cancel, W::Fill, 36.0);
        }
        d.close();
        d.close();
    }
    d.gap(W::Fill, 8.0);
    // The lanes (`:229-279`), once this generation's list arrived.
    if st.loaded && lanes.is_empty() {
        d.text("b3_research_empty", tr(EMPTY), &ui::meta().w(W::Fill));
    }
    if st.loaded {
        for (i, l) in lanes.iter().enumerate() {
            let id = format!("b3_research_lane_{i}");
            ui::card_open(d, &id, 5.0);
            d.text(
                &format!("{id}_key"),
                &ui::fit_w(&l.key, inner_w, 13.5, Face::Semibold),
                &Txt::new(13.5, Face::Semibold, tok::TEXT).w(W::Fill),
            );
            let model = l.model.clone().unwrap_or_else(|| tr("Model not specified").into());
            d.text(
                &format!("{id}_route"),
                &ui::fit_w(&format!("{} · {model}", l.provider), inner_w, 12.5, Face::Regular),
                &Txt::new(12.5, Face::Regular, tok::TEXT).w(W::Fill),
            );
            let style = l.api_type.clone().unwrap_or_else(|| tr("Provider default").into());
            d.text(&format!("{id}_style"), &format!("{} {style}", tr("API style:")), &ui::meta().w(W::Fill));
            if let Some(desc) = l.description.as_deref().filter(|s| !s.is_empty()) {
                d.text(&format!("{id}_desc"), desc, &ui::meta().w(W::Fill).wrap());
            }
            if can_upsert || can_remove {
                d.gap(W::Fill, 2.0);
                let kind = if st.busy || lock { Btn::OutlineOff } else { Btn::Outline };
                let short = ui::fit_w(&l.key, 140.0, 13.0, Face::Medium);
                let (edit, remove) = (tr1("Edit {value0}", &short), tr1("Remove {value0}", &short));
                let fits = pill_w(&edit) + 8.0 + pill_w(&remove) <= inner_w;
                let acts = d.anon();
                if fits {
                    d.view(&acts, "width: Fill height: Fit flow: Right spacing: 8");
                } else {
                    d.view(&acts, "width: Fill height: Fit flow: Down spacing: 8");
                }
                let w = if fits { W::Fit } else { W::Fill };
                if can_upsert {
                    d.button(&format!("{id}_edit"), &edit, &format!("b3.research.edit#{i}"), kind, w, 32.0);
                }
                if can_remove {
                    d.button(&format!("{id}_remove"), &remove, &format!("b3.research.remove#{i}"), kind, w, 32.0);
                }
                d.close();
            }
            d.close();
            d.gap(W::Fill, 4.0);
        }
    }
    // Add or replace a lane (`:280-347`).
    if can_upsert {
        d.gap(W::Fill, 8.0);
        ui::section_title(d, "b3_research_form_title", tr("Add or replace a lane"));
        d.text("b3_research_form_help", tr(HELP), &ui::meta().w(W::Fill).wrap());
        d.gap(W::Fill, 6.0);
        let form = format!("b3_research_form_{}", st.form_gen);
        d.surface(
            &form,
            "width: Fill height: Fit flow: Down spacing: 10 padding: Inset{left: 14 right: 14 top: 12 bottom: 14}",
            tok::SURFACE,
            12.0,
            Some(tok::HAIRLINE),
        );
        let s = &st.snap;
        pair(
            d,
            compact,
            ("b3_research_key", "research.key", tr("Lane key"), &s.key),
            ("b3_research_provider", "research.provider", tr("Provider identity"), &s.provider),
        );
        pair(
            d,
            compact,
            ("b3_research_model", "research.model", tr("Model"), &s.model),
            ("b3_research_api_type", "research.api_type", tr("API style"), &s.api_type),
        );
        pair(
            d,
            compact,
            ("b3_research_base_url", "research.base_url", tr("Base URL"), &s.base_url),
            ("b3_research_api_key_env", "research.api_key_env", tr("API key environment name"), &s.api_key_env),
        );
        let cred = credential().1.clone();
        masked_field(d, "b3_research_credential", "research.credential", tr(CREDENTIAL_LABEL), &cred);
        field(d, "b3_research_description", "research.description", tr("Description"), &s.description, W::Fill);
        pair(
            d,
            compact,
            ("b3_research_context_window", "research.context_window", tr("Context window (optional)"), &s.context_window),
            ("b3_research_max_output", "research.max_output_tokens", tr("Maximum output tokens (optional)"), &s.max_output_tokens),
        );
        // The footer: "Review lane save" (both variants; the live gate shows
        // one) and "Clear lane draft". The fieldset is disabled while busy or
        // while a confirmation is open (`:290`).
        let held = st.busy || st.confirm.is_some();
        let clear = if held { Btn::OutlineOff } else { Btn::Outline };
        let armed = if held || lock { Btn::Disabled } else { Btn::Primary };
        let fits = pill_w(tr("Clear lane draft")) + 8.0 + pill_w(tr("Review lane save")) <= inner_w;
        let h = if fits { 34.0 } else { 36.0 };
        d.gap(W::Fill, 2.0);
        pills_open(d, fits);
        if fits {
            d.button("b3_research_clear", tr("Clear lane draft"), "b3.research.clear", clear, W::Fit, h);
        }
        let both = d.anon();
        let review_w = if fits { format!("{}", pill_w(tr("Review lane save")).ceil()) } else { "Fill".to_owned() };
        d.view(&both, &format!("width: {review_w} height: {h} flow: Overlay"));
        d.view("b3_research_review_off", "width: Fill height: Fit flow: Right");
        d.button("b3_research_review_disabled", tr("Review lane save"), "b3.research.review", Btn::Disabled, W::Fill, h);
        d.close();
        d.view("b3_research_review_on", "width: Fill height: Fit flow: Right");
        d.button("b3_research_review", tr("Review lane save"), "b3.research.review", armed, W::Fill, h);
        d.close();
        d.close();
        if !fits {
            // The phone sheet: the primary on top, full width.
            d.button("b3_research_clear", tr("Clear lane draft"), "b3.research.clear", clear, W::Fill, h);
        }
        d.close();
        d.close();
    }
    ui::body_close(d);
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The credential slot is process-wide: tests that touch it run one at a time.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static L: Mutex<()> = Mutex::new(());
        L.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn store_with_lane() -> Store {
        let store = Store::new();
        store.domains.config.set_supported_methods(
            [SubProvidersList::NAME, SubProvidersUpsert::NAME, SubProvidersRemove::NAME].iter().map(|s| s.to_string()).collect(),
        );
        store.domains.profile.set_current("dsflash".into());
        store.domains.profile.set_sub_providers(vec![SubProvider {
            key: "web".into(),
            provider: "zhipu".into(),
            model: Some("glm-4-flash".into()),
            api_key_env: Some("ZHIPU_API_KEY".into()),
            base_url: None,
            description: Some("Web research".into()),
            default_context_window: None,
            max_output_tokens: None,
            api_type: Some("openai".into()),
        }]);
        store
    }

    #[test]
    fn the_draft_params_are_the_webs_whitelist() {
        let d = Draft { key: " web ".into(), provider: " zhipu ".into(), model: "glm-4-flash".into(), ..Default::default() };
        let p = serde_json::to_value(d.params().unwrap()).unwrap();
        assert_eq!(p, json!({"key": "web", "provider": "zhipu", "model": "glm-4-flash"}));
        assert!(Draft { key: "k".into(), ..Default::default() }.params().is_err(), "a provider is required");
        let bad = Draft { key: "k".into(), provider: "p".into(), context_window: "1.5".into(), ..Default::default() };
        assert!(bad.params().is_err(), "counts are whole u32s");
        let ok = Draft { key: "k".into(), provider: "p".into(), max_output_tokens: "8192".into(), ..Default::default() };
        assert_eq!(ok.params().unwrap().max_output_tokens, Some(8192));
    }

    #[test]
    fn the_strict_parse_is_the_webs() {
        let row = json!({"key": "web", "provider": "zhipu", "model": null, "api_key_env": null, "base_url": null,
                          "description": null, "api_type": null, "default_context_window": null, "max_output_tokens": 4096});
        assert!(lanes_ok(&json!({"profile_id": "p", "sub_providers": [row.clone()]}), "p"));
        assert!(!lanes_ok(&json!({"profile_id": "q", "sub_providers": [row.clone()]}), "p"), "the Profile must echo");
        assert!(!lanes_ok(&json!({"profile_id": "p", "sub_providers": [row.clone(), row.clone()]}), "p"), "unique keys");
        let mut missing = row.clone();
        missing.as_object_mut().unwrap().remove("model");
        assert!(!lanes_ok(&json!({"profile_id": "p", "sub_providers": [missing]}), "p"), "every field present");
        assert_eq!(
            receipt_ok(&json!({"profile_id": "p", "sub_providers": [], "applied": true, "restart_required": false}), "p"),
            Some((true, false))
        );
        assert_eq!(receipt_ok(&json!({"profile_id": "p", "sub_providers": [], "applied": true}), "p"), None);
    }

    #[test]
    fn the_credential_never_enters_the_draft_and_is_taken_once() {
        let _s = serial();
        let mut st = ResearchState::default();
        input_changed(&mut st, "research.credential", "not-a-real-credential");
        assert_eq!(st.draft, Draft::default(), "the draft never sees it");
        assert_eq!(take_credential().as_deref(), Some("not-a-real-credential"));
        assert_eq!(take_credential(), None, "taken at dispatch, then gone");
        input_changed(&mut st, "research.credential", "x");
        st.confirm = Some(Confirm::Remove("web".into()));
        perform(&mut st, "b3.research.cancel", 0, &Store::new());
        assert_eq!(take_credential(), None, "cancel clears it");
    }

    #[test]
    fn the_dialog_lowers_balanced_and_gates_its_mutations() {
        let _s = serial();
        let store = store_with_lane();
        let mut st = ResearchState { loaded: true, ..Default::default() };
        let mut d = Dsl::new();
        build(&mut d, &st, &Frame::DESKTOP, &store);
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        assert!(dsl.contains("is_password: true"), "the credential input is masked");
        assert!(dsl.contains("zhipu · glm-4-flash") && dsl.contains("API style: openai"));
        let taps: Vec<String> = crate::screens::taps::wired_taps(&dsl).into_iter().map(|(_, e)| e).collect();
        for want in [
            "b3.research.edit#0",
            "b3.research.remove#0",
            "b3.research.review",
            "b3.research.clear",
            "b3.research.refresh",
            "b3.close",
        ] {
            assert!(taps.iter().any(|t| t == want), "{want}: {taps:?}");
        }
        // Remove asks first; Confirm dispatches with the current generation.
        assert_eq!(perform(&mut st, "b3.research.remove", 0, &store), Outcome::Done);
        assert_eq!(st.confirm, Some(Confirm::Remove("web".into())));
        assert_eq!(perform(&mut st, "b3.research.confirm", 0, &store), Outcome::Spawn(Job::ResearchRemove(0, "web".into())));
        st.confirm = None;
        // Locked (a turn runs, or the lease is held): no mutation routes.
        st.turn_busy = true;
        assert_eq!(perform(&mut st, "b3.research.remove", 0, &store), Outcome::Done);
        assert!(st.confirm.is_none());
        st.turn_busy = false;
        store.domains.profile.set_profile_busy(true);
        let mut d = Dsl::new();
        build(&mut d, &st, &Frame::DESKTOP, &store);
        let locked_dsl = d.finish();
        assert!(locked_dsl.contains(LOCKED));
        let taps: Vec<String> = crate::screens::taps::wired_taps(&locked_dsl).into_iter().map(|(_, e)| e).collect();
        assert!(!taps
            .iter()
            .any(|t| t.starts_with("b3.research.edit") || t.starts_with("b3.research.remove") || t == "b3.research.review"));
        store.domains.profile.set_profile_busy(false);
        // Edit fills the draft (and the snapshot the inputs mount with).
        perform(&mut st, "b3.research.edit", 0, &store);
        assert_eq!(st.draft.key, "web");
        assert_eq!(st.snap.api_key_env, "ZHIPU_API_KEY");
        // Review asks; the fieldset is held while the confirmation is open.
        assert_eq!(perform(&mut st, "b3.research.review", 0, &store), Outcome::Done);
        assert!(matches!(st.confirm, Some(Confirm::Save(_))));
        assert_eq!(perform(&mut st, "b3.research.clear", 0, &store), Outcome::Done);
        assert_eq!(st.draft.key, "web", "Clear is disabled while a confirmation is open");
    }

    #[test]
    fn the_phone_sheet_stacks_the_fields() {
        let _s = serial();
        let store = store_with_lane();
        let st = ResearchState { loaded: true, ..Default::default() };
        let mut d = Dsl::new();
        build(&mut d, &st, &Frame { avail_w: 360.0, avail_h: 776.0 }, &store);
        let dsl = d.finish();
        assert!(dsl.contains("flow: Down spacing: 10"), "pairs stack on a phone");
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
    }
}
