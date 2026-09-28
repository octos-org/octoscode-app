Create ONE original image containing 12 complete UI component screens for a desktop AI coding agent app called
"OctosCode". Generate every screen together in this single image. Do not generate separate images.

Requested quality: high
Grid: 4 columns × 3 rows, in reading order (row 1 left→right = screens 1–4, row 2 = 5–8, row 3 = 9–12)
Shared logical artboard: 406 × 776 (portrait) for every screen. Every screen is the same size, fully visible, flat,
front-on, with generous, equal gutters and no overlapping frames. Put the screen number and a 2–4 word caption ONLY in
the gutter above each screen, never inside it. No device bezels, no window chrome, no perspective, no shadows between
screens, no cropped edges.

Visual language (match a calm, professional native macOS coding app): white and near-white (#FFFFFF / #F7F7F8)
surfaces, 1 px hairline dividers (#E5E5E7), near-black text (#1D1D1F), secondary text grey (#6E6E73), one black
primary control (pill buttons and the user's message bubble are solid #000 with white text), blue only for toggles and
links (#2F6FEB), diff green (#1F883D, background #E6F4EA) and diff red (#CF222E, background #FDECEC). regular-width (NOT condensed, NOT narrow) SF Pro Text-like sans at
normal letter spacing for UI, SF-Mono-like monospace for code, paths and commands. Corner radius 12 for cards, 999 for pills. Plenty of
whitespace, quiet line icons, no gradients, no illustrations, no mascots, no emoji. All text in English, crisp and
legible for later measurement.

Fixture used consistently everywhere:
- Workspace "octos" · Local · branch "feat/steer-queue"
- Model picker label "v4-flash ▾" (always ONE line, never wrapped); permission chip "Ask for approval"
- Thread titles: "Fix steer queue drop on reconnect", "Add session fork", "Review PR #2566", "Bump octos-core to a6ea8505",
  "Why is hydrate slow?"
- Files: crates/octos-cli/src/api/ui_protocol_transport.rs (+31 −4), crates/octos-core/src/ui_protocol.rs (+9 −1),
  crates/octos-cli/tests/steer_queue.rs (+22 −0)
- Command: `cargo test -p octos-cli steer_queue` → "test result: ok. 12 passed; 0 failed"
- Approval: command `git push origin feat/steer-queue`, reason "Push the fix branch so CI can run"

Depict these 12 screens, one complete component per screen, each on a white 406×776 panel:
1. THREAD LIST: header "OctosCode ▾" with bell and search icons; "New chat" row with compose icon; the 5 thread
   titles as single-line rows, the first one selected (light grey rounded highlight); a small fork glyph on "Add session fork".
2. NEW CHAT: centered prompt "What should we build in octos?" with a small terminal-bubble icon above; at the bottom
   three context chips "octos", "Local", "feat/steer-queue"; below them the composer (see 8).
3. STREAMING TURN: a right-aligned black pill user message "Fix the steer queue so queued steers survive a reconnect";
   below, a grey activity row "Working · 12s" with a small spinner; two lines of assistant text still being written
   ending in a caret; composer at the bottom whose round send button is replaced by a black round STOP button.
4. TOOL CELLS: three stacked, collapsed tool rows, each with a small icon, a monospace summary and a status at right:
   "Read ui_protocol_transport.rs  ·  412 lines" ✓, "Search 'steer_dropped'  ·  7 matches" ✓, and one EXPANDED row
   "Ran cargo test -p octos-cli steer_queue" ✓ showing a light grey monospace output box with the last 3 lines of output
   and "test result: ok. 12 passed; 0 failed" in green.
5. INLINE APPROVAL: a card with a hairline border and a small shield icon, title "Run this command?", the command in
   a monospace box `git push origin feat/steer-queue`, the reason line in grey, and three buttons in one row: black pill
   "Approve once", outlined pill "Approve for session", text button "Deny". Keyboard hints "Y / S / N" in tiny grey.
6. USER QUESTION: a card titled "Octos needs a decision", question "Where should queued steers be persisted?", three
   radio options ("In the session ledger (recommended)", "In memory only", "Ask each time"), an optional one-line
   "Add a note" input, and a black pill "Submit answer" (enabled) with grey text "Skip".
7. EDITED FILES: card "Edited 3 files" with totals "+62 −5" in green/red, three per-file rows (path grey, filename black,
   +/− at right), a "Show diff" chevron, and at the top right "Undo ↺" as plain black text (not a link) and outlined pill "Review".
8. COMPOSER STATES: the composer shown twice, stacked. Top: idle, placeholder "Ask Octos anything", left icons "+",
   "Ask for approval" chip, right the model picker "v4-flash ▾" on one line, mic icon, black round send button.
   Bottom: while a turn runs, a queued chip above the input "1 queued · Steer now · ✕" and the typed text
   "also add a test for reconnect" with the STOP button.
9. COMPLETED ANSWER: collapsed row "Worked for 3m 4s ›"; answer text "Queued steers now survive a reconnect." with 4
   bullets containing inline grey monospace chips (`steer_dropped`, `ui_protocol_transport.rs`, `12 passed`, `a6ea8505`);
   below, a row of three small icons (copy, thumbs, share) and grey timestamp "Sep 28, 9:41 PM".
10. GOAL AND PLAN: a slim strip "Goal · Fix steer queue on reconnect · 18m" with pause/stop icons, above a plan card
   "Plan · 3 of 5" with a checklist: 3 checked done steps, 1 in-progress step with a small spinner, 1 pending step.
11. REVIEW DIFF: header "Review" with scope pill "Last turn ▾" and "+62 −5"; one file header
   "ui_protocol_transport.rs  +31 −4"; a unified diff (single column, fits 406 wide) with a single line-number gutter that increases by one per line (removed lines keep old numbers, added lines continue new numbers, never duplicated), 2 red removed lines,
   4 green added lines, and a grey folded row "412 unmodified lines".
12. SETTINGS CARD: section title "Permissions" and a grouped card with 2 rows (title, grey description, blue toggle at
   right): "Default permissions" (on), "Full access" (off); below, section "Model" with a row "Default model" and a picker
   "deepseek-v4-flash ▾".

The composer is roomy: one row of controls, 56 px tall input, nothing wraps.

Render all interface text and controls clearly enough for later measurement. Every label, button, status and data
value will be rebuilt as a native component, so keep text sharp and unambiguous.
