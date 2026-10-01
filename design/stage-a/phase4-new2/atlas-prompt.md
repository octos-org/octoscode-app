Create ONE original image containing 12 complete UI screens for a desktop AI coding agent app called "OctosCode".
Generate every screen together in this single image. Do not generate separate images.

Requested quality: high
Grid: 4 columns × 3 rows, in reading order (row 1 = screens 1–4, row 2 = 5–8, row 3 = 9–12)
Shared logical artboard: 406 × 776 (portrait) for every screen. Every screen is the same size, fully visible, flat, front-on, with generous, equal
gutters and no overlapping frames. Put the screen number and a 2–4 word caption ONLY in the gutter above each screen, never inside it. No device bezels,
no window chrome, no perspective, no shadows between screens, no cropped edges.

Visual language (match a calm, professional native macOS coding app): white and near-white (#FFFFFF / #F7F7F8)
surfaces, 1 px hairline dividers (#E5E5E7), near-black text (#1D1D1F), secondary text grey (#6E6E73), one black
primary control (pill buttons and the user's message bubble are solid #000 with white text), blue only for toggles and
links (#2F6FEB), diff green (#1F883D, background #E6F4EA) and diff red (#CF222E, background #FDECEC). the Inter typeface (regular width, generous letter spacing, NEVER condensed or narrow;
letters as wide as in Inter or SF Pro Text) for UI, SF-Mono-like monospace for code, paths and commands. Corner radius 12 for cards, 999 for pills. Plenty of
whitespace, quiet line icons, no gradients, no illustrations, no mascots, no emoji. All text in English, crisp and
legible for later measurement.

IMPORTANT: these are full screens, not chat views. Do NOT add a composer, context chips, a model picker, an "Ask for approval" chip or any footer bar to ANY screen unless the screen description below asks for it. Each panel contains only what its description lists.
Typography: Inter Regular/Medium at normal width, NEVER condensed, NEVER a narrow or compressed face; letter widths as in SF Pro Text.

IMPORTANT: these are full screens/panels, not chat views. Do NOT add a composer or footer bar unless the description says so.
Typography: Inter Regular/Medium at normal width, NEVER condensed; letter widths as in SF Pro Text.

Fixture: workspaces "octos" (branch "feat/steer-queue") and "octoscode-app" (branch "main"); thread titles "Fix steer queue drop on reconnect",
"Add session fork", "Review PR #2566", "Bump octos-core to a6ea8505", "Why is hydrate slow?"; server "127.0.0.1:50190"; profile "dsflash".

Depict these 12 screens:
1. SIDEBAR GROUPED: the left sidebar as a full panel. Header "OctosCode" + a "New chat" row with a pencil icon. A search field "Search chats".
   A small segmented control "By workspace | All" (By workspace selected) and a sort label "Recent ▾". Two workspace groups with disclosure chevrons:
   "octos" (expanded, 3 thread rows) and "octoscode-app" (expanded, 2 rows). Each thread row: title (one line, ellipsis) and a right-aligned
   relative time in grey ("2m", "1h", "Yesterday"). A "+ Add workspace" row at the bottom of the tree.
2. SIDEBAR STATUSES: the same sidebar, flat ("All" selected), showing the five session states as small leading indicators with text: a pulsing
   dot "Running", an amber dot + "Waiting for input", a grey check "Done", a red dot "Failed", a hollow circle "Idle"; each row has its relative time.
3. SIDEBAR SEARCH: the search field contains "hydrate"; one matching row "Why is hydrate slow?" with "hydrate" highlighted; below it the grey
   empty-state for the other group "No chats in octoscode-app match "hydrate"" and a plain link "Clear search".
4. WORKSPACE COLLAPSED: "octos" collapsed (chevron right, a small count badge "3"), "octoscode-app" expanded; a hover-style overflow "⋯" on the
   workspace header with a small menu "New chat here", "Rename", "Remove from sidebar".
5. COMPACT DRAWER: a narrow window (the 406 panel) with the conversation dimmed behind and the sidebar sliding in from the left as a drawer
   (88% width) with a close "×" at its top right; the drawer shows the grouped list; a thin focus ring on the first row (keyboard focus).
6. SETTINGS · GENERAL: settings panel titled "Settings" with section "General": rows "Desktop notifications" with a blue toggle ON and grey help
   "Notify when a turn needs you or finishes while OctosCode is in the background", "Theme  System ▾", "Octos server  127.0.0.1:50190 · Connected",
   and at the bottom a red-text row "Stop server…" with grey help "Shuts down Octos on this computer".
7. STOP SERVER CONFIRM: a centred modal over the dimmed settings: title "Stop the Octos server?", body "All sessions on this computer stop. Running
   turns are interrupted.", buttons outline "Cancel" and red-filled "Stop server".
8. SETTINGS · PERMISSIONS: section "Permissions": three radio presets with one-line descriptions: "Ask for approval" (selected), "Auto-approve in
   workspace", "Full access"; below, a grey readback line "Server: ask before shell, write and network" and a small "Advanced…" link.
9. SETTINGS · SANDBOX: section "Sandbox": rows "Workspace write" toggle ON, "Network access" toggle OFF, "Read outside workspace" toggle OFF,
   each with grey help; a footnote "Applies to new turns in this session".
10. NEW-SESSION DEFAULTS: a compact strip above an empty conversation: "New chat defaults · Ask for approval · Workspace write · v4-flash ▾ ·
    Thinking: On" with a small "Change" link; under it a centred placeholder "Ask Octos anything" composer (this screen MAY show the composer).
11. THINKING TOGGLE: settings section "Model" with "Model  v4-flash ▾", "Thinking" segmented "Off | On | High" (On selected) and grey help
    "Shows the model's reasoning while it works".
12. HELD BY ANOTHER CLIENT: a conversation screen with a full-width neutral banner at the top: info icon, "This session is open in OctoSense
    [remote]. You can read along; take over to send." and a black pill "Take over"; the thread content below is read-only (greyed composer).
