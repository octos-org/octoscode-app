# Stage A · phase 4 · board 4: four new surfaces, the widget kit, zh

Status: **waiting for operator approval**. Nothing is built from this board until it is approved.

One 3840 × 2160 image (`atlas.png`) holds:

- **Zone A, new surfaces:** parity rows 23 (diff word marks and syntax colours), 6 (/btw aside owned by its Session), 270 (peer dock in the sidebar) and 15 (skill-job status).
- **Zone B, the widget kit:** the shared widgets as the native app already draws them.
- **Zone C, Chinese variants:** key widgets in zh, set in the same kit.

`docs/ux/a27/board4-vs-native.png` puts board frames next to today's native captures (A13 diff review, A14 Skills, A18 sidebar), so you can check that the board still looks like the same app.

## How it was made

- `outer/scripts/genatlas.py phase4-new4` was run with `ATLAS_SIZE=3840x2160`, model gpt-image-2, quality high.
  - OpenAI's prompting guide says the longest edge must be *less than* 3840 px. The API still accepted 3840x2160 and returned a 3840 × 2160 PNG, so no smaller fallback size was needed.
- Three generations were made (the cap). This board keeps **generation 2** (sha256 `2fccfac8…8d287`).
  - `atlas-prompt.md` is that generation's exact prompt, and `generation.json` is its metadata.
- The two rejected images are not committed:
  - **Generation 1** (`ff3718a8…`) drew most word marks as red or green text instead of highlights. It also gave added and removed rows both line numbers, did not colour keywords, and garbled the path and the "Stop" link.
  - **Generation 3** (`3ce5012f…`) drew literal "|" separators through the diff rows, garbled the waiting peer's row ("Workng · gpt-12.4k"), and shrank the bottom-row frames.
- Generation 2 is the only one that draws row 23 correctly. Its text slips are listed under [Errata](#errata); build from this README's copy, not from those pixels.

## Region index

Boxes are pixel boxes in `atlas.png` (x0,y0 – x1,y1), accurate to about ±20 px. Row numbers are line numbers in `docs/parity-matrix.csv` (the header is line 1). Native ids are the code that exists today. "New" marks what a build has to add.

### Zone A: new surfaces

| Region | Box | What it shows | Parity rows | Web source | Native ids (exists → to add) |
|---|---|---|---|---|---|
| 1 Diff review · desktop | 30,117 – 1229,1035 | The review dialog over the dimmed app. Word-level change marks and per-file syntax colours, two files; the `.conf` file has no grammar, so it is plain text that still has word marks. | **23** (C); 22 (A) | `features/review/DiffReviewDialog.tsx`, `diff-presentation.ts:26-160`, `app/styles.css:1615-2030` | `screens/board3/diff_review.rs` `build` :216 (`b3_diff_file_{f}_head` / `_mark` / `_path`, `…_h{h}_header`, `…_l{l}` with `_prefix` and `_code`). `highlight.rs` `Tok` / `Tok::color` :50/:62 exists but is used only for code blocks. **New:** a port of `decorateDiffHunk` / `changedWords` / `canDecorateDiff`, per-token runs and word-mark fills on each `_code` line. |
| 1b Large preview fallback | 30,1051 – 1229,1257 | The bound: plain text, the line tint only, red or green text, no syntax colours, no word marks, and the note "Large preview shown as plain text. All lines are included." | **23** | `DiffReviewDialog.tsx` (`plainNotice`), `canDecorateDiff` | `diff_review.rs`. **New:** the notice row and the bound (400 lines / 40,000 characters / 2,000 per line). |
| 2 Diff review · phone | 1298,121 – 1722,1071 | The web's compact layout: a full-screen sheet with the preview id hidden. Each hunk scrolls sideways (lines clipped, a scroll thumb under the hunk). | **23** | `app/styles.css:2155-2200` (`@media` compact) | `diff_review.rs` plus A13's sideways scroll (`docs/ux/a13/diff-review-scrolled-phone.png`) |
| 3 Skills · background jobs | 1791,121 – 2191,1071 | A "Background jobs" section in the Skills dialog: Running / Queued / Done / Failed, each with the job's message. | **15** (C); 9-14 (A) | `features/skills/SkillsDialog.tsx` has no job UI. The server record is `octos-cli/src/api/skill_action_jobs.rs` `SkillActionJobRecord`. | `screens/dialog_view.rs` `skills` :663 (`dlg_skills_t_title`, `dlg_skills_scope`, `dlg_skills_t_inst_head`). Today `state.rs:506` and `octos_ui.rs:949` drop the event. **New:** a job store and the section. |
| 4 /btw aside · answering | 30,1304 – 499,2058 | The aside attached above the composer of the Session that asked. The main turn keeps working above it. | **6** (C); 2-5, 7, 8 (A) | `features/btw/BtwAsidePanel.tsx`, `lazy-btw-controller.ts:128`, `app/App.tsx:2707` (inside `composer-wrap`) | `screens/sessions.rs` `SessUi.aside_question` / `aside_answer` / `aside_state` :61-67 (one aside for the whole process today), `ASIDE_CARD` :34, `lower_screen("aside")` (only tests call it), `fluid::composer` :1087. **New:** a per-Session aside and a mounted panel. |
| 5 /btw aside · states | 572,1304 – 1154,2066 | (a) Answered, (b) collapsed to one row, (c) stale after the Session's connection changed. | **6** | `lazy-btw-controller.ts` (`STALE` / `FAILED` copy, `dismiss`, `clearSettled`) | same as 4 |
| 6 Peer dock · expanded | 1298,1124 – 1722,2092 | The dock between the session tree and the sidebar footer. Rows use the Fleet's words; an approval is threaded under the waiting row. | **270** (C); 109-113 (A); footer row 269 (C) | `features/peers/PeerDock.tsx`, `peer-row-view.ts`, `features/shell/ProductSidebar.tsx:959-967` | Slot: `lib.rs` `threads_column`, between `oc_sidebar_body` :348 and `oc_sidebar_foot` :355. Words and glyphs: `fleetview::Status::word` / `glyph` :67/:98. Actions: as `fleetview::row_card` :946 and `fleet_driver` `row_command`. **New:** the dock block. |
| 7 Peer dock · collapsed | 1791,1124 – 2191,2051 | The dock folded to one pill above the footer, with a hint for ⌥P. | **270** | `PeerDock.tsx` (`collapsed`), `formatPeerDockPill`, `App.tsx:1088-1119` (Alt+P) | `lib.rs` logs "shortcut Alt+P: no peer dock" today. **New:** fold state. |

The dimmed desktop sidebar inside frame 1 also shows the dock in its desktop place.

### Zone B: widget kit (what the app already draws)

| Group | Box | Native ids | Parity rows it serves |
|---|---|---|---|
| K1 buttons · 32 px pills | 2300,81 – 3032,222 | `screens/board3/ui.rs` `Dsl::button` :584 with `enum Btn` :912 (Primary, Outline, Secondary, Disabled, …), `Dsl::link` :647, `dialog_view::PILL_H` :54 (32 px) | every dialog |
| K2 chips · status chips | 2300,259 – 3032,455 | `Dsl::chip` :563 (20 px, radius 6). Status: `fleetview::Status` `word` / `glyph` / `tone` :67/:98/:111, `status_chip`. Stopped and Failed use "✕" (U+2715). | 110 |
| K3 text fields | 2300,485 – 3032,758 | `Dsl::input` / `input_secret` / `input_icon` :721/:742/:766 (radius 8). The eye icon and the error state: `board1_kit::Field` (`.password()`, `.error()`) :275/:338, used by the provider editor (`b1_prov_key`, `b1_prov_eye`, `b1_prov_error`) | 38, 98 |
| K4 toggles · segments | 2300,788 – 3032,1021 | `OcToggle` chrome.rs:176 (track 40×24 in a 44×32 hit area), `OcSegment` :208, `Dsl::segmented` with `Seg::Pill` / `Seg::Tab` ui.rs:831/:905, `sb_seg_ws` / `sb_seg_all`, `th_off` / `th_on` / `th_high` | 107, 137 |
| K5 radio rows | 2300,1051 – 3032,1293 | `OcRadio` chrome.rs:196, `pm_ask_radio` / `pm_ws_radio` / `pm_full_radio` (hits `perm_ask` / `perm_workspace` / `perm_full`) | 31, 271 |
| K6 tabs | 2300,1328 – 3032,1496 | `hd_tabs` → `hd_tab_chat_*` / `hd_tab_traj_*` chrome.rs:630; inventory `b3_inv_tab` (`Seg::Tab`) | 140-142, 345 |
| K7 menus | 2300,1516 – 3032,1708 | `OcWorkspaceMenu` chrome.rs:516 (`sb_menu_new` / `_rename` / `_remove`); composer `popover_open`, `option_row`, `build_models` (seats.rs) | 30, 265 |
| K8 dialog frame | 3082,81 – 3820,380 | `ui::shell_open` :997, `header`, `close_glyph` :1094 (28 × 28), `scope()` :1108 (11.5 px mono, muted), `shell_close`; the A5 family `dialog_view::IDS` (`dialog_frame`, `dialog_close`) | 328 and every board-3 dialog |
| K9 session rows | 3082,398 – 3820,752 | `thread_list` chrome.rs:341 → `SbRowTpl` (`sb_r_sel`, `sb_r_title`, `sb_r_time`); dots `sb_st_run` / `_wait` / `_fail` / `_done` / `_idle` :416-431 | 231, 242, 265, 267 |
| K10 notices | 3082,778 – 3820,1116 | `ui::failure` :287 (red lead plus a muted `<id>_detail`), `ui::banner` :1145, `TRow::Notice` (rows.rs), `board1_kit::callout`. There is no shared info / warning / error type yet. | 35, 127, 358 |
| K11 session strip | 3082,1138 – 3820,1304 | `strip::lower` strip.rs:257 → `b3_strip`, `b3_strip_model` / `_state` / `_perm` | 105, 106, 362 |
| K12 composer · seats | 3082,1334 – 3820,1506 | `fluid::composer` :1087 (`i0_composer*`, `plus_hit`, `approval_pill_hit`, `model_seat_hit`, `mic_hit`, `send_hit`); labels `permission_seat_label` / `model_seat_label` (seats.rs) | 32, 122 |
| K13 reconnect banner | 3082,1550 – 3820,1698 | `reconnect::lower` :187 → `a12_link_banner`, `_accent`, `_mark`, `_title`, `_detail`, `_retry`, `_leave` | 207 |

Tokens: the board keeps the approved boards' palette. Build with the native tokens that A18 raised for contrast:

- In `screens/board3/ui.rs` `tok` (:38-83): secondary text `#61666b`, link `#3564c6`, amber `#a35a00` on `#fff3e0`, chip `#f0f0f2`.
- In `theme.rs`: the danger fill `#d1242f`.
- Syntax colours are `highlight::Tok::color`: keyword `#b4235a`, string `#2b6f3a`, constant `#1864ab`, function `#5f3dc4`, punctuation `#495057`, comment `#5f666d`.

### Zone C: Chinese (zh)

Strings come from the web's zh catalogs (`features/preferences/zh.ts`, `features/peers/peer-copy.ts`) and the native `screens/board3/fleet_copy.rs`. The session titles are user content.

| Specimen | Box | Copy |
|---|---|---|
| Z1 dialog title | 2253,1829 – 2486,2102 | 配置档案技能 · 服务器配置档案：dsflash |
| Z2 buttons | 2516,1829 – 2738,2102 | 仅此一次批准 · 拒绝 · 取消 · 刷新 · 全部展开 |
| Z3 notice | 2752,1829 – 2981,2102 | 无法访问服务器上的文件夹。 / 请重试，或改为直接输入工作区路径。 |
| Z4 session rows | 3007,1829 – 3274,2102 | 新建会话 · 搜索会话 · rows 修复重连时引导队列丢失 / 为什么 hydrate 很慢？ |
| Z5 aside | 3315,1829 – 3527,2102 | 旁问 — /btw · 关闭 · 正在回答… · 此旁问不会保存到对话。 |
| Z6 CJK face | 3561,1829 – 3820,2102 | 等待你的批准 · 工作中, set in LXGW WenKai and then in Noto Sans SC |

CJK face: as the brief asked, the board sets Chinese in a brush-stroke regular-script face that approximates **LXGW WenKai**. The generator cannot reproduce an exact font. The native app currently uses **two** Chinese faces:

- **Noto Sans SC** (`design.rs` `cjk_face` :304 / `cjk_members` :323, files `resources/ux/NotoSansSC-*.ttf`) for the conversation and lowered cards, with WenKai loaded lazily as a fallback.
- **LXGW WenKai** as the main Chinese face in the hand-written kits (`ui::text_style` :99-117, `chrome.rs` `OcFace400/500/600` :58-69, `board1_kit::font_spaced` :75-95), which draw the dialogs, the sidebar and the Fleet.

Z6 shows both faces side by side. See open question 7.

## What each surface encodes (from the web's behaviour)

### Row 23: word marks and syntax colours in the diff review

- **Syntax colours.** Each file is coloured by its extension (`diffLanguage`), on context, removed and added lines alike. The code text keeps its token colour; only the row background is tinted. A file whose grammar is missing stays plain text but keeps its word marks (web e2e `diff-review.spec.ts:188`).
- **When word marks apply.** Only inside a change block with as many removed lines as added lines, pairing lines by position, and only when a pair shares at least 25 % of its words (`changedWords`, a word LCS of at most 160 tokens). Blocks of unequal size keep the line tint only, so the board's rows +48 and +49 have no marks.
- **How a word mark looks.** A rounded fill behind the characters: the success colour at 22 % on added rows, the error colour at 20 % on removed rows.
- **The bound.** If the preview has more than 400 lines, or more than 40,000 characters, or any line longer than 2,000 characters, **nothing** is decorated. The rule is all or nothing for the whole preview, and the note "Large preview shown as plain text. All lines are included." appears. The content is never truncated (frame 1b).
- **Layout.** On desktop the dialog is min(1080, 100%) × min(780, 100%). On phone it becomes a full-screen sheet without the preview id, and every hunk scrolls sideways (frame 2).
- **Native today.** Line tints only, no word marks, no syntax colours. `Line::mark()` in `screens/review.rs` only returns +, − or nothing.

### Row 6: /btw aside owned by the Session that asked

- **Request and answer.** `session/btw` returns `{answer, model}` in one reply. The panel shows "Answering…" and then the whole Markdown answer; there is **no token streaming** (open question 2).
- **Placement.** Inside `composer-wrap`, directly above the composer of the Session that asked. Height is min(50 % of the window, 480 px) and the panel scrolls inside. On desktop it uses the chat column width: clamp(736 px, 62vw, 1040 px) + 32.
- **Ownership.** There is one aside per retained Session.
  - Switching to another Session shows that Session's aside, or none; switching back brings the first one back.
  - If the Session's connection or authority changes while an aside is answering, it shows the stale copy. A failure shows "The aside could not be answered. Try again."
  - Once an ordinary prompt is admitted in that Session, an aside that has already answered is cleared. An empty submit dismisses the aside.
  - `/btw` with no question shows "Use /btw <question> for a temporary side answer. Nothing was sent to the model."
- **Additions on the board** (not in the web; open question 3):
  - a monospace scope line ("octos · Fix steer queue drop on reconnect") naming the Session that asked;
  - a collapse chevron, with a one-row collapsed state.
- **Copy.**
  - English: "Aside — /btw", "Close", "Answering…", "This aside is not saved to the conversation.", and the stale copy "The Session connection changed before the aside completed. Ask again when it is ready." The board splits the stale copy into a red lead and a muted cause, like the kit's notices.
  - zh: 旁问 — /btw, 关闭, 正在回答…, 此旁问不会保存到对话。

### Row 270: the peer dock in the sidebar

- **Placement.** Session tree, then the dock, then the footer (Add workspace / Fleet), as in web `ProductSidebar`. The dock is hidden while there are no peers (the web renders nothing), so no empty state is drawn.
- **Row.** Glyph, "Peer N · model", the elapsed time (the web's format, "4m12s") and "↓ tokens". The second line is the **Fleet's** status word:
  - Requested / Starting / Still starting… / Working;
  - Waiting for your approval / Waiting for your answer;
  - Finished / Stopped / Failed / Outcome unknown.
  
  The web dock shows a raw lifecycle status there instead. Glyphs follow `fleetview`: ○ ● ⚠ ✓ ✕ ?.
- **Threaded approval.** Only a row blocked on an approval grows the card: "asks to run ‹tool›" plus the target, then Approve once / Deny / Stop and the link "Approve for session". The actions are bound to that row's real pending ids; ⌥Y / ⌥N act on the focused row, and ⌥P folds the dock.
  - The web renders two overlapping action sets here: Approve / Deny from `onApprovalRespond`, plus the row actions. The board merges them into one set (open question 5).
- **Collapsed.** One pill: "Peers 3 · 1 working · ⚠ 1 waiting · 1/3 finished". This is the web's `formatPeerDockPill` ("3 · 1 live · 1/3 landed · 1 blocked · 1 done") rewritten with the Fleet's words.

### Row 15: skill-job status

- **Data.** `skill/action/job/updated` carries `{profile_id, session_id, job}`, where `job` is the server's `SkillActionJobRecord`: `job_id`, `batch_id`, `action_id`, `skill_id`, `status`, `filename`, `output`, `error`, `created_at` and `updated_at`.
- **Status mapping.** This matches the plan labels in `fleet.rs` :167-171.

  | Server status | Chip | Message line |
  |---|---|---|
  | queued | ○ Queued (blue) | none |
  | running | ● Running (green) | none |
  | succeeded | ✓ Done (grey) | the job's `output` |
  | failed | ✕ Failed (red) | lead "Couldn't finish this job.", then the `error` as a muted cause |
  | cancelled | ✕ Stopped (grey) | none |
  | abandoned (the server reports "orphaned across restart") | ✕ Stopped (grey) | proposed cause "The server restarted before this job finished." |

- **Section.** "Background jobs" sits at the top of the Skills dialog, newest first. The header counts active jobs ("1 running · 1 queued"). It shows the jobs of the Profile and Session that the dialog is scoped to.
- The web has no screen that consumes this event; this is the minimal honest one.

## Errata

The generator got these strings wrong. Build from the corrections, not from the image.

- **Frames 1 and 2:** `Duration::from_millis` is drawn with a stray glyph ("from_millsɛ"). Frame 2's clipped "redelivered" is cut by the scroll edge.
- **Frame 3:** "Queeed" should read **Queued**, and "deck-skiil" should read **deck-skill**.
- **Frame 4:** "feat/ster-queue" should read **feat/steer-queue**.
- **Frames 6 and 7:** workspace headers carry an extra chevron. Use one chevron at the left only (› collapsed, ⌄ expanded), with the count badge at the right.
- **Frames 4 and 5** are drawn shorter than a 406 × 776 artboard, and frames 6 and 7 a little taller. Every phone artboard is 406 × 776.
- **K8:** the "Refresh" pill is grey-filled natively (`Btn::Secondary`).
- **Z2:** 刷浙 should read **刷新**.
- **Z3:** 或政为 should read **或改为**.
- **Z4:** 搜案会话 should read **搜索会话**; 修复重连时引导队丢夹 should read **修复重连时引导队列丢失**; and the waiting row's ⚠ should be an amber **dot**.
- **Z5:** the outline button **关闭** is missing next to the header.

## Open design questions for the operator

1. **Phone diff review.** Should it be a full-screen sheet (the web, frame 2) or the inset modal the native app shows today (A13)?
2. **/btw streaming.** The server answers `/btw` in one reply, so the board shows "Answering…" and then the answer, not streamed text. Is that acceptable? Streaming would need a server change.
3. **/btw additions.** The web has only Close. Keep the board's collapse chevron and one-row collapsed state, and the scope line naming the Session that asked? (Keep both, keep one, or drop both.) Should the sidebar row of a Session that holds an aside get a small marker while you are on another Session? The board draws none.
4. **Peer dock height on phone.** In the phone drawer the expanded dock takes rows from the session tree. Should phone start collapsed? The web starts expanded on every size.
5. **Peer dock actions.** Is the single action set OK (Approve once · Deny · Stop + "Approve for session")? A question-blocked row ("Waiting for your answer") is not drawn; should it reuse the Fleet's answer card inside the dock?
6. **Skill jobs scope and seed.**
   - Show jobs only for the Session the dialog is scoped to (the board), or for the whole Profile?
   - The server also offers `skill/action/job/list` and `skill/action/job/read` (feature `skill.action_jobs.v1`). Should the section be seeded from the list when it opens? The native client has no list method yet, so without it the section knows only the jobs announced since connecting.
   - The "abandoned" cause copy is new.
7. **One Chinese face.** Today Chinese is drawn in Noto Sans SC in the conversation and in LXGW WenKai in dialogs, the sidebar and the Fleet. Pick one for all UI text (Z6 shows both).
   - My recommendation is Noto Sans SC: it is plain and even like Inter, and A1 chose it as the bundled face. WenKai would stay the fallback for rare characters.
   - Then the three hand-written kits would switch from WenKai to `design::cjk_members`.

## Operator decisions (2026-10-02)
- Board APPROVED as is. Build copy comes from this README (the generator misspelled some English and drew wrong CJK characters).
- Phone diff review: a full-screen sheet (as the web and this board), not the small pop-up.
- /btw aside: keep the collapsed one-row state, the asking-session line, and a sidebar marker on a Session row that holds an aside. The answer arrives in one reply ("Answering…" then the answer).
- CJK typeface: Noto Sans SC for all UI text; LXGW WenKai stays as the fallback for rare characters.
  - Done: `chrome.rs` OcFace400/500/600 (the CJK member is `design::cjk_face_path`, WenKai lazy), `board1_kit::font_spaced` and board 3 `ui::text_style` (both `design::cjk_members`). Evidence: docs/ux/a24/zh-{desktop,phone}.
- Defaults the operator did not change (the board's proposals): the dock starts folded on phone; one dock button set (Approve once, Deny, Stop, plus Approve for session); skill jobs cover the dialog's Session.
