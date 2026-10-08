use std::collections::HashMap;

use crate::base::interactive::InteractiveNode;
use crate::context::Context;
use crate::document::Document;
use crate::geometry::{Pos2, Rect};
use crate::input::{Event, Key, Modifiers, PointerButton, TouchPhase};
use crate::node::{Element, NodeId, Rects};

#[derive(Clone, Debug, PartialEq)]
pub struct ForwardedInput {
    pub events: Vec<Event>,
    pub rect: Rect,
    pub hovered: bool,
    pub focused: bool,
    pub pointer: Option<Pos2>,
    pub modifiers: Modifiers,
}

#[derive(Default)]
pub struct Routing {
    hovered: Option<NodeId>,
    focused: Option<NodeId>,
    captor: Option<NodeId>,
    buttons: u8,
    touches: HashMap<(u64, u64), NodeId>,
    swallowed: bool,
}

impl Routing {
    pub(crate) fn swallow_press(&mut self) {
        self.swallowed = true;
        self.captor = None;
    }
}

pub fn wants_forward(element: &dyn Element) -> bool {
    element
        .as_any()
        .downcast_ref::<InteractiveNode>()
        .is_some_and(|catcher| !catcher.on_forward.is_empty())
}

fn deepest(doc: &Document, rects: &Rects, id: NodeId, pos: Pos2) -> Option<NodeId> {
    if !rects
        .visible(&id)
        .is_some_and(|rect| rect.contains_half_open(pos))
    {
        return None;
    }
    let node = doc.arena.get(id);
    for child in node.children().into_iter().rev() {
        if let Some(found) = deepest(doc, rects, child, pos) {
            return Some(found);
        }
    }
    let catcher = node.as_any().downcast_ref::<InteractiveNode>()?;
    if catcher.on_forward.is_empty() {
        return None;
    }
    let local = pos - rects.get(&id)?.min.to_vec2();
    (catcher.forward_at.is_empty() || catcher.forward_at.call(local)).then_some(id)
}

pub fn sink_at(doc: &Document, rects: &Rects, root: NodeId, pos: Pos2) -> Option<NodeId> {
    if doc.modal_open() {
        for overlay in doc.overlay_stack.iter().rev() {
            if let Some(found) = doc
                .overlay_content(*overlay)
                .and_then(|content| deepest(doc, rects, content, pos))
            {
                return Some(found);
            }
            if !doc.light_overlay_misses(*overlay, pos) {
                return None;
            }
        }
        return match doc.pointer_passes_under_overlays(pos) {
            true => deepest(doc, rects, root, pos),
            false => None,
        };
    }
    for overlay in doc.floating_overlays().into_iter().rev() {
        let Some(content) = doc.overlay_content(overlay) else {
            continue;
        };
        if rects
            .visible(&content)
            .is_some_and(|rect| rect.contains_half_open(pos))
        {
            return deepest(doc, rects, content, pos);
        }
    }
    deepest(doc, rects, root, pos)
}

fn cycle_focus(doc: &mut Document, step: isize) -> bool {
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

fn button_mask(button: PointerButton) -> u8 {
    1 << match button {
        PointerButton::Primary => 0,
        PointerButton::Secondary => 1,
        PointerButton::Middle => 2,
        PointerButton::Back => 3,
        PointerButton::Forward => 4,
    }
}

pub(super) fn takes_keys(doc: &Document) -> bool {
    doc.focused_node()
        .is_some_and(|focused| doc.arena.contains(focused) && wants_forward(doc.arena.get(focused)))
}

pub(super) fn route(
    doc: &mut Document,
    ctx: &Context,
    rects: &Rects,
    root: NodeId,
    pointer: bool,
    keys: super::Keys,
) {
    let mut events = ctx.input(|input| input.events.clone());
    let cycle = events.iter().rev().find_map(|event| match event {
        Event::Key {
            key: Key::F6,
            pressed: true,
            modifiers,
            ..
        } => Some(if modifiers.shift { -1 } else { 1 }),
        _ => None,
    });
    if let Some(step) = cycle.filter(|_| !keys.ignored())
        && cycle_focus(doc, step)
    {
        events.retain(|event| !matches!(event, Event::Key { key: Key::F6, .. }));
    }
    let modifiers = ctx.input(|input| input.modifiers);
    let position = ctx.input(|input| input.pointer.interact_pos());
    let mut routing = std::mem::take(&mut doc.forward);
    let alive = |doc: &Document, id: Option<NodeId>| {
        id.filter(|id| doc.arena.contains(*id) && wants_forward(doc.arena.get(*id)))
    };
    routing.captor = alive(doc, routing.captor);
    routing
        .touches
        .retain(|_, sink| doc.arena.contains(*sink) && wants_forward(doc.arena.get(*sink)));
    let focused = alive(doc, doc.focused_node()).filter(|_| !keys.ignored());
    let at = |doc: &Document, pos: Pos2| match pointer {
        true => sink_at(doc, rects, root, pos),
        false => None,
    };
    let hovered = position.and_then(|pos| at(doc, pos));
    let swallowed = routing.swallowed;
    let pressed_at = |doc: &Document, pos: Pos2| match swallowed {
        true => None,
        false => at(doc, pos),
    };
    let mut routed: Vec<(NodeId, Vec<Event>)> = Vec::new();
    let mut deliver = |to: Option<NodeId>, event: &Event| {
        let Some(to) = to else {
            return;
        };
        match routed.iter_mut().find(|(sink, _)| *sink == to) {
            Some((_, events)) => events.push(event.clone()),
            None => routed.push((to, vec![event.clone()])),
        }
    };
    for event in &events {
        match event {
            Event::PointerMoved(pos) => {
                deliver(routing.captor.or_else(|| pressed_at(doc, *pos)), event)
            }
            Event::PointerGone => deliver(routing.captor.or(routing.hovered), event),
            Event::PointerButton {
                pos,
                button,
                pressed,
                ..
            } => {
                let to = routing.captor.or_else(|| pressed_at(doc, *pos));
                deliver(to, event);
                match (pressed, to) {
                    (true, Some(to)) => {
                        routing.captor = Some(to);
                        routing.buttons |= button_mask(*button);
                    }
                    (false, _) => {
                        routing.buttons &= !button_mask(*button);
                        if routing.buttons == 0 {
                            routing.captor = None;
                        }
                    }
                    (true, None) => {}
                }
            }
            Event::Touch { id, phase, pos, .. } => {
                let finger = (id.device, id.finger);
                let to = match phase {
                    TouchPhase::Start => {
                        let to = pressed_at(doc, *pos);
                        if let Some(to) = to {
                            routing.touches.insert(finger, to);
                        }
                        to
                    }
                    TouchPhase::Move => routing.touches.get(&finger).copied(),
                    TouchPhase::End | TouchPhase::Cancel => routing.touches.remove(&finger),
                };
                deliver(to, event);
            }
            Event::Scroll(_)
            | Event::ScrollEnded
            | Event::Zoom(_)
            | Event::FileHovered
            | Event::FileHoverCancelled
            | Event::FileDropped(_) => deliver(hovered, event),
            Event::Key { .. }
            | Event::PhysicalKey { .. }
            | Event::Text(_)
            | Event::Ime(_)
            | Event::Modifiers(_)
            | Event::PointerMotion(_)
            | Event::Focus(false) => deliver(focused, event),
            Event::Focus(true) | Event::Back(_) | Event::InterceptedKey(_) => {}
        }
    }
    let held = ctx.input(|input| {
        [
            (input.pointer.primary_down, PointerButton::Primary),
            (input.pointer.secondary_down, PointerButton::Secondary),
            (input.pointer.middle_down, PointerButton::Middle),
        ]
    });
    for (down, button) in held {
        if !down {
            routing.buttons &= !button_mask(button);
        }
    }
    if held.iter().all(|(down, _)| !down) {
        routing.swallowed = false;
    }
    if routing.buttons == 0 {
        routing.captor = None;
    }
    let pointed = events
        .iter()
        .any(|event| matches!(event, Event::PointerMoved(_) | Event::PointerButton { .. }));
    if pointed
        && let Some(hovered) = alive(doc, hovered)
        && !routed.iter().any(|(sink, _)| *sink == hovered)
    {
        routed.push((hovered, Vec::new()));
    }
    for changed in [routing.hovered, hovered, routing.focused, focused]
        .into_iter()
        .flatten()
    {
        if alive(doc, Some(changed)).is_some() && !routed.iter().any(|(sink, _)| *sink == changed) {
            let moved = (routing.hovered == Some(changed)) != (hovered == Some(changed))
                || (routing.focused == Some(changed)) != (focused == Some(changed));
            if moved {
                routed.push((changed, Vec::new()));
            }
        }
    }
    routing.hovered = hovered;
    routing.focused = focused;
    doc.forward = routing;
    for (sink, events) in routed {
        if !doc.arena.contains(sink) {
            continue;
        }
        let Some(on_forward) = doc
            .arena
            .get(sink)
            .as_any()
            .downcast_ref::<InteractiveNode>()
            .map(|catcher| catcher.on_forward.clone())
        else {
            continue;
        };
        let raw = ForwardedInput {
            events,
            rect: rects.get(&sink).unwrap_or(Rect::NOTHING),
            hovered: hovered == Some(sink),
            focused: focused == Some(sink),
            pointer: position.filter(|_| hovered == Some(sink)),
            modifiers,
        };
        on_forward.call(raw);
    }
}
