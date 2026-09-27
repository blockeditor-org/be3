use crate::base::click_catcher::ClickCatcherNode;
use crate::base::embed::EmbedNode;
use crate::base::list::Direction;
use crate::context::Context;
use crate::document::Document;
use crate::geometry::{Pos2, Vec2};
use crate::input::{AutoscrollGesture, CursorIcon, Event, Key};
use crate::node::{Element, InteractInput, NodeId, Rects};

pub(crate) const AUTOSCROLL_DEAD_ZONE: f32 = 10.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Autoscroll {
    target: NodeId,
    origin: Pos2,
    pos: Pos2,
    held: bool,
    travelled: bool,
}

pub(crate) struct Tracked {
    pub(crate) input: InteractInput,
    pub(crate) swallows_escape: bool,
}

pub(crate) fn track(
    doc: &mut Document,
    ctx: &Context,
    rects: &Rects,
    root: NodeId,
    input: InteractInput,
) -> Tracked {
    let Some(mut autoscroll) = doc.autoscroll else {
        return start(doc, rects, root, input);
    };
    if !doc.contains(autoscroll.target) || catcher(doc, autoscroll.target).is_none() {
        doc.autoscroll = None;
        return Tracked {
            input,
            swallows_escape: false,
        };
    }
    let mut swallows_escape = false;
    let mut ended = false;
    for event in ctx.input(|input| input.events.clone()) {
        match event {
            Event::Key {
                key, pressed: true, ..
            } => {
                ended = true;
                swallows_escape |= key == Key::Escape;
            }
            Event::Focus(false) => ended = true,
            _ => {}
        }
    }
    let pressed = input.pressed_this_frame
        || input.secondary_pressed_this_frame
        || input.middle_pressed_this_frame;
    ended |= pressed || input.touch_started || input.scroll != Vec2::ZERO;
    let input = match pressed || input.scroll != Vec2::ZERO {
        false => input,
        true => InteractInput {
            pressed_this_frame: false,
            secondary_pressed_this_frame: false,
            middle_pressed_this_frame: false,
            secondary_drag: input.secondary_drag.filter(|drag| !drag.started),
            scroll: Vec2::ZERO,
            ..input
        },
    };
    if !ended && let Some(pos) = input.pointer_pos {
        autoscroll.travelled |=
            autoscroll.held && pos.distance(autoscroll.origin) > AUTOSCROLL_DEAD_ZONE;
        if pos != autoscroll.pos {
            autoscroll.pos = pos;
            report(doc, &autoscroll, false);
        }
    }
    if autoscroll.held && !input.middle_down {
        autoscroll.held = false;
        ended |= autoscroll.travelled;
    }
    doc.autoscroll = match ended {
        true => {
            report(doc, &autoscroll, true);
            None
        }
        false => Some(autoscroll),
    };
    Tracked {
        input,
        swallows_escape,
    }
}

pub(crate) fn show_cursor(doc: &Document, ctx: &Context) {
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

fn start(doc: &mut Document, rects: &Rects, root: NodeId, input: InteractInput) -> Tracked {
    let untouched = Tracked {
        input,
        swallows_escape: false,
    };
    if !input.middle_pressed_this_frame {
        return untouched;
    }
    let Some(pos) = input.pointer_pos else {
        return untouched;
    };
    let Some(target) = super::target(doc, rects, root, Some(pos), &claims_middle) else {
        return untouched;
    };
    if !catcher(doc, target).is_some_and(ClickCatcherNode::wants_autoscroll) {
        return untouched;
    }
    let autoscroll = Autoscroll {
        target,
        origin: pos,
        pos,
        held: true,
        travelled: false,
    };
    doc.autoscroll = Some(autoscroll);
    report(doc, &autoscroll, false);
    Tracked {
        input: InteractInput {
            middle_pressed_this_frame: false,
            ..input
        },
        swallows_escape: false,
    }
}

fn claims_middle(element: &dyn Element) -> bool {
    let any = element.as_any();
    any.is::<EmbedNode>()
        || any
            .downcast_ref::<ClickCatcherNode>()
            .is_some_and(ClickCatcherNode::claims_middle)
}

fn catcher(doc: &Document, id: NodeId) -> Option<&ClickCatcherNode> {
    doc.arena
        .get(id)
        .as_any()
        .downcast_ref::<ClickCatcherNode>()
        .filter(|catcher| catcher.wants_autoscroll())
}

fn report(doc: &Document, autoscroll: &Autoscroll, ended: bool) {
    let Some(catcher) = catcher(doc, autoscroll.target) else {
        return;
    };
    let on_autoscroll = catcher.on_autoscroll.clone();
    on_autoscroll.call(AutoscrollGesture {
        origin: autoscroll.origin,
        pos: autoscroll.pos,
        ended,
    });
}
