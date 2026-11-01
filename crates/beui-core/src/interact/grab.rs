use ::reactive::Get;

use crate::base::interactive::InteractiveNode;
use crate::context::Context;
use crate::document::Document;
use crate::node::Rects;
use beui_types::NodeId;
use beui_types::{Event, PointerEvent, PointerPhase, PointerStage, ScrollGesture, WheelInput};
use beui_types::{Pos2, Vec2};

use super::pointer::input_at;

pub(crate) fn grabber(doc: &Document) -> Option<NodeId> {
    doc.pointers
        .grabber
        .filter(|id| super::hit::interactive(doc, *id).is_some_and(|node| node.grabs_pointer))
}

fn node(doc: &Document, id: NodeId) -> Option<&InteractiveNode> {
    super::hit::interactive(doc, id)
}

pub(crate) fn frame(doc: &mut Document, ctx: &Context) {
    if grabber(doc).is_none() {
        doc.pointers.grabber = None;
        return;
    }
    if ctx.input(|input| input.events.contains(&Event::PointersCancelled)) {
        end(doc);
    }
}

pub(crate) fn end(doc: &mut Document) -> bool {
    let Some(id) = grabber(doc) else {
        return false;
    };
    let pos = doc.last_pointer.map_or(Pos2::ZERO, |sample| sample.pos);
    let now = doc.now();
    let rects = std::rc::Rc::clone(&doc.rects);
    let input = input_at(
        &rects,
        id,
        beui_types::input::PointerId::Mouse,
        None,
        PointerStage::Cancel,
        pos,
        doc.watch_modifiers().get_untracked(),
        now,
    );
    if let Some(node) = node(doc, id) {
        node.on_pointer.clone().call(input);
    }
    true
}

pub(crate) fn pointer(doc: &mut Document, rects: &Rects, event: &PointerEvent, held: bool) -> bool {
    let Some(id) = grabber(doc) else {
        return false;
    };
    let (button, stage) = match event.phase {
        PointerPhase::Down(button) => (Some(button), PointerStage::Down),
        PointerPhase::Move if event.pointer.hovers() => (None, PointerStage::Move),
        PointerPhase::Up(button) => (Some(button), PointerStage::Up),
        PointerPhase::Cancel | PointerPhase::Gone => (None, PointerStage::Cancel),
        PointerPhase::Scroll(delta) => {
            let (local, size) = super::pointer::local(rects, id, event.pos);
            let wheel = WheelInput::Scroll(ScrollGesture {
                delta,
                fling: Vec2::ZERO,
                pos: event.pos,
                local,
                size,
                modifiers: event.modifiers,
                held,
                unclaimed: false,
            });
            doc.pointers.grabbed = Some(id);
            return node(doc, id).is_some_and(|node| node.on_wheel.clone().call(wheel));
        }
        _ => return false,
    };
    let input = input_at(
        rects,
        id,
        event.pointer,
        button,
        stage,
        event.pos,
        event.modifiers,
        event.time,
    );
    doc.pointers.grabbed = Some(id);
    node(doc, id).is_some_and(|node| node.on_pointer.clone().call(input))
}

impl Document {
    pub fn pointer_grabber(&self) -> Option<NodeId> {
        grabber(self)
    }
}
