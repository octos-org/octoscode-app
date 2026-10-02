//! Board-3 screen 1 — RUNTIME INVENTORY (rows: inventory × 3).
//!
//! Web: `features/inventory/InventoryDialog.tsx` — two read-only modes,
//! `tools` (`tool/status/list`) and `mcp` (`mcp/status/list`), reached from the
//! `/tools` and `/mcp` commands (`features/commands/registry.ts`, intents
//! "tools"/"mcp"); one dialog shape: header, scope, search, the mode's list,
//! the empty state. The approved board draws the two modes as one dialog with
//! a "Tools | MCP servers" segmented control, so each segment IS a web mode:
//! switching reloads that mode exactly as the web re-keys the dialog by mode
//! (`InventoryDialog.tsx:66-73`).
//!
//! Search is the web's client-side filter (`:74-101`): the trimmed,
//! lower-cased query must occur in the space-joined
//! `name category status policy detail aliases…` (tools) or
//! `id displayName status transport tools…` (servers).
//!
//! Transport (`packages/client/src/inventory.ts:171-190`): the request carries
//! `{session_id, profile_id, include_denied|include_disabled: true}`; a reply
//! for another scope is refused ("Invalid or wrong-scope tool status"); an
//! unadvertised method fails closed ("<method> is not advertised by this
//! server").
use serde_json::Value;

use octoscode_store::Store;

use super::host::Outcome;
use super::ui::{self, tok, Dsl, Face, Frame, Txt, W};

pub const TOOLS_METHOD: &str = "tool/status/list";
pub const MCP_METHOD: &str = "mcp/status/list";

/// The two web modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Tools,
    Mcp,
}

/// One projected tool row (`RuntimeTool`, `inventory.ts:6-14`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRow {
    pub name: String,
    pub category: String,
    pub status: String,
    pub policy: String,
    pub aliases: Vec<String>,
    pub backend: Option<String>,
    pub detail: Option<String>,
}

/// One projected MCP server row (`RuntimeMcpServer`, `inventory.ts:21-29`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerRow {
    pub id: String,
    pub display_name: Option<String>,
    pub transport: Option<String>,
    pub status: String,
    pub tool_count: u64,
    pub tools: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Summary {
    pub connected: u64,
    pub connecting: u64,
    pub failed: u64,
    pub disabled: u64,
}

#[derive(Debug, Clone, Default)]
pub struct InvState {
    pub tab: Tab,
    /// The live query (every keystroke).
    pub query: String,
    /// The query the DSL embeds — refreshed only on a structural remount, so
    /// typing never rebuilds the input (see `Dsl::input`).
    pub query_snap: String,
    pub loading: bool,
    pub error: Option<String>,
    pub tools_error: Option<String>,
    pub mcp_error: Option<String>,
    pub tools: Option<(String, Vec<ToolRow>)>,
    pub servers: Option<(Vec<ServerRow>, Summary)>,
    /// Bumped per request; a reply for an older ticket is dropped
    /// (`InventoryDialog.tsx:35-46` generation guard).
    pub ticket: u64,
    pub scope: String,
}

impl InvState {
    pub fn on_open(&mut self) {
        self.loading = true;
        self.error = None;
        self.query_snap = self.query.clone();
    }
}

fn search_hit(hay: &[&str], q: &str) -> bool {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    hay.join(" ").to_lowercase().contains(&q)
}

/// The web's tool filter (`InventoryDialog.tsx:75-88`).
pub fn tool_matches(t: &ToolRow, q: &str) -> bool {
    let mut hay: Vec<&str> = vec![&t.name, &t.category, &t.status, &t.policy];
    if let Some(d) = &t.detail {
        hay.push(d);
    }
    for a in &t.aliases {
        hay.push(a);
    }
    search_hit(&hay, q)
}

/// The web's server filter (`InventoryDialog.tsx:89-101`).
pub fn server_matches(s: &ServerRow, q: &str) -> bool {
    let mut hay: Vec<&str> = vec![&s.id, &s.status];
    if let Some(d) = &s.display_name {
        hay.push(d);
    }
    if let Some(t) = &s.transport {
        hay.push(t);
    }
    for t in &s.tools {
        hay.push(t);
    }
    search_hit(&hay, q)
}

// ------------------------------------------------------------------ parsing

/// `parseRuntimeTools` (`inventory.ts:53-93`): scope-checked; rows need a
/// non-empty unique name. `None` = the web's "Invalid or wrong-scope".
pub fn parse_tools(v: &Value, session: &str, profile: &str) -> Option<(String, Vec<ToolRow>)> {
    if v.get("session_id")?.as_str()? != session || v.get("profile_id")?.as_str()? != profile {
        return None;
    }
    let policy_id = v.get("policy_id")?.as_str()?.to_owned();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for row in v.get("tools")?.as_array()? {
        let name = row.get("name")?.as_str()?;
        if name.is_empty() || !seen.insert(name.to_owned()) {
            return None;
        }
        let s = |k: &str| row.get(k).and_then(|x| x.as_str()).map(str::to_owned);
        out.push(ToolRow {
            name: name.to_owned(),
            category: s("category")?,
            status: s("status")?,
            policy: s("policy")?,
            aliases: row
                .get("aliases")?
                .as_array()?
                .iter()
                .map(|a| a.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()?,
            backend: s("backend_tool"),
            detail: s("detail"),
        });
    }
    Some((policy_id, out))
}

/// `parseRuntimeMcp` (`inventory.ts:95-150`).
pub fn parse_mcp(v: &Value, session: &str, profile: &str) -> Option<(Vec<ServerRow>, Summary)> {
    if v.get("session_id")?.as_str()? != session || v.get("profile_id")?.as_str()? != profile {
        return None;
    }
    let sm = v.get("summary")?;
    let n = |k: &str| sm.get(k).and_then(|x| x.as_u64());
    let summary = Summary {
        connected: n("connected")?,
        connecting: n("connecting")?,
        failed: n("failed")?,
        disabled: n("disabled")?,
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for row in v.get("servers")?.as_array()? {
        let id = row.get("id")?.as_str()?;
        if id.is_empty() || !seen.insert(id.to_owned()) {
            return None;
        }
        let s = |k: &str| row.get(k).and_then(|x| x.as_str()).map(str::to_owned);
        out.push(ServerRow {
            id: id.to_owned(),
            display_name: s("display_name"),
            transport: s("transport"),
            status: s("status")?,
            tool_count: row.get("tool_count")?.as_u64()?,
            tools: row
                .get("tools")?
                .as_array()?
                .iter()
                .map(|a| a.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()?,
            error: s("error"),
        });
    }
    Some((out, summary))
}

// --------------------------------------------------------------- transport

/// The capability gate (`inventory.ts:160-166`).
pub fn gate(supported: &[String], method: &str, session: &str, profile: &str) -> Result<(), String> {
    if !supported.iter().any(|m| m == method) {
        return Err(format!("{method} is not advertised by this server"));
    }
    if session.is_empty() || profile.is_empty() {
        return Err("A confirmed session and profile are required".into());
    }
    Ok(())
}

/// One mode's read: gate, request, the web's scope-checked parse.
async fn read_mode(
    conv: &crate::flow::Conversation,
    tab: Tab,
    session: &str,
    profile: &str,
    supported: &[String],
) -> Result<Value, String> {
    let (method, params) = match tab {
        Tab::Tools => (
            TOOLS_METHOD,
            serde_json::json!({"session_id": session, "profile_id": profile, "include_denied": true}),
        ),
        Tab::Mcp => (
            MCP_METHOD,
            serde_json::json!({"session_id": session, "profile_id": profile, "include_disabled": true}),
        ),
    };
    gate(supported, method, session, profile)?;
    conv.client()
        .request(method, params)
        .await
        .map_err(|e| e.to_string().chars().take(512).collect())
}

/// Load BOTH runtime inventories (the board's one dialog shows the two web
/// modes together) through the production client.
pub async fn load(conv: &crate::flow::Conversation) -> Result<String, String> {
    let ticket = {
        let mut st = super::host::state();
        st.inv.ticket += 1;
        st.inv.loading = true;
        st.inv.error = None;
        st.inv.ticket
    };
    let session = conv.session_id();
    let profile = conv.profile();
    let supported = conv.store.domains.config.supported_methods();
    let tools = read_mode(conv, Tab::Tools, &session, &profile, &supported).await;
    let mcp = read_mode(conv, Tab::Mcp, &session, &profile, &supported).await;
    let mut st = super::host::state();
    if st.inv.ticket != ticket {
        return Ok("stale inventory reply dropped".into());
    }
    st.inv.loading = false;
    st.inv.scope = format!("{profile} · {session}");
    st.inv.tools_error = None;
    st.inv.mcp_error = None;
    match tools.map(|v| parse_tools(&v, &session, &profile)) {
        Ok(Some((policy, rows))) => {
            // The store's tool domain keeps the last inventory seen.
            conv.store.domains.tool.set(
                rows.iter()
                    .map(|r| octoscode_store::domains::tool::RuntimeTool {
                        name: r.name.clone(),
                        category: Some(r.category.clone()),
                        status: Some(r.status.clone()),
                        policy: Some(r.policy.clone()),
                        aliases: r.aliases.clone(),
                    })
                    .collect(),
            );
            st.inv.tools = Some((policy, rows));
        }
        Ok(None) => st.inv.tools_error = Some("Invalid or wrong-scope tool status".into()),
        Err(e) => st.inv.tools_error = Some(e),
    }
    match mcp.map(|v| parse_mcp(&v, &session, &profile)) {
        Ok(Some((rows, summary))) => st.inv.servers = Some((rows, summary)),
        Ok(None) => st.inv.mcp_error = Some("Invalid or wrong-scope MCP status".into()),
        Err(e) => st.inv.mcp_error = Some(e),
    }
    Ok(format!(
        "{} tools, {} servers",
        st.inv.tools.as_ref().map(|t| t.1.len()).unwrap_or(0),
        st.inv.servers.as_ref().map(|s| s.0.len()).unwrap_or(0)
    ))
}

// ------------------------------------------------------------------ actions

pub fn perform(st: &mut InvState, action: &str, _index: usize) -> Outcome {
    match action {
        "b3.inv.tab.tools" | "b3.inv.tab.mcp" => {
            // Both modes are loaded; the selected segment leads (`/tools`
            // vs `/mcp`) — no refetch on a switch.
            st.tab = if action.ends_with("tools") { Tab::Tools } else { Tab::Mcp };
            st.query_snap = st.query.clone();
            Outcome::Done
        }
        "b3.inv.refresh" => {
            if st.loading {
                return Outcome::Done; // the web disables Refresh while loading
            }
            st.query_snap = st.query.clone();
            st.loading = true;
            Outcome::Spawn(super::host::Job::InventoryLoad)
        }
        _ => Outcome::Unrouted,
    }
}

pub fn input_changed(st: &mut InvState, key: &str, text: &str) {
    if key == "inv.search" {
        st.query = text.to_owned();
    }
}

// -------------------------------------------------------------------- view

/// Truncate to a pixel budget (Inter ~0.56em per glyph, mono 0.6em).
pub fn fit(s: &str, px_budget: f64, font_px: f64, mono: bool) -> String {
    let face = if mono { ui::Face::Mono } else { ui::Face::Regular };
    ui::fit_w(s, px_budget, font_px, face)
}

fn status_chip(d: &mut Dsl, id: &str, status: &str) {
    let on = matches!(status, "enabled" | "allowed" | "active" | "available");
    let (fg, bg) = if on { (tok::TEXT, tok::CHIP) } else { (tok::MUTED, tok::SURFACE2) };
    d.chip(id, status, fg, bg, Some(tok::HAIRLINE), false);
}

fn server_dot(status: &str) -> &'static str {
    match status {
        "connected" | "ready" => tok::GREEN,
        "connecting" | "starting" => tok::BLUE,
        "failed" | "error" => tok::RED,
        _ => tok::FAINT,
    }
}

pub fn build(d: &mut Dsl, st: &InvState, frame: &Frame, _store: &Store) {
    let width = frame.dialog_w(760.0);
    let compact = frame.compact(width);
    let pad = ui::dialog_pad(frame, width);
    let inner_w = width - 2.0 * pad - 10.0; // the scroll gutter
    ui::shell_open(d, frame, width);

    // Header: title + refresh + close (`InventoryDialog.tsx:109-119`).
    let row = d.anon();
    d.view(&row, "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 4");
    d.text("b3_title", "Runtime inventory", &ui::title().w(W::Fill));
    ui::icon_button(d, "b3_inv_refresh", "b3_refresh.svg", 16.0, "b3.inv.refresh");
    ui::close_glyph(d, "b3.close");
    d.close();
    // The scope line (`:120-122`).
    let scope = if st.scope.is_empty() { "Loading scope…".to_owned() } else { st.scope.clone() };
    d.text("b3_inv_scope", &scope, &ui::micro().w(W::Fill));
    d.gap(W::Fill, 12.0);

    // Search (`:128-134`).
    d.surface(
        "b3_inv_search_field",
        "width: Fill height: 40 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8 padding: Inset{left: 12 right: 12 top: 0 bottom: 0}",
        tok::SURFACE,
        10.0,
        Some("#d1d1d6ff"),
    );
    d.icon("b3_inv_search_icon", "b3_search.svg", 16.0, tok::FAINT);
    search_input(d, &st.query_snap);
    d.close();
    d.gap(W::Fill, 12.0);

    // The board's segmented control: the selected web mode leads.
    d.segmented(
        "b3_inv_tab",
        &[
            ("Tools", "b3.inv.tab.tools".to_owned()),
            ("MCP servers", "b3.inv.tab.mcp".to_owned()),
        ],
        if st.tab == Tab::Tools { 0 } else { 1 },
        W::Fill,
        ui::Seg::Tab,
    );
    d.gap(W::Fill, 12.0);

    ui::body_open(d, frame, width, 160.0);
    if st.loading {
        d.text("b3_inv_loading", "Loading runtime inventory…", &ui::meta());
        d.gap(W::Fill, 6.0);
    }
    if let Some(e) = &st.error {
        d.text("b3_inv_error", e, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
    }
    match st.tab {
        Tab::Tools => {
            tools_section(d, st, compact, inner_w);
            section_rule(d);
            servers_section(d, st, compact, inner_w);
        }
        Tab::Mcp => {
            servers_section(d, st, compact, inner_w);
            section_rule(d);
            tools_section(d, st, compact, inner_w);
        }
    }
    ui::body_close(d);
    ui::shell_close(d);
}

/// The full-width rule between the two inventories.
fn section_rule(d: &mut Dsl) {
    d.gap(W::Fill, 12.0);
    d.hairline();
    d.gap(W::Fill, 12.0);
}

/// A table header row between two hairlines (the board's column heads).
fn header_row(d: &mut Dsl, heads: &[&str], cols: &[f64]) {
    d.hairline();
    let head = d.anon();
    d.view(&head, "width: Fill height: 30 flow: Right align: Align{x: 0.0 y: 0.5}");
    for (i, h) in heads.iter().enumerate() {
        d.text("", h, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Px(cols[i])));
    }
    d.close();
    d.hairline();
    d.gap(W::Fill, 4.0);
}

fn search_input(d: &mut Dsl, snap: &str) {
    // The field's own hairline box is the search surface above; the input
    // itself is bare (no second border).
    d.inputs.push(("b3_inv_search".to_owned(), "inv.search".to_owned()));
    let style = ui::text_style(Face::Regular, 13.5);
    d.open(
        "b3_inv_search",
        "TextInput",
        &format!(
            "width: Fill height: Fit padding: Inset{{left: 0 right: 0 top: 4 bottom: 4}} margin: 0\ntext: {} empty_text: \"Search names, status, or tools…\"\nflow: Right is_read_only: false\ndraw_bg +: {{pixel: fn() {{return vec4(0.0, 0.0, 0.0, 0.0)}}}}\ndraw_text +: {{color: {t} color_hover: {t} color_focus: {t} color_down: {t} color_disabled: {f} color_empty: {f} color_empty_hover: {f} color_empty_focus: {f}}}\ndraw_text.text_style: {style}\ndraw_cursor +: {{color: {t}}}\ndraw_selection +: {{color: #2f6feb33 color_hover: #2f6feb33 color_focus: #2f6feb40 color_down: #2f6feb40 color_empty: #00000000 color_disabled: #00000000}}",
            ui::lit(snap),
            t = tok::TEXT,
            f = tok::FAINT,
        ),
    );
    d.close();
}

/// Desktop column widths for the tools table (sum = the inner width).
fn tool_cols(inner_w: f64) -> [f64; 6] {
    // Tool | Category | Status | Policy | Aliases | Backend
    let fixed = [160.0, 104.0, 92.0, 124.0, 0.0, 96.0];
    let used: f64 = fixed.iter().sum();
    let aliases = (inner_w - used).max(80.0);
    [fixed[0], fixed[1], fixed[2], fixed[3], aliases, fixed[5]]
}

fn tools_section(d: &mut Dsl, st: &InvState, compact: bool, inner_w: f64) {
    if let Some(e) = &st.tools_error {
        d.text("b3_inv_tools_error", e, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
        return;
    }
    let Some((policy, rows)) = &st.tools else { return };
    d.text(
        "b3_inv_count",
        &format!("{} tools reported · Policy {}", rows.len(), policy),
        &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill),
    );
    d.gap(W::Fill, 8.0);
    let cols = tool_cols(inner_w);
    if compact {
        d.hairline();
    } else {
        header_row(d, &["Tool", "Category", "Status", "Policy", "Aliases", "Backend"], &cols);
    }
    for (i, t) in rows.iter().enumerate() {
        let rid = format!("b3_inv_tool_{i}");
        if compact {
            d.view(&rid, "width: Fill height: Fit flow: Down padding: Inset{top: 6 bottom: 6}");
            let l1 = d.anon();
            d.view(&l1, "width: Fill height: 28 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
            d.text("", &fit(&t.name, inner_w - 90.0, 13.0, true), &Txt::new(13.0, Face::Mono, tok::TEXT).w(W::Fill));
            status_chip(d, &format!("{rid}_status"), &t.status);
            d.close();
            let mut meta = vec![t.category.clone(), t.policy.clone()];
            if !t.aliases.is_empty() {
                meta.push(format!("Aliases: {}", t.aliases.join(", ")));
            }
            if let Some(b) = t.backend.as_ref().filter(|b| *b != &t.name) {
                meta.push(format!("Backend: {b}"));
            }
            d.text("", &fit(&meta.join(" · "), inner_w, 11.5, false), &Txt::new(11.5, Face::Regular, tok::MUTED).w(W::Fill));
            d.close();
        } else {
            d.view(&rid, "width: Fill height: 34 flow: Right align: Align{x: 0.0 y: 0.5}");
            d.text("", &fit(&t.name, cols[0] - 10.0, 13.0, true), &Txt::new(13.0, Face::Mono, tok::TEXT).w(W::Px(cols[0])));
            d.text("", &fit(&t.category, cols[1] - 8.0, 13.0, false), &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Px(cols[1])));
            let sc = d.anon();
            d.view(&sc, &format!("width: {} height: Fit flow: Right", cols[2]));
            status_chip(d, &format!("{rid}_status"), &t.status);
            d.close();
            d.text("", &fit(&t.policy, cols[3] - 8.0, 13.0, false), &Txt::new(13.0, Face::Regular, tok::MUTED).w(W::Px(cols[3])));
            let aliases = if t.aliases.is_empty() { "—".to_owned() } else { t.aliases.join(", ") };
            d.text("", &fit(&aliases, cols[4] - 10.0, 13.0, true), &Txt::new(13.0, Face::Mono, tok::TEXT).w(W::Px(cols[4])));
            let backend = t.backend.clone().filter(|b| b != &t.name).unwrap_or_else(|| "native".into());
            d.text("", &fit(&backend, cols[5] - 4.0, 13.0, false), &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Px(cols[5])));
            d.close();
        }
    }
    let empty = d.anon();
    d.view(&empty, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5} padding: Inset{top: 6}");
    d.text("b3_inv_tools_empty", "No matching tools.", &Txt::new(13.0, Face::Regular, tok::MUTED));
    d.close();
}

fn server_cols(inner_w: f64) -> [f64; 5] {
    // ID | Transport | Status | toolCount | Summary
    let fixed = [150.0, 90.0, 130.0, 84.0, 0.0];
    let used: f64 = fixed.iter().sum();
    [fixed[0], fixed[1], fixed[2], fixed[3], (inner_w - used).max(80.0)]
}

fn servers_section(d: &mut Dsl, st: &InvState, compact: bool, inner_w: f64) {
    if let Some(e) = &st.mcp_error {
        d.text("b3_inv_mcp_error", e, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
        return;
    }
    let Some((rows, sm)) = &st.servers else { return };
    // The summary row (`InventoryDialog.tsx:172-179`).
    d.text(
        "b3_inv_summary",
        &format!(
            "{} connected · {} connecting · {} failed · {} disabled",
            sm.connected, sm.connecting, sm.failed, sm.disabled
        ),
        &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill),
    );
    d.gap(W::Fill, 8.0);
    let cols = server_cols(inner_w);
    if compact {
        d.hairline();
    } else {
        header_row(d, &["ID", "Transport", "Status", "toolCount", "Summary"], &cols);
    }
    for (i, s) in rows.iter().enumerate() {
        let rid = format!("b3_inv_server_{i}");
        let name = s.display_name.clone().unwrap_or_else(|| s.id.clone());
        let summary = match &s.error {
            Some(e) => e.clone(),
            None if s.tools.is_empty() => "—".to_owned(),
            None => s.tools.join(", "),
        };
        let summary_color = if s.error.is_some() { tok::RED } else { tok::MUTED };
        if compact {
            d.view(&rid, "width: Fill height: Fit flow: Down padding: Inset{top: 6 bottom: 6}");
            let l1 = d.anon();
            d.view(&l1, "width: Fill height: 28 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
            d.text("", &fit(&name, inner_w - 150.0, 13.0, true), &Txt::new(13.0, Face::Mono, tok::TEXT).w(W::Fill));
            if let Some(t) = &s.transport {
                d.chip("", t, tok::MUTED, tok::SURFACE2, Some(tok::HAIRLINE), true);
            }
            d.dot(server_dot(&s.status), 8.0);
            d.text("", &s.tool_count.to_string(), &Txt::new(12.5, Face::Regular, tok::TEXT));
            d.close();
            d.text("", &fit(&format!("{} · {}", s.status, summary), inner_w, 11.5, false), &Txt::new(11.5, Face::Regular, summary_color).w(W::Fill));
            d.close();
        } else {
            d.view(&rid, "width: Fill height: 34 flow: Right align: Align{x: 0.0 y: 0.5}");
            d.text("", &fit(&name, cols[0] - 10.0, 13.0, true), &Txt::new(13.0, Face::Mono, tok::TEXT).w(W::Px(cols[0])));
            let tc = d.anon();
            d.view(&tc, &format!("width: {} height: Fit flow: Right", cols[1]));
            if let Some(t) = &s.transport {
                d.text("", t, &Txt::new(13.0, Face::Regular, tok::TEXT));
            }
            d.close();
            let stc = d.anon();
            d.view(&stc, &format!("width: {} height: Fit flow: Right align: Align{{x: 0.0 y: 0.5}} spacing: 7", cols[2]));
            d.dot(server_dot(&s.status), 8.0);
            d.text("", &s.status, &Txt::new(13.0, Face::Regular, tok::TEXT));
            d.close();
            d.text("", &s.tool_count.to_string(), &Txt::new(13.0, Face::Regular, tok::TEXT).w(W::Px(cols[3])));
            d.text("", &fit(&summary, cols[4] - 4.0, 12.0, false), &Txt::new(12.0, Face::Regular, summary_color).w(W::Px(cols[4])));
            d.close();
        }
    }
    let empty = d.anon();
    d.view(&empty, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5} padding: Inset{top: 6}");
    let msg = if rows.is_empty() { "No MCP servers reported by this runtime." } else { "No matching servers." };
    d.text("b3_inv_servers_empty", msg, &Txt::new(13.0, Face::Regular, tok::MUTED));
    d.close();
}

/// Post-mount visibility: the live query filters both inventories without a
/// remount; each shows its own empty state.
pub fn visibility(st: &InvState, _store: &Store) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    if let Some((_, rows)) = &st.tools {
        let mut any = false;
        for (i, t) in rows.iter().enumerate() {
            let hit = tool_matches(t, &st.query);
            any |= hit;
            out.push((format!("b3_inv_tool_{i}"), hit));
        }
        out.push(("b3_inv_tools_empty".into(), !any));
    }
    if let Some((rows, _)) = &st.servers {
        let mut any = false;
        for (i, s) in rows.iter().enumerate() {
            let hit = server_matches(s, &st.query);
            any |= hit;
            out.push((format!("b3_inv_server_{i}"), hit));
        }
        out.push(("b3_inv_servers_empty".into(), !any));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tools_reply() -> Value {
        json!({
            "session_id": "dsflash:main", "profile_id": "dsflash", "policy_id": "default",
            "tools": [
                {"name": "fs.read", "category": "filesystem", "status": "enabled", "policy": "allow", "aliases": ["read"], "backend_tool": "read_file"},
                {"name": "http.get", "category": "network", "status": "disabled", "policy": "deny", "aliases": ["get"], "detail": "blocked by profile"}
            ]
        })
    }

    #[test]
    fn the_tool_reply_is_scope_checked_like_the_web() {
        let v = tools_reply();
        let (policy, rows) = parse_tools(&v, "dsflash:main", "dsflash").expect("in scope");
        assert_eq!(policy, "default");
        assert_eq!(rows[0].backend.as_deref(), Some("read_file"));
        assert!(parse_tools(&v, "other:main", "dsflash").is_none(), "wrong session refused");
        assert!(parse_tools(&v, "dsflash:main", "other").is_none(), "wrong profile refused");
    }

    #[test]
    fn duplicate_or_empty_names_refuse_the_whole_reply() {
        let mut v = tools_reply();
        v["tools"][1]["name"] = json!("fs.read");
        assert!(parse_tools(&v, "dsflash:main", "dsflash").is_none());
    }

    #[test]
    fn search_matches_the_web_fields_case_insensitively() {
        let (_, rows) = parse_tools(&tools_reply(), "dsflash:main", "dsflash").unwrap();
        assert!(tool_matches(&rows[0], "  FILESYSTEM "));
        assert!(tool_matches(&rows[1], "blocked"), "detail is searched");
        assert!(tool_matches(&rows[1], "get"), "aliases are searched");
        assert!(!tool_matches(&rows[0], "network"));
        assert!(tool_matches(&rows[0], ""), "empty query matches all");
    }

    #[test]
    fn the_mcp_reply_parses_the_summary_and_rows() {
        let v = json!({
            "session_id": "s", "profile_id": "p",
            "summary": {"connected": 2, "connecting": 1, "failed": 1, "disabled": 0},
            "servers": [
                {"id": "github", "transport": "stdio", "status": "connected", "tool_count": 9, "tools": ["issues"]},
                {"id": "sentry", "transport": "http", "status": "failed", "tool_count": 0, "tools": [], "error": "401"}
            ]
        });
        let (rows, sm) = parse_mcp(&v, "s", "p").expect("parses");
        assert_eq!(sm.connected, 2);
        assert_eq!(rows[1].error.as_deref(), Some("401"));
        assert!(server_matches(&rows[0], "issues"), "tools are searched");
        assert!(server_matches(&rows[1], "HTTP"), "transport is searched");
    }

    #[test]
    fn the_gate_fails_closed_without_the_advertised_method() {
        let err = gate(&[], TOOLS_METHOD, "s", "p").unwrap_err();
        assert_eq!(err, "tool/status/list is not advertised by this server");
        assert!(gate(&[TOOLS_METHOD.to_owned()], TOOLS_METHOD, "", "p").is_err());
        assert!(gate(&[TOOLS_METHOD.to_owned()], TOOLS_METHOD, "s", "p").is_ok());
    }

    #[test]
    fn the_filter_hides_rows_and_shows_the_empty_state() {
        let (policy, rows) = parse_tools(&tools_reply(), "dsflash:main", "dsflash").unwrap();
        let mut st = InvState { tools: Some((policy, rows)), ..Default::default() };
        st.query = "zzz".into();
        let vis = visibility(&st, &Store::new());
        assert!(vis.iter().any(|(id, v)| id == "b3_inv_tools_empty" && *v));
        assert!(vis.iter().filter(|(id, _)| id.starts_with("b3_inv_tool_")).all(|(_, v)| !v));
        st.query = "fs".into();
        let vis = visibility(&st, &Store::new());
        assert!(vis.iter().any(|(id, v)| id == "b3_inv_tool_0" && *v));
        assert!(vis.iter().any(|(id, v)| id == "b3_inv_tools_empty" && !*v));
    }

    #[test]
    fn the_dialog_lowers_with_its_controls_on_the_shared_tap_path() {
        let (policy, rows) = parse_tools(&tools_reply(), "dsflash:main", "dsflash").unwrap();
        let st = InvState { tools: Some((policy, rows)), scope: "dsflash · dsflash:main".into(), ..Default::default() };
        for frame in [Frame::DESKTOP, Frame { avail_w: 360.0, avail_h: 780.0 }] {
            let mut d = Dsl::new();
            build(&mut d, &st, &frame, &Store::new());
            let taps = d.taps.clone();
            let dsl = d.finish();
            let wired = crate::screens::taps::wired_taps(&dsl);
            assert_eq!(wired, taps);
            for ev in ["b3.close", "b3.inv.refresh", "b3.inv.tab.tools", "b3.inv.tab.mcp"] {
                assert!(wired.iter().any(|(_, e)| e == ev), "{ev} wired");
            }
            assert!(dsl.contains("fs.read"));
            assert_eq!(dsl.matches('{').count(), dsl.matches('}').count(), "balanced");
        }
    }
}
