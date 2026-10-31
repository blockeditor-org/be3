use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use beui_view::reactive::*;
use beui_view::types::{
    AutoscrollGesture, Direction, DragGesture, PointerButton, PointerId, PointerInput,
    PointerStage, Pos2, ScrollGesture, SecondaryDrag, TOUCH_DRAG_THRESHOLD, Vec2, WheelInput,
    Velocity, ZoomGesture, vec2,
};

pub const AUTOSCROLL_DEAD_ZONE: f32 = 10.0;

pub fn first_of<T: Copy + 'static>(
    handlers: impl IntoIterator<Item = Callback<T, bool>>,
) -> Callback<T, bool> {
    let handlers: Vec<Callback<T, bool>> = handlers
        .into_iter()
        .filter(|handler| !handler.is_empty())
        .collect();
    match handlers.as_slice() {
        [] => Callback::empty(),
        [only] => *only,
        _ => Callback::new(move |input: T| handlers.iter().any(|handler| handler.call(input))),
    }
}

pub fn scroll_wheel(
    axis: impl IntoProp<Option<Direction>>,
    on_scroll: impl IntoCallback<ScrollGesture, ()>,
) -> Callback<WheelInput, bool> {
    let axis = axis.into_prop();
    let on_scroll = on_scroll.into_callback();
    Callback::new(move |input: WheelInput| {
        let WheelInput::Scroll(gesture) = input else {
            return false;
        };
        let along = match gesture.delta {
            Vec2::ZERO => gesture.fling,
            delta => delta,
        };
        let takes = along == Vec2::ZERO
            || match axis.get_untracked() {
                Some(axis) => axis.main(along) != 0.0,
                None => true,
            };
        if takes {
            on_scroll.call(gesture);
        }
        takes
    })
}

struct Track {
    pointer: PointerId,
    origin: Pos2,
    last: Pos2,
    axis: Option<Direction>,
    started: bool,
    velocity: Velocity,
}

impl Track {
    fn new(input: PointerInput) -> Self {
        Self {
            pointer: input.pointer,
            origin: input.pos,
            last: input.pos,
            axis: None,
            started: false,
            velocity: Velocity::new(input.time, input.pos),
        }
    }

    fn sample(&mut self, now: Instant, pos: Pos2) {
        self.velocity.sample(now, pos);
    }

    fn gesture(&mut self, delta: Vec2, scale: f32, ended: bool, cancelled: bool) -> DragGesture {
        let started = !std::mem::replace(&mut self.started, true);
        DragGesture {
            started,
            ended,
            cancelled,
            delta: delta / scale,
            velocity: self.velocity.get() / scale,
        }
    }
}

fn along(axis: Direction, movement: Vec2) -> Vec2 {
    match axis {
        Direction::Vertical => vec2(0.0, movement.y),
        Direction::Horizontal => vec2(movement.x, 0.0),
    }
}

pub fn scroll_drag(
    axis: impl IntoProp<Option<Direction>>,
    on_drag: impl IntoCallback<DragGesture, ()>,
) -> Callback<PointerInput, bool> {
    chaining_scroll_drag(axis, |_: Vec2| false, on_drag)
}

pub fn chaining_scroll_drag(
    axis: impl IntoProp<Option<Direction>>,
    chains: impl Fn(Vec2) -> bool + 'static,
    on_drag: impl IntoCallback<DragGesture, ()>,
) -> Callback<PointerInput, bool> {
    let axis = axis.into_prop();
    let on_drag = on_drag.into_callback();
    let mut tracks: Vec<Track> = Vec::new();
    Callback::new(move |input: PointerInput| {
        if input.stage == PointerStage::Down {
            tracks.retain(|track| track.pointer != input.pointer);
            tracks.push(Track::new(input));
            return false;
        }
        let Some(index) = tracks
            .iter()
            .position(|track| track.pointer == input.pointer)
        else {
            return false;
        };
        let track = &mut tracks[index];
        let mut drags = Vec::new();
        let mut claimed = false;
        match input.stage {
            PointerStage::Down => {}
            PointerStage::Cancel => {
                if track.axis.is_some() {
                    drags.push(track.gesture(Vec2::ZERO, input.scale, false, true));
                }
                tracks.remove(index);
            }
            PointerStage::Move | PointerStage::Up => {
                track.sample(input.time, input.pos);
                if let Some(settled) = input.settles
                    && track.axis.is_none()
                    && axis.get_untracked().is_none_or(|axis| axis == settled)
                {
                    let movement = input.pos - track.origin;
                    if !input.unclaimed && chains(along(settled, movement)) {
                        return false;
                    }
                    let distance = movement.x.hypot(movement.y);
                    track.axis = Some(settled);
                    claimed = true;
                    let delta = along(settled, movement * (1.0 - TOUCH_DRAG_THRESHOLD / distance));
                    drags.push(track.gesture(delta, input.scale, false, false));
                } else if let Some(axis) = track.axis {
                    let delta = along(axis, input.pos - track.last);
                    drags.push(track.gesture(delta, input.scale, false, false));
                }
                track.last = input.pos;
                if input.stage == PointerStage::Up {
                    if track.axis.is_some() {
                        drags.push(track.gesture(Vec2::ZERO, input.scale, true, false));
                    }
                    tracks.remove(index);
                }
            }
        }
        for drag in drags {
            on_drag.call(drag);
        }
        claimed
    })
}

#[derive(Clone, Copy)]
struct Scrolling {
    origin: Pos2,
    pos: Pos2,
    held: bool,
    travelled: bool,
}

#[derive(Clone, Copy)]
pub struct Autoscroll {
    pub pointer: Callback<PointerInput, bool>,
    pub wheel: Callback<WheelInput, bool>,
    pub active: ReadSignal<bool>,
}

pub fn autoscroll(on_autoscroll: impl IntoCallback<AutoscrollGesture, ()>) -> Autoscroll {
    let on_autoscroll = on_autoscroll.into_callback();
    let active = create_signal(false);
    let state: Rc<RefCell<Option<Scrolling>>> = Rc::default();
    let report = move |scrolling: Scrolling, ended: bool| {
        on_autoscroll.call(AutoscrollGesture {
            origin: scrolling.origin,
            pos: scrolling.pos,
            ended,
        });
    };
    let finish = {
        let state = Rc::clone(&state);
        move || {
            let Some(scrolling) = state.borrow_mut().take() else {
                return false;
            };
            active.set(false);
            report(scrolling, true);
            true
        }
    };
    let pointer = {
        let state = Rc::clone(&state);
        let finish = finish.clone();
        Callback::new(move |input: PointerInput| {
            let scrolling = *state.borrow();
            let Some(mut scrolling) = scrolling else {
                if input.stage != PointerStage::Down || input.button != Some(PointerButton::Middle)
                {
                    return false;
                }
                let scrolling = Scrolling {
                    origin: input.pos,
                    pos: input.pos,
                    held: true,
                    travelled: false,
                };
                *state.borrow_mut() = Some(scrolling);
                active.set(true);
                report(scrolling, false);
                return true;
            };
            match input.stage {
                PointerStage::Down => finish(),
                PointerStage::Move if !input.pointer.hovers() => false,
                PointerStage::Move => {
                    scrolling.travelled |= scrolling.held
                        && input.pos.distance(scrolling.origin) > AUTOSCROLL_DEAD_ZONE;
                    let moved = input.pos != scrolling.pos;
                    scrolling.pos = input.pos;
                    *state.borrow_mut() = Some(scrolling);
                    if moved {
                        report(scrolling, false);
                    }
                    false
                }
                PointerStage::Up
                    if scrolling.held && input.button == Some(PointerButton::Middle) =>
                {
                    scrolling.held = false;
                    *state.borrow_mut() = Some(scrolling);
                    if scrolling.travelled {
                        finish();
                    }
                    false
                }
                PointerStage::Up => false,
                PointerStage::Cancel => {
                    finish();
                    false
                }
            }
        })
    };
    let wheel =
        Callback::new(move |input: WheelInput| matches!(input, WheelInput::Scroll(_)) && finish());
    Autoscroll {
        pointer,
        wheel,
        active: active.read_only(),
    }
}

pub fn secondary_drag(
    on_drag: impl IntoCallback<SecondaryDrag, ()>,
) -> Callback<PointerInput, bool> {
    let on_drag = on_drag.into_callback();
    let mut held: Vec<(PointerId, SecondaryDrag)> = Vec::new();
    Callback::new(move |input: PointerInput| {
        if input.button != Some(PointerButton::Secondary) {
            return false;
        }
        if input.stage == PointerStage::Down {
            held.retain(|(pointer, _)| *pointer != input.pointer);
            if !input.inside {
                return false;
            }
            let drag = SecondaryDrag {
                from: input.pos,
                pos: input.pos,
                started: true,
                ended: false,
                cancelled: false,
                modifiers: input.modifiers,
            };
            held.push((input.pointer, drag));
            on_drag.call(drag);
            return false;
        }
        let Some(index) = held
            .iter()
            .position(|(pointer, _)| *pointer == input.pointer)
        else {
            return false;
        };
        let last = held[index].1;
        let drag = match input.stage {
            PointerStage::Down => return false,
            PointerStage::Move if input.pos == last.pos => return false,
            PointerStage::Move => SecondaryDrag {
                pos: input.pos,
                started: false,
                modifiers: input.modifiers,
                ..last
            },
            PointerStage::Up => SecondaryDrag {
                pos: input.pos,
                started: false,
                ended: true,
                modifiers: input.modifiers,
                ..last
            },
            PointerStage::Cancel => SecondaryDrag {
                started: false,
                cancelled: true,
                ..last
            },
        };
        match input.stage {
            PointerStage::Move => held[index].1 = drag,
            _ => {
                held.remove(index);
            }
        }
        on_drag.call(drag);
        false
    })
}

#[derive(Clone, Copy)]
pub struct PanZoom {
    pub pointer: Callback<PointerInput, bool>,
    pub wheel: Callback<WheelInput, bool>,
    pub panning: ReadSignal<bool>,
}

#[derive(Default)]
struct Pinch {
    middle: Option<(PointerId, Pos2)>,
    touches: Vec<(PointerId, Pos2)>,
    holds: usize,
    span: Option<f32>,
    center: Option<Pos2>,
    pinching: bool,
    factor: f32,
    pan: Vec2,
    scheduled: bool,
    reference: Option<PointerInput>,
}

impl Pinch {
    fn measure(&mut self) {
        let held: Vec<Pos2> = self.touches.iter().map(|(_, pos)| *pos).take(3).collect();
        let [first, second] = held[..] else {
            self.span = None;
            self.center = None;
            return;
        };
        let span = first.distance(second);
        let center = Pos2::new((first.x + second.x) * 0.5, (first.y + second.y) * 0.5);
        if let (Some(previous_span), Some(previous_center)) = (self.span, self.center)
            && self.holds == 0
            && previous_span > 0.0
            && span > 0.0
        {
            self.factor *= span / previous_span;
            self.pan += center - previous_center;
        }
        self.span = Some(span);
        self.center = Some(center);
    }

    fn take(&mut self) -> Option<(Option<ZoomGesture>, Option<Vec2>)> {
        self.scheduled = false;
        let (factor, pan) = (
            std::mem::replace(&mut self.factor, 1.0),
            std::mem::take(&mut self.pan),
        );
        if factor == 1.0 && pan == Vec2::ZERO {
            return None;
        }
        let (center, reference) = (self.center?, self.reference?);
        if self.touches.len() >= 2 {
            self.pinching = true;
        }
        let zoom = (factor != 1.0).then(|| ZoomGesture {
            factor,
            pos: center,
            local: reference.local + (center - reference.pos) / reference.scale,
            size: reference.size,
        });
        Some((zoom, (pan != Vec2::ZERO).then_some(pan)))
    }
}

pub fn pan_zoom(
    on_pan: impl IntoCallback<Vec2, ()>,
    on_zoom: impl IntoCallback<ZoomGesture, ()>,
) -> PanZoom {
    let (on_pan, on_zoom) = (on_pan.into_callback(), on_zoom.into_callback());
    let panning = create_signal(false);
    let pinch = Rc::new(RefCell::new(Pinch {
        factor: 1.0,
        ..Pinch::default()
    }));
    let settle = {
        let pinch = Rc::clone(&pinch);
        move || {
            let now = {
                let pinch = pinch.borrow();
                pinch.middle.is_some() || pinch.pinching
            };
            if panning.get_untracked() != now {
                panning.set(now);
            }
        }
    };
    let flush = {
        let pinch = Rc::clone(&pinch);
        let settle = settle.clone();
        move || {
            let taken = pinch.borrow_mut().take();
            settle();
            let Some((zoom, pan)) = taken else {
                return;
            };
            if let Some(zoom) = zoom {
                on_zoom.call(zoom);
            }
            if let Some(pan) = pan {
                on_pan.call(pan);
            }
        }
    };
    let pointer = Callback::new(move |input: PointerInput| {
        let mut pan = None;
        let mut taken = false;
        let mut schedule = false;
        {
            let mut pinch = pinch.borrow_mut();
            match (input.button, input.pointer) {
                (Some(PointerButton::Middle), _) => match input.stage {
                    PointerStage::Down => {
                        pinch.middle = Some((input.pointer, input.pos));
                        taken = true;
                    }
                    PointerStage::Move | PointerStage::Up | PointerStage::Cancel => {
                        if let Some((pointer, last)) = pinch.middle
                            && pointer == input.pointer
                        {
                            if input.stage != PointerStage::Cancel && input.pos != last {
                                pan = Some(input.pos - last);
                            }
                            pinch.middle = match input.stage {
                                PointerStage::Move => Some((pointer, input.pos)),
                                _ => None,
                            };
                        }
                    }
                },
                (Some(PointerButton::Secondary), PointerId::Touch(_)) => match input.stage {
                    PointerStage::Down => pinch.holds += 1,
                    PointerStage::Up | PointerStage::Cancel => {
                        pinch.holds = pinch.holds.saturating_sub(1);
                    }
                    PointerStage::Move => {}
                },
                (_, PointerId::Touch(_)) => match input.stage {
                    PointerStage::Down | PointerStage::Move => {
                        match pinch
                            .touches
                            .iter_mut()
                            .find(|(pointer, _)| *pointer == input.pointer)
                        {
                            Some(touch) => touch.1 = input.pos,
                            None => pinch.touches.push((input.pointer, input.pos)),
                        }
                        pinch.measure();
                        pinch.reference = Some(input);
                        schedule = !std::mem::replace(&mut pinch.scheduled, true);
                    }
                    PointerStage::Up | PointerStage::Cancel => {
                        pinch
                            .touches
                            .retain(|(pointer, _)| *pointer != input.pointer);
                        pinch.span = None;
                        pinch.center = None;
                        if pinch.touches.len() < 2 {
                            pinch.pinching = false;
                        }
                    }
                },
                _ => {}
            }
        }
        if schedule {
            after_input(flush.clone());
        }
        settle();
        if let Some(pan) = pan {
            on_pan.call(pan);
        }
        taken
    });
    let wheel = Callback::new(move |input: WheelInput| match input {
        WheelInput::Zoom(zoom) => {
            on_zoom.call(zoom);
            true
        }
        WheelInput::Scroll(_) => false,
    });
    PanZoom {
        pointer,
        wheel,
        panning: panning.read_only(),
    }
}
