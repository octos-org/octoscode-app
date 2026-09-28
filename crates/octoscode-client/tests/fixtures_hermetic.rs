//! Repo-wide guard (outer loop, 2026-09-28): every recorded real-traffic fixture must be hermetic and secret-free.
//! Four lanes shipped machine paths in fixtures despite the lesson; this makes it a build failure instead of a review note.
use std::fs;

#[test]
fn every_fixture_is_hermetic_and_secret_free() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    let mut checked = 0;
    for entry in fs::read_dir(dir).expect("fixtures dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        for bad in ["/Users/", "/var/folders/", "/home/runner/"] {
            assert!(!text.contains(bad), "{name}: machine path `{bad}`: scrub it to <WORKSPACE>/<TMP>/<HOME>");
        }
        for (i, w) in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')).enumerate() {
            assert!(!(w.starts_with("sk-") && w.len() >= 20), "{name}: key-shaped token at word {i}");
        }
        checked += 1;
    }
    assert!(checked >= 8, "expected the recorded fixtures, found {checked}");
}
