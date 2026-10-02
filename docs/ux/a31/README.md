# A31 — parity row 15: skill-job status (the Skills dialog's "Background jobs")

Design: board 4 region 3 (`design/stage-a/phase4-new4/atlas.png`, box 1791,121–2191,1071) and README
"Row 15: skill-job status"; the README copy and Errata win over the pixels ("Queued", "deck-skill"). The web has
no job UI (`SkillsDialog.tsx`), so this is the minimal honest one the board draws.

## What the user sees

At the top of the Skills dialog (under its description, above the registry search and Installed):

- **"Background jobs"**, with the active-job count on the right ("1 running · 1 queued"; nothing when none is active);
- one row per job, **newest first** (by `updated_at`): the skill and its action (mono), the file (else the input's
  file name, else the job id), the status chip and the time since the job's last change (the web's relative time);
- the README's status table: queued **○ Queued** (blue), running **● Running** (green), succeeded **✓ Done** (grey)
  with the job's `output` (first line), failed **✕ Failed** (red) with "Couldn't finish this job." and the `error`
  as a muted mono cause, cancelled **✕ Stopped** (grey), abandoned (orphaned across a restart) **✕ Stopped** (grey)
  with "The server restarted before this job finished.";
- the states around them: "Loading background jobs…", "No background jobs in this Session.", a list failure
  ("Couldn't load background jobs." + the cause), and at most 20 rows (every active job always shows; older finished
  ones are counted with the web's "(+N omitted)").

Code: `crates/octoscode-module/src/screens/skill_jobs.rs` (the read and the view model), drawn by
`screens/dialog_view.rs` `skill_jobs_section` (called from `skills`).

## Decisions

1. **Scope: the dialog's Profile AND Session** (the operator default). The Skills dialog is Profile-scoped ("Server
   Profile: dsflash"); the jobs are the active Session's under that Profile. The store keys every record by
   (Profile, Session) and refuses a record whose own `profile_id` / `session_id` disagree with the envelope that
   carried it (octos's listener only emits matching pairs), so another Session's or Profile's job never shows.
2. **Seeding (recorded here as the task asked).**
   - The native client now asks for `skill.action_jobs.v1` (after the web's 21 features,
     `octoscode-client/src/features.rs` `NATIVE_UI_FEATURES`). Without it octos never sends
     `skill/action/job/updated` to the connection (its ledger filter drops it, live and on replay) and does not
     offer `skill/action/job/list|read` (`ui_protocol_transport.rs` `skill_action_jobs_available`). Asking changes
     nothing else the native client uses (the token also lets `skill/action/invoke` run background actions, which
     the native client never calls).
   - **When the server advertises the feature** (the `session/open` reply's `supported_features`, with
     `skill/action/job/list` in `supported_methods`), opening Skills reads `skill/action/job/list
     {profile_id, session_id}` for the dialog's scope, so the section is honest after a reconnect; the
     notifications keep it live while it is open.
   - **Without the feature** (an older server) no list is read: the section shows only the jobs announced since
     connecting and says so in a muted line: "Only jobs announced since the app connected are shown; this server
     doesn't list earlier jobs."
   - `skill/action/job/read` is not used: the list carries every field.
3. **The ordering bar.** List replies (applied on the refresh task) and notifications (applied on the UI's event
   drain) race. The store keeps, per job, the greatest record by ONE total order — a terminal status first (a
   finished job never turns active again), then `updated_at` as an instant (parsed: chrono's `AutoSi` text does not
   sort by time), then the status rank, then the content — so any order of application converges, a duplicate or
   an older record changes nothing, and every reconcile is idempotent. A list reply also drops the jobs the client
   knew BEFORE its request that it no longer names (pruned by the server's 256-per-Session retention, or lost with
   its ledger) and keeps the jobs announced since (its snapshot may predate them); "before" is the store's own
   apply counter, never a clock. A reply overtaken by a newer applied reply is ignored whole.
4. **Copy and Chinese.** Every string goes through `tr()`. The web has no key for the section's copy, so its Chinese
   is native copy in `crates/octoscode-module/src/i18n/native.rs` (each entry cites its source; a test proves none is
   a web key or an alias, placeholders match, values are Chinese): 后台作业 ("job" is 作业, apart from the web's
   background tasks 后台任务), 排队中, 无法完成此作业。, 服务器在此作业完成前已重启。, …; the count pairs the web's
   "{count} 个运行中" with "{value0} 个排队中". The Skills dialog's own copy now goes through `tr()` too (the web's
   keys, and aliases of the same `SkillsDialog.tsx` controls: Skills → 配置档案技能, Installed → 已安装技能,
   Registry → 技能注册表, Install → 检查安装; the warning's native wording "on this device" with 此设备), so a Chinese
   dialog reads Chinese around its jobs (i18n ratchet 314 → 305). Chinese is drawn in Noto Sans SC (the kit's
   `design::cjk_members`).
5. **The kit, not the artboard's scale.** Chips are the kit's 20 px status chips (K2; the Fleet's glyphs ○ ● ✓ ✕
   and inks), file names the Installed rows' 13.5 px semibold, rows in the kit's bordered list card. The "✕" is
   drawn by the symbols face (Inter has no U+2715), as in the Fleet's chips.

## Evidence

- **Failing first**: `failing-first.log` — the tests on main behind compile-only stubs (commit f7e5b1a6): store 0/9,
  client 1/3, module 0/9.
- **Tests (production path)**: `crates/octoscode-module/tests/a31_skill_jobs.rs` (9: the real `Conversation`
  against a scripted server — the handshake asks for the feature after the web's list; opening Skills reads the list
  for the dialog's Profile + Session and the section shows every status newest first; an update applied before the
  list reply survives it, and an older in-flight update applied after it changes nothing; a stale list reply after a
  newer update never regresses it, and a terminal job never turns active; another Session's / Profile's job (and a
  mislabelled record) never shows; no list without the feature, the note instead; zh; the 20-row cap keeps every
  active job), `crates/octoscode-store/tests/a31_skill_jobs.rs` (9: every permutation of a job's records converges,
  instants not text, retention, list rules), `crates/octoscode-client/tests/a31_skill_jobs.rs` (3: the registry owns
  the event; `docs/cards/22-ignored.csv` lost the ignore entry).
- **Click walk**: `tools/walk/a31_skill_jobs.py` against `replay_serve --scenario skill-jobs`
  (`crates/octoscode-module/examples/replay_scenarios/skill_jobs.rs`: synthetic jobs in octos-cli's own shapes; the
  server sends two NEWER updates before the stale list reply; another Session's and another Profile's jobs; a trigger
  file sends a live transition). Phases `seeded` (the /skills palette row CLICK, the race, every status, the live
  update, the Session scope by the other Session's sidebar row CLICK), `announced` (the feature withdrawn) and `zh`.
  Results and captures: `desktop/` and `phone/` (`walk.log`, `replay.log`, PNG + scrubbed snap per step).
- **Live** (`tools/walk/a31_live_jobs.py`, `live/desktop/`, `live/phone/`): a PRIVATE octos serve (the pinned
  a6ea8505 build, a copy of the dsflash profile, a mode-600 token; started and stopped by the script, its data
  deleted, every written file scanned for the token: 0). The real `session/open` advertised `skill.action_jobs.v1`
  (octos lists it only for a client that asked) and `skill/action/job/list`; the /skills palette row CLICK sent
  `skill/action/job/list {profile_id: dsflash, session_id: dsflash:main}` and the section showed the real reply, "No
  background jobs in this Session." — 8/8 at desktop and at phone. No model turn. The phone capture is not committed:
  its header shows the private serve's folder, which lives under the operator's home (its scrubbed `walk.log` and
  `trace.jsonl` are).
- **Build**: every walk and live result above is on `HOST …/host-agent-a75a253d9ab3961b2/target/debug/octosense built
  from agent-a75a253d9ab3961b2@f61e68b5` (the branch with main merged).
- **UX** (`docs/ux-scores.csv`, area `a31`): before 3 (main has no section: `docs/ux/a14/after-*/skills-open-*.png`),
  after 9 desktop and phone for every state. Side by side: `compare-phone.png` (board | main | A31),
  `compare-desktop.png`, `compare-desktop-before-after.png`, `compare-phone-zh-announced.png`.

## Not covered

- A job created by a live octos: that needs an installed skill with a background action and `skill/action/invoke`
  (feature `skill.actions.v1`), which the native client does not implement; the job frames are built in octos-cli's
  own shapes instead (`replay_scenarios/skill_jobs.rs` cites them).
