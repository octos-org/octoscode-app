# Phase-1 live gate: PASSED (2026-09-28, outer loop)

**Setup.** Real `octos serve` at octos `a6ea8505` (solo; isolated data dir holding a private copy of the operator's `dsflash`
profile; scratch workspace git repo with `main.rs`), the native octoscode `AppModule` in the OctoSense desktop shell
(`--features app-octoscode`, window hidden, `--remote 8490`), driven by real input only (`/click`, `/t`) by
`live-gate.sh`. Model: **deepseek-v4-flash** (`dsflash`).

| Gate step (8.8 condition 8) | Result | Evidence |
|---|---|---|
| open workspace | `session/open` with the workspace cwd; `conn: Live` | `s1-open.json`, trace |
| prompt | typed into the composer, Send; draft cleared | `s3-completed.json` |
| streamed answer | answer streamed into the timeline in ~5 s, `Worked for 1s` | `g2-streaming.png`, `g3-completed.png` |
| interrupt | 2nd turn interrupted **mid-stream** after 424 streamed chars; server `outcome=captured ack=interrupted`; timeline ends with `[system.notice] interrupted` | `g4-interrupted.png`, `s4-interrupted.json`, `serve-excerpt.log` |
| protocol trace | 266 frames: out `session/open`, `turn/start` ×2, `turn/interrupt`, `session/list` ×2; in `projection/envelope` ×236, `turn/started` ×2, `progress/updated` ×9 … (no secrets) | `trace.jsonl` |

**Answer text (verbatim, first line):** `main.rs` prints a single line, `5`: `main` calls `println!("{}", add(2, 3))`, and since the helper `add(a: i32, b: i32) -> i32 { a + b }` returns the sum of its two arguments, `2 + 3` evaluates to `5` and gets formatted into the `{}` placeholder.

**History.** Run 1 failed on my script (it typed into the shell's App Library field). Run 2 **found a real defect**: we
negotiate `projection.envelope.v2` but had no `projection/envelope` handler, so every live update was dropped. Fakes had
sent bare `message/delta`, and **the live server sends none**. Fixed in #13 with a recorded real-traffic fixture. Runs 3–4 pass.

**Defects found live (→ card #14):** (1) the turn's `user.message` renders **after** the assistant answer; (2) a stray
`[assistant.text] .` entry; (3) session list / threads not refreshed after open (`sessions: 0`); (4) every run reuses
session `dsflash:main`, so context carries over between runs (a fresh session per open is expected).

**Not yet:** the view is the marked **fallback**, not the design-flow cards (Stage B round 5 in progress). No device run.
**Model spend:** 8 short `dsflash` turns total across all runs (≈ 3k output tokens).
