Create ONE original image containing 12 complete UI screens for a desktop AI coding agent app called
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
links (#2F6FEB), diff green (#1F883D, background #E6F4EA) and diff red (#CF222E, background #FDECEC). the Inter typeface (regular width, generous letter spacing, NEVER condensed or narrow;
letters as wide as in Inter or SF Pro Text) for UI, SF-Mono-like monospace for code, paths and commands. Corner radius 12 for cards, 999 for pills. Plenty of
whitespace, quiet line icons, no gradients, no illustrations, no mascots, no emoji. All text in English, crisp and
legible for later measurement.

IMPORTANT: these are full screens, not chat views. Do NOT add a composer, context chips, a model picker, an "Ask for approval" chip or any footer bar to ANY screen unless the screen description below asks for it (only screens 9, 10 and 11 show a composer). Each panel contains only what its description lists.
Typography: Inter Regular/Medium at normal width, NEVER condensed, NEVER a narrow or compressed face; letter widths as in SF Pro Text.

Fixture used consistently everywhere:
- Workspace "octos" · Local · branch "feat/steer-queue"
- Model picker label "v4-flash ▾" (always ONE line, never wrapped); permission chip "Ask for approval"
- Thread titles: "Fix steer queue drop on reconnect", "Add session fork", "Review PR #2566", "Bump octos-core to a6ea8505",
  "Why is hydrate slow?"
- Files: crates/octos-cli/src/api/ui_protocol_transport.rs (+31 −4), crates/octos-core/src/ui_protocol.rs (+9 −1),
  crates/octos-cli/tests/steer_queue.rs (+22 −0)
- Command: `cargo test -p octos-cli steer_queue` → "test result: ok. 12 passed; 0 failed"
- Approval: command `git push origin feat/steer-queue`, reason "Push the fix branch so CI can run"

THE COMPOSER (identical wherever it appears, as in the approved conversation board): a white rounded card, placeholder
"Ask Octos anything" (never "Say something"), one control row below the input: "+" icon, an outlined pill "Ask for
approval", then at the right the model picker "v4-flash ▾", a mic icon and a solid black round send button with a white
arrow (white button with black arrow in dark mode). No paperclip, image or @ icons.

Depict these 12 screens, one complete screen per panel, each on a white 406×776 panel:
1. REVIEW PANEL: header "Review" with scope pill "Last turn ▾" and totals "+62 −5"; a file list of 3 rows (file icon,
   filename, +/− at right, the first selected); below it the selected file's unified diff (single line-number gutter whose numbers strictly increase by one per row and never repeat,
   2 red removed and 3 green added lines, word-level highlight on one changed word) and a grey folded row
   "⋮ 412 unmodified lines ⋮".
2. CODE REVIEW RUN: title "Code review" with a black pill "Start review"; below, a running state card "Reviewing 3 files ·
   2 specialists" with a small spinner, then 2 finding cards each with a severity chip ("High" red, "Low" grey), a file
   path in monospace, and a one-line finding.
3. GOAL: a card "Goal" with objective text "Fix steer queue on reconnect", status chip "Active", a token budget bar
   "41k of 100k", elapsed "18m", and actions: outlined pills "Pause", "Stop" and a plain "Clear goal".
4. LOOPS: title "Loops" with a "+ New loop" text action; 3 loop rows (name, schedule in grey like "every 15 min",
   status dot, and a row of small icon actions pause/resume, fire now, delete): "Run CI smoke", "Sync main", "Nightly
   review" (paused, grey).
5. MONITORS: a panel title "Monitors" inside the panel at the top; 2 monitor rows each with a command in monospace ("cargo test -q", "tail -n 50 app.log"),
   "fired 3×" / "no change", an interval "30s", and pause/delete icons; a grey empty-state line under them
   "Monitors fire when the output changes".
6. FLEET: title "Fleet · 3 peers"; peer rows grouped under a goal heading "Fix steer queue": each row with a status
   word chip ("Running", "Blocked", "Done"), a label ("tests", "docs", "review"), elapsed and token count in grey, and a
   small "Steer" text action; the "Blocked" row shows a yellow attention dot.
7. TASKS: title "Tasks"; 2 task rows (tool icon, name, status chip "Running"/"Done", elapsed); the running one expanded
   with a light grey monospace live output box (4 lines, the last ending in a caret) and an outlined pill "Cancel".
8. RESUME: title "Resume a session"; a list of 4 past sessions (title, grey "octos · 2h ago · 14 turns"), the first
   selected; a confirmation strip at the bottom "Resume “Add session fork”?" with black pill "Resume" and "Cancel".
9. ATTACHMENTS: the composer with 2 image thumbnails above the input (each with a small ✕ and a size "1.2 MB"), one
   showing an upload progress ring; a grey limit note "2 of 4 images · 20 MB max"; the usual composer controls below.
10. SIDE QUESTION: the conversation behind, and a complementary card "Aside — /btw" with the question "What does
   steer_dropped mean?" and a short 2-line answer, plus a plain "Dismiss aside" action at the card's top right.
11. DARK MODE CONVERSATION: the same conversation layout as a turn in dark mode: background #1C1C1E, surfaces #2C2C2E,
   text #F2F2F7, secondary #98989F, the user bubble white with black text, a tool row with a green ✓, answer text with
   grey inline code chips (#3A3A3C), and the composer with a white round send button.
12. DARK MODE SETTINGS: the Settings screen (Connection · Live, Workspace · octos, Profile · octos-dev, Desktop notifications toggle on,
   Copy diagnostics, red Disconnect) in the same dark palette.

Render all interface text and controls clearly enough for later measurement. Every label, button, status and data
value will be rebuilt as a native component, so keep text sharp and unambiguous.
