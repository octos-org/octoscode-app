#!/usr/bin/env python3
"""Unit tests for `render_variants.relativize()` (card #18f item 1).

Run:  python3 design/stage-b/conversation/tools/test_render_variants.py

Why these two cases (they are the two halves of the #18f item-1 regression):

* A **fill node** is wrapped by `design.rs` and inset with a `margin`, so its
  authored `x`/`y` must be converted to the offset FROM its immediate parent (a
  nested fill would otherwise apply the outer offset twice). A nested fill TEXT
  leaf under a padded parent must therefore keep its PARENT-relative offset —
  that is the invariant the restored `relativize()` provides.

* An **`Input`** is never wrapped: `design.rs` pins it with window-absolute
  `abs_pos` (`right_anchor` excludes `NodeKind::Input`, and no `is_fill_*` branch
  matches it). Rebasing one subtracts the parent's absolute origin and drags the
  question-card note placeholder up to the card's top-left, over the `?` icon and
  title. So an `input` must be left untouched no matter its `fillw`.
"""
from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from render_variants import relativize


class Relativize(unittest.TestCase):
    """#18f item 1: fill nodes are rebased on their parent; inputs are not."""

    def test_nested_fill_text_keeps_parent_relative_offset(self):
        tree = {
            "t": "stack", "id": "root", "x": 0.0, "y": 0.0, "w": 374.0,
            "c": [{
                # the padded parent is itself `fillw` (RESPONSIVE['question-card']
                # flags note_box), so it is rebased too and passes its own origin on
                "t": "stack", "id": "note_box", "x": 7.0, "y": 414.0, "w": 358.0,
                "fillw": 1,
                "c": [{
                    "t": "text", "id": "note", "x": 21.828, "y": 427.8,
                    "w": 328.343, "fillw": 1, "variant": "single_line",
                }],
            }],
        }
        relativize(tree)
        note = tree["c"][0]["c"][0]
        # offset from note_box (7, 414), not the root-absolute (21.828, 427.8)
        self.assertEqual(note["x"], round(21.828 - 7.0, 2))
        self.assertEqual(note["y"], round(427.8 - 414.0, 2))
        # right inset keeps the box at x .. x+authored_w at any parent width
        self.assertEqual(note["padright"], round(358.0 - (note["x"] + 328.343), 2))

    def test_input_is_pinned_absolute_not_rebased(self):
        tree = {
            "t": "stack", "id": "root", "x": 0.0, "y": 0.0, "w": 374.0,
            "c": [{
                "t": "stack", "id": "note_box", "x": 7.0, "y": 414.0, "w": 358.0,
                "fillw": 1,
                "c": [{
                    "t": "input", "id": "note_input", "x": 21.828, "y": 427.8,
                    "w": 328.343, "fillw": 1,
                }],
            }],
        }
        relativize(tree)
        note = tree["c"][0]["c"][0]
        # still parent-relative (NOT 21.828 - 7.0), just normalised to 2 decimals
        self.assertEqual((note["x"], note["y"]), (21.83, 427.8))
        self.assertIsNone(note.get("padright"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
