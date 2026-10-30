//! The Flow tab's canvas: cards joined by lines, on a surface the person
//! drags and scrolls ([`crate::screens::flow_graph`] is its model).
//!
//! Adapted from OctoBuddy's `FlowCanvas` (`src/flow.rs`, Apache-2.0, same
//! Makepad 2.0 widget API). Kept: the cards made from named templates, a
//! line from a parent's right edge to its child's left edge (an arrowhead at
//! the child, and at the parent too for a peer that reports back), dots
//! moving along the line of a card at work, dragging a card, panning by drag
//! or scroll, and bringing a card into view. Left out with the orchestration
//! they belong to: waves and their gates, dropping a card on another outer
//! loop, the right-click menu, and the panels drawn over the canvas.
//!
//! - Drag a card to place it (`FlowAction::Placed`; the host keeps where).
//! - Click a card to open it (`FlowAction::Open` with its key).
//! - Drag the canvas, or scroll, to pan.
use makepad_widgets::*;
use std::collections::HashMap;

script_mod! {
    use mod.prelude.widgets.*

    mod.widgets.OcFlowCanvasBase = #(FlowCanvas::register_widget(vm))
    mod.widgets.OcFlowCanvas = set_type_default() do mod.widgets.OcFlowCanvasBase{
        width: Fill height: Fill
        draw_bg +: {color: theme.color_bg_app}
        line_color: #x8E8E93FF
        lit_color: #x2F6FEBFF
    }

    // The tag beside a card's title.
    let OcFlowTag = RoundedView{
        width: Fit height: Fit new_batch: true padding: Inset{left: 5 right: 5 top: 1 bottom: 1}
        draw_bg +: {color: #x8E8E9326 border_radius: 3.0}
        tag := Label{
            width: Fit height: Fit padding: 0 text: ""
            draw_text +: {color: theme.color_text_muted text_style: theme.oc_text_caps{font_size: 7.5}}
        }
    }
    // One card: title and tag, the status line, the detail line. The tone
    // variants below only recolour its border and fill.
    let OcFlowCard = RoundedView{
        width: Fill height: Fill new_batch: true flow: Down spacing: 4
        padding: Inset{left: 12 right: 12 top: 10 bottom: 10}
        draw_bg +: {color: theme.color_bg_container border_radius: 6.0 border_size: 1.0 border_color: #x8E8E9359}
        View{
            width: Fill height: Fit flow: Right spacing: 6 align: Align{y: 0.5}
            title := Label{
                width: Fill height: Fit padding: 0 text: "" max_lines: 1 text_overflow: TextOverflow.Ellipsis
                draw_text +: {color: theme.color_fg_app text_style: theme.oc_text_brand{font_size: 9.75}}
            }
            kind := OcFlowTag{}
        }
        status := Label{
            width: Fill height: Fit padding: 0 text: "" max_lines: 1 text_overflow: TextOverflow.Ellipsis
            draw_text +: {color: theme.color_text_muted text_style: theme.oc_text_row{font_size: 8.5}}
        }
        detail := Label{
            width: Fill height: Fit padding: 0 text: "" max_lines: 1 text_overflow: TextOverflow.Ellipsis
            draw_text +: {color: theme.color_fg_app text_style: theme.oc_text_row{font_size: 8.5}}
        }
    }
    mod.widgets.OcFlowCard = OcFlowCard{}
    // The fill stays the theme's opaque container colour (a card dragged
    // over another hides it); the border carries the state.
    // The active session: blue.
    mod.widgets.OcFlowCardLit = OcFlowCard{draw_bg +: {border_size: 2.0 border_color: #x2F6FEBFF}}
    // At work: green. Waiting on the person: amber. Failed: red. Ended: faint.
    mod.widgets.OcFlowCardWorking = OcFlowCard{draw_bg +: {border_size: 2.0 border_color: #x1F883DFF}}
    mod.widgets.OcFlowCardWaiting = OcFlowCard{draw_bg +: {border_size: 2.0 border_color: #xD08700FF}}
    mod.widgets.OcFlowCardFailed = OcFlowCard{draw_bg +: {border_size: 2.0 border_color: #xCF222EFF}}
    mod.widgets.OcFlowCardDone = OcFlowCard{draw_bg +: {border_color: #x8E8E9340}}
    // An agent: sent for one job, shown while it runs.
    mod.widgets.OcFlowCardAgent = OcFlowCard{
        padding: Inset{left: 10 right: 10 top: 7 bottom: 7} spacing: 3
        draw_bg +: {border_size: 1.5 border_color: #x8250DFFF}
    }
}

/// One card, as the canvas lays it out.
#[derive(Clone, Debug)]
pub struct Card {
    pub id: LiveId,
    /// What the card stands for (`flow_graph::Node::key`).
    pub key: String,
    pub template: LiveId,
    /// Its top-left corner, in canvas points.
    pub pos: DVec2,
    pub size: DVec2,
    /// The card it hangs from.
    pub parent: Option<LiveId>,
    pub link: Link,
    /// At work: its line moves.
    pub active: bool,
    /// Drawn with the accent line (the active session's).
    pub lit: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Link {
    /// Both ways: a peer reports back.
    #[default]
    Peer,
    /// One way: an agent is sent.
    Sub,
}

#[derive(Clone, Debug, Default)]
pub enum FlowAction {
    #[default]
    None,
    /// A card was clicked (its key).
    Open(String),
    /// A card was dragged to a new place (its key, canvas points).
    Placed(String, DVec2),
}

enum Drag {
    Card {
        index: usize,
        grab: DVec2,
        start: DVec2,
        moved: bool,
    },
    Pan {
        last: DVec2,
    },
}

#[derive(Script, Widget)]
pub struct FlowCanvas {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[redraw]
    #[live]
    draw_bg: DrawColor,
    #[live]
    draw_vector: DrawVector,
    #[live]
    line_color: Vec4f,
    #[live]
    lit_color: Vec4f,
    #[rust]
    templates: HashMap<LiveId, ScriptObjectRef>,
    #[rust]
    widgets: HashMap<LiveId, (LiveId, WidgetRef)>,
    #[rust]
    cards: Vec<Card>,
    #[rust]
    pan: DVec2,
    #[rust]
    origin: DVec2,
    /// The canvas's size when last drawn.
    #[rust]
    seen: DVec2,
    #[rust]
    drag: Option<Drag>,
    #[rust]
    area: Area,
    /// Where the moving dots are, 0..1 along each line.
    #[rust]
    phase: f64,
    #[rust]
    next_frame: NextFrame,
}

impl ScriptHook for FlowCanvas {
    fn on_before_apply(
        &mut self,
        _vm: &mut ScriptVm,
        apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        if apply.is_reload() {
            self.templates.clear();
        }
    }

    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        apply: &Apply,
        _scope: &mut Scope,
        value: ScriptValue,
    ) {
        // The card templates, as FlatList keeps its items'.
        if !apply.is_eval() {
            if let Some(obj) = value.as_object() {
                vm.vec_with(obj, |vm, vec| {
                    for kv in vec {
                        if let (Some(id), Some(template)) = (kv.key.as_id(), kv.value.as_object()) {
                            self.templates
                                .insert(id, vm.bx.heap.new_object_ref(template));
                        }
                    }
                });
            }
            // Fresh templates (a hot reload, or the live re-theme's
            // re-apply, `a26_host::retheme`): the cards are made again from
            // them on the next `set_cards`, which the host fills anew.
            self.widgets.clear();
        }
    }
}

impl FlowCanvas {
    /// The cards to draw, and the widget of each (made from its template on
    /// first use, for the host to fill).
    pub fn set_cards(&mut self, cx: &mut Cx, cards: Vec<Card>) -> Vec<WidgetRef> {
        let mut out = Vec::new();
        for card in &cards {
            let fresh = self
                .widgets
                .get(&card.id)
                .is_none_or(|(t, _)| *t != card.template);
            if fresh {
                if let Some(t) = self.templates.get(&card.template) {
                    let value: ScriptValue = t.as_object().into();
                    let widget = cx.with_vm(|vm| WidgetRef::script_from_value(vm, value));
                    cx.widget_tree_insert_child(self.widget_uid(), card.id, widget.clone());
                    self.widgets.insert(card.id, (card.template, widget));
                }
            }
            out.push(
                self.widgets
                    .get(&card.id)
                    .map(|(_, w)| w.clone())
                    .unwrap_or_default(),
            );
        }
        self.widgets
            .retain(|id, _| cards.iter().any(|c| c.id == *id));
        // A card being dragged keeps where the person holds it.
        if let Some(Drag::Card { index, .. }) = &self.drag {
            if let (Some(old), Some(new)) = (self.cards.get(*index), cards.get(*index)) {
                if old.id != new.id {
                    self.drag = None;
                }
            }
        }
        let held = match &self.drag {
            Some(Drag::Card {
                index, moved: true, ..
            }) => self.cards.get(*index).map(|c| (c.id, c.pos)),
            _ => None,
        };
        self.cards = cards;
        if let Some((id, pos)) = held {
            if let Some(c) = self.cards.iter_mut().find(|c| c.id == id) {
                c.pos = pos;
            }
        }
        out
    }

    /// Brings a card into view, unless it already is: the canvas stays where
    /// the person put it.
    pub fn focus(&mut self, cx: &mut Cx, id: LiveId) {
        if let Some(card) = self.cards.iter().find(|c| c.id == id) {
            let at = self.pan + card.pos;
            let end = at + card.size;
            if self.seen.x > 0.0
                && at.x >= 0.0
                && at.y >= 0.0
                && end.x <= self.seen.x
                && end.y <= self.seen.y
            {
                return;
            }
            self.pan = dvec2(30.0, 48.0) - card.pos;
            self.redraw(cx);
        }
    }

    fn screen(&self, pos: DVec2) -> DVec2 {
        self.origin + self.pan + pos
    }

    /// The top card under a point (the last drawn is on top).
    fn card_at(&self, abs: DVec2) -> Option<usize> {
        self.cards
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| {
                Rect {
                    pos: self.screen(c.pos),
                    size: c.size,
                }
                .contains(abs)
            })
            .map(|(i, _)| i)
    }

    /// A line from `from` (a parent's edge) to `to` (a child's edge):
    /// arrowheads at the child, and at the parent too for a peer; dots
    /// running along it while the child works. `dir` is the way it leaves
    /// the parent: right, or down (a card below).
    #[allow(clippy::too_many_arguments)]
    fn draw_line(
        &mut self,
        from: DVec2,
        to: DVec2,
        dir: DVec2,
        color: Vec4f,
        width: f32,
        link: Link,
        active: bool,
    ) {
        let along = (to - from).x * dir.x + (to - from).y * dir.y;
        let bend = (along.abs() * 0.5).max(30.0);
        let (c1, c2) = (from + dir * bend, to - dir * bend);
        // The stroke stops short of the heads, so their tips stay sharp.
        let head = 7.0;
        let start = if link == Link::Peer {
            from + dir * head
        } else {
            from
        };
        let end = to - dir * head;
        self.draw_vector
            .set_color(color.x, color.y, color.z, color.w);
        self.draw_vector.move_to(start.x as f32, start.y as f32);
        self.draw_vector.bezier_to(
            c1.x as f32,
            c1.y as f32,
            c2.x as f32,
            c2.y as f32,
            end.x as f32,
            end.y as f32,
        );
        self.draw_vector.stroke(width);
        self.arrow(to, dir, head, color);
        if link == Link::Peer {
            self.arrow(from, -dir, head, color);
        }
        if active {
            for k in 0..3 {
                let t = (self.phase + k as f64 / 3.0).fract();
                let u = 1.0 - t;
                let p = from * (u * u * u)
                    + c1 * (3.0 * u * u * t)
                    + c2 * (3.0 * u * t * t)
                    + to * (t * t * t);
                self.draw_vector
                    .set_color(color.x, color.y, color.z, color.w);
                self.draw_vector.circle(p.x as f32, p.y as f32, 3.0);
                self.draw_vector.fill();
            }
        }
    }

    /// A filled arrowhead with its tip at `tip`, pointing along `dir`.
    fn arrow(&mut self, tip: DVec2, dir: DVec2, size: f64, color: Vec4f) {
        let n = dvec2(-dir.y, dir.x) * (size * 0.55);
        let back = tip - dir * size;
        self.draw_vector
            .set_color(color.x, color.y, color.z, color.w);
        self.draw_vector.move_to(tip.x as f32, tip.y as f32);
        self.draw_vector
            .line_to((back.x + n.x) as f32, (back.y + n.y) as f32);
        self.draw_vector
            .line_to((back.x - n.x) as f32, (back.y - n.y) as f32);
        self.draw_vector.close();
        self.draw_vector.fill();
    }
}

impl Widget for FlowCanvas {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        // The dots move while any card works.
        if let Some(ev) = self.next_frame.is_event(event) {
            self.phase = (ev.time * 0.6).fract();
            if self.cards.iter().any(|c| c.active) {
                self.redraw(cx);
            }
        }
        match event.hits(cx, self.area) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => match self.card_at(fe.abs) {
                Some(index) => {
                    let grab = fe.abs - self.screen(self.cards[index].pos);
                    self.drag = Some(Drag::Card {
                        index,
                        grab,
                        start: fe.abs,
                        moved: false,
                    });
                }
                None => self.drag = Some(Drag::Pan { last: fe.abs }),
            },
            Hit::FingerMove(fe) => match &mut self.drag {
                Some(Drag::Card {
                    index,
                    grab,
                    start,
                    moved,
                }) => {
                    let index = *index;
                    if !*moved && (fe.abs - *start).length() > 4.0 {
                        *moved = true;
                    }
                    if *moved {
                        let pos = fe.abs - *grab - self.origin - self.pan;
                        self.cards[index].pos = pos;
                        self.redraw(cx);
                    }
                }
                Some(Drag::Pan { last }) => {
                    self.pan += fe.abs - *last;
                    *last = fe.abs;
                    self.redraw(cx);
                }
                None => {}
            },
            Hit::FingerUp(fe) if fe.is_primary_hit() => match self.drag.take() {
                Some(Drag::Card {
                    index,
                    moved: false,
                    ..
                }) => {
                    cx.widget_action(uid, FlowAction::Open(self.cards[index].key.clone()));
                }
                Some(Drag::Card { index, .. }) => {
                    let card = &self.cards[index];
                    cx.widget_action(uid, FlowAction::Placed(card.key.clone(), card.pos));
                    self.redraw(cx);
                }
                _ => {}
            },
            Hit::FingerScroll(fe) => {
                self.pan -= fe.scroll;
                self.redraw(cx);
            }
            Hit::FingerHoverOver(fe) => {
                let over_card = self.card_at(fe.abs).is_some();
                cx.set_cursor(if over_card {
                    MouseCursor::Hand
                } else {
                    MouseCursor::Default
                });
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.origin = rect.pos;
        self.seen = rect.size;
        self.draw_bg.draw_abs(cx, rect);
        cx.begin_turtle(
            Walk {
                abs_pos: Some(rect.pos),
                width: Size::Fixed(rect.size.x),
                height: Size::Fixed(rect.size.y),
                ..Default::default()
            },
            Layout {
                clip_x: true,
                clip_y: true,
                ..Layout::default()
            },
        );
        // The lines first, under the cards: from a parent's right edge to
        // its child's left edge (an agent below its parent: from the bottom).
        self.draw_vector.begin();
        for i in 0..self.cards.len() {
            let card = self.cards[i].clone();
            let Some(p) = card
                .parent
                .and_then(|p| self.cards.iter().position(|c| c.id == p))
            else {
                continue;
            };
            let from = self.cards[p].clone();
            let below = card.link == Link::Sub && card.pos.y >= from.pos.y + from.size.y - 1.0;
            let (a, b, dir) = if below {
                (
                    self.screen(from.pos) + dvec2(from.size.x * 0.3, from.size.y),
                    self.screen(card.pos) + dvec2(card.size.x * 0.5, 0.0),
                    dvec2(0.0, 1.0),
                )
            } else {
                (
                    self.screen(from.pos) + dvec2(from.size.x, from.size.y * 0.5),
                    self.screen(card.pos) + dvec2(0.0, card.size.y * 0.5),
                    dvec2(1.0, 0.0),
                )
            };
            let (color, width) = if card.active || from.lit {
                (self.lit_color, 2.0)
            } else {
                (self.line_color, 1.5)
            };
            self.draw_line(a, b, dir, color, width, card.link, card.active);
        }
        self.draw_vector.end(cx);
        if self.cards.iter().any(|c| c.active) {
            self.next_frame = cx.new_next_frame();
        }
        for card in self.cards.clone() {
            let Some((_, widget)) = self.widgets.get(&card.id).cloned() else {
                continue;
            };
            let walk = Walk::fixed(card.size.x, card.size.y).with_abs_pos(self.screen(card.pos));
            widget.draw_walk_all(cx, scope, walk);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl FlowCanvasRef {
    pub fn focus(&self, cx: &mut Cx, id: LiveId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.focus(cx, id);
        }
    }

    pub fn set_cards(&self, cx: &mut Cx, cards: Vec<Card>) -> Vec<WidgetRef> {
        self.borrow_mut()
            .map(|mut inner| inner.set_cards(cx, cards))
            .unwrap_or_default()
    }

    /// What happened on the canvas this frame, in order.
    pub fn actions(&self, actions: &Actions) -> Vec<FlowAction> {
        let uid = self.widget_uid();
        actions
            .iter()
            .filter_map(|a| a.as_widget_action())
            .filter(|a| a.widget_uid == uid)
            .map(|a| a.cast::<FlowAction>())
            .filter(|a| !matches!(a, FlowAction::None))
            .collect()
    }
}
