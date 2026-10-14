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
use winit::keyboard::{NamedKey, PhysicalKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{
    CursorGrabMode, Fullscreen, Window, WindowId,
};

use accesskit::TreeUpdate;
use beui_core::app::{SafeArea, Setup, Waker};
use beui_core::clipboard::{PasteAnswer, PasteKind, Pasted};
use beui_core::file_picker::{FilePickAnswer, FilePickRequest};
use beui_core::geometry::{Pos2, Vec2, pos2, vec2};
use beui_core::input::{
    CursorIcon, DroppedFile, Event, ImeEvent, Key, Modifiers, PointerButton, TouchId, TouchPhase,
};
use beui_core::renderer::{Loaded, WindowHandle};
use beui_core::runner::{Adapter, Input, Keystroke, Launch, Platform, Running, ScrollDelta};
use beui_core::window::WindowState;
use clipboard::Clipboard;

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
    let mut runner = Runner {
        runner: beui_core::runner::Runner::new(launch),
        size,
        #[cfg(all(unix, not(target_os = "macos")))]
        app_id,
        load: Some(load),
        surface: None,
        pointer: Pos2::ZERO,
        error: None,
        next_update: None,
        prepared: false,
        clipboard: Clipboard::new(),
        proxy: event_loop.create_proxy(),
    };
    event_loop.run_app(&mut runner)?;
    match runner.error {
        Some(error) => Err(error.into()),
        None => runner.runner.finish(),
    }
}

struct Surface {
    window: Arc<Window>,
    fullscreen: bool,
    accessibility: AccessKitAdapter,
}

struct Runner {
    runner: beui_core::runner::Runner,
    size: Vec2,
    #[cfg(all(unix, not(target_os = "macos")))]
    app_id: Option<String>,
    load: Option<Load>,
    surface: Option<Surface>,
    pointer: Pos2,
    error: Option<String>,
    next_update: Option<Instant>,
    prepared: bool,
    clipboard: Clipboard,
    proxy: EventLoopProxy<UserEvent>,
}

struct WinitPlatform<'a> {
    surface: &'a mut Surface,
    clipboard: &'a mut Clipboard,
}

impl Platform for WinitPlatform<'_> {
    fn copy(&mut self, text: String) {
        self.clipboard.set(text);
    }

    fn paste(&mut self, kinds: &[PasteKind], answer: PasteAnswer) {
        answer.send(kinds.iter().find_map(|kind| {
            match kind {
                PasteKind::Text => self
                    .clipboard
                    .get()
                    .filter(|text| !text.is_empty())
                    .map(Pasted::Text),
                PasteKind::Image => self.clipboard.image().map(Pasted::Image),
            }
        }));
    }

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        file_picker::open(request, answer);
    }

    fn show_window(&mut self, window: &WindowState, shown: &WindowState) {
        let surface = &*self.surface;
        if window.pointer_locked != shown.pointer_locked {
            lock_pointer(&surface.window, window.pointer_locked);
        }
        if (window.cursor, window.pointer_locked) != (shown.cursor, shown.pointer_locked) {
            show_cursor(surface, window);
        }
        if window.ime != shown.ime {
            if window.ime.is_some() != shown.ime.is_some() {
                surface.window.set_ime_allowed(window.ime.is_some());
            }
            if let Some(area) = &window.ime {
                surface.window.set_ime_cursor_area(
                    LogicalPosition::new(area.cursor.min.x, area.cursor.min.y),
                    LogicalSize::new(area.cursor.width().max(1.0), area.cursor.height().max(1.0)),
                );
            }
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.surface
            .window
            .set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
    }

    fn publish_accessibility(&mut self, tree: &mut dyn FnMut() -> TreeUpdate) {
        self.surface.accessibility.update_if_active(tree);
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

    fn push(&mut self, input: impl Into<Input>) {
        self.runner.push(input);
    }

    fn request_redraw(&self) {
        if let Some(surface) = &self.surface {
            surface.window.request_redraw();
        }
    }

    fn update(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(surface) = &mut self.surface else {
            return false;
        };
        let mut platform = WinitPlatform {
            surface,
            clipboard: &mut self.clipboard,
        };
        let Some(frame) = self.runner.frame(&mut platform, SafeArea::default()) else {
            self.next_update = None;
            if self.runner.exited() {
                event_loop.exit();
            }
            return false;
        };
        self.next_update = frame
            .wait()
            .and_then(|wait| Instant::now().checked_add(wait));
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
        let proxy = self.proxy.clone();
        let accessibility =
            AccessKitAdapter::with_event_loop_proxy(event_loop, &window, proxy.clone());
        window.set_visible(true);
        let Some(load) = self.load.take() else {
            return;
        };
        let loaded = match load(window.clone()) {
            Ok(loaded) => loaded,
            Err(error) => return self.fail(event_loop, error),
        };
        self.runner.reports().scaled(window.scale_factor() as f32);
        let mut setup = Setup::new(Waker::new(move || {
            let _ = proxy.send_event(UserEvent::Wake);
        }));
        setup.provide(window.clone());
        self.surface = Some(Surface {
            window,
            fullscreen: false,
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
                self.runner.accessibility_requested();
                surface.window.request_redraw();
            }
            accesskit_winit::WindowEvent::ActionRequested(request) => {
                self.runner.accessibility_action(request);
                surface.window.request_redraw();
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                self.runner.accessibility_deactivated();
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
            WindowEvent::CloseRequested => match self.runner.os_close() {
                true => event_loop.exit(),
                false => self.request_redraw(),
            },
            WindowEvent::Destroyed => self.exit(event_loop),
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    let reports = self.runner.reports();
                    let fullscreen = surface.window.fullscreen().is_some();
                    if std::mem::replace(&mut surface.fullscreen, fullscreen) != fullscreen {
                        reports.fullscreen(fullscreen);
                    }
                    reports.resized(size.width, size.height);
                    surface.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.runner.reports().scaled(scale_factor as f32);
                self.request_redraw();
            }
            WindowEvent::Focused(focused) => self.push(Event::Focus(focused)),
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.push(Event::Modifiers(Modifiers::with_meta(
                    state.alt_key(),
                    state.control_key(),
                    state.shift_key(),
                    state.super_key(),
                    cfg!(target_os = "macos"),
                )));
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = physical(position);
                self.push(Event::PointerMoved(self.pointer));
            }
            WindowEvent::CursorLeft { .. } => self.push(Event::PointerGone),
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button) = pointer_button(button) {
                    self.runner.pointer_button(
                        self.pointer,
                        button,
                        state == ElementState::Pressed,
                    );
                }
            }
            WindowEvent::Touch(touch) => {
                self.push(Event::Touch {
                    id: TouchId {
                        device: hash(touch.device_id),
                        finger: touch.id,
                    },
                    phase: touch_phase(touch.phase),
                    pos: physical(touch.location),
                    force: touch.force.map(touch_force),
                });
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                self.push(match delta {
                    MouseScrollDelta::LineDelta(x, y) => ScrollDelta::Lines(vec2(x, y)),
                    MouseScrollDelta::PixelDelta(position) => {
                        ScrollDelta::Pixels(physical(position).to_vec2())
                    }
                });
                if cfg!(target_os = "linux") && phase == winit::event::TouchPhase::Ended {
                    self.push(Event::ScrollEnded);
                }
            }
            WindowEvent::PinchGesture { delta, .. } => {
                self.push(Event::Zoom(1.0 + delta as f32));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                #[cfg(target_os = "linux")]
                let code = {
                    use winit::platform::scancode::PhysicalKeyExtScancode;
                    event.physical_key.to_scancode()
                };
                #[cfg(not(target_os = "linux"))]
                let code = None;
                let key = match logical_key(&event.key_without_modifiers()) {
                    Logical::Key(key) => Some(key),
                    Logical::Ignored => None,
                    Logical::Unknown => match event.physical_key {
                        PhysicalKey::Code(code) => Key::from_code(&format!("{code:?}")),
                        PhysicalKey::Unidentified(_) => unidentified_key(event.physical_key),
                    },
                };
                self.push(Keystroke {
                    key,
                    pressed: event.state == ElementState::Pressed,
                    repeat: event.repeat,
                    code,
                    text: event.text.map(|text| text.to_string()),
                });
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
        if !self.runner.window().pointer_locked {
            return;
        }
        self.push(Event::PointerMotion(vec2(delta.0 as f32, delta.1 as f32)));
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

fn physical(position: PhysicalPosition<f64>) -> Pos2 {
    pos2(position.x as f32, position.y as f32)
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
        CursorIcon::ContextMenu => winit::window::CursorIcon::ContextMenu,
        CursorIcon::Cell => winit::window::CursorIcon::Cell,
        CursorIcon::VerticalText => winit::window::CursorIcon::VerticalText,
        CursorIcon::Copy => winit::window::CursorIcon::Copy,
        CursorIcon::NoDrop => winit::window::CursorIcon::NoDrop,
        CursorIcon::AllScroll => winit::window::CursorIcon::AllScroll,
        CursorIcon::ZoomIn => winit::window::CursorIcon::ZoomIn,
        CursorIcon::ZoomOut => winit::window::CursorIcon::ZoomOut,
        CursorIcon::ResizeColumn => winit::window::CursorIcon::ColResize,
        CursorIcon::ResizeRow => winit::window::CursorIcon::RowResize,
        CursorIcon::ResizeNorth => winit::window::CursorIcon::NResize,
        CursorIcon::ResizeEast => winit::window::CursorIcon::EResize,
        CursorIcon::ResizeSouth => winit::window::CursorIcon::SResize,
        CursorIcon::ResizeWest => winit::window::CursorIcon::WResize,
        CursorIcon::ResizeNorthEast => winit::window::CursorIcon::NeResize,
        CursorIcon::ResizeNorthWest => winit::window::CursorIcon::NwResize,
        CursorIcon::ResizeSouthEast => winit::window::CursorIcon::SeResize,
        CursorIcon::ResizeSouthWest => winit::window::CursorIcon::SwResize,
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
                (Some(c), None) => Key::from_char(c),
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
        NamedKey::Super => Key::meta(cfg!(target_os = "macos")),
        _ => return None,
    };
    Some(key)
}

fn show_cursor(surface: &Surface, state: &WindowState) {
    let window = &surface.window;
    match cursor(state.cursor) {
        Some(icon) => {
            window.set_cursor(icon);
            window.set_cursor_visible(!state.pointer_locked);
        }
        None => window.set_cursor_visible(false),
    }
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
