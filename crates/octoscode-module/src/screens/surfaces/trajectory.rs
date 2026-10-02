//! A6 — the session TRAJECTORY (the web's `features/supervision/`:
//! `SessionTrajectory.tsx`, `TaskDetailDialog.tsx`, `model.ts`,
//! `use-supervision.ts`), drawn in the Gate-B conversation-10 language.
//!
//! The conversation header's "Chat | Trajectory" tabs (`App.tsx:2377-2403`,
//! shown only while the server advertises one of the three supervision
//! surfaces) switch the pane: the Trajectory replaces the transcript and the
//! composer (`App.tsx:2546-2557`, `:2642`) with
//! * **Session status** — `session/status/read`'s model (`title ?? model`),
//!   `permission_profile` and `health.status`, "Not reported" otherwise
//!   (`SessionTrajectory.tsx:40-75`);
//! * **Plan** — the live checklist (`plan.todos.v1`), or "No plan is active.";
//! * **Background tasks** — `task/list` rows merged with live `task/updated`
//!   (`tasksFromList` / `applyTaskUpdated`, `model.ts:84-137`), each opening
//!   the task detail when output or artifacts are readable, with Cancel on a
//!   cancellable row (`taskIsCancellable`, `model.ts:139-141`).
//!
//! The task detail (`TaskDetailDialog.tsx`) reads `task/output/read` with a
//! byte cursor (`limit_bytes: 131072`, `use-supervision.ts:178-189`), pages
//! with "Load more output" from `next_cursor` (`:251-305`), appends live
//! `task/output/delta` by UTF-8 byte offset and FAILS CLOSED on a cursor gap
//! (`appendTaskOutputDelta`, `model.ts:143-174`), and lists/reads the task's
//! artifacts with paging (`:358-475`, `appendTaskArtifactPage`, `:176-191`).
use octoscode_store::domains::task::TaskSnapshot;
use octoscode_store::Store;
use serde_json::{json, Value};

use crate::screens::board3::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

// ------------------------------------------------------------ availability

/// Which supervision surfaces the server advertises
/// (`configureCapabilities`, `use-supervision.ts:61-97`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Avail {
    pub plan: bool,
    pub task_list: bool,
    pub task_output: bool,
    pub artifacts: bool,
    pub cancel: bool,
    pub status: bool,
}

impl Avail {
    pub fn of(store: &Store) -> Avail {
        let methods = store.domains.config.supported_methods();
        let has = |m: &str| methods.iter().any(|x| x == m);
        let feature = |f: &str| store.domains.config.has_capability(f);
        Avail {
            plan: feature("plan.todos.v1"),
            task_list: has("task/list"),
            task_output: has("task/output/read"),
            artifacts: feature("harness.task_artifacts.v1")
                && has("task/artifact/list")
                && has("task/artifact/read"),
            cancel: has("task/cancel"),
            status: has("session/status/read"),
        }
    }

    /// The tabs show only while one of the three sections can render
    /// (`App.tsx:2377-2380`; the tab falls back to Chat otherwise,
    /// `:992-1006`).
    pub fn any(&self) -> bool {
        self.plan || self.task_list || self.status
    }

    /// A row opens its detail only when output or artifacts are readable
    /// (`SessionTrajectory.tsx:117-130`).
    pub fn openable(&self) -> bool {
        self.task_output || self.artifacts
    }
}

// ------------------------------------------------------------------ model

/// The row title (`tasksFromList`: `summary ?? role ?? tool_name`;
/// `applyTaskUpdated`: `summary ?? role ?? event.title`, `model.ts:88/107`).
pub fn task_title(t: &TaskSnapshot) -> String {
    t.summary
        .clone()
        .or_else(|| t.role.clone())
        .or_else(|| t.title.clone())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| if t.tool_name.is_empty() { "Task".to_owned() } else { t.tool_name.clone() })
}

/// `taskIsCancellable` (`model.ts:139-141`).
pub fn cancellable(t: &TaskSnapshot) -> bool {
    t.state == "pending" || t.state == "running"
}

/// The row's second line: the status, then the phase and the artifact count
/// when the server reported them.
pub fn task_meta(t: &TaskSnapshot) -> String {
    let mut parts: Vec<String> = vec![t.status.clone()];
    if let Some(p) = t.phase.as_deref().filter(|p| !p.trim().is_empty()) {
        parts.push(p.trim().to_owned());
    }
    if t.artifact_count > 0 {
        parts.push(format!(
            "{} artifact{}",
            t.artifact_count,
            if t.artifact_count == 1 { "" } else { "s" }
        ));
    }
    if !t.output_files.is_empty() {
        parts.push(format!("{} file{}", t.output_files.len(), if t.output_files.len() == 1 { "" } else { "s" }));
    }
    parts.join(" · ")
}

/// `session/status/read`'s three facts (`SessionTrajectory.tsx:46-72`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStatus {
    pub model: Option<String>,
    pub permission: Option<String>,
    pub health: Option<String>,
}

pub fn parse_status(v: &Value) -> RuntimeStatus {
    let model = v.get("model").and_then(|m| {
        m.get("title")
            .and_then(|t| t.as_str())
            .or_else(|| m.get("model").and_then(|t| t.as_str()))
            .map(str::to_owned)
    });
    RuntimeStatus {
        model,
        permission: v.get("permission_profile").and_then(|p| p.as_str()).map(str::to_owned),
        health: v.get("health").and_then(|h| h.get("status")).and_then(|s| s.as_str()).map(str::to_owned),
    }
}

/// One `task/output/read` page's cursor facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutputPage {
    pub next_offset: u64,
    pub total_bytes: u64,
    pub complete: bool,
    pub source: String,
}

/// One artifact record (`TaskArtifactRecord`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Artifact {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub path: Option<String>,
    pub content: Option<String>,
}

impl Artifact {
    fn from_value(v: &Value) -> Option<Artifact> {
        Some(Artifact {
            id: v.get("id")?.as_str()?.to_owned(),
            title: v.get("title").and_then(|t| t.as_str()).unwrap_or_default().to_owned(),
            kind: v.get("kind").and_then(|t| t.as_str()).unwrap_or_default().to_owned(),
            status: v.get("status").and_then(|t| t.as_str()).unwrap_or_default().to_owned(),
            path: v.get("path").and_then(|t| t.as_str()).map(str::to_owned),
            content: v.get("content").and_then(|t| t.as_str()).map(str::to_owned),
        })
    }

    /// The list's second line (`TaskDetailDialog.tsx:117-119`).
    pub fn meta(&self) -> String {
        self.path.clone().unwrap_or_else(|| format!("{} · {}", self.kind, self.status))
    }
}

/// The selected artifact's read so far (`TaskArtifactReadResult`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactPage {
    pub artifact: Artifact,
    pub content: String,
    pub has_more: bool,
    pub next_offset: Option<u64>,
}

/// `TaskDetailState` (`model.ts:26-37`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detail {
    pub active: bool,
    pub task_id: Option<String>,
    pub loading: bool,
    pub loading_more: bool,
    pub output: Option<OutputPage>,
    pub text: String,
    pub artifacts: Option<Vec<Artifact>>,
    pub selected: Option<ArtifactPage>,
    pub artifact_loading: bool,
    pub error: Option<String>,
}

/// The cursor-gap error (`model.ts:155-160`).
pub const GAP_ERROR: &str = "Live task output has a cursor gap. Load more output to resynchronize.";

/// `appendTaskOutputDelta` (`model.ts:143-174`): a delta for another task or
/// before the first read is ignored; a stale frame (ending at/before the
/// expected offset) is a no-op; a frame starting PAST it fails closed with
/// [`GAP_ERROR`] (the text is never corrupted); an overlapping frame
/// contributes only its suffix past the expected offset (UTF-8 bytes).
pub fn append_delta(detail: &mut Detail, task_id: &str, offset: u64, text: &str) -> bool {
    if detail.task_id.as_deref() != Some(task_id) {
        return false;
    }
    let Some(out) = detail.output.as_mut() else { return false };
    let expected = out.next_offset;
    let end = offset + text.len() as u64;
    if end <= expected {
        return false;
    }
    if offset > expected {
        detail.error = Some(GAP_ERROR.to_owned());
        return false;
    }
    let overlap = (expected - offset) as usize;
    // A split inside a UTF-8 sequence decodes lossily on the web; natively the
    // suffix starts at the next char boundary.
    let mut cut = overlap.min(text.len());
    while cut < text.len() && !text.is_char_boundary(cut) {
        cut += 1;
    }
    detail.text.push_str(&text[cut..]);
    out.next_offset = end;
    out.total_bytes = out.total_bytes.max(end);
    out.complete = false;
    true
}

/// `appendTaskArtifactPage` (`model.ts:176-191`): only a page of the SAME
/// artifact appends; anything else keeps the current read.
pub fn append_artifact_page(current: &mut ArtifactPage, next: ArtifactPage) {
    if current.artifact.id != next.artifact.id {
        return;
    }
    current.content.push_str(&next.content);
    current.has_more = next.has_more;
    current.next_offset = next.next_offset;
}

/// The Trajectory's UI state (`SupervisionRuntimeState` minus what the store
/// already holds: the task rows and the plan).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrajState {
    pub loading: bool,
    pub error: Option<String>,
    /// (session, status) — a read for another session never shows.
    pub status: Option<(String, RuntimeStatus)>,
    /// The session the last refresh ran for (a session switch refreshes once).
    pub refreshed_for: Option<String>,
    pub detail: Detail,
}

// ---------------------------------------------------------------- loaders

fn readable(e: &octoscode_client::ClientError) -> String {
    match e {
        octoscode_client::ClientError::Rpc { error, .. } => error.message.clone(),
        other => other.to_string(),
    }
}

/// Fold one authoritative `task/list` result into the store
/// (`tasksFromList`, `model.ts:84-102`): the session's rows are REPLACED in
/// the server's order; a result for another session is refused.
pub fn fold_task_list(store: &Store, session: &str, v: &Value) -> Result<usize, String> {
    if v.get("session_id").and_then(|s| s.as_str()) != Some(session) {
        return Err("task/list returned another session".into());
    }
    let rows: Vec<TaskSnapshot> = v
        .get("tasks")
        .and_then(|t| t.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
        .iter()
        .filter_map(|t| {
            let id = t.get("id")?.as_str()?.to_owned();
            let state = t.get("state").and_then(|s| s.as_str()).unwrap_or("unknown").to_owned();
            let s = |k: &str| t.get(k).and_then(|v| v.as_str()).map(str::to_owned);
            Some(
                TaskSnapshot::from_list_row(
                    id,
                    s("tool_name").unwrap_or_default(),
                    state.clone(),
                    s("status").unwrap_or(state),
                    None,
                    s("role"),
                    s("source"),
                    s("summary"),
                    t.get("artifact_count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    t.get("output_files")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
                        .unwrap_or_default(),
                    s("error"),
                    s("updated_at"),
                )
                .with_phase(s("current_phase"))
                .with_session(session),
            )
        })
        .collect();
    let n = rows.len();
    store.domains.task.replace_session_rows(session, rows);
    Ok(n)
}

/// `refresh` (`use-supervision.ts:99-161`): `task/list` and
/// `session/status/read`, each only when advertised, settled independently
/// (`Promise.allSettled`): one failing never hides the other; the errors are
/// joined with " · ".
pub async fn refresh(conv: &crate::flow::Conversation, st: &std::sync::Mutex<TrajState>) -> Result<String, String> {
    let store = &conv.store;
    let session = conv.session_id();
    let avail = Avail::of(store);
    if !avail.task_list && !avail.status {
        return Ok("nothing advertised".into());
    }
    {
        let mut s = st.lock().unwrap();
        s.loading = true;
        s.error = None;
        s.refreshed_for = Some(session.clone());
    }
    let client = conv.client();
    let tasks = async {
        if avail.task_list {
            Some(client.request("task/list", json!({ "session_id": session })).await)
        } else {
            None
        }
    };
    // A15 — the read names the Profile (see `board3::strip::load_status`):
    // this app's `<profile>:main` id embeds none, and a token connection's
    // fallback (`_main`) is not a configured profile.
    let profile = conv.profile();
    let status = async {
        if avail.status {
            Some(client.request("session/status/read", json!({ "session_id": session, "profile_id": profile })).await)
        } else {
            None
        }
    };
    let (tasks, status) = tokio::join!(tasks, status);
    let mut errors: Vec<String> = Vec::new();
    let mut listed = 0usize;
    if let Some(r) = tasks {
        match r {
            Ok(v) => match fold_task_list(store, &session, &v) {
                Ok(n) => listed = n,
                Err(e) => errors.push(e),
            },
            Err(e) => errors.push(readable(&e)),
        }
    }
    let mut status_row: Option<RuntimeStatus> = None;
    if let Some(r) = status {
        match r {
            Ok(v) if v.get("session_id").and_then(|s| s.as_str()) == Some(session.as_str()) => {
                status_row = Some(parse_status(&v));
            }
            Ok(_) => errors.push("session/status/read returned another session".into()),
            Err(e) => errors.push(readable(&e)),
        }
    }
    {
        let mut s = st.lock().unwrap();
        // A refresh for a session that is no longer active folds nothing
        // into the view (the web's stale-request guard, `:131-137`).
        if conv.session_id() != session {
            s.loading = false;
            return Ok("stale".into());
        }
        s.loading = false;
        s.error = (!errors.is_empty()).then(|| errors.join(" · "));
        if let Some(r) = status_row {
            s.status = Some((session.clone(), r));
        }
    }
    Ok(format!("{listed} task(s), {} error(s)", errors.len()))
}

/// `openTaskDetail` (`use-supervision.ts:163-249`): the first output page
/// (`limit_bytes: 131072`) and the artifact list, settled independently and
/// identity-checked (`session_id` + `task_id`).
pub async fn open_task(
    conv: &crate::flow::Conversation,
    st: &std::sync::Mutex<TrajState>,
    task_id: String,
) -> Result<String, String> {
    let session = conv.session_id();
    let avail = Avail::of(&conv.store);
    if !avail.task_list || !avail.openable() || task_id.is_empty() {
        return Err("task detail is not offered by this server".into());
    }
    {
        let mut s = st.lock().unwrap();
        s.detail = Detail { active: true, task_id: Some(task_id.clone()), loading: true, ..Default::default() };
    }
    let client = conv.client();
    let output = async {
        if avail.task_output {
            Some(
                client
                    .request(
                        "task/output/read",
                        json!({ "session_id": session, "task_id": task_id, "limit_bytes": 131_072 }),
                    )
                    .await,
            )
        } else {
            None
        }
    };
    let artifacts = async {
        if avail.artifacts {
            Some(client.request("task/artifact/list", json!({ "session_id": session, "task_id": task_id })).await)
        } else {
            None
        }
    };
    let (output, artifacts) = tokio::join!(output, artifacts);
    let mut s = st.lock().unwrap();
    if s.detail.task_id.as_deref() != Some(task_id.as_str()) || conv.session_id() != session {
        return Ok("stale".into());
    }
    let mut errors: Vec<String> = Vec::new();
    s.detail.loading = false;
    if let Some(r) = output {
        match r {
            Ok(v) if same_task(&v, &session, &task_id) => {
                s.detail.text = v.get("text").and_then(|t| t.as_str()).unwrap_or_default().to_owned();
                s.detail.output = Some(output_page(&v));
            }
            Ok(_) => errors.push("task/output/read returned a mismatched task".into()),
            Err(e) => errors.push(readable(&e)),
        }
    }
    if let Some(r) = artifacts {
        match r {
            Ok(v) if same_task(&v, &session, &task_id) => {
                s.detail.artifacts = Some(
                    v.get("artifacts")
                        .and_then(|a| a.as_array())
                        .map(|a| a.iter().filter_map(Artifact::from_value).collect())
                        .unwrap_or_default(),
                );
            }
            Ok(_) => errors.push("task/artifact/list returned a mismatched task".into()),
            Err(e) => errors.push(readable(&e)),
        }
    }
    s.detail.error = (!errors.is_empty()).then(|| errors.join(" · "));
    Ok(format!("output {} bytes, {} artifact(s)", s.detail.text.len(), s.detail.artifacts.as_ref().map(Vec::len).unwrap_or(0)))
}

fn same_task(v: &Value, session: &str, task_id: &str) -> bool {
    v.get("session_id").and_then(|s| s.as_str()) == Some(session)
        && v.get("task_id").and_then(|s| s.as_str()) == Some(task_id)
}

fn output_page(v: &Value) -> OutputPage {
    OutputPage {
        next_offset: v.pointer("/next_cursor/offset").and_then(|o| o.as_u64()).unwrap_or(0),
        total_bytes: v.get("total_bytes").and_then(|o| o.as_u64()).unwrap_or(0),
        complete: v.get("complete").and_then(|c| c.as_bool()).unwrap_or(true),
        source: v.get("source").and_then(|c| c.as_str()).unwrap_or_default().to_owned(),
    }
}

/// `loadMoreTaskOutput` (`use-supervision.ts:251-305`): the next page from
/// `next_cursor`, appended.
pub async fn load_more(conv: &crate::flow::Conversation, st: &std::sync::Mutex<TrajState>) -> Result<String, String> {
    let session = conv.session_id();
    let (task_id, cursor) = {
        let mut s = st.lock().unwrap();
        let d = &mut s.detail;
        let (Some(task_id), Some(out)) = (d.task_id.clone(), d.output.as_ref()) else {
            return Err("no task output to page".into());
        };
        if out.complete || d.loading_more {
            return Ok("nothing to load".into());
        }
        let c = out.next_offset;
        d.loading_more = true;
        d.error = None;
        (task_id, c)
    };
    let r = conv
        .client()
        .request(
            "task/output/read",
            json!({ "session_id": session, "task_id": task_id, "cursor": {"offset": cursor}, "limit_bytes": 131_072 }),
        )
        .await;
    let mut s = st.lock().unwrap();
    s.detail.loading_more = false;
    match r {
        Ok(v) if same_task(&v, &session, &task_id) && s.detail.task_id.as_deref() == Some(task_id.as_str()) => {
            s.detail.text.push_str(v.get("text").and_then(|t| t.as_str()).unwrap_or_default());
            s.detail.output = Some(output_page(&v));
            s.detail.error = None;
            Ok(format!("{} bytes", s.detail.text.len()))
        }
        Ok(_) => Ok("stale".into()),
        Err(e) => {
            s.detail.error = Some(readable(&e));
            Err(readable(&e))
        }
    }
}

/// `cancelTask` (`use-supervision.ts:307-356`): optimistic `cancelling`, the
/// server's status on success, the previous row back on failure.
pub async fn cancel(conv: &crate::flow::Conversation, st: &std::sync::Mutex<TrajState>, task_id: String) -> Result<String, String> {
    let store = &conv.store;
    let session = conv.session_id();
    let Some(before) = store.domains.task.snapshot(&task_id) else {
        return Err("unknown task".into());
    };
    if !Avail::of(store).cancel || !cancellable(&before) {
        return Err("this task cannot be cancelled".into());
    }
    let mut busy = before.clone();
    busy.state = "cancelling".into();
    busy.status = "cancelling".into();
    store.domains.task.upsert_snapshot(busy);
    super::wake();
    match conv.client().request("task/cancel", json!({ "task_id": task_id, "session_id": session })).await {
        Ok(v) if v.get("task_id").and_then(|t| t.as_str()) == Some(task_id.as_str()) => {
            let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("cancelled").to_owned();
            let mut done = before;
            done.state = status.clone();
            done.status = status.clone();
            store.domains.task.upsert_snapshot(done);
            Ok(status)
        }
        Ok(_) => {
            store.domains.task.upsert_snapshot(before);
            Ok("stale".into())
        }
        Err(e) => {
            store.domains.task.upsert_snapshot(before);
            st.lock().unwrap().error = Some(readable(&e));
            Err(readable(&e))
        }
    }
}

/// `readTaskArtifact` (`use-supervision.ts:358-418`): inline content shows
/// at once; otherwise `task/artifact/read` (`limit_bytes: 262144`).
pub async fn read_artifact(conv: &crate::flow::Conversation, st: &std::sync::Mutex<TrajState>, index: usize) -> Result<String, String> {
    let session = conv.session_id();
    let (task_id, artifact) = {
        let s = st.lock().unwrap();
        let Some(task_id) = s.detail.task_id.clone() else { return Err("no task".into()) };
        let Some(a) = s.detail.artifacts.as_ref().and_then(|l| l.get(index)).cloned() else {
            return Err("no such artifact".into());
        };
        (task_id, a)
    };
    if let Some(content) = artifact.content.clone() {
        st.lock().unwrap().detail.selected =
            Some(ArtifactPage { artifact, content, has_more: false, next_offset: None });
        return Ok("inline".into());
    }
    {
        let mut s = st.lock().unwrap();
        s.detail.artifact_loading = true;
        s.detail.error = None;
    }
    let r = conv
        .client()
        .request(
            "task/artifact/read",
            json!({ "session_id": session, "task_id": task_id, "artifact_id": artifact.id, "limit_bytes": 262_144 }),
        )
        .await;
    let mut s = st.lock().unwrap();
    s.detail.artifact_loading = false;
    match r {
        Ok(v) if same_task(&v, &session, &task_id) => {
            let page = artifact_page(&v, &artifact);
            let n = page.content.len();
            s.detail.selected = Some(page);
            Ok(format!("{n} bytes"))
        }
        Ok(_) => Ok("stale".into()),
        Err(e) => {
            s.detail.error = Some(readable(&e));
            Err(readable(&e))
        }
    }
}

fn artifact_page(v: &Value, fallback: &Artifact) -> ArtifactPage {
    let artifact = v.get("artifact").and_then(Artifact::from_value).unwrap_or_else(|| fallback.clone());
    ArtifactPage {
        content: v
            .get("content")
            .and_then(|c| c.as_str())
            .map(str::to_owned)
            .or_else(|| artifact.content.clone())
            .unwrap_or_default(),
        has_more: v.get("has_more").and_then(|b| b.as_bool()).unwrap_or(false),
        next_offset: v.pointer("/next_cursor/offset").and_then(|o| o.as_u64()),
        artifact,
    }
}

/// `loadMoreTaskArtifact` (`use-supervision.ts:420-475`).
pub async fn more_artifact(conv: &crate::flow::Conversation, st: &std::sync::Mutex<TrajState>) -> Result<String, String> {
    let session = conv.session_id();
    let (task_id, sel, cursor) = {
        let mut s = st.lock().unwrap();
        let Some(task_id) = s.detail.task_id.clone() else { return Err("no task".into()) };
        let Some(sel) = s.detail.selected.clone() else { return Err("no artifact".into()) };
        let (true, Some(c)) = (sel.has_more, sel.next_offset) else { return Ok("complete".into()) };
        if s.detail.artifact_loading {
            return Ok("busy".into());
        }
        s.detail.artifact_loading = true;
        s.detail.error = None;
        (task_id, sel, c)
    };
    let r = conv
        .client()
        .request(
            "task/artifact/read",
            json!({ "session_id": session, "task_id": task_id, "artifact_id": sel.artifact.id,
                    "cursor": {"offset": cursor}, "limit_bytes": 262_144 }),
        )
        .await;
    let mut s = st.lock().unwrap();
    s.detail.artifact_loading = false;
    match r {
        Ok(v) if same_task(&v, &session, &task_id) => {
            let next = artifact_page(&v, &sel.artifact);
            if let Some(cur) = s.detail.selected.as_mut() {
                append_artifact_page(cur, next);
            }
            Ok("appended".into())
        }
        Ok(_) => Ok("stale".into()),
        Err(e) => {
            s.detail.error = Some(readable(&e));
            Err(readable(&e))
        }
    }
}

// ------------------------------------------------------------------- view

fn section_title(d: &mut Dsl, id: &str, label: &str, count: Option<usize>) {
    let row = d.anon();
    d.view(&row, "width: Fill height: 26 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    d.text(id, label, &Txt::new(13.5, Face::Semibold, tok::TEXT).w(W::Fill));
    if let Some(n) = count {
        d.chip(&format!("{id}_count"), &n.to_string(), tok::MUTED, tok::CHIP, None, false);
    }
    d.close();
}

fn empty_box(d: &mut Dsl, id: &str, text: &str) {
    d.surface(
        &format!("{id}_box"),
        "width: Fill height: Fit flow: Down padding: Inset{left: 14 right: 14 top: 14 bottom: 14}",
        tok::SURFACE,
        12.0,
        Some("#d9d9dcff"),
    );
    d.text(id, text, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.close();
}

fn state_color(state: &str) -> &'static str {
    match state {
        "running" | "pending" | "cancelling" => tok::BLUE,
        "failed" | "cancelled" => tok::RED,
        "completed" => tok::GREEN,
        _ => tok::FAINT,
    }
}

/// The Trajectory pane (`SessionTrajectory.tsx`), centred at the web's
/// `min(100%, 760px)` with the pane's gutters.
pub fn pane(d: &mut Dsl, store: &Store, st: &TrajState, width: f64, phone: bool) {
    let avail = Avail::of(store);
    let session = store.active_session().unwrap_or_default();
    let col_w = (width - if phone { 32.0 } else { 48.0 }).min(760.0).max(200.0).floor();
    d.view("cv_tr_root", "width: Fill height: Fill flow: Down align: Align{x: 0.5 y: 0.0}");
    d.open(
        "cv_tr_scroll",
        "ScrollYView",
        "width: Fill height: Fill flow: Down align: Align{x: 0.5 y: 0.0} padding: Inset{left: 0 top: 0 right: 0 bottom: 0}",
    );
    d.view(
        "cv_tr_col",
        &format!(
            "width: {col_w} height: Fit flow: Down spacing: 0 padding: Inset{{left: 0 right: 0 top: {} bottom: 48}}",
            if phone { 14.0 } else { 20.0 }
        ),
    );
    // Header: title + subtitle, Refresh on the right.
    let head = d.anon();
    d.view(&head, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 12");
    let tcol = d.anon();
    d.view(&tcol, "width: Fill height: Fit flow: Down spacing: 4");
    d.text("cv_tr_title", "Trajectory", &Txt::new(if phone { 19.0 } else { 21.0 }, Face::Semibold, tok::TEXT).w(W::Fill));
    d.text(
        "cv_tr_sub",
        "Plan and background work for this session.",
        &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    d.close();
    if avail.task_list || avail.status {
        let label = if st.loading { "Refreshing…" } else { "Refresh" };
        d.button("cv_tr_refresh", label, "cv.traj.refresh", if st.loading { Btn::Disabled } else { Btn::Outline }, W::Fit, 32.0);
    }
    d.close();
    if let Some(err) = &st.error {
        d.gap(W::Fill, 14.0);
        d.surface(
            "cv_tr_error_box",
            "width: Fill height: Fit flow: Down padding: Inset{left: 12 right: 12 top: 10 bottom: 10}",
            tok::RED_BG,
            10.0,
            None,
        );
        d.text("cv_tr_error", err, &Txt::new(12.5, Face::Regular, tok::RED).w(W::Fill).wrap());
        d.close();
    }
    // Session status.
    if avail.status {
        d.gap(W::Fill, 22.0);
        section_title(d, "cv_tr_status_title", "Session status", None);
        d.gap(W::Fill, 8.0);
        let status = st.status.as_ref().filter(|(s, _)| s == &session).map(|(_, r)| r.clone());
        match status {
            Some(r) => {
                ui::card_open(d, "cv_tr_status", 0.0);
                for (i, (label, value)) in [
                    ("Model", r.model.clone()),
                    ("Permission", r.permission.clone()),
                    ("Health", r.health.clone()),
                ]
                .iter()
                .enumerate()
                {
                    if i > 0 {
                        d.hairline();
                    }
                    let row = d.anon();
                    d.view(&row, "width: Fill height: 34 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 12");
                    d.text(&format!("cv_tr_st_label_{i}"), label, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill));
                    d.text(
                        &format!("cv_tr_st_value_{i}"),
                        value.as_deref().unwrap_or("Not reported"),
                        &Txt::new(12.5, Face::Medium, if value.is_some() { tok::TEXT } else { tok::FAINT }),
                    );
                    d.close();
                }
                d.close();
            }
            None => empty_box(d, "cv_tr_status_empty", "Session status has not loaded yet."),
        }
    }
    // Plan.
    if avail.plan {
        d.gap(W::Fill, 22.0);
        let plan = store.domains.task.plan(&session).filter(|p| !p.items.is_empty());
        section_title(d, "cv_tr_plan_title", "Plan", Some(plan.as_ref().map(|p| p.items.len()).unwrap_or(0)));
        d.gap(W::Fill, 8.0);
        match plan {
            Some(p) => {
                let list = d.anon();
                d.view(&list, "width: Fill height: Fit flow: Down spacing: 6");
                for (i, item) in p.items.iter().enumerate() {
                    d.surface(
                        &format!("cv_tr_plan_{i}"),
                        "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{left: 12 right: 12 top: 11 bottom: 11}",
                        tok::SURFACE,
                        12.0,
                        Some(tok::HAIRLINE),
                    );
                    d.icon(&format!("cv_tr_plan_mark_{i}"), super::plan::mark(&item.status), 20.0, tok::BLUE);
                    d.text(
                        &format!("cv_tr_plan_title_{i}"),
                        item.title.trim(),
                        &Txt::new(13.5, if item.status == "in_progress" { Face::Medium } else { Face::Regular }, tok::TEXT)
                            .w(W::Fill)
                            .wrap(),
                    );
                    if let Some(pr) = item.priority.as_deref().filter(|p| !p.trim().is_empty()) {
                        d.chip(&format!("cv_tr_plan_prio_{i}"), pr.trim(), tok::MUTED, tok::SURFACE, Some(tok::HAIRLINE), false);
                    }
                    d.text(
                        &format!("cv_tr_plan_status_{i}"),
                        super::plan::status_label(&item.status),
                        &Txt::new(11.5, Face::Regular, tok::FAINT),
                    );
                    d.close();
                }
                d.close();
            }
            None => empty_box(d, "cv_tr_plan_empty", "No plan is active."),
        }
    }
    // Background tasks.
    if avail.task_list {
        d.gap(W::Fill, 22.0);
        let rows = store.domains.task.session_rows(&session);
        section_title(d, "cv_tr_tasks_title", "Background tasks", Some(rows.len()));
        d.gap(W::Fill, 8.0);
        if rows.is_empty() {
            empty_box(d, "cv_tr_tasks_empty", "No background tasks in this session.");
        } else {
            let list = d.anon();
            d.view(&list, "width: Fill height: Fit flow: Down spacing: 6");
            for (i, t) in rows.iter().enumerate() {
                task_row(d, i, t, &avail, col_w);
            }
            d.close();
        }
    }
    d.close(); // col
    d.close(); // scroll
    d.close(); // root
}

fn task_row(d: &mut Dsl, i: usize, t: &TaskSnapshot, avail: &Avail, col_w: f64) {
    d.surface(
        &format!("cv_tr_task_{i}"),
        "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 4 right: 6 top: 4 bottom: 4}",
        tok::SURFACE,
        12.0,
        Some(tok::HAIRLINE),
    );
    // The body: a tap target when the detail can open (`SessionTrajectory.tsx:117-130`).
    d.view(&format!("cv_tr_task_body_{i}"), "width: Fill height: Fit flow: Overlay");
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{left: 10 right: 6 top: 8 bottom: 8}");
    d.dot(state_color(&t.state), 8.0);
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 2");
    let budget = col_w - 150.0;
    d.text(
        &format!("cv_tr_task_title_{i}"),
        &ui::fit_w(&task_title(t), budget, 13.5, Face::Medium),
        &Txt::new(13.5, Face::Medium, tok::TEXT).w(W::Fill),
    );
    d.text(
        &format!("cv_tr_task_meta_{i}"),
        &ui::fit_w(&task_meta(t), budget, 11.5, Face::Regular),
        &Txt::new(11.5, Face::Regular, tok::MUTED).w(W::Fill),
    );
    if let Some(err) = t.error.as_deref().filter(|e| !e.trim().is_empty()) {
        d.text(&format!("cv_tr_task_err_{i}"), err.trim(), &Txt::new(11.5, Face::Regular, tok::RED).w(W::Fill).wrap());
    }
    d.close();
    if avail.openable() {
        d.icon(&format!("cv_tr_task_chev_{i}"), "b3_chevron_right.svg", 14.0, tok::MUTED);
    }
    d.close();
    if avail.openable() {
        d.tap(&format!("cv_tr_task_open_{i}"), &format!("cv.task.open#{i}"));
    }
    d.close();
    if avail.cancel && cancellable(t) {
        d.button(&format!("cv_tr_task_cancel_{i}"), "Cancel", &format!("cv.task.cancel#{i}"), Btn::Outline, W::Fit, 30.0);
    } else if t.state == "cancelling" {
        d.button(&format!("cv_tr_task_cancel_{i}"), "Cancelling…", "cv.noop", Btn::Disabled, W::Fit, 30.0);
    }
    d.close();
}

/// The task detail dialog (`TaskDetailDialog.tsx`): output and artifacts
/// side by side on a desktop window, stacked on a phone.
pub fn detail_dialog(d: &mut Dsl, store: &Store, st: &TrajState, frame: &Frame) {
    let avail = Avail::of(store);
    let det = &st.detail;
    let task = det.task_id.as_deref().and_then(|id| store.domains.task.snapshot(id));
    let width = frame.dialog_w(if avail.task_output && avail.artifacts { 880.0 } else { 680.0 });
    let compact = frame.compact(width);
    ui::shell_open(d, frame, width);
    // Header: eyebrow, title, the state pill and the close glyph.
    let head = d.anon();
    d.view(&head, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.0} spacing: 10");
    let tcol = d.anon();
    d.view(&tcol, "width: Fill height: Fit flow: Down spacing: 3");
    d.text("cv_td_eyebrow", "SUPERVISED TASK", &Txt::new(11.0, Face::Medium, tok::BLUE).w(W::Fill));
    let title = task.as_ref().map(task_title).unwrap_or_else(|| "Task output".into());
    d.text("cv_td_title", &ui::fit_w(&title, width - 180.0, 17.0, Face::Semibold), &Txt::new(17.0, Face::Semibold, tok::TEXT).w(W::Fill));
    d.close();
    let state = task.as_ref().map(|t| t.state.clone()).unwrap_or_else(|| "loading".into());
    let (fg, bg) = match state.as_str() {
        "completed" => (tok::GREEN, tok::GREEN_BG),
        "failed" | "cancelled" => (tok::RED, tok::RED_BG),
        "running" | "pending" | "cancelling" => (tok::BLUE, tok::BLUE_BG),
        _ => (tok::MUTED, tok::CHIP),
    };
    d.chip("cv_td_state", &state, fg, bg, None, false);
    ui::close_glyph(d, "cv.detail.close");
    d.close();
    d.gap(W::Fill, 14.0);
    let pane_h = ((frame.dialog_max_h() - 140.0) / if compact && avail.task_output && avail.artifacts { 2.0 } else { 1.0 })
        .max(140.0)
        .floor();
    let grid = d.anon();
    d.view(
        &grid,
        &format!("width: Fill height: Fit flow: {} spacing: 14", if compact { "Down" } else { "Right" }),
    );
    if avail.task_output {
        let pane = d.anon();
        d.view(&pane, "width: Fill height: Fit flow: Down spacing: 8");
        let ph = d.anon();
        d.view(&ph, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        d.text("cv_td_out_title", "Output", &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill));
        if let Some(o) = &det.output {
            d.text(
                "cv_td_out_meta",
                &format!("{} bytes · {}", group_thousands(o.total_bytes), if o.source.is_empty() { "runtime".into() } else { o.source.replace('_', " ") }),
                &Txt::new(11.5, Face::Regular, tok::MUTED),
            );
        }
        d.close();
        d.surface(
            "cv_td_out_box",
            "width: Fill height: Fit flow: Down padding: Inset{left: 12 right: 4 top: 10 bottom: 10}",
            tok::SURFACE2,
            10.0,
            Some(tok::HAIRLINE),
        );
        d.open(
            "cv_td_out_scroll",
            "ScrollYView",
            &format!("width: Fill height: Fit max_height: {pane_h} flow: Down padding: Inset{{right: 8}}"),
        );
        let text = if det.loading {
            "Reading task output…".to_owned()
        } else if det.text.is_empty() {
            "No output has been captured.".to_owned()
        } else {
            // Show the tail of a long log (the renderer lays out every line).
            tail_lines(&det.text, 400)
        };
        let per_line = (((if compact { width } else { width / 2.0 }) - 70.0) / (12.0 * 0.6)).floor() as usize;
        d.text(
            "cv_td_out_text",
            &super::takeover::hard_wrap(&text, per_line.max(20)),
            &Txt::new(12.0, Face::Mono, if det.text.is_empty() { tok::MUTED } else { tok::TEXT }).w(W::Fill).wrap(),
        );
        d.close();
        d.close();
        if det.output.as_ref().map(|o| !o.complete).unwrap_or(false) {
            let label = if det.loading_more { "Loading…" } else { "Load more output" };
            d.button("cv_td_more", label, "cv.detail.more", if det.loading_more { Btn::Disabled } else { Btn::Outline }, W::Fit, 32.0);
        }
        d.close();
    }
    if avail.artifacts {
        let pane = d.anon();
        d.view(&pane, &format!("width: {} height: Fit flow: Down spacing: 8", if compact || !avail.task_output { "Fill".to_owned() } else { "300".to_owned() }));
        let ph = d.anon();
        d.view(&ph, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        d.text("cv_td_art_title", "Artifacts", &Txt::new(13.0, Face::Semibold, tok::TEXT).w(W::Fill));
        d.chip("cv_td_art_count", &det.artifacts.as_ref().map(Vec::len).unwrap_or(0).to_string(), tok::MUTED, tok::CHIP, None, false);
        d.close();
        let arts = det.artifacts.clone().unwrap_or_default();
        if arts.is_empty() {
            empty_box(d, "cv_td_art_empty", if det.loading { "Reading artifacts…" } else { "No artifacts were reported." });
        } else {
            let list = d.anon();
            d.view(&list, "width: Fill height: Fit flow: Down spacing: 6");
            for (i, a) in arts.iter().enumerate() {
                let on = det.selected.as_ref().map(|s| s.artifact.id == a.id).unwrap_or(false);
                d.surface(
                    &format!("cv_td_art_{i}_box"),
                    "width: Fill height: Fit flow: Overlay",
                    if on { tok::BLUE_BG } else { tok::SURFACE },
                    10.0,
                    Some(if on { tok::BLUE } else { tok::HAIRLINE }),
                );
                let col = d.anon();
                d.view(&col, "width: Fill height: Fit flow: Down spacing: 2 padding: Inset{left: 12 right: 12 top: 9 bottom: 9}");
                d.text(&format!("cv_td_art_{i}_title"), &ui::fit_w(&a.title, 260.0, 13.0, Face::Medium), &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Fill));
                d.text(&format!("cv_td_art_{i}_meta"), &ui::fit_w(&a.meta(), 260.0, 11.5, Face::Regular), &Txt::new(11.5, Face::Regular, tok::MUTED).w(W::Fill));
                d.close();
                d.tap(&format!("cv_td_art_{i}"), &format!("cv.art.read#{i}"));
                d.close();
            }
            d.close();
        }
        if det.artifact_loading {
            d.text("cv_td_art_loading", "Reading artifact…", &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill));
        } else if let Some(sel) = &det.selected {
            d.surface(
                "cv_td_art_content_box",
                "width: Fill height: Fit flow: Down spacing: 6 padding: Inset{left: 12 right: 4 top: 10 bottom: 10}",
                tok::SURFACE2,
                10.0,
                Some(tok::HAIRLINE),
            );
            d.text("cv_td_art_content_title", &sel.artifact.title, &Txt::new(12.5, Face::Medium, tok::TEXT).w(W::Fill));
            d.open("cv_td_art_scroll", "ScrollYView", "width: Fill height: Fit max_height: 180 flow: Down padding: Inset{right: 8}");
            let body = if sel.content.is_empty() { "No text content.".to_owned() } else { tail_lines(&sel.content, 200) };
            d.text("cv_td_art_content", &super::takeover::hard_wrap(&body, 36), &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill).wrap());
            d.close();
            d.close();
            if sel.has_more {
                d.button("cv_td_art_more", "Load more artifact", "cv.art.more", Btn::Outline, W::Fit, 30.0);
            }
        }
        d.close();
    }
    d.close(); // grid
    if let Some(err) = &det.error {
        d.gap(W::Fill, 10.0);
        d.text("cv_td_error", err, &Txt::new(12.5, Face::Regular, tok::RED).w(W::Fill).wrap());
    }
    ui::shell_close(d);
}

/// The last `max` lines of `s` (a long log keeps its newest output in view).
pub fn tail_lines(s: &str, max: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= max {
        return s.trim_end().to_owned();
    }
    lines[lines.len() - max..].join("\n")
}

/// `toLocaleString()` for a byte count (en-US grouping).
pub fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(task: &str, next: u64) -> Detail {
        Detail {
            active: true,
            task_id: Some(task.into()),
            output: Some(OutputPage { next_offset: next, total_bytes: next, complete: true, source: "runtime_projection".into() }),
            text: "Hello".into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_delta_appends_by_utf8_byte_offset_and_fails_closed_on_a_gap() {
        // model.test.ts:45 / :90
        let mut d = detail("t1", 5);
        assert!(!append_delta(&mut d, "other", 5, " x"), "another task is ignored");
        assert!(!append_delta(&mut d, "t1", 0, "Hello"), "a stale replay is a no-op");
        assert_eq!(d.text, "Hello");
        assert!(append_delta(&mut d, "t1", 3, "lo wörld"), "an overlap contributes its suffix");
        assert_eq!(d.text, "Hello wörld");
        let end = 3 + "lo wörld".len() as u64;
        assert_eq!(d.output.as_ref().unwrap().next_offset, end);
        assert!(!d.output.as_ref().unwrap().complete);
        assert!(!append_delta(&mut d, "t1", end + 4, "gap"), "a gap never touches the text");
        assert_eq!(d.text, "Hello wörld");
        assert_eq!(d.error.as_deref(), Some(GAP_ERROR));
        let mut none = Detail { task_id: Some("t1".into()), ..Default::default() };
        assert!(!append_delta(&mut none, "t1", 0, "x"), "no first read yet: nothing to append to");
    }

    #[test]
    fn rows_title_by_summary_then_role_then_tool_and_only_live_rows_cancel() {
        let mut t = TaskSnapshot::from_list_row(
            "1".into(), "c24b-probe".into(), "running".into(), "running".into(),
            None, None, None, None, 0, vec![], None, None,
        );
        assert_eq!(task_title(&t), "c24b-probe");
        t.role = Some("reviewer".into());
        assert_eq!(task_title(&t), "reviewer");
        t.summary = Some("c24b-probe completed".into());
        assert_eq!(task_title(&t), "c24b-probe completed");
        assert!(cancellable(&t));
        t.state = "completed".into();
        assert!(!cancellable(&t));
        t.artifact_count = 2;
        t.phase = Some("verify".into());
        t.status = "completed".into();
        assert_eq!(task_meta(&t), "completed · verify · 2 artifacts");
    }

    #[test]
    fn the_recorded_status_read_folds_model_permission_and_health() {
        let v = json!({"session_id": "s", "model": {"model": "deepseek-v4-flash", "provider": "deepseek"},
                        "permission_profile": "workspace_write", "health": {"status": "ok"}});
        let r = parse_status(&v);
        assert_eq!(r.model.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(r.permission.as_deref(), Some("workspace_write"));
        assert_eq!(r.health.as_deref(), Some("ok"));
        assert_eq!(parse_status(&json!({})), RuntimeStatus::default());
    }

    #[test]
    fn artifact_pages_append_only_to_the_same_artifact() {
        let a = Artifact { id: "a1".into(), ..Default::default() };
        let mut cur = ArtifactPage { artifact: a.clone(), content: "one ".into(), has_more: true, next_offset: Some(4) };
        append_artifact_page(&mut cur, ArtifactPage { artifact: Artifact { id: "zz".into(), ..Default::default() }, content: "x".into(), has_more: false, next_offset: None });
        assert_eq!(cur.content, "one ");
        append_artifact_page(&mut cur, ArtifactPage { artifact: a, content: "two".into(), has_more: false, next_offset: None });
        assert_eq!(cur.content, "one two");
        assert!(!cur.has_more);
        assert_eq!(group_thousands(1_234_567), "1,234,567");
        assert_eq!(tail_lines("a\nb\nc", 2), "b\nc");
    }
}
