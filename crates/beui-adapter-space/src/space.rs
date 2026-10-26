use std::rc::Rc;
use std::time::{Duration, Instant};

use beui_core::app::Setup;
use beui_core::color::Color32;
use beui_core::context::{Context, FrameOutput, WindowOutput};
use beui_core::damage::Region;
use beui_core::display::Layer;
use beui_core::geometry::{Pos2, Rect, Vec2, pos2};
use beui_core::input::{CursorIcon, Input, Key, Keystroke, Modifiers, PointerButton, TouchPhase};
use beui_core::painter::Shape;
use beui_core::runner::{
    Controller, Hand, HostView, Hosted, Incoming, SCROLL_LINE, Units, WindowHost, scrolled,
};
use beui_core::window::{
    WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest, WindowState,
};
use beui_renderer_wgpu::present::GpuSetup;
use glam::{Mat4, Quat, Vec3};

use crate::carry::{Carry, NEAREST};
use crate::chrome::{self, Chrome, Corner, Gesture, Look, Part};
use crate::cursor::{self, Sprite};
use crate::head::{Head, Ray, View, Views};
use crate::hold::Hold;
use crate::laser::{self, Ending, Laser};
use crate::panel::{Hit, Panel, Pose, pick};
use crate::render::{Beam, ChromeFrame, Compositor, Frame, PanelFrame};
use crate::resize::Resize;

pub const SKY: Color32 = Color32::from_rgb(22, 25, 31);
const MOST_SECONDS: f32 = 0.1;
const FILLED: f32 = 0.8;
pub const OPEN_DISTANCE: f32 = 1.0;
pub const OPEN_NEARER: f32 = 0.1;
pub const OPEN_GRAB: Pos2 = pos2(24.0, 16.0);
pub const OVER_NEARER: f32 = 0.03;
pub const CURSOR_LIFT: f32 = 0.01;
pub const CURSOR_MARGIN: f32 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Stroke {
    key: Option<Key>,
    evdev: Option<u32>,
}

impl Stroke {
    fn of(keystroke: &Keystroke) -> Self {
        Self {
            key: keystroke.key,
            evdev: keystroke.evdev,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Pointer {
    Mouse,
    Hand(Hand),
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Held {
    pub carry: Option<Carry>,
    pub resize: Option<Resize>,
    pub gesture: Option<Gesture>,
    pub hovered: Option<WindowId>,
    pub captured: Option<WindowId>,
    last: Option<(WindowId, Pos2)>,
}

impl Held {
    pub fn gripped(&self) -> Option<WindowId> {
        self.carry
            .as_ref()
            .map(|carry| carry.window)
            .or(self.resize.map(|resize| resize.window))
    }

    fn holds(&self, window: WindowId) -> bool {
        self.gripped() == Some(window)
            || self
                .gesture
                .is_some_and(|gesture| gesture.window() == window)
    }

    fn forget(&mut self, window: WindowId) {
        if self.hovered == Some(window) {
            self.hovered = None;
        }
        if self.captured == Some(window) {
            self.captured = None;
        }
        if self
            .carry
            .as_ref()
            .is_some_and(|carry| carry.window == window)
        {
            self.carry = None;
        }
        if self.resize.is_some_and(|resize| resize.window == window) {
            self.resize = None;
        }
        if self
            .gesture
            .is_some_and(|gesture| gesture.window() == window)
        {
            self.gesture = None;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Both {
    pub window: WindowId,
    original: Panel,
    start: [Vec3; 2],
}

#[derive(Clone, PartialEq, Debug)]
struct Shown {
    platform: Vec2,
    head: Head,
    panels: Vec<Panel>,
    cursor: Option<Vec3>,
    icon: CursorIcon,
    chrome: Vec<(WindowId, Look, f32)>,
    beams: Vec<Beam>,
}

pub struct Space {
    pub head: Head,
    tracked: Option<Views>,
    pub panels: Vec<Panel>,
    pub mouse: Held,
    pub lasers: Vec<Laser>,
    pub both: Option<Both>,
    pub chrome: Chrome,
    compositor: Option<Compositor>,
    context: Option<Context>,
    chrome_shown: Vec<(WindowId, Look, WindowOutput)>,
    platform: Vec2,
    pixels_per_point: f32,
    now: Option<Instant>,
    clock: Option<Instant>,
    cursor: Option<Vec3>,
    cursor_distance: f32,
    grabbed: bool,
    hold: Hold,
    regrabbing: Vec<PointerButton>,
    app_locked: bool,
    sprite: Sprite,
    focused: Option<WindowId>,
    pressed: u32,
    dropped: u32,
    grip_held: bool,
    grip_pressed: u32,
    reports: Vec<Incoming>,
    forwarded: Vec<Keystroke>,
    swallowed: Vec<Stroke>,
    walking: Vec<Key>,
    modifiers: Modifiers,
    shown: Option<(Shown, WindowOutput)>,
    scales: Vec<(WindowId, f32)>,
    rescaling: Vec<(WindowId, f32, Instant)>,
}

impl Default for Space {
    fn default() -> Self {
        Self::new()
    }
}

impl Space {
    pub fn new() -> Self {
        Self {
            head: Head::default(),
            tracked: None,
            panels: Vec::new(),
            mouse: Held::default(),
            lasers: Vec::new(),
            both: None,
            chrome: Chrome::default(),
            compositor: None,
            context: None,
            chrome_shown: Vec::new(),
            platform: Vec2::ZERO,
            pixels_per_point: 1.0,
            now: None,
            clock: None,
            cursor: None,
            cursor_distance: OPEN_DISTANCE,
            grabbed: true,
            hold: Hold::default(),
            regrabbing: Vec::new(),
            app_locked: false,
            sprite: Sprite::new(),
            focused: None,
            pressed: 0,
            dropped: 0,
            grip_held: false,
            grip_pressed: 0,
            reports: Vec::new(),
            forwarded: Vec::new(),
            swallowed: Vec::new(),
            walking: Vec::new(),
            modifiers: Modifiers::NONE,
            shown: None,
            scales: Vec::new(),
            rescaling: Vec::new(),
        }
    }

    pub fn steering(&self) -> bool {
        self.modifiers.ctrl && self.modifiers.shift
    }

    pub fn set_views(&mut self, views: Option<Views>) {
        if let Some((position, rotation)) = views.as_ref().and_then(Views::centre) {
            self.head = Head::posed(position, rotation);
        }
        self.tracked = views;
    }

    pub fn tracked(&self) -> Option<&Views> {
        self.tracked.as_ref()
    }

    pub fn views(&self) -> Vec<View> {
        match &self.tracked {
            Some(tracked) => tracked.views.clone(),
            None => vec![self.head.view(self.platform)],
        }
    }

    pub fn scale(&self, window: WindowId) -> Option<f32> {
        self.scales
            .iter()
            .find(|(held, _)| *held == window)
            .map(|(_, scale)| *scale)
    }

    fn rescale(&mut self) {
        let gripped: Vec<WindowId> = self.holders().flat_map(Held::gripped).collect();
        let gestured: Vec<WindowId> = self
            .holders()
            .filter_map(|held| held.gesture.as_ref().map(Gesture::window))
            .chain(self.both.map(|both| both.window))
            .collect();
        let views = self.views();
        for panel in &self.panels {
            let window = panel.window;
            let held = self.scale(window);
            let wanted = crate::scale::wanted(panel, &views);
            let scale = crate::scale::chosen(held, wanted, panel.size, self.pixels_per_point);
            if held.is_some() {
                if held == Some(scale) || gripped.contains(&window) || gestured.contains(&window) {
                    self.rescaling.retain(|(waiting, ..)| *waiting != window);
                    continue;
                }
                let Some(now) = self.clock else {
                    continue;
                };
                let since = self
                    .rescaling
                    .iter()
                    .find(|(waiting, next, _)| *waiting == window && *next == scale)
                    .map(|(.., since)| *since);
                let Some(since) = since else {
                    self.rescaling.retain(|(waiting, ..)| *waiting != window);
                    self.rescaling.push((window, scale, now));
                    continue;
                };
                if now.saturating_duration_since(since) < crate::scale::SETTLE {
                    continue;
                }
                self.rescaling.retain(|(waiting, ..)| *waiting != window);
            }
            self.scales.retain(|(held, _)| *held != window);
            self.rescaling.retain(|(waiting, ..)| *waiting != window);
            self.scales.push((window, scale));
            self.reports.push(Incoming::Window(WindowReport::Rastered {
                window,
                pixels_per_point: scale,
            }));
        }
    }

    fn settled_in(&self) -> Duration {
        let Some(now) = self.clock else {
            return Duration::MAX;
        };
        self.rescaling
            .iter()
            .map(|(.., since)| (*since + crate::scale::SETTLE).saturating_duration_since(now))
            .min()
            .unwrap_or(Duration::MAX)
    }

    pub fn panel(&self, window: WindowId) -> Option<&Panel> {
        self.panels.iter().find(|panel| panel.window == window)
    }

    fn panel_mut(&mut self, window: WindowId) -> Option<&mut Panel> {
        self.panels.iter_mut().find(|panel| panel.window == window)
    }

    pub fn pick(&self, ray: Ray) -> Option<Hit> {
        pick(&self.panels, ray)
    }

    pub fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    fn focus(&mut self, window: Option<WindowId>) {
        if self.focused == window {
            return;
        }
        for (window, focused) in [(self.focused, false), (window, true)] {
            if let Some(window) = window {
                self.reports
                    .push(Incoming::Window(WindowReport::Focused { window, focused }));
            }
        }
        self.focused = window;
    }

    fn pressed_on(&self, ray: Ray) -> Option<WindowId> {
        let chrome = self.chrome_hit(ray).filter(|_| self.pressed == 0);
        chrome
            .map(|(_, hit)| hit.window)
            .or_else(|| self.pick(ray).map(|hit| hit.window))
    }

    pub fn grabbed(&self) -> bool {
        self.grabbed
    }

    pub fn cursor(&self) -> Option<Vec3> {
        self.cursor
    }

    fn drawn(&self) -> Option<(Pos2, bool)> {
        let across = self.head.across(self.platform, self.cursor?);
        let margin = CURSOR_MARGIN * self.pixels_per_point;
        let screen = glam::Vec2::new(self.platform.x.max(1.0), self.platform.y.max(1.0));
        let limit = (glam::Vec2::ONE - 2.0 * margin / screen).max(glam::Vec2::splat(1e-3));
        let over = (across.x.abs() / limit.x).max(across.y.abs() / limit.y);
        let (across, clamped) = match over > 1.0 {
            true => (across / over, true),
            false => (across, false),
        };
        let at = Head::on_screen(self.platform, across);
        Some((pos2(at.x, at.y), clamped))
    }

    pub fn cursor_screen(&self) -> Option<Pos2> {
        self.drawn().map(|(at, _)| at)
    }

    pub fn cursor_ray(&self) -> Option<Ray> {
        let (at, clamped) = self.drawn()?;
        match clamped {
            true => Some(self.head.ray(self.platform, at.to_vec2())),
            false => self.cursor.map(|point| self.toward(point)),
        }
    }

    fn toward(&self, point: Vec3) -> Ray {
        Ray {
            origin: self.head.position,
            direction: (point - self.head.position)
                .normalize_or(self.head.rotation() * Vec3::NEG_Z),
        }
    }

    fn hit_along(&self, ray: Ray) -> Option<Hit> {
        self.chrome_hit(ray)
            .map(|(_, hit)| hit)
            .or_else(|| self.pick(ray))
    }

    pub fn cursor_point(&self) -> Option<Vec3> {
        self.hit_along(self.cursor_ray()?).map(|hit| hit.point)
    }

    pub fn cursor_at(&self) -> Option<Vec3> {
        let saved = self.cursor?;
        if !self.drawn()?.1 {
            return Some(saved);
        }
        let ray = self.cursor_ray()?;
        let distance = self
            .cursor_point()
            .map_or(saved.distance(ray.origin), |point| {
                point.distance(ray.origin) * (1.0 - CURSOR_LIFT)
            });
        Some(ray.at(distance))
    }

    fn place_cursor(&mut self, ray: Ray) {
        let distance = match self.hit_along(ray) {
            Some(hit) => {
                self.cursor_distance = hit.distance;
                hit.distance
            }
            None => self.cursor_distance,
        };
        self.cursor = Some(ray.at(distance * (1.0 - CURSOR_LIFT)));
    }

    fn centre_cursor(&mut self) {
        if self.cursor.is_none() {
            let middle = self.platform / 2.0;
            self.place_cursor(self.head.ray(self.platform, middle));
        }
    }

    fn ray_of(&self, units: Units, window: WindowId, at: Pos2) -> Ray {
        match (units, self.panel(window)) {
            (Units::Points, Some(panel)) => self.toward(panel.world(at)),
            (Units::Points, None) => self
                .head
                .ray(self.platform, at.to_vec2() * self.pixels_per_point),
            (Units::Physical | Units::Logical, _) => self.head.ray(self.platform, at.to_vec2()),
        }
    }

    pub fn cursor_scale(&self, at: Vec3) -> f32 {
        let points = self.platform.y.max(1.0) / self.pixels_per_point;
        at.distance(self.head.position) * 2.0 * (crate::head::FOV_Y / 2.0).tan() / points
    }

    pub fn cursor_icon(&self, output: &FrameOutput) -> CursorIcon {
        let held = self.mouse.gesture.map(|gesture| gesture.part());
        if let Some(part) = held.or(self.chrome.hovered.map(|(_, part)| part)) {
            return chrome_cursor(part, held.is_some());
        }
        self.mouse
            .hovered
            .or(self.mouse.captured)
            .or(self.mouse.gripped())
            .and_then(|window| output.window(window))
            .map_or(CursorIcon::Default, |window| window.window.requests.cursor)
    }

    fn remember_cursor(&mut self) {
        if let Some(point) = self.cursor_point() {
            self.cursor_distance = point.distance(self.head.position);
        }
    }

    fn cursor_frame(&self, at: Vec3) -> PanelFrame {
        let scale = self.cursor_scale(at);
        let rotation = self.head.rotation();
        let (right, up) = (rotation * Vec3::X, rotation * Vec3::Y);
        let centre = cursor::SIZE / 2.0 - cursor::HOTSPOT.to_vec2();
        let middle = at + (right * centre.x - up * centre.y) * scale;
        PanelFrame {
            window: self.sprite.window,
            model: Mat4::from_cols(
                (right * cursor::SIZE.x * scale).extend(0.0),
                (up * cursor::SIZE.y * scale).extend(0.0),
                (rotation * Vec3::Z).extend(0.0),
                middle.extend(1.0),
            ),
            metres: glam::Vec2::ONE,
            bend: 0.0,
            chrome: None,
        }
    }

    fn open_main(&mut self, incoming: &mut Vec<Incoming>) {
        if self.panel(WindowId::MAIN).is_some() || self.platform.x < 1.0 || self.platform.y < 1.0 {
            return;
        }
        let size = self.platform / self.pixels_per_point;
        let mut panel = Panel::new(
            WindowId::MAIN,
            Pose {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            size,
        );
        let half_width = (crate::head::FOV_Y / 2.0).tan() * self.platform.x / self.platform.y;
        let distance = panel.metres().x / 2.0 / (FILLED * half_width);
        panel.pose.position = self.head.position + Vec3::new(0.0, 0.0, -distance);
        self.panels.push(panel);
        self.focused = Some(WindowId::MAIN);
        incoming.push(Incoming::Window(WindowReport::Resized {
            window: WindowId::MAIN,
            size,
        }));
    }

    fn walk(&mut self, now: Instant) {
        let seconds = self
            .now
            .replace(now)
            .map_or(0.0, |then| {
                now.saturating_duration_since(then).as_secs_f32()
            })
            .min(MOST_SECONDS);
        if !self.steering() || self.walking.is_empty() {
            return;
        }
        let held = |key| f32::from(u8::from(self.walking.contains(&key)));
        let strafe = held(Key::D) - held(Key::A);
        let forward = held(Key::W) - held(Key::S);
        self.head.walk(strafe, forward, seconds);
        self.head.tilt(held(Key::Q) - held(Key::E), seconds);
    }

    fn take(&mut self, incoming: Incoming, out: &mut Vec<Incoming>) {
        match incoming {
            Incoming::Input {
                units,
                window,
                input,
            } => self.platform_input(units, window, input, out),
            Incoming::Controller(controller) => self.controller(controller, out),
            other => out.push(other),
        }
    }

    fn pixels(&self, units: Units, motion: Vec2) -> Vec2 {
        match units {
            Units::Points => motion * self.pixels_per_point,
            Units::Physical | Units::Logical => motion,
        }
    }

    fn platform_input(
        &mut self,
        units: Units,
        window: WindowId,
        input: Input,
        out: &mut Vec<Incoming>,
    ) {
        match input {
            Input::Text(_)
            | Input::Ime(_)
            | Input::Paste(_)
            | Input::InterceptedKey { .. }
            | Input::InterceptedTap(_)
                if self.focused.is_none() => {}
            Input::Key(keystroke) => self.key(units, keystroke, out),
            Input::Modifiers(modifiers) => self.modifiers(units, modifiers, out),
            Input::PointerMotion(motion) if self.steering() => {
                self.head.look(self.pixels(units, motion));
            }
            Input::PointerMotion(motion) if !self.app_locked => {
                self.move_cursor(self.pixels(units, motion), out);
            }
            Input::PointerMoved(at) => {
                self.place_cursor(self.ray_of(units, window, at));
                self.moved(out);
            }
            Input::PointerButton {
                button, pressed, ..
            } => self.press(input, button, pressed, out),
            Input::Scroll(delta) if self.mouse.carry.is_some() => {
                let lines = scrolled(delta, self.pixels_per_point).y / SCROLL_LINE;
                let (head, now, others) = (self.head, self.clock, self.panels.clone());
                if let Some(carry) = &mut self.mouse.carry
                    && let Some(panel) = others.iter().find(|panel| panel.window == carry.window)
                {
                    carry.scroll(lines, &head, panel, &others, now);
                }
            }
            Input::ScrollStarted | Input::ScrollEnded if self.mouse.carry.is_some() => {}
            Input::PointerLeft => {
                self.cursor = None;
                self.leave(out);
            }
            Input::PointersCancelled => {
                self.mouse.captured = None;
                self.pressed = 0;
                self.dropped = 0;
                for panel in &self.panels {
                    out.push(Incoming::points(Input::PointersCancelled).in_window(panel.window));
                }
            }
            input => {
                if let Input::Focus(false) | Input::PointerUnlocked = input {
                    self.grabbed = false;
                    self.hold.cancel();
                }
                match position(&input) {
                    Some(at) => self.point(input, self.ray_of(units, window, at), out),
                    None => {
                        if let Some(window) = self.mouse.hovered.or(self.mouse.captured) {
                            out.push(Incoming::new(units, input).in_window(window));
                        } else if !matches!(
                            input,
                            Input::Scroll(_)
                                | Input::ScrollStarted
                                | Input::ScrollEnded
                                | Input::Zoom(_)
                                | Input::PointerMotion(_)
                        ) {
                            out.push(Incoming::new(units, input));
                        }
                    }
                }
            }
        }
    }

    fn move_cursor(&mut self, pixels: Vec2, out: &mut Vec<Incoming>) {
        self.centre_cursor();
        let Some(from) = self.cursor_screen() else {
            return;
        };
        let margin = Vec2::splat(CURSOR_MARGIN * self.pixels_per_point);
        let most = (self.platform - margin).max(margin);
        let to = (from + pixels).to_vec2().max(margin).min(most);
        self.place_cursor(self.head.ray(self.platform, to));
        self.moved(out);
    }

    fn moved(&mut self, out: &mut Vec<Incoming>) {
        let Some(ray) = self.cursor_ray() else {
            return;
        };
        let input = Input::PointerMoved(Pos2::ZERO);
        let direction = self.head.rotation().inverse() * ray.direction;
        match (&mut self.mouse.carry, self.mouse.resize) {
            (Some(carry), _) => carry.aim(direction),
            (None, Some(_)) => self.resize_to(Pointer::Mouse, ray, out),
            (None, None) => {
                if !self.chrome_pointer(&input, out) {
                    self.point(input, ray, out);
                }
            }
        }
    }

    fn press(
        &mut self,
        input: Input,
        button: PointerButton,
        pressed: bool,
        out: &mut Vec<Incoming>,
    ) {
        if pressed && !self.grabbed {
            self.grabbed = true;
            self.regrabbing.push(button);
            return;
        }
        if !pressed && let Some(at) = self.regrabbing.iter().position(|held| *held == button) {
            self.regrabbing.remove(at);
            return;
        }
        self.centre_cursor();
        let Some(ray) = self.cursor_ray() else {
            return;
        };
        if pressed && self.mouse.gripped().is_none() && self.mouse.gesture.is_none() {
            self.focus(self.pressed_on(ray));
        }
        if !self.chrome_pointer(&input, out) {
            self.button(input, ray, out);
        }
    }

    fn leave(&mut self, out: &mut Vec<Incoming>) {
        if let Some(window) = self.mouse.hovered.take() {
            out.push(Incoming::points(Input::PointerLeft).in_window(window));
        }
    }

    fn beyond(&mut self, pointer: Pointer, window: WindowId, ray: Ray) -> Pos2 {
        let hit = self.panel(window).and_then(|panel| panel.intersect(ray));
        let Some(held) = self.held_mut(pointer) else {
            return pos2(-1.0, -1.0);
        };
        match hit {
            Some(hit) => {
                held.last = Some((window, hit.pos));
                hit.pos
            }
            None => match held.last {
                Some((last, pos)) if last == window => pos,
                _ => pos2(-1.0, -1.0),
            },
        }
    }

    fn point(&mut self, input: Input, ray: Ray, out: &mut Vec<Incoming>) {
        match self.pick(ray) {
            Some(hit) => {
                self.mouse.hovered = Some(hit.window);
                self.mouse.last = Some((hit.window, hit.pos));
                out.push(Incoming::points(placed_at(input, hit.pos)).in_window(hit.window));
            }
            None => {
                self.leave(out);
                if let Some(window) = self.mouse.captured.filter(|_| self.pressed > 0) {
                    let pos = self.beyond(Pointer::Mouse, window, ray);
                    out.push(Incoming::points(placed_at(input, pos)).in_window(window));
                }
            }
        }
    }

    fn button(&mut self, input: Input, ray: Ray, out: &mut Vec<Incoming>) {
        let Input::PointerButton { pressed, .. } = input else {
            return;
        };
        if let Some(window) = self.mouse.gripped() {
            match pressed {
                true => self.grip_pressed += 1,
                false if self.grip_held => {
                    let grab = match &self.mouse.carry {
                        Some(_) => self.grab_pos(Pointer::Mouse, window),
                        None => self.beyond(Pointer::Mouse, window, ray),
                    };
                    self.end_grip(Pointer::Mouse, out);
                    self.release(placed_at(input, grab), window, out);
                }
                false => {
                    self.grip_pressed = self.grip_pressed.saturating_sub(1);
                    if self.grip_pressed == 0 {
                        self.end_grip(Pointer::Mouse, out);
                    }
                }
            }
            return;
        }
        match pressed {
            true => match self.pick(ray) {
                Some(hit) => {
                    self.mouse.hovered = Some(hit.window);
                    if self.pressed == 0 {
                        self.mouse.captured = Some(hit.window);
                    }
                    self.pressed += 1;
                    self.mouse.last = Some((hit.window, hit.pos));
                    out.push(Incoming::points(placed_at(input, hit.pos)).in_window(hit.window));
                }
                None => self.dropped += 1,
            },
            false if self.dropped > 0 => self.dropped -= 1,
            false => {
                let Some(window) = self.mouse.captured.filter(|_| self.pressed > 0) else {
                    return;
                };
                match self.pick(ray) {
                    Some(hit) => {
                        self.mouse.hovered = Some(hit.window);
                        self.mouse.last = Some((hit.window, hit.pos));
                        self.release(placed_at(input, hit.pos), hit.window, out);
                    }
                    None => {
                        self.leave(out);
                        let at = self.beyond(Pointer::Mouse, window, ray);
                        self.release(placed_at(input, at), window, out);
                    }
                }
            }
        }
    }

    fn release(&mut self, input: Input, window: WindowId, out: &mut Vec<Incoming>) {
        out.push(Incoming::points(input).in_window(window));
        self.pressed = self.pressed.saturating_sub(1);
        if self.pressed == 0 {
            self.mouse.captured = None;
        }
    }

    fn grab_pos(&self, pointer: Pointer, window: WindowId) -> Pos2 {
        let Some(panel) = self.panel(window) else {
            return Pos2::ZERO;
        };
        self.ray(pointer)
            .and_then(|ray| panel.intersect(ray))
            .map_or(Pos2::ZERO, |hit| hit.pos)
    }

    fn modifiers(&mut self, units: Units, modifiers: Modifiers, out: &mut Vec<Incoming>) {
        let was = self.steering();
        self.modifiers = modifiers;
        match (was, self.steering()) {
            (false, true) => {
                for forwarded in self.forwarded.drain(..) {
                    self.swallowed.push(Stroke::of(&forwarded));
                    out.push(Incoming::new(
                        units,
                        Input::Key(Keystroke {
                            pressed: false,
                            repeat: false,
                            text: None,
                            ..forwarded
                        }),
                    ));
                }
                out.push(Incoming::new(
                    units,
                    Input::Modifiers(Modifiers {
                        ctrl: false,
                        shift: false,
                        ..modifiers
                    }),
                ));
            }
            (true, true) => {}
            (true, false) => {
                self.walking.clear();
                out.push(Incoming::new(units, Input::Modifiers(modifiers)));
            }
            (false, false) => out.push(Incoming::new(units, Input::Modifiers(modifiers))),
        }
    }

    fn key(&mut self, units: Units, keystroke: Keystroke, out: &mut Vec<Incoming>) {
        let stroke = Stroke::of(&keystroke);
        let placed = keystroke.evdev.and_then(Key::from_evdev).or(keystroke.key);
        let walks = matches!(
            placed,
            Some(Key::W | Key::A | Key::S | Key::D | Key::Q | Key::E)
        );
        if !keystroke.pressed
            && let Some(key) = placed
        {
            self.walking.retain(|held| *held != key);
        }
        if self.swallowed.contains(&stroke) {
            if !keystroke.pressed {
                self.swallowed.retain(|held| *held != stroke);
            }
            return;
        }
        let chord = matches!(keystroke.key, Some(Key::Ctrl | Key::Shift));
        if keystroke.pressed && self.steering() {
            self.swallowed.push(stroke);
            if walks
                && let Some(key) = placed
                && !self.walking.contains(&key)
            {
                self.walking.push(key);
            }
            return;
        }
        if !self.hold.key(&keystroke, self.clock, self.grabbed) {
            return;
        }
        if self.focused.is_none() && keystroke.pressed {
            self.swallowed.push(stroke);
            return;
        }
        if chord {
            match keystroke.pressed {
                true if !keystroke.repeat => self.forwarded.push(keystroke.clone()),
                true => {}
                false => self.forwarded.retain(|held| Stroke::of(held) != stroke),
            }
        }
        out.push(Incoming::new(units, Input::Key(keystroke)));
    }

    pub fn begin_carry(&mut self, pointer: Pointer, window: WindowId) {
        if self.holders().any(|held| held.gripped() == Some(window))
            || self.both.is_some_and(|both| both.window == window)
            || self.held(pointer).is_none_or(|held| held.carry.is_some())
        {
            return;
        }
        let Some(panel) = self.panel(window).copied() else {
            return;
        };
        let ray = self.ray(pointer);
        let grab = ray
            .and_then(|ray| panel.intersect(ray))
            .map_or(panel.pose.position, |hit| hit.point);
        let carry = match (pointer, ray) {
            (Pointer::Hand(_), Some(ray)) => {
                Carry::along(&panel, &self.head, ray, grab, self.clock)
            }
            (Pointer::Hand(_), None) => return,
            (Pointer::Mouse, _) => Carry::begin(&panel, &self.head, grab, self.clock),
        };
        if let Some(held) = self.held_mut(pointer) {
            held.carry = Some(carry);
        }
        self.focus(Some(window));
        match pointer {
            Pointer::Mouse => self.grip(window),
            Pointer::Hand(hand) => self.laser_mut(hand).ending = Ending::Trigger,
        }
    }

    fn grip(&mut self, window: WindowId) {
        self.grip_held = self.mouse.captured == Some(window) && self.pressed > 0;
        self.grip_pressed = 0;
    }

    fn end_grip(&mut self, pointer: Pointer, out: &mut Vec<Incoming>) {
        self.end_carry(pointer, out);
        self.end_resize(pointer, out);
    }

    pub fn begin_resize(&mut self, pointer: Pointer, window: WindowId, edge: WindowEdge) {
        if self
            .held(pointer)
            .is_none_or(|held| held.gripped().is_some())
        {
            return;
        }
        let Some(panel) = self.panel(window).copied() else {
            return;
        };
        let grab = self
            .ray(pointer)
            .and_then(|ray| panel.intersect(ray))
            .map_or_else(|| edge_point(edge, panel.size), |hit| hit.pos);
        if let Some(held) = self.held_mut(pointer) {
            held.resize = Some(Resize::begin(&panel, edge, grab));
        }
        if pointer == Pointer::Mouse {
            self.grip(window);
        }
    }

    fn resize_to(&mut self, pointer: Pointer, ray: Ray, out: &mut Vec<Incoming>) {
        let Some(resize) = self.held(pointer).and_then(|held| held.resize) else {
            return;
        };
        let Some(hit) = resize.original().intersect(ray) else {
            return;
        };
        let (pose, size) = resize.to(hit.pos);
        let Some(panel) = self.panel_mut(resize.window) else {
            return;
        };
        panel.pose = pose;
        if panel.size != size {
            panel.size = size;
            out.push(Incoming::Window(WindowReport::Resized {
                window: resize.window,
                size,
            }));
        }
    }

    pub fn end_resize(&mut self, pointer: Pointer, out: &mut Vec<Incoming>) {
        let Some(resize) = self.held_mut(pointer).and_then(|held| held.resize.take()) else {
            return;
        };
        if let Some(panel) = self.panel(resize.window) {
            out.push(Incoming::Window(WindowReport::Placed {
                window: resize.window,
                placement: panel.placement(),
            }));
        }
    }

    fn pointer_in(&self, window: WindowId) -> Pointer {
        self.lasers
            .iter()
            .find(|laser| laser.captured() == Some(window))
            .map_or(Pointer::Mouse, |laser| Pointer::Hand(laser.hand))
    }

    fn active_pointer(&self) -> Pointer {
        self.lasers
            .iter()
            .find(|laser| laser.captured().is_some())
            .map_or(Pointer::Mouse, |laser| Pointer::Hand(laser.hand))
    }

    pub fn held(&self, pointer: Pointer) -> Option<&Held> {
        match pointer {
            Pointer::Mouse => Some(&self.mouse),
            Pointer::Hand(hand) => self.laser(hand).map(|laser| &laser.held),
        }
    }

    fn held_mut(&mut self, pointer: Pointer) -> Option<&mut Held> {
        match pointer {
            Pointer::Mouse => Some(&mut self.mouse),
            Pointer::Hand(hand) => self
                .lasers
                .iter_mut()
                .find(|laser| laser.hand == hand)
                .map(|laser| &mut laser.held),
        }
    }

    fn holders(&self) -> impl Iterator<Item = &Held> {
        std::iter::once(&self.mouse).chain(self.lasers.iter().map(|laser| &laser.held))
    }

    fn pointers(&self) -> Vec<Pointer> {
        std::iter::once(Pointer::Mouse)
            .chain(self.lasers.iter().map(|laser| Pointer::Hand(laser.hand)))
            .collect()
    }

    pub fn laser(&self, hand: Hand) -> Option<&Laser> {
        self.lasers.iter().find(|laser| laser.hand == hand)
    }

    fn laser_mut(&mut self, hand: Hand) -> &mut Laser {
        let at = match self.lasers.iter().position(|laser| laser.hand == hand) {
            Some(at) => at,
            None => {
                self.lasers.push(Laser::new(hand));
                self.lasers.len() - 1
            }
        };
        &mut self.lasers[at]
    }

    pub fn ray(&self, pointer: Pointer) -> Option<Ray> {
        match pointer {
            Pointer::Mouse => self.cursor_ray(),
            Pointer::Hand(hand) => self.laser(hand).and_then(Laser::aimed),
        }
    }

    pub fn request(&mut self, request: &WindowRequest) {
        match request {
            WindowRequest::Open {
                window,
                placement,
                size,
            } => self.open(*window, placement, *size),
            WindowRequest::BeginMove(window) => self.begin_carry(self.pointer_in(*window), *window),
            WindowRequest::BeginResize { window, edge } => {
                self.begin_resize(self.pointer_in(*window), *window, *edge)
            }
            WindowRequest::Resize { window, size } => self.resize_window(*window, *size),
            WindowRequest::Close(window) => self.close(*window),
        }
    }

    pub fn open(&mut self, window: WindowId, placement: &WindowPlacement, size: Vec2) {
        let opening = self.panel(window).is_none();
        if opening {
            let mut panel = Panel::new(window, Pose::default(), size);
            let placed = match placement {
                WindowPlacement::Host(placement) => panel.placed(placement),
                WindowPlacement::Over { window: other, at } => self
                    .over(*other, *at, size)
                    .map(|(pose, metres_per_point)| {
                        panel.pose = pose;
                        panel.metres_per_point = metres_per_point;
                    })
                    .is_some(),
                WindowPlacement::AtPointer => false,
            };
            if !placed {
                panel.pose = self.at_pointer(size);
            }
            self.panels.push(panel);
        }
        let Some(panel) = self.panel(window) else {
            return;
        };
        let reports = [
            WindowReport::Resized {
                window,
                size: panel.size,
            },
            WindowReport::Placed {
                window,
                placement: panel.placement(),
            },
        ];
        self.reports.extend(reports.map(Incoming::Window));
        if opening {
            self.focus(Some(window));
        }
    }

    pub fn resize_window(&mut self, window: WindowId, size: Vec2) {
        if self
            .holders()
            .any(|held| held.resize.is_some_and(|resize| resize.window == window))
        {
            return;
        }
        let Some(panel) = self.panel_mut(window) else {
            return;
        };
        let corner = panel.frame_at(Pos2::ZERO);
        panel.size = size;
        panel.anchor(Pos2::ZERO, corner);
        self.reports
            .push(Incoming::Window(WindowReport::Resized { window, size }));
    }

    fn at_pointer(&self, size: Vec2) -> Pose {
        let ray = self.ray(self.active_pointer()).unwrap_or(Ray {
            origin: self.head.position,
            direction: self.head.rotation() * Vec3::NEG_Z,
        });
        let distance = pick(&self.panels, ray).map_or(OPEN_DISTANCE, |hit| {
            (hit.distance - OPEN_NEARER).max(NEAREST)
        });
        let point = ray.at(distance);
        let mut panel = Panel::new(
            WindowId::MAIN,
            Pose {
                position: point,
                rotation: Quat::IDENTITY,
            },
            size,
        );
        panel.face(self.head.position, self.head.up());
        let grab = pos2(OPEN_GRAB.x.min(size.x / 2.0), OPEN_GRAB.y.min(size.y / 2.0));
        panel.pose.position = point - panel.pose.rotation * panel.offset(grab);
        panel.pose
    }

    fn over(&self, other: WindowId, at: Pos2, size: Vec2) -> Option<(Pose, f32)> {
        let other = self.panel(other)?;
        let mut panel = Panel::new(WindowId::MAIN, other.pose, size);
        panel.metres_per_point = other.metres_per_point;
        let corner = other.offset(at);
        let local = |panel: &Panel| {
            other.pose.rotation.inverse() * (panel.pose.position - other.pose.position)
        };
        let half = panel.metres() / 2.0;
        let centre = corner + Vec3::new(half.x, -half.y, 0.0);
        let mut depth = OVER_NEARER;
        for _ in 0..self.panels.len() {
            let stacked = self.panels.iter().any(|placed| {
                let at = local(placed);
                let reach = placed.metres() / 2.0 + half;
                (at.z - depth).abs() < OVER_NEARER / 2.0
                    && (at.x - centre.x).abs() < reach.x
                    && (at.y - centre.y).abs() < reach.y
            });
            if !stacked {
                break;
            }
            depth += OVER_NEARER;
        }
        panel.pose.position =
            other.pose.position + other.pose.rotation * (centre + Vec3::Z * depth);
        Some((panel.pose, panel.metres_per_point))
    }

    pub fn close(&mut self, window: WindowId) {
        if window.is_main() {
            return;
        }
        self.panels.retain(|panel| panel.window != window);
        self.scales.retain(|(held, _)| *held != window);
        self.rescaling.retain(|(waiting, ..)| *waiting != window);
        if let Some(compositor) = &self.compositor {
            compositor.forget(window);
        }
        if self.focused == Some(window) {
            self.focused = None;
            self.focus(Some(WindowId::MAIN));
        }
        if self.mouse.captured == Some(window) {
            self.pressed = 0;
        }
        self.mouse.forget(window);
        for laser in &mut self.lasers {
            laser.held.forget(window);
        }
        if self.both.is_some_and(|both| both.window == window) {
            self.both = None;
        }
        if self.chrome.hovered.is_some_and(|(on, _)| on == window) {
            self.chrome.hovered = None;
        }
        if self.chrome.slider == Some(window) {
            self.chrome.slider = None;
        }
        self.chrome_shown.retain(|(on, ..)| *on != window);
    }

    pub fn end_carry(&mut self, pointer: Pointer, out: &mut Vec<Incoming>) {
        let Some(held) = self.held_mut(pointer) else {
            return;
        };
        let Some(mut carry) = held.carry.take() else {
            return;
        };
        if let Some(Gesture::Move { .. }) = held.gesture {
            held.gesture = None;
            if pointer == Pointer::Mouse {
                self.chrome.hovered = self.on_chrome();
            }
        }
        let (head, now, others) = (self.head, self.clock, self.panels.clone());
        let Some(panel) = self.panel_mut(carry.window) else {
            return;
        };
        carry.finish();
        carry.place(&head, panel, &others, now);
        out.push(Incoming::Window(WindowReport::Placed {
            window: carry.window,
            placement: panel.placement(),
        }));
    }

    fn place_carried(&mut self) {
        for pointer in self.pointers() {
            let ray = self.ray(pointer);
            let (head, now, others) = (self.head, self.clock, self.panels.clone());
            let Some(mut carry) = self.held_mut(pointer).and_then(|held| held.carry.take()) else {
                continue;
            };
            if let Some(ray) = ray {
                carry.follow(ray);
            }
            if let Some(panel) = self.panel_mut(carry.window) {
                carry.place(&head, panel, &others, now);
                if pointer == Pointer::Mouse {
                    let grabbed = carry.grabbed_at(&head);
                    self.cursor =
                        Some(head.position + (grabbed - head.position) * (1.0 - CURSOR_LIFT));
                }
            }
            if let Some(held) = self.held_mut(pointer) {
                held.carry = Some(carry);
            }
        }
        self.place_both();
    }

    fn place_both(&mut self) {
        let Some(both) = self.both else {
            return;
        };
        let grips = Hand::BOTH.map(|hand| self.laser(hand).and_then(Laser::grip_point));
        let [Some(left), Some(right)] = grips else {
            return;
        };
        let (from, to) = (both.start[1] - both.start[0], right - left);
        let original = both.original;
        let ratio = to.length() / from.length().max(1e-3);
        let turn = Quat::from_rotation_arc(from.normalize_or(Vec3::X), to.normalize_or(Vec3::X));
        let middle = (both.start[0] + both.start[1]) / 2.0;
        let Some(panel) = self.panel_mut(both.window) else {
            return;
        };
        panel.metres_per_point =
            original.metres_per_point * ratio.clamp(chrome::SMALLEST_SCALE, chrome::LARGEST_SCALE);
        panel.pose.rotation = (turn * original.pose.rotation).normalize();
        panel.pose.position = (left + right) / 2.0 + turn * (original.pose.position - middle);
    }

    fn end_both(&mut self, out: &mut Vec<Incoming>) {
        let Some(both) = self.both.take() else {
            return;
        };
        if let Some(panel) = self.panel(both.window) {
            out.push(Incoming::Window(WindowReport::Placed {
                window: both.window,
                placement: panel.placement(),
            }));
        }
    }

    fn app_locks(&self, output: &FrameOutput) -> bool {
        self.mouse
            .hovered
            .or(self.mouse.captured)
            .filter(|_| self.mouse.gripped().is_none())
            .and_then(|window| output.window(window))
            .is_some_and(|window| window.window.requests.pointer_locked)
    }

    fn window_state(&self, output: &FrameOutput) -> WindowState {
        let mut state = output.main().window.clone();
        state.requests.cursor = CursorIcon::None;
        state.requests.pointer_locked = self.grabbed || self.app_locked;
        state.requests.ime = None;
        state.appearance.clear_color = SKY;
        state
    }
}

impl WindowHost for Space {
    fn setup(&mut self, setup: &Setup) {
        self.compositor = setup.get::<GpuSetup>().map(Compositor::new);
        self.context = setup.get::<Context>().cloned();
        self.shown = None;
        self.sprite.repaint();
        self.chrome_shown.clear();
    }

    fn input(&mut self, incoming: &mut Vec<Incoming>, view: &HostView) {
        self.platform = view.platform;
        self.pixels_per_point = view.pixels_per_point;
        self.clock = Some(view.now);
        self.walk(view.now);
        if self.hold.due(view.now) {
            self.grabbed = false;
        }
        let mut out = std::mem::take(&mut self.reports);
        self.open_main(&mut out);
        for item in incoming.drain(..) {
            self.take(item, &mut out);
        }
        for hand in self
            .lasers
            .iter()
            .map(|laser| laser.hand)
            .collect::<Vec<_>>()
        {
            self.aim_laser(hand, &mut out);
        }
        self.place_carried();
        self.rescale();
        self.remember_cursor();
        out.append(&mut self.reports);
        *incoming = out;
    }

    fn frame(&mut self, output: &FrameOutput, view: &HostView) -> Hosted {
        self.platform = view.platform;
        self.pixels_per_point = view.pixels_per_point;
        self.clock = Some(view.now);
        for request in &output.window_requests {
            self.request(request);
        }
        let resizing: Vec<WindowId> = self
            .holders()
            .filter_map(|held| held.resize.map(|resize| resize.window))
            .collect();
        for window in &output.windows {
            if !resizing.contains(&window.id)
                && let Some(panel) = self.panel_mut(window.id)
            {
                panel.size = window.size;
            }
        }
        self.place_carried();
        self.rescale();
        self.remember_cursor();
        let animating = self.steering() && !self.walking.is_empty()
            || self.holders().any(|held| {
                held.carry
                    .as_ref()
                    .is_some_and(|carry| carry.turning(self.clock))
            })
            || self.lasers.iter().any(Laser::settling);
        let repaint_after = match animating || !self.reports.is_empty() {
            true => Duration::ZERO,
            false => self.settled_in().min(self.hold.due_in(self.clock)),
        };
        self.app_locked = self.app_locks(output);
        let state = self.window_state(output);
        let chrome_changed = self.refresh_chrome(view.pixels_per_point);
        let Some(compositor) = self.compositor.clone() else {
            return Hosted {
                window: WindowOutput {
                    window: state,
                    ..output.main().clone()
                },
                repaint_after,
            };
        };
        let mut changed = chrome_changed;
        for window in &output.windows {
            if self.panel(window.id).is_some() {
                changed |= compositor.update(window);
            }
        }
        let icon = self.cursor_icon(output);
        if self.sprite.changed(icon, view.pixels_per_point) {
            let sprite = self.sprite.paint(icon, view.pixels_per_point);
            changed |= compositor.update(&sprite);
        }
        let shown = Shown {
            platform: view.platform,
            head: self.head,
            panels: self.panels.clone(),
            cursor: self.cursor_at(),
            icon,
            chrome: self
                .chrome_shown
                .iter()
                .map(|(window, look, _)| (*window, *look, self.chrome_opacity(*window)))
                .collect(),
            beams: self.beams(),
        };
        let size = view.platform / view.pixels_per_point;
        if let Some((last, window)) = &mut self.shown
            && *last == shown
            && !changed
        {
            window.changed = false;
            window.window = state;
            window.size = size;
            return Hosted {
                window: window.clone(),
                repaint_after,
            };
        }
        let frame = self.scene(&output.windows, view);
        let rect = Rect::from_min_size(Pos2::ZERO, size);
        let window = WindowOutput {
            id: WindowId::MAIN,
            size,
            layers: Rc::new(vec![Layer::Shape(Shape::Drawing {
                rect,
                drawing: compositor.drawing(frame),
                clip: rect.expand(1.0),
            })]),
            window: state,
            changed: true,
            damage: Region::NOTHING,
            moved: None,
            pixels_per_point: view.pixels_per_point,
        };
        self.shown = Some((shown, window.clone()));
        Hosted {
            window,
            repaint_after,
        }
    }
}

pub struct ViewTarget<'a> {
    pub view: usize,
    pub texture: &'a wgpu::TextureView,
    pub format: wgpu::TextureFormat,
}

impl Space {
    pub fn draw(&mut self, encoder: &mut wgpu::CommandEncoder, targets: &[ViewTarget<'_>]) {
        let (Some(compositor), Some(now)) = (self.compositor.clone(), self.clock) else {
            return;
        };
        let views = self.views();
        let frame = self.scene(
            &[],
            &HostView {
                platform: self.platform,
                pixels_per_point: self.pixels_per_point,
                now,
            },
        );
        let targets: Vec<_> = targets
            .iter()
            .filter_map(|target| {
                let view = views.get(target.view)?;
                Some((target.view, view, target.texture, target.format))
            })
            .collect();
        compositor.draw_views(encoder, &frame, &targets);
    }

    pub(crate) fn scene(&mut self, windows: &[WindowOutput], view: &HostView) -> Frame {
        self.refresh_chrome(view.pixels_per_point);
        Frame {
            pixels_per_point: view.pixels_per_point,
            windows: windows.to_vec(),
            chrome: self
                .chrome_shown
                .iter()
                .map(|(_, _, output)| output.clone())
                .collect(),
            view_projection: self.head.view_projection(view.platform),
            eye: self.head.position,
            right: self.head.rotation() * Vec3::X,
            up: self.head.rotation() * Vec3::Y,
            cursor: self.cursor_at().map(|at| self.cursor_frame(at)),
            beams: self.beams(),
            panels: self
                .panels
                .iter()
                .map(|panel| PanelFrame {
                    window: panel.window,
                    model: panel.model(),
                    metres: panel.metres(),
                    bend: panel.bend,
                    chrome: Some(ChromeFrame {
                        span: chrome::span(panel),
                        opacity: self.chrome_opacity(panel.window),
                    }),
                })
                .collect(),
        }
    }

    fn chrome_hit(&self, ray: Ray) -> Option<(Part, Hit)> {
        let app = self.pick(ray);
        let (part, hit) = chrome::pick(&self.panels, ray, self.chrome.slider)?;
        app.is_none_or(|app| hit.distance < app.distance)
            .then_some((part, hit))
    }

    pub fn chrome_opacity(&self, window: WindowId) -> f32 {
        match self.focused == Some(window) {
            true => 1.0,
            false => chrome::FAINT,
        }
    }

    fn refresh_chrome(&mut self, pixels_per_point: f32) -> bool {
        let mut changed = false;
        let mut shown = Vec::with_capacity(self.panels.len());
        for panel in &self.panels {
            let scale = match self.scale(panel.window) {
                Some(window) => chrome::pixels_per_point(panel, window),
                None => self
                    .context
                    .as_ref()
                    .map_or(pixels_per_point, Context::pixels_per_point),
            };
            let held = self
                .holders()
                .filter_map(|held| held.gesture)
                .find(|gesture| gesture.window() == panel.window)
                .map(|gesture| gesture.part());
            let look = chrome::look(panel, &self.chrome, held, scale);
            let held = self
                .chrome_shown
                .iter()
                .find(|(window, held, _)| *window == panel.window && *held == look);
            match held {
                Some(held) => shown.push(held.clone()),
                None => {
                    let output = chrome::output(panel.window, &look, self.context.as_ref());
                    if let Some(compositor) = &self.compositor {
                        compositor.update_chrome(&output);
                    }
                    changed = true;
                    shown.push((panel.window, look, output));
                }
            }
        }
        self.chrome_shown = shown;
        changed
    }

    fn on_chrome(&self) -> Option<(WindowId, Part)> {
        let (part, hit) = self.chrome_hit(self.cursor_ray()?)?;
        Some((hit.window, part))
    }

    fn chrome_pointer(&mut self, input: &Input, out: &mut Vec<Incoming>) -> bool {
        if self.mouse.gripped().is_some() {
            return false;
        }
        if self.mouse.gesture.is_some() {
            match input {
                Input::PointerButton { pressed: false, .. } => {
                    self.end_gesture(Pointer::Mouse, out)
                }
                Input::PointerButton { pressed: true, .. } => {}
                _ => self.drag_chrome(Pointer::Mouse),
            }
            return true;
        }
        if self.pressed > 0 {
            self.chrome.hovered = None;
            return false;
        }
        self.chrome.hovered = self.on_chrome();
        let Some((window, part)) = self.chrome.hovered else {
            return false;
        };
        match input {
            Input::PointerButton { pressed: false, .. } => return false,
            Input::PointerButton { pressed: true, .. } => {
                self.leave(out);
                self.begin_gesture(Pointer::Mouse, window, part);
            }
            _ => self.leave(out),
        }
        true
    }

    fn begin_gesture(&mut self, pointer: Pointer, window: WindowId, part: Part) {
        let (Some(panel), Some(ray)) = (self.panel(window).copied(), self.ray(pointer)) else {
            return;
        };
        if self.holders().any(|held| held.holds(window))
            || self.both.is_some_and(|both| both.window == window)
        {
            return;
        }
        let at = panel.intersect(ray).map(|hit| hit.pos);
        let gesture = match part {
            Part::Scale(corner) => Gesture::Scale {
                original: panel,
                anchor: corner.opposite().of(panel.size),
                grab: at.unwrap_or(corner.of(panel.size)),
            },
            Part::Move => {
                self.begin_carry(pointer, window);
                if let Some(held) = self.held_mut(pointer)
                    && held.carry.is_some()
                {
                    held.gesture = Some(Gesture::Move { window });
                }
                return;
            }
            Part::Turn => Gesture::Turn {
                window,
                rotation: panel.pose.rotation,
                start: ray,
                sight: chrome::Sight::new(ray.origin, panel.pose.position, self.head.rotation()),
            },
            Part::Roll => Gesture::Roll {
                original: panel,
                start: chrome::roll_angle(&panel, ray).unwrap_or(0.0),
            },
            Part::Bend => {
                self.chrome.slider = match self.chrome.slider {
                    Some(open) if open == window => None,
                    _ => Some(window),
                };
                Gesture::Press { window }
            }
            Part::Slider => Gesture::Bend { original: panel },
        };
        if let Some(held) = self.held_mut(pointer) {
            held.gesture = Some(gesture);
        }
        self.drag_chrome(pointer);
    }

    fn drag_chrome(&mut self, pointer: Pointer) {
        let (Some(gesture), Some(ray)) = (
            self.held(pointer).and_then(|held| held.gesture),
            self.ray(pointer),
        ) else {
            return;
        };
        let Some(panel) = self.panel_mut(gesture.window()) else {
            return;
        };
        match gesture {
            Gesture::Scale {
                original,
                anchor,
                grab,
            } => {
                if let Some(hit) = original.intersect(ray) {
                    let scaled = chrome::scaled(&original, anchor, grab, hit.pos);
                    panel.metres_per_point = scaled.metres_per_point;
                    panel.pose = scaled.pose;
                }
            }
            Gesture::Turn {
                rotation,
                start,
                sight,
                ..
            } => {
                panel.pose.rotation =
                    chrome::turned(rotation, start.direction, ray.direction, sight)
            }
            Gesture::Roll { original, start } => {
                if let Some(angle) = chrome::roll_angle(&original, ray) {
                    panel.pose.rotation = chrome::rolled(&original, start, angle);
                }
            }
            Gesture::Bend { original } => {
                if let Some(hit) = original.intersect(ray) {
                    panel.bend = chrome::bend_of(&original, hit.pos);
                }
            }
            Gesture::Move { .. } | Gesture::Press { .. } => {}
        }
    }

    fn end_gesture(&mut self, pointer: Pointer, out: &mut Vec<Incoming>) {
        let Some(gesture) = self.held_mut(pointer).and_then(|held| held.gesture.take()) else {
            return;
        };
        if matches!(
            gesture,
            Gesture::Scale { .. }
                | Gesture::Turn { .. }
                | Gesture::Roll { .. }
                | Gesture::Bend { .. }
        ) && let Some(panel) = self.panel(gesture.window())
        {
            out.push(Incoming::Window(WindowReport::Placed {
                window: panel.window,
                placement: panel.placement(),
            }));
        }
        if pointer == Pointer::Mouse {
            self.chrome.hovered = self.on_chrome();
        }
    }
}

impl Space {
    fn controller(&mut self, controller: Controller, out: &mut Vec<Incoming>) {
        let hand = controller.hand();
        match controller {
            Controller::Pose { aim, grip, .. } => {
                let Some(aim) = laser::pose_of(aim) else {
                    return;
                };
                let grip = laser::pose_of(grip).unwrap_or(aim);
                self.laser_mut(hand).pose(aim, grip);
                self.aim_laser(hand, out);
            }
            Controller::Toward { window, at, .. } => {
                let Some(target) = self.panel(window).map(|panel| panel.world(at)) else {
                    return;
                };
                let head = self.head;
                let laser = self.laser_mut(hand);
                let origin = laser
                    .aim
                    .map_or_else(|| Laser::resting(hand, &head), |aim| aim.position);
                let aim = Pose {
                    position: origin,
                    rotation: laser::looking(target - origin),
                };
                let grip = laser.grip.map_or(aim, |grip| Pose {
                    rotation: aim.rotation,
                    ..grip
                });
                laser.pose(aim, grip);
                self.aim_laser(hand, out);
            }
            Controller::Trigger { value, .. } => {
                let laser = self.laser_mut(hand);
                laser.trigger = value;
                match laser.pressed {
                    false if value >= laser::PRESS => self.trigger_down(hand, out),
                    true if value <= laser::LET_GO => self.trigger_up(hand, out),
                    _ => {}
                }
            }
            Controller::Squeeze { value, .. } => {
                let laser = self.laser_mut(hand);
                laser.squeeze = value;
                match laser.squeezed {
                    false if value >= laser::PRESS => self.squeeze_down(hand),
                    true if value <= laser::LET_GO => self.squeeze_up(hand, out),
                    _ => {}
                }
            }
            Controller::Lost { .. } => self.lose_laser(hand, out),
        }
    }

    fn aim_laser(&mut self, hand: Hand, out: &mut Vec<Incoming>) {
        let now = self.clock;
        if self.laser_mut(hand).smooth(now) {
            self.laser_moved(hand, out);
        }
    }

    fn touch(hand: Hand, phase: TouchPhase, pos: Pos2, window: WindowId) -> Incoming {
        Incoming::points(Input::Touch {
            id: laser::touch_id(hand),
            phase,
            pos,
            force: None,
        })
        .in_window(window)
    }

    fn laser_moved(&mut self, hand: Hand, out: &mut Vec<Incoming>) {
        let pointer = Pointer::Hand(hand);
        let Some(laser) = self.laser(hand) else {
            return;
        };
        let Some(ray) = laser.aimed() else {
            return;
        };
        let held = &laser.held;
        if held.carry.is_some() {
            return;
        }
        if held.resize.is_some() {
            self.resize_to(pointer, ray, out);
            return;
        }
        if held.gesture.is_some() {
            self.drag_chrome(pointer);
            return;
        }
        let Some(window) = laser.captured().filter(|_| laser.frozen.is_none()) else {
            return;
        };
        let pos = self.beyond(pointer, window, ray);
        let laser = self.laser_mut(hand);
        if laser.sent != Some(pos) {
            laser.sent = Some(pos);
            out.push(Self::touch(hand, TouchPhase::Move, pos, window));
        }
    }

    fn trigger_down(&mut self, hand: Hand, out: &mut Vec<Incoming>) {
        let pointer = Pointer::Hand(hand);
        let laser = self.laser_mut(hand);
        laser.pressed = true;
        laser.frozen = laser.ray;
        let busy = laser.held.gripped().is_some() || laser.held.gesture.is_some();
        let Some(ray) = laser.aimed() else {
            return;
        };
        if busy || self.both.is_some() {
            return;
        }
        if let Some((part, hit)) = self.chrome_hit(ray) {
            self.focus(Some(hit.window));
            self.begin_gesture(pointer, hit.window, part);
            return;
        }
        let Some(hit) = self.pick(ray) else {
            self.focus(None);
            return;
        };
        self.focus(Some(hit.window));
        let laser = self.laser_mut(hand);
        laser.held.captured = Some(hit.window);
        laser.held.last = Some((hit.window, hit.pos));
        laser.sent = Some(hit.pos);
        out.push(Self::touch(hand, TouchPhase::Start, hit.pos, hit.window));
    }

    fn trigger_up(&mut self, hand: Hand, out: &mut Vec<Incoming>) {
        let pointer = Pointer::Hand(hand);
        let laser = self.laser_mut(hand);
        laser.pressed = false;
        let (ray, ending, captured) = (laser.aimed(), laser.ending, laser.held.captured);
        let (carrying, resizing, gesturing) = (
            laser.held.carry.is_some(),
            laser.held.resize.is_some(),
            laser.held.gesture.is_some(),
        );
        let mut at = None;
        if carrying && ending == Ending::Trigger {
            at = captured.map(|window| self.grab_pos(pointer, window));
            self.end_carry(pointer, out);
        } else if resizing {
            at = captured
                .zip(ray)
                .map(|(window, ray)| self.beyond(pointer, window, ray));
            self.end_resize(pointer, out);
        } else if gesturing && !carrying {
            self.end_gesture(pointer, out);
        }
        if let Some(window) = captured {
            let at = at
                .or_else(|| ray.map(|ray| self.beyond(pointer, window, ray)))
                .or(self.laser_mut(hand).sent)
                .unwrap_or(Pos2::ZERO);
            out.push(Self::touch(hand, TouchPhase::End, at, window));
        }
        let laser = self.laser_mut(hand);
        laser.held.captured = None;
        laser.frozen = None;
        laser.sent = None;
    }

    fn squeeze_down(&mut self, hand: Hand) {
        let pointer = Pointer::Hand(hand);
        let laser = self.laser_mut(hand);
        laser.squeezed = true;
        if laser.held.gripped().is_some()
            || laser.held.gesture.is_some()
            || laser.captured().is_some()
        {
            return;
        }
        let Some(ray) = laser.ray else {
            return;
        };
        let Some(window) = self.hit_along(ray).map(|hit| hit.window) else {
            return;
        };
        let other = match hand {
            Hand::Left => Hand::Right,
            Hand::Right => Hand::Left,
        };
        let partner = self.laser(other).is_some_and(|laser| {
            laser.squeezed
                && laser.ending == Ending::Squeeze
                && laser
                    .held
                    .carry
                    .as_ref()
                    .is_some_and(|carry| carry.window == window)
        });
        if partner {
            let grips = Hand::BOTH.map(|hand| self.laser(hand).and_then(Laser::grip_point));
            if let ([Some(left), Some(right)], Some(panel)) = (grips, self.panel(window).copied()) {
                self.laser_mut(other).held.carry = None;
                self.both = Some(Both {
                    window,
                    original: panel,
                    start: [left, right],
                });
            }
            return;
        }
        self.begin_carry(pointer, window);
        let laser = self.laser_mut(hand);
        if laser.held.carry.is_some() {
            laser.ending = Ending::Squeeze;
        }
    }

    fn squeeze_up(&mut self, hand: Hand, out: &mut Vec<Incoming>) {
        let laser = self.laser_mut(hand);
        laser.squeezed = false;
        let carrying = laser.held.carry.is_some() && laser.ending == Ending::Squeeze;
        if self.both.is_some() {
            self.end_both(out);
        } else if carrying {
            self.end_carry(Pointer::Hand(hand), out);
        }
    }

    fn lose_laser(&mut self, hand: Hand, out: &mut Vec<Incoming>) {
        let pointer = Pointer::Hand(hand);
        let laser = self.laser_mut(hand);
        if let Some(window) = laser.held.captured.take() {
            let at = laser.sent.unwrap_or(Pos2::ZERO);
            out.push(Self::touch(hand, TouchPhase::Cancel, at, window));
        }
        self.end_carry(pointer, out);
        self.end_resize(pointer, out);
        self.end_gesture(pointer, out);
        if self.both.is_some() {
            self.end_both(out);
        }
        let laser = self.laser_mut(hand);
        laser.lose();
        laser.pressed = false;
        laser.squeezed = false;
        laser.trigger = 0.0;
        laser.squeeze = 0.0;
        laser.sent = None;
    }

    pub fn beams(&self) -> Vec<Beam> {
        self.lasers
            .iter()
            .filter_map(|laser| {
                let ray = laser.aimed()?;
                let hit = self.hit_along(ray);
                Some(Beam {
                    start: ray.origin,
                    end: hit.map_or(ray.at(laser::REACH), |hit| hit.point),
                    hit: hit.is_some(),
                    pressed: laser.pressed,
                })
            })
            .collect()
    }
}

fn chrome_cursor(part: Part, held: bool) -> CursorIcon {
    match part {
        Part::Scale(Corner::TopLeft | Corner::BottomRight) => CursorIcon::ResizeNwSe,
        Part::Scale(_) => CursorIcon::ResizeNeSw,
        Part::Move | Part::Turn | Part::Roll | Part::Slider if held => CursorIcon::Grabbing,
        Part::Move | Part::Turn | Part::Roll | Part::Slider => CursorIcon::Grab,
        Part::Bend => CursorIcon::PointingHand,
    }
}

fn position(input: &Input) -> Option<Pos2> {
    match input {
        Input::PointerMoved(pos)
        | Input::Predicted(pos)
        | Input::PointerButton { pos, .. }
        | Input::Touch { pos, .. }
        | Input::Pen { pos, .. } => Some(*pos),
        _ => None,
    }
}

fn placed_at(input: Input, pos: Pos2) -> Input {
    let from = position(&input).unwrap_or(Pos2::ZERO);
    input.translated(pos - from)
}

fn edge_point(edge: WindowEdge, size: Vec2) -> Pos2 {
    use WindowEdge::*;
    let x = match edge {
        Left | TopLeft | BottomLeft => 0.0,
        Right | TopRight | BottomRight => size.x,
        Top | Bottom => size.x / 2.0,
    };
    let y = match edge {
        Top | TopLeft | TopRight => 0.0,
        Bottom | BottomLeft | BottomRight => size.y,
        Left | Right => size.y / 2.0,
    };
    pos2(x, y)
}

#[cfg(test)]
impl Space {
    pub(crate) fn cursor_drawn(&self) -> Option<(u64, (u32, u32))> {
        let compositor = self.compositor.as_ref()?;
        compositor.paint_windows();
        compositor.drawn(self.sprite.window)
    }
}
