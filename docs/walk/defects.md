# Walk-runner defects (#33a)

Final full run (`docs/walk/results.csv`, task/33a): **0 fail** — every scripted row
(114 of 114 eligible) passes against the replay server, so there are no defects to
file under the entry's rule (one line per FAIL: row, what you saw, capture path).

Trail for the earlier rounds (all check-side, fixed on this branch — none were app
defects, so none are defects under this card's rule):

- run1/run2 blocked all 114 selected rows: the mount predicate checked for a widget
  id `heading`, which the #28e shell no longer emits (the OctosCode title is the
  TEXT of an unnamed Label). Forensics: tmp/33a-start.log (fixed in dc51b34).
- 19 conversation fails (validate run): the composer-clear burst lost every key
  after the first on a populated draft; fixed with the probed home → shift+end →
  backspace mechanism (3b9f4d1; evidence in the probes + results-checks.csv of
  that run, since overwritten by the green final run).

## #33b — rows whose new specific check FAILS (filed per the entry)

- **row 165** (settings, "keeps server connection actions in General settings"): the
  Session settings drawer renders Model / Permissions / Sandbox / Context and NO
  connect/disconnect action anywhere in the drawer (check `General settings carries
  the server connection action` -> `connection action in drawer=None`). Capture:
  docs/walk/evidence/area-settings.png (final full run).
- **row 209** (recovery, "a failed local-command chunk restores input and never sends
  command text to the model"): '/bogus-command walk probe' + return sends no model
  turn (fail-closed holds) but the composer is CLEARED (draft='') instead of
  restoring the input for the user to fix (check `a failed local command restores
  the typed input and sends nothing` -> `sent=False draft_restored=False`). Capture:
  docs/walk/evidence/area-recovery.png (final full run).
- **row 190** (composer, "continues past an unknown turn without replaying it…"):
  both prompts driven sequentially each get their own answer, but after reselecting
  the thread row the timeline-bubble assertion fails (`turns_own_no_replay=False`,
  sessions stable at `1`): the first bubble is not (or not uniquely) present in the
  post-reselect snap — likely PortalList virtualization unmounting the offscreen
  first bubble (raw whole-snap text double-counts via the thread-row title's two
  instances). Capture: docs/walk/evidence/area-composer.png (final full run);
  retests tmp/33b-composer-retest2.log, full run tmp/33b-walk-final3.log.

## #39a — live FAIL (REAL dsflash gate, --live run 3)

- **row 2** (conversation, "keeps a background turn alive while a sibling Session is
  focused"): a real turn was started, then a sibling session was focused via New
  chat. At that point the sidebar `thread_list` rendered ZERO rows and the status
  label flipped to `conn: Live   sessions: 0` (it read `sessions: 1` at mount); the
  original session's row (`dsflash:main`) never came back within the ~23 s
  observation window (15 s title-text poll + waits), so the background turn's
  outcome was unreachable in the UI. Check: `a live turn keeps running while a
  sibling session is focused` -> "the original session row never came back".
  Candidate causes (unverified): `session/list` not refreshed after `session/new`,
  or the new-session navigation dropping the store's session rows. Capture:
  docs/walk/evidence/phase3/39a-live-row2-fail.png (+ .snap.json: empty
  thread_list, status `sessions: 0`, timeline showing only the fresh session's
  newchat/composer widgets). Row 1 (a real coding turn streams, terminates, and
  the timeline keeps bubble+answer) PASSED in the same run
  (streamed+terminal=1.7 s, prose=2, bubbles laid out=2).

## #41d — live FAIL (REAL dsflash gate, row 33): file deliveries have no native attachment surface

- **row 33** ("shows files the agent delivers, previews images, downloads them…"):
  the scripted live slice asked the model to create two files (note-a.txt,
  note-b.txt) in the live workspace. The turn ran for real (5s; 3 tool cells:
  write_file ×2 + bash; the assistant prose renders the delivery as a markdown
  FILE TABLE). The check asserts the web contract — laid-out ATTACHMENT rows —
  and found ZERO: the native timeline has no attachment-row widget at all
  (instrument: the failure snap carries no id/text matching
  attach/download/file outside the prose itself; capture:
  docs/walk/evidence/phase3/41d-row33-fail.png + .snap.json). The delivery
  CONTENT is visible (prose table + tool cells), but the web's attachment
  surface (rows, preview, download, persists across reload) has no native
  equivalent yet. Candidate app card: an attachment-row kind in
  screen::timeline_rows fed from the delivery/file notifications. Turns spent
  on this row: 1 (of the 60 cap). Rows 51/63/65/67/76/77/92/101/193/194/197/
  198/199 stay unscripted (approvals the model must raise itself, phone
  viewports, browser clipboard/link surfaces) — dispositions in
  .peer/report-41d.md.

## #41e — live FAIL (REAL dsflash gate, row 50, the ONE allowed prompt): the gate executes shell commands without raising an approval

- **row 50** ("approval shortcuts ignore modified keys and IME composition" — its
  live prerequisite is an approval card existing at all): the card allowed one
  prompt; `run \`ls\` in a shell` was sent under dsflash. The turn ran for real
  (2s; a `bash` tool cell -> "done"; the prose renders the ls output table) and
  NO approval card ever appeared — the failure snap carries ZERO widgets whose
  id contains "approval" (not even zero-width), i.e. no `approval/requested`
  notification arrived and the shell never had a card to show. NOT a native
  shell defect: the gate's autonomy configuration executes the tool call
  directly. This empirically closes row 50's prerequisite — the model/gate will
  not produce the approval state on demand (the composer's "Ask for approval"
  label is static component art; the mode is the gate's). Capture:
  docs/walk/evidence/phase3/41e-row50-fail.png (+ .snap.json). Turns spent: 1
  (cumulative 5/60 across #41d+#41e). No app card candidate; re-test only if
  the operator reconfigures dsflash's autonomy mode.
## #41c — five reproduced FAILs from the new row-specific checks (runs 3+5 agree)
Each is a walk row's OWN case now asserted specifically (tools/walk/run.py `#41c` block); each FAIL reproduced in
run 3 (`tmp/41c-walk-run3.log`) and run 5 (`tmp/41c-walk-run5.log`) after the check-harness bugs were fixed out.
| row | check | measured |
|---|---|---|
| 72 | Alt+D reaches a fleet capability notice surface to focus | keys.rs has no KeyD Alt arm; snap after keyd&alt carries no capability/notice widget — `capability-notice widgets=none` |
| 73 | Alt+P toggles a peer dock fold | no `peer_dock` widget mounts at all — `peer_dock rects=None -> None` |
| 128 | Fleet's roster rows render with real rects | `fleet_row_1 rect=[0,0,0,0]` under r6-peer: ids mount, rects stay collapsed (the id-only smoke check masked this) |
| 102 | the palette's /monitor command reaches a monitors surface | `/monitor` is table data only (`palette_commands`, lib.rs:3099); executing it closes the palette and mounts nothing — `monitor ids=[] texts=[]` (the run arm logs `no native effect`) |
| 58 | Escape closes the settings drawer and the trigger still works | Esc does NOT close the drawer; the check recovered via the drawer's own `settings_close` control and the trigger re-opens fine — so only the Esc leg is broken |
Non-#41c fails in run 5: row 57 (longcode clipboard) is the pre-existing known-red; row 57/58's `longcode` check
(a 227-column line) is likewise pre-existing (kept failing on main).
Also recorded: `keyk&cmd` does not open the palette on the walk host while `keyk&ctrl` does (three runs), with cmd
delivery itself proven (Cmd+E opens the review dock, #36e) — keys.rs:106 accepts `logo || ctrl`, so the logo arm
is the suspect; the a11y check asserts the ctrl leg (proven delivery).
## #42a — live row 50 disposition: GATE CONFIGURATION, not an app defect (shell ran with no approval card under the static "Ask for approval" pill)
Verdict: (b) — the server/profile policy decides; the app's (missing) mode
send is not what produced row 50. Evidence:
1. The native app sends NO permission mode anywhere: the gate trace
   (live-gate/evidence/trace.jsonl, 5768 frames) carries `session/open`
   outbound = `{"cwd","profile_id","session_id"}` only (no mode, no
   sandbox), and ZERO outbound `permission/profile` frames (the string
   only appears inside `session/open`'s capability list). In the crate,
   `permission/profile/*` appears ONLY in tests
   (crates/octoscode-client/tests/r2_replay.rs:293-309) — no production
   sender.
2. BUT the web, under the gate's exact conditions, sends nothing either:
   the mode/sandbox defaults are THIS BROWSER's localStorage preference,
   applied at CREATION only (src-web .../session-config/session-defaults.ts:4-8);
   with nothing stored `loadSessionDefaults` returns null
   (App.tsx:643-648) and the creation path skips the send entirely
   (App.tsx:1941 `if (outcome === "opened" && defaults && ...)`). When a
   preference IS stored, the web fires ONE
   `setPermissionProfile({session_id, update:{mode, network}})` after
   creation (App.tsx:1956-1972). A headless gate has no stored browser
   preference, so web-on-the-gate ≡ native: no mode sent. Parity holds on
   this path; the missing native control is a FEATURE gap, not row 50's
   cause.
3. The deciding policy is server-side: the gate profile carries its own
   execution policy — live-gate/data/profiles/dsflash.json:
   `sandbox: {enabled: true, mode: "auto", workspace_write: true,
   allow_network: false, docker: {...}}`. Upstream, the approval flow only
   asks when the command policy returns Ask
   (src-octos @4231669 crates/octos-agent/src/policy.rs:28-36:
   `ApprovalPolicy::Ask` is the DEFAULT — "Ask an interactive client when
   a command policy returns Decision::Ask"); a workspace-contained shell
   command under a writable sandbox does not hit Ask, so it executes with
   no approval card. The permission-profile default (no client override)
   is WorkspaceWrite + network Deny
   (src-octos @4231669 crates/octos-core/src/ui_protocol.rs:2349-2357),
   and WorkspaceWrite deliberately "leaves the inherited (writable)
   sandbox untouched" (crates/octos-agent/src/policy.rs:234-258).
4. The pill that motivated the row is NOT a live control: the composer's
   "Ask for approval" is a static artboard label (it0_composer_2_0, from
   the lowered composer DSL), and the string exists nowhere in the web
   source — the web's real control labels are Read/Write/Full access
   (src-web .../shell/permission-projection.ts:52-60) fed from the
   server's `permission/profile/list` current mode.
Follow-up (feature gap, NOT this row's defect): the native app has no
mode selector and no creation-time `permission/profile/set` (the web's §7
new-session-defaults). Candidate app card: bind the pill to the server's
live current mode + a selector that sends `permission/profile/set`
(client generic `.request()` already proven by r2_replay.rs:293-309).
Turns spent on this row: 1 (of the 60 cap, coordinated with p0-harness's
ledger — no re-run needed for a (b) disposition).

## Pre-existing (main-carried): the hermetic scan trips on design/stage-a/phase4-new/atlas-prompt.md (#P4b2 found while running the ACK suite)

`octoscode-client --test repo_hermetic no_machine_paths_or_secrets_anywhere_tracked` FAILS:
`design/stage-a/phase4-new/atlas-prompt.md: literal /Users/`. A/B: the file (and its 2 literals) arrive
from origin/main `99b7a4f` (`git show 99b7a4f:design/stage-a/phase4-new/atlas-prompt.md | grep -c /Users/` → 2);
this lane's committed files scan clean (`git ls-files docs/walk/evidence | xargs grep -l /Users/` → empty).
Known-red for main to fix (scrub to `<WORKSPACE>` per the fixture-hygiene rule); recorded here so the next
lane's full-suite read is not surprised. Full suite on task/P4b2 @ merge `d29f3e2` with --no-fail-fast:
TOTAL passed=531 failed=1 (this file).
