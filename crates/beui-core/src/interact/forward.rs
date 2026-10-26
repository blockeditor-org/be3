use crate::base::interactive::InteractiveNode;
use crate::context::Context;
use crate::document::Document;
use std::time::Instant;

use crate::geometry::{Pos2, Rect};
use crate::input::{
    Event, Input, KeyPress, Keystroke, Modifiers, PenPhase, PointerEvent, PointerId, PointerPhase,
    ScrollDelta, TouchPhase,
};
use crate::node::{Element, NodeId, Rects};
use crate::window::WindowId;

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

#[derive(Clone, Debug, PartialEq)]
pub struct Replayed {
    pub input: Input,
    pub time: Option<Instant>,
}

impl From<Input> for Replayed {
    fn from(input: Input) -> Self {
        Self { input, time: None }
    }
}

#[derive(Default)]
pub struct Replay {
    focused: bool,
    modifiers: Modifiers,
    pressed: u32,
}

impl Replay {
    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn replay(&mut self, input: &ForwardedInput, zoom: f32) -> Vec<Replayed> {
        let zoom = match zoom.is_finite() && zoom > 0.0 {
            true => zoom,
            false => 1.0,
        };
        let mut out = Vec::new();
        for event in &input.events {
            match event {
                Forwarded::Pointer { event, .. } => self.pointer(event, input.rect, zoom, &mut out),
                Forwarded::Left => out.push(Input::PointerLeft.into()),
                Forwarded::Focus(focused) | Forwarded::Input(Event::Focus(focused)) => {
                    if self.focused != *focused {
                        self.focused = *focused;
                        out.push(Input::Focus(*focused).into());
                    }
                }
                Forwarded::Input(event) => self.input(event, &mut out),
            }
        }
        out
    }

    pub fn intercepted(&mut self, press: Option<KeyPress>, modifiers: Modifiers) -> Vec<Input> {
        let mut out = Vec::new();
        self.modifiers(modifiers, &mut out);
        let mut out: Vec<Input> = out.into_iter().map(|replayed| replayed.input).collect();
        if let Some(press) = press
            && press.key != crate::input::Key::Unidentified
        {
            out.push(Input::InterceptedKey {
                key: press.key,
                pressed: press.pressed,
                repeat: press.repeat,
            });
        }
        out
    }

    fn pointer(&mut self, event: &PointerEvent, rect: Rect, zoom: f32, out: &mut Vec<Replayed>) {
        let at = |pos: Pos2| Pos2::ZERO + (pos - rect.min) / zoom;
        let pos = at(event.pos);
        let timed = |input: Input| Replayed {
            input,
            time: Some(event.time),
        };
        if let PointerId::Touch(id) = event.pointer {
            let phase = match event.phase {
                PointerPhase::Down(_) => TouchPhase::Start,
                PointerPhase::Move => TouchPhase::Move,
                PointerPhase::Up(_) => TouchPhase::End,
                PointerPhase::Cancel => TouchPhase::Cancel,
                _ => return,
            };
            out.push(timed(Input::Touch {
                id,
                phase,
                pos,
                force: event.force,
            }));
        } else if event.pointer == PointerId::Pen {
            let phase = match event.phase {
                PointerPhase::Move => PenPhase::Move,
                PointerPhase::Down(button) => PenPhase::Down(button),
                PointerPhase::Up(button) => PenPhase::Up(button),
                PointerPhase::Cancel | PointerPhase::Gone => PenPhase::Gone,
                _ => return,
            };
            if matches!(phase, PenPhase::Down(_) | PenPhase::Up(_)) {
                self.modifiers(event.modifiers, out);
            }
            out.push(timed(Input::Pen {
                phase,
                pos,
                pen: event.pen.unwrap_or_default(),
            }));
        } else {
            match event.phase {
                PointerPhase::Move => out.push(timed(Input::PointerMoved(pos))),
                PointerPhase::Down(button) | PointerPhase::Up(button) => {
                    let pressed = matches!(event.phase, PointerPhase::Down(_));
                    self.pressed = match pressed {
                        true => self.pressed.saturating_add(1),
                        false => self.pressed.saturating_sub(1),
                    };
                    self.modifiers(event.modifiers, out);
                    out.push(timed(Input::PointerButton {
                        pos,
                        button,
                        pressed,
                    }));
                }
                PointerPhase::Scroll(delta) => {
                    self.modifiers(event.modifiers, out);
                    out.push(Input::Scroll(ScrollDelta::Pixels(delta / zoom)).into());
                }
                PointerPhase::ScrollStarted => out.push(Input::ScrollStarted.into()),
                PointerPhase::ScrollEnded { .. } => out.push(Input::ScrollEnded.into()),
                PointerPhase::Zoom(factor) => out.push(Input::Zoom(factor).into()),
                PointerPhase::Cancel | PointerPhase::Gone => {
                    if std::mem::take(&mut self.pressed) > 0 {
                        out.push(timed(Input::PointersCancelled));
                    }
                }
            }
        }
        if let (PointerPhase::Move, Some(predicted)) = (event.phase, event.predicted) {
            out.push(Input::Predicted(at(predicted)).into());
        }
    }

    fn input(&mut self, event: &Event, out: &mut Vec<Replayed>) {
        let input = match event {
            Event::PointerMotion(delta) => Input::PointerMotion(*delta),
            Event::Key { press, evdev, text } => {
                self.modifiers(press.modifiers, out);
                Input::Key(Keystroke {
                    key: Some(press.key),
                    pressed: press.pressed,
                    repeat: press.repeat,
                    evdev: *evdev,
                    text: text.clone(),
                })
            }
            Event::Modifiers(modifiers) => return self.modifiers(*modifiers, out),
            Event::Text(text) => Input::Text(text.clone()),
            Event::Paste(pasted) => Input::Paste(pasted.clone()),
            Event::Ime(ime) => Input::Ime(ime.clone()),
            _ => return,
        };
        out.push(input.into());
    }

    fn modifiers(&mut self, modifiers: Modifiers, out: &mut Vec<Replayed>) {
        if self.modifiers != modifiers {
            self.modifiers = modifiers;
            out.push(Input::Modifiers(modifiers).into());
        }
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
    let reachable = |doc: &Document, sink: NodeId| {
        doc.window_layers(doc.window_of(sink))
            .is_some_and(|layers| {
                doc.layer_of(&layers, sink)
                    .is_some_and(|layer| layers.reachable(layer))
            })
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
