//! Decision D10c — the makepad remote bridge refuses browser requests.
//!
//! The bridge (`--remote` / `MAKEPAD_REMOTE`) injects real input, serves screen
//! grabs and returns every TextInput's raw buffer. Upstream it answered every
//! request with `Access-Control-Allow-Origin: *` and checked nothing, so a web
//! page could read `/snap` or fire `/click` on a running app.
//! `patches/makepad/remote-browser-guard.patch` closes that; this test pins the
//! patch's shape and, with `MAKEPAD_PATCH_TREES=<root>:<root>:…` (as for
//! tests/a25_makepad_patch.rs), that it is applied in every tree. The APK
//! tree's `remote.rs` differs by its own android-remote edit, so the trees are
//! compared by "applied", not byte for byte.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PATCH: &str = "patches/makepad/remote-browser-guard.patch";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn patch_text() -> String {
    std::fs::read_to_string(repo().join(PATCH)).expect("the tracked patch exists")
}

#[test]
fn the_guard_patch_touches_only_the_bridge_and_drops_the_wildcard_cors() {
    let text = patch_text();
    let touched: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("+++ b/")).collect();
    assert_eq!(touched, ["platform/src/remote.rs"]);
    // The request head is checked before anything is routed.
    assert!(text.contains("+        if refused_head(&head) {"));
    assert!(text.contains("+    fn refused_head(head: &str) -> bool {"));
    // Browser markers and DNS rebinding.
    for needle in ["\"origin\"", "\"referer\"", "starts_with(\"sec-fetch-\")", "name == \"host\" && !host_is_local"] {
        assert!(text.contains(needle), "the guard checks {needle}");
    }
    // A per-launch token on every request: compared in constant time, written
    // 0600 for the harness, never printed (the listening line names the file).
    for needle in [
        "x-makepad-token",
        "fn same_bytes(a: &[u8], b: &[u8]) -> bool",
        "TOKEN.get().is_some_and(|want| token_matches(&head, want))",
        "opts.mode(0o600)",
        "Permissions::from_mode(0o700)",
        "format!(\"mprt_{hex}\")",
        "token_file={token_at}",
        "/dev/urandom",
    ] {
        assert!(text.contains(needle), "the token path carries {needle}");
    }
    assert!(!text.lines().any(|l| l.starts_with('+') && l.contains("println!") && l.contains("{token}")));
    // The wildcard CORS header is removed, never re-added.
    assert!(text.lines().any(|l| l.starts_with('-') && l.contains("Access-Control-Allow-Origin: *")));
    assert!(!text.lines().any(|l| l.starts_with('+') && l.contains("Access-Control-Allow-Origin")));
}

#[test]
fn every_tree_has_the_guard_applied() {
    let Ok(roots) = std::env::var("MAKEPAD_PATCH_TREES") else {
        eprintln!("skipped: set MAKEPAD_PATCH_TREES=<root>:<root>:... to check the trees");
        return;
    };
    for root in roots.split(':').filter(|s| !s.is_empty()) {
        let root = PathBuf::from(root);
        let tree = if root.join("makepad/platform/src").is_dir() { root.join("makepad") } else { root };
        let status = Command::new("patch")
            .args(["-R", "-p1", "-f", "-s", "--dry-run", "-d"])
            .arg(&tree)
            .stdin(std::fs::File::open(repo().join(PATCH)).unwrap())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("patch(1)");
        assert!(status.success(), "the guard is not applied in {}", tree.display());
    }
}
