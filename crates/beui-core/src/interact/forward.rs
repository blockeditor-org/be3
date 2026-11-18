//! Routing to forwarding catchers (guides/beui_keyboard.md, One input path). Who each pointer
//! event, key and focus change is for follows the forwarding router's rules
//! (`be_lib_input_router::Router`): a press holds its catcher until it lifts, hover and
//! `Left` follow the catcher under the pointer, a held gesture stays with the catcher it began
//! on, and the focus is told as it moves. What lies under a point is beui's own hit test
//! (`Catchers`): its overlays, scrims and claims.

use crate::base::interactive::InteractiveNode;
use crate::context::Context;
use crate::document::Document;

use crate::node::{Element, Rects};
use be_lib_input_router::{HitTest, Pointer, Router};
use beui_types::NodeId;
use beui_types::WindowId;
use beui_types::{Event, PointerId};
use beui_types::{Pos2, Rect};

use super::hit;
use super::pointer::Routed;

use beui_types::{Forwarded, ForwardedInput, Over};

#[derive(Default)]
pub struct Routing {
    pub(crate) router: Router<NodeId>,
    /// Where each catcher the pointer is over or held by was when it last heard the pointer,
    /// so one that moves under a resting pointer hears it again (`reforward`).
    placed: Vec<(NodeId, Pos2)>,
}

impl Routing {
    pub(crate) fn hovered(&self) -> Option<NodeId> {
        self.router.hovered()
    }
}

/// The router's name for a beui pointer.
pub(crate) fn pointer(id: PointerId) -> Pointer {
    match id {
        PointerId::Mouse => Pointer::Mouse,
        PointerId::Pen => Pointer::Pen,
        PointerId::Touch(id) => Pointer::Touch(id),
        PointerId::Keyboard => Pointer::Keyboard,
        PointerId::Relayed => Pointer::Relayed,
    }
}

/// The catcher a wheel, pinch or touchpad gesture at `at` is for, `found` being the one beui's
/// hit test finds there. A held gesture latched to a catcher that went lets go first.
pub(crate) fn gesture(
    doc: &mut Document,
    found: Option<NodeId>,
    at: Pos2,
    phase: be_lib_input_router::Phase,
) -> Option<NodeId> {
    if let Some(Some(latched)) = doc.forward.router.latched()
        && !(doc.arena.contains(latched) && wants_forward(doc.arena.get(latched)))
    {
        doc.forward.router.forget(latched);
    }
    doc.forward.router.gesture(&Catchers::at(found), at, phase)
}

/// The router's hit test over a document's catchers, for one pointer event: beui's hit test
/// found the catcher under it (`sink`, `None` where the document's own UI takes it: a control,
/// a scrim or a popup over the catcher, or a claim) and whether the press pans a camera.
/// Positions stay in the window's points, as `ForwardedInput` carries them; claims on catchers'
/// ancestors are beui's (`hit::claimant`), so no catcher places another.
pub(crate) struct Catchers {
    pub(crate) sink: Option<NodeId>,
    pub(crate) pans: bool,
}

impl Catchers {
    pub(crate) fn at(sink: Option<NodeId>) -> Self {
        Self { sink, pans: false }
    }
}

impl HitTest for Catchers {
    type Target = NodeId;

    fn hit(&self, _at: Pos2) -> Option<NodeId> {
        self.sink
    }

    fn local(&self, _target: NodeId, at: Pos2) -> Option<Pos2> {
        Some(at)
    }

    fn placer(&self, _target: NodeId) -> Option<NodeId> {
        None
    }

    fn grants_gestures(&self, _target: NodeId, _at: Pos2) -> bool {
        false
    }

    /// beui moves its own focus on a press (`press_focus`); the router is told where it went.
    fn takes_focus(&self, _target: NodeId) -> bool {
        false
    }

    fn pans(&self, _at: Pos2) -> bool {
        self.pans
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
    // Under the pointer only where it can reach, as `reforward` judges it, so the two never
    // take turns telling a sink under a layer that holds input from it that the pointer left.
    let hovered = position
        .and_then(|pos| sink_at(doc, rects, window, pos))
        .filter(|sink| reachable(doc, *sink));
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
    let mut entered = Vec::new();
    let mut reached = Vec::new();
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
            reached.extend(to);
            deliver(
                to,
                Forwarded::Pointer {
                    event: *pointer_event,
                    over,
                },
            );
        }
        if !keep.get(index).copied().unwrap_or(true) {
            if let Event::Key { press, .. } = event
                && press.pressed
                && !press.repeat
            {
                deliver(focused, Forwarded::KeyTaken);
            }
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
        // A held gesture whose end will not come: the window lost the keyboard or the pointer.
        if matches!(event, Event::Focus(false) | Event::PointerGone) {
            doc.forward.router.end_gesture();
        }
    }
    for sink in entered {
        doc.forward.router.entered(sink);
    }
    let was_hovered = doc.forward.router.hovered();
    for left in doc.forward.router.hover(hovered) {
        if alive(doc, Some(left)).is_some() {
            deliver(Some(left), Forwarded::Left);
        }
    }
    // A sink that came under a mouse resting where it was (it moved there, or what covered it
    // went) hears where the pointer is, as `reforward` tells it after a layout: without it, a
    // sink told it was left keeps thinking so. Not while another holds the mouse, which
    // `reforward` leaves alone too.
    let resting = ctx.input(|input| input.pointer.mouse().map(|pos| (input.pointer.window, pos)));
    let holder = alive(doc, doc.forward.router.held(Pointer::Mouse));
    if let (Some(sink), Some((window, pos))) = (alive(doc, hovered), resting)
        && was_hovered != hovered
        && holder.is_none_or(|holder| holder == sink)
        && !reached.contains(&sink)
    {
        doc.forward.router.entered(sink);
        let moved = resting_move(doc, ctx, rects, window, pos);
        deliver(Some(sink), moved);
    }
    for told in doc.forward.router.focus(focused) {
        if let be_lib_input_router::Routed::Input {
            target,
            input: beui_types::Input::Focus(has),
        } = told
            && alive(doc, Some(target)).is_some()
        {
            deliver(Some(target), Forwarded::Focus(has));
        }
    }
    let pointed = events.iter().any(|event| {
        matches!(
            event,
            Event::PointerMoved(_) | Event::PointerRested(_) | Event::PointerButton { .. }
        )
    });
    for sink in [was_hovered, hovered]
        .into_iter()
        .flatten()
        .filter(|_| pointed || was_hovered != hovered)
    {
        if alive(doc, Some(sink)).is_some() && !deliveries.iter().any(|(to, _)| *to == sink) {
            deliveries.push((sink, Vec::new()));
        }
    }
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
    let captured = alive(doc, doc.forward.router.held(Pointer::Mouse));
    if captured.is_none() {
        let under = sink_at(doc, rects, window, pos);
        let under = alive(doc, under);
        if under != doc.forward.router.hovered() {
            let mut deliveries = Vec::new();
            for left in doc.forward.router.hover(under) {
                if alive(doc, Some(left)).is_some() {
                    deliveries.push((left, vec![Forwarded::Left]));
                }
            }
            if let Some(under) = under {
                doc.forward.router.entered(under);
                let moved = resting_move(doc, ctx, rects, window, pos);
                deliveries.push((under, vec![moved]));
            }
            deliver_all(doc, rects, deliveries, Some(pos), modifiers);
            return;
        }
    }
    let Some(sink) = captured.or_else(|| alive(doc, doc.forward.router.hovered())) else {
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
        rested: true,
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
    let (hovered, focused) = (doc.forward.router.hovered(), doc.forward.router.focused());
    for (sink, events) in &deliveries {
        let pointed = events.iter().any(|event| {
            matches!(event, Forwarded::Pointer { event, .. } if event.pointer == PointerId::Mouse)
        });
        if let (true, Some(rect)) = (pointed, rects.get(sink)) {
            doc.forward.placed.retain(|(held, _)| held != sink);
            doc.forward.placed.push((*sink, rect.min));
        }
    }
    let keep: Vec<NodeId> = [hovered, doc.forward.router.held(Pointer::Mouse)]
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
