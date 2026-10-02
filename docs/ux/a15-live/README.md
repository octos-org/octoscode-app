# A15 live evidence (own octos serve a6ea8505, dsflash, copied data; never the operator's serve)

- `smoke/`: `tools/judge/live_smoke.py` on build 4e6bee13, 18/18 (`checks.txt`), with the app's protocol trace `trace.jsonl`.
  The app was restarted mid-history (the APP, not the serve), the session reopened with every earlier turn, and the next prompt
  streamed.
- `approval/`: the ask-mode proof on build acd68b94, 19/19 (`checks.txt`). `rm -rf scratch_dir` showed the approval card,
  Approve once ran it, and the folder was gone on disk.
- `restart-order/`: zero-turn check of the stopped-notice order after a restart (build 7cd0a5a3).

Redactions (integrator, 2026-10-02): the scratch workspace path in each capture's header is painted over; in the trace, machine
paths read `<scratch>` / `<worktree>` / `<home>`. No token appears in any file. The token lived in a mode-600 file, now deleted.
