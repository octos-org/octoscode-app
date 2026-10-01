Create ONE original image containing 9 complete UI screens for a desktop and mobile AI coding agent app called "OctosCode".
Generate every screen together in this single image. Do not generate separate images.

Requested quality: high
Grid: 3 columns × 3 rows, in reading order (row 1 = screens 1–3, row 2 = 4–6, row 3 = 7–9)
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

IMPORTANT: these are full screens, not chat views. Do NOT add a composer, context chips, a model picker or any footer bar to any screen.
Typography: Inter Regular/Medium at normal width, NEVER condensed; letter widths as in SF Pro Text.

Fixture: server "http://192.168.1.20:50190" (a computer on the same network), workspace "octos", profile "dsflash".

Depict these 9 screens:
1. PAIR THIS DEVICE: title "Pair with Octos", a large square camera viewfinder (light grey, rounded 12, with four corner brackets) captioned
   "Scan the pairing QR shown in Octos on your computer", below it a divider "or", an input "Paste pairing link" (placeholder
   "octos://pair?code=…"), and a black pill "Pair". A plain link "Enter server and token instead" at the bottom.
2. PAIRING…: the same title, a centred small spinner and the line "Pairing with 192.168.1.20…", a grey line "This code works once.", and a
   secondary outline pill "Cancel". No token box anywhere.
3. LINK PROBLEM: title "Pair with Octos", a light red callout (#FDECEC, red text #CF222E) "This pairing link was already used." with a grey
   second line "Ask Octos for a new code.", then the normal connect form below it with "Server" prefilled "http://192.168.1.20:50190" and an empty
   "Access token" field, and a black pill "Connect".
4. CAN'T PAIR: title "Pair with Octos", a neutral grey callout with an info icon "This server doesn't support pairing." and a second callout
   "Octos on another computer must be paired from that computer." Below: black pill "Use server and token".
5. PAIRED: a settings-style screen titled "Connection", rows "Server  192.168.1.20:50190", "Paired  Today, 9:41 PM", "Stays on this device only"
   (grey), and a red text button "Forget this device" at the bottom.
6. PROVIDER EDITOR: title "Edit provider", fields "Name" = "DeepSeek · Official API", "Base URL" = "https://api.deepseek.com/v1", "API key"
   (masked dots with an eye icon), "Models" as a list of 3 rows with checkmarks (deepseek-v4-flash (default), deepseek-v4, deepseek-chat), and two
   pills at the bottom: outline "Cancel", black "Save".
7. PROVIDER REJECTED: the same editor with the API key field outlined red and a red message "The provider rejected this key (401). Your draft is kept."
   — the key still masked, no raw error text, no key shown; the other fields keep their values; black pill "Try again".
8. CHOOSE A FOLDER: title "Choose workspace folder", a breadcrumb "/ › Users › dev › code", a list of folder rows with folder icons
   (octos, octoscode-app, notes, scratch), one row selected (light grey fill) "octoscode-app", a grey footnote "3 hidden by the server", a path box
   at the bottom "/Users/dev/code/octoscode-app", and a black pill "Use this folder".
9. FOLDER REFUSED: the same browser opened into "/ › private", a neutral callout "The server won't list this folder." with a grey next step
   "Pick another folder or type a path you can access.", an outline pill "Back to /Users/dev", and the path box below.
