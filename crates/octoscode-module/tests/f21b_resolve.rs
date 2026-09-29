//! Card #21b — the running app resolved `design/components/` from the *process
//! cwd*, which differs from the test harness (tests run from the repo root), so
//! the launched app fell back to the placeholders. This test runs `resolve` with
//! the cwd set to a temp dir and proves every screen component still resolves
//! **on disk** (card #21b step 1).
//!
//! It is its own integration-test binary (one test) so the process-global cwd
//! change cannot race other tests.
use octoscode_module::components::{self, ItemKind};

#[test]
fn resolve_finds_components_with_cwd_outside_the_repo() {
    // A cwd with no `design/components` under it — the launched-app situation.
    let tmp = std::env::temp_dir().join(format!("octoscode-21b-cwd-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("temp dir");
    std::env::set_current_dir(&tmp).expect("chdir");
    assert!(
        !std::path::Path::new("design/components").is_dir(),
        "the temp cwd must not contain design/components"
    );

    // The candidate roots must include an exe-relative walk-up (the installed app
    // case), so resolve does not depend on the process cwd.
    let candidates = components::components_dir_candidates();
    assert!(
        candidates.iter().any(|p| p.ends_with("design/components")),
        "no design/components candidate: {candidates:?}"
    );

    // The kit actually used must carry the real component set, not just the
    // shared placeholder kit — every id resolves on disk with no placeholder.
    for kind in ItemKind::ALL {
        let (component, on_disk) = components::resolve(*kind);
        assert!(on_disk, "{} fell back to the placeholder (cwd-independent resolve broken)", kind.id());
        assert!(
            !component.ledger.contains("PLACEHOLDER component"),
            "{} lowered the PLACEHOLDER ledger",
            kind.id()
        );
    }

    let _ = std::fs::remove_dir(&tmp);
}
