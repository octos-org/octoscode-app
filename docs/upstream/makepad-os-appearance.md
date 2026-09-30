# Upstream PR draft — OS appearance (dark mode) for the makepad platform

**Target:** OctoSense-org/Makepad (the fork this app pins: `octoscript-makepad-fork`,
rev `6881fb6c3c3220e407633b0ba5211c3d42c7e625`; base checkout
`.shared-cargo-home/git/checkouts/makepad-d00a25647c09e43e/6cf0385`).
**Motivation (this app):** octoscode #31d workflow 3 — the theme preference's `system`
value must follow the OS appearance. The platform today exposes no OS-appearance API
(searched `platform/src/` for OsTheme/dark_mode/appearance/color_scheme: the only hit is
`SystemBarAppearance`, which styles the *system status bar* on mobile, not the app
palette — `platform/src/cx_api.rs:1438`).

## What the smallest hook looks like

One event + one query, mirroring the existing `Event`/`Cx` patterns:

```rust
// platform/src/event/os_theme.rs (new)
/// The OS appearance changed (or was resolved at startup).
#[derive(Clone, Debug)]
pub struct OsThemeEvent { pub dark: bool }

// platform/src/os/apple/apple_events.rs (macOS arm)
// KVO on NSApp.effectiveAppearance, or a one-shot read:
//   let dark = NSApp.effectiveAppearance.bestMatch(
//       from: ["Aqua","DarkAqua"]) == "DarkAqua"
// Windows: HKCU\...\Themes\Personalize AppsUseLightTheme (WM_SETTINGCHANGE).
// Linux (freedesktop): portal SettingChanged "org.freedesktop.appearance" 1.
```

- `Cx::os_theme() -> Option<bool>` for the startup query (None = unknown platform),
  delivered as `Event::OsTheme(OsThemeEvent)` on change, like the existing
  `Event::WindowCloseRequested` family.
- No styling behavior changes in the platform itself — apps map the event to their own
  palette (this app: `screens::theme::set_os_reader` today, the event hook when the PR
  lands).

## Why a KVO/event shape (not a poll)

The web's contract is reactive-in-startup-resolution only (`use-theme.ts` reads
`prefers-color-scheme` at mount and relies on CSS `color-scheme` for live OS flips).
Makepad apps can do better with the event: this app's `system` mode would flip live
instead of at restart — strictly more parity, zero API surface beyond one event.

## Current in-tree stopgap (removed when the PR lands)

`crates/octoscode-module/src/screens/theme.rs` resolves `system` through an injected
reader; the macOS implementation shells out to `defaults read -g AppleInterfaceStyle`
(exit 1 / missing key = light). Verified on-platform this card (light OS → light, dark
preference still wins), but the upstream event is the right home: no subprocess, live
changes, and the other two platform backends in one place.

## Test plan (upstream)

A `makepad-test` that fakes the platform callback (the existing `test_run` harness)
asserting the event fires on appearance change and `Cx::os_theme()` agrees with the last
event. App-side (this repo) the seam is already tested: `f31d_theme.rs ::
f31d_os_appearance_reader_drives_system_resolution`.
