use crate::base::interactive::InteractiveNode;
use crate::context::Context;
use crate::document::Document;

use crate::node::{Element, Rects};
use beui_types::NodeId;
use beui_types::WindowId;
use beui_types::{Event, PointerId};
use beui_types::{Pos2, Rect};

use super::hit;
use super::pointer::Routed;

use beui_types::{Forwarded, ForwardedInput, Over};

#[derive(Default)]
pub struct Routing {
    hovered: Option<NodeId>,
    focused: Option<NodeId>,
    entered: Vec<NodeId>,
    placed: Vec<(NodeId, Pos2)>,
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

fn reachable(doc: &Document, sink: NodeId) -> bool {
    doc.window_layers(doc.window_of(sink))
        .is_some_and(|layers| {
            doc.layer_of(&layers, sink)
                .is_some_and(|layer| layers.reachable(layer))
        })
}

fn sink_at(doc: &mut Document, rects: &Rects, window: WindowId, pos: Pos2) -> Option<NodeId> {
    let hit = super::pointer::hit(doc, rects, window, pos, false);
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
    let window = ctx.input(|input| input.pointer.window);
    let hovered = position.and_then(|pos| sink_at(doc, rects, window, pos));
    let pointer_events = ctx.input(|input| input.pointer_events.clone());
    let routed = std::mem::take(&mut doc.pointers.routed);
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
            let over = over.map(|node| Over {
                node,
                local: Pos2::ZERO
                    + (pointer_event.pos - rects.get(&node).map_or(Pos2::ZERO, |rect| rect.min)),
            });
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
    let placed = std::mem::take(&mut doc.forward.placed);
    doc.forward = Routing {
        hovered,
        focused,
        entered,
        placed,
    };
    deliver_all(doc, rects, deliveries, position, modifiers);
}

pub(crate) fn reforward(doc: &mut Document, ctx: &Context, rects: &Rects) {
    let Some((window, pos)) =
        ctx.input(|input| input.pointer.mouse().map(|pos| (input.pointer.window, pos)))
    else {
        return;
    };
    let alive = |doc: &Document, id: Option<NodeId>| {
        id.filter(|id| {
            doc.arena.contains(*id) && wants_forward(doc.arena.get(*id)) && reachable(doc, *id)
        })
    };
    let modifiers = ctx.input(|input| input.modifiers);
    let captured = alive(doc, doc.pointers.sink_of(PointerId::Mouse));
    if captured.is_none() {
        let under = sink_at(doc, rects, window, pos);
        let under = alive(doc, under);
        if under != doc.forward.hovered {
            let mut deliveries = Vec::new();
            if let Some(left) = alive(doc, doc.forward.hovered)
                && doc.forward.entered.contains(&left)
            {
                deliveries.push((left, vec![Forwarded::Left]));
            }
            let hovered = doc.forward.hovered;
            doc.forward.entered.retain(|sink| Some(*sink) != hovered);
            doc.forward.hovered = under;
            if let Some(under) = under {
                if !doc.forward.entered.contains(&under) {
                    doc.forward.entered.push(under);
                }
                let moved = resting_move(doc, ctx, rects, window, pos);
                deliveries.push((under, vec![moved]));
            }
            deliver_all(doc, rects, deliveries, Some(pos), modifiers);
            return;
        }
    }
    let Some(sink) = captured.or_else(|| alive(doc, doc.forward.hovered)) else {
        return;
    };
    let Some(placed) = doc
        .forward
        .placed
        .iter()
        .find(|(held, _)| *held == sink)
        .map(|(_, placed)| *placed)
    else {
        return;
    };
    if rects.get(&sink).is_none_or(|rect| rect.min == placed) {
        return;
    }
    let moved = resting_move(doc, ctx, rects, window, pos);
    deliver_all(doc, rects, vec![(sink, vec![moved])], Some(pos), modifiers);
}

fn resting_move(
    doc: &mut Document,
    ctx: &Context,
    rects: &Rects,
    window: WindowId,
    pos: Pos2,
) -> Forwarded {
    let event = beui_types::PointerEvent {
        pointer: PointerId::Mouse,
        window,
        phase: beui_types::PointerPhase::Move,
        pos,
        modifiers: ctx.input(|input| input.modifiers),
        clicks: 0,
        force: None,
        pen: None,
        predicted: None,
        time: ctx.now(),
        source: 0,
    };
    let over = sink_at(doc, rects, window, pos).map(|node| Over {
        node,
        local: Pos2::ZERO + (pos - rects.get(&node).map_or(pos, |rect| rect.min)),
    });
    Forwarded::Pointer { event, over }
}

fn deliver_all(
    doc: &mut Document,
    rects: &Rects,
    deliveries: Vec<(NodeId, Vec<Forwarded>)>,
    position: Option<Pos2>,
    modifiers: beui_types::Modifiers,
) {
    let Routing {
        hovered, focused, ..
    } = doc.forward;
    for (sink, events) in &deliveries {
        let pointed = events.iter().any(|event| {
            matches!(event, Forwarded::Pointer { event, .. } if event.pointer == PointerId::Mouse)
        });
        if let (true, Some(rect)) = (pointed, rects.get(sink)) {
            doc.forward.placed.retain(|(held, _)| held != sink);
            doc.forward.placed.push((*sink, rect.min));
        }
    }
    let keep: Vec<NodeId> = [hovered, doc.pointers.sink_of(PointerId::Mouse)]
        .into_iter()
        .flatten()
        .collect();
    doc.forward.placed.retain(|(held, _)| keep.contains(held));
    for (sink, events) in deliveries {
        if !doc.arena.contains(sink) {
            continue;
        }
        let Some(on_forward) = hit::interactive(doc, sink).map(|catcher| catcher.on_forward) else {
            continue;
        };
        let events = into_window(doc, doc.window_of(sink), events);
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

fn into_window(doc: &Document, home: WindowId, mut events: Vec<Forwarded>) -> Vec<Forwarded> {
    let Some(ctx) = doc.context() else {
        return events;
    };
    for forwarded in &mut events {
        let Forwarded::Pointer { event, .. } = forwarded else {
            continue;
        };
        if event.window == home {
            continue;
        }
        if let (Some(from), Some(to)) = (ctx.window_origin(event.window), ctx.window_origin(home)) {
            event.pos += from - to;
            event.window = home;
        }
    }
    events
}

#[cfg(test)]
mod tests;
