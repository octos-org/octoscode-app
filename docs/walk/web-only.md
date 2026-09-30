# Web-only rows (natively_handled=none) — confirmation per row (#33a item 4)

The 16 `none` rows of `docs/walk-rows.csv`, each confirmed against the web source
(`web_src` column). Two kinds: **web-only** — the behaviour lives in browser mechanics
the native client does not have (URL parsing, tab storage, CSS/matchMedia); **real gap**
— a native surface the app does not ship yet (filed honestly, not papered over).

| row | spec | case (short) | verdict | evidence |
|---|---|---|---|---|
| 87 | e2e/model-management.spec.ts:62 | manages a provider through the DSH-style Models settings flow | **real gap** | the native settings drawer ships Session settings (Model/Permissions/Sandbox/Context sections, `settings_drawer`) but no provider CRUD editor; the web flow is a browser form (`connectAndStartWorkspace` → `openModelSettings` → "Add provider" form) |
| 88 | e2e/model-management.spec.ts:168 | keeps a rejected provider draft while redacting the failure | **real gap** | same missing native provider editor; redaction lives in that form's rejection path (spec reads `rejectedCredential` fill → server reject → draft retained) |
| 104 | e2e/onboarding.spec.ts:18 | validates the address without opening a socket… | web-only | wor: "Pure connect-screen address validation and focus handling; the case deliberately…" — the native walk app auto-connects (gate mode not wired, CONNECT_MISSING) |
| 105 | e2e/onboarding.spec.ts:47 | explains missing authentication while preserving password privacy… | web-only | wor: client-side auth-field validation and masking only; no connection established |
| 108 | e2e/pairing-link.spec.ts:48 | a good link connects with no token box… | web-only | wor: pairing-link intake is browser-only: query-string handling plus tab/legacy storage |
| 110 | e2e/pairing-link.spec.ts:159 | an expired code and a malformed link each get their own bounded copy | web-only | wor: browser-side URL parsing and bounded copy |
| 111 | e2e/pairing-link.spec.ts:184 | an octos origin off this computer is refused without a request | web-only | wor: refused before any request — browser-side origin check |
| 112 | e2e/pairing-link.spec.ts:205 | the paired token survives only this tab until Forget | web-only | wor: tab-scoped credential lifetime (session storage isolation) |
| 114 | e2e/pairing-link.spec.ts:272 | a pairing-capable server this browser has seen offers itself once | web-only | wor: browser-side probe of a remembered endpoint |
| 211 | e2e/theme.spec.ts:34 | theme cycle preserves one palette across manual and system modes | web-only | wor: CSS custom-property resolution across matchMedia and reload; no protocol involvement. (Native counterpart shipped in #31d/#31d2: preference + OS reader + token set, verified in f31d/f30e tests and the dark captures.) |
| 213 | e2e/theme.spec.ts:115 | theme ${method} failure keeps the app usable… | web-only | wor: injects a browser Storage failure; purely browser-side. (Native persistence writes a file; the failure path is best-effort by design — use-theme.ts:24-26 parity.) |
| 216 | e2e/visual.spec.ts:52 | connect gate matches baseline | web-only | wor: pure visual-regression snapshot of the connect gate; browser rendering only (gate mode not wired natively) |
| 217 | e2e/visual.spec.ts:70 | empty chooser matches baseline | web-only | wor: visual-regression snapshot of the workspace chooser |
| 220 | e2e/workspace-browse.spec.ts:41 | browses the server's folders, drills in and out… | **real gap** (gate surface) | the browser flow is the pre-connect gate's folder picker (`connect(page, BROWSE_TOKEN)` → `openBrowser` → `data-browse-notice`); the native walk instance auto-connects and has no gate browser — same CONNECT_MISSING scope as rows 104–217 |
| 221 | e2e/workspace-browse.spec.ts:85 | renders bounded copy when the server refuses a folder | **real gap** (gate surface) | same gate browser (`hidden_skipped` notice flow) |
| 223 | e2e/workspace-browse.spec.ts:144 | picking a subfolder fills the path box without leaving the parent | **real gap** (gate surface) | same gate browser ("Use folder Projects" → path box value) |

Summary: **11 web-only** (browser mechanics: URL/pairing intake ×6, storage/visual ×4,
CSS theme resolution ×2 counting 211/213 with native counterparts already verified),
**5 real gaps** (model-management provider CRUD ×2 — no native editor; the connect
gate's workspace browser ×3 — gate mode not wired in the walk instance).

Note on the real gaps: none of them is a walk-runner target (they are `none` rows);
they are recorded here for Phase-4 planning, not fixed in this card ("don't fix app
code in this card").
