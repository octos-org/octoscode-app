# Web-only (bucket B) rows for operator confirmation — 47 after #41b3

Each: capability | why it's web-only (evidence). Confirm = excluded from Phase 4. Reject = becomes a Phase-4 gap.

## activity (1)
- [ ] activity: navigator model (search + all/running/failed/done counts) — web navigator internals: ActivityNavigator.tsx:42 (single-session native)

## g-autonomy2 (1)
- [ ] Foreign-session isolation: other sessions surface as activity only — web multi-session activity feed: autonomy/model.ts:243 (single-session native)

## g-composer (5)
- [ ] latest-request-wins gate: an async op may publish product state only while it owns the latest request generati — web async util internals: request-authority.ts:16-28 (native flow owns turn sequencing directly)
- [ ] stale-scope cleanup: a superseded op may still clear the loading/busy flag it installed while no newer request — web retire-if-owner rule: request-authority.ts:52-56 (no equivalent async-op registry natively)
- [ ] monotonic request token for latest-request-wins state (RequestGate) — web RequestGate: request-gate.ts:2-21
- [ ] gate invalidation on projection reset: invalidate() retires every completion owned by the reset projection — web reset semantics: request-authority.ts:30-33
- [ ] Safari-safe clipboard write: the rich ClipboardItem write starts synchronously inside the click, falling back  — browser ClipboardItem specifics: copy-conversation.ts:40-56

## g-control (1)
- [ ] Lazy peer authority load (deferred manager, authority rotation) — web lazy authority internals: lazy-peer-manager.ts:32

## g-history (6)
- [ ] Mutation lease/authority: acquire a workspace-wide lease for undo (record-scoped for rewind/fork); abort on an — web mutation-lease internals: history-binding.ts:156-227 (native has no history mutation to lease)
- [ ] Cancel/unmount safety: canonical reconciliation continues after the dialog unmounts without selecting the owne — web coordinator internals: history-coordinator.ts:200-290 (no dialog to unmount natively)
- [ ] Legacy credential cleanup: remove tokens saved by the former device-memory feature across every origin, report — web-only migration: remembered-token.ts:1-15 (no native device-memory legacy)
- [ ] Pairing link: read one-use code from the URL, strip it before first render, exchange it for a token via one un — web pairing flow: pairing.ts:61-127 (native uses the OTP email/verify flow)
- [ ] Pairing link bounded copy + loopback rule: each error kind gets its own next step; only http(s) loopback origi — web pairing copy/loopback rules: pairing.ts:19-57
- [ ] Pairing discovery: one unauthenticated GET to <origin>/pair/info, never a port range; 404 = simply unsupported — web pairing discovery: pairing.ts:207-243

## g-settings (2)
- [ ] Conversation link row: copy a saved conversation link from General settings — session-links deep-link flow is web-only (CopySessionLink.tsx:11; consistent with the g-timeline verdict)
- [ ] Profile mutation leases: modal-independent write ownership; duplicate acquisition rejected — web lease internals: profile-mutation-leases.ts:1 (no concurrent mutation UI natively)

## g-timeline (10)
- [ ] Copy conversation link from Settings — only with a confirmed session reference, action locked — web-only deep-link flow: src-web/apps/web/src/features/session-links/CopySessionLink.tsx:11 (native has no browser-URL session-link surface)
- [ ] Clipboard-denied fallback: visible readonly textarea + 'Conversation link copied' sr-status — web clipboard API specific: CopySessionLink.tsx:71 (navigator.clipboard.writeText; no web-clipboard denial path natively)
- [ ] Parse + strictly validate a saved routing tuple (workspaceRoot/profileId/sessionId; 3-part array; 4096/512/102 — browser routing-intent parser: saved-session-link.ts:11 (no saved-link concept in the native app)
- [ ] Build navigation URL: strip credential params (token/auth/access_token/api_key), set ?s=, refuse non-http(s) a — web URL object + ?s= param: create-saved-session-url.ts:8
- [ ] Open-saved-conversation confirm panel: shows server/workspace; an untrusted bookmark is a visible choice, neve — browser bookmark/confirm flow: SavedSessionLinkPanel.tsx:15 (the underlying open is native-tested: flow.rs:928 + f26_replay.rs)
- [ ] A saved link from a different workspace is refused before requesting history — link-scoping is web-link flow: SavedSessionLinkPanel.tsx:15
- [ ] Track background turn transitions and maintain an unread count; prefix the tab title '(n) title' — tab-title presentation: attention/model.ts:24 (no browser tab title natively)
- [ ] AttentionBridge loads after authentication and reports attention settings to the shell — web shell bridge: AttentionBridge.tsx:10
- [ ] Authority fencing: a lazy command factory is retired if the source/epoch changes; delayed results are dropped — web lazy-command-factory internals: inspection-binding.ts:23
- [ ] Document-scoped palette binding: update language + palette on the same document without remount or Session mut — browser document-scoped internals: preferences/model.ts:156

## session:links-resume (4)
- [ ] create a shareable saved-session URL from a confirmed routing tuple — web deep-link URL flow: create-saved-session-url.ts:8
- [ ] parse a saved session reference from a URL (reject non-web/incomplete) — web URL parser: saved-session-link.ts:11
- [ ] a saved link takes precedence over the tab's remembered conversation — web tab semantics: use-octos-session.ts:3070
- [ ] per-record driver-inventory snapshot with stable identity across selection — web per-record snapshot internals: driver-inventory-snapshot.ts:221 (native single session)

## session:list-sidebar (6)
- [ ] merge server catalog rows with tab-known refs (server title wins, recency = max) — web tab-registry merge: workspace-session-catalog.ts:107 (no tab registry natively)
- [ ] tab-known registry of opened Sessions, bounded to 100 by recency — web tab registry bounded 100 by recency: known-session-registry.ts:57 (no tabs natively)
- [ ] cap a foreground turn's owner socket as a background transport (two-phase prepare/commit) — web multi-tab transport handoff: background-turn-manager.ts:127 (native owns one connection)
- [ ] bound retained background turn transports (MAX 8) and reconnect-evict lost records — web multi-record internals: background-turn-manager.ts:16
- [ ] attention-turns feed: live + last-terminal turn per background record — web background feed: use-octos-session.ts:407 (native shows the foreground timeline + terminal-gated activity row, #32i)
- [ ] selection cleanup: switching never mutates the previous record's view — web per-record view isolation: session-record-manager.ts:442 (native has a single current-session view)

## session:store-hydrate (9)
- [ ] re-open and hydrate a parked owner without taking its cleanup authority — web parked-owner/cleanup-authority internals: active-session-runtime.ts:74 (native has a single current-session view, nothing parks)
- [ ] topic-scoped notification routing (exact or base#topic; a topicless Session is not a wildcard) — web multi-topic scoping: scope.ts:7 (native binds ONE SessionKey per connection)
- [ ] bounded in-memory draft cache with eviction — web multi-session draft cache internals: session-draft-cache.ts:35 (native keeps ONE draft — nothing to evict)
- [ ] lazy session record manager: deferred per-scope engine — web per-scope deferred engine: lazy-session-record-manager.ts:21 (native single-session)
- [ ] fresh profile-neutral web session id + bind to profile before open — web browser-session identity: session-identity.ts:10 (native opens pre-bound SessionKeys)
- [ ] per-record Session manager: one record per scope; queue/controller survive selection — web multi-record isolation: session-record-manager.ts:223 (native single current-session pointer by design)
- [ ] server-adopted Session install (adoptOnRecord) without a second open — web tab-ownership adopt flow: session-record-manager.ts:589 (native swaps store+ui on Connect per #32h)
- [ ] recovery surfacing: non-fatal notice when only other records failed — web multi-record recovery notice: use-octos-session.ts:3816 (native single record)
- [ ] useServerConnection controller — web React controller: use-server-connection.ts:26 (the loop is native flow.rs's own)

## workspace (2)
- [ ] workspace: product state container (sessions/activity/token-cost/launch) — web product-state internals: workspace/model.ts:21 (single-session native AppState by design)
- [ ] workspace: product controller (listWorkspaceSessions/deleteSession/observeTokenCost) — web React controller: use-workspace-product.ts:42 (the loop is native flow/lib's own)
