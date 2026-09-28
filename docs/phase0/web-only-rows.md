# Walk rows marked web-only: operator to confirm "not applicable" (8.8 condition 5)

From `docs/walk-rows.csv` (#4). The lane marked these as browser mechanics with no native meaning. **Nothing is
dropped until the operator confirms.** Mark each ✓ (drop) or ✗ (keep, native equivalent needed).

| # | spec | case | reason | confirm |
|---|---|---|---|---|
| 1 | `e2e/connection-storage.spec.ts` | closed-tab drafts restore only for the authenticated principal and Forget clears | Unsent-draft persistence is browser localStorage/sessionStorage keyed by the connected principal; there is no  | |
| 2 | `e2e/connection-storage.spec.ts` | a delayed principal lookup preserves newly typed text and cannot restore a retir | Draft restoration is scoped to the browser-stored principal in localStorage/sessionStorage; no native surface  | |
| 3 | `e2e/connection-storage.spec.ts` | failed Forget stays visible across identity edits until saved data can actually  | Clearing saved connection data is a browser localStorage/sessionStorage operation with no native equivalent. | |
| 4 | `e2e/connection-storage.spec.ts` | denied storage getters still allow an in-memory connection and readable cleanup  | Exercises blocked browser storage getters (localStorage/sessionStorage) with an in-memory fallback; there is n | |
| 5 | `e2e/connection-storage.spec.ts` | a 51st unsent draft evicts the oldest, stays saved, and never blocks navigation | Draft capacity and eviction are browser localStorage cache mechanics with no native equivalent. | |
| 6 | `e2e/connection-storage.spec.ts` | a draft typed while identity loads is never the capacity eviction victim | Draft capacity and eviction are browser localStorage cache mechanics with no native equivalent. | |
| 7 | `e2e/copy-conversation.spec.ts` | keeps the phone header for the workspace title: no copy button there | A responsive-layout assertion about which header control renders at a phone viewport. | |
| 8 | `e2e/draft-recovery.spec.ts` | reload restores exact unsent text without dispatching it, and sent drafts stay c | Unsent-draft persistence is browser localStorage behavior with no native-client equivalent. | |
| 9 | `e2e/draft-recovery.spec.ts` | a rejected draft storage write preserves editing and warns before text can be lo | Draft-save failure handling is browser localStorage behavior with no native-client equivalent. | |
| 10 | `e2e/final-reading.spec.ts` | tree focus follows a collapsed parent and no longer references a removed search  | Pure browser focus bookkeeping: tree active-row tracking has no native counterpart. | |
| 11 | `e2e/final-reading.spec.ts` | a missing optional syntax grammar leaves readable and copyable plain code | About the web client's optional syntax-grammar asset load and its plain-code fallback; no native counterpart. | |
| 12 | `e2e/local-preferences.spec.ts` | only explicit save persists display preferences; reload restores preferences and | Display preferences live in browser local storage and draft recovery is browser-local; the workspace connectio | |
| 13 | `e2e/modal-a11y.spec.ts` | Escape closes the modal and returns focus to its opener | Pure modal focus management in the browser; no native counterpart. | |
| 14 | `e2e/modal-a11y.spec.ts` | initial focus lands on the surface's first focusable control | Pure modal focus management in the browser; no native counterpart. | |
| 15 | `e2e/modal-a11y.spec.ts` | Tab and Shift+Tab cycle focus inside the modal only | Pure modal focus management in the browser; no native counterpart. | |
| 16 | `e2e/modal-a11y.spec.ts` | background content is hidden from assistive tech while open | Pure modal background-hiding bookkeeping in the browser; no native counterpart. | |
| 17 | `e2e/onboarding.spec.ts` | validates the address without opening a socket and leaves focus on the correctio | Pure connect-screen address validation and focus handling; the case deliberately opens no socket and sends no  | |
| 18 | `e2e/onboarding.spec.ts` | explains missing authentication while preserving password privacy and IME entry | Client-side auth-field validation and masking only; no connection is established and no protocol frame is sent | |
| 19 | `e2e/pairing-link.spec.ts` | a good link connects with no token box and leaves nothing in the address | Pairing-link intake is browser-only: query-string handling plus tab and legacy storage migration, with no ws p | |
| 20 | `e2e/pairing-link.spec.ts` | an expired code and a malformed link each get their own bounded copy | Expired and malformed link handling is browser-side URL parsing and bounded copy; no protocol session is opene | |
| 21 | `e2e/pairing-link.spec.ts` | an octos origin off this computer is refused without a request | Refusing a non-local origin happens before any request, so it is browser-side origin validation with no protoc | |
| 22 | `e2e/pairing-link.spec.ts` | the paired token survives only this tab until Forget | Tab-scoped credential lifetime (session storage isolation, reload persistence and Forget eviction) is browser  | |
| 23 | `e2e/pairing-link.spec.ts` | a pairing-capable server this browser has seen offers itself once | The discovery offer is a browser-side probe of a remembered endpoint, made with no protocol session. | |
| 24 | `e2e/product.spec.ts` | deep link: ?s= restores the selected session after reload | Session selection is carried in the ?s= URL query and restored on reload; the assertion is browser URL routing | |
| 25 | `e2e/product.spec.ts` | keeps the DSH-aligned dark product shell WCAG A/AA clean | Axe-core audit of the dark shell plus CSS colour and animation inspection; pure rendering with no native meani | |
| 26 | `e2e/settings-mobile.spec.ts` | Settings remain operable at ${viewport.width}×${viewport.height} | A responsive-layout assertion about settings controls staying inside a phone viewport. | |
| 27 | `e2e/theme.spec.ts` | theme cycle preserves one palette across manual and system modes | Checks CSS custom-property resolution across matchMedia and reload; no protocol or native surface is involved. | |
| 28 | `e2e/theme.spec.ts` | theme ${method} failure keeps the app usable and the in-memory theme active | Injects a browser Storage failure for the theme preference; purely browser-side storage and CSS behavior. | |
| 29 | `e2e/theme.spec.ts` | blocked ${storageKind} property keeps connection, session launch, and theme usab | Blocks browser localStorage/sessionStorage to prove the client stays usable; the assertion is about browser st | |
| 30 | `e2e/visual.spec.ts` | connect gate matches baseline | Pure visual-regression snapshot of the connect gate; browser rendering only, with no native protocol surface. | |
| 31 | `e2e/visual.spec.ts` | empty chooser matches baseline | Visual-regression snapshot of the workspace chooser; browser rendering only. | |
