# Stage B · phase 4 · board 5: the Memory cards

Board 5 (`design/stage-a/phase4-new5/`, signed off 2026-10-03: D1 yes with two fixes, D2 A, D3 "Capabilities",
D4 follow the app theme) drawn in the app, one card per frame, each judged against its crop of the atlas.

## What a card is here

Board 5's frames are states of ONE dialog whose content the server fills at runtime (lists of entities, hits, notes)
in a desktop window shorter than the 406 × 776 artboard and on a 360 px phone. Since A4 the board-3 dialogs are built
with the board-3 **dialog kit** (`crates/octoscode-module/src/screens/board3/ui.rs`, "Why flow, not the measured
card") instead of measured L0 cards, and Memory is a board-3 dialog (`board3/memory.rs`, `Dialog::Memory`). So each
card is that kit surface in the frame's state, **rendered by the real app** and **reached by CLICK** from
Settings > Capabilities > Memory, against `replay_serve --scenario memory` (faithful replies, no model) — the same
render the shipped build draws, not a parallel mock-up.

Per card, `cards/phase4n5-NN/`:

- `reference.png` — the frame cut from `atlas.png` with the box in board 5's README;
- `card.json` — the frame, the state, how it is reached, the native ids, the methods and the evidence list.

Per capture, `evidence/gate-b/`:

- `phase4n5-NN-native-<window>-<tag>.png` — the app's capture (desktop: the OctosCode window; phone: the 360 × 780
  frame in the phone shell);
- `phase4n5-NN-review-<window>-<tag>.png` — the reference beside it at one height;
- `phase4n5-NN-snap-<window>-<tag>.json` — its `/snap` (rects, texts; scrubbed).

`tools/cards.py` builds all of it from the walks' output (`tools/walk/a36_memory.py`, `tools/walk/a36_capabilities.py`).

## The cards

| Card | Frame | State | Desktop | Phone |
|---|---|---|---|---|
| phase4n5-01 | 1 | Settings > Capabilities: Skills, MCP servers, Memory | ✓ | (03) |
| phase4n5-02 | 2 | the overview | ✓ | (04) |
| phase4n5-03 | 3 | Settings > Capabilities, the rail chip | | ✓ |
| phase4n5-04 | 4 | the overview, full-screen sheet | (02) | ✓ |
| phase4n5-05 | 5 | search results, the kind filter, the untrusted chip | ✓ | ✓ |
| phase4n5-06 | 6 | a hit opened: untrusted content as data, the callout | ✓ | ✓ |
| phase4n5-07 | 7 | an entity page | ✓ | ✓ |
| phase4n5-08 | 8 | long-term memory; the server cut it: the notice | ✓ | ✓ |
| phase4n5-09 | 9 | Add a note; the receipt | ✓ | ✓ |
| phase4n5-10a | 10a | an empty profile | ✓ | ✓ |
| phase4n5-10b | 10b | loading | ✓ | ✓ |
| phase4n5-10c | 10c | refused — D1's bounded problem + next step | ✓ | ✓ |
| phase4n5-11 | 11 | dark (D4) | ✓ | ✓ |
| phase4n5-12 | 12 | 中文 (Noto Sans SC) | ✓ | ✓ |

Scores: `docs/ux-scores.csv`, area `a36` (every card ≥ 9, with the numeric `/snap` checks of its walk step).

## Where a card departs from its frame, and why

- **Frame 2/4/11/12, the scope line** — "Server Profile: dsflash" in plain Inter, the muted ink (D1), not the board's
  monospace.
- **Frame 10c, the refusal** — the cause is a bounded sentence and a next step (D1), never the server's
  "No ProfileRuntime registered for profile 'admin'". Today's octos answers memory for the signed-in account, not the
  Session's profile (D2 A), so the live server shows exactly this card (`docs/ux/a36/live/`).
- **Frame 8, the truncation notice** leads the page instead of closing it: a cut MEMORY.md is 96 KB, and a notice
  after it sat a thousand lines down (the walk could not reach it).
- **Frame 2, the entities' order** — the server sorts them by name (`dsflash`, `octos-core`, `steer-queue`); the
  board listed them unsorted.
- **Frames 5/6, the chips** — the kit's neutral chip has no border (board README errata); the Episode hit carries
  the untrusted chip (the server marks mirrored episodes untrusted; errata).
- **Frame 2, the desktop dialog** is `min(640, 100%) × min(720, 100%)` at one size for every page (the board drew
  it about 580 px wide at full height): moving between the overview, the results and a record never resizes it.
- **Frame 4, "Add note" in the empty state** (10a) is kept: writing is allowed whenever the profile is confirmed.
- **Recent notes** (README, approved with the board, not drawn) use the Entities row style with the day in place of
  the mono name; a row opens the day's note.
