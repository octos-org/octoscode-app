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
