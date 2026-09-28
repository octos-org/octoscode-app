# DRAFT for the operator to file — OctoSense: a generic `Request` command in `octos-app-transport`

**Status:** draft. The exact patch, its test, and the PR text are produced by card #8 at
`docs/upstream/octosense-generic-request-PR.md` and `patches/octosense/0001-transport-generic-request.patch`.

**Summary:** `OutboundCommand` (`apps/appcard/app/crates/octos-app-transport/src/lib.rs:125`) is a closed enum of 8
commands, although the transport's internals are generic (`serialize_request(id, method, params)` at `proto.rs:167`,
`RpcRegistry`). Adding `Request { method, params: Value, reply }` through the same path lets clients reach every UI
Protocol method the way the web client does (`client.ts:868`), without changing connection, reconnect or replay logic.
octoscode-app carries it as a `[patch]` until this lands (its D10a).
