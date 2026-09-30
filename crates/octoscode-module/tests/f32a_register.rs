//! #32a — the module registers with the declared capabilities.
//!
//! The shell consumes the module through the `AppModule` trait object
//! (`linked_modules()` pushes `&OCTOSCODE_MODULE`; the App Library row comes
//! from `id`/`label`, the host grant check from `capabilities`). Exercise
//! that exact seam: the static through the trait object, the declared
//! capabilities non-empty and exactly the AppCard pair.

use makepad_app_module::AppModule;
use octoscode_module::OCTOSCODE_MODULE;

#[test]
fn the_module_registers_with_the_declared_capabilities() {
    // The registry seam: a `&'static dyn AppModule` (what linked_modules()
    // stores).
    let m: &'static dyn AppModule = &OCTOSCODE_MODULE;
    // The App Library entry: registry id + the name a person reads.
    assert_eq!(m.id(), "octoscode");
    assert_eq!(m.label(), "OctosCode");
    // The capability declaration the host honours as grants — the SAME pair
    // AppCard declares (storage + net), never empty, no duplicates.
    let caps = m.capabilities();
    assert_eq!(caps, &["storage", "net"], "declared capabilities");
    let mut uniq = caps.to_vec();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(uniq.len(), caps.len(), "no duplicate capabilities");
}
