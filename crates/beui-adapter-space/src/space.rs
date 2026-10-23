use std::rc::Rc;
use std::time::{Duration, Instant};

use beui_core::app::Setup;
use beui_core::color::Color32;
use beui_core::context::{FrameOutput, WindowOutput};
use beui_core::damage::Region;
use beui_core::display::Layer;
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_core::input::{CursorIcon, Input, Key, KeySide, Keystroke, Modifiers};
use beui_core::painter::Shape;
use beui_core::runner::{HostView, Hosted, Incoming, SCROLL_LINE, Units, WindowHost, scrolled};
use beui_core::window::{WindowId, WindowReport, WindowRequest, WindowState};
use beui_renderer_wgpu::present::GpuSetup;
use glam::{Quat, Vec3};

use crate::carry::Carry;
use crate::head::Head;
use crate::panel::{Hit, Panel, Pose, pick};
use crate::render::{Compositor, Frame, PanelFrame};

pub const SKY: Color32 = Color32::from_rgb(22, 25, 31);
const MOST_SECONDS: f32 = 0.1;
const CURSOR_RADIUS: f32 = 0.006;
const FILLED: f32 = 0.8;

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
    compositor: Option<Compositor>,
    platform: Vec2,
    pixels_per_point: f32,
    now: Option<Instant>,
    cursor: Option<Cursor>,
    hovered: Option<WindowId>,
    last: Pos2,
    captured: Option<WindowId>,
    pressed: u32,
    dropped: u32,
    carry_held: bool,
    carry_pressed: u32,
    right_ctrl: bool,
    right_shift: bool,
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
            compositor: None,
            platform: Vec2::ZERO,
            pixels_per_point: 1.0,
            now: None,
            cursor: None,
            hovered: None,
            last: Pos2::ZERO,
            captured: None,
            pressed: 0,
            dropped: 0,
            carry_held: false,
            carry_pressed: 0,
            right_ctrl: false,
            right_shift: false,
            forwarded: Vec::new(),
            swallowed: Vec::new(),
            walking: Vec::new(),
            modifiers: Modifiers::NONE,
            shown: None,
        }
    }

    pub fn steering(&self) -> bool {
        self.right_ctrl && self.right_shift
    }

    pub fn panel(&self, window: WindowId) -> Option<&Panel> {
        self.panels.iter().find(|panel| panel.window == window)
    }

    fn panel_mut(&mut self, window: WindowId) -> Option<&mut Panel> {
        self.panels.iter_mut().find(|panel| panel.window == window)
    }

    pub fn cursor_point(&self) -> Option<Vec3> {
        match self.cursor? {
            Cursor::Platform(at) => {
                let ray = self.head.ray(self.platform, at.to_vec2());
                match self.captured.and_then(|window| self.panel(window)) {
                    Some(panel) => panel.intersect(ray).map(|hit| hit.point),
                    None => pick(&self.panels, ray).map(|hit| hit.point),
                }
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
            .map_or(0.0, |then| now.saturating_duration_since(then).as_secs_f32())
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
                units: Units::Points,
                window,
                input,
            } => self.driven(window, input, out),
            Incoming::Input { units, input, .. } => self.platform_input(units, input, out),
            other => out.push(other),
        }
    }

    fn driven(&mut self, window: WindowId, input: Input, out: &mut Vec<Incoming>) {
        if let Some(pos) = position(&input) {
            self.cursor = Some(Cursor::Window(window, pos));
        }
        if let Input::PointerButton { pressed: false, .. } = input
            && self.carry.is_some()
        {
            self.end_carry(out);
        }
        out.push(Incoming::points(input).in_window(window));
    }

    fn platform_input(&mut self, units: Units, input: Input, out: &mut Vec<Incoming>) {
        match input {
            Input::Key(keystroke) => self.key(units, keystroke, out),
            Input::Modifiers(modifiers) => {
                self.modifiers = modifiers;
                if !self.steering() {
                    out.push(Incoming::new(units, Input::Modifiers(modifiers)));
                }
            }
            Input::PointerMotion(motion) if self.steering() => self.head.look(motion),
            Input::PointerMoved(at) => {
                self.cursor = Some(Cursor::Platform(at));
                match &mut self.carry {
                    Some(carry) => carry.aim(Head::direction(self.platform, at.to_vec2())),
                    None => self.point(Input::PointerMoved(at), at, out),
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
                if self.captured.is_none()
                    && let Some(window) = self.hovered.take()
                {
                    out.push(Incoming::points(Input::PointerLeft).in_window(window));
                }
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
                    if let Some(window) = self.captured.or(self.hovered) {
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

    fn target(&self, at: Pos2) -> Option<Hit> {
        let ray = self.head.ray(self.platform, at.to_vec2());
        match self.captured {
            Some(window) => self.panel(window)?.intersect(ray),
            None => pick(&self.panels, ray),
        }
    }

    fn hover(&mut self, hit: Option<Hit>, out: &mut Vec<Incoming>) {
        if self.captured.is_some() {
            return;
        }
        let now = hit.map(|hit| hit.window);
        if self.hovered != now
            && let Some(window) = self.hovered
        {
            out.push(Incoming::points(Input::PointerLeft).in_window(window));
        }
        self.hovered = now;
    }

    fn point(&mut self, input: Input, at: Pos2, out: &mut Vec<Incoming>) {
        let hit = self.target(at);
        self.hover(hit, out);
        if let Some(hit) = hit {
            self.last = hit.pos;
            out.push(Incoming::points(input.translated(hit.pos - at)).in_window(hit.window));
        }
    }

    fn button(&mut self, input: Input, out: &mut Vec<Incoming>) {
        let Input::PointerButton { pos, pressed, .. } = input else {
            return;
        };
        if let Some(carry) = self.carry {
            match pressed {
                true => self.carry_pressed += 1,
                false if self.carry_held => {
                    let grab = self.grab_pos(carry.window);
                    self.end_carry(out);
                    self.release(input.translated(grab - pos), carry.window, out);
                }
                false => {
                    self.carry_pressed = self.carry_pressed.saturating_sub(1);
                    if self.carry_pressed == 0 {
                        self.end_carry(out);
                    }
                }
            }
            return;
        }
        match pressed {
            true => match self.target(pos) {
                Some(hit) => {
                    self.hover(Some(hit), out);
                    self.pressed += 1;
                    self.captured = Some(hit.window);
                    self.last = hit.pos;
                    out.push(
                        Incoming::points(input.translated(hit.pos - pos)).in_window(hit.window),
                    );
                }
                None => self.dropped += 1,
            },
            false if self.dropped > 0 => self.dropped -= 1,
            false => {
                let Some(window) = self.captured else {
                    return;
                };
                match self.target(pos) {
                    Some(hit) => self.release(input.translated(hit.pos - pos), window, out),
                    None => self.release(input.translated(self.last - pos), window, out),
                }
                if self.captured.is_none() {
                    let hit = self.target(pos);
                    self.hover(hit, out);
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

    fn key(&mut self, units: Units, keystroke: Keystroke, out: &mut Vec<Incoming>) {
        let stroke = Stroke::of(&keystroke);
        let right_ctrl = keystroke.is(Key::Ctrl, KeySide::Right);
        let right_shift = keystroke.is(Key::Shift, KeySide::Right);
        if right_ctrl || right_shift {
            let was = self.steering();
            match right_ctrl {
                true => self.right_ctrl = keystroke.pressed,
                false => self.right_shift = keystroke.pressed,
            }
            let now = self.steering();
            if !was && now {
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
                        ..self.modifiers
                    }),
                ));
                self.swallowed.push(stroke);
                return;
            }
            if was && !now {
                self.walking.clear();
                self.swallowed.retain(|held| *held != stroke);
                out.push(Incoming::new(units, Input::Modifiers(self.modifiers)));
                return;
            }
            if now || self.swallowed.contains(&stroke) {
                if !keystroke.pressed {
                    self.swallowed.retain(|held| *held != stroke);
                }
                return;
            }
            match keystroke.pressed {
                true if !keystroke.repeat => self.forwarded.push(keystroke.clone()),
                true => {}
                false => self.forwarded.retain(|held| Stroke::of(held) != stroke),
            }
            out.push(Incoming::new(units, Input::Key(keystroke)));
            return;
        }
        let placed = keystroke
            .evdev
            .and_then(Key::from_evdev)
            .or(keystroke.key);
        let walks = matches!(placed, Some(Key::W | Key::A | Key::S | Key::D));
        if self.steering() && walks {
            let Some(key) = placed else {
                return;
            };
            match keystroke.pressed {
                true => {
                    if !self.walking.contains(&key) {
                        self.walking.push(key);
                    }
                    if !self.swallowed.contains(&stroke) {
                        self.swallowed.push(stroke);
                    }
                }
                false => {
                    self.walking.retain(|held| *held != key);
                    self.swallowed.retain(|held| *held != stroke);
                }
            }
            return;
        }
        if self.swallowed.contains(&stroke) {
            if !keystroke.pressed {
                self.swallowed.retain(|held| *held != stroke);
                if let Some(key) = placed {
                    self.walking.retain(|held| *held != key);
                }
            }
            return;
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
        self.carry_held = self.captured == Some(window) && self.pressed > 0;
        self.carry_pressed = 0;
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
        let over = self.captured.or(self.hovered).is_some() && self.carry.is_none();
        state.requests.cursor = match over {
            true => state.requests.cursor,
            false => CursorIcon::Default,
        };
        state.requests.pointer_locked =
            self.steering() || (over && state.requests.pointer_locked);
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
        let mut out = Vec::with_capacity(incoming.len() + 1);
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
            if let WindowRequest::BeginMove(window) = request {
                self.begin_carry(*window);
            }
        }
        for window in &output.windows {
            if let Some(panel) = self.panel_mut(window.id) {
                panel.size = window.size;
            }
        }
        self.place_carried();
        let repaint_after = match self.steering() && !self.walking.is_empty() {
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
