# D10d. makepad's text layouter starts a truncation ellipsis where the kept text ends, and re-aligns the truncated row: DECIDED (A32, 2026-10-02)

The judge's "below 9" item 1. A single-line title truncated with an ellipsis could end in TWO dots
("Fix steer queue drop..") instead of "…". The repro is A29's sidebar marker state:
`docs/ux/a29/desktop/desktop-2-switched-marker.png`, A29's row 810 in `docs/ux-scores.csv` (scored 8 for it).

## Root cause (numbers)

The leading hypothesis was a width mismatch: the Fill title truncated for a width that did not yet subtract the
following Fit chip, so the label's clip cut the third dot. **It is refuted.** A temporary trace in the A32 host
tree's `DrawText::draw_walk` (`docs/ux/a32/trace-instrument.diff`) logged the real values during
`tools/walk/a29_btw_aside.py` at desktop (`docs/ux/a32/trace-before.txt`).

| | value |
|---|---|
| width the title was truncated for (`max_width_in_lpxs`) | **145.811** |
| the label's walk | `Fixed(145.81)`: the row View defers the Fill title past its Fit siblings (`defer_walk_turtle`) and resolves it after the chip and the time |
| the label's turtle rect (its clip) / its /snap rect | 145.811 / `[92,300,146,17]` |
| the last kept glyph, "p" | x = 127.996, advance 8.572, **ends at 136.568** |
| the ellipsis glyph | **x = 132.631**, advance 12.100, ends at 144.730 |
| overlap | **3.938 px** = Inter's space advance (576/2048 em at 14 px) |

So the truncation width equals the final rect, and nothing is clipped: the ellipsis ends at 144.730 < 145.811.
The ellipsis is drawn 3.938 px too far left, over the "p". Inter's U+2026 at 14 px has its dots at x 1.09–2.94,
5.13–6.97 and 9.16–11.01. They land at 133.73–135.57, inside the "p" (its ink ends at 135.86), and at 137.76–139.60
and 141.79–143.64. Two dots are visible. This is also why A29's clip-off try still showed two dots.

Why one space advance: the title is an `OcLabel` (`flow: Right{wrap: true}`, the stock Label's default too). So
DrawText lays it out with `wrap: true`, `max_rows: Some(1)` and `ellipsis: true`.
- At 145.811 px the row fits "Fix steer queue drop " (its trailing space included, 140.506) but not the "o" after
  it (148.90). The last allowed row breaks there, a soft wrap.
- `finish_current_row(soft_wrap)` keeps the space's glyph in `glyphs` (caret positions) but leaves its advance out
  of `width_in_lpxs`.
- `truncate_last_row_with_ellipsis` then popped that glyph and subtracted its advance **a second time**. It placed
  the ellipsis at `width_in_lpxs`, one space short of the pen.

The bug needs a row that soft-wraps at a space and is then truncated: a bound between the end of "…word " and that
plus the next letter. That is why it was intermittent and per-width. At 155.811 px the "o" fits, the row does not
end at a space, and the same title drew "drop…" correctly. Non-wrapping labels (`flow: Right`, e.g. the peer dock
and the board-1 kit's `one_line()`) never soft-wrap, so they never showed it.

A second, latent bug in the same function: a row's `origin_in_lpxs.x` (from `align`) is computed in
`finish_current_row`, from the width before truncation. A centred or right-aligned truncated row therefore kept a
stale origin. An overflowing non-wrapping centred row sat at x = −63.6 for a 120 px bound, its first glyphs outside
the label. The board-1 kit's `.centered()` / `.right()` + `.one_line()` labels (pairing captions, `kv_row` values)
take that path whenever they overflow.

## Decision

One tracked makepad patch, `patches/makepad/layouter-ellipsis-pen.patch`. It is the shared path every `Label` /
`DrawText` with `text_overflow: Ellipsis` goes through, so no screen is patched one by one. It touches only
`draw/src/text/layouter.rs` and only adds lines:
1. `truncate_last_row_with_ellipsis` first sets the row's width to the **pen**: the last glyph's origin + advance,
   exactly the position `append_text` advanced to. A soft-wrap space it left out is then popped once, and the
   ellipsis starts where the kept text ends. For a row that was not soft-wrapped this is a no-op.
2. After the ellipsis is appended, the row is aligned again from its new width when there is a bound:
   `align * (max_width - width)`, the same formula as `finish_current_row`.
3. Three makepad unit tests that reproduce both bugs.

Not chosen:
- a kit-level workaround (e.g. `flow: Right` without wrap on our ellipsis labels): per call site, and it leaves
  every other wrapping label in the app, and in makepad, exposed;
- a wider label or a smaller font: the bug moves to another width.

## Proof

- **Failing first.** Commit 0753f06e has the tests-only patch. In the A32 host tree,
  `cargo test -p makepad-draw --lib text::layouter` fails 3 of 25 (`docs/ux/a32/makepad-tests-red.txt`):
  - "the ellipsis starts at 135.68001 but the last kept glyph ends at 139.45601: drawn 3.776001 over it" (IBM Plex
    Sans' space at 16 px);
  - the sweep fails at its first soft-wrap-at-space bound (30 px);
  - "wrap=false align=0.5: the row starts at -63.60801".

  `tests/a32_makepad_ellipsis.rs` was red too.
- **Green.** With the fix (b0d21845), 46/46 makepad text tests pass, the 25 layouter tests included
  (`docs/ux/a32/makepad-tests-green.txt`). The same patch applied to a copy of the APK tree's `.mk` layouter (no
  OctoSense line-breaking edit) also passes 25/25. The whole makepad-draw lib suite has 3 failures,
  `vector::triangulate::road_pack_tests`, which fail identically with the original layouter: pre-existing and
  unrelated.
- **The sweep test** (`ellipsis_row_geometry_holds_at_every_bound`) lays out the A29 title at every bound from 30 to
  320 px in 0.5 px steps, in four configurations: wrap with one row, wrap with two rows, no wrap with one row, no
  wrap unbounded. For every truncated layout it asserts:
  - the ellipsis starts at the last kept glyph's end (±0.01 px);
  - the row ends at the ellipsis' end;
  - the row fits the bound.

  It requires more than 500 truncations and more than 20 rows broken at a space.
- **Live, A29's marker state** (`tools/walk/a29_btw_aside.py`: desktop 67/67, phone 67/67).
  - Before the merge, with A29's rects (title [92,300,146,17]), the title reads "Fix steer queue dro…": the ellipsis
    at 127.996 = the "o"'s end, ending at 140.096 ≤ 145.811.
  - On the final clean host, merged with main, the title is [92,332,142,17] (bound 141.650, the bug's band again),
    the chip [240,331,46,18] and the time [292,333,24,15]. The title still reads "Fix steer queue dro…", with three
    dots.
  - Captures: `docs/ux/a32/desktop-2-switched-marker.png`, `docs/ux/a32/phone-2-switched-marker.png` (drawer: "Fix
    steer queue drop on r…"), the answered states `*-3-answered-in-x-marker.png`, and
    `docs/ux/a32/marker-title-before-after.png`.
- **Live sweep** (`tools/walk/a32_ellipsis_sweep.py` with the trace instrument; every truncated text drawn gets a
  verdict per geometry). Zero two-dot or outside-the-label endings:
  - desktop (`docs/ux/a32/sweep-desktop.json`, on the branch merged with main): the sidebar rows with and without
    the chip, the header with the sidebar (177.4 px) and with the rail (401 px), and the peer dock; each
    window-opening animation sweeps the title bounds frame by frame. 2 launches, 5 states, 19 rows measured, 28
    truncation geometries, 0 bad.

    After the merge the chip row's bound is 141.650 px, again inside the bug's band: it keeps "drop " (140.506) but
    not the "o" (148.90). It draws "Fix steer queue dro…" with the ellipsis at the "o"'s end, 127.996;
  - phone frames 300–368 px (`docs/ux/a32/sweep-phone.json`, merged): the drawer is min(320, w−48), so X's row
    title is truncated at 23 distinct bounds, from 113.7 to 226.1 px (with and without the chip). 11 launches, 19
    states, 84 rows measured, 70 truncation geometries, 0 bad; 23 kit-truncated texts, all fitting their labels;
  - the dialog family and the peer dock (`docs/ux/a32/sweep-dialogs.json`, merged, with the kit fix), with the trace
    on, desktop and phone:
    - 122 makepad truncation geometries, 0 bad;
    - 47 kit-truncated texts, every one inside its label: dialog titles "Apply a patch to sr…" (169.2 px in a 181.7 px
      label), fleet slugs "review-the-reconnect-…", skill rows "0.3.0 · 1 tool · octos-org/re…", diff paths;
    - walk results: `a5_dialogs` 37/37 + 37/37, `a28_diff_words` 29/29 + 31/31, `a10_diff_review` 19/19 + 19/19,
      `a10_skills` 17/17 + 17/17, `a10_fleet` 50/51 + 56/56. The one fleet miss is its race step, not an ellipsis
      check; it passed 56/56 on the final clean host.
    - `a30_peer_dock`: 52/58 desktop, 53/60 phone. Every miss is A30's check "the tree keeps 2+ session rows". On
      the merged branch the fleet scenario's tree holds ONE session, "New chat" (session_rows=1). A30's own evidence,
      from before main merged A22, reads 2–3.
      - Likely cause, not proven here: A22's row 228, "only full Sessions of the requested profile projected", for
        which 38374b47 fixed the btw scenario's fixture.
      - It reproduces on the final clean host. The dock's own geometry passes: no label outside or overlapping, and
        aligned to the tree's grid.
      - Nothing in this change touches session data; it is left to the integrator.
- `tests/a32_makepad_ellipsis.rs` pins the patch's shape and, with `MAKEPAD_PATCH_TREES`, that it is applied in
  every tree.

## The kit's side: board-3 titles truncated by estimate

The dialog titles, row names and paths of the board-3 kit are truncated by our kit, not by makepad.
- `ui::fit_w` cuts the string to its room from per-character em estimates and appends "…".
- A plain `flow: Right` Label then draws that string.

A run the estimate under-measures is wider than its label, and the label's clip cuts the trailing "…": the same
two-dot symptom. The classes it under-measures, against Inter's real advances:
- "-": 0.46 em, estimated at 0.34;
- capitals: up to 0.77, estimated at 0.68;
- digits: up to 0.66, estimated at 0.58.

`tests/a32_kit_ellipsis.rs` writes each label as the kit does (`Dsl::text`) and lays it out the way makepad's
DrawText does, with the kit's real faces, in a label of exactly the room `fit_w` was given. The sweep covers 132
strings, 8 (face, size) pairs and 44 rooms. It was red first (1c2d69b1): 44 of 20,202 kit-truncated runs ended past
their label. The worst was the fleet slug "review-the-reconnect-backoff-before-mergi…" at 13 px in 280 px, ending
3.92 px outside, which clips the third dot.

**Kit fix (758aaec3, `screens/board3/ui.rs` `Dsl::text`).** The kit writes makepad's own ellipsis
(`max_lines: 1 text_overflow: TextOverflow.Ellipsis`) on a run that is:
- not wrapping;
- bounded (`Fill` or a fixed width);
- ends in "…";
- has no line break.

A run that fits draws exactly as before; one that does not is cut again by makepad, inside its label. Its row does
not wrap, so this works on any tree, patched or not. Runs that do not end in "…" (table columns, pills, keys),
natural-width runs and wrapping runs are untouched. Result: 20,202 runs, 46 cut again by makepad, 0 past their label.

## Apply (integrator)

The patch was applied only to A32's own host tree. For the four trees (N = the oa.noindex work root):
```
scripts/apply-makepad-patches.sh $N/host-<name>/.sources $N/octosense-fork/.sources $N/apk-build/.sources $N/apk-build/.mk
MAKEPAD_PATCH_TREES=$N/host-<name>/.sources:$N/octosense-fork/.sources:$N/apk-build/.sources:$N/apk-build/.mk \
  cargo test -p octoscode-module --test a32_makepad_ellipsis
```
It dry-runs "applicable" on octosense-fork, apk-build `.sources` and apk-build `.mk` today (`--check`). The script
is idempotent and the other two patches stay "applied". Rebuild the hosts afterwards: the fix is in makepad-draw,
not in our crates.

## Back to clean upstream

`patch -R -p1 -d <tree>/makepad < patches/makepad/layouter-ellipsis-pen.patch`. When upstream takes the fix
(`D10d-upstream-pr.md`), drop the patch file. Nothing in our crates depends on it.

## Upstream

`D10d-upstream-pr.md` is the PR draft for the operator to submit. No agent opens it.
