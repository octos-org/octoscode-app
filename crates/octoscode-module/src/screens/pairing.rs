//! #D1/#A2 — native pairing (atlas board 1 screens p4-01..p4-05), one owner
//! per action id.
//!
//! The web's intake is `features/connection/pairing.ts`: a one-use code in the
//! `pair` parameter beside an `octos` origin, exchanged ONCE for the server's
//! API token by an unauthenticated `POST <origin>/pair/claim` before any socket
//! exists (`pairing.ts:61-127`, `:185-205`). The wire half lives in
//! `octoscode_client::pairing` (the same contract, ported); this module owns
//! the five screens' state and meaning:
//!
//! | card | atlas | state |
//! |---|---|---|
//! | `p4-01` | 1 Pair this device | scan (camera scanner) or paste a link, then Pair |
//! | `p4-02` | 2 Pairing… | the one exchange in flight; Cancel abandons it (latest-request-wins) |
//! | `p4-03` | 3 Link problem | a refused link: its own bounded copy + the server/token form, origin prefilled |
//! | `p4-04` | 4 Can't pair | the server answered 404 — pairing not supported; "Use server and token" |
//! | `p4-05` | 5 Paired | the connection rows and "Forget this device" |
//!
//! The rules the web pins and this module keeps:
//! - both parameters are required (`pairing.ts:76-77`);
//! - only an http(s) origin on THIS computer is used, and any other origin is
//!   refused WITHOUT a request and is NOT prefilled (walk 111);
//! - every refusal kind has its own bounded headline and next step (walk 110);
//! - the code is never stored or logged: the paste field is cleared once the
//!   exchange is attempted, and `PairingLink` redacts itself in `Debug`;
//! - the paired token lives in memory for this app instance only and Forget
//!   removes it so the connect form returns empty (walk 112).
use octoscode_client::pairing::{self as wire, PairingErrorKind, PairingLink, PairingResult};
use serde_json::Value;

use super::board1_kit::{self as kit, Field, Text};
use super::board1::{Layout, Ui};

/// The nine `#D1` pairing/editor/browser cards are one board; this module owns
/// the five pairing cards.
pub const CARDS: &[(&str, &str)] = &[
    ("pair", "p4-01"),
    ("pairing", "p4-02"),
    ("link_problem", "p4-03"),
    ("no_pairing", "p4-04"),
    ("paired", "p4-05"),
];

/// Which pairing screen is showing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// `p4-01` — scan or paste.
    #[default]
    Pair,
    /// `p4-02` — the one-use exchange (and the connect it hands to) in flight.
    Pairing,
    /// `p4-03` — the link was refused; the form falls back with the origin kept.
    LinkProblem,
    /// `p4-04` — the server does not offer pairing.
    NoPairing,
    /// `p4-05` — the connection rows plus Forget.
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

    /// The Stage-B card directory under `design/stage-b/phase4/cards`.
    pub fn card_dir(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(s, _)| *s == self)
            .map(|(_, dir)| *dir)
            .expect("every screen has a card")
    }

    pub fn from_card(card: &str) -> Option<Screen> {
        Self::ALL.iter().find(|(_, d)| *d == card).map(|(s, _)| *s)
    }
}

/// Why the form fell back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// A typed pairing outcome (client-side refusal or the server's wire kind).
    Pairing(PairingErrorKind),
    /// What was pasted is not a pairing link at all (one parameter, or none).
    NotALink,
    /// The Server field is not a usable address (`connect::endpoint_error`).
    BadEndpoint(&'static str),
    /// The connect the claim handed to failed (the address answered nothing).
    ConnectFailed,
}

impl Problem {
    /// The board's headline + next step for this situation. One bounded pair
    /// per situation, never the server's prose (walk 110). The board's own
    /// copy is used verbatim where the atlas has it (p4-03 "already used",
    /// p4-04 "doesn’t support pairing").
    pub fn copy(&self) -> (&'static str, &'static str) {
        match self {
            Problem::Pairing(k) => match k {
                PairingErrorKind::CodeUnknown => (
                    "This pairing link was already used.",
                    "Ask Octos for a new code.",
                ),
                PairingErrorKind::CodeExpired => (
                    "This pairing link has expired.",
                    "Restart Octos on your computer for a fresh link.",
                ),
                PairingErrorKind::CodeLocked => (
                    "Too many pairing attempts.",
                    "Restart the Octos server, then pair again.",
                ),
                PairingErrorKind::CodeInvalid => (
                    "This pairing link isn’t complete.",
                    "Copy the whole link again from Octos.",
                ),
                PairingErrorKind::OriginNotLoopback => (
                    "This link points to another computer.",
                    "Pairing links only work for Octos on this computer.",
                ),
                PairingErrorKind::NotSupported => (
                    "This server doesn’t support pairing.",
                    "Octos on another computer must be paired from that computer.",
                ),
                PairingErrorKind::Unreachable => (
                    "Octos isn’t answering at that address.",
                    "Check that Octos is still running, then try again.",
                ),
            },
            Problem::NotALink => (
                "That isn’t a pairing link.",
                "Paste the whole link Octos printed, or enter the server and token.",
            ),
            Problem::BadEndpoint(why) => ("That server address can’t be used.", why),
            Problem::ConnectFailed => (
                "Paired, but the connection didn’t open.",
                "Check the server address, then connect with the token.",
            ),
        }
    }

    /// Which screen shows this problem (the atlas: a 404 is p4-04, everything
    /// else falls back to p4-03's form).
    pub fn screen(&self) -> Screen {
        match self {
            Problem::Pairing(PairingErrorKind::NotSupported) => Screen::NoPairing,
            _ => Screen::LinkProblem,
        }
    }
}

/// The paired credential's provenance, for p4-05's rows.
#[derive(Clone, PartialEq, Eq)]
pub struct Paired {
    /// The origin the claim answered with (`PairingClaim::server_origin`).
    pub origin: String,
    /// When the exchange succeeded (ms since the epoch).
    pub at_ms: u64,
}

impl std::fmt::Debug for Paired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Paired").field("origin", &self.origin).field("at_ms", &self.at_ms).finish()
    }
}

/// The UI-local pairing state. The token lives here and in ONE other place
/// (the connect form's draft it is handed to): it is never written to a log
/// line or a copy, and Forget clears it (walk 112).
#[derive(Clone, Default, PartialEq)]
pub struct PairingUi {
    /// The paste field's live text. Cleared once an exchange is attempted —
    /// the link carries the one-use code.
    pub link_draft: String,
    /// p4-03's Server field, prefilled from a refused link's origin.
    pub server: String,
    /// p4-03's Access token field (password input; never rendered as text).
    pub token: String,
    /// The refusal currently shown, if any.
    pub problem: Option<Problem>,
    /// The exchange (or the connect it handed to) is in flight (p4-02).
    pub exchanging: bool,
    /// Latest-request-wins: every exchange and every Cancel bumps this, so a
    /// late answer from an abandoned exchange is dropped (`pairing.ts` callers
    /// abort the in-flight request; the native seam is this generation).
    pub generation: u64,
    /// The host p4-02 names ("Pairing with 127.0.0.1…").
    pub pairing_host: String,
    /// Set once a claim succeeded: p4-05's rows.
    pub paired: Option<Paired>,
    /// The connected server, for p4-05 when the token came from the form.
    pub connected_server: String,
    /// A one-line note under the viewfinder (the platform has no scanner).
    pub scan_note: Option<&'static str>,
    /// Where we are.
    pub screen: Screen,
}

impl std::fmt::Debug for PairingUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairingUi")
            .field("screen", &self.screen)
            .field("problem", &self.problem)
            .field("exchanging", &self.exchanging)
            .field("server", &self.server)
            .field("token", &format_args!("<{} chars>", self.token.chars().count()))
            .field("link_draft", &format_args!("<{} chars>", self.link_draft.chars().count()))
            .finish()
    }
}

impl PairingUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget: the credential is gone and the form is empty again (walk 112).
    pub fn forget(&mut self) {
        self.token.clear();
        self.server.clear();
        self.link_draft.clear();
        self.paired = None;
        self.connected_server.clear();
        self.problem = None;
        self.exchanging = false;
        self.generation += 1;
        self.screen = Screen::Pair;
    }

    fn refuse(&mut self, problem: Problem, origin: Option<&str>) {
        self.exchanging = false;
        // Walk 109: the origin is prefilled. Walk 111: a refused foreign
        // address is NOT prefilled into the form.
        let foreign = problem == Problem::Pairing(PairingErrorKind::OriginNotLoopback);
        match origin {
            Some(o) if !foreign => self.server = wire::loopback_origin(o).unwrap_or_else(|| o.to_owned()),
            _ if foreign => self.server.clear(),
            _ => {}
        }
        self.screen = problem.screen();
        self.problem = Some(problem);
    }
}

/// The action ids the five screens emit, with what each one means.
pub const ACTIONS: &[(&str, &str)] = &[
    ("pair.scan", "open the device's QR scanner from the viewfinder (p4-01)"),
    ("pair.scanned", "a scanned QR text arrived (host: NativeQrScanned)"),
    ("pair.paste", "the paste-link field's live text (p4-01)"),
    ("pair.submit", "exchange the link's one-use code for a token (p4-01 Pair / Return)"),
    ("pair.fallback", "leave pairing for the server+token form (p4-01)"),
    ("pair.manual", "use server and token with the link's origin (p4-04)"),
    ("pair.cancel", "abandon the in-flight exchange (p4-02)"),
    ("pair.back", "the back chevron: one step back, or close"),
    ("pair.forget", "Forget this device: drop the token and return to the form (p4-05)"),
    ("connect.server", "the Server field's live text (p4-03)"),
    ("connect.token", "the Access token field's live text (p4-03)"),
    ("connect.submit", "connect with the form's server+token (p4-03 Connect / Return)"),
];

/// The ids [`apply`] routes.
pub const ROUTED: &[&str] = &[
    "pair.scan",
    "pair.scanned",
    "pair.paste",
    "pair.submit",
    "pair.fallback",
    "pair.manual",
    "pair.cancel",
    "pair.back",
    "pair.forget",
    "connect.server",
    "connect.token",
    "connect.submit",
];

pub fn is_action(id: &str) -> bool {
    ACTIONS.iter().any(|(a, _)| *a == id)
}

pub fn is_routed(id: &str) -> bool {
    ROUTED.contains(&id)
}

/// Declared but unrouted (kept for the coverage contract every screen has).
pub fn unrouted() -> Vec<&'static str> {
    ACTIONS.iter().map(|(a, _)| *a).filter(|a| !ROUTED.contains(a)).collect()
}

/// What the host must do after an action (everything else was UI-local).
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    /// POST the code (`octoscode_client::pairing::claim_pairing_code`), then
    /// hand the answer to [`finish_exchange`] with this generation.
    Exchange { link: PairingLink, generation: u64 },
    /// Connect with this server+token through the production connect path.
    Connect { server: String, token: String },
    /// Close pairing and show the server+token form, the origin prefilled.
    LeaveToForm { server: Option<String> },
    /// Open the platform QR scanner (`Cx::show_qr_scanner`).
    Scan,
    /// Drop the credential everywhere and disconnect.
    Forget,
    /// Close the pairing surface.
    Close,
}

/// Apply one action to the state. `value` carries an input's live text.
pub fn apply(ui: &mut PairingUi, action: &str, value: Option<&str>) -> Option<Out> {
    let v = || value.unwrap_or_default().to_owned();
    match action {
        "pair.paste" => {
            ui.link_draft = v();
            ui.scan_note = None;
            None
        }
        "pair.scanned" => {
            ui.link_draft = v();
            submit(ui)
        }
        "pair.scan" => {
            ui.scan_note = None;
            Some(Out::Scan)
        }
        "pair.submit" => submit(ui),
        "pair.fallback" => Some(Out::LeaveToForm { server: None }),
        "pair.manual" => Some(Out::LeaveToForm {
            server: (!ui.server.is_empty()).then(|| ui.server.clone()),
        }),
        "pair.cancel" => {
            cancel(ui);
            None
        }
        "pair.back" => match ui.screen {
            Screen::Pair | Screen::Paired => Some(Out::Close),
            Screen::Pairing => {
                cancel(ui);
                None
            }
            Screen::LinkProblem | Screen::NoPairing => {
                ui.problem = None;
                ui.screen = Screen::Pair;
                None
            }
        },
        "pair.forget" => {
            ui.forget();
            Some(Out::Forget)
        }
        "connect.server" => {
            ui.server = v();
            None
        }
        "connect.token" => {
            ui.token = v();
            None
        }
        "connect.submit" => {
            // Validation runs BEFORE the socket opens (`ConnectionPanel.tsx:30-49`,
            // the order `connect::apply` keeps).
            if let Some(why) = super::connect::endpoint_error(&ui.server) {
                ui.problem = Some(Problem::BadEndpoint(why));
                ui.screen = Screen::LinkProblem;
                return None;
            }
            Some(Out::Connect {
                server: ui.server.trim().to_owned(),
                token: ui.token.clone(),
            })
        }
        _ => None,
    }
}

fn cancel(ui: &mut PairingUi) {
    ui.generation += 1;
    ui.exchanging = false;
    ui.problem = None;
    ui.screen = Screen::Pair;
}

/// "Pair": read the draft the way the web reads its URL, refuse locally what
/// the web refuses before a request, else start the one exchange.
fn submit(ui: &mut PairingUi) -> Option<Out> {
    let draft = std::mem::take(&mut ui.link_draft);
    let Some(link) = wire::read_pairing_link(&draft) else {
        ui.refuse(Problem::NotALink, None);
        return None;
    };
    match wire::validate_link(&link) {
        Err(kind) => {
            ui.refuse(Problem::Pairing(kind), Some(&link.origin));
            None
        }
        Ok((origin, _)) => {
            ui.generation += 1;
            ui.exchanging = true;
            ui.problem = None;
            ui.pairing_host = host_of(&origin);
            ui.server = origin;
            ui.screen = Screen::Pairing;
            Some(Out::Exchange {
                link,
                generation: ui.generation,
            })
        }
    }
}

/// The exchange answered. A stale generation (Cancel, or a newer exchange)
/// is dropped. On success the token is handed to the connect path and p4-02
/// stays up until the connection is live (the web: "the workspace gate
/// appears with no token box").
pub fn finish_exchange(ui: &mut PairingUi, generation: u64, result: PairingResult) -> Option<Out> {
    if generation != ui.generation || !ui.exchanging {
        return None;
    }
    match result {
        Ok(claim) => {
            ui.paired = Some(Paired {
                origin: claim.server_origin.clone(),
                at_ms: now_ms(),
            });
            ui.server = claim.server_origin.clone();
            ui.token = claim.token.clone();
            ui.pairing_host = host_of(&claim.server_origin);
            Some(Out::Connect {
                server: claim.server_origin,
                token: claim.token,
            })
        }
        Err(kind) => {
            let origin = ui.server.clone();
            ui.refuse(Problem::Pairing(kind), Some(&origin));
            None
        }
    }
}

/// The connect a claim handed to failed: fall back to p4-03 with the origin.
pub fn connect_failed(ui: &mut PairingUi) {
    let origin = ui.server.clone();
    ui.refuse(Problem::ConnectFailed, Some(&origin));
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `http://127.0.0.1:8422` -> `127.0.0.1` (p4-02 names the host only).
pub fn host_of(origin: &str) -> String {
    url::Url::parse(origin)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.trim_matches(['[', ']']).to_owned()))
        .unwrap_or_else(|| origin.to_owned())
}

/// `http://127.0.0.1:8422` -> `127.0.0.1:8422` (p4-05's Server row).
pub fn host_port(origin: &str) -> String {
    origin
        .split("://")
        .nth(1)
        .unwrap_or(origin)
        .trim_end_matches('/')
        .to_owned()
}

/// p4-05's "Today, 9:41 PM" in the device's own time zone.
pub fn paired_when(at_ms: u64, now_ms: u64) -> String {
    use chrono::{Local, TimeZone};
    let Some(at) = Local.timestamp_millis_opt(at_ms as i64).single() else {
        return String::new();
    };
    let now = Local
        .timestamp_millis_opt(now_ms as i64)
        .single()
        .unwrap_or(at);
    let clock = at.format("%-I:%M %p").to_string();
    let days = (now.date_naive() - at.date_naive()).num_days();
    match days {
        0 => format!("Today, {clock}"),
        1 => format!("Yesterday, {clock}"),
        _ => format!("{}, {clock}", at.format("%b %-d")),
    }
}

// --------------------------------------------------------------------- views

/// The native view of the current pairing screen.
pub fn view(ui: &PairingUi, l: &Layout) -> Ui {
    let mut v = Ui::default();
    let title = if ui.screen == Screen::Paired { "Connection" } else { "Pair with Octos" };
    v.header(l, "b1_pair_back", "pair.back", title);
    match ui.screen {
        Screen::Pair => pair_view(ui, l, &mut v),
        Screen::Pairing => pairing_view(ui, l, &mut v),
        Screen::LinkProblem => problem_view(ui, l, &mut v),
        Screen::NoPairing => no_pairing_view(ui, l, &mut v),
        Screen::Paired => paired_view(ui, l, &mut v),
    }
    v
}

fn pair_view(ui: &PairingUi, l: &Layout, v: &mut Ui) {
    v.push(kit::gap(if l.phone { 28.0 } else { 16.0 }));
    // The viewfinder: a light-grey rounded square with four corner brackets
    // (atlas screen 1). The whole box is the scan control.
    let vf_w = (l.content_w * 0.78).min(288.0).round();
    let vf_h = if l.phone { (vf_w * 0.76).round().min(222.0) } else { 150.0 };
    let bracket = |file: &str, ax: f64, ay: f64| {
        format!(
            "View {{ width: Fill height: Fill align: Align{{x: {ax} y: {ay}}} padding: 16\n{}}}\n",
            kit::svg("", file, 24.0)
        )
    };
    v.push(format!(
        "View {{ width: Fill height: Fit align: Align{{x: 0.5 y: 0.0}}\nView {{ width: {vf_w} height: {vf_h} flow: Overlay\nDesignSurface {{ width: Fill height: Fill draw_bg.color: #f1f1f3ff draw_bg.radius: 12 draw_bg.border_width: 1 draw_bg.border_position: 1 draw_bg.border_color: {} }}\n{}{}{}{}{}}}\n}}\n",
        kit::HAIR,
        bracket("b1_vf_tl.svg", 0.0, 0.0),
        bracket("b1_vf_tr.svg", 1.0, 0.0),
        bracket("b1_vf_bl.svg", 0.0, 1.0),
        bracket("b1_vf_br.svg", 1.0, 1.0),
        kit::hit("b1_pair_scan", true),
    ));
    v.button("b1_pair_scan", "pair.scan");
    v.push(kit::gap(if l.phone { 16.0 } else { 12.0 }));
    v.push(Text::new("b1_pair_cap1", "Scan the pairing QR shown in Octos").px(14.0).color(kit::MUTED).fill().centered().one_line().dsl());
    v.push(kit::gap(2.0));
    v.push(Text::new("b1_pair_cap2", "on your computer").px(14.0).color(kit::MUTED).fill().centered().one_line().dsl());
    if let Some(note) = ui.scan_note {
        v.push(kit::gap(6.0));
        v.push(Text::new("b1_pair_scan_note", note).px(13.0).color(kit::FAINT).fill().centered().dsl());
    }
    v.push(kit::gap(if l.phone { 30.0 } else { 14.0 }));
    v.push(kit::or_divider());
    v.push(kit::gap(if l.phone { 26.0 } else { 12.0 }));
    v.push(
        Field::new("b1_pair_link", &ui.link_draft)
            .label("Paste pairing link")
            .placeholder("octos://pair?code=…")
            .dsl(),
    );
    v.input("b1_pair_link", "pair.paste");
    v.returns("b1_pair_link", "pair.submit");
    v.push(kit::gap(if l.phone { 32.0 } else { 18.0 }));
    v.push(kit::pill_primary("b1_pair_submit", "Pair", "Fill"));
    v.button("b1_pair_submit", "pair.submit");
    v.push(kit::gap(if l.phone { 16.0 } else { 8.0 }));
    v.push(centered(&kit::link(
        "b1_pair_fallback",
        "Enter server and token instead",
        kit::BLUE,
        14.0,
        500,
    )));
    v.button("b1_pair_fallback", "pair.fallback");
}

fn pairing_view(ui: &PairingUi, l: &Layout, v: &mut Ui) {
    let host = if ui.pairing_host.is_empty() { "Octos".to_owned() } else { ui.pairing_host.clone() };
    v.push(kit::gap(if l.phone { 160.0 } else { 52.0 }));
    v.push(format!(
        "View {{ width: Fill height: Fit align: Align{{x: 0.5 y: 0.0}}\n{}}}\n",
        kit::svg("b1_pair_spinner", "b1_spinner.svg", if l.phone { 56.0 } else { 48.0 })
    ));
    v.push(kit::gap(if l.phone { 34.0 } else { 20.0 }));
    let line = format!("Pairing with {host}\u{2026}");
    v.push(Text::new("b1_pairing_line", &line).px(17.0).fill().centered().one_line().dsl());
    v.push(kit::gap(if l.phone { 40.0 } else { 14.0 }));
    v.push(Text::new("b1_pairing_once", "This code works once.").px(15.0).color(kit::MUTED).fill().centered().one_line().dsl());
    v.spacer(l, 56.0, 96.0);
    v.push(centered(&kit::pill_outline("b1_pair_cancel", "Cancel", "136")));
    v.button("b1_pair_cancel", "pair.cancel");
}

fn problem_view(ui: &PairingUi, l: &Layout, v: &mut Ui) {
    let (head, next) = ui
        .problem
        .as_ref()
        .map(Problem::copy)
        .unwrap_or(("That pairing link didn’t work.", "Enter the server and token instead."));
    v.push(kit::gap(if l.phone { 24.0 } else { 16.0 }));
    v.push(kit::callout(true, false, head, Some(next)));
    v.push(kit::gap(if l.phone { 28.0 } else { 18.0 }));
    v.push(
        Field::new("b1_conn_server", &ui.server)
            .label("Server")
            .placeholder("http://127.0.0.1:50190")
            .dsl(),
    );
    v.input("b1_conn_server", "connect.server");
    v.push(kit::gap(if l.phone { 22.0 } else { 14.0 }));
    v.push(Field::new("b1_conn_token", &ui.token).label("Access token").password().dsl());
    v.input("b1_conn_token", "connect.token");
    v.returns("b1_conn_token", "connect.submit");
    v.spacer(l, 22.0, 64.0);
    v.push(kit::pill_primary("b1_conn_submit", "Connect", "Fill"));
    v.button("b1_conn_submit", "connect.submit");
    if l.phone {
        v.push(kit::gap(70.0));
    }
}

fn no_pairing_view(ui: &PairingUi, l: &Layout, v: &mut Ui) {
    let (head, next) = ui
        .problem
        .as_ref()
        .map(Problem::copy)
        .unwrap_or_else(|| Problem::Pairing(PairingErrorKind::NotSupported).copy());
    v.push(kit::gap(if l.phone { 24.0 } else { 16.0 }));
    v.push(kit::callout(false, true, head, None));
    v.push(kit::gap(14.0));
    v.push(kit::callout(false, false, next, None));
    v.spacer(l, 40.0, 64.0);
    v.push(kit::pill_primary("b1_pair_manual", "Use server and token", "Fill"));
    v.button("b1_pair_manual", "pair.manual");
    if l.phone {
        v.push(kit::gap(70.0));
    }
}

fn paired_view(ui: &PairingUi, l: &Layout, v: &mut Ui) {
    let server = ui
        .paired
        .as_ref()
        .map(|p| p.origin.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ui.connected_server.clone());
    let mut rows = vec![kit::kv_row("Server", &host_port(&server))];
    match &ui.paired {
        Some(p) => rows.push(kit::kv_row("Paired", &paired_when(p.at_ms, now_ms()))),
        None => rows.push(kit::kv_row("Signed in", "With an access token")),
    }
    rows.push(kit::note_row("Stays on this device only", kit::MUTED));
    v.push(kit::gap(if l.phone { 24.0 } else { 16.0 }));
    v.push(kit::list_card("b1_conn_rows", &rows));
    v.spacer(l, 40.0, 40.0);
    v.push(centered(&kit::link("b1_pair_forget", "Forget this device", kit::RED, 15.0, 500)));
    v.button("b1_pair_forget", "pair.forget");
    if l.phone {
        v.push(kit::gap(90.0));
    }
}

fn centered(inner: &str) -> String {
    format!("View {{ width: Fill height: Fit align: Align{{x: 0.5 y: 0.0}}\n{inner}}}\n")
}

/// The bindings this screen projects (the key itself never: only its mask).
pub fn query(ui: &PairingUi, id: &str) -> Option<Value> {
    match id {
        "pair.server" => Some(Value::String(ui.server.clone())),
        "pair.paired" => Some(Value::Bool(ui.paired.is_some())),
        "pair.exchanging" => Some(Value::Bool(ui.exchanging)),
        "pair.screen" => Some(Value::String(ui.screen.card_dir().to_owned())),
        "pair.error" => ui.problem.as_ref().map(|p| Value::String(p.copy().0.to_owned())),
        _ => None,
    }
}

// ------------------------------------------------------------- live state

/// The UI-local pairing state between taps (the web keeps it in component
/// state too, `App.tsx:634`). One `OnceLock`, the `workspace.rs:117-123` shape.
pub fn state() -> std::sync::MutexGuard<'static, PairingUi> {
    static STATE: std::sync::OnceLock<std::sync::Mutex<PairingUi>> = std::sync::OnceLock::new();
    STATE
        .get_or_init(|| std::sync::Mutex::new(PairingUi::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Replace the live pairing state.
pub fn set(ui: PairingUi) {
    *state() = ui;
}

/// Apply one action to the LIVE state (the production entry point).
pub fn perform(id: &str, value: Option<&str>) -> Option<Out> {
    apply(&mut state(), id, value)
}

// ------------------------------------------------- the Stage-B card (design)

/// The live copy overrides for one Stage-B pairing card (the design-flow
/// artifact; the app mounts [`view`]).
pub fn copies(screen: Screen, ui: &PairingUi) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    match screen {
        Screen::Pairing if !ui.pairing_host.is_empty() => {
            out.push(("t_pairing_text".into(), format!("Pairing with {}\u{2026}", ui.pairing_host)))
        }
        Screen::LinkProblem | Screen::NoPairing => {
            if let Some(p) = &ui.problem {
                out.push(("t_cal1_text".into(), p.copy().0.to_owned()));
            }
        }
        _ => {}
    }
    out
}

/// Lower one Stage-B pairing card (`design/stage-b/phase4/cards/<dir>`) — the
/// accepted design artifact, kept so the card pipeline's own checks still run.
/// The running app mounts the native [`view`] instead (board1.rs explains why).
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
    Ok(super::taps::wire_card_events_dir(&dsl, &dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_client::pairing::PairingClaim;

    fn good() -> &'static str {
        "http://app.invalid/?octos=http://127.0.0.1:8422&pair=3QK7ZP2M"
    }

    #[test]
    fn a_good_link_starts_one_exchange_and_clears_the_code_from_the_field() {
        let mut ui = PairingUi::new();
        apply(&mut ui, "pair.paste", Some(good()));
        let out = apply(&mut ui, "pair.submit", None);
        match out {
            Some(Out::Exchange { link, generation }) => {
                assert_eq!(link.code, "3QK7ZP2M");
                assert_eq!(generation, ui.generation);
            }
            other => panic!("expected an exchange, got {other:?}"),
        }
        assert_eq!(ui.screen, Screen::Pairing);
        assert_eq!(ui.pairing_host, "127.0.0.1");
        assert!(ui.link_draft.is_empty(), "the one-use code must not stay in the field");
    }

    #[test]
    fn one_parameter_alone_is_not_a_link() {
        let mut ui = PairingUi::new();
        apply(&mut ui, "pair.paste", Some("?octos=http://127.0.0.1:8422"));
        assert_eq!(apply(&mut ui, "pair.submit", None), None);
        assert_eq!(ui.problem, Some(Problem::NotALink));
        assert_eq!(ui.screen, Screen::LinkProblem);
    }

    #[test]
    fn a_foreign_origin_is_refused_without_a_request_and_not_prefilled() {
        // walk 111
        let mut ui = PairingUi::new();
        ui.server = "http://127.0.0.1:1".into();
        apply(&mut ui, "pair.paste", Some("?octos=http://192.168.1.20:50190&pair=3QK7ZP2M"));
        assert_eq!(apply(&mut ui, "pair.submit", None), None, "no exchange leaves");
        assert_eq!(ui.problem, Some(Problem::Pairing(PairingErrorKind::OriginNotLoopback)));
        assert_eq!(ui.server, "", "the refused address is not prefilled");
    }

    #[test]
    fn a_used_link_falls_back_to_the_form_with_the_origin_prefilled() {
        // walk 109
        let mut ui = PairingUi::new();
        apply(&mut ui, "pair.paste", Some(good()));
        let Some(Out::Exchange { generation, .. }) = apply(&mut ui, "pair.submit", None) else {
            panic!("exchange");
        };
        assert_eq!(finish_exchange(&mut ui, generation, Err(PairingErrorKind::CodeUnknown)), None);
        assert_eq!(ui.screen, Screen::LinkProblem);
        assert_eq!(ui.server, "http://127.0.0.1:8422");
        assert_eq!(ui.problem.as_ref().unwrap().copy().0, "This pairing link was already used.");
    }

    #[test]
    fn a_404_is_cant_pair_and_offers_server_and_token() {
        // walk 113
        let mut ui = PairingUi::new();
        apply(&mut ui, "pair.paste", Some(good()));
        let Some(Out::Exchange { generation, .. }) = apply(&mut ui, "pair.submit", None) else {
            panic!("exchange");
        };
        finish_exchange(&mut ui, generation, Err(PairingErrorKind::NotSupported));
        assert_eq!(ui.screen, Screen::NoPairing);
        assert_eq!(
            apply(&mut ui, "pair.manual", None),
            Some(Out::LeaveToForm { server: Some("http://127.0.0.1:8422".into()) })
        );
    }

    #[test]
    fn cancel_drops_a_late_answer() {
        let mut ui = PairingUi::new();
        apply(&mut ui, "pair.paste", Some(good()));
        let Some(Out::Exchange { generation, .. }) = apply(&mut ui, "pair.submit", None) else {
            panic!("exchange");
        };
        apply(&mut ui, "pair.cancel", None);
        let late = finish_exchange(
            &mut ui,
            generation,
            Ok(PairingClaim { token: "t".into(), server_origin: "http://127.0.0.1:8422".into() }),
        );
        assert_eq!(late, None, "an abandoned exchange may not connect");
        assert_eq!(ui.screen, Screen::Pair);
        assert!(ui.token.is_empty());
    }

    #[test]
    fn a_good_claim_hands_the_token_to_the_connect_path() {
        let mut ui = PairingUi::new();
        apply(&mut ui, "pair.paste", Some(good()));
        let Some(Out::Exchange { generation, .. }) = apply(&mut ui, "pair.submit", None) else {
            panic!("exchange");
        };
        let out = finish_exchange(
            &mut ui,
            generation,
            Ok(PairingClaim { token: "tok".into(), server_origin: "http://127.0.0.1:8422".into() }),
        );
        assert_eq!(out, Some(Out::Connect { server: "http://127.0.0.1:8422".into(), token: "tok".into() }));
        assert!(ui.paired.is_some());
        assert_eq!(ui.screen, Screen::Pairing, "p4-02 stays up until the connection is live");
    }

    #[test]
    fn every_situation_owns_its_own_bounded_copy() {
        // walk 110 — one headline per situation, each with a next step.
        let mut seen: Vec<&str> = Vec::new();
        let mut all: Vec<Problem> = PairingErrorKind::ALL.iter().map(|k| Problem::Pairing(*k)).collect();
        all.push(Problem::NotALink);
        all.push(Problem::BadEndpoint("Enter the address of your Octos server."));
        all.push(Problem::ConnectFailed);
        for p in &all {
            let (head, next) = p.copy();
            assert!(!head.is_empty() && !next.is_empty(), "{p:?} needs a next step");
            assert!(!seen.contains(&head), "{head:?} reused");
            seen.push(head);
        }
    }

    #[test]
    fn forget_clears_the_credential_and_returns_the_form() {
        let mut ui = PairingUi::new();
        ui.token = "secret".into();
        ui.server = "http://127.0.0.1:8422".into();
        ui.paired = Some(Paired { origin: ui.server.clone(), at_ms: 1 });
        ui.screen = Screen::Paired;
        assert_eq!(apply(&mut ui, "pair.forget", None), Some(Out::Forget));
        assert_eq!(ui.token, "");
        assert_eq!(ui.server, "");
        assert!(ui.paired.is_none());
        assert_eq!(ui.screen, Screen::Pair);
    }

    #[test]
    fn debug_never_prints_the_token_or_the_draft() {
        let mut ui = PairingUi::new();
        ui.token = "tok-secret".into();
        ui.link_draft = good().into();
        let d = format!("{ui:?}");
        assert!(!d.contains("tok-secret") && !d.contains("3QK7ZP2M"), "{d}");
    }

    #[test]
    fn every_declared_action_is_routed() {
        assert_eq!(unrouted(), Vec::<&str>::new());
    }

    #[test]
    fn paired_when_reads_today() {
        let now = 1_790_000_000_000u64;
        assert!(paired_when(now, now).starts_with("Today, "));
    }
}
