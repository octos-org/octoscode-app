//! The shared protocol-id gate — the web's `isProtocolUuid`
//! (`packages/client/src/protocol-id.ts:1-33`), ported value-for-value.
//!
//! #P4f2 row 7: the web validates an approval's `typedDetails.diff.preview_id`
//! through this gate before binding `D` to the diff review
//! (`interaction.ts:94-102`: `isRecord(diff) && isPreviewId(diff.preview_id)`).
//! The rule matters because the comment records WHY: "Non-canonical forms here
//! are accepted by the native decoder, so the shared protocol-id gate must
//! accept them too — otherwise a genuine accepted turn id would be dropped as
//! malformed." So this is not cosmetic: a strict RFC check would refuse a real
//! id the server already accepted.
//!
//! The shapes, exactly as the web accepts them:
//! * hyphenated 8-4-4-4-12, case-insensitive hex;
//! * simple 32 hex digits;
//! * `{...}` braces around either (total length 38);
//! * `urn:uuid:` + the **hyphenated** form only (a URN-wrapped *simple* id is
//!   NOT accepted — Rust routes a 45-byte input only to the hyphenated parser).
//! * nil UUIDs are valid (version-agnostic).
use serde_json::Value;

const HEX: &[u8] = b"0123456789abcdefABCDEF";

fn all_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| HEX.contains(&b))
}

fn is_hyphenated(s: &str) -> bool {
    // 8-4-4-4-12 = 36 chars, hyphens at 8, 13, 18, 23.
    let b = s.as_bytes();
    b.len() == 36
        && s.chars().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c != '-',
        })
        && all_hex(&format!("{}{}{}{}", &s[0..8], &s[9..13], &s[14..18], &s[19..23]))
        && all_hex(&s[24..36])
}

fn is_simple(s: &str) -> bool {
    s.len() == 32 && all_hex(s)
}

fn is_uuid_inner(s: &str) -> bool {
    is_hyphenated(s) || is_simple(s)
}

/// Whether a JSON value is a protocol id the native decoder would accept.
pub fn is_protocol_uuid(value: &Value) -> bool {
    let Some(s) = value.as_str() else {
        return false;
    };
    if is_uuid_inner(s) {
        return true;
    }
    // `{...}` — exactly 38 chars, braces stripped, then either shape.
    if s.len() == 38 && s.starts_with('{') && s.ends_with('}') {
        return is_uuid_inner(&s[1..s.len() - 1]);
    }
    // `urn:uuid:` routes ONLY to the hyphenated parser.
    if let Some(body) = s.strip_prefix("urn:uuid:") {
        return is_hyphenated(body);
    }
    false
}

/// The `PreviewId` string form octos-core prints (`PreviewId(pub Uuid)`,
/// `ui_protocol.rs:666`).
pub fn preview_id_string(id: &octos_core::ui_protocol::PreviewId) -> String {
    id.0.hyphenated().to_string()
}

/// Build a hyphenated preview id without pulling in a clock-dependent value in
/// tests: the caller supplies the 32 hex digits. (Kept private to tests.)
#[cfg(test)]
fn hyphenated(hex: &str) -> Option<String> {
    if hex.len() != 32 || !all_hex(hex) {
        return None;
    }
    Some(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // protocol-id.test.ts's own cases, mirrored.
    #[test]
    fn matches_the_web_protocol_id_gate() {
        let nil = "00000000-0000-0000-0000-000000000000";
        assert!(is_protocol_uuid(&json!(nil)), "nil is valid (version-agnostic)");
        assert!(
            is_protocol_uuid(&json!("018f3e2a-1b2c-7def-8901-234567890abc")),
            "hyphenated"
        );
        assert!(
            is_protocol_uuid(&json!("018F3E2A1B2C7DEF8901234567890ABC")),
            "simple, case-insensitive"
        );
        assert!(is_protocol_uuid(&json!("{018f3e2a-1b2c-7def-8901-234567890abc}")));
        assert!(is_protocol_uuid(&json!("{018f3e2a1b2c7def8901234567890abc}")));
        assert!(is_protocol_uuid(&json!("urn:uuid:018f3e2a-1b2c-7def-8901-234567890abc")));

        // A URN-wrapped SIMPLE id is not an accepted shape.
        assert!(!is_protocol_uuid(&json!("urn:uuid:018f3e2a1b2c7def8901234567890abc")));
        // Not strings.
        assert!(!is_protocol_uuid(&json!(42)));
        assert!(!is_protocol_uuid(&json!(null)));
        assert!(!is_protocol_uuid(&json!([nil])));
        // Too short / too long / wrong separators.
        assert!(!is_protocol_uuid(&json!("018f3e2a-1b2c-7def-8901-234567890ab")));
        assert!(!is_protocol_uuid(&json!("018f3e2a-1b2c-7def-8901-234567890abcd")));
        assert!(!is_protocol_uuid(&json!("018f3e2a1b2c7def8901234567890abcg")));
        // Braces only count at exactly 38 chars.
        assert!(!is_protocol_uuid(&json!("{018f3e2a-1b2c-7def-8901-234567890abc")));
    }

    #[test]
    fn hyphenated_helper_shapes_32_hex() {
        assert_eq!(
            hyphenated("0123456789abcdef0123456789abcdef").as_deref(),
            Some("01234567-89ab-cdef-0123-456789abcdef")
        );
        assert!(hyphenated("short").is_none());
        assert!(hyphenated("0123456789abcdef0123456789abcdeZ").is_none());
    }
}
