# D10c. The makepad remote bridge needs a per-launch token and refuses browser requests; the daily APK has no bridge: DECIDED (integrator + supervisor, 2026-10-02)

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

0. **A per-launch token on every request** (the supervisor's follow-up: header checks stop browsers, not local
   programs or other phone apps).
   - At start the bridge takes `MAKEPAD_REMOTE_TOKEN` when given (at run time, or baked in at build time on phones,
     like the port). Otherwise it generates `mprt_` + 32 bytes from /dev/urandom, as hex.
   - It writes the token to `<tmp>/makepad-remote/port-<PORT>.token` (directory 0700, file 0600) and refuses every
     request without `X-Makepad-Token: <token>` (403 `{"err":"token"}`). The comparison is constant-time.
   - The token is never printed or logged; the listening line names only the file.
   - Clients:
     - `tools/walk/bridgeauth.py`: importing it installs a urllib opener that sends the token for a loopback port
       with a token file. Every walk, judge and outer script that drives the bridge imports it.
     - `harness/bcurl`: curl with the token as a header read from a file descriptor, never on a command line.
       `harness/headless.sh` and the outer scripts use it.
   - The `mprt_` prefix lets `repo_hermetic` fail any tracked file that carries a token.
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

The desktop Dev.app keeps the bridge (the judge checks it is up and connected). With the token and the guard, no web
page and no other user's process can drive it. A process of the same user that reads the 0600 token file can, as it
can read any of that user's files. On the phone, only a device-test APK has a bridge at all.

## Proof

Token: host build be2ed9e8 + the patch, app on a scratch port.
- The token file is `-rw-------` and its directory `drwx------`.
- No token -> 403 `{"err":"token"}`; a wrong token -> 403.
- `harness/bcurl` (with and without a scheme), urllib with `bridgeauth`, and `snap_has.py` -> 200; urllib without it
  -> 403.
- The token plus `Origin` or `Sec-Fetch-Site` -> 403.
- The token is in the app log 0 times and on no other process's command line.
- `headless.sh` start/stop, run.py walks (a26_palettes 52/52, a3_chrome 47/47 + 40/40, a9_prefs 28/28 at desktop
  and phone) and the judge tour (live, 12 captures, 0 flags) all pass through the token.

Browser guard:

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

`D10c-upstream-pr.md` is the PR draft for the operator to submit. No agent opens it.
