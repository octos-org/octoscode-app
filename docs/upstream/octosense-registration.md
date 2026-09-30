# [DRAFT] OctoSense PR: register OctosCode as a system app (Phase 3, §8.19)

Operator-facing draft. Nothing is pushed to OctoSense; the complete patch
travels with this repo as `integration/octosense-register.patch` (2550 lines,
applies clean to `OctoSense-org/OctoSense` at `ca62dfa` — the
`feat/transport-generic-request` rev the octoscode pins).

## 1. What the patch does

- **Root `Cargo.toml`** — vendors the three octoscode crates as workspace
  members + path deps so they resolve against the in-tree
  `octos-app-transport` / `octos-app-store`:
  `apps/octoscode/{client,store,module}`.
- **`crates/shell/Cargo.toml`** — optional `octoscode-module` dependency and
  the feature `app-octoscode = ["dep:octoscode-module", "app-hub",
  "octos-core"]` (the App Hub host, as the other in-process modules do).
- **`crates/shell/src/apps.rs`** — `linked_modules()` pushes
  `&octoscode_module::OCTOSCODE_MODULE` under the feature, next to the
  AppCard push. The App Library row follows from the existing machinery:
  `bundled_modules_catalog()` (`crates/shell/src/apps.rs:307`) turns linked
  modules into launcher rows (`id` + `label`).
- **`desktop/Cargo.toml` / `phone/Cargo.toml`** — feature forwarding
  (`app-octoscode = ["octosense-shell/app-octoscode"]`).
- The vendored crates are the octoscode repo current trees (the module
  implements the `AppModule` trait: `register` installs the design/kit
  vocabulary into the host isolate; `create` mints `OctoscodeView`).

## 2. Capability declaration (the host honours these as grants)

`AppModule::capabilities` returns **`["storage", "net"]`** — the same pair
AppCard declares (`apps/appcard/module/src/lib.rs`): the module holds the
WebSocket to the octos serve (`net`), and its state — sessions, settings,
the trace — lives on disk under the instance storage jail (`storage`).

A test pins the declaration through the trait object (the vtable the shell
exercises): octoscode `tests/f32a_register.rs`.

## 3. The four ingredients of a WORKING Android build (verified by the
outer loop on a OnePlus 6T, installed beside OctoSense, package
`dev.makepad.octosense.octoscode`, label `OctosCode`)

1. **The buildtool built from the pinned fork**:
   `cargo build --release` in `.sources/makepad/tools/cargo_makepad`
   (43 s) — the tool compiles the Java activity of the checkout it was
   BUILT in; the installed `~/.cargo/bin/cargo-makepad` carries a different
   baked Java root and crashes at the first JNI call
   (`phone/docs/build-tool.md`).
2. **The contracts jar**: `gradlew :contracts:exportHomeContracts` before
   the Android build.
3. **Run the build from `phone/`** (the Java source roots the tool passes
   to javac are resolved from there).
4. **The `octoscript-makepad` `[patch]` pointed at the octoscode fork**
   (the renderer the module renders through; the clean `.sources` checkout
   lacks `to_makepad_ui_in_slot`).

## 4. The route that did NOT work (recorded 2026-09-30, redirect timebox)

Command (from the octoscode repo, fork at `.forks/octosense-fork` with the
registration patch applied, toolchain symlinked into the local tool 's
baked dir, JAVA_HOME = Android Studio JBR):

```
target/release/cargo-makepad android --package-name=dev.makepad.octosense.octoscode \
  --app-label=OctosCode build -p octosense-home --features app-octoscode
```

Failed at javac after 18 min of compilation — 707 errors, first:

```
phone/resources/android/java/dev/makepad/octosense/AccountsSettingsClient.java:7:
error: package dev.makepad.octosense.accounts does not exist
    import dev.makepad.octosense.accounts.AccountsSettingsBackend;
```

The build-tool Java-template sync (the `accounts` package lives on the
buildtool branch, not in `phone/resources/android/java`) is build-tool
side, not the app: fixed by ingredients 1-3 above.

## 5. Handoff

The outer loop built and verified the APK (installs beside OctoSense); the
octoscode lane ships the registration patch, the capability declaration and
its test, and this draft. Icon art follows the AppCard launcher convention
as a follow-up.
