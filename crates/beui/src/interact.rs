use crate::base::list::Direction;
use crate::context::Context;
use crate::geometry::{Pos2, Rect, Vec2, vec2};
use crate::input::{Event, Key, KeyPress};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{InteractInput, NodeId, NodeMap};

pub(crate) fn interact(
    doc: &mut Document,
    ctx: &Context,
    painter: &Painter,
    rects: &NodeMap<Rect>,
    root: NodeId,
    keyboard_interactive: bool,
) {
    let modifiers = ctx.input(|input| input.modifiers);
    let wheel = ctx.input(|input| input.scroll_delta);
    let wheel = if modifiers.shift && wheel.x == 0.0 {
        vec2(wheel.y, 0.0)
    } else {
        wheel
    };
    let input = InteractInput {
        pointer_pos: ctx.input(|input| input.pointer.interact_pos()),
        pointer_down: ctx.input(|input| input.pointer.primary_down),
        pressed_this_frame: ctx.input(|input| input.pointer.primary_pressed()),
        released_this_frame: ctx.input(|input| input.pointer.primary_released()),
        secondary_pressed_this_frame: ctx.input(|input| input.pointer.secondary_pressed()),
        middle_down: ctx.input(|input| input.pointer.middle_down),
        middle_pressed_this_frame: ctx.input(|input| input.pointer.middle_pressed()),
        scroll: wheel,
        zoom: ctx.input(|input| input.zoom_factor * input.touch.pinch()),
        touch_pan: ctx.input(|input| input.touch.pinch_pan()),
        zoom_pos: ctx.input(|input| input.touch.pinch_center().or(input.pointer.interact_pos())),
        wheel_target: None,
        zoom_target: None,
        touch_started: ctx.input(|input| input.touch.started()),
        touch_active: ctx.input(|input| input.touch.active()),
        touch_ended: ctx.input(|input| input.touch.ended()),
        touch_cancelled: ctx.input(|input| input.touch.cancelled()),
        touch_dragged: ctx.input(|input| input.touch.dragged()),
        touch_scrolling: false,
        touch_scroll_delta: ctx.input(|input| input.touch.scroll_delta()),
        touch_velocity: ctx.input(|input| input.touch.velocity()),
        touch_scroll_target: None,
        clicks: ctx.input(|input| input.pointer.clicks()),
        modifiers,
    };

    if input.touch_started {
        doc.touch_scroll_vertical = target(doc, rects, root, input.pointer_pos, &|element| {
            catches_drag(element, Direction::Vertical)
        });
        doc.touch_scroll_horizontal = target(doc, rects, root, input.pointer_pos, &|element| {
            catches_drag(element, Direction::Horizontal)
        });
    }
    if input.pressed_this_frame
        && let Some(pos) = input.pointer_pos
    {
        let under: Vec<NodeId> = if doc.overlay_stack.is_empty() {
            vec![root]
        } else {
            doc.overlay_stack.iter().rev().copied().collect()
        };
        let layers: Vec<NodeId> = doc
            .floating_overlays()
            .into_iter()
            .rev()
            .chain(under)
            .collect();
        if let Some(captor) = layers
            .into_iter()
            .find_map(|layer| captor(doc, rects, layer, pos))
        {
            doc.capture_pointer(captor);
        }
    }
    let wheel_target = (input.scroll != Vec2::ZERO)
        .then(|| {
            target(doc, rects, root, input.pointer_pos, &|element| {
                wants_wheel(element, input.scroll)
            })
        })
        .flatten();
    let zoom_target = (input.zoom != 1.0 || input.touch_pan != Vec2::ZERO)
        .then(|| target(doc, rects, root, input.zoom_pos, &wants_gestures))
        .flatten();
    let (vertical, horizontal) = ctx.input(|state| {
        (
            state.touch.scrolling(),
            state.touch.scrolling_horizontally(),
        )
    });
    let touch_scroll_target = match (vertical, horizontal) {
        (true, _) => doc.touch_scroll_vertical,
        (_, true) => doc.touch_scroll_horizontal,
        _ => None,
    };
    let input = InteractInput {
        touch_scrolling: vertical || (horizontal && touch_scroll_target.is_some()),
        touch_scroll_target,
        wheel_target,
        zoom_target,
        ..input
    };

    let mut focus_target = None;
    let covered = input
        .pointer_pos
        .is_some_and(|pos| doc.floating_covers(pos));
    let under = match covered {
        false => input,
        true => InteractInput {
            pointer_pos: None,
            zoom_pos: None,
            wheel_target: None,
            zoom_target: None,
            touch_scroll_target: None,
            ..input
        },
    };
    let mut pool = doc.take_interact_pool();
    if doc.overlay_stack.is_empty() {
        interact_node(
            doc,
            painter,
            &under,
            rects,
            root,
            &mut focus_target,
            &mut pool,
        );
    } else {
        for overlay in doc.overlay_stack.clone() {
            interact_node(
                doc,
                painter,
                &under,
                rects,
                overlay,
                &mut focus_target,
                &mut pool,
            );
        }
    }
    let floating: Vec<NodeId> = doc
        .floating_overlays()
        .into_iter()
        .filter_map(|overlay| doc.overlay_content(overlay))
        .collect();
    let shadowed: Vec<bool> = floating
        .iter()
        .enumerate()
        .map(|(level, _)| {
            floating[level + 1..].iter().any(|above| {
                doc.node_rect(*above)
                    .is_some_and(|rect| input.pointer_pos.is_some_and(|pos| rect.contains(pos)))
            })
        })
        .collect();
    for (content, shadowed) in floating.into_iter().zip(shadowed) {
        let above = match shadowed {
            false => input,
            true => InteractInput {
                pointer_pos: None,
                zoom_pos: None,
                wheel_target: None,
                zoom_target: None,
                touch_scroll_target: None,
                ..input
            },
        };
        interact_node(
            doc,
            painter,
            &above,
            rects,
            content,
            &mut focus_target,
            &mut pool,
        );
    }
    doc.put_back_interact_pool(pool);

    if doc.pointer_capture.is_none()
        && ((input.pressed_this_frame && !input.touch_started)
            || (input.touch_ended && !input.touch_dragged && !input.touch_cancelled))
    {
        doc.update_focus(focus_target);
    }

    doc.validate_focus();
    for event in ctx.input(|input| input.events.clone()) {
        doc.validate_focus();
        let (key, pressed, repeat, modifiers) = match event {
            Event::Focus(false) => {
                doc.cancel_focus_activation();
                continue;
            }
            Event::Text(_) | Event::Key { .. } if !keyboard_interactive => continue,
            Event::Text(text) => {
                doc.text_focused(&text);
                doc.reveal_focus(painter);
                continue;
            }
            Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
            } => (key, pressed, repeat, modifiers),
            _ => continue,
        };
        let press = KeyPress {
            key,
            pressed,
            repeat,
            modifiers,
        };
        if doc.key_focused(press) {
            doc.reveal_focus(painter);
            continue;
        }
        if doc.key_ancestor(press) {
            continue;
        }
        match key {
            Key::Tab if pressed && !modifiers.ctrl && !modifiers.alt => {
                if modifiers.shift {
                    doc.focus_previous();
                } else {
                    doc.focus_next();
                }
            }
            Key::Escape if pressed && !doc.overlay_stack.is_empty() => {
                doc.close_topmost_overlay();
            }
            Key::Escape if pressed => doc.cancel_focus_activation(),
            Key::Enter | Key::Space if !pressed || (!modifiers.ctrl && !modifiers.alt) => {
                doc.set_focus_key_pressed(key, pressed, repeat);
            }
            Key::ArrowLeft | Key::ArrowDown if pressed && !modifiers.ctrl && !modifiers.alt => {
                doc.step_focused(-1.0)
            }
            Key::ArrowRight | Key::ArrowUp if pressed && !modifiers.ctrl && !modifiers.alt => {
                doc.step_focused(1.0)
            }
            _ => {}
        }
        doc.reveal_focus(painter);
    }
    doc.validate_focus();
    if input.touch_ended || input.touch_cancelled {
        doc.touch_scroll_vertical = None;
        doc.touch_scroll_horizontal = None;
    }
    if !input.pointer_down {
        doc.pointer_capture = None;
    }
}

fn captor(doc: &mut Document, rects: &NodeMap<Rect>, id: NodeId, pos: Pos2) -> Option<NodeId> {
    let rect = *rects.get(&id)?;
    for child in doc.arena.get(id).children().into_iter().rev() {
        if let Some(found) = captor(doc, rects, child, pos) {
            return Some(found);
        }
    }
    let mut element = doc.arena.take(id);
    let captures = element.captures(doc, pos, rect);
    doc.arena.put_back(id, element);
    captures.then_some(id)
}

fn catches_drag(element: &dyn crate::node::Element, direction: Direction) -> bool {
    element
        .as_any()
        .downcast_ref::<crate::base::click_catcher::ClickCatcherNode>()
        .is_some_and(|catcher| catcher.catches_drag(direction))
}

fn wants_wheel(element: &dyn crate::node::Element, wheel: Vec2) -> bool {
    element
        .as_any()
        .downcast_ref::<crate::base::click_catcher::ClickCatcherNode>()
        .is_some_and(|catcher| catcher.wants_wheel(wheel))
}

fn wants_gestures(element: &dyn crate::node::Element) -> bool {
    element
        .as_any()
        .downcast_ref::<crate::base::click_catcher::ClickCatcherNode>()
        .is_some_and(crate::base::click_catcher::ClickCatcherNode::wants_gestures)
}

fn target(
    doc: &Document,
    rects: &NodeMap<Rect>,
    root: NodeId,
    pos: Option<Pos2>,
    wants: &dyn Fn(&dyn crate::node::Element) -> bool,
) -> Option<NodeId> {
    let pos = pos?;
    if doc.overlay_stack.is_empty() {
        return deepest(doc, rects, root, pos, wants);
    }
    doc.overlay_stack
        .iter()
        .rev()
        .find_map(|overlay| deepest(doc, rects, *overlay, pos, wants))
}

fn deepest(
    doc: &Document,
    rects: &NodeMap<Rect>,
    id: NodeId,
    pos: Pos2,
    wants: &dyn Fn(&dyn crate::node::Element) -> bool,
) -> Option<NodeId> {
    if !rects.get(&id).is_some_and(|rect| rect.contains(pos)) {
        return None;
    }
    let node = doc.arena.get(id);
    for child in node.children().into_iter().rev() {
        if let Some(found) = deepest(doc, rects, child, pos, wants) {
            return Some(found);
        }
    }
    wants(node).then_some(id)
}

fn interact_node(
    doc: &mut Document,
    painter: &Painter,
    input: &InteractInput,
    rects: &NodeMap<Rect>,
    id: NodeId,
    focus_target: &mut Option<NodeId>,
    pool: &mut Vec<Vec<NodeId>>,
) {
    let Some(&rect) = rects.get(&id) else {
        return;
    };
    let mut children = pool.pop().unwrap_or_default();
    let mut element = doc.arena.take(id);
    element.interact(doc, painter, input, id, rect, focus_target, &mut children);
    doc.arena.put_back(id, element);

    for &child in &children {
        if rects.contains_key(&child) {
            interact_node(doc, painter, input, rects, child, focus_target, pool);
        }
    }
    children.clear();
    pool.push(children);
}
