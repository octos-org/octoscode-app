//! A9 — why a Connect failed, told honestly.
//!
//! The web cannot tell an unreachable server from a refused token: the
//! browser's WebSocket reports both as "Could not open the Octos UI Protocol
//! connection", so §5.1 classifies that raw error as *unreachable* and the
//! panel adds the honest second half ("Check that your server is running.
//! If it requires authentication, enter its token above.",
//! `ConnectionPanel.tsx:279-306`, `handshakeAmbiguous`).
//!
//! The native transport retries and gives up with no reason either
//! (`octos-app-transport` ws: the dial error is logged, not reported), but a
//! native client CAN ask: one HTTP request to the same endpoint the socket
//! dials (`<origin>/api/ui-protocol/ws`, ws.rs `build_request`) with the
//! same bearer. The server answers a refused or missing token with 401
//! (measured against a live server), a disallowed origin with 403 "Origin
//! not allowed", and a closed port refuses the connection. That verdict
//! becomes the raw error §5.1 already classifies (`connect::failure_for`):
//!
//! | probe                     | raw error                                          | §5.1 kind |
//! |---------------------------|----------------------------------------------------|-----------|
//! | connection refused / DNS  | "Could not open the Octos UI Protocol connection"  | unreachable |
//! | 401 / 403 (token)         | "The server refused this token"                    | rejected-token (focus the field, keep the value) |
//! | 403 "Origin not allowed"  | "Origin not allowed"                               | origin-not-allowed |
//! | any other answer          | "The server answered, but the Octos UI Protocol connection did not open." | unclassified (shown verbatim — never a false diagnosis) |
//! | no answer in time         | the handshake error, flagged AMBIGUOUS             | unreachable + the web's honest second half |
use std::time::Duration;

/// What the probe learned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing is listening (refused, no route, unknown host).
    Unreachable,
    /// The server refused the token (401/403).
    Rejected,
    /// The server refused this client's origin (403 "Origin not allowed").
    OriginNotAllowed,
    /// The server answered and accepted the request: the socket failed for
    /// another reason.
    Reachable,
    /// No answer in time: it cannot tell (the web's ambiguity).
    Inconclusive,
}

/// The raw error the verdict stands for (classified by `connect::failure_for`).
pub fn raw_error(v: Verdict) -> &'static str {
    match v {
        Verdict::Unreachable | Verdict::Inconclusive => "Could not open the Octos UI Protocol connection",
        Verdict::Rejected => "The server refused this token",
        Verdict::OriginNotAllowed => "Origin not allowed",
        Verdict::Reachable => "The server answered, but the Octos UI Protocol connection did not open.",
    }
}

/// The handshake ambiguity's honest second half (`ConnectionPanel.tsx:284-292`),
/// only when the probe could not tell.
pub fn ambiguity_hint(v: Verdict, token: &str) -> Option<&'static str> {
    (v == Verdict::Inconclusive).then(|| {
        if token.trim().is_empty() {
            "Check that your server is running. If it requires authentication, enter its token above."
        } else {
            "Check that your server is running and your token is current."
        }
    })
}

/// The HTTP URL of the endpoint the socket dials: the server's origin with
/// `/api/ui-protocol/ws` (ws/wss map back to http/https).
pub fn probe_url(server: &str) -> Option<String> {
    let mut url = url::Url::parse(server.trim()).ok()?;
    let scheme = match url.scheme() {
        "http" | "ws" => "http",
        "https" | "wss" => "https",
        _ => return None,
    };
    url.set_scheme(scheme).ok()?;
    url.set_query(None);
    url.set_fragment(None);
    url.set_path("/api/ui-protocol/ws");
    Some(url.to_string())
}

/// Map an answer to a verdict (pure, for the tests).
pub fn verdict_of(status: Option<u16>, body: &str, timed_out: bool) -> Verdict {
    match status {
        None if timed_out => Verdict::Inconclusive,
        None => Verdict::Unreachable,
        Some(403) if body.to_ascii_lowercase().contains("origin not allowed") => Verdict::OriginNotAllowed,
        Some(401) | Some(403) => Verdict::Rejected,
        Some(_) => Verdict::Reachable,
    }
}

/// Ask the server once (3 s), with the same bearer the socket used. The
/// token rides only the Authorization header and is never logged.
pub async fn probe(server: &str, token: &str) -> Verdict {
    let Some(url) = probe_url(server) else { return Verdict::Unreachable };
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(_) => return Verdict::Inconclusive,
    };
    let mut req = client.get(&url);
    if !token.trim().is_empty() {
        req = req.bearer_auth(token.trim());
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            verdict_of(Some(status), &body, false)
        }
        Err(e) => verdict_of(None, "", e.is_timeout()),
    }
}

// ------------------------------------------------------- the Connect hook

/// One attempt's probe: (server, token) -> its verdict once it answered.
struct Probe {
    key: (String, String),
    verdict: Option<Verdict>,
}

static PROBE: std::sync::Mutex<Option<Probe>> = std::sync::Mutex::new(None);
/// The ambiguity hint the Connect card shows under the error (or none).
static HINT: std::sync::Mutex<Option<&'static str>> = std::sync::Mutex::new(None);

fn lock<T>(m: &'static std::sync::Mutex<T>) -> std::sync::MutexGuard<'static, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// The hint for the card (`ConnectView::hint`).
pub fn hint() -> Option<&'static str> {
    *lock(&HINT)
}

/// A refused token's focus request: applied after the remounted card has
/// been drawn (a key focus set on a just-mounted, never-drawn input does not
/// stick), so it waits for a later sync.
static FOCUS_AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

pub fn request_token_focus() {
    *lock(&FOCUS_AT) = Some(std::time::Instant::now());
    makepad_widgets::SignalToUI::set_ui_signal();
}

/// Whether the deferred focus is due now (once); keeps the UI ticking until.
pub fn token_focus_due() -> bool {
    let mut f = lock(&FOCUS_AT);
    match *f {
        Some(at) if at.elapsed() >= std::time::Duration::from_millis(60) => {
            *f = None;
            true
        }
        Some(_) => {
            makepad_widgets::SignalToUI::set_ui_signal();
            false
        }
        None => false,
    }
}

static RECHECK_AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// Diagnostics: re-read the focus a moment after it was given.
pub fn schedule_focus_recheck() {
    *lock(&RECHECK_AT) = Some(std::time::Instant::now());
    makepad_widgets::SignalToUI::set_ui_signal();
}

pub fn focus_recheck_due() -> bool {
    let mut f = lock(&RECHECK_AT);
    match *f {
        Some(at) if at.elapsed() >= std::time::Duration::from_millis(300) => {
            *f = None;
            true
        }
        Some(_) => {
            makepad_widgets::SignalToUI::set_ui_signal();
            false
        }
        None => false,
    }
}

/// A new attempt clears the last one's hint.
pub fn clear_hint() {
    *lock(&HINT) = None;
}

/// The transport gave up on the attempt in `ui`: probe once, then classify
/// with the verdict (`connect::ConnectUi::note_connect_error`). Until the
/// probe answers (<= 3 s) the card keeps "Connecting…". Without a runtime
/// the old classification stands, flagged ambiguous. A refused token is
/// also forgotten from the per-origin store (credentials.rs: a known-bad
/// credential is never prefilled again) while the FIELD keeps it.
pub fn on_gave_up(ui: &mut crate::screens::connect::ConnectUi, rt: Option<&tokio::runtime::Handle>) {
    let key = (ui.server.clone(), ui.token.clone());
    let finished = {
        let p = lock(&PROBE);
        p.as_ref().filter(|p| p.key == key).map(|p| p.verdict)
    };
    match (finished, rt) {
        (Some(Some(v)), _) => {
            *lock(&PROBE) = None;
            classify(ui, v);
        }
        (Some(None), _) => {} // still probing
        (None, Some(handle)) => {
            *lock(&PROBE) = Some(Probe { key: key.clone(), verdict: None });
            handle.spawn(async move {
                let v = probe(&key.0, &key.1).await;
                makepad_widgets::log!("[octoscode] a9 connect probe: {v:?}");
                if let Some(p) = lock(&PROBE).as_mut().filter(|p| p.key == key) {
                    p.verdict = Some(v);
                }
                makepad_widgets::SignalToUI::set_ui_signal();
            });
        }
        (None, None) => classify(ui, Verdict::Inconclusive),
    }
}

fn classify(ui: &mut crate::screens::connect::ConnectUi, v: Verdict) {
    ui.note_connect_error(raw_error(v), &crate::screens::connect::clock_12h());
    *lock(&HINT) = ambiguity_hint(v, &ui.token);
    if v == Verdict::Rejected {
        crate::credentials::forget_token(&ui.server);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::connect::{failure_for, FailureKind};

    #[test]
    fn last_tried_reads_the_device_clock_in_12_hours() {
        use crate::screens::connect::{clock_12h, clock_12h_of};
        assert_eq!(clock_12h_of(0, 5), "12:05 AM");
        assert_eq!(clock_12h_of(12, 0), "12:00 PM");
        assert_eq!(clock_12h_of(20, 32), "8:32 PM");
        use chrono::Timelike;
        let now = chrono::Local::now();
        let want = clock_12h_of(now.hour(), now.minute());
        let got = clock_12h();
        // The same wall clock (or the next minute, at a boundary).
        let next = clock_12h_of((now.hour() + (now.minute() + 1) / 60) % 24, (now.minute() + 1) % 60);
        assert!(got == want || got == next, "{got} vs {want}");
    }

    #[test]
    fn the_probe_dials_the_socket_endpoint_over_http() {
        assert_eq!(probe_url("http://127.0.0.1:50190").as_deref(), Some("http://127.0.0.1:50190/api/ui-protocol/ws"));
        assert_eq!(probe_url("wss://octos.example.com/x?y=1").as_deref(), Some("https://octos.example.com/api/ui-protocol/ws"));
        assert_eq!(probe_url("ftp://x"), None);
    }

    #[test]
    fn each_answer_maps_to_its_section_5_1_kind() {
        let kind = |v: Verdict| failure_for(raw_error(v), "http://h:1").map(|f| f.kind);
        assert_eq!(verdict_of(None, "", false), Verdict::Unreachable);
        assert_eq!(kind(Verdict::Unreachable), Some(FailureKind::Unreachable));
        assert_eq!(verdict_of(Some(401), "", false), Verdict::Rejected);
        assert_eq!(verdict_of(Some(403), "forbidden", false), Verdict::Rejected);
        let rejected = failure_for(raw_error(Verdict::Rejected), "http://h:1").unwrap();
        assert_eq!(rejected.kind, FailureKind::RejectedToken);
        assert!(rejected.focus_token, "a refused token focuses the field");
        assert_eq!(verdict_of(Some(403), "Origin not allowed", false), Verdict::OriginNotAllowed);
        assert_eq!(kind(Verdict::OriginNotAllowed), Some(FailureKind::OriginNotAllowed));
        // A server that answered: never a false diagnosis.
        assert_eq!(verdict_of(Some(426), "", false), Verdict::Reachable);
        assert_eq!(kind(Verdict::Reachable), None, "unclassified, shown verbatim");
        // No answer in time: the web's ambiguity, with its honest hint.
        assert_eq!(verdict_of(None, "", true), Verdict::Inconclusive);
        assert_eq!(kind(Verdict::Inconclusive), Some(FailureKind::Unreachable));
        assert_eq!(
            ambiguity_hint(Verdict::Inconclusive, ""),
            Some("Check that your server is running. If it requires authentication, enter its token above.")
        );
        assert_eq!(ambiguity_hint(Verdict::Inconclusive, "t"), Some("Check that your server is running and your token is current."));
        assert_eq!(ambiguity_hint(Verdict::Rejected, ""), None, "a definite verdict needs no hedge");
    }
}
