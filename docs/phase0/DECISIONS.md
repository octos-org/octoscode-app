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

## D9. App shape: DECIDED by the operator (2026-09-28): native AppModule mounting design-flow L0 cards (evidence: `docs/spike-d9/REPORT.md`, card #6 10/10)
Both shapes were built as minimal apps, registered in the OctoSense desktop shell, connected to a local `octos serve`
@ `a6ea8505` (`profile/local/create` → `session/open`), and driven headlessly (evidence + screenshots in `docs/spike-d9/evidence/`).

| | (i) native `AppModule` | (ii) contained script app + host service |
|---|---|---|
| Streaming protocol → UI | **direct** (module owns the transport receiver) | **none today**: the isolate's `host` object has only `request` / `has` / `capabilities` (`makepad/widgets/src/splash_host.rs:243-335`, outer-verified). Needs a new `host.subscribe` upstream, or polling via `start_interval` as a stopgap |
| Capability | none needed | `octoscode` isn't in App Hub's **closed** `KNOWN_CAPABILITIES` (`app-policy/src/manifest.rs:18`), so an upstream policy change is needed (News waits on the same, App-Hub#18) |
| Design flow | via **L0 cards mounted by the host** (appcard's `l0_card.rs`/`l0_widgets.rs`/`l0_page_recipes.rs`; §8.5 Stage C "Rust host mounts the L0 cards (AppShell pattern)") | native target of `flows/script-app` + `tools/octo` |
| Size of the spike | 283 LOC, 5 files outside the app dir | 336 LOC, 3 files outside |

**Outer-loop recommendation: (i) native `AppModule` that mounts design-flow L0 cards**, which is exactly §8.5 Stage C.
Reasons: octoscode is push-driven (streaming `message/delta`, tool progress, approvals, goal/loop/agent updates:
~49 notification kinds), and (ii) can't carry any of that without two upstream App Hub changes (a push channel +
a capability family). Every screen still goes image → ≥ 9/10 → L0 mapping. Only the host is Rust. Revisit (ii) when
App Hub gains `host.subscribe` (it would suit read-mostly panels, e.g. settings, as separate bundles).
The lane recommended (ii). Its premise that the design flow can't feed shape (i) overlooks the L0-card path above.

## D10. Dependency route: DECIDED (evidence: `baseline.md` §4)
Depend on `octos-app-transport` / `octos-app-store` by **git + pinned rev** of `OctoSense-org/OctoSense`. Proved to
build standalone with no `[patch]`. Path deps are for the local dev loop only.

## D11. Validation model: DECIDED (operator)
Lanes self-validate UI with the headless harness (hidden window, real input, the app's own `/g`, clean exit). The
outer loop re-runs a sample. Known limits: needs the macOS GUI session; `--remote` is compiled out on Android. Known
red: 7 appcard lib tests on the clean base (`baseline.md`). Diffs aren't gated on them.
