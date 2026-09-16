use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use crate::geometry::{Pos2, Vec2};

const MULTI_CLICK_DELAY: f32 = 0.3;
const MULTI_CLICK_DISTANCE: f32 = 6.0;
const MULTI_CLICK_LIMIT: u32 = 4;
const TOUCH_DRAG_THRESHOLD: f32 = 8.0;
const TOUCH_VELOCITY_WINDOW: f32 = 0.12;
const MAX_TOUCH_VELOCITY: f32 = 4_000.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Backspace,
    Delete,
    End,
    Enter,
    Escape,
    Home,
    Minus,
    PageDown,
    PageUp,
    Plus,
    Space,
    Tab,
    Zero,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        alt: false,
        ctrl: false,
        shift: false,
    };

    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };

    pub const CTRL: Self = Self {
        ctrl: true,
        ..Self::NONE
    };

    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyPress {
    pub key: Key,
    pub pressed: bool,
    pub repeat: bool,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerPress {
    pub pos: Pos2,
    pub fraction: Vec2,
    pub clicks: u32,
    pub modifiers: Modifiers,
    pub touch: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScrollGesture {
    pub delta: Vec2,
    pub pos: Pos2,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ZoomGesture {
    pub factor: f32,
    pub pos: Pos2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TouchId {
    pub device: u64,
    pub finger: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TouchPoint {
    pub id: TouchId,
    pub pos: Pos2,
    pub force: Option<f32>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    Focus(bool),
    Modifiers(Modifiers),
    Key {
        key: Key,
        pressed: bool,
        repeat: bool,
        modifiers: Modifiers,
    },
    PointerButton {
        pos: Pos2,
        button: PointerButton,
        pressed: bool,
        modifiers: Modifiers,
    },
    PointerGone,
    PointerMoved(Pos2),
    Scroll(Vec2),
    Text(String),
    Touch {
        id: TouchId,
        phase: TouchPhase,
        pos: Pos2,
        force: Option<f32>,
    },
    Zoom(f32),
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum CursorIcon {
    #[default]
    Default,
    Crosshair,
    Grab,
    Grabbing,
    NotAllowed,
    PointingHand,
    ResizeHorizontal,
    ResizeVertical,
    Text,
    Wait,
}

#[derive(Clone, Default, Debug)]
pub struct RawInput {
    pub events: Vec<Event>,
}

#[derive(Clone, Debug)]
pub struct InputState {
    pub events: Vec<Event>,
    pub pointer: Pointer,
    pub touch: TouchState,
    pub scroll_delta: Vec2,
    pub zoom_factor: f32,
    pub modifiers: Modifiers,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            pointer: Pointer::default(),
            touch: TouchState::default(),
            scroll_delta: Vec2::ZERO,
            zoom_factor: 1.0,
            modifiers: Modifiers::default(),
        }
    }
}

impl InputState {
    pub(crate) fn scaled(&self, factor: f32) -> Self {
        let scale = |pos: Pos2| Pos2::new(pos.x * factor, pos.y * factor);
        let mut input = self.clone();
        for event in &mut input.events {
            match event {
                Event::PointerMoved(pos)
                | Event::PointerButton { pos, .. }
                | Event::Touch { pos, .. } => *pos = scale(*pos),
                _ => {}
            }
        }
        input.pointer.pos = input.pointer.pos.map(scale);
        input.pointer.last_click = input
            .pointer
            .last_click
            .map(|(when, pos)| (when, scale(pos)));
        let touch = &mut input.touch;
        for point in touch.points.values_mut() {
            point.pos = scale(point.pos);
        }
        touch.start = touch.start.map(scale);
        touch.previous = touch.previous.map(scale);
        touch.scroll_delta = touch.scroll_delta * factor;
        touch.pinch_pan = touch.pinch_pan * factor;
        touch.pinch_center = touch.pinch_center.map(scale);
        touch.pinch_span = touch.pinch_span.map(|span| span * factor);
        for (_, pos) in &mut touch.samples {
            *pos = scale(*pos);
        }
        touch.velocity = touch.velocity * factor;
        input
    }

    pub(crate) fn begin_frame(&mut self, raw: RawInput) {
        self.pointer.begin_frame();
        self.touch.begin_frame(&mut self.pointer);
        self.scroll_delta = Vec2::ZERO;
        self.zoom_factor = 1.0;
        let suppress_mouse = self.pointer.from_touch
            || raw
                .events
                .iter()
                .any(|event| matches!(event, Event::Touch { .. }));
        for event in &raw.events {
            match event {
                Event::PointerMoved(pos) if !suppress_mouse => self.pointer.pos = Some(*pos),
                Event::PointerGone if !suppress_mouse => {
                    self.pointer.pos = None;
                    self.pointer.primary_down = false;
                    self.pointer.secondary_down = false;
                    self.pointer.middle_down = false;
                }
                Event::Focus(false) => {
                    self.touch.cancel(&mut self.pointer);
                    self.pointer.pos = None;
                    self.pointer.primary_down = false;
                    self.pointer.secondary_down = false;
                    self.pointer.middle_down = false;
                }
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } if !suppress_mouse => {
                    self.pointer.pos = Some(*pos);
                    self.modifiers = *modifiers;
                    match button {
                        PointerButton::Primary => {
                            self.pointer.primary_down = *pressed;
                            if *pressed {
                                self.pointer.primary_pressed = true;
                                self.pointer.count_click(*pos);
                            } else {
                                self.pointer.primary_released = true;
                            }
                        }
                        PointerButton::Secondary => {
                            self.pointer.secondary_down = *pressed;
                            if *pressed {
                                self.pointer.secondary_pressed = true;
                            } else {
                                self.pointer.secondary_released = true;
                            }
                        }
                        PointerButton::Middle => {
                            self.pointer.middle_down = *pressed;
                            if *pressed {
                                self.pointer.middle_pressed = true;
                            } else {
                                self.pointer.middle_released = true;
                            }
                        }
                    }
                }
                Event::Scroll(delta) => self.scroll_delta = self.scroll_delta + *delta,
                Event::Zoom(factor) => self.zoom_factor *= *factor,
                Event::Key { modifiers, .. } | Event::Modifiers(modifiers) => {
                    self.modifiers = *modifiers;
                }
                Event::Touch {
                    id,
                    phase,
                    pos,
                    force,
                } => self
                    .touch
                    .event(*id, *phase, *pos, *force, &mut self.pointer),
                _ => {}
            }
        }
        self.events = raw.events;
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Pointer {
    pub pos: Option<Pos2>,
    pub primary_down: bool,
    pub primary_pressed: bool,
    pub primary_released: bool,
    pub secondary_down: bool,
    pub secondary_pressed: bool,
    pub secondary_released: bool,
    pub middle_down: bool,
    pub middle_pressed: bool,
    pub middle_released: bool,
    clicks: u32,
    last_click: Option<(Instant, Pos2)>,
    from_touch: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum TouchDirection {
    #[default]
    Undecided,
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug)]
pub struct TouchState {
    points: HashMap<TouchId, TouchPoint>,
    primary: Option<TouchId>,
    start: Option<Pos2>,
    previous: Option<Pos2>,
    direction: TouchDirection,
    multi: bool,
    started: bool,
    ended: bool,
    cancelled: bool,
    dragged: bool,
    scroll_delta: Vec2,
    samples: VecDeque<(Instant, Pos2)>,
    velocity: Vec2,
    pinch: f32,
    pinch_pan: Vec2,
    pinch_center: Option<Pos2>,
    pinch_span: Option<f32>,
}

impl Default for TouchState {
    fn default() -> Self {
        Self {
            points: HashMap::new(),
            primary: None,
            start: None,
            previous: None,
            direction: TouchDirection::Undecided,
            multi: false,
            started: false,
            ended: false,
            cancelled: false,
            dragged: false,
            scroll_delta: Vec2::ZERO,
            samples: VecDeque::new(),
            velocity: Vec2::ZERO,
            pinch: 1.0,
            pinch_pan: Vec2::ZERO,
            pinch_center: None,
            pinch_span: None,
        }
    }
}

impl TouchState {
    fn begin_frame(&mut self, pointer: &mut Pointer) {
        self.started = false;
        self.ended = false;
        self.cancelled = false;
        self.scroll_delta = Vec2::ZERO;
        self.pinch = 1.0;
        self.pinch_pan = Vec2::ZERO;
        if self.primary.is_none() {
            self.start = None;
            self.previous = None;
            self.direction = TouchDirection::Undecided;
            self.multi = false;
            self.dragged = false;
            self.samples.clear();
            self.velocity = Vec2::ZERO;
            self.pinch_center = None;
            self.pinch_span = None;
            if pointer.from_touch {
                pointer.pos = None;
                pointer.from_touch = false;
            }
        }
    }

    fn event(
        &mut self,
        id: TouchId,
        phase: TouchPhase,
        pos: Pos2,
        force: Option<f32>,
        pointer: &mut Pointer,
    ) {
        match phase {
            TouchPhase::Start => self.start(id, pos, force, pointer),
            TouchPhase::Move => self.move_to(id, pos, force, pointer),
            TouchPhase::End => self.finish(id, pos, force, false, pointer),
            TouchPhase::Cancel => self.finish(id, pos, force, true, pointer),
        }
    }

    fn start(&mut self, id: TouchId, pos: Pos2, force: Option<f32>, pointer: &mut Pointer) {
        self.points.insert(id, TouchPoint { id, pos, force });
        self.measure_pinch();
        if self.primary.is_some() {
            self.cancelled = true;
            self.dragged = true;
            self.multi = true;
            self.direction = TouchDirection::Undecided;
            return;
        }
        self.primary = Some(id);
        self.start = Some(pos);
        self.previous = Some(pos);
        self.started = true;
        self.direction = TouchDirection::Undecided;
        self.dragged = false;
        self.samples.clear();
        self.samples.push_back((Instant::now(), pos));
        self.velocity = Vec2::ZERO;
        pointer.pos = Some(pos);
        pointer.primary_down = true;
        pointer.primary_pressed = true;
        pointer.from_touch = true;
        pointer.count_click(pos);
    }

    fn move_to(&mut self, id: TouchId, pos: Pos2, force: Option<f32>, pointer: &mut Pointer) {
        let Some(point) = self.points.get_mut(&id) else {
            return;
        };
        point.pos = pos;
        point.force = force;
        self.measure_pinch();
        if self.primary != Some(id) {
            return;
        }
        let previous = self.previous.replace(pos).unwrap_or(pos);
        pointer.pos = Some(pos);
        if self.multi {
            self.sample(pos);
            return;
        }
        let movement = pos - self.start.unwrap_or(pos);
        if self.direction == TouchDirection::Undecided
            && movement.x.hypot(movement.y) >= TOUCH_DRAG_THRESHOLD
        {
            self.dragged = true;
            self.direction = if movement.y.abs() >= movement.x.abs() {
                TouchDirection::Vertical
            } else {
                TouchDirection::Horizontal
            };
            self.scroll_delta = self.scroll_delta + self.along(movement);
        } else if self.direction != TouchDirection::Undecided {
            self.scroll_delta = self.scroll_delta + self.along(pos - previous);
        }
        self.sample(pos);
    }

    fn finish(
        &mut self,
        id: TouchId,
        pos: Pos2,
        force: Option<f32>,
        cancelled: bool,
        pointer: &mut Pointer,
    ) {
        self.move_to(id, pos, force, pointer);
        self.points.remove(&id);
        self.pinch_span = None;
        self.pinch_center = None;
        if self.primary != Some(id) {
            return;
        }
        self.primary = None;
        self.ended = !cancelled;
        self.cancelled |= cancelled || !self.points.is_empty();
        pointer.primary_down = false;
        pointer.primary_released = true;
        pointer.from_touch = true;
        if cancelled {
            pointer.pos = None;
        }
    }

    fn measure_pinch(&mut self) {
        let mut points = self.points.values();
        let (Some(first), Some(second), None) = (points.next(), points.next(), points.next())
        else {
            self.pinch_center = None;
            self.pinch_span = None;
            return;
        };
        let (first, second) = (first.pos, second.pos);
        let span = first.distance(second);
        let center = Pos2::new((first.x + second.x) * 0.5, (first.y + second.y) * 0.5);
        if let (Some(previous_span), Some(previous_center)) = (self.pinch_span, self.pinch_center)
            && previous_span > 0.0
            && span > 0.0
        {
            self.pinch *= span / previous_span;
            self.pinch_pan = self.pinch_pan + (center - previous_center);
        }
        self.pinch_span = Some(span);
        self.pinch_center = Some(center);
    }

    fn along(&self, movement: Vec2) -> Vec2 {
        match self.direction {
            TouchDirection::Vertical => Vec2::new(0.0, movement.y),
            TouchDirection::Horizontal => Vec2::new(movement.x, 0.0),
            TouchDirection::Undecided => Vec2::ZERO,
        }
    }

    fn cancel(&mut self, pointer: &mut Pointer) {
        if self.primary.is_some() {
            self.cancelled = true;
        }
        self.points.clear();
        self.primary = None;
        self.start = None;
        self.previous = None;
        self.direction = TouchDirection::Undecided;
        self.multi = false;
        self.dragged = false;
        self.samples.clear();
        self.velocity = Vec2::ZERO;
        self.pinch_span = None;
        self.pinch_center = None;
        pointer.from_touch = false;
    }

    fn sample(&mut self, pos: Pos2) {
        let now = Instant::now();
        self.samples.push_back((now, pos));
        while self.samples.len() > 2
            && self.samples.front().is_some_and(|(when, _)| {
                now.duration_since(*when).as_secs_f32() > TOUCH_VELOCITY_WINDOW
            })
        {
            self.samples.pop_front();
        }
        let Some((then, origin)) = self.samples.front().copied() else {
            return;
        };
        let elapsed = now.duration_since(then).as_secs_f32();
        if elapsed <= f32::EPSILON {
            return;
        }
        let measured = (pos - origin) * elapsed.recip();
        let speed = measured.x.hypot(measured.y);
        self.velocity = if speed > MAX_TOUCH_VELOCITY {
            measured * (MAX_TOUCH_VELOCITY / speed)
        } else {
            measured
        };
    }

    pub fn points(&self) -> impl Iterator<Item = &TouchPoint> {
        self.points.values()
    }

    pub fn primary_pos(&self) -> Option<Pos2> {
        self.primary.map(|id| self.points[&id].pos)
    }

    pub fn active(&self) -> bool {
        self.primary.is_some()
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn dragged(&self) -> bool {
        self.dragged
    }

    pub fn scrolling(&self) -> bool {
        self.direction == TouchDirection::Vertical
    }

    pub fn scrolling_horizontally(&self) -> bool {
        self.direction == TouchDirection::Horizontal
    }

    pub fn scroll_delta(&self) -> Vec2 {
        self.scroll_delta
    }

    pub fn velocity(&self) -> Vec2 {
        self.velocity
    }

    pub fn pinch(&self) -> f32 {
        self.pinch
    }

    pub fn pinch_pan(&self) -> Vec2 {
        self.pinch_pan
    }

    pub fn pinch_center(&self) -> Option<Pos2> {
        self.pinch_center
    }
}

impl Pointer {
    fn begin_frame(&mut self) {
        self.primary_pressed = false;
        self.primary_released = false;
        self.secondary_pressed = false;
        self.secondary_released = false;
        self.middle_pressed = false;
        self.middle_released = false;
    }

    fn count_click(&mut self, pos: Pos2) {
        let now = Instant::now();
        let repeated = self.last_click.is_some_and(|(when, at)| {
            now.duration_since(when).as_secs_f32() <= MULTI_CLICK_DELAY
                && at.distance(pos) <= MULTI_CLICK_DISTANCE
        });
        self.clicks = if repeated {
            self.clicks % MULTI_CLICK_LIMIT + 1
        } else {
            1
        };
        self.last_click = Some((now, pos));
    }

    pub fn clicks(&self) -> u32 {
        self.clicks
    }

    pub fn interact_pos(&self) -> Option<Pos2> {
        self.pos
    }

    pub fn primary_pressed(&self) -> bool {
        self.primary_pressed
    }

    pub fn primary_released(&self) -> bool {
        self.primary_released
    }

    pub fn secondary_pressed(&self) -> bool {
        self.secondary_pressed
    }

    pub fn secondary_released(&self) -> bool {
        self.secondary_released
    }

    pub fn middle_pressed(&self) -> bool {
        self.middle_pressed
    }

    pub fn middle_released(&self) -> bool {
        self.middle_released
    }
}
