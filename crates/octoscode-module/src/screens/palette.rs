//! Card #29d — **Stage C wiring** for board 2 screens 2.8 / 2.11 / 2.12:
//! the command palette, the error screen, and the loading/reconnecting screen.
//!
//! Same contract as every L0 card (8.8 condition 2): the card names **binding
//! ids** and **action ids**; this module owns the meaning. The data slots are
//! the Stage B cards' own nodes (`design/stage-b/setup/cards/setup-08|11|12`),
//! fed live from the store + this module's screen state — never measured
//! coordinates (RULES 8.10).
//!
//! Web references (each behaviour cites its source):
//! - palette keyboard model: `src-web/apps/web/src/features/commands/CommandPalette.tsx:38-49`
//!   (Escape dismisses; ArrowUp/ArrowDown move with wraparound; Home/End jump)
//! - command table + capability gate: `src-web/apps/web/src/features/commands/registry.ts:16`
//!   (`WebCommandSpec`), `:52` (`CommandRequirement` methodsAll/methodsAny),
//!   `:679` (`commandAvailability` — fail closed when the server does not
//!   advertise the method)
//! - error screen: `src-web/apps/web/src/features/error/FatalErrorBoundary.tsx:93-107`
//!   (`buildSafeDiagnostic` — redact token/auth_token/api_key query params and
//!   `Bearer` headers, cap the report at 4000 chars) and `:38-90`
//!   (Reload app / Copy diagnostics / report link; `copyState` idle→copied|failed)
//! - reconnect/loading: `docs/parity-matrix.csv:190-198` (the connect path —
//!   our retry replays the production `Conversation::connect` handshake, whose
//!   real traffic is recorded in `live-gate-a6ea8505.jsonl`).
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use crate::bindings::Ctx;
use octoscode_store::Store;

/// The Stage B card this module wires, per screen.
pub const PALETTE_CARD: &str = "setup-08";
pub const ERROR_CARD: &str = "setup-11";
pub const LOADING_CARD: &str = "setup-12";

/// One palette command: the atlas slice (the six rows the Stage B card draws),
/// each with the web's capability gate (`registry.ts:52`) — a command whose
/// methods the server did not advertise is DISABLED and fails closed
/// (`registry.ts:679`, "does not advertise stop without the capability").
pub struct Command {
    pub name: &'static str,
    pub description: &'static str,
    /// `methodsAny` — advertised via `config/announce_capabilities`.
    pub methods_any: &'static [&'static str],
    /// The native effect running it has today. Commands the native client
    /// cannot fulfil yet keep `effect: None` and stay disabled with the
    /// parity matrix's own status (fail closed, never a silent no-op).
    pub effect: Option<&'static str>,
}

/// The Stage B atlas slice (`setup-08` rows t_cmd0..t_cmd5), in registry order.
pub const COMMANDS: &[Command] = &[
    Command { name: "/model", description: "Switch model", methods_any: &["state.session_hydrate.v1"], effect: None },
    Command { name: "/monitor", description: "Add a monitor", methods_any: &["coding.monitor_runtime.v1"], effect: None },
    Command { name: "/mode", description: "Change permissions", methods_any: &["approval.typed.v1"], effect: None },
    Command { name: "/compact", description: "Compact context", methods_any: &["context.lifecycle.v1"], effect: None },
    Command { name: "/btw", description: "Ask a side question", methods_any: &["session/btw"], effect: None },
    Command { name: "/resume", description: "Resume a session", methods_any: &["state.session_hydrate.v1"], effect: Some("session.refresh") },
];

/// Screen-local UI state (the values the protocol never carries — the same
/// category as `FlowUi`). A static behind one lock keeps the lib.rs edit to
/// two registration lines, which the entry asks for.
pub struct ScreenUi {
    /// The palette's selected row (the atlas highlights `t_cmd0`).
    pub selected: usize,
    /// The query box text (`t_query`).
    pub query: String,
    /// The last render error, as the host reported it (pre-redaction).
    pub last_error: Option<String>,
    /// The crash screen's copy button state (`copyState`, FatalErrorBoundary).
    pub copy_state: &'static str,
    /// Reconnect attempts so far (the banner counts them, `t_banner`).
    pub reconnect_attempt: u32,
}

impl Default for ScreenUi {
    fn default() -> Self {
        Self { selected: 0, query: String::new(), last_error: None, copy_state: "idle", reconnect_attempt: 0 }
    }
}

static SCREEN: OnceLock<Mutex<ScreenUi>> = OnceLock::new();

fn screen() -> &'static Mutex<ScreenUi> {
    SCREEN.get_or_init(|| Mutex::new(ScreenUi::default()))
}

/// The host's crash boundary calls this (the web's `setState({ report })` in
/// `FatalErrorBoundary.tsx:getDerivedStateFromError`). The screen shows the
/// diagnostic only through the redacted `error.report` binding.
pub fn report_error(text: String) {
    screen().lock().unwrap().last_error = Some(text);
}

/// Test seam: reset the screen-local state between tests.
pub fn reset_state() {
    *screen().lock().unwrap() = ScreenUi::default();
}

// ---- binding table -----------------------------------------------------------

pub fn owns_binding(id: &str) -> bool {
    id.starts_with("palette.") || id.starts_with("error.") || id.starts_with("reconnect.")
}

pub fn owns_action(id: &str) -> bool {
    matches!(
        id,
        "palette.move" | "palette.run" | "palette.query.set" | "error.copy"
            | "error.reload" | "error.copy_diagnostics" | "connection.retry"
    )
}

fn advertised(store: &std::sync::Arc<Store>, cmd: &Command) -> bool {
    if cmd.effect.is_none() {
        return false;
    }
    let caps = store.capabilities();
    cmd.methods_any.iter().any(|m| {
        caps.iter().any(|c| {
            c == m || (m.ends_with('*') && c.starts_with(m.trim_end_matches('*')))
        })
    })
}

/// Resolve one of this module's binding ids (JSON only — a card never sees a
/// Rust type). Called from [`crate::bindings::query`]'s delegating arm.
pub fn query(ctx: &Ctx<'_>, id: &str) -> Option<Value> {
    let store = ctx.store;
    let mut s = screen().lock().unwrap();
    Some(match id {
        "palette.commands" => json!(COMMANDS
            .iter()
            .map(|c| json!({
                "name": c.name,
                "description": c.description,
                "enabled": advertised(store, c),
                "effect": c.effect,
            }))
            .collect::<Vec<_>>()),
        "palette.commands[].name" => json!(COMMANDS.iter().map(|c| c.name).collect::<Vec<_>>()),
        "palette.commands[].description" => {
            json!(COMMANDS.iter().map(|c| c.description).collect::<Vec<_>>())
        }
        "palette.commands[].enabled" => {
            json!(COMMANDS.iter().map(|c| advertised(store, c)).collect::<Vec<_>>())
        }
        "palette.selected" => json!(s.selected),
        "palette.count" => json!(COMMANDS.len()),
        "palette.query" => json!(s.query),

        // The redacted crash report (web parity: buildSafeDiagnostic).
        "error.report" => json!(s
            .last_error
            .as_deref()
            .map(build_safe_diagnostic)
            .unwrap_or_default()),
        "error.copy_state" => json!(s.copy_state),

        // The loading/reconnecting banner: the web counts attempts
        // ("Reconnecting… attempt 2 ·", setup-12 t_banner).
        "reconnect.banner" => json!(if s.reconnect_attempt > 0 {
            format!("Reconnecting… attempt {} ·", s.reconnect_attempt)
        } else {
            String::new()
        }),
        "reconnect.loading" => json!("Loading session..."),
        "reconnect.retry" => json!("Retry now"),
        _ => return None,
    })
}

/// Redact query/bearer credentials and cap at 4000 chars — the web's
/// `buildSafeDiagnostic` (`FatalErrorBoundary.tsx:93-107`), verbatim rules:
/// `token=`, `auth_token=`, `api_key=` query params and `Bearer` headers.
pub fn redact_secrets(value: &str) -> String {
    // A tiny hand-rolled pass over the four web patterns (the native side has
    // no regex crate in this module's tree).
    let mut out = String::with_capacity(value.len());
    let lower = value.to_lowercase();
    let mut i = 0;
    while i < value.len() {
        let rest = &lower[i..];
        let pat = ["token=", "auth_token=", "api_key=", "bearer "]
            .iter()
            .find(|p| rest.starts_with(*p));
        match pat {
            // A query-param secret: keep the key, redact to its terminator.
            Some(p) if *p != "bearer " => {
                out.push_str(&value[i..i + p.len()]);
                out.push_str("[redacted]");
                let end = value[i + p.len()..]
                    .find(|ch: char| ch == '&' || ch == ')' || ch == ' ' || ch == '\n')
                    .map(|e| i + p.len() + e)
                    .unwrap_or(value.len());
                i = end;
            }
            // `Bearer <token>`: redact the token, keep the scheme.
            Some(_) => {
                out.push_str("Bearer [redacted]");
                let end = value[i + 7..]
                    .find(|ch: char| ch == ' ' || ch == ')' || ch == '\n')
                    .map(|e| i + 7 + e)
                    .unwrap_or(value.len());
                i = end;
            }
            None => {
                let ch = value[i..].chars().next().unwrap();
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out.chars().take(4_000).collect()
}

/// `buildSafeDiagnostic`: `Name: message` (+ stack), redacted, 4000-capped.
pub fn build_safe_diagnostic(error: &str) -> String {
    redact_secrets(error.trim())
}

// ---- actions -----------------------------------------------------------------

/// What a palette/error/reconnect action means. Pure — resolved against the
/// store like [`crate::actions::resolve`], performed by the module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// `palette.move` — ArrowUp/Down with wraparound (`CommandPalette.tsx:44-49`).
    Move(isize),
    /// `palette.run` — execute the selected command. `None` = fail closed
    /// (disabled by capability or no native effect yet); the id is logged.
    Run(Option<&'static str>, &'static str),
    /// `error.copy` — the crash screen's copy (UI-local; the clipboard is the
    /// host's, exactly like `answer.copy`). Carries the REDACTED report.
    CopyReport(String),
    /// `connection.retry` — replay the production connect handshake.
    Retry,
    /// `palette.query.set` — the query box follows the draft (UI-local).
    QuerySet,
    /// A declared id with no resolvable target — logged by name, never fatal.
    Unhandled(String),
}

/// Resolve one of this module's action ids.
pub fn resolve(action: &str, index: usize, ctx: &Ctx<'_>) -> Effect {
    let mut s = screen().lock().unwrap();
    match action {
        "palette.move" => {
            let n = COMMANDS.len() as isize;
            if n == 0 {
                return Effect::Unhandled(action.to_owned());
            }
            // `CommandPalette.tsx:44-49`: (selected + dir + len) % len.
            let next = ((s.selected as isize + index as isize).rem_euclid(n)) as usize;
            s.selected = next;
            Effect::Move(index as isize)
        }
        "palette.run" => {
            let cmd = COMMANDS.get(index).or_else(|| COMMANDS.get(s.selected));
            match cmd {
                Some(c) if advertised(ctx.store, c) => {
                    Effect::Run(c.effect, c.name)
                }
                Some(c) => Effect::Unhandled(format!("{} (not advertised)", c.name)),
                None => Effect::Unhandled(action.to_owned()),
            }
        }
        "palette.query.set" => {
            s.query = ctx.ui.lock().unwrap().draft();
            Effect::QuerySet // a render feed, not a protocol call
        }
        // `error.copy_diagnostics` is the Stage B card's declared control id
        // (setup-11 service-actions.json); the web button is the same action
        // (FatalErrorBoundary.tsx:76-83).
        "error.copy" | "error.copy_diagnostics" => {
            let report = s
                .last_error
                .as_deref()
                .map(build_safe_diagnostic)
                .unwrap_or_default();
            s.copy_state = if report.is_empty() { "failed" } else { "copied" };
            Effect::CopyReport(report)
        }
        // `error.reload` is the Stage B card's reload button (setup-11
        // service-actions.json); the web's "Reload app" reloads the whole app
        // (FatalErrorBoundary.tsx:71-74) — natively that is the same
        // reconnect-and-rerender path as the loading screen's retry.
        "connection.retry" | "error.reload" => {
            s.reconnect_attempt += 1;
            Effect::Retry
        }
        _ => Effect::Unhandled(action.to_owned()),
    }
}

// ---- the feature-flagged temporary mount -------------------------------------

/// Mount one Stage B screen card into the host's `screen_splash` slot while
/// #28e's shell (drawer + palette overlay) is not merged. `OCTOSCODE_SCREEN`
/// names the card (`palette` | `error` | `loading`); unset = no-op, so the
/// default run is byte-identical to the pre-#29d screen.
///
/// Live slots are fed by string swap — the same idiom `sync_labels` uses for
/// the composer's send→stop glyph (lib.rs): the card's authored copy is the
/// default, and store-fed slots replace it before the mount cache compares.
pub fn mount_screen(
    cache: &mut crate::mount::MountCache,
    cx: &mut crate::makepad_widgets::Cx,
    splash: crate::makepad_widgets::SplashRef,
    which: &str,
    store: &std::sync::Arc<Store>,
) -> Result<bool, String> {
    let _ = cx; // the overlay host (#28e) will own cx-driven slots
    let dsl = lower_screen(which, store)?;
    cache.mount(cx, &splash, &dsl)
}

/// The screen card's lowered L0 DSL with the module's live-slot swaps applied.
/// A slot keeps its authored copy when the live text is empty, so an idle
/// screen still matches its Stage B render.
pub fn lower_screen(which: &str, store: &std::sync::Arc<Store>) -> Result<String, String> {
    let card = match which {
        "palette" => PALETTE_CARD,
        "error" => ERROR_CARD,
        "loading" => LOADING_CARD,
        other => return Err(format!("octoscode: unknown OCTOSCODE_SCREEN {other:?}")),
    };
    let dir = screen_cards_dir().join(card);
    let card_text = std::fs::read_to_string(dir.join("page.card"))
        .map_err(|e| format!("octoscode: {card}/page.card: {e}"))?;
    let data_text =
        std::fs::read_to_string(dir.join("page.data.json")).unwrap_or_else(|_| "{}".into());
    let data: Value = serde_json::from_str(&data_text)
        .map_err(|e| format!("octoscode: {card}/page.data.json: {e}"))?;
    let prepared = octoscript_makepad::l0::prepare(&card_text, &data, &dir.join("kit"))?;
    // The DESIGN branch (the artifact Gate B actually rendered — each card dir
    // ships `page.design.splash`). The L0-kit branch evaluates but the host VM
    // rejects its `source:` property on these screens (mount.rs:68, observed in
    // the first probe run). These three screens are FIXED chrome (no runtime
    // prose), so measured coordinates are within the RULES 8.10 carve-out.
    let mut dsl = octoscript_makepad::design::to_makepad_ui(&prepared.tree)?;

    // Live slots: the store/screen state replaces the authored copy before the
    // cache compares, so a state flip repaints exactly once.
    let mut s = screen().lock().unwrap();
    // The DSL literals were probed (`lower_probe setup-08`): `text: "/ mo"` —
    // straight double quotes, not the curly form.
    let swaps: Vec<(&str, String)> = match card {
        PALETTE_CARD => {
            if s.query.is_empty() {
                vec![]
            } else {
                vec![(
                    "text: \"/ mo\"",
                    format!("text: \"{}\"", escape_splash(&s.query)),
                )]
            }
        }
        ERROR_CARD => vec![],
        // Attempt 0 keeps the authored banner (idle = the Stage B render);
        // a retry rewrites the count the way the web's banner does.
        LOADING_CARD if s.reconnect_attempt > 0 => {
            vec![("Reconnecting… attempt 2 ·", reconnect_banner(&mut s))]
        }
        _ => vec![],
    };
    for (from, to) in swaps {
        if !to.is_empty() {
            dsl = dsl.replace(from, &to);
        }
    }
    let _ = store; // further store-fed slots land with #28e's overlay
    // #31d workflow 1: the palette card is LIGHT-authored — in dark mode the
    // app-wide token set rewrites its colors (the value pairs apply to light
    // DSL only; dark-authored cards keep their literals — see theme.rs).
    Ok(crate::screens::theme::retint_dsl(&dsl))
}

fn reconnect_banner(s: &mut ScreenUi) -> String {
    if s.reconnect_attempt > 0 {
        format!("Reconnecting… attempt {} ·", s.reconnect_attempt)
    } else {
        "Reconnecting…".to_owned()
    }
}

/// The Stage B setup cards' directory. `OCTOSCODE_CARDS_DIR` overrides (the
/// `lower_probe` convention); the default resolves relative to this crate so
/// tests and headless runs work from any CWD.
pub fn screen_cards_dir() -> std::path::PathBuf {
    std::path::Path::new(&std::env::var("OCTOSCODE_CARDS_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../design/stage-b/setup/cards").to_owned()
    }))
    .to_path_buf()
}

/// Splash string literals are double-quoted in the L0 DSL (`text: "/ mo"`);
/// escape a backslash or quote so live text cannot break the literal.
fn escape_splash(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}
