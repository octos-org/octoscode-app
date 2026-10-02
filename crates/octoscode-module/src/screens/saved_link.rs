//! A20 — parity row 247 (session:links-resume): a SAVED conversation link is
//! opened only in the exact workspace it names, and a candidate whose
//! workspace differs is refused BEFORE any history is requested.
//!
//! ## The web
//! A saved link is the `?s=` tuple `[workspaceRoot, profileId, sessionId]`
//! (`features/session-links/saved-session-link.ts:11-44`, created from an
//! opened Session's confirmed root, `create-saved-session-url.ts`). It is
//! "an untrusted bookmark … a visible user choice, never an automatic open"
//! (`SavedSessionLinkPanel.tsx:14`): the panel names the server and the
//! workspace, and "Open conversation" opens it with `requireExactWorkspace`
//! (`App.tsx:1795-1830`). A saved link with no workspace is refused before
//! any open (`use-octos-session.ts:3068-3077`); otherwise the candidate is
//! opened on an ISOLATED staging client and refused when the server confirms
//! another root, before `session/hydrate` (`candidate-session.ts:170-186` +
//! `validateCandidateWorkspace`, `:229-243`; e2e `session-links.spec.ts:254`
//! "refuses a saved-link candidate with a different workspace before
//! requesting history"). Fresh launches are not saved links: they may
//! canonicalize the path (`validateCandidateWorkspace`'s
//! `requireExactWorkspace = false` default; `candidate-session.test.ts:178`
//! "accepts the exact saved workspace while fresh launches may canonicalize")
//! — [`validate_candidate_workspace`].
//!
//! ## Natively
//! The link is the one the inspector copies (`board3::inspector::
//! conversation_link`, `octoscode://session?s=<tuple>`), handed to the app at
//! launch as `OCTOSCODE_SESSION_LINK` — the native analog of the web's `?s=`
//! address, exactly as `OCTOS_PAIRING_LINK` is the analog of `?pair=`
//! (`board1::launch_link`). After the connect-time launch, the "Open saved
//! conversation" panel offers it (a board-3 dialog). A native open is NOT
//! isolated — one socket, and `session/open` switches the window — so the
//! refusal moves BEFORE the open: the candidate's workspace is checked
//! against what is known without opening anything ([`precondition`]):
//!
//! 1. the link carries a workspace at all (`use-octos-session.ts:3071`);
//! 2. a workspace this app already confirmed for the Session (an accepted
//!    open's `workspace_root`) must be the link's;
//! 3. the server's attestation of the link's workspace — the read-only
//!    catalog read `session/list {cwd, profile_id}` answers the canonical
//!    root it would open (`SessionListResult.workspace_root`, octos-core
//!    `ui_protocol.rs:3199-3213`) — must be the link's, exactly.
//!
//! Only then does `session/open {cwd: <the link's workspace>}` go out, and the
//! open reply's own exact check (`flow.rs`, row 204) still refuses a root the
//! server answered differently before any `session/hydrate`. A refusal keeps
//! the link and says why on the panel; nothing else changes.
use std::sync::Mutex;

use serde_json::json;

use octoscode_store::Store;

use super::board3::host::Outcome;
use super::board3::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

/// The launch-time handover (the `?s=` address's native analog).
pub const ENV: &str = "OCTOSCODE_SESSION_LINK";

/// `SavedSessionReference` (`saved-session-link.ts:1-5`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedReference {
    pub workspace_root: String,
    pub profile_id: String,
    pub session_id: String,
}

/// The web's bounds (`saved-session-link.ts:7`): workspace, profile, session.
const LIMITS: [usize; 3] = [4_096, 512, 1_024];

/// `parseSavedSessionReference` (`saved-session-link.ts:11-44`) over the
/// tuple, or over the native conversation link that carries it
/// (`octoscode://session?s=<url-encoded tuple>`). "Decode routing intent
/// only. A valid link is never proof a session exists."
pub fn parse_reference(value: &str) -> Option<SavedReference> {
    let value = value.trim();
    let tuple = match value.strip_prefix("octoscode://session?") {
        Some(query) => url::form_urlencoded::parse(query.as_bytes())
            .find(|(k, _)| k == "s")
            .map(|(_, v)| v.into_owned())?,
        None => value.to_owned(),
    };
    if tuple.is_empty() || tuple.len() > 12_000 {
        return None;
    }
    let parts: Vec<String> = serde_json::from_str::<Vec<serde_json::Value>>(&tuple)
        .ok()?
        .into_iter()
        .map(|p| p.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    if parts.len() != 3 {
        return None;
    }
    for (part, limit) in parts.iter().zip(LIMITS) {
        if part.is_empty() || part.trim() != part || part.chars().count() > limit || part.chars().any(is_control_or_format) {
            return None;
        }
    }
    // "Server paths may be POSIX, a Windows drive, or a Windows UNC share."
    if !is_server_path(&parts[0]) {
        return None;
    }
    Some(SavedReference { workspace_root: parts[0].clone(), profile_id: parts[1].clone(), session_id: parts[2].clone() })
}

/// `/[\p{Cc}\p{Cf}]/u` — a control or format character.
fn is_control_or_format(c: char) -> bool {
    c.is_control()
        || matches!(c as u32, 0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x890..=0x891 | 0x8E2 | 0x180E
            | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB
            | 0x110BD | 0x110CD | 0x13430..=0x1343F | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A | 0xE0001 | 0xE0020..=0xE007F)
}

/// `/^(?:\/|[a-z]:[\\/]|\\\\[^\\]+\\[^\\]+)/i`.
fn is_server_path(p: &str) -> bool {
    let b = p.as_bytes();
    if b.first() == Some(&b'/') {
        return true;
    }
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/') {
        return true;
    }
    if let Some(rest) = p.strip_prefix("\\\\") {
        let mut it = rest.splitn(2, '\\');
        let (host, share) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
        return !host.is_empty() && !share.is_empty() && !share.starts_with('\\');
    }
    false
}

/// The web's refusal copy (`candidate-session.ts:240`,
/// `use-octos-session.ts:3074`).
pub const DIFFERENT_WORKSPACE: &str = "The server opened a different workspace from the saved link.";

/// `validateCandidateWorkspace(config, opened, requireExactWorkspace)`
/// (`candidate-session.ts:229-243`): "Saved references already contain a
/// canonical path confirmed by Core", so a saved link needs the EXACT root; a
/// fresh launch (`require_exact = false`) may be canonicalized by the server.
pub fn validate_candidate_workspace(requested_cwd: &str, confirmed_root: Option<&str>, require_exact: bool) -> Result<(), &'static str> {
    if require_exact && (requested_cwd.is_empty() || confirmed_root != Some(requested_cwd)) {
        return Err(DIFFERENT_WORKSPACE);
    }
    Ok(())
}

/// Why a saved-link candidate is refused before anything is opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The plain lead the panel shows.
    pub lead: String,
    /// The two workspaces, side by side.
    pub detail: String,
}

/// A20 — the pre-open precondition (module doc, steps 1-3): `known` = the
/// workspace this app already confirmed for the candidate Session (an
/// accepted open's `workspace_root`), `attested` = the server's canonical
/// root for the link's workspace (the catalog read). Either one that is not
/// EXACTLY the link's refuses the candidate; nothing has been opened.
pub fn precondition(reference: &SavedReference, known: Option<&str>, attested: Option<&str>) -> Result<(), Refusal> {
    let link = reference.workspace_root.as_str();
    if link.trim().is_empty() {
        return Err(Refusal { lead: DIFFERENT_WORKSPACE.into(), detail: "The saved link names no workspace.".into() });
    }
    if let Some(known) = known.filter(|k| !k.trim().is_empty()) {
        if validate_candidate_workspace(link, Some(known), true).is_err() {
            return Err(Refusal {
                lead: "This conversation belongs to a different workspace than the saved link.".into(),
                detail: format!("Saved link: {link}\nThis conversation: {known}\nNothing was opened."),
            });
        }
    }
    if let Some(attested) = attested.filter(|a| !a.trim().is_empty()) {
        if validate_candidate_workspace(link, Some(attested), true).is_err() {
            return Err(Refusal {
                lead: "The server resolves the saved link's workspace to a different folder.".into(),
                detail: format!("Saved link: {link}\nOn the server: {attested}\nNothing was opened."),
            });
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ state

/// The panel's state (what the protocol never carries).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkState {
    /// The link as handed over (`None` = no link pending).
    pub raw: Option<String>,
    /// Its parsed reference (`None` with a link = invalid).
    pub reference: Option<SavedReference>,
    /// The server the panel names (the connection's origin).
    pub server: String,
    pub opening: bool,
    pub error: Option<Refusal>,
}

static STATE: Mutex<LinkState> = Mutex::new(LinkState {
    raw: None,
    reference: None,
    server: String::new(),
    opening: false,
    error: None,
});

fn lock() -> std::sync::MutexGuard<'static, LinkState> {
    STATE.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn snapshot() -> LinkState {
    lock().clone()
}

/// Test seam.
pub fn reset() {
    *lock() = LinkState::default();
}

/// Hold `link` for the panel (parsed now; an invalid one is held too, so the
/// panel can say so — `App.tsx:1878-1890`).
pub fn set_link(link: &str, server: &str) {
    *lock() = LinkState {
        raw: Some(link.to_owned()),
        reference: parse_reference(link),
        server: crate::credentials::origin(server).unwrap_or_else(|| server.to_owned()),
        opening: false,
        error: None,
    };
}

/// Read the launch-time link once (cleared from the environment, as the web
/// strips its address after reading it). Returns whether one is pending.
pub fn take_launch_link(server: &str) -> bool {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    if ONCE.set(()).is_err() {
        return lock().raw.is_some();
    }
    let Ok(link) = std::env::var(ENV) else { return false };
    std::env::remove_var(ENV);
    set_link(&link, server);
    makepad_widgets::log!(
        "[octoscode] saved link handed over at launch ({})",
        if lock().reference.is_some() { "a complete reference" } else { "invalid" }
    );
    true
}

/// After the connect-time launch: offer the pending link on its panel —
/// unless it names the Session already on screen in its own workspace (the
/// web drops a link that is already the active one, `App.tsx:1839-1842`).
pub fn offer(store: &Store) {
    let st = snapshot();
    if st.raw.is_none() {
        return;
    }
    if let Some(r) = &st.reference {
        let active = store.active_session();
        if active.as_deref() == Some(r.session_id.as_str())
            && store.domains.session.workspace_root(&r.session_id).as_deref() == Some(r.workspace_root.as_str())
        {
            reset();
            makepad_widgets::log!("[octoscode] saved link: already the open conversation");
            return;
        }
    }
    let _ = super::board3::host::open(super::board3::host::Dialog::SavedLink);
    super::board3::host::wake();
}

/// The panel's controls.
pub fn perform(action: &str) -> Outcome {
    match action {
        "b3.link.dismiss" => {
            if lock().opening {
                return Outcome::Done;
            }
            reset();
            makepad_widgets::log!("[octoscode] saved link dismissed");
            Outcome::Close
        }
        "b3.link.open" => {
            let mut st = lock();
            if st.opening || st.reference.is_none() {
                return Outcome::Done;
            }
            st.opening = true;
            st.error = None;
            Outcome::Spawn(super::board3::host::Job::SavedLinkOpen)
        }
        _ => Outcome::Unrouted,
    }
}

/// No live conversation: say so on the panel.
pub fn job_unavailable() {
    let mut st = lock();
    st.opening = false;
    st.error = Some(Refusal { lead: "The Octos server connection is not ready.".into(), detail: String::new() });
}

/// How long the open may wait for the server's answer.
const OPEN_WAIT: std::time::Duration = std::time::Duration::from_secs(20);

/// The server's attestation of `cwd` for `profile`: the canonical root a
/// `session/list {cwd, profile_id}` read reports (read-only — it opens,
/// subscribes and hydrates nothing). `None` when the server offers no scoped
/// catalog or does not attest this listing (`profile_id` echoed with the root).
pub async fn attested_root(conv: &crate::flow::Conversation, cwd: &str, profile: &str) -> Option<String> {
    let config = &conv.store.domains.config;
    let offered = config.supported_methods().iter().any(|m| m == "session/list")
        && (config.supported_features().iter().any(|f| f == "session.workspace_cwd.v1")
            || conv.store.capabilities().iter().any(|f| f == "session.workspace_cwd.v1"));
    if !offered {
        return None;
    }
    let reply = conv.client().request("session/list", json!({"cwd": cwd, "profile_id": profile})).await.ok()?;
    let root = reply.get("workspace_root").and_then(|r| r.as_str()).filter(|r| !r.trim().is_empty())?;
    (reply.get("profile_id").and_then(|p| p.as_str()) == Some(profile)).then(|| root.to_owned())
}

/// "Open conversation" (`openSavedLink`, `App.tsx:1795-1830`): the pre-open
/// precondition, then the exact-workspace open.
pub async fn open(conv: &crate::flow::Conversation) -> Result<String, String> {
    let Some(reference) = snapshot().reference else {
        lock().opening = false;
        return Err("no saved link".into());
    };
    let known = conv.store.domains.session.workspace_root(&reference.session_id);
    let attested = attested_root(conv, &reference.workspace_root, &reference.profile_id).await;
    if let Err(refusal) = precondition(&reference, known.as_deref(), attested.as_deref()) {
        makepad_widgets::log!(
            "[octoscode] saved link refused before any open: {} ({})",
            refusal.lead,
            reference.session_id
        );
        let mut st = lock();
        st.opening = false;
        st.error = Some(refusal.clone());
        return Err(refusal.lead);
    }
    // The open carries the link's own Profile and workspace (`App.tsx:1812-
    // 1819`); the reply arm refuses any other confirmed root before hydrate.
    let previous = conv.session_id();
    let previous_cwd = conv.store.domains.session.workspace_root(&previous);
    let previous_profile = conv.profile();
    if reference.profile_id != previous_profile {
        conv.adopt_profile(reference.profile_id.clone());
    }
    let outcome = conv.watch_next_open();
    let sent = conv.open_session(&reference.session_id, Some(reference.workspace_root.clone())).await;
    let refused = match sent {
        Err(e) => Some(e),
        Ok(_) => match tokio::time::timeout(OPEN_WAIT, outcome).await {
            Ok(Ok(Ok(_))) => None,
            Ok(Ok(Err(reason))) => Some(reason),
            _ => Some("The saved conversation could not be opened. Check the server and try again.".into()),
        },
    };
    let Some(reason) = refused else {
        reset();
        super::board3::host::close_if(super::board3::host::Dialog::SavedLink);
        super::board3::host::wake();
        return Ok(format!("opened the saved conversation {}", reference.session_id));
    };
    // Refused after the open (the server confirmed another root, or refused):
    // the Session that was on screen comes back (a failed candidate never
    // interrupts the current one, `use-octos-session.ts:3113-3117`).
    if reference.profile_id != previous_profile {
        conv.adopt_profile(previous_profile);
    }
    if !previous.is_empty() && previous != reference.session_id {
        let _ = conv.open_session(&previous, previous_cwd).await;
    }
    let lead = if reason.contains("instead of") { DIFFERENT_WORKSPACE.to_owned() } else { reason.clone() };
    let mut st = lock();
    st.opening = false;
    st.error = Some(Refusal { lead: lead.clone(), detail: String::new() });
    Err(lead)
}

// ------------------------------------------------------------------- view

/// The panel (`SavedSessionLinkPanel.tsx:15-89`; the invalid link's
/// `App.tsx:1878-1890`), on the board-3 dialog frame.
pub fn build(d: &mut Dsl, frame: &Frame) {
    let st = snapshot();
    let width = frame.dialog_w(520.0);
    ui::shell_open(d, frame, width);
    let Some(reference) = st.reference.clone() else {
        ui::header(d, "This conversation link is invalid", "b3.link.dismiss");
        d.gap(W::Fill, 6.0);
        d.text(
            "b3_link_invalid",
            "It does not contain a complete server workspace and conversation reference.",
            &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Fill).wrap(),
        );
        d.gap(W::Fill, 16.0);
        let foot = d.anon();
        d.view(&foot, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5} spacing: 8");
        d.button("b3_link_dismiss", "Dismiss link", "b3.link.dismiss", Btn::Outline, W::Fit, 34.0);
        d.close();
        ui::shell_close(d);
        return;
    };
    ui::header(d, "Open saved conversation", "b3.link.dismiss");
    d.gap(W::Fill, 4.0);
    d.text(
        "b3_link_desc",
        "Open this conversation on the connected server. Check that the server and workspace match the link you saved.",
        &Txt::new(13.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    d.gap(W::Fill, 12.0);
    ui::body_open(d, frame, width, 190.0);
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 10");
    ui::card_open(d, "b3_link_dest", 8.0);
    row(d, "server", "Server", &st.server, false);
    row(d, "workspace", "Workspace", &reference.workspace_root, true);
    d.close();
    ui::card_open(d, "b3_link_details", 8.0);
    ui::section_title(d, "b3_link_details_title", "Conversation details");
    row(d, "profile", "Profile", &reference.profile_id, true);
    row(d, "session", "Session", &reference.session_id, true);
    d.close();
    d.text(
        "b3_link_note",
        "If the conversation no longer exists, the server may open an empty session. Opening the link does not send a message.",
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    if let Some(e) = &st.error {
        // `role="alert"`: the refusal, red, with the two workspaces under it.
        d.surface(
            "b3_link_alert",
            "width: Fill height: Fit flow: Right spacing: 10 padding: Inset{left: 12 right: 12 top: 10 bottom: 10}",
            tok::RED_BG,
            10.0,
            Some(tok::RED),
        );
        d.icon("b3_link_alert_icon", "b3_warning.svg", 16.0, tok::RED);
        let c = d.anon();
        d.view(&c, "width: Fill height: Fit flow: Down spacing: 4");
        d.text("b3_link_error", &e.lead, &Txt::new(12.5, Face::Medium, tok::RED_TEXT).w(W::Fill).wrap());
        for (i, line) in e.detail.lines().enumerate() {
            d.text(&format!("b3_link_error_{i}"), line, &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill).wrap());
        }
        d.close();
        d.close();
    }
    d.close();
    ui::body_close(d);
    d.gap(W::Fill, 14.0);
    let foot = d.anon();
    d.view(&foot, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5} spacing: 8");
    let (dismiss, primary) = if st.opening { (Btn::Disabled, Btn::Disabled) } else { (Btn::Outline, Btn::Primary) };
    d.button("b3_link_dismiss", "Dismiss link", "b3.link.dismiss", dismiss, W::Fit, 34.0);
    d.button(
        "b3_link_open",
        if st.opening { "Opening conversation…" } else { "Open conversation" },
        "b3.link.open",
        primary,
        W::Fit,
        34.0,
    );
    d.close();
    ui::shell_close(d);
}

/// One `<dt>/<dd>` pair: the muted term over its value.
fn row(d: &mut Dsl, id: &str, term: &str, value: &str, mono: bool) {
    let c = d.anon();
    d.view(&c, "width: Fill height: Fit flow: Down spacing: 2");
    d.text(&format!("b3_link_{id}_term"), term, &Txt::new(11.5, Face::Medium, tok::MUTED).w(W::Fill));
    let face = if mono { Face::Mono } else { Face::Regular };
    d.text(&format!("b3_link_{id}"), value, &Txt::new(12.5, face, tok::TEXT).w(W::Fill).wrap());
    d.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    // saved-session-link.test.ts — "refuses non-web links and incomplete
    // references" + the native link form the inspector copies.
    #[test]
    fn a_saved_reference_parses_from_the_tuple_or_the_native_link_and_nothing_else() {
        let r = parse_reference(r#"["/workspace/saved-link","_main","_main:api:web-saved-link"]"#).unwrap();
        assert_eq!(r.workspace_root, "/workspace/saved-link");
        assert_eq!(r.session_id, "_main:api:web-saved-link");
        let link = crate::screens::board3::inspector::conversation_link("/home/user/ws", "a20", "a20:api:xray").unwrap();
        assert_eq!(
            parse_reference(&link),
            Some(SavedReference { workspace_root: "/home/user/ws".into(), profile_id: "a20".into(), session_id: "a20:api:xray".into() })
        );
        assert!(parse_reference(r#"["C:\\work","p","p:api:s"]"#).is_some(), "a Windows drive");
        assert!(parse_reference(r#"["\\\\host\\share","p","p:api:s"]"#).is_some(), "a UNC share");
        for bad in [
            "",
            "not json",
            r#"["/w","p"]"#,
            r#"["/w","p","s","x"]"#,
            r#"["relative/path","p","p:api:s"]"#,
            r#"["/w"," p","p:api:s"]"#,
            r#"["/w","","p:api:s"]"#,
            r#"["/w","p","p:api:\u0007"]"#,
            r#"["/w","p","p:api:s\u200b"]"#,
            r#"["/w",1,"p:api:s"]"#,
            "octoscode://session?x=1",
        ] {
            assert_eq!(parse_reference(bad), None, "{bad:?} is refused");
        }
        let long = format!(r#"["/w","{}","p:api:s"]"#, "p".repeat(513));
        assert_eq!(parse_reference(&long), None, "the web's bounds");
    }

    // candidate-session.test.ts:178 — "accepts the exact saved workspace
    // while fresh launches may canonicalize" (+ :152 the rejections).
    #[test]
    fn the_exact_saved_workspace_is_accepted_while_fresh_launches_may_canonicalize() {
        assert_eq!(validate_candidate_workspace("/srv/project", Some("/srv/project"), true), Ok(()));
        assert_eq!(validate_candidate_workspace("/srv/project/../project", Some("/srv/project"), false), Ok(()));
        assert_eq!(validate_candidate_workspace("/srv/project", Some("/srv/another-project"), true), Err(DIFFERENT_WORKSPACE));
        assert_eq!(validate_candidate_workspace("/srv/project", None, true), Err(DIFFERENT_WORKSPACE));
        assert_eq!(validate_candidate_workspace("", Some("/srv/project"), true), Err(DIFFERENT_WORKSPACE));
    }

    #[test]
    fn a_candidate_known_or_attested_elsewhere_is_refused_before_any_open() {
        let r = SavedReference { workspace_root: "/home/user/link-ws".into(), profile_id: "a20".into(), session_id: "a20:api:xray".into() };
        assert_eq!(precondition(&r, None, None), Ok(()), "nothing known: the open's own check decides");
        assert_eq!(precondition(&r, Some("/home/user/link-ws"), Some("/home/user/link-ws")), Ok(()));
        let known = precondition(&r, Some("/home/user/real-ws"), None).unwrap_err();
        assert!(known.lead.contains("different workspace") && known.detail.contains("/home/user/real-ws"));
        let attested = precondition(&r, None, Some("/home/user/real-ws")).unwrap_err();
        assert!(attested.lead.contains("different folder") && attested.detail.contains("On the server: /home/user/real-ws"));
        let empty = SavedReference { workspace_root: String::new(), ..r };
        assert_eq!(precondition(&empty, None, None).unwrap_err().lead, DIFFERENT_WORKSPACE);
    }

    #[test]
    fn the_panel_lowers_its_destination_its_refusal_and_its_two_controls() {
        let _g = crate::screens::theme::test_lock();
        set_link(r#"["/home/user/link-ws","a20","a20:api:xray"]"#, "http://127.0.0.1:8485");
        let mut d = Dsl::new();
        build(&mut d, &Frame::DESKTOP);
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        for t in ["Open saved conversation", "/home/user/link-ws", "a20:api:xray", "http://127.0.0.1:8485", "Open conversation", "Dismiss link"] {
            assert!(dsl.contains(t), "{t}");
        }
        let taps = crate::screens::taps::wired_taps(&dsl);
        assert!(taps.iter().any(|(_, e)| e == "b3.link.open") && taps.iter().any(|(_, e)| e == "b3.link.dismiss"));
        assert_eq!(perform("b3.link.open"), Outcome::Spawn(super::super::board3::host::Job::SavedLinkOpen));
        assert_eq!(perform("b3.link.open"), Outcome::Done, "one opening at a time");
        {
            let mut st = lock();
            st.opening = false;
            st.error = Some(Refusal { lead: "The server resolves the saved link's workspace to a different folder.".into(), detail: "Saved link: /a\nOn the server: /b".into() });
        }
        let mut d = Dsl::new();
        build(&mut d, &Frame::DESKTOP);
        let dsl = d.finish();
        assert!(dsl.contains("different folder") && dsl.contains("On the server: /b"));
        assert_eq!(perform("b3.link.dismiss"), Outcome::Close);
        assert_eq!(snapshot(), LinkState::default(), "dismissed: the link is gone");
        set_link("not a link", "http://127.0.0.1:8485");
        let mut d = Dsl::new();
        build(&mut d, &Frame::DESKTOP);
        assert!(d.finish().contains("This conversation link is invalid"));
        assert_eq!(perform("b3.link.open"), Outcome::Done, "an invalid link never opens");
        reset();
    }
}
