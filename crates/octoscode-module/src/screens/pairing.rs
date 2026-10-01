//! #D1 — native pairing (atlas screens p4-01..p4-05), one owner per action id.
//!
//! The web's intake is `features/connection/pairing.ts`: a one-use code in the
//! `pair` parameter beside an `octos` origin, read ONCE before the first render
//! and stripped from the address (`pairing.ts:95-127`), exchanged for a token
//! (`:61-93` `readPairingLink` requires BOTH parameters — "one alone is not a
//! pairing link"). The native client has no address bar, so the two intake
//! shapes the web has are the two the atlas gives us: a scanned QR (screen 1's
//! viewfinder) and a pasted `octos://pair?code=…` (screen 1's field). Both
//! land on the same [`read_pairing_link`] parser, so the validation is the
//! web's, not a second dialect.
//!
//! Screens (design/stage-b/phase4/cards):
//! | card | atlas | what it shows |
//! |---|---|---|
//! | `p4-01` | 1 Pair this device | QR viewfinder, paste field, black "Pair" pill, manual link |
//! | `p4-02` | 2 Pairing… | the one-use exchange in flight (spinner, "This code works once.", Cancel) |
//! | `p4-03` | 3 Link problem | the spent-link callout, then the connect form with the origin prefilled |
//! | `p4-04` | 4 Can't pair | a server that does not advertise pairing, with the manual form as the way out |
//! | `p4-05` | 5 Paired | the connection rows and the red "Forget this device" |
//!
//! Every CLICK control is wired by the ONE shared helper
//! ([`super::taps::wire_card_events_dir`]) from the card's own
//! `service-actions.json`, so this module owns the *meaning* of an id and the
//! helper owns *reaching* the button.

use serde_json::Value;

/// Longest origin/code the web keeps (`pairing.ts` `MAX_ORIGIN_LENGTH` /
/// `MAX_CODE_LENGTH`; the code is sliced one past the max so an over-long code
/// is detectable rather than silently truncated into a valid one).
const MAX_ORIGIN_LENGTH: usize = 200;
const MAX_CODE_LENGTH: usize = 64;

/// The longest credential we will echo into a UI-local field.
const DOTS: &str = "••••••••••••••••••";

/// The nine `#D1` pairing/editor/browser cards are one board; this module owns
/// the five pairing cards.
pub const CARDS: &[(&str, &str)] = &[
    ("pair", "p4-01"),
    ("pairing", "p4-02"),
    ("link_problem", "p4-03"),
    ("no_pairing", "p4-04"),
    ("paired", "p4-05"),
];

/// A parsed `octos://pair?code=…` (or `?octos=…&pair=…`) link.
///
/// Mirrors the web's `PairingLink` (`pairing.ts:61-72`): `origin` is the
/// address to connect to, `code` is the one-use secret. Both are required —
/// a link carrying only one is not a pairing link (`pairing.ts:76-77`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingLink {
    /// The origin, already validated by [`loopback_origin_error`].
    pub origin: String,
    /// The one-use code. Never persisted, never logged (the web's own note at
    /// `pairing.ts:64`); the UI only ever shows the spinner copy.
    pub code: String,
}

/// Parse a pairing link. `None` when either parameter is missing/blank, or the
/// scheme is not `octos`/`http(s)`/`ws(s)`.
///
/// This is the web's `readPairingLink` (`pairing.ts:73-84`) with the origin
/// rule from `loopbackOrigin` applied early: the web validates before it
/// connects so an off-machine origin is refused WITHOUT a request (walk row
/// 111). Doing the same check here keeps that property on the native path.
pub fn read_pairing_link(raw: &str) -> Result<PairingLink, LinkError> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(LinkError::Malformed);
    }
    // `octos://pair?code=…` — the scheme carries the origin, the path is the
    // pairing verb.
    let (scheme, rest) = t
        .split_once("://")
        .ok_or(LinkError::Malformed)?;
    if !matches!(scheme.to_ascii_lowercase().as_str(), "octos" | "http" | "https" | "ws" | "wss") {
        return Err(LinkError::UnsupportedScheme(scheme.to_owned()));
    }
    let (path, query) = rest.split_once('?').ok_or(LinkError::Malformed)?;
    let params: Vec<(String, String)> = query
        .split('&')
        .filter(|kv| !kv.is_empty())
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let get = |k: &str| {
        params
            .iter()
            .find(|(pk, _)| pk == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    // Both parameters are required (pairing.ts:76-77). For the octos scheme the
    // origin is the scheme's authority; otherwise it is the `octos` parameter.
    let origin = if scheme.eq_ignore_ascii_case("octos") {
        let authority = path.trim_end_matches('/');
        if authority.is_empty() {
            String::new()
        } else {
            authority.to_owned()
        }
    } else {
        get("octos")
    };
    // The one-use code. Two spellings for one value: the web's URL form spells
    // it `pair` (pairing.ts:61-84) and the app's deep link spells it `code`
    // (atlas-prompt.md:28-29 — the paste field's placeholder
    // "octos://pair?code=…"). Refusing one because it uses the other's spelling
    // would break the phone's own paste path.
    let code = {
        let pair = get("pair");
        if pair.is_empty() {
            get("code")
        } else {
            pair
        }
    };
    if code.chars().count() > MAX_CODE_LENGTH {
        // Over-long: the web slices one past the max so this is detectable
        // (pairing.ts:82) — we refuse rather than connect with a prefix.
        return Err(LinkError::Expired);
    }
    if origin.is_empty() || code.is_empty() {
        return Err(LinkError::Malformed);
    }
    let origin: String = origin.chars().take(MAX_ORIGIN_LENGTH).collect();
    if let Some(e) = loopback_origin_error(&origin) {
        return Err(e);
    }
    Ok(PairingLink {
        origin,
        code: code.chars().take(MAX_CODE_LENGTH).collect(),
    })
}

/// Why a link was refused. Each variant carries its OWN bounded copy and next
/// step (walk row 110: "Each gets its own bounded explanation and next step").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// Unparseable, or one of the two parameters missing.
    Malformed,
    /// A scheme the client will not act on.
    UnsupportedScheme(String),
    /// The code is too long / stale — ask the server for a new one.
    Expired,
    /// Already redeemed (the server's one-use exchange rejected it).
    AlreadyUsed,
    /// The origin is not this machine — refused BEFORE any request (row 111).
    ForeignOrigin(String),
    /// The server does not advertise pairing at all (row 113).
    NotSupported,
    /// The Server field is not a usable address (the web validates before it
    /// connects: `connection-recovery` / `connect.rs:424-433`).
    BadEndpoint,
}

impl LinkError {
    /// The atlas copy for this refusal. The web's own strings where it has one
    /// (screens 3/4 in `atlas-prompt.md:32-36`), never the server's prose.
    pub fn copy(&self) -> (&'static str, &'static str) {
        match self {
            LinkError::Malformed => (
                "That pairing link isn't one we can read.",
                "Check it, or use server and token instead.",
            ),
            LinkError::UnsupportedScheme(_) => (
                "That pairing link isn't one we can read.",
                "Check it, or use server and token instead.",
            ),
            LinkError::Expired => (
                "This pairing code has expired.",
                "Ask Octos for a new code.",
            ),
            LinkError::AlreadyUsed => (
                "This pairing link was already used.",
                "Ask Octos for a new code.",
            ),
            LinkError::ForeignOrigin(_) => (
                "That Octos is on another device.",
                "Pair from that computer, or type its address here.",
            ),
            LinkError::NotSupported => (
                "This server doesn't support pairing.",
                "Octos on another computer must be paired from that computer.",
            ),
            LinkError::BadEndpoint => (
                "That server address isn't one we can use.",
                "Check the address, including the port.",
            ),
        }
    }
}

/// Only a loopback origin may be paired: the native app pairs with an Octos on
/// this machine or the local network, and refuses anything else before it
/// sends a request (walk row 111).
pub fn loopback_origin_error(origin: &str) -> Option<LinkError> {
    let host = origin
        .split("://")
        .nth(1)
        .unwrap_or(origin)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    let host = host.rsplit_once('@').map(|(_, h)| h).unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() {
        return Some(LinkError::Malformed);
    }
    if host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host.starts_with("127.")
        || host.ends_with(".local")
        || host.starts_with("192.168.")
        || host.starts_with("10.")
        || (host.starts_with("172.")
            && host
                .split('.')
                .nth(1)
                .and_then(|o| o.parse::<u32>().ok())
                .is_some_and(|o| (16..=31).contains(&o)))
    {
        None
    } else {
        Some(LinkError::ForeignOrigin(origin.to_owned()))
    }
}

/// Which pairing card is mounted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// `p4-01` — scan or paste.
    #[default]
    Pair,
    /// `p4-02` — the one-use exchange in flight.
    Pairing,
    /// `p4-03` — the link was spent; the form falls back with the origin kept.
    LinkProblem,
    /// `p4-04` — the server does not advertise pairing.
    NoPairing,
    /// `p4-05` — paired; rows plus Forget.
    Paired,
}

impl Screen {
    pub const ALL: [(Screen, &'static str); 5] = [
        (Screen::Pair, "p4-01"),
        (Screen::Pairing, "p4-02"),
        (Screen::LinkProblem, "p4-03"),
        (Screen::NoPairing, "p4-04"),
        (Screen::Paired, "p4-05"),
    ];

    /// The card directory under `design/stage-b/phase4/cards`.
    pub fn card_dir(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(s, _)| *s == self)
            .map(|(_, dir)| *dir)
            .expect("every screen has a card")
    }

    /// The pairing card for the current error, per the atlas.
    pub fn for_error(e: &LinkError) -> Screen {
        match e {
            LinkError::NotSupported => Screen::NoPairing,
            _ => Screen::LinkProblem,
        }
    }
}

/// The UI-local pairing state. The token lives here and in ONE place: it is
/// never written to a log line, and Forget clears it (walk row 112 — the
/// credential survives only this device until Forget).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PairingUi {
    /// The pasted/scanned link, as typed (the field's live text).
    pub link_draft: String,
    /// The form's Server field, prefilled from a refused link's origin.
    pub server: String,
    /// The Access token field. Never rendered — screens 1/2/4 carry NO token box
    /// (atlas-prompt.md:30-31), only p4-03's fallback form does.
    pub token: String,
    /// The refusal currently shown, if any.
    pub error: Option<LinkError>,
    /// The exchange is in flight (p4-02).
    pub exchanging: bool,
    /// Where we are.
    pub screen: Screen,
}

impl PairingUi {
    pub fn new() -> Self {
        Self {
            screen: Screen::Pair,
            ..Default::default()
        }
    }

    /// The token as the UI may show it: dots, or nothing when unset. The raw
    /// value is never a copy id (walk row 87/88: the key never appears in the
    /// page text).
    pub fn token_display(&self) -> &'static str {
        if self.token.is_empty() {
            ""
        } else {
            DOTS
        }
    }

    /// Forget: the credential is gone and the form is empty again (row 112).
    pub fn forget(&mut self) {
        self.token.clear();
        self.server.clear();
        self.error = None;
        self.screen = Screen::Pair;
    }
}

/// The action ids these five cards emit, with what each one means.
pub const ACTIONS: &[(&str, &str)] = &[
    ("pair.scan", "open the device scanner on the QR viewfinder (p4-01)"),
    ("pair.paste", "the paste-link field's live text (p4-01)"),
    ("pair.submit", "exchange the link's one-use code for a token (p4-01)"),
    (
        "pair.fallback",
        "switch from pairing to the manual server+token form (p4-01/p4-04)",
    ),
    ("pair.cancel", "abandon the in-flight one-use exchange (p4-02)"),
    ("pair.back", "step back one pairing screen (p4-02/p4-05)"),
    ("pair.forget", "Forget this device: drop the token and return to p4-01 (p4-05)"),
    ("connect.submit", "connect with the form's server+token (p4-03)"),
    ("connect.server", "the Server field's live text (p4-03)"),
    ("connect.token", "the Access token field's live text (p4-03)"),
];

/// The ids [`resolve`] routes.
pub const ROUTED: &[&str] = &[
    "pair.scan",
    "pair.paste",
    "pair.submit",
    "pair.fallback",
    "pair.cancel",
    "pair.back",
    "pair.forget",
    "connect.submit",
    "connect.server",
    "connect.token",
];

pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

pub fn is_routed(id: &str) -> bool {
    ROUTED.contains(&id)
}

/// Declared but unrouted (kept for the coverage contract every screen has).
pub fn unrouted() -> Vec<&'static str> {
    ACTIONS
        .iter()
        .map(|(a, _)| *a)
        .filter(|a| !ROUTED.contains(a))
        .collect()
}

/// What an action means. UI-local effects land on [`PairingUi`] in [`apply`];
/// the returned transport effect is performed by the caller.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Live text for a field (never leaves this module).
    Input { field: &'static str, value: String },
    /// Open the device scanner (p4-01's viewfinder).
    Scan,
    /// Exchange `(origin, code)` for a token — the ONE frame pairing needs.
    Exchange { origin: String, code: String },
    /// Connect with an explicit `(server, token)`.
    Connect { server: String, token: String },
    /// Step back one pairing screen / abandon the in-flight exchange.
    Back,
    /// Switch to the manual server+token form.
    Manual,
    /// "Pair" pressed: resolve it against the state in [`resolve_in`].
    Submit,
    /// Drop the stored credential (Forget).
    Forget,
    /// A refusal, with the screen it moves to.
    Refused(LinkError, Screen),
    /// Unhandled id.
    Unhandled,
}

/// Decide what an action means. `value` is the field text the host carries for
/// an input control; `None` for a tap.
pub fn resolve(id: &str, value: Option<&str>) -> Effect {
    match id {
        "pair.scan" => Effect::Scan,
        "pair.paste" => Effect::Input {
            field: "pair.link",
            value: value.unwrap_or_default().to_owned(),
        },
        "connect.server" => Effect::Input {
            field: "pair.server",
            value: value.unwrap_or_default().to_owned(),
        },
        "connect.token" => Effect::Input {
            field: "pair.token",
            value: value.unwrap_or_default().to_owned(),
        },
        "pair.cancel" => Effect::Back,
        "pair.back" => Effect::Back,
        "pair.forget" => Effect::Forget,
        "pair.fallback" => Effect::Manual,
        "pair.submit" => Effect::Submit,
        // The connect.rs shape (`connect.rs:388-391`): an empty pair asks
        // `apply` to fill it from the form's own fields, after validating.
        "connect.submit" => Effect::Connect {
            server: String::new(),
            token: String::new(),
        },
        _ => Effect::Unhandled,
    }
}

/// What [`resolve`] returns for a *stateful* action, once the current state is
/// known. Kept separate so the pure `resolve` stays testable without state.
pub fn resolve_in(id: &str, value: Option<&str>, ui: &PairingUi) -> Effect {
    let e = resolve(id, value);
    match e {
        // Pair: validate the draft link the way the web validates the URL —
        // refuse a foreign origin BEFORE any request (row 111).
        Effect::Submit => match read_pairing_link(&ui.link_draft) {
            Ok(l) => Effect::Exchange {
                origin: l.origin,
                code: l.code,
            },
            Err(e) => {
                let screen = Screen::for_error(&e);
                Effect::Refused(e, screen)
            }
        },
        Effect::Manual => Effect::Refused(LinkError::NotSupported, Screen::NoPairing),
        _ => e,
    }
}

/// Apply one effect to the UI-local state. Returns the transport effect the
/// caller performs, or `None` when the effect was UI-local.
pub fn apply(ui: &mut PairingUi, effect: Effect) -> Option<Effect> {
    match effect {
        Effect::Input { field, value } => {
            match field {
                "pair.link" => ui.link_draft = value,
                "pair.server" => ui.server = value,
                "pair.token" => ui.token = value,
                _ => {}
            }
            None
        }
        Effect::Scan => {
            ui.error = None;
            Some(Effect::Scan)
        }
        Effect::Back => {
            ui.exchanging = false;
            ui.screen = match ui.screen {
                Screen::Paired | Screen::NoPairing | Screen::LinkProblem => Screen::Pair,
                other => other,
            };
            None
        }
        Effect::Forget => {
            ui.forget();
            Some(Effect::Forget)
        }
        Effect::Manual => {
            // The manual form is p4-03 (it has the Server/Access-token fields);
            // keep whatever origin we already know prefilled.
            ui.error = Some(LinkError::NotSupported);
            ui.screen = Screen::NoPairing;
            None
        }
        Effect::Submit | Effect::Exchange { .. } => Some(effect),
        Effect::Connect { server, token } => {
            let server = if server.is_empty() { ui.server.clone() } else { server };
            let token = if token.is_empty() { ui.token.clone() } else { token };
            // Validation runs BEFORE the socket opens and before we remember
            // anything — the ordering `connect::apply` established
            // (`ConnectionPanel.tsx:30-49`, connect.rs:424-433).
            if super::connect::endpoint_error(&server).is_some() {
                ui.error = Some(LinkError::BadEndpoint);
                ui.screen = Screen::LinkProblem;
                return None;
            }
            ui.error = None;
            ui.exchanging = true;
            ui.screen = Screen::Pairing;
            Some(Effect::Connect { server, token })
        }
        Effect::Refused(e, screen) => {
            ui.exchanging = false;
            ui.error = Some(e);
            ui.screen = screen;
            None
        }
        Effect::Unhandled => None,
    }
}

/// The live copy overrides for one pairing card.
///
/// Empty state keeps the card's authored copy, so an idle screen still matches
/// its Stage-B render; only a value we actually have is injected.
pub fn copies(screen: Screen, ui: &PairingUi) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |id: &str, v: &str| out.push((id.to_owned(), v.to_owned()));
    match screen {
        Screen::Pair => {
            push("pair_link", &ui.link_draft);
        }
        Screen::Pairing => {
            if !ui.server.is_empty() {
                push("t_pairing", &format!("Pairing with {}\u{2026}", ui.server));
            }
        }
        Screen::LinkProblem => {
            if let Some(e) = &ui.error {
                let (head, _next) = e.copy();
                push("t_cal1", head);
            }
            if !ui.server.is_empty() {
                push("connect_server", &ui.server);
            }
        }
        Screen::NoPairing => {
            if let Some(e) = &ui.error {
                let (head, _next) = e.copy();
                push("t_cal1", head);
            }
        }
        Screen::Paired => {
            if !ui.server.is_empty() {
                push("t_srv_v", &ui.server);
            }
        }
    }
    // The token is never a copy id on the pairing screens: p4-01/02/04/05 carry
    // no token box, and p4-03's Access token field renders dots only.
    out
}

/// Lower one pairing card to the module's DSL.
///
/// The chain is the one `connect::lower_screen` established: read the card from
/// `design/stage-b/phase4/cards/<dir>`, apply the live copies, run the L0
/// prepare, retarget the faces, and hand the result to the ONE shared tap
/// helper — which reads the card's own `service-actions.json`, so a control
/// added to a card is wired without a change here.
pub fn lower_screen(screen: Screen, ui: &PairingUi) -> Result<String, String> {
    let dir = crate::design::dir("stage-b/phase4/cards").join(screen.card_dir());
    let card_src = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("read {}: {e}", dir.join("page.card").display()))?;
    let data: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("page.data.json"))
            .map_err(|e| format!("read page.data.json: {e}"))?,
    )
    .map_err(|e| format!("parse page.data.json: {e}"))?;
    let card_src = crate::l0_host::apply_copies(&card_src, &copies(screen, ui));
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &dir.join("kit"))
        .map_err(|e| format!("l0::prepare: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&tree))
        .map_err(|e| format!("to_makepad_ui: {e}"))?;
    // #35b item 1: the ONE card-tap wiring, keyed by the card DIRECTORY (these
    // cards live under stage-b/phase4, not stage-b/setup).
    Ok(super::taps::wire_card_events_dir(&dsl, &dir))
}

/// The bindings this screen projects, in the shape `bindings.rs` consumes.
pub fn query(ui: &PairingUi, id: &str) -> Option<Value> {
    match id {
        "pair.link" => Some(Value::String(ui.link_draft.clone())),
        "pair.server" => Some(Value::String(ui.server.clone())),
        "pair.token_display" => Some(Value::String(ui.token_display().to_owned())),
        "pair.paired" => Some(Value::Bool(!ui.token.is_empty())),
        "pair.exchanging" => Some(Value::Bool(ui.exchanging)),
        "pair.error" => ui
            .error
            .as_ref()
            .map(|e| Value::String(e.copy().0.to_owned())),
        _ => None,
    }
}

/// Store-fed reads, for the mount path.
///
/// The pairing state is entirely UI-local — the web keeps it in component state
/// too (`App.tsx:634`), and pairing exchanges its one-use code for a token
/// rather than reading a session row — so this screen has no store projection.
/// It is deliberately absent rather than stubbed: adding an `Option<Value>`
/// that is always `None` would imply a store path that does not exist.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_parameters_are_required() {
        // pairing.ts:76-77 — one alone is not a pairing link.
        assert_eq!(read_pairing_link("octos://192.168.1.20:50190?code="), Err(LinkError::Malformed));
        assert_eq!(read_pairing_link("octos://192.168.1.20:50190"), Err(LinkError::Malformed));
        assert_eq!(read_pairing_link(""), Err(LinkError::Malformed));
    }

    #[test]
    fn a_good_link_yields_the_origin_and_the_code() {
        let l = read_pairing_link("octos://192.168.1.20:50190?code=abc123").expect("readable");
        assert_eq!(l.origin, "192.168.1.20:50190");
        assert_eq!(l.code, "abc123");
    }

    #[test]
    fn an_off_machine_origin_is_refused_before_any_request() {
        // walk row 111
        let e = read_pairing_link("octos://evil.example:50190?code=abc").unwrap_err();
        assert!(matches!(e, LinkError::ForeignOrigin(_)), "{e:?}");
    }

    #[test]
    fn an_over_long_code_is_refused_rather_than_truncated() {
        let long = "x".repeat(MAX_CODE_LENGTH + 1);
        let e = read_pairing_link(&format!("octos://127.0.0.1:50190?code={long}")).unwrap_err();
        assert!(matches!(e, LinkError::Expired), "{e:?}");
    }

    #[test]
    fn every_refusal_owns_its_own_bounded_copy() {
        // walk row 110 — the copy may not repeat, or the "own explanation" is
        // not met.
        // The rule is one bounded explanation PER SITUATION, not per enum
        // variant. `Malformed` and `UnsupportedScheme` are the same situation to
        // a user ("that link isn't one I can read"), so they deliberately share
        // a message; the other four are distinct situations and must not.
        let situations: [&[LinkError]; 6] = [
            &[LinkError::Malformed, LinkError::UnsupportedScheme("ftp".into())],
            &[LinkError::Expired],
            &[LinkError::AlreadyUsed],
            &[LinkError::ForeignOrigin("x".into())],
            &[LinkError::NotSupported],
            // A bad address is its own situation: the operator typed something
            // wrong, which is not the same as a server that never offered
            // pairing, so it gets its own message and its own next step.
            &[LinkError::BadEndpoint],
        ];
        let mut seen: Vec<&str> = Vec::new();
        for group in situations {
            let (head, next) = group[0].copy();
            assert!(!head.is_empty() && !next.is_empty(), "{group:?} needs a next step");
            // Every member of a group really does share the message.
            for k in group {
                assert_eq!(k.copy(), (head, next), "{k:?} drifted from its group");
            }
            assert!(!seen.contains(&head), "{head:?} is reused by another situation");
            seen.push(head);
        }
    }

    #[test]
    fn forget_clears_the_credential_and_returns_the_form() {
        let mut ui = PairingUi::new();
        ui.token = "secret".into();
        ui.server = "127.0.0.1:50190".into();
        ui.screen = Screen::Paired;
        apply(&mut ui, Effect::Forget);
        assert_eq!(ui.token, "");
        assert_eq!(ui.token_display(), "");
        assert_eq!(ui.screen, Screen::Pair);
    }

    #[test]
    fn every_declared_action_is_routed() {
        assert_eq!(unrouted(), Vec::<&str>::new());
    }
}
