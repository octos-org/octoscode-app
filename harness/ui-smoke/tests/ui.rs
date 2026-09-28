//! Scripted hidden-window UI smoke tests for the shared native harness.
//!
//! These drive App Hub's `card-host` through `makepad_test` (see
//! `docs/harness/GUIDE.md`). Each test builds `card-host` once through Cargo's
//! owning workspace, launches its OWN hidden instance with `--remote` on an
//! ephemeral port, drives it, then closes it with `/gq` and waits for exit.
//!
//! Run (exactly as the guide says):
//!   cargo test --release --test ui                       # serial
//!   MAKEPAD_TEST_PARALLEL=1 cargo test --release --test ui   # two at once
//!
//! Env:
//!   OCTOSENSE_NATIVE_ROOT  the prepared native workspace (default:
//!                          /Users/yuechen/home/oa.noindex/native)

use makepad_test::{run_with_config, Selector, TestApp, TestConfig};
use std::path::PathBuf;

/// `<repo>/harness/ui-smoke/tests/ui.rs` → `<repo>`.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("harness/ui-smoke must sit two levels under the repo root")
        .to_path_buf()
}

/// The prepared native workspace holding `OctoSense-App-Hub` + pinned siblings.
fn native_root() -> PathBuf {
    std::env::var_os("OCTOSENSE_NATIVE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/Users/yuechen/home/oa.noindex/native"))
}

/// A config that builds and launches `card-host` on the committed notes fixture.
///
/// `test` names the test: it is also the per-test app-data directory, so two
/// concurrent tests never share a storage jail (required under
/// `MAKEPAD_TEST_PARALLEL=1`).
fn notes_app(test: &str) -> TestConfig {
    let root = repo_root();
    let bundle = root.join("harness/ui-smoke/fixtures/notes/bundle");
    let card_host = native_root().join("OctoSense-App-Hub/crates/card-host");
    let app_data = root.join("tmp/ui-smoke").join(format!("{test}-state"));

    let mut config = TestConfig::new(card_host, "octosense-card-host", test)
        .expect("TestConfig::new");
    // The package `octosense-card-host` builds one bin named `card-host`.
    config.bin_name = Some("card-host".into());
    // The fixture is pre-stamped, so no `--stamp` (keeps the bundle immutable
    // and lets two tests share the committed fixture concurrently).
    config.app_args = vec![
        "--bundle".into(),
        bundle.to_string_lossy().into_owned(),
        "--app-data".into(),
        app_data.to_string_lossy().into_owned(),
        "--allow-unsigned".into(),
    ];
    config
}

/// Type a note, click `Add`, and prove the list changed.
#[test]
fn add_note() {
    run_with_config(notes_app("add_note"), |app: TestApp| {
        app.locator(Selector::id("entry"))
            .wait_visible()
            .fill("hello harness");
        app.locator(Selector::id("entry")).wait_value("hello harness");
        app.locator(Selector::widget_type("Button").text_exact("Add"))
            .wait_visible()
            .click();
        // The entry clears and the note appears in the rendered list.
        app.locator(Selector::id("entry")).wait_value("");
        app.locator(Selector::widget_type("Label").text_exact("hello harness"))
            .wait_visible();
    })
    .expect("add_note");
}

/// A second, independent hidden instance — the pair that
/// `MAKEPAD_TEST_PARALLEL=1` runs concurrently (one owned app each).
#[test]
fn add_second_note() {
    run_with_config(notes_app("add_second_note"), |app: TestApp| {
        app.locator(Selector::id("entry"))
            .wait_visible()
            .fill("second instance");
        app.locator(Selector::widget_type("Button").text_exact("Add"))
            .wait_visible()
            .click();
        app.locator(Selector::widget_type("Label").text_exact("second instance"))
            .wait_visible();
    })
    .expect("add_second_note");
}
