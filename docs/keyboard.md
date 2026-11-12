# Keyboard model (card #31e)

The native shell routes every key through ONE resolver — `crates/octoscode-module/src/screens/keys.rs`
(`screens::keys::resolve`), performed by the `Event::KeyDown` arm in `lib.rs`. Every rule cites its web
source; the pure table is pinned by `tests/f31e_keys.rs` (10 tests), and each binding has a live `/k`
receipt from `examples/keys_probe.rs` (the real shell + an in-process protocol server on MY port block).

| Binding | Web source | Native behaviour | Live evidence (`tmp/31e-evidence/`) |
|---|---|---|---|
| **Enter** (composer, no overlay) | `ComposerInput.tsx:237-243` (`!shiftKey && !ctrlKey && !metaKey`) | sends the draft via `conv.submit_draft()` — the same production path as the send affordance | `<- turn/start` after `/t` + Enter (the focused composer consumes Return itself, matching the web: the component handles Enter before the shell) |
| **Shift+Enter** | `ComposerInput.tsx:237` (`!shiftKey` guard) | never intercepted — the multiline input's own newline | resolver unit test (`enter_sends_and_shift_enter_is_the_newline`) |
| **Esc** (palette open) | `CommandPalette.tsx:40-43` | closes the overlay first | `/snap`: 19 visible rows → 0 after Esc; `key Escape -> PaletteClose` receipt |
| **Esc** (live turn) | `ComposerInput.tsx:245-252`; the approval card's Escape is the same interrupt (`ApprovalPanel.tsx:38-39`) | interrupts the turn (`turn/interrupt`) | `<- turn/interrupt` receipt with a live turn |
| **Esc** (diff review open) | the overlay-first precedence above; the approval card's Review diff / `D` opens the review (`ApprovalPanel.tsx:45`, `:93-99`) | closes the review first (`lib.rs` `escape_chrome`, A6) — never a `turn/interrupt` of the turn that waits on the approval | A6 walk (`tools/walk/a6_surfaces_walk.py`): `review diff opened=true`, Esc → the approval card again, no `<- turn/interrupt` |
| **Cmd/Ctrl+K** | `App.tsx:1096` (`(metaKey \|\| ctrlKey) && "k"`) | toggles the palette overlay | `/g` open/closed pairs; `key KeyK -> PaletteToggle` receipt |
| **"/"** (empty draft) | #28e item 5 (the composer's changed-action owns the same open) | opens (idempotent set — never toggles, the two paths converge) | `key Slash -> PaletteOpen` receipt |
| **↑ / ↓** (palette open) | `CommandPalette.tsx:44-49` (`(selected + dir + len) % len`, wraparound) | moves the selection through `screens::palette::resolve("palette.move")`; the highlight follows the LIVE row (draw fix: explicit redraw) | `g6-down5-highlight.png`: after ↓×5 the highlight rect sits on row 5 (`370,374` vs rows at y204+) |
| **Enter** (palette open) | the listbox's Enter runs the selection | runs the command via `screens::palette::resolve("palette.run")`, then closes the overlay | `<- session/list` count 1→2 (the `/resume` command's `session.refresh`) |
| **Alt+A** | `registry.ts:614` (Alt REQUIRED; `:633` rejects Ctrl/Meta so Cmd+Alt+A/AltGr stay inert) | resolves the show-approval parity shortcut; logged (the approval surface lands with the approval Stage-C screen) | `[octoscode] Alt+A show-approval (pending=true)` |
| **Y / S / N** (pending approval) | `ApprovalPanel.tsx:46-54` — approve/request, approve/session, deny/request; `:37-43` bare keys only | gate on the store's pending list (the client's `approval/requested` rows) OR the flow flag; sends `approval/respond {approval_id, decision, approval_scope, session_id}` — the r5-turn recording's grammar + the web's scope | three receipts: `approve/request`, `approve/session`, `deny/request` |
| **focus order** | `CommandPalette.tsx:66` (roving tabIndex; the search field is first focus) | the palette's search input takes focus when the overlay opens; Esc returns focus implicitly by closing the overlay | the palette path is keyboard-only end-to-end (open → move → run, no mouse) |

## Test/evidence map

- **Unit** (`tests/f31e_keys.rs`, all guards): Enter/Shift+Enter, Esc precedence (overlay → turn → ignore),
  Cmd/Ctrl+K + Alt+K rejection, "/" empty-draft/non-empty/open, arrows closed/open, Alt+A chords rejected,
  Y/S/N bare-only + no-pending, #28e chords preserved, the recorded `approval/respond` grammar, the store's
  pending FIFO. `ctest -p octoscode-module --test f31e_keys` → `10 passed; 0 failed`.
- **Live** (`examples/keys_probe.rs`, the real `OctoscodeView` shell): every binding above with `/k` +
  `/snap` + `/g` + `/log` + the server's wire receipts; `key -> action` lines are the resolver's own log.
  Evidence: `tmp/31e-evidence/` (app.log carries both channels: `[octoscode] key … -> …` and
  `[keys-probe-server] <- approval/respond …`).

## Known gaps (honest)

- The probe's fake server pushes `approval/requested`, but the client's notification-distribution path for
  pushed cards does not reach the store in the probe environment — the Y/S/N evidence seeds the store
  directly (`OCTOSCODE_APPROVAL_SEED`, the `OCTOSCODE_SYNTHETIC_TIMELINE` precedent). Distribution belongs
  to the client domain's card, not this keyboard card.
- `approval/decided` settle is likewise client-domain: with three seeded cards the keyboard always answers
  the FIFO head (`oldest_pending_id`), so the three receipts all name `a1-approve-me`. The grammar and the
  per-key semantics are still proven per key.
- The real desktop host owns a real key focus surface; the probe proves the resolver + arm on the same
  widget the host mounts, not the OS focus ring (unverified on device, per RULES).

While a response is running, Enter steers eligible text into that response by default. Tab explicitly queues the current draft for a later turn. `/steer off` restores queue-only submission. Image attachments and changes to the active reasoning effort remain queued. Ctrl+X or **Send now** requests interruption and sends pending input after the old turn stops, preserving the unfinished draft. Only inputs returned by `turn/steer_dropped` are replayed. Ctrl+X keeps its normal cut behavior when text is selected; Command+X always cuts.
