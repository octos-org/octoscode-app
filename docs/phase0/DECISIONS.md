# Phase 0: decisions (gate package, outer loop, 2026-09-28)

Supersedes `DECISIONS-draft.md`. Every claim links to a merged artifact in this repo. Lane scores:
`outer/scores.json` (not tracked).

| Evidence | File | Card / score |
|---|---|---|
| Parity matrix: 361 capabilities / 39 web features | `docs/parity-matrix.{csv,md}`, `docs/parity/*` | #1a–#1j, 8–10/10 |
| Core protocol matrix (125 methods) + pin delta | `docs/protocol-matrix.{csv,md}`, `docs/protocol-pin-delta.md` | #2, #2b, 9–10/10 |
| AppUI extension methods (34) | `docs/protocol-ext-matrix.{csv,md}` | #2c, 10/10 |
| Build baseline, dependency probe, registration recipes | `docs/baseline.{md,csv}` | #3, 9/10 |
| Phase-4 walk rows (234 cases / 49 Playwright specs) | `docs/walk-rows.{csv,md}` | #4, 10/10 |
| Headless native test harness | `harness/`, `docs/harness/` | #5, 9/10 |
| App-shape spike | `docs/spike-d9/` | #6 (pending) |

## D1. Code home: DECIDED (operator)
Local git repo `~/home/octoscode-app`, no GitHub remote yet. The operator decides the remote later.

## D2. Rust vs Octoscript boundary: DECIDED (operator directive 8.5)
Every screen goes through the design flow (image generation → ≥ 9/10 vs Codex → Octoscript-Makepad mapping). Rust
supplies only the wiring: protocol client, session store, async/state. Exactly how depends on D9.

## D3. Codex-style design board: OPEN (operator sign-off, Stage A)
Image generation uses the operator-supplied OpenAI Images key (`gpt-image-2`), outer loop only, not the ChatGPT/Codex
quota. The outer loop reviews with vision. Needs the operator's Codex reference screenshots (app window only).

## D4. Protocol target and parity gate: PROPOSED
- **Target octos `a6ea8505`** (OctoSense's pin). The web pin `4231669` is its ancestor; the diff is 31 inserted lines,
  0 removed, all additive (`protocol-pin-delta.md`). The method-name set is identical. It includes in-process AppUI
  (6804ee5d).
- **Gap:** of the 125 core methods, native handles 35, decodes-only 23, and lacks 67. 53 web-used core methods are not
  handled. Of the 34 extension methods, 26 are served by octos and all 34 are absent natively.
- **Not blocking:** 8 extension methods (`session/driver/*`, `session/wake/*`, `peer/dispatch|control`) exist in no
  octos revision, including main. The web gates them on `external_driver_v1`, and native must gate them the same way.
- **Gate:** every web-used core + served-extension method is `handled` natively with a test through the production
  client path.

## D5. Transport per target: DECIDED (8.4)
Desktop: WebSocket to `octos serve`. Android: spawned `serve --stdio`. HarmonyOS/embedded: out of scope.
`a6ea8505` makes in-process AppUI available later.

## D6. Octos revision / operator WIP: DECIDED (operator)
The stale `~/home/Octoscript-AppCard` checkout and its WIP `deb433e9` are ignored, and lanes are forbidden from
touching them. The native base is OctoSense `apps/appcard/app` @ `6e9bfd4` (public main has since moved; pin
deliberately).

## D7. Scope: DECIDED
All 39 web feature dirs. **Size of the job:** 361 capabilities: 14 exist, 76 partial, **271 missing**. Largest
areas: `session` 59 (32 missing), `autonomy` 26 (all missing; goal/loop/monitor/agent notifications land on explicit
no-op arms), `timeline` 12, `connection` 13. **23 capabilities have no web test**, so a spec must be written before
a native card can claim them (list in `parity-matrix.md`).

## D8. Behavioural acceptance: DECIDED (8.3)
234 walk rows from 49 Playwright specs. 226 need only a fixture server, 5 none, and **3 need a real model turn**
(`dsflash`). 31 are web-only (browser mechanics) with reasons, to be reviewed rather than silently dropped.

## D9. App shape: OPEN (spike #6 running)
Two ways to ship inside OctoSense, both with cited recipes in `baseline.md` §5:
- **(i) native `AppModule`** like appcard: Rust owns UI + protocol. It mounts design-flow L0 cards.
- **(ii) contained OctoScript app + Rust host service** like Mail: the script app is the design-flow output
  (App Hub isolate, manifest capabilities), and the host service owns the octos client and session store.
Open question the spike answers: can a host service **stream** protocol notifications (`message/delta` etc.) into a
contained script app? If not, (ii) can't carry a live timeline without new App Hub work.

## D10. Dependency route: DECIDED (evidence: `baseline.md` §4)
Depend on `octos-app-transport` / `octos-app-store` by **git + pinned rev** of `OctoSense-org/OctoSense`. Proved to
build standalone with no `[patch]`. Path deps are for the local dev loop only.

## D11. Validation model: DECIDED (operator)
Lanes self-validate UI with the headless harness (hidden window, real input, the app's own `/g`, clean exit). The
outer loop re-runs a sample. Known limits: needs the macOS GUI session; `--remote` is compiled out on Android. Known
red: 7 appcard lib tests on the clean base (`baseline.md`). Diffs aren't gated on them.
