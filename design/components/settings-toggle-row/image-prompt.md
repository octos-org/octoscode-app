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

IMPORTANT: these are full screens, not chat views. Do NOT add a composer, context chips, a model picker, an "Ask for approval" chip or any footer bar to ANY screen unless the screen description below asks for it. Each panel contains only what its description lists.
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

Depict these 12 screens, one complete screen per panel, each on a white 406×776 panel:
1. CONNECT: title "Connect to Octos", two labelled inputs "Server" (value "http://127.0.0.1:50190") and "Access token"
   (masked dots), a small grey helper line "Stored for this server only", a full-width black pill "Connect", and a plain
   text link "Use local solo server" below.
2. CONNECT FAILED: the same form with the token field outlined in red (#CF222E) and focused, a red message under it
   "Token rejected by the server", the Server value kept, and a grey status line "Last tried 9:41 PM · Retry".
3. ONBOARDING: title "Set up a local profile", input "Profile name" = "octos-dev", a provider list with radio rows
   "DeepSeek · Official API" (selected), "Kimi Coding Plan", "GLM Coding Plan", an "API key" masked input with an eye
   icon, a grey note "Your key never leaves this machine", and black pill "Create profile".
4. WORKSPACE PICKER: title "Open a workspace", a "Server folder" row "~/home/octos" at the top with a folder icon,
   section "Recent" with 4 rows (folder icon, name bold, path grey below: "octos", "octoscode-app", "robrix2",
   "octos-web"), and at the bottom an outlined pill "Browse folders…" and a plain "New folder" text action.
5. SESSION SETTINGS: a right-side drawer look filling the panel: header "Session settings" with a close ✕; section
   "Model" with a picker "deepseek-v4-flash ▾" and grey note "Saved for this profile"; section "Permissions" with a
   segmented control "On request | On failure | Never" (first selected); section "Sandbox" read-only rows
   "Enabled · Network off · Workspace write".
6. GENERAL SETTINGS: title "Settings", grouped card rows each with a label and value at right: "Connection · Live"
   (green dot), "Workspace · octos", "Profile · octos-dev", a "Desktop notifications" row with a blue toggle (on), a row
   "Copy diagnostics" with a copy icon, and at the bottom a red text action "Disconnect".
7. MODEL SETTINGS: title "Models", a list of 3 route cards ("DeepSeek · Official API", "Kimi Coding Plan", "GLM Coding
   Plan") each with a model count "3 models" and a status dot; the first expanded to show models "deepseek-v4-flash
   (default)", "deepseek-v4-pro" and buttons outlined pill "Test route", outlined pill "Discover models".
8. COMMAND PALETTE: a floating rounded panel over a dimmed conversation: a search field "/" with typed "mo", and a
   list of 6 command rows with name in monospace and grey description: "/model  Switch model", "/monitor  Add a
   monitor", "/mode  Change permissions", "/compact  Compact context", "/btw  Ask a side question", "/resume  Resume a
   session"; the first row highlighted; keyboard hint "↑↓ to move · ↵ to run · esc" at the bottom.
9. CONTEXT PANEL: title "Context", a horizontal usage bar at 62% with label "124k of 200k tokens", a small breakdown
   list ("System 8k", "Conversation 96k", "Tools 20k"), a segmented choice "Compaction: LLM | Heuristic", and an
   outlined pill "Compact now" with grey note "Keeps the last 4 turns".
10. SKILLS: title "Skills" with a search field "Search registry", section "Installed" with 3 rows (name, version grey,
   a small "Remove" text at right): "rust-review 1.2.0", "git-helper 0.9.1", "docs-writer 2.0.0"; section "Registry"
   with 2 result rows each with an outlined pill "Install".
11. ERROR SCREEN: centered, a quiet warning icon, title "Something went wrong", grey text "OctosCode hit an unexpected
   error. Your sessions are safe on the server.", a black pill "Reload", an outlined pill "Copy diagnostics", and a
   grey link "Report issue".
12. LOADING AND RECONNECTING: top half: a slim amber banner (#FFF4E5, text #8A5A00) "Reconnecting… attempt 2 · Retry
   now" above a conversation skeleton (3 grey rounded placeholder bars); bottom half: a centered small spinner with
   "Loading session…" and a grey "Cancel" text action.

Render all interface text and controls clearly enough for later measurement. Every label, button, status and data
value will be rebuilt as a native component, so keep text sharp and unambiguous.
