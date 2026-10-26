use std::rc::Rc;
use std::time::{Duration, Instant};

use beui_core::app::Setup;
use beui_core::color::Color32;
use beui_core::context::{Context, FrameOutput, WindowOutput};
use beui_core::damage::Region;
use beui_core::display::Layer;
use beui_core::geometry::{Pos2, Rect, Vec2, pos2};
use beui_core::input::{CursorIcon, Input, Key, Keystroke, Modifiers, PointerButton};
use beui_core::painter::Shape;
use beui_core::runner::{HostView, Hosted, Incoming, SCROLL_LINE, Units, WindowHost, scrolled};
use beui_core::window::{
    WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest, WindowState,
};
use beui_renderer_wgpu::present::GpuSetup;
use glam::{Mat4, Quat, Vec3};

use crate::carry::{Carry, NEAREST};
use crate::chrome::{self, Chrome, Corner, Gesture, Look, Part};
use crate::cursor::{self, Sprite};
use crate::head::{Head, Ray};
use crate::hold::Hold;
use crate::panel::{Hit, Panel, Pose, pick};
use crate::render::{ChromeFrame, Compositor, Frame, PanelFrame};
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

#[derive(Clone, PartialEq, Debug)]
struct Shown {
    platform: Vec2,
    head: Head,
    panels: Vec<Panel>,
    cursor: Option<Vec3>,
    icon: CursorIcon,
    chrome: Vec<(WindowId, Look, f32)>,
}

pub struct Space {
    pub head: Head,
    pub panels: Vec<Panel>,
    pub carry: Option<Carry>,
    pub resize: Option<Resize>,
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
    hovered: Option<WindowId>,
    focused: Option<WindowId>,
    last: Option<(WindowId, Pos2)>,
    captured: Option<WindowId>,
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
            panels: Vec::new(),
            carry: None,
            resize: None,
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
            hovered: None,
            focused: None,
            last: None,
            captured: None,
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

    pub fn scale(&self, window: WindowId) -> Option<f32> {
        self.scales
            .iter()
            .find(|(held, _)| *held == window)
            .map(|(_, scale)| *scale)
    }

    fn rescale(&mut self) {
        let gripped = [
            self.gripped(),
            self.chrome.gesture.as_ref().map(Gesture::window),
        ];
        for panel in &self.panels {
            let window = panel.window;
            let held = self.scale(window);
            let wanted = crate::scale::wanted(panel, &self.head, self.platform);
            let scale = crate::scale::chosen(held, wanted, panel.size, self.pixels_per_point);
            if held.is_some() {
                if held == Some(scale) || gripped.contains(&Some(window)) {
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
        let held = self.chrome.gesture.map(|gesture| gesture.part());
        if let Some(part) = held.or(self.chrome.hovered.map(|(_, part)| part)) {
            return chrome_cursor(part, held.is_some());
        }
        self.hovered
            .or(self.captured)
            .or(self.gripped())
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
            Input::Scroll(delta) if self.carry.is_some() => {
                let lines = scrolled(delta, self.pixels_per_point).y / SCROLL_LINE;
                let (head, now, others) = (self.head, self.clock, self.panels.clone());
                if let Some(carry) = &mut self.carry
                    && let Some(panel) = others.iter().find(|panel| panel.window == carry.window)
                {
                    carry.scroll(lines, &head, panel, &others, now);
                }
            }
            Input::ScrollStarted | Input::ScrollEnded if self.carry.is_some() => {}
            Input::PointerLeft => {
                self.cursor = None;
                self.leave(out);
            }
            Input::PointersCancelled => {
                self.captured = None;
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
                        if let Some(window) = self.hovered.or(self.captured) {
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
        match (&mut self.carry, self.resize) {
            (Some(carry), _) => carry.aim(&self.head, direction),
            (None, Some(_)) => self.resize_to(ray, out),
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
        if pressed && self.gripped().is_none() && self.chrome.gesture.is_none() {
            self.focus(self.pressed_on(ray));
        }
        if !self.chrome_pointer(&input, out) {
            self.button(input, ray, out);
        }
    }

    fn leave(&mut self, out: &mut Vec<Incoming>) {
        if let Some(window) = self.hovered.take() {
            out.push(Incoming::points(Input::PointerLeft).in_window(window));
        }
    }

    fn beyond(&mut self, window: WindowId, ray: Ray) -> Pos2 {
        match self.panel(window).and_then(|panel| panel.intersect(ray)) {
            Some(hit) => {
                self.last = Some((window, hit.pos));
                hit.pos
            }
            None => match self.last {
                Some((last, pos)) if last == window => pos,
                _ => pos2(-1.0, -1.0),
            },
        }
    }

    fn point(&mut self, input: Input, ray: Ray, out: &mut Vec<Incoming>) {
        match self.pick(ray) {
            Some(hit) => {
                self.hovered = Some(hit.window);
                self.last = Some((hit.window, hit.pos));
                out.push(Incoming::points(placed_at(input, hit.pos)).in_window(hit.window));
            }
            None => {
                self.leave(out);
                if let Some(window) = self.captured.filter(|_| self.pressed > 0) {
                    let pos = self.beyond(window, ray);
                    out.push(Incoming::points(placed_at(input, pos)).in_window(window));
                }
            }
        }
    }

    fn gripped(&self) -> Option<WindowId> {
        self.carry
            .as_ref()
            .map(|carry| carry.window)
            .or(self.resize.map(|resize| resize.window))
    }

    fn button(&mut self, input: Input, ray: Ray, out: &mut Vec<Incoming>) {
        let Input::PointerButton { pressed, .. } = input else {
            return;
        };
        if let Some(window) = self.gripped() {
            match pressed {
                true => self.grip_pressed += 1,
                false if self.grip_held => {
                    let grab = match &self.carry {
                        Some(_) => self.grab_pos(window),
                        None => self.beyond(window, ray),
                    };
                    self.end_grip(out);
                    self.release(placed_at(input, grab), window, out);
                }
                false => {
                    self.grip_pressed = self.grip_pressed.saturating_sub(1);
                    if self.grip_pressed == 0 {
                        self.end_grip(out);
                    }
                }
            }
            return;
        }
        match pressed {
            true => match self.pick(ray) {
                Some(hit) => {
                    self.hovered = Some(hit.window);
                    if self.pressed == 0 {
                        self.captured = Some(hit.window);
                    }
                    self.pressed += 1;
                    self.last = Some((hit.window, hit.pos));
                    out.push(Incoming::points(placed_at(input, hit.pos)).in_window(hit.window));
                }
                None => self.dropped += 1,
            },
            false if self.dropped > 0 => self.dropped -= 1,
            false => {
                let Some(window) = self.captured.filter(|_| self.pressed > 0) else {
                    return;
                };
                match self.pick(ray) {
                    Some(hit) => {
                        self.hovered = Some(hit.window);
                        self.last = Some((hit.window, hit.pos));
                        self.release(placed_at(input, hit.pos), hit.window, out);
                    }
                    None => {
                        self.leave(out);
                        let at = self.beyond(window, ray);
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
            self.captured = None;
        }
    }

    fn grab_pos(&self, window: WindowId) -> Pos2 {
        let Some(panel) = self.panel(window) else {
            return Pos2::ZERO;
        };
        self.cursor_ray()
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

    pub fn begin_carry(&mut self, window: WindowId) {
        if self.carry.is_some() {
            return;
        }
        let Some(panel) = self.panel(window) else {
            return;
        };
        let grab = self
            .cursor_ray()
            .and_then(|ray| panel.intersect(ray))
            .map_or(panel.pose.position, |hit| hit.point);
        self.carry = Some(Carry::begin(panel, &self.head, grab, self.clock));
        self.focus(Some(window));
        self.grip(window);
    }

    fn grip(&mut self, window: WindowId) {
        self.grip_held = self.captured == Some(window) && self.pressed > 0;
        self.grip_pressed = 0;
    }

    fn end_grip(&mut self, out: &mut Vec<Incoming>) {
        self.end_carry(out);
        self.end_resize(out);
    }

    pub fn begin_resize(&mut self, window: WindowId, edge: WindowEdge) {
        if self.gripped().is_some() {
            return;
        }
        let Some(panel) = self.panel(window) else {
            return;
        };
        let grab = self
            .cursor_ray()
            .and_then(|ray| panel.intersect(ray))
            .map_or_else(|| edge_point(edge, panel.size), |hit| hit.pos);
        self.resize = Some(Resize::begin(panel, edge, grab));
        self.grip(window);
    }

    fn resize_to(&mut self, ray: Ray, out: &mut Vec<Incoming>) {
        let Some(resize) = self.resize else {
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

    pub fn end_resize(&mut self, out: &mut Vec<Incoming>) {
        let Some(resize) = self.resize.take() else {
            return;
        };
        if let Some(panel) = self.panel(resize.window) {
            out.push(Incoming::Window(WindowReport::Placed {
                window: resize.window,
                placement: panel.placement(),
            }));
        }
    }

    pub fn request(&mut self, request: &WindowRequest) {
        match request {
            WindowRequest::Open {
                window,
                placement,
                size,
            } => self.open(*window, placement, *size),
            WindowRequest::BeginMove(window) => self.begin_carry(*window),
            WindowRequest::BeginResize(window, edge) => self.begin_resize(*window, *edge),
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
        if self.resize.is_some_and(|resize| resize.window == window) {
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
        let ray = self.cursor_ray().unwrap_or(Ray {
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
        if self.hovered == Some(window) {
            self.hovered = None;
        }
        if self.focused == Some(window) {
            self.focused = None;
            self.focus(Some(WindowId::MAIN));
        }
        if self.captured == Some(window) {
            self.captured = None;
            self.pressed = 0;
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
        if self.chrome.hovered.is_some_and(|(on, _)| on == window) {
            self.chrome.hovered = None;
        }
        if self
            .chrome
            .gesture
            .is_some_and(|gesture| gesture.window() == window)
        {
            self.chrome.gesture = None;
        }
        if self.chrome.slider == Some(window) {
            self.chrome.slider = None;
        }
        self.chrome_shown.retain(|(on, ..)| *on != window);
    }

    pub fn end_carry(&mut self, out: &mut Vec<Incoming>) {
        let Some(mut carry) = self.carry.take() else {
            return;
        };
        if let Some(Gesture::Move { .. }) = self.chrome.gesture {
            self.chrome.gesture = None;
            self.chrome.hovered = self.on_chrome();
        }
        let (head, now, others) = (self.head, self.clock, self.panels.clone());
        let Some(panel) = self.panel_mut(carry.window) else {
            return;
        };
        carry.place(&head, panel, &others, now);
        panel.face(head.position, head.up());
        out.push(Incoming::Window(WindowReport::Placed {
            window: carry.window,
            placement: panel.placement(),
        }));
    }

    fn place_carried(&mut self) {
        let (head, now, others) = (self.head, self.clock, self.panels.clone());
        let Some(mut carry) = self.carry.take() else {
            return;
        };
        if let Some(panel) = self.panel_mut(carry.window) {
            carry.place(&head, panel, &others, now);
            let grabbed = carry.grabbed_at(&head);
            self.cursor = Some(head.position + (grabbed - head.position) * (1.0 - CURSOR_LIFT));
        }
        self.carry = Some(carry);
    }

    fn app_locks(&self, output: &FrameOutput) -> bool {
        self.hovered
            .or(self.captured)
            .filter(|_| self.gripped().is_none())
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
        let resizing = self.resize.map(|resize| resize.window);
        for window in &output.windows {
            if Some(window.id) != resizing
                && let Some(panel) = self.panel_mut(window.id)
            {
                panel.size = window.size;
            }
        }
        self.place_carried();
        self.rescale();
        self.remember_cursor();
        let animating = self.steering() && !self.walking.is_empty()
            || self
                .carry
                .as_ref()
                .is_some_and(|carry| carry.turning(self.clock));
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

impl Space {
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
            let scale = self
                .context
                .as_ref()
                .map_or(pixels_per_point, Context::pixels_per_point);
            let look = chrome::look(panel, &self.chrome, scale);
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
        if self.gripped().is_some() {
            return false;
        }
        if self.chrome.gesture.is_some() {
            match input {
                Input::PointerButton { pressed: false, .. } => self.end_gesture(out),
                Input::PointerButton { pressed: true, .. } => {}
                _ => self.drag_chrome(),
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
                self.begin_gesture(window, part);
            }
            _ => self.leave(out),
        }
        true
    }

    fn begin_gesture(&mut self, window: WindowId, part: Part) {
        let (Some(panel), Some(ray)) = (self.panel(window).copied(), self.cursor_ray()) else {
            return;
        };
        let at = panel.intersect(ray).map(|hit| hit.pos);
        self.chrome.gesture = Some(match part {
            Part::Scale(corner) => Gesture::Scale {
                original: panel,
                anchor: corner.opposite().of(panel.size),
                grab: at.unwrap_or(corner.of(panel.size)),
            },
            Part::Move => {
                self.begin_carry(window);
                self.chrome.gesture = Some(Gesture::Move { window });
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
        });
        self.drag_chrome();
    }

    fn drag_chrome(&mut self) {
        let (Some(gesture), Some(ray)) = (self.chrome.gesture, self.cursor_ray()) else {
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

    fn end_gesture(&mut self, out: &mut Vec<Incoming>) {
        let Some(gesture) = self.chrome.gesture.take() else {
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
        self.chrome.hovered = self.on_chrome();
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
