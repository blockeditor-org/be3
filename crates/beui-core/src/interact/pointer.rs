use std::time::{Duration, Instant};

use crate::base::interactive::{InteractiveNode, local_in, press_at};
use crate::base::list::Direction;
use crate::base::overlay::OverlayNode;
use crate::context::Context;
use crate::document::{Acted, Document};
use crate::geometry::{Pos2, Rect, Vec2, vec2};
use crate::input::{
    FINGER_TAP_TIME, Modifiers, Pen, PointerButton, PointerEvent, PointerId, PointerInput,
    PointerPhase, PointerPress, PointerSample, PointerStage, ScrollGesture, TOUCH_DRAG_THRESHOLD,
    TouchId, WheelInput, ZoomGesture,
};
use crate::node::{NodeId, NodeOf, Rects};
use crate::window::WindowId;

use super::hit::{self, Hit, interactive};
use super::{WHEEL_LATCH_SLOP, WHEEL_LATCH_TIMEOUT, grab};

#[derive(Default)]
pub struct Pointers {
    sessions: Vec<Session>,
    hovered: Vec<NodeId>,
    hovered_at: Option<(WindowId, Pos2)>,
    wheel_latch: Option<(NodeId, Instant, WindowId, Pos2)>,
    wheel_held: bool,
    fingers: Fingers,
    kept_off: Vec<TouchId>,
    pub(crate) grabber: Option<NodeId>,
    pub(crate) grabbed: Option<NodeId>,
    after_input: Vec<Box<dyn FnOnce()>>,
    hits: Vec<(WindowId, Pos2, bool, Hit)>,
    current: usize,
    pub(crate) routed: Vec<Routed>,
    pub(crate) shift: Vec2,
    pub(crate) space: bool,
}

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Routed {
    pub sink: Option<NodeId>,
    pub over: Option<NodeId>,
}

impl Pointers {
    pub(crate) fn sink_of(&self, pointer: PointerId) -> Option<NodeId> {
        self.sessions
            .iter()
            .filter(|session| session.pointer == pointer)
            .find_map(|session| session.sink)
    }

    pub(crate) fn forget_hits(&mut self) {
        self.hits.clear();
    }

    fn route(&mut self, sink: Option<NodeId>) {
        if let Some(routed) = self.routed.get_mut(self.current) {
            routed.sink = sink;
        }
    }

    fn touches(&self) -> impl Iterator<Item = (TouchId, Pos2, Pos2)> + '_ {
        self.sessions
            .iter()
            .filter_map(|session| match session.pointer {
                PointerId::Touch(id) => Some((
                    id,
                    session.pos - session.shift,
                    session.origin - session.shift,
                )),
                PointerId::Mouse | PointerId::Pen => None,
            })
    }
}

impl Document {
    pub fn after_input(&mut self, after: impl FnOnce() + 'static) {
        self.pointers.after_input.push(Box::new(after));
    }
}

pub(crate) fn hit(
    doc: &mut Document,
    rects: &Rects,
    window: WindowId,
    pos: Pos2,
    touch: bool,
) -> Hit {
    let cached = doc
        .pointers
        .hits
        .iter()
        .find(|(held, at, reach, _)| *held == window && *at == pos && *reach == touch);
    if let Some((_, _, _, hit)) = cached {
        return hit.clone();
    }
    let found = hit::hit(doc, rects, window, pos, touch);
    doc.pointers.hits.push((window, pos, touch, found.clone()));
    found
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Kind {
    finger: bool,
    reaches: bool,
    settles: bool,
    holds_sink: bool,
    buttons: bool,
    shows_cursor: bool,
}

impl Kind {
    const MOUSE: Kind = Kind {
        finger: false,
        reaches: false,
        settles: false,
        holds_sink: true,
        buttons: true,
        shows_cursor: true,
    };

    const FINGER: Kind = Kind {
        finger: true,
        reaches: true,
        settles: true,
        holds_sink: false,
        buttons: false,
        shows_cursor: false,
    };

    fn of(pointer: PointerId) -> Kind {
        match pointer {
            PointerId::Mouse | PointerId::Pen => Kind::MOUSE,
            PointerId::Touch(_) => Kind::FINGER,
        }
    }

    fn held_sink(self, doc: &Document, pointer: PointerId) -> Option<NodeId> {
        match self.holds_sink {
            true => doc.pointers.sink_of(pointer),
            false => None,
        }
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
    window: WindowId,
    origin_window: WindowId,
    kind: Kind,
    button: PointerButton,
    role: Role,
    origin: Pos2,
    pos: Pos2,
    shift: Vec2,
    path: Vec<NodeId>,
    owners: Vec<NodeId>,
    listeners: Vec<NodeId>,
    claimed: bool,
    focus: Option<NodeId>,
    pressed_focus: Option<NodeId>,
    sink: Option<NodeId>,
    modifiers: Modifiers,
    clicks: u32,
    pen: Option<Pen>,
    predicted: Option<Pos2>,
    time: Instant,
    dragged: bool,
    axis: Option<Direction>,
    drag_target: Option<NodeId>,
    cancelled: bool,
    long_press: Option<Instant>,
    last_drag: Option<Pos2>,
    dragged_this_frame: bool,
    outside: Option<Vec<NodeOf<OverlayNode>>>,
}

impl Session {
    fn press(&self, (rect, scale): (Rect, f32), pos: Pos2) -> PointerPress {
        PointerPress {
            window: self.window,
            pen: self.pen,
            predicted: self.predicted,
            ..press_at(
                rect,
                scale,
                pos,
                self.clicks,
                self.modifiers,
                self.kind.finger,
                self.time,
            )
        }
    }

    fn input(&self, rects: &Rects, id: NodeId, stage: PointerStage, pos: Pos2) -> PointerInput {
        let button = match self.role {
            Role::Middle => PointerButton::Middle,
            _ => self.button,
        };
        input_at(
            rects,
            id,
            self.pointer,
            Some(button),
            stage,
            pos - self.shift,
            self.modifiers,
            self.time,
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn input_at(
    rects: &Rects,
    id: NodeId,
    pointer: PointerId,
    button: Option<PointerButton>,
    stage: PointerStage,
    pos: Pos2,
    modifiers: Modifiers,
    time: Instant,
) -> PointerInput {
    let (local, size, scale) = rects
        .scaled(&id)
        .map_or((pos, Vec2::ZERO, 1.0), |(rect, scale)| {
            (local_in(rect, scale, pos), rect.size() / scale, scale)
        });
    PointerInput {
        pointer,
        button,
        stage,
        pos,
        local,
        size,
        scale,
        inside: over(rects, id, pos),
        modifiers,
        time,
        settles: None,
        unclaimed: false,
    }
}

fn hear(doc: &Document, id: NodeId, input: PointerInput) -> bool {
    if doc.pointers.grabbed == Some(id) {
        return false;
    }
    let Some(on_pointer) = interactive(doc, id).map(|node| node.on_pointer) else {
        return false;
    };
    !on_pointer.is_empty() && on_pointer.call(input)
}

fn listeners(doc: &Document, path: &[NodeId]) -> Vec<NodeId> {
    path.iter()
        .copied()
        .filter(|id| interactive(doc, *id).is_some_and(InteractiveNode::hears_pointers))
        .collect()
}

fn tell(doc: &Document, rects: &Rects, session: &Session, stage: PointerStage, pos: Pos2) {
    for id in session.listeners.clone() {
        hear(doc, id, session.input(rects, id, stage, pos));
    }
}

#[derive(Default)]
struct Fingers {
    most: usize,
    moved: bool,
    since: Option<Instant>,
    hold: Option<Hold>,
    tap: Option<usize>,
}

struct Hold {
    finger: TouchId,
    anchor: Pos2,
    listeners: Vec<NodeId>,
    pos: Pos2,
}

pub(crate) struct Frame<'a> {
    ctx: &'a Context,
    rects: &'a Rects,
    now: Instant,
    long_press: Duration,
}

impl<'a> Frame<'a> {
    pub(crate) fn new(ctx: &'a Context, rects: &'a Rects) -> Self {
        Self {
            ctx,
            rects,
            now: ctx.now(),
            long_press: ctx.input(|input| input.long_press_delay),
        }
    }
}

pub(crate) struct Down {
    event: PointerEvent,
    button: PointerButton,
    role: Role,
    hit: Hit,
}

pub(crate) struct LongPressed {
    pos: Pos2,
    clicks: u32,
    modifiers: Modifiers,
    hit: Hit,
}

pub(crate) fn begin(doc: &mut Document, frame: &Frame) -> Vec<LongPressed> {
    let count = frame.ctx.input(|input| input.pointer_events.len());
    doc.pointers.routed = vec![Routed::default(); count];
    doc.pointers.hits.clear();
    doc.pointers.fingers.tap = None;
    for session in &mut doc.pointers.sessions {
        session.dragged_this_frame = false;
    }
    let (left, window, pos) = frame.ctx.input(|input| {
        (
            input.pointer.outside,
            input.pointer.window,
            input.pointer.pos,
        )
    });
    let within = |pos: Pos2| {
        frame
            .ctx
            .window_size(window)
            .is_some_and(|size| Rect::from_min_size(Pos2::ZERO, size).contains(pos))
    };
    let outside = left && frame.ctx.opens_windows() && !pos.is_some_and(within);
    doc.drag_board().set_outside(outside);
    grab::frame(doc, frame.ctx);
    long_presses(doc, frame)
}

pub(crate) fn event(
    doc: &mut Document,
    frame: &Frame,
    index: usize,
    event: PointerEvent,
) -> Option<Down> {
    doc.pointers.current = index;
    let down = pointer_event(doc, frame, event);
    if down.is_none() {
        route_over(doc, frame, index, event);
    }
    down
}

pub(crate) fn finish_down(doc: &mut Document, frame: &Frame, index: usize, down: Down) {
    doc.pointers.current = index;
    let event = down.event;
    press_down(doc, frame, down);
    route_over(doc, frame, index, event);
}

fn route_over(doc: &mut Document, frame: &Frame, index: usize, event: PointerEvent) {
    if doc.pointers.routed[index].sink.is_some() {
        let hit = hit(doc, frame.rects, event.window, event.pos, false);
        doc.pointers.routed[index].over = hit::sink(doc, frame.rects, &hit.path, event.pos);
    }
}

pub(crate) fn end_frame(doc: &mut Document, frame: &Frame) {
    doc.pointers.hits.clear();
    frame_drags(doc, frame);
    for after in std::mem::take(&mut doc.pointers.after_input) {
        after();
    }
    let mouse = frame
        .ctx
        .input(|input| input.pointer.mouse().map(|pos| (input.pointer.window, pos)));
    hover(doc, frame, mouse);
    if let Some(fingers) = doc.pointers.fingers.tap
        && doc.layers().blocking() == 0
    {
        doc.finger_tap(fingers);
    }
    cursor(doc, frame);
    track(doc, frame);
}

fn pointer_event(doc: &mut Document, frame: &Frame, event: PointerEvent) -> Option<Down> {
    doc.pointers.hits.clear();
    doc.pointers.grabbed = None;
    let held = doc.pointers.wheel_held;
    if grab::pointer(doc, frame.rects, &event, held) {
        return None;
    }
    if kept_off(doc, frame, &event) {
        doc.pointers.route(None);
        return None;
    }
    match event.phase {
        PointerPhase::Down(button) => return down(doc, frame, event, button),
        PointerPhase::Move => moved(doc, frame, event),
        PointerPhase::Up(button) => up(doc, frame, event, button),
        PointerPhase::Cancel | PointerPhase::Gone => {
            let sink = doc
                .pointers
                .sink_of(event.pointer)
                .or(doc.forward.hovered());
            doc.pointers.route(sink);
            cancel(doc, frame, event.pointer);
        }
        PointerPhase::ScrollStarted => {
            doc.pointers.wheel_held = true;
            wheel(doc, frame, event, Vec2::ZERO, Vec2::ZERO);
            catch_wheel(doc, frame, event);
        }
        PointerPhase::Scroll(delta) => wheel(doc, frame, event, delta, Vec2::ZERO),
        PointerPhase::ScrollEnded { fling } => {
            let held = std::mem::take(&mut doc.pointers.wheel_held);
            wheel(doc, frame, event, Vec2::ZERO, fling);
            if held && fling == Vec2::ZERO {
                lift_latched(doc, frame, event);
            }
        }
        PointerPhase::Zoom(factor) => {
            let path = hit(doc, frame.rects, event.window, event.pos, false).path;
            let sink = match gesture_claimant(doc, frame.rects, &path, event.pos) {
                Some(_) => None,
                None => wheel_sink(doc, frame, event.window, event.pos),
            };
            doc.pointers.route(sink);
            let candidates = match gesture_claimant(doc, frame.rects, &path, event.pos) {
                Some(claimant) => vec![claimant],
                None => path,
            };
            for id in candidates {
                let (local, size) = local(frame.rects, id, event.pos);
                let zoom = WheelInput::Zoom(ZoomGesture {
                    factor,
                    pos: event.pos,
                    local,
                    size,
                });
                if turn(doc, id, zoom) {
                    break;
                }
            }
        }
    }
    None
}

fn kept_off(doc: &mut Document, frame: &Frame, event: &PointerEvent) -> bool {
    let PointerId::Touch(id) = event.pointer else {
        return false;
    };
    let held = doc.pointers.kept_off.contains(&id);
    match event.phase {
        PointerPhase::Down(_) => {
            let off = !frame.ctx.preferences().canvas_touch.reaches()
                && find(doc, event.pointer, None).is_none()
                && on_drawing(doc, frame, event.window, event.pos);
            if off && !held {
                doc.pointers.kept_off.push(id);
            }
            off
        }
        PointerPhase::Up(_) | PointerPhase::Cancel | PointerPhase::Gone => {
            doc.pointers.kept_off.retain(|kept| *kept != id);
            held
        }
        _ => held,
    }
}

fn on_drawing(doc: &mut Document, frame: &Frame, window: WindowId, pos: Pos2) -> bool {
    let hit = hit(doc, frame.rects, window, pos, true);
    let at = pos + hit.shift;
    hit.path.iter().any(|id| {
        interactive(doc, *id)
            .zip(frame.rects.visible(id))
            .is_some_and(|_| hit::draws_at(doc, frame.rects, *id, at))
    })
}

fn node_mut(doc: &mut Document, id: NodeId) -> Option<&mut InteractiveNode> {
    if !doc.arena.contains(id) {
        return None;
    }
    doc.arena
        .state_mut(id)
        .as_any_mut()
        .downcast_mut::<InteractiveNode>()
}

fn rect_of(doc: &Document, rects: &Rects, id: NodeId) -> Option<(Rect, f32)> {
    interactive(doc, id)?;
    rects.scaled(&id)
}

fn turn(doc: &Document, id: NodeId, wheel: WheelInput) -> bool {
    if doc.pointers.grabbed == Some(id) {
        return false;
    }
    let Some(on_wheel) = interactive(doc, id).map(|node| node.on_wheel) else {
        return false;
    };
    !on_wheel.is_empty() && on_wheel.call(wheel)
}

pub(crate) fn local(rects: &Rects, id: NodeId, pos: Pos2) -> (Pos2, Vec2) {
    rects
        .scaled(&id)
        .map_or((pos, Vec2::ZERO), |(rect, scale)| {
            (local_in(rect, scale, pos), rect.size() / scale)
        })
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

fn down(
    doc: &mut Document,
    frame: &Frame,
    event: PointerEvent,
    button: PointerButton,
) -> Option<Down> {
    if let Some(index) = find(doc, event.pointer, Some(button)) {
        let stale = doc.pointers.sessions.remove(index);
        end(doc, frame, stale, true);
    }
    doc.set_touched(event.pointer != PointerId::Mouse);
    doc.finger_pressed = event.pointer.is_touch();
    let kind = Kind::of(event.pointer);
    let role = match button {
        _ if kind.finger && primary_touch(doc).is_some() => Role::Finger,
        PointerButton::Primary => Role::Press,
        PointerButton::Secondary if kind.buttons => Role::Secondary,
        PointerButton::Middle if kind.buttons => Role::Middle,
        _ => Role::Other,
    };
    let hit = hit(
        doc,
        frame.rects,
        event.window,
        event.pos,
        kind.reaches && role == Role::Press,
    );
    let role = match role {
        Role::Press
            if kind.buttons
                && doc.pointers.space
                && gesture_claimant(doc, frame.rects, &hit.path, event.pos).is_some() =>
        {
            Role::Middle
        }
        role => role,
    };
    let down = Down {
        event,
        button,
        role,
        hit,
    };
    if !matches!(role, Role::Press | Role::Secondary) {
        press_down(doc, frame, down);
        return None;
    }
    doc.acted = Acted::Pointer(event.pos + down.hit.shift);
    if kind.settles {
        press_down(doc, frame, down);
        return None;
    }
    doc.pressed_outside(
        frame.rects,
        &down.hit.outside,
        event.window,
        event.pos,
        role == Role::Press,
    );
    Some(down)
}

fn press_down(doc: &mut Document, frame: &Frame, down: Down) {
    let Down {
        event,
        button,
        role,
        hit,
    } = down;
    let kind = Kind::of(event.pointer);
    let pos = event.pos + hit.shift;
    let claimant = match role {
        Role::Press | Role::Secondary => {
            hit::claimant(doc, frame.rects, event.window, pos, event.modifiers)
        }
        _ => None,
    };
    let panned = match role {
        Role::Middle => gesture_claimant(doc, frame.rects, &hit.path, pos),
        _ => None,
    };
    let sink = kind
        .held_sink(doc, event.pointer)
        .filter(|_| panned.is_none())
        .or_else(|| match claimant {
            Some(claimant) => interactive(doc, claimant)
                .is_some_and(InteractiveNode::forwards)
                .then_some(claimant),
            None => hit::sink(doc, frame.rects, &hit.path, pos),
        });
    let sink = sink.filter(|_| panned.is_none());
    doc.pointers.route(sink);
    let mut session = Session {
        pointer: event.pointer,
        window: event.window,
        origin_window: event.window,
        kind,
        button,
        role,
        origin: pos,
        pos,
        shift: hit.shift,
        path: hit.path.clone(),
        owners: Vec::new(),
        listeners: match (role, claimant) {
            _ if hit.blocked => Vec::new(),
            (Role::Middle, _) => Vec::new(),
            (_, Some(claimant)) => listeners(doc, &[claimant]),
            (_, None) => listeners(doc, &hit.path),
        },
        claimed: claimant.is_some(),
        focus: None,
        pressed_focus: None,
        sink,
        modifiers: event.modifiers,
        clicks: event.clicks,
        pen: event.pen,
        predicted: None,
        time: event.time,
        dragged: false,
        axis: None,
        drag_target: None,
        cancelled: false,
        long_press: None,
        last_drag: None,
        dragged_this_frame: false,
        outside: match kind.settles && role == Role::Press {
            true => Some(hit.outside.clone()),
            false => None,
        },
    };
    match role {
        _ if hit.blocked => {}
        Role::Press => press(doc, frame, &mut session, claimant),
        Role::Secondary => secondary(doc, frame, &mut session, claimant),
        Role::Middle => middle(doc, frame, &mut session, &hit, panned),
        Role::Finger | Role::Other => {}
    }
    if role != Role::Middle {
        for id in session.listeners.clone() {
            if hear(
                doc,
                id,
                session.input(frame.rects, id, PointerStage::Down, pos),
            ) {
                session.listeners = vec![id];
                break;
            }
        }
    }
    doc.pointers.sessions.push(session);
    if let PointerId::Touch(id) = event.pointer {
        finger_down(doc, frame, id, event.pos);
    }
}

fn reached(doc: &Document, rects: &Rects, path: &[NodeId], pos: Pos2) -> (Vec<NodeId>, bool) {
    let mut reached = Vec::new();
    for id in path {
        reached.push(*id);
        let captures = match (interactive(doc, *id), rects.visible(id)) {
            (Some(_), Some(_)) => hit::captures(doc, rects, *id, pos),
            _ => false,
        };
        if captures {
            return (reached, true);
        }
    }
    (reached, false)
}

fn focus_target(doc: &Document, rects: &Rects, reached: &[NodeId], pos: Pos2) -> Option<NodeId> {
    if reached
        .iter()
        .any(|id| interactive(doc, *id).is_some_and(|node| !node.press_focus))
    {
        return doc.focused_node();
    }
    reached.iter().copied().find(|id| {
        interactive(doc, *id).is_some_and(|node| node.focus.is_some()) && over(rects, *id, pos)
    })
}

fn press(doc: &mut Document, frame: &Frame, session: &mut Session, claimant: Option<NodeId>) {
    let pos = session.pos;
    let (reached, captured) = match claimant {
        Some(claimant) => (vec![claimant], true),
        None => reached(doc, frame.rects, &session.path, pos),
    };
    session.owners = reached
        .iter()
        .copied()
        .filter(|id| interactive(doc, *id).is_some_and(InteractiveNode::holds_presses))
        .collect();
    let target = focus_target(doc, frame.rects, &reached, pos);
    session.focus = match captured {
        true => target.or_else(|| doc.focused_node()),
        false => target,
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
    session.pressed_focus = focused;
    if session.kind.settles {
        session.long_press = Some(frame.now + frame.long_press);
        frame.ctx.request_repaint_after(frame.long_press);
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
}

fn middle(
    doc: &mut Document,
    frame: &Frame,
    session: &mut Session,
    hit: &Hit,
    panned: Option<NodeId>,
) {
    let pos = session.pos;
    let takes = |doc: &Document, id: NodeId| {
        hear(
            doc,
            id,
            session.input(frame.rects, id, PointerStage::Down, pos),
        )
    };
    let taker = match (panned, hit.embed) {
        (Some(panned), _) => takes(doc, panned).then_some(panned),
        (None, true) => None,
        (None, false) => {
            let mut taker = None;
            for id in session.path.clone() {
                let Some(node) = interactive(doc, id) else {
                    continue;
                };
                let stops = node.stops_middle();
                if takes(doc, id) {
                    taker = Some(id);
                    break;
                }
                if stops {
                    break;
                }
            }
            taker
        }
    };
    session.listeners = taker.into_iter().collect();
    session.owners = session
        .path
        .iter()
        .copied()
        .filter(|id| {
            over(frame.rects, *id, pos)
                && interactive(doc, *id).is_some_and(|node| !node.on_middle_click.is_empty())
        })
        .collect();
}

fn moved(doc: &mut Document, frame: &Frame, event: PointerEvent) {
    doc.acted = Acted::Pointer(event.pos + doc.pointers.shift);
    let sink = match find(doc, event.pointer, None) {
        Some(_) => doc.pointers.sink_of(event.pointer),
        None => {
            let hit = hit(doc, frame.rects, event.window, event.pos, false);
            hit::sink(doc, frame.rects, &hit.path, event.pos)
        }
    };
    doc.pointers.route(sink);
    if let PointerId::Touch(id) = event.pointer {
        finger_moved(doc, frame, id, event.pos);
    }
    for index in 0..doc.pointers.sessions.len() {
        if doc.pointers.sessions[index].pointer != event.pointer {
            continue;
        }
        with_session(doc, index, |doc, session| {
            let pos = event.pos + session.shift;
            session.window = event.window;
            session.modifiers = event.modifiers;
            session.pen = event.pen;
            session.predicted = event.predicted.map(|predicted| predicted + session.shift);
            session.time = event.time;
            follow(doc, frame, session, pos, PointerStage::Move);
        });
    }
}

fn with_session(
    doc: &mut Document,
    index: usize,
    change: impl FnOnce(&mut Document, &mut Session),
) {
    let mut session = doc.pointers.sessions.remove(index);
    change(doc, &mut session);
    doc.pointers.sessions.insert(index, session);
}

fn follow(
    doc: &mut Document,
    frame: &Frame,
    session: &mut Session,
    pos: Pos2,
    stage: PointerStage,
) {
    let settles = match session.role == Role::Press && session.kind.settles {
        true => settle(session, pos),
        false => None,
    };
    offer(doc, frame, session, stage, pos, settles);
    session.pos = pos;
    if session.role == Role::Press && !session.cancelled && session.last_drag != Some(pos) {
        drag(doc, frame, session, pos);
    }
}

fn settle(session: &mut Session, pos: Pos2) -> Option<Direction> {
    if session.cancelled || session.axis.is_some() {
        return None;
    }
    let movement = pos - session.origin;
    if movement.x.hypot(movement.y) < TOUCH_DRAG_THRESHOLD {
        return None;
    }
    session.dragged = true;
    session.long_press = None;
    let axis = match movement.y.abs() >= movement.x.abs() {
        true => Direction::Vertical,
        false => Direction::Horizontal,
    };
    session.axis = Some(axis);
    Some(axis)
}

fn offer(
    doc: &mut Document,
    frame: &Frame,
    session: &mut Session,
    stage: PointerStage,
    pos: Pos2,
    settles: Option<Direction>,
) {
    let Some(axis) = settles else {
        tell(doc, frame.rects, session, stage, pos);
        return;
    };
    let candidates: Vec<NodeId> = match session.claimed {
        true => session.owners.first().copied().into_iter().collect(),
        false => session.path.clone(),
    };
    let mut heard = Vec::new();
    let mut winner = None;
    for id in candidates {
        if interactive(doc, id).is_some_and(|node| node.holds_drag(axis)) {
            winner = Some(id);
            break;
        }
        if session.listeners.contains(&id) {
            heard.push(id);
            let input = PointerInput {
                settles: Some(axis),
                ..session.input(frame.rects, id, stage, pos)
            };
            if hear(doc, id, input) {
                winner = Some(id);
                break;
            }
        }
    }
    if winner.is_none() {
        winner = heard.iter().copied().find(|id| {
            let input = PointerInput {
                settles: Some(axis),
                unclaimed: true,
                ..session.input(frame.rects, *id, stage, pos)
            };
            hear(doc, *id, input)
        });
    }
    for id in session.listeners.clone() {
        if !heard.contains(&id) {
            hear(doc, id, session.input(frame.rects, id, stage, pos));
        }
    }
    session.drag_target = winner;
    if winner.is_none() || session.claimed {
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

fn up(doc: &mut Document, frame: &Frame, event: PointerEvent, button: PointerButton) {
    let primary = is_primary(doc, event.pointer);
    let kind = Kind::of(event.pointer);
    let Some(index) = find(doc, event.pointer, Some(button)) else {
        let sink = kind.held_sink(doc, event.pointer);
        doc.pointers.route(sink);
        return;
    };
    let mut session = doc.pointers.sessions.remove(index);
    let sink = kind.held_sink(doc, event.pointer).or(session.sink);
    doc.pointers.route(sink);
    let pos = event.pos + session.shift;
    session.window = event.window;
    session.predicted = None;
    session.time = event.time;
    follow(doc, frame, &mut session, pos, PointerStage::Up);
    end(doc, frame, session, false);
    if let PointerId::Touch(id) = event.pointer {
        finger_lifted(doc, frame, id, Some(event.pos), false, primary);
    }
}

fn end(doc: &mut Document, frame: &Frame, mut session: Session, lost: bool) {
    let pos = session.pos;
    let cancelled = lost || session.cancelled;
    match session.role {
        Role::Press => {
            let focused = doc.focused_node();
            let owners = std::mem::take(&mut session.owners);
            if !cancelled {
                for owner in owners.iter().rev() {
                    let Some(rect) = rect_of(doc, frame.rects, *owner) else {
                        continue;
                    };
                    let clicked = match session.kind.settles {
                        true => !session.dragged,
                        false => {
                            session.window == session.origin_window
                                && over(frame.rects, *owner, pos)
                        }
                    };
                    if !clicked {
                        continue;
                    }
                    let Some(node) = interactive(doc, *owner) else {
                        continue;
                    };
                    let (on_click, on_click_at) = (node.on_click, node.on_click_at);
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
            if session.kind.settles
                && !session.dragged
                && !cancelled
                && doc.focused_node() == focused
                && focused == session.pressed_focus
            {
                doc.press_focus(session.focus);
            }
            if let Some(outside) = session.outside.take()
                && !session.dragged
                && !cancelled
            {
                doc.pressed_outside(
                    frame.rects,
                    &outside,
                    session.origin_window,
                    session.origin - session.shift,
                    true,
                );
            }
            if doc.dragging() {
                doc.drag_board().order(doc);
                match cancelled {
                    true => doc.drag_board().end(None),
                    false => doc.drag_board().finish(),
                }
            }
        }
        Role::Secondary => {}
        Role::Middle => {
            if !cancelled {
                for owner in session.owners.iter().rev() {
                    if session.window != session.origin_window || !over(frame.rects, *owner, pos) {
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
    if lost {
        tell(doc, frame.rects, &session, PointerStage::Cancel, pos);
    }
}

fn cancel(doc: &mut Document, frame: &Frame, pointer: PointerId) {
    let primary = is_primary(doc, pointer);
    let mut held = false;
    while let Some(index) = find(doc, pointer, None) {
        let session = doc.pointers.sessions.remove(index);
        end(doc, frame, session, true);
        held = true;
    }
    if let PointerId::Touch(id) = pointer
        && held
    {
        finger_lifted(doc, frame, id, None, true, primary);
    }
}

fn is_primary(doc: &Document, pointer: PointerId) -> bool {
    primary_touch(doc).is_some_and(|index| doc.pointers.sessions[index].pointer == pointer)
}

fn long_presses(doc: &mut Document, frame: &Frame) -> Vec<LongPressed> {
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
    let mut pressed = Vec::new();
    for index in fired {
        let owners = std::mem::take(&mut doc.pointers.sessions[index].owners);
        for owner in owners.into_iter().rev() {
            cancel_owner(doc, owner);
        }
        let session = &doc.pointers.sessions[index];
        let (pos, clicks, modifiers) = (session.pos, session.clicks, session.modifiers);
        let window = session.window;
        doc.acted = Acted::Pointer(pos);
        let hit = hit(doc, frame.rects, window, pos, false);
        doc.pressed_outside(frame.rects, &hit.outside, window, pos, false);
        pressed.push(LongPressed {
            pos,
            clicks,
            modifiers,
            hit,
        });
    }
    pressed
}

pub(crate) fn long_pressed(doc: &mut Document, frame: &Frame, pressed: Vec<LongPressed>) {
    for LongPressed {
        pos,
        clicks,
        modifiers,
        hit,
    } in pressed
    {
        for id in hit.path.iter().rev() {
            if !over(frame.rects, *id, pos) {
                continue;
            }
            let (Some(node), Some((rect, scale))) = (interactive(doc, *id), frame.rects.scaled(id))
            else {
                continue;
            };
            node.on_secondary_press.clone().call(press_at(
                rect, scale, pos, clicks, modifiers, true, frame.now,
            ));
        }
    }
}

fn primary_touch(doc: &Document) -> Option<usize> {
    doc.pointers
        .sessions
        .iter()
        .position(|session| session.kind.finger && session.role == Role::Press)
}

fn finger_down(doc: &mut Document, frame: &Frame, id: TouchId, pos: Pos2) {
    let count = doc.pointers.touches().count();
    let fingers = &mut doc.pointers.fingers;
    if count == 1 {
        fingers.most = 0;
        fingers.moved = false;
        fingers.since = Some(frame.now);
    }
    fingers.most = fingers.most.max(count);
    let Some(primary) = primary_touch(doc)
        .filter(|index| doc.pointers.sessions[*index].pointer != PointerId::Touch(id))
    else {
        return;
    };
    with_session(doc, primary, |doc, session| {
        cancel_primary(doc, frame, session, id, pos, count == 2);
    });
}

fn cancel_primary(
    doc: &mut Document,
    frame: &Frame,
    session: &mut Session,
    id: TouchId,
    pos: Pos2,
    second: bool,
) {
    let holds = !session.dragged && doc.pointers.fingers.hold.is_none() && second;
    if holds {
        let hit = hit(doc, frame.rects, session.window, pos, false);
        let hold = Hold {
            finger: id,
            anchor: session.pos,
            listeners: listeners(doc, &hit.path),
            pos,
        };
        tell_hold(doc, frame.rects, &hold, PointerStage::Down, pos, frame.now);
        doc.pointers.fingers.hold = Some(hold);
    }
    let owners = std::mem::take(&mut session.owners);
    for owner in owners.into_iter().rev() {
        cancel_owner(doc, owner);
    }
    if !session.cancelled
        && let Some(target) = session.drag_target
    {
        let pos = session.pos;
        hear(
            doc,
            target,
            session.input(frame.rects, target, PointerStage::Cancel, pos),
        );
    }
    session.cancelled = true;
    session.dragged = true;
    session.long_press = None;
    session.drag_target = None;
}

fn tell_hold(
    doc: &Document,
    rects: &Rects,
    hold: &Hold,
    stage: PointerStage,
    pos: Pos2,
    time: Instant,
) {
    for id in hold.listeners.clone() {
        let input = input_at(
            rects,
            id,
            PointerId::Touch(hold.finger),
            Some(PointerButton::Secondary),
            stage,
            pos,
            Modifiers::NONE,
            time,
        );
        hear(doc, id, input);
    }
}

fn finger_moved(doc: &mut Document, frame: &Frame, id: TouchId, pos: Pos2) {
    let primary = is_primary(doc, PointerId::Touch(id));
    let Some((_, _, origin)) = doc.pointers.touches().find(|(finger, _, _)| *finger == id) else {
        return;
    };
    let fingers = &mut doc.pointers.fingers;
    if origin.distance(pos) >= TOUCH_DRAG_THRESHOLD {
        fingers.moved = true;
    }
    let mut moved = false;
    let mut dropped = None;
    if let Some(hold) = &mut fingers.hold {
        if hold.finger == id {
            if hold.pos != pos {
                hold.pos = pos;
                moved = true;
            }
        } else if primary && pos.distance(hold.anchor) >= TOUCH_DRAG_THRESHOLD {
            dropped = fingers.hold.take();
        }
    }
    if moved && let Some(hold) = &doc.pointers.fingers.hold {
        tell_hold(doc, frame.rects, hold, PointerStage::Move, pos, frame.now);
    }
    if let Some(hold) = dropped {
        tell_hold(
            doc,
            frame.rects,
            &hold,
            PointerStage::Cancel,
            hold.pos,
            frame.now,
        );
    }
}

fn finger_lifted(
    doc: &mut Document,
    frame: &Frame,
    id: TouchId,
    pos: Option<Pos2>,
    cancelled: bool,
    primary: bool,
) {
    let now = doc.now();
    let held = doc.pointers.touches().count();
    let fingers = &mut doc.pointers.fingers;
    if held == 0 {
        let quick = fingers
            .since
            .take()
            .is_some_and(|since| now.saturating_duration_since(since) <= FINGER_TAP_TIME);
        if !cancelled && quick && !fingers.moved && fingers.most >= 2 {
            fingers.tap = Some(fingers.most);
        }
    }
    let lifted = fingers.hold.as_ref().is_some_and(|hold| hold.finger == id);
    if !lifted && !primary {
        return;
    }
    let Some(hold) = fingers.hold.take() else {
        return;
    };
    match lifted && !cancelled {
        true => {
            let pos = pos.unwrap_or(hold.pos);
            tell_hold(doc, frame.rects, &hold, PointerStage::Up, pos, frame.now);
        }
        false => tell_hold(
            doc,
            frame.rects,
            &hold,
            PointerStage::Cancel,
            hold.pos,
            frame.now,
        ),
    }
}

fn gesture_claimant(doc: &Document, rects: &Rects, path: &[NodeId], pos: Pos2) -> Option<NodeId> {
    path.iter()
        .rev()
        .copied()
        .find(|id| hit::takes_gestures_at(doc, rects, *id, pos))
}

fn wheel(doc: &mut Document, frame: &Frame, event: PointerEvent, delta: Vec2, fling: Vec2) {
    let pos = event.pos;
    let path = hit(doc, frame.rects, event.window, pos, false).path;
    let claimed = gesture_claimant(doc, frame.rects, &path, pos);
    let sink = match claimed {
        Some(_) => None,
        None => wheel_sink(doc, frame, event.window, pos),
    };
    doc.pointers.route(sink);
    let (delta, fling) = match event.modifiers.shift && delta.x == 0.0 && fling.x == 0.0 {
        true => (vec2(delta.y, 0.0), vec2(fling.y, 0.0)),
        false => (delta, fling),
    };
    let along = match delta {
        Vec2::ZERO => fling,
        delta => delta,
    };
    if along == Vec2::ZERO {
        return;
    }
    let held = doc.pointers.wheel_held;
    let gesture = |id: NodeId, unclaimed: bool| {
        let (local, size) = local(frame.rects, id, pos);
        WheelInput::Scroll(ScrollGesture {
            delta,
            fling,
            pos,
            local,
            size,
            modifiers: event.modifiers,
            held,
            unclaimed,
        })
    };
    let along_path = |unclaimed: bool| {
        for id in path.iter().copied() {
            let node = interactive(doc, id)?;
            let forwards = node.forwards_wheels();
            if turn(doc, id, gesture(id, unclaimed)) {
                return Some(id);
            }
            if forwards {
                return None;
            }
        }
        None
    };
    let target = latched(doc, frame, event.window, pos)
        .filter(|id| turn(doc, *id, gesture(*id, false)))
        .or_else(|| claimed.filter(|id| turn(doc, *id, gesture(*id, false))))
        .or_else(|| along_path(false))
        .or_else(|| along_path(true));
    doc.pointers.wheel_latch = target.map(|target| (target, frame.now, event.window, pos));
}

fn still(doc: &Document, frame: &Frame, target: NodeId, event: PointerEvent, held: bool) {
    let (local, size) = local(frame.rects, target, event.pos);
    let wheel = WheelInput::Scroll(ScrollGesture {
        delta: Vec2::ZERO,
        fling: Vec2::ZERO,
        pos: event.pos,
        local,
        size,
        modifiers: event.modifiers,
        held,
        unclaimed: false,
    });
    turn(doc, target, wheel);
}

fn catch_wheel(doc: &mut Document, frame: &Frame, event: PointerEvent) {
    let Some(target) = hit::claimant(doc, frame.rects, event.window, event.pos, event.modifiers)
        .filter(|target| interactive(doc, *target).is_some_and(|node| !node.on_wheel.is_empty()))
    else {
        return;
    };
    doc.pointers.wheel_latch = Some((target, frame.now, event.window, event.pos));
    still(doc, frame, target, event, true);
}

fn lift_latched(doc: &mut Document, frame: &Frame, event: PointerEvent) {
    if let Some((target, _, _, _)) = doc.pointers.wheel_latch {
        still(doc, frame, target, event, false);
    }
}

fn latched(doc: &Document, frame: &Frame, window: WindowId, pos: Pos2) -> Option<NodeId> {
    let (latched, last, held, at) = doc.pointers.wheel_latch?;
    let recent = frame.now.saturating_duration_since(last) < WHEEL_LATCH_TIMEOUT;
    let still = held == window && at.distance(pos) <= WHEEL_LATCH_SLOP;
    let under = over(frame.rects, latched, pos);
    (recent && still && under).then_some(latched)
}

fn wheel_sink(doc: &mut Document, frame: &Frame, window: WindowId, pos: Pos2) -> Option<NodeId> {
    hit(doc, frame.rects, window, pos, false)
        .path
        .into_iter()
        .find_map(|id| {
            let node = interactive(doc, id)?;
            match (node.forwards_wheels(), node.on_wheel.is_empty()) {
                (true, _) => Some(Some(id)),
                (false, false) => Some(None),
                (false, true) => None,
            }
        })
        .flatten()
}

fn frame_drags(doc: &mut Document, frame: &Frame) {
    for index in 0..doc.pointers.sessions.len() {
        let session = &doc.pointers.sessions[index];
        if session.role != Role::Press || session.cancelled || session.dragged_this_frame {
            continue;
        }
        if session.last_drag.is_none() {
            with_session(doc, index, |doc, session| {
                let pos = session.pos;
                drag(doc, frame, session, pos);
            });
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

fn hover(doc: &mut Document, frame: &Frame, pointer: Option<(WindowId, Pos2)>) {
    let path: Vec<NodeId> = match pointer {
        Some((window, pos)) => hit(doc, frame.rects, window, pos, false)
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
        if let Some(node) = node_mut(doc, id)
            && node.pointed
        {
            node.pointed = false;
            node.on_pointed_change.clone().call(false);
        }
    }
    let moved = pointer != doc.pointers.hovered_at;
    doc.pointers.hovered_at = pointer;
    if let Some((_, pos)) = pointer {
        for id in path.iter().rev() {
            let Some((rect, scale)) = frame.rects.scaled(id) else {
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
            if moved && !node.pointed {
                node.pointed = true;
                node.on_pointed_change.clone().call(true);
            }
            let Some(node) = node_mut(doc, *id) else {
                continue;
            };
            if node.hover_pos != Some(pos) {
                node.hover_pos = Some(pos);
                let press = press_at(
                    rect,
                    scale,
                    pos,
                    0,
                    frame.ctx.input(|input| input.modifiers),
                    false,
                    frame.now,
                );
                node.on_hover_move.clone().call(press);
            }
        }
    }
    doc.pointers.hovered = path;
}

fn cursor(doc: &Document, frame: &Frame) {
    let grabbed = grab::grabber(doc).and_then(|id| interactive(doc, id)?.cursor);
    let held = doc
        .pointers
        .sessions
        .iter()
        .filter(|session| session.kind.shows_cursor && session.role == Role::Press)
        .flat_map(|session| session.owners.iter())
        .chain(doc.pointers.hovered.iter())
        .find_map(|id| interactive(doc, *id).and_then(|node| node.cursor));
    if let Some(cursor) = grabbed.or(held) {
        frame.ctx.set_cursor_icon(cursor);
    }
}

fn track(doc: &mut Document, frame: &Frame) {
    let shift = doc
        .pointers
        .sessions
        .iter()
        .find(|session| session.kind.finger && session.role == Role::Press)
        .map_or(Vec2::ZERO, |session| session.shift);
    doc.pointers.shift = shift;
    let (pointer, window, touch, modifiers) = frame.ctx.input(|input| {
        (
            input.pointer.pos,
            input.pointer.window,
            input.pointer.from_touch(),
            input.modifiers,
        )
    });
    let Some(pos) = pointer.map(|pos| pos + shift) else {
        return;
    };
    doc.last_pointer = Some(PointerSample { pos, window, touch });
    let board = doc.drag_board();
    board.order(doc);
    board.track(crate::drag_board::DragPoint::at(pos, modifiers).in_window(window));
}
