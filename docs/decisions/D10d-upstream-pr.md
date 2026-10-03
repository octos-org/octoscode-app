# Upstream PR draft (for the operator to submit — **not opened by an agent**)

**Repo:** `OctoSense-org/makepad` · **Base:** `6cf03859630761f5cb99ce7fcdfd8c30475d9ab8`
**Branch to propose:** `fix/ellipsis-after-soft-wrap`
**Patch:** [`patches/makepad/layouter-ellipsis-pen.patch`](../../patches/makepad/layouter-ellipsis-pen.patch)
(one file, `draw/src/text/layouter.rs`, additions only). Decision record:
[`D10d-makepad-layouter-ellipsis-pen.md`](D10d-makepad-layouter-ellipsis-pen.md).

## Title

text: start a truncation ellipsis where the kept text ends; re-align the truncated row

## Summary

`truncate_last_row_with_ellipsis` pops glyphs and places the ellipsis from the row's `width_in_lpxs`. A
soft-wrapped row keeps the space it broke at as its last glyph (for caret positions), while `finish_current_row`
leaves that space out of the width. Popping the space therefore subtracts its advance a second time, and the
ellipsis is placed one space advance short of the text, over the last glyph it keeps.

This is visible on any wrapping `Label` (the default `Flow::right_wrap()`) with `max_lines: 1` and
`text_overflow: Ellipsis` whose bound keeps a word's trailing space but not the next letter. "Fix steer queue
drop…" then reads "Fix steer queue drop..": the first dot is inside the "p" (Inter, 14 px: the ellipsis at
x = 132.631, the "p" ending at 136.568). Nothing is clipped, so `clip_x: false` does not help.

The row's `origin_in_lpxs.x` (from `align`) is likewise computed before truncation changes the width. An
overflowing centred non-wrapping row is left at a negative origin, its first glyphs outside its bound.

This PR:
- sets the row's width to the pen (the last glyph's origin + advance) before truncating, so a soft-wrap space is
  subtracted once (a no-op for rows that did not soft-wrap);
- re-computes the row's origin from its truncated width when there is a bound (`align * (max_width - width)`, as
  `finish_current_row` does).

## Tests

Three new unit tests in `layouter.rs`:
- `ellipsis_after_a_soft_wrap_space_starts_where_the_text_ends`: the reported title at a bound that keeps
  "drop " but not the "o".
- `ellipsis_row_geometry_holds_at_every_bound`: every bound from 30 to 320 px in 0.5 px steps, wrapping (1 and 2
  rows) and not. Each truncated row starts its ellipsis at the last kept glyph's end, ends at the ellipsis' end,
  and fits the bound.
- `a_truncated_aligned_row_is_placed_inside_its_bound`: align 0 / 0.5 / 1, wrap and no wrap.

Before the fix all three fail ("drawn 3.776 over it", "the row starts at -63.6"); after it all 25 layouter tests
pass.
