# Spike #7 — a pure-OctoScript OUP client (no Rust), over `net.web_socket`

Task #7. Decides whether `octoscode-app` can be a **pure OctoScript bundle** that speaks the Octos UI
Protocol (JSON-RPC 2.0 over WebSocket) itself, with no Rust client, and whether Splash stays usable at
coding-app scale.

**Verdict: BLOCKED on the client, at the very first frame.** A contained Splash isolate can *open* a
`net.web_socket`, *receive* frames and run every callback — but it has **no way to send a frame**. The
whole JSON-RPC client (id counter, pending map, request builder, notification dispatch, reconnect) is
implementable and is implemented here; the one missing primitive is a send. Until the runtime grows one,
a pure-Script OUP client cannot even call `profile/local/create`. Scale, by contrast, is fine.

**Verified** = I ran it and the line is in `evidence/`. **Inferred** = reasoned from source, not run.

## Environment

| Item | Value |
| --- | --- |
| Base | `task/7` off `origin/main` @ `6851f2a` |
| App Hub | `native/OctoSense-App-Hub` @ `64bd6c01` → `tmp/app-hub`; card-host release-built there |
| makepad (runtime) | `native/makepad` @ `d0a9def` (workspace-patched); OctoSense's `.sources/makepad` @ `6cf03859` |
| octos serve | `a6ea8505`, `--solo --port 50082 --host 127.0.0.1 --auth-token spike-dummy-token`; fresh `tmp/serve7-*` |
| ports | 8390 (card-host), 50082 (serve), 8766 (Python WS push fixture) |
| host | `Darwin Mac 25.5.0 … arm64`, `rustc 1.95.0`, `cargo 1.95.0` |
| model turns | none (RULES) |

## The crux: a contained app can receive, not send

Probe bundle: open a socket to a fixture WS server, log the callbacks, try to write.
Verbatim (`evidence/send-probe/PROOF.txt`, `cardhost-send-attempt.log`):

```
SERVER: listening on 8765
SERVER: client connected                                  <- the WS OPENS
PROBE: returned 0000000000000002                          <- net.web_socket returns an id
PROBE: on_opened                                          <- the callback FIRES
[E] method write_string not found on id                   <- the id has NO send method
[...:cardhost-recv-socketstream.log]
[E] this app may not open a raw socket                    <- socket_stream refused for a policed isolate
PROBE: RECV: {"jsonrpc":"2.0","method":"message/delta",…} <- server->client frames DO arrive
```

So: `net.web_socket(request, net.WebSocketEvents{on_opened on_string on_binary on_closed on_error})`
returns an escaped `LiveId` (`net.rs:1116-1160`). That value is registered with **no handle methods** —
`net.rs` adds methods only to `socket_stream_type` (`net.rs:928-1114`), never to a web-socket type. The
only script-facing send path, `socket_stream.write/write_string`, is gated by `sockets_allowed()` which
is `!is_enforced(heap_key)` (`splash_policy.rs:120-124`) — false for any policed isolate, i.e. every
contained app.

Cross-checked statically on two independent runtime revisions: neither `d0a9def` (this workspace) nor
`6cf03859` (OctoSense's `.sources/makepad`) registers any send on a client web socket
(`grep -nE "id!\(send\)|id_lut!\(send\)|ws_send|send_string|send_text|web_socket_send"`
→ nothing in `platform/script/std/src/net.rs`). The backend *can* send (`NetworkBackend::ws_send`,
`backend.rs:89`) and the browser target calls it (`platform/src/os/web/web_socket.rs:89-95`), but **no
script-facing caller exists**. `WsSend` is reached only by a Rust test and the studio socket.

**Consequence:** the frame builder is correct and complete, but `request()` has nowhere to put the bytes:

```
BLOCKED-SEND: {"jsonrpc":"2.0","id":"1","method":"profile/local/create","params":{…}}
BLOCKED-SEND: {"jsonrpc":"2.0","id":"2","method":"session/open","params":{…}}
BLOCKED-SEND: {"jsonrpc":"2.0","id":"3","method":"session/list","params":{}}
```

The serve run confirms it end-to-end: `grep -c "appui … session open session=oup"` on the 50082 log →
**0**. The app's `session/open` never reached the server, because the app could not send
(`evidence/serve-side.txt`).

## What *is* implementable in pure Splash (and is)

`bundle/main.splash` (209 lines) carries the full client shape, minus the send:

- **JSON-RPC id counter + pending map**: `rpc_id`, `pending{id → callback}` (`request()`).
- **Frame builder**: `{jsonrpc:"2.0" id method params}.to_json()` — byte-identical to what the web client
  emits for the same call.
- **Notification dispatch by method**: `notified{method → count}` and a live counter (below).
- **Reconnect with backoff**: `on_closed → start_timeout(backoff)` doubling the delay.

The one Splash gotcha worth recording: **dot-access to a missing key is a hard error**
(`property id not found in prototype chain`, `object_heap.rs:935`), so a JSON-RPC frame that has `method`
but no `id` blows up on `msg.id`. Bracket indexing is nil-safe (`o["k"] → nil`, probed):
`if msg["method"] != nil { … } else if msg["id"] != nil { … }`.

## Push works (receive path) — live counter

The bundle counts notifications **by method** and logs each. Verbatim from the run
(`evidence/final/cardhost.log`):

```
OUP: pushed message/delta x1
OUP: pushed message/delta x5
OUP: pushed message/delta x25
```

245+ frames received over the run. So a *pushed* frame from a server reaches a contained app's script
and its dispatch code runs. (Trigger: a local WS fixture pushing `message/delta` at 2.5 Hz. The octos
server itself pushes nothing unprompted without a turn — `tmp/probe_pushes.py`: `SERVER PUSH: (none
within 4s)` — so the fixture stands in for a real streaming turn, which RULES forbid.)

## Scale test (Splash perf) — measured

Click `stream`: the bundle appends synthetic `message/delta` rows through the **same `dispatch()` code**,
25 rows per 20 ms tick (50 Hz), to 2,000 rows, rendered in a `ScrollYView`. Timestamps are `sys.simsecs`.

| Metric | Measured (verbatim) |
| --- | --- |
| 2,000 rows | `DONE rows=2000 frames=80 first100=65.26612500000084ms total=1585.6327079999985ms` |
| ms per 100 rows (first) | 65.3 ms |
| ms per 100 rows (to 2,000) | 79.3 ms avg (1585.6 / 20) |
| `/snap` before | 17.6 / 18.0 / 17.9 ms |
| `/snap` after 2,000 rows + diff | 18.2 / 17.7 / 18.9 ms |
| click latency (idle) | 18.9 / 18.8 / 18.2 ms |
| click latency (after stream) | 18.6 / 17.8 ms — the app stays responsive |
| RSS before stream | 210.0 MB |
| RSS after 2,000 rows | 213.7 MB |
| RSS after monolithic-diff attempt | 333.6 MB |
| RSS after chunked-diff attempt | 439.9 MB |

**Big diffs are the real ceiling.** A single 5,000-line diff built as one string in one run hits an
**uncatchable hard bail**:

```
[E] script time budget exceeded        (vm.rs:1046)
```

The size/latency curve (one run, `evidence/diff-ceiling.txt`):

| lines | bytes | ms in one run |
| --- | --- | --- |
| 200 | 2,090 | 0.61 |
| 500 | 5,390 | 2.86 |
| 1000 | 10,890 | 10.97 |
| 2000 | 22,890 | 42.44 |

so ~2,000 lines is ~42 ms of pure concatenation; 5,000 in one go blows the run budget. A **chunked**
variant (500 lines/tick) instead hit a second, different wall on the next pass:

```
[E] script heap allocation limit exceeded while concatenating strings:
    requested 17606 bytes, 9681 remaining   (vm.rs:990)
```

The isolate heap still holds the previous eval's strings, so repeated big concatenations exhaust it.

**Third finding — the pump resets state.** A body containing `sys.simsecs` is **re-evaluated once per
simulated second** (`splash.rs:641`: `if (…) || sim_due { … eval_styled_body(cx,false) }`). Verbatim:
`[SPLASH] live re-eval: epoch 0 -> 0 sim_due=true (7331 bytes)`, and the script's module state resets —
`rpc_id` restarts at 1 every time (`"id":"1","id":"2","id":"3"` repeats 30+ times). A client that holds a
connection and a pending map in module state would be wiped every second unless it avoids `sys.simsecs`
in the body or persists state elsewhere. The control probe with **no** `sys.simsecs` shows
`live re-eval` count = **0** and runs its setup exactly once.

**AppCard `ChatList` comparison: not run.** The appcard native app was not built in this card (it is the
appcard dev loop, a separate tree), so no native-ChatList numbers are claimed. This is the one perf
question left open; it is **not run**, not inferred.

## Verified vs inferred

| Claim | Status |
| --- | --- |
| A contained app opens a ws and fires `on_opened` | **verified** (probe run) |
| The returned id has no send method | **verified** (`write_string not found on id`) |
| `socket_stream` is refused for a policed isolate | **verified** (`may not open a raw socket`) |
| Server→client frames arrive and dispatch runs | **verified** (245+ pushes) |
| The app's `session/open` never reached the serve | **verified** (0 serve-log lines) |
| No script-facing ws-send in two runtime revs | **verified** (static grep, both revs) |
| The pump re-evals a `sys.simsecs` body 1/sec and resets state | **verified** (96 re-evals; control = 0) |
| 2,000 rows / big-diff numbers | **verified** (instrument) |
| Native ChatList scale comparison | **not run** (separate app tree) |
| "Reusing `octos.session.*` capabilities would server-side proxy the protocol" | **inferred** (read `app-policy/src/services.rs`, not exercised) |

## Verdict

**Can a pure-OctoScript client carry octoscode-app?** Not today. The receive half is proven; the
transmit half is impossible on this runtime revision. A coding app cannot open a session, send a prompt,
or answer an approval without sending. **D9's "native AppModule" recommendation stands**, and this spike
does not overturn it.

**Upstream changes a pure-Script client would need (in order):**

1. **A `net.web_socket` send method** — the blocker. Give the value `net.web_socket` returns a handle type
   with `write_string`/`write` (mirroring `socket_stream_type`), routed to `NetworkBackend::ws_send`
   (`backend.rs:89`). ~30 lines in `platform/script/std/src/net.rs`, the one place the plumbing already
   reaches. Without it nothing else matters.
2. **A loopback grant for the app's own server** — a manifest cannot list `127.0.0.1:port` (bare public
   names only), and only the host pushes its own origin (`card-host/src/host.rs:275`). This spike used a
   dev-only `--grant-host` flag (`patch/app-hub.diff`, 58 lines) to prove the rest; a real build needs a
   supported way for an app to reach its own local service.
3. **A token hand-off** — worked here by writing `<jail>/octos.json` and reading it with `fs.read`
   (the bundle dials `cfg["url"] + "?token=" + cfg["token"]`; the server's `extract_token` accepts the
   query param, `router.rs:1174`). Fine as a spike; would need to be a real, documented mechanism.
4. **State that survives the 1 Hz re-eval** — or a rule that a connection-holding body must not use
   `sys.simsecs`. Module state (a pending map, an id counter) is wiped every simulated second.
5. **A big-diff path** — a 5,000-line diff cannot be assembled by string concatenation in one run (time
   budget) nor incrementally without risking the heap cap. This needs a streaming/paged viewer, not a
   string. This is true for a native client too; it is a Splash cost, not a pure-Script-specific one.

**What the Spike does NOT say:** if a Rust host service proxies the protocol (shape (ii) from #6), all of
the above vanishes — the send happens in Rust, and Splash only sees `host.request` replies. That is why
the D9 recommendation is *native module or host service*, and this spike adds evidence for it, not against.

## Acceptance

- `--grant-host` log line: `card-host: --grant-host pushed isolate allowlist entry 127.0.0.1:8766`.
- WS connect: `SERVER: client connected` + `OUP: connected; OUP session/open…`.
- `session/open` in `/snap`: **BLOCKED** (the finding) — the app's frame is built
  (`BLOCKED-SEND: {…"session/open"…}`) but the serve log shows 0 opens from it.
- Perf table: measured numbers above, no estimates.
- `lsof -iTCP -sTCP:LISTEN -P | grep -E ':839[0-9]|:50082'` → nothing left; serve stopped.
