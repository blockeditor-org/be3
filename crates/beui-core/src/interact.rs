pub mod autoscroll;
pub mod forward;

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::base::list::Direction;
use crate::context::Context;
use crate::geometry::{Pos2, Rect, Vec2, vec2};
use crate::input::{BackGesture, Event, ImeEvent, Key, KeyPress, Modifiers};
use crate::painter::Painter;

use crate::document::{Acted, Document, GlobalKeyPress};
use crate::node::{InteractInput, NodeId, NodeMap, Rects};

pub const WHEEL_LATCH_TIMEOUT: Duration = Duration::from_millis(500);
const WHEEL_LATCH_SLOP: f32 = 2.0;
pub const TOUCH_REACH: f32 = 12.0;

#[derive(Clone, Copy)]
pub enum Keys {
    Ignored,
    All,
    Except(fn(&Event) -> bool),
}

impl Keys {
    pub fn ignored(self) -> bool {
        matches!(self, Self::Ignored)
    }

    fn claimed(self, event: &Event) -> bool {
        match self {
            Self::Except(claimed) => claimed(event),
            _ => false,
        }
    }
}

pub fn interact(
    doc: &mut Document,
    ctx: &Context,
    painter: &Painter,
    rects: &Rects,
    root: NodeId,
    pointer: bool,
    keys: Keys,
) {
    let modifiers = ctx.input(|input| input.modifiers);
    let wheel = ctx.input(|input| input.scroll_delta);
    let fling = ctx.input(|input| input.scroll_fling);
    let (wheel, fling) = if modifiers.shift && wheel.x == 0.0 && fling.x == 0.0 {
        (vec2(wheel.y, 0.0), vec2(fling.y, 0.0))
    } else {
        (wheel, fling)
    };
    if ctx.input(|input| input.events.iter().any(is_activity)) {
        doc.note_input();
    }
    let touching =
        ctx.input(|input| input.touch.active() || input.touch.ended() || input.touch.cancelled());
    let raw_pointer = ctx.input(|input| input.pointer.interact_pos());
    if pointer
        && ctx.input(|input| input.touch.started())
        && let Some(pos) = ctx.input(|input| input.pointer.press_pos)
    {
        doc.touch_shift = touch_shift(doc, rects, root, pos);
    }
    if !touching {
        doc.touch_shift = Vec2::ZERO;
    }
    let pressed = ctx.input(|input| {
        input.pointer.primary_pressed()
            || input.pointer.secondary_pressed()
            || input.touch.started()
    });
    let moved = ctx.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, Event::PointerMoved(_)))
    });
    let typed = !keys.ignored()
        && ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, Event::Key { pressed: true, .. }))
        });
    let press = ctx
        .input(|input| input.pointer.press_pos)
        .filter(|_| pointer && pressed);
    let motion = raw_pointer.filter(|_| pointer && moved);
    if let Some(pos) = press {
        doc.acted = Acted::Pointer(pos + doc.touch_shift);
    } else if typed {
        doc.acted = Acted::Keys;
    } else if let Some(pos) = motion {
        doc.acted = Acted::Pointer(pos + doc.touch_shift);
    }
    if pointer && let Some(pos) = raw_pointer {
        doc.last_pointer = Some(crate::input::PointerSample {
            pos: pos + doc.touch_shift,
            touch: ctx.input(|input| input.pointer.from_touch()),
        });
    }
    let input = InteractInput {
        pointer_pos: raw_pointer.map(|pos| pos + doc.touch_shift),
        press_pos: ctx
            .input(|input| input.pointer.press_pos)
            .map(|pos| pos + doc.touch_shift),
        pointer_down: ctx.input(|input| input.pointer.primary_down),
        pressed_this_frame: ctx.input(|input| input.pointer.primary_pressed()),
        released_this_frame: ctx.input(|input| input.pointer.primary_released()),
        secondary_pressed_this_frame: ctx.input(|input| input.pointer.secondary_pressed()),
        secondary_drag: ctx.input(|input| input.pointer.secondary_drag),
        middle_down: ctx.input(|input| input.pointer.middle_down),
        middle_pressed_this_frame: ctx.input(|input| input.pointer.middle_pressed()),
        middle_released_this_frame: ctx.input(|input| input.pointer.middle_released()),
        scroll: wheel,
        scroll_fling: fling,
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
        visible: Rect::EVERYTHING,
    };
    let input = match pointer {
        true => input,
        false => without_pointer(input),
    };
    let autoscroll::Tracked {
        input,
        swallows_escape,
    } = autoscroll::track(doc, ctx, rects, root, input);

    if (input.pressed_this_frame || input.touch_started || input.secondary_pressed_this_frame)
        && let Some(pos) = input.pointer_pos
    {
        doc.dismiss_light_overlays(pos);
    }
    if (input.pressed_this_frame || input.touch_started)
        && let Some(pos) = input.pointer_pos
    {
        doc.pressed_outside(rects, pos);
    }
    if input.touch_started {
        doc.touch_scroll_vertical = target(doc, rects, root, input.pointer_pos, &|element| {
            catches_drag(element, Direction::Vertical)
        });
        doc.touch_scroll_horizontal = target(doc, rects, root, input.pointer_pos, &|element| {
            catches_drag(element, Direction::Horizontal)
        });
    }
    let mouse = !input.touch_started && !input.touch_active;
    let claimed = (input.pressed_this_frame && mouse)
        .then_some(input.pointer_pos)
        .flatten()
        .and_then(|pos| claimant(doc, rects, root, pos, input.modifiers));
    if let Some(claimant) = claimed {
        doc.capture_pointer(claimant);
        doc.press_claim = Some(claimant);
        claim_press(doc, claimant);
    } else if input.pressed_this_frame
        && let Some(pos) = input.pointer_pos
        && let Some(captor) = doc
            .pointer_layers(root)
            .into_iter()
            .find_map(|layer| captor(doc, rects, layer, pos))
    {
        doc.capture_pointer(captor);
    }
    if input.secondary_pressed_this_frame
        && mouse
        && let Some(pos) = input.pointer_pos
        && let Some(claimant) = claimant(doc, rects, root, pos, input.modifiers)
    {
        doc.secondary_claim = Some(claimant);
        claim_press(doc, claimant);
    }
    let wheel = match input.scroll {
        Vec2::ZERO => input.scroll_fling,
        scroll => scroll,
    };
    let wheel_target = (wheel != Vec2::ZERO)
        .then(|| {
            let now = doc.now();
            let target =
                latched_wheel_target(doc, rects, input.pointer_pos, wheel, now).or_else(|| {
                    target(doc, rects, root, input.pointer_pos, &|element| {
                        wants_wheel(element, wheel)
                    })
                });
            doc.wheel_latch = target.map(|target| (target, now, input.pointer_pos));
            target
        })
        .flatten();
    let zoom_target = (input.zoom != 1.0 || input.touch_pan != Vec2::ZERO)
        .then(|| target(doc, rects, root, input.zoom_pos, &wants_gestures))
        .flatten();
    let (vertical, horizontal) = match pointer {
        true => ctx.input(|state| {
            (
                state.touch.scrolling(),
                state.touch.scrolling_horizontally(),
            )
        }),
        false => (false, false),
    };
    let touch_scroll_target = match (vertical, horizontal) {
        (true, _) => doc.touch_scroll_vertical,
        (_, true) => doc.touch_scroll_horizontal,
        _ => None,
    };
    let input = InteractInput {
        touch_scrolling: touch_scroll_target.is_some(),
        touch_scroll_target,
        wheel_target,
        zoom_target,
        ..input
    };

    if let Some(pos) = input.pointer_pos {
        let drags = doc.drag_board();
        drags.track(crate::drag_board::DragPoint {
            pos,
            modifiers: input.modifiers,
        });
    }
    let mut focus_target = None;
    let focus_before = doc.focused_node();
    let covered = input
        .pointer_pos
        .is_some_and(|pos| doc.floating_covers(pos));
    let under = match covered {
        false => input,
        true => hidden_pointer(input),
    };
    let engaged_before = doc.engaged.clone();
    let reach = Reach::new(doc, rects, &input);
    doc.interact_pass += 1;
    let mut pool = doc.take_interact_pool();
    if doc.overlay_stack.is_empty() {
        interact_node(
            doc,
            painter,
            &under,
            root,
            &reach,
            &mut focus_target,
            &mut pool,
        );
    } else {
        if input
            .pointer_pos
            .is_some_and(|pos| doc.pointer_passes_under_overlays(pos))
        {
            interact_node(
                doc,
                painter,
                &under,
                root,
                &reach,
                &mut focus_target,
                &mut pool,
            );
        }
        for overlay in doc.overlay_stack.clone() {
            interact_node(
                doc,
                painter,
                &under,
                overlay.id(),
                &reach,
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
                doc.node_rect(*above).is_some_and(|rect| {
                    input
                        .pointer_pos
                        .is_some_and(|pos| rect.contains_half_open(pos))
                })
            })
        })
        .collect();
    let modal = doc.modal_open();
    for (content, shadowed) in floating.into_iter().zip(shadowed) {
        let above = match shadowed || modal {
            false => input,
            true => hidden_pointer(input),
        };
        interact_node(
            doc,
            painter,
            &above,
            content,
            &reach,
            &mut focus_target,
            &mut pool,
        );
    }
    doc.put_back_interact_pool(pool);
    disengage_unreached(doc, engaged_before);
    autoscroll::show_cursor(doc, ctx);

    if pointer
        && let Some(fingers) = ctx.input(|input| input.touch.finger_tap())
        && doc.overlay_stack.is_empty()
    {
        doc.finger_tap(fingers);
    }
    if pointer && (input.pressed_this_frame || input.touch_started) {
        doc.set_focus_visible(false);
    }
    if doc.pointer_capture.is_none()
        && doc.focused_node() == focus_before
        && ((input.pressed_this_frame && !input.touch_started)
            || (input.touch_ended && !input.touch_dragged && !input.touch_cancelled))
    {
        doc.press_focus(focus_target);
    }

    doc.validate_focus();
    if !keys.ignored() {
        global_keys(doc, ctx);
    }
    forward::route(doc, ctx, rects, root, pointer, keys);
    let forwarded_keys = forward::takes_keys(doc);
    if ctx.pointer_locked() && pointer && !keys.ignored() && !forwarded_keys {
        let motion = ctx.input(|input| input.pointer.motion);
        if motion != Vec2::ZERO {
            doc.motion_focused(motion);
        }
        doc.validate_focus();
    }

    for event in ctx.input(|input| input.events.clone()) {
        ::reactive::settle(|| {});
        doc.validate_focus();
        if let Event::Modifiers(modifiers) | Event::Key { modifiers, .. } = &event {
            doc.set_modifiers(*modifiers);
        }
        let (key, pressed, repeat, modifiers) = match event {
            Event::Key {
                key: Key::Escape, ..
            } if swallows_escape => continue,
            Event::Focus(false) => {
                doc.cancel_focus_activation();
                if ctx.pointer_locked() {
                    doc.update_focus(None);
                }
                continue;
            }
            Event::Back(gesture) if !keys.ignored() => {
                doc.back(gesture);
                continue;
            }
            Event::Text(_) | Event::Key { .. } | Event::Ime(_) if keys.ignored() => continue,
            Event::Text(_) | Event::Key { .. } | Event::Ime(_) if forwarded_keys => continue,
            Event::Key { key, .. } if key.is_modifier() || keys.claimed(&event) => {
                continue;
            }
            Event::Text(text) => {
                doc.text_focused(&text);
                doc.reveal_focus(painter);
                continue;
            }
            Event::Ime(ime) => {
                let reveals = !matches!(ime, ImeEvent::Enabled | ImeEvent::Disabled);
                doc.ime_focused(ime);
                if reveals {
                    doc.reveal_focus(painter);
                }
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
        if pressed && !modifiers.ctrl && !modifiers.alt {
            doc.set_focus_visible(true);
        }
        if doc.overlay_stack.is_empty() && doc.key_shortcut(press) {
            doc.reveal_focus(painter);
            continue;
        }
        if doc.key_focused(press) {
            doc.reveal_focus(painter);
            continue;
        }
        if doc.overlay_stack.is_empty() && doc.key_unhandled(press) {
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
                doc.dismiss_topmost_overlay();
            }
            Key::Escape if pressed => doc.cancel_focus_activation(),
            Key::BrowserBack if pressed => doc.back(BackGesture::Invoked),
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
    if !input.pointer_down && doc.dragging() {
        if input.touch_cancelled {
            doc.drag_board().end(None);
        } else if input.released_this_frame || input.touch_ended {
            doc.drag_board().finish();
        }
    }
    if !input.pointer_down {
        doc.pointer_capture = None;
        doc.press_claim = None;
    }
    if input
        .secondary_drag
        .is_none_or(|drag| drag.ended || drag.cancelled)
    {
        doc.secondary_claim = None;
    }
}

fn hidden_pointer(input: InteractInput) -> InteractInput {
    InteractInput {
        pointer_pos: None,
        press_pos: None,
        secondary_drag: input.secondary_drag.filter(|drag| !drag.started),
        zoom_pos: None,
        wheel_target: None,
        zoom_target: None,
        ..input
    }
}

fn claim_press(doc: &mut Document, claimant: NodeId) {
    match forward::wants_forward(doc.arena.get(claimant)) {
        true => doc.forward.claim_press(claimant),
        false => doc.forward.swallow_press(),
    }
}

fn claimant(
    doc: &Document,
    rects: &Rects,
    root: NodeId,
    pos: Pos2,
    held: Modifiers,
) -> Option<NodeId> {
    if !held.any() {
        return None;
    }
    let modal = !doc.overlay_stack.is_empty();
    for layer in doc.pointer_layers(root) {
        if let Some(found) = claiming(doc, rects, layer, pos, held) {
            return Some(found);
        }
        if modal {
            return None;
        }
        let covers = layer != root
            && doc
                .arena
                .kind_of::<crate::base::overlay::OverlayNode>(layer)
                .and_then(|overlay| doc.overlay_content(overlay))
                .and_then(|content| rects.visible(&content))
                .is_some_and(|rect| rect.contains_half_open(pos));
        if covers {
            return None;
        }
    }
    None
}

fn claiming(
    doc: &Document,
    rects: &Rects,
    id: NodeId,
    pos: Pos2,
    held: Modifiers,
) -> Option<NodeId> {
    let rect = rects.visible(&id)?;
    let node = doc.arena.get(id);
    if node
        .as_any()
        .downcast_ref::<crate::base::interactive::InteractiveNode>()
        .is_some_and(|catcher| catcher.claims(pos, rect, held))
    {
        return Some(id);
    }
    node.children()
        .into_iter()
        .rev()
        .find_map(|child| claiming(doc, rects, child, pos, held))
}

fn global_keys(doc: &mut Document, ctx: &Context) {
    let events = ctx.input(|input| input.events.clone());
    let taps = taps(doc, &events);
    if events
        .iter()
        .any(|event| matches!(event, Event::Focus(false)))
    {
        doc.globally_held.clear();
    }
    if !events.iter().any(|event| {
        matches!(
            event,
            Event::Key { .. } | Event::InterceptedKey(_) | Event::InterceptedTap(_)
        )
    }) {
        return;
    }
    let in_app = forward::takes_keys(doc);
    let typing = in_app || doc.focus_types();
    let mut keep = vec![true; events.len()];
    for (index, event) in events.iter().enumerate() {
        let (key, pressed, repeat, modifiers) = match *event {
            Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
            } => (key, pressed, repeat, modifiers),
            Event::Focus(false) => {
                doc.globally_held.clear();
                continue;
            }
            Event::InterceptedKey(press) => {
                doc.set_modifiers(press.modifiers);
                doc.key_global(GlobalKeyPress {
                    press,
                    typing: true,
                    in_app: true,
                    held: false,
                    tap: false,
                });
                ::reactive::settle(|| {});
                keep[index] = false;
                continue;
            }
            Event::InterceptedTap(key) => {
                doc.key_global(GlobalKeyPress {
                    press: KeyPress {
                        key,
                        pressed: false,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    },
                    typing: true,
                    in_app: true,
                    held: false,
                    tap: true,
                });
                ::reactive::settle(|| {});
                keep[index] = false;
                continue;
            }
            _ => continue,
        };
        doc.set_modifiers(modifiers);
        let press = KeyPress {
            key,
            pressed,
            repeat,
            modifiers,
        };
        let offered = GlobalKeyPress {
            press,
            typing,
            in_app,
            held: false,
            tap: false,
        };
        if pressed && !repeat {
            doc.globally_held.retain(|(other, _)| *other != key);
        }
        let taken = match doc
            .globally_held
            .iter()
            .position(|(other, _)| *other == key)
        {
            Some(at) => {
                let taker = doc.globally_held[at].1.clone();
                if !pressed {
                    doc.globally_held.remove(at);
                }
                match taker {
                    Some(taker) => {
                        taker(GlobalKeyPress {
                            held: true,
                            ..offered
                        });
                    }
                    None if pressed => {
                        forward::cycle_focus(doc, cycle_step(modifiers));
                    }
                    None => {}
                }
                true
            }
            None => {
                let taker = match doc.global_taker(offered) {
                    Some(taker) => Some(Some(taker)),
                    None if pressed && key == Key::F6 => {
                        forward::cycle_focus(doc, cycle_step(modifiers)).then_some(None)
                    }
                    None => None,
                };
                if pressed && let Some(taker) = &taker {
                    doc.globally_held.push((key, taker.clone()));
                }
                taker.is_some()
            }
        };
        ::reactive::settle(|| {});
        if taken {
            drop_key(&mut keep, &events, index, pressed);
        }
        if taps.contains(&index) {
            doc.key_global(GlobalKeyPress {
                tap: true,
                ..offered
            });
            ::reactive::settle(|| {});
        }
    }
    if keep.iter().all(|keep| *keep) {
        return;
    }
    let mut kept = keep.into_iter();
    ctx.retain_events(|_| kept.next().unwrap_or(true));
}

fn is_activity(event: &Event) -> bool {
    matches!(
        event,
        Event::Key { .. }
            | Event::PointerButton { .. }
            | Event::PointerMotion(_)
            | Event::PointerMoved(_)
            | Event::Scroll(_)
            | Event::PhysicalKey { .. }
            | Event::Text(_)
            | Event::Touch { .. }
            | Event::Zoom(_)
    )
}

fn cycle_step(modifiers: Modifiers) -> isize {
    match modifiers.shift {
        true => -1,
        false => 1,
    }
}

fn drop_key(keep: &mut [bool], events: &[Event], index: usize, pressed: bool) {
    keep[index] = false;
    for before in (0..index).rev() {
        match events[before] {
            Event::Modifiers(_) | Event::Text(_) => {}
            Event::PhysicalKey {
                pressed: physical, ..
            } if physical == pressed => {
                keep[before] = false;
                break;
            }
            _ => break,
        }
    }
    for after in index + 1..events.len() {
        match events[after] {
            Event::Text(_) => keep[after] = false,
            _ => break,
        }
    }
}

fn taps(doc: &mut Document, events: &[Event]) -> Vec<usize> {
    let mut taps = Vec::new();
    for (index, event) in events.iter().enumerate() {
        match *event {
            Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
            } => doc.tapping = (key.is_modifier() && alone(key, modifiers)).then_some(key),
            Event::Key {
                key,
                pressed: false,
                ..
            } if doc.tapping == Some(key) => {
                doc.tapping = None;
                taps.push(index);
            }
            Event::Key { pressed: true, .. }
            | Event::PointerButton { pressed: true, .. }
            | Event::Scroll(_)
            | Event::Focus(false)
            | Event::Touch {
                phase: crate::input::TouchPhase::Start,
                ..
            } => doc.tapping = None,
            _ => {}
        }
    }
    taps
}

fn alone(key: Key, held: Modifiers) -> bool {
    Modifiers {
        alt: held.alt && key != Key::Alt,
        ctrl: held.ctrl && key != Key::Ctrl,
        shift: held.shift && key != Key::Shift,
        logo: held.logo && key != Key::Logo,
    } == Modifiers::NONE
}

fn without_pointer(input: InteractInput) -> InteractInput {
    InteractInput {
        pointer_pos: None,
        press_pos: None,
        pointer_down: false,
        pressed_this_frame: false,
        released_this_frame: false,
        secondary_pressed_this_frame: false,
        secondary_drag: None,
        middle_down: false,
        middle_pressed_this_frame: false,
        middle_released_this_frame: false,
        scroll: Vec2::ZERO,
        scroll_fling: Vec2::ZERO,
        zoom: 1.0,
        touch_pan: Vec2::ZERO,
        zoom_pos: None,
        touch_started: false,
        touch_active: false,
        touch_ended: false,
        touch_cancelled: false,
        touch_dragged: false,
        touch_scroll_delta: Vec2::ZERO,
        touch_velocity: Vec2::ZERO,
        clicks: 0,
        ..input
    }
}

fn captor(doc: &mut Document, rects: &Rects, id: NodeId, pos: Pos2) -> Option<NodeId> {
    let rect = rects.visible(&id)?;
    let mut element = doc.arena.take(id);
    let intercepts = rect.contains(pos) && element.intercepts(doc, pos, rect);
    doc.arena.put_back(id, element);
    if intercepts {
        return Some(id);
    }
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
        .downcast_ref::<crate::base::interactive::InteractiveNode>()
        .is_some_and(|catcher| catcher.catches_drag(direction))
}

fn wants_wheel(element: &dyn crate::node::Element, wheel: Vec2) -> bool {
    element
        .as_any()
        .downcast_ref::<crate::base::interactive::InteractiveNode>()
        .is_some_and(|catcher| catcher.wants_wheel(wheel))
}

fn latched_wheel_target(
    doc: &Document,
    rects: &Rects,
    pointer: Option<Pos2>,
    wheel: Vec2,
    now: Instant,
) -> Option<NodeId> {
    let (latched, last, at) = doc.wheel_latch?;
    let recent = now.saturating_duration_since(last) < WHEEL_LATCH_TIMEOUT;
    let still = match (at, pointer) {
        (Some(at), Some(pos)) => at.distance(pos) <= WHEEL_LATCH_SLOP,
        (at, pos) => at == pos,
    };
    let under = pointer.is_some_and(|pos| {
        rects
            .visible(&latched)
            .is_some_and(|rect| rect.contains_half_open(pos))
    });
    let still_wants = doc.arena.contains(latched) && wants_wheel(doc.arena.get(latched), wheel);
    (recent && still && under && still_wants).then_some(latched)
}

fn wants_gestures(element: &dyn crate::node::Element) -> bool {
    element
        .as_any()
        .downcast_ref::<crate::base::interactive::InteractiveNode>()
        .is_some_and(crate::base::interactive::InteractiveNode::wants_gestures)
}

fn target(
    doc: &Document,
    rects: &Rects,
    root: NodeId,
    pos: Option<Pos2>,
    wants: &dyn Fn(&dyn crate::node::Element) -> bool,
) -> Option<NodeId> {
    let pos = pos?;
    doc.pointer_layers(root)
        .into_iter()
        .find_map(|layer| match layer == root {
            true => deepest(doc, rects, root, pos, wants),
            false => doc
                .arena
                .get(layer)
                .children()
                .into_iter()
                .rev()
                .find_map(|child| deepest(doc, rects, child, pos, wants)),
        })
}

fn deepest(
    doc: &Document,
    rects: &Rects,
    id: NodeId,
    pos: Pos2,
    wants: &dyn Fn(&dyn crate::node::Element) -> bool,
) -> Option<NodeId> {
    if !rects
        .visible(&id)
        .is_some_and(|rect| rect.contains_half_open(pos))
    {
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

struct Reach<'a> {
    rects: &'a Rects,
    wanted: HashSet<NodeId>,
}

impl<'a> Reach<'a> {
    fn new(doc: &mut Document, rects: &'a Rects, input: &InteractInput) -> Self {
        if doc.interact_bounds_version != Some(rects.version()) {
            doc.interact_bounds = NodeMap::default();
            doc.interact_bounds_version = Some(rects.version());
        }
        let seeds: Vec<NodeId> = std::mem::take(&mut doc.engaged)
            .into_iter()
            .chain(doc.pointer_capture)
            .chain(input.wheel_target)
            .chain(input.zoom_target)
            .chain(input.touch_scroll_target)
            .collect();
        let mut wanted = HashSet::new();
        let mut pending = seeds;
        while let Some(id) = pending.pop() {
            if !wanted.insert(id) {
                continue;
            }
            pending.extend(doc.arena.parent(id));
            pending.extend(doc.interact_parents.get(&id).copied());
        }
        Self { rects, wanted }
    }

    fn reaches(&self, doc: &mut Document, input: &InteractInput, id: NodeId) -> bool {
        if self.wanted.contains(&id) {
            return true;
        }
        let started = input
            .secondary_drag
            .filter(|drag| drag.started)
            .map(|drag| drag.from);
        let pressed = input.press_pos.filter(|_| input.pressed_this_frame);
        let probes = [input.pointer_pos, started, pressed];
        if probes.iter().all(Option::is_none) {
            return false;
        }
        let bounds = subtree_bounds(doc, self.rects, id);
        probes
            .into_iter()
            .flatten()
            .any(|pos| bounds.contains_half_open(pos))
    }
}

fn subtree_bounds(doc: &mut Document, rects: &Rects, id: NodeId) -> Rect {
    if let Some(bounds) = doc.interact_bounds.get(&id) {
        return *bounds;
    }
    let Some(rect) = rects.get(&id) else {
        return Rect::NOTHING;
    };
    doc.interact_bounds.insert(id, rect);
    if !doc.arena.contains(id) {
        return rect;
    }
    let mut bounds = rect;
    for child in doc.arena.get(id).children() {
        if rects.contains_key(&child) {
            bounds = bounds.union(subtree_bounds(doc, rects, child));
        }
    }
    doc.interact_bounds.insert(id, bounds);
    bounds
}

fn interact_node(
    doc: &mut Document,
    painter: &Painter,
    input: &InteractInput,
    id: NodeId,
    reach: &Reach<'_>,
    focus_target: &mut Option<NodeId>,
    pool: &mut Vec<Vec<NodeId>>,
) {
    let rects = reach.rects;
    let (Some(rect), Some(visible)) = (rects.get(&id), rects.visible(&id)) else {
        return;
    };
    if !reach.reaches(doc, input, id) {
        return;
    }
    let pass = doc.interact_pass;
    if doc.interacted.insert(id, pass) == Some(pass) {
        return;
    }
    let mut children = pool.pop().unwrap_or_default();
    let mut element = doc.arena.take(id);
    let clipped = InteractInput { visible, ..*input };
    element.interact(
        doc,
        painter,
        &clipped,
        id,
        rect,
        focus_target,
        &mut children,
    );
    let engaged = element.engaged();
    doc.arena.put_back(id, element);
    if engaged {
        doc.engaged.push(id);
    }

    for &child in &children {
        if rects.contains_key(&child) && !doc.is_culled(child) {
            doc.interact_parents.insert(child, id);
            interact_node(doc, painter, input, child, reach, focus_target, pool);
        }
    }
    children.clear();
    pool.push(children);
}

fn disengage_unreached(doc: &mut Document, engaged: Vec<NodeId>) {
    let pass = doc.interact_pass;
    for id in engaged {
        if !doc.arena.contains(id) || doc.interacted.get(&id) == Some(&pass) {
            continue;
        }
        let mut element = doc.arena.take(id);
        element.disengage(doc);
        doc.arena.put_back(id, element);
    }
}

fn touch_shift(doc: &Document, rects: &Rects, root: NodeId, pos: Pos2) -> Vec2 {
    let modal = !doc.overlay_stack.is_empty();
    for layer in doc.pointer_layers(root) {
        let tops: Vec<NodeId> = match (layer == root, modal) {
            (true, _) => vec![root],
            (false, true) => doc.arena.get(layer).children().into_iter().rev().collect(),
            (false, false) => doc
                .arena
                .kind_of::<crate::base::overlay::OverlayNode>(layer)
                .and_then(|overlay| doc.overlay_content(overlay))
                .into_iter()
                .collect(),
        };
        if tops
            .iter()
            .any(|top| deepest(doc, rects, *top, pos, &presses).is_some())
        {
            return Vec2::ZERO;
        }
        let mut nearest: Option<(f32, Pos2)> = None;
        for top in &tops {
            nearest_press(doc, rects, *top, pos, Rect::EVERYTHING, &mut nearest);
        }
        if let Some((_, at)) = nearest {
            return at - pos;
        }
        let covered = tops.iter().any(|top| {
            rects
                .get(top)
                .is_some_and(|rect| rect.contains_half_open(pos))
        });
        if layer != root && covered {
            return Vec2::ZERO;
        }
    }
    Vec2::ZERO
}

fn nearest_press(
    doc: &Document,
    rects: &Rects,
    id: NodeId,
    pos: Pos2,
    clip: Rect,
    nearest: &mut Option<(f32, Pos2)>,
) {
    let Some(rect) = rects.visible(&id).map(|rect| rect.intersect(clip)) else {
        return;
    };
    if !rect.is_positive() || !rect.expand(TOUCH_REACH).contains(pos) {
        return;
    }
    let node = doc.arena.get(id);
    if presses(node) {
        let at = Pos2::new(
            pos.x.clamp(rect.left(), rect.right().next_down()),
            pos.y.clamp(rect.top(), rect.bottom().next_down()),
        );
        let away = at.distance(pos);
        if away <= TOUCH_REACH && nearest.is_none_or(|(held, _)| away < held) {
            *nearest = Some((away, at));
        }
    }
    for child in node.children() {
        nearest_press(doc, rects, child, pos, rect, nearest);
    }
}

fn presses(element: &dyn crate::node::Element) -> bool {
    element
        .as_any()
        .downcast_ref::<crate::base::interactive::InteractiveNode>()
        .is_some_and(|catcher| catcher.claims_touches())
}
