//! #A2 — board 1 (`design/stage-a/phase4-new`, operator-approved 2026-10-01)
//! as LIVE, reachable native surfaces: pairing (p4-01..05), the provider editor
//! (p4-06/07), the workspace picker that opens the folder browser, and the
//! folder browser itself (p4-08/09).
//!
//! ## Why native views, not the Stage-B L0 cards
//! The Stage-B cards (`design/stage-b/phase4/cards/p4-0*`) are measured 406x776
//! phone artboards: every node sits at an `abs_pos`. Mounted as-is they are a
//! frozen phone width — taller than the 603 px desktop window and wider than a
//! 360 px phone — and their folder/model rows are runtime data in measured
//! boxes (RULES 8.10). So the screens are emitted as flowing DSL in the board's
//! visual language ([`super::board1_kit`]) and adapted the way the web adapts:
//! a centred dialog of the web's own widths on a desktop window
//! (`ConnectionPanel.module.css:15` 440 px, `NewSessionWorkspacePicker.module.css:23`
//! `min(540px, 100vw - 48px)`) and a full-width sheet below 560 px (the same
//! file's `@media (max-width: 560px)`). The Stage-B cards stay as the design
//! artifacts (`<set>::lower_screen`).
//!
//! ## Where each surface opens (the web's places)
//! | surface | opened from | web |
//! |---|---|---|
//! | pairing p4-01 | the first-run Connect screen's "Pair with a link instead", or a launch link (`OCTOS_PAIRING_LINK`) | `pairing.ts:95-127` (the link in the address) |
//! | pairing p4-01 (A11) | the Connect screen's discovery offer "Connect to <host>" ([`super::discovery`]) | `ConnectionPanel.tsx:178-194` (§Discovery) |
//! | connection p4-05 | Settings → General → "Connection…" | Settings → General → Forget (walk 112) |
//! | provider p4-06/07 | Settings → Model → "Edit provider…" | Settings → Models → Edit (walk 87/88) |
//! | picker | Settings → General → "Open a workspace…" | `NewSessionWorkspacePicker.tsx` (New session) |
//! | browser p4-08/09 | the picker's "Browse folders…" | the picker's Browse… (walk 220-223) |
//!
//! ## Host contract (lib.rs)
//! lib.rs owns one dock (`board1_dock`/`board1_splash`) and four added arms:
//! mount [`view`] while [`is_open`]; route [`collect`]'s events through
//! [`route`]; perform each [`Work`] ([`spawn`] runs the async ones); and feed
//! [`note_context`] the connection state each sync.
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use makepad_widgets::*;

use super::board1_kit::{self as kit, Text};
use super::{browser, pairing, provider};
use crate::flow::Conversation;

// ------------------------------------------------------------------- layout

/// The measured frame one surface is laid out in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// The module view's size.
    pub w: f64,
    pub h: f64,
    /// Below 560 px the surface is a full-width sheet, not a dialog.
    pub phone: bool,
    /// The card's width (the window's width on a phone).
    pub card_w: f64,
    /// The card's side padding.
    pub pad: f64,
    /// The width a full-width control gets.
    pub content_w: f64,
}

/// The web's breakpoint (`NewSessionWorkspacePicker.module.css:445`).
pub const PHONE_MAX: f64 = 560.0;

impl Layout {
    pub fn of(surface: Surface, w: f64, h: f64) -> Layout {
        let w = if w > 0.0 { w } else { 990.0 };
        let h = if h > 0.0 { h } else { 603.0 };
        let phone = w < PHONE_MAX;
        // The web's dialog widths: the connection card 440, the picker 540.
        let max = match surface {
            Surface::Pairing => 440.0,
            Surface::Provider => 460.0,
            Surface::Picker | Surface::Browser => 540.0,
        };
        let (card_w, pad) = if phone { (w, 20.0) } else { (max.min(w - 48.0), 28.0) };
        Layout { w, h, phone, card_w, pad, content_w: card_w - 2.0 * pad }
    }
}

/// One surface's view: its DSL and the controls the host routes.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Ui {
    pub dsl: String,
    /// (widget id, action id) — a click.
    pub buttons: Vec<(String, String)>,
    /// (widget id, action id) — an input's live text.
    pub inputs: Vec<(String, String)>,
    /// (widget id, action id) — Return in an input.
    pub returns: Vec<(String, String)>,
}

impl Ui {
    pub fn push(&mut self, s: impl AsRef<str>) {
        self.dsl.push_str(s.as_ref());
    }
    pub fn button(&mut self, id: &str, action: &str) {
        self.buttons.push((id.to_owned(), action.to_owned()));
    }
    pub fn input(&mut self, id: &str, action: &str) {
        self.inputs.push((id.to_owned(), action.to_owned()));
    }
    pub fn returns(&mut self, id: &str, action: &str) {
        self.returns.push((id.to_owned(), action.to_owned()));
    }
    /// Push what follows to the bottom of a phone sheet (the board anchors
    /// p4-02's Cancel, p4-03's Connect, p4-05's Forget low on the screen); a
    /// desktop dialog hugs its content, so there it is a fixed gap.
    pub fn spacer(&mut self, l: &Layout, desktop_gap: f64, phone_min: f64) {
        if l.phone {
            self.push(format!("View {{ width: Fill height: Fill }}\n{}", kit::gap(phone_min)));
        } else {
            self.push(kit::gap(desktop_gap));
        }
    }

    /// The board's header: the back chevron and the centred title. On a phone
    /// the chevron sits on its own row above the title (the atlas); in a
    /// desktop dialog both share one row, the web's dialog header shape
    /// (`NewSessionWorkspacePicker.tsx` header: back button + heading).
    /// The back chevron's 32 px hit sits a few px into the card's left padding
    /// (the board's optical alignment) but never so far that the card clips
    /// it below the 28 px target (judge: b1_pk_back / b1_pair_back measured 24 px).
    pub fn header(&mut self, l: &Layout, back_id: &str, back_action: &str, title: &str) {
        let t = Text::new("b1_title", title)
            .px(title_px(l, title))
            .weight(600)
            .fill()
            .centered()
            .one_line()
            .dsl();
        if l.phone {
            self.push(format!(
                "View {{ width: Fill height: Fit flow: Down\nView {{ width: Fill height: Fit margin: Inset{{left: -2}}\n{}}}\n{}{t}}}\n",
                kit::back_button(back_id),
                kit::gap(14.0)
            ));
        } else {
            self.push(format!(
                "View {{ width: Fill height: 36 flow: Overlay\nView {{ width: Fill height: Fill align: Align{{x: 0.5 y: 0.5}} padding: Inset{{left: 40 right: 40}}\n{t}}}\nView {{ width: Fit height: Fill align: Align{{x: 0.0 y: 0.5}} margin: Inset{{left: -4}}\n{}}}\n}}\n",
                kit::back_button(back_id)
            ));
        }
        self.button(back_id, back_action);
    }
}

/// The header title's size: the board's (23 desktop, 24 phone before the
/// phone type scale), shrunk so a long title still fits its row on a narrow
/// phone (Inter SemiBold runs ~0.5 em per character; 0.56 keeps a margin).
pub fn title_px(l: &Layout, title: &str) -> f64 {
    let (target, scale, avail) = if l.phone {
        (24.0, 1.08, l.content_w)
    } else {
        // The desktop row keeps 40 px each side for the back chevron.
        (23.0, 1.0, l.content_w - 80.0)
    };
    let chars = title.chars().count().max(1) as f64;
    let fit = (avail / (chars * 0.56 * scale)).floor();
    target.min(fit).max(17.0)
}

// ------------------------------------------------------------------ surfaces

/// The board-1 surfaces the dock can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Pairing,
    Provider,
    Picker,
    Browser,
}

/// The new-session workspace picker (the board-2.4 "Open a workspace" card's
/// content; the web's `NewSessionWorkspacePicker` "choose" view): the
/// server's working directory first, then the recent paths, then Browse.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PickerUi {
    /// The server's working directory (`server-working-directory.ts:2-23`).
    pub server_root: Option<String>,
    /// (name, path) — `recents::load_recent_workspaces`, newest first.
    pub recents: Vec<(String, String)>,
    /// `onboarding.workspace_browse.v1` is advertised (row 166, fail closed).
    pub browse_advertised: bool,
    pub loading: bool,
    /// A session is being opened at this path.
    pub starting: Option<String>,
    /// The last failure, in our own words.
    pub error: Option<String>,
}

#[derive(Default)]
struct Host {
    /// Open surfaces, the visible one last (Picker → Browser stacks).
    stack: Vec<Surface>,
    dirty: bool,
    size: (f64, f64),
    dsl: String,
    ui: Ui,
    /// Work an async task produced for the host (a claim's Connect).
    pending: Vec<Work>,
    picker: PickerUi,
    /// The connect a pairing claim handed over is in flight.
    awaiting_connect: bool,
    /// The open reply's `supported_methods`, from the last [`note_context`].
    methods: Vec<String>,
    /// A surface was open at the last [`take_ime_reset`].
    was_open: bool,
    /// The view was rebuilt since the last [`take_ime_reset`].
    ime_reset: bool,
}

fn host() -> MutexGuard<'static, Host> {
    static HOST: OnceLock<Mutex<Host>> = OnceLock::new();
    HOST.get_or_init(|| Mutex::new(Host::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Something changed that the next view must show.
pub fn mark_dirty() {
    host().dirty = true;
}

pub fn is_open() -> bool {
    !host().stack.is_empty()
}

pub fn top() -> Option<Surface> {
    host().stack.last().copied()
}

fn push_surface(s: Surface) {
    let mut h = host();
    h.stack.retain(|x| *x != s);
    h.stack.push(s);
    h.dirty = true;
}

/// Close the visible surface (back to the one under it, or closed).
pub fn pop() {
    let mut h = host();
    h.stack.pop();
    h.dirty = true;
}

pub fn close_all() {
    let mut h = host();
    h.stack.clear();
    h.dirty = true;
}

pub fn picker() -> PickerUi {
    host().picker.clone()
}

/// A session is being opened from the picker or the browser: the surface
/// stays up until it settles (the web's "keeps an in-flight workspace
/// creation visible when Escape or the backdrop is used",
/// `closeOnBackdrop={!creating}`, the back button `disabled={creating}`).
fn starting() -> bool {
    host().picker.starting.is_some()
}

// --------------------------------------------------------------------- work

/// What the host performs after an action.
#[derive(Debug, Clone, PartialEq)]
pub enum Work {
    /// The one-use exchange (`claim_pairing_code`).
    PairExchange { link: octoscode_client::pairing::PairingLink, generation: u64 },
    /// Connect through the production connect path (`perform_screen_action`).
    Connect { server: String, token: String },
    /// Show the server+token Connect form, its Server prefilled.
    LeaveToForm { server: Option<String> },
    /// Open the platform QR scanner.
    Scan,
    /// Forget the credential and disconnect.
    Forget,
    /// `profile/llm/list` + `profile/llm/catalog` into the editor.
    ProviderLoad,
    /// The editor's Test / Save / Fetch (A23). It carries no key: the
    /// transport reads it from the editor when it runs.
    ProviderTransport(provider::Effect),
    /// A23 — the editor opened from the providers dialog closed: reopen the
    /// dialog (re-read), with the editor's outcome line.
    ReturnToProviders { notice: Option<String> },
    /// A23 — open a URL in the platform browser (the GLM guide link).
    OpenUrl(String),
    /// `onboarding/workspace_list`.
    BrowserList { path: Option<String>, resolve_ancestor: bool },
    /// `onboarding/workspace_create`.
    BrowserCreate { parent: String, name: String },
    /// The picker's server root + recents.
    PickerLoad,
    /// Start a new session in `cwd` (`Conversation::new_chat`).
    NewSession { cwd: String },
}

/// The action ids that open a surface (the entry points).
pub const OPENERS: &[(&str, &str)] = &[
    ("b1.open.pairing", "open Pair with Octos (p4-01) — the Connect screen's link"),
    ("b1.open.connection", "open Connection (p4-05) — Settings → General"),
    ("b1.open.provider", "open Edit provider (p4-06) — Settings → Model"),
    ("b1.open.provider.routes", "A23: the providers dialog's Edit / Add provider — the editor the dialog seeded (no reload)"),
    ("b1.open.picker", "open the new-session workspace picker (the web's view \"choose\")"),
    ("b1.open.add", "+ Add workspace: the folder browser over the picker (the web's view \"add\"/\"browse\"; the picker alone when browsing is not advertised)"),
    ("b1.backdrop", "a click on the dialog's backdrop closes it (ModalSurface closeOnBackdrop)"),
    ("b1.open.discovered", "A11: the Connect card's discovery offer \"Connect to <host>\" — pairing (p4-01) with the remembered server that answered /pair/info, or a tokenless connect when it needs no token"),
];

/// The picker's own action ids.
pub const PICKER_ACTIONS: &[(&str, &str)] = &[
    ("picker.server", "start a session in the server's working directory"),
    ("picker.recent.0", "start a session in the 1st recent path"),
    ("picker.recent.1", "start a session in the 2nd recent path"),
    ("picker.recent.2", "start a session in the 3rd recent path"),
    ("picker.recent.3", "start a session in the 4th recent path"),
    ("picker.browse", "open the folder browser (advertised-gated)"),
    ("picker.newfolder", "open the folder browser with New folder open"),
    ("picker.close", "the back chevron: close the picker"),
];

/// Whether board 1 owns this id (one owner per id: pairing, provider,
/// browser, the picker and the openers never overlap — fd1 pins it).
pub fn owns(action: &str) -> bool {
    OPENERS.iter().any(|(a, _)| *a == action)
        || PICKER_ACTIONS.iter().any(|(a, _)| *a == action)
        || action == "pair.scan.cancelled"
        || pairing::is_action(action)
        || provider::is_action(action)
        || browser::is_action(action)
}

/// Route one board-1 event. UI-local halves are applied here; the returned
/// work is the host's to perform.
pub fn route(action: &str, value: Option<&str>) -> Vec<Work> {
    let mut out = Vec::new();
    match action {
        "b1.open.pairing" => {
            {
                let mut p = pairing::state();
                p.problem = None;
                p.exchanging = false;
                p.screen = pairing::Screen::Pair;
            }
            push_surface(Surface::Pairing);
        }
        "b1.open.connection" => {
            pairing::state().screen = pairing::Screen::Paired;
            push_surface(Surface::Pairing);
        }
        "b1.open.provider" => {
            // Each operation on its own advertised method (row 37): the open
            // reply's `supported_methods`, failing closed.
            // A23: a fresh editor every time (the web's `openEdit`), seeded
            // by the load.
            let methods = host().methods.clone();
            let caps = provider::Caps::from_methods(&methods);
            {
                let mut fresh = provider::ProviderUi::new("deepseek");
                fresh.caps = caps;
                fresh.can_fetch = methods.iter().any(|m| m == "profile/llm/fetch_models");
                fresh.origin = provider::Origin::Settings;
                provider::set(fresh);
            }
            push_surface(Surface::Provider);
            if caps.read || caps.catalog {
                out.push(Work::ProviderLoad);
            }
        }
        "b1.open.provider.routes" => {
            // A23 — `board3::routes` seeded the editor (a configured row or
            // the catalog's new provider) and closed itself.
            provider::state().busy = false;
            push_surface(Surface::Provider);
        }
        "b1.open.picker" => {
            {
                let mut h = host();
                h.picker.loading = true;
                h.picker.error = None;
                h.picker.starting = None;
            }
            push_surface(Surface::Picker);
            out.push(Work::PickerLoad);
        }
        "b1.open.add" => {
            // The web's + Add workspace opens the picker at its "add" view,
            // whose Browse… is the folder browser and whose back returns to
            // "choose" (NewSessionWorkspacePicker.tsx:128-138). Natively: the
            // browser over the picker, so its back chevron lands on the
            // picker; fail closed to the picker alone (row 166).
            out.extend(route("b1.open.picker", None));
            if host().picker.browse_advertised {
                out.extend(route_picker("picker.browse"));
            }
        }
        "b1.open.discovered" => {
            // A11 — the offer's one button, used once (the web's
            // `useDiscoveredOrigin`, ConnectionGate.tsx:358-366): the form now
            // names that server. A server that needs a token is paired with
            // (p4-01, its origin kept for p4-03/p4-04's fallbacks); one that
            // needs none connects at once, tokenless.
            if let Some(offer) = super::discovery::take() {
                makepad_widgets::log!(
                    "[octoscode] discovery: offer used -> {}",
                    if offer.pairing_required { "pairing" } else { "connect" }
                );
                if offer.pairing_required {
                    out.push(Work::LeaveToForm { server: Some(offer.origin.clone()) });
                    out.extend(route("b1.open.pairing", None));
                    let mut p = pairing::state();
                    p.server = offer.origin.clone();
                    p.pairing_host = pairing::host_of(&offer.origin);
                } else {
                    out.push(Work::Connect { server: offer.origin, token: String::new() });
                }
            }
        }
        "b1.backdrop" => {
            // Never mid-exchange or mid-save: the web's ModalSurface keeps the
            // dialog while work is in flight (`closeOnBackdrop={!creating}`).
            let busy = pairing::state().exchanging || provider::state().busy || starting();
            if !busy {
                let from_routes = top() == Some(Surface::Provider) && provider::state().origin == provider::Origin::Routes;
                close_all();
                if from_routes {
                    out.push(Work::ReturnToProviders { notice: None });
                }
            }
        }
        "pair.scan.cancelled" => {
            let mut p = pairing::state();
            p.scan_note = Some(if value == Some("unsupported") {
                "This device has no camera scanner. Paste the link instead."
            } else {
                "No code was scanned. Try again, or paste the link."
            });
            drop(p);
            mark_dirty();
        }
        a if PICKER_ACTIONS.iter().any(|(p, _)| *p == a) => out.extend(route_picker(a)),
        a if pairing::is_action(a) => {
            let o = pairing::perform(a, value);
            if !is_typing(a) {
                mark_dirty();
            }
            match o {
                None => {}
                Some(pairing::Out::Exchange { link, generation }) => out.push(Work::PairExchange { link, generation }),
                Some(pairing::Out::Connect { server, token }) => {
                    // p4-03's manual Connect: the connect screen takes over.
                    close_all();
                    out.push(Work::Connect { server, token });
                }
                Some(pairing::Out::LeaveToForm { server }) => {
                    close_all();
                    out.push(Work::LeaveToForm { server });
                }
                Some(pairing::Out::Scan) => out.push(Work::Scan),
                Some(pairing::Out::Forget) => {
                    close_all();
                    out.push(Work::Forget);
                }
                Some(pairing::Out::Close) => pop(),
            }
        }
        a if provider::is_action(a) => {
            let e = provider::perform(a, value);
            if !is_typing(a) {
                mark_dirty();
            }
            match e {
                None => {}
                Some(provider::Effect::Close) => {
                    pop();
                    // A23 — opened from the providers dialog: back to it.
                    if provider::state().origin == provider::Origin::Routes {
                        out.push(Work::ReturnToProviders { notice: None });
                    }
                }
                Some(t @ (provider::Effect::Test | provider::Effect::Save | provider::Effect::Fetch)) => {
                    out.push(Work::ProviderTransport(t))
                }
                Some(provider::Effect::OpenUrl(u)) => out.push(Work::OpenUrl(u)),
                Some(_) => {}
            }
        }
        a if browser::is_action(a) => {
            let e = browser::perform(a, value);
            if !is_typing(a) {
                mark_dirty();
            }
            match e {
                None => {}
                Some(browser::Effect::List(path)) => out.push(Work::BrowserList { path, resolve_ancestor: false }),
                Some(browser::Effect::Create { parent, name }) => out.push(Work::BrowserCreate { parent, name }),
                Some(browser::Effect::Use(path)) => {
                    host().picker.starting = Some(path.clone());
                    out.push(Work::NewSession { cwd: path });
                }
                // An in-flight session start stays visible (the web disables
                // the back button and Escape while creating).
                Some(browser::Effect::Close) if !starting() => pop(),
                Some(_) => {}
            }
        }
        _ => {}
    }
    out
}

/// Live text never rebuilds the view: the field keeps its focus and caret
/// (the composer's #32h lesson — re-mounting per keystroke kills the IME).
fn is_typing(action: &str) -> bool {
    matches!(action, "pair.paste" | "connect.server" | "connect.token" | "browser.path" | "browser.create_name")
        || provider::is_input(action)
}

fn route_picker(action: &str) -> Vec<Work> {
    let mut out = Vec::new();
    let pk = host().picker.clone();
    let start = |path: String, out: &mut Vec<Work>| {
        host().picker.starting = Some(path.clone());
        mark_dirty();
        out.push(Work::NewSession { cwd: path });
    };
    match action {
        "picker.close" if !starting() => pop(),
        "picker.close" => {}
        "picker.server" => {
            if let Some(root) = pk.server_root.clone() {
                start(root, &mut out);
            }
        }
        "picker.browse" | "picker.newfolder" => {
            if !pk.browse_advertised {
                // Fail closed: no affordance, no request (row 166).
                return out;
            }
            let open_at = {
                let b = browser::state();
                b.chosen.clone().or_else(|| pk.server_root.clone())
            };
            {
                let mut b = browser::state();
                b.loading = true;
                b.screen = browser::Screen::Browser;
                b.failure = None;
                b.attempted = None;
                b.new_folder_open = action == "picker.newfolder";
                b.new_folder.clear();
                b.name_problem = None;
            }
            push_surface(Surface::Browser);
            // Walk 223: the browser reopens at the folder chosen last.
            out.push(Work::BrowserList { path: open_at, resolve_ancestor: true });
        }
        a => {
            if let Some(i) = a.strip_prefix("picker.recent.").and_then(|n| n.parse::<usize>().ok()) {
                if let Some((_, path)) = pk.recents.get(i).cloned() {
                    start(path, &mut out);
                }
            }
        }
    }
    out
}

// -------------------------------------------------------------- the picker

fn picker_view(pk: &PickerUi, l: &Layout) -> Ui {
    let mut v = Ui::default();
    v.header(l, "b1_pk_back", "picker.close", "Open a workspace");
    v.push(kit::gap(if l.phone { 22.0 } else { 16.0 }));
    let (root_label, root_path) = match &pk.server_root {
        Some(p) => ("Server folder", p.clone()),
        None if pk.loading => ("Server folder", "Reading the server\u{2026}".to_owned()),
        // server-working-directory.ts:2-23 — the fallback when no root was reported.
        None => ("Server folder (path not reported)", String::new()),
    };
    let root_row = format!(
        "View {{ width: Fill height: {} flow: Overlay\nView {{ width: Fill height: Fill flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 18 right: 16}} spacing: 16\n{}View {{ width: Fill height: Fit flow: Down spacing: 3\n{}{}}}\n{}}}\n{}}}\n",
        if l.phone { 76 } else { 64 },
        kit::svg("", "b1_folder.svg", 26.0),
        Text::new("b1_pk_server_t", root_label).px(16.0).fill().one_line().dsl(),
        Text::new("b1_pk_server_p", &root_path).px(14.0).color(kit::MUTED).fill().one_line().dsl(),
        kit::svg("", "b1_chevron_right.svg", 16.0),
        kit::hit("b1_pk_server", true)
    );
    v.push(kit::list_card("b1_pk_server_card", &[root_row]));
    v.button("b1_pk_server", "picker.server");
    v.push(kit::gap(if l.phone { 22.0 } else { 16.0 }));
    v.push(Text::new("", "Recent").px(15.0).fill().one_line().dsl());
    v.push(kit::gap(8.0));
    let rows: Vec<String> = pk
        .recents
        .iter()
        .take(4)
        .enumerate()
        .map(|(i, (name, path))| {
            let id = format!("b1_pk_recent_{i}");
            v.button(&id, &format!("picker.recent.{i}"));
            format!(
                "View {{ width: Fill height: {} flow: Overlay\nView {{ width: Fill height: Fill flow: Right align: Align{{x: 0.0 y: 0.5}} padding: Inset{{left: 18 right: 16}} spacing: 16\n{}View {{ width: Fill height: Fit flow: Down spacing: 3\n{}{}}}\n}}\n{}}}\n",
                if l.phone { 66 } else { 56 },
                kit::svg("", "b1_folder.svg", 24.0),
                Text::new(&format!("b1_pk_recent_t{i}"), name).px(15.0).fill().one_line().dsl(),
                Text::new("", path).px(13.0).color(kit::MUTED).fill().one_line().dsl(),
                kit::hit(&id, true)
            )
        })
        .collect();
    if rows.is_empty() {
        v.push(Text::new("b1_pk_norecent", "No recent workspaces yet.").px(14.0).color(kit::MUTED).fill().dsl());
    } else {
        v.push(kit::list_card("b1_pk_recents", &rows));
    }
    if let Some(e) = &pk.error {
        v.push(kit::gap(10.0));
        v.push(Text::new("b1_pk_error", e).px(14.0).color(kit::RED).fill().dsl());
    }
    if let Some(p) = &pk.starting {
        v.push(kit::gap(10.0));
        let line = format!("Starting a session in {}\u{2026}", super::recents::workspace_name(p));
        v.push(Text::new("b1_pk_starting", &line).px(14.0).color(kit::MUTED).fill().one_line().dsl());
    }
    // Row 166: Browse / New folder exist ONLY when the server advertised the
    // browse feature — fail closed, structurally.
    if pk.browse_advertised {
        v.push(kit::gap(if l.phone { 30.0 } else { 20.0 }));
        v.push(kit::pill_outline("b1_pk_browse", "Browse folders\u{2026}", "Fill"));
        v.button("b1_pk_browse", "picker.browse");
        v.push(kit::gap(10.0));
        v.push(format!(
            "View {{ width: Fill height: Fit align: Align{{x: 0.5 y: 0.0}}\n{}}}\n",
            kit::link("b1_pk_newfolder", "New folder", kit::INK, 15.0, 400)
        ));
        v.button("b1_pk_newfolder", "picker.newfolder");
    }
    v
}

// ------------------------------------------------------------------ the view

fn compose(surface: Surface, w: f64, h: f64) -> Ui {
    let l = Layout::of(surface, w, h);
    kit::set_type_scale(if l.phone { 1.08 } else { 1.0 });
    let body = match surface {
        Surface::Pairing => pairing::view(&pairing::state(), &l),
        Surface::Provider => provider::view(&provider::state(), &l),
        Surface::Browser => browser::view(&browser::state(), &l),
        Surface::Picker => {
            let pk = host().picker.clone();
            picker_view(&pk, &l)
        }
    };
    kit::set_type_scale(1.0);
    wrap(body, &l)
}

/// The dialog (desktop) or sheet (phone) around a surface's body.
fn wrap(body: Ui, l: &Layout) -> Ui {
    let mut ui = Ui { dsl: String::new(), ..body.clone() };
    if l.phone {
        // An opaque sheet: a plain `View`'s `show_bg` paints nothing on this
        // fork (the first phone capture showed the Connect card through it),
        // a `SolidView` does (the desktop scrim below is one).
        ui.dsl = format!(
            "b1_sheet := View {{ width: Fill height: Fill flow: Overlay\nSolidView {{ width: Fill height: Fill draw_bg.color: {} }}\nb1_card := View {{ width: Fill height: Fill flow: Down padding: Inset{{left: {} right: {} top: 12 bottom: 20}}\n{}}}\n}}\n",
            kit::WHITE,
            l.pad,
            l.pad,
            body.dsl
        );
    } else {
        ui.dsl = format!(
            "View {{ width: Fill height: Fill flow: Overlay\nSolidView {{ width: Fill height: Fill draw_bg.color: #0000003d }}\n{}View {{ width: Fill height: Fill align: Align{{x: 0.5 y: 0.5}} padding: 20\nb1_card := DesignSurface {{ width: {} height: Fit flow: Down\ndraw_bg.color: {} draw_bg.radius: 18 draw_bg.border_width: 1 draw_bg.border_position: 1 draw_bg.border_color: {}\nView {{ width: Fill height: Fit flow: Down padding: Inset{{left: {} right: {} top: 16 bottom: 24}}\n{}}}\n}}\n}}\n}}\n",
            kit::hit("b1_backdrop", false),
            l.card_w,
            kit::WHITE,
            kit::HAIR,
            l.pad,
            l.pad,
            body.dsl
        );
        ui.buttons.insert(0, ("b1_backdrop".to_owned(), "b1.backdrop".to_owned()));
    }
    ui
}

/// The dock's DSL while a surface is open (`None` = closed). Rebuilt only
/// when something structural changed or the window resized — typing never
/// rebuilds it (see [`is_typing`]).
pub fn view(w: f64, h: f64) -> Option<String> {
    let surface = top()?;
    let (rebuild, structural) = {
        let hh = host();
        let structural = hh.dirty || hh.dsl.is_empty();
        (structural || hh.size != (w, h), structural)
    };
    if rebuild {
        let ui = compose(surface, w, h);
        let mut hh = host();
        hh.dsl = ui.dsl.clone();
        hh.ui = ui;
        hh.size = (w, h);
        hh.dirty = false;
        // Only a structural rebuild drops the keyboard: a window that
        // resizes FOR the keyboard (adjustResize) must not hide it again.
        if structural {
            hh.ime_reset = true;
        }
    }
    Some(host().dsl.clone())
}

/// Whether the soft keyboard must go: a surface closed, or its view was
/// rebuilt. A re-mount drops the focused field, so the keyboard it raised has
/// no owner left (measured on the phone shell: after Save closed the editor
/// the keyboard stayed up over the sidebar's + Add workspace).
pub fn take_ime_reset() -> bool {
    let mut h = host();
    let open = !h.stack.is_empty();
    let closed = h.was_open && !open;
    h.was_open = open;
    std::mem::take(&mut h.ime_reset) || closed
}

/// The controls of the mounted view (for tests and the click walk).
pub fn controls() -> Ui {
    host().ui.clone()
}

/// The routed events in `actions`: every mounted control's click, every
/// input's live text and Return, and the platform's QR answer.
pub fn collect(cx: &mut Cx, root: &View, actions: &Actions) -> Vec<(String, Option<String>)> {
    let mut ui = if is_open() { host().ui.clone() } else { Ui::default() };
    // The always-mounted entry points: the Connect screen's link and the
    // Settings drawer's rows route whether or not a surface is open.
    ui.buttons.extend(entry_controls());
    let mut out = Vec::new();
    for (id, action) in &ui.inputs {
        if let Some(text) = root.text_input(cx, &[LiveId::from_str(id)]).changed(actions) {
            out.push((action.clone(), Some(text)));
        }
    }
    for (id, action) in &ui.returns {
        if root.text_input(cx, &[LiveId::from_str(id)]).returned(actions).is_some() {
            out.push((action.clone(), None));
        }
    }
    for (id, action) in &ui.buttons {
        if root.button(cx, &[LiveId::from_str(id)]).clicked(actions) {
            out.push((action.clone(), None));
        }
    }
    use makepad_widgets::makepad_platform::event::{NativeQrCancelled, NativeQrScanned};
    for a in actions.iter() {
        if let Some(scan) = a.downcast_ref::<NativeQrScanned>() {
            out.push(("pair.scanned".to_owned(), Some(scan.json.clone())));
        } else if let Some(c) = a.downcast_ref::<NativeQrCancelled>() {
            out.push(("pair.scan.cancelled".to_owned(), Some(c.reason.clone())));
        }
    }
    out
}

/// The action Escape performs on the visible surface (its back chevron's).
pub fn escape_action() -> &'static str {
    match top() {
        Some(Surface::Pairing) => "pair.back",
        Some(Surface::Provider) => "provider.back",
        Some(Surface::Browser) => "browser.close",
        Some(Surface::Picker) => "picker.close",
        None => "b1.backdrop",
    }
}

/// Whether one control id is in the mounted view (the click walk's check).
pub fn mounted_control(id: &str) -> bool {
    let h = host();
    h.ui.buttons.iter().chain(h.ui.inputs.iter()).any(|(i, _)| i == id)
}

// ----------------------------------------------------------------- context

/// The live connection facts the surfaces read, fed by the host each sync.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub live: bool,
    /// The connect form's server (p4-05's row when the token came from it).
    pub server: String,
    /// The connect the pairing claim handed over has failed.
    pub connect_failed: bool,
    pub capabilities: Vec<String>,
    /// The open reply's `supported_methods` (the per-method gates).
    pub methods: Vec<String>,
}

/// Fold the connection state in. Closes pairing once the connection a claim
/// handed over is live; falls back to p4-03 when it failed.
pub fn note_context(ctx: &Context) {
    if ctx.live {
        // A11: a live connection leaves the discovery offer nothing to do.
        super::discovery::note_live();
    }
    let advertised = ctx.capabilities.iter().any(|c| c == browser::BROWSE_FEATURE);
    {
        let mut h = host();
        if h.picker.browse_advertised != advertised {
            h.picker.browse_advertised = advertised;
            h.dirty = true;
        }
        if h.methods != ctx.methods {
            h.methods = ctx.methods.clone();
        }
    }
    {
        let mut p = pairing::state();
        if p.connected_server != ctx.server {
            p.connected_server = ctx.server.clone();
        }
    }
    let awaiting = host().awaiting_connect;
    if awaiting {
        if ctx.live {
            host().awaiting_connect = false;
            {
                let mut p = pairing::state();
                p.exchanging = false;
                p.screen = pairing::Screen::Paired;
            }
            if top() == Some(Surface::Pairing) {
                pop();
            }
            makepad_widgets::log!("[octoscode] pairing: connected — the paired token is live");
        } else if ctx.connect_failed {
            host().awaiting_connect = false;
            pairing::connect_failed(&mut pairing::state());
            mark_dirty();
        }
    }
}

/// Work an async task left for the host (drained on every Signal).
pub fn take_pending() -> Vec<Work> {
    std::mem::take(&mut host().pending)
}

fn leave_for_host(w: Work) {
    host().pending.push(w);
    SignalToUI::set_ui_signal();
}

/// The host is about to hand a pairing claim's token to the connect path.
pub fn awaiting_connect(on: bool) {
    host().awaiting_connect = on;
}

// ------------------------------------------------------------------ env

/// `OCTOSCODE_SCREEN=p4-0N|picker` (a capture convenience — the real entries
/// are clicks): open that surface in that state, once.
pub fn open_from_env(which: &str) -> Vec<Work> {
    static ONCE: OnceLock<()> = OnceLock::new();
    if ONCE.set(()).is_err() {
        return Vec::new();
    }
    match which {
        "picker" => route("b1.open.picker", None),
        card => {
            if let Some(s) = pairing::Screen::from_card(card) {
                let w = route(if s == pairing::Screen::Paired { "b1.open.connection" } else { "b1.open.pairing" }, None);
                pairing::state().screen = s;
                mark_dirty();
                w
            } else if card == "p4-06" || card == "p4-07" {
                let w = route("b1.open.provider", None);
                if card == "p4-07" {
                    provider::state().reject("HTTP 401 - authentication failed");
                }
                w
            } else if card == "p4-08" || card == "p4-09" {
                host().picker.browse_advertised = true;
                let w = route("picker.browse", None);
                w
            } else {
                Vec::new()
            }
        }
    }
}

/// A pairing link handed to the app at launch (`OCTOS_PAIRING_LINK`, the
/// native analog of the web's `?octos=&pair=` address): read once, cleared
/// from the environment like the web strips the URL (`pairing.ts:95-127`),
/// and exchanged at once — no form, no token box (walk 108).
pub fn launch_link() -> Vec<Work> {
    static ONCE: OnceLock<()> = OnceLock::new();
    if ONCE.set(()).is_err() {
        return Vec::new();
    }
    let Ok(link) = std::env::var("OCTOS_PAIRING_LINK") else {
        return Vec::new();
    };
    // The value must not outlive its one use (the web strips its URL).
    std::env::remove_var("OCTOS_PAIRING_LINK");
    let mut out = route("b1.open.pairing", None);
    pairing::perform("pair.paste", Some(&link));
    out.extend(route("pair.submit", None));
    out
}

// ---------------------------------------------------------- the connect card

/// The first-run Connect screen's way into pairing: one "Pair with a link
/// instead" link under the token hint, injected into the lowered setup-01
/// card (its 46..360 column, between "Stored for this server only" and the
/// Connect pill). The card itself is untouched otherwise.
pub fn with_connect_entry(dsl: &str) -> String {
    const ANCHOR: &str = "beauty_0_0_7 := Label {";
    let entry = format!(
        "b1_connect_pair := ButtonFlat {{\nwidth: Fit height: 30 abs_pos: vec2(41, 444) padding: Inset{{left: 6 right: 6}} margin: 0 align: Align{{x: 0.0 y: 0.5}}\ntext: \"Pair with a link instead\"\ndraw_bg +: {{color: {c} color_hover: #00000008 color_down: #00000012 color_focus: {c} color_disabled: {c} border_size: 0.0 border_radius: 6.0 border_color: {c} border_color_hover: {c} border_color_down: {c} border_color_focus: {c} border_color_disabled: {c}}}\ndraw_text +: {{color: {b} color_hover: {b} color_down: {b} color_focus: {b} color_disabled: {b} text_style: {}}}\n}}\n",
        kit::font(500, 14.0),
        c = kit::CLEAR,
        b = kit::BLUE,
    );
    match dsl.find(ANCHOR) {
        Some(i) => format!("{}{}{}", &dsl[..i], entry, &dsl[i..]),
        None => dsl.to_owned(),
    }
}

/// The Settings dialog's board-1 entries. The rows themselves are A3's
/// Settings panel (`chrome.rs` `OcSettingsPanel`): "Model providers · Edit"
/// in the Model section (the web's `ModelManagementSection.tsx` row "Edit"
/// opens the provider editor) and "This device · Details" in the Connection
/// section. Their hits are plain buttons, routed here by id.
pub fn settings_controls() -> Vec<(String, String)> {
    vec![
        ("b1_set_provider".to_owned(), "b1.open.provider".to_owned()),
        ("b1_set_connection".to_owned(), "b1.open.connection".to_owned()),
    ]
}

/// Every always-mounted entry control: (widget id, opener action id). The
/// sidebar's "+ Add workspace" is A3's (`sb_add_hit` -> `workspace.add`),
/// which lib.rs answers with `b1.open.add`.
pub fn entry_controls() -> Vec<(String, String)> {
    let mut v = vec![
        ("b1_connect_pair".to_owned(), "b1.open.pairing".to_owned()),
        // A11: the discovery offer's button on the same card
        // (`fluid::connect_card_with_offer`, shown while `discovery::offer`).
        ("connect_offer".to_owned(), "b1.open.discovered".to_owned()),
    ];
    v.extend(settings_controls());
    v
}

// ------------------------------------------------------------- execution

/// Run one async [`Work`] to completion — the production half of every
/// board-1 action that talks to a server (what [`spawn`] runs on the module's
/// runtime; the transport tests drive it directly). UI-integration work
/// (Connect, LeaveToForm, Scan, Forget) is the host's (`lib.rs`
/// `perform_board1_work`) and is refused here.
pub async fn execute(work: Work, conv: Option<Arc<Conversation>>) -> Result<(), String> {
    let need_conv = |what: &str| {
        makepad_widgets::log!("[octoscode] board1 {what}: no connection yet");
        Err(format!("board1 {what}: no connection yet"))
    };
    let out = match work {
        Work::PairExchange { link, generation } => {
            let result = octoscode_client::pairing::claim_pairing_code(&link).await;
            // The kind is logged, never the code or the token.
            match &result {
                Ok(_) => makepad_widgets::log!("[octoscode] pairing: claim accepted"),
                Err(k) => makepad_widgets::log!("[octoscode] pairing: claim refused ({})", k.as_str()),
            }
            let out = pairing::finish_exchange(&mut pairing::state(), generation, result);
            if let Some(pairing::Out::Connect { server, token }) = out {
                awaiting_connect(true);
                leave_for_host(Work::Connect { server, token });
            }
            Ok(())
        }
        Work::ProviderLoad => {
            let Some(conv) = conv else { return need_conv("provider load") };
            provider::load(&conv).await
        }
        Work::ProviderTransport(effect) => {
            let Some(conv) = conv else {
                provider::perform_failed("Connect to a server first.");
                mark_dirty();
                return need_conv("provider save");
            };
            let fetch = effect == provider::Effect::Fetch;
            match provider::perform_transport(&conv, effect).await {
                Ok(true) => {
                    makepad_widgets::log!("[octoscode] board1 provider: saved (profile/llm/upsert applied)");
                    if top() == Some(Surface::Provider) {
                        pop();
                    }
                    // A23 — back to the providers dialog with the web's line
                    // (`ModelManagementSection` save).
                    let (origin, mode) = {
                        let p = provider::state();
                        (p.origin, p.mode)
                    };
                    if origin == provider::Origin::Routes {
                        let notice = if mode == provider::Mode::Edit {
                            super::model_settings::copy::SAVED_EDIT
                        } else {
                            super::model_settings::copy::SAVED
                        };
                        leave_for_host(Work::ReturnToProviders { notice: Some(notice.to_owned()) });
                    }
                    Ok(())
                }
                Ok(false) if fetch => {
                    makepad_widgets::log!("[octoscode] board1 provider: models fetched");
                    Ok(())
                }
                Ok(false) => {
                    makepad_widgets::log!("[octoscode] board1 provider: test passed");
                    Ok(())
                }
                Err(e) => {
                    makepad_widgets::log!("[octoscode] board1 provider: {e}");
                    Err(e)
                }
            }
        }
        Work::BrowserList { path, resolve_ancestor } => {
            let Some(conv) = conv else { return need_conv("browse") };
            browser::list(&conv, path, resolve_ancestor).await
        }
        Work::BrowserCreate { parent, name } => {
            let Some(conv) = conv else { return need_conv("create folder") };
            browser::create(&conv, parent, name).await
        }
        Work::PickerLoad => {
            let recents: Vec<(String, String)> =
                super::recents::load_recent_workspaces(&*super::recents::store(), &super::recents::endpoint())
                    .into_iter()
                    .map(|r| (r.name, r.path))
                    .collect();
            host().picker.recents = recents;
            let Some(conv) = conv else {
                let mut h = host();
                h.picker.loading = false;
                h.dirty = true;
                drop(h);
                return need_conv("picker");
            };
            let advertised = conv.store.capabilities().iter().any(|c| c == browser::BROWSE_FEATURE);
            let session_root = conv.store.domains.session.workspace_root(&conv.session_id());
            use octoscode_client::domains::profile::{WorkspaceList, WorkspaceListParams};
            // The server's working directory: a `workspace_list` with no path
            // lists exactly it (`profile.rs` WorkspaceListParams); without the
            // browse feature, the open session's reported root
            // (`server-working-directory.ts:2-23`).
            let root = if advertised {
                conv.client()
                    .call::<WorkspaceList>(WorkspaceListParams { path: None })
                    .await
                    .ok()
                    .map(|l| l.canonical_path)
                    .or(session_root)
            } else {
                session_root
            };
            let mut h = host();
            h.picker.server_root = root;
            h.picker.browse_advertised = advertised;
            h.picker.loading = false;
            Ok(())
        }
        Work::NewSession { cwd } => {
            let Some(conv) = conv else {
                {
                    let mut h = host();
                    h.picker.error = Some("Connect to a server first.".to_owned());
                    h.picker.starting = None;
                }
                mark_dirty();
                return need_conv("new session");
            };
            // A8 — a workspace launch asks `launch/resolve` first when the
            // server advertises it (cross-profile -> the decision panel).
            let launched = match crate::screens::launch::create(&conv, cwd.clone()).await {
                crate::screens::launch::Launched::Opened(id) => Ok(id),
                crate::screens::launch::Launched::AwaitingChoice => Ok("awaiting the profile choice".to_owned()),
                crate::screens::launch::Launched::Stale => Ok("superseded by a newer launch".to_owned()),
                crate::screens::launch::Launched::Failed(e) => Err(e),
            };
            match launched {
                Ok(id) => {
                    makepad_widgets::log!("[octoscode] board1: new session {id} in the chosen workspace");
                    let rows = super::recents::remember_workspace(
                        &*super::recents::store(),
                        &super::recents::endpoint(),
                        &cwd,
                        super::recents::now_ms(),
                    );
                    let mut h = host();
                    h.picker.recents = rows.into_iter().map(|r| (r.name, r.path)).collect();
                    h.picker.starting = None;
                    h.stack.clear();
                    Ok(())
                }
                Err(e) => {
                    makepad_widgets::log!("[octoscode] board1 new session: {e}");
                    let mut h = host();
                    h.picker.starting = None;
                    h.picker.error = Some("The session didn’t open. Check the folder, then try again.".to_owned());
                    Err(e)
                }
            }
        }
        Work::Connect { .. }
        | Work::LeaveToForm { .. }
        | Work::Scan
        | Work::Forget
        | Work::ReturnToProviders { .. }
        | Work::OpenUrl(_) => {
            makepad_widgets::log!("[octoscode] board1: {work:?} is the host's to perform");
            Err("board1: host work".to_owned())
        }
    };
    mark_dirty();
    out
}

/// Run one async [`Work`] on the module's runtime and wake the UI when done.
pub fn spawn(work: Work, rt: &tokio::runtime::Runtime, conv: Option<Arc<Conversation>>) {
    rt.spawn(async move {
        let _ = execute(work, conv).await;
        SignalToUI::set_ui_signal();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cx_with_vocabulary() -> Cx {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(makepad_widgets::script_mod);
        cx.with_vm(octoscript_widgets::design::script_mod);
        cx.with_vm(octoscript_widgets::kit::script_mod);
        cx
    }

    /// Evaluate one composed view in the app's VM path (`mount::eval_component`,
    /// the call `MountCache::mount` makes) and require every routed control to
    /// exist in the evaluated tree — a control the DSL lost is a dead tap.
    fn evaluates_with_every_control(cx: &mut Cx, what: &str, ui: &Ui) {
        let view = crate::mount::eval_component(cx, MAIN_SPLASH_VM_ID, &ui.dsl)
            .unwrap_or_else(|e| panic!("{what}: {e}\n{}", ui.dsl));
        for (id, action) in ui.buttons.iter().chain(ui.inputs.iter()) {
            let w = view.widget(cx, &[LiveId::from_str(id)]);
            assert!(!w.is_empty(), "{what}: control {id} ({action}) is not in the evaluated view");
        }
        assert!(!ui.buttons.is_empty(), "{what}: a surface with no control");
    }

    #[test]
    fn every_board1_view_evaluates_at_desktop_and_phone_sizes() {
        let mut cx = cx_with_vocabulary();
        for (w, h) in [(990.0, 603.0), (360.0, 748.0)] {
            for (screen, _) in pairing::Screen::ALL {
                let mut p = pairing::PairingUi::new();
                p.screen = screen;
                p.pairing_host = "127.0.0.1".into();
                p.problem = Some(pairing::Problem::Pairing(
                    octoscode_client::pairing::PairingErrorKind::CodeUnknown,
                ));
                let l = Layout::of(Surface::Pairing, w, h);
                let ui = wrap(pairing::view(&p, &l), &l);
                evaluates_with_every_control(&mut cx, &format!("pairing {screen:?} @{w}"), &ui);
            }
            for rejected in [false, true] {
                let mut p = provider::ProviderUi::new("deepseek");
                if rejected {
                    p.key = "k".into();
                    p.reject("HTTP 401");
                }
                let l = Layout::of(Surface::Provider, w, h);
                let ui = wrap(provider::view(&p, &l), &l);
                evaluates_with_every_control(&mut cx, &format!("provider rejected={rejected} @{w}"), &ui);
            }
            for refused in [false, true] {
                let mut b = browser::BrowserUi {
                    path: "/home/user/code".into(),
                    entries: ["octos", "octoscode-app", "notes", "scratch"]
                        .iter()
                        .map(|n| browser::Entry { name: (*n).into(), path: format!("/home/user/code/{n}") })
                        .collect(),
                    parent: Some("/home/user".into()),
                    writable: true,
                    hidden_skipped: 3,
                    ..Default::default()
                };
                b.pick(1);
                if refused {
                    b.refuse(
                        browser::refusal_from(true, "workspace_list_permission_denied", None).unwrap(),
                        Some("/private".into()),
                    );
                }
                let l = Layout::of(Surface::Browser, w, h);
                let ui = wrap(browser::view(&b, &l), &l);
                evaluates_with_every_control(&mut cx, &format!("browser refused={refused} @{w}"), &ui);
            }
            let pk = PickerUi {
                server_root: Some("/home/user/code".into()),
                recents: vec![("octos".into(), "/home/user/code/octos".into())],
                browse_advertised: true,
                ..Default::default()
            };
            let l = Layout::of(Surface::Picker, w, h);
            let ui = wrap(picker_view(&pk, &l), &l);
            evaluates_with_every_control(&mut cx, &format!("picker @{w}"), &ui);
        }
    }

    /// The tests that walk the live host state run one at a time (the
    /// screens' own unit tests use local values).
    fn live_state() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn the_keyboard_is_dismissed_when_a_surface_rebuilds_or_closes_not_while_typing() {
        let _s = live_state();
        close_all();
        let _ = take_ime_reset();
        route("b1.open.provider", None);
        view(412.0, 794.0);
        assert!(take_ime_reset(), "a fresh surface drops any stale keyboard");
        assert!(!take_ime_reset());
        route("provider.key", Some("sk-typed"));
        view(412.0, 794.0);
        assert!(!take_ime_reset(), "typing keeps the keyboard");
        view(412.0, 700.0);
        assert!(!take_ime_reset(), "a window resized for the keyboard keeps it");
        route("provider.cancel", None);
        assert!(view(412.0, 700.0).is_none());
        assert!(take_ime_reset(), "closing drops the keyboard");
        provider::state().key.clear();
    }

    #[test]
    fn add_workspace_opens_the_browser_over_the_picker_and_fails_closed() {
        let _s = live_state();
        close_all();
        note_context(&Context { capabilities: vec![browser::BROWSE_FEATURE.into()], ..Default::default() });
        let w = route("b1.open.add", None);
        assert!(matches!(w.as_slice(), [Work::PickerLoad, Work::BrowserList { resolve_ancestor: true, .. }]), "{w:?}");
        assert_eq!(top(), Some(Surface::Browser));
        // The browser's back lands on the picker (the web's add -> choose).
        route("browser.close", None);
        assert_eq!(top(), Some(Surface::Picker));
        close_all();
        // Without the advertised feature: the picker alone, no browse request.
        note_context(&Context::default());
        assert_eq!(route("b1.open.add", None), vec![Work::PickerLoad]);
        assert_eq!(top(), Some(Surface::Picker));
        close_all();
    }

    #[test]
    fn the_desktop_dialog_uses_the_webs_widths_and_the_phone_a_sheet() {
        let d = Layout::of(Surface::Pairing, 990.0, 603.0);
        assert!(!d.phone);
        assert_eq!(d.card_w, 440.0);
        assert_eq!(Layout::of(Surface::Browser, 990.0, 603.0).card_w, 540.0);
        // A window narrower than the dialog keeps a 24 px margin each side.
        assert_eq!(Layout::of(Surface::Browser, 580.0, 600.0).card_w, 532.0);
        let p = Layout::of(Surface::Browser, 360.0, 780.0);
        assert!(p.phone);
        assert_eq!(p.card_w, 360.0);
        assert_eq!(p.content_w, 320.0);
    }

    #[test]
    fn a_long_title_shrinks_to_its_row_on_a_narrow_phone() {
        let desk = Layout::of(Surface::Browser, 990.0, 603.0);
        assert_eq!(title_px(&desk, "Choose workspace folder"), 23.0);
        assert_eq!(title_px(&Layout::of(Surface::Pairing, 990.0, 603.0), "Pair with Octos"), 23.0);
        for w in [360.0, 412.0] {
            let p = Layout::of(Surface::Browser, w, 780.0);
            for t in ["Pair with Octos", "Connection", "Edit provider", "Open a workspace", "Choose workspace folder"] {
                let px = title_px(&p, t);
                assert!(px <= 24.0 && px >= 17.0, "{t} at {w}: {px}");
                // The estimated scaled width stays inside the sheet's content.
                assert!(t.chars().count() as f64 * 0.56 * 1.08 * px <= p.content_w, "{t} at {w}: {px}");
            }
        }
        assert_eq!(title_px(&Layout::of(Surface::Pairing, 412.0, 794.0), "Pair with Octos"), 24.0);
    }

    #[test]
    fn the_connect_entry_lands_inside_the_connect_card() {
        let fake = "beauty_0 := DesignSurface {\nbeauty_0_0 := DesignSurface {\nbeauty_0_0_7 := Label {\n}\n}\n}\n";
        let out = with_connect_entry(fake);
        let entry = out.find("b1_connect_pair := ButtonFlat").expect("injected");
        assert!(entry < out.find("beauty_0_0_7").unwrap(), "inside the card, before its last child");
        assert_eq!(with_connect_entry("no anchor"), "no anchor");
    }

    #[test]
    fn every_owned_id_has_exactly_one_owner() {
        let sets: [(&str, fn(&str) -> bool); 3] = [
            ("pairing", pairing::is_action),
            ("provider", provider::is_action),
            ("browser", browser::is_action),
        ];
        let ids: Vec<&str> = pairing::ACTIONS
            .iter()
            .chain(provider::ACTIONS)
            .chain(browser::ACTIONS)
            .chain(OPENERS)
            .chain(PICKER_ACTIONS)
            .map(|(a, _)| *a)
            .collect();
        for id in ids {
            let owners = sets.iter().filter(|(_, f)| f(id)).count()
                + OPENERS.iter().filter(|(a, _)| *a == id).count()
                + PICKER_ACTIONS.iter().filter(|(a, _)| *a == id).count();
            assert_eq!(owners, 1, "{id} has {owners} owners");
            assert!(owns(id), "{id} is not routed by board 1");
        }
    }
}
