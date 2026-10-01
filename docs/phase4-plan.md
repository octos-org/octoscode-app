# Phase 4 plan — capability gaps as feature-sized cards (supervisor §8.22)

## Bucket definitions (docs/parity-matrix.csv `phase4_bucket` = generator, `phase4_bucket_manual` = hand pass, `operator_confirmed`)
- **A — implemented-with-test:** the native code exists (file:line) AND a test exercises it through the production path (unit/replay/specific walk check). Row is `exists`.
- **B — web-only (operator-confirmed):** the user-visible BEHAVIOUR exists only on the web (browser storage/clipboard/URL/tab internals, web-only migrations,
  web multi-record plumbing with no user-visible behaviour natively). 45 rows, confirmed by the operator 2026-10-01.
- **C — Phase-4 gap:** missing or untested natively. Each row has a one-line gap in docs/phase4-gaps.md. Work = implement and/or test, then flip to A.

**Final bucket = `phase4_bucket_manual` when set, else `phase4_bucket`.** To flip a row, set `phase4_bucket_manual=A` AND `phase4_evidence_manual`
(file:line + production-path test); a regen may rewrite `phase4_bucket` but never the manual columns. Count A with the final bucket only.

Totals at triage: **A 42 · B 45 · C 214**.

## Cards, in order of the walk rows they unblock
| # | card | C rows | open walk rows unblocked | notes |
|---|---|---|---|---|
| P4b | timeline | 29 | 34 (16-19,23,59,62,63,76,77,80,144-147,150,153,154,…) | RUNNING (p0-map-b); incl. a11y announcements |
| P4c | settings | 13 | 25 (71,144-171…) | incl. notifications opt-in (native-surface rows 4/86) |
| P4a | composer | 39 | 1 (200) + live rows 33/50/65/67/92 | RUNNING (p0-harness); approval mode (pill unwired), attachments/+, mic, persisted defaults |
| P4-pair | native pairing (deep link / QR / paste) | 2 + walk 108-114 | 7 | operator: IN SCOPE; reuse OctoSense's phone QR scanner |
| P4d | control (2 cards: d1 queue/steer/stop, d2 the rest) | 34 | 0 | parity only |
| P4e | autonomy + autonomy2 (2 cards) | 32 | 0 | goals/loops/monitors/fleet |
| P4f | history (undo/rewind/fork) | 14 | 0 | |
| P4g | sessions: store-hydrate + list-sidebar + links-resume | 27 | 0 | |
| P4h | workspace + activity + error + connection | 26 | 0 | |
Plus #43a recording pass (39 fixture-limited walk rows, ≤ 80 dsflash turns), #43b runner (3 rows), and the 11 native-surface rows (they sit inside P4c/P4a/P4d).

## Estimate
Throughput on comparable cards today: ~8-12 rows per 2-hour glm card (more with MiniMax). 214 C rows ≈ 20 cards ≈ 40 lane-hours; with 4-5 lanes in parallel
and review/merge/device checks, **≈ 1.5–2 days of wall time**, then the Phase-4 walk (all walkable rows, specific checks) and the operator's walk.
Order: P4b, P4c, P4a, P4-pair first (they unblock walk rows), then P4d-h.
