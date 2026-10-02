# Final-build live proofs (integrator, 2026-10-02, build 26440b33 = main's code at 5da3784c)

Supervisor's bar, run on the shipping build against a private `octos serve` (a6ea8505, dsflash, a COPY of the gate data,
a fresh instance dir, its own token). The operator's serve on :50190 was never touched.

| Proof | Result | Evidence |
|---|---|---|
| Server SIGTERM mid-session, with a real dsflash turn: same port restart, the same Session re-opened `{reconnect: true}`, no Connect card, the banner names the server in use (never the default), draft and timeline survive with no duplicate turn, the next prompt streams | 50/50 | `sigterm/walk.log`, captures |
| Server SIGKILL, same checks | 50/50 | `sigkill/walk.log`, captures |
| App (not the serve) restarted mid-history: the Session reopens with every earlier turn (file write, Chinese answer, queue, stop) read by scrolling the transcript, and the next prompt streams | 18/18 | `app-restart/checks.txt`, `app-restart/trace.jsonl`, captures |

Two walk checks were updated after A15 made the server title a Session from its first prompt:
- the "same conversation" header check now accepts an untitled header that comes back carrying that title. The Session id
  is the identity, and it is checked separately.
- the duplicate-turn count reads transcript rows only. The header title and the sidebar row now repeat the first prompt.

The restart check in `tools/judge/live_smoke.py` reads the scrolled transcript, never the title.

Redactions: the header path is painted over in captures, and machine paths read `<scratch>` / `<home>` in text. No token
appears in any file. The A12 walk keeps its own trace in a temporary state dir, so the SIGTERM/SIGKILL wire facts are in
the walk logs: the reconnect open and the hydrate replies.
