# A21 follow-up — A19's live `upgrade` + `restore` on main, through the A21 bootstrap (phone 360x780)

Build: main `998d5d0f` (host built from `agent-a3f18ae08e6f31aed@998d5d0f`, the bridge requires its per-launch
token). Window: phone 360x780 (`--test-action page:0 --test-action launch-octoscode`).

Same setup as `../live-upgrade-desktop/README.md` (a copy of the live gate's data/inst/ws in `tmp/`, a private
`octos serve --solo` on 8493, the previous build's app state, the PRODUCT launch path with no harness env), run
after the desktop pass on the same server copy — so the Session now holds the desktop run's turn, as in A19b.

## Results
| phase | checks | A19b |
|---|---|---|
| upgrade (first launch after the upgrade, history only — no turn, as A19b's phone run) | **9/9** (`walk.log`) | 9/9 |
| restore (relaunch, same app state) | **6/6** (`relaunch/walk.log`) | — (A19b relaunched on the desktop only) |

- `app-log.txt`: `bootstrap: Restore(http://127.0.0.1:8493, token: true) (… restore target: false, auto-connect:
  None)` -> the migration -> `startup: Migrated("dsflash:main")`; never the empty welcome; `dsflash:main` reopened in
  its folder; no `launch/resolve`, nothing created, no history read refused; all 10 distinct prompts by scrolling
  (119 transcript texts; A19b's phone run: 10 / 118). Sampled from the first frame: 1 frame, 0 welcome, 0 loading.
- Relaunch (`relaunch/app-log.txt`): `bootstrap: Restore(… restore target: true, auto-connect: Some(true))` ->
  `startup: Restored("dsflash:main")`; the desktop run's prompt bubble on screen; all 10 prompts by scrolling
  (119 texts).

Files and the pre-commit scan: as on the desktop.
