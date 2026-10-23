use std::rc::Rc;
use std::time::{Duration, Instant};

use beui_core::app::Setup;
use beui_core::color::Color32;
use beui_core::context::{FrameOutput, WindowOutput};
use beui_core::damage::Region;
use beui_core::display::Layer;
use beui_core::geometry::{Pos2, Rect, Vec2, pos2};
use beui_core::input::{CursorIcon, Input, Key, Keystroke, Modifiers};
use beui_core::painter::Shape;
use beui_core::runner::{HostView, Hosted, Incoming, SCROLL_LINE, Units, WindowHost, scrolled};
use beui_core::window::{
    WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest, WindowState,
};
use beui_renderer_wgpu::present::GpuSetup;
use glam::{Quat, Vec3};

use crate::carry::{Carry, NEAREST};
use crate::head::{Head, Ray};
use crate::panel::{POINTS_PER_METRE, Panel, Pose, pick};
use crate::render::{Compositor, Frame, PanelFrame};
use crate::resize::Resize;

pub const SKY: Color32 = Color32::from_rgb(22, 25, 31);
const MOST_SECONDS: f32 = 0.1;
const CURSOR_RADIUS: f32 = 0.006;
const FILLED: f32 = 0.8;
pub const OPEN_DISTANCE: f32 = 1.0;
pub const OPEN_NEARER: f32 = 0.1;
pub const OPEN_GRAB: Pos2 = pos2(24.0, 16.0);
pub const BESIDE_GAP: f32 = 0.05;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Cursor {
    Platform(Pos2),
    Window(WindowId, Pos2),
}

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
}

pub struct Space {
    pub head: Head,
    pub panels: Vec<Panel>,
    pub carry: Option<Carry>,
    pub resize: Option<Resize>,
    compositor: Option<Compositor>,
    platform: Vec2,
    pixels_per_point: f32,
    now: Option<Instant>,
    cursor: Option<Cursor>,
    hovered: Option<WindowId>,
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
            compositor: None,
            platform: Vec2::ZERO,
            pixels_per_point: 1.0,
            now: None,
            cursor: None,
            hovered: None,
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
        }
    }

    pub fn steering(&self) -> bool {
        self.modifiers.ctrl && self.modifiers.shift
    }

    pub fn panel(&self, window: WindowId) -> Option<&Panel> {
        self.panels.iter().find(|panel| panel.window == window)
    }

    fn panel_mut(&mut self, window: WindowId) -> Option<&mut Panel> {
        self.panels.iter_mut().find(|panel| panel.window == window)
    }

    pub fn cursor_ray(&self) -> Option<Ray> {
        match self.cursor? {
            Cursor::Platform(at) => Some(self.head.ray(self.platform, at.to_vec2())),
            Cursor::Window(window, pos) => {
                let point = self.panel(window)?.world(pos);
                Some(Ray {
                    origin: self.head.position,
                    direction: (point - self.head.position).normalize_or(Vec3::NEG_Z),
                })
            }
        }
    }

    pub fn cursor_point(&self) -> Option<Vec3> {
        match self.cursor? {
            Cursor::Platform(at) => {
                pick(&self.panels, self.head.ray(self.platform, at.to_vec2())).map(|hit| hit.point)
            }
            Cursor::Window(window, pos) => self.panel(window).map(|panel| panel.world(pos)),
        }
    }

    fn open_main(&mut self, incoming: &mut Vec<Incoming>) {
        if self.panel(WindowId::MAIN).is_some() || self.platform.x < 1.0 || self.platform.y < 1.0 {
            return;
        }
        let size = self.platform / self.pixels_per_point;
        let mut panel = Panel {
            window: WindowId::MAIN,
            pose: Pose {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            size,
        };
        let half_width = (crate::head::FOV_Y / 2.0).tan() * self.platform.x / self.platform.y;
        let distance = panel.metres().x / 2.0 / (FILLED * half_width);
        panel.pose.position = self.head.position + Vec3::new(0.0, 0.0, -distance);
        self.panels.push(panel);
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
    }

    fn take(&mut self, incoming: Incoming, out: &mut Vec<Incoming>) {
        match incoming {
            Incoming::Input {
                units,
                input: input @ (Input::Key(_) | Input::Modifiers(_)),
                ..
            } => self.platform_input(units, input, out),
            Incoming::Input {
                units,
                input: Input::PointerMotion(motion),
                ..
            } if self.steering() => self.head.look(self.pixels(units, motion)),
            Incoming::Input {
                units: Units::Points,
                window,
                input,
            } => self.driven(window, input, out),
            Incoming::Input { units, input, .. } => self.platform_input(units, input, out),
            other => out.push(other),
        }
    }

    fn pixels(&self, units: Units, motion: Vec2) -> Vec2 {
        match units {
            Units::Points => motion * self.pixels_per_point,
            Units::Physical | Units::Logical => motion,
        }
    }

    fn driven(&mut self, window: WindowId, input: Input, out: &mut Vec<Incoming>) {
        if let Some(pos) = position(&input) {
            self.cursor = Some(Cursor::Window(window, pos));
        }
        if self.resize.is_some()
            && let Some(ray) = self.cursor_ray()
        {
            self.resize_to(ray, out);
        }
        if let Input::PointerButton { pressed: false, .. } = input
            && self.gripped().is_some()
        {
            self.end_grip(out);
        }
        out.push(Incoming::points(input).in_window(window));
    }

    fn platform_input(&mut self, units: Units, input: Input, out: &mut Vec<Incoming>) {
        match input {
            Input::Key(keystroke) => self.key(units, keystroke, out),
            Input::Modifiers(modifiers) => self.modifiers(units, modifiers, out),
            Input::PointerMoved(at) => {
                self.cursor = Some(Cursor::Platform(at));
                let ray = self.head.ray(self.platform, at.to_vec2());
                match (&mut self.carry, self.resize) {
                    (Some(carry), _) => {
                        carry.aim(&self.head, Head::direction(self.platform, at.to_vec2()));
                    }
                    (None, Some(_)) => self.resize_to(ray, out),
                    (None, None) => self.point(Input::PointerMoved(at), at, out),
                }
            }
            Input::PointerButton { pos, .. } => {
                self.cursor = Some(Cursor::Platform(pos));
                self.button(input, out);
            }
            Input::Scroll(delta) if self.carry.is_some() => {
                let lines = scrolled(delta, self.pixels_per_point).y / SCROLL_LINE;
                if let Some(carry) = &mut self.carry {
                    carry.scroll(lines);
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
            input => match position(&input) {
                Some(at) => self.point(input, at, out),
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
            },
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

    fn point(&mut self, input: Input, at: Pos2, out: &mut Vec<Incoming>) {
        let ray = self.head.ray(self.platform, at.to_vec2());
        match pick(&self.panels, ray) {
            Some(hit) => {
                self.hovered = Some(hit.window);
                self.last = Some((hit.window, hit.pos));
                out.push(Incoming::points(input.translated(hit.pos - at)).in_window(hit.window));
            }
            None => {
                self.leave(out);
                if let Some(window) = self.captured.filter(|_| self.pressed > 0) {
                    let pos = self.beyond(window, ray);
                    out.push(Incoming::points(input.translated(pos - at)).in_window(window));
                }
            }
        }
    }

    fn gripped(&self) -> Option<WindowId> {
        self.carry
            .map(|carry| carry.window)
            .or(self.resize.map(|resize| resize.window))
    }

    fn button(&mut self, input: Input, out: &mut Vec<Incoming>) {
        let Input::PointerButton { pos, pressed, .. } = input else {
            return;
        };
        let ray = self.head.ray(self.platform, pos.to_vec2());
        if let Some(window) = self.gripped() {
            match pressed {
                true => self.grip_pressed += 1,
                false if self.grip_held => {
                    let grab = match self.carry {
                        Some(_) => self.grab_pos(window),
                        None => self.beyond(window, ray),
                    };
                    self.end_grip(out);
                    self.release(input.translated(grab - pos), window, out);
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
            true => match pick(&self.panels, ray) {
                Some(hit) => {
                    self.hovered = Some(hit.window);
                    if self.pressed == 0 {
                        self.captured = Some(hit.window);
                    }
                    self.pressed += 1;
                    self.last = Some((hit.window, hit.pos));
                    out.push(
                        Incoming::points(input.translated(hit.pos - pos)).in_window(hit.window),
                    );
                }
                None => self.dropped += 1,
            },
            false if self.dropped > 0 => self.dropped -= 1,
            false => {
                let Some(window) = self.captured.filter(|_| self.pressed > 0) else {
                    return;
                };
                match pick(&self.panels, ray) {
                    Some(hit) => {
                        self.hovered = Some(hit.window);
                        self.last = Some((hit.window, hit.pos));
                        self.release(input.translated(hit.pos - pos), hit.window, out);
                    }
                    None => {
                        self.leave(out);
                        let at = self.beyond(window, ray);
                        self.release(input.translated(at - pos), window, out);
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
        match self.cursor {
            Some(Cursor::Platform(at)) => panel
                .intersect(self.head.ray(self.platform, at.to_vec2()))
                .map_or(Pos2::ZERO, |hit| hit.pos),
            Some(Cursor::Window(_, pos)) => pos,
            None => Pos2::ZERO,
        }
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
        let walks = matches!(placed, Some(Key::W | Key::A | Key::S | Key::D));
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
        let grab = match self.cursor {
            Some(Cursor::Window(on, pos)) if on == window => panel.world(pos),
            Some(Cursor::Platform(at)) => panel
                .intersect(self.head.ray(self.platform, at.to_vec2()))
                .map_or(panel.pose.position, |hit| hit.point),
            _ => panel.pose.position,
        };
        self.carry = Some(Carry::begin(panel, &self.head, grab));
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
                placement: panel.pose.placement(),
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
        if self.panel(window).is_none() {
            let pose = match placement {
                WindowPlacement::Host(placement) => Pose::from_placement(placement),
                WindowPlacement::Beside(other) => self.beside(*other, size),
                WindowPlacement::AtPointer => None,
            }
            .unwrap_or_else(|| self.at_pointer(size));
            self.panels.push(Panel { window, pose, size });
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
                placement: panel.pose.placement(),
            },
        ];
        self.reports.extend(reports.map(Incoming::Window));
    }

    pub fn resize_window(&mut self, window: WindowId, size: Vec2) {
        if self.resize.is_some_and(|resize| resize.window == window) {
            return;
        }
        let Some(panel) = self.panel_mut(window) else {
            return;
        };
        let corner = panel.world(Pos2::ZERO);
        panel.size = size;
        panel.pose.position = corner - panel.pose.rotation * panel.offset(Pos2::ZERO);
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
        let mut panel = Panel {
            window: WindowId::MAIN,
            pose: Pose {
                position: point,
                rotation: Quat::IDENTITY,
            },
            size,
        };
        panel.face(self.head.position);
        let grab = pos2(OPEN_GRAB.x.min(size.x / 2.0), OPEN_GRAB.y.min(size.y / 2.0));
        panel.pose.position = point - panel.pose.rotation * panel.offset(grab);
        panel.pose
    }

    fn beside(&self, other: WindowId, size: Vec2) -> Option<Pose> {
        let other = self.panel(other)?;
        let metres = glam::Vec2::new(size.x, size.y) / POINTS_PER_METRE;
        let local = |panel: &Panel| {
            other.pose.rotation.inverse() * (panel.pose.position - other.pose.position)
        };
        let top = other.metres().y / 2.0;
        let mut left = other.metres().x / 2.0 + BESIDE_GAP;
        for _ in 0..self.panels.len() {
            let blocking = self.panels.iter().find(|panel| {
                let at = local(panel);
                let half = panel.metres() / 2.0;
                at.z.abs() < 0.01
                    && at.x + half.x > left
                    && at.x - half.x < left + metres.x
                    && at.y + half.y > top - metres.y
                    && at.y - half.y < top
            });
            let Some(blocking) = blocking else {
                break;
            };
            left = local(blocking).x + blocking.metres().x / 2.0 + BESIDE_GAP;
        }
        let shift = Vec3::new(left + metres.x / 2.0, top - metres.y / 2.0, 0.0);
        Some(Pose {
            position: other.pose.position + other.pose.rotation * shift,
            rotation: other.pose.rotation,
        })
    }

    pub fn close(&mut self, window: WindowId) {
        if window.is_main() {
            return;
        }
        self.panels.retain(|panel| panel.window != window);
        if let Some(compositor) = &self.compositor {
            compositor.forget(window);
        }
        if self.hovered == Some(window) {
            self.hovered = None;
        }
        if self.captured == Some(window) {
            self.captured = None;
            self.pressed = 0;
        }
        if self.carry.is_some_and(|carry| carry.window == window) {
            self.carry = None;
        }
        if self.resize.is_some_and(|resize| resize.window == window) {
            self.resize = None;
        }
    }

    pub fn end_carry(&mut self, out: &mut Vec<Incoming>) {
        let Some(carry) = self.carry.take() else {
            return;
        };
        let head = self.head;
        let Some(panel) = self.panel_mut(carry.window) else {
            return;
        };
        carry.place(&head, panel);
        panel.face(head.position);
        out.push(Incoming::Window(WindowReport::Placed {
            window: carry.window,
            placement: panel.pose.placement(),
        }));
    }

    fn place_carried(&mut self) {
        let head = self.head;
        if let Some(carry) = self.carry
            && let Some(panel) = self.panel_mut(carry.window)
        {
            carry.place(&head, panel);
        }
    }

    fn window_state(&self, output: &FrameOutput) -> WindowState {
        let mut state = output.main().window.clone();
        let over = self
            .hovered
            .or(self.captured)
            .filter(|_| self.gripped().is_none())
            .and_then(|window| output.window(window))
            .map(|window| &window.window.requests);
        state.requests.cursor = over.map_or(CursorIcon::Default, |requests| requests.cursor);
        state.requests.pointer_locked =
            self.steering() || over.is_some_and(|requests| requests.pointer_locked);
        state.requests.ime = None;
        state.appearance.clear_color = SKY;
        state
    }
}

impl WindowHost for Space {
    fn setup(&mut self, setup: &Setup) {
        self.compositor = setup.get::<GpuSetup>().map(Compositor::new);
        self.shown = None;
    }

    fn input(&mut self, incoming: &mut Vec<Incoming>, view: &HostView) {
        self.platform = view.platform;
        self.pixels_per_point = view.pixels_per_point;
        self.walk(view.now);
        let mut out = std::mem::take(&mut self.reports);
        self.open_main(&mut out);
        for item in incoming.drain(..) {
            self.take(item, &mut out);
        }
        *incoming = out;
        self.place_carried();
    }

    fn frame(&mut self, output: &FrameOutput, view: &HostView) -> Hosted {
        self.platform = view.platform;
        self.pixels_per_point = view.pixels_per_point;
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
        let animating = self.steering() && !self.walking.is_empty();
        let repaint_after = match animating || !self.reports.is_empty() {
            true => Duration::ZERO,
            false => Duration::MAX,
        };
        let Some(compositor) = &self.compositor else {
            return Hosted {
                window: output.main().clone(),
                repaint_after,
            };
        };
        let mut changed = false;
        for window in &output.windows {
            if self.panel(window.id).is_some() {
                changed |= compositor.update(window, view.pixels_per_point);
            }
        }
        let shown = Shown {
            platform: view.platform,
            head: self.head,
            panels: self.panels.clone(),
            cursor: self.cursor_point(),
        };
        let size = view.platform / view.pixels_per_point;
        let state = self.window_state(output);
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
        let distance = shown
            .cursor
            .map_or(1.0, |point| point.distance(self.head.position));
        let frame = Frame {
            view_projection: self.head.view_projection(view.platform),
            eye: self.head.position,
            right: self.head.rotation() * Vec3::X,
            up: self.head.rotation() * Vec3::Y,
            cursor: shown.cursor.map(|point| {
                let toward = (self.head.position - point).normalize_or_zero();
                (point + toward * 0.002, CURSOR_RADIUS * distance)
            }),
            panels: self
                .panels
                .iter()
                .map(|panel| PanelFrame {
                    window: panel.window,
                    model: panel.model(),
                })
                .collect(),
        };
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
        };
        self.shown = Some((shown, window.clone()));
        Hosted {
            window,
            repaint_after,
        }
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
