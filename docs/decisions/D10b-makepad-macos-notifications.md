# D10b. A makepad notification API for macOS and Android: DECIDED (operator, 2026-10-02)

The operator decided: desktop and phone notifications, on **macOS and Android**. The native app
already had the Settings row and the opt-in (A3/A7, parity row 321), but nothing could appear:

- At the pinned makepad rev, `CxOsOp::ShowNotification { title, body }` is handled **only on
  Android** (`platform/src/os/linux/android/android.rs:2824`), and only as fire-and-forget: no
  identity, no withdrawal, no permission, no click. Every other backend drops it
  (`cx_api.rs:729-737` "No-op on platforms whose backend does not handle it").
- The web's behaviour (parity rows 276, 322, 323) needs all four: a permission request with
  pending / granted / denied / failed answers and an "unavailable" state
  (`apps/web/src/features/attention/desktop-notifications.ts:33-85`), one notice at a time that is
  closed when stale (`:86-125`), focus only on an explicit click (`:105-108`), and the notice
  withdrawn when the window gains focus (`use-attention.ts:49-55`).

So we add a small notification API to makepad's platform layer and carry it as **one patch**
until upstream takes it.

## The pinned tree

- makepad **`6cf03859630761f5cb99ce7fcdfd8c30475d9ab8`** (`OctoSense-org/makepad`): the rev
  OctoSense's `Cargo.toml` pins for every `makepad-*` crate and `runtime-patches.lock.json`
  names as `base_revision`; it is the HEAD of each tree's `.sources/makepad` checkout.
- OctoSense builds makepad from `$OCTOSENSE_WORKSPACE/makepad` (`[patch]` paths into
  `.sources/makepad`; `.cargo/config.toml` sets `OCTOSENSE_WORKSPACE = ".sources"`).

## The patch

`patches/makepad/macos-notifications.patch` — a unified diff against that tree, 10 files:

| file | what |
|---|---|
| `platform/src/notification.rs` (new) | the API: `Cx::post_notification(id, title, body)`, `close_notification(id)`, `query_notification_authorization()`, `request_notification_authorization()`, `focused_window()`; the answers `NotificationAuthorizationResult { status, requested, error }`, `NotificationClicked { id }`, `NotificationFailed { id, error }` as actions (`Cx::post_action`); every target without a backend answers `Unavailable` |
| `platform/src/os/apple/macos/macos_notifications.rs` (new) | `UNUserNotificationCenter`: settings / requestAuthorization / add / remove, the delegate (a click activates the app, orders its main window front — not while `MAKEPAD_HIDE_WINDOWS` — and posts `NotificationClicked`; banners also while the app is frontmost); **`Unavailable` unless the main bundle is an `.app` with an identifier** |
| `platform/src/cx.rs`, `platform/src/os/cx_shared.rs` | `Cx::focused_window`: the last `WindowGotFocus` not followed by its `WindowLostFocus`, recorded in `call_event_handler` (code created after the focus event — a module opened mid-run — still knows) |
| `platform/src/lib.rs`, `platform/src/os/apple/macos/mod.rs` | module declarations |
| `platform/src/os/linux/android/android_jni.rs` | `to_java_post_notification` / `_close_notification` / `_notification_authorization`; JNI `onNotificationClicked` / `onNotificationAuthorization` / `onNotificationFailed` -> `Cx::post_action` |
| `tools/cargo_makepad/.../MakepadActivity.java`, `MakepadNative.java` | `postNotification` (tag = id, a PendingIntent per id carrying it), `closeNotification`, `notificationAuthorization(request)` (`POST_NOTIFICATIONS` from API 33, `areNotificationsEnabled`), the click in `onNewIntent` (the activity is `singleTask`) |
| `tools/cargo_makepad/src/android/mod.rs` | `<uses-permission android:name="android.permission.POST_NOTIFICATIONS"/>` (targetSdk is 35; the phone runs Android 15) |

It deliberately **stays off** `cx_api.rs`, `remote.rs`, `app_main.rs` and `android/android.rs`:
those four differ in the APK tree (it also carries `makepad-android-remote.patch`), so a patch
touching them could not apply to every tree. The new `Cx` methods live in `notification.rs` and
call the backends directly; the Android answers go through `Cx::post_action` from JNI (the
platform's own contract for dialog answers), never through `android.rs`.

Why "Unavailable" outside a bundle: measured with a probe (A25) — in a bare binary whose
embedded Info.plist carries a bundle id (exactly what `platform/build.rs` produces for a
`cargo run` build), `UNUserNotificationCenter.currentNotificationCenter` raises an uncaught
NSException and the process aborts; inside an `.app` (even exec'd directly, not through
LaunchServices) it answers. The backend therefore reports `Unavailable` honestly and never
fakes a success.

## Where it applies (one command, every tree)

```
scripts/apply-makepad-patches.sh [--check] <root>...
```

Each root holds `makepad/` (an OctoSense `.sources`, or the APK tree's `.mk`). Idempotent; a tree
that is not the pinned rev stops it before anything is written. The trees that build the app:

- the host copy `host-<name>/.sources` (the desktop app, `outer/scripts/hostbuild.sh`);
- `octosense-fork/.sources` (the shared fork);
- `apk-build/.sources` (the APK's Rust crates);
- `apk-build/.mk` — the APK's **cargo-makepad**: it compiles the Java from its own source dir at
  APK build time and embeds the manifest template in its binary, so after patching `.mk`
  **rebuild cargo-makepad** (the `.mk/target/release/cargo-makepad` the APK script runs).

`crates/octoscode-module/tests/a25_makepad_patch.rs` checks the result: with
`MAKEPAD_PATCH_TREES=<root>:<root>:...` every file the patch touches must be byte-identical across
the trees and the patch must reverse-apply in each (it skips with a message when unset, as in CI).

## The app side

`crates/octoscode-module/src/attention.rs` ports the web's attention feature over a `NotifyOs`
seam. Its production adapter calls this API when the build has it: `build.rs` sets
`cfg(makepad_notifications)` when `$OCTOSENSE_WORKSPACE/makepad/platform/src/notification.rs`
exists (the host, APK and fork builds once patched). This repository's own workspace compiles
the pinned git rev without the patch; there the adapter answers `Unavailable` — so merging the
app before the patch reaches a tree never breaks a build, the toggle just reads "Unavailable".

## Back to clean upstream

- Undo in a tree: `patch -R -p1 -d <root>/makepad < patches/makepad/macos-notifications.patch`
  (or `git -C <root>/makepad checkout -- . && git -C <root>/makepad clean -fd platform/src`).
- When upstream merges the API (PR text: `docs/decisions/D10b-upstream-pr.md`): bump the makepad
  rev in OctoSense, drop the patch and `scripts/apply-makepad-patches.sh`'s call for it, delete
  the `cfg(makepad_notifications)` probe in `crates/octoscode-module/build.rs` and the
  `cfg(not(makepad_notifications))` adapter in `src/attention.rs`. Nothing else changes: the app
  sits on the API, not on the patch.

## Verified (A25, 2026-10-02)

- `cargo check -p makepad-platform` for aarch64-apple-darwin, aarch64-linux-android,
  aarch64-apple-ios and wasm32-unknown-unknown (the patched host tree); `javac -source 1.8`
  against `android-33` `android.jar` and `javap -s` (the JNI descriptors match the Rust calls).
- The patch dry-runs cleanly on octosense-fork, apk-build `.sources`, apk-build `.mk` and
  host-octoscode-app; applied to copies of the first three, every touched file is identical to
  the patched host copy (`a25_makepad_patch.rs` with `MAKEPAD_PATCH_TREES`).
- Real macOS, own bundle `dev.octoscode.desktop.a25`: `NotDetermined` read through the API; the
  Settings toggle's request made macOS show its permission prompt
  (`docs/ux/a25/live-macos/permission-prompt.png`). Not yet observed: a delivered banner and a
  real click — they need the operator to allow "OctosCode A25"; `tools/walk/a25_live_macos.py`
  then runs the whole round trip.
- Android: compiled only (no devices in this lane); the device check is below.

## Android: what to check on the device (the integrator, at APK time)

Build after patching `apk-build/.sources` AND `apk-build/.mk`, with cargo-makepad rebuilt from
`.mk` (the Java and the manifest come from it). The same app code drives it: the Settings row
calls `request_notification_authorization`, a due notice `post_notification`, focus
`close_notification`, and the click arrives as `NotificationClicked`.

1. Manifest: `aapt dump permissions <apk>` (or `dumpsys package <pkg>`) lists
   `android.permission.POST_NOTIFICATIONS`.
2. Fresh install, Settings > General > Desktop notifications: the row shows the toggle OFF and the
   default help (no system dialog yet: nothing asks before the toggle).
3. Tap the toggle: the row reads "Enabling…" and Android 15 shows its "Allow OctosCode to send you
   notifications?" dialog. Allow → the toggle turns ON, "Desktop notifications are on." Deny →
   OFF with the red "Permission was not granted…" (first refusal: Android may ask again) or
   "Notifications are blocked. Allow them in Settings › Apps › OctosCode › Notifications."
   (refused for good).
4. With it ON, send a prompt and pull the notification shade down before the turn ends (the
   window loses focus, the app stays in the foreground): one notification appears — title = the
   Session's label, body "A background response finished. Return to OctosCode to review it."
   (a turn that waits for an approval: "…needs your input…"). With the app in front and focused,
   a finishing turn posts nothing. Leaving with Home also works while the process runs, but
   Android 14+ freezes a cached background app after a few seconds, so a long turn may only
   notify when the app runs again — test with the shade first.
5. Open another Session, then tap the notification: OctosCode comes to the front ON the notice's
   Session (log: `attention: notice click -> open <session>`), and the notification is gone.
6. Post one, then return to the app by the launcher instead: the notification is withdrawn
   (focus acknowledges) — log `attention: window focused — acknowledged`.
7. Settings › Apps › OctosCode › Notifications OFF while the toggle is ON, then let a turn finish
   in the background: nothing posts and the row turns OFF with "Notification permission changed."
8. `adb logcat | grep -E "attention|Makepad"`: no `postNotification failed` lines.
