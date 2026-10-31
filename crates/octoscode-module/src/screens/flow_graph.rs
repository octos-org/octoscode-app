//! The Flow view's model: the conversation and the work it sent out, as cards
//! joined by lines (`crate::flow_canvas` draws it).
//!
//! Adapted from OctoBuddy's flow canvas (`src/flow.rs` and `flow_cards` in
//! its `src/lib.rs`), where an outer loop's inner loops are OctoBuddy's own
//! orchestration. Here every card is something the server already reports,
//! read from the store the Fleet and the sidebar read:
//!
//! - a **session** card: the active session, and any other session a listed
//!   peer was staged from (`peer/staged`'s originating session);
//! - a **peer** card per Fleet row (`fleetview::rows`): the same "Peer N ·
//!   model", status word and elapsed/tokens the Fleet and the peer dock show,
//!   hung from the session that staged it (the active one when unknown);
//! - an **agent** card per agent still running (`agent/updated`, the Agents
//!   panel's roster), hung from its parent agent, else from the peer or the
//!   session it runs in.
//!
//! OctoBuddy's rounds, waves, gates and acceptance have no counterpart in
//! the protocol, so they are not drawn. Where a card sits is the person's
//! (dragged positions are kept per card key, on disk); otherwise the
//! default layout puts the sessions in a column and each one's peers to its
//! right, OctoBuddy's arrangement.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use octoscode_store::Store;

use super::board3::fleetview::{self, Status};
use super::board3::{agents, switcher};
use super::{peers, sidebar};
use crate::i18n::tr;

/// What a card stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Session,
    Peer,
    Agent,
}

/// How a card is joined to its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    /// A peer session: it reports back, so the line has an arrow at each end.
    Peer,
    /// An agent: sent one way.
    Sub,
}

/// The card's tone (its template): what it is doing now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Idle,
    Working,
    Waiting,
    Done,
    Failed,
}

/// What a click on a card opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The session's conversation (`session.open` with its store index, or
    /// the Chat tab when it is the active one).
    Session { id: String, index: Option<usize> },
    /// The Fleet pane (a peer's details and controls).
    Fleet,
    /// The Agents panel.
    Agents,
}

/// One card, laid out.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Stable across syncs: `s:<session>`, `p:<fleet key>`, `a:<agent id>`.
    pub key: String,
    pub kind: Kind,
    pub parent: Option<String>,
    pub link: Link,
    pub tone: Tone,
    /// The active session's card.
    pub lit: bool,
    /// At work: its line carries moving dots.
    pub active: bool,
    pub pos: (f64, f64),
    pub size: (f64, f64),
    pub title: String,
    /// The tag beside the title (session / peer / agent).
    pub tag: String,
    pub status: String,
    /// The second line: the model, the brief, the task.
    pub detail: String,
    pub target: Target,
}

pub const SESSION_SIZE: (f64, f64) = (240.0, 92.0);
pub const PEER_SIZE: (f64, f64) = (260.0, 92.0);
pub const AGENT_SIZE: (f64, f64) = (190.0, 64.0);
/// Below the canvas's floating hint.
const TOP: f64 = 48.0;
const LEFT: f64 = 30.0;
const PEER_DX: f64 = 300.0;
const PEER_DY: f64 = 108.0;
const AGENT_DY: f64 = 76.0;

fn clip(text: &str, n: usize) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let mut out: String = line.chars().take(n).collect();
    if line.chars().count() > n {
        out.push('…');
    }
    out
}

fn agent_live(status: &str) -> bool {
    !agents::TERMINAL.contains(&status)
}

fn peer_tone(status: Status) -> Tone {
    match status {
        Status::Working => Tone::Working,
        Status::WaitingApproval | Status::WaitingAnswer => Tone::Waiting,
        Status::Requested | Status::Starting | Status::StillStarting => Tone::Idle,
        Status::Finished | Status::Stopped => Tone::Done,
        Status::Failed | Status::Unknown => Tone::Failed,
    }
}

fn session_tone(status: sidebar::Status) -> Tone {
    match status {
        sidebar::Status::Running => Tone::Working,
        sidebar::Status::Waiting => Tone::Waiting,
        sidebar::Status::Done => Tone::Done,
        sidebar::Status::Failed => Tone::Failed,
        sidebar::Status::Idle => Tone::Idle,
    }
}

/// Whether the Flow tab has something to draw: the active session sent out
/// at least one peer or a running agent (one card alone is not a graph).
pub fn available(store: &Store) -> bool {
    let Some(active) = store.active_session() else {
        return false;
    };
    let now = peers::now_ms();
    let rows = fleetview::rows(store, now);
    let from_active = |slug: &str| {
        store
            .domains
            .peer
            .get(slug)
            .and_then(|p| p.origin_session_id)
            .is_none_or(|o| o == active)
    };
    rows.iter().any(|r| from_active(&r.slug))
        || store
            .domains
            .autonomy
            .agents()
            .iter()
            .any(|a| a.session_id == active && agent_live(&a.status))
}

/// The cards for the active session, laid out (a dragged card where it was
/// left, `placed`; the default arrangement otherwise).
pub fn build(store: &Store, now_ms: u64, placed: &HashMap<String, (f64, f64)>) -> Vec<Node> {
    let Some(active) = store.active_session() else {
        return Vec::new();
    };
    let sessions = store.sessions();
    let rows = fleetview::rows(store, now_ms);
    // Each peer row with the session that staged it.
    let peered: Vec<(String, fleetview::FleetRow)> = rows
        .into_iter()
        .map(|r| {
            let origin = store
                .domains
                .peer
                .get(&r.slug)
                .and_then(|p| p.origin_session_id)
                .unwrap_or_else(|| active.clone());
            (origin, r)
        })
        .collect();
    // The sessions: the active one first, then the other origins in order.
    let mut outers: Vec<String> = vec![active.clone()];
    for (o, _) in &peered {
        if !outers.contains(o) {
            outers.push(o.clone());
        }
    }
    let live: Vec<_> = store
        .domains
        .autonomy
        .agents()
        .into_iter()
        .filter(|a| agent_live(&a.status))
        .collect();

    let at = |key: &str, default: (f64, f64)| placed.get(key).copied().unwrap_or(default);
    let mut out: Vec<Node> = Vec::new();
    let mut y = TOP;
    for sid in &outers {
        let key = format!("s:{sid}");
        let index = sessions.iter().position(|s| &s.id == sid);
        let title = sessions
            .iter()
            .find(|s| &s.id == sid)
            .map(switcher::title_of)
            .unwrap_or_else(|| switcher::known_session_title(sid));
        let status = sidebar::session_status(store, sid, Some(active.as_str()));
        let my_peers: Vec<&fleetview::FleetRow> = peered
            .iter()
            .filter(|(o, _)| o == sid)
            .map(|(_, r)| r)
            .collect();
        let my_agents: Vec<_> = live
            .iter()
            .filter(|a| &a.session_id == sid && a.parent_agent_id.is_none())
            .collect();
        let pos = at(&key, (LEFT, y));
        let tall = (my_peers.len() as f64 * PEER_DY).max(
            SESSION_SIZE.1
                + my_agents.len() as f64 * AGENT_DY
                + if my_agents.is_empty() { 0.0 } else { 20.0 },
        );
        y = y.max(pos.1) + tall + 36.0;
        let counts = if my_peers.is_empty() {
            tr("No peers yet").to_owned()
        } else {
            let working = my_peers
                .iter()
                .filter(|r| r.status == Status::Working)
                .count();
            format!(
                "{} · {}",
                crate::i18n::tr1("{value0} peers", &my_peers.len().to_string()),
                crate::i18n::tr1("{value0} working", &working.to_string())
            )
        };
        out.push(Node {
            key: key.clone(),
            kind: Kind::Session,
            parent: None,
            link: Link::Peer,
            tone: session_tone(status),
            lit: sid == &active,
            active: false,
            pos,
            size: SESSION_SIZE,
            title: clip(&title, 48),
            tag: tr("Session").to_owned(),
            status: tr(sidebar::status_label(status, false)).to_owned(),
            detail: counts,
            target: Target::Session {
                id: sid.clone(),
                index,
            },
        });
        // Its own agents, under its card.
        for (k, a) in my_agents.iter().enumerate() {
            let akey = format!("a:{}", a.agent_id);
            out.push(agent_node(
                a,
                &akey,
                Some(key.clone()),
                at(
                    &akey,
                    (
                        pos.0 + 24.0,
                        pos.1 + SESSION_SIZE.1 + 20.0 + k as f64 * AGENT_DY,
                    ),
                ),
            ));
        }
        // Its peers, to its right; each one's agents to the peer's right.
        for (j, r) in my_peers.iter().enumerate() {
            let pkey = format!("p:{}", r.key);
            let ppos = at(&pkey, (pos.0 + PEER_DX, pos.1 + j as f64 * PEER_DY));
            let detail = if r.title.is_empty() {
                r.label.clone()
            } else {
                r.title.clone()
            };
            let mut status = format!("{} {}", r.status.glyph(), fleetview::status_word(r.status));
            if r.elapsed_ms > 0 || r.tokens > 0 {
                status.push_str(&format!(
                    " · {} · {}",
                    peers::format_elapsed(r.elapsed_ms),
                    peers::format_tokens(r.tokens)
                ));
            }
            out.push(Node {
                key: pkey.clone(),
                kind: Kind::Peer,
                parent: Some(key.clone()),
                link: Link::Peer,
                tone: peer_tone(r.status),
                lit: false,
                active: r.status == Status::Working,
                pos: ppos,
                size: PEER_SIZE,
                title: r.label.clone(),
                tag: tr("Peer").to_owned(),
                status,
                detail: clip(&detail, 60),
                target: Target::Fleet,
            });
            let peer_agents: Vec<_> = live
                .iter()
                .filter(|a| {
                    a.parent_agent_id.is_none()
                        && (a.session_id == r.key
                            || r.identity.as_deref() == Some(a.session_id.as_str()))
                })
                .collect();
            for (k, a) in peer_agents.iter().enumerate() {
                let akey = format!("a:{}", a.agent_id);
                out.push(agent_node(
                    a,
                    &akey,
                    Some(pkey.clone()),
                    at(
                        &akey,
                        (ppos.0 + PEER_SIZE.0 + 40.0, ppos.1 + k as f64 * AGENT_DY),
                    ),
                ));
            }
        }
    }
    // Agents an agent sent: under their parent's card, when it is shown.
    let mut pending: Vec<_> = live
        .iter()
        .filter(|a| a.parent_agent_id.is_some())
        .collect();
    loop {
        let before = pending.len();
        pending.retain(|a| {
            let parent = format!("a:{}", a.parent_agent_id.as_deref().unwrap_or_default());
            let Some(p) = out.iter().find(|n| n.key == parent).cloned() else {
                return true;
            };
            let siblings = out
                .iter()
                .filter(|n| n.parent.as_deref() == Some(parent.as_str()))
                .count();
            let akey = format!("a:{}", a.agent_id);
            let default = (
                p.pos.0 + 24.0,
                p.pos.1 + p.size.1 + 20.0 + siblings as f64 * AGENT_DY,
            );
            out.push(agent_node(a, &akey, Some(parent), at(&akey, default)));
            false
        });
        if pending.is_empty() || pending.len() == before {
            break;
        }
    }
    out
}

fn agent_node(
    a: &octoscode_store::domains::autonomy::AgentRecord,
    key: &str,
    parent: Option<String>,
    pos: (f64, f64),
) -> Node {
    let name = if a.nickname.trim().is_empty() {
        a.role.clone()
    } else {
        a.nickname.clone()
    };
    let what = a
        .last_task
        .clone()
        .or_else(|| a.title.clone())
        .or_else(|| a.summary.clone())
        .unwrap_or_default();
    Node {
        key: key.to_owned(),
        kind: Kind::Agent,
        parent,
        link: Link::Sub,
        tone: Tone::Working,
        lit: false,
        active: true,
        pos,
        size: AGENT_SIZE,
        title: clip(&name, 40),
        tag: tr("Agent").to_owned(),
        status: a.status.clone(),
        detail: clip(&what, 60),
        target: Target::Agents,
    }
}

// ------------------------------------------------------- where cards sit

fn layout_path() -> PathBuf {
    if let Ok(p) = std::env::var("OCTOSCODE_FLOW_LAYOUT_FILE") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".octoscode")
        .join("flow-layout-v1.json")
}

fn layout() -> MutexGuard<'static, HashMap<String, (f64, f64)>> {
    static L: OnceLock<Mutex<HashMap<String, (f64, f64)>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(load(&layout_path())))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Read the saved positions (`{"<card key>": [x, y], …}`); a missing or
/// unreadable file is an empty layout.
pub fn load(path: &std::path::Path) -> HashMap<String, (f64, f64)> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    let Ok(map) = serde_json::from_str::<HashMap<String, (f64, f64)>>(&text) else {
        return HashMap::new();
    };
    map.into_iter()
        .filter(|(_, (x, y))| x.is_finite() && y.is_finite())
        .collect()
}

/// The positions the person gave cards.
pub fn placed() -> HashMap<String, (f64, f64)> {
    layout().clone()
}

/// A card was dragged to `pos`: remember it (best effort on disk).
pub fn place(key: &str, pos: (f64, f64)) {
    let mut l = layout();
    l.insert(key.to_owned(), pos);
    let path = layout_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(&*l) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_store::domains::autonomy::AgentRecord;
    use octoscode_store::domains::peer::{Origin, Peer, PeerRow, RowStatus};
    use octoscode_store::Session;

    fn session(id: &str, title: &str) -> Session {
        Session {
            id: id.into(),
            title: Some(title.into()),
            message_count: 1,
            updated_at: None,
            last_prompt: None,
            active_turn: false,
        }
    }

    fn agent(id: &str, session: &str, parent: Option<&str>, status: &str) -> AgentRecord {
        AgentRecord {
            agent_id: id.into(),
            session_id: session.into(),
            profile_id: "dev".into(),
            path: String::new(),
            role: "worker".into(),
            nickname: format!("agent {id}"),
            backend_kind: "native".into(),
            status: status.into(),
            title: None,
            parent_agent_id: parent.map(Into::into),
            task_id: None,
            artifact_count: 0,
            output_tail: None,
            updated_at_ms: 0,
            last_task: Some(format!("task of {id}")),
            summary: None,
        }
    }

    fn staged(store: &Store, slug: &str, origin: &str, started: bool) {
        let mut p = Peer::named(slug);
        p.origin_session_id = Some(origin.into());
        store.domains.peer.observe_staged(p);
        let mut row = PeerRow::opening(
            &format!("dev:local:tui#peer-{slug}"),
            slug,
            Origin::Staged,
            "t",
            0,
        );
        if started {
            row.status = RowStatus::Started;
        }
        row.brief = format!("Brief for {slug}\nmore");
        store.domains.peer.stage_row(row, false);
    }

    fn store() -> Store {
        let store = Store::new();
        store.set_sessions(vec![
            session("dev:api:a", "Main work"),
            session("dev:api:b", "Earlier"),
        ]);
        store.set_active(Some("dev:api:a".into()));
        store
    }

    #[test]
    fn nothing_to_draw_without_peers_or_running_agents() {
        let store = store();
        assert!(!available(&store));
        store
            .domains
            .autonomy
            .upsert_agent(agent("x", "dev:api:a", None, "completed"));
        assert!(!available(&store), "a finished agent is not drawn");
        store
            .domains
            .autonomy
            .upsert_agent(agent("y", "dev:api:a", None, "running"));
        assert!(available(&store));
    }

    #[test]
    fn peers_hang_from_the_session_that_staged_them() {
        let store = store();
        staged(&store, "tests", "dev:api:a", true);
        staged(&store, "docs", "dev:api:b", false);
        assert!(available(&store));
        let nodes = build(&store, 1_000, &HashMap::new());
        let keys: Vec<&str> = nodes.iter().map(|n| n.key.as_str()).collect();
        assert_eq!(keys[0], "s:dev:api:a", "the active session first");
        let a = nodes.iter().find(|n| n.key == "s:dev:api:a").unwrap();
        assert!(a.lit);
        assert_eq!(a.title, "Main work");
        assert_eq!(
            a.target,
            Target::Session {
                id: "dev:api:a".into(),
                index: Some(0)
            }
        );
        let b = nodes.iter().find(|n| n.key == "s:dev:api:b").unwrap();
        assert!(!b.lit);
        let tests = nodes
            .iter()
            .find(|n| n.kind == Kind::Peer && n.detail.starts_with("Brief for tests"))
            .unwrap();
        assert_eq!(tests.parent.as_deref(), Some("s:dev:api:a"));
        assert_eq!(tests.tone, Tone::Working);
        assert!(tests.active, "a working peer's line moves");
        assert!(
            tests.title.starts_with("Peer "),
            "never the slug: {}",
            tests.title
        );
        assert_eq!(tests.target, Target::Fleet);
        let docs = nodes
            .iter()
            .find(|n| n.kind == Kind::Peer && n.detail.starts_with("Brief for docs"))
            .unwrap();
        assert_eq!(docs.parent.as_deref(), Some("s:dev:api:b"));
        assert_eq!(
            docs.tone,
            Tone::Idle,
            "a peer still opening is not working yet"
        );
        // Default layout: peers to the right of their session.
        assert!(tests.pos.0 > a.pos.0 && docs.pos.0 > b.pos.0);
        assert!(b.pos.1 > a.pos.1, "the sessions stack");
    }

    #[test]
    fn running_agents_hang_from_their_parent_agent_or_session() {
        let store = store();
        store
            .domains
            .autonomy
            .upsert_agent(agent("lead", "dev:api:a", None, "running"));
        store
            .domains
            .autonomy
            .upsert_agent(agent("child", "dev:api:a", Some("lead"), "running"));
        store
            .domains
            .autonomy
            .upsert_agent(agent("gone", "dev:api:a", None, "completed"));
        let nodes = build(&store, 0, &HashMap::new());
        let lead = nodes.iter().find(|n| n.key == "a:lead").unwrap();
        assert_eq!(lead.parent.as_deref(), Some("s:dev:api:a"));
        assert_eq!(lead.link, Link::Sub);
        assert_eq!(lead.detail, "task of lead");
        let child = nodes.iter().find(|n| n.key == "a:child").unwrap();
        assert_eq!(child.parent.as_deref(), Some("a:lead"));
        assert!(child.pos.1 > lead.pos.1, "under its parent");
        assert!(!nodes.iter().any(|n| n.key == "a:gone"));
    }

    #[test]
    fn a_placed_card_keeps_its_place() {
        let store = store();
        staged(&store, "tests", "dev:api:a", true);
        let mut placed = HashMap::new();
        placed.insert("s:dev:api:a".to_owned(), (500.0, 400.0));
        let nodes = build(&store, 0, &placed);
        assert_eq!(nodes[0].pos, (500.0, 400.0));
        let peer = nodes.iter().find(|n| n.kind == Kind::Peer).unwrap();
        assert_eq!(
            peer.pos,
            (500.0 + PEER_DX, 400.0),
            "its peers follow it by default"
        );
    }

    #[test]
    fn the_layout_file_round_trips_and_tolerates_junk() {
        let dir = tempdir();
        let path = dir.join("flow.json");
        std::fs::write(&path, r#"{"s:x":[1.5,2.0],"p:y":[3,4]}"#).unwrap();
        let l = load(&path);
        assert_eq!(l.get("s:x"), Some(&(1.5, 2.0)));
        assert_eq!(l.get("p:y"), Some(&(3.0, 4.0)));
        std::fs::write(&path, "not json").unwrap();
        assert!(load(&path).is_empty());
        assert!(load(&dir.join("missing.json")).is_empty());
    }

    fn tempdir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "octoscode-flow-{}-{}",
            std::process::id(),
            peers::now_ms()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
