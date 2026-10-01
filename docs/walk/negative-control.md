# Negative control — the walk-runner's checks CAN fail (#33a review item 2)

Deliberately broken setup: `WALK_SCENARIO=conversation=session` — the conversation
area's checks run against the **session** replay fixture (a real a6ea8505 recording
that carries no coding turn), instead of the conversation fixture. The shell still
mounts normally (same binary, same env, real app window); only the fixture differs.

Command (rc=1):

    WALK_SCENARIO=conversation=session python3 tools/walk/run.py --only conversation --port 8370

Result (tmp/33a-negative-control.log, full output; row CSVs kept at
tmp/33a-negative-results*.csv):

| check | kind | verdict on the broken setup |
|---|---|---|
| module reaches conn: Live with the OctosCode heading | smoke | pass |
| the thread list renders the opened session row | smoke | pass |
| the composer accepts typed text (prompt input) | specific | pass (draft round-trip works on any fixture) |
| composing clears the draft on send | specific | pass (send clears locally regardless of fixture) |
| **a sent prompt streams an assistant answer row** | specific | **FAIL** (timed out 12s — the session fixture never streams an answer) |
| the user's own prompt renders as a row | specific | pass (the local echo still renders) |
| **the answer row renders after the prompt row (order)** | specific | **FAIL** (no answer row exists) |
| **the turn's timeline item kinds are present** | specific | **FAIL** (`kinds=[composer, newchat, threadrow, userbubble]` — no `assistantprose`) |
| the Stop control is present for the live turn | smoke | pass |

Reading: the runner **fails (rc=1, 3 FAIL rows) — it does not block and does not
pass** — precisely because the specific checks assert the fixture-dependent
behaviour the user would SEE. The smoke checks passing on the broken setup is the
review's point made visible: a green smoke row says "the shell mounted", nothing
more. This is why results.csv now carries the `depth` column (06b259c).

## Re-verified on #41c (2026-09-30, after the row-specific checks landed)

    WALK_SCENARIO=conversation=session python3 tools/walk/run.py --only conversation --limit 10

→ **rc=1** (`tmp/41c-negative.log`): the specific checks still fail on the broken setup — the streaming,
order, and timeline-kind checks all FAIL (the fixture never streams an answer), and the runner's summary is
`fail=10` with `pass by depth specific=0 smoke=0`. The negative control was NOT weakened by the #41c changes.
