# Upstream PR (for the operator to submit — **not opened by a lane**)

**Repo:** `OctoSense-org/OctoSense` · **Target:** `apps/appcard/app/crates/octos-app-transport`
**Branch to propose:** `feat/transport-generic-request` (from `main` @ `6e9bfd40`)
**Local fork commit:** `ca62dfab360b52e6355681fd17348ad33afd8de2`
**Patch:** [`patches/octosense/0001-transport-generic-request.patch`](../../patches/octosense/0001-transport-generic-request.patch)

## One-line summary

Add `OutboundCommand::Request { method, params, reply }`: a generic UI-Protocol
call, so a caller can issue **any** method — not only the handful with a typed
`OutboundCommand` today.

## Motivation

`octos-app-transport` currently exposes ~11 typed commands (`OpenSession`,
`StartTurn`, `SessionList`, `HydrateSession`, …). The protocol has **125 core
methods** (`docs/protocol-matrix.csv`) plus **34 AppUI extension methods**
(`docs/protocol-ext-matrix.csv`). The web client does not enumerate them: it has
one generic `request(method, params)` (`packages/client/src/client.ts:426`) and
a thin typed wrapper per domain on top.

Without a generic command, every non-typed method forces an upstream change to
`OutboundCommand` + `proto.rs` + a new `PendingReply` variant — for ~79 methods
that is 79 upstream patches to move one client forward. The generic command
collapses that to **one**.

This is exactly the seam `octos-app-transport` already has: `serialize_request`
+ `RpcRegistry` are method-agnostic. The typed commands are conveniences over
that seam, not the seam itself. This PR lets a consumer use the seam directly.

## The change (108 insertions, 4 deletions; 2 files)

1. `lib.rs` — one new `OutboundCommand` variant:
   ```rust
   Request {
       method: String,
       params: serde_json::Value,
       reply: oneshot::Sender<Result<serde_json::Value, RpcError>>,
   },
   ```
2. `proto.rs` — routed through the **same** `serialize_request` +
   `RpcRegistry` path as every typed command; the reply oneshot receives the
   raw `result` value (typed decode is the caller's job) or the server's
   `RpcError`. `PendingRequest.method` becomes an owned `String` (a generic
   request's method is runtime data); the two existing constructors pass
   `methods::SESSION_OPEN.to_string()`.

**Nothing else changes.** Unknown-method JSON-RPC errors flow back through the
existing `PendingReply` error path.

## Tests

Two unit tests in `proto.rs::tests`, both green:

- `generic_request_builds_a_jsonrpc_frame_and_registers_its_reply` — the frame
  is well-formed JSON-RPC 2.0 with the caller's method + params, the pending
  reply is recorded under the frame id, and a delivered result reaches the
  caller's oneshot verbatim.
- `generic_request_error_reaches_the_caller` — an `RpcError` reaches the
  caller's oneshot as `Err`.

Crate suite on the fork branch: `test result: ok. 32 passed; 0 failed`.

## Compatibility

Purely additive: a new enum variant plus one `PendingRequest` field widened
from `&'static str` to `String`. Existing callers are unaffected (they pass a
`&'static str` and it converts). No wire change. No dependency change.

## Exit (for us)

Our root `Cargo.toml` carries a `[patch]` redirecting
`octos-app-transport` (and `-store`, to keep one copy) at the local fork. **Drop
that `[patch]` and bump the git rev when this lands upstream** — see D10a in
`docs/phase0/DECISIONS.md`.
