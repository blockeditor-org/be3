#![cfg(not(any(target_os = "android", target_arch = "wasm32")))]

mod clipboard;
mod file_picker;
#[cfg(test)]
mod tests;

pub use winit;

use std::error::Error;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

use accesskit_winit::{Adapter as AccessKitAdapter, Event as AccessKitEvent};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::{
    DeviceEvent, DeviceId, ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, NamedKey, PhysicalKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{
    CursorGrabMode, CustomCursor, CustomCursorSource, Fullscreen, Window, WindowId,
};

use accesskit::TreeUpdate;
use beui_core::app::{SafeArea, Setup, Waker};
use beui_core::context::Context;
use beui_core::file_picker::FilePickRequest;
use beui_core::geometry::Vec2;
use beui_core::geometry::{Pos2, pos2, vec2};
use beui_core::input::{
    CursorIcon, DroppedFile, Event, ImeArea, ImeEvent, Key, Modifiers, PointerButton, TouchId,
    TouchPhase,
};
use beui_core::renderer::{Loaded, WindowHandle};
use beui_core::runner::{Adapter, Launch, Platform, Running};
use clipboard::Clipboard;
use file_picker::FilePicker;

const LINE_HEIGHT: f32 = 40.0;
const TOUCH_CURSOR_SIZE: u16 = 20;
const TOUCH_CURSOR_RADIUS: f32 = TOUCH_CURSOR_SIZE as f32 / 2.0;
const TOUCH_CURSOR_STROKE: f32 = 1.0;
const TOUCH_CURSOR_SAMPLES: u16 = 4;
#[cfg(target_os = "linux")]
const EVDEV_BACK: u32 = 158;
const XKB_AUDIO_MIC_MUTE: u32 = 0x1008_ffb2;

enum UserEvent {
    AccessKit(AccessKitEvent),
    Wake,
}

impl From<AccessKitEvent> for UserEvent {
    fn from(event: AccessKitEvent) -> Self {
        Self::AccessKit(event)
    }
}

type Load = Box<dyn FnOnce(Arc<dyn WindowHandle>) -> Result<Vec<Loaded>, Box<dyn Error>>>;

pub struct Winit {
    load: Load,
}

impl Winit {
    pub fn new(
        load: impl FnOnce(Arc<dyn WindowHandle>) -> Result<Vec<Loaded>, Box<dyn Error>> + 'static,
    ) -> Self {
        Self {
            load: Box::new(load),
        }
    }
}

impl Adapter for Winit {
    fn name(&self) -> &'static str {
        "winit"
    }

    fn run(self: Box<Self>, launch: Launch) -> Running {
        Box::pin(async move { run(launch, self.load) })
    }
}

fn run(launch: Launch, load: Load) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let size = launch.options.size;
    #[cfg(all(unix, not(target_os = "macos")))]
    let app_id = launch.options.app_id.clone();
    launch.context.set_test_ids_published(false);
    let mut runner = Runner {
        runner: beui_core::runner::Runner::new(launch),
        size,
        #[cfg(all(unix, not(target_os = "macos")))]
        app_id,
        load: Some(load),
        surface: None,
        modifiers: Modifiers::NONE,
        pointer: Pos2::ZERO,
        emulated_touch: false,
        held_buttons: 0,
        pointer_left: false,
        error: None,
        next_update: None,
        prepared: false,
        clipboard: Clipboard::new(),
        file_picker: FilePicker::new(),
        event_loop_proxy: event_loop.create_proxy(),
        accessibility_active: false,
    };
    event_loop.run_app(&mut runner)?;
    match runner.error {
        Some(error) => Err(error.into()),
        None => Ok(()),
    }
}

struct Surface {
    window: Arc<Window>,
    pointer_locked: bool,
    touch_cursor: CustomCursor,
    ime: Option<ImeArea>,
    accessibility: AccessKitAdapter,
}

struct Runner {
    runner: beui_core::runner::Runner,
    size: Vec2,
    #[cfg(all(unix, not(target_os = "macos")))]
    app_id: Option<String>,
    load: Option<Load>,
    surface: Option<Surface>,
    modifiers: Modifiers,
    pointer: Pos2,
    emulated_touch: bool,
    held_buttons: u8,
    pointer_left: bool,
    error: Option<String>,
    next_update: Option<Instant>,
    prepared: bool,
    clipboard: Clipboard,
    file_picker: FilePicker,
    event_loop_proxy: EventLoopProxy<UserEvent>,
    accessibility_active: bool,
}

struct WinitPlatform<'a> {
    surface: &'a mut Surface,
    clipboard: &'a mut Clipboard,
    file_picker: &'a FilePicker,
    proxy: &'a EventLoopProxy<UserEvent>,
    accessibility_active: bool,
}

impl Platform for WinitPlatform<'_> {
    fn copy(&mut self, text: String) {
        self.clipboard.set(text);
    }

    fn paste(&mut self) -> Option<String> {
        let text = self.clipboard.get();
        if text.is_some() {
            self.surface.window.request_redraw();
        }
        text
    }

    fn pick_file(&mut self, request: FilePickRequest) {
        let proxy = self.proxy.clone();
        self.file_picker.open(request, move || {
            let _ = proxy.send_event(UserEvent::Wake);
        });
    }

    fn set_cursor(&mut self, icon: CursorIcon, touch_emulation: bool) {
        let window = &self.surface.window;
        if touch_emulation {
            window.set_cursor(self.surface.touch_cursor.clone());
            window.set_cursor_visible(true);
            return;
        }
        match cursor(icon) {
            Some(icon) => {
                window.set_cursor(icon);
                window.set_cursor_visible(!self.surface.pointer_locked);
            }
            None => window.set_cursor_visible(false),
        }
    }

    fn lock_pointer(&mut self, locked: bool) {
        self.surface.pointer_locked = locked;
        lock_pointer(&self.surface.window, locked);
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.surface
            .window
            .set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
    }

    fn show_ime(&mut self, ime: Option<&ImeArea>) {
        let surface = &mut *self.surface;
        if ime == surface.ime.as_ref() {
            return;
        }
        if ime.is_some() != surface.ime.is_some() {
            surface.window.set_ime_allowed(ime.is_some());
        }
        if let Some(area) = ime {
            surface.window.set_ime_cursor_area(
                LogicalPosition::new(area.cursor.min.x, area.cursor.min.y),
                LogicalSize::new(area.cursor.width().max(1.0), area.cursor.height().max(1.0)),
            );
        }
        surface.ime = ime.cloned();
    }

    fn publish_accessibility(&mut self, tree: &mut dyn FnMut() -> TreeUpdate) {
        if self.accessibility_active {
            self.surface.accessibility.update_if_active(tree);
        }
    }
}

impl Runner {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        self.exit(event_loop);
    }

    fn exit(&mut self, event_loop: &ActiveEventLoop) {
        self.runner.exit();
        event_loop.exit();
    }

    fn push(&mut self, event: Event) {
        self.runner.push(event);
    }

    fn context(&self) -> &Context {
        self.runner.context()
    }

    fn request_redraw(&self) {
        if let Some(surface) = &self.surface {
            surface.window.request_redraw();
        }
    }

    fn scale_factor(&self) -> f64 {
        let native = self
            .surface
            .as_ref()
            .map_or(1.0, |surface| surface.window.scale_factor());
        let context = self.context();
        context
            .simulated_pixels_per_point()
            .map_or(native * f64::from(context.zoom_factor()), f64::from)
    }

    fn logical(&self, position: PhysicalPosition<f64>) -> Pos2 {
        let scale = self.scale_factor();
        pos2((position.x / scale) as f32, (position.y / scale) as f32)
    }

    fn update(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(surface) = &mut self.surface else {
            return false;
        };
        self.file_picker.deliver(self.runner.context());
        let pixels_per_point = surface.window.scale_factor() as f32;
        let mut platform = WinitPlatform {
            surface,
            clipboard: &mut self.clipboard,
            file_picker: &self.file_picker,
            proxy: &self.event_loop_proxy,
            accessibility_active: self.accessibility_active,
        };
        let Some(frame) = self
            .runner
            .frame(&mut platform, pixels_per_point, SafeArea::default())
        else {
            self.next_update = None;
            return false;
        };
        self.next_update = Instant::now().checked_add(frame.repaint_after);
        if frame.close_requested {
            self.exit(event_loop);
        }
        frame.pending
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        if !std::mem::take(&mut self.prepared) || self.runner.has_events() {
            self.update(event_loop);
        }
        if self.runner.present() {
            self.request_redraw();
        }
    }

    fn attach(&mut self) -> Result<(), Box<dyn Error>> {
        let (Some(surface), Some(renderers)) = (&self.surface, self.runner.renderers()) else {
            return Ok(());
        };
        if renderers.attached() {
            return Ok(());
        }
        let size = surface.window.inner_size();
        renderers.attach(surface.window.clone(), size.width, size.height)?;
        surface.window.request_redraw();
        Ok(())
    }
}

impl ApplicationHandler<UserEvent> for Runner {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if (self.runner.has_events()
            || self
                .next_update
                .is_some_and(|deadline| deadline <= Instant::now()))
            && self.update(event_loop)
        {
            self.prepared = true;
            self.request_redraw();
        }
        event_loop.set_control_flow(match self.next_update {
            Some(deadline) => ControlFlow::WaitUntil(deadline),
            None => ControlFlow::Wait,
        });
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() {
            if let Err(error) = self.attach() {
                self.fail(event_loop, error);
            }
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(self.runner.title().to_owned())
            .with_visible(false)
            .with_inner_size(LogicalSize::new(self.size.x, self.size.y));
        #[cfg(all(unix, not(target_os = "macos")))]
        let attributes = match &self.app_id {
            Some(app_id) => {
                use winit::platform::wayland::WindowAttributesExtWayland;
                WindowAttributesExtWayland::with_name(attributes, app_id, app_id)
            }
            None => attributes,
        };
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => return self.fail(event_loop, error),
        };
        let accessibility = AccessKitAdapter::with_event_loop_proxy(
            event_loop,
            &window,
            self.event_loop_proxy.clone(),
        );
        let touch_cursor = event_loop.create_custom_cursor(touch_cursor_source());
        window.set_visible(true);
        let Some(load) = self.load.take() else {
            return;
        };
        let loaded = match load(window.clone()) {
            Ok(loaded) => loaded,
            Err(error) => return self.fail(event_loop, error),
        };
        let proxy = self.event_loop_proxy.clone();
        let mut setup = Setup::new(Waker::new(move || {
            let _ = proxy.send_event(UserEvent::Wake);
        }));
        setup.provide(window.clone());
        self.surface = Some(Surface {
            window,
            pointer_locked: false,
            touch_cursor,
            ime: None,
            accessibility,
        });
        if let Err(error) = self.runner.start(loaded, setup) {
            return self.fail(event_loop, error);
        }
        if let Err(error) = self.attach() {
            self.fail(event_loop, error);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(renderers) = self.runner.renderers() {
            renderers.detach();
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        self.prepared = false;
        let event = match event {
            UserEvent::Wake => {
                self.request_redraw();
                return;
            }
            UserEvent::AccessKit(event) => event,
        };
        let Some(surface) = &self.surface else {
            return;
        };
        if event.window_id != surface.window.id() {
            return;
        }
        match event.window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                self.accessibility_active = true;
                self.context().reset_accessibility();
                self.request_redraw();
            }
            accesskit_winit::WindowEvent::ActionRequested(request) => {
                self.runner.context().accessibility_action(request);
                surface.window.request_redraw();
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                self.accessibility_active = false;
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = self.surface.as_ref().map(|surface| surface.window.id());
        if window != Some(window_id) {
            return;
        }
        if let Some(surface) = &mut self.surface {
            surface.accessibility.process_event(&surface.window, &event);
        }
        if !matches!(event, WindowEvent::RedrawRequested) {
            self.prepared = false;
        }
        match event {
            WindowEvent::CloseRequested => {
                if self.runner.close_requested() {
                    self.exit(event_loop);
                } else {
                    self.request_redraw();
                }
            }
            WindowEvent::Destroyed => self.exit(event_loop),
            WindowEvent::Resized(size) => {
                if let (Some(surface), Some(renderers)) = (&self.surface, self.runner.renderers()) {
                    renderers.resize(size.width, size.height);
                    surface.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => self.request_redraw(),
            WindowEvent::Focused(focused) => {
                if !focused {
                    self.emulated_touch = false;
                    self.held_buttons = 0;
                    self.pointer_left = false;
                }
                self.push(Event::Focus(focused));
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                let command = cfg!(target_os = "macos");
                self.modifiers = Modifiers {
                    alt: state.alt_key(),
                    ctrl: state.control_key() || (command && state.super_key()),
                    shift: state.shift_key(),
                    logo: !command && state.super_key(),
                };
                self.push(Event::Modifiers(self.modifiers));
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = self.logical(position);
                if self.context().touch_emulation() {
                    if self.emulated_touch {
                        self.push(emulated_touch(TouchPhase::Move, self.pointer));
                    }
                } else {
                    self.push(Event::PointerMoved(self.pointer));
                }
            }
            WindowEvent::CursorEntered { .. } => self.pointer_left = false,
            WindowEvent::CursorLeft { .. } => {
                if self.emulated_touch || self.held_buttons != 0 {
                    self.pointer_left = true;
                } else {
                    self.push(Event::PointerGone);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                let bit = mouse_button_bit(button);
                if pressed {
                    self.held_buttons |= bit;
                } else {
                    self.held_buttons &= !bit;
                }
                let released_outside = !pressed && self.pointer_left && self.held_buttons == 0;
                if released_outside {
                    self.pointer_left = false;
                }
                if self.context().touch_emulation() {
                    if button != MouseButton::Left {
                        return;
                    }
                    if pressed != self.emulated_touch {
                        self.emulated_touch = pressed;
                        self.push(emulated_touch(
                            if pressed {
                                TouchPhase::Start
                            } else {
                                TouchPhase::End
                            },
                            self.pointer,
                        ));
                    }
                    return;
                }
                let Some(button) = pointer_button(button) else {
                    return;
                };
                self.push(Event::PointerButton {
                    pos: self.pointer,
                    button,
                    pressed,
                    modifiers: self.modifiers,
                });
                if released_outside {
                    self.push(Event::PointerGone);
                }
            }
            WindowEvent::Touch(touch) => {
                self.push(Event::Touch {
                    id: TouchId {
                        device: hash(touch.device_id),
                        finger: touch.id,
                    },
                    phase: touch_phase(touch.phase),
                    pos: self.logical(touch.location),
                    force: touch.force.map(touch_force),
                });
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => vec2(x * LINE_HEIGHT, y * LINE_HEIGHT),
                    MouseScrollDelta::PixelDelta(position) => self.logical(position).to_vec2(),
                };
                if delta != Vec2::ZERO {
                    self.push(Event::Scroll(delta));
                }
                if cfg!(target_os = "linux") && phase == winit::event::TouchPhase::Ended {
                    self.push(Event::ScrollEnded);
                }
            }
            WindowEvent::PinchGesture { delta, .. } => {
                self.push(Event::Zoom(1.0 + delta as f32));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                #[cfg(target_os = "linux")]
                if !event.repeat {
                    use winit::platform::scancode::PhysicalKeyExtScancode;
                    if let Some(code) = event.physical_key.to_scancode() {
                        self.push(Event::PhysicalKey { code, pressed });
                    }
                }
                let named = match logical_key(&event.key_without_modifiers()) {
                    Logical::Key(key) => Some(key),
                    Logical::Ignored => None,
                    Logical::Unknown => match event.physical_key {
                        PhysicalKey::Code(code) => key(code),
                        PhysicalKey::Unidentified(_) => unidentified_key(event.physical_key),
                    },
                };
                if pressed
                    && named == Some(Key::V)
                    && self.modifiers.ctrl
                    && !self.modifiers.alt
                    && let Some(text) = self.clipboard.get()
                {
                    self.push(Event::Text(text));
                }
                if let Some(key) = named.filter(|key| !(event.repeat && key.is_modifier())) {
                    self.push(Event::Key {
                        key,
                        pressed,
                        repeat: event.repeat,
                        modifiers: self.modifiers,
                    });
                }
                if pressed
                    && !self.modifiers.command()
                    && let Some(text) = event.text
                    && !text.chars().any(char::is_control)
                {
                    self.push(Event::Text(text.to_string()));
                }
            }
            WindowEvent::Ime(Ime::Enabled) => self.push(Event::Ime(ImeEvent::Enabled)),
            WindowEvent::Ime(Ime::Preedit(text, _)) => {
                self.push(Event::Ime(ImeEvent::SetComposingText(text)));
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.push(Event::Ime(ImeEvent::CommitText(text)))
            }
            WindowEvent::Ime(Ime::Disabled) => {
                self.push(Event::Ime(ImeEvent::SetComposingText(String::new())));
                self.push(Event::Ime(ImeEvent::Disabled));
            }
            WindowEvent::HoveredFile(_) => self.push(Event::FileHovered),
            WindowEvent::HoveredFileCancelled => self.push(Event::FileHoverCancelled),
            WindowEvent::DroppedFile(path) => {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_owned();
                self.push(Event::FileDropped(DroppedFile {
                    name,
                    path: Some(path),
                    bytes: None,
                }));
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device: DeviceId,
        event: DeviceEvent,
    ) {
        let DeviceEvent::MouseMotion { delta } = event else {
            return;
        };
        if !self.context().pointer_locked() {
            return;
        }
        let scale = self.scale_factor() as f32;
        self.push(Event::PointerMotion(vec2(
            delta.0 as f32 / scale,
            delta.1 as f32 / scale,
        )));
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.exit(event_loop);
    }
}

fn hash(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn emulated_touch(phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch {
        id: TouchId {
            device: 0,
            finger: 0,
        },
        phase,
        pos,
        force: None,
    }
}

fn mouse_button_bit(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 1,
        MouseButton::Right => 2,
        MouseButton::Middle => 4,
        MouseButton::Back => 8,
        MouseButton::Forward => 16,
        MouseButton::Other(_) => 32,
    }
}

fn touch_phase(phase: winit::event::TouchPhase) -> TouchPhase {
    match phase {
        winit::event::TouchPhase::Started => TouchPhase::Start,
        winit::event::TouchPhase::Moved => TouchPhase::Move,
        winit::event::TouchPhase::Ended => TouchPhase::End,
        winit::event::TouchPhase::Cancelled => TouchPhase::Cancel,
    }
}

fn touch_force(force: winit::event::Force) -> f32 {
    match force {
        winit::event::Force::Normalized(force) => force as f32,
        winit::event::Force::Calibrated {
            force,
            max_possible_force,
            ..
        } => (force / max_possible_force) as f32,
    }
}

fn touch_cursor_source() -> CustomCursorSource {
    let size = usize::from(TOUCH_CURSOR_SIZE);
    let mut rgba = vec![0; size * size * 4];
    let samples = f32::from(TOUCH_CURSOR_SAMPLES);
    let sample_count = f32::from(TOUCH_CURSOR_SAMPLES * TOUCH_CURSOR_SAMPLES);
    for y in 0..TOUCH_CURSOR_SIZE {
        for x in 0..TOUCH_CURSOR_SIZE {
            let mut alpha = 0.0;
            let mut white = 0.0;
            for sample_y in 0..TOUCH_CURSOR_SAMPLES {
                for sample_x in 0..TOUCH_CURSOR_SAMPLES {
                    let x = f32::from(x) + (f32::from(sample_x) + 0.5) / samples;
                    let y = f32::from(y) + (f32::from(sample_y) + 0.5) / samples;
                    let distance = (x - TOUCH_CURSOR_RADIUS).hypot(y - TOUCH_CURSOR_RADIUS);
                    if distance <= TOUCH_CURSOR_RADIUS {
                        if distance <= TOUCH_CURSOR_RADIUS - TOUCH_CURSOR_STROKE {
                            alpha += 96.0;
                            white += 96.0;
                        } else {
                            alpha += 255.0;
                        }
                    }
                }
            }
            let offset = (usize::from(y) * size + usize::from(x)) * 4;
            let alpha = alpha / sample_count;
            let color = if alpha == 0.0 {
                0.0
            } else {
                white / sample_count / alpha * 255.0
            };
            rgba[offset..offset + 3].fill(color.round() as u8);
            rgba[offset + 3] = alpha.round() as u8;
        }
    }
    CustomCursor::from_rgba(
        rgba,
        TOUCH_CURSOR_SIZE,
        TOUCH_CURSOR_SIZE,
        TOUCH_CURSOR_SIZE / 2,
        TOUCH_CURSOR_SIZE / 2,
    )
    .expect("the touch cursor dimensions and hotspot are valid")
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Back => Some(PointerButton::Back),
        MouseButton::Forward => Some(PointerButton::Forward),
        MouseButton::Other(_) => None,
    }
}

fn cursor(icon: CursorIcon) -> Option<winit::window::CursorIcon> {
    Some(match icon {
        CursorIcon::Default => winit::window::CursorIcon::Default,
        CursorIcon::Crosshair => winit::window::CursorIcon::Crosshair,
        CursorIcon::Grab => winit::window::CursorIcon::Grab,
        CursorIcon::Grabbing => winit::window::CursorIcon::Grabbing,
        CursorIcon::NotAllowed => winit::window::CursorIcon::NotAllowed,
        CursorIcon::PointingHand => winit::window::CursorIcon::Pointer,
        CursorIcon::ResizeHorizontal => winit::window::CursorIcon::EwResize,
        CursorIcon::ResizeVertical => winit::window::CursorIcon::NsResize,
        CursorIcon::ResizeNeSw => winit::window::CursorIcon::NeswResize,
        CursorIcon::ResizeNwSe => winit::window::CursorIcon::NwseResize,
        CursorIcon::Text => winit::window::CursorIcon::Text,
        CursorIcon::Wait => winit::window::CursorIcon::Wait,
        CursorIcon::Move => winit::window::CursorIcon::Move,
        CursorIcon::Progress => winit::window::CursorIcon::Progress,
        CursorIcon::Help => winit::window::CursorIcon::Help,
        CursorIcon::Alias => winit::window::CursorIcon::Alias,
        CursorIcon::None => return None,
    })
}

#[cfg(target_os = "linux")]
fn unidentified_key(physical: PhysicalKey) -> Option<Key> {
    use winit::platform::scancode::PhysicalKeyExtScancode;
    match physical.to_scancode() {
        Some(EVDEV_BACK) => Some(Key::BrowserBack),
        _ => None,
    }
}

#[cfg(not(target_os = "linux"))]
fn unidentified_key(_: PhysicalKey) -> Option<Key> {
    None
}

enum Logical {
    Key(Key),
    Ignored,
    Unknown,
}

fn logical_key(logical: &winit::keyboard::Key) -> Logical {
    use winit::keyboard::Key as Winit;
    let named = match logical {
        Winit::Named(named) => named_key(*named),
        Winit::Character(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => character_key(c),
                _ => None,
            }
        }
        Winit::Unidentified(winit::keyboard::NativeKey::Xkb(XKB_AUDIO_MIC_MUTE)) => {
            Some(Key::MicMute)
        }
        Winit::Unidentified(_) | Winit::Dead(_) => None,
    };
    match (named, logical) {
        (Some(key), _) => Logical::Key(key),
        (None, Winit::Named(_)) => Logical::Ignored,
        (None, _) => Logical::Unknown,
    }
}

fn named_key(named: NamedKey) -> Option<Key> {
    let key = match named {
        NamedKey::ArrowDown => Key::ArrowDown,
        NamedKey::ArrowLeft => Key::ArrowLeft,
        NamedKey::ArrowRight => Key::ArrowRight,
        NamedKey::ArrowUp => Key::ArrowUp,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Delete => Key::Delete,
        NamedKey::End => Key::End,
        NamedKey::Enter => Key::Enter,
        NamedKey::Escape => Key::Escape,
        NamedKey::Home => Key::Home,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::Space => Key::Space,
        NamedKey::Tab => Key::Tab,
        NamedKey::Insert => Key::Insert,
        NamedKey::BrowserBack => Key::BrowserBack,
        NamedKey::F1 => Key::F1,
        NamedKey::F2 => Key::F2,
        NamedKey::F3 => Key::F3,
        NamedKey::F4 => Key::F4,
        NamedKey::F5 => Key::F5,
        NamedKey::F6 => Key::F6,
        NamedKey::F7 => Key::F7,
        NamedKey::F8 => Key::F8,
        NamedKey::F9 => Key::F9,
        NamedKey::F10 => Key::F10,
        NamedKey::F11 => Key::F11,
        NamedKey::F12 => Key::F12,
        NamedKey::F13 => Key::F13,
        NamedKey::F14 => Key::F14,
        NamedKey::F15 => Key::F15,
        NamedKey::F16 => Key::F16,
        NamedKey::F17 => Key::F17,
        NamedKey::F18 => Key::F18,
        NamedKey::F19 => Key::F19,
        NamedKey::F20 => Key::F20,
        NamedKey::F21 => Key::F21,
        NamedKey::F22 => Key::F22,
        NamedKey::F23 => Key::F23,
        NamedKey::F24 => Key::F24,
        NamedKey::AudioVolumeUp => Key::VolumeUp,
        NamedKey::AudioVolumeDown => Key::VolumeDown,
        NamedKey::AudioVolumeMute => Key::VolumeMute,
        NamedKey::MicrophoneVolumeMute => Key::MicMute,
        NamedKey::BrightnessUp => Key::BrightnessUp,
        NamedKey::BrightnessDown => Key::BrightnessDown,
        NamedKey::MediaPlayPause | NamedKey::MediaPlay | NamedKey::MediaPause => {
            Key::MediaPlayPause
        }
        NamedKey::MediaTrackNext => Key::MediaNext,
        NamedKey::MediaTrackPrevious => Key::MediaPrevious,
        NamedKey::MediaStop => Key::MediaStop,
        NamedKey::Shift => Key::Shift,
        NamedKey::Control => Key::Ctrl,
        NamedKey::Alt => Key::Alt,
        NamedKey::Super if cfg!(target_os = "macos") => Key::Ctrl,
        NamedKey::Super => Key::Logo,
        _ => return None,
    };
    Some(key)
}

fn character_key(c: char) -> Option<Key> {
    let key = match c.to_ascii_lowercase() {
        '[' | '{' => Key::BracketLeft,
        ']' | '}' => Key::BracketRight,
        '-' | '_' => Key::Minus,
        '=' | '+' => Key::Plus,
        ' ' => Key::Space,
        '0' | ')' => Key::Zero,
        '1' | '!' => Key::One,
        '2' | '@' => Key::Two,
        '3' | '#' => Key::Three,
        '4' | '$' => Key::Four,
        '5' | '%' => Key::Five,
        '6' | '^' => Key::Six,
        '7' | '&' => Key::Seven,
        '8' | '*' => Key::Eight,
        '9' | '(' => Key::Nine,
        '`' | '~' => Key::Backtick,
        ',' | '<' => Key::Comma,
        '.' | '>' => Key::Period,
        '/' | '?' => Key::Slash,
        '\\' | '|' => Key::Backslash,
        ';' | ':' => Key::Semicolon,
        '\'' | '"' => Key::Quote,
        'a' => Key::A,
        'b' => Key::B,
        'c' => Key::C,
        'd' => Key::D,
        'e' => Key::E,
        'f' => Key::F,
        'g' => Key::G,
        'h' => Key::H,
        'i' => Key::I,
        'j' => Key::J,
        'k' => Key::K,
        'l' => Key::L,
        'm' => Key::M,
        'n' => Key::N,
        'o' => Key::O,
        'p' => Key::P,
        'q' => Key::Q,
        'r' => Key::R,
        's' => Key::S,
        't' => Key::T,
        'u' => Key::U,
        'v' => Key::V,
        'w' => Key::W,
        'x' => Key::X,
        'y' => Key::Y,
        'z' => Key::Z,
        _ => return None,
    };
    Some(key)
}

fn key(code: KeyCode) -> Option<Key> {
    let key = match code {
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::ArrowUp => Key::ArrowUp,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::End => Key::End,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::Home => Key::Home,
        KeyCode::BracketLeft => Key::BracketLeft,
        KeyCode::BracketRight => Key::BracketRight,
        KeyCode::Minus | KeyCode::NumpadSubtract => Key::Minus,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::Equal | KeyCode::NumpadAdd => Key::Plus,
        KeyCode::Space => Key::Space,
        KeyCode::Tab => Key::Tab,
        KeyCode::Digit0 | KeyCode::Numpad0 => Key::Zero,
        KeyCode::Digit1 | KeyCode::Numpad1 => Key::One,
        KeyCode::Digit2 | KeyCode::Numpad2 => Key::Two,
        KeyCode::Digit3 | KeyCode::Numpad3 => Key::Three,
        KeyCode::Digit4 | KeyCode::Numpad4 => Key::Four,
        KeyCode::Digit5 | KeyCode::Numpad5 => Key::Five,
        KeyCode::Digit6 | KeyCode::Numpad6 => Key::Six,
        KeyCode::Digit7 | KeyCode::Numpad7 => Key::Seven,
        KeyCode::Digit8 | KeyCode::Numpad8 => Key::Eight,
        KeyCode::Digit9 | KeyCode::Numpad9 => Key::Nine,
        KeyCode::Backquote => Key::Backtick,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyB => Key::B,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyH => Key::H,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyK => Key::K,
        KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyT => Key::T,
        KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        KeyCode::Insert => Key::Insert,
        KeyCode::Comma => Key::Comma,
        KeyCode::Period | KeyCode::NumpadDecimal => Key::Period,
        KeyCode::Slash | KeyCode::NumpadDivide => Key::Slash,
        KeyCode::Backslash => Key::Backslash,
        KeyCode::Semicolon => Key::Semicolon,
        KeyCode::Quote => Key::Quote,
        KeyCode::BrowserBack => Key::BrowserBack,
        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F4 => Key::F4,
        KeyCode::F5 => Key::F5,
        KeyCode::F6 => Key::F6,
        KeyCode::F7 => Key::F7,
        KeyCode::F8 => Key::F8,
        KeyCode::F9 => Key::F9,
        KeyCode::F10 => Key::F10,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        KeyCode::F13 => Key::F13,
        KeyCode::F14 => Key::F14,
        KeyCode::F15 => Key::F15,
        KeyCode::F16 => Key::F16,
        KeyCode::F17 => Key::F17,
        KeyCode::F18 => Key::F18,
        KeyCode::F19 => Key::F19,
        KeyCode::F20 => Key::F20,
        KeyCode::F21 => Key::F21,
        KeyCode::F22 => Key::F22,
        KeyCode::F23 => Key::F23,
        KeyCode::F24 => Key::F24,
        KeyCode::AudioVolumeUp => Key::VolumeUp,
        KeyCode::AudioVolumeDown => Key::VolumeDown,
        KeyCode::AudioVolumeMute => Key::VolumeMute,
        KeyCode::MediaPlayPause => Key::MediaPlayPause,
        KeyCode::MediaTrackNext => Key::MediaNext,
        KeyCode::MediaTrackPrevious => Key::MediaPrevious,
        KeyCode::MediaStop => Key::MediaStop,
        _ => return None,
    };
    Some(key)
}

fn lock_pointer(window: &Window, locked: bool) {
    let grab = match locked {
        true => CursorGrabMode::Locked,
        false => CursorGrabMode::None,
    };
    if window.set_cursor_grab(grab).is_err() && locked {
        let _ = window.set_cursor_grab(CursorGrabMode::Confined);
    }
    window.set_cursor_visible(!locked);
}
