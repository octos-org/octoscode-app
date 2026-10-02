# A20 live evidence (own octos serve a6ea8505 on :50301, dsflash, copied live-gate data, fresh instance dir; never the operator's serve)

`tools/judge/a20_live.py` on build ed0e42e1 (desktop), **39/39** (`checks.txt`), with the app's protocol trace `trace.jsonl`.
One model turn in all.

- Row 250 (`01`-`04`): Settings > Permissions > Ask for approval; Session X (`dsflash:main`) asks to run `rm -rf scratch_dir`
  -> the approval card shows in X. New chat opens Session Y: no card in Y; Y / S / N pressed in Y put NO `approval/respond`
  in the trace; the sidebar shows exactly one waiting row, X's. Back on X (a sidebar CLICK): `session/open` X, its canonical
  `session/hydrate {include: ["pending_approvals"]}`, the card again; Approve once -> ONE `approval/respond {session_id:
  dsflash:main, approval_id: …}`, the turn completes, `scratch_dir` is gone on disk. No `approval/respond` ever carried Y's id.
- Row 247 (`05`-`07`, no model turn): the app relaunched with `OCTOSCODE_SESSION_LINK` naming Session Y in `link-ws`, a
  symlink to `ws` that the server canonicalizes: the "Open saved conversation" panel; Open conversation -> refused on the
  panel; the trace shows the catalog read `session/list {cwd: <scratch>/live-a20/link-ws}` and then NO `session/open` and NO
  `session/hydrate` for Y. Dismiss link. Relaunched with the exact `ws`: Open conversation -> `session/open {Y, cwd: ws}`
  and Y's history read; the panel closes.

Redactions: the captures are cropped to the OctosCode window; the header path and every machine path on the saved-link
panel are repainted as `<scratch>/…`; in the trace machine paths read `<scratch>` / `<worktree>` / `<home>` / `<tmp>`.
The token lived in a mode-600 file and the serve's environment only (`OCTOS_AUTH_TOKEN`; the app got it as
`OCTOS_BEARER`); every file the run wrote was scanned for it (whole, first 8, last 8: 0 hits); the serve was stopped and
the copied data, instance dir and token deleted.
