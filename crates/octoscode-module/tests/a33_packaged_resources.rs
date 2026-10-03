//! A33 / decision D10f — the standalone OctosCode app builds against OUR makepad.
//!
//! 1. `patches/makepad/packaged-file-resource.patch`: a packaged build
//!    (`MAKEPAD_PACKAGE_DIR`, the self-contained OctosCode.app) still reads a
//!    `file_resource(<absolute path>)` from the filesystem. Without it every
//!    SVG icon the module names by its materialized path drew nothing in the
//!    app bundle. This pins the patch's shape and that it carries its unit
//!    test.
//! 2. This workspace compiles makepad from the fork that carries every
//!    `patches/makepad/*.patch` (`tools/prepare-makepad-fork.sh`, the root
//!    Cargo.toml's `[patch]`), so the suite builds the notification API, the
//!    `--remote` token guard and the ellipsis fix too. When the fork is
//!    present (every build: the `[patch]` needs it), each patch must be
//!    applied in it.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PATCH: &str = "patches/makepad/packaged-file-resource.patch";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn the_packaged_file_resource_patch_reads_only_resources_without_a_dependency_path() {
    let text = read(PATCH);
    let touched: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("+++ b/")).collect();
    assert_eq!(touched, ["platform/src/script/res.rs"], "one file");
    // The rule: an unpackaged build is unchanged; a packaged one reads the
    // absolute path only for a resource WITHOUT a dependency path.
    assert!(
        text.contains("+                if !is_packaged || res.dependency_path.is_none() {\n"),
        "the direct read is gated on (unpackaged || no dependency path)"
    );
    // The old `else if` (direct read only when unpackaged) is what it replaces.
    assert!(text.contains("-                } else if let Some(result) = load_file_direct(&res.abs_path) {\n"));
    // It carries the makepad unit test that reproduces the failure.
    assert!(text.contains("+    fn a_packaged_build_still_reads_file_resource_from_its_absolute_path() {\n"));
    assert!(text.contains("cx.package_root = Some(\"makepad\".to_string());"));
    assert!(text.contains("\"a crate resource comes from the package\""));
}

#[test]
fn the_workspace_builds_makepad_from_the_patched_fork() {
    let cargo = read("Cargo.toml");
    let section = cargo
        .split("[patch.\"https://github.com/OctoSense-org/makepad.git\"]")
        .nth(1)
        .expect("the root Cargo.toml patches the makepad git source");
    for (krate, path) in [
        ("makepad-widgets", ".forks/makepad-fork/widgets"),
        ("makepad-app-module", ".forks/makepad-fork/libs/app_module"),
        ("makepad-script", ".forks/makepad-fork/platform/script"),
    ] {
        assert!(
            section.contains(&format!("{krate} = {{ path = \"{path}\" }}")),
            "{krate} resolves to the fork ({path})"
        );
    }
    let script = read("tools/prepare-makepad-fork.sh");
    assert!(script.contains("PIN=\"6cf03859630761f5cb99ce7fcdfd8c30475d9ab8\""), "the pin OctoSense uses");
    assert!(script.contains("scripts/apply-makepad-patches.sh"), "the same apply script every tree uses");
    // The fork exists wherever this test compiles (the [patch] needs it):
    // every patch is applied in it (reverse-applies cleanly).
    let fork = repo().join(".forks/makepad-fork");
    if !fork.join("platform/src").is_dir() {
        eprintln!("skipped: no .forks/makepad-fork (run tools/prepare-makepad-fork.sh)");
        return;
    }
    let mut patches: Vec<PathBuf> = std::fs::read_dir(repo().join("patches/makepad"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "patch"))
        .collect();
    patches.sort();
    assert!(patches.len() >= 4, "{patches:?}");
    for p in &patches {
        let status = Command::new("patch")
            .args(["-R", "-p1", "-f", "-s", "--dry-run", "-d"])
            .arg(&fork)
            .stdin(std::fs::File::open(p).unwrap())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("patch(1)");
        assert!(status.success(), "{} is not applied in .forks/makepad-fork", p.display());
    }
    // The notification API the suite now compiles (cfg(makepad_notifications)).
    assert!(fork.join("platform/src/notification.rs").is_file());
}
