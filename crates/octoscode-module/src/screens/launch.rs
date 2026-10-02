//! A8 — the WORKSPACE LAUNCH: what happens between "start a session in this
//! folder" and the `session/open`, ported from the web's
//! `features/workspace/launch-model.ts` (the runtime state
//! `idle | resolving | awaiting_choice | opening`),
//! `features/session/launch-transition.ts` (the lease taken BEFORE
//! `launch/resolve`; an older resolver is rejected),
//! `use-octos-session.ts:3010-3060` `resolveInitialLaunch` and
//! `features/workspace/LaunchDecisionPanel.tsx` (the decision panel).
//!
//! When the server advertises `launch/resolve` AND `session.workspace_cwd.v1`
//! a workspace launch asks it first: `resume` / `activate` open at once with
//! the resolved profile; `cross_profile` waits for the person to choose which
//! profile owns the new Session; `no_profile` offers to create the local
//! profile first (the web routes it to its onboarding panel). Without the
//! advertisement the launch opens directly (the web's fallback). The composer
//! text typed while a choice was pending moves to the new Session only once
//! its open COMMITS (`product.spec.ts` "moves drafts only after a
//! profile-choice Session transition commits").
use std::sync::Mutex;

use serde_json::json;

use octoscode_store::Store;

use super::board3::host::Outcome;
use super::board3::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

/// `LaunchRuntimeState.phase` (`launch-model.ts:3-7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Resolving,
    AwaitingChoice,
    Opening,
}

/// `LaunchResolveResult` (octos-core `ui_protocol.rs:3321-3332`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// `resume` | `activate` | `cross_profile` | `no_profile`.
    pub decision: String,
    pub resolved_profile: Option<String>,
    pub existing_profiles: Vec<String>,
}

/// The launch runtime (`LaunchRuntimeState`) plus the transition lease.
#[derive(Debug, Clone, Default)]
pub struct LaunchState {
    pub phase: Phase,
    pub cwd: Option<String>,
    pub decision: Option<Decision>,
    pub error: Option<String>,
    /// The lease generation: `begin` takes a new one; a resolver or an
    /// opening holding an older lease is stale (`launch-transition.ts`).
    pub lease: u64,
}

static STATE: Mutex<LaunchState> = Mutex::new(LaunchState { phase: Phase::Idle, cwd: None, decision: None, error: None, lease: 0 });

fn lock() -> std::sync::MutexGuard<'static, LaunchState> {
    STATE.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn snapshot() -> LaunchState {
    lock().clone()
}

/// Test seam.
pub fn reset() {
    *lock() = LaunchState::default();
}

/// Own the transition BEFORE `launch/resolve` (`launch-transition.ts:25-60`):
/// the new lease retires every older one.
pub fn begin(cwd: &str) -> u64 {
    let mut st = lock();
    st.lease += 1;
    st.phase = Phase::Resolving;
    st.cwd = Some(cwd.to_owned());
    st.decision = None;
    st.error = None;
    st.lease
}

pub fn is_current(lease: u64) -> bool {
    lock().lease == lease
}

/// Whether the server takes the launch probe.
pub fn advertised(store: &Store) -> bool {
    store.domains.config.supported_methods().iter().any(|m| m == "launch/resolve")
        && store.capabilities().iter().any(|f| f == "session.workspace_cwd.v1")
}

/// What a launch came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launched {
    /// The new Session opened (its id).
    Opened(String),
    /// The person has to choose (the decision panel is open).
    AwaitingChoice,
    /// A newer launch owns the transition: this one stopped, silently.
    Stale,
    Failed(String),
}

fn parse_decision(v: &serde_json::Value) -> Option<Decision> {
    Some(Decision {
        decision: v.get("decision")?.as_str()?.to_owned(),
        resolved_profile: v.get("resolved_profile").and_then(|p| p.as_str()).map(str::to_owned),
        existing_profiles: v
            .get("existing_profiles")
            .and_then(|e| e.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
    })
}

/// Open the new Session in `cwd` under `profile` (the connection adopts the
/// chosen profile first when it differs).
async fn open_as(conv: &crate::flow::Conversation, cwd: &str, profile: Option<&str>, lease: u64) -> Launched {
    if !is_current(lease) {
        return Launched::Stale;
    }
    lock().phase = Phase::Opening;
    if let Some(p) = profile.filter(|p| *p != conv.profile()) {
        conv.adopt_profile(p.to_owned());
    }
    match conv.new_chat(Some(cwd.to_owned())).await {
        Ok(id) => {
            let mut st = lock();
            if st.lease == lease {
                *st = LaunchState { lease, ..Default::default() };
            }
            Launched::Opened(id)
        }
        Err(e) => {
            crate::screens::drafts::cancel_carry();
            let mut st = lock();
            if st.lease == lease {
                // A failed candidate returns to the server's choice when there
                // was one (`restoreLaunchChoice`), else to idle with the error.
                st.phase = if st.decision.is_some() { Phase::AwaitingChoice } else { Phase::Idle };
                st.error = Some("The new coding session could not be opened.".into());
            }
            Launched::Failed(e)
        }
    }
}

/// Start a session in `cwd`: resolve first when advertised
/// (`resolveInitialLaunch`), else open directly.
pub async fn create(conv: &crate::flow::Conversation, cwd: String) -> Launched {
    let lease = begin(&cwd);
    if !advertised(&conv.store) {
        return open_as(conv, &cwd, None, lease).await;
    }
    let reply = conv
        .client()
        .request("launch/resolve", json!({"cwd": cwd, "profile_id": conv.profile()}))
        .await;
    // A newer launch took the transition while this one resolved.
    if !is_current(lease) {
        return Launched::Stale;
    }
    let decision = match reply.ok().as_ref().and_then(parse_decision) {
        Some(d) => d,
        None => {
            let mut st = lock();
            st.phase = Phase::Idle;
            st.error = Some("The server could not resolve this workspace.".into());
            return Launched::Failed("launch/resolve".into());
        }
    };
    match decision.decision.as_str() {
        // `resume` always, and `activate` for a freshly minted id, open at
        // once with the resolved profile.
        "resume" | "activate" => {
            let profile = decision.resolved_profile.clone();
            open_as(conv, &cwd, profile.as_deref(), lease).await
        }
        "cross_profile" | "no_profile" => {
            {
                let mut st = lock();
                st.phase = Phase::AwaitingChoice;
                st.decision = Some(decision);
            }
            super::board3::host::open(super::board3::host::Dialog::Launch);
            super::board3::host::wake();
            Launched::AwaitingChoice
        }
        other => {
            let mut st = lock();
            st.phase = Phase::Idle;
            st.error = Some(format!("Unknown launch decision: {other}"));
            Launched::Failed(other.to_owned())
        }
    }
}

/// `chooseLaunchProfile(profile)`: the panel's choice opens the Session; the
/// composer's text follows it once the open commits.
pub async fn choose(conv: &crate::flow::Conversation, profile: String) -> Launched {
    let (lease, cwd) = {
        let st = lock();
        (st.lease, st.cwd.clone())
    };
    let Some(cwd) = cwd else { return Launched::Failed("no pending launch".into()) };
    crate::screens::drafts::carry_next_switch();
    let r = open_as(conv, &cwd, Some(&profile), lease).await;
    if matches!(r, Launched::Opened(_)) {
        super::board3::host::close();
        super::board3::host::wake();
    }
    r
}

/// `no_profile`: create the local profile (`profile/local/create`, the
/// onboarding's first step), then open with it.
pub async fn create_profile_and_open(conv: &crate::flow::Conversation) -> Launched {
    let (lease, cwd) = {
        let st = lock();
        (st.lease, st.cwd.clone())
    };
    let Some(cwd) = cwd else { return Launched::Failed("no pending launch".into()) };
    lock().phase = Phase::Opening;
    match conv.create_profile().await {
        Ok(id) => {
            crate::screens::drafts::carry_next_switch();
            let r = open_as(conv, &cwd, Some(&id), lease).await;
            if matches!(r, Launched::Opened(_)) {
                super::board3::host::close();
                super::board3::host::wake();
            }
            r
        }
        Err(e) => {
            let mut st = lock();
            st.phase = Phase::AwaitingChoice;
            st.error = Some("The profile could not be created.".into());
            Launched::Failed(e.to_string())
        }
    }
}

/// `cancelLaunch`: the pending launch is dropped (its lease retired).
pub fn cancel() {
    let mut st = lock();
    let lease = st.lease + 1;
    *st = LaunchState { lease, ..Default::default() };
}

// ------------------------------------------------------------------ actions

/// Route one `b3.launch.*` action (the panel's controls).
pub fn perform(action: &str, index: usize) -> Outcome {
    let st = snapshot();
    let opening = st.phase == Phase::Opening;
    match action {
        "b3.launch.cancel" => {
            if opening {
                return Outcome::Done;
            }
            cancel();
            Outcome::Close
        }
        "b3.launch.choose" => {
            if opening || st.phase != Phase::AwaitingChoice {
                return Outcome::Done;
            }
            let Some(d) = &st.decision else { return Outcome::Done };
            let choices = choices(d);
            match choices.get(index) {
                Some(p) => Outcome::Spawn(super::board3::host::Job::LaunchChoose(p.clone())),
                None => Outcome::Done,
            }
        }
        "b3.launch.create_profile" => {
            if opening || st.phase != Phase::AwaitingChoice {
                return Outcome::Done;
            }
            Outcome::Spawn(super::board3::host::Job::LaunchCreateProfile)
        }
        _ => Outcome::Unrouted,
    }
}

/// The panel's profile buttons: the resolved profile first, then each
/// existing one (`LaunchDecisionPanel.tsx:84-111`).
pub fn choices(d: &Decision) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(p) = &d.resolved_profile {
        out.push(p.clone());
    }
    for p in &d.existing_profiles {
        if !out.contains(p) {
            out.push(p.clone());
        }
    }
    out
}

// --------------------------------------------------------------------- view

fn choice_button(d: &mut Dsl, id: &str, title: &str, sub: &str, event: Option<&str>) {
    d.surface(
        &format!("{id}_box"),
        "width: Fill height: Fit flow: Overlay",
        if event.is_some() { tok::SURFACE } else { tok::SURFACE2 },
        12.0,
        Some(tok::HAIRLINE),
    );
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 3 padding: Inset{left: 14 right: 14 top: 12 bottom: 12}");
    d.text(&format!("{id}_title"), title, &Txt::new(14.0, Face::Medium, if event.is_some() { tok::TEXT } else { tok::FAINT }).w(W::Fill).wrap());
    d.text(&format!("{id}_sub"), sub, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.close();
    if let Some(ev) = event {
        d.tap(id, ev);
    }
    d.close();
}

/// The decision panel (`LaunchDecisionPanel.tsx:52-128`), in the board-3
/// dialog style.
pub fn build(d: &mut Dsl, frame: &Frame) {
    let st = snapshot();
    let width = frame.dialog_w(520.0);
    let opening = st.phase == Phase::Opening;
    ui::shell_open(d, frame, width);
    d.text("b3_launch_eyebrow", "Workspace launch", &Txt::new(11.5, Face::Medium, tok::MUTED));
    let no_profile = st.decision.as_ref().is_some_and(|x| x.decision == "no_profile");
    let title = if no_profile { "Set up a profile for this workspace" } else { "Choose this workspace’s profile" };
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5}");
    d.text("b3_title", title, &ui::title().w(W::Fill).wrap());
    ui::close_glyph(d, "b3.launch.cancel");
    d.close();
    d.gap(W::Fill, 6.0);
    let body = if no_profile {
        "This server has no profile yet. Create the local profile, then the Session opens in this folder."
    } else {
        "This folder is known to more than one profile. Choose which profile should own the new Session."
    };
    d.text("b3_launch_body", body, &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Fill).wrap());
    d.gap(W::Fill, 8.0);
    // The folder and the choices share the body (one right edge: the body
    // keeps the scroll bar's gutter).
    ui::body_open(d, frame, width, 170.0);
    let list = d.anon();
    d.view(&list, "width: Fill height: Fit flow: Down spacing: 8");
    if let Some(cwd) = &st.cwd {
        // The web's `<code>` folder line: mono text on the grey well (a
        // label, not a field — nothing here is edited or copied).
        d.surface(
            "b3_launch_cwd_field",
            "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 10 right: 10 top: 7 bottom: 7}",
            tok::SURFACE2,
            8.0,
            Some(tok::HAIRLINE),
        );
        d.text("b3_launch_cwd", cwd, &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill).wrap());
        d.close();
    }
    if let Some(e) = &st.error {
        d.text("b3_launch_error", e, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
    }
    d.gap(W::Fill, 4.0);
    if no_profile {
        let ev = (!opening).then_some("b3.launch.create_profile");
        choice_button(d, "b3_launch_create", "Create the local profile", "Then start a coding Session in this folder", ev);
    } else if let Some(dec) = &st.decision {
        for (i, p) in choices(dec).iter().enumerate() {
            let ev = format!("b3.launch.choose#{i}");
            let (title, sub) = if Some(p) == dec.resolved_profile.as_ref() {
                (format!("Start {p} here"), "Create this profile’s coding conversation in the folder")
            } else {
                (format!("Start new session with {p}"), "Use this existing profile for a new Session")
            };
            choice_button(d, &format!("b3_launch_choice_{i}"), &title, sub, (!opening).then_some(ev.as_str()));
        }
    }
    d.close();
    ui::body_close(d);
    d.gap(W::Fill, 12.0);
    let foot = d.anon();
    d.view(&foot, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    d.text(
        "b3_launch_status",
        if opening { "Opening durable session…" } else { "Server decision" },
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill),
    );
    d.button("b3_launch_cancel", "Cancel", "b3.launch.cancel", if opening { Btn::Disabled } else { Btn::Outline }, W::Fit, 34.0);
    d.close();
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_lease_retires_an_older_resolver() {
        let _g = crate::screens::theme::test_lock();
        reset();
        let a = begin("/home/user/a");
        assert_eq!(snapshot().phase, Phase::Resolving);
        let b = begin("/home/user/b");
        assert!(!is_current(a) && is_current(b), "the older resolver is stale");
        cancel();
        assert!(!is_current(b), "a cancel retires the lease too");
        assert_eq!(snapshot().phase, Phase::Idle);
    }

    #[test]
    fn the_panel_offers_the_resolved_profile_first_then_the_existing_ones() {
        let d = Decision { decision: "cross_profile".into(), resolved_profile: Some("a8".into()), existing_profiles: vec!["glm".into(), "a8".into()] };
        assert_eq!(choices(&d), vec!["a8".to_owned(), "glm".to_owned()]);
    }

    #[test]
    fn the_panel_lowers_with_its_choices_and_is_inert_while_opening() {
        let _g = crate::screens::theme::test_lock();
        reset();
        begin("/home/user/octos");
        {
            let mut st = lock();
            st.phase = Phase::AwaitingChoice;
            st.decision = Some(Decision { decision: "cross_profile".into(), resolved_profile: Some("a8".into()), existing_profiles: vec!["glm".into()] });
        }
        let mut d = Dsl::new();
        build(&mut d, &Frame::DESKTOP);
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        for t in ["Choose this workspace’s profile", "Start a8 here", "Start new session with glm", "Server decision"] {
            assert!(dsl.contains(t), "{t}");
        }
        let taps = crate::screens::taps::wired_taps(&dsl);
        assert!(taps.iter().any(|(_, e)| e == "b3.launch.choose#1"));
        lock().phase = Phase::Opening;
        let mut d = Dsl::new();
        build(&mut d, &Frame::DESKTOP);
        let dsl = d.finish();
        assert!(dsl.contains("Opening durable session…"));
        assert!(!crate::screens::taps::wired_taps(&dsl).iter().any(|(_, e)| e.starts_with("b3.launch.choose")), "no choice while opening");
        assert_eq!(perform("b3.launch.cancel", 0), Outcome::Done, "cancel is inert while opening");
    }
}
