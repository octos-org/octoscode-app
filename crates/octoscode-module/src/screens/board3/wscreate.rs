//! Board-3 screens 2-3 — WORKSPACE CREATE + CREATE FOLDER (rows:
//! workspace-create × 6).
//!
//! Web: `features/workspace-create/NewSessionWorkspacePicker.tsx` (the
//! "Add workspace" / "Choose a workspace" dialog, `:147-161`), the server's
//! working directory pinned first (`server-working-directory.ts:11-25`,
//! "Server's working directory (path not reported)" when `workspace_root` is
//! blank), the recent-workspaces list (`workspace/workspace-recents.ts`,
//! natively `screens/recents.rs`), the path form ("Start session"; the only
//! validation is a non-blank trimmed path, `:73-78`), and the folder browser
//! `WorkspaceFolderBrowser.tsx` + `workspace-browse.ts` (`onboarding/
//! workspace_list {path}`, `onboarding/workspace_create {parent, name}`, the
//! name pre-validation `:165-179`, the refusal copy `:71-126`), gated on the
//! advertised `onboarding.workspace_browse.v1` feature (fail closed: without
//! it no browse affordance — `workspace-browse-adapter.ts:21-25`).
//!
//! Opening: the web reaches the picker from the sidebar's "Add workspace"
//! (`ProductSidebar.tsx:651-661`) and New Session (`App.tsx:1972-1988`);
//! natively the same dialog opens from the `/workspace` command and the
//! sidebar's "Add workspace" control. Choosing a row or submitting a path
//! mints a fresh session and opens it at that cwd (`session/open {session_id,
//! profile_id, cwd}`, App.tsx:1893-1971), then remembers the workspace.
use serde_json::Value;

use octoscode_store::Store;

use crate::screens::recents;

use super::host::Outcome;
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

pub const BROWSE_FEATURE: &str = "onboarding.workspace_browse.v1";
pub const LIST_METHOD: &str = "onboarding/workspace_list";
pub const CREATE_METHOD: &str = "onboarding/workspace_create";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    /// The board's screen 2: wd + recents + path form.
    #[default]
    Pick,
    /// The board's screen 3: the folder browser with "Create new folder".
    Browse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub writable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Listing {
    pub canonical_path: String,
    pub parent_path: Option<String>,
    pub writable: bool,
    pub entries: Vec<Entry>,
    pub truncated: bool,
    pub hidden_skipped: u64,
}

#[derive(Debug, Clone, Default)]
pub struct WsState {
    pub view: View,
    pub path: String,
    pub path_snap: String,
    pub folder: String,
    pub folder_snap: String,
    /// The pre-validation message, shown only after a submit; typing clears it.
    pub folder_error: Option<String>,
    pub error: Option<String>,
    pub busy: bool,
    pub listing: Option<Listing>,
    pub show_entries: bool,
    pub ticket: u64,
}

/// `workspace-browse.ts:165-179`: the web's pre-validation, in order. The
/// name is sent untrimmed when valid.
pub fn validate_folder_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("Enter a name for the new folder.");
    }
    if name.contains('/') || name.contains('\\') {
        return Err("A folder name can't contain a slash. Enter one name only.");
    }
    if name == "." || name == ".." {
        return Err("Enter a folder name other than . or ..");
    }
    if name.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f) {
        return Err("A folder name can't contain control characters. Use plain text.");
    }
    if name.trim() != name {
        return Err("A folder name can't start or end with a space. Trim it.");
    }
    if name.len() > 255 {
        return Err("That folder name is too long. Use up to 255 bytes.");
    }
    Ok(())
}

/// The refusal copy (`workspace-browse.ts:71-126`): `message + " " + next`.
pub fn refusal(kind: &str) -> &'static str {
    match kind {
        "workspace_list_invalid_path" => "That path can't be browsed. Browse from the server's working directory instead.",
        "workspace_list_not_found" => "That folder is no longer on the server. Go up one level and pick a folder that still exists.",
        "workspace_list_not_a_directory" => "That path is a file, not a folder. Go up one level and pick a folder.",
        "workspace_list_permission_denied" => "Octos can't open that folder. Pick a folder the Octos server is allowed to read.",
        "workspace_list_root_escape" => "That folder is outside the area Octos may browse. Pick a folder inside your own projects instead.",
        "workspace_create_invalid_name" => "The server rejected that folder name. Use a single name without slashes, up to 255 bytes.",
        "workspace_create_parent_not_found" => "The folder you're creating in is no longer on the server. Go up one level and try again.",
        "workspace_create_parent_not_a_directory" => "The place you're creating in is a file, not a folder. Go up one level and pick a folder.",
        "workspace_create_permission_denied" => "Octos can't create a folder here. Pick a folder the Octos server is allowed to write to.",
        "workspace_create_root_escape" => "That location is outside the area Octos may write to. Create the folder inside your own projects instead.",
        "workspace_create_exists_not_directory" => "A file of that name is already here. Choose a different folder name.",
        "profile_local_unsupported" => "This server doesn't offer folder browsing. Type the workspace path instead.",
        _ => "Couldn't reach the server's folders. Try again, or type the workspace path instead.",
    }
}

fn refusal_of(e: &octoscode_client::ClientError) -> &'static str {
    match e {
        octoscode_client::ClientError::Rpc { error, .. } => refusal(
            error
                .data
                .as_ref()
                .and_then(|d| d.get("kind"))
                .and_then(|k| k.as_str())
                .unwrap_or("unknown"),
        ),
        _ => refusal("unknown"),
    }
}

/// `onboarding/workspace_list` result (≤500 directories).
pub fn parse_listing(v: &Value) -> Option<Listing> {
    let entries = v
        .get("entries")?
        .as_array()?
        .iter()
        .take(500)
        .map(|e| {
            Some(Entry {
                name: e.get("name")?.as_str()?.to_owned(),
                path: e.get("path")?.as_str()?.to_owned(),
                writable: e.get("writable").and_then(|w| w.as_bool()).unwrap_or(false),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Listing {
        canonical_path: v.get("canonical_path")?.as_str()?.to_owned(),
        parent_path: v.get("parent_path").and_then(|p| p.as_str()).map(str::to_owned),
        writable: v.get("writable").and_then(|w| w.as_bool()).unwrap_or(false),
        entries,
        truncated: v.get("truncated").and_then(|t| t.as_bool()).unwrap_or(false),
        hidden_skipped: v.get("hidden_skipped").and_then(|h| h.as_u64()).unwrap_or(0),
    })
}

/// The notices line (`workspace-browse.ts:242-259`).
pub fn notices(l: &Listing) -> String {
    let mut out = Vec::new();
    if l.truncated {
        out.push(format!("Only the first {} folders are shown.", l.entries.len()));
    }
    if l.hidden_skipped > 0 {
        out.push(format!("{} hidden folders aren't shown.", l.hidden_skipped));
    }
    out.join(" ")
}

pub fn browse_advertised(store: &Store) -> bool {
    store.domains.config.has_capability(BROWSE_FEATURE)
}

/// The server's working directory (the active open reply's `workspace_root`).
pub fn server_wd(store: &Store) -> Option<String> {
    let sid = store.domains.session.active()?;
    store.domains.session.workspace_root(&sid).filter(|r| !r.trim().is_empty())
}

// --------------------------------------------------------------- transport

pub async fn list(conv: &crate::flow::Conversation, path: Option<String>) -> Result<String, String> {
    let ticket = {
        let mut st = super::host::state();
        st.ws.ticket += 1;
        st.ws.busy = true;
        st.ws.error = None;
        st.ws.ticket
    };
    let r = conv
        .client()
        .request(LIST_METHOD, serde_json::json!({ "path": path }))
        .await;
    let mut st = super::host::state();
    if st.ws.ticket != ticket {
        return Ok("stale listing dropped".into());
    }
    st.ws.busy = false;
    match r {
        Ok(v) => match parse_listing(&v) {
            Some(l) => {
                let n = l.entries.len();
                st.ws.listing = Some(l);
                st.ws.show_entries = true;
                Ok(format!("{n} folders"))
            }
            None => {
                st.ws.error = Some(refusal("unknown").into());
                Err("bad listing".into())
            }
        },
        Err(e) => {
            // Failures keep the last good listing on screen (the reducer's
            // "failed" arm).
            st.ws.error = Some(refusal_of(&e).into());
            Err(e.to_string())
        }
    }
}

pub async fn create_folder(conv: &crate::flow::Conversation, parent: String, name: String) -> Result<String, String> {
    let r = conv
        .client()
        .request(CREATE_METHOD, serde_json::json!({ "parent": parent, "name": name }))
        .await;
    match r {
        Ok(v) => {
            let path = v.get("canonical_path").and_then(|p| p.as_str()).map(str::to_owned);
            {
                let mut st = super::host::state();
                st.ws.busy = false;
                st.ws.folder.clear();
                st.ws.folder_snap.clear();
            }
            // Move into the new folder (`WorkspaceFolderBrowser`:119-121); a
            // `created:false` (it already existed) counts as success.
            list(conv, path.or(Some(parent))).await
        }
        Err(e) => {
            let mut st = super::host::state();
            st.ws.busy = false;
            st.ws.error = Some(refusal_of(&e).into());
            Err(e.to_string())
        }
    }
}

/// Start a NEW session at `cwd` (the picker's row / path submit), then
/// remember the workspace — written after the open succeeds, never before
/// (`screens/workspace.rs` P4h1 rule).
pub async fn start(conv: &crate::flow::Conversation, cwd: String) -> Result<String, String> {
    let r = conv.new_chat(Some(cwd.clone())).await;
    let mut st = super::host::state();
    st.ws.busy = false;
    match r {
        Ok(id) => {
            recents::remember_workspace(&*recents::store(), &recents::endpoint(), &cwd, recents::now_ms());
            st.open = None;
            Ok(format!("session {id} at {cwd}"))
        }
        Err(e) => {
            st.ws.error = Some(e.clone());
            Err(e)
        }
    }
}

// ------------------------------------------------------------------ actions

pub fn perform(st: &mut WsState, action: &str, index: usize, store: &Store) -> Outcome {
    match action {
        "b3.ws.wd" => match server_wd(store) {
            Some(wd) => {
                st.busy = true;
                Outcome::Spawn(super::host::Job::WsStart(wd))
            }
            None => Outcome::Done, // the disabled "(path not reported)" row
        },
        "b3.ws.recent" => {
            let rows = recents::load_recent_workspaces(&*recents::store(), &recents::endpoint());
            match rows.get(index) {
                Some(r) => {
                    st.busy = true;
                    Outcome::Spawn(super::host::Job::WsStart(r.path.clone()))
                }
                None => Outcome::Done,
            }
        }
        "b3.ws.start" => {
            let path = st.path.trim().to_owned();
            if path.is_empty() {
                st.error = Some("Enter a workspace path on the Octos server.".into());
                st.path_snap = st.path.clone();
                return Outcome::Done;
            }
            st.busy = true;
            Outcome::Spawn(super::host::Job::WsStart(path))
        }
        "b3.ws.browse" => {
            if !browse_advertised(store) {
                return Outcome::Done; // fail closed: no browse without the feature
            }
            st.view = View::Browse;
            st.path_snap = st.path.clone();
            let start = (!st.path.trim().is_empty()).then(|| st.path.trim().to_owned());
            Outcome::Spawn(super::host::Job::WsList(start))
        }
        "b3.ws.back" => {
            st.view = View::Pick;
            st.path_snap = st.path.clone();
            st.error = None;
            Outcome::Done
        }
        "b3.ws.up" => match st.listing.as_ref().and_then(|l| l.parent_path.clone()) {
            Some(p) => Outcome::Spawn(super::host::Job::WsList(Some(p))),
            None => Outcome::Done,
        },
        "b3.ws.enter" => match st.listing.as_ref().and_then(|l| l.entries.get(index)) {
            Some(e) => Outcome::Spawn(super::host::Job::WsList(Some(e.path.clone()))),
            None => Outcome::Done,
        },
        "b3.ws.use" => {
            // "Use" fills the path box and returns to the path view — it does
            // NOT submit (NewSessionWorkspacePicker.tsx:500-504).
            let chosen = if index == usize::MAX {
                st.listing.as_ref().map(|l| l.canonical_path.clone())
            } else {
                st.listing.as_ref().and_then(|l| l.entries.get(index)).map(|e| e.path.clone())
            };
            if let Some(p) = chosen {
                st.path = p.clone();
                st.path_snap = p;
                st.view = View::Pick;
            }
            Outcome::Done
        }
        "b3.ws.use_current" => perform(st, "b3.ws.use", usize::MAX, store),
        "b3.ws.toggle_entries" => {
            st.show_entries = !st.show_entries;
            st.folder_snap = st.folder.clone();
            Outcome::Done
        }
        "b3.ws.create" => {
            st.folder_snap = st.folder.clone();
            if let Err(msg) = validate_folder_name(&st.folder) {
                st.folder_error = Some(msg.to_owned());
                return Outcome::Done;
            }
            let Some(parent) = st.listing.as_ref().filter(|l| l.writable).map(|l| l.canonical_path.clone()) else {
                return Outcome::Done;
            };
            st.busy = true;
            st.folder_error = None;
            Outcome::Spawn(super::host::Job::WsCreate(parent, st.folder.clone()))
        }
        _ => Outcome::Unrouted,
    }
}

pub fn input_changed(st: &mut WsState, key: &str, text: &str) {
    match key {
        "ws.path" => {
            st.path = text.to_owned();
            st.error = None;
        }
        "ws.folder" => {
            st.folder = text.to_owned();
            st.folder_error = None; // typing clears the error
        }
        _ => {}
    }
}

pub fn input_returned(st: &mut WsState, key: &str, store: &Store) -> Outcome {
    match key {
        "ws.path" => perform(st, "b3.ws.start", 0, store),
        "ws.folder" => perform(st, "b3.ws.create", 0, store),
        _ => Outcome::Done,
    }
}

// -------------------------------------------------------------------- view

fn wd_row(d: &mut Dsl, store: &Store) {
    ui::field_label(d, "b3_ws_wd_label", "Server's working directory");
    d.gap(W::Fill, 6.0);
    let wd = server_wd(store);
    d.surface("b3_ws_wd", "width: Fill height: 40 flow: Overlay", tok::SURFACE2, 10.0, Some(tok::HAIRLINE));
    let row = d.anon();
    d.view(&row, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 12 right: 12 top: 0 bottom: 0}");
    match &wd {
        Some(p) => {
            d.text("b3_ws_wd_path", &super::inventory::fit(p, 520.0, 12.5, true), &Txt::new(12.5, Face::Mono, tok::TEXT).w(W::Fill));
            d.icon("", "b3_chevron_right.svg", 14.0, tok::MUTED);
        }
        None => {
            d.text("b3_ws_wd_path", "/", &Txt::new(12.5, Face::Mono, tok::FAINT).w(W::Fill));
            d.text("b3_ws_wd_missing", "(path not reported)", &Txt::new(11.5, Face::Regular, tok::FAINT));
        }
    }
    d.close();
    if wd.is_some() {
        d.tap("b3_ws_wd_tap", "b3.ws.wd");
    }
    d.close();
}

fn pick_view(d: &mut Dsl, st: &WsState, store: &Store, inner_w: f64) {
    wd_row(d, store);
    d.gap(W::Fill, 16.0);
    let rows = recents::load_recent_workspaces(&*recents::store(), &recents::endpoint());
    ui::field_label(d, "b3_ws_recent_label", "Recent workspaces");
    d.gap(W::Fill, 6.0);
    if rows.is_empty() {
        d.text("b3_ws_recent_empty", "No recent workspace paths", &ui::meta());
    }
    let list = d.anon();
    d.view(&list, "width: Fill height: Fit flow: Down spacing: 6");
    for (i, r) in rows.iter().take(5).enumerate() {
        let rid = format!("b3_ws_recent_{i}");
        d.surface(&rid, "width: Fill height: 58 flow: Overlay", tok::SURFACE, 12.0, Some(tok::HAIRLINE));
        let row = d.anon();
        d.view(&row, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 11 padding: Inset{left: 12 right: 10 top: 0 bottom: 0}");
        d.icon("", "b3_folder.svg", 18.0, tok::MUTED);
        let col = d.anon();
        d.view(&col, "width: Fill height: Fit flow: Down spacing: 3");
        d.text(&format!("{rid}_name"), &r.name, &Txt::new(13.5, Face::Medium, tok::TEXT).w(W::Fill));
        d.text(&format!("{rid}_path"), &super::inventory::fit(&r.path, inner_w - 220.0, 11.5, true), &Txt::new(11.5, Face::Mono, tok::MUTED).w(W::Fill));
        d.close();
        d.text("", "Start a new session in …", &Txt::new(11.5, Face::Regular, tok::MUTED));
        d.icon("", "b3_chevron_right.svg", 14.0, tok::MUTED);
        d.close();
        d.tap(&format!("{rid}_tap"), &format!("b3.ws.recent#{i}"));
        d.close();
    }
    d.close();
    d.gap(W::Fill, 16.0);
    ui::field_label(d, "b3_ws_path_label", "Workspace path");
    d.gap(W::Fill, 6.0);
    // Side by side (the board); stacked on a phone-narrow card, where the
    // pill would squeeze the field to a clipped placeholder.
    let narrow = inner_w < 420.0;
    let form = d.anon();
    d.view(
        &form,
        if narrow {
            "width: Fill height: Fit flow: Down spacing: 8"
        } else {
            "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8"
        },
    );
    d.input("b3_ws_path", "ws.path", &st.path_snap, "Enter a path or choose a folder…", false, 40.0);
    let kind = if st.busy { Btn::Disabled } else { Btn::Primary };
    let w = if narrow { W::Fill } else { W::Fit };
    d.button("b3_ws_start", if st.busy { "Starting…" } else { "Start session" }, "b3.ws.start", kind, w, 40.0);
    d.close();
    d.gap(W::Fill, 8.0);
    let help = d.anon();
    d.view(&help, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
    d.text(
        "b3_ws_help",
        "Enter a path on the Octos server, for example /home/user/projects/my-app.",
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    if browse_advertised(store) {
        d.link("b3_ws_browse", "Browse…", Some("b3.ws.browse"), 12.5);
    }
    d.close();
    if !browse_advertised(store) {
        d.gap(W::Fill, 10.0);
        gate_note(d);
    }
}

/// The board's fail-closed note with the visibly disabled drill-in control.
fn gate_note(d: &mut Dsl) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
    d.text(
        "b3_ws_gate",
        "The folder browser is available only when the server advertises it.",
        &Txt::new(12.0, Face::Regular, tok::FAINT).w(W::Fill).wrap(),
    );
    d.surface("b3_ws_gate_btn", "width: 34 height: 28 flow: Overlay align: Align{x: 0.5 y: 0.5}", tok::DISABLED_BG, 8.0, None);
    d.icon("", "b3_chevron_right.svg", 13.0, tok::FAINT);
    d.close();
    d.close();
}

fn browse_view(d: &mut Dsl, st: &WsState, store: &Store, inner_w: f64) {
    wd_row(d, store);
    d.gap(W::Fill, 14.0);
    let listing = st.listing.clone().unwrap_or_default();
    // Path bar: Up + "Current folder" + the canonical path.
    let bar = d.anon();
    d.view(&bar, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    if listing.parent_path.is_some() {
        ui::icon_button(d, "b3_ws_up", "icon_arrow_up.svg", 14.0, "b3.ws.up");
    }
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 2");
    d.text("", "Current folder", &Txt::new(11.5, Face::Regular, tok::MUTED));
    let cur = if st.busy && listing.canonical_path.is_empty() { "Loading folders…".to_owned() } else { listing.canonical_path.clone() };
    d.text("b3_ws_current", &super::inventory::fit(&cur, inner_w - 140.0, 12.0, true), &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill));
    d.close();
    d.link("b3_ws_use_current", "Use this folder", Some("b3.ws.use_current"), 12.5);
    d.close();
    d.gap(W::Fill, 14.0);
    if listing.writable {
        ui::field_label(d, "b3_ws_folder_label", "Create new folder");
        d.gap(W::Fill, 6.0);
        d.input("b3_ws_folder", "ws.folder", &st.folder_snap, "New folder name", false, 40.0);
        if let Some(e) = &st.folder_error {
            d.gap(W::Fill, 4.0);
            d.text("b3_ws_folder_error", e, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
        }
        d.gap(W::Fill, 10.0);
        let kind = if st.busy { Btn::Disabled } else { Btn::Primary };
        d.button("b3_ws_create", if st.busy { "Creating…" } else { "Create" }, "b3.ws.create", kind, W::Fill, 40.0);
        d.gap(W::Fill, 10.0);
    }
    d.link("b3_ws_back", "Back to workspaces", Some("b3.ws.back"), 13.0);
    d.gap(W::Fill, 14.0);
    // The disclosure row: "▾ · N existing folders".
    d.view("b3_ws_entries_head", "width: Fill height: 30 flow: Overlay");
    let head = d.anon();
    d.view(&head, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
    d.icon(
        "",
        if st.show_entries { "b3_chevron_down_dark.svg" } else { "b3_chevron_right_dark.svg" },
        13.0,
        tok::TEXT,
    );
    let n = listing.entries.len();
    d.text("b3_ws_entries_count", &format!("· {n} existing folder{}", if n == 1 { "" } else { "s" }), &Txt::new(13.0, Face::Regular, tok::TEXT));
    d.close();
    d.tap("b3_ws_entries_toggle", "b3.ws.toggle_entries");
    d.close();
    if st.show_entries {
        if listing.entries.is_empty() && !st.busy {
            d.text("b3_ws_noentries", "No subfolders here.", &ui::meta());
        }
        for (i, e) in listing.entries.iter().enumerate() {
            let rid = format!("b3_ws_entry_{i}");
            d.view(&rid, "width: Fill height: 36 flow: Overlay");
            let row = d.anon();
            d.view(&row, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10 padding: Inset{left: 4 right: 4 top: 0 bottom: 0}");
            d.icon("", "b3_folder.svg", 16.0, tok::MUTED);
            d.text(&format!("{rid}_name"), &e.name, &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Fill));
            if !e.writable {
                d.text("", "read-only", &Txt::new(11.5, Face::Regular, tok::FAINT));
            }
            d.icon("", "b3_chevron_right.svg", 13.0, tok::MUTED);
            d.close();
            d.tap(&format!("{rid}_tap"), &format!("b3.ws.enter#{i}"));
            d.close();
        }
        let note = notices(&listing);
        if !note.is_empty() {
            d.text("b3_ws_notices", &note, &ui::micro().w(W::Fill));
        }
    }
}


pub fn build(d: &mut Dsl, st: &WsState, frame: &Frame, store: &Store) {
    let width = frame.dialog_w(560.0);
    let pad = ui::dialog_pad(frame, width);
    let inner_w = width - 2.0 * pad;
    ui::shell_open(d, frame, width);
    let title = match st.view {
        View::Pick => "Add workspace",
        View::Browse => "Browse server folders",
    };
    ui::header(d, title, "b3.close");
    d.gap(W::Fill, 14.0);
    ui::body_open(d, frame, width, 50.0);
    if let Some(e) = &st.error {
        d.text("b3_ws_error", e, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
        d.gap(W::Fill, 8.0);
    }
    match st.view {
        View::Pick => pick_view(d, st, store, inner_w),
        View::Browse => browse_view(d, st, store, inner_w),
    }
    ui::body_close(d);
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn folder_names_are_pre_validated_in_the_web_order() {
        assert_eq!(validate_folder_name(""), Err("Enter a name for the new folder."));
        assert!(validate_folder_name("a/b").unwrap_err().contains("slash"));
        assert!(validate_folder_name("..").unwrap_err().contains("other than"));
        assert!(validate_folder_name("a\u{7}").unwrap_err().contains("control"));
        assert!(validate_folder_name("   ").unwrap_err().contains("start or end with a space"), "spaces-only is the whitespace check");
        assert!(validate_folder_name(&"x".repeat(256)).unwrap_err().contains("255"));
        assert!(validate_folder_name("notes").is_ok());
    }

    #[test]
    fn refusals_are_whitelisted_and_unknown_falls_back() {
        assert!(refusal("workspace_create_exists_not_directory").contains("different folder name"));
        assert!(refusal("something_new").starts_with("Couldn't reach"));
    }

    #[test]
    fn the_listing_parses_with_notices() {
        let v = json!({
            "canonical_path": "/home/user/octos", "parent_path": "/home/user", "writable": true,
            "entries": [{"name": "docs", "path": "/home/user/octos/docs", "writable": true}],
            "truncated": true, "hidden_skipped": 2
        });
        let l = parse_listing(&v).unwrap();
        assert_eq!(l.entries[0].name, "docs");
        assert_eq!(notices(&l), "Only the first 1 folders are shown. 2 hidden folders aren't shown.");
    }

    #[test]
    fn browse_fails_closed_without_the_feature_and_create_validates_first() {
        let store = Store::new();
        let mut st = WsState::default();
        assert_eq!(perform(&mut st, "b3.ws.browse", 0, &store), Outcome::Done);
        assert_eq!(st.view, View::Pick, "no browse without onboarding.workspace_browse.v1");
        store.set_capabilities(vec![BROWSE_FEATURE.to_owned()]);
        assert_eq!(perform(&mut st, "b3.ws.browse", 0, &store), Outcome::Spawn(super::super::host::Job::WsList(None)));
        st.listing = Some(Listing { canonical_path: "/home/user/octos".into(), writable: true, ..Default::default() });
        input_changed(&mut st, "ws.folder", " notes");
        assert_eq!(perform(&mut st, "b3.ws.create", 0, &store), Outcome::Done);
        assert!(st.folder_error.is_some());
        input_changed(&mut st, "ws.folder", "notes");
        assert!(st.folder_error.is_none(), "typing clears the error");
        assert_eq!(
            perform(&mut st, "b3.ws.create", 0, &store),
            Outcome::Spawn(super::super::host::Job::WsCreate("/home/user/octos".into(), "notes".into()))
        );
    }

    #[test]
    fn a_blank_path_never_starts_a_session() {
        let store = Store::new();
        let mut st = WsState::default();
        input_changed(&mut st, "ws.path", "   ");
        assert_eq!(perform(&mut st, "b3.ws.start", 0, &store), Outcome::Done);
        assert_eq!(st.error.as_deref(), Some("Enter a workspace path on the Octos server."));
        input_changed(&mut st, "ws.path", " /home/user/octos ");
        assert_eq!(perform(&mut st, "b3.ws.start", 0, &store), Outcome::Spawn(super::super::host::Job::WsStart("/home/user/octos".into())));
    }

    #[test]
    fn use_fills_the_path_and_returns_without_submitting() {
        let store = Store::new();
        let mut st = WsState { view: View::Browse, ..Default::default() };
        st.listing = Some(Listing {
            canonical_path: "/home/user".into(),
            entries: vec![Entry { name: "octos".into(), path: "/home/user/octos".into(), writable: true }],
            ..Default::default()
        });
        assert_eq!(perform(&mut st, "b3.ws.use_current", 0, &store), Outcome::Done);
        assert_eq!(st.path, "/home/user");
        assert_eq!(st.view, View::Pick);
    }

    #[test]
    fn both_views_lower_balanced() {
        let store = Store::new();
        for view in [View::Pick, View::Browse] {
            let st = WsState {
                view,
                listing: Some(Listing { canonical_path: "/home/user/octos".into(), writable: true, entries: vec![Entry { name: "docs".into(), path: "/d".into(), writable: true }], ..Default::default() }),
                ..Default::default()
            };
            let mut d = Dsl::new();
            build(&mut d, &st, &Frame::DESKTOP, &store);
            let dsl = d.finish();
            assert_eq!(dsl.matches('{').count(), dsl.matches('}').count(), "{view:?}");
        }
    }
}
