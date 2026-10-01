//! #43a — the walk rows' SPECIFIC checks over the recorded real-gate fixture
//! (`r43a-recovery-a6ea8505.jsonl`, live gate 127.0.0.1:50190, dsflash, 3
//! real turns + a 0-turn hydrate supplement).
//!
//! The entry: the recording ships with the row's SPECIFIC check, shown
//! FAILING on the old fixture set and PASSING on the new one. So the second
//! test runs the same check over EVERY other committed fixture and asserts it
//! fails there — the frames these rows need did not exist before this card.
//!
//! Row mapping (docs/phase4-gaps.csv `reason_class=fixture`, joined to
//! docs/walk-rows.csv protocol_methods):
//! - `session/hydrate` (7 rows: 162/174/181/187/188/189/204): an OUT request
//!   with `{session_id}` AND an IN authoritative reply carrying
//!   messages/turns/cursor (+ the pending_* recovery sections);
//! - `peer/staged` (rows 115/117/119): the agent-initiated staging event with
//!   its brief;
//! - `turn/completed` (rows 117/119/204): `turn_terminal` envelopes;
//! - `message/delta` / `message/reasoning_delta` (rows 189/…):
//!   assistant_delta / reasoning_delta envelopes.
use std::fs;
use std::path::{Path, PathBuf};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn lines(path: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(path)
        .expect("fixture readable")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("fixture line is JSON"))
        .collect()
}

/// The SPECIFIC check the 10 fixture-limited rows need (see the module doc).
/// Ok = the frames exist with the real shapes; Err = which frame is missing.
fn check(path: &Path) -> Result<(), String> {
    let v = lines(path);
    let name = path.file_name().unwrap().to_string_lossy().to_string();

    // session/hydrate: an OUT request and its IN authoritative reply.
    let hydrate_out = v
        .iter()
        .find(|f| f["method"] == "session/hydrate" && f["dir"] == "out")
        .ok_or_else(|| format!("{name}: no session/hydrate OUT frame"))?;
    if hydrate_out["body"]["session_id"].as_str().is_none() {
        return Err(format!("{name}: hydrate OUT without session_id"));
    }
    let hydrate_in = v
        .iter()
        .find(|f| f["method"] == "session/hydrate" && f["dir"] == "in")
        .ok_or_else(|| format!("{name}: no session/hydrate IN reply (rows 162/174/181/187/188/189/204)"))?;
    for key in ["messages", "turns", "cursor"] {
        if hydrate_in["body"].get(key).is_none() {
            return Err(format!("{name}: hydrate IN reply missing `{key}`"));
        }
    }
    // The recovery sections the pending-state rows read.
    for key in ["pending_approvals", "pending_questions"] {
        if hydrate_in["body"].get(key).is_none() {
            return Err(format!("{name}: hydrate IN reply missing recovery section `{key}`"));
        }
    }

    // peer/staged: the agent-initiated staging event (rows 115/117/119).
    let staged = v
        .iter()
        .find(|f| f["method"] == "peer/staged")
        .ok_or_else(|| format!("{name}: no peer/staged frame (rows 115/117/119)"))?;
    if staged["body"]["brief"].as_str().map(str::is_empty).unwrap_or(true) {
        return Err(format!("{name}: peer/staged without a brief"));
    }

    // turn/completed: turn_terminal envelopes (rows 117/119/204).
    let terminals = v
        .iter()
        .filter(|f| {
            f["method"] == "projection/envelope" && f["body"]["payload"]["type"] == "turn_terminal"
        })
        .count();
    if terminals == 0 {
        return Err(format!("{name}: no turn_terminal envelope (rows 117/119/204)"));
    }

    // message/delta + message/reasoning_delta envelopes (row 189 and the
    // transcript rows).
    let deltas = v
        .iter()
        .filter(|f| {
            f["method"] == "projection/envelope"
                && matches!(
                    f["body"]["payload"]["type"].as_str(),
                    Some("assistant_delta") | Some("reasoning_delta")
                )
        })
        .count();
    if deltas == 0 {
        return Err(format!("{name}: no assistant/reasoning delta envelopes"));
    }
    Ok(())
}

fn committed_fixtures() -> Vec<PathBuf> {
    let mut out: Vec<_> = fs::read_dir(fixtures_dir())
        .expect("fixtures dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("jsonl"))
        .collect();
    out.sort();
    out
}

/// The new recording carries every frame the 10 fixture-limited rows need.
#[test]
fn r43a_fixture_satisfies_the_fixture_limited_rows() {
    let new = fixtures_dir().join("r43a-recovery-a6ea8505.jsonl");
    check(&new).expect("the new recording passes the rows' specific check");
}

/// The entry's FAIL-on-old: the SAME check fails on every OTHER committed
/// fixture — these frames never existed in the recorded corpus before #43a.
#[test]
fn the_same_check_fails_on_every_preexisting_fixture() {
    let new = fixtures_dir().join("r43a-recovery-a6ea8505.jsonl");
    let others: Vec<PathBuf> = committed_fixtures()
        .into_iter()
        .filter(|p| p != &new)
        .collect();
    assert!(
        others.len() > 10,
        "expected the pre-existing fixture corpus, found {}",
        others.len()
    );
    let mut failures = 0;
    for p in &others {
        if check(p).is_err() {
            failures += 1;
        }
    }
    assert_eq!(
        failures,
        others.len(),
        "the specific check unexpectedly passed on an old fixture — \
         the frames existed before #43a after all"
    );
}
