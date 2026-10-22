//! A33 / decision D10f — the one-command build from a fresh clone, pinned.
//!
//! The end-to-end check is the clean-room run (`docs/ux/a33/clean-room.md`: a
//! fresh `git clone`, `tools/build-macos.sh --package --octosense` from an
//! empty cargo cache, both variants built, the unzipped app run with every
//! build tree unreadable). This suite pins the pieces that run proves, so a
//! later edit cannot silently drop one: the OctoSense checkout (upstream, with
//! the module redirected to this checkout), what build-macos.sh runs, what the
//! bundle is built with, and that every script still parses.
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn every_build_script_parses() {
    for script in [
        "tools/build-macos.sh",
        "tools/check-fresh-clone-macos.sh",
        "tools/package-macos.sh",
        "tools/prepare-makepad-fork.sh",
        "tools/prepare-octosense.sh",
        "tools/prepare-octoscript-makepad.sh",
        "scripts/apply-makepad-patches.sh",
    ] {
        let out = Command::new("bash").arg("-n").arg(repo().join(script)).output().expect("bash");
        assert!(out.status.success(), "{script}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

/// OctoSense main links the module itself (`app-octoscode`, by git): the host
/// is upstream at a pinned revision, unpatched, and only the module is built
/// from this checkout, by a cargo `--config` [patch] on that git source.
#[test]
fn the_octosense_host_is_upstream_with_the_module_redirected_here() {
    let script = read("tools/prepare-octosense.sh");
    let pin = script
        .lines()
        .find_map(|l| l.strip_prefix("PIN=\""))
        .and_then(|r| r.strip_suffix('"'))
        .expect("prepare-octosense.sh pins a revision");
    assert!(pin.len() == 40 && pin.chars().all(|c| c.is_ascii_hexdigit()), "a full commit id: {pin}");
    assert!(script.contains("REMOTE=\"https://github.com/OctoSense-org/OctoSense\""));
    assert!(!script.contains("git apply") && !script.contains("patches/"), "and patches nothing");
    let build = read("tools/build-macos.sh");
    assert!(
        build.contains(
            "MODULE_PATCH=\"patch.\\\"https://github.com/octos-org/octoscode-app.git\\\".octoscode-module.path=\\\"$REPO/crates/octoscode-module\\\"\""
        ),
        "the module's git source is redirected to this checkout's crate"
    );
    for profile in ["cargo build --release -p octosense", "cargo build -p octosense"] {
        assert!(
            build.contains(&format!("{profile} --features app-octoscode --config \"$MODULE_PATCH\"")),
            "{profile} builds with the redirect"
        );
    }
}

#[test]
fn build_macos_runs_every_step_of_both_variants() {
    let s = read("tools/build-macos.sh");
    for needle in [
        "prepare-makepad-fork.sh",
        "prepare-octosense.sh",
        "prepare-octoscript-makepad.sh",
        "cargo build -p octoscode-desktop --bin octoscode",
        "package-macos.sh",
        // --octosense: OctoSense's own setup.py, our makepad patches on its
        // .sources, its desktop kernel staged beside the binary, the host.
        "python3 tools/setup.py",
        "scripts/apply-makepad-patches.sh\" \"$HOST/.sources\"",
        "python3 tools/kernel-artifact.py --host --stage \"$KERNEL_DIR\"",
        "\"$HOST/apps/octoscode/BUILT_FROM\"",
        "cargo build -p octosense --features app-octoscode",
        "xcode-select -p",
    ] {
        assert!(s.contains(needle), "build-macos.sh runs {needle}");
    }
    // It never prepares a fork it does not own (a shared checkout linked into .forks/).
    assert!(s.contains("this script prepares only the forks it owns"));
}

/// The standalone app's renderer is Octoscript-Makepad unpatched, at the rev
/// the root Cargo.toml pins: the checkout script and the pin cannot drift.
#[test]
fn the_renderer_checkout_is_the_pinned_rev() {
    let cargo = read("Cargo.toml");
    let pin = cargo
        .lines()
        .find(|l| l.starts_with("octoscript-makepad = { git = "))
        .and_then(|l| l.split("rev = \"").nth(1))
        .and_then(|r| r.split('"').next())
        .expect("octoscript-makepad is pinned by rev");
    let script = read("tools/prepare-octoscript-makepad.sh");
    assert!(script.contains(&format!("PIN=\"{pin}\"")), "the checkout script takes the pinned rev {pin}");
    assert!(!script.contains("git apply"), "and patches nothing");
    assert!(cargo.contains("octoscript-node = { path = \".forks/octoscript-makepad/crates/octoscript-node\" }"), "one octoscript-node");
}

/// A35b — nobody gets the slow build by default. Measured on the standalone
/// app with the Makepad instrument (tools/walk/a35_browser_latency_walk.py):
/// at opt-level 0 the folder browser's open and navigations took 0.3-0.7 s,
/// optimized well under 0.1 s. The one-command build is release unless
/// `--debug` asks otherwise, every package is the optimized `app-bundle`
/// profile, and even a plain `cargo build` (the dev profile) optimizes the
/// dependencies, where the UI thread's time goes.
#[test]
fn the_default_builds_are_optimized() {
    let s = read("tools/build-macos.sh");
    assert!(s.contains("\nPROFILE=release\n"), "build-macos.sh builds release by default");
    assert!(s.contains("--debug) PROFILE=debug;"), "a debug build is opt-in (--debug)");
    assert!(!s.contains("\nPROFILE=debug\n"), "never the default");
    assert!(s.contains("cargo build --release -p octoscode-desktop --bin octoscode"));
    assert!(read("tools/package-macos.sh").contains("cargo build --profile app-bundle -p octoscode-desktop"));
    assert!(read("tools/package-desktop.py").contains("\"--profile\", \"app-bundle\""));
    let cargo = read("Cargo.toml");
    assert!(
        cargo.contains("[profile.dev.package.\"*\"]\nopt-level = 3\n"),
        "the dev profile optimizes every dependency"
    );
}

/// `--package` builds the standalone app against the forks the root
/// Cargo.toml patches, so that is what it checks for: `tools/build-macos.sh`
/// alone prepares them (no renderer fork).
#[test]
fn package_macos_checks_the_forks_the_workspace_patches() {
    let s = read("tools/package-macos.sh");
    let cargo = read("Cargo.toml");
    for (checked, patched) in [
        ("makepad-fork/widgets/Cargo.toml", "path = \".forks/makepad-fork/widgets\""),
        (
            "octoscript-makepad/crates/octoscript-makepad/Cargo.toml",
            "octoscript-makepad = { path = \".forks/octoscript-makepad/crates/octoscript-makepad\" }",
        ),
    ] {
        assert!(s.contains(checked), "package-macos.sh checks .forks/{checked}");
        assert!(cargo.contains(patched), "the root Cargo.toml patches it: {patched}");
    }
    assert!(!s.contains("octoscript-makepad-fork"), "no renderer fork");
}

#[test]
fn the_bundle_is_self_contained_and_named_octoscode() {
    let s = read("tools/package-macos.sh");
    for needle in [
        "MAKEPAD=apple_bundle MAKEPAD_PACKAGE_DIR=makepad",
        "cargo build --profile app-bundle -p octoscode-desktop",
        "package_resources.py",
        "<key>CFBundleName</key><string>$APP_NAME</string>",
        "codesign --force --sign - --timestamp=none",
        "ditto -c -k --sequesterRsrc --keepParent",
    ] {
        assert!(s.contains(needle), "package-macos.sh: {needle}");
    }
    assert!(s.contains("APP_NAME=\"OctosCode\"") && s.contains("BUNDLE_ID=\"org.octos.octoscode\""));
    let cargo = read("Cargo.toml");
    assert!(cargo.contains("[profile.app-bundle]\ninherits = \"release\""), "the bundle's own profile");
    // `cargo run` launches carry the same name in the menu bar.
    let config = read(".cargo/config.toml");
    assert!(config.contains("MAKEPAD_BUNDLE_NAME = { value = \"OctosCode\", force = true }"));
    assert!(config.contains("MAKEPAD_BUNDLE_IDENTIFIER = { value = \"org.octos.octoscode\", force = true }"));
}
