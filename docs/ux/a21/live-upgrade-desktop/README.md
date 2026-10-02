# A21 follow-up — A19's live `upgrade` + `restore` on main, through the A21 bootstrap (desktop)

Build: main `998d5d0f` (host built from `agent-a3f18ae08e6f31aed@998d5d0f`, makepad patches applied: the bridge
requires its per-launch token). Window: desktop 990x603 in the 1400x900 shell.

## Setup (the A19b recipe)
- A COPY of the live gate's `data`, `inst` and `ws` in the worktree's `tmp/` (never committed, deleted after the
  run), served by a private `octos serve --solo` on 127.0.0.1:8493 (working directory = the copy, a fresh random
  token handed over by env, never argv).
- App state = the previous build's: A1's `last-server` = that server + its token, NOTHING in `connection-v1.json`,
  no auto-connect marker.
- The PRODUCT launch path: no `OCTOS_BEARER`, no `OCTOS_PROFILE_ID`, no `OCTOS_BASE_URL`, no `OCTOS_WORKSPACE_CWD`.
  The app dials only because A21's bootstrap restores the remembered server (`app-log.txt`:
  `bootstrap: Restore(http://127.0.0.1:8493, token: true) (… restore target: false, auto-connect: None)`), and
  A19's one-time migration runs on that restore (`startup: Migrated("dsflash:main")`).

## Results
| phase | checks | A19b |
|---|---|---|
| upgrade (first launch after the upgrade) | **13/13** (`walk.log`) | 13/13 |
| restore (relaunch, same app state) | **6/6** (`relaunch/walk.log`) | 6/6 |

- Never the empty welcome (sampled from the first frame: 2 frames, 0 welcome, 0 loading).
- Lands in `dsflash:main` in its folder (`…/upgrade/ws`, found by the per-workspace catalog read); no
  `launch/resolve`, nothing created, no history read refused; the profile resolved before the socket.
- The WHOLE history by scrolling: all 9 distinct prompts, 121 transcript texts (A19b: 9 / 121).
- ONE dsflash turn: "In one short sentence: say hello after the A21 bootstrap." — `turn/start` x1, `response`
  progress, `stream_end`, the composer idle again (`03-next-prompt-desktop.png`).
- Relaunch: `bootstrap: Restore(… restore target: true, auto-connect: Some(true))` -> `startup:
  Restored("dsflash:main")`; no `launch/resolve`; the remembered Session opened with its profile and folder; the new
  prompt's bubble on screen; all 10 distinct prompts by scrolling (125 texts; A19b 10 / 124).

## Differences from A19b
- The launch path: A19b launched with the harness env (`OCTOS_BASE_URL` + `OCTOS_BEARER`); this run has none — the
  bootstrap's restore of A1's remembered server is what dials.
- The first run crashed at the scroll-read: the instrument answered a coalesced scroll input (`/m?…&wait=1`) with
  404, and `tools/walk/a10_lib.py`'s `get` raised. Fixed there with the rule `walk_env.App.get` and the judge tour
  (4c3e51e8) already apply: a 404 on an input route is a delivered input. The counts above are the re-run on a fresh
  copy.

## Files
`walk.log`, `trace-upgrade-desktop.jsonl` (the app's protocol trace, machine paths scrubbed), `app-log.txt` (the
app's launch decisions), `ux-checks.txt`, captures + snaps (no field values; the header's folder painted over);
`relaunch/` the same for the restore. Scanned before commit: no machine path, no bridge token, and none of the run's
secrets (the serve token, the live gate's token, every key-like value of the copied data — whole, first 8, last 8).
