# D10c. The makepad remote bridge refuses browser requests; the daily APK has no bridge: DECIDED (integrator, 2026-10-02)

Found by the judge while preparing the APK rebuild.

## What was exposed

The makepad remote bridge (`--remote` / `MAKEPAD_REMOTE`, `platform/src/remote.rs`) is how every walk and judge
check drives the app. It:
- injects real mouse and keyboard input (`/click`, `/t`, `/k`);
- serves screen grabs (`/g`);
- returns every widget's text and every TextInput's raw buffer (`/snap`, the `val` field, masked fields included).

At the pinned rev it checked nothing and answered **every** request with `Access-Control-Allow-Origin: *`. Two
consequences:

- **Any app with the bridge on, desktop or phone.** A web page open in any browser on the same machine could read
  `/snap` (the wildcard CORS header lets a cross-origin `fetch` read the reply) and fire `/click` blindly (an `<img>`
  GET needs no CORS at all). The operator's Dev.app runs with the bridge on 127.0.0.1:8400
  (`outer/scripts/open-dev-app.sh`).
- **The APK.** Upstream compiles the bridge out on Android. The APK tree carries a local android-remote edit that
  enables it there, and on Android `MAKEPAD_REMOTE` is read at **build time** (`option_env!`). `apk-sync.sh --build`
  set `MAKEPAD_REMOTE=8765`, so every APK built by it opened the bridge on 127.0.0.1:8765. On Android, any other app
  on the device (and any web page in the phone's browser) could then drive OctosCode.

No evidence of misuse; the exposure is the design gap itself.

## Decision

1. **Browser guard in the bridge**, as one tracked patch, `patches/makepad/remote-browser-guard.patch`, applied to
   every tree by `scripts/apply-makepad-patches.sh` (host, octosense-fork and apk-build `.sources`, and apk-build
   `.mk`). The patch:
   - refuses (403 `{"err":"browser"}`) any request carrying `Origin`, `Referer` or `Sec-Fetch-*`. Every browser
     request carries at least one; curl, urllib and the harness send none.
   - refuses a `Host` that names a domain other than `localhost` (DNS rebinding). IP literals stay allowed for
     fleet boxes driven from another machine.
   - stops sending `Access-Control-Allow-Origin: *`.
2. **The daily APK has no bridge.** `outer/scripts/apk-sync.sh <ref> --build` no longer sets `MAKEPAD_REMOTE`. A
   device-test APK must be asked for explicitly (`--build-test-bridge`, printed as a warning), and is never the
   operator's daily install.

The desktop Dev.app keeps the bridge (the judge checks it is up and connected). With the guard, no web page can reach
it. Local processes of the same user are trusted on desktop in any case.

## Proof

- Host build d07ca8cc + the patch, app on a scratch port:
  - plain curl / urllib / `Host: localhost` -> 200;
  - `Origin` -> 403;
  - `Sec-Fetch-Site: cross-site` on `/click` -> 403;
  - `Referer` -> 403;
  - `Host: rebound.site.example` -> 403;
  - no `Access-Control-Allow-Origin` on any reply.
- `tests/makepad_bridge_guard.rs`:
  - the patch touches only `platform/src/remote.rs`, guards before routing, and removes the wildcard header;
  - with `MAKEPAD_PATCH_TREES` set, the patch is applied in all four trees.
- The patch also carries makepad unit tests (`browser_guard_tests`) for the header rules.

## Upstream

The guard is small and general, so it can go upstream with D10b's notification PR. That PR is for the operator to
open; no agent opens it.
