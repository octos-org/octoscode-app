Create ONE original image containing 4 complete DESKTOP window mockups of a native macOS coding-agent app called "OctosCode".
Generate all 4 together in this single image. Grid: 2 columns × 2 rows, reading order (row 1: frames 1–2, row 2: frames 3–4).
Each frame is a full desktop app window with a 16:10 aspect (1440 × 900 logical), fully visible, flat, front-on, with generous equal
gutters. Put the frame number and a 2–4 word caption ONLY in the gutter above each frame. Each window has a thin macOS title bar with
the three traffic-light dots at the left and nothing else in it. No device bezels, no perspective, no desk, no wallpaper.

Visual language (match a calm, professional native macOS coding app): white and near-white (#FFFFFF / #F7F7F8)
surfaces, 1 px hairline dividers (#E5E5E7), near-black text (#1D1D1F), secondary text grey (#6E6E73), one black
primary control (pill buttons and the user's message bubble are solid #000 with white text), blue only for toggles and
links (#2F6FEB), diff green (#1F883D, background #E6F4EA) and diff red (#CF222E, background #FDECEC). the Inter typeface (regular width, generous letter spacing, NEVER condensed or narrow;
letters as wide as in Inter or SF Pro Text) for UI, SF-Mono-like monospace for code, paths and commands. Corner radius 12 for cards, 999 for pills. Plenty of
whitespace, quiet line icons, no gradients, no illustrations, no mascots, no emoji. All text in English, crisp and
legible for later measurement.

IMPORTANT: these are full screens, not chat views. Do NOT add a composer, context chips, a model picker, an "Ask for approval" chip or any footer bar to ANY screen unless the screen description below asks for it. Each panel contains only what its description lists.
Typography: Inter Regular/Medium at normal width, NEVER condensed, NEVER a narrow or compressed face; letter widths as in SF Pro Text.


Desktop layout rules (same in every frame): a LEFT SIDEBAR 260 px wide on #F7F7F8 with "OctosCode ▾" at the top, a "New chat" row with a
compose icon, then section labels in small grey caps ("THREADS", and where asked "GOALS", "LOOPS", "FLEET") with single-line rows; the
CENTER CONVERSATION column has content centered with a max width of 720 px and the composer docked at the bottom of that column; an
optional RIGHT PANEL 560 px wide separated by a 1 px hairline. Hover and selection use a light grey rounded highlight. Density is desktop,
not phone: 13–14 px UI text, 32–36 px rows.

Fixture used consistently everywhere:
- Workspace "octos" · Local · branch "feat/steer-queue"
- Model picker label "v4-flash ▾" (always ONE line, never wrapped); permission chip "Ask for approval"
- Thread titles: "Fix steer queue drop on reconnect", "Add session fork", "Review PR #2566", "Bump octos-core to a6ea8505",
  "Why is hydrate slow?"
- Files: crates/octos-cli/src/api/ui_protocol_transport.rs (+31 −4), crates/octos-core/src/ui_protocol.rs (+9 −1),
  crates/octos-cli/tests/steer_queue.rs (+22 −0)
- Command: `cargo test -p octos-cli steer_queue` → "test result: ok. 12 passed; 0 failed"
- Approval: command `git push origin feat/steer-queue`, reason "Push the fix branch so CI can run"

THE COMPOSER (identical everywhere): a white rounded card, placeholder "Ask Octos anything", one control row: "+" icon,
outlined pill "Ask for approval", then at the right the model picker "v4-flash ▾", a mic icon and a solid black round send button.

Depict these 4 frames:
1. CONVERSATION WITH REVIEW OPEN: sidebar with THREADS (5 titles, first selected). Center: a black user bubble "Fix the steer queue so
   queued steers survive a reconnect", a collapsed "Worked for 3m 4s ›" row, an answer paragraph with inline grey code chips, an "Edited 3
   files +62 −5" card with Undo ↺ and Review, and the composer. Right panel "Review" with scope pill "Last turn ▾" and "+62 −5", a file list
   of 3 rows (first selected), and a SIDE-BY-SIDE diff of ui_protocol_transport.rs (old on the left, new on the right, line numbers in each
   gutter, 2 red removed lines, 4 green added lines, a grey folded row "412 unmodified lines").
2. SETTINGS DRAWER: the same conversation dimmed slightly; a right drawer 420 px wide "Session settings ✕" with sections "Model"
   (picker "deepseek-v4-flash ▾", grey "Saved for this profile"), "Permissions" (segmented "On request | On failure | Never"), "Sandbox"
   (read-only "Enabled · Network off · Workspace write"), and "Context" (usage bar "124k of 200k", outlined pill "Compact now").
3. COMMAND PALETTE AND AUTONOMY SIDEBAR: sidebar shows sections GOALS ("Fix steer queue on reconnect · 18m" with a small progress ring),
   LOOPS ("Run CI smoke · every 15 min", "Nightly review · paused" in grey), FLEET ("tests · Running", "docs · Blocked" with a yellow dot,
   "review · Done"), then THREADS. Over the dimmed conversation, a centered floating command palette 560 px wide near the top: search field
   with "/" and typed "mo", 6 command rows (monospace name, grey description: /model Switch model, /monitor Add a monitor, /mode Change
   permissions, /compact Compact context, /btw Ask a side question, /resume Resume a session), first highlighted, hint row "↑↓ move · ↵ run · esc".
4. FIRST RUN: no sidebar content yet (sidebar shows only "OctosCode" and a grey "No threads yet"); the center shows a single centered card
   480 px wide "Connect to Octos" with inputs "Server" = "http://127.0.0.1:50190" and "Access token" (masked), grey "Stored for this server
   only", a full-width black pill "Connect", and a blue link "Use local solo server".

Render all interface text and controls clearly enough for later measurement. Every label, button, status and data value will be rebuilt as
a native component, so keep text sharp and unambiguous.
