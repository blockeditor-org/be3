mod keyboard;

use std::array;
use std::time::{Duration, Instant};

use crate::color::Color32;
use crate::font::FontId;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::icons;
use crate::input::{Event, Modifiers, PointerButton, RawInput, TouchId, TouchPhase};
use crate::painter::Painter;
use crate::styled::Theme;
use crate::styled::theme::{CARD_RADIUS, ICON_SIZE};

use keyboard::{KeyId, Keyboard};

const BAR_HEIGHT: f32 = 56.0;
const BAR_PADDING: f32 = 6.0;
const BAR_SPACING: f32 = 6.0;
const BAR_WIDTH: f32 = 520.0;
const CURSOR_SIZE: f32 = 26.0;
const CURSOR_SHADOW: f32 = 1.5;
const TAP_DISTANCE: f32 = 10.0;
const TAP_TIME: Duration = Duration::from_millis(300);
const DOUBLE_TAP_TIME: Duration = Duration::from_millis(350);
const MIDDLE_HOLD: Duration = Duration::from_millis(180);
pub(crate) const SCROLL_TICK: f32 = 22.0;
pub(crate) const WHEEL_LINE: f32 = 40.0;
const SURFACE: Color32 = Color32::from_rgba_unmultiplied(14, 17, 23, 232);
const SHADOW: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 190);

const BUTTONS: [PointerButton; 3] = [
    PointerButton::Primary,
    PointerButton::Middle,
    PointerButton::Secondary,
];
const BUTTON_ICONS: [&str; 4] = [
    icons::ICON_LEFT_CLICK,
    icons::ICON_MOUSE,
    icons::ICON_RIGHT_CLICK,
    icons::ICON_KEYBOARD,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Trackpad,
    Button(usize),
    Keyboard,
    Key(KeyId),
    Ignored,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Middle {
    Up,
    Pending(Instant),
    Held,
    Scrolling,
}

struct Finger {
    role: Role,
    origin: Pos2,
    last: Pos2,
    started: Instant,
    moved: bool,
}

struct Layout {
    bar: Rect,
    buttons: [Rect; 4],
    keyboard: Option<Rect>,
    trackpad: Rect,
}

pub(crate) struct MouseSimulation {
    enabled: bool,
    active: bool,
    scale: f32,
    viewport: Rect,
    cursor: Pos2,
    reported: Option<Pos2>,
    fingers: Vec<(TouchId, Finger)>,
    holders: [Option<TouchId>; BUTTONS.len()],
    emitted: [bool; BUTTONS.len()],
    clicks: Vec<usize>,
    middle: Middle,
    middle_travel: f32,
    scroll: Vec2,
    dragging: bool,
    scrolling: bool,
    last_tap: Option<Instant>,
    keyboard_open: bool,
    keyboard: Keyboard,
    painted: Rect,
}

impl Default for MouseSimulation {
    fn default() -> Self {
        Self {
            enabled: false,
            active: false,
            scale: 1.0,
            viewport: Rect::NOTHING,
            cursor: Pos2::ZERO,
            reported: None,
            fingers: Vec::new(),
            holders: [None; BUTTONS.len()],
            emitted: [false; BUTTONS.len()],
            clicks: Vec::new(),
            middle: Middle::Up,
            middle_travel: 0.0,
            scroll: Vec2::ZERO,
            dragging: false,
            scrolling: false,
            last_tap: None,
            keyboard_open: false,
            keyboard: Keyboard::default(),
            painted: Rect::NOTHING,
        }
    }
}

impl MouseSimulation {
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub(crate) fn painting(&self) -> bool {
        self.enabled || self.painted.is_positive()
    }

    pub(crate) fn reserved(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let layout = self.layout();
        layout.bar.height() + layout.keyboard.map_or(0.0, |rect| rect.height())
    }

    pub(crate) fn measure(&mut self, viewport: Rect, scale: f32) {
        self.viewport = viewport;
        self.scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
    }

    pub(crate) fn translate(&mut self, raw: RawInput) -> (RawInput, Option<Duration>) {
        if !self.enabled && !self.active {
            return (raw, None);
        }
        let mut out = Vec::with_capacity(raw.events.len() + 8);
        if !self.enabled {
            self.stop(&mut out);
            out.extend(raw.events);
            return (RawInput { events: out }, None);
        }
        if !self.active {
            self.start();
        }
        for event in raw.events {
            match event {
                Event::Touch { id, phase, pos, .. } => self.touch(id, phase, pos),
                Event::Focus(false) => {
                    self.cancel();
                    out.push(Event::Focus(false));
                }
                other => out.push(other),
            }
        }
        let wake = self.settle();
        self.flush(&mut out);
        (RawInput { events: out }, wake)
    }

    fn start(&mut self) {
        self.active = true;
        self.cursor = if self.viewport.is_positive() {
            self.viewport.center()
        } else {
            Pos2::ZERO
        };
        self.reported = None;
    }

    fn stop(&mut self, out: &mut Vec<Event>) {
        self.cancel();
        self.flush(out);
        if self.reported.take().is_some() {
            out.push(Event::PointerGone);
        }
        self.keyboard_open = false;
        self.keyboard.clear();
        self.active = false;
    }

    fn cancel(&mut self) {
        self.fingers.clear();
        self.holders = [None; BUTTONS.len()];
        self.middle = Middle::Up;
        self.middle_travel = 0.0;
        self.dragging = false;
        self.scrolling = false;
        self.last_tap = None;
        self.keyboard.clear();
    }

    fn settle(&mut self) -> Option<Duration> {
        let Middle::Pending(since) = self.middle else {
            return None;
        };
        let elapsed = since.elapsed();
        if elapsed >= MIDDLE_HOLD {
            self.middle = Middle::Held;
            return None;
        }
        Some(MIDDLE_HOLD - elapsed)
    }

    fn flush(&mut self, out: &mut Vec<Event>) {
        let modifiers = self.keyboard.modifiers();
        if self.reported != Some(self.cursor) {
            self.reported = Some(self.cursor);
            out.push(Event::PointerMoved(self.cursor));
        }
        for index in 0..BUTTONS.len() {
            let desired = self.held(index);
            if desired != self.emitted[index] {
                self.emitted[index] = desired;
                out.push(self.button_event(index, desired, modifiers));
            }
        }
        for index in std::mem::take(&mut self.clicks) {
            if self.emitted[index] {
                continue;
            }
            out.push(self.button_event(index, true, modifiers));
            out.push(self.button_event(index, false, modifiers));
        }
        if self.scroll != Vec2::ZERO {
            out.push(Event::Modifiers(modifiers));
            out.push(Event::Scroll(std::mem::replace(
                &mut self.scroll,
                Vec2::ZERO,
            )));
        }
        self.keyboard.drain(out);
    }

    fn button_event(&self, index: usize, pressed: bool, modifiers: Modifiers) -> Event {
        Event::PointerButton {
            pos: self.cursor,
            button: BUTTONS[index],
            pressed,
            modifiers,
        }
    }

    fn held(&self, index: usize) -> bool {
        match index {
            0 => self.holders[0].is_some() || self.dragging,
            1 => self.middle == Middle::Held,
            _ => self.holders[index].is_some(),
        }
    }

    fn touch(&mut self, id: TouchId, phase: TouchPhase, pos: Pos2) {
        match phase {
            TouchPhase::Start => self.begin(id, pos),
            TouchPhase::Move => self.advance(id, pos),
            TouchPhase::End => self.finish(id, pos, false),
            TouchPhase::Cancel => self.finish(id, pos, true),
        }
    }

    fn begin(&mut self, id: TouchId, pos: Pos2) {
        let role = self.role(self.local(pos));
        match role {
            Role::Button(1) => {
                self.holders[1] = Some(id);
                self.middle = Middle::Pending(Instant::now());
                self.middle_travel = 0.0;
            }
            Role::Button(index) => self.holders[index] = Some(id),
            Role::Keyboard => {
                self.keyboard_open = !self.keyboard_open;
                self.keyboard.clear();
            }
            Role::Key(key) => self.keyboard.press(key),
            Role::Trackpad => {
                if self.trackpad_fingers() > 0 {
                    self.scrolling = true;
                    self.dragging = false;
                } else if self
                    .last_tap
                    .is_some_and(|at| at.elapsed() <= DOUBLE_TAP_TIME)
                {
                    self.dragging = true;
                    self.last_tap = None;
                }
            }
            Role::Ignored => {}
        }
        self.fingers.push((
            id,
            Finger {
                role,
                origin: pos,
                last: pos,
                started: Instant::now(),
                moved: false,
            },
        ));
    }

    fn advance(&mut self, id: TouchId, pos: Pos2) {
        let scale = self.scale;
        let count = self.trackpad_fingers().max(1) as f32;
        let Some(index) = self.fingers.iter().position(|(other, _)| *other == id) else {
            return;
        };
        let (role, delta, moved) = {
            let finger = &mut self.fingers[index].1;
            let delta = pos - finger.last;
            finger.last = pos;
            if !finger.moved && finger.origin.distance(pos) >= TAP_DISTANCE * scale {
                finger.moved = true;
            }
            (finger.role, delta, finger.moved)
        };
        match role {
            Role::Trackpad => {
                if self.scrolling {
                    self.scroll = self.scroll + delta * count.recip();
                } else {
                    self.move_cursor(delta);
                }
            }
            Role::Button(1) => {
                if moved && matches!(self.middle, Middle::Pending(_)) {
                    self.middle = Middle::Scrolling;
                }
                self.middle_travel += delta.y;
                let tick = SCROLL_TICK * scale;
                while self.middle_travel.abs() >= tick {
                    let direction = self.middle_travel.signum();
                    self.middle_travel -= direction * tick;
                    self.scroll = self.scroll + vec2(0.0, direction * WHEEL_LINE);
                }
            }
            _ => {}
        }
    }

    fn finish(&mut self, id: TouchId, pos: Pos2, cancelled: bool) {
        self.advance(id, pos);
        let Some(index) = self.fingers.iter().position(|(other, _)| *other == id) else {
            return;
        };
        let (_, finger) = self.fingers.remove(index);
        let quick = !cancelled && !finger.moved && finger.started.elapsed() <= TAP_TIME;
        match finger.role {
            Role::Button(1) => {
                self.holders[1] = None;
                if quick && matches!(self.middle, Middle::Pending(_)) {
                    self.clicks.push(1);
                }
                self.middle = Middle::Up;
                self.middle_travel = 0.0;
            }
            Role::Button(button) => self.holders[button] = None,
            Role::Key(key) => self.keyboard.release(key),
            Role::Trackpad => {
                if self.trackpad_fingers() > 0 {
                    return;
                }
                if self.scrolling {
                    self.scrolling = false;
                } else if self.dragging {
                    self.dragging = false;
                    if quick {
                        self.last_tap = Some(Instant::now());
                    }
                } else if quick {
                    self.clicks.push(0);
                    self.last_tap = Some(Instant::now());
                }
            }
            Role::Keyboard | Role::Ignored => {}
        }
    }

    fn move_cursor(&mut self, delta: Vec2) {
        let bounds = self.viewport;
        self.cursor = pos2(
            (self.cursor.x + delta.x).clamp(bounds.left(), bounds.right()),
            (self.cursor.y + delta.y).clamp(bounds.top(), bounds.bottom()),
        );
    }

    fn trackpad_fingers(&self) -> usize {
        self.fingers
            .iter()
            .filter(|(_, finger)| finger.role == Role::Trackpad)
            .count()
    }

    fn role(&self, pos: Pos2) -> Role {
        let layout = self.layout();
        if let Some(index) = layout
            .buttons
            .iter()
            .position(|button| button.contains(pos))
        {
            return match index {
                3 => Role::Keyboard,
                index => Role::Button(index),
            };
        }
        if let Some(rect) = layout.keyboard {
            if let Some(key) = Keyboard::hit(rect, pos) {
                return Role::Key(key);
            }
            if rect.contains(pos) {
                return Role::Ignored;
            }
        }
        if layout.bar.contains(pos) {
            return Role::Ignored;
        }
        if layout.trackpad.contains(pos) {
            return Role::Trackpad;
        }
        Role::Ignored
    }

    #[cfg(test)]
    pub(crate) fn cursor(&self) -> Pos2 {
        self.cursor
    }

    #[cfg(test)]
    pub(crate) fn button_center(&self, index: usize) -> Pos2 {
        self.document(self.layout().buttons[index].center())
    }

    #[cfg(test)]
    pub(crate) fn trackpad_center(&self) -> Pos2 {
        self.document(self.layout().trackpad.center())
    }

    #[cfg(test)]
    pub(crate) fn key_center(&self, label: &str) -> Option<Pos2> {
        let rect = self.layout().keyboard?;
        Keyboard::locate(rect, label).map(|pos| self.document(pos))
    }

    #[cfg(test)]
    fn document(&self, pos: Pos2) -> Pos2 {
        pos2(pos.x * self.scale, pos.y * self.scale)
    }

    fn bounds(&self) -> Rect {
        self.viewport.scaled(self.scale.recip())
    }

    fn local(&self, pos: Pos2) -> Pos2 {
        pos2(pos.x / self.scale, pos.y / self.scale)
    }

    fn layout(&self) -> Layout {
        let bounds = self.bounds();
        let bar_height = BAR_HEIGHT.min(bounds.height() / 3.0).max(0.0);
        let bar = Rect::from_min_max(
            pos2(bounds.left(), bounds.bottom() - bar_height),
            bounds.max,
        );
        let keyboard = self.keyboard_open.then(|| {
            let height = Keyboard::height().min((bounds.height() - bar_height).max(0.0));
            Rect::from_min_max(
                pos2(bounds.left(), bar.top() - height),
                pos2(bounds.right(), bar.top()),
            )
        });
        let inner = bar.shrink(BAR_PADDING);
        let width = inner.width().min(BAR_WIDTH);
        let slot = ((width - BAR_SPACING * 3.0) / 4.0).max(0.0);
        let left = inner.center().x - width / 2.0;
        let buttons = array::from_fn(|index| {
            Rect::from_min_size(
                pos2(left + index as f32 * (slot + BAR_SPACING), inner.top()),
                vec2(slot, inner.height().max(0.0)),
            )
        });
        let trackpad = Rect::from_min_max(
            bounds.min,
            pos2(
                bounds.right(),
                keyboard.map_or(bar.top(), |rect| rect.top()),
            ),
        );
        Layout {
            bar,
            buttons,
            keyboard,
            trackpad,
        }
    }

    pub(crate) fn paint(&mut self, painter: &Painter) -> Rect {
        if !self.enabled || !self.bounds().is_positive() {
            let damage = self.painted;
            self.painted = Rect::NOTHING;
            return damage;
        }
        let layout = self.layout();
        painter.rect_filled(layout.bar, 0.0, SURFACE);
        for (index, bounds) in layout.buttons.iter().enumerate() {
            let active = match index {
                3 => self.keyboard_open,
                index => self.held(index) || self.holders[index].is_some(),
            };
            let fill = if active {
                Theme::DARK.accent
            } else {
                Theme::DARK.surface_raised
            };
            let color = if active {
                Theme::DARK.on_accent
            } else {
                Theme::DARK.text
            };
            painter.rect_filled(*bounds, f32::from(CARD_RADIUS), fill);
            glyph(
                painter,
                bounds.center(),
                BUTTON_ICONS[index],
                ICON_SIZE,
                color,
            );
        }
        if let Some(rect) = layout.keyboard {
            self.keyboard.paint(painter, rect);
        }
        let cursor = self.paint_cursor(painter);
        let damage = layout
            .bar
            .union(layout.keyboard.unwrap_or(Rect::NOTHING))
            .union(cursor)
            .union(self.painted);
        self.painted = layout
            .bar
            .union(layout.keyboard.unwrap_or(Rect::NOTHING))
            .union(cursor);
        damage
    }

    fn paint_cursor(&self, painter: &Painter) -> Rect {
        let at = self.local(self.cursor);
        let color = if self.held(0) {
            Theme::DARK.accent
        } else {
            Theme::DARK.knob
        };
        let font = FontId::icons(CURSOR_SIZE);
        let galley = painter.layout(icons::ICON_ARROW_SELECTOR_TOOL, font, f32::INFINITY);
        let origin = at - vec2(CURSOR_SIZE, CURSOR_SIZE) * 0.25;
        painter.galley(
            origin + vec2(CURSOR_SHADOW, CURSOR_SHADOW),
            galley.clone(),
            SHADOW,
        );
        painter.galley(origin, galley, color);
        Rect::from_min_size(origin, vec2(CURSOR_SIZE, CURSOR_SIZE)).expand(CURSOR_SHADOW * 2.0)
    }
}

fn glyph(painter: &Painter, center: Pos2, text: &str, size: f32, color: Color32) {
    let galley = painter.layout(text, FontId::icons(size), f32::INFINITY);
    let extent = galley.size();
    painter.galley(center - extent * 0.5, galley, color);
}
