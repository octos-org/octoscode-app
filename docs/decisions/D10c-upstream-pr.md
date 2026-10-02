# Upstream PR draft (for the operator to submit — **not opened by an agent**)

**Repo:** `OctoSense-org/makepad` · **Base:** `6cf03859630761f5cb99ce7fcdfd8c30475d9ab8`
**Branch to propose:** `fix/remote-bridge-auth`
**Patch:** [`patches/makepad/remote-browser-guard.patch`](../../patches/makepad/remote-browser-guard.patch)
(one file, `platform/src/remote.rs`). Decision record: [`D10c-makepad-remote-browser-guard.md`](D10c-makepad-remote-browser-guard.md).

## Title

remote: require a per-launch token, refuse browser requests, drop the wildcard CORS header

## Summary

The remote bridge (`--remote` / `MAKEPAD_REMOTE`) injects real input, serves screen grabs and returns every widget's
text, including a TextInput's raw buffer. Today it checks nothing and answers every request with
`Access-Control-Allow-Origin: *`. So:
- any web page in any browser on the machine can read `/snap` and fire `/click` on an app started with `--remote`;
- on a phone build with the bridge baked in, any other app on the device can do the same.

This PR:
- **Requires a per-launch token on every request** (`X-Makepad-Token`, constant-time compare).
  - The token is `MAKEPAD_REMOTE_TOKEN` when set (at run time, or at build time on phones, like the port), else
    `mprt_` + 32 random bytes as hex.
  - It is written to `<tmp>/makepad-remote/port-<PORT>.token` (dir 0700, file 0600) for local tooling, and never
    printed; the listening line names only the file.
- **Refuses browser requests** (403): any `Origin`, `Referer` or `Sec-Fetch-*` header, and a `Host` naming a
  domain other than localhost (DNS rebinding). IP literals stay allowed for driving a fleet box over the network.
- **Removes `Access-Control-Allow-Origin: *`.**

## Migration for tools

Send the token from the port's file. curl: `curl -H @<(printf 'X-Makepad-Token: %s\n' "$(cat "$TMPDIR/makepad-remote/port-$PORT.token")") …`.
Python: add the header to each request, or install an opener once.

## Tests

`browser_guard_tests` in `remote.rs`:
- scripted clients pass and browser-marked requests are refused;
- only the launch token opens the bridge (missing, wrong, truncated, or in the query string: refused).
