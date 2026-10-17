use crate::base::interactive::InteractiveNode;
use crate::context::Context;
use crate::document::Document;
use crate::geometry::{Pos2, Rect};
use crate::input::{Event, Modifiers, PointerEvent, PointerId};
use crate::node::{Element, NodeId, Rects};

use super::hit;
use super::pointer::Routed;

#[derive(Clone, Debug, PartialEq)]
pub enum Forwarded {
    Pointer {
        event: PointerEvent,
        over: Option<NodeId>,
    },
    Left,
    Focus(bool),
    Input(Event),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ForwardedInput {
    pub events: Vec<Forwarded>,
    pub rect: Rect,
    pub hovered: bool,
    pub focused: bool,
    pub pointer: Option<Pos2>,
    pub modifiers: Modifiers,
}

impl ForwardedInput {
    pub fn inputs(&self) -> impl Iterator<Item = &Event> {
        self.events.iter().filter_map(|event| match event {
            Forwarded::Input(event) => Some(event),
            _ => None,
        })
    }

    pub fn pointer_events(&self) -> impl Iterator<Item = &PointerEvent> {
        self.events.iter().filter_map(|event| match event {
            Forwarded::Pointer { event, .. } => Some(event),
            _ => None,
        })
    }
}

#[derive(Default)]
pub struct Routing {
    hovered: Option<NodeId>,
    focused: Option<NodeId>,
    entered: Vec<NodeId>,
}

impl Routing {
    pub(crate) fn hovered(&self) -> Option<NodeId> {
        self.hovered
    }
}

pub fn wants_forward(element: &dyn Element) -> bool {
    element
        .as_any()
        .downcast_ref::<InteractiveNode>()
        .is_some_and(InteractiveNode::forwards)
}

fn sink_at(doc: &mut Document, rects: &Rects, root: NodeId, pos: Pos2) -> Option<NodeId> {
    let hit = super::pointer::hit(doc, rects, root, pos, false);
    hit::sink(doc, rects, &hit.path, pos)
}

pub(crate) fn cycle_focus(doc: &mut Document, step: isize) -> bool {
    let sinks: Vec<NodeId> = doc
        .focusables()
        .into_iter()
        .filter(|id| wants_forward(doc.arena.get(*id)))
        .collect();
    if sinks.is_empty() {
        return false;
    }
    let count = sinks.len() as isize;
    let next = match doc
        .focused_node()
        .and_then(|focused| sinks.iter().position(|sink| *sink == focused))
    {
        Some(index) => (index as isize + step).rem_euclid(count),
        None if step >= 0 => 0,
        None => count - 1,
    };
    doc.focus_focusable(sinks[next as usize]);
    true
}

pub(crate) fn takes_keys(doc: &Document) -> bool {
    doc.focused_node()
        .is_some_and(|focused| doc.arena.contains(focused) && wants_forward(doc.arena.get(focused)))
}

pub(crate) fn shortcuts_inhibited(doc: &Document) -> bool {
    doc.focused_node().is_some_and(|focused| {
        doc.arena.contains(focused)
            && doc
                .arena
                .get(focused)
                .as_any()
                .downcast_ref::<InteractiveNode>()
                .is_some_and(|catcher| catcher.inhibits_shortcuts)
    })
}

pub(super) fn route(
    doc: &mut Document,
    ctx: &Context,
    rects: &Rects,
    root: NodeId,
    events: &[Event],
    keep: &[bool],
) {
    doc.pointers.forget_hits();
    let modifiers = ctx.input(|input| input.modifiers);
    let position = ctx.input(|input| input.pointer.interact_pos());
    let alive = |doc: &Document, id: Option<NodeId>| {
        id.filter(|id| doc.arena.contains(*id) && wants_forward(doc.arena.get(*id)))
    };
    let focused = alive(doc, doc.focused_node());
    let hovered = position.and_then(|pos| sink_at(doc, rects, root, pos));
    let pointer_events = ctx.input(|input| input.pointer_events.clone());
    let routed = std::mem::take(&mut doc.pointers.routed);
    let layers = doc.layers();
    let reachable = |doc: &Document, sink: NodeId| {
        doc.layer_of(&layers, sink)
            .is_some_and(|layer| layers.reachable(layer))
    };
    let mut deliveries: Vec<(NodeId, Vec<Forwarded>)> = Vec::new();
    let mut deliver = |to: Option<NodeId>, event: Forwarded| {
        let Some(to) = to else {
            return;
        };
        match deliveries.iter_mut().find(|(sink, _)| *sink == to) {
            Some((_, events)) => events.push(event),
            None => deliveries.push((to, vec![event])),
        }
    };
    let mut entered = std::mem::take(&mut doc.forward.entered);
    let mut pointed = 0;
    for (index, event) in events.iter().enumerate() {
        while let Some(pointer_event) = pointer_events
            .get(pointed)
            .filter(|pointer_event| pointer_event.source == index)
        {
            let Routed { sink, over } = routed.get(pointed).copied().unwrap_or_default();
            pointed += 1;
            let to = alive(doc, sink.filter(|sink| reachable(doc, *sink)));
            if let Some(to) = to
                && pointer_event.pointer.hovers()
                && !entered.contains(&to)
            {
                entered.push(to);
            }
            deliver(
                to,
                Forwarded::Pointer {
                    event: *pointer_event,
                    over,
                },
            );
        }
        if !keep.get(index).copied().unwrap_or(true) {
            continue;
        }
        match event {
            Event::FileHovered | Event::FileHoverCancelled | Event::FileDropped(_) => {
                deliver(hovered, Forwarded::Input(event.clone()))
            }
            Event::Key { .. }
            | Event::Text(_)
            | Event::Paste(_)
            | Event::Ime(_)
            | Event::Modifiers(_)
            | Event::PointerMotion(_)
            | Event::Focus(_) => deliver(focused, Forwarded::Input(event.clone())),
            _ => {}
        }
    }
    let captured = [PointerId::Mouse, PointerId::Pen].map(|pointer| doc.pointers.sink_of(pointer));
    entered.retain(|sink| {
        let stays = hovered == Some(*sink) || captured.contains(&Some(*sink));
        if !stays && alive(doc, Some(*sink)).is_some() {
            deliver(Some(*sink), Forwarded::Left);
        }
        stays
    });
    let before = doc.forward.focused;
    for sink in [before, focused].into_iter().flatten() {
        if before != focused && alive(doc, Some(sink)).is_some() {
            deliver(Some(sink), Forwarded::Focus(focused == Some(sink)));
        }
    }
    let pointed = events
        .iter()
        .any(|event| matches!(event, Event::PointerMoved(_) | Event::PointerButton { .. }));
    for sink in [doc.forward.hovered, hovered]
        .into_iter()
        .flatten()
        .filter(|_| pointed || doc.forward.hovered != hovered)
    {
        if alive(doc, Some(sink)).is_some() && !deliveries.iter().any(|(to, _)| *to == sink) {
            deliveries.push((sink, Vec::new()));
        }
    }
    doc.forward = Routing {
        hovered,
        focused,
        entered,
    };
    for (sink, events) in deliveries {
        if !doc.arena.contains(sink) {
            continue;
        }
        let Some(on_forward) = hit::interactive(doc, sink).map(|catcher| catcher.on_forward) else {
            continue;
        };
        on_forward.call(ForwardedInput {
            events,
            rect: rects.get(&sink).unwrap_or(Rect::NOTHING),
            hovered: hovered == Some(sink),
            focused: focused == Some(sink),
            pointer: position.filter(|_| hovered == Some(sink)),
            modifiers,
        });
    }
}
