# D10d. The transport never drops a server reply: DECIDED (supervisor, 2026-10-02)

"A client must never silently drop a server reply." Found by A22 while hardening row 203's 4096-event candidate
test, which flaked under load.

## What happened

`octos-app-transport` (our fork, D10a) handed replies, error replies and connection states to its consumer with
`try_emit`. That is a `try_send` that **dropped** the event, with a warning, when the 64-slot event channel was
full. Notifications already waited for room. OctosCode's consumer applies each event on its own task. It falls
behind whenever the server streams fast: another client's turn, a replay, a flood of live events. So a reply
that arrived right after a burst was dropped:

- the Session being opened never left "Loading conversation…";
- its per-Session history-read queue (A15: one socket answers in order) stayed one read behind for good, so the
  Session's next history reply was judged against the lost read and discarded as stale.

The flood test had worked around it by holding the history reply until the flood was taken in. That workaround is
gone now.

## Decision

1. **The transport waits instead of dropping.** OctoSense fork patch **0002**,
   `patches/octosense/0002-transport-never-drop-a-reply.patch`, fork commit `06ce11e` on top of 0001's `ca62dfa`:
   - every event goes through `emit`, which awaits room in the channel, as notifications already did;
   - `handle_response` and `fail_all_pending` become async;
   - the ws and kernel loops await their state changes.

   A slow consumer now slows the reader on replies too, and TCP flow control reaches the server. Delivery ends only
   when the consumer is gone. No API change.
2. **A remaining loss is LOUD and forces a resync.** This covers a server that never answers, a request that
   could not be sent, or any loss outside the transport. When a Session's history is still not read
   `HISTORY_WAIT` (20 s) after the read was asked:
   - the app says so in its log (`… presumed LOST …`);
   - it resets that Session's history-read queue;
   - once per Session until its history settles, it opens the Session again, which reads its history again.

   A Session whose read is lost again shows A19b's failure ("the server did not answer in time"). Code:
   `crates/octoscode-module/src/flow.rs` `resync_lost_history_reads`, called by the `HISTORY_WAIT` wake.

## Trees

Every tree that builds the transport needs 0002. On 2026-10-02 all of these carried exactly the shared fork's
transport files, so the patch applies as is:

- `~/home/oa.noindex/octosense-fork`: the shared fork, at `ca62dfa` = pin + 0001, with unrelated uncommitted edits;
- every `~/home/oa.noindex/host-*` copy (21 lanes): `apps/appcard/app/crates/octos-app-transport`;
- `~/home/oa.noindex/apk-build`: the same path; not a git checkout.

Commands (each was run on a scratch tree in the same state; the shared trees are the integrator's to change):

```
P=<repo>/patches/octosense/0002-transport-never-drop-a-reply.patch
# the shared fork: applies ONLY 0002, as a commit; leaves its other uncommitted edits alone
bash <repo>/tools/prepare-octosense-fork.sh ~/home/oa.noindex/octosense-fork
# every host copy and the APK tree (a second run says "previously applied" and changes nothing)
for t in ~/home/oa.noindex/host-* ~/home/oa.noindex/apk-build; do
  patch -p1 --forward --dry-run -d "$t" < "$P" >/dev/null && patch -p1 --forward -d "$t" < "$P"
done
```

Do not use `git apply` inside a tree that sits in another git repository. Paths outside the current directory are
then silently ignored.

`tools/prepare-octosense-fork.sh` now applies 0001 then 0002:

- A tree at pin + 0001 gets only 0002.
- A fresh checkout gets both.
- A tree in any other state with uncommitted changes is refused unless `--force`. Before this change, the script
  reset such a tree and lost the changes.

## Verification

- Fork, `proto.rs`: `a_reply_that_finds_the_event_channel_full_waits_and_is_never_dropped` and
  `every_reply_arrives_in_order_through_a_full_channel`. Red before (every reply dropped), green after.
  `docs/ux/a22/transport/failing-first-fork.log`, `fixed-fork.log`.
- App, real WS transport: `tests/a22_sessions.rs every_history_reply_arrives_in_order_behind_a_flood_of_live_events`.
  Each history read is answered right behind 400 live events, with a consumer as slow as a busy UI. Every reply
  commits in the server's order and no read is left waiting. Red on 0001 in 7 of 7 runs: "the server answered 2
  history reads …, the client took 1; … 1 read(s) still waiting". Green with 0002. `failing-first-app.log`.
- Clippy (`-D warnings`) on the transport, and the transport and store suites: green.

## Exit

When the upstream PR (`docs/upstream/octosense-transport-never-drop-a-reply-PR.md`) lands: drop 0002 from
`tools/prepare-octosense-fork.sh` and bump the pinned rev, together with D10a's exit. Keep the app's safety net.
It also covers losses outside the transport.
