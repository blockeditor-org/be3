use std::time::{Duration, Instant};

use beui_view::types::input::{Input, Key, Modifiers, PointerId, typing};
use beui_view::types::{Pos2, Rect, Vec2};

use super::compose::{Composed, compose};
use super::layout::{Action, KeyAt, KeyDef, Layer, Layout, Modifier, Slider, Slot, Value, layout};
use super::tables;

pub const LONG_PRESS: Duration = Duration::from_millis(500);
pub const REPEAT: Duration = Duration::from_millis(65);
const CIRCLE_SENSITIVITY: i32 = 2;
const SWIPE_SHARE: f32 = 0.35;
const SLIDE_SHARE: f32 = 0.3;
const WORD_STEPS: f32 = 2.5;

const DIRECTION_TO_SLOT: [Slot; 16] = [
    Slot::N,
    Slot::Ne,
    Slot::Ne,
    Slot::E,
    Slot::E,
    Slot::Se,
    Slot::Se,
    Slot::S,
    Slot::S,
    Slot::Sw,
    Slot::Sw,
    Slot::W,
    Slot::W,
    Slot::Nw,
    Slot::Nw,
    Slot::N,
];

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
    pub function: bool,
    pub superscript: bool,
    pub subscript: bool,
}

impl Mods {
    pub fn has(self, modifier: Modifier) -> bool {
        match modifier {
            Modifier::Shift => self.shift,
            Modifier::Ctrl => self.ctrl,
            Modifier::Alt => self.alt,
            Modifier::Meta => self.meta,
            Modifier::Fn => self.function,
            Modifier::Superscript => self.superscript,
            Modifier::Subscript => self.subscript,
        }
    }

    fn set(&mut self, modifier: Modifier) {
        match modifier {
            Modifier::Shift => self.shift = true,
            Modifier::Ctrl => self.ctrl = true,
            Modifier::Alt => self.alt = true,
            Modifier::Meta => self.meta = true,
            Modifier::Fn => self.function = true,
            Modifier::Superscript => self.superscript = true,
            Modifier::Subscript => self.subscript = true,
        }
    }

    fn chord(self) -> Modifiers {
        Modifiers {
            shift: self.shift,
            ctrl: self.ctrl,
            alt: self.alt,
            logo: self.meta,
        }
    }

    fn commands(self) -> bool {
        self.ctrl || self.alt || self.meta
    }
}

pub fn modify(value: Value, mods: Mods) -> Value {
    let mut value = value;
    if mods.function {
        value = tables::function(value);
    }
    if mods.superscript {
        value = tables::superscript(value);
    } else if mods.subscript {
        value = tables::subscript(value);
    }
    if mods.shift {
        value = tables::shifted(value);
    }
    value
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Latch {
    Off,
    Latched,
    Locked,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GestureState {
    Cancelled,
    Swiped,
    Clockwise,
    Anticlockwise,
    EndedCenter,
    EndedClockwise,
    EndedAnticlockwise,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GestureName {
    None,
    Swipe,
    Roundtrip,
    Circle,
}

#[derive(Clone, Copy, Debug)]
struct Gesture {
    direction: i32,
    state: GestureState,
}

impl Gesture {
    fn new(direction: i32) -> Self {
        Self {
            direction,
            state: GestureState::Swiped,
        }
    }

    fn name(self) -> GestureName {
        match self.state {
            GestureState::Cancelled => GestureName::None,
            GestureState::Swiped => GestureName::Swipe,
            GestureState::EndedCenter => GestureName::Roundtrip,
            GestureState::Clockwise
            | GestureState::Anticlockwise
            | GestureState::EndedClockwise
            | GestureState::EndedAnticlockwise => GestureName::Circle,
        }
    }

    fn in_progress(self) -> bool {
        matches!(
            self.state,
            GestureState::Swiped | GestureState::Clockwise | GestureState::Anticlockwise
        )
    }

    fn turned(&mut self, direction: i32) -> bool {
        let difference = direction_difference(self.direction, direction);
        let clockwise = difference > 0;
        match self.state {
            GestureState::Swiped => {
                if difference.abs() < CIRCLE_SENSITIVITY {
                    return false;
                }
                self.state = match clockwise {
                    true => GestureState::Clockwise,
                    false => GestureState::Anticlockwise,
                };
                self.direction = direction;
                true
            }
            GestureState::Clockwise | GestureState::Anticlockwise => {
                self.direction = direction;
                if (self.state == GestureState::Clockwise) == clockwise {
                    return false;
                }
                self.state = GestureState::Cancelled;
                true
            }
            _ => false,
        }
    }

    fn centered(&mut self) -> bool {
        match self.state {
            GestureState::Swiped => {
                self.state = GestureState::EndedCenter;
                true
            }
            GestureState::Clockwise => {
                self.state = GestureState::EndedClockwise;
                false
            }
            GestureState::Anticlockwise => {
                self.state = GestureState::EndedAnticlockwise;
                false
            }
            _ => false,
        }
    }
}

fn direction_difference(from: i32, to: i32) -> i32 {
    if from == to {
        return 0;
    }
    let left = (from - to + 16) % 16;
    let right = (to - from + 16) % 16;
    match left < right {
        true => -left,
        false => right,
    }
}

fn direction_of(delta: Vec2) -> i32 {
    let angle = f64::from(delta.y).atan2(f64::from(delta.x)) + std::f64::consts::PI;
    ((angle * 8.0 / std::f64::consts::PI) as i32 + 12) % 16
}

#[derive(Clone, Copy, Debug)]
struct Sliding {
    slider: Slider,
    start: Option<f32>,
    steps: i32,
}

#[derive(Clone, Copy, Debug)]
struct Pointer {
    id: PointerId,
    key: KeyAt,
    down: Pos2,
    value: Option<Value>,
    capital: bool,
    gesture: Option<Gesture>,
    mods: Mods,
    held_since: Instant,
    repeat_at: Option<Instant>,
    repeated: bool,
    sliding: Option<Sliding>,
    used: bool,
}

impl Pointer {
    fn modifier(&self, def: &KeyDef) -> Option<Modifier> {
        match (def.value(Slot::Center), self.gesture) {
            (Some(Value::Modifier(modifier)), None) => Some(modifier),
            _ => None,
        }
    }
}

pub struct Engine {
    layer: Layer,
    pointers: Vec<Pointer>,
    latched: Vec<(Modifier, bool)>,
    compose: Option<String>,
    events: Vec<Input>,
    paste: bool,
    swipe: f32,
    step: f32,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            layer: Layer::Text,
            pointers: Vec::new(),
            latched: Vec::new(),
            compose: None,
            events: Vec::new(),
            paste: false,
            swipe: 14.0,
            step: 12.0,
        }
    }
}

impl Engine {
    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn layout(&self) -> &'static Layout {
        layout(self.layer)
    }

    pub fn set_layer(&mut self, layer: Layer) {
        self.layer = layer;
        self.pointers.clear();
    }

    pub fn composing(&self) -> bool {
        self.compose.is_some()
    }

    pub fn sequence(&self) -> Option<&str> {
        self.compose.as_deref()
    }

    pub fn set_key_size(&mut self, size: Vec2) {
        let side = size.x.min(size.y).max(1.0);
        self.swipe = side * SWIPE_SHARE;
        self.step = size.x.max(1.0) * SLIDE_SHARE;
    }

    pub fn latch(&self, modifier: Modifier) -> Latch {
        match self.latched.iter().find(|(held, _)| *held == modifier) {
            Some((_, true)) => Latch::Locked,
            Some((_, false)) => Latch::Latched,
            None => Latch::Off,
        }
    }

    pub fn mods(&self) -> Mods {
        let mut mods = Mods::default();
        for (modifier, _) in &self.latched {
            mods.set(*modifier);
        }
        let layout = self.layout();
        for pointer in &self.pointers {
            if let Some(modifier) = layout
                .key(pointer.key)
                .and_then(|def| pointer.modifier(def))
            {
                mods.set(modifier);
            }
        }
        mods
    }

    pub fn held(&self, modifier: Modifier) -> bool {
        let layout = self.layout();
        self.pointers.iter().any(|pointer| {
            layout
                .key(pointer.key)
                .and_then(|def| pointer.modifier(def))
                == Some(modifier)
        })
    }

    pub fn pressed(&self, key: KeyAt) -> Option<Option<Value>> {
        self.pointers
            .iter()
            .rev()
            .find(|pointer| pointer.key == key)
            .map(|pointer| pointer.value.map(|value| self.shown(pointer, value)))
    }

    fn shown(&self, pointer: &Pointer, value: Value) -> Value {
        let mut mods = pointer.mods;
        mods.shift |= pointer.capital;
        modify(value, mods)
    }

    pub fn take_events(&mut self) -> Vec<Input> {
        std::mem::take(&mut self.events)
    }

    pub fn take_paste(&mut self) -> bool {
        std::mem::take(&mut self.paste)
    }

    pub fn hit(layout: &Layout, rect: Rect, pos: Pos2) -> Option<KeyAt> {
        let units = layout.height();
        if units <= 0.0 || !rect.is_positive() {
            return None;
        }
        let local = pos - rect.min;
        let mut top = 0.0;
        let rows = layout.rows.len();
        for (row, line) in layout.rows.iter().enumerate() {
            let bottom = top + line.height / units * rect.height();
            if local.y < bottom || row + 1 == rows {
                let width = layout.width();
                let mut left = 0.0;
                let columns = line.keys.len();
                for (column, def) in line.keys.iter().enumerate() {
                    left += def.shift / width * rect.width();
                    let right = left + def.width / width * rect.width();
                    if local.x < right || column + 1 == columns {
                        return Some(KeyAt { row, column });
                    }
                    left = right;
                }
                return None;
            }
            top = bottom;
        }
        None
    }

    pub fn press(&mut self, id: PointerId, key: KeyAt, pos: Pos2, now: Instant) {
        self.pointers.retain(|pointer| pointer.id != id);
        let Some(def) = self.layout().key(key).copied() else {
            return;
        };
        let mods = self.mods();
        let value = def.value(Slot::Center);
        let normal = !value.is_some_and(Value::is_modifier);
        if normal {
            let layout = self.layout();
            for pointer in &mut self.pointers {
                if layout
                    .key(pointer.key)
                    .and_then(|def| pointer.modifier(def))
                    .is_some()
                {
                    pointer.used = true;
                }
            }
        }
        self.pointers.push(Pointer {
            id,
            key,
            down: pos,
            value,
            capital: false,
            gesture: None,
            mods,
            held_since: now,
            repeat_at: None,
            repeated: false,
            sliding: None,
            used: false,
        });
    }

    pub fn moved(&mut self, id: PointerId, pos: Pos2, now: Instant) {
        let Some(index) = self.pointers.iter().position(|pointer| pointer.id == id) else {
            return;
        };
        let layout = self.layout();
        let swipe = self.swipe;
        let mut pointer = self.pointers[index];
        let Some(def) = layout.key(pointer.key).copied() else {
            return;
        };
        if let Some(sliding) = pointer.sliding {
            self.pointers[index].sliding = Some(self.slide(sliding, pointer, pos));
            return;
        }
        let delta = pos - pointer.down;
        let distance = delta.x.abs() + delta.y.abs();
        if distance < swipe {
            let Some(mut gesture) = pointer.gesture.filter(|gesture| gesture.in_progress()) else {
                return;
            };
            gesture.centered();
            pointer.gesture = Some(gesture);
            self.apply_gesture(&mut pointer, &def, gesture);
            self.pointers[index] = pointer;
            return;
        }
        let direction = direction_of(delta);
        match pointer.gesture {
            None => {
                pointer.gesture = Some(Gesture::new(direction));
                if let Some(value) = nearest(&def, direction, pointer.mods) {
                    pointer.value = Some(value);
                    pointer.held_since = now;
                    if let Value::Slider(slider) = value {
                        pointer.sliding = Some(Sliding {
                            slider,
                            start: None,
                            steps: 0,
                        });
                    }
                }
            }
            Some(mut gesture) => {
                if gesture.turned(direction) && gesture.in_progress() {
                    self.apply_gesture(&mut pointer, &def, gesture);
                    pointer.held_since = now;
                }
                pointer.gesture = Some(gesture);
            }
        }
        self.pointers[index] = pointer;
    }

    fn apply_gesture(&self, pointer: &mut Pointer, def: &KeyDef, gesture: Gesture) {
        match gesture.name() {
            GestureName::None | GestureName::Swipe => {}
            GestureName::Roundtrip => {
                pointer.value = nearest(def, gesture.direction, pointer.mods)
                    .filter(|value| !matches!(value, Value::Slider(_)));
                pointer.capital = true;
            }
            GestureName::Circle => {
                pointer.value = def.value(Slot::Center);
                pointer.capital = true;
            }
        }
    }

    fn slide(&mut self, mut sliding: Sliding, pointer: Pointer, pos: Pos2) -> Sliding {
        let start = match sliding.start {
            Some(start) => start,
            None => {
                let travelled = (pos - pointer.down).x.abs() + (pos - pointer.down).y.abs();
                if travelled < self.swipe + self.step {
                    return sliding;
                }
                sliding.start = Some(pos.x);
                pos.x
            }
        };
        let step = match sliding.slider {
            Slider::CursorLeft | Slider::CursorRight => self.step,
            Slider::SelectWords => self.step * WORD_STEPS,
        };
        let moved = ((pos.x - start) / step).trunc() as i32;
        let mods = pointer.mods;
        match sliding.slider {
            Slider::CursorLeft | Slider::CursorRight => {
                let difference = moved - sliding.steps;
                let key = match difference > 0 {
                    true => Key::ArrowRight,
                    false => Key::ArrowLeft,
                };
                for _ in 0..difference.abs() {
                    typing::press_chord(key, mods.chord(), Modifiers::NONE, &mut self.events);
                }
                sliding.steps = moved;
            }
            Slider::SelectWords => {
                let words = (-moved - 1).max(0);
                let difference = words - sliding.steps;
                let key = match difference > 0 {
                    true => Key::ArrowLeft,
                    false => Key::ArrowRight,
                };
                let select = Modifiers {
                    shift: true,
                    ctrl: true,
                    ..Modifiers::NONE
                };
                for _ in 0..difference.abs() {
                    typing::press_chord(key, select, Modifiers::NONE, &mut self.events);
                }
                sliding.steps = words;
            }
        }
        sliding
    }

    pub fn released(&mut self, id: PointerId, now: Instant) {
        let Some(index) = self.pointers.iter().position(|pointer| pointer.id == id) else {
            return;
        };
        let pointer = self.pointers.remove(index);
        let Some(def) = self.layout().key(pointer.key).copied() else {
            return;
        };
        if let Some(sliding) = pointer.sliding {
            match (sliding.slider, sliding.start) {
                (Slider::SelectWords, Some(_)) if sliding.steps > 0 => {
                    typing::press_chord(
                        Key::Backspace,
                        Modifiers::NONE,
                        Modifiers::NONE,
                        &mut self.events,
                    );
                    self.clear_latched();
                }
                (Slider::SelectWords, Some(_)) => {}
                (Slider::SelectWords, None) => {
                    self.emit(Value::Key(Key::Backspace), pointer.mods, now)
                }
                (Slider::CursorLeft | Slider::CursorRight, _) => {}
            }
            return;
        }
        if let Some(modifier) = pointer.modifier(&def) {
            if !pointer.used {
                self.toggle_latch(modifier);
            }
            return;
        }
        if pointer.repeated {
            return;
        }
        let Some(value) = pointer.value else {
            return;
        };
        let mut mods = pointer.mods;
        mods.shift |= pointer.capital;
        self.emit(value, mods, now);
    }

    pub fn cancelled(&mut self, id: PointerId) {
        self.pointers.retain(|pointer| pointer.id != id);
    }

    pub fn cancel_all(&mut self) {
        self.pointers.clear();
    }

    pub fn tap(&mut self, value: Value, now: Instant) {
        let mods = self.mods();
        self.emit(value, mods, now);
    }

    pub fn type_text(&mut self, text: &str) {
        typing::typed(text, Modifiers::NONE, &mut self.events);
        self.clear_latched();
    }

    pub fn tick(&mut self, now: Instant) -> Option<Instant> {
        let layout = self.layout();
        let mut next: Option<Instant> = None;
        for index in 0..self.pointers.len() {
            let pointer = self.pointers[index];
            if pointer.sliding.is_some() {
                continue;
            }
            let Some(def) = layout.key(pointer.key) else {
                continue;
            };
            if let Some(modifier) = pointer.modifier(def) {
                if pointer.used {
                    continue;
                }
                match now >= pointer.held_since + LONG_PRESS {
                    true => {
                        self.latched.retain(|(held, _)| *held != modifier);
                        self.latched.push((modifier, true));
                        self.pointers[index].used = true;
                    }
                    false => next = earliest(next, pointer.held_since + LONG_PRESS),
                }
                continue;
            }
            let Some(value) = pointer
                .value
                .filter(|value| modify(*value, pointer.mods).repeats())
            else {
                continue;
            };
            let due = pointer.repeat_at.unwrap_or(pointer.held_since + LONG_PRESS);
            if now >= due {
                let mut mods = pointer.mods;
                mods.shift |= pointer.capital;
                self.emit(value, mods, now);
                if let Some(pointer) = self.pointers.get_mut(index) {
                    pointer.repeated = true;
                    pointer.repeat_at = Some(now + REPEAT);
                }
                next = earliest(next, now + REPEAT);
            } else {
                next = earliest(next, due);
            }
        }
        next
    }

    fn toggle_latch(&mut self, modifier: Modifier) {
        match self.latch(modifier) {
            Latch::Off => self.latched.push((modifier, false)),
            Latch::Latched | Latch::Locked => self.latched.retain(|(held, _)| *held != modifier),
        }
    }

    fn clear_latched(&mut self) {
        self.latched.retain(|(_, locked)| *locked);
    }

    fn emit(&mut self, value: Value, mods: Mods, _now: Instant) {
        let value = modify(value, mods);
        match value {
            Value::Char(character) => {
                self.type_char(character, mods);
                self.clear_latched();
            }
            Value::Key(key) => {
                if self.compose.take().is_none() || key != Key::Escape {
                    typing::press_chord(key, mods.chord(), Modifiers::NONE, &mut self.events);
                }
                self.clear_latched();
            }
            Value::Modifier(modifier) => self.toggle_latch(modifier),
            Value::CapsLock => match self.latch(Modifier::Shift) {
                Latch::Locked => self.latched.retain(|(held, _)| *held != Modifier::Shift),
                _ => {
                    self.latched.retain(|(held, _)| *held != Modifier::Shift);
                    self.latched.push((Modifier::Shift, true));
                }
            },
            Value::Action(action) => {
                self.act(action);
                self.clear_latched();
            }
            Value::Layer(layer) => {
                self.layer = layer;
                self.pointers.clear();
            }
            Value::Compose => {
                self.compose = match self.compose {
                    Some(_) => None,
                    None => Some(String::new()),
                };
            }
            Value::Slider(_) => {}
        }
    }

    fn type_char(&mut self, character: char, mods: Mods) {
        if let Some(mut sequence) = self.compose.take() {
            sequence.push(character);
            match compose(&sequence) {
                Composed::Pending => self.compose = Some(sequence),
                Composed::Done(result) => typing::typed(result, Modifiers::NONE, &mut self.events),
                Composed::Failed => {}
            }
            return;
        }
        if mods.commands()
            && let Some((key, shift)) = typing::keyable(character.to_ascii_lowercase())
        {
            let chord = Modifiers {
                shift: shift || character.is_ascii_uppercase(),
                ..mods.chord()
            };
            typing::press_chord(key, chord, Modifiers::NONE, &mut self.events);
            return;
        }
        typing::typed(&character.to_string(), Modifiers::NONE, &mut self.events);
    }

    fn act(&mut self, action: Action) {
        let ctrl = Modifiers::CTRL;
        let (key, modifiers) = match action {
            Action::Copy => (Key::C, ctrl),
            Action::Paste => (Key::V, ctrl),
            Action::SelectAll => (Key::A, ctrl),
            Action::Undo => (Key::Z, ctrl),
            Action::Redo => (
                Key::Z,
                Modifiers {
                    shift: true,
                    ..ctrl
                },
            ),
            Action::DeleteWordLeft => (Key::Backspace, ctrl),
            Action::DeleteWordRight => (Key::Delete, ctrl),
            Action::PastePlain => {
                self.paste = true;
                return;
            }
        };
        typing::press_chord(key, modifiers, Modifiers::NONE, &mut self.events);
    }
}

fn earliest(held: Option<Instant>, due: Instant) -> Option<Instant> {
    Some(held.map_or(due, |held| held.min(due)))
}

fn nearest(def: &KeyDef, direction: i32, _mods: Mods) -> Option<Value> {
    for offset in [0, -1, 1, -2, 2, -3, 3] {
        let slot = DIRECTION_TO_SLOT[((direction + offset + 16) % 16) as usize];
        if let Some(value) = def.value(slot) {
            if matches!(value, Value::Slider(_)) && offset.abs() >= 2 {
                continue;
            }
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
mod tests;
