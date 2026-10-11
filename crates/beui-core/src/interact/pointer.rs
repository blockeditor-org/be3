use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::base::interactive::{InteractiveNode, press_at};
use crate::base::list::Direction;
use crate::context::Context;
use crate::document::{Acted, Document};
use crate::geometry::{Pos2, Rect, Vec2, vec2};
use crate::input::{
    DragGesture, FINGER_TAP_TIME, MAX_TOUCH_VELOCITY, Modifiers, PointerButton, PointerEvent,
    PointerId, PointerPhase, PointerPress, PointerSample, ScrollGesture, SecondaryDrag,
    TOUCH_DRAG_THRESHOLD, TOUCH_VELOCITY_WINDOW, TouchId, ZoomGesture,
};
use crate::node::{NodeId, Rects};

use super::hit::{self, Hit, interactive};
use super::{WHEEL_LATCH_SLOP, WHEEL_LATCH_TIMEOUT, autoscroll};

#[derive(Default)]
pub struct Pointers {
    sessions: Vec<Session>,
    hovered: Vec<NodeId>,
    wheel_latch: Option<(NodeId, Instant, Option<Pos2>)>,
    fingers: Fingers,
    pinch_target: Option<NodeId>,
    pub(crate) routed: Vec<(usize, Option<NodeId>)>,
    pub(crate) wheel_sink: Option<NodeId>,
    pub(crate) shift: Vec2,
}

impl Pointers {
    pub fn holding(&self) -> bool {
        !self.sessions.is_empty()
    }

    pub(crate) fn sink_of(&self, pointer: PointerId) -> Option<NodeId> {
        self.sessions
            .iter()
            .filter(|session| session.pointer == pointer)
            .find_map(|session| session.sink)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Press,
    Secondary,
    Middle,
    Finger,
    Other,
}

struct Session {
    pointer: PointerId,
    button: PointerButton,
    role: Role,
    origin: Pos2,
    pos: Pos2,
    previous: Pos2,
    shift: Vec2,
    path: Vec<NodeId>,
    owners: Vec<NodeId>,
    claimed: bool,
    focus: Option<NodeId>,
    sink: Option<NodeId>,
    pan: Option<NodeId>,
    modifiers: Modifiers,
    clicks: u32,
    dragged: bool,
    axis: Option<Direction>,
    drag_target: Option<NodeId>,
    drag_started: bool,
    cancelled: bool,
    long_press: Option<Instant>,
    last_drag: Option<Pos2>,
    dragged_this_frame: bool,
    samples: VecDeque<(Instant, Pos2)>,
    velocity: Vec2,
}

impl Session {
    fn touch(&self) -> bool {
        self.pointer.is_touch()
    }

    fn press(&self, rect: Rect, pos: Pos2) -> PointerPress {
        press_at(rect, pos, self.clicks, self.modifiers, self.touch())
    }

    fn secondary(&self, pos: Pos2) -> SecondaryDrag {
        SecondaryDrag {
            from: self.origin,
            pos,
            started: false,
            ended: false,
            cancelled: false,
            modifiers: self.modifiers,
        }
    }
}

#[derive(Default)]
struct Fingers {
    held: Vec<(TouchId, Pos2, Pos2)>,
    most: usize,
    moved: bool,
    since: Option<Instant>,
    hold: Option<Hold>,
    span: Option<f32>,
    center: Option<Pos2>,
    pinch: f32,
    pan: Vec2,
    tap: Option<usize>,
}

struct Hold {
    finger: TouchId,
    anchor: Pos2,
    owners: Vec<NodeId>,
    drag: SecondaryDrag,
}

struct Frame<'a> {
    ctx: &'a Context,
    rects: &'a Rects,
    root: NodeId,
    now: Instant,
    long_press: Duration,
}

pub(crate) struct Dispatched {
    pub swallows_escape: bool,
}

pub(crate) fn dispatch(
    doc: &mut Document,
    ctx: &Context,
    rects: &Rects,
    root: NodeId,
    enabled: bool,
) -> Dispatched {
    let frame = Frame {
        ctx,
        rects,
        root,
        now: ctx.now(),
        long_press: ctx.input(|input| input.long_press_delay),
    };
    doc.pointers.routed.clear();
    doc.pointers.fingers.pinch = 1.0;
    doc.pointers.fingers.pan = Vec2::ZERO;
    doc.pointers.fingers.tap = None;
    for session in &mut doc.pointers.sessions {
        session.dragged_this_frame = false;
    }
    let tracked = autoscroll::frame(doc, ctx, enabled);
    if !enabled {
        let pointers: Vec<PointerId> = doc
            .pointers
            .sessions
            .iter()
            .map(|session| session.pointer)
            .collect();
        for pointer in pointers {
            cancel(doc, &frame, pointer);
        }
        hover(doc, &frame, None);
        doc.pointers.wheel_sink = None;
        return Dispatched {
            swallows_escape: tracked.swallows_escape,
        };
    }
    long_presses(doc, &frame);
    for event in ctx.input(|input| input.pointer_events.clone()) {
        if autoscroll::pointer(doc, &event) {
            continue;
        }
        match event.phase {
            PointerPhase::Down(button) => down(doc, &frame, event, button),
            PointerPhase::Move => moved(doc, &frame, event),
            PointerPhase::Up(button) => up(doc, &frame, event, button),
            PointerPhase::Cancel | PointerPhase::Gone => {
                let sink = doc
                    .pointers
                    .sink_of(event.pointer)
                    .or(doc.forward.hovered());
                doc.pointers.routed.push((event.source, sink));
                cancel(doc, &frame, event.pointer);
            }
        }
        ::reactive::settle(|| {});
    }
    frame_drags(doc, &frame);
    pinch(doc, &frame);
    if !tracked.wheel_taken {
        wheel(doc, &frame);
    }
    let mouse = ctx.input(|input| input.pointer.pos.filter(|_| !input.pointer.from_touch()));
    hover(doc, &frame, mouse);
    wheel_sink(doc, &frame, mouse);
    if let Some(fingers) = doc.pointers.fingers.tap
        && doc.overlay_stack.is_empty()
    {
        doc.finger_tap(fingers);
    }
    cursor(doc, &frame);
    track(doc, &frame);
    autoscroll::show_cursor(doc, ctx);
    Dispatched {
        swallows_escape: tracked.swallows_escape,
    }
}

fn node_mut(doc: &mut Document, id: NodeId) -> Option<&mut InteractiveNode> {
    if !doc.arena.contains(id) {
        return None;
    }
    doc.arena
        .get_mut(id)
        .as_any_mut()
        .downcast_mut::<InteractiveNode>()
}

fn rect_of(doc: &Document, rects: &Rects, id: NodeId) -> Option<Rect> {
    interactive(doc, id)?;
    rects.get(&id)
}

fn over(rects: &Rects, id: NodeId, pos: Pos2) -> bool {
    rects
        .visible(&id)
        .is_some_and(|rect| rect.contains_half_open(pos))
}

fn arm(doc: &mut Document, id: NodeId, armed: bool) {
    if let Some((on_active_change, active)) = node_mut(doc, id).and_then(|node| node.arm(armed)) {
        on_active_change.call(active);
    }
}

fn find(doc: &Document, pointer: PointerId, button: Option<PointerButton>) -> Option<usize> {
    doc.pointers.sessions.iter().position(|session| {
        session.pointer == pointer && button.is_none_or(|button| session.button == button)
    })
}

fn down(doc: &mut Document, frame: &Frame, event: PointerEvent, button: PointerButton) {
    if let Some(index) = find(doc, event.pointer, Some(button)) {
        let stale = doc.pointers.sessions.remove(index);
        end(doc, frame, stale, true);
    }
    let touch = event.pointer.is_touch();
    let finger = touch && doc.pointers.sessions.iter().any(|session| session.touch());
    let role = match (touch, finger, button) {
        (true, true, _) => Role::Finger,
        (_, _, PointerButton::Primary) => Role::Press,
        (false, _, PointerButton::Secondary) => Role::Secondary,
        (false, _, PointerButton::Middle) => Role::Middle,
        _ => Role::Other,
    };
    if matches!(role, Role::Press | Role::Secondary) {
        doc.dismiss_light_overlays(event.pos);
        ::reactive::settle(|| {});
    }
    if role == Role::Press {
        doc.pressed_outside(frame.rects, event.pos);
    }
    let hit = hit::hit(
        doc,
        frame.rects,
        frame.root,
        event.pos,
        touch && role == Role::Press,
    );
    let pos = event.pos + hit.shift;
    if matches!(role, Role::Press | Role::Secondary) {
        doc.acted = Acted::Pointer(pos);
    }
    let claimant = match role {
        Role::Press | Role::Secondary => {
            hit::claimant(doc, frame.rects, frame.root, pos, event.modifiers)
        }
        _ => None,
    };
    let captor = match event.pointer {
        PointerId::Mouse => doc.pointers.sink_of(PointerId::Mouse),
        PointerId::Touch(_) => None,
    };
    let sink = captor.or_else(|| match claimant {
        Some(claimant) => interactive(doc, claimant)
            .is_some_and(InteractiveNode::forwards)
            .then_some(claimant),
        None => hit::sink(doc, frame.rects, &hit.path, pos),
    });
    doc.pointers.routed.push((event.source, sink));
    let mut session = Session {
        pointer: event.pointer,
        button,
        role,
        origin: pos,
        pos,
        previous: pos,
        shift: hit.shift,
        path: hit.path.clone(),
        owners: Vec::new(),
        claimed: claimant.is_some(),
        focus: None,
        sink,
        pan: None,
        modifiers: event.modifiers,
        clicks: event.clicks,
        dragged: false,
        axis: None,
        drag_target: None,
        drag_started: false,
        cancelled: false,
        long_press: None,
        last_drag: None,
        dragged_this_frame: false,
        samples: VecDeque::from([(frame.now, pos)]),
        velocity: Vec2::ZERO,
    };
    if let PointerId::Touch(id) = event.pointer {
        finger_down(doc, frame, id, pos);
    }
    match role {
        Role::Press => press(doc, frame, &mut session, claimant, &hit.scrims),
        Role::Secondary => secondary(doc, frame, &mut session, claimant),
        Role::Middle => middle(doc, frame, &mut session, &hit),
        Role::Finger | Role::Other => {}
    }
    doc.pointers.sessions.push(session);
}

fn reached(doc: &Document, rects: &Rects, path: &[NodeId], pos: Pos2) -> (Vec<NodeId>, bool) {
    let mut reached = Vec::new();
    for id in path {
        reached.push(*id);
        let captures = match (interactive(doc, *id), rects.visible(id)) {
            (Some(node), Some(rect)) => node.captures(pos, rect),
            _ => false,
        };
        if captures {
            return (reached, true);
        }
    }
    (reached, false)
}

fn focus_target(doc: &Document, rects: &Rects, reached: &[NodeId], pos: Pos2) -> Option<NodeId> {
    for id in reached {
        let Some(focus) = interactive(doc, *id).and_then(|node| node.focus.as_ref()) else {
            continue;
        };
        if !over(rects, *id, pos) {
            continue;
        }
        return match focus.press_focus {
            true => Some(*id),
            false => doc.focused_node(),
        };
    }
    None
}

fn press(
    doc: &mut Document,
    frame: &Frame,
    session: &mut Session,
    claimant: Option<NodeId>,
    scrims: &[NodeId],
) {
    let pos = session.pos;
    let (reached, captured) = match claimant {
        Some(claimant) => (vec![claimant], true),
        None => reached(doc, frame.rects, &session.path, pos),
    };
    session.owners = reached
        .iter()
        .copied()
        .filter(|id| interactive(doc, *id).is_some_and(InteractiveNode::holds_presses))
        .chain(scrims.iter().copied())
        .collect();
    session.focus = match captured {
        true => doc.focused_node(),
        false => focus_target(doc, frame.rects, &reached, pos),
    };
    let focused = doc.focused_node();
    doc.set_focus_visible(false);
    for owner in session.owners.clone().into_iter().rev() {
        let change = node_mut(doc, owner).and_then(|node| node.arm(true));
        let pressed = rect_of(doc, frame.rects, owner).zip(interactive(doc, owner));
        if let Some((rect, node)) = pressed {
            node.on_press.clone().call(session.press(rect, pos));
        }
        if let Some((on_active_change, active)) = change {
            on_active_change.call(active);
        }
    }
    if session.touch() {
        session.long_press = Some(frame.now + frame.long_press);
    } else if doc.focused_node() == focused {
        doc.press_focus(session.focus);
    }
}

fn drag(doc: &mut Document, frame: &Frame, session: &mut Session, pos: Pos2) {
    session.last_drag = Some(pos);
    session.dragged_this_frame = true;
    for owner in session.owners.clone().into_iter().rev() {
        let Some(rect) = rect_of(doc, frame.rects, owner) else {
            continue;
        };
        if let Some(node) = interactive(doc, owner) {
            node.on_drag.clone().call(session.press(rect, pos));
        }
    }
}

fn secondary(doc: &mut Document, frame: &Frame, session: &mut Session, claimant: Option<NodeId>) {
    let pos = session.pos;
    session.owners = match claimant {
        Some(claimant) => vec![claimant],
        None => session
            .path
            .iter()
            .copied()
            .filter(|id| {
                over(frame.rects, *id, pos)
                    && interactive(doc, *id).is_some_and(InteractiveNode::takes_secondary)
            })
            .collect(),
    };
    for owner in session.owners.clone().into_iter().rev() {
        let Some(rect) = rect_of(doc, frame.rects, owner) else {
            continue;
        };
        if let Some(node) = interactive(doc, owner) {
            node.on_secondary_press
                .clone()
                .call(session.press(rect, pos));
        }
    }
    let drag = SecondaryDrag {
        started: true,
        ..session.secondary(pos)
    };
    secondary_drag(doc, &session.owners, drag);
}

fn secondary_drag(doc: &mut Document, owners: &[NodeId], drag: SecondaryDrag) {
    for owner in owners.iter().rev() {
        if let Some(node) = interactive(doc, *owner) {
            node.on_secondary_drag.clone().call(drag);
        }
    }
}

fn middle(doc: &mut Document, frame: &Frame, session: &mut Session, hit: &Hit) {
    let pos = session.pos;
    let target = match hit.embed {
        true => None,
        false => session
            .path
            .iter()
            .copied()
            .find(|id| interactive(doc, *id).is_some_and(InteractiveNode::claims_middle)),
    };
    if let Some(target) = target
        && interactive(doc, target).is_some_and(InteractiveNode::wants_autoscroll)
    {
        autoscroll::start(doc, target, pos);
        return;
    }
    session.pan =
        target.filter(|id| interactive(doc, *id).is_some_and(|node| !node.on_pan_drag.is_empty()));
    session.owners = session
        .path
        .iter()
        .copied()
        .filter(|id| {
            over(frame.rects, *id, pos)
                && interactive(doc, *id).is_some_and(|node| !node.on_middle_click.is_empty())
        })
        .collect();
    if let Some(pan) = session.pan {
        set_pan(doc, pan, true);
    }
}

fn set_pan(doc: &mut Document, id: NodeId, active: bool) {
    let Some(node) = node_mut(doc, id) else {
        return;
    };
    if node.pan_active == active {
        return;
    }
    node.pan_active = active;
    node.on_pan_active_change.clone().call(active);
}

fn moved(doc: &mut Document, frame: &Frame, event: PointerEvent) {
    if let PointerId::Touch(id) = event.pointer {
        finger_moved(doc, id, event.pos);
    }
    let indices: Vec<usize> = doc
        .pointers
        .sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| session.pointer == event.pointer)
        .map(|(index, _)| index)
        .collect();
    let sink = match indices.is_empty() {
        true => under(doc, frame, event.pos),
        false => doc.pointers.sink_of(event.pointer),
    };
    doc.pointers.routed.push((event.source, sink));
    for index in indices.into_iter().rev() {
        let mut session = doc.pointers.sessions.remove(index);
        let pos = event.pos + session.shift;
        follow(doc, frame, &mut session, pos, false);
        doc.pointers.sessions.insert(index, session);
    }
}

fn under(doc: &mut Document, frame: &Frame, pos: Pos2) -> Option<NodeId> {
    let hit = hit::hit(doc, frame.rects, frame.root, pos, false);
    hit::sink(doc, frame.rects, &hit.path, pos)
}

fn follow(doc: &mut Document, frame: &Frame, session: &mut Session, pos: Pos2, releasing: bool) {
    match session.role {
        Role::Press => {
            if session.touch() {
                track_touch(doc, frame, session, pos);
            }
            session.pos = pos;
            let last = match releasing {
                true => Some(session.last_drag.unwrap_or(session.origin)),
                false => session.last_drag,
            };
            if !session.cancelled && last != Some(pos) {
                drag(doc, frame, session, pos);
            }
        }
        Role::Secondary if releasing => session.pos = pos,
        Role::Secondary => {
            if session.pos != pos {
                session.pos = pos;
                let drag = session.secondary(pos);
                secondary_drag(doc, &session.owners, drag);
            }
        }
        Role::Middle => {
            if let Some(pan) = session.pan
                && pos != session.pos
                && let Some(node) = interactive(doc, pan)
            {
                node.on_pan_drag.clone().call(pos - session.pos);
            }
            session.pos = pos;
        }
        Role::Finger | Role::Other => session.pos = pos,
    }
}

fn track_touch(doc: &mut Document, frame: &Frame, session: &mut Session, pos: Pos2) {
    sample(session, frame.now, pos);
    if session.cancelled {
        return;
    }
    let delta = match session.axis {
        None => {
            let movement = pos - session.origin;
            let distance = movement.x.hypot(movement.y);
            if distance < TOUCH_DRAG_THRESHOLD {
                return;
            }
            session.dragged = true;
            session.long_press = None;
            let axis = match movement.y.abs() >= movement.x.abs() {
                true => Direction::Vertical,
                false => Direction::Horizontal,
            };
            session.axis = Some(axis);
            decide(doc, session, axis);
            along(axis, movement * (1.0 - TOUCH_DRAG_THRESHOLD / distance))
        }
        Some(axis) => along(axis, pos - session.previous),
    };
    session.previous = pos;
    scroll_drag(doc, session, delta, false, false);
}

fn along(axis: Direction, movement: Vec2) -> Vec2 {
    match axis {
        Direction::Vertical => vec2(0.0, movement.y),
        Direction::Horizontal => vec2(movement.x, 0.0),
    }
}

fn decide(doc: &mut Document, session: &mut Session, axis: Direction) {
    if session.claimed {
        session.drag_target = session
            .owners
            .first()
            .copied()
            .filter(|id| interactive(doc, *id).is_some_and(|node| node.catches_drag(axis)));
        return;
    }
    session.drag_target = session
        .path
        .iter()
        .copied()
        .find(|id| interactive(doc, *id).is_some_and(|node| node.catches_drag(axis)));
    if session.drag_target.is_none() {
        return;
    }
    let (kept, lost): (Vec<NodeId>, Vec<NodeId>) = session
        .owners
        .iter()
        .partition(|id| interactive(doc, **id).is_some_and(|node| node.holds_drag(axis)));
    session.owners = kept;
    for owner in lost.into_iter().rev() {
        cancel_owner(doc, owner);
    }
}

fn cancel_owner(doc: &mut Document, owner: NodeId) {
    if let Some(node) = interactive(doc, owner)
        && node.armed
    {
        node.on_cancel.clone().call();
    }
    arm(doc, owner, false);
}

fn scroll_drag(
    doc: &mut Document,
    session: &mut Session,
    delta: Vec2,
    ended: bool,
    cancelled: bool,
) {
    let Some(target) = session.drag_target else {
        return;
    };
    let Some(node) = interactive(doc, target) else {
        return;
    };
    let started = !std::mem::replace(&mut session.drag_started, true);
    node.on_scroll_drag.clone().call(DragGesture {
        started,
        ended,
        cancelled,
        delta,
        velocity: session.velocity,
    });
}

fn sample(session: &mut Session, now: Instant, pos: Pos2) {
    session.samples.push_back((now, pos));
    while session.samples.len() > 2
        && session.samples.front().is_some_and(|(when, _)| {
            now.duration_since(*when).as_secs_f32() > TOUCH_VELOCITY_WINDOW
        })
    {
        session.samples.pop_front();
    }
    let Some((then, origin)) = session.samples.front().copied() else {
        return;
    };
    let elapsed = now.duration_since(then).as_secs_f32();
    if elapsed <= f32::EPSILON {
        return;
    }
    let measured = (pos - origin) * elapsed.recip();
    let speed = measured.x.hypot(measured.y);
    session.velocity = match speed > MAX_TOUCH_VELOCITY {
        true => measured * (MAX_TOUCH_VELOCITY / speed),
        false => measured,
    };
}

fn up(doc: &mut Document, frame: &Frame, event: PointerEvent, button: PointerButton) {
    if let PointerId::Touch(id) = event.pointer {
        finger_lifted(doc, id, Some(event.pos), false);
    }
    let Some(index) = find(doc, event.pointer, Some(button)) else {
        let sink = match event.pointer {
            PointerId::Mouse => doc.pointers.sink_of(PointerId::Mouse),
            PointerId::Touch(_) => None,
        };
        doc.pointers.routed.push((event.source, sink));
        return;
    };
    let mut session = doc.pointers.sessions.remove(index);
    let sink = match event.pointer {
        PointerId::Mouse => doc.pointers.sink_of(PointerId::Mouse).or(session.sink),
        PointerId::Touch(_) => session.sink,
    };
    doc.pointers.routed.push((event.source, sink));
    let pos = event.pos + session.shift;
    follow(doc, frame, &mut session, pos, true);
    end(doc, frame, session, false);
}

fn end(doc: &mut Document, frame: &Frame, mut session: Session, cancelled: bool) {
    let pos = session.pos;
    let cancelled = cancelled || session.cancelled;
    match session.role {
        Role::Press => {
            let focused = doc.focused_node();
            let owners = std::mem::take(&mut session.owners);
            if !cancelled {
                for owner in owners.iter().rev() {
                    let Some(rect) = rect_of(doc, frame.rects, *owner) else {
                        continue;
                    };
                    let clicked = match session.touch() {
                        true => !session.dragged,
                        false => over(frame.rects, *owner, pos),
                    };
                    if !clicked {
                        continue;
                    }
                    let Some(node) = interactive(doc, *owner) else {
                        continue;
                    };
                    let (on_click, on_click_at) = (node.on_click.clone(), node.on_click_at.clone());
                    on_click.call();
                    on_click_at.call(session.press(rect, pos));
                }
            }
            for owner in owners.iter().rev() {
                match cancelled {
                    true => cancel_owner(doc, *owner),
                    false => arm(doc, *owner, false),
                }
            }
            scroll_drag(doc, &mut session, Vec2::ZERO, !cancelled, cancelled);
            if session.touch() && !session.dragged && !cancelled && doc.focused_node() == focused {
                doc.press_focus(session.focus);
            }
            if doc.dragging() {
                match cancelled {
                    true => doc.drag_board().end(None),
                    false => doc.drag_board().finish(),
                }
            }
        }
        Role::Secondary => {
            let drag = SecondaryDrag {
                ended: !cancelled,
                cancelled,
                ..session.secondary(pos)
            };
            secondary_drag(doc, &session.owners, drag);
        }
        Role::Middle => {
            if let Some(pan) = session.pan {
                let pinched = doc.pointers.pinch_target == Some(pan);
                set_pan(doc, pan, pinched);
            }
            if !cancelled {
                for owner in session.owners.iter().rev() {
                    if !over(frame.rects, *owner, pos) {
                        continue;
                    }
                    if let Some(node) = interactive(doc, *owner) {
                        node.on_middle_click.clone().call();
                    }
                }
            }
        }
        Role::Finger | Role::Other => {}
    }
}

fn cancel(doc: &mut Document, frame: &Frame, pointer: PointerId) {
    if let PointerId::Touch(id) = pointer {
        finger_lifted(doc, id, None, true);
    }
    while let Some(index) = find(doc, pointer, None) {
        let session = doc.pointers.sessions.remove(index);
        end(doc, frame, session, true);
    }
}

fn long_presses(doc: &mut Document, frame: &Frame) {
    let mut fired = Vec::new();
    for (index, session) in doc.pointers.sessions.iter_mut().enumerate() {
        let Some(due) = session.long_press else {
            continue;
        };
        if session.cancelled {
            session.long_press = None;
            continue;
        }
        if frame.now < due {
            frame.ctx.request_repaint_after(due - frame.now);
            continue;
        }
        session.long_press = None;
        session.cancelled = true;
        fired.push(index);
    }
    for index in fired {
        let owners = std::mem::take(&mut doc.pointers.sessions[index].owners);
        for owner in owners.into_iter().rev() {
            cancel_owner(doc, owner);
        }
        let session = &doc.pointers.sessions[index];
        let (pos, clicks, modifiers) = (session.pos, session.clicks, session.modifiers);
        doc.dismiss_light_overlays(pos);
        ::reactive::settle(|| {});
        doc.acted = Acted::Pointer(pos);
        let hit = hit::hit(doc, frame.rects, frame.root, pos, false);
        for id in hit.path.iter().rev() {
            if !over(frame.rects, *id, pos) {
                continue;
            }
            let (Some(node), Some(rect)) = (interactive(doc, *id), frame.rects.get(id)) else {
                continue;
            };
            node.on_secondary_press
                .clone()
                .call(press_at(rect, pos, clicks, modifiers, true));
        }
        ::reactive::settle(|| {});
    }
}

fn primary_touch(doc: &Document) -> Option<usize> {
    doc.pointers
        .sessions
        .iter()
        .position(|session| session.touch() && session.role == Role::Press)
}

fn finger_down(doc: &mut Document, frame: &Frame, id: TouchId, pos: Pos2) {
    let fingers = &mut doc.pointers.fingers;
    if fingers.held.is_empty() {
        fingers.most = 0;
        fingers.moved = false;
        fingers.since = Some(frame.now);
    }
    fingers.held.push((id, pos, pos));
    fingers.most = fingers.most.max(fingers.held.len());
    measure(fingers);
    let Some(primary) = primary_touch(doc) else {
        return;
    };
    let mut session = doc.pointers.sessions.remove(primary);
    let holds = !session.dragged
        && doc.pointers.fingers.hold.is_none()
        && doc.pointers.fingers.held.len() == 2;
    if holds {
        let hit = hit::hit(doc, frame.rects, frame.root, pos, false);
        let owners: Vec<NodeId> = hit
            .path
            .iter()
            .copied()
            .filter(|node| {
                over(frame.rects, *node, pos)
                    && interactive(doc, *node)
                        .is_some_and(|catcher| !catcher.on_secondary_drag.is_empty())
            })
            .collect();
        let drag = SecondaryDrag {
            from: pos,
            pos,
            started: true,
            ended: false,
            cancelled: false,
            modifiers: Modifiers::NONE,
        };
        secondary_drag(doc, &owners, drag);
        doc.pointers.fingers.hold = Some(Hold {
            finger: id,
            anchor: session.pos,
            owners,
            drag,
        });
    }
    let owners = std::mem::take(&mut session.owners);
    for owner in owners.into_iter().rev() {
        cancel_owner(doc, owner);
    }
    if !session.cancelled {
        scroll_drag(doc, &mut session, Vec2::ZERO, false, true);
    }
    session.cancelled = true;
    session.dragged = true;
    session.long_press = None;
    session.drag_target = None;
    doc.pointers.sessions.insert(primary, session);
}

fn finger_moved(doc: &mut Document, id: TouchId, pos: Pos2) {
    let primary = primary_touch(doc).map(|index| doc.pointers.sessions[index].pointer);
    let fingers = &mut doc.pointers.fingers;
    let Some(held) = fingers.held.iter_mut().find(|(finger, _, _)| *finger == id) else {
        return;
    };
    held.1 = pos;
    if held.2.distance(pos) >= TOUCH_DRAG_THRESHOLD {
        fingers.moved = true;
    }
    let mut moved_hold = None;
    let mut dropped_hold = None;
    if let Some(hold) = &mut fingers.hold {
        if hold.finger == id {
            if hold.drag.pos != pos {
                hold.drag = SecondaryDrag {
                    pos,
                    started: false,
                    ..hold.drag
                };
                moved_hold = Some((hold.owners.clone(), hold.drag));
            }
        } else if primary == Some(PointerId::Touch(id))
            && pos.distance(hold.anchor) >= TOUCH_DRAG_THRESHOLD
        {
            dropped_hold = fingers.hold.take();
        }
    }
    measure(&mut doc.pointers.fingers);
    if let Some((owners, drag)) = moved_hold {
        secondary_drag(doc, &owners, drag);
    }
    if let Some(hold) = dropped_hold {
        let drag = SecondaryDrag {
            cancelled: true,
            started: false,
            ..hold.drag
        };
        secondary_drag(doc, &hold.owners, drag);
    }
}

fn finger_lifted(doc: &mut Document, id: TouchId, pos: Option<Pos2>, cancelled: bool) {
    let now = doc.now();
    let primary = primary_touch(doc).map(|index| doc.pointers.sessions[index].pointer);
    let fingers = &mut doc.pointers.fingers;
    let Some(index) = fingers.held.iter().position(|(finger, _, _)| *finger == id) else {
        return;
    };
    fingers.held.remove(index);
    if fingers.held.is_empty() {
        let quick = fingers
            .since
            .take()
            .is_some_and(|since| now.saturating_duration_since(since) <= FINGER_TAP_TIME);
        if !cancelled && quick && !fingers.moved && fingers.most >= 2 {
            fingers.tap = Some(fingers.most);
        }
    }
    fingers.span = None;
    fingers.center = None;
    let lifted = fingers.hold.as_ref().is_some_and(|hold| hold.finger == id);
    if !lifted && primary != Some(PointerId::Touch(id)) {
        return;
    }
    let Some(hold) = fingers.hold.take() else {
        return;
    };
    let ended = lifted && !cancelled;
    let drag = SecondaryDrag {
        pos: pos.filter(|_| lifted).unwrap_or(hold.drag.pos),
        started: false,
        ended,
        cancelled: !ended,
        ..hold.drag
    };
    secondary_drag(doc, &hold.owners, drag);
}

fn measure(fingers: &mut Fingers) {
    let (Some(first), Some(second), None) = (
        fingers.held.first(),
        fingers.held.get(1),
        fingers.held.get(2),
    ) else {
        fingers.span = None;
        fingers.center = None;
        return;
    };
    let (first, second) = (first.1, second.1);
    let span = first.distance(second);
    let center = Pos2::new((first.x + second.x) * 0.5, (first.y + second.y) * 0.5);
    if let (Some(previous_span), Some(previous_center)) = (fingers.span, fingers.center)
        && fingers.hold.is_none()
        && previous_span > 0.0
        && span > 0.0
    {
        fingers.pinch *= span / previous_span;
        fingers.pan += center - previous_center;
    }
    fingers.span = Some(span);
    fingers.center = Some(center);
}

fn pinch(doc: &mut Document, frame: &Frame) {
    let zoom = frame.ctx.input(|input| input.zoom_factor) * doc.pointers.fingers.pinch;
    let pan = doc.pointers.fingers.pan;
    let fingers = doc.pointers.fingers.held.len();
    if fingers < 2
        && let Some(target) = doc.pointers.pinch_target.take()
    {
        let panning = doc
            .pointers
            .sessions
            .iter()
            .any(|session| session.pan == Some(target));
        set_pan(doc, target, panning);
    }
    if zoom == 1.0 && pan == Vec2::ZERO {
        return;
    }
    let Some(pos) = doc
        .pointers
        .fingers
        .center
        .or_else(|| frame.ctx.input(|input| input.pointer.pos))
    else {
        return;
    };
    let target = match doc.pointers.pinch_target {
        Some(target) => Some(target),
        None => {
            let hit = hit::hit(doc, frame.rects, frame.root, pos, false);
            hit.path
                .into_iter()
                .find(|id| interactive(doc, *id).is_some_and(InteractiveNode::wants_gestures))
        }
    };
    let Some(target) = target else {
        return;
    };
    if fingers >= 2 {
        doc.pointers.pinch_target = Some(target);
        set_pan(doc, target, true);
    }
    let Some(node) = interactive(doc, target) else {
        return;
    };
    let (on_zoom, on_pan_drag) = (node.on_zoom.clone(), node.on_pan_drag.clone());
    if zoom != 1.0 {
        on_zoom.call(ZoomGesture { factor: zoom, pos });
    }
    if pan != Vec2::ZERO {
        on_pan_drag.call(pan);
    }
}

fn wheel(doc: &mut Document, frame: &Frame) {
    let modifiers = frame.ctx.input(|input| input.modifiers);
    let (wheel, fling) = frame
        .ctx
        .input(|input| (input.scroll_delta, input.scroll_fling));
    let (wheel, fling) = match modifiers.shift && wheel.x == 0.0 && fling.x == 0.0 {
        true => (vec2(wheel.y, 0.0), vec2(fling.y, 0.0)),
        false => (wheel, fling),
    };
    let along = match wheel {
        Vec2::ZERO => fling,
        wheel => wheel,
    };
    if along == Vec2::ZERO {
        return;
    }
    let pointer = frame.ctx.input(|input| input.pointer.pos);
    let target = latched(doc, frame, pointer, along).or_else(|| {
        let pos = pointer?;
        let hit = hit::hit(doc, frame.rects, frame.root, pos, false);
        for id in hit.path {
            let node = interactive(doc, id)?;
            if node.wants_wheel(along) {
                return Some(id);
            }
            if node.forwards() {
                return None;
            }
        }
        None
    });
    doc.pointers.wheel_latch = target.map(|target| (target, frame.now, pointer));
    let (Some(target), Some(pos)) = (target, pointer) else {
        return;
    };
    let Some(node) = interactive(doc, target) else {
        return;
    };
    node.on_scroll.clone().call(ScrollGesture {
        delta: wheel,
        fling,
        pos,
        modifiers,
    });
}

fn latched(doc: &Document, frame: &Frame, pointer: Option<Pos2>, wheel: Vec2) -> Option<NodeId> {
    let (latched, last, at) = doc.pointers.wheel_latch?;
    let recent = frame.now.saturating_duration_since(last) < WHEEL_LATCH_TIMEOUT;
    let still = match (at, pointer) {
        (Some(at), Some(pos)) => at.distance(pos) <= WHEEL_LATCH_SLOP,
        (at, pos) => at == pos,
    };
    let under = pointer.is_some_and(|pos| over(frame.rects, latched, pos));
    let wants = interactive(doc, latched).is_some_and(|node| node.wants_wheel(wheel));
    (recent && still && under && wants).then_some(latched)
}

fn wheel_sink(doc: &mut Document, frame: &Frame, pointer: Option<Pos2>) {
    let path = match pointer {
        Some(pos) => hit::hit(doc, frame.rects, frame.root, pos, false).path,
        None => Vec::new(),
    };
    doc.pointers.wheel_sink = path
        .into_iter()
        .find_map(|id| {
            let node = interactive(doc, id)?;
            match (node.forwards(), node.on_scroll.is_empty()) {
                (true, _) => Some(Some(id)),
                (false, false) => Some(None),
                (false, true) => None,
            }
        })
        .flatten();
}

fn frame_drags(doc: &mut Document, frame: &Frame) {
    for index in 0..doc.pointers.sessions.len() {
        let session = &doc.pointers.sessions[index];
        if session.role != Role::Press || session.cancelled || session.dragged_this_frame {
            continue;
        }
        if session.last_drag.is_none() {
            let mut session = doc.pointers.sessions.remove(index);
            let pos = session.pos;
            drag(doc, frame, &mut session, pos);
            doc.pointers.sessions.insert(index, session);
            continue;
        }
        let pos = session.pos;
        let repeats: Vec<NodeId> = session
            .owners
            .iter()
            .copied()
            .filter(|id| interactive(doc, *id).is_some_and(|node| node.repeat_drag))
            .collect();
        if repeats.is_empty() {
            continue;
        }
        frame.ctx.request_repaint();
        let session = &doc.pointers.sessions[index];
        let presses: Vec<(NodeId, PointerPress)> = repeats
            .iter()
            .rev()
            .filter_map(|id| Some((*id, session.press(rect_of(doc, frame.rects, *id)?, pos))))
            .collect();
        for (id, press) in presses {
            if let Some(node) = interactive(doc, id) {
                node.on_drag.clone().call(press);
            }
        }
    }
}

fn hover(doc: &mut Document, frame: &Frame, pointer: Option<Pos2>) {
    let path: Vec<NodeId> = match pointer {
        Some(pos) => hit::hit(doc, frame.rects, frame.root, pos, false)
            .path
            .into_iter()
            .filter(|id| over(frame.rects, *id, pos))
            .collect(),
        None => Vec::new(),
    };
    let left: Vec<NodeId> = doc
        .pointers
        .hovered
        .iter()
        .copied()
        .filter(|id| !path.contains(id))
        .collect();
    for id in left {
        let Some(node) = node_mut(doc, id) else {
            continue;
        };
        node.hover_pos = None;
        if node.hovered {
            node.hovered = false;
            node.on_hover_change.clone().call(false);
        }
    }
    if let Some(pos) = pointer {
        for id in path.iter().rev() {
            let Some(rect) = frame.rects.get(id) else {
                continue;
            };
            let Some(node) = node_mut(doc, *id) else {
                continue;
            };
            if !node.hovered {
                node.hovered = true;
                node.on_hover_change.clone().call(true);
            }
            let Some(node) = node_mut(doc, *id) else {
                continue;
            };
            if node.hover_pos != Some(pos) {
                node.hover_pos = Some(pos);
                let press = press_at(
                    rect,
                    pos,
                    0,
                    frame.ctx.input(|input| input.modifiers),
                    false,
                );
                node.on_hover_move.clone().call(press);
            }
        }
    }
    doc.pointers.hovered = path;
}

fn cursor(doc: &Document, frame: &Frame) {
    let held = doc
        .pointers
        .sessions
        .iter()
        .filter(|session| session.pointer == PointerId::Mouse && session.role == Role::Press)
        .flat_map(|session| session.owners.iter())
        .chain(doc.pointers.hovered.iter())
        .find_map(|id| interactive(doc, *id).and_then(|node| node.cursor));
    if let Some(cursor) = held {
        frame.ctx.set_cursor_icon(cursor);
    }
}

fn track(doc: &mut Document, frame: &Frame) {
    let shift = doc
        .pointers
        .sessions
        .iter()
        .find(|session| session.touch() && session.role == Role::Press)
        .map_or(Vec2::ZERO, |session| session.shift);
    doc.pointers.shift = shift;
    let (pointer, touch, modifiers) = frame.ctx.input(|input| {
        (
            input.pointer.pos,
            input.pointer.from_touch(),
            input.modifiers,
        )
    });
    let Some(pos) = pointer.map(|pos| pos + shift) else {
        return;
    };
    doc.last_pointer = Some(PointerSample { pos, touch });
    doc.drag_board()
        .track(crate::drag_board::DragPoint { pos, modifiers });
}
