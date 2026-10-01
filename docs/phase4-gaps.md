# Phase-4 gaps — smoke-only walk rows without a specific check (#41c)

The walk had 51 smoke-only rows. #41c added row-specific checks for 7 of them
(rows 58, 72, 73, 87, 102, 128, 172 — see tools/walk/run.py `#41c` block); the
other 44 are listed here with the reason each cannot get one today. These are
coverage gaps, NOT defects (unless the walk run surfaced a real FAIL — those go
to docs/walk/defects.md). Machine-readable: docs/phase4-gaps.csv.

Reason classes:

- **fixture** — the row needs protocol frames (or timing) the committed recordings lack. RULES require
  replaying recorded real traffic; this lane has no live gate to record from, so each needs a recording
  pass (outer loop).
- **native-surface** — the native surface the case describes is not mounted/implemented yet (measured 41c recon).
- **harness** — the walk runner cannot vary what the row needs (viewport width, per-row env, first-run flow).

| row | area | reason class | reason |
|---|---|---|---|
| 4 | settings | native-surface | no notifications opt-in section in the settings drawer (41c recon: header + connection action only) |
| 9 | settings | fixture | profile-identity frames (scoped mismatch) absent from the session fixture |
| 10 | settings | fixture | cross-profile launch binding frames absent from the session fixture |
| 13 | recovery | fixture | external-held-session media/reasoning restore frames absent |
| 14 | recovery | fixture | peer/control seat-release frames absent from r6-peer (measured: 0 peer/control frames) |
| 20 | recovery | fixture | workspace restore receipt + canonical refresh frames absent |
| 66 | recovery | fixture | parked-question restore/resolve frames absent |
| 86 | settings | native-surface | no provider-confirmation surface; also viewport-width semantics the fixed 900px walk window cannot vary |
| 98 | recovery | fixture | goal pause/resume/stop receipt frames absent |
| 106 | keyboard | harness | first-run flow never mounts: the walk app auto-connects to the replay server (same class as CONNECT_MISSING) |
| 115 | recovery | fixture | peer-turn frames that flip a staging row to live are absent (fleet/goals/loops rows collapse under fixtures — measured) |
| 117 | recovery | fixture | peer-turn terminal frames absent (same collapse) |
| 119 | recovery | fixture | peer-turn terminal frames absent (same collapse) |
| 120 | peer | fixture | needs an advertised-vs-unadvertised fixture PAIR to distinguish absence-by-design from absence-by-missing-surface; r6-peer has no peer/control either way |
| 121 | peer | fixture | same pair requirement as row 120 for external_driver_v1 |
| 122 | peer | fixture | peer/control frames absent from r6-peer (measured: gather x6, prepare x3, control x0) |
| 123 | peer | fixture | peer/control accepted-receipt frames absent |
| 125 | peer | fixture | peer_control_refused typed-refusal frames absent |
| 126 | peer | fixture | driver_fence_stale typed-error frames absent |
| 127 | peer | fixture | peer/control frames absent (same as 122); no command surface to drive |
| 129 | peer | native-surface | Fleet onboarding (Start/brief) surface not mounted under fixtures (roster rows collapse — measured) |
| 130 | peer | fixture | peer/dispatch frames absent (measured: dispatch x0 in r6-peer) |
| 135 | peer | fixture | peer/dispatch + stale-lease error frames absent |
| 136 | peer | fixture | peer/dispatch refusal frames absent |
| 137 | peer | native-surface | protocol console / Advanced disclosure surface not mounted under fixtures |
| 138 | peer | native-surface | lane picker surface not mounted under fixtures |
| 142 | keyboard | native-surface | plan surface not mounted under fixtures (goals/loops rows collapse — measured) |
| 151 | peer | fixture | peer/dispatch + acknowledgement-timeout timing semantics cannot be produced by a recorded fixture |
| 162 | recovery | fixture | origin/tab-credential/workspace restore-across-refresh frames absent |
| 164 | settings | native-surface | no Model/Models section in the drawer (measured) to separate runtime from profile default |
| 174 | recovery | fixture | pending structured question + reload frames absent |
| 176 | settings | native-surface | workspace-launch-decision surface absent from the drawer |
| 181 | settings | fixture | skill-write + profile-lock frames absent from the session fixture |
| 183 | peer | harness | ${viewport.width} semantics; the walk window is fixed at 900px and the runner cannot vary the viewport per row |
| 184 | peer | native-surface | skip-link surface absent; multiline command a11y semantics need the commands surface |
| 185 | peer | native-surface | no session-search surface in the native shell to exercise the empty-search way back |
| 186 | recovery | native-surface | the /resume candidates list is not implemented (41c recon: palette closes, no candidates surface) |
| 187 | recovery | fixture | held historical-open + late-transcript frames absent |
| 188 | recovery | fixture | retained busy historical-session frames absent |
| 189 | recovery | fixture | per-session reasoning visibility + model-effort frames absent |
| 204 | recovery | fixture | multi-session transcripts (distinct sessions across a reload) absent from the single-session r3 fixture |
| 207 | recovery | fixture | slow model-management import frames + cancel timing cannot be produced by a recorded fixture |
| 210 | recovery | fixture | late clipboard command-result injection timing cannot be produced by a recorded fixture |
| 212 | settings | harness | needs per-row theme env control; the runner fixes the app env at launch (OCTOSCODE_THEME is process-wide) |

Split after #41c: specific=7 checked in-lane (7 rows), documented gaps=44 (this file). smoke=0 among rows that CAN have a specific check; the 44 above are the rows that cannot, each with its reason.
