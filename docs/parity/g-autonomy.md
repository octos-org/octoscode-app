# Oracle map — group `g-autonomy`

Parity matrix for the `g-autonomy` web feature dirs, cut to seed later Phase-2/3 cards.
Scope per the #1a addendum (2026-09-28): `autonomy` moved to lane p0-map-e (#1g); this lane owns 6 dirs.
Machine-readable twin: `docs/parity/g-autonomy.csv`.

**Dirs** (smallest first): `btw`, `skills`, `context`, `review`, `product-controls`, `models`.

- Rows (capabilities): **40** across **6** dirs.
- `native_status`: exists **3**, missing **29**, partial **8**.

## Per-feature summary

| feature | src_loc | capabilities | exists | partial | missing | unit tests | e2e specs |
|---|---:|---:|---:|---:|---:|---:|---:|
| `btw` | 276 | 7 | 0 | 0 | 7 | 6 | 2 |
| `skills` | 356 | 7 | 0 | 0 | 7 | 6 | 1 |
| `context` | 432 | 6 | 2 | 0 | 4 | 8 | 1 |
| `review` | 1081 | 6 | 0 | 1 | 5 | 9 | 3 |
| `product-controls` | 1356 | 7 | 1 | 2 | 4 | 11 | 4 |
| `models` | 1367 | 7 | 0 | 5 | 2 | 13 | 2 |
| **total** | | **40** | **3** | **8** | **29** | | |

## Top 5 largest gaps

Ranked by (dir size, user-visibility) among `missing`/`partial` capabilities:

- **`models`** — model notices: keep a sticky per-session notice board of the five runtime dispositions with exact copy (e.g. 'restart_required' keeps naming the model the server still runs) _(missing)_. no native runtime-disposition notice board; the model notice copy/rules (model-notices.ts:97 `noticeMessage`, :201 `nextModelNoticeBoard`) have no native analogue  
  web `src-web/apps/web/src/features/models/model-notices.ts:97`; methods `profile/llm/select; profile/llm/upsert`.
- **`models`** — model selection: read the session's selectable models and switch the active model, reporting each runtime disposition (reloaded / deferred / restart_required / persisted_but_not_live / unchanged) _(missing)_. profile/llm/select is an octos-cli AppUI extension (packages/client/src/onboarding-methods.ts:10); no native session-level model switcher and no native disposition notice board  
  web `src-web/apps/web/src/features/models/use-model-selection.ts:48`; methods `profile/llm/select`.
- **`product-controls`** — control seat: drive an already-accepted external-master peer with exactly one peer/control frame per command (mounted only when peer/control + external_driver_v1 are advertised and a binding is observed) _(missing)_. peer/control is an AppUI extension (packages/client/src/peer-protocol.ts) with no native client support; no control seat in the native composer  
  web `src-web/apps/web/src/features/product-controls/SessionControlBar.tsx:871`; methods `peer/control`.
- **`product-controls`** — driver disclosure: a read-only external-recovery disclosure seat that re-gates the peer console on peer/control + peer/dispatch and stages a new peer (Acquire seat) _(missing)_. peer/dispatch + peer/control are AppUI extensions; no native driver disclosure or fleet console  
  web `src-web/apps/web/src/features/product-controls/SessionControlBar.tsx:183`; methods `peer/dispatch; peer/control`.
- **`product-controls`** — model menu: show the authoritative model catalog grouped by provider and select only a currently usable entry (unavailable entries stay explainable, not selectable) _(missing)_. profile/llm/* are octos-cli AppUI extensions (packages/client/src/onboarding-methods.ts:6-12) with no proto.rs entry or store consumer; no native model menu  
  web `src-web/apps/web/src/features/product-controls/SessionControlBar.tsx:565`; methods `profile/llm/catalog; profile/llm/select`.

## Capabilities with NO web test at all

These need a spec written **before** a native parity card can be judged:

- `skills` — skills jobs: surface the background skill-action job snapshot (SkillActionJobUpdated) (`src-web/apps/web/src/features/skills/SkillsDialog.tsx:21`)
- `review` — permission profile: read the coding permission profile (list) and apply an explicit update from the review/coding-safety surface (`src-web/apps/web/src/features/review/use-coding-safety.ts:95`)
