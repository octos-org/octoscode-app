//! A21 — the pre-connection bootstrap (parity row 199) and the native
//! connection envelope (row 195, the web's tab / durable split).
//!
//! ## The web (`src-web/apps/web/src`)
//! `features/connection/connection-bootstrap.ts` decides, before the product
//! shell loads, what the first paint needs:
//! - the default endpoint: `VITE_OCTOS_DEFAULT_ENDPOINT`, else the page origin
//!   (`:11-15`) — a PREFILL of the connect form (`initialConnection`,
//!   `:17-23`: that endpoint, no token, no profile, no workspace);
//! - whether the tab talks to a server before the operator says anything:
//!   `autoStartKind` (`:44-48`) is `restore` only when the tab was already
//!   connected (its `autoConnect` marker, `preferences.ts:66`, set once the
//!   connection authenticated, `App.tsx:848-852`) and no pairing link drives
//!   the connect; otherwise null — the operator presses Connect
//!   (`ConnectionGate.tsx:164-176`, `App.tsx:814-845`).
//! A Disconnect clears the marker but keeps the tab's identity
//! (`ConnectionGate.tsx:308-318`); an identity change replaces the tab
//! envelope (`:266-307`); Forget clears both storage scopes (`:319-356`).
//!
//! ## Natively
//! The app window is the web's one tab, and a relaunch is that tab's refresh
//! ("Refresh is now a recovery event rather than a logout", web ADR 0017;
//! `architecture.md:150-157`): the envelope — A1's token for the remembered
//! server, the `auto-connect` marker beside it, A19's restore hints, the tab
//! drafts — survives a relaunch, and ONLY that one identity's; the durable
//! part is the server address (A1's `last-server`). So at launch
//! ([`boot`], run first by `OctoscodeView::start`):
//! - a remembered server whose connection was allowed to restore (the marker,
//!   or — for a device the previous build used, which dialed its server at
//!   every launch — a saved token or a remembered Session) RESTORES itself:
//!   the transport dials it with its token and A19's plan opens the
//!   remembered Session ([`AutoStart::Restore`]);
//! - otherwise NOTHING is dialed: the Connect card shows the remembered
//!   server (and its token), else the default endpoint — `OCTOS_BASE_URL`,
//!   the build's `VITE_OCTOS_DEFAULT_ENDPOINT`, else the built-in
//!   [`BUILTIN_ENDPOINT`] (a native app has no page origin);
//! - a launch pairing link (`OCTOS_PAIRING_LINK`) drives its own connect and
//!   nothing races it.
//!
//! ## The TEST-ONLY harness start (no web equivalent)
//! The web never dials from configuration: its suites fill the form and
//! press Connect (`e2e/product.spec.ts:18-31` connectServer,
//! `e2e/connection-storage.spec.ts:7-23`) or reload a tab that was connected
//! (`product.spec.ts:1708-1711`). The native click walks, judge tours and the
//! live gate instead hand the app a server AND a harness identity in its
//! environment — `OCTOS_BEARER` (a token) or `OCTOS_PROFILE_ID` (A19's
//! dev/test profile override, `screens::launch::Start::Explicit`). Neither is
//! ever set by the product, so [`AutoStart::Harness`] — dial `OCTOS_BASE_URL`
//! at launch, as every build did before — is taken only then.

/// The built-in default endpoint (the Connect card's authored address,
/// `screens::connect::ConnectUi::default`).
pub const BUILTIN_ENDPOINT: &str = "http://127.0.0.1:50190";

/// What a launch does on its own (`autoStartKind`).
#[derive(Clone, PartialEq, Eq)]
pub enum AutoStart {
    /// `restore`: dial the remembered server with its own token.
    Restore { server: String, token: String },
    /// TEST-ONLY: a harness handed over a server plus a credential/profile.
    Harness { server: String, token: String },
    /// null: the Connect card; the operator presses Connect.
    None,
}

/// Never prints a token.
impl std::fmt::Debug for AutoStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutoStart::Restore { server, token } => write!(f, "Restore({server}, token: {})", !token.is_empty()),
            AutoStart::Harness { server, token } => write!(f, "Harness({server}, token: {})", !token.is_empty()),
            AutoStart::None => write!(f, "None"),
        }
    }
}

/// The facts the decision reads ([`Inputs::current`] reads the real ones).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Inputs {
    /// `OCTOS_PAIRING_LINK` (A2's launch link, the web's `?octos=&pair=`).
    pub pairing_link: bool,
    /// `OCTOS_BASE_URL`: the build/test default endpoint.
    pub env_base: Option<String>,
    /// `OCTOS_BEARER`: a harness's token.
    pub env_bearer: Option<String>,
    /// `OCTOS_PROFILE_ID` is set (A19's dev/test override).
    pub env_profile: bool,
    /// The remembered server (A1's `last-server`).
    pub remembered: Option<String>,
    /// Its saved token.
    pub token: Option<String>,
    /// A19 remembers a Session to restore on it.
    pub restore_target: bool,
    /// The tab's auto-connect marker (`None`: never written).
    pub auto_connect: Option<bool>,
}

fn set(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

impl Inputs {
    /// The production facts: the launch environment and the stores.
    pub fn current() -> Inputs {
        let env = |k: &str| set(std::env::var(k).ok());
        let remembered = crate::credentials::last_server();
        Inputs {
            pairing_link: env("OCTOS_PAIRING_LINK").is_some(),
            env_base: env("OCTOS_BASE_URL"),
            env_bearer: env("OCTOS_BEARER"),
            env_profile: env("OCTOS_PROFILE_ID").is_some(),
            token: remembered.as_deref().and_then(crate::credentials::token_for),
            restore_target: remembered.as_deref().and_then(super::remembered::load).is_some(),
            auto_connect: crate::credentials::auto_connect(),
            remembered,
        }
    }
}

/// `autoStartKind` natively (see the module doc for the order).
pub fn decide(i: &Inputs) -> AutoStart {
    // A pairing link drives its own connect; nothing else may race it
    // (`connection-bootstrap.ts:45`).
    if i.pairing_link {
        return AutoStart::None;
    }
    // TEST-ONLY: a harness's server plus its identity.
    if let Some(base) = set(i.env_base.clone()) {
        let bearer = set(i.env_bearer.clone());
        if bearer.is_some() || i.env_profile {
            return AutoStart::Harness { server: base, token: bearer.unwrap_or_default() };
        }
    }
    // `restore`: only a connection this app already had, still allowed.
    let Some(server) = set(i.remembered.clone()) else { return AutoStart::None };
    let allowed = match i.auto_connect {
        Some(on) => on,
        None => i.token.is_some() || i.restore_target,
    };
    if allowed {
        AutoStart::Restore { server, token: i.token.clone().unwrap_or_default() }
    } else {
        AutoStart::None
    }
}

/// `defaultEndpoint()` (`connection-bootstrap.ts:11-15`).
pub fn default_endpoint() -> String {
    set(std::env::var("OCTOS_BASE_URL").ok()).unwrap_or_else(|| BUILTIN_ENDPOINT.to_owned())
}

/// The launch: the Connect card's initial values and what starts on its own.
pub struct Boot {
    /// The card's server: the remembered one, else the default endpoint.
    pub server: String,
    /// Its token (the tab's own; never another origin's).
    pub token: Option<String>,
    pub auto: AutoStart,
}

/// The bootstrap `OctoscodeView::start` runs before anything else touches a
/// server: the former device memory is purged (`ConnectionGate.tsx:204-206`),
/// the card's initial values read, the auto-start decided.
pub fn boot() -> Boot {
    crate::credentials::purge_device_memory();
    let i = Inputs::current();
    let auto = decide(&i);
    let server = i.remembered.clone().unwrap_or_else(default_endpoint);
    makepad_widgets::log!(
        "[octoscode] bootstrap: {auto:?} (remembered server: {}, saved token: {}, restore target: {}, auto-connect: {:?})",
        i.remembered.is_some(),
        i.token.is_some(),
        i.restore_target,
        i.auto_connect
    );
    Boot { server, token: i.token, auto }
}

/// A Connect pressed for (`server`, `token`) — the web's `changeConnection`
/// + `requestConnect` (`ConnectionGate.tsx:266-268`): the tab's auto-connect
/// is off until this connection authenticates. (The identity switch itself —
/// the previous server's token and restore hints dropped — is A1's
/// `remember_server` and A19's `on_connect_identity` on the same path; the
/// tab drafts follow at the new connection, `screens::drafts`.)
pub fn adopt_identity(server: &str, _token: &str) {
    let _ = server;
    noted(None);
    crate::credentials::set_auto_connect(false);
}

/// The origin whose authentication was already recorded (`sync_labels`
/// asks every frame; the stores are read once per connection).
static NOTED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn noted(v: Option<String>) {
    *NOTED.lock().unwrap_or_else(|p| p.into_inner()) = v;
}

/// The connection to `server` is authenticated (its socket is up): the tab
/// may restore it at the next launch (`App.tsx:848-852`). Only for the
/// remembered server (a harness's server is never remembered). Idempotent.
pub fn note_authenticated(server: &str) {
    let origin = crate::credentials::origin(server);
    if origin.is_none() || *NOTED.lock().unwrap_or_else(|p| p.into_inner()) == origin {
        return;
    }
    noted(origin);
    let same = crate::credentials::last_server()
        .and_then(|s| crate::credentials::origin(&s))
        .is_some_and(|o| crate::credentials::origin(server).as_deref() == Some(o.as_str()));
    if same && crate::credentials::auto_connect() != Some(true) {
        crate::credentials::set_auto_connect(true);
        makepad_widgets::log!("[octoscode] bootstrap: the connection authenticated — it restores at the next launch");
    }
}

/// Disconnect (`ConnectionGate.tsx:308-318`): no unattended restore; the
/// server and its token stay for the Connect card.
pub fn note_disconnect() {
    noted(None);
    if crate::credentials::last_server().is_some() {
        crate::credentials::set_auto_connect(false);
        makepad_widgets::log!("[octoscode] bootstrap: disconnected — the next launch shows the Connect card");
    }
}

/// Forget (`ConnectionGate.tsx:319-356`, beyond A9's address + token):
/// the confirmed principal's durable drafts (`clearDurableDrafts`), the tab
/// drafts and principal (`clearConnectionPreferences`), every origin's saved
/// token (`clearRememberedTokens`), every restore hint and the auto-connect
/// marker. Back to the initial draft.
pub fn forget_tab(server: &str) {
    let _ = server;
    noted(None);
    let remembered = crate::drafts::principal_scope();
    super::drafts::forget(remembered.as_ref().map(|(o, p)| (o.as_str(), p.as_str())));
    crate::drafts::forget();
    crate::credentials::forget_all_tokens();
    super::remembered::forget_all();
    crate::credentials::clear_auto_connect();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remembered(token: Option<&str>, target: bool, marker: Option<bool>) -> Inputs {
        Inputs {
            remembered: Some("http://127.0.0.1:50190".into()),
            token: token.map(str::to_owned),
            restore_target: target,
            auto_connect: marker,
            ..Default::default()
        }
    }

    #[test]
    fn only_an_allowed_remembered_connection_restores() {
        let restore = |t: &str| AutoStart::Restore { server: "http://127.0.0.1:50190".into(), token: t.into() };
        // The previous build's device (no marker): a token or a Session.
        assert_eq!(decide(&remembered(Some("t"), false, None)), restore("t"));
        assert_eq!(decide(&remembered(None, true, None)), restore(""));
        assert_eq!(decide(&remembered(None, false, None)), AutoStart::None, "an address alone is a prefill");
        // The marker decides once written.
        assert_eq!(decide(&remembered(Some("t"), true, Some(false))), AutoStart::None);
        assert_eq!(decide(&remembered(None, false, Some(true))), restore(""), "an authenticated tokenless server");
        assert_eq!(decide(&Inputs::default()), AutoStart::None, "nothing remembered: the Connect card");
    }

    #[test]
    fn the_harness_start_is_test_only_and_the_pairing_link_wins() {
        let base = Some("http://127.0.0.1:8480".to_owned());
        let h = |t: &str| AutoStart::Harness { server: "http://127.0.0.1:8480".into(), token: t.into() };
        assert_eq!(decide(&Inputs { env_base: base.clone(), env_bearer: Some("b".into()), ..Default::default() }), h("b"));
        assert_eq!(decide(&Inputs { env_base: base.clone(), env_profile: true, ..Default::default() }), h(""));
        assert_eq!(decide(&Inputs { env_base: base.clone(), env_bearer: Some("  ".into()), ..Default::default() }), AutoStart::None);
        assert_eq!(decide(&Inputs { env_bearer: Some("b".into()), ..Default::default() }), AutoStart::None, "no server, no dial");
        assert_eq!(
            decide(&Inputs { pairing_link: true, env_base: base, env_bearer: Some("b".into()), ..remembered(Some("t"), true, Some(true)) }),
            AutoStart::None
        );
    }
}
