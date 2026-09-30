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
