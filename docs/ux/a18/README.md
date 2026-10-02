# A18 — readable light-theme text; one "Turn stopped" per Stop

Evidence build: `HOST …/host-agent-aef3e8e3b40196174/target/debug/octosense built from agent-aef3e8e3b40196174@7e969e14`
(this branch merged with main `acce6587`). Baseline: the same sources at main `487c02fb` (before A18), built
into a separate host dir. Both measured by the same tool, against the same replay servers, at the shell's
desktop (1400x900, OctosCode 990x603) and phone (360x780) sizes.

## 1. Light-theme text contrast (walk results row 212)

The web's `e2e/theme.spec.ts:72-112` sets MANUAL light on a dark OS, runs a real turn, and expects axe's
color-contrast rule to report ZERO violations on the conversation and then on Settings. The web's light text
levels (`app/theme.css:79-91`) never go below ~5.5:1: secondary `#61666b`, tertiary `#5f646b`, caption `#646970`.

### How it was measured (end to end, on pixels)

`tools/judge/contrast_walk.py <host-bin> <desktop|phone> <port> <rport> <outdir> <light|dark> <flow>`:
`OCTOSCODE_THEME=light|dark` (the stored preference; the OS stays as it is), every surface reached by CLICKS
against a replay server (no model): `chrome` (the fresh chat, a completed turn, a stopped turn, the composer's
model / permission menus, the session pane, the sidebar menus, the phone drawer, every Settings section and the
Stop-server confirm), `seats` (A10's seat simulator: the model menu's provider groups and rows, the permission
menu, the session pane), `surfaces` / `approval` / `plan` (A6's r23 turns: reasoning rows, a tool row and
delivered files, the question card, the approval card, the plan).

`tools/judge/contrast.py` measures each visible text node of `/snap?all=1` on the app's own 2x framebuffer
(`/g?raw=1`): background = the rect's modal colour, text = the glyph core (the highest-contrast 4 % of the ink,
median), WCAG 2.x ratio. At 2x the cores read the tokens exactly (`#a1a1a6` on white measures 2.57, its nominal
value). An overlay counts only what appeared with it; Settings counts only its drawer's nodes; a row half under
the header is skipped. A disabled control's style is exempt, as axe skips disabled controls (see the guard).

Per-surface totals: `contrast-surfaces.csv` — after: 92 surfaces (light and dark, desktop and phone), 0 with a
text node below its threshold; before: 56 of the 86 baseline surfaces had at least one. Every element, before and
after: `contrast-elements.csv`. The row's own check (`tools/walk/run.py` `theme_light`, its own
estimator) agrees: before 4 nodes below 4.5 (`Model not reported` 2.57, `Permissions not reported` 2.57,
`Search chats` 3.22, `All` 4.46), after none (worst `Change` 4.57). Walk results row 212, run alone
(`WALK_ONLY_ROWS=212`, `walk-row-212.csv`): fail (`worst='All' 1.02:1 violations=[('Search chats', 4.39), ('All',
1.02)]`) -> pass (`widgets=29 worst='Change' 4.57:1 violations=[]`); `docs/walk/results*.csv` are left for the
integrator's next full run.

### Before -> after, light (element | before | after | background)

| Element (surface) | Before | After | Background |
|---|---|---|---|
| Status strip "Model not reported" / "Permissions not reported" | 2.57 `#a1a1a6` | 5.96 `#5f646b` | `#ffffff` |
| Sidebar "Search chats" placeholder | 3.26 `#8e8e93` (run.py: 3.22) | 5.80 `#61666b` | `#ffffff` |
| Sidebar inactive segment "All" | 4.46 `#6e6e73` | 5.10 `#61666b` | `#f0f0f2` |
| Settings > Model thinking segments "Off" / "High" | 4.46 `#6e6e73` | 5.10 `#61666b` | `#f0f0f2` |
| Composer model menu: "Model" title, provider groups, every model id | 2.57 `#a1a1a6` | 5.96 `#5f646b` | `#ffffff` |
| Composer permission menu: "Permission" title | 2.57 | 5.96 | `#ffffff` |
| Question card: "Choose one, or write your own", "Type another answer", "Choose an option to continue" | 2.57 | 5.96 | `#ffffff` |
| Approval card: "shell · command", "Y / S / N" | 2.57 | 5.96 | `#ffffff` |
| Plan: "Done" / "In progress" / "Pending" / "Updated 3d ago" | 2.57 | 5.96 | `#ffffff` |
| Settings > General: the Stop server confirm's label | 4.23 `#ffffff` on `#e5383b` | 5.24 on `#d1242f` | the red fill |
| Secondary text (notice body, header path, Settings help, "now", "Interrupted") | 5.07 `#6e6e73` | 5.80 `#61666b` | `#ffffff` |
| Board-3 links ("Expand all", "New session with…", "Stop turn · Esc") | 4.57 `#2f6feb` | 5.54 `#3564c6` | `#ffffff` |
| Shell links ("Change", "Advanced…") | 4.57 `#2f6feb` | 4.57 (unchanged) | `#ffffff` |

The one sub-4.5 text left in light is the session pane's / model menu's unavailable model ("GLM-4 Flash",
"This configured model is unavailable.") — a disabled option with no tap target, drawn in the disabled ink
(2.57; exempt as axe exempts a disabled control). Its reason line is the tertiary ink (5.96).

### Dark: nothing regresses (and the shell's own dark failures are fixed)

| Element (dark) | Before | After | Background |
|---|---|---|---|
| "Turn stopped" title on the transcript | 1.02 `#1d1d1f` (invisible) | 15.20 `#f5f5f7` | `#1c1f22` |
| Its body, the strip's caption | 3.26 `#6e6e73` | 5.77 `#98989d` | `#1c1f22` |
| Strip facts "Model not reported" | 2.57 (a WHITE strip box in dark) | 6.44 `#a1a1a6` | `#1c1f22` |
| Thinking block (a white card in dark) | card `#ffffff` | card `#1c1f22`, chevron `#98989d` (5.93) | — |
| Links "Expand all", "Stop turn · Esc", "Change", "Advanced…" | 3.62 `#2f6feb` | 6.23 `#679efe` | `#1c1f22` |
| "Stop server…" / "Forget server" | 3.16 / 2.73 | 5.97 `#ff6b6b` | `#1c1f22` |
| Sidebar "All", Settings segments | 4.85 `#98989d` | 4.85 (unchanged) | `#2c2c2e` |

Every dark surface of the walk: 0 text nodes below 4.5 after (`contrast-surfaces.csv`, rows `*-dark-*`).

### The guard (a checked rule, not a one-off)

`crates/octoscode-module/src/screens/theme.rs` `CONTRAST_PAIRS`: every TEXT ink against every fill it is drawn on
— the shell's roles and inks, fluid's conversation components, board 3 on the theme's surfaces (rows, strip,
conversation surfaces, A14's dialog kit; dark = their `TOKENS` twin) and board 3 / board 1 in their fixed light
palette. `contrast_tests` compute the WCAG 2.x ratio (sRGB linearisation, `(L1+0.05)/(L2+0.05)`) of each pair in
BOTH palettes (body 4.5, UI glyph 3.0), pin the replaced board values as failing, keep `EXEMPT_INKS` (the disabled
ink and the shell's disabled field / idle dot, each with its reason) out of the text set, keep `TOKENS` a dark
fixed point, and scan the sources: a board-3 `Txt` or chip naming a fill/accent token (`tok::BLUE`, `tok::GREEN`,
`tok::RED`, …) as its text ink, or an undeclared `draw_text` literal in chrome.rs, fails with the token to use.

### Decisions

- The light text levels are the web's: MUTED = secondary `#61666b`, FAINT = tertiary `#5f646b`, board 1's
  placeholder = caption `#646970`; the shell's `color_text_muted` = `#61666b` (the sidebar's search and rename
  placeholders take it). The web keeps no level below ~5.5:1 and draws secondary and tertiary as one level; the
  hierarchy it keeps — primary vs the rest — is kept (16.8:1 vs 5.8-6.0:1). The board's lighter faint grey
  (`#A1A1A6`) is given up for informational text; it stays as the DISABLED ink only.
- Accent TEXT has its own tokens (the web's `-text` values): `BLUE_TEXT #3564c6`, `GREEN_TEXT #166534`,
  `RED_TEXT #c50f0f`; `BLUE` / `GREEN` / `RED` stay the board's fills, dots, toggles and icons.
- Board 3's transcript rows and the status strip now follow the theme (`retint_dsl`), so darkening the light
  inks cannot regress dark; their icons take a dark ink there (`themed_icons`). Every new light value has a dark
  twin in `TOKENS`. The shell's link / destructive labels take spliced dark inks (`chrome::ink`, like its icons).

## 2. One "Turn stopped" per Stop

The live smoke's capture (`docs/ux/a15-live/smoke/06-stopped.png`) showed the stopped turn's notice and a second
identical one below it. The smoke's own trace (`docs/ux/a15-live/smoke/trace.jsonl`) says where the second came
from — not the same turn twice: the startup hydrate returned NO durable rows (the serve's session store had been
reset) while the replay window still held an earlier run's four completed turns and its stop (`01a0fc1d-1349…`,
"turn interrupted by client"). The history fold noted that stop (`01-connected.png` shows it before any turn);
on that build a notice-only turn drew below every turn, so one Stop showed two notices.

Fix (`flow.rs` `fold_history`): a retained terminal notes a turn of THIS transcript only. A completed turn always
persists its rows, so a completed terminal whose turn the transcript does not hold proves that part of the window
was discarded; a stop that follows it with no held turn in between is not noted. A held turn's notice and a stop
with nothing discarded before it (a session whose first turn was stopped) are restored as before — A15's restart
behaviour. The row model also draws at most one outcome notice per turn id (`rows.rs`).

Evidence:
- `crates/octoscode-module/tests/a18_stopped_notice.rs` (production path: the composer's submit, Stop's
  `turn/interrupt`, the open's hydrate, against a fake Core whose replay window outlives a reset store): one
  notice live, after a re-open and after a restart, in place; a live stop plus a re-hydrate stay one row.
  Fails on the baseline (the earlier run's stop is drawn), passes here.
- `tools/walk/a18_stop_walk.py` against `replay_serve --scenario a10 --stale-window` (the smoke's own first-launch
  hydrate, `a18-stale-window-a6ea8505`): CLICK a prompt, CLICK Stop.
  Baseline: desktop 5/9, phone 6/9 — a "Turn stopped" before any Stop, two after one Stop (`stop/before-*`).
  This branch: desktop 9/9, phone 9/9 — none before, exactly one, below the stopped prompt (`stop/after-*`).
- A6 surfaces walk: desktop 116/116, phone 119/119; A15 history walk: desktop 40/40, phone 37/37 (unchanged).

## Reproduce

```
python3 tools/judge/contrast_walk.py <host-bin> desktop <port> <rport> <outdir> light chrome   # seats | surfaces | approval | plan
python3 tools/judge/contrast.py files <snap.json> <full.png> [--before F] [--within ID] [--clip ID] [--exempt FG[:BG]]
python3 tools/walk/a18_stop_walk.py <host-bin> desktop <port> <rport> <outdir>
cargo test -p octoscode-module --lib contrast_tests; cargo test -p octoscode-module --test a18_stopped_notice
```
