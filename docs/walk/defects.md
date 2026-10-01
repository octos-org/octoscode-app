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
