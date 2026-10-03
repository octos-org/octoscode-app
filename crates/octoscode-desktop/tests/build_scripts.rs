//! A33 / decision D10f — the one-command build from a fresh clone, pinned.
//!
//! The end-to-end check is the clean-room run (`docs/ux/a33/clean-room.md`: a
//! fresh `git clone`, `tools/build-macos.sh --package --octosense` from an
//! empty cargo cache, both variants built, the unzipped app run with every
//! build tree unreadable). This suite pins the pieces that run proves, so a
//! later edit cannot silently drop one: the fork patch stack (0003 included,
//! its lock hunk additive), what build-macos.sh runs, what the bundle is built
//! with, and that every script still parses.
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
        "tools/prepare-octosense-fork.sh",
        "tools/prepare-octoscript-makepad-fork.sh",
        "scripts/apply-makepad-patches.sh",
    ] {
        let out = Command::new("bash").arg("-n").arg(repo().join(script)).output().expect("bash");
        assert!(out.status.success(), "{script}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

#[test]
fn the_octosense_fork_carries_the_shell_wiring_as_patch_0003() {
    let script = read("tools/prepare-octosense-fork.sh");
    let order: Vec<usize> = ["0001-transport-generic-request.patch", "0002-transport-never-drop-a-reply.patch", "0003-shell-octoscode-module.patch"]
        .iter()
        .map(|p| script.find(&format!("patches/octosense/{p}")).unwrap_or_else(|| panic!("{p} is in the stack")))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "applied in order 0001, 0002, 0003");
    let patch = read("patches/octosense/0003-shell-octoscode-module.patch");
    assert!(patch.starts_with("From "), "a git format-patch (the script commits its Subject and author)");
    assert!(patch.contains("\nSubject: [PATCH] feat(shell): host the octoscode module (app-octoscode)\n"));
    let touched: Vec<&str> = patch.lines().filter_map(|l| l.strip_prefix("+++ b/")).collect();
    assert_eq!(
        touched,
        [
            "Cargo.lock",
            "Cargo.toml",
            "apps/appcard/app/app/src/app/l0_eval.rs",
            "crates/shell/Cargo.toml",
            "crates/shell/src/apps.rs",
            "desktop/Cargo.toml",
            "phone/Cargo.toml",
        ]
    );
    for needle in [
        "+    \"apps/octoscode/module\",",
        "+octoscode-module = { path = \"apps/octoscode/module\" }",
        "+octoscript-makepad = { path = \"../octoscript-makepad-fork/crates/octoscript-makepad\" }",
        "+app-octoscode = [\"dep:octoscode-module\", \"app-hub\", \"octos-core\"]",
        "+    out.push(&octoscode_module::OCTOSCODE_MODULE);",
        "+app-octoscode = [\"octosense-shell/app-octoscode\"]",
        "+        padright: f32_prop(vm, value, id!(padright)),",
    ] {
        assert!(patch.contains(needle), "0003 carries {needle}");
    }
    // The lock hunk only ADDS entries (the three crates, the shell's new
    // dependency): it pins no new version of anything OctoSense locks.
    let lock = patch.split("diff --git a/Cargo.lock b/Cargo.lock").nth(1).unwrap();
    let lock = lock.split("\ndiff --git ").next().unwrap();
    assert!(
        !lock.lines().any(|l| l.starts_with('-') && !l.starts_with("---")),
        "the Cargo.lock hunk removes nothing"
    );
    for krate in ["octoscode-client", "octoscode-module", "octoscode-store"] {
        assert!(lock.contains(&format!("+name = \"{krate}\"")), "{krate} is locked");
    }
}

#[test]
fn build_macos_runs_every_step_of_both_variants() {
    let s = read("tools/build-macos.sh");
    for needle in [
        "prepare-makepad-fork.sh",
        "prepare-octosense-fork.sh",
        "prepare-octoscript-makepad-fork.sh",
        "cargo build -p octoscode-desktop --bin octoscode",
        "package-macos.sh",
        // --octosense: OctoSense's own setup.py, our makepad patches on its
        // .sources, the vendored crates + design (hostbuild.sh's steps), the host.
        "python3 tools/setup.py",
        "scripts/apply-makepad-patches.sh\" \"$HOST/.sources\"",
        "\"$HOST/apps/octoscode/$c/\"",
        "s|path = \"../../.forks/octoscript-makepad-fork/|path = \"../../../../octoscript-makepad-fork/|",
        "rsync -a --delete \"$REPO/design/\" \"$HOST/apps/design/\"",
        "cargo build -p octosense --features app-octoscode",
        "xcode-select -p",
    ] {
        assert!(s.contains(needle), "build-macos.sh runs {needle}");
    }
    // It never prepares a fork it does not own (a shared checkout linked into .forks/).
    assert!(s.contains("this script prepares only the forks it owns"));
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
