# Phase-4 walk rows — one per web Playwright case

The final acceptance walk: each row is one `test(...)` case from the 49 web
Playwright specs, rewritten as user-level steps to run against the **native app**
+ a real `octos serve`. Machine-readable twin: `docs/walk-rows.csv`.

- Specs: **49** (49 expected).
- Cases (rows): **234**.
- `needs`: fixture **226**, none **5**, real-turn **3**.
- `natively_handled`: all **175**, none **16**, some **43**.

## Real-turn quota

**3** rows need a **real model turn** (they live in `e2e-live/`, which
proxies a real `octos serve` — see `playwright.live.config.ts`). Running the whole
walk once therefore costs ~3 model turns. The remaining
**226** `fixture` rows run against the scripted mock server
(`apps/web/scripts/mock-ui-server.mjs`, port 50080) and **5** `none` rows need
no server round-trip at all.

### Real-turn rows

- `e2e-live/glm-model.spec.ts:39` — runs a coding turn with the expected runtime model and restores it after refresh
- `e2e-live/glm-model.spec.ts:175` — keeps a background turn alive while a sibling Session is focused
- `e2e-live/turn-collision.spec.ts:30` — discloses the other client's turn instead of claiming it

## Counts per spec

| spec | cases | real-turn | fixture | none | web-only |
|---|---:|---:|---:|---:|---:|
| `e2e-live/glm-model.spec.ts` | 2 | 2 | 0 | 0 | 0 |
| `e2e-live/turn-collision.spec.ts` | 1 | 1 | 0 | 0 | 0 |
| `e2e/attention.spec.ts` | 4 | 0 | 4 | 0 | 0 |
| `e2e/auth-identity.spec.ts` | 3 | 0 | 3 | 0 | 0 |
| `e2e/canonical-recovery.spec.ts` | 1 | 0 | 1 | 0 | 0 |
| `e2e/capacity-and-peers.spec.ts` | 1 | 0 | 1 | 0 | 0 |
| `e2e/chat-handover.spec.ts` | 3 | 0 | 3 | 0 | 0 |
| `e2e/command-surface.spec.ts` | 8 | 0 | 8 | 0 | 0 |
| `e2e/connection-storage.spec.ts` | 6 | 0 | 6 | 0 | 6 |
| `e2e/copy-conversation.spec.ts` | 3 | 0 | 3 | 0 | 1 |
| `e2e/delivered-files.spec.ts` | 2 | 0 | 2 | 0 | 0 |
| `e2e/diff-review.spec.ts` | 3 | 0 | 3 | 0 | 0 |
| `e2e/draft-recovery.spec.ts` | 2 | 0 | 2 | 0 | 2 |
| `e2e/driver-discovery.spec.ts` | 10 | 0 | 10 | 0 | 0 |
| `e2e/final-input.spec.ts` | 7 | 0 | 7 | 0 | 0 |
| `e2e/final-reading.spec.ts` | 7 | 0 | 7 | 0 | 2 |
| `e2e/interaction-ownership.spec.ts` | 4 | 0 | 4 | 0 | 0 |
| `e2e/keyboard-parity.spec.ts` | 6 | 0 | 6 | 0 | 0 |
| `e2e/loaf.spec.ts` | 1 | 0 | 1 | 0 | 0 |
| `e2e/local-preferences.spec.ts` | 6 | 0 | 6 | 0 | 1 |
| `e2e/modal-a11y.spec.ts` | 4 | 0 | 4 | 0 | 4 |
| `e2e/modal-stack.spec.ts` | 2 | 0 | 2 | 0 | 0 |
| `e2e/model-management.spec.ts` | 2 | 0 | 2 | 0 | 0 |
| `e2e/multi-session-fence-soak.spec.ts` | 1 | 0 | 1 | 0 | 0 |
| `e2e/multi-session-recovery-soak.spec.ts` | 4 | 0 | 4 | 0 | 0 |
| `e2e/native-workflows.spec.ts` | 10 | 0 | 10 | 0 | 0 |
| `e2e/onboarding.spec.ts` | 4 | 0 | 2 | 2 | 2 |
| `e2e/pairing-link.spec.ts` | 7 | 0 | 7 | 0 | 5 |
| `e2e/peer-activity-and-restore.spec.ts` | 5 | 0 | 5 | 0 | 0 |
| `e2e/peer-control.spec.ts` | 8 | 0 | 8 | 0 | 0 |
| `e2e/peer-controller.spec.ts` | 11 | 0 | 11 | 0 | 0 |
| `e2e/peer-dock.spec.ts` | 1 | 0 | 1 | 0 | 0 |
| `e2e/peer-readonly-composer.spec.ts` | 1 | 0 | 1 | 0 | 0 |
| `e2e/plan-card.spec.ts` | 3 | 0 | 3 | 0 | 0 |
| `e2e/product.spec.ts` | 37 | 0 | 37 | 0 | 2 |
| `e2e/profile-mutation.spec.ts` | 2 | 0 | 2 | 0 | 0 |
| `e2e/release-readiness.spec.ts` | 3 | 0 | 3 | 0 | 0 |
| `e2e/resume-and-reasoning.spec.ts` | 4 | 0 | 4 | 0 | 0 |
| `e2e/runtime-recovery.spec.ts` | 1 | 0 | 1 | 0 | 0 |
| `e2e/server-shutdown.spec.ts` | 1 | 0 | 1 | 0 | 0 |
| `e2e/session-links.spec.ts` | 8 | 0 | 8 | 0 | 0 |
| `e2e/settings-mobile.spec.ts` | 1 | 0 | 1 | 0 | 1 |
| `e2e/simultaneous-sessions.spec.ts` | 5 | 0 | 5 | 0 | 0 |
| `e2e/surface-recovery.spec.ts` | 5 | 0 | 5 | 0 | 0 |
| `e2e/theme.spec.ts` | 4 | 0 | 2 | 2 | 3 |
| `e2e/timeline-interactions.spec.ts` | 1 | 0 | 1 | 0 | 0 |
| `e2e/visual.spec.ts` | 4 | 0 | 3 | 1 | 2 |
| `e2e/workspace-browse.spec.ts` | 5 | 0 | 5 | 0 | 0 |
| `e2e/workspace-ux.spec.ts` | 10 | 0 | 10 | 0 | 0 |
| **total** | **234** | **3** | **226** | **5** | **31** |

## Web-only rows

Browser mechanics with no native meaning. Listed, not silently dropped.

| spec:line | case | reason |
|---|---|---|
| `e2e/connection-storage.spec.ts:63` | closed-tab drafts restore only for the authenticated principal and Forget clears that principal | Unsent-draft persistence is browser localStorage/sessionStorage keyed by the connected principal; there is no native-client equivalent. |
| `e2e/connection-storage.spec.ts:132` | a delayed principal lookup preserves newly typed text and cannot restore a retired identity | Draft restoration is scoped to the browser-stored principal in localStorage/sessionStorage; no native surface is involved. |
| `e2e/connection-storage.spec.ts:201` | failed Forget stays visible across identity edits until saved data can actually be cleared | Clearing saved connection data is a browser localStorage/sessionStorage operation with no native equivalent. |
| `e2e/connection-storage.spec.ts:267` | denied storage getters still allow an in-memory connection and readable cleanup feedback | Exercises blocked browser storage getters (localStorage/sessionStorage) with an in-memory fallback; there is no native equivalent. |
| `e2e/connection-storage.spec.ts:298` | a 51st unsent draft evicts the oldest, stays saved, and never blocks navigation | Draft capacity and eviction are browser localStorage cache mechanics with no native equivalent. |
| `e2e/connection-storage.spec.ts:384` | a draft typed while identity loads is never the capacity eviction victim | Draft capacity and eviction are browser localStorage cache mechanics with no native equivalent. |
| `e2e/copy-conversation.spec.ts:68` | keeps the phone header for the workspace title: no copy button there | A responsive-layout assertion about which header control renders at a phone viewport. |
| `e2e/draft-recovery.spec.ts:21` | reload restores exact unsent text without dispatching it, and sent drafts stay cleared | Unsent-draft persistence is browser localStorage behavior with no native-client equivalent. |
| `e2e/draft-recovery.spec.ts:55` | a rejected draft storage write preserves editing and warns before text can be lost | Draft-save failure handling is browser localStorage behavior with no native-client equivalent. |
| `e2e/final-reading.spec.ts:165` | tree focus follows a collapsed parent and no longer references a removed search row | Pure browser focus bookkeeping: tree active-row tracking has no native counterpart. |
| `e2e/final-reading.spec.ts:583` | a missing optional syntax grammar leaves readable and copyable plain code | About the web client's optional syntax-grammar asset load and its plain-code fallback; no native counterpart. |
| `e2e/local-preferences.spec.ts:652` | only explicit save persists display preferences; reload restores preferences and the separate draft without storing credentials | Display preferences live in browser local storage and draft recovery is browser-local; the workspace connection is the only native touch. |
| `e2e/modal-a11y.spec.ts:60` | Escape closes the modal and returns focus to its opener | Pure modal focus management in the browser; no native counterpart. |
| `e2e/modal-a11y.spec.ts:74` | initial focus lands on the surface's first focusable control | Pure modal focus management in the browser; no native counterpart. |
| `e2e/modal-a11y.spec.ts:86` | Tab and Shift+Tab cycle focus inside the modal only | Pure modal focus management in the browser; no native counterpart. |
| `e2e/modal-a11y.spec.ts:102` | background content is hidden from assistive tech while open | Pure modal background-hiding bookkeeping in the browser; no native counterpart. |
| `e2e/onboarding.spec.ts:18` | validates the address without opening a socket and leaves focus on the correction | Pure connect-screen address validation and focus handling; the case deliberately opens no socket and sends no protocol frame. |
| `e2e/onboarding.spec.ts:47` | explains missing authentication while preserving password privacy and IME entry | Client-side auth-field validation and masking only; no connection is established and no protocol frame is sent. |
| `e2e/pairing-link.spec.ts:48` | a good link connects with no token box and leaves nothing in the address | Pairing-link intake is browser-only: query-string handling plus tab and legacy storage migration, with no ws protocol surface. |
| `e2e/pairing-link.spec.ts:159` | an expired code and a malformed link each get their own bounded copy | Expired and malformed link handling is browser-side URL parsing and bounded copy; no protocol session is opened. |
| `e2e/pairing-link.spec.ts:184` | an octos origin off this computer is refused without a request | Refusing a non-local origin happens before any request, so it is browser-side origin validation with no protocol traffic. |
| `e2e/pairing-link.spec.ts:205` | the paired token survives only this tab until Forget | Tab-scoped credential lifetime (session storage isolation, reload persistence and Forget eviction) is browser storage semantics with no protocol surface. |
| `e2e/pairing-link.spec.ts:272` | a pairing-capable server this browser has seen offers itself once | The discovery offer is a browser-side probe of a remembered endpoint, made with no protocol session. |
| `e2e/product.spec.ts:1718` | deep link: ?s= restores the selected session after reload | Session selection is carried in the ?s= URL query and restored on reload; the assertion is browser URL routing with no native equivalent. |
| `e2e/product.spec.ts:1842` | keeps the DSH-aligned dark product shell WCAG A/AA clean | Axe-core audit of the dark shell plus CSS colour and animation inspection; pure rendering with no native meaning. |
| `e2e/settings-mobile.spec.ts:46` | Settings remain operable at ${viewport.width}×${viewport.height} | A responsive-layout assertion about settings controls staying inside a phone viewport. |
| `e2e/theme.spec.ts:34` | theme cycle preserves one palette across manual and system modes | Checks CSS custom-property resolution across matchMedia and reload; no protocol or native surface is involved. |
| `e2e/theme.spec.ts:115` | theme ${method} failure keeps the app usable and the in-memory theme active | Injects a browser Storage failure for the theme preference; purely browser-side storage and CSS behavior. |
| `e2e/theme.spec.ts:157` | blocked ${storageKind} property keeps connection, session launch, and theme usable | Blocks browser localStorage/sessionStorage to prove the client stays usable; the assertion is about browser storage behavior. |
| `e2e/visual.spec.ts:52` | connect gate matches baseline | Pure visual-regression snapshot of the connect gate; browser rendering only, with no native protocol surface. |
| `e2e/visual.spec.ts:70` | empty chooser matches baseline | Visual-regression snapshot of the workspace chooser; browser rendering only. |

## Method coverage note

`protocol_methods` uses the verified names from `docs/protocol-matrix.csv`;
`natively_handled` is derived from that matrix's `native_status` column
(`handled` counts; `decoded-only`/`absent` do not). `none` means the case
exercises no protocol method (pure client/UI behaviour).
