pub mod keyboard;

use std::array;
use std::time::{Duration, Instant};

use beui_components_styled::Theme;
use beui_components_styled::theme::{CARD_RADIUS, ICON_SIZE};
use beui_core::color::Color32;
use beui_core::context::{Context, InputSimulation};
use beui_core::font::FontId;
use beui_core::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use beui_core::icons;
use beui_core::input::{
    CursorIcon, Event, Modifiers, PointerButton, RawInput, TouchId, TouchPhase,
};
use beui_core::painter::Painter;

use keyboard::{KeyId, Keyboard};

const BAR_HEIGHT: f32 = 56.0;
const BAR_PADDING: f32 = 6.0;
const BAR_SPACING: f32 = 6.0;
const BAR_WIDTH: f32 = 520.0;
const CURSOR_SIZE: f32 = 26.0;
const CURSOR_SHADOW: f32 = 1.5;
const TAP_DISTANCE: f32 = 10.0;
const TAP_TRAVEL: f32 = 4.0;
const IBEAM_HEIGHT: f32 = 18.0;
const IBEAM_SERIF: f32 = 3.5;
const IBEAM_WIDTH: f32 = 1.5;
const TAP_TIME: Duration = Duration::from_millis(300);
pub const DOUBLE_TAP_TIME: Duration = Duration::from_millis(250);
const MIDDLE_HOLD: Duration = Duration::from_millis(180);
pub const SCROLL_TICK: f32 = 22.0;
pub const WHEEL_LINE: f32 = 40.0;
const BORDER_WIDTH: f32 = 1.0;
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

#[derive(Clone, Copy, PartialEq)]
enum Left {
    Up,
    Tapped(Instant),
    Armed { locked: bool, held: Vec2 },
    Dragging { locked: bool },
    Locked,
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
    travel: f32,
}

struct Layout {
    bar: Rect,
    buttons: [Rect; 4],
    keyboard: Option<Rect>,
    trackpad: Rect,
}

pub struct MouseSimulation {
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
    left: Left,
    scrolling: bool,
    skew: Duration,
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
            left: Left::Up,
            scrolling: false,
            skew: Duration::ZERO,
            keyboard_open: false,
            keyboard: Keyboard::default(),
            painted: Rect::NOTHING,
        }
    }
}

impl MouseSimulation {
    fn now(&self) -> Instant {
        Instant::now() + self.skew
    }

    pub fn advance_clock(&mut self, by: Duration) {
        self.skew += by;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn paints(&self) -> bool {
        self.enabled || self.painted.is_positive()
    }

    fn reserved_height(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let layout = self.layout();
        let height = layout.bar.height() + layout.keyboard.map_or(0.0, |rect| rect.height());
        height * self.scale
    }

    fn measured(&mut self, viewport: Rect, scale: f32) {
        self.viewport = viewport;
        self.scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
    }

    fn translated(&mut self, raw: RawInput) -> (RawInput, Option<Duration>) {
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
        self.left = Left::Up;
        self.scrolling = false;
        self.keyboard.clear();
    }

    fn settle(&mut self) -> Option<Duration> {
        let now = self.now();
        let left = match self.left {
            Left::Tapped(at) => {
                let elapsed = now.saturating_duration_since(at);
                if elapsed >= DOUBLE_TAP_TIME {
                    self.left = Left::Up;
                    None
                } else {
                    Some(DOUBLE_TAP_TIME - elapsed)
                }
            }
            _ => None,
        };
        let middle = match self.middle {
            Middle::Pending(since) => {
                let elapsed = now.saturating_duration_since(since);
                if elapsed >= MIDDLE_HOLD {
                    self.middle = Middle::Held;
                    None
                } else {
                    Some(MIDDLE_HOLD - elapsed)
                }
            }
            _ => None,
        };
        match (left, middle) {
            (Some(left), Some(middle)) => Some(left.min(middle)),
            (left, middle) => left.or(middle),
        }
    }

    fn flush(&mut self, out: &mut Vec<Event>) {
        let modifiers = self.keyboard.modifiers();
        let pressed_at = self.reported.unwrap_or(self.cursor);
        for index in 0..BUTTONS.len() {
            if self.held(index) && !self.emitted[index] {
                self.emitted[index] = true;
                out.push(self.button_event(index, pressed_at, true, modifiers));
            }
        }
        if self.reported != Some(self.cursor) {
            self.reported = Some(self.cursor);
            out.push(Event::PointerMoved(self.cursor));
        }
        for index in 0..BUTTONS.len() {
            if !self.held(index) && self.emitted[index] {
                self.emitted[index] = false;
                out.push(self.button_event(index, self.cursor, false, modifiers));
            }
        }
        for index in std::mem::take(&mut self.clicks) {
            if self.emitted[index] {
                continue;
            }
            out.push(self.button_event(index, self.cursor, true, modifiers));
            out.push(self.button_event(index, self.cursor, false, modifiers));
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

    fn button_event(&self, index: usize, pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
        Event::PointerButton {
            pos,
            button: BUTTONS[index],
            pressed,
            modifiers,
        }
    }

    fn held(&self, index: usize) -> bool {
        match index {
            0 => self.holders[0].is_some() || self.left != Left::Up,
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
                self.middle = Middle::Pending(self.now());
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
                    self.left = match self.left {
                        Left::Armed { locked: true, .. } | Left::Dragging { locked: true } => {
                            Left::Locked
                        }
                        Left::Locked => Left::Locked,
                        _ => Left::Up,
                    };
                } else {
                    let now = self.now();
                    self.left = match self.left {
                        Left::Tapped(at)
                            if now.saturating_duration_since(at) >= DOUBLE_TAP_TIME =>
                        {
                            Left::Up
                        }
                        Left::Tapped(_) => Left::Armed {
                            locked: false,
                            held: Vec2::ZERO,
                        },
                        Left::Locked => Left::Armed {
                            locked: true,
                            held: Vec2::ZERO,
                        },
                        other => other,
                    };
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
                started: self.now(),
                moved: false,
                travel: 0.0,
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
            finger.travel += delta.length();
            if !finger.moved && finger.origin.distance(pos) >= TAP_DISTANCE * scale {
                finger.moved = true;
            }
            (finger.role, delta, finger.moved)
        };
        match role {
            Role::Trackpad => {
                if self.scrolling {
                    self.scroll += delta * count.recip();
                } else if let Left::Armed { locked, held } = self.left {
                    match moved {
                        true => {
                            self.left = Left::Dragging { locked };
                            self.move_cursor(held + delta);
                        }
                        false => {
                            self.left = Left::Armed {
                                locked,
                                held: held + delta,
                            }
                        }
                    }
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
                    self.scroll += vec2(0.0, direction * WHEEL_LINE);
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
        let quick = !cancelled
            && !finger.moved
            && self.now().saturating_duration_since(finger.started) <= TAP_TIME;
        let tapped = quick && finger.travel < TAP_TRAVEL * self.scale;
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
                if std::mem::take(&mut self.scrolling) {
                    return;
                }
                self.left = match (self.left, tapped) {
                    (Left::Up, true) => Left::Tapped(self.now()),
                    (Left::Armed { locked: false, .. }, true) => Left::Locked,
                    (Left::Armed { locked: true, .. }, false) => Left::Locked,
                    (Left::Dragging { locked: true }, _) => Left::Locked,
                    (Left::Locked, _) => Left::Locked,
                    _ => Left::Up,
                };
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

    pub fn left_held(&self) -> bool {
        self.emitted[0]
    }

    pub fn cursor(&self) -> Pos2 {
        self.cursor
    }

    pub fn button_center(&self, index: usize) -> Pos2 {
        self.document(self.layout().buttons[index].center())
    }

    pub fn trackpad_center(&self) -> Pos2 {
        self.document(self.layout().trackpad.center())
    }

    pub fn key_center(&self, label: &str) -> Option<Pos2> {
        let rect = self.layout().keyboard?;
        Keyboard::locate(rect, label).map(|pos| self.document(pos))
    }

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

    fn painted_at(&mut self, painter: &Painter, icon: CursorIcon) -> Rect {
        if !self.enabled || !self.bounds().is_positive() {
            let damage = self.painted;
            self.painted = Rect::NOTHING;
            return damage;
        }
        let layout = self.layout();
        let top = layout.keyboard.unwrap_or(layout.bar);
        painter.rect_filled(layout.bar, 0.0, Theme::DARK.background);
        painter.rect_filled(
            Rect::from_min_max(top.min, pos2(top.right(), top.top() + BORDER_WIDTH)),
            0.0,
            Theme::DARK.border,
        );
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
        let cursor = self.paint_cursor(painter, icon);
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

    fn paint_cursor(&self, painter: &Painter, icon: CursorIcon) -> Rect {
        let at = self.local(self.cursor);
        let color = if self.held(0) {
            Theme::DARK.accent
        } else {
            Theme::DARK.knob
        };
        let shadow = vec2(CURSOR_SHADOW, CURSOR_SHADOW);
        match icon {
            CursorIcon::None => {}
            CursorIcon::Text => {
                ibeam(painter, at + shadow, SHADOW);
                ibeam(painter, at, color);
            }
            icon => {
                let (text, hotspot, angle) = cursor_glyph(icon);
                let painter = painter.rotated(at, angle);
                let galley = painter.layout(text, FontId::icons(CURSOR_SIZE), f32::INFINITY);
                let origin = at - hotspot * CURSOR_SIZE;
                painter.galley(origin + shadow, galley.clone(), SHADOW);
                painter.galley(origin, galley, color);
            }
        }
        Rect::from_center_size(at, vec2(CURSOR_SIZE, CURSOR_SIZE) * 3.0)
    }
}

fn cursor_glyph(icon: CursorIcon) -> (&'static str, Vec2, f32) {
    let center = vec2(0.48, 0.56);
    let diagonal = std::f32::consts::FRAC_PI_4;
    match icon {
        CursorIcon::Default | CursorIcon::None | CursorIcon::Text => {
            (icons::ICON_ARROW_SELECTOR_TOOL, vec2(0.23, 0.19), 0.0)
        }
        CursorIcon::PointingHand => (icons::ICON_PAN_TOOL_ALT, vec2(0.29, 0.15), 0.0),
        CursorIcon::Crosshair => (icons::ICON_ADD, center, 0.0),
        CursorIcon::Grab => (icons::ICON_PAN_TOOL, center, 0.0),
        CursorIcon::Grabbing => (icons::ICON_BACK_HAND, center, 0.0),
        CursorIcon::NotAllowed => (icons::ICON_BLOCK, center, 0.0),
        CursorIcon::ResizeHorizontal => (icons::ICON_WIDTH, center, 0.0),
        CursorIcon::ResizeVertical => (icons::ICON_HEIGHT, center, 0.0),
        CursorIcon::ResizeNwSe => (icons::ICON_WIDTH, center, diagonal),
        CursorIcon::ResizeNeSw => (icons::ICON_WIDTH, center, -diagonal),
        CursorIcon::Wait => (icons::ICON_HOURGLASS, center, 0.0),
        CursorIcon::Progress => (icons::ICON_PROGRESS_ACTIVITY, center, 0.0),
        CursorIcon::Move => (icons::ICON_OPEN_WITH, center, 0.0),
        CursorIcon::Help => (icons::ICON_HELP, center, 0.0),
        CursorIcon::Alias => (icons::ICON_SHORTCUT, center, 0.0),
    }
}

fn ibeam(painter: &Painter, at: Pos2, color: Color32) {
    let half = IBEAM_HEIGHT / 2.0;
    let (top, bottom) = (at.y - half, at.y + half);
    painter.line(pos2(at.x, top), pos2(at.x, bottom), IBEAM_WIDTH, color);
    for y in [top, bottom] {
        painter.line(
            pos2(at.x - IBEAM_SERIF, y),
            pos2(at.x + IBEAM_SERIF, y),
            IBEAM_WIDTH,
            color,
        );
    }
}

fn glyph(painter: &Painter, center: Pos2, text: &str, size: f32, color: Color32) {
    let galley = painter.layout(text, FontId::icons(size), f32::INFINITY);
    let extent = galley.size();
    painter.galley(center - extent * 0.5, galley, color);
}

impl InputSimulation for MouseSimulation {
    fn enabled(&self) -> bool {
        self.is_enabled()
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enable(enabled);
    }

    fn translate(&mut self, raw: RawInput) -> (RawInput, Option<Duration>) {
        self.translated(raw)
    }

    fn measure(&mut self, viewport: Rect, scale: f32) {
        self.measured(viewport, scale);
    }

    fn reserved(&self) -> f32 {
        self.reserved_height()
    }

    fn painting(&self) -> bool {
        self.paints()
    }

    fn paint(&mut self, painter: &Painter, icon: CursorIcon) -> Rect {
        self.painted_at(painter, icon)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

pub fn simulate(ctx: &Context, enabled: bool) {
    if ctx
        .input_simulation_mut(|simulation| simulation.set_enabled(enabled))
        .is_none()
        && enabled
    {
        let mut simulation = MouseSimulation::default();
        simulation.enable(true);
        ctx.set_input_simulation(Box::new(simulation));
    }
}
