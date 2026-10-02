//! Pairing — open the client from a server-printed pairing link
//! (WEB-PAIRING-CONTRACT-5100 §Client), ported from the web's
//! `apps/web/src/features/connection/pairing.ts`.
//!
//! Pairing is ONE unauthenticated HTTP request to the server's own origin,
//! made before any socket exists, so it lives beside the protocol client
//! rather than inside it (the web keeps it out of the protocol package for the
//! same reason, `pairing.ts:4-10`). The code is a single-use secret: it is held
//! in memory for exactly one request and is never written to storage or a log
//! — [`PairingLink`] and [`PairingClaim`] redact themselves in `Debug`.
//!
//! | web (`pairing.ts`) | here |
//! |---|---|
//! | `readPairingLink` `:73-84` (both parameters required) | [`read_pairing_link`] |
//! | `loopbackOrigin` `:137-163` (http(s) loopback only, no credentials) | [`loopback_origin`] |
//! | `wellFormedCode` `:165-169` (trim, uppercase, 1..=64) | [`well_formed_code`] |
//! | `claimPairingCode` `:185-205` (POST `<origin>/pair/claim`) | [`claim_pairing_code`] |
//! | `probePairingInfo` `:221-237` (GET `<origin>/pair/info`) | [`probe_pairing_info`] |
//! | `PAIRING_ERROR_COPY` `:44-57` | [`PairingErrorKind::web_copy`] |
//!
//! The server half is octos `crates/octos-cli/src/api/pairing.rs`: both
//! endpoints answer 404 to any non-loopback peer, `/pair/claim` answers
//! `{"error":{"kind":…}}` with one of four wire kinds, and a good claim
//! answers `{"token":…,"server_origin":…}`.
use std::time::Duration;

/// Crockford base32 — the printed alphabet, with no I/L/O/U (`pairing.ts:13`).
pub const PAIRING_CODE_ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// `pairing.ts:14-16`.
pub const MAX_CODE_LENGTH: usize = 64;
pub const MAX_ORIGIN_LENGTH: usize = 2_048;
pub const MAX_TOKEN_LENGTH: usize = 16_384;
/// The web relies on the browser's fetch defaults; a native client must bound
/// the one request itself so a wedged address can never hang the pairing
/// screen (the same 15 s the connect path uses, `lib.rs` connect_now).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Why a pairing did not produce a token (`PairingErrorKind`, `pairing.ts:18-27`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PairingErrorKind {
    /// Wire: a wrong code, or a code already burned by a successful claim.
    CodeUnknown,
    /// Wire: past the server's five-minute window.
    CodeExpired,
    /// Wire: the attempt budget is spent; the code is burned.
    CodeLocked,
    /// Wire (or refused locally): the body/code is not a well-formed code.
    CodeInvalid,
    /// Refused by this client BEFORE any request: the origin is not this computer.
    OriginNotLoopback,
    /// The address answered 404: the server does not offer pairing.
    NotSupported,
    /// No answer, or an answer this client cannot read.
    Unreachable,
}

impl PairingErrorKind {
    /// The kind string (`pairing.ts:18-27`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CodeUnknown => "pair_code_unknown",
            Self::CodeExpired => "pair_code_expired",
            Self::CodeLocked => "pair_code_locked",
            Self::CodeInvalid => "pair_code_invalid",
            Self::OriginNotLoopback => "pair_origin_not_loopback",
            Self::NotSupported => "pair_not_supported",
            Self::Unreachable => "pair_unreachable",
        }
    }

    /// Only the four kinds the server names on the wire (`WIRE_ERROR_KINDS`,
    /// `pairing.ts:29-34`); anything else in an error body is not trusted.
    pub fn from_wire(kind: &str) -> Option<Self> {
        Some(match kind {
            "pair_code_unknown" => Self::CodeUnknown,
            "pair_code_expired" => Self::CodeExpired,
            "pair_code_locked" => Self::CodeLocked,
            "pair_code_invalid" => Self::CodeInvalid,
            _ => return None,
        })
    }

    /// The web's bounded copy per kind, verbatim (`PAIRING_ERROR_COPY`,
    /// `pairing.ts:40-57`). The native screens word their own headline + next
    /// step from the board, but this table is kept so a parity test can pin
    /// that every kind still has exactly one bounded message.
    pub fn web_copy(self) -> &'static str {
        match self {
            Self::CodeUnknown => {
                "That link was already used. Start the server again for a fresh link."
            }
            Self::CodeExpired => "That link expired. Start the server again for a fresh link.",
            Self::CodeLocked => "Too many attempts. Restart the Octos server.",
            Self::CodeInvalid => "That link is malformed. Copy it again from the server.",
            Self::OriginNotLoopback => {
                "That link points to a server that is not on this computer. Pairing links only work for an Octos server on localhost."
            }
            Self::NotSupported => "That server does not offer pairing links. Enter its token below.",
            Self::Unreachable => "Could not reach that Octos server. Check that it is still running.",
        }
    }

    pub const ALL: [PairingErrorKind; 7] = [
        Self::CodeUnknown,
        Self::CodeExpired,
        Self::CodeLocked,
        Self::CodeInvalid,
        Self::OriginNotLoopback,
        Self::NotSupported,
        Self::Unreachable,
    ];
}

/// A parsed pairing link (`PairingLink`, `pairing.ts:59-64`).
///
/// `origin` is the RAW `octos` value — validate it with [`loopback_origin`]
/// before using it. `code` is the raw one-use secret: never persisted, never
/// logged, so `Debug` prints only its length.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingLink {
    pub origin: String,
    pub code: String,
}

impl std::fmt::Debug for PairingLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairingLink")
            .field("origin", &self.origin)
            .field("code", &format_args!("<{} chars>", self.code.chars().count()))
            .finish()
    }
}

/// Read a pairing link out of what the operator pasted or scanned.
///
/// The web reads it from the page's own query string (`readPairingLink`,
/// `pairing.ts:73-84`): `?octos=<server origin>&pair=<code>`, BOTH required —
/// one alone is not a pairing link. A native client has no address bar, so it
/// accepts the same parameters out of any of the shapes the operator can hold:
/// the full printed link (`https://app.example/?octos=…&pair=…`, which is what
/// `octos serve --web-url` prints), the bare query, or the app's own
/// `octos://pair?…` deep link (the board's placeholder spells the code
/// parameter `code`, so `code` is read as an alias of `pair`).
///
/// Values are form-decoded like `URLSearchParams`. The origin is cut at
/// [`MAX_ORIGIN_LENGTH`] and the code at `MAX_CODE_LENGTH + 1`, so an
/// over-long code stays detectable (it fails [`well_formed_code`]) instead of
/// being silently truncated into a valid one (`pairing.ts:80-83`).
pub fn read_pairing_link(text: &str) -> Option<PairingLink> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    // The query: everything after the first `?` (a fragment never carries the
    // parameters), or the whole text when it is already a bare `k=v&k=v` run.
    let query = match t.split_once('?') {
        Some((_, q)) => q,
        None if t.contains('=') => t,
        None => return None,
    };
    let query = query.split('#').next().unwrap_or("");
    let mut origin = String::new();
    let mut code = String::new();
    let mut code_alias = String::new();
    for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
        match k.as_ref() {
            // The FIRST value wins, as `URLSearchParams.get` does.
            "octos" if origin.is_empty() => origin = v.trim().to_owned(),
            "pair" if code.is_empty() => code = v.trim().to_owned(),
            "code" if code_alias.is_empty() => code_alias = v.trim().to_owned(),
            _ => {}
        }
    }
    let code = if code.is_empty() { code_alias } else { code };
    if origin.is_empty() || code.is_empty() {
        return None;
    }
    Some(PairingLink {
        origin: origin.chars().take(MAX_ORIGIN_LENGTH).collect(),
        code: code.chars().take(MAX_CODE_LENGTH + 1).collect(),
    })
}

/// Canonicalize an `octos` value, or `None` when it is not an http(s) origin
/// on THIS computer (`loopbackOrigin`, `pairing.ts:137-163`). Any path, query
/// or fragment is discarded — only the origin is ever used — and credentials
/// in the address are refused outright.
pub fn loopback_origin(value: &str) -> Option<String> {
    let url = url::Url::parse(value.trim()).ok()?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    if !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let host = url.host()?;
    let loopback = match host {
        url::Host::Domain(d) => {
            let d = d.to_ascii_lowercase();
            d == "localhost" || d.ends_with(".localhost")
        }
        url::Host::Ipv4(ip) => ip.is_loopback(),
        url::Host::Ipv6(ip) => ip.is_loopback(),
    };
    loopback.then(|| url.origin().ascii_serialization())
}

/// `wellFormedCode` (`pairing.ts:165-169`): trimmed, uppercased, 1..=64 chars.
pub fn well_formed_code(code: &str) -> Option<String> {
    let canonical = code.trim().to_uppercase();
    let n = canonical.chars().count();
    (n > 0 && n <= MAX_CODE_LENGTH).then_some(canonical)
}

/// What a good claim hands back (`PairingClaim`, `pairing.ts:171-174`). The
/// token is the server's API bearer token: `Debug` never prints it.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingClaim {
    pub token: String,
    /// The origin to connect to: the server's echo when it is itself a
    /// loopback origin, else the link's own origin (`readClaim`, `:283-294`).
    pub server_origin: String,
}

impl std::fmt::Debug for PairingClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairingClaim")
            .field("token", &format_args!("<{} chars>", self.token.chars().count()))
            .field("server_origin", &self.server_origin)
            .finish()
    }
}

/// `PairingResult` (`pairing.ts:176-178`).
pub type PairingResult = Result<PairingClaim, PairingErrorKind>;

/// The validation half of [`claim_pairing_code`]: the canonical origin and
/// code, or the kind this client refuses with WITHOUT a request
/// (`pairing.ts:189-192`). Split out so a caller can refuse before it shows a
/// spinner, and so the "no request" property is testable on its own.
pub fn validate_link(link: &PairingLink) -> Result<(String, String), PairingErrorKind> {
    let origin = loopback_origin(&link.origin).ok_or(PairingErrorKind::OriginNotLoopback)?;
    let code = well_formed_code(&link.code).ok_or(PairingErrorKind::CodeInvalid)?;
    Ok((origin, code))
}

fn http() -> Option<reqwest::Client> {
    // `redirect: "error"` + `credentials: "omit"` + no referrer (`:249-256`):
    // never follow a redirect (a 3xx is read as an unusable answer), carry no
    // cookie store, and send no Referer (reqwest sends none).
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(REQUEST_TIMEOUT)
        .build()
        .ok()
}

/// POST the code to `<octos>/pair/claim` and read back the API token
/// (`claimPairingCode`, `pairing.ts:185-205`).
///
/// - a non-loopback origin or a malformed code is refused with NO request;
/// - no answer → [`PairingErrorKind::Unreachable`];
/// - `404` → [`PairingErrorKind::NotSupported`] (the server's loopback gate and
///   a pairing-less deployment both answer 404, `pairing.rs:325-338`);
/// - any other non-2xx → the body's wire kind, else `Unreachable`;
/// - 2xx → the claim, or `Unreachable` when the body is not one.
pub async fn claim_pairing_code(link: &PairingLink) -> PairingResult {
    let (origin, code) = validate_link(link)?;
    let client = http().ok_or(PairingErrorKind::Unreachable)?;
    let response = client
        .post(format!("{origin}/pair/claim"))
        .header("content-type", "application/json")
        .body(serde_json::json!({ "code": code }).to_string())
        .send()
        .await
        .map_err(|_| PairingErrorKind::Unreachable)?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(PairingErrorKind::NotSupported);
    }
    let body: Option<serde_json::Value> = response.json().await.ok();
    if !status.is_success() {
        return Err(body
            .as_ref()
            .and_then(wire_error_kind)
            .unwrap_or(PairingErrorKind::Unreachable));
    }
    body.as_ref()
        .and_then(|b| read_claim(b, &origin))
        .ok_or(PairingErrorKind::Unreachable)
}

/// `wireErrorKind` (`pairing.ts:270-276`): `{ "error": { "kind": <wire kind> } }`.
fn wire_error_kind(body: &serde_json::Value) -> Option<PairingErrorKind> {
    body.get("error")?
        .get("kind")?
        .as_str()
        .and_then(PairingErrorKind::from_wire)
}

/// `readClaim` (`pairing.ts:278-294`).
fn read_claim(body: &serde_json::Value, origin: &str) -> Option<PairingClaim> {
    let token = body.get("token")?.as_str()?;
    if token.is_empty() || token.chars().count() > MAX_TOKEN_LENGTH {
        return None;
    }
    let echoed = body
        .get("server_origin")
        .and_then(|v| v.as_str())
        .and_then(loopback_origin);
    Some(PairingClaim {
        token: token.to_owned(),
        server_origin: echoed.unwrap_or_else(|| origin.to_owned()),
    })
}

/// `PairingInfo` (`pairing.ts:207-212`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingInfo {
    pub product: String,
    pub version: String,
    pub pairing_required: bool,
    pub server_origin: String,
}

/// `PairingProbe` (`pairing.ts:214-219`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingProbe {
    Available(PairingInfo),
    /// 404 — this server simply does not do pairing. Never a complaint.
    Unsupported,
    /// Unreachable, or an answer this client cannot read. Silent either way.
    Unavailable,
}

/// One unauthenticated GET on one origin, never a port range
/// (`probePairingInfo`, `pairing.ts:221-237`).
pub async fn probe_pairing_info(origin: &str) -> PairingProbe {
    let Some(canonical) = loopback_origin(origin) else {
        return PairingProbe::Unavailable;
    };
    let Some(client) = http() else {
        return PairingProbe::Unavailable;
    };
    let response = match client
        .get(format!("{canonical}/pair/info"))
        .header("accept", "application/json")
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return PairingProbe::Unavailable,
    };
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return PairingProbe::Unsupported;
    }
    if !response.status().is_success() {
        return PairingProbe::Unavailable;
    }
    let body: Option<serde_json::Value> = response.json().await.ok();
    match body.as_ref().and_then(|b| read_info(b, &canonical)) {
        Some(info) => PairingProbe::Available(info),
        None => PairingProbe::Unavailable,
    }
}

/// `readInfo` (`pairing.ts:296-309`): only an `octos` product counts.
fn read_info(body: &serde_json::Value, origin: &str) -> Option<PairingInfo> {
    if body.get("product")?.as_str()? != "octos" {
        return None;
    }
    let echoed = body
        .get("server_origin")
        .and_then(|v| v.as_str())
        .and_then(loopback_origin);
    Some(PairingInfo {
        product: "octos".to_owned(),
        version: body
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned(),
        pairing_required: body.get("pairing_required").and_then(|v| v.as_bool()) == Some(true),
        server_origin: echoed.unwrap_or_else(|| origin.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_parameters_are_required() {
        // pairing.test.ts:25 "needs both parameters"
        assert_eq!(read_pairing_link("?octos=http://127.0.0.1:50080"), None);
        assert_eq!(read_pairing_link("?pair=ABCD2345"), None);
        assert_eq!(read_pairing_link(""), None);
        assert_eq!(read_pairing_link("octos://pair"), None);
        assert_eq!(read_pairing_link("octos://pair?code=ABCD2345"), None);
    }

    #[test]
    fn the_printed_link_the_query_and_the_deep_link_all_read() {
        let want = PairingLink {
            origin: "http://127.0.0.1:50080".into(),
            code: "3QK7ZP2M".into(),
        };
        // `octos serve --web-url` prints exactly this shape (pairing.rs:289).
        assert_eq!(
            read_pairing_link("https://app.example.com/?octos=http://127.0.0.1:50080&pair=3QK7ZP2M"),
            Some(want.clone())
        );
        assert_eq!(read_pairing_link("?octos=http%3A%2F%2F127.0.0.1%3A50080&pair=3QK7ZP2M"), Some(want.clone()));
        assert_eq!(read_pairing_link("octos://pair?octos=http://127.0.0.1:50080&code=3QK7ZP2M"), Some(want.clone()));
        // The fragment never carries the parameters.
        assert_eq!(read_pairing_link("https://a/?octos=http://127.0.0.1:50080&pair=3QK7ZP2M#x"), Some(want));
    }

    #[test]
    fn an_over_long_code_stays_detectable() {
        let long = "A".repeat(MAX_CODE_LENGTH + 10);
        let link = read_pairing_link(&format!("?octos=http://127.0.0.1:1&pair={long}")).unwrap();
        assert_eq!(link.code.chars().count(), MAX_CODE_LENGTH + 1);
        assert_eq!(validate_link(&link), Err(PairingErrorKind::CodeInvalid));
    }

    #[test]
    fn only_http_loopback_origins_are_accepted() {
        // pairing.test.ts:119 "accepts only http(s) loopback addresses"
        assert_eq!(loopback_origin("http://127.0.0.1:50080/x?y#z").as_deref(), Some("http://127.0.0.1:50080"));
        assert_eq!(loopback_origin("http://localhost:3000").as_deref(), Some("http://localhost:3000"));
        assert_eq!(loopback_origin("https://app.localhost").as_deref(), Some("https://app.localhost"));
        assert_eq!(loopback_origin("http://[::1]:8080").as_deref(), Some("http://[::1]:8080"));
        assert_eq!(loopback_origin("http://127.9.9.9").as_deref(), Some("http://127.9.9.9"));
        // pairing.test.ts:131 "refuses anything that is not this computer"
        for bad in [
            "http://192.168.1.20:50190",
            "http://10.0.0.2",
            "https://octos.example.com",
            "ws://127.0.0.1:1",
            "file:///etc/passwd",
            "http://user:pw@127.0.0.1:1",
            "not a url",
        ] {
            assert_eq!(loopback_origin(bad), None, "{bad} must be refused");
        }
    }

    #[test]
    fn the_code_is_trimmed_and_uppercased() {
        assert_eq!(well_formed_code(" 3qk7zp2m ").as_deref(), Some("3QK7ZP2M"));
        assert_eq!(well_formed_code("   "), None);
    }

    #[test]
    fn debug_never_prints_the_secrets() {
        let link = PairingLink { origin: "http://127.0.0.1:1".into(), code: "SECRETCODE".into() };
        assert!(!format!("{link:?}").contains("SECRETCODE"));
        let claim = PairingClaim { token: "tok-secret".into(), server_origin: "http://127.0.0.1:1".into() };
        assert!(!format!("{claim:?}").contains("tok-secret"));
    }

    #[test]
    fn only_the_four_wire_kinds_are_trusted() {
        for k in PairingErrorKind::ALL {
            let wire = PairingErrorKind::from_wire(k.as_str());
            match k {
                PairingErrorKind::CodeUnknown
                | PairingErrorKind::CodeExpired
                | PairingErrorKind::CodeLocked
                | PairingErrorKind::CodeInvalid => assert_eq!(wire, Some(k)),
                _ => assert_eq!(wire, None, "{k:?} is client-side only"),
            }
            assert!(!k.web_copy().is_empty());
        }
    }
}
