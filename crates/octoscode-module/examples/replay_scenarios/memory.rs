//! A36b — the replay server's `memory` scenario (board 5): the Memory
//! dialog and Settings > Capabilities against faithful replies. No model.
//!
//! Two worlds (`--memory-mode`):
//!
//! - `proposal` (default) — SYNTHETIC, `a36-memory-proposal-synthetic.jsonl`
//!   (`tools/fixtures/a36_memory_fixture.py`): octos-cli's own reply shapes
//!   plus the upstream proposal's `profile_id` echo
//!   (`docs/proposals/memory-profile-scope.md`), board 5's content. Search
//!   filters the fixture's hits (and the notes added in this run) by the
//!   query's words and `kinds`, `limit` applies; a load counts a visit; an
//!   ingest validates like the server (`doc:<source>:` ids, documents only)
//!   and adds the note, so a later search finds it.
//! - `today` — RECORDED, `a36-memory-a6ea8505.jsonl` (A36's probe of a
//!   private serve at a6ea8505): the overview is the token identity's
//!   (`admin`) empty memory with no profile named; search / load / ingest
//!   answer `-32603 runtime_unavailable`; an unknown entity `-32170`.
//!
//! `--memory-mode empty`: the proposal's echo, an empty profile.
//! `--memory-recent`: the overview lists the last week's daily notes.
//! `--memory-truncated`: MEMORY.md cut by the server (96 KiB of 140 KiB).
//! `--memory-refuse search` (any method): that method answers the recorded
//! `runtime_unavailable` refusal (the "profile not running" state).
//! `--slow memory/<m>=<ms>` holds a memory reply (the loading state).
//!
//! `mcp/status/list` and `tool/status/list` answer with the RECORDED replies
//! of the same probe (no MCP server configured), re-pointed at the request's
//! Session and Profile, so the Capabilities walk's inventory reads cleanly.
use std::collections::BTreeMap;

use serde_json::{json, Value};

pub const METHODS: [&str; 5] = ["memory/overview", "memory/entity", "memory/search", "memory/load", "memory/ingest"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Proposal,
    Today,
    Empty,
}

#[derive(Clone)]
pub struct MemorySim {
    pub mode: Mode,
    recent: bool,
    truncated: bool,
    refuse: Vec<String>,
    overview: Value,
    hits: Vec<Value>,
    records: BTreeMap<String, Value>,
    pages: BTreeMap<String, Value>,
    /// The notes added in this run (`doc:` records).
    notes: Vec<Value>,
    visits: BTreeMap<String, u64>,
    recorded: BTreeMap<String, Vec<Value>>,
}

fn fixture(file: &str) -> Vec<Value> {
    let path = format!("{}/../octoscode-client/tests/fixtures/{file}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("fixture JSON"))
        .collect()
}

/// RFC 3339 `ago_ms` before now (chrono's `AutoSi`, what serde writes).
fn ts(ago_ms: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::milliseconds(ago_ms)).to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
}

fn not_found(kind: &str, id: &str) -> Value {
    json!({"code": -32170, "message": format!("{kind} not found: {id}"),
           "data": {"rest_status": 404, "kind": "not_found", "resource_type": kind, "identifier": id}})
}

impl MemorySim {
    pub fn load(args: &[String]) -> Self {
        let mode = match args.iter().position(|a| a == "--memory-mode").and_then(|i| args.get(i + 1)).map(String::as_str) {
            Some("today") => Mode::Today,
            Some("empty") => Mode::Empty,
            _ => Mode::Proposal,
        };
        let refuse = args.windows(2).filter(|w| w[0] == "--memory-refuse").map(|w| format!("memory/{}", w[1])).collect();
        let synthetic = fixture("a36-memory-proposal-synthetic.jsonl");
        let mut recorded: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for f in fixture("a36-memory-a6ea8505.jsonl").into_iter().filter(|f| f["dir"] == "in") {
            recorded.entry(f["method"].as_str().unwrap_or("").to_owned()).or_default().push(f["body"].clone());
        }
        let replies = |m: &str| -> Vec<Value> {
            synthetic.iter().filter(|f| f["dir"] == "in" && f["method"] == m).map(|f| f["body"].clone()).collect()
        };
        let overview = replies("memory/overview").first().map(|b| b["overview"].clone()).unwrap_or(Value::Null);
        let hits = replies("memory/search").first().and_then(|b| b["hits"].as_array().cloned()).unwrap_or_default();
        let records = replies("memory/load")
            .into_iter()
            .map(|b| (b["record"]["id"].as_str().unwrap_or("").to_owned(), b))
            .collect();
        let pages =
            replies("memory/entity").into_iter().map(|b| (b["name"].as_str().unwrap_or("").to_owned(), b)).collect();
        let sim = Self {
            mode,
            recent: args.iter().any(|a| a == "--memory-recent"),
            truncated: args.iter().any(|a| a == "--memory-truncated"),
            refuse,
            overview,
            hits,
            records,
            pages,
            notes: Vec::new(),
            visits: BTreeMap::new(),
            recorded,
        };
        println!(
            "[replay-serve] memory: mode={:?} recent={} truncated={} refuse={:?} ({} hits, {} records, {} pages)",
            sim.mode,
            sim.recent,
            sim.truncated,
            sim.refuse,
            sim.hits.len(),
            sim.records.len(),
            sim.pages.len()
        );
        sim
    }

    pub fn handles(method: &str) -> bool {
        METHODS.contains(&method) || method == "mcp/status/list" || method == "tool/status/list"
    }

    fn recorded(&self, method: &str) -> Value {
        self.recorded.get(method).and_then(|v| v.first().cloned()).unwrap_or(Value::Null)
    }

    /// The recorded `-32603 runtime_unavailable` refusal (a6ea8505).
    fn runtime_unavailable(&self) -> Value {
        self.recorded("memory/search")["error"].clone()
    }

    fn with_echo(&self, mut body: Value, profile: &str) -> Value {
        if self.mode != Mode::Today {
            body["profile_id"] = json!(profile);
        } else if let Some(m) = body.as_object_mut() {
            m.remove("profile_id");
        }
        body
    }

    pub fn reply(&mut self, method: &str, p: &Value, session: &str) -> Result<Value, Value> {
        let profile = p["profile_id"].as_str().unwrap_or("dsflash").to_owned();
        // The recorded inventory, re-pointed at the asking Session / Profile.
        if method == "mcp/status/list" || method == "tool/status/list" {
            let mut body = self.recorded(method);
            body["session_id"] = json!(p["session_id"].as_str().unwrap_or(session));
            body["profile_id"] = json!(profile);
            return Ok(body);
        }
        if self.refuse.iter().any(|m| m == method) {
            return Err(self.runtime_unavailable());
        }
        if self.mode == Mode::Today {
            return match method {
                "memory/overview" => Ok(self.recorded("memory/overview")),
                "memory/entity" => Err(not_found("memory_entity", p["name"].as_str().unwrap_or(""))),
                _ => Err(self.runtime_unavailable()),
            };
        }
        match method {
            "memory/overview" => {
                let mut o = self.overview.clone();
                if self.mode == Mode::Empty {
                    o = self.recorded("memory/overview")["overview"].clone();
                } else {
                    // "Updated 2h ago" on every run.
                    o["long_term_updated_at"] = json!(ts(2 * 3_600_000));
                    if !self.recent {
                        o["recent"] = json!([]);
                    }
                    if self.truncated {
                        let base = o["long_term"].as_str().unwrap_or("").to_owned();
                        let mut text = base.clone();
                        let mut n = 0;
                        while text.len() < 96 * 1024 {
                            n += 1;
                            text.push_str(&format!(
                                "\n## Session notes {n}\n- Kept the steer queue's order stable across reconnect {n}.\n- Reviewed the backoff after send {n}.\n"
                            ));
                        }
                        let cut = (0..=96 * 1024).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
                        text.truncate(cut);
                        o["long_term"] = json!(text);
                        o["long_term_truncated"] = json!(true);
                        o["long_term_total_bytes"] = json!(140 * 1024);
                    }
                }
                Ok(self.with_echo(json!({"overview": o}), &profile))
            }
            "memory/search" => {
                let query = p["query"].as_str().unwrap_or("").trim().to_lowercase();
                if query.is_empty() {
                    return Err(json!({"code": -32602, "message": "memory/search: `query` must not be empty"}));
                }
                let kinds: Vec<String> =
                    p["kinds"].as_array().map(|a| a.iter().filter_map(|k| k.as_str().map(str::to_owned)).collect()).unwrap_or_default();
                let limit = p["limit"].as_u64().unwrap_or(10).clamp(1, 50) as usize;
                let words: Vec<&str> = query.split_whitespace().collect();
                // The fixture's hits are the server's ranked answer to ITS
                // query (BM25 + vectors find the backoff note too); any other
                // query matches the words of a title or an abstract.
                let fixture_query = query == "steer queue";
                let notes: Vec<Value> = self
                    .notes
                    .iter()
                    .map(|r| {
                        json!({"id": r["id"], "kind": "document", "source": r["source"], "title": r["title"],
                               "abstract": r["abstract"], "score": 0.6, "timestamp": r["timestamp"], "trust": "untrusted"})
                    })
                    .collect();
                let mut hits: Vec<Value> = self
                    .hits
                    .iter()
                    .chain(notes.iter())
                    .filter(|h| {
                        if fixture_query && self.hits.iter().any(|f| f["id"] == h["id"]) {
                            return true;
                        }
                        let hay = format!("{} {}", h["title"].as_str().unwrap_or(""), h["abstract"].as_str().unwrap_or("")).to_lowercase();
                        words.iter().any(|w| hay.contains(w))
                    })
                    .filter(|h| kinds.is_empty() || kinds.iter().any(|k| h["kind"] == k.as_str()))
                    .cloned()
                    .collect();
                hits.sort_by(|a, b| b["score"].as_f64().partial_cmp(&a["score"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
                hits.truncate(limit);
                Ok(self.with_echo(json!({"hits": hits}), &profile))
            }
            "memory/load" => {
                let id = p["id"].as_str().unwrap_or("").trim().to_owned();
                let mut body = match self.records.get(&id) {
                    Some(b) => b.clone(),
                    None => match self.notes.iter().find(|r| r["id"] == id.as_str()) {
                        Some(r) => json!({"record": r, "page_truncated": false}),
                        None => return Err(not_found("memory_record", &id)),
                    },
                };
                // A load is a visit (the server touches the record first, so
                // the fixture's count already includes the first one).
                let base = body["record"]["visits"].as_u64().unwrap_or(1).saturating_sub(1);
                let seen = self.visits.entry(id.clone()).or_insert(base);
                *seen += 1;
                body["record"]["visits"] = json!(*seen);
                body["record"]["last_visit"] = json!(ts(0));
                Ok(self.with_echo(body, &profile))
            }
            "memory/entity" => {
                let name = p["name"].as_str().unwrap_or("");
                match self.pages.get(name) {
                    Some(b) => Ok(self.with_echo(b.clone(), &profile)),
                    None => Err(not_found("memory_entity", name)),
                }
            }
            "memory/ingest" => {
                let records = p["records"].as_array().cloned().unwrap_or_default();
                if records.is_empty() {
                    return Err(json!({"code": -32602, "message": "memory/ingest: `records` must contain at least one record"}));
                }
                let (mut inserted, mut updated, mut unchanged) = (0, 0, 0);
                for (i, r) in records.iter().enumerate() {
                    let source = r["source"].as_str().unwrap_or("");
                    let id = r["id"].as_str().unwrap_or("");
                    if r["kind"] != "document" {
                        return Err(json!({"code": -32602, "message": format!("memory/ingest: records[{i}]: knowledge pages are written through save_memory / the memory bank, not ingest")}));
                    }
                    let prefix = format!("doc:{source}:");
                    if source.is_empty() || !id.starts_with(&prefix) || id.len() == prefix.len() {
                        return Err(json!({"code": -32602, "message": format!("memory/ingest: records[{i}].id {id:?} must be namespaced as {prefix}<key>")}));
                    }
                    let mut stored = r.clone();
                    stored["trust"] = json!("untrusted");
                    stored["visits"] = json!(0);
                    stored["promoted"] = json!(false);
                    stored["updated_at"] = json!(ts(0));
                    match self.notes.iter_mut().find(|n| n["id"] == id) {
                        Some(n) if n["title"] == stored["title"] && n["abstract"] == stored["abstract"] && n["body"] == stored["body"] => {
                            unchanged += 1
                        }
                        Some(n) => {
                            *n = stored;
                            updated += 1
                        }
                        None => {
                            self.notes.push(stored);
                            inserted += 1
                        }
                    }
                }
                Ok(self.with_echo(
                    json!({"inserted": inserted, "updated": updated, "unchanged": unchanged, "vectors_stored": 0, "embedded": 0}),
                    &profile,
                ))
            }
            _ => Ok(json!({})),
        }
    }
}
