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

Fixture: workspaces "octos" (branch "feat/steer-queue") and "octoscode-app" (branch "main"); thread titles "Fix steer queue drop on reconnect",
"Add session fork", "Review PR #2566", "Bump octos-core to a6ea8505", "Why is hydrate slow?"; server "127.0.0.1:50190"; profile "dsflash".
Mock paths use /home/user only. The session status words are: "Working…", "Thinking…", "Waiting for input", "Done", "Stopped".

Depict these 12 screens, one per parity-matrix surface group:
1. RUNTIME INVENTORY: a modal titled "Runtime inventory" with a search field "Search names, status, or tools…" and two tabs,
   "Tools" selected and "MCP servers". Upper half: a table of runtime tool rows — name in monospace, grey category, a grey
   status chip ("enabled" / "disabled"), grey "Aliases:" and "Backend:" values, a right-aligned count. Lower half: the MCP
   server list — id in monospace, transport chip ("stdio" / "http"), a status dot, a right-aligned "toolCount", a grey
   one-line summary, and a header row "connected ·" / "connecting ·" / "failed ·" with counts. Grey empty states
   "No matching tools." and "No matching servers." at the right edge. Close control top-right.
2. WORKSPACE CREATE: a modal titled "Add workspace" over a dimmed new-chat screen. A "Server's working directory" row pinned
   FIRST with the monospace path "/home/user/octos" and grey help "(path not reported)" when the server reports none. Below, a
   "Recent workspaces" list of two rows — "octos" and "octoscode-app" — each with a monospace path and a grey
   "Start a new session in …" affordance. A path field with a black "Enter" button; grey help "Choosing a folder fills the path box."
3. CREATE FOLDER: the same modal in its create state — a folder field containing "notes", a grey validation line only when the
   name is invalid, a black "Create" button, a text link "Back to workspaces", a disclosure row "· 3 existing folders" showing
   the parent listing, and a grey note "The folder browser is available only when the server advertises it." where the
   unavailable drill-in control is visibly disabled.
4. FLEET: a full peers panel. Header "Fleet" with a grey "No peers yet" when empty. One peer card: an avatar dot, the model
   name in monospace, a status chip, a right-aligned "Advanced" link, a short brief, and a black pill "Start" over a
   "Model" + "Brief" form. A second card in a waiting state: an amber dot with "Waiting for you", a single-line input
   "Enter steering text", a grey "Dismiss" link, a thin progress rule and the muted footnote "Only while working". Grey empty
   state "Open a project first" when no workspace is open, and grey "Loading models…".
5. INSPECTOR: a modal inspector with a monospace slash header "/thread", the title "Thread", a scope line
   "/home/user/octos · main" and a refresh icon. Three stacked sections: the thread graph (rows "current", "parent" and two
   named parents, each with a right-aligned depth), the approval scopes ("Session" and "Workspace" rows with blue "allowed"
   tags), and a "Copy link" button above its monospace link value. A grey "Refresh" control bottom-right.
6. REASONING EFFORT: a compact panel titled "Thinking effort" with a segmented control "Low | Medium | High | Max" ("High"
   selected) and grey help "Sets how much the model thinks before answering". Below it a toggle row "Show reasoning" ON with
   the help "Shows the model's reasoning while it works", and a second toggle "Default on for new chats" ON. Below, a
   transcript with two assistant entries, each a one-line grey summary row with a disclosure chevron — "Weighed two
   approaches", "Checked the retry path" — a right-aligned token count and duration, one of them expanded showing four short
   indented reasoning lines. A small link row above them: "Expand all" and "Collapse all".
7. RESUME CANDIDATES: a modal titled "Resume chat" over a dimmed palette. A caution banner reading "History browsing authority
   changed." with the grey line "Refresh the catalog before selecting this row." A list of three unverified candidate rows —
   thread title, workspace tag, relative time — each with an amber "unverified" chip. Selecting one shows a confirmation row
   requiring an exact match, with a disabled primary button until confirmed. A grey note "A confirmed source Session is
   required to browse history." and a locked row "This retained Session is closed."
8. SESSION STRIP: a narrow horizontal strip above a conversation, three segments split by hairline dividers — the model in the
   first ("Model not reported" in grey when absent), the activity word in the middle, and permissions in the third
   ("Permissions not reported" grey). Under it a grey line "Model, permissions, sandbox" and small states "Reconnecting" /
   "Resuming chat…" / "Handing back control…" where applicable, plus an info row "Another app is using this session".
9. TRANSCRIPT ROWS: a conversation panel showing three special entry kinds. A grey system-notice row with an info icon and the
   text "Server restarted; reconnecting the session."; a delivered-file attachment card with a file icon, the monospace name
   "report.pdf", a grey size "1.2 MiB" and outline buttons "Preview" and "Download"; and a plain user message bubble in
   solid black with white text. Each row is separated by a hairline divider, not a box.
10. ATTACHMENTS: a modal titled "Images" with four square thumbnail slots in a row, three filled with grey placeholder images
    and one empty dashed slot, and a counter "3 / 4 image slots used". Below, a scope line "· Session:" with the value
    "dsflash" and a grey limit note "20 MiB per image". Outline buttons "Choose image files", "Cancel uploads" and
    "Cancel uploads and close", plus a small remove affordance on each thumbnail.
11. HISTORY CHECKPOINTS: a modal titled "Conversation history" listing checkpoints newest-first as rows: a monospace
    checkpoint number, the relative time and a one-line grey summary, each with a right-aligned "Restore" link. The
    currently live turn marked with a black check and greyed. A grey note that restoring starts a new turn, and a text link
    "Copy as Markdown" at the bottom.
12. SESSION SWITCHER / VIM MODE: split panel. Left: a compact "Open a different session" modal listing the other sessions of
    the same workspace — thread title, workspace tag, relative time ("2m", "1h", "Yesterday") — with the current session
    marked by a black check and greyed. Right: a grey monospace key legend panel with two columns listing "Enter", "Escape",
    "w", "dd", "ciw", "yy", under a small uppercase badge "NORMAL" and a second badge "PENDING 2". Do NOT draw a composer on
    either half; the legend panel is the content.
