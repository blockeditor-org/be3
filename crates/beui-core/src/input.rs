use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::geometry::{Pos2, Vec2};

pub mod typing;

pub use ui_types::input::{
    BackEdge, BackGesture, CursorIcon, ImeEvent, ImeText, Input, Key, KeyChord, KeyPress, Keyboard,
    Keystroke, Modifiers, Pen, PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};
pub use ui_types::window::ImeArea;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DroppedFile {
    pub name: String,
    pub path: Option<std::path::PathBuf>,
    pub bytes: Option<std::sync::Arc<[u8]>>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FileDrag {
    Hovered,
    Cancelled,
    Dropped(DroppedFile),
}

pub const LONG_PRESS_DELAY: Duration = Duration::from_millis(500);
pub const FINGER_TAP_TIME: Duration = Duration::from_millis(350);
const WHEEL_VELOCITY_WINDOW: Duration = Duration::from_millis(100);
const SYNTHESIZED_SLOP: f32 = 2.0;

pub use beui_types::input::{
    AutoscrollGesture, ClickCounter, DragGesture, MAX_TOUCH_VELOCITY, PointerId, PointerInput, PointerPress,
    PointerSample, PointerStage, ScrollGesture, SecondaryDrag, SoftKeyboard, TOUCH_DRAG_THRESHOLD,
    TOUCH_VELOCITY_WINDOW, TouchDrags, WheelInput, ZoomGesture,
};

#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    Focus(bool),
    Modifiers(Modifiers),
    Key {
        press: KeyPress,
        evdev: Option<u32>,
        text: Option<String>,
    },
    PointerButton {
        pos: Pos2,
        button: PointerButton,
        pressed: bool,
        modifiers: Modifiers,
    },
    PointerGone,
    PointersCancelled,
    PointerMotion(Vec2),
    PointerUnlocked,
    PointerMoved(Pos2),
    ScrollStarted,
    Scroll(Vec2),
    ScrollEnded,
    Text(String),
    Paste(crate::clipboard::Pasted),
    Touch {
        id: TouchId,
        phase: TouchPhase,
        pos: Pos2,
        force: Option<f32>,
    },
    Pen {
        phase: PenPhase,
        pos: Pos2,
        pen: Pen,
    },
    Predicted(Pos2),
    Time(Instant),
    Zoom(f32),
    Ime(ImeEvent),
    FileHovered,
    FileHoverCancelled,
    FileDropped(DroppedFile),
    Back(BackGesture),
    InterceptedKey(KeyPress),
    InterceptedTap(Key),
    PointerWindow(crate::window::WindowId),
    PointerOutside,
}

impl Event {
    pub fn key(press: KeyPress) -> Self {
        Self::Key {
            press,
            evdev: None,
            text: None,
        }
    }

    pub fn press(&self) -> Option<KeyPress> {
        match self {
            Self::Key { press, .. } => Some(*press),
            _ => None,
        }
    }

    pub fn is_pointer(&self) -> bool {
        matches!(
            self,
            Self::PointerButton { .. }
                | Self::PointerGone
                | Self::PointersCancelled
                | Self::PointerMotion(_)
                | Self::PointerMoved(_)
                | Self::ScrollStarted
                | Self::Scroll(_)
                | Self::ScrollEnded
                | Self::Touch { .. }
                | Self::Pen { .. }
                | Self::Predicted(_)
                | Self::Time(_)
                | Self::Zoom(_)
                | Self::PointerWindow(_)
                | Self::PointerOutside
        )
    }

    pub fn is_key(&self) -> bool {
        matches!(
            self,
            Self::Key { .. }
                | Self::Text(_)
                | Self::Ime(_)
                | Self::Modifiers(_)
                | Self::InterceptedKey(_)
                | Self::InterceptedTap(_)
        )
    }

    pub fn modifiers(&self) -> Option<Modifiers> {
        match self {
            Self::Key { press, .. } | Self::InterceptedKey(press) => Some(press.modifiers),
            Self::Modifiers(modifiers) | Self::PointerButton { modifiers, .. } => Some(*modifiers),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PointerPhase {
    Down(PointerButton),
    Move,
    Up(PointerButton),
    Cancel,
    Gone,
    ScrollStarted,
    Scroll(Vec2),
    ScrollEnded { fling: Vec2 },
    Zoom(f32),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerEvent {
    pub pointer: PointerId,
    pub window: crate::window::WindowId,
    pub phase: PointerPhase,
    pub pos: Pos2,
    pub modifiers: Modifiers,
    pub clicks: u32,
    pub force: Option<f32>,
    pub pen: Option<Pen>,
    pub predicted: Option<Pos2>,
    pub time: Instant,
    pub source: usize,
}

#[derive(Clone, Debug)]
pub struct InputState {
    pub events: Vec<Event>,
    pub pointer_events: Vec<PointerEvent>,
    pub pointer: Pointer,
    pub modifiers: Modifiers,
    pub long_press_delay: Duration,
    pub window: crate::window::WindowId,
    touches: Vec<(TouchId, Pos2)>,
    touched: Vec<Pos2>,
    source: usize,
    time: Option<Instant>,
    force: Option<f32>,
    pen: Option<Pen>,
    pen_at: Option<Pos2>,
    scroll_samples: VecDeque<(Instant, Vec2)>,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            pointer_events: Vec::new(),
            pointer: Pointer::default(),
            modifiers: Modifiers::default(),
            long_press_delay: LONG_PRESS_DELAY,
            window: crate::window::WindowId::MAIN,
            touches: Vec::new(),
            touched: Vec::new(),
            source: 0,
            time: None,
            force: None,
            pen: None,
            pen_at: None,
            scroll_samples: VecDeque::new(),
        }
    }
}

impl InputState {
    pub fn scaled(&self, factor: f32) -> Self {
        let scale = |pos: Pos2| Pos2::new(pos.x * factor, pos.y * factor);
        let mut input = self.clone();
        for event in &mut input.events {
            match event {
                Event::PointerMoved(pos)
                | Event::PointerButton { pos, .. }
                | Event::Touch { pos, .. }
                | Event::Pen { pos, .. }
                | Event::Predicted(pos) => *pos = scale(*pos),
                Event::Scroll(delta) => *delta *= factor,
                _ => {}
            }
        }
        for event in &mut input.pointer_events {
            event.pos = scale(event.pos);
            event.predicted = event.predicted.map(scale);
            match &mut event.phase {
                PointerPhase::Scroll(delta) | PointerPhase::ScrollEnded { fling: delta } => {
                    *delta *= factor;
                }
                _ => {}
            }
        }
        for (_, pos) in &mut input.touches {
            *pos = scale(*pos);
        }
        for pos in &mut input.touched {
            *pos = scale(*pos);
        }
        input.pointer.pos = input.pointer.pos.map(scale);
        input.pointer.motion *= factor;
        input.pointer.clicks.scale(scale);
        input
    }

    pub fn only(&self, keep: impl Fn(&Event) -> bool) -> Self {
        let mut input = self.clone();
        let mut moved = Vec::with_capacity(self.events.len());
        input.events.clear();
        for event in &self.events {
            match keep(event) {
                true => {
                    moved.push(Some(input.events.len()));
                    input.events.push(event.clone());
                }
                false => moved.push(None),
            }
        }
        input.pointer_events = self
            .pointer_events
            .iter()
            .filter_map(|event| {
                let source = moved.get(event.source).copied().flatten()?;
                Some(PointerEvent { source, ..*event })
            })
            .collect();
        input
    }

    pub fn with_event(&self, event: Event) -> Self {
        let mut input = self.clone();
        input.events.push(event);
        input
    }

    pub fn seen_from(&self, window: crate::window::WindowId) -> Self {
        if window.is_main() {
            return self.clone();
        }
        let mut input = self.only(|event| !matches!(event, Event::PointerWindow(_)));
        input.pointer_events.retain(|event| event.window == window);
        for event in &mut input.pointer_events {
            event.window = crate::window::WindowId::MAIN;
        }
        match input.pointer.window == window {
            true => input.pointer.window = crate::window::WindowId::MAIN,
            false => {
                input.pointer.pos = None;
                input.pointer.motion = Vec2::ZERO;
            }
        }
        input.window = crate::window::WindowId::MAIN;
        input
    }

    pub fn without_pointer(&self, leaving: bool) -> Self {
        let mut input = self.only(|event| !event.is_pointer());
        input.pointer_events.clear();
        if leaving {
            let source = 0;
            if let Some(pos) = self.pointer.mouse() {
                input.emit_at(source, PointerId::Mouse, PointerPhase::Gone, pos);
            }
            for (id, pos) in &self.touches {
                input.emit_at(source, PointerId::Touch(*id), PointerPhase::Cancel, *pos);
            }
            if let Some(pos) = self.pen_at {
                input.emit_at(source, PointerId::Pen, PointerPhase::Gone, pos);
            }
        }
        input.pen_at = None;
        input.pointer.pos = None;
        input.pointer.motion = Vec2::ZERO;
        input.touches.clear();
        input
    }

    fn emit_at(&mut self, source: usize, pointer: PointerId, phase: PointerPhase, pos: Pos2) {
        self.source = source;
        self.force = None;
        self.pen = None;
        self.emit(pointer, phase, pos);
    }

    pub fn touching(&self) -> bool {
        !self.touches.is_empty()
    }

    pub(crate) fn begin_frame(&mut self, events: Vec<Event>, now: Instant) {
        self.pointer.motion = Vec2::ZERO;
        if self.touches.is_empty() {
            self.pointer.lift_finger();
        }
        self.pointer_events.clear();
        self.touched = self.touches.iter().map(|(_, pos)| *pos).collect();
        self.touched
            .extend(events.iter().filter_map(|event| match event {
                Event::Touch { pos, .. } => Some(*pos),
                _ => None,
            }));
        for (source, event) in events.iter().enumerate() {
            self.source = source;
            self.time = match events.get(source + 1) {
                Some(Event::Time(at)) => Some(*at),
                _ => Some(now),
            };
            self.force = None;
            self.pen = None;
            if let Some(modifiers) = event.modifiers() {
                self.modifiers = modifiers;
            }
            match event {
                Event::PointerWindow(window) => self.window = *window,
                Event::PointerOutside => self.pointer.outside = true,
                Event::PointerMoved(pos) if !self.on_finger(*pos) => {
                    self.pointer.place_mouse(*pos, self.window);
                    self.emit(PointerId::Mouse, PointerPhase::Move, *pos);
                }
                Event::PointerMotion(delta) => {
                    self.pointer.motion += *delta;
                }
                Event::PointerGone if self.touched.is_empty() => self.mouse_gone(),
                Event::PointersCancelled => {
                    for (id, pos) in std::mem::take(&mut self.touches) {
                        self.emit(PointerId::Touch(id), PointerPhase::Cancel, pos);
                    }
                    self.pointer.lift_finger();
                    self.pen_event(PenPhase::Gone, self.pen_at.unwrap_or_default(), now);
                    self.mouse_gone();
                }
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    ..
                } if !self.on_finger(*pos) => {
                    self.pointer.place_mouse(*pos, self.window);
                    if *pressed {
                        self.pointer.outside = false;
                    }
                    if *pressed && *button == PointerButton::Primary {
                        self.pointer.count_click(*pos, now);
                    }
                    let phase = match pressed {
                        true => PointerPhase::Down(*button),
                        false => PointerPhase::Up(*button),
                    };
                    self.emit(PointerId::Mouse, phase, *pos);
                }
                Event::ScrollStarted => {
                    self.scroll_samples.clear();
                    self.wheel(PointerPhase::ScrollStarted);
                }
                Event::Scroll(delta) => {
                    match self.scroll_samples.back_mut() {
                        Some((when, sum)) if *when == now => *sum += *delta,
                        _ => self.scroll_samples.push_back((now, *delta)),
                    }
                    self.wheel(PointerPhase::Scroll(*delta));
                }
                Event::ScrollEnded => {
                    let fling = self.fling(now);
                    self.wheel(PointerPhase::ScrollEnded { fling });
                }
                Event::Zoom(factor) => self.wheel(PointerPhase::Zoom(*factor)),
                Event::Touch {
                    id,
                    phase,
                    pos,
                    force,
                } => {
                    self.force = *force;
                    self.touch(*id, *phase, *pos, now);
                }
                Event::Pen { phase, pos, pen } => {
                    self.pen = Some(*pen);
                    self.pen_event(*phase, *pos, now);
                }
                Event::Predicted(pos) => {
                    let stamped = events[..source]
                        .iter()
                        .rev()
                        .take_while(|event| matches!(event, Event::Time(_)))
                        .count();
                    self.predict(source - stamped, *pos);
                }
                _ => {}
            }
        }
        self.events = events;
        while self
            .scroll_samples
            .front()
            .is_some_and(|(when, _)| now.duration_since(*when) > WHEEL_VELOCITY_WINDOW)
        {
            self.scroll_samples.pop_front();
        }
    }

    fn on_finger(&self, pos: Pos2) -> bool {
        self.touched
            .iter()
            .any(|touched| touched.distance(pos) <= SYNTHESIZED_SLOP)
    }

    fn wheel(&mut self, phase: PointerPhase) {
        if let Some(pos) = self.pointer.mouse() {
            self.emit(PointerId::Mouse, phase, pos);
        }
    }

    fn emit(&mut self, pointer: PointerId, phase: PointerPhase, pos: Pos2) {
        self.pointer_events.push(PointerEvent {
            pointer,
            window: self.window,
            phase,
            pos,
            modifiers: self.modifiers,
            clicks: self.pointer.clicks(),
            force: self.force,
            pen: self.pen,
            predicted: None,
            time: self.time.unwrap_or_else(crate::timer::now),
            source: self.source,
        });
    }

    fn predict(&mut self, source: usize, pos: Pos2) {
        if let Some(moved) = self
            .pointer_events
            .last_mut()
            .filter(|moved| moved.phase == PointerPhase::Move && moved.source + 1 == source)
        {
            moved.predicted = Some(pos);
        }
    }

    fn pen_event(&mut self, phase: PenPhase, pos: Pos2, now: Instant) {
        let phase = match phase {
            PenPhase::Gone => {
                let Some(at) = self.pen_at.take() else {
                    return;
                };
                if self.pointer.mouse() == Some(at) {
                    self.pointer.pos = None;
                }
                self.emit(PointerId::Pen, PointerPhase::Gone, at);
                return;
            }
            PenPhase::Move => PointerPhase::Move,
            PenPhase::Down(button) => {
                if button == PointerButton::Primary {
                    self.pointer.count_click(pos, now);
                }
                PointerPhase::Down(button)
            }
            PenPhase::Up(button) => PointerPhase::Up(button),
        };
        self.pen_at = Some(pos);
        self.pointer.place_mouse(pos, self.window);
        self.emit(PointerId::Pen, phase, pos);
    }

    fn mouse_gone(&mut self) {
        if let Some(pos) = self.pointer.mouse() {
            self.emit(PointerId::Mouse, PointerPhase::Gone, pos);
            self.pointer.pos = None;
        }
    }

    fn touch(&mut self, id: TouchId, phase: TouchPhase, pos: Pos2, now: Instant) {
        let held = self.touches.iter().position(|(finger, _)| *finger == id);
        let first = self.touches.first().map(|(finger, _)| *finger);
        match (phase, held) {
            (TouchPhase::Start, None) => {
                if self.touches.is_empty() {
                    let pointer = &mut self.pointer;
                    pointer.pos = Some(pos);
                    pointer.window = self.window;
                    pointer.from_touch = true;
                    pointer.count_click(pos, now);
                }
                self.touches.push((id, pos));
                self.emit(
                    PointerId::Touch(id),
                    PointerPhase::Down(PointerButton::Primary),
                    pos,
                );
            }
            (TouchPhase::Move, Some(index)) => {
                self.touches[index].1 = pos;
                if first == Some(id) {
                    self.pointer.pos = Some(pos);
                    self.pointer.window = self.window;
                }
                self.emit(PointerId::Touch(id), PointerPhase::Move, pos);
            }
            (TouchPhase::End | TouchPhase::Cancel, Some(index)) => {
                self.touches.remove(index);
                if first == Some(id) {
                    self.pointer.pos = (phase != TouchPhase::Cancel).then_some(pos);
                }
                let phase = match phase {
                    TouchPhase::End => PointerPhase::Up(PointerButton::Primary),
                    _ => PointerPhase::Cancel,
                };
                self.emit(PointerId::Touch(id), phase, pos);
            }
            _ => {}
        }
    }

    fn fling(&mut self, now: Instant) -> Vec2 {
        let samples = std::mem::take(&mut self.scroll_samples);
        let recent: Vec<(Instant, Vec2)> = samples
            .into_iter()
            .filter(|(when, _)| now.duration_since(*when) <= WHEEL_VELOCITY_WINDOW)
            .collect();
        let (Some((first, _)), Some((last, _))) = (recent.first(), recent.last()) else {
            return Vec2::ZERO;
        };
        let elapsed = last.duration_since(*first).as_secs_f32();
        if elapsed <= f32::EPSILON {
            return Vec2::ZERO;
        }
        let travelled = recent
            .iter()
            .skip(1)
            .fold(Vec2::ZERO, |sum, (_, delta)| sum + *delta);
        travelled * elapsed.recip()
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Pointer {
    pub pos: Option<Pos2>,
    pub window: crate::window::WindowId,
    pub outside: bool,
    pub motion: Vec2,
    clicks: ClickCounter,
    from_touch: bool,
}

impl Pointer {
    fn count_click(&mut self, pos: Pos2, now: Instant) {
        self.clicks.count(pos, now);
    }

    pub fn clicks(&self) -> u32 {
        self.clicks.clicks()
    }

    pub fn interact_pos(&self) -> Option<Pos2> {
        self.pos
    }

    pub fn from_touch(&self) -> bool {
        self.from_touch
    }

    pub fn mouse(&self) -> Option<Pos2> {
        self.pos.filter(|_| !self.from_touch)
    }

    fn place_mouse(&mut self, pos: Pos2, window: crate::window::WindowId) {
        self.pos = Some(pos);
        self.window = window;
        self.from_touch = false;
    }

    fn lift_finger(&mut self) {
        if self.from_touch {
            self.pos = None;
            self.from_touch = false;
        }
    }
}
