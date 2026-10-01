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
