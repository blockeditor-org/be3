use crate::base::interactive::InteractiveNode;
use crate::context::Context;
use crate::document::Document;
use crate::geometry::{Pos2, Rect};
use crate::input::{Event, Modifiers};
use crate::node::{Element, NodeId, Rects};

use super::hit;

#[derive(Clone, Debug, PartialEq)]
pub struct ForwardedInput {
    pub events: Vec<Event>,
    pub rect: Rect,
    pub hovered: bool,
    pub focused: bool,
    pub pointer: Option<Pos2>,
    pub under: Vec<Option<NodeId>>,
    pub modifiers: Modifiers,
}

#[derive(Default)]
pub struct Routing {
    hovered: Option<NodeId>,
    focused: Option<NodeId>,
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
    let hit = hit::hit(doc, rects, root, pos, false);
    hit::sink(doc, rects, &hit.path, pos)
}

pub(super) fn cycle_focus(doc: &mut Document, step: isize) -> bool {
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

pub(super) fn takes_keys(doc: &Document) -> bool {
    doc.focused_node()
        .is_some_and(|focused| doc.arena.contains(focused) && wants_forward(doc.arena.get(focused)))
}

pub(super) fn shortcuts_inhibited(doc: &Document) -> bool {
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

#[allow(clippy::too_many_arguments)]
pub(super) fn route(
    doc: &mut Document,
    ctx: &Context,
    rects: &Rects,
    root: NodeId,
    events: &[Event],
    keep: &[bool],
    pointer: bool,
    keys: super::Keys,
) {
    let modifiers = ctx.input(|input| input.modifiers);
    let position = ctx
        .input(|input| input.pointer.interact_pos())
        .filter(|_| pointer);
    let alive = |doc: &Document, id: Option<NodeId>| {
        id.filter(|id| doc.arena.contains(*id) && wants_forward(doc.arena.get(*id)))
    };
    let focused = alive(doc, doc.focused_node()).filter(|_| !keys.ignored());
    let hovered = position.and_then(|pos| sink_at(doc, rects, root, pos));
    let wheeled = alive(doc, doc.pointers.wheel_sink);
    let routed = doc.pointers.routed.clone();
    let mut deliveries: Vec<(NodeId, Vec<Event>)> = Vec::new();
    let mut deliver = |to: Option<NodeId>, event: &Event| {
        let Some(to) = to else {
            return;
        };
        match deliveries.iter_mut().find(|(sink, _)| *sink == to) {
            Some((_, events)) => events.push(event.clone()),
            None => deliveries.push((to, vec![event.clone()])),
        }
    };
    for (index, event) in events.iter().enumerate() {
        if !keep.get(index).copied().unwrap_or(true) {
            continue;
        }
        match event {
            Event::PointerMoved(_)
            | Event::PointerButton { .. }
            | Event::PointerGone
            | Event::Touch { .. } => {
                let to = routed
                    .iter()
                    .find(|(source, _)| *source == index)
                    .and_then(|(_, sink)| *sink)
                    .filter(|sink| {
                        doc.lock()
                            .is_none_or(|lock| doc.is_within(*sink, lock.id()))
                    });
                deliver(alive(doc, to), event);
            }
            Event::Scroll(_) | Event::ScrollEnded | Event::Zoom(_) => deliver(wheeled, event),
            Event::FileHovered | Event::FileHoverCancelled | Event::FileDropped(_) => {
                deliver(hovered, event)
            }
            Event::Key { .. }
            | Event::PhysicalKey { .. }
            | Event::Text(_)
            | Event::Ime(_)
            | Event::Modifiers(_)
            | Event::PointerMotion(_)
            | Event::Focus(false) => deliver(focused, event),
            Event::Focus(true)
            | Event::Back(_)
            | Event::InterceptedKey(_)
            | Event::InterceptedTap(_) => {}
        }
    }
    let pointed = events
        .iter()
        .any(|event| matches!(event, Event::PointerMoved(_) | Event::PointerButton { .. }));
    if pointed
        && let Some(hovered) = alive(doc, hovered)
        && !deliveries.iter().any(|(sink, _)| *sink == hovered)
    {
        deliveries.push((hovered, Vec::new()));
    }
    let before = (doc.forward.hovered, doc.forward.focused);
    for changed in [before.0, hovered, before.1, focused].into_iter().flatten() {
        if alive(doc, Some(changed)).is_some()
            && !deliveries.iter().any(|(sink, _)| *sink == changed)
        {
            let moved = (before.0 == Some(changed)) != (hovered == Some(changed))
                || (before.1 == Some(changed)) != (focused == Some(changed));
            if moved {
                deliveries.push((changed, Vec::new()));
            }
        }
    }
    doc.forward = Routing { hovered, focused };
    for (sink, events) in deliveries {
        if !doc.arena.contains(sink) {
            continue;
        }
        let Some(on_forward) =
            hit::interactive(doc, sink).map(|catcher| catcher.on_forward.clone())
        else {
            continue;
        };
        let under = events
            .iter()
            .map(|event| match event {
                Event::PointerMoved(pos) | Event::PointerButton { pos, .. } if pointer => {
                    sink_at(doc, rects, root, *pos)
                }
                _ => hovered,
            })
            .collect();
        let raw = ForwardedInput {
            events,
            rect: rects.get(&sink).unwrap_or(Rect::NOTHING),
            hovered: hovered == Some(sink),
            focused: focused == Some(sink),
            pointer: position.filter(|_| hovered == Some(sink)),
            under,
            modifiers,
        };
        on_forward.call(raw);
    }
}
