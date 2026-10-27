use crate::base::interactive::InteractiveNode;
use crate::base::list::Direction;
use crate::context::Context;
use crate::document::Document;
use crate::geometry::Pos2;
use crate::input::{
    AutoscrollGesture, CursorIcon, Event, PointerButton, PointerEvent, PointerPhase,
};
use crate::node::NodeId;

pub const AUTOSCROLL_DEAD_ZONE: f32 = 10.0;

#[derive(Clone, Copy, Debug)]
pub struct Autoscroll {
    target: NodeId,
    origin: Pos2,
    pos: Pos2,
    held: bool,
    travelled: bool,
}

pub(crate) fn start(doc: &mut Document, target: NodeId, pos: Pos2) {
    let autoscroll = Autoscroll {
        target,
        origin: pos,
        pos,
        held: true,
        travelled: false,
    };
    doc.autoscroll = Some(autoscroll);
    report(doc, &autoscroll, false);
}

pub(crate) fn frame(doc: &mut Document, ctx: &Context) {
    let Some(autoscroll) = doc.autoscroll else {
        return;
    };
    if catcher(doc, autoscroll.target).is_none() {
        doc.autoscroll = None;
        return;
    }
    let lost = ctx.input(|input| input.events.contains(&Event::PointersCancelled));
    if lost {
        finish(doc, &autoscroll);
    }
}

pub(crate) fn end(doc: &mut Document) -> bool {
    let Some(autoscroll) = doc.autoscroll else {
        return false;
    };
    finish(doc, &autoscroll);
    true
}

pub(crate) fn pointer(doc: &mut Document, event: &PointerEvent) -> bool {
    let Some(mut autoscroll) = doc.autoscroll else {
        return false;
    };
    match event.phase {
        PointerPhase::Down(_) => {
            finish(doc, &autoscroll);
            true
        }
        PointerPhase::Move if event.pointer.hovers() => {
            autoscroll.travelled |=
                autoscroll.held && event.pos.distance(autoscroll.origin) > AUTOSCROLL_DEAD_ZONE;
            if event.pos != autoscroll.pos {
                autoscroll.pos = event.pos;
                report(doc, &autoscroll, false);
            }
            doc.autoscroll = Some(autoscroll);
            false
        }
        PointerPhase::Up(PointerButton::Middle) if autoscroll.held => {
            autoscroll.held = false;
            match autoscroll.travelled {
                true => finish(doc, &autoscroll),
                false => doc.autoscroll = Some(autoscroll),
            }
            false
        }
        PointerPhase::Cancel | PointerPhase::Gone => {
            finish(doc, &autoscroll);
            false
        }
        PointerPhase::Scroll(_) => {
            finish(doc, &autoscroll);
            true
        }
        _ => false,
    }
}

fn finish(doc: &mut Document, autoscroll: &Autoscroll) {
    doc.autoscroll = None;
    report(doc, autoscroll, true);
}

pub fn show_cursor(doc: &Document, ctx: &Context) {
    let Some(autoscroll) = doc.autoscroll else {
        return;
    };
    let Some(catcher) = catcher(doc, autoscroll.target) else {
        return;
    };
    ctx.set_cursor_icon(match catcher.scroll_axis {
        Some(Direction::Vertical) => CursorIcon::ResizeVertical,
        Some(Direction::Horizontal) => CursorIcon::ResizeHorizontal,
        None => CursorIcon::Move,
    });
}

fn catcher(doc: &Document, id: NodeId) -> Option<&InteractiveNode> {
    super::hit::interactive(doc, id).filter(|catcher| catcher.wants_autoscroll())
}

fn report(doc: &Document, autoscroll: &Autoscroll, ended: bool) {
    let Some(catcher) = catcher(doc, autoscroll.target) else {
        return;
    };
    let on_autoscroll = catcher.on_autoscroll;
    on_autoscroll.call(AutoscrollGesture {
        origin: autoscroll.origin,
        pos: autoscroll.pos,
        ended,
        edge: false,
    });
}
