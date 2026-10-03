# Upstream PR (for the operator to submit — **not opened by a lane**)

**Repo:** `OctoSense-org/OctoSense` · **Target:** `apps/appcard/app/crates/octos-app-transport`
**Branch to propose:** `fix/transport-never-drop-a-reply`, on top of `feat/transport-generic-request` (D10a; or on
`main` once that lands — the two touch different hunks of `proto.rs` and apply in either order)
**Local fork commit:** `06ce11e` (on `ca62dfa` = `6e9bfd40` + 0001)
**Patch:** [`patches/octosense/0002-transport-never-drop-a-reply.patch`](../../patches/octosense/0002-transport-never-drop-a-reply.patch)

## One-line summary

Never drop a server reply: every event the transport hands to its consumer — a reply, an error reply, a
notification, a connection-state change — waits for room in the bounded event channel instead of being dropped
when it is full.

## Motivation

`proto.rs` delivered events two ways. Notifications (durable and `message/delta`) waited for room
(`events.send(..).await`). Replies (`RpcResult`, `CapabilityNegotiated`, `SessionsListed`, `SessionHydrated`),
error replies (`RpcError`) and connection states went through `try_emit`, a `try_send` that **dropped** the
event with a warning when the channel (`CHANNEL_BUFFER` = 64) was full.

A full channel is ordinary. A consumer that applies each event on a busy UI thread falls behind whenever the
server streams fast: another client's turn in a Session, a replay after a reconnect, a long history. The next
reply is then dropped. Unlike a notification, a reply is often the only answer to a request the consumer is
counting on. Seen in OctosCode, a client of this crate:

- A Session is opened while another client streams in it. The `session/hydrate` reply lands on a full channel
  and is dropped. The Session never leaves "Loading conversation…".
- OctosCode answers history reads in request order (one socket answers in order). Its history-read queue stays
  one read behind for good, so the Session's next history reply is judged against the lost read and discarded
  as stale.

The warning ("event channel full, dropping frame") is the only trace. Nothing upstream of the transport can tell
a reply was lost.

## The change (3 files: `proto.rs`, `ws/mod.rs`, `kernel.rs`)

1. `proto.rs`: `try_emit` is replaced by `emit`, an `async fn` that awaits `events.send(evt)`. A closed receiver
   ends delivery with a `debug` log (the consumer is gone; there is nobody to tell).
2. `handle_response` and `fail_all_pending` become `async` and await every event they emit. The notification
   paths use the same `emit`.
3. `ws/mod.rs` and `kernel.rs` await their connection-state changes.

`emit` is `pub(crate)`; no public type or signature changes.

## Behaviour change

When the consumer is slow, the read loop now waits on a reply exactly as it already waited on a notification. TCP
flow control then reaches the server. In `ws::run_live`, the `select!` does not take commands or tick the
heartbeat while it waits; that was already the case during a burst of notifications.

**Deadlock check.** The transport task is the only producer of events and reads commands in the same loop. A
consumer that awaits the command channel without draining events could stall both. That was true before this
change for notifications. Consumers should drain events on their own task, as OctosCode does: its `on_event`
is synchronous and its history reads use `try_send`.

## Tests

Two new tests in `proto.rs::tests`. Both fail before the change, because every reply is dropped:

- `a_reply_that_finds_the_event_channel_full_waits_and_is_never_dropped`: a one-slot channel, already full. A
  `session/hydrate` reply waits and is delivered once the consumer takes the earlier event.
- `every_reply_arrives_in_order_through_a_full_channel`: history replies for three Sessions, an error reply, a
  typed `session/list` result and a `turn/start` result arrive through a one-slot channel and a slow consumer.
  They are interleaved with notifications, and all of them come out in the server's order.

Before (fork at `ca62dfa` + the tests only):

```
assertion `left == right` failed: the reply was dropped
  left: ["ConnectionState(Live)"]
 right: ["ConnectionState(Live)", "reply p:api:b"]
assertion `left == right` failed: every reply, in the server's order
  left: ["n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9"]
 right: ["n1", "n2", "n3", "n4", "n5", "reply p:api:a", "n6", "n7", "n8", "reply p:api:b", "error session/hydrate", "listed", "n9", "turn", "reply p:api:d"]
```

After, run from the fork root (these commands were run):

```
cargo test --locked -p octos-app-transport --lib proto::      # 6 passed
cargo test --locked -p octos-app-transport -p octos-app-store # 45 + 34 + 1 + 1 passed, 1 ignored (live), 0 failed
cargo clippy --locked -p octos-app-transport --all-targets --no-deps -- -D warnings   # clean
```

Not run: `cargo clippy` for `octos-app` / `octos-app-render`, which this change does not touch.

## Compatibility

No API change. The only behaviour change is that replies are never dropped; the cost is that a slow consumer
now slows the reader on replies too. No wire change, no dependency change.

## Exit (for us)

OctosCode carries this as `patches/octosense/0002-transport-never-drop-a-reply.patch`, applied by
`tools/prepare-octosense-fork.sh` after 0001. When the PR lands, drop the patch from the script and bump the
pinned rev (with D10a's exit). See `docs/decisions/D10d-transport-never-drop-a-reply.md`.
