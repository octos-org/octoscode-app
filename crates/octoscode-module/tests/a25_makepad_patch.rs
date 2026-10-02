//! A25 / decision D10b — the makepad notification patch is the SAME in every
//! tree that builds the app.
//!
//! `MAKEPAD_PATCH_TREES=<root>:<root>:…` names the trees after
//! `scripts/apply-makepad-patches.sh` ran on them (each root a `.sources`, the
//! APK's `.mk`, or a makepad checkout — the integrator's host copy,
//! octosense-fork and apk-build). Every file
//! `patches/makepad/macos-notifications.patch` touches must then be
//! byte-identical across the trees, and the patch must be applied in each
//! (it reverse-applies cleanly). Without the variable (CI, a fresh clone)
//! the tree comparison skips with a message; the patch's own shape is always
//! checked.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PATCH: &str = "patches/makepad/macos-notifications.patch";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn patch_text() -> String {
    std::fs::read_to_string(repo().join(PATCH)).expect("the tracked patch exists")
}

/// The files the patch touches, in patch order (`+++ b/<path>`).
fn touched(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("+++ b/"))
        .map(|p| p.split_whitespace().next().unwrap_or("").to_owned())
        .collect()
}

fn tree_of(root: &str) -> PathBuf {
    let root = PathBuf::from(root);
    if root.join("makepad/platform/src").is_dir() {
        root.join("makepad")
    } else {
        root
    }
}

/// FNV-1a 64: a fingerprint for the report (the assertion compares bytes).
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3))
}

#[test]
fn the_patch_adds_the_api_and_touches_only_these_makepad_sources() {
    let text = patch_text();
    assert_eq!(
        touched(&text),
        [
            "platform/src/notification.rs",
            "platform/src/os/apple/macos/macos_notifications.rs",
            "platform/src/cx.rs",
            "platform/src/lib.rs",
            "platform/src/os/cx_shared.rs",
            "platform/src/os/apple/macos/mod.rs",
            "platform/src/os/linux/android/android_jni.rs",
            "tools/cargo_makepad/src/android/java/dev/makepad/android/MakepadActivity.java",
            "tools/cargo_makepad/src/android/java/dev/makepad/android/MakepadNative.java",
            "tools/cargo_makepad/src/android/mod.rs",
        ]
    );
    // The two new files are created, not edited.
    assert_eq!(text.matches("--- /dev/null").count(), 2);
    // It stays off the files the APK tree's own android-remote patch edits
    // (cx_api.rs, remote.rs, app_main.rs, android.rs): one patch file applies
    // to every tree.
    for f in ["cx_api.rs", "remote.rs", "app_main.rs", "android/android.rs"] {
        assert!(!touched(&text).iter().any(|t| t.ends_with(f)), "{f}");
    }
    // The app's API, the platform side and the Android bridge are all there.
    for needle in [
        "pub fn post_notification(",
        "pub fn close_notification(",
        "pub fn query_notification_authorization(",
        "pub fn request_notification_authorization(",
        "pub fn focused_window(",
        "pub struct NotificationClicked",
        "UNUserNotificationCenter",
        "ends_with(\".app\")",
        "Java_dev_makepad_android_MakepadNative_onNotificationClicked",
        "public void postNotification(",
        "android.permission.POST_NOTIFICATIONS",
    ] {
        assert!(text.contains(needle), "the patch carries {needle}");
    }
}

#[test]
fn every_tree_carries_the_same_patched_files() {
    let Ok(roots) = std::env::var("MAKEPAD_PATCH_TREES") else {
        eprintln!(
            "skipped: set MAKEPAD_PATCH_TREES=<root>:<root>:... (the host, octosense-fork and apk-build \
             .sources, and apk-build/.mk, after scripts/apply-makepad-patches.sh) to compare the trees"
        );
        return;
    };
    let trees: Vec<PathBuf> = roots.split(':').filter(|s| !s.is_empty()).map(tree_of).collect();
    assert!(trees.len() >= 2, "name at least two trees to compare: {roots}");
    let text = patch_text();
    for t in &trees {
        let status = Command::new("patch")
            .args(["-R", "-p1", "-f", "-s", "--dry-run", "-d"])
            .arg(t)
            .stdin(std::fs::File::open(repo().join(PATCH)).unwrap())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("patch(1)");
        assert!(status.success(), "the patch is not applied in {}", t.display());
    }
    for f in touched(&text) {
        let first = std::fs::read(trees[0].join(&f)).unwrap_or_else(|e| panic!("{f} in {}: {e}", trees[0].display()));
        let mut row = format!("{f}: {:016x}", fnv(&first));
        for t in &trees[1..] {
            let bytes = std::fs::read(t.join(&f)).unwrap_or_else(|e| panic!("{f} in {}: {e}", t.display()));
            assert!(bytes == first, "{f} differs between {} and {}", trees[0].display(), t.display());
            row.push_str(&format!(" {:016x}", fnv(&bytes)));
        }
        eprintln!("{row}");
    }
}
