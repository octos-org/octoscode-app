# Protocol pin delta — octos `a6ea8505` vs web pin `4231669`

Scope: the single contract file `crates/octos-core/src/ui_protocol.rs`.
Command (both file:line citations below are from this diff and the two `git show`s):

```
git -C <WORKSPACE>/src-octos diff 4231669 a6ea8505 \
  -- crates/octos-core/src/ui_protocol.rs
git -C <WORKSPACE>/src-octos show 4231669:crates/octos-core/src/ui_protocol.rs
git -C <WORKSPACE>/src-octos show a6ea8505:crates/octos-core/src/ui_protocol.rs
```

`git diff --stat 4231669 a6ea8505 -- crates/octos-core/src/ui_protocol.rs` →
`1 file changed, 31 insertions(+)` — **no deletions, no renames**. Every difference is
additive (a new optional field or a new list entry); nothing is removed, renamed, or made
required, so a client written against either pin decodes the other's frames.

## 1. Method-name set: identical

Both pins define **125** `methods::*` consts, and the *value* set is identical:

```
only in 4231669: []
only in a6ea8505: []
name-set identical: True
```

The only method-registry change is two **notifications that were already declared** at
`4231669` being *added to the advertised `UI_PROTOCOL_NOTIFICATION_METHODS` list* at
`a6ea8505` (they were emitted but not advertised before):

| list entry | 4231669 | a6ea8505 | class |
|---|---|---|---|
| `turn/steer_dropped` | absent from the list | `crates/octos-core/src/ui_protocol.rs:1445` | additive (advertised now) |
| `session/orchestration` | absent from the list | `crates/octos-core/src/ui_protocol.rs:1488` | additive (advertised now) |

Both `methods::TURN_STEER_DROPPED` / `methods::SESSION_ORCHESTRATION` consts and their
`UiNotification` variants / decode arms exist in *both* pins (4231669 `:6698`, `:6720`,
`:7049`, `:7089`); only the advertised-list membership differs. `UI_PROTOCOL_NOTIFICATION_METHODS`
grows 49 → 51.

## 2. Field-level changes

### 2.1 `SessionOpenParams.client_commands` — **additive**

| | 4231669 | a6ea8505 |
|---|---|---|
| struct | `crates/octos-core/src/ui_protocol.rs:1993` | `crates/octos-core/src/ui_protocol.rs:1995` |
| new field | — | `client_commands: Option<Vec<String>>` @ `:2010` |
| serde | — | `#[serde(default, skip_serializing_if = "Option::is_none")]` |

Optional + defaulted: an old client that omits it still decodes; an old server ignores it.

### 2.2 `SessionListParams.profile_id` — **additive**

| | 4231669 | a6ea8505 |
|---|---|---|
| struct | `crates/octos-core/src/ui_protocol.rs:3173` | `crates/octos-core/src/ui_protocol.rs:3177` |
| existing `cwd` | `:3179` | `:3183` |
| new field | — | `profile_id: Option<String>` @ `:3192` |

### 2.3 `SessionListResult.{workspace_root, profile_id}` — **additive**

| | 4231669 | a6ea8505 |
|---|---|---|
| struct | `crates/octos-core/src/ui_protocol.rs:3186` | `crates/octos-core/src/ui_protocol.rs:3199` |
| existing `sessions` | `:3187` | `:3200` |
| new field | — | `workspace_root: Option<String>` @ `:3208` |
| new field | — | `profile_id: Option<String>` @ `:3212` |

Semantics per the doc-comment: `workspace_root` is present **only** when the server
actually scoped the listing to a project store (`<root>/.octos/<profile_id>`); a legacy
global listing omits it, so a client must not place rows under a workspace unless this
attests the scope. `profile_id` is present exactly when `workspace_root` is.

### 2.4 `TurnCompletedEvent.token_usage` — **additive**

| | 4231669 | a6ea8505 |
|---|---|---|
| struct | `crates/octos-core/src/ui_protocol.rs:6217` | `crates/octos-core/src/ui_protocol.rs:6242` |
| existing `session_result` | `:6235` | `:6260` |
| new field | — | `token_usage: Option<EnvelopeTokenUsage>` @ `:6266` |

Exact per-turn usage with reasoning / cache-read / cache-write kept distinct; absent when
the producer has no typed total, so consumers fall back to `tokens_in` / `tokens_out`.
(`EnvelopeTokenUsage` also appears at `a6ea8505:6292` inside the v2 envelope payload type —
that const already existed at the web pin as part of the projection envelope.)

**No breaking field-level change exists between the two pins.** Nothing was renamed,
removed, or changed from optional to required.

## 3. In-process AppUI serving at `a6ea8505` (commit `6804ee5d`) — yes

```
git -C <WORKSPACE>/src-octos show --stat --oneline 6804ee5d
   6804ee5d feat(cli): serve an AppUI connection in-process for embedding hosts
   crates/octos-cli/src/embedded.rs     | 75 ++++++++++++++++++++++++++++++++
   crates/octos-cli/src/lib.rs          |  2 ++
```

`crates/octos-cli/src/embedded.rs` exists at `a6ea8505` (blob
`7a1613a847139b9d9ddaea6fb94fbe33e37eb1df`) and is **absent at the web pin `4231669`**
(`git ls-tree a6ea8505 -- crates/octos-cli/src/embedded.rs` lists it; there is no such
path in the pin's tree, since commit `6804ee5d` is one of the 41 commits that separate
them).

What it exposes — **one public entry point**:

```rust
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn serve_io<R, W>(home: &Path, reader: R, writer: W) -> eyre::Result<()>
where R: AsyncRead + Unpin, W: AsyncWrite + Unpin + Send + 'static;
```

`serve_io` serves **a single frontend connection over any in-process async pipe pair**
(no child executable, no network listener, no alternate model loop). It:
- loads the host config from the private `home` and resolves the stored `_main` profile;
- defaults mobile card composition to fast inference (reasoning effort `Disabled`) unless
  a saved model / gateway preference overrides it;
- builds the provider with `create_provider_with_api_type` (rather than `bootstrap`'s
  `create_provider`, which prints a `Model: <id>` line to stderr — noise for a host that
  owns its stderr);
- bootstraps the local OUP runtime and hands the connection to
  `crate::api::ui_protocol_transport::embedded_stdio_connection_with_io`.

**Runtime requirement (documented in the file's header):** the AppUI dispatcher's poll
chain is deeper than Tokio's default 2 MiB worker stack, so the *host* must build its
runtime with `thread_stack_size(8 * 1024 * 1024)` and `spawn` (not `block_on`) `serve_io`;
on a default `#[tokio::main]` the first `session/open` overflows the worker stack and
aborts the process.

## 4. Recommendation: may the port target `a6ea8505` instead of `4231669`?

**Yes — this is safe and is the better target.** Grounds:

1. The 125-method *name/value* set is identical, so no call site needs to be re-mapped.
2. The only changes are **additive optional fields** plus two notification-list
   memberships. The `#[serde(default, skip_serializing_if = "Option::is_none")]`
   attributes mean either generation of client/server still interoperates on the wire.
3. `a6ea8505` additionally ships in-process AppUI serving (`embedded.rs`), which the
   native app needs and which the web pin does not have — a reason to prefer the newer
   pin rather than a risk of targeting it.
4. The native client already pins `a6ea8505`
   (`ref/OctoSense/Cargo.toml:85`), so adopting it as the port target removes a
   divergence between the contract doc and the client's real dependency.

Caveat: the two notification-list additions mean `server.hello` capabilities advertise
`turn/steer_dropped` and `session/orchestration` at `a6ea8505` where they did not at
`4231669`; a client gating on `supported_methods` will therefore *see* them (correctly —
both have real decode arms). No client change is required.
