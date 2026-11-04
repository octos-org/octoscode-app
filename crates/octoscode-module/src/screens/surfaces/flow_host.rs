//! The host glue for the Flow tab: an `impl OctoscodeView` block (as
//! `host.rs` is for the other surfaces). `sync_flow` lays the cards out from
//! the store into the canvas and fills them; `flow_actions` carries out what
//! the person did on it.
use std::sync::Mutex;

use makepad_widgets::*;
use octoscode_store::Store;

use crate::flow_canvas::{Card, FlowAction, FlowCanvasWidgetRefExt, Link as CanvasLink};
use crate::i18n::tr;
use crate::screens::flow_graph::{self, Kind, Link, Node, Target, Tone};

/// What the last sync drew: the nodes (a click names one by key), the
/// session the canvas was last brought to, and a signature of the cards (a
/// change redraws the canvas).
#[derive(Default)]
struct Drawn {
    nodes: Vec<Node>,
    focused: Option<String>,
    signature: String,
}

fn drawn() -> std::sync::MutexGuard<'static, Drawn> {
    static D: std::sync::OnceLock<Mutex<Drawn>> = std::sync::OnceLock::new();
    D.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// The card template for a node (the ids the Flow view's DSL names).
pub fn template(n: &Node) -> LiveId {
    match (n.kind, n.lit, n.tone) {
        (Kind::Agent, _, _) => live_id!(Agent),
        (Kind::Session, true, _) => live_id!(SessionLit),
        (_, _, Tone::Working) => live_id!(Working),
        (_, _, Tone::Waiting) => live_id!(Waiting),
        (_, _, Tone::Failed) => live_id!(Failed),
        (_, _, Tone::Done) => live_id!(Done),
        (Kind::Session, false, Tone::Idle) => live_id!(Session),
        (_, _, Tone::Idle) => live_id!(Idle),
    }
}

/// What a click on the card `key` asks the host to perform, in order.
pub fn open_actions(nodes: &[Node], key: &str, active: Option<&str>) -> Vec<(&'static str, usize)> {
    let Some(n) = nodes.iter().find(|n| n.key == key) else {
        return Vec::new();
    };
    match &n.target {
        Target::Session { id, .. } if Some(id.as_str()) == active => vec![("cv.tab.chat", 0)],
        Target::Session { index: Some(i), .. } => vec![("cv.tab.chat", 0), ("session.open", *i)],
        Target::Session { index: None, .. } => Vec::new(),
        Target::Fleet => vec![("b3.open.fleet", 0)],
        Target::Agents => vec![("b3.open.agents", 0)],
    }
}

impl crate::OctoscodeView {
    /// Lay the Flow out from the store (only while its tab is shown).
    pub(crate) fn sync_flow(&mut self, cx: &mut Cx, store: &Store) {
        let nodes = flow_graph::build(
            store,
            crate::screens::peers::now_ms(),
            &flow_graph::placed(),
        );
        let cards: Vec<Card> = nodes
            .iter()
            .map(|n| Card {
                id: LiveId::from_str(&n.key),
                key: n.key.clone(),
                template: template(n),
                pos: dvec2(n.pos.0, n.pos.1),
                size: dvec2(n.size.0, n.size.1),
                parent: n.parent.as_deref().map(LiveId::from_str),
                link: match n.link {
                    Link::Peer => CanvasLink::Peer,
                    Link::Sub => CanvasLink::Sub,
                },
                active: n.active,
                lit: n.lit,
            })
            .collect();
        let canvas = self.view.widget(cx, ids!(flow_canvas));
        let widgets = canvas.as_flow_canvas().set_cards(cx, cards);
        for (w, n) in widgets.iter().zip(&nodes) {
            w.label(cx, ids!(title)).set_text(cx, &n.title);
            w.label(cx, ids!(status)).set_text(cx, &n.status);
            w.label(cx, ids!(detail)).set_text(cx, &n.detail);
            w.label(cx, ids!(kind.tag)).set_text(cx, &n.tag);
        }
        self.view.label(cx, ids!(flow_hint)).set_text(
            cx,
            tr("Drag a card to move it, drag the canvas to pan. Click a card to open it."),
        );
        let signature = format!("{nodes:?}");
        let active = store.active_session();
        let mut d = drawn();
        if d.signature != signature {
            d.signature = signature;
            canvas.redraw(cx);
        }
        // Opened on a session (or another one chosen): its card comes into view.
        if d.focused != active {
            d.focused = active.clone();
            if let Some(id) = active {
                canvas
                    .as_flow_canvas()
                    .focus(cx, LiveId::from_str(&format!("s:{id}")));
            }
        }
        d.nodes = nodes;
    }

    /// The Flow tab's clicks and drags.
    pub(crate) fn flow_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let canvas = self.view.widget(cx, ids!(flow_canvas)).as_flow_canvas();
        for action in canvas.actions(actions) {
            match action {
                FlowAction::Open(key) => {
                    let active = { self.bridge.lock().unwrap().store.active_session() };
                    let todo = open_actions(&drawn().nodes, &key, active.as_deref());
                    makepad_widgets::log!("[octoscode] flow: open {key} -> {todo:?}");
                    for (action, index) in todo {
                        self.perform_action(cx, action, index);
                    }
                }
                FlowAction::Placed(key, pos) => flow_graph::place(&key, (pos.x, pos.y)),
                FlowAction::None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(key: &str, kind: Kind, target: Target, tone: Tone, lit: bool) -> Node {
        Node {
            key: key.into(),
            kind,
            parent: None,
            link: Link::Peer,
            tone,
            lit,
            active: false,
            pos: (0.0, 0.0),
            size: (1.0, 1.0),
            title: String::new(),
            tag: String::new(),
            status: String::new(),
            detail: String::new(),
            target,
        }
    }

    #[test]
    fn a_click_opens_what_the_card_stands_for() {
        let nodes = vec![
            node(
                "s:a",
                Kind::Session,
                Target::Session {
                    id: "a".into(),
                    index: Some(0),
                },
                Tone::Idle,
                true,
            ),
            node(
                "s:b",
                Kind::Session,
                Target::Session {
                    id: "b".into(),
                    index: Some(3),
                },
                Tone::Idle,
                false,
            ),
            node(
                "s:c",
                Kind::Session,
                Target::Session {
                    id: "c".into(),
                    index: None,
                },
                Tone::Idle,
                false,
            ),
            node("p:x", Kind::Peer, Target::Fleet, Tone::Working, false),
            node("a:y", Kind::Agent, Target::Agents, Tone::Working, false),
        ];
        assert_eq!(
            open_actions(&nodes, "s:a", Some("a")),
            vec![("cv.tab.chat", 0)],
            "the active session: back to its chat"
        );
        assert_eq!(
            open_actions(&nodes, "s:b", Some("a")),
            vec![("cv.tab.chat", 0), ("session.open", 3)]
        );
        assert!(
            open_actions(&nodes, "s:c", Some("a")).is_empty(),
            "a session not in the list opens nothing"
        );
        assert_eq!(
            open_actions(&nodes, "p:x", Some("a")),
            vec![("b3.open.fleet", 0)]
        );
        assert_eq!(
            open_actions(&nodes, "a:y", Some("a")),
            vec![("b3.open.agents", 0)]
        );
        assert!(open_actions(&nodes, "gone", Some("a")).is_empty());
    }

    #[test]
    fn each_tone_has_its_template() {
        let t = |kind, tone, lit| template(&node("k", kind, Target::Fleet, tone, lit));
        assert_eq!(
            t(Kind::Session, Tone::Working, true),
            live_id!(SessionLit),
            "the active session stays blue"
        );
        assert_eq!(t(Kind::Session, Tone::Idle, false), live_id!(Session));
        assert_eq!(t(Kind::Peer, Tone::Working, false), live_id!(Working));
        assert_eq!(t(Kind::Peer, Tone::Waiting, false), live_id!(Waiting));
        assert_eq!(t(Kind::Peer, Tone::Failed, false), live_id!(Failed));
        assert_eq!(t(Kind::Peer, Tone::Done, false), live_id!(Done));
        assert_eq!(t(Kind::Peer, Tone::Idle, false), live_id!(Idle));
        assert_eq!(t(Kind::Agent, Tone::Working, false), live_id!(Agent));
    }
}
