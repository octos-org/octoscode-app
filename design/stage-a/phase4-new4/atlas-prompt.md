Create ONE original image: a single 4K design board for a desktop AI coding agent app called "OctosCode". It holds 8 new screens (phone artboards plus one wide desktop frame), a widget-kit component sheet and Chinese (zh) variants of key widgets, all in ONE consistent visual language.
Generate everything together in this single image. Do not generate separate images.

Requested quality: high
Canvas: landscape 3840 × 2160 on a pure white background (#FFFFFF), generous outer margins, three zones separated by wide white gutters (no lines between zones):
- ZONE A "NEW SURFACES": the left part of the canvas, two rows.
  Top row, left to right: frame 1 (a WIDE desktop frame, about 1200 × 727 px, 990 × 600 proportions) with the short wide strip 1b (about 1200 × 125 px) directly under it; frame 2; frame 3.
  Bottom row, left to right: frames 4, 5, 6, 7.
- ZONE B "WIDGET KIT": the right part of the canvas, upper three quarters: one component sheet in two columns of labelled specimen groups, K1–K7 in the left column, K8–K13 in the right column.
- ZONE C "中文 ZH": the lower right, to the right of frame 7 and under the kit: one row of Chinese specimens Z1–Z6.
Frames 2–7 are phone artboards of ONE identical size: logical 406 × 776 (portrait), drawn about 470 px wide × 900 px tall, all at the same scale; the tops of the top-row frames align, and the tops of the four bottom-row frames align. Every frame is fully visible, flat, front-on, white with a 1 px #E5E5E7 border and radius 12, never overlapping.
Kit and zh specimens are drawn at that same scale (a 32 px pill in the kit is exactly as tall as a 32 px pill inside a phone frame).
Captions: each frame's number and a 2–5 word caption go ONLY in the gutter above it, never inside it (for example "1 · Diff review · desktop"). Each kit and zh group has a small uppercase grey label above it (for example "K1 BUTTONS · 32 PX PILLS").
Each zone has a small uppercase heading in its top gutter: "A · NEW SURFACES", "B · WIDGET KIT", "C · 中文 ZH". No device bezels, no window chrome, no traffic lights, no perspective, no shadows between frames, no cropped edges.

Visual language (match a calm, professional native macOS coding app; identical to the three approved OctosCode boards): white and near-white (#FFFFFF / #F7F7F8)
surfaces, 1 px hairline dividers (#E5E5E7), near-black text (#1D1D1F), secondary text grey (#6E6E73), one black
primary control (pill buttons and the user's message bubble are solid #000 with white text; the bubble is a plain rounded rectangle with no tail), blue only for toggles, selected tabs, radios and
links (#2F6FEB), diff green (#1F883D, background #E6F4EA) and diff red (#CF222E, background #FDECEC), amber only for "waiting" (#A35A00 text on #FFF3E0). The Inter typeface (regular width, generous letter spacing, NEVER condensed or narrow;
letters as wide as in Inter or SF Pro Text) for all UI text including questions and answers, SF-Mono-like monospace ONLY for code, paths, ids and commands. Corner radius 12 for cards, 16 for dialogs, 8 for text fields, 999 for pills; pills and text fields are 32 px tall.
Chips are small 20 px rounded rectangles (radius 6, 11 px medium text, no border), never pills: neutral chips are grey text on #F0F0F2; status chips are tinted (green #166534 on #E6F4EA, amber #A35A00 on #FFF3E0, blue #2F6FEB on #EAF1FD, red #C50F0F on #FDECEC, grey #6E6E73 on #F0F0F2).
Sidebar session rows show status with small DOTS (blue running, amber waiting, red failed, grey check done, hollow circle idle); the ⚠ glyph is used only in peer rows, chips and notices. Plenty of
whitespace, quiet thin line icons, no gradients, no illustrations, no mascots, no emoji. All English text crisp, correctly spelled and
legible for later measurement.
Chinese text: the LXGW WenKai typeface (霞鹜文楷: a soft regular-script face with visible brush-pen strokes, tapered ends and slanted dots, regular weight) at the same size, baseline and colour as the English beside it; never bold, never a heavy black sans (the one exception is the second line of Z6). Every Chinese character must be a real, correctly written Simplified Chinese character, exactly as given.

IMPORTANT: these are full screens and specimens, not chat views. Do NOT add a composer, context chips, a model picker or any footer bar to ANY frame unless its description asks for it (only frame 4 and kit group K12 show a composer). Each frame contains only what its description lists.
Typography: Inter Regular/Medium at normal width, NEVER condensed, NEVER a narrow or compressed face; letter widths as in SF Pro Text.

Fixture: workspaces "octos" (branch "feat/steer-queue") and "octoscode-app" (branch "main"); thread titles "Fix steer queue drop on reconnect",
"Add session fork", "Review PR #2566", "Bump octos-core to a6ea8505", "Why is hydrate slow?"; server "127.0.0.1:50190"; profile "dsflash".
Mock paths use /home/user only. Peers are labelled "Peer 1 · glm-4.6", "Peer 2 · gpt-5.4", "Peer 3 · deepseek-v4-flash".

ZONE A — depict these frames:
1. DIFF REVIEW · DESKTOP (wide frame, 990 × 600 proportions): the OctosCode window content with the app behind dimmed under a grey scrim (a sidebar with "OctosCode",
   "New chat", "Search chats" and thread rows; the conversation of "Fix steer queue drop on reconnect"). Centred on it, a white dialog (radius 16, hairline border) about 85% of the frame width:
   - header: a small grey eyebrow "Authoritative diff preview" over the title "Retry the steer queue with backoff"; at the right a monospace green "+5" and red "−3", an outline pill "Refresh" and a 28 px close "×";
   - under a hairline, a status row: grey chips "ready" and "pending_store", and a grey monospace id "0192…00f1" at the right;
   - file card 1 (radius 10, hairline border): a header with a small square badge "M", the monospace path "crates/octos-cli/src/steer_queue.rs" (file name: steer_queue.rs) and a grey "modified" at the
     right; a hunk header in blue monospace on a pale band "@@ -42,8 +42,10 @@ impl SteerQueue {"; then these monospace rows (old number | new number | mark | code). A removed row shows ONLY
     its old number (the new-number column is blank); an added row shows ONLY its new number (the old-number column is blank); line numbers are grey:
       42 | 42 |   | fn redeliver(&mut self, conn: &Conn) -> Result<()> {
       43 | 43 |   |     let pending = self.drain_pending();
       44 |    | − |     let delay = Duration::from_millis(500);              (pale red row)
          | 44 | + |     let delay = self.backoff.next(attempt);              (pale green row)
       45 | 45 |   |     for msg in pending.iter() {
       46 |    | − |         conn.send(msg)?;                                  (pale red row)
          | 46 | + |         conn.send_with_retry(msg, &self.backoff)?;        (pale green row)
       47 | 47 |   |     }
          | 48 | + |     self.backoff.reset();                                 (pale green row)
          | 49 | + |     tracing::debug!("redelivered {} messages", pending.len());   (pale green row)
       48 | 50 |   |     Ok(())
       49 | 51 |   | }
     SYNTAX COLOURS on EVERY row, context, removed and added alike: the keywords fn, let, for, in, mut and self in raspberry #B4235A; the names redeliver, drain_pending, from_millis, next,
     iter, send, send_with_retry, reset, debug!, len and Ok in purple #5F3DC4; the number 500 and the types Duration, Conn, Result in blue #1864AB; the string "redelivered {} messages" in green
     #2B6F3A; punctuation dark grey #495057; everything else near-black. The code text is NEVER recoloured red or green because a row is removed or added: only the row background is tinted.
     WORD-LEVEL CHANGE MARKS: a filled rounded highlight BEHIND the characters, like a highlighter pen, clearly darker than its row tint (#F2BEC3 on removed rows, #B4DDBF on added rows), with
     the syntax colours still visible inside it, exactly on: row 44 "Duration::from_millis" and "500"; row +44 "self.backoff.next" and "attempt"; row 46 "send"; row +46 "send_with_retry" and
     ", &self.backoff". The unpaired added rows +48 and +49 have NO word marks, only their row tint.
   - file card 2: badge "M", path "config/octos.conf", "modified", hunk "@@ -3,1 +3,1 @@"; two rows in PLAIN near-black code with no syntax colours (this file type has no grammar) that still
     carry word marks: "3 |   | − | steer.retry.delay_ms = 500" with a highlight on "500", and "  | 3 | + | steer.retry.delay_ms = 250" with a highlight on "250".
1b. LARGE PREVIEW FALLBACK (a short wide strip directly under frame 1, the same width, captioned "1b · Large preview fallback"): the same dialog's content area for a 1,240-line preview: a grey
   12 px note "Large preview shown as plain text. All lines are included." above a file header "M Cargo.lock" and three rows with row tints only, the code text itself red-tinted on the removed
   row and green-tinted on the added row, NO syntax colours and NO word marks: "1240 | 1240 |   | source = "registry+https://github.com/rust-lang/crates.io-index"",
   "1241 |  | − | version = "0.24.0"", "  | 1241 | + | version = "0.24.1"".
2. DIFF REVIEW · PHONE (phone artboard): the same review as a full-screen phone sheet (no dimmed app behind, no inset dialog): eyebrow "Authoritative diff preview", the title
   "Retry the steer queue wi…" with an ellipsis; a second header line with "+5 −3", an outline pill "Refresh" and "×"; chips "ready" and "pending_store" (no id on phone). File card 1 with
   the header "M crates/octos-cli/src/ste…" and "modified", its hunk header and the same rows, line numbers, +/− marks, syntax colours and word-mark highlights as frame 1. The rows do NOT wrap:
   they run past the card's right edge and are clipped mid-word, and a thin rounded grey horizontal scrollbar thumb (about 55% of the width) sits under the hunk to show it scrolls sideways.
   File card 2 "M config/octos.conf" below with its two marked rows.
3. SKILLS · BACKGROUND JOBS (phone artboard): the Skills dialog as a phone sheet: title "Skills", a grey monospace scope line "Server Profile: dsflash", a 28 px close "×" top-right; a grey
   paragraph "Skills are shared by this Profile, not installed on this device." Then a section "Background jobs" with a grey "1 running · 1 queued" at the right, and four job rows separated
   by hairlines; each row: the monospace "source-skill · source.import" (skill · action), the file name in grey, a status chip at the right and a grey relative time:
   - "report.pdf", green chip "● Running", "12s"
   - "notes.md", blue chip "○ Queued", "now"
   - "q3-review.pdf", grey chip "✓ Done", "2m", and a message line "Imported 14 pages"
   - "scan-007.pdf", red chip "✕ Failed", "5m", a red lead "Couldn't finish this job." and a grey cause "pdftotext exited with status 1"
   Below, a section "Installed" with two rows: "source-skill" with grey "0.3.1 · 4 tools", and "deck-skill" with grey "1.2.0 · 2 tools".
4. /BTW ASIDE · ANSWERING (phone artboard; this frame MAY show the composer): the conversation of "Fix steer queue drop on reconnect": a compact top bar with a ≡ icon, the title
   "Fix steer queue drop…" and a grey monospace "octos · feat/steer-queue"; the transcript: a black user bubble "Make redeliver survive a reconnect." and under it the main turn still
   running: a small spinner with grey "Working… · 1m 12s". Directly above the composer, the ASIDE PANEL: a near-white (#F7F7F8) card, 1 px hairline border, radius 12, padding 16: a header
   "Aside — /btw" (semibold) with a small chevron and a small outline button "Close" at the right; a grey monospace line "octos · Fix steer queue drop on reconnect" (the Session that asked);
   the question in Inter "why does redeliver drain the whole queue first?"; a tiny spinner with "Answering…"; a grey footnote "This aside is not saved to the conversation." Then the composer: a
   white rounded card with the placeholder "Ask Octos anything", a "+" button, an outline pill "Ask for approval", the model seat "v4-flash ⌄", a mic icon and a black round send button.
5. /BTW ASIDE · STATES (a normal phone artboard holding three stacked variants like a spec sheet, each with a one-line grey caption under it; no composer):
   (a) the answered panel: header "Aside — /btw" with chevron and "Close"; the scope line; the question; a three-line answer in Inter "Draining first keeps the order stable: redeliver resends
       from a snapshot, so a message that arrives mid-resend waits for the next pass." with only "redeliver" in an inline-code chip; the footnote "This aside is not saved to the conversation."
       Caption "Answered".
   (b) the collapsed aside: ONE 40 px row with a chevron "›", "Aside — /btw", the question truncated in grey "why does redeliver drain…", a grey "Answered" and a small "Close".
       Caption "Collapsed · stays with its Session".
   (c) the stale aside: header, scope line and question; a red lead "The Session connection changed before the aside completed." and a grey "Ask again when it is ready."
       Caption "Session changed while answering".
   At the bottom a grey note: "Switching Sessions hides this aside; it returns with its Session."
6. PEER DOCK · EXPANDED (phone artboard showing the left sidebar as a full panel, as on the approved sidebar board): header "OctosCode"; a plain row "New chat" with a pencil icon at its LEFT
   (no field, no border); a search field "Search chats"; a segmented control "By workspace | All" and "Recent ⌄"; workspace "octos" expanded with rows "Fix steer queue drop on reconnect"
   (selected, light grey fill, "2m"), "Add session fork" ("1h"), "Review PR #2566" ("Yesterday"); workspace "octoscode-app" collapsed with a count badge "2". Then, under a hairline, the PEER
   DOCK pinned above the footer: a header "PEERS" (small uppercase grey, letter-spaced) with a grey link "Hide peers" at the right, and three two-line peer rows:
   - a green "●", monospace "Peer 1 · glm-4.6", grey "4m12s" at the right; line two grey "Working · ↓ 12.4k"
   - an amber "⚠", monospace "Peer 2 · gpt-5.4", "1m05s"; line two amber "Waiting for your approval"; THREADED under it (indented, with a thin vertical thread line on its left): a compact card
     "asks to run shell" with the monospace command "cargo test -p octos-cli" on a light grey band; one row of three 32 px pills: black "Approve once", outline "Deny", outline "Stop"; under
     them a blue link "Approve for session" and, at the right, a tiny grey hint "⌥Y / ⌥N"
   - a grey "✓", monospace "Peer 3 · deepseek-v4-flash", "7m40s"; line two grey "Finished · ↓ 31k"
   Footer under a hairline: two stacked rows "+ Add workspace" and "✦ Fleet".
7. PEER DOCK · COLLAPSED (phone artboard, the same sidebar with the same plain "New chat" row): "octoscode-app" now expanded too (rows "Bump octos-core to a6ea8505" "2h" and
   "Why is hydrate slow?" "Yesterday"); the dock folded into ONE full-width pill row above the footer (fill #F7F7F8, hairline border, radius 8): "Peers" followed by grey
   "3 · 1 working · ⚠ 1 waiting · 1/3 finished" and a chevron at the right; under it a tiny grey hint "Show peers · ⌥P". Footer: two stacked rows "+ Add workspace" and "✦ Fleet".

ZONE B — WIDGET KIT (one sheet; every specimen exactly as the screens above draw it):
K1 BUTTONS · 32 PX PILLS: a black pill "Approve once"; outline pills "Deny" and "Refresh"; a red pill "Stop server"; a disabled light-grey pill "Start" with grey text; blue text links
   "Expand all" and "Change"; a small grey dimension mark "32" beside the row.
K2 CHIPS: neutral chips "ready", "pending_store", "low risk", "enabled", "stdio"; status chips "● Working" (green), "⚠ Waiting for your approval" (amber), "○ Requested" (blue),
   "✓ Finished" (grey fill), "✕ Stopped" (grey fill), "✕ Failed" (red); an amber outline chip "unverified"; a grey count badge "3".
K3 TEXT FIELDS: four stacked fields with small labels: "Search" — empty, a magnifier icon and the grey placeholder "Search chats"; "Folder" — filled "notes"; "API key" — masked dots with an
   eye icon; "API key" — error: a red 1.5 px outline, masked dots and the red message "The provider rejected this key (401). Your draft is kept."
K4 TOGGLES · SEGMENTS: a row "Show reasoning" with a blue toggle ON; a row "Network access" with a grey toggle OFF; segmented "By workspace | All" (a white selected segment on a light grey
   track); segmented "Off | On | High" ("On" selected).
K5 RADIO ROWS: "Ask for approval" (selected, blue radio) with grey help "Server asks before shell, write and network"; "Auto-approve in workspace" with "Approve actions in this workspace";
   "Full access" with "No prompts · full read/write/network".
K6 TABS: underline tabs "Chat" (selected: near-black with a 2 px blue underline) and "Trajectory" (grey); track tabs "Tools | MCP servers" ("Tools" selected, white on light grey).
K7 MENU: a popover (white, hairline border, radius 12, very soft shadow) with "New chat here", "Rename", "Remove from sidebar"; and a model-menu fragment: a grey group header "Deepseek",
   a row "DeepSeek V4 Flash" with a check and a grey monospace line "deepseek-v4-flash · deepseek".
K8 DIALOG FRAME: a dialog card (radius 16, hairline border): a grey monospace slash header "/permissions", the title "Remembered approvals", a grey monospace scope "Session: dsflash:main", a
   refresh icon and a 28 px close "×" with its 28 px hit circle drawn faintly and a dimension mark "28"; a grey line "Read-only server snapshot."; a grey-filled pill "Refresh" at the bottom right.
K9 SESSION ROWS: a workspace header "octos" with a down chevron "⌄" and "⋯"; five rows with leading status DOTS and right-aligned grey times: blue dot "Fix steer queue drop on reconnect" "2m"
   (selected, light grey fill), amber dot "Add session fork" "1h", grey check "Review PR #2566" "Yesterday", red dot "Bump octos-core to a6ea8505" "2h", hollow circle "Why is hydrate slow?" "Yesterday".
K10 NOTICES (plain lead + muted cause): info — a light grey callout with an ⓘ icon, "This server doesn't support pairing." and grey "Octos on another computer must be paired from that
   computer."; warning — a pale amber callout with ⚠, "History browsing authority changed." and grey "Refresh the catalog before selecting this row."; error — a pale red callout, red lead
   "Couldn't read the tools for this session." and grey "Invalid or wrong-scope tool status".
K11 SESSION STRIP: three segments in a hairline box "deepseek-v4-flash | Working… | Workspace write" with the grey caption "Model, permissions, sandbox"; a second strip "Model not reported |
   Reconnecting | Permissions not reported" with the outer two segments in grey.
K12 COMPOSER · SEATS: a composer card (white, radius 20, hairline border) with the placeholder "Ask Octos anything"; bottom row: a "+" icon, an outline pill seat "Write · Network allowed",
   the model seat "DeepSeek V4 Flash ⌄", a mic icon and a black round send button with a white arrow.
K13 RECONNECT BANNER: a white card with a 3 px blue left accent bar, a refresh icon in a pale amber rounded square, "Reconnecting to Octos" over grey "Connection lost · retry 6 · 127.0.0.1:50190",
   an outline pill "Retry now" and a text button "Disconnect".

ZONE C — 中文 ZH (Chinese variants; each specimen has a tiny grey English label above it; copy every Chinese string character by character):
Z1 "Dialog title": the title "配置档案技能", a grey monospace scope "服务器配置档案：dsflash" and a close "×".
Z2 "Buttons": a black pill "仅此一次批准", outline pills "拒绝", "取消" and "刷新", a blue link "全部展开".
Z3 "Notice": a pale red callout with the red lead "无法访问服务器上的文件夹。" (it ends with 文件夹, "folder": 文 件 夹) and the grey cause "请重试，或改为直接输入工作区路径。"
Z4 "Session rows": a plain row "新建会话" with a pencil icon at its left, a search field with the placeholder "搜索会话" (搜索 = search: 搜 索), then two rows: an amber DOT "修复重连时引导队列丢失" "2m",
   a grey check "为什么 hydrate 很慢？" "1h".
Z5 "Aside": the header "旁问 — /btw" with an outline button "关闭", a spinner with "正在回答…", and the grey footnote "此旁问不会保存到对话。"
Z6 "CJK face": the line "等待你的批准 · 工作中" set twice for comparison, each labelled in tiny grey: first "LXGW WenKai" in the brush-stroke regular-script face used above; then "Noto Sans SC"
   in a plain sans with straight, even-width strokes and square ends (like PingFang SC), visibly different from the first.
