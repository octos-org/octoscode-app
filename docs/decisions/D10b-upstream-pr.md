# Upstream PR draft (for the operator to submit — **not opened by an agent**)

**Repo:** `OctoSense-org/makepad` · **Base:** `6cf03859630761f5cb99ce7fcdfd8c30475d9ab8`
**Branch to propose:** `feat/notification-api`
**Patch:** [`patches/makepad/macos-notifications.patch`](../../patches/makepad/macos-notifications.patch)
(10 files; 2 new). Decision record: [`D10b-makepad-macos-notifications.md`](D10b-makepad-macos-notifications.md).

## Title

platform: system notifications with an id — post, close, permission, click (macOS + Android)

## Summary

`Cx::show_notification(title, body)` is fire-and-forget and only Android implements it. An app
that notifies when work finishes in the background (a chat, a build, a download) needs more:

- a **permission** it can read without prompting and request on an explicit user action, with an
  honest "unavailable" when the process cannot post at all;
- an **identity** per notice, so it can be replaced and **withdrawn** (e.g. when the person comes
  back to the window by other means);
- the **click**, delivered to the app with that identity, so it can open what the notice is
  about;
- whether the app **has focus right now**, so a notice is only posted while the person is not
  looking — even from code created after the last focus event.

This PR adds that, keeps `show_notification` unchanged, and implements it on macOS
(UserNotifications) and Android (NotificationManager). Every other target answers
`Unavailable`.

## API

```rust
// platform/src/notification.rs
impl Cx {
    pub fn post_notification(&mut self, id: &str, title: &str, body: &str);
    pub fn close_notification(&mut self, id: &str);
    pub fn query_notification_authorization(&mut self);   // never prompts
    pub fn request_notification_authorization(&mut self); // may prompt, once
    pub fn focused_window(&self) -> Option<WindowId>;
}
pub enum NotificationAuthorization { Unavailable, NotDetermined, Granted, Denied }
// answers, delivered as actions (Cx::post_action) from whatever thread the OS answered on:
pub struct NotificationAuthorizationResult { pub status: NotificationAuthorization, pub requested: bool, pub error: Option<String> }
pub struct NotificationClicked { pub id: String }
pub struct NotificationFailed { pub id: String, pub error: String }
```

Usage:

```rust
// Settings toggle
cx.request_notification_authorization();
// later, when work finishes and cx.focused_window().is_none()
cx.post_notification("build:42", "Build finished", "3 warnings");
// in handle_event
if let Event::Actions(actions) = event {
    for a in actions.iter() {
        if let Some(c) = a.downcast_ref::<NotificationClicked>() { open(&c.id) }
        if let Some(r) = a.downcast_ref::<NotificationAuthorizationResult>() { show(r.status) }
    }
}
```

Semantics:

- Posting never brings the app forward. A post that finds the permission not granted does not
  post; it answers with `NotificationAuthorizationResult { requested: false, .. }`.
- A click brings the app forward (macOS: activate + main window ordered front, deminiaturized;
  Android: the activity resumes through its PendingIntent) and then posts `NotificationClicked`.
- The same id replaces the earlier notice; `close_notification` withdraws a shown or pending one.
- macOS: banners are presented while the app is frontmost too (the app decides when a notice is
  due).

## Design notes

- **Answers are actions, not new `Event` variants.** `Cx::post_action` is already the
  cross-thread contract for asynchronous platform answers (file dialogs, downloads); no
  `Event::to_u32`/name tables change and no backend's event match has to learn a variant.
- **No new `CxOsOp` variants.** The methods call the backends directly (UserNotifications'
  calls are asynchronous and safe on the main thread; on Android they are JNI calls the
  activity hops onto its looper). This also keeps the change off `cx_api.rs` and
  `android/android.rs`.
- **macOS needs a real bundle.** `UNUserNotificationCenter.currentNotificationCenter` raises an
  NSException (abort) in a bare binary whose embedded Info.plist carries a bundle id — which is
  what `platform/build.rs` produces for `cargo run`. The backend checks that the main bundle is
  an `.app` with an identifier and otherwise answers `Unavailable`, never touching the center.
- **Android 13+** needs `POST_NOTIFICATIONS` declared (cargo-makepad's manifest, added) and
  requested at runtime; below 33 the permission is the app's notification switch
  (`areNotificationsEnabled`). A refusal Android will ask again about reads `NotDetermined`, a
  permanent one `Denied`.
- `Cx::focused_window` is recorded in `call_event_handler`, before any handler runs.

## Tests and checks

- `cargo check -p makepad-platform` on aarch64-apple-darwin, aarch64-linux-android,
  aarch64-apple-ios, wasm32-unknown-unknown.
- Java: `javac -source 1.8 -target 1.8` against `android-33` `android.jar`; `javap -s` shows the
  descriptors the JNI calls use (`(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V`,
  `(Ljava/lang/String;)V`, `(Z)V`, `(IZLjava/lang/String;)V`).
- macOS, inside an `.app` (`dev.octoscode.desktop.a25`): the query answers `NotDetermined`; the
  request shows the system prompt; outside a bundle the query answers `Unavailable` without
  touching the center (no abort).
- Downstream (OctosCode): the app's attention model over this API — unit tests with a fake OS,
  a click walk at desktop and phone sizes, and a live round trip script.

Suggested follow-ups (not in this PR): iOS (the same UserNotifications code), Linux
(org.freedesktop.Notifications), Windows (toast), web (the Notification API), and delivering a
click that LAUNCHED the app (cold start) once the app exists.
