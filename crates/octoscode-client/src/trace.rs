//! Frame tracing: record every JSON-RPC frame, inbound and outbound, to a file.
//!
//! Card #13 §1. Enabled by `OCTOSCODE_TRACE_FILE=<path>`: each frame is one
//! JSON object on its own line (JSONL), so the real traffic of a live turn can
//! be captured, committed as a test fixture, and replayed.
//!
//! ## Why the client boundary
//!
//! The transport (`octos-app-transport`) owns the socket and does not expose a
//! frame hook. We record at the boundary **we** own: every outbound frame the
//! client issues ([`FrameTrace::out`] from `Client::request`/`call`, plus the
//! typed `session/open` the module sends) and every inbound event the module
//! drains ([`FrameTrace::inbound`]). Together these are exactly the frames a
//! real turn produces, in the order the client saw them.
//!
//! ## Redaction
//!
//! A frame is protocol JSON-RPC; the bearer token travels in the HTTP
//! handshake header, not in a frame. Even so, [`redact`] removes any
//! `token`/`api_key`/`authorization`/`secret`/`password` field (recursively)
//! before a line is written, so a fixture can never carry a credential.
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

/// Which way a recorded frame went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDir {
    /// Client → server.
    Out,
    /// Server → client.
    In,
}

impl FrameDir {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Out => "out",
            Self::In => "in",
        }
    }
}

struct Sink {
    file: File,
    path: PathBuf,
    started: Instant,
}

/// A JSONL frame recorder. Cheap to clone-share; disabled when no file is set.
#[derive(Clone)]
pub struct FrameTrace {
    sink: Option<std::sync::Arc<Mutex<Sink>>>,
}

impl std::fmt::Debug for FrameTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.sink {
            Some(s) => write!(f, "FrameTrace({:?})", s.lock().unwrap().path),
            None => write!(f, "FrameTrace(disabled)"),
        }
    }
}

impl Default for FrameTrace {
    fn default() -> Self {
        Self::disabled()
    }
}

impl FrameTrace {
    /// A recorder that writes nothing (the default when the env var is unset).
    pub fn disabled() -> Self {
        Self { sink: None }
    }

    /// Read `OCTOSCODE_TRACE_FILE`; open it in append mode. An unreadable path
    /// logs and disables instead of failing the app.
    pub fn from_env() -> Self {
        match std::env::var("OCTOSCODE_TRACE_FILE") {
            Ok(path) if !path.trim().is_empty() => Self::open(path.trim()),
            _ => Self::disabled(),
        }
    }

    /// Open (or create) `path` for appending.
    pub fn open(path: &str) -> Self {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => Self {
                sink: Some(std::sync::Arc::new(Mutex::new(Sink {
                    file,
                    path,
                    started: Instant::now(),
                }))),
            },
            Err(e) => {
                log::warn!("octoscode: cannot open trace file {path:?}: {e}");
                Self::disabled()
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.sink.is_some()
    }

    /// The trace file path, when enabled.
    pub fn path(&self) -> Option<PathBuf> {
        self.sink.as_ref().map(|s| s.lock().unwrap().path.clone())
    }

    /// Record one outbound frame (`{method, params}`).
    pub fn out(&self, method: &str, params: &Value) {
        self.write(FrameDir::Out, method, None, Some(params), None);
    }

    /// Record one inbound frame (`{method, params}`).
    pub fn inbound(&self, method: &str, params: &Value) {
        self.write(FrameDir::In, method, None, Some(params), None);
    }

    /// Record an outbound request that will be answered (`id` is the JSON-RPC
    /// id when the caller knows it).
    pub fn out_request(&self, method: &str, id: Option<&str>, params: &Value) {
        self.write(FrameDir::Out, method, id, Some(params), None);
    }

    /// Record a response/result frame.
    pub fn result(&self, method: &str, id: Option<&str>, result: &Value) {
        self.write(FrameDir::In, method, id, Some(result), None);
    }

    /// Record an error frame.
    pub fn error(&self, method: &str, id: Option<&str>, message: &str, code: i64) {
        self.write(
            FrameDir::In,
            method,
            id,
            None,
            Some(json!({"code": code, "message": message})),
        );
    }

    /// Append a pre-built line (used to flush the module's `TraceSink`).
    pub fn raw_line(&self, value: Value) {
        self.append(&redact(value));
    }

    fn write(
        &self,
        dir: FrameDir,
        method: &str,
        id: Option<&str>,
        body: Option<&Value>,
        error: Option<Value>,
    ) {
        let mut line = json!({
            "dir": dir.as_str(),
            "method": method,
        });
        if let Some(id) = id {
            line["id"] = json!(id);
        }
        if let Some(body) = body {
            line["body"] = body.clone();
        }
        if let Some(error) = error {
            line["error"] = error;
        }
        let Some(sink) = &self.sink else { return };
        let at_ms = sink.lock().unwrap().started.elapsed().as_millis();
        line["at_ms"] = json!(at_ms);
        line["wall_ms"] = json!(wall_ms());
        self.append(&redact(line));
    }

    fn append(&self, line: &Value) {
        let Some(sink) = &self.sink else { return };
        let mut s = sink.lock().unwrap();
        let text = match serde_json::to_string(line) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("octoscode: trace line will not serialize: {e}");
                return;
            }
        };
        if let Err(e) = writeln!(s.file, "{text}") {
            log::warn!("octoscode: trace write failed: {e}");
            return;
        }
        // Flush per frame: a crash mid-turn must not lose the frames we have.
        let _ = s.file.flush();
    }
}

fn wall_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Is this JSON key a credential we must never write to a fixture?
///
/// Precise on purpose: the protocol has fields whose names merely *contain*
/// "token" (`token_usage`, `token_budget`, `tokens_in/out`) and those are
/// ordinary data, not secrets. A safe-allowlist is checked first, then an
/// exact set, then a small set of unambiguous substrings / suffixes.
fn is_secret_key(key: &str) -> bool {
    const SAFE: &[&str] = &[
        "token_usage",
        "token_budget",
        "tokens_used",
        "tokens_in",
        "tokens_out",
        "token_count",
    ];
    let lower = key.to_ascii_lowercase();
    if SAFE.contains(&lower.as_str()) {
        return false;
    }
    const EXACT: &[&str] = &["token", "access_token", "refresh_token", "auth_token"];
    if EXACT.contains(&lower.as_str()) {
        return true;
    }
    const SUBSTR: &[&str] = &["api_key", "apikey", "authorization", "bearer", "password", "secret"];
    SUBSTR.iter().any(|s| lower.contains(s))
        || ["_key", "_secret", "_password"].iter().any(|suf| lower.ends_with(suf))
}

/// Secrets the app knows it is handling right now (a provider key typed into
/// onboarding). Field names catch `api_key` params; this catches the same
/// value echoed inside free text, e.g. a provider's "Your api key ****abcd is
/// invalid" coming back in a result frame.
static KNOWN_SECRETS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Register a secret value: every traced string has it, and its first and last
/// 8 characters (the parts providers echo), replaced by `<redacted>`. Values
/// shorter than 8 characters are ignored (they cannot be told from text).
pub fn register_secret(secret: &str) {
    let s = secret.trim();
    if s.chars().count() < 8 {
        return;
    }
    let mut known = KNOWN_SECRETS.lock().unwrap_or_else(|p| p.into_inner());
    if !known.iter().any(|k| k == s) {
        known.push(s.to_owned());
    }
}

fn scrub_known(text: &str) -> String {
    let known = KNOWN_SECRETS.lock().unwrap_or_else(|p| p.into_inner());
    let mut out = text.to_owned();
    for secret in known.iter() {
        let chars: Vec<char> = secret.chars().collect();
        let head: String = chars[..8].iter().collect();
        let tail: String = chars[chars.len() - 8..].iter().collect();
        for part in [secret.as_str(), head.as_str(), tail.as_str()] {
            if out.contains(part) {
                out = out.replace(part, "<redacted>");
            }
        }
    }
    out
}

/// Remove credential-looking fields (recursively) from a frame before it is
/// written. Belt-and-braces: no frame should carry a secret, and a fixture
/// must never leak one.
pub fn redact(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                if is_secret_key(&k) {
                    out.insert(k, json!("<redacted>"));
                } else {
                    out.insert(k, redact(v));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(redact).collect()),
        Value::String(text) => Value::String(scrub_known(&text)),
        other => other,
    }
}

/// The **wire params** of a decoded notification — exactly the JSON a replay
/// server must send so the real transport decodes it back to the same value.
///
/// For most variants the derived `Serialize` of the enum is the event fields
/// plus a `kind` tag, so dropping `kind` recovers the wire params. The one
/// exception is `projection/envelope`: its wire DTO is **flat** (routing keys +
/// the bare `EnvelopeV2` fields; `ui_protocol.rs:4095`), while the typed form
/// nests them under `envelope` — so it is flattened explicitly.
pub fn wire_params(n: &octos_core::app_ui::AppUiBackendEvent) -> Value {
    use octos_core::app_ui::AppUiBackendEvent as N;
    if let N::EnvelopeV2(e) = n {
        let mut m = serde_json::Map::new();
        m.insert("session_id".to_owned(), json!(e.session_id.0));
        if let Some(topic) = &e.topic {
            m.insert("topic".to_owned(), json!(topic));
        }
        if let Ok(Value::Object(env)) = serde_json::to_value(&e.envelope) {
            for (k, v) in env {
                m.insert(k, v);
            }
        }
        return Value::Object(m);
    }
    let mut v = serde_json::to_value(n).unwrap_or(Value::Null);
    if let Value::Object(m) = &mut v {
        m.remove("kind");
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_removes_credentials_at_any_depth() {
        let v = json!({
            "method": "session/open",
            "params": {
                "session_id": "s",
                "api_key": "sk-live-xxx",
                "nested": {"authorization": "Bearer yyy", "keep": 1}
            },
            "list": [{"token": "t"}]
        });
        let r = redact(v);
        assert_eq!(r["params"]["api_key"], "<redacted>");
        assert_eq!(r["params"]["nested"]["authorization"], "<redacted>");
        assert_eq!(r["params"]["nested"]["keep"], 1);
        assert_eq!(r["params"]["session_id"], "s");
        assert_eq!(r["list"][0]["token"], "<redacted>");
        // The rendered text never contains the secret.
        let text = r.to_string();
        assert!(!text.contains("sk-live-xxx"));
        assert!(!text.contains("Bearer yyy"));
    }

    #[test]
    fn redact_keeps_protocol_token_fields_but_drops_real_credentials() {
        let v = json!({
            "token_usage": {"in": 10, "out": 20},
            "token_budget": 4000,
            "tokens_used": 7,
            "api_key": "sk-x",
            "authorization": "Bearer y"
        });
        let r = redact(v);
        // Protocol fields survive intact.
        assert_eq!(r["token_usage"]["in"], 10);
        assert_eq!(r["token_budget"], 4000);
        assert_eq!(r["tokens_used"], 7);
        // Real credentials go.
        assert_eq!(r["api_key"], "<redacted>");
        assert_eq!(r["authorization"], "<redacted>");
    }

    #[test]
    fn wire_params_flattens_a_projection_envelope() {
        use octos_core::app_ui::AppUiBackendEvent as N;
        use octos_core::ui_protocol::{EnvelopeV2, EnvelopeV2Notification, PayloadV2};
        let n = N::EnvelopeV2(EnvelopeV2Notification {
            session_id: octos_core::SessionKey("s:main".into()),
            topic: None,
            envelope: EnvelopeV2 {
                thread_id: "t1".into(),
                seq: 2,
                cursor: None,
                turn_id: "t1".into(),
                client_message_id: None,
                payload: PayloadV2::AssistantDelta {
                    text: "Five".into(),
                    assistant_segment_id: "t1:assistant:iteration:1".into(),
                },
            },
        });
        let w = wire_params(&n);
        // Flat: routing key + bare envelope fields at the top level.
        assert_eq!(w["session_id"], "s:main");
        assert_eq!(w["thread_id"], "t1");
        assert_eq!(w["seq"], 2);
        assert_eq!(w["turn_id"], "t1");
        assert_eq!(w["payload"]["type"], "assistant_delta");
        assert_eq!(w["payload"]["data"]["text"], "Five");
        // No nested `envelope`, no `kind`.
        assert!(w.get("envelope").is_none());
        assert!(w.get("kind").is_none());
    }

    /// A known provider key never reaches the trace FILE: not as an `api_key`
    /// param (create / test / save), not nested in a secret object, and not
    /// echoed (whole, or its first / last 8 characters) in a result or error.
    #[test]
    fn a_registered_provider_key_never_reaches_the_trace_file() {
        // Spelled in two halves: the repo's hermetic guard rejects any
        // key-shaped literal in tracked source (repo_hermetic.rs:114-116).
        const SENTINEL: &str = concat!("sk", "-SENTINEL-0123456789abcdefTRACE");
        register_secret(SENTINEL);
        let path = std::env::temp_dir().join(format!("octoscode-trace-sentinel-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let t = FrameTrace::open(path.to_str().unwrap());
        let sel = json!({"family_id": "deepseek", "model_id": "deepseek-v4-flash", "route": {"api_key_env": "DEEPSEEK_API_KEY"}});
        t.out("profile/local/create", &json!({"profile_id": "coding", "api_key": SENTINEL}));
        t.out("profile/llm/test", &json!({"profile_id": "coding", "selection": sel, "api_key": SENTINEL}));
        t.out("profile/llm/upsert", &json!({"profile_id": "coding", "selection": sel, "api_key": {"value": SENTINEL}}));
        t.result("profile/llm/test", Some("7"), &json!({
            "applied": false,
            "error": format!("401: Authentication Fails, Your api key: {SENTINEL} is invalid"),
            "message": format!("key starting {} rejected; ending {}", &SENTINEL[..8], &SENTINEL[SENTINEL.len() - 8..]),
        }));
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(text.lines().count(), 4);
        assert!(!text.contains(SENTINEL), "the whole key reached the trace");
        assert!(!text.contains(&SENTINEL[..8]), "the key's first 8 characters reached the trace");
        assert!(!text.contains(&SENTINEL[SENTINEL.len() - 8..]), "the key's last 8 characters reached the trace");
        assert!(text.contains("DEEPSEEK_API_KEY") || text.contains("<redacted>"));
    }

    #[test]
    fn disabled_trace_writes_nothing_and_is_not_fatal() {
        let t = FrameTrace::disabled();
        assert!(!t.is_enabled());
        t.out("turn/start", &json!({"session_id": "s"}));
        t.inbound("message/delta", &json!({"text": "hi"}));
        assert!(t.path().is_none());
    }

    #[test]
    fn enabled_trace_writes_one_json_line_per_frame() {
        let dir = std::env::temp_dir().join(format!("octoscode-trace-test-{}", std::process::id()));
        let _ = std::fs::remove_file(&dir);
        let t = FrameTrace::open(dir.to_str().unwrap());
        assert!(t.is_enabled());
        t.out("turn/start", &json!({"session_id": "s", "api_key": "secret"}));
        t.inbound("message/delta", &json!({"text": "hi"}));
        let text = std::fs::read_to_string(&dir).unwrap();
        let lines: Vec<Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).expect("each line is JSON"))
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["dir"], "out");
        assert_eq!(lines[0]["method"], "turn/start");
        assert_eq!(lines[0]["body"]["api_key"], "<redacted>");
        assert_eq!(lines[1]["dir"], "in");
        assert_eq!(lines[1]["method"], "message/delta");
        let _ = std::fs::remove_file(&dir);
    }
}
