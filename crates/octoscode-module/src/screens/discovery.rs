//! A11 — pairing discovery: a pairing-capable server this device has seen
//! offers itself ONCE on the first-run Connect card (walk row 114,
//! `e2e/pairing-link.spec.ts` "a pairing-capable server this browser has seen
//! offers itself once"; row 113 is its 404 twin).
//!
//! The web (`src-web/apps/web/src`):
//! - `app/ConnectionGate.tsx:236-252` — probe ONLY the last saved origin, once,
//!   and only when this tab has no credential (no pairing link in the address,
//!   no restored connection, no token);
//! - `features/connection/pairing.ts:221-237` `probePairingInfo` — one
//!   unauthenticated GET `<origin>/pair/info` on one loopback origin, never a
//!   port range; a 404 is "simply unsupported" and never a complaint, an
//!   unreachable or unreadable answer is silent;
//! - `features/connection/ConnectionPanel.tsx:178-194` — the offer: "Found
//!   Octos on {host}." and ONE button "Connect to {host}" (`role="status"`,
//!   never an alert); `ConnectionGate.tsx:358-366` — the button uses that one
//!   origin and the offer is gone; any identity change (the address or the
//!   token edited, Forget) also clears it (`:259-266`, `:316`).
//!
//! The native rules (each one a request NOT made, or an offer NOT shown):
//! - the remembered server is [`crate::credentials::last_server`] (the web's
//!   durable endpoint); nothing remembered → nothing to probe;
//! - no probe while a credential is at hand: a stored token for that origin
//!   ([`crate::credentials::token_for`]) or an `OCTOS_BEARER` launch, nor while
//!   a launch pairing link (`OCTOS_PAIRING_LINK`) is being claimed;
//! - only an http(s) origin on THIS computer
//!   ([`octoscode_client::pairing::loopback_origin`]) is probed — any other
//!   remembered address gets no request at all;
//! - at most ONE probe per app run and ONE offer (one origin);
//! - the offer is dismissed for the rest of the run once it is used, once the
//!   person edits the Server or Access token field (the web's identity
//!   change), or once a connection is live; an answer that lands after such a
//!   dismissal is dropped (latest-request-wins);
//! - Use: a server that needs a token (`pairing_required`) leads into A2's
//!   pairing flow (p4-01) with its origin in the form; a server that needs none
//!   connects at once, tokenless (the web's `useDiscoveredOrigin`). Routed by
//!   [`super::board1`] (`b1.open.discovered`).
use std::sync::{Mutex, MutexGuard, OnceLock};

use octoscode_client::pairing::{self as wire, PairingProbe};

/// The offer on the Connect card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The origin to use: the server's loopback echo, else the probed origin
    /// (`readInfo`, `pairing.ts:296-309`).
    pub origin: String,
    /// `host[:port]` — what the web prints (`discoveredLabel`,
    /// `ConnectionPanel.tsx:364-371`: "host:port is what the server printed").
    pub label: String,
    /// The server needs a token: Use leads into pairing, else it connects.
    pub pairing_required: bool,
}

impl Offer {
    /// "Found Octos on 127.0.0.1:50190." (`ConnectionPanel.tsx:181`).
    pub fn message(&self) -> String {
        format!("Found Octos on {}.", self.label)
    }

    /// "Connect to 127.0.0.1:50190" (`ConnectionPanel.tsx:190`).
    pub fn action(&self) -> String {
        format!("Connect to {}", self.label)
    }
}

/// Why no probe was made — each is a request that was NOT sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// A launch pairing link is being claimed instead.
    PairingLink,
    /// A credential is at hand (a stored token for that server, or a bearer
    /// handed over at launch).
    Credential,
    /// No server was remembered: there is nothing to offer.
    NoServer,
    /// The remembered server is not an http(s) origin on this computer.
    NotLoopback,
    /// The one probe of this run was already made.
    Once,
}

impl Skip {
    pub fn reason(self) -> &'static str {
        match self {
            Skip::PairingLink => "a pairing link is being claimed",
            Skip::Credential => "a credential is at hand",
            Skip::NoServer => "no remembered server",
            Skip::NotLoopback => "the remembered server is not an http(s) origin on this computer — no request",
            Skip::Once => "already probed once in this run",
        }
    }
}

/// The facts the decision reads ([`Inputs::current`] reads the real ones).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inputs {
    /// The remembered server address (`credentials::last_server`).
    pub remembered: Option<String>,
    /// A token is stored for that server's origin.
    pub stored_token: bool,
    /// The app was launched with a bearer token (`OCTOS_BEARER`).
    pub launch_bearer: bool,
    /// The app was launched with a pairing link (`OCTOS_PAIRING_LINK`).
    pub pairing_link: bool,
}

impl Inputs {
    /// The production facts: the credential store and the launch environment.
    pub fn current() -> Inputs {
        let remembered = crate::credentials::last_server();
        let stored_token = remembered
            .as_deref()
            .and_then(crate::credentials::token_for)
            .is_some();
        let set = |k: &str| std::env::var(k).map(|v| !v.trim().is_empty()).unwrap_or(false);
        Inputs {
            remembered,
            stored_token,
            launch_bearer: set("OCTOS_BEARER"),
            pairing_link: set("OCTOS_PAIRING_LINK"),
        }
    }
}

/// Which origin to probe, or why none (`ConnectionGate.tsx:236-245`, in the
/// web's order; the loopback gate is `probePairingInfo`'s, `pairing.ts:225-226`).
pub fn decide(i: &Inputs) -> Result<String, Skip> {
    if i.pairing_link {
        return Err(Skip::PairingLink);
    }
    if i.stored_token || i.launch_bearer {
        return Err(Skip::Credential);
    }
    let server = i
        .remembered
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(Skip::NoServer)?;
    wire::loopback_origin(server).ok_or(Skip::NotLoopback)
}

/// `host[:port]` of an origin, as `new URL(origin).host` prints it (the
/// default port dropped, an IPv6 host in brackets).
pub fn label(origin: &str) -> String {
    match url::Url::parse(origin) {
        Ok(u) => match (u.host_str(), u.port()) {
            (Some(h), Some(p)) => format!("{h}:{p}"),
            (Some(h), None) => h.to_owned(),
            _ => origin.to_owned(),
        },
        Err(_) => origin.to_owned(),
    }
}

/// One run's discovery state.
#[derive(Debug, Default)]
pub struct Discovery {
    /// The one decision of this run was taken (a probe is never repeated).
    started: bool,
    /// Bumped by every dismissal: an older probe's answer is dropped.
    generation: u64,
    /// The generation of the probe in flight.
    pending: Option<u64>,
    /// The offer on the card.
    offer: Option<Offer>,
    /// The label of the offer that appeared this run. Sticky: the card's
    /// mount key reads it, so a dismissal hides the offer IN PLACE instead of
    /// remounting the card under a field being typed into (the #32h lesson).
    appeared: Option<String>,
    /// Used or dismissed: nothing is offered again this run.
    closed: bool,
}

impl Discovery {
    /// Take the run's one decision: `Ok((origin, generation))` means probe
    /// that origin now. Any second call is [`Skip::Once`].
    pub fn begin(&mut self, i: &Inputs) -> Result<(String, u64), Skip> {
        if self.started {
            return Err(Skip::Once);
        }
        self.started = true;
        let origin = decide(i)?;
        self.pending = Some(self.generation);
        Ok((origin, self.generation))
    }

    /// The probe answered. Only a current answer from a pairing-capable server
    /// becomes the offer; a 404 or no answer offers nothing and says nothing.
    pub fn finish(&mut self, generation: u64, probe: &PairingProbe) -> Option<&Offer> {
        if self.closed || self.pending != Some(generation) {
            return None;
        }
        self.pending = None;
        if let PairingProbe::Available(info) = probe {
            let offer = Offer {
                origin: info.server_origin.clone(),
                label: label(&info.server_origin),
                pairing_required: info.pairing_required,
            };
            self.appeared = Some(offer.label.clone());
            self.offer = Some(offer);
        }
        self.offer.as_ref()
    }

    /// The person changed the identity (or a connection went live): the offer
    /// goes for the rest of the run and an answer still in flight is dropped.
    /// Returns whether an offer was showing.
    pub fn dismiss(&mut self) -> bool {
        self.closed = true;
        self.generation += 1;
        self.pending = None;
        self.offer.take().is_some()
    }

    /// The offer's button: hand the offer over once (then it is gone).
    pub fn take(&mut self) -> Option<Offer> {
        let offer = self.offer.take();
        if offer.is_some() {
            self.closed = true;
            self.generation += 1;
        }
        offer
    }

    pub fn offer(&self) -> Option<&Offer> {
        self.offer.as_ref()
    }

    pub fn appeared(&self) -> Option<&str> {
        self.appeared.as_deref()
    }

    /// A probe is in flight.
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
}

// ------------------------------------------------------------ the live state

/// The running app's discovery (one per process — the web's one per page
/// load). The `pairing::state` shape.
pub fn state() -> MutexGuard<'static, Discovery> {
    static STATE: OnceLock<Mutex<Discovery>> = OnceLock::new();
    STATE
        .get_or_init(|| Mutex::new(Discovery::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// The production entry (lib.rs `start`): decide once from the real facts and
/// probe off the UI thread. Returns the decision.
pub fn start_once() -> Result<String, Skip> {
    start(&Inputs::current())
}

/// [`start_once`] with explicit facts.
pub fn start(i: &Inputs) -> Result<String, Skip> {
    let decided = state().begin(i);
    match decided {
        Ok((origin, generation)) => {
            makepad_widgets::log!(
                "[octoscode] discovery: probing {origin}/pair/info (the remembered server, no credential)"
            );
            spawn_probe(origin.clone(), generation);
            Ok(origin)
        }
        Err(skip) => {
            if skip != Skip::Once {
                makepad_widgets::log!("[octoscode] discovery: no probe ({})", skip.reason());
            }
            Err(skip)
        }
    }
}

/// The one GET, on its own thread (it must not wait for the transport's
/// runtime: the first-run launch has none).
fn spawn_probe(origin: String, generation: u64) {
    let run = move || {
        let probe = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt.block_on(wire::probe_pairing_info(&origin)),
            Err(_) => PairingProbe::Unavailable,
        };
        settle(generation, &probe);
        makepad_widgets::SignalToUI::set_ui_signal();
    };
    if std::thread::Builder::new()
        .name("octoscode-discovery".into())
        .spawn(run)
        .is_err()
    {
        makepad_widgets::log!("[octoscode] discovery: could not start the probe");
    }
}

/// Fold a probe answer into the live state and say what happened.
pub fn settle(generation: u64, probe: &PairingProbe) -> Option<Offer> {
    let shown = state().finish(generation, probe).cloned();
    match (probe, &shown) {
        (PairingProbe::Available(_), Some(o)) => makepad_widgets::log!(
            "[octoscode] discovery: Found Octos on {} ({})",
            o.label,
            if o.pairing_required { "pairing required" } else { "no token needed" }
        ),
        (PairingProbe::Available(_), None) => {
            makepad_widgets::log!("[octoscode] discovery: answer dropped (dismissed meanwhile)")
        }
        (PairingProbe::Unsupported, _) => makepad_widgets::log!(
            "[octoscode] discovery: the server does not offer pairing (404) — nothing to offer"
        ),
        (PairingProbe::Unavailable, _) => {
            makepad_widgets::log!("[octoscode] discovery: no usable answer — nothing to offer")
        }
    }
    shown
}

/// The offer the card shows now.
pub fn offer() -> Option<Offer> {
    state().offer().cloned()
}

/// The offer label that appeared this run (the card's mount key).
pub fn appeared() -> Option<String> {
    state().appeared().map(str::to_owned)
}

/// The Server or Access token field was edited (the web's identity change).
pub fn note_identity_edit() {
    if state().dismiss() {
        makepad_widgets::log!("[octoscode] discovery: offer dismissed (the connection fields were edited)");
    }
}

/// A connection is live: the offer has nothing left to do.
pub fn note_live() {
    let mut s = state();
    if s.offer().is_some() || s.pending() {
        s.dismiss();
    }
}

/// The offer's button (`b1.open.discovered`): the offer, once.
pub fn take() -> Option<Offer> {
    state().take()
}

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_client::pairing::PairingInfo;

    fn seen(server: &str) -> Inputs {
        Inputs { remembered: Some(server.to_owned()), ..Inputs::default() }
    }

    fn capable(origin: &str, required: bool) -> PairingProbe {
        PairingProbe::Available(PairingInfo {
            product: "octos".into(),
            version: "2.0.3-rc.13".into(),
            pairing_required: required,
            server_origin: origin.into(),
        })
    }

    #[test]
    fn only_the_remembered_loopback_server_without_a_credential_is_probed() {
        // ConnectionGate.tsx:236-245 in the web's order.
        assert_eq!(decide(&seen("http://127.0.0.1:50190/")).as_deref(), Ok("http://127.0.0.1:50190"));
        assert_eq!(decide(&seen("http://localhost:8422")).as_deref(), Ok("http://localhost:8422"));
        assert_eq!(decide(&Inputs::default()), Err(Skip::NoServer));
        assert_eq!(decide(&seen("  ")), Err(Skip::NoServer));
        let mut with_token = seen("http://127.0.0.1:50190");
        with_token.stored_token = true;
        assert_eq!(decide(&with_token), Err(Skip::Credential));
        let mut bearer = seen("http://127.0.0.1:50190");
        bearer.launch_bearer = true;
        assert_eq!(decide(&bearer), Err(Skip::Credential));
        let mut link = seen("http://127.0.0.1:50190");
        link.pairing_link = true;
        assert_eq!(decide(&link), Err(Skip::PairingLink));
    }

    #[test]
    fn a_server_off_this_computer_gets_no_request() {
        // pairing.ts:225-226: a non-loopback origin is "unavailable" before
        // any request; natively the decision refuses it, so nothing is sent.
        for s in ["http://192.168.1.20:50190", "https://octos.example.com", "ftp://127.0.0.1:1", "http://user:pw@127.0.0.1:1"] {
            assert_eq!(decide(&seen(s)), Err(Skip::NotLoopback), "{s}");
        }
    }

    #[test]
    fn the_label_is_host_and_port_as_the_web_prints_it() {
        assert_eq!(label("http://127.0.0.1:50190"), "127.0.0.1:50190");
        assert_eq!(label("http://localhost"), "localhost");
        assert_eq!(label("https://localhost:443"), "localhost");
        assert_eq!(label("http://[::1]:8422"), "[::1]:8422");
    }

    #[test]
    fn one_probe_per_run_and_one_offer() {
        let mut d = Discovery::default();
        let (origin, g) = d.begin(&seen("http://127.0.0.1:8431")).unwrap();
        assert_eq!(origin, "http://127.0.0.1:8431");
        assert_eq!(d.begin(&seen("http://127.0.0.1:8431")), Err(Skip::Once), "never a second probe");
        let o = d.finish(g, &capable("http://127.0.0.1:8431", true)).cloned().unwrap();
        assert_eq!(o.message(), "Found Octos on 127.0.0.1:8431.");
        assert_eq!(o.action(), "Connect to 127.0.0.1:8431");
        assert!(o.pairing_required);
        assert_eq!(d.take(), Some(o), "the button hands the offer over once");
        assert_eq!(d.take(), None);
        assert_eq!(d.offer(), None, "used: it is gone");
        assert_eq!(d.appeared(), Some("127.0.0.1:8431"), "the mount key stays put");
    }

    #[test]
    fn a_skipped_decision_still_spends_the_run() {
        let mut d = Discovery::default();
        assert_eq!(d.begin(&Inputs::default()), Err(Skip::NoServer));
        assert_eq!(d.begin(&seen("http://127.0.0.1:8431")), Err(Skip::Once));
    }

    #[test]
    fn a_404_or_no_answer_offers_nothing_and_says_nothing() {
        for probe in [PairingProbe::Unsupported, PairingProbe::Unavailable] {
            let mut d = Discovery::default();
            let (_, g) = d.begin(&seen("http://127.0.0.1:8431")).unwrap();
            assert_eq!(d.finish(g, &probe), None);
            assert_eq!(d.appeared(), None);
        }
    }

    #[test]
    fn the_server_echo_names_the_offer() {
        // readInfo: the server's loopback echo wins over the probed spelling.
        let mut d = Discovery::default();
        let (_, g) = d.begin(&seen("http://localhost:8431")).unwrap();
        let o = d.finish(g, &capable("http://127.0.0.1:8431", false)).cloned().unwrap();
        assert_eq!(o.origin, "http://127.0.0.1:8431");
        assert!(!o.pairing_required);
    }

    #[test]
    fn an_edit_dismisses_the_offer_and_drops_a_late_answer() {
        // Dismissed while showing: gone for the run.
        let mut d = Discovery::default();
        let (_, g) = d.begin(&seen("http://127.0.0.1:8431")).unwrap();
        d.finish(g, &capable("http://127.0.0.1:8431", true));
        assert!(d.dismiss());
        assert_eq!(d.offer(), None);
        // Dismissed before the answer: the late answer is dropped.
        let mut d = Discovery::default();
        let (_, g) = d.begin(&seen("http://127.0.0.1:8431")).unwrap();
        assert!(!d.dismiss());
        assert_eq!(d.finish(g, &capable("http://127.0.0.1:8431", true)), None);
        assert_eq!(d.appeared(), None);
    }
}
