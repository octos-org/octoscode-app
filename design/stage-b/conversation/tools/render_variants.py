#!/usr/bin/env python3
"""Card #16b: render each L0 component's short/long variants, hidden, at 360 and 540.

For every component under `design/components/<id>/` this copies the component,
applies a variant transform to its `mapped.json` tree (text, responsive
fill/fit flags, and structural inserts for tool-cell expanded/failed and the
composer queued chip), re-compiles it with the flow's OWN compiler
(`flows/image-lib/compile.py::compile_page`), renders it hidden with
`beauty-host` at 360 and 540, and writes:

  design/components/<id>/variants/<variant>-<w>.png   the raw native render
  design/components/<id>/review-<variant>.png         atlas crop | native@360 | native@540

Why go through `compile_page` rather than hand-editing the kit: `fillw`/`fith`
are carried in the node's *style*, which the compiler lowers to the kit pack and
`design.rs` reads as `a.fillw`/`a.fith`; hand-editing the kit would bypass the
one code path production uses. Inserted nodes get matching `semantic-map.json`
entries so `preflight` stays green (it requires every id classified).

Run:  python3 tools/render_variants.py [component ...]
"""
import hashlib
import json
from collections import Counter
import os
import shutil
import socket
import subprocess
import sys
import threading
import time
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parents[1]          # design/stage-b/conversation
ROOT = Path(__file__).resolve().parents[4]          # repo root (p0-harness)
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
COMPONENTS = ROOT / "design/components"
WORK = ROOT / "tmp/stage-b/render-variants-18"
ART_PORT = 8181                                     # 8179/8180 are other lanes' art servers
PORTS = [8346, 8347, 8348, 8349]                    # this card's block (8340-8349)
WIDTHS = (360, 540)
# Card #18c/#18e: the atlas crop is card-tight, so the extracted root sits at
# x=0 and a `Fill` root would run edge-to-edge with its right border/radius at
# the window's last pixel. Give it the page gutter the atlas normalised away so
# all four rounded corners are inside the render at every width.
ROOT_GUTTER_X = 16
ROOT_GUTTER_Y = 8

INTER4 = "self:resources/ux/Inter-400.ttf"
INTER5 = "self:resources/ux/Inter-500.ttf"
INTER7 = "self:resources/ux/Inter-700.ttf"
MONO = "self:resources/ux/LiberationMono-Regular.ttf"
RED = 0xFFCF222E
GREY = 0xFF6E6E73
INK = 0xFF1D1D1F
MONOBG = 0xFFF6F6F7

# Responsive flags (card #16b: fill the slot width, height from content). Applied
# to every variant. `fillw` on a text node lets it wrap to the slot; `fith` on
# the root lets it hug its content.
def _btn(*ids):
    """A KitButton is a `stack` wrapping `_surface` (the rounded pill), `_control`
    (the hit area) and `_label`. `#18b`: the pill stayed at its measured width
    inside a slot-filling button, so the fill stopped short at 540 and the pill
    clipped at 360. The container + surface + control fill the slot.

    The LABEL deliberately keeps its measured box: bisected on this renderer, a
    free-standing `text` node carrying ANY responsive flag (`fillw`/`fitw`/`fith`)
    paints nothing (deny label ink 301 -> 0), so filling the label would erase it.
    At the reference width the label's measured x is already exact."""
    d = {}
    for i in ids:
        d[i] = {"fillw": 1}
        d[i + "_surface"] = {"fillw": 1}
        d[i + "_control"] = {"fillw": 1, "fillh": 1}
    return d

RESPONSIVE = {   'thread-row': {   'thread_1': {'fillw': 1, 'fith': 1},
                      'thread_1_surface': {'fillw': 1},
                      'thread_1_control': {'fillw': 1, 'fillh': 1},
                      'thread_1_label': {'fillw': 1}},
    'new-chat': {   'new_chat': {'fillw': 1, 'fith': 1},
                    'new_chat_surface': {'fillw': 1},
                    'new_chat_control': {'fillw': 1, 'fillh': 1},
                    'new_chat_label': {'fillw': 1}},
    'worked-for': {   'worked_row': {'fillw': 1, 'fith': 1},
                      'worked_row_surface': {'fillw': 1},
                      'worked_row_control': {'fillw': 1, 'fillh': 1},
                      'worked_row_label': {'fillw': 1}},
    'user-bubble': {'user_bubble': {'alignx': 1}, 't01': {}, 't02': {}},
    'working-row': {'working_row': {'fillw': 1, 'fith': 1}, 't03': {'fillw': 1}},
    'assistant-prose': {'answer_prose': {'fillw': 1, 'fith': 1}, 'answer_md': {'fillw': 1, 'fith': 1}},
    'answer-actions': {   'answer_actions': {'fillw': 1, 'fith': 1},
                          't11': {'fillw': 1, 'alignx': 1, 'x': 0}},
    'tool-cell': {   'tool_1': {'fillw': 1, 'fith': 1},
                     't01': {'fillw': 1},
                     't02': {'fillw': 1},
                     'icon_check1': {'alignx': 1}},
    'composer': {   'composer_idle': {'fillw': 1, 'fith': 1},
                    'composer_idle_input': {'fillw': 1},
                    'send1': {'alignx': 1}},
    'approval-card': {   'approval_card': {'fillw': 1, 'fith': 1},
                         'cmd_box': {'fillw': 1},
                         'approve_once': {'fillw': 1},
                         'approve_once_surface': {'fillw': 1},
                         'approve_once_control': {'fillw': 1, 'fillh': 1},
                         'approve_session': {'fillw': 1},
                         'approve_session_surface': {'fillw': 1},
                         'approve_session_control': {'fillw': 1, 'fillh': 1},
                         'deny': {'fillw': 1},
                         'deny_surface': {'fillw': 1},
                         'deny_control': {'fillw': 1, 'fillh': 1}},
    'question-card': {   'question_card': {'fillw': 1, 'fith': 1},
                         'note_box': {'fillw': 1},
                         'note_input': {'fillw': 1},
                         'opt_ledger': {'fillw': 1},
                         'opt_memory': {'fillw': 1},
                         'opt_ask': {'fillw': 1},
                         'submit_answer': {'fillw': 1},
                         'submit_answer_surface': {'fillw': 1},
                         'submit_answer_control': {'fillw': 1, 'fillh': 1},
                         'skip': {'fillw': 1},
                         'skip_surface': {'fillw': 1},
                         'skip_control': {'fillw': 1, 'fillh': 1}},
    'edited-files-card': {   'edited_files_card': {'fillw': 1, 'fith': 1},
                             'files_card': {'fillw': 1},
                             'div_1': {'fillw': 1},
                             'div_2': {'fillw': 1},
                             'review_surface': {'fillw': 1},
                             'review_control': {'fillw': 1, 'fillh': 1},
                             # Card #18d (外环补充): the trailing cluster pins to
                             # the slot's right edge with the #16c `alignx`
                             # mechanism: the per-file +N/-N stats, the disclosure
                             # chevron, and the Undo/Review group.
                             # The per-file +N (green) / -N (red) stats are TWO
                             # adjacent TEXT runs. Right-anchoring them is not
                             # expressible here: design.rs's #16c wrapper is
                             # `alignx==1 && kind != Text`, so on a text node
                             # `alignx` only sets the run's alignment INSIDE its box,
                             # and adding `fillw` makes each run a full-width label
                             # (measured: both boxes 0..540, so +31 and -4 overlapped).
                             # Keep the measured pair, which matches the atlas at 360.
                             # atlas: chevron 333..340.5 in a 374 root -> inset ~20.5
                             'icon_show': {'alignx': 1, 'x': 337.5},
                             'undo_group': {'alignx': 1},
                             # Card #18e: the Review pill must HUG its label and be
                             # right-anchored (the #18c state). `fillw` made the
                             # button span the slot, so it covered the title and the
                             # Undo group (measured: the header band collapsed to
                             # "E..."). `alignx` alone right-anchors the measured
                             # 79px pill; `_btn` keeps its surface/control filling
                             # that box.
                             'review': {'alignx': 1}},
    'plan-card': {'plan_card': {'fillw': 1, 'fith': 1}, 'plan_steps': {'fillw': 1}},
    'goal-strip': {'goal_strip': {'fillw': 1, 'fith': 1, 'variant': 'row'}, 't01': {'fillw': 1}},
    'diff-view': {   'diff_view': {'fillw': 1, 'fith': 1},
                     'diff_rows': {'fillw': 1},
                     'file_divider': {'fillw': 1},
                     'file_header': {'fillw': 1},
                     'scope_pill': {'fillw': 1},
                     'row_1': {'fillw': 1},
                     'row_2': {'fillw': 1},
                     'row_3': {'fillw': 1},
                     'row_4': {'fillw': 1},
                     'row_5': {'fillw': 1},
                     'row_6': {'fillw': 1},
                     'folded': {'fillw': 1}},
    'settings-group': {   'settings_group': {'fillw': 1, 'fith': 1},
                          'perm_card': {'fillw': 1},
                          # NOTE (#18d): `variant:"row"` here emits NOTHING (the
                          # measured parent is an Overlay), so the card stays an
                          # overlay and only the chevron is right-anchored.
                          'model_card': {'fillw': 1},
                          'perm_divider': {'fillw': 1},
                          # Card #18d (外环补充): pin the trailing controls to the
                          # slot's right edge with the #16c `alignx` mechanism.
                          'toggle1': {'alignx': 1},
                          'toggle2': {'alignx': 1},
                          # t_pick is a TEXT run: `alignx` here is the run's own
                          # alignment inside a FILLED label, which overlays its
                          # trailing chevron (measured: chevron squeezed to 4px).
                          # Keep the value measured; anchor the chevron instead.
                          't_pick': {},
                          # the chevron rides inside `model_row` (ROW_WRAP below),
                          # so it no longer needs its own anchor.
                          'pick_chev': {},
                          't01': {'w': 340}}}

# The expanded tool-cell console, from scene 04's third card (`tool_3_output`).
OUTPUT_BOX = {"t": "stack", "id": "tool_1_output", "x": 10, "y": 84, "w": 351, "h": 116,
              "fillw": 1,
              "variant": "surface", "bg": MONOBG, "radius": 8, "c": [
                  {"t": "text", "id": "o1", "x": 18, "y": 10, "w": 316, "h": 17, "size": 13.4,
                   "weight": 400, "font_src": MONO, "line_height": 16.6, "color": 4281216815,
                   "variant": "single_line", "text": "running 12 tests"},
                  {"t": "text", "id": "o2", "x": 18, "y": 38, "w": 316, "h": 17, "size": 13.4,
                   "weight": 400, "font_src": MONO, "line_height": 16.6, "color": 4281479731,
                   "variant": "single_line", "text": "test steer_queue::reconnect_ok ... ok"},
                  {"t": "text", "id": "o3", "x": 18, "y": 66, "w": 316, "h": 17, "size": 13.4,
                   "weight": 500, "font_src": MONO, "line_height": 16.6, "color": 4281362226,
                   "variant": "single_line", "text": "test result: ok. 12 passed; 0 failed"},
                  {"t": "text", "id": "o4", "x": 18, "y": 92, "w": 316, "h": 17, "size": 13.4,
                   "weight": 400, "font_src": MONO, "line_height": 16.6, "color": 4281216815,
                   "variant": "single_line", "text": "Finished in 0.42s"},
              ]}
# A red status glyph, used by the failed variant (the asset check icon cannot be
# recoloured through the kit, so it is collapsed to zero and replaced by a glyph).
FAILED_X = {"t": "text", "id": "status_x", "x": 336, "y": 25, "w": 20, "h": 23, "size": 17,
            "weight": 700, "font_src": INTER7, "line_height": 21, "color": RED,
            "variant": "single_line", "text": "\u2715"}
# Card #16c: the compose (pencil) glyph lives at the new-chat row's right edge in
# scene 01, as a SIBLING of `new_chat` — so extraction (which takes only the root
# subtree) dropped it. Re-attach it, parent-relative (scene x347-16=331, y130-120=10).
COMPOSE_ICON = {"t": "svg", "id": "icon_compose", "x": 332, "y": 24.5, "w": 24, "h": 28,
                "alignx": 1, "src": ""}
# Card #16c: a command cell ("Ran cargo test") carries the terminal `>_` glyph, not
# the file glyph (which is for Read/Edit). Same measured box as `icon_file`.
TERM_ICON = {"t": "svg", "id": "icon_term", "x": 25.791, "y": 27.391, "w": 23.807, "h": 29.674, "src": ""}
# The asset each inserted svg binds to (relative to the component folder). The
# semantic map needs it or preflight reports the icon as artwork-less.
ICON_ASSETS = {"icon_compose": "assets/icon_compose.svg",
               "icon_term": "assets/icon_term.svg"}

# The queued chip, from scene 08 (`queued_row`). Card #16c: it sits ABOVE the
# input (atlas: the chip row is over the composer, not under the controls).
QUEUED = {"t": "stack", "id": "queued_row", "x": 10, "y": 10, "w": 240, "h": 50,
          "variant": "surface", "bg": 4294440952, "radius": 12, "c": [
              {"t": "text", "id": "q1", "x": 16, "y": 13, "w": 208, "h": 24, "size": 15,
               "weight": 500, "font_src": INTER5, "line_height": 18, "color": 4280953387,
               "variant": "single_line", "text": "1 queued \u00b7 Steer now \u00b7 \u2715"}]}

# Card #18d (外环补充): the two per-file stats (+N green / -N red) are adjacent
# TEXT runs. `alignx` on a text node only aligns the run INSIDE its own box and
# `fillw` makes each box full-width, so two of them overlapped (measured 0..540).
# The #16c wrapper (`alignx==1 && kind != Text`) does not apply to text either.
# The mechanism that DOES render text side by side is a flow ROW (goal-strip), so
# group the pair into a fill-width row with a leading fill-spacer that pushes them
# to the right edge, and a small trailing spacer holding the atlas inset.
ROW_WRAP = {
    "edited-files-card": [
        ("files_card", "stats_1", ["file_1_add", "file_1_del"], 13.5),
        ("files_card", "stats_2", ["file_2_add", "file_2_del"], 13.5),
        ("files_card", "stats_3", ["file_3_add", "file_3_del"], 13.5),
    ],
    # the model value + its disclosure chevron as one right-anchored cluster
    # (atlas: value 191..321, chevron 329..336.5 in the 348 card).
    "settings-group": [
        ("model_card", "model_row", ["t_pick", "pick_chev"], 15.0),
    ],
}


VARIANTS = {   'thread-row': {   'short': {   'text': {'thread_1_label': 'Add session fork'},
                                   'flags': {'thread_1_surface': {'bg': 4294835709}}},
                      'long': {   'text': {   'thread_1_label': 'Bump octos-core to a6ea8505 and '
                                                                're-verify the steer queue timeout'},
                                  'flags': {'thread_1_surface': {'bg': 4294046195}}}},
    'new-chat': {   'short': {   'text': {},
                                 'insert': [   (   'new_chat',
                                                   {   't': 'svg',
                                                       'id': 'icon_compose',
                                                       'x': 347,
                                                       'y': 10,
                                                       'w': 24,
                                                       'h': 28,
                                                       'alignx': 1,
                                                       'src': ''})]},
                    'long': {   'text': {},
                                'insert': [   (   'new_chat',
                                                  {   't': 'svg',
                                                      'id': 'icon_compose',
                                                      'x': 347,
                                                      'y': 10,
                                                      'w': 24,
                                                      'h': 28,
                                                      'alignx': 1,
                                                      'src': ''})]}},
    'user-bubble': {   'short': {   'text': {'t01': 'Retry the build'},
                                    'drop': ['t02'],
                                    'flags': {'user_bubble': {'w': 135.0, 'h': 50.2}}},
                       'long': {   'text': {'t01': 'Fix the steer queue so queued steers '
                                                   'survive a reconnect'},
                                   'drop': ['t02'],
                                   'flags': {   'user_bubble': {'h': 85.09},
                                                't01': {   'variant': 'wrap', 'w': 252.6,
                                                           'h': 63.0, 'line_height': 38.0}}}},
    'working-row': {   'short': {'text': {'t03': 'Working • 3s'}},
                       'long': {'text': {'t03': 'Working • 12s'}}},
    'assistant-prose': {   'short': {'text': {'answer_md': 'Fixed `steer_dropped` handling.'}},
                           'long': {   'text': {   'answer_md': 'Queued steers now survive a '
                                                                'reconnect.\n'
                                                                '\n'
                                                                '• Fixed loss of queued steers when '
                                                                'reconnecting after a drop in '
                                                                '`steer_dropped` handling.\n'
                                                                '\n'
                                                                '• Updated `ui_protocol_transport.rs` '
                                                                'to persist queued steers to the '
                                                                'session ledger.\n'
                                                                '\n'
                                                                '• All tests pass: `12 passed`.\n'
                                                                '\n'
                                                                '• Changes included in commit '
                                                                '`a6ea8505`.'}}},
    'worked-for': {   'short': {'text': {'worked_row_label': 'Worked for 3s ›'}},
                      'long': {'text': {'worked_row_label': 'Worked for 3m 4s ›'}}},
    'answer-actions': {'short': {'text': {'t11': 'now'}}, 'long': {'text': {'t11': 'Sep 28, 9:41 PM'}}},
    'tool-cell': {   'short': {'text': {'t01': 'Read ui_protocol_transport.rs', 't02': '• 412 lines'}},
                     'long': {   'text': {'t01': 'Ran cargo test -p octos-cli', 't02': '• 12 passed'},
                                 'flags': {'tool_1': {'h': 210, 'fith': 0}},
                                 'drop': ['icon_file'],
                                 'insert': [   (   'tool_1',
                                                   {   't': 'svg',
                                                       'id': 'icon_term',
                                                       'x': 25.791,
                                                       'y': 27.391,
                                                       'w': 23.807,
                                                       'h': 29.674,
                                                       'src': ''}),
                                               (   'tool_1',
                                                   {   't': 'stack',
                                                       'id': 'tool_1_output',
                                                       'x': 10,
                                                       'y': 84,
                                                       'w': 351,
                                                       'h': 116,
                                                       'variant': 'surface',
                                                       'bg': 4294375159,
                                                       'radius': 8,
                                                       'c': [   {   't': 'text',
                                                                    'id': 'o1',
                                                                    'x': 18,
                                                                    'y': 10,
                                                                    'w': 316,
                                                                    'h': 17,
                                                                    'size': 13.4,
                                                                    'weight': 400,
                                                                    'font_src': 'self:resources/ux/LiberationMono-Regular.ttf',
                                                                    'line_height': 16.6,
                                                                    'color': 4281216815,
                                                                    'variant': 'single_line',
                                                                    'text': 'running 12 tests'},
                                                                {   't': 'text',
                                                                    'id': 'o2',
                                                                    'x': 18,
                                                                    'y': 38,
                                                                    'w': 316,
                                                                    'h': 17,
                                                                    'size': 13.4,
                                                                    'weight': 400,
                                                                    'font_src': 'self:resources/ux/LiberationMono-Regular.ttf',
                                                                    'line_height': 16.6,
                                                                    'color': 4281479731,
                                                                    'variant': 'single_line',
                                                                    'text': 'test '
                                                                            'steer_queue::reconnect_ok '
                                                                            '... ok'},
                                                                {   't': 'text',
                                                                    'id': 'o3',
                                                                    'x': 18,
                                                                    'y': 66,
                                                                    'w': 316,
                                                                    'h': 17,
                                                                    'size': 13.4,
                                                                    'weight': 500,
                                                                    'font_src': 'self:resources/ux/LiberationMono-Regular.ttf',
                                                                    'line_height': 16.6,
                                                                    'color': 4281362226,
                                                                    'variant': 'single_line',
                                                                    'text': 'test result: ok. 12 '
                                                                            'passed; 0 failed'},
                                                                {   't': 'text',
                                                                    'id': 'o4',
                                                                    'x': 18,
                                                                    'y': 92,
                                                                    'w': 316,
                                                                    'h': 17,
                                                                    'size': 13.4,
                                                                    'weight': 400,
                                                                    'font_src': 'self:resources/ux/LiberationMono-Regular.ttf',
                                                                    'line_height': 16.6,
                                                                    'color': 4281216815,
                                                                    'variant': 'single_line',
                                                                    'text': 'Finished in 0.42s'}]})]},
                     'failed': {   'text': {   't01': 'Ran cargo test -p octos-cli',
                                               't02': '• exit 2 · 0 passed, 2 failed'},
                                   'flags': {   'icon_check1': {'w': 0, 'h': 0},
                                                't02': {'color': 4291764782}},
                                   'drop': ['icon_file'],
                                   'insert': [   (   'tool_1',
                                                     {   't': 'svg',
                                                         'id': 'icon_term',
                                                         'x': 25.791,
                                                         'y': 27.391,
                                                         'w': 23.807,
                                                         'h': 29.674,
                                                         'src': ''}),
                                                 (   'tool_1',
                                                     {   't': 'text',
                                                         'id': 'status_x',
                                                         'x': 336,
                                                         'y': 25,
                                                         'w': 20,
                                                         'h': 23,
                                                         'size': 17,
                                                         'weight': 700,
                                                         'font_src': 'self:resources/ux/Inter-700.ttf',
                                                         'line_height': 21,
                                                         'color': 4291764782,
                                                         'variant': 'single_line',
                                                         'text': '✕'})]}},
    'composer': {   'short': {'text': {}},
                    'long': {   'text': {'composer_idle_input': 'also add a test for reconnect'},
                                'flags': {   'composer_idle': {'h': 250, 'fith': 0},
                                             'composer_idle_input': {'y': 68.556},
                                             'icon_plus1': {'y': 186.778},
                                             'pill1': {'y': 179.0},
                                             'pill1_t': {'y': 193.5},
                                             'icon_mic1': {'y': 183.611},
                                             't04': {'y': 196.5},
                                             'send1': {'y': 176.222},
                                             'icon_send': {'y': 186.778}},
                                'insert': [   (   'composer_idle',
                                                  {   't': 'stack',
                                                      'id': 'queued_row',
                                                      'x': 10,
                                                      'y': 10,
                                                      'w': 240,
                                                      'h': 50,
                                                      'variant': 'surface',
                                                      'bg': 4294440952,
                                                      'radius': 12,
                                                      'c': [   {   't': 'text',
                                                                   'id': 'q1',
                                                                   'x': 16,
                                                                   'y': 13,
                                                                   'w': 208,
                                                                   'h': 24,
                                                                   'size': 15,
                                                                   'weight': 500,
                                                                   'font_src': 'self:resources/ux/Inter-500.ttf',
                                                                   'line_height': 18,
                                                                   'color': 4280953387,
                                                                   'variant': 'single_line',
                                                                   'text': '1 queued · Steer now · '
                                                                           '✕'}]})]}},
    'approval-card': {   'short': {   'text': {   't02': 'git push origin feat/steer-queue',
                                                  'reason_text': 'Reason: Push the fix branch so CI '
                                                                 'can run'}},
                         'long': {   'text': {   't02': 'cargo test -p octos-cli steer_queue -- '
                                                        '--nocapture',
                                                 'reason_text': 'Reason: Run the full steer-queue '
                                                                'integration suite before pushing so a '
                                                                'regression in the durable queue is '
                                                                'caught locally rather than in CI.'},
                                     'flags': {   'approval_card': {'h': 704},
                                                  't02': {'variant': None, 'h': 56},
                                                  'cmd_box': {'h': 112},
                                                  'reason_text': {'y': 210, 'h': 130},
                                                  'approve_once': {'y': 352},
                                                  'approve_session': {'y': 435},
                                                  'deny': {'y': 518},
                                                  't_hint': {'y': 618}}}},
    'question-card': {   'short': {   'text': {   'question_text': 'Where should queued steers be '
                                                                   'persisted?'}},
                         'long': {   'text': {   'question_text': 'Where should queued steers be '
                                                                  'persisted so they survive both a '
                                                                  'reconnect and an app restart '
                                                                  'without losing ordering?'},
                                     'flags': {   'question_card': {'h': 720},
                                                  'question_text': {'h': 110}}}},
    'edited-files-card': {   'short': {'text': {'t01': 'Edited 1 file', 't02': '+12 -2'}},
                             'long': {'text': {'t01': 'Edited 3 files', 't02': '+62 -5'}}},
    'plan-card': {   'short': {   'text': {   't02': 'Plan · 1 of 2',
                                              'step_0_label': 'Reproduce reconnect drop',
                                              'step_1_label': 'Implement durable queue'},
                                  'drop': [   'step_2_label',
                                              'step_3_label',
                                              'step_4_label',
                                              'icon_step2',
                                              'icon_step3',
                                              'icon_step4'],
                                  'flags': {'plan_card': {'h': 300}}},
                     'long': {'text': {'t02': 'Plan · 3 of 5'}}},
    'goal-strip': {   'short': {'text': {'t01': 'Goal · Fix steer queue · 2m'}},
                      'long': {'text': {'t01': 'Goal · Fix steer queue on reconnect · 18m'}}},
    'diff-view': {   'short': {   'text': {   't_file': 'ui_protocol.rs',
                                              't_fadd': '+9',
                                              't_fdel': '-1',
                                              't_fold': '⋮ 88 unmodified lines ⋮'}},
                     'long': {   'text': {   't_file': 'ui_protocol_transport.rs',
                                             't_fadd': '+31',
                                             't_fdel': '-4',
                                             't_fold': '⋮ 412 unmodified lines ⋮'}}},
    'settings-group': {   'short': {'text': {'t01': 'Permissions'}},
                          'long': {'text': {'t01': 'Permissions and defaults'}}}}

ROLE_BY_KIND = {"stack": "layout", "text": "text", "svg": "icon", "button": "button",
                "input": "input"}


def _sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


def walk(node):
    yield node
    for c in node.get("c", []):
        yield from walk(c)


def absolutize(node, ox, oy):
    """Rewrite a subtree's box-relative coords to root-absolute, in place.

    The measured backend emits every node's own x/y as `abs_pos` (design.rs),
    which makepad resolves against the WINDOW, not the parent. An inserted
    subtree authored with coords relative to its own box would therefore paint
    at the box's origin: the console's four lines all landed at y=10/38/66 (OCR)
    instead of 94/122/… Add each ancestor's offset on the way down.
    """
    node["x"] = node.get("x", 0.0) + ox
    node["y"] = node.get("y", 0.0) + oy
    for c in node.get("c", []):
        absolutize(c, node["x"], node["y"])


def wait_port(port, timeout=40):
    for _ in range(int(timeout * 4)):
        try:
            with socket.create_connection(("127.0.0.1", port), 0.5):
                return True
        except OSError:
            time.sleep(0.25)
    return False


def relativize(tree):
    """Rebase exactly the nodes `design.rs` positions with a MARGIN.

    `design.rs` wraps a node (and insets the wrapper with `margin`) in three cases
    only: a `fillw` stack, a `fillw` left-aligned single-line text, and a
    right-anchored (`alignx: 1`) node that is neither text nor input. For those the
    source `x`/`y` must be the offset FROM the immediate parent, and the right inset
    is the node's authored right gap (`parent_w - x - w`), which keeps its box at
    `x .. x+authored_w` at any parent width (card #18c/#18e).

    Every OTHER node is pinned with `abs_pos`, and makepad `abs_pos` is
    window-absolute, so its authored coordinates must stay ABSOLUTE. Rebasing one
    moved it to the card origin: card #18f, the question-card note `input` is flagged
    `fillw` but `design.rs` never wraps an `Input`, so the shift put the placeholder
    "Add a note" at the card's top-left over the `?` icon and title.
    """
    def wrapped(c):
        kind = c.get("t")
        fillw = c.get("fillw") == 1
        alignx1 = c.get("alignx") == 1
        if kind == "text":
            return fillw and c.get("alignx", 0) == 0 and c.get("variant") != "markdown"
        if kind == "input":
            return False
        if kind == "stack":
            return fillw or alignx1
        return alignx1

    def rec(n, nax, nay):
        pw = n.get("w")
        for c in n.get("c", []) or []:
            cx, cy = c.get("x", 0.0), c.get("y", 0.0)
            if wrapped(c):
                if cx is not None:
                    c["x"] = round(cx - nax, 2)
                if cy is not None:
                    c["y"] = round(cy - nay, 2)
                if pw is not None and c.get("w") is not None:
                    c["padright"] = round(pw - ((cx - nax) + c["w"]), 2)
            # Recurse with the child's OWN authored absolute origin: in both the
            # wrapped and the abs_pos case makepad insets a child within its
            # immediate parent's box, so a wrapped grandchild is relative to `c`.
            rec(c, cx, cy)

    rec(tree, tree.get("x") or 0.0, tree.get("y") or 0.0)
    return tree


def right_edge_ok(png_path):
    """Card #18c acceptance check: the rightmost 4px column of a variant must be
    background (near-white page ground, or the host's #4c4c4c), i.e. no card
    border, radius or tint may be cut off at the render's right edge."""
    a = np.asarray(Image.open(png_path).convert("RGB")).astype(int)
    strip = a[:, -4:, :]
    white = bool((strip.min(axis=2) > 238).all())
    ground = bool((np.abs(strip - 76).max(axis=2) <= 8).all())
    top = Counter(map(tuple, strip.reshape(-1, 3))).most_common(1)[0][0]
    return white or ground, [int(v) for v in top]


def apply_variant(tree, comp, variant):
    spec = VARIANTS[comp][variant]
    # Merge per NODE ID, not per top-level key: otherwise a variant flag like
    # {"tool_1": {"h": 190}} would *replace* the responsive {"fillw":1,"fith":1}
    # for that node, and the root would pin its atlas width again.
    flags = {k: dict(v) for k, v in RESPONSIVE[comp].items()}
    for nid, fl in spec.get("flags", {}).items():
        flags.setdefault(nid, {}).update(fl)
    inserts = spec.get("insert", [])
    drops = set(spec.get("drop", []))
    # Card #18c item 4 (kept across the #16 merge): a positional flag moves a
    # node AND its subtree. Without this a moved KitButton left its
    # `_surface`/`_label` at the old y and the button rendered detached.
    moved = []
    for n in walk(tree):
        if n["id"] in spec.get("text", {}):
            n["text"] = spec["text"][n["id"]]
        if n["id"] in flags:
            before = (n.get("x"), n.get("y"))
            n.update(flags[n["id"]])
            if (n.get("x"), n.get("y")) != before:
                moved.append((n, before[0], before[1]))
    for n, bx, by in moved:
        dx = (n.get("x") or 0) - (bx or 0)
        dy = (n.get("y") or 0) - (by or 0)
        for c in walk(n):
            if c is n:
                continue
            if c.get("x") is not None:
                c["x"] = round(c["x"] + dx, 2)
            if c.get("y") is not None:
                c["y"] = round(c["y"] + dy, 2)
    # Card #18d (外环补充): the #16c right-anchor wrapper emits
    # `margin: Inset{top: a.y}`, where `a.y` must be PARENT-relative. The measure
    # stage writes window-absolute y, so a right-anchored node nested under a
    # parent at y>0 double-counts (the settings toggles inside `perm_card` at y=66
    # landed at 168 = 66+102). Rebase each `alignx` node's y on its parent's y; a
    # node whose parent is the root (y=0) is unchanged.
    def _rebase(node):
        for c in node.get("c", []) or []:
            if c.get("alignx") == 1 and c.get("y") is not None:
                # y feeds the #16c wrapper's `margin: top`, x feeds the child's
                # right inset (`gap = parent_w - x - w`). Both must be
                # PARENT-relative, but the measure stage writes them absolute, so
                # a node nested under a parent at (px, py) lands at y+py and its
                # gap is short by px (settings toggles: gap 7 -> flush at 540).
                #
                # Card #18e: `design.rs` measures a right-anchored node's CHILDREN
                # from the node's own box, so the whole SUBTREE must move with it.
                # Rebasing only the node left its knob at the old absolute
                # coordinate, so design.rs emitted `margin: Inset{left: 35.4 top:
                # 70.6}` inside a 55x46 toggle -> the knob resolved to 0x0 and the
                # toggle rendered as a bare pill (settings-group, both cards).
                dx = -(node.get("x") or 0.0)
                dy = -(node.get("y") or 0.0)
                for m in walk(c):
                    if m.get("x") is not None:
                        m["x"] = round(m["x"] + dx, 2)
                    if m.get("y") is not None:
                        m["y"] = round(m["y"] + dy, 2)
            else:
                _rebase(c)
    _rebase(tree)
    # Card #18d: group the flagged adjacent text leaves into one fill-width flow
    # row; a leading fill-spacer pushes the pair to the right, a trailing spacer
    # holds the atlas inset.
    for parent_id, row_id, kid_ids, inset in ROW_WRAP.get(comp, []):
        parent = next((n for n in walk(tree) if n["id"] == parent_id), None)
        kids = [n for n in walk(tree) if n["id"] in kid_ids]
        if parent is None or len(kids) != len(kid_ids):
            continue
        y0 = min(k["y"] for k in kids)
        h = max(k["h"] for k in kids)
        row = {"t": "stack", "id": row_id, "x": 0.0, "y": y0, "w": parent.get("w", 0.0),
               "h": h, "fillw": 1, "variant": "row", "c": []}
        row["c"].append({"t": "stack", "id": row_id + "_spacer", "x": 0.0,
                         "y": 0.0, "w": 8.0, "h": h, "fillw": 1})
        for k in kids:
            k["x"] = 0.0
            k["y"] = 0.0
            row["c"].append(k)
        if inset:
            row["c"].append({"t": "stack", "id": row_id + "_pad", "x": 0.0,
                             "y": 0.0, "w": inset, "h": h})
        parent["c"] = [c for c in parent.get("c", []) if c["id"] not in kid_ids]
        parent.setdefault("c", []).append(row)
    if drops:
        def prune(node):
            node["c"] = [c for c in node.get("c", []) if c["id"] not in drops]
            for c in node["c"]:
                prune(c)
        prune(tree)
    for parent_id, node in inserts:
        parent = next(n for n in walk(tree) if n["id"] == parent_id)
        node = json.loads(json.dumps(node))
        # The measured backend emits a node's own x/y as ROOT-absolute
        # `abs_pos` (design.rs), so a subtree written with coords relative to
        # its own box would paint at the box's origin, not inside it — the
        # inserted console's lines all stacked at y=10/38/66 (OCR). Convert the
        # insert to absolute, exactly like the v6 tree it is copied from
        # (scene 04: nested `tool_3_output` at y=398, its line `t07` at 442.86).
        absolutize(node, parent.get("x", 0.0), parent.get("y", 0.0))
        parent.setdefault("c", []).append(node)
    # Card #18e: restore the root gutter the #18 merge dropped. The atlas crop is
    # card-tight, so the extracted root sits at x=0; a `Fill` root (every card
    # here is `fillw`) then runs edge-to-edge and its right border/radius lands on
    # the window's last pixel. Rebase the fill children on their parents, then
    # give the root the page gutter the atlas normalised away so all four rounded
    # corners stay inside the render at every width.
    relativize(tree)
    tree["x"] = round(tree.get("x", 0) + ROOT_GUTTER_X, 2)
    tree["y"] = round(tree.get("y", 0) + ROOT_GUTTER_Y, 2)
    tree["padright"] = float(ROOT_GUTTER_X)   # the root is gutted on BOTH sides
    return tree, inserts, drops


def build_workspace(comp, variant):
    dest = WORK / f"{comp}-{variant}"
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(COMPONENTS / comp, dest)
    for stale in ("page.card", "page.data.json", "kit", "semantic-preflight.json",
                  "semantic-audit.json", "semantic-repair.json", "mapping.json",
                  "semantic-state.json"):
        p = dest / stale
        if p.is_dir():
            shutil.rmtree(p)
        elif p.exists():
            p.unlink()
    mapped = json.loads((dest / "mapped.json").read_text())
    tree, inserts, drops = apply_variant(mapped["tree"], comp, variant)
    mapped["tree"] = tree
    (dest / "mapped.json").write_text(json.dumps(mapped, indent=2, ensure_ascii=False) + "\n")
    semantic = json.loads((dest / "semantic-map.json").read_text())
    known = {e["id"] for e in semantic["elements"]}
    # A dropped node must leave the semantic map too, or preflight reports it as
    # "absent from the composition".
    semantic["elements"] = [e for e in semantic["elements"] if e["id"] not in drops]
    # Card #18d: register the row + its spacers so preflight stays green.
    for _pid, row_id, _kids, _inset in ROW_WRAP.get(comp, []):
        for rid in (row_id, row_id + "_spacer", row_id + "_pad"):
            if rid in known:
                continue
            semantic["elements"].append({
                "id": rid, "role": "layout",
                "basis": "authored stack/text node (card #18d right-anchor row)",
                "confidence": 1.0, "decision": "declared"})
            known.add(rid)
    for parent_id, node in inserts:
        for n in walk(node):
            if n["id"] in known:
                continue
            role = ROLE_BY_KIND.get(n["t"], "layout")
            entry = {"id": n["id"], "role": role,
                     "basis": f"authored {n['t']} node (card #16c variant)",
                     "confidence": 1.0, "decision": "declared"}
            # Card #16c: `compile_page` reads `asset.path` for EVERY svg node, and
            # preflight requires an `icon` role to carry verified artwork
            # provenance — so an inserted glyph must declare its SVG (copied into
            # the component's assets/ beside the other icons).
            if n["t"] == "svg":
                path = ICON_ASSETS.get(n["id"])
                if not path:
                    raise SystemExit(f"inserted svg {n['id']} needs an entry in ICON_ASSETS")
                asset = Path("design/components") / comp / path
                entry["asset"] = {"path": path, "sha256": _sha(asset), "method": "reference_svg",
                                  "reference_sha256": _sha(COMPONENTS / comp / "reference.png"),
                                  "fit": "stretch", "clip": True,
                                  "notes": "Source glyph re-attached from the owning scene (card #16c)"}
            semantic["elements"].append(entry)
            known.add(n["id"])
    semantic.pop("contract_sha256", None)
    semantic.pop("reference_sha256", None)
    (dest / "semantic-map.json").write_text(json.dumps(semantic, indent=2) + "\n")
    return dest


def compile_component(dest, comp):
    # `compile.py`/`semantics.py` live in `flows/image-lib`; `flow.py` (its
    # `sha`/`local` helpers) lives in `flows/image-to-card`; both import
    # `core.native_paths` from `flows/`.
    for p in (CLONE / "flows/image-lib", CLONE / "flows/image-to-card", CLONE / "flows"):
        sys.path.insert(0, str(p))
    from compile import compile_page
    from flow import sha
    # `compile_page` re-derives the two sha bindings the semantic map must match.
    semantic = json.loads((dest / "semantic-map.json").read_text())
    semantic["contract_sha256"] = sha(dest / "contract.json")
    semantic["reference_sha256"] = sha(dest / "reference.png")
    (dest / "semantic-map.json").write_text(json.dumps(semantic, indent=2) + "\n")
    # The default artwork origin is 8170, which another lane's process owns; point
    # the compiled SVG `src` at OUR art server, or an icon node renders 0x0.
    result = compile_page(dest, artwork_origin=f"http://127.0.0.1:{ART_PORT}/ux-images")
    # Publish this workspace's compiled assets where that server serves them
    # (`published/ux-images/<id>/assets/...`); idempotent.
    pub = PUBLISHED / "ux-images" / comp / "assets"
    pub.mkdir(parents=True, exist_ok=True)
    for f in (dest / "assets").glob("*"):
        shutil.copy(f, pub / f.name)
    return result


def render(dest, port, w, h):
    request = {
        "card": str(dest / "page.card"), "data": str(dest / "page.data.json"),
        "kit_dir": str(dest / "kit"), "format": "l0-kit", "width": w, "height": h,
        "nonce": f"var-{dest.name}-{w}", "result": str(dest / "native.json"),
        "layout": str(dest / "layout.json"), "actions": str(dest / "actions.json"),
    }
    (dest / "request.json").write_text(json.dumps(request))
    with (dest / f"host-{w}.log").open("w") as log:
        env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1",
               "BEAUTY_REQUEST": str(dest / "request.json")}
        proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env,
                                stdout=log, stderr=subprocess.STDOUT)
        try:
            if not wait_port(port):
                return {"error": "no port"}
            time.sleep(6)
            snap = subprocess.check_output(
                ["curl", "-s", "--max-time", "10", f"127.0.0.1:{port}/snap?all=1"]).decode()
            (dest / f"snap-{w}.json").write_text(snap)
            grab = json.loads(subprocess.check_output(
                ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
            png = grab.get("png")
            if png and Path(png).is_file():
                shutil.copy(png, dest / f"native-{w}.png")
            subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                           capture_output=True)
            time.sleep(1)
        finally:
            if proc.poll() is None:
                proc.kill()
    text = (dest / f"host-{w}.log").read_text(errors="ignore")
    nodes = [(e["i"], e["ty"], e["r"]) for e in json.loads(snap)["s"] if e["i"].startswith("beauty")]
    return {"w": w, "nodes": nodes, "font_warnings": text.count("not available in this build")}


def assemble(comp, variant, panels):
    """atlas crop | native@360 | native@540, top-aligned, on white."""
    imgs = [Image.open(p).convert("RGB") for p in panels]
    gap, pad = 12, 16
    h = max(i.height for i in imgs)
    w = sum(i.width for i in imgs) + gap * (len(imgs) - 1) + pad * 2
    canvas = Image.new("RGB", (w, h + pad * 2), (255, 255, 255))
    x = pad
    for i in imgs:
        canvas.paste(i, (x, pad))
        x += i.width + gap
    out = COMPONENTS / comp / f"review-{variant}.png"
    canvas.save(out)
    return out


def main():
    only = sys.argv[1:]
    WORK.mkdir(parents=True, exist_ok=True)
    httpd = ThreadingHTTPServer(("127.0.0.1", ART_PORT), partial(Quiet, directory=str(PUBLISHED)))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    time.sleep(1)
    summary = {}
    try:
        for comp in sorted(VARIANTS):
            if only and comp not in only:
                continue
            summary[comp] = {}
            vdir = COMPONENTS / comp / "variants"
            vdir.mkdir(exist_ok=True)
            for vi, variant in enumerate(VARIANTS[comp]):
                dest = build_workspace(comp, variant)
                try:
                    compile_component(dest, comp)
                except Exception as exc:            # noqa: BLE001 - surface the real error
                    print(json.dumps({"component": comp, "variant": variant,
                                      "compile_error": str(exc)[:400]}))
                    continue
                renders = []
                for wi, w in enumerate(WIDTHS):
                    r = render(dest, PORTS[(vi * len(WIDTHS) + wi) % len(PORTS)], w, 640)
                    renders.append(r)
                    if r.get("nodes") is not None:
                        shutil.copy(dest / f"native-{w}.png", vdir / f"{variant}-{w}.png")
                panels = [str(COMPONENTS / comp / "reference.png")]
                for w in WIDTHS:
                    p = vdir / f"{variant}-{w}.png"
                    panels.append(str(p))
                # Card #18e acceptance: the rightmost 4px of EVERY variant must be
                # background (no border/radius/tint cut off at the render edge).
                edge = {}
                for w in WIDTHS:
                    p = vdir / f"{variant}-{w}.png"
                    if p.is_file():
                        ok_e, col = right_edge_ok(p)
                        edge[w] = {"ok": ok_e, "colour": col}
                edge_ok = bool(edge) and all(v["ok"] for v in edge.values())
                ok = all(Path(p).is_file() for p in panels) and edge_ok
                review = str(assemble(comp, variant, panels)) if ok else None
                summary[comp][variant] = {
                    "render": str(Path(review).relative_to(ROOT)) if review else None,
                    "widths": list(WIDTHS),
                    "right_edge": edge,
                    "nodes": {f"w{r['w']}": r.get("nodes") for r in renders},
                    "font_warnings": sum(r.get("font_warnings", 0) for r in renders),
                }
                print(json.dumps({"component": comp, "variant": variant, "review": review,
                                  "ok": ok}, ensure_ascii=False))
    finally:
        httpd.shutdown()
    (WORK / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
